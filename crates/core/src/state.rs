use std::f64::consts::{PI, TAU};

use frames::ReferenceFrame;
use hifitime::Epoch;
use thiserror::Error;
use units::uom::si::{angle::radian, length::meter, ratio::ratio};
use units::{Angle, GravitationalParameter, Length, Position, Ratio, Velocity, VelocityVector};

use crate::{CartesianCoordinates, FramedPosition, FramedVelocity, KinematicError};

/// Coordinates tied to the epoch at which they are valid.
///
/// File formats may provide a timed coordinate sample without the mass,
/// inertia, and attitude required to construct a [`crate::Spacecraft`].
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

    /// Consumes the sample and returns its coordinates.
    #[must_use]
    pub fn into_coordinates(self) -> C {
        self.coordinates
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

/// Elliptic osculating Keplerian state `(a, e, i, Omega, omega, nu)`.
///
/// The supported regime is `a > 0` and `0 <= e < 1`. All angles are typed
/// quantities; `nu` is true anomaly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeplerianState {
    frame: ReferenceFrame,
    semi_major_axis: Length,
    eccentricity: Ratio,
    inclination: Angle,
    right_ascension_of_ascending_node: Angle,
    argument_of_periapsis: Angle,
    true_anomaly: Angle,
}

impl KeplerianState {
    /// Constructs and validates an elliptic osculating Keplerian state.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        frame: ReferenceFrame,
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
        Ok(Self {
            frame,
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
    pub const fn frame(self) -> ReferenceFrame {
        self.frame
    }

    /// Returns `a`.
    #[must_use]
    pub const fn semi_major_axis(self) -> Length {
        self.semi_major_axis
    }

    /// Returns `e`.
    #[must_use]
    pub const fn eccentricity(self) -> Ratio {
        self.eccentricity
    }

    /// Returns `i`.
    #[must_use]
    pub const fn inclination(self) -> Angle {
        self.inclination
    }

    /// Returns `Omega`.
    #[must_use]
    pub const fn right_ascension_of_ascending_node(self) -> Angle {
        self.right_ascension_of_ascending_node
    }

    /// Returns `omega`.
    #[must_use]
    pub const fn argument_of_periapsis(self) -> Angle {
        self.argument_of_periapsis
    }

    /// Returns `nu`.
    #[must_use]
    pub const fn true_anomaly(self) -> Angle {
        self.true_anomaly
    }

    fn validated(self) -> Result<ValidatedKeplerian, StateError> {
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
/// `lv=nu+omega+Omega`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EquinoctialState {
    frame: ReferenceFrame,
    semi_major_axis: Length,
    eccentricity_x: Ratio,
    eccentricity_y: Ratio,
    inclination_x: Ratio,
    inclination_y: Ratio,
    true_longitude: Angle,
}

impl EquinoctialState {
    /// Constructs and validates an elliptic equinoctial state.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        frame: ReferenceFrame,
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
        Ok(Self {
            frame,
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
    pub const fn frame(self) -> ReferenceFrame {
        self.frame
    }

    /// Returns `a`.
    #[must_use]
    pub const fn semi_major_axis(self) -> Length {
        self.semi_major_axis
    }

    /// Returns `ex`.
    #[must_use]
    pub const fn eccentricity_x(self) -> Ratio {
        self.eccentricity_x
    }

    /// Returns `ey`.
    #[must_use]
    pub const fn eccentricity_y(self) -> Ratio {
        self.eccentricity_y
    }

    /// Returns `hx`.
    #[must_use]
    pub const fn inclination_x(self) -> Ratio {
        self.inclination_x
    }

    /// Returns `hy`.
    #[must_use]
    pub const fn inclination_y(self) -> Ratio {
        self.inclination_y
    }

    /// Returns `lv`.
    #[must_use]
    pub const fn true_longitude(self) -> Angle {
        self.true_longitude
    }
}

/// The closed set of currently supported six-element spacecraft states.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpacecraftState {
    /// Cartesian `(x, y, z, vx, vy, vz)` state.
    Cartesian(CartesianState),
    /// Keplerian `(a, e, i, Omega, omega, nu)` state.
    Keplerian(KeplerianState),
    /// Equinoctial `(a, ex, ey, hx, hy, lv)` state.
    Equinoctial(EquinoctialState),
}

impl SpacecraftState {
    /// Returns the frame shared by the six elements.
    #[must_use]
    pub const fn frame(self) -> ReferenceFrame {
        match self {
            Self::Cartesian(state) => state.frame(),
            Self::Keplerian(state) => state.frame(),
            Self::Equinoctial(state) => state.frame(),
        }
    }
}

impl From<CartesianState> for SpacecraftState {
    fn from(state: CartesianState) -> Self {
        Self::Cartesian(state)
    }
}

impl From<KeplerianState> for SpacecraftState {
    fn from(state: KeplerianState) -> Self {
        Self::Keplerian(state)
    }
}

impl From<EquinoctialState> for SpacecraftState {
    fn from(state: EquinoctialState) -> Self {
        Self::Equinoctial(state)
    }
}

/// An orbital state qualified by the epoch at which its elements are valid.
///
/// This is the complete input and output of translational propagation. It does
/// not imply that spacecraft mass, inertia, or attitude were propagated.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Orbit {
    epoch: Epoch,
    state: SpacecraftState,
}

impl Orbit {
    /// Associates an orbital representation with its epoch.
    #[must_use]
    pub const fn new(epoch: Epoch, state: SpacecraftState) -> Self {
        Self { epoch, state }
    }

    /// Returns the epoch at which the orbital state is valid.
    #[must_use]
    pub const fn epoch(self) -> Epoch {
        self.epoch
    }

    /// Returns the native orbital representation.
    #[must_use]
    pub const fn state(self) -> SpacecraftState {
        self.state
    }
}

/// Source-side counterpart to [`From`].
///
/// This blanket implementation makes `value.to()` equivalent to
/// `Target::from(value)` while retaining target inference.
pub trait To<Target>: Sized {
    /// Performs an infallible conversion.
    fn to(self) -> Target;
}

impl<Source, Target> To<Target> for Source
where
    Target: From<Source>,
{
    fn to(self) -> Target {
        Target::from(self)
    }
}

/// A state plus the explicit context needed to change representation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbitalConversion<Source> {
    source: Source,
    gravitational_parameter: GravitationalParameter,
}

impl<Source> OrbitalConversion<Source> {
    /// Supplies a source state and central-body gravitational parameter.
    #[must_use]
    pub const fn new(source: Source, gravitational_parameter: GravitationalParameter) -> Self {
        Self {
            source,
            gravitational_parameter,
        }
    }

    /// Returns the source state.
    #[must_use]
    pub const fn source(&self) -> &Source {
        &self.source
    }

    /// Returns the conversion gravitational parameter.
    #[must_use]
    pub const fn gravitational_parameter(&self) -> GravitationalParameter {
        self.gravitational_parameter
    }
}

/// Source-side counterpart to [`TryFrom`] for orbital representations.
pub trait TryTo<Target>: Sized {
    /// Performs a fallible representation conversion with explicit gravity.
    fn try_to(self, gravitational_parameter: GravitationalParameter) -> Result<Target, StateError>;
}

impl<Source, Target> TryTo<Target> for Source
where
    Target: TryFrom<OrbitalConversion<Source>, Error = StateError>,
{
    fn try_to(self, gravitational_parameter: GravitationalParameter) -> Result<Target, StateError> {
        Target::try_from(OrbitalConversion::new(self, gravitational_parameter))
    }
}

/// Common access to every supported orbital representation.
///
/// Implementors do not cache all representations. Each accessor converts from
/// the native six elements using the supplied central-body gravity.
pub trait OrbitalElements: Copy {
    /// Provides Cartesian elements.
    fn cartesian(
        self,
        gravitational_parameter: GravitationalParameter,
    ) -> Result<CartesianState, StateError>;

    /// Provides Keplerian elements.
    fn keplerian(
        self,
        gravitational_parameter: GravitationalParameter,
    ) -> Result<KeplerianState, StateError>;

    /// Provides equinoctial elements.
    fn equinoctial(
        self,
        gravitational_parameter: GravitationalParameter,
    ) -> Result<EquinoctialState, StateError>;
}

impl<Source> OrbitalElements for Source
where
    Source: Copy + TryTo<CartesianState> + TryTo<KeplerianState> + TryTo<EquinoctialState>,
{
    fn cartesian(
        self,
        gravitational_parameter: GravitationalParameter,
    ) -> Result<CartesianState, StateError> {
        TryTo::<CartesianState>::try_to(self, gravitational_parameter)
    }

    fn keplerian(
        self,
        gravitational_parameter: GravitationalParameter,
    ) -> Result<KeplerianState, StateError> {
        TryTo::<KeplerianState>::try_to(self, gravitational_parameter)
    }

    fn equinoctial(
        self,
        gravitational_parameter: GravitationalParameter,
    ) -> Result<EquinoctialState, StateError> {
        TryTo::<EquinoctialState>::try_to(self, gravitational_parameter)
    }
}

macro_rules! identity_try_from {
    ($state:ty) => {
        impl TryFrom<OrbitalConversion<$state>> for $state {
            type Error = StateError;

            fn try_from(conversion: OrbitalConversion<$state>) -> Result<Self, Self::Error> {
                Ok(conversion.source)
            }
        }
    };
}

identity_try_from!(CartesianState);
identity_try_from!(KeplerianState);
identity_try_from!(EquinoctialState);

impl TryFrom<OrbitalConversion<CartesianState>> for KeplerianState {
    type Error = StateError;

    fn try_from(conversion: OrbitalConversion<CartesianState>) -> Result<Self, Self::Error> {
        keplerian_from_cartesian(conversion.gravitational_parameter, conversion.source)
    }
}

impl TryFrom<OrbitalConversion<CartesianState>> for EquinoctialState {
    type Error = StateError;

    fn try_from(conversion: OrbitalConversion<CartesianState>) -> Result<Self, Self::Error> {
        let keplerian = KeplerianState::try_from(conversion)?;
        keplerian_to_equinoctial(keplerian)
    }
}

impl TryFrom<OrbitalConversion<KeplerianState>> for CartesianState {
    type Error = StateError;

    fn try_from(conversion: OrbitalConversion<KeplerianState>) -> Result<Self, Self::Error> {
        cartesian_from_keplerian(
            conversion.gravitational_parameter,
            conversion.source.validated()?,
            conversion.source.frame,
        )
    }
}

impl TryFrom<OrbitalConversion<KeplerianState>> for EquinoctialState {
    type Error = StateError;

    fn try_from(conversion: OrbitalConversion<KeplerianState>) -> Result<Self, Self::Error> {
        keplerian_to_equinoctial(conversion.source)
    }
}

impl TryFrom<OrbitalConversion<EquinoctialState>> for KeplerianState {
    type Error = StateError;

    fn try_from(conversion: OrbitalConversion<EquinoctialState>) -> Result<Self, Self::Error> {
        equinoctial_to_keplerian(conversion.source)
    }
}

impl TryFrom<OrbitalConversion<EquinoctialState>> for CartesianState {
    type Error = StateError;

    fn try_from(conversion: OrbitalConversion<EquinoctialState>) -> Result<Self, Self::Error> {
        let keplerian = equinoctial_to_keplerian(conversion.source)?;
        CartesianState::try_from(OrbitalConversion::new(
            keplerian,
            conversion.gravitational_parameter,
        ))
    }
}

macro_rules! enum_try_from {
    ($target:ty, $method:ident) => {
        impl TryFrom<OrbitalConversion<SpacecraftState>> for $target {
            type Error = StateError;

            fn try_from(
                conversion: OrbitalConversion<SpacecraftState>,
            ) -> Result<Self, Self::Error> {
                match conversion.source {
                    SpacecraftState::Cartesian(state) => {
                        state.$method(conversion.gravitational_parameter)
                    }
                    SpacecraftState::Keplerian(state) => {
                        state.$method(conversion.gravitational_parameter)
                    }
                    SpacecraftState::Equinoctial(state) => {
                        state.$method(conversion.gravitational_parameter)
                    }
                }
            }
        }
    };
}

enum_try_from!(CartesianState, cartesian);
enum_try_from!(KeplerianState, keplerian);
enum_try_from!(EquinoctialState, equinoctial);

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
    gravitational_parameter: GravitationalParameter,
    elements: ValidatedKeplerian,
    frame: ReferenceFrame,
) -> Result<CartesianState, StateError> {
    let e = elements.eccentricity;
    let nu = elements.true_anomaly_rad;
    let p = elements.semi_major_axis_m * (1.0 - e * e);
    let radius = p / (1.0 + e * nu.cos());
    let speed_scale = (gravitational_parameter.as_cubic_metres_per_second_squared() / p).sqrt();
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
        frame,
        Position::from_metres(position[0], position[1], position[2]),
        VelocityVector::from_metres_per_second(velocity[0], velocity[1], velocity[2]),
    )
}

