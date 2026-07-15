use std::{
    f64::consts::{PI, TAU},
    sync::Arc,
};

use frames::{FrameOrigin, InertialFrame, ReferenceFrame};
use hifitime::Epoch;
use thiserror::Error;
use units::uom::si::{angle::radian, length::meter, ratio::ratio};
use units::{Angle, Length, Position, Ratio, Velocity, VelocityVector};

use gravity::SharedCentralGravity;
use orskit_core::SpacecraftState as SpacecraftStateContract;

use crate::cartesian::{CartesianCoordinates, FramedPosition, FramedVelocity, KinematicError};

/// Coordinates tied to the epoch at which they are valid.
///
/// File formats may provide a timed coordinate sample without the mass,
/// inertia, and attitude required to construct an [`orskit_core::Spacecraft`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoordinateSample<C> {
    epoch: Epoch,
    coordinates: C,
}

impl<C> CoordinateSample<C> {
    /// Associates coordinates with an epoch.
    #[must_use]
    pub const fn new(epoch: Epoch, coordinates: C) -> Self {
        Self { epoch, coordinates }
    }

    /// Returns the sample epoch.
    #[must_use]
    pub const fn epoch(&self) -> Epoch {
        self.epoch
    }

    /// Returns the sampled coordinates.
    #[must_use]
    pub const fn coordinates(&self) -> &C {
        &self.coordinates
    }
}

/// Cartesian orbital state `(x, y, z, vx, vy, vz)` in one reference frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CartesianState {
    frame: ReferenceFrame,
    position: Position,
    velocity: VelocityVector,
}

impl CartesianState {
    /// Constructs a finite Cartesian state.
    pub fn new(
        frame: ReferenceFrame,
        position: Position,
        velocity: VelocityVector,
    ) -> Result<Self, StateError> {
        if !position.is_finite() {
            return Err(StateError::NonFiniteCartesianPosition);
        }
        if !velocity.is_finite() {
            return Err(StateError::NonFiniteCartesianVelocity);
        }
        Ok(Self {
            frame,
            position,
            velocity,
        })
    }

    /// Returns the coordinate frame.
    #[must_use]
    pub const fn frame(self) -> ReferenceFrame {
        self.frame
    }

    /// Returns `(x, y, z)`.
    #[must_use]
    pub const fn position(self) -> Position {
        self.position
    }

    /// Returns `(vx, vy, vz)`.
    #[must_use]
    pub const fn velocity(self) -> VelocityVector {
        self.velocity
    }

    /// Returns the scalar speed.
    #[must_use]
    pub fn speed(self) -> Velocity {
        self.velocity.speed()
    }
}

impl TryFrom<CartesianCoordinates> for CartesianState {
    type Error = StateError;

    fn try_from(coordinates: CartesianCoordinates) -> Result<Self, Self::Error> {
        let position = coordinates.position();
        let velocity = coordinates.velocity();
        if position.frame() != velocity.frame() {
            return Err(StateError::MismatchedCartesianFrames);
        }
        Self::new(position.frame(), position.value(), velocity.value())
    }
}

impl From<CartesianState> for CartesianCoordinates {
    fn from(state: CartesianState) -> Self {
        Self::new(
            FramedPosition::new(state.position, state.frame)
                .expect("CartesianState guarantees finite position"),
            FramedVelocity::new(state.velocity, state.frame)
                .expect("CartesianState guarantees finite velocity"),
        )
    }
}

/// Elliptic circular state `(a, ex, ey, i, Omega, alpha_v)`.
///
/// `ex=e cos(omega)`, `ey=e sin(omega)`, and `alpha_v=nu+omega`, where
/// `nu` is true anomaly and `omega` is the argument of periapsis. This
/// representation remains valid at zero eccentricity by treating the true
/// latitude argument as the physically meaningful angle. It supports the
/// elliptic regime `a > 0` and `hypot(ex, ey) < 1`.
#[derive(Debug, Clone)]
pub struct CircularState {
    frame: InertialFrame,
    central_gravity: SharedCentralGravity,
    semi_major_axis: Length,
    eccentricity_x: Ratio,
    eccentricity_y: Ratio,
    inclination: Angle,
    right_ascension_of_ascending_node: Angle,
    true_latitude_argument: Angle,
}

impl PartialEq for CircularState {
    fn eq(&self, other: &Self) -> bool {
        self.frame == other.frame
            && Arc::ptr_eq(&self.central_gravity, &other.central_gravity)
            && self.semi_major_axis == other.semi_major_axis
            && self.eccentricity_x == other.eccentricity_x
            && self.eccentricity_y == other.eccentricity_y
            && self.inclination == other.inclination
            && self.right_ascension_of_ascending_node == other.right_ascension_of_ascending_node
            && self.true_latitude_argument == other.true_latitude_argument
    }
}

