use std::f64::consts::{PI, TAU};

use frames::{FrameOrigin, InertialFrame, ReferenceFrame};
use hifitime::Epoch;
use thiserror::Error;
use units::uom::si::{angle::radian, length::meter, ratio::ratio};
use units::{Angle, Length, Position, Ratio, Velocity, VelocityVector};

use crate::{
    CartesianCoordinates, FramedPosition, FramedVelocity, GravityContext, GravityContextId,
    KinematicError,
};

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

    /// Converts this Cartesian state to osculating Keplerian elements.
    pub fn to_keplerian(self, context: &GravityContext) -> Result<KeplerianState, StateError> {
        keplerian_from_cartesian(context, self)
    }

    /// Converts this Cartesian state to osculating equinoctial elements.
    pub fn to_equinoctial(self, context: &GravityContext) -> Result<EquinoctialState, StateError> {
        EquinoctialState::try_from(self.to_keplerian(context)?)
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
/// quantities; `nu` is true anomaly. Each state is bound to the identity of
/// the sourced gravity context used to define or interpret its elements.
#[derive(Debug, Clone, PartialEq)]
pub struct KeplerianState {
    frame: InertialFrame,
    gravity_context_id: GravityContextId,
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
        frame: InertialFrame,
        gravity_context_id: GravityContextId,
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
            gravity_context_id,
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
    pub const fn gravity_context_id(&self) -> &GravityContextId {
        &self.gravity_context_id
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

    /// Converts these elements using a context matching their bound identity.
    pub fn to_cartesian(self, context: &GravityContext) -> Result<CartesianState, StateError> {
        cartesian_from_keplerian(context, self)
    }

    /// Converts to equinoctial elements without requiring gravity data.
    pub fn to_equinoctial(self) -> Result<EquinoctialState, StateError> {
        EquinoctialState::try_from(self)
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
#[derive(Debug, Clone, PartialEq)]
pub struct EquinoctialState {
    frame: InertialFrame,
    gravity_context_id: GravityContextId,
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
        frame: InertialFrame,
        gravity_context_id: GravityContextId,
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
            gravity_context_id,
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
    pub const fn gravity_context_id(&self) -> &GravityContextId {
        &self.gravity_context_id
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

    /// Converts these elements using a context matching their bound identity.
    pub fn to_cartesian(self, context: &GravityContext) -> Result<CartesianState, StateError> {
        self.to_keplerian()?.to_cartesian(context)
    }

    /// Converts to Keplerian elements without requiring gravity data.
    pub fn to_keplerian(self) -> Result<KeplerianState, StateError> {
        KeplerianState::try_from(self)
    }
}

/// The closed set of currently supported six-element spacecraft states.
#[derive(Debug, Clone, PartialEq)]
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
    pub const fn frame(&self) -> ReferenceFrame {
        match self {
            Self::Cartesian(state) => state.frame(),
            Self::Keplerian(state) => state.frame(),
            Self::Equinoctial(state) => state.frame(),
        }
    }

    /// Returns the bound gravity-context identity for an element representation.
    #[must_use]
    pub const fn gravity_context_id(&self) -> Option<&GravityContextId> {
        match self {
            Self::Cartesian(_) => None,
            Self::Keplerian(state) => Some(state.gravity_context_id()),
            Self::Equinoctial(state) => Some(state.gravity_context_id()),
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
#[derive(Debug, Clone, PartialEq)]
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
    pub const fn epoch(&self) -> Epoch {
        self.epoch
    }

    /// Returns the native orbital representation.
    #[must_use]
    pub fn state(&self) -> SpacecraftState {
        self.state.clone()
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
        source.gravity_context_id,
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
        source.gravity_context_id,
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
    context: &GravityContext,
    source: KeplerianState,
) -> Result<CartesianState, StateError> {
    validate_bound_gravity_context(context, source.gravity_context_id(), source.frame())?;
    let elements = source.validated()?;
    let e = elements.eccentricity;
    let nu = elements.true_anomaly_rad;
    let p = elements.semi_major_axis_m * (1.0 - e * e);
    let radius = p / (1.0 + e * nu.cos());
    let speed_scale = (context
        .gravitational_parameter()
        .as_cubic_metres_per_second_squared()
        / p)
        .sqrt();
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
    context: &GravityContext,
    state: CartesianState,
) -> Result<KeplerianState, StateError> {
    validate_gravity_origin(context, state.frame)?;
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

    let mu = context
        .gravitational_parameter()
        .as_cubic_metres_per_second_squared();
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
        context.id().clone(),
        Length::new::<meter>(semi_major_axis),
        Ratio::new::<ratio>(if circular { 0.0 } else { eccentricity }),
        Angle::new::<radian>(inclination),
        Angle::new::<radian>(raan),
        Angle::new::<radian>(argument_of_periapsis),
        Angle::new::<radian>(true_anomaly),
    )
}

fn validate_bound_gravity_context(
    context: &GravityContext,
    state_context_id: &GravityContextId,
    frame: ReferenceFrame,
) -> Result<(), StateError> {
    if state_context_id != context.id() {
        return Err(StateError::GravityContextIdMismatch {
            state: state_context_id.clone(),
            supplied: context.id().clone(),
        });
    }
    validate_gravity_origin(context, frame)
}

fn validate_gravity_origin(
    context: &GravityContext,
    frame: ReferenceFrame,
) -> Result<(), StateError> {
    let frame_origin = frame.origin();
    if frame_origin != context.origin() {
        return Err(StateError::GravityContextOriginMismatch {
            context: context.id().clone(),
            context_origin: context.origin(),
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
    /// An element state is bound to a different sourced gravity context.
    #[error("state gravity context {state:?} does not match supplied context {supplied:?}")]
    GravityContextIdMismatch {
        /// Context identity stored in the element state.
        state: GravityContextId,
        /// Context identity supplied for Cartesian conversion.
        supplied: GravityContextId,
    },
    /// The Cartesian frame origin differs from the gravity context origin.
    #[error(
        "frame origin {frame_origin} does not match gravity context {context:?} origin {context_origin}"
    )]
    GravityContextOriginMismatch {
        /// Supplied gravity context identity.
        context: GravityContextId,
        /// Origin configured by the gravity context.
        context_origin: FrameOrigin,
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
    use units::uom::si::velocity::meter_per_second;
    use units::GravitationalParameter;

    fn earth_mu() -> GravitationalParameter {
        GravitationalParameter::from_cubic_metres_per_second_squared(3.986_004_418e14)
            .expect("Earth gravitational parameter is positive")
    }

    fn gravity_context(
        origin: FrameOrigin,
        gravitational_parameter: GravitationalParameter,
        scenario: &str,
    ) -> GravityContext {
        GravityContext::new(
            origin,
            gravitational_parameter,
            crate::ScientificSource::new(
                "orskit test suite",
                "orbital conversion fixture",
                scenario,
                "urn:orskit:test:orbital-conversion",
            )
            .expect("complete test provenance"),
        )
    }

    fn earth_context() -> GravityContext {
        gravity_context(FrameOrigin::Body(frames::Body::EARTH), earth_mu(), "earth")
    }

    fn keplerian(inclination: f64, raan: f64, periapsis: f64, anomaly: f64) -> KeplerianState {
        let context = earth_context();
        KeplerianState::new(
            InertialFrame::GCRF,
            context.id().clone(),
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
        let equinoctial = keplerian.clone().to_equinoctial().expect("convertible");

        assert_eq!(SpacecraftState::from(cartesian), cartesian.to());
        assert_eq!(SpacecraftState::from(keplerian.clone()), keplerian.to());
        assert_eq!(SpacecraftState::from(equinoctial.clone()), equinoctial.to());
    }

    #[test]
    fn gravity_is_required_only_at_the_cartesian_conversion_boundary() {
        let context = earth_context();
        let source = KeplerianState::new(
            InertialFrame::GCRF,
            context.id().clone(),
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
        let cartesian = source.to_cartesian(&context).expect("context-bound K to C");
        let recovered_cartesian = recovered
            .clone()
            .to_cartesian(&context)
            .expect("context-bound recovered K to C");
        let recovered_elements = cartesian
            .to_keplerian(&context)
            .expect("context-bound C to K");

        assert_eq!(equinoctial.gravity_context_id(), context.id());
        assert_eq!(recovered.gravity_context_id(), context.id());
        assert_eq!(recovered_elements.gravity_context_id(), context.id());
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
    fn cartesian_conversion_rejects_context_identity_and_origin_mismatches() {
        let source = keplerian(0.2, 0.3, 0.4, 0.5);
        let state_context_id = source.gravity_context_id().clone();
        let other_identity = gravity_context(
            FrameOrigin::Body(frames::Body::EARTH),
            earth_mu(),
            "other-earth-scenario",
        );
        let other_context_id = other_identity.id().clone();
        assert_eq!(
            source.to_cartesian(&other_identity),
            Err(StateError::GravityContextIdMismatch {
                state: state_context_id,
                supplied: other_context_id,
            })
        );

        let mars_context = gravity_context(
            FrameOrigin::Body(frames::Body::MARS),
            earth_mu(),
            "mars-origin",
        );
        let mars_context_id = mars_context.id().clone();
        let cartesian = CartesianState::new(
            ReferenceFrame::GCRF,
            Position::from_metres(7_000_000.0, 0.0, 0.0),
            VelocityVector::from_metres_per_second(0.0, 7_500.0, 0.0),
        )
        .expect("finite state");
        assert_eq!(
            cartesian.to_keplerian(&mars_context),
            Err(StateError::GravityContextOriginMismatch {
                context: mars_context_id,
                context_origin: FrameOrigin::Body(frames::Body::MARS),
                frame_origin: FrameOrigin::Body(frames::Body::EARTH),
            })
        );
    }

    #[test]
    fn circular_equatorial_conversion_matches_analytic_vector() {
        let radius = 7_000_000.0;
        let state = keplerian(0.0, 0.0, 0.0, 0.0);
        let cartesian = state
            .to_cartesian(&earth_context())
            .expect("valid conversion");
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
            .to_cartesian(&earth_context())
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
        let context = earth_context();
        let retrograde = KeplerianState::new(
            InertialFrame::GCRF,
            context.id().clone(),
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
        let result = radial.to_keplerian(&earth_context());
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
            state(FrameMotion::Unspecified).to_keplerian(&earth_context()),
            Err(StateError::CartesianFrameNotExplicitlyInertial)
        );
        state(FrameMotion::Inertial)
            .to_keplerian(&earth_context())
            .expect("explicit custom inertial axes are supported");
    }
}