fn keplerian_from_cartesian(
    gravitational_parameter: GravitationalParameter,
    state: CartesianState,
) -> Result<KeplerianState, StateError> {
    if !state.frame.is_inertial() {
        return Err(StateError::CartesianFrameNotExplicitlyInertial);
    }

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

    let mu = gravitational_parameter.as_cubic_metres_per_second_squared();
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
        state.frame,
        Length::new::<meter>(semi_major_axis),
        Ratio::new::<ratio>(if circular { 0.0 } else { eccentricity }),
        Angle::new::<radian>(inclination),
        Angle::new::<radian>(raan),
        Angle::new::<radian>(argument_of_periapsis),
        Angle::new::<radian>(true_anomaly),
    )
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
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
    use frames::{CustomFrameId, FrameMotion, FrameOrientation, FrameOrigin};
    use units::uom::si::velocity::meter_per_second;

    fn earth_mu() -> GravitationalParameter {
        GravitationalParameter::from_cubic_metres_per_second_squared(3.986_004_418e14)
            .expect("Earth gravitational parameter is positive")
    }

    fn keplerian(inclination: f64, raan: f64, periapsis: f64, anomaly: f64) -> KeplerianState {
        KeplerianState::new(
            ReferenceFrame::GCRF,
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
    fn from_and_to_wrap_all_concrete_states() {
        let cartesian = CartesianState::new(
            ReferenceFrame::GCRF,
            Position::from_metres(7_000_000.0, 0.0, 0.0),
            VelocityVector::from_metres_per_second(0.0, 7_500.0, 0.0),
        )
        .expect("finite state");
        let keplerian = keplerian(0.2, 0.3, 0.4, 0.5);
        let equinoctial = keplerian.equinoctial(earth_mu()).expect("convertible");

        assert_eq!(SpacecraftState::from(cartesian), cartesian.to());
        assert_eq!(SpacecraftState::from(keplerian), keplerian.to());
        assert_eq!(SpacecraftState::from(equinoctial), equinoctial.to());
    }

    #[test]
    fn try_from_try_to_and_orbital_elements_cover_every_pair() {
        let source = KeplerianState::new(
            ReferenceFrame::GCRF,
            Length::new::<meter>(7_200_000.0),
            Ratio::new::<ratio>(0.1),
            Angle::new::<radian>(0.7),
            Angle::new::<radian>(1.1),
            Angle::new::<radian>(0.4),
            Angle::new::<radian>(2.0),
        )
        .expect("elliptic state");
        let via_try_from = CartesianState::try_from(OrbitalConversion::new(source, earth_mu()))
            .expect("TryFrom conversion");
        let via_try_to: CartesianState = source.try_to(earth_mu()).expect("TryTo conversion");
        let equinoctial = source.equinoctial(earth_mu()).expect("trait conversion");
        let enum_state: SpacecraftState = equinoctial.to();
        let recovered = enum_state.keplerian(earth_mu()).expect("enum conversion");
        let recovered_cartesian = recovered.cartesian(earth_mu()).expect("conversion");

        let concrete_cartesian = via_try_to;
        let concrete_keplerian = source;
        let concrete_equinoctial = equinoctial;
        let _: CartesianState = concrete_cartesian.try_to(earth_mu()).expect("C to C");
        let _: KeplerianState = concrete_cartesian.try_to(earth_mu()).expect("C to K");
        let _: EquinoctialState = concrete_cartesian.try_to(earth_mu()).expect("C to E");
        let _: CartesianState = concrete_keplerian.try_to(earth_mu()).expect("K to C");
        let _: KeplerianState = concrete_keplerian.try_to(earth_mu()).expect("K to K");
        let _: EquinoctialState = concrete_keplerian.try_to(earth_mu()).expect("K to E");
        let _: CartesianState = concrete_equinoctial.try_to(earth_mu()).expect("E to C");
        let _: KeplerianState = concrete_equinoctial.try_to(earth_mu()).expect("E to K");
        let _: EquinoctialState = concrete_equinoctial.try_to(earth_mu()).expect("E to E");

        for enum_state in [
            SpacecraftState::Cartesian(concrete_cartesian),
            SpacecraftState::Keplerian(concrete_keplerian),
            SpacecraftState::Equinoctial(concrete_equinoctial),
        ] {
            enum_state.cartesian(earth_mu()).expect("enum to C");
            enum_state.keplerian(earth_mu()).expect("enum to K");
            enum_state.equinoctial(earth_mu()).expect("enum to E");
        }

        assert_eq!(via_try_from, via_try_to);
        assert_vector_close(
            recovered_cartesian.position().to_metres(),
            via_try_to.position().to_metres(),
            1.0e-8,
        );
        assert_vector_close(
            recovered_cartesian.velocity().to_metres_per_second(),
            via_try_to.velocity().to_metres_per_second(),
            1.0e-10,
        );
    }

    #[test]
    fn circular_equatorial_conversion_matches_analytic_vector() {
        let radius = 7_000_000.0;
        let state = keplerian(0.0, 0.0, 0.0, 0.0);
        let cartesian = state.cartesian(earth_mu()).expect("valid conversion");
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
    fn polar_conversion_has_expected_axes() {
        let radius = 7_000_000.0;
        let speed = (earth_mu().as_cubic_metres_per_second_squared() / radius).sqrt();
        let cartesian = keplerian(PI / 2.0, 0.0, 0.0, PI / 2.0)
            .cartesian(earth_mu())
            .expect("valid conversion");

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
        let retrograde = KeplerianState::new(
            ReferenceFrame::GCRF,
            Length::new::<meter>(7_000_000.0),
            Ratio::new::<ratio>(0.1),
            Angle::new::<radian>(PI),
            Angle::new::<radian>(0.0),
            Angle::new::<radian>(0.0),
            Angle::new::<radian>(0.0),
        )
        .expect("valid Keplerian state");
        let singular: Result<EquinoctialState, _> = retrograde.try_to(earth_mu());
        assert_eq!(singular, Err(StateError::RetrogradeEquinoctialSingularity));

        let radial = CartesianState::new(
            ReferenceFrame::GCRF,
            Position::from_metres(7_000_000.0, 0.0, 0.0),
            VelocityVector::from_metres_per_second(1_000.0, 0.0, 0.0),
        )
        .expect("finite state");
        let result: Result<KeplerianState, _> = radial.try_to(earth_mu());
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
            state(FrameMotion::Unspecified).keplerian(earth_mu()),
            Err(StateError::CartesianFrameNotExplicitlyInertial)
        );
        state(FrameMotion::Inertial)
            .keplerian(earth_mu())
            .expect("explicit custom inertial axes are supported");
    }
}