impl CircularState {
    /// Constructs and validates elliptic circular elements.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        frame: InertialFrame,
        central_gravity: SharedCentralGravity,
        semi_major_axis: Length,
        eccentricity_x: Ratio,
        eccentricity_y: Ratio,
        inclination: Angle,
        right_ascension_of_ascending_node: Angle,
        true_latitude_argument: Angle,
    ) -> Result<Self, StateError> {
        validate_positive_axis(semi_major_axis)?;
        let eccentricity_x_value = finite_ratio(eccentricity_x, "circular ex")?;
        let eccentricity_y_value = finite_ratio(eccentricity_y, "circular ey")?;
        if !(0.0..1.0).contains(&eccentricity_x_value.hypot(eccentricity_y_value)) {
            return Err(StateError::EccentricityOutOfRange);
        }
        let inclination_value = finite_angle(inclination, "inclination")?;
        if !(0.0..=PI).contains(&inclination_value) {
            return Err(StateError::InclinationOutOfRange);
        }
        finite_angle(
            right_ascension_of_ascending_node,
            "right ascension of ascending node",
        )?;
        finite_angle(true_latitude_argument, "true latitude argument")?;
        validate_gravity_origin(&central_gravity, frame.reference_frame())?;
        Ok(Self {
            frame,
            central_gravity,
            semi_major_axis,
            eccentricity_x,
            eccentricity_y,
            inclination,
            right_ascension_of_ascending_node,
            true_latitude_argument,
        })
    }

    /// Returns the coordinate frame.
    #[must_use]
    pub const fn frame(&self) -> ReferenceFrame {
        self.frame.reference_frame()
    }

    /// Returns the affirmative inertial-frame capability carried by the state.
    #[must_use]
    pub const fn inertial_frame(&self) -> InertialFrame {
        self.frame
    }

    /// Returns the sourced gravity context to which these elements are bound.
    #[must_use]
    pub const fn central_gravity(&self) -> &SharedCentralGravity {
        &self.central_gravity
    }

    /// Returns `a`.
    #[must_use]
    pub const fn semi_major_axis(&self) -> Length {
        self.semi_major_axis
    }

    /// Returns `ex = e cos(omega)`.
    #[must_use]
    pub const fn eccentricity_x(&self) -> Ratio {
        self.eccentricity_x
    }

    /// Returns `ey = e sin(omega)`.
    #[must_use]
    pub const fn eccentricity_y(&self) -> Ratio {
        self.eccentricity_y
    }

    /// Returns `i`.
    #[must_use]
    pub const fn inclination(&self) -> Angle {
        self.inclination
    }

    /// Returns `Omega`.
    #[must_use]
    pub const fn right_ascension_of_ascending_node(&self) -> Angle {
        self.right_ascension_of_ascending_node
    }

    /// Returns `alpha_v = nu + omega`.
    #[must_use]
    pub const fn true_latitude_argument(&self) -> Angle {
        self.true_latitude_argument
    }
}

/// Elliptic osculating Keplerian state `(a, e, i, Omega, omega, nu)`.
///
/// The supported regime is `a > 0` and `0 <= e < 1`. All angles are typed
/// quantities; `nu` is true anomaly. Each state is bound to the identity of
/// the sourced gravity context used to define or interpret its elements.
#[derive(Debug, Clone)]
pub struct KeplerianState {
    frame: InertialFrame,
    central_gravity: SharedCentralGravity,
    semi_major_axis: Length,
    eccentricity: Ratio,
    inclination: Angle,
    right_ascension_of_ascending_node: Angle,
    argument_of_periapsis: Angle,
    true_anomaly: Angle,
}

impl PartialEq for KeplerianState {
    fn eq(&self, other: &Self) -> bool {
        self.frame == other.frame
            && Arc::ptr_eq(&self.central_gravity, &other.central_gravity)
            && self.semi_major_axis == other.semi_major_axis
            && self.eccentricity == other.eccentricity
            && self.inclination == other.inclination
            && self.right_ascension_of_ascending_node == other.right_ascension_of_ascending_node
            && self.argument_of_periapsis == other.argument_of_periapsis
            && self.true_anomaly == other.true_anomaly
    }
}

impl KeplerianState {
    /// Constructs and validates an elliptic osculating Keplerian state.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        frame: InertialFrame,
        central_gravity: SharedCentralGravity,
        semi_major_axis: Length,
        eccentricity: Ratio,
        inclination: Angle,
        right_ascension_of_ascending_node: Angle,
        argument_of_periapsis: Angle,
        true_anomaly: Angle,
    ) -> Result<Self, StateError> {
        ValidatedKeplerian::from_values(
            semi_major_axis,
            eccentricity,
            inclination,
            right_ascension_of_ascending_node,
            argument_of_periapsis,
            true_anomaly,
        )?;
        validate_gravity_origin(&central_gravity, frame.reference_frame())?;
        Ok(Self {
            frame,
            central_gravity,
            semi_major_axis,
            eccentricity,
            inclination,
            right_ascension_of_ascending_node,
            argument_of_periapsis,
            true_anomaly,
        })
    }

    /// Returns the coordinate frame.
    #[must_use]
    pub const fn frame(&self) -> ReferenceFrame {
        self.frame.reference_frame()
    }

    /// Returns the affirmative inertial-frame capability carried by the state.
    #[must_use]
    pub const fn inertial_frame(&self) -> InertialFrame {
        self.frame
    }

    /// Returns the sourced gravity context to which these elements are bound.
    #[must_use]
    pub const fn central_gravity(&self) -> &SharedCentralGravity {
        &self.central_gravity
    }

    /// Returns `a`.
    #[must_use]
    pub const fn semi_major_axis(&self) -> Length {
        self.semi_major_axis
    }

    /// Returns `e`.
    #[must_use]
    pub const fn eccentricity(&self) -> Ratio {
        self.eccentricity
    }

    /// Returns `i`.
    #[must_use]
    pub const fn inclination(&self) -> Angle {
        self.inclination
    }

    /// Returns `Omega`.
    #[must_use]
    pub const fn right_ascension_of_ascending_node(&self) -> Angle {
        self.right_ascension_of_ascending_node
    }

    /// Returns `omega`.
    #[must_use]
    pub const fn argument_of_periapsis(&self) -> Angle {
        self.argument_of_periapsis
    }

    /// Returns `nu`.
    #[must_use]
    pub const fn true_anomaly(&self) -> Angle {
        self.true_anomaly
    }

    fn validated(&self) -> Result<ValidatedKeplerian, StateError> {
        ValidatedKeplerian::from_values(
            self.semi_major_axis,
            self.eccentricity,
            self.inclination,
            self.right_ascension_of_ascending_node,
            self.argument_of_periapsis,
            self.true_anomaly,
        )
    }
}

/// Elliptic equinoctial state `(a, ex, ey, hx, hy, lv)`.
///
/// Definitions are `ex=e cos(omega+Omega)`, `ey=e sin(omega+Omega)`,
/// `hx=tan(i/2) cos(Omega)`, `hy=tan(i/2) sin(Omega)`, and
/// `lv=nu+omega+Omega`. Each state is bound to the identity of the sourced
/// gravity context used to define or interpret its elements.
#[derive(Debug, Clone)]
pub struct EquinoctialState {
    frame: InertialFrame,
    central_gravity: SharedCentralGravity,
    semi_major_axis: Length,
    eccentricity_x: Ratio,
    eccentricity_y: Ratio,
    inclination_x: Ratio,
    inclination_y: Ratio,
    true_longitude: Angle,
}

impl PartialEq for EquinoctialState {
    fn eq(&self, other: &Self) -> bool {
        self.frame == other.frame
            && Arc::ptr_eq(&self.central_gravity, &other.central_gravity)
            && self.semi_major_axis == other.semi_major_axis
            && self.eccentricity_x == other.eccentricity_x
            && self.eccentricity_y == other.eccentricity_y
            && self.inclination_x == other.inclination_x
            && self.inclination_y == other.inclination_y
            && self.true_longitude == other.true_longitude
    }
}

impl EquinoctialState {
    /// Constructs and validates an elliptic equinoctial state.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        frame: InertialFrame,
        central_gravity: SharedCentralGravity,
        semi_major_axis: Length,
        eccentricity_x: Ratio,
        eccentricity_y: Ratio,
        inclination_x: Ratio,
        inclination_y: Ratio,
        true_longitude: Angle,
    ) -> Result<Self, StateError> {
        validate_positive_axis(semi_major_axis)?;
        let ex = finite_ratio(eccentricity_x, "equinoctial ex")?;
        let ey = finite_ratio(eccentricity_y, "equinoctial ey")?;
        finite_ratio(inclination_x, "equinoctial hx")?;
        finite_ratio(inclination_y, "equinoctial hy")?;
        finite_angle(true_longitude, "true longitude")?;
        if !(0.0..1.0).contains(&ex.hypot(ey)) {
            return Err(StateError::EccentricityOutOfRange);
        }
        validate_gravity_origin(&central_gravity, frame.reference_frame())?;
        Ok(Self {
            frame,
            central_gravity,
            semi_major_axis,
            eccentricity_x,
            eccentricity_y,
            inclination_x,
            inclination_y,
            true_longitude,
        })
    }

    /// Returns the coordinate frame.
    #[must_use]
    pub const fn frame(&self) -> ReferenceFrame {
        self.frame.reference_frame()
    }

    /// Returns the affirmative inertial-frame capability carried by the state.
    #[must_use]
    pub const fn inertial_frame(&self) -> InertialFrame {
        self.frame
    }

    /// Returns the sourced gravity context to which these elements are bound.
    #[must_use]
    pub const fn central_gravity(&self) -> &SharedCentralGravity {
        &self.central_gravity
    }

    /// Returns `a`.
    #[must_use]
    pub const fn semi_major_axis(&self) -> Length {
        self.semi_major_axis
    }

    /// Returns `ex`.
    #[must_use]
    pub const fn eccentricity_x(&self) -> Ratio {
        self.eccentricity_x
    }

    /// Returns `ey`.
    #[must_use]
    pub const fn eccentricity_y(&self) -> Ratio {
        self.eccentricity_y
    }

    /// Returns `hx`.
    #[must_use]
    pub const fn inclination_x(&self) -> Ratio {
        self.inclination_x
    }

    /// Returns `hy`.
    #[must_use]
    pub const fn inclination_y(&self) -> Ratio {
        self.inclination_y
    }

    /// Returns `lv`.
    #[must_use]
    pub const fn true_longitude(&self) -> Angle {
        self.true_longitude
    }
}

impl SpacecraftStateContract for CartesianState {
    fn frame(&self) -> ReferenceFrame {
        CartesianState::frame(*self)
    }
}

impl SpacecraftStateContract for CircularState {
    fn frame(&self) -> ReferenceFrame {
        CircularState::frame(self)
    }
}

impl SpacecraftStateContract for KeplerianState {
    fn frame(&self) -> ReferenceFrame {
        KeplerianState::frame(self)
    }
}

impl SpacecraftStateContract for EquinoctialState {
    fn frame(&self) -> ReferenceFrame {
        EquinoctialState::frame(self)
    }
}

impl TryFrom<KeplerianState> for EquinoctialState {
    type Error = StateError;

    fn try_from(source: KeplerianState) -> Result<Self, Self::Error> {
        keplerian_to_equinoctial(source)
    }
}

impl TryFrom<EquinoctialState> for KeplerianState {
    type Error = StateError;

    fn try_from(source: EquinoctialState) -> Result<Self, Self::Error> {
        equinoctial_to_keplerian(source)
    }
}

impl TryFrom<(CartesianState, SharedCentralGravity)> for KeplerianState {
    type Error = StateError;

    fn try_from(source: (CartesianState, SharedCentralGravity)) -> Result<Self, Self::Error> {
        let (state, gravity) = source;
        keplerian_from_cartesian(&gravity, state)
    }
}

impl TryFrom<KeplerianState> for CartesianState {
    type Error = StateError;

    fn try_from(source: KeplerianState) -> Result<Self, Self::Error> {
        Self::try_from(&source)
    }
}

impl TryFrom<&KeplerianState> for CartesianState {
    type Error = StateError;

    fn try_from(source: &KeplerianState) -> Result<Self, Self::Error> {
        cartesian_from_keplerian(source.central_gravity(), source)
    }
}

impl TryFrom<(CartesianState, SharedCentralGravity)> for EquinoctialState {
    type Error = StateError;

    fn try_from(source: (CartesianState, SharedCentralGravity)) -> Result<Self, Self::Error> {
        let keplerian = KeplerianState::try_from(source)?;
        Self::try_from(keplerian)
    }
}

impl TryFrom<EquinoctialState> for CartesianState {
    type Error = StateError;

    fn try_from(source: EquinoctialState) -> Result<Self, Self::Error> {
        let keplerian = KeplerianState::try_from(source)?;
        Self::try_from(keplerian)
    }
}

impl TryFrom<KeplerianState> for CircularState {
    type Error = StateError;

    fn try_from(source: KeplerianState) -> Result<Self, Self::Error> {
        let eccentricity = source.eccentricity.get::<ratio>();
        let periapsis = source.argument_of_periapsis.get::<radian>();
        CircularState::new(
            source.frame,
            source.central_gravity,
            source.semi_major_axis,
            Ratio::new::<ratio>(eccentricity * periapsis.cos()),
            Ratio::new::<ratio>(eccentricity * periapsis.sin()),
            source.inclination,
            source.right_ascension_of_ascending_node,
            Angle::new::<radian>(source.true_anomaly.get::<radian>() + periapsis),
        )
    }
}

impl TryFrom<CircularState> for KeplerianState {
    type Error = StateError;

    fn try_from(source: CircularState) -> Result<Self, Self::Error> {
        let eccentricity_x = source.eccentricity_x.get::<ratio>();
        let eccentricity_y = source.eccentricity_y.get::<ratio>();
        let eccentricity = eccentricity_x.hypot(eccentricity_y);
        let periapsis = if eccentricity <= 64.0 * f64::EPSILON {
            0.0
        } else {
            eccentricity_y.atan2(eccentricity_x)
        };
        KeplerianState::new(
            source.frame,
            source.central_gravity,
            source.semi_major_axis,
            Ratio::new::<ratio>(eccentricity),
            source.inclination,
            source.right_ascension_of_ascending_node,
            Angle::new::<radian>(periapsis),
            Angle::new::<radian>(source.true_latitude_argument.get::<radian>() - periapsis),
        )
    }
}

impl TryFrom<(CartesianState, SharedCentralGravity)> for CircularState {
    type Error = StateError;

    fn try_from(source: (CartesianState, SharedCentralGravity)) -> Result<Self, Self::Error> {
        let keplerian = KeplerianState::try_from(source)?;
        Self::try_from(keplerian)
    }
}

impl TryFrom<CircularState> for CartesianState {
    type Error = StateError;

    fn try_from(source: CircularState) -> Result<Self, Self::Error> {
        let keplerian = KeplerianState::try_from(source)?;
        Self::try_from(keplerian)
    }
}

impl TryFrom<CircularState> for EquinoctialState {
    type Error = StateError;

    fn try_from(source: CircularState) -> Result<Self, Self::Error> {
        let keplerian = KeplerianState::try_from(source)?;
        Self::try_from(keplerian)
    }
}

impl TryFrom<EquinoctialState> for CircularState {
    type Error = StateError;

    fn try_from(source: EquinoctialState) -> Result<Self, Self::Error> {
        let keplerian = KeplerianState::try_from(source)?;
        Self::try_from(keplerian)
    }
}

fn keplerian_to_equinoctial(source: KeplerianState) -> Result<EquinoctialState, StateError> {
    let e = source.eccentricity.get::<ratio>();
    let i = source.inclination.get::<radian>();
    if (PI - i).abs() <= 16.0 * f64::EPSILON {
        return Err(StateError::RetrogradeEquinoctialSingularity);
    }
    let raan = source.right_ascension_of_ascending_node.get::<radian>();
    let periapsis = source.argument_of_periapsis.get::<radian>();
    let anomaly = source.true_anomaly.get::<radian>();
    let longitude_of_periapsis = periapsis + raan;
    let inclination_scale = (i / 2.0).tan();
    EquinoctialState::new(
        source.frame,
        source.central_gravity,
        source.semi_major_axis,
        Ratio::new::<ratio>(e * longitude_of_periapsis.cos()),
        Ratio::new::<ratio>(e * longitude_of_periapsis.sin()),
        Ratio::new::<ratio>(inclination_scale * raan.cos()),
        Ratio::new::<ratio>(inclination_scale * raan.sin()),
        Angle::new::<radian>(anomaly + longitude_of_periapsis),
    )
}

fn equinoctial_to_keplerian(source: EquinoctialState) -> Result<KeplerianState, StateError> {
    let ex = source.eccentricity_x.get::<ratio>();
    let ey = source.eccentricity_y.get::<ratio>();
    let hx = source.inclination_x.get::<ratio>();
    let hy = source.inclination_y.get::<ratio>();
    let longitude_of_periapsis = ey.atan2(ex);
    let raan = hy.atan2(hx);
    KeplerianState::new(
        source.frame,
        source.central_gravity,
        source.semi_major_axis,
        Ratio::new::<ratio>(ex.hypot(ey)),
        Angle::new::<radian>(2.0 * hx.hypot(hy).atan()),
        Angle::new::<radian>(raan),
        Angle::new::<radian>(longitude_of_periapsis - raan),
        Angle::new::<radian>(source.true_longitude.get::<radian>() - longitude_of_periapsis),
    )
}

#[derive(Debug, Clone, Copy)]
struct ValidatedKeplerian {
    semi_major_axis_m: f64,
    eccentricity: f64,
    inclination_rad: f64,
    raan_rad: f64,
    argument_of_periapsis_rad: f64,
    true_anomaly_rad: f64,
}

impl ValidatedKeplerian {
    fn from_values(
        semi_major_axis: Length,
        eccentricity: Ratio,
        inclination: Angle,
        right_ascension_of_ascending_node: Angle,
        argument_of_periapsis: Angle,
        true_anomaly: Angle,
    ) -> Result<Self, StateError> {
        validate_positive_axis(semi_major_axis)?;
        let eccentricity = finite_ratio(eccentricity, "eccentricity")?;
        if !(0.0..1.0).contains(&eccentricity) {
            return Err(StateError::EccentricityOutOfRange);
        }
        let inclination = finite_angle(inclination, "inclination")?;
        if !(0.0..=PI).contains(&inclination) {
            return Err(StateError::InclinationOutOfRange);
        }
        Ok(Self {
            semi_major_axis_m: semi_major_axis.get::<meter>(),
            eccentricity,
            inclination_rad: inclination,
            raan_rad: finite_angle(
                right_ascension_of_ascending_node,
                "right ascension of ascending node",
            )?,
            argument_of_periapsis_rad: finite_angle(
                argument_of_periapsis,
                "argument of periapsis",
            )?,
            true_anomaly_rad: finite_angle(true_anomaly, "true anomaly")?,
        })
    }
}

fn cartesian_from_keplerian(
    gravity: &SharedCentralGravity,
    source: &KeplerianState,
) -> Result<CartesianState, StateError> {
    validate_gravity_origin(gravity, source.frame())?;
    let elements = source.validated()?;
    let e = elements.eccentricity;
    let nu = elements.true_anomaly_rad;
    let p = elements.semi_major_axis_m * (1.0 - e * e);
    let radius = p / (1.0 + e * nu.cos());
    let speed_scale = (gravity.parameter().as_cubic_metres_per_second_squared() / p).sqrt();
    let (sin_nu, cos_nu) = nu.sin_cos();
    let position_perifocal = [radius * cos_nu, radius * sin_nu, 0.0];
    let velocity_perifocal = [-speed_scale * sin_nu, speed_scale * (e + cos_nu), 0.0];

    let (sin_raan, cos_raan) = elements.raan_rad.sin_cos();
    let (sin_i, cos_i) = elements.inclination_rad.sin_cos();
    let (sin_periapsis, cos_periapsis) = elements.argument_of_periapsis_rad.sin_cos();
    let rotation = [
        [
            cos_raan * cos_periapsis - sin_raan * sin_periapsis * cos_i,
            -cos_raan * sin_periapsis - sin_raan * cos_periapsis * cos_i,
            sin_raan * sin_i,
        ],
        [
            sin_raan * cos_periapsis + cos_raan * sin_periapsis * cos_i,
            -sin_raan * sin_periapsis + cos_raan * cos_periapsis * cos_i,
            -cos_raan * sin_i,
        ],
        [sin_periapsis * sin_i, cos_periapsis * sin_i, cos_i],
    ];
    let position = rotate(rotation, position_perifocal);
    let velocity = rotate(rotation, velocity_perifocal);
    CartesianState::new(
        source.frame(),
        Position::from_metres(position[0], position[1], position[2]),
        VelocityVector::from_metres_per_second(velocity[0], velocity[1], velocity[2]),
    )
}

fn keplerian_from_cartesian(
    gravity: &SharedCentralGravity,
    state: CartesianState,
) -> Result<KeplerianState, StateError> {
    validate_gravity_origin(gravity, state.frame)?;
    let inertial_frame = InertialFrame::try_from(state.frame)
        .map_err(|_| StateError::CartesianFrameNotExplicitlyInertial)?;

    let position_m = state.position.to_metres();
    let velocity_m_s = state.velocity.to_metres_per_second();
    let radius = norm(position_m);
    let speed = norm(velocity_m_s);
    if radius == 0.0 || speed == 0.0 {
        return Err(StateError::DegenerateCartesianOrbit);
    }

    let angular_momentum = cross(position_m, velocity_m_s);
    let angular_momentum_norm = norm(angular_momentum);
    if angular_momentum_norm <= 64.0 * f64::EPSILON * radius * speed {
        return Err(StateError::DegenerateCartesianOrbit);
    }

    let mu = gravity.parameter().as_cubic_metres_per_second_squared();
    let velocity_cross_momentum = cross(velocity_m_s, angular_momentum);
    let eccentricity_vector = subtract(
        scale(velocity_cross_momentum, 1.0 / mu),
        scale(position_m, 1.0 / radius),
    );
    let eccentricity = norm(eccentricity_vector);
    let specific_energy = 0.5 * dot(velocity_m_s, velocity_m_s) - mu / radius;
    if !specific_energy.is_finite() || specific_energy >= 0.0 || eccentricity >= 1.0 {
        return Err(StateError::NonEllipticCartesianOrbit);
    }

    let semi_major_axis = -mu / (2.0 * specific_energy);
    let inclination = (angular_momentum[2] / angular_momentum_norm)
        .clamp(-1.0, 1.0)
        .acos();
    let node = [-angular_momentum[1], angular_momentum[0], 0.0];
    let node_norm = norm(node);
    let equatorial = node_norm <= 64.0 * f64::EPSILON * angular_momentum_norm;
    let circular = eccentricity <= 64.0 * f64::EPSILON;
    let raan = if equatorial {
        0.0
    } else {
        node[1].atan2(node[0]).rem_euclid(TAU)
    };

    let (argument_of_periapsis, true_anomaly) = match (circular, equatorial) {
        (false, false) => (
            oriented_angle(node, eccentricity_vector, angular_momentum),
            oriented_angle(eccentricity_vector, position_m, angular_momentum),
        ),
        (false, true) => (
            (angular_momentum[2].signum() * eccentricity_vector[1])
                .atan2(eccentricity_vector[0])
                .rem_euclid(TAU),
            oriented_angle(eccentricity_vector, position_m, angular_momentum),
        ),
        (true, false) => (0.0, oriented_angle(node, position_m, angular_momentum)),
        (true, true) => (
            0.0,
            (angular_momentum[2].signum() * position_m[1])
                .atan2(position_m[0])
                .rem_euclid(TAU),
        ),
    };

    KeplerianState::new(
        inertial_frame,
        Arc::clone(gravity),
        Length::new::<meter>(semi_major_axis),
        Ratio::new::<ratio>(if circular { 0.0 } else { eccentricity }),
        Angle::new::<radian>(inclination),
        Angle::new::<radian>(raan),
        Angle::new::<radian>(argument_of_periapsis),
        Angle::new::<radian>(true_anomaly),
    )
}

fn validate_gravity_origin(
    gravity: &SharedCentralGravity,
    frame: ReferenceFrame,
) -> Result<(), StateError> {
    let frame_origin = frame.origin();
    if frame_origin != gravity.origin() {
        return Err(StateError::CentralGravityOriginMismatch {
            gravity_origin: gravity.origin(),
            frame_origin,
        });
    }
    Ok(())
}

fn dot(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0].mul_add(right[0], left[1].mul_add(right[1], left[2] * right[2]))
}

fn cross(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [
        left[1].mul_add(right[2], -left[2] * right[1]),
        left[2].mul_add(right[0], -left[0] * right[2]),
        left[0].mul_add(right[1], -left[1] * right[0]),
    ]
}

fn norm(vector: [f64; 3]) -> f64 {
    dot(vector, vector).sqrt()
}

fn scale(vector: [f64; 3], factor: f64) -> [f64; 3] {
    vector.map(|component| component * factor)
}

fn subtract(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|index| left[index] - right[index])
}

fn oriented_angle(from: [f64; 3], to: [f64; 3], normal: [f64; 3]) -> f64 {
    let denominator = norm(from) * norm(to);
    let cosine = (dot(from, to) / denominator).clamp(-1.0, 1.0);
    let sine = dot(cross(from, to), normal) / (denominator * norm(normal));
    sine.atan2(cosine).rem_euclid(TAU)
}

fn rotate(matrix: [[f64; 3]; 3], vector: [f64; 3]) -> [f64; 3] {
    matrix.map(|row| row[0].mul_add(vector[0], row[1].mul_add(vector[1], row[2] * vector[2])))
}

fn validate_positive_axis(value: Length) -> Result<(), StateError> {
    let value = value.get::<meter>();
    if !value.is_finite() {
        return Err(StateError::NonFiniteElement {
            element: "semi-major axis",
        });
    }
    if value <= 0.0 {
        return Err(StateError::NotPositiveSemiMajorAxis);
    }
    Ok(())
}

fn finite_ratio(value: Ratio, element: &'static str) -> Result<f64, StateError> {
    let value = value.get::<ratio>();
    if value.is_finite() {
        Ok(value)
    } else {
        Err(StateError::NonFiniteElement { element })
    }
}

fn finite_angle(value: Angle, element: &'static str) -> Result<f64, StateError> {
    let value = value.get::<radian>();
    if value.is_finite() {
        Ok(value)
    } else {
        Err(StateError::NonFiniteElement { element })
    }
}

/// Invalid orbital state or representation conversion input.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StateError {
    /// A Cartesian position component is NaN or infinite.
    #[error("Cartesian position components must be finite")]
    NonFiniteCartesianPosition,
    /// A Cartesian velocity component is NaN or infinite.
    #[error("Cartesian velocity components must be finite")]
    NonFiniteCartesianVelocity,
    /// A named orbital element is NaN or infinite.
    #[error("{element} must be finite")]
    NonFiniteElement {
        /// Rejected element.
        element: &'static str,
    },
    /// Elliptic semi-major axis must be positive.
    #[error("semi-major axis must be strictly positive")]
    NotPositiveSemiMajorAxis,
    /// This slice supports elliptic eccentricities only.
    #[error("eccentricity must satisfy 0 <= e < 1")]
    EccentricityOutOfRange,
    /// Inclination uses the conventional prograde-to-retrograde interval.
    #[error("inclination must satisfy 0 <= i <= pi radians")]
    InclinationOutOfRange,
    /// The selected equinoctial convention is singular at exactly 180 degrees.
    #[error("equinoctial hx/hy are singular for inclination pi radians")]
    RetrogradeEquinoctialSingularity,
    /// Cartesian position and velocity use different frames.
    #[error("Cartesian position and velocity frames must match")]
    MismatchedCartesianFrames,
    /// The frame axes were not affirmatively classified as inertial.
    #[error("Cartesian orbital-element conversion requires explicitly inertial axes")]
    CartesianFrameNotExplicitlyInertial,
    /// The Cartesian frame origin differs from the gravity context origin.
    #[error("frame origin {frame_origin} does not match central-gravity origin {gravity_origin}")]
    CentralGravityOriginMismatch {
        /// Origin configured by the gravity context.
        gravity_origin: FrameOrigin,
        /// Origin carried by the state frame.
        frame_origin: FrameOrigin,
    },
    /// Cartesian state does not define a stable osculating orbital plane.
    #[error("Cartesian state is degenerate for orbital-element conversion")]
    DegenerateCartesianOrbit,
    /// This conversion slice supports bound elliptic Cartesian states only.
    #[error("Cartesian state must describe a bound elliptic orbit")]
    NonEllipticCartesianOrbit,
    /// Coordinate adaptation failed finite-value validation.
    #[error(transparent)]
    DerivedKinematics(#[from] KinematicError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use frames::{CustomFrameId, FrameMotion, FrameOrientation, FrameOrigin, InertialFrame};
    use orskit_core::{Orbit, OrbitParts};
    use units::uom::si::velocity::meter_per_second;
    use units::GravitationalParameter;

    fn earth_mu() -> GravitationalParameter {
        GravitationalParameter::try_from(3.986_004_418e14)
            .expect("Earth gravitational parameter is positive")
    }

    fn central_gravity(
        origin: FrameOrigin,
        parameter: GravitationalParameter,
    ) -> SharedCentralGravity {
        Arc::new(gravity::PointMass::new(origin, parameter))
    }

    fn earth_gravity() -> SharedCentralGravity {
        central_gravity(FrameOrigin::Body(frames::Body::EARTH), earth_mu())
    }

    fn keplerian(inclination: f64, raan: f64, periapsis: f64, anomaly: f64) -> KeplerianState {
        let gravity = earth_gravity();
        KeplerianState::new(
            InertialFrame::GCRF,
            gravity,
            Length::new::<meter>(7_000_000.0),
            Ratio::new::<ratio>(0.0),
            Angle::new::<radian>(inclination),
            Angle::new::<radian>(raan),
            Angle::new::<radian>(periapsis),
            Angle::new::<radian>(anomaly),
        )
        .expect("fixture state is valid")
    }

    fn assert_vector_close(actual: [f64; 3], expected: [f64; 3], tolerance: f64) {
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!(
                (actual - expected).abs() <= tolerance,
                "actual {actual} differs from expected {expected} by more than {tolerance}"
            );
        }
    }

    #[test]
    fn infallible_coordinate_conversion_remains_explicit() {
        let cartesian = CartesianState::new(
            ReferenceFrame::GCRF,
            Position::from_metres(7_000_000.0, 0.0, 0.0),
            VelocityVector::from_metres_per_second(0.0, 7_500.0, 0.0),
        )
        .expect("finite state");
        let coordinates: CartesianCoordinates = cartesian.into();
        assert_eq!(CartesianState::try_from(coordinates), Ok(cartesian));
    }

    #[test]
    fn gravity_is_required_only_at_the_cartesian_conversion_boundary() {
        let gravity = earth_gravity();
        let source = KeplerianState::new(
            InertialFrame::GCRF,
            Arc::clone(&gravity),
            Length::new::<meter>(7_200_000.0),
            Ratio::new::<ratio>(0.1),
            Angle::new::<radian>(0.7),
            Angle::new::<radian>(1.1),
            Angle::new::<radian>(0.4),
            Angle::new::<radian>(2.0),
        )
        .expect("elliptic state");
        let equinoctial = EquinoctialState::try_from(source.clone()).expect("context-free K to E");
        let recovered = KeplerianState::try_from(equinoctial.clone()).expect("context-free E to K");
        let cartesian: CartesianState = source.try_into().expect("Keplerian to Cartesian");
        let recovered_cartesian: CartesianState = recovered
            .clone()
            .try_into()
            .expect("recovered Keplerian to Cartesian");
        let recovered_elements = KeplerianState::try_from((cartesian, Arc::clone(&gravity)))
            .expect("Cartesian to Keplerian");

        assert!(Arc::ptr_eq(equinoctial.central_gravity(), &gravity));
        assert!(Arc::ptr_eq(recovered.central_gravity(), &gravity));
        assert!(Arc::ptr_eq(recovered_elements.central_gravity(), &gravity));
        assert_vector_close(
            recovered_cartesian.position().to_metres(),
            cartesian.position().to_metres(),
            1.0e-8,
        );
        assert_vector_close(
            recovered_cartesian.velocity().to_metres_per_second(),
            cartesian.velocity().to_metres_per_second(),
            1.0e-10,
        );
    }

    #[test]
    fn standard_conversions_cover_the_supported_representation_graph() {
        let gravity = earth_gravity();
        let source = KeplerianState::new(
            InertialFrame::GCRF,
            Arc::clone(&gravity),
            Length::new::<meter>(7_000_000.0),
            Ratio::new::<ratio>(0.1),
            Angle::new::<radian>(0.7),
            Angle::new::<radian>(1.1),
            Angle::new::<radian>(0.4),
            Angle::new::<radian>(2.0),
        )
        .expect("valid elliptic fixture");

        let equinoctial: EquinoctialState =
            source.clone().try_into().expect("Keplerian to equinoctial");
        let recovered: KeplerianState = equinoctial
            .clone()
            .try_into()
            .expect("equinoctial to Keplerian");
        let circular: CircularState = source.clone().try_into().expect("Keplerian to circular");
        let circular_keplerian: KeplerianState =
            circular.clone().try_into().expect("circular to Keplerian");
        let circular_equinoctial: EquinoctialState = circular
            .clone()
            .try_into()
            .expect("circular to equinoctial");
        let equinoctial_circular: CircularState = equinoctial
            .clone()
            .try_into()
            .expect("equinoctial to circular");
        let cartesian: CartesianState = source.try_into().expect("Keplerian to Cartesian");
        let cartesian_keplerian: KeplerianState = (cartesian, Arc::clone(&gravity))
            .try_into()
            .expect("Cartesian to Keplerian");
        let cartesian: CartesianState = cartesian_keplerian
            .try_into()
            .expect("Keplerian to Cartesian");
        let cartesian_equinoctial: EquinoctialState = (cartesian, Arc::clone(&gravity))
            .try_into()
            .expect("Cartesian to equinoctial");
        let cartesian_circular: CircularState = (cartesian, Arc::clone(&gravity))
            .try_into()
            .expect("Cartesian to circular");
        let cartesian_from_equinoctial: CartesianState =
            equinoctial.try_into().expect("equinoctial to Cartesian");
        let cartesian_from_circular: CartesianState =
            circular.try_into().expect("circular to Cartesian");
        let cartesian_from_recovered: CartesianState =
            recovered.try_into().expect("Keplerian to Cartesian");
        let cartesian_from_circular_keplerian: CartesianState = circular_keplerian
            .try_into()
            .expect("Keplerian to Cartesian");
        let mapped: Orbit<EquinoctialState> =
            Orbit::new(Epoch::from_tai_seconds(42.0), cartesian_from_recovered)
                .try_map_state(|state| (state, Arc::clone(&gravity)).try_into())
                .expect("orbit state conversion");
        let OrbitParts { epoch, state } = mapped.into();
        let mapped_cartesian: CartesianState = state.try_into().expect("mapped state to Cartesian");

        assert_eq!(epoch, Epoch::from_tai_seconds(42.0));
        assert_vector_close(
            cartesian_from_equinoctial.position().to_metres(),
            mapped_cartesian.position().to_metres(),
            1.0e-8,
        );
        assert_vector_close(
            cartesian_from_equinoctial.velocity().to_metres_per_second(),
            mapped_cartesian.velocity().to_metres_per_second(),
            1.0e-10,
        );
        assert_vector_close(
            CartesianState::try_from(cartesian_equinoctial)
                .expect("equinoctial to Cartesian")
                .position()
                .to_metres(),
            mapped_cartesian.position().to_metres(),
            1.0e-8,
        );
        assert_vector_close(
            cartesian_from_circular.position().to_metres(),
            cartesian_from_circular_keplerian.position().to_metres(),
            1.0e-8,
        );
        assert_vector_close(
            cartesian_from_circular.position().to_metres(),
            CartesianState::try_from(circular_equinoctial)
                .expect("equinoctial to Cartesian")
                .position()
                .to_metres(),
            1.0e-8,
        );
        assert_vector_close(
            CartesianState::try_from(cartesian_circular)
                .expect("circular to Cartesian")
                .velocity()
                .to_metres_per_second(),
            cartesian_from_circular.velocity().to_metres_per_second(),
            1.0e-10,
        );
        assert_vector_close(
            CartesianState::try_from(equinoctial_circular)
                .expect("circular to Cartesian")
                .position()
                .to_metres(),
            cartesian_from_equinoctial.position().to_metres(),
            1.0e-8,
        );
    }

    #[test]
    fn cartesian_conversion_rejects_wrong_gravity_origin() {
        let mars_gravity = central_gravity(FrameOrigin::Body(frames::Body::MARS), earth_mu());
        let cartesian = CartesianState::new(
            ReferenceFrame::GCRF,
            Position::from_metres(7_000_000.0, 0.0, 0.0),
            VelocityVector::from_metres_per_second(0.0, 7_500.0, 0.0),
        )
        .expect("finite state");
        assert_eq!(
            KeplerianState::try_from((cartesian, mars_gravity)),
            Err(StateError::CentralGravityOriginMismatch {
                gravity_origin: FrameOrigin::Body(frames::Body::MARS),
                frame_origin: FrameOrigin::Body(frames::Body::EARTH),
            })
        );
    }

    #[test]
    fn element_constructors_reject_gravity_from_another_origin() {
        let mars_gravity = central_gravity(FrameOrigin::Body(frames::Body::MARS), earth_mu());
        let expected = StateError::CentralGravityOriginMismatch {
            gravity_origin: FrameOrigin::Body(frames::Body::MARS),
            frame_origin: FrameOrigin::Body(frames::Body::EARTH),
        };

        assert_eq!(
            KeplerianState::new(
                InertialFrame::GCRF,
                Arc::clone(&mars_gravity),
                Length::new::<meter>(7_200_000.0),
                Ratio::new::<ratio>(0.1),
                Angle::new::<radian>(0.7),
                Angle::new::<radian>(1.1),
                Angle::new::<radian>(0.4),
                Angle::new::<radian>(2.0),
            ),
            Err(expected.clone())
        );
        assert_eq!(
            EquinoctialState::new(
                InertialFrame::GCRF,
                mars_gravity,
                Length::new::<meter>(7_200_000.0),
                Ratio::new::<ratio>(0.1),
                Ratio::new::<ratio>(0.05),
                Ratio::new::<ratio>(0.2),
                Ratio::new::<ratio>(0.3),
                Angle::new::<radian>(2.0),
            ),
            Err(expected)
        );
    }

    #[test]
    fn circular_equatorial_conversion_matches_analytic_vector() {
        let radius = 7_000_000.0;
        let state = keplerian(0.0, 0.0, 0.0, 0.0);
        let cartesian: CartesianState = state.try_into().expect("valid conversion");
        let expected_speed = (earth_mu().as_cubic_metres_per_second_squared() / radius).sqrt();

        assert_vector_close(cartesian.position().to_metres(), [radius, 0.0, 0.0], 1.0e-8);
        assert_vector_close(
            cartesian.velocity().to_metres_per_second(),
            [0.0, expected_speed, 0.0],
            1.0e-10,
        );
        assert_eq!(cartesian.speed().get::<meter_per_second>(), expected_speed);
    }

    #[test]
    fn circular_state_true_latitude_matches_analytic_vector() {
        let radius = 7_000_000.0;
        let circular = CircularState::new(
            InertialFrame::GCRF,
            earth_gravity(),
            Length::new::<meter>(radius),
            Ratio::new::<ratio>(0.0),
            Ratio::new::<ratio>(0.0),
            Angle::new::<radian>(0.0),
            Angle::new::<radian>(0.0),
            Angle::new::<radian>(PI / 2.0),
        )
        .expect("valid circular elements");
        let cartesian: CartesianState = circular.try_into().expect("circular to Cartesian");
        let speed = (earth_mu().as_cubic_metres_per_second_squared() / radius).sqrt();

        assert_vector_close(cartesian.position().to_metres(), [0.0, radius, 0.0], 1.0e-8);
        assert_vector_close(
            cartesian.velocity().to_metres_per_second(),
            [-speed, 0.0, 0.0],
            1.0e-10,
        );
    }

    #[test]
    fn polar_conversion_has_expected_axes() {
        let radius = 7_000_000.0;
        let speed = (earth_mu().as_cubic_metres_per_second_squared() / radius).sqrt();
        let state = keplerian(PI / 2.0, 0.0, 0.0, PI / 2.0);
        let cartesian: CartesianState = state.try_into().expect("valid conversion");

        assert_vector_close(cartesian.position().to_metres(), [0.0, 0.0, radius], 1.0e-8);
        assert_vector_close(
            cartesian.velocity().to_metres_per_second(),
            [-speed, 0.0, 0.0],
            1.0e-10,
        );
    }

    #[test]
    fn coordinate_adapter_rejects_mixed_frames() {
        let coordinates = CartesianCoordinates::new(
            FramedPosition::new(
                Position::from_metres(7_000_000.0, 0.0, 0.0),
                ReferenceFrame::GCRF,
            )
            .expect("finite"),
            FramedVelocity::new(
                VelocityVector::from_metres_per_second(0.0, 7_500.0, 0.0),
                ReferenceFrame::EME2000,
            )
            .expect("finite"),
        );
        assert_eq!(
            CartesianState::try_from(coordinates),
            Err(StateError::MismatchedCartesianFrames)
        );
    }

    #[test]
    fn invalid_conics_and_singularities_are_rejected() {
        let gravity = earth_gravity();
        let retrograde = KeplerianState::new(
            InertialFrame::GCRF,
            gravity,
            Length::new::<meter>(7_000_000.0),
            Ratio::new::<ratio>(0.1),
            Angle::new::<radian>(PI),
            Angle::new::<radian>(0.0),
            Angle::new::<radian>(0.0),
            Angle::new::<radian>(0.0),
        )
        .expect("valid Keplerian state");
        let singular = EquinoctialState::try_from(retrograde);
        assert_eq!(singular, Err(StateError::RetrogradeEquinoctialSingularity));

        let radial = CartesianState::new(
            ReferenceFrame::GCRF,
            Position::from_metres(7_000_000.0, 0.0, 0.0),
            VelocityVector::from_metres_per_second(1_000.0, 0.0, 0.0),
        )
        .expect("finite state");
        let result = KeplerianState::try_from((radial, earth_gravity()));
        assert_eq!(result, Err(StateError::DegenerateCartesianOrbit));
    }

    #[test]
    fn cartesian_conversion_requires_explicitly_inertial_axes() {
        let id = CustomFrameId::new(17);
        let custom_frame = |motion| {
            ReferenceFrame::new(
                FrameOrigin::Body(frames::Body::EARTH),
                FrameOrientation::custom(id, motion),
            )
        };
        let state = |motion| {
            CartesianState::new(
                custom_frame(motion),
                Position::from_metres(7_000_000.0, 0.0, 0.0),
                VelocityVector::from_metres_per_second(0.0, 7_500.0, 0.0),
            )
            .expect("finite state")
        };

        assert_eq!(
            KeplerianState::try_from((state(FrameMotion::Unspecified), earth_gravity())),
            Err(StateError::CartesianFrameNotExplicitlyInertial)
        );
        KeplerianState::try_from((state(FrameMotion::Inertial), earth_gravity()))
            .expect("explicit custom inertial axes are supported");
    }
}
