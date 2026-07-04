use std::{f64::consts::PI, fmt};

use hifitime::Epoch;
use orskit_frames::ReferenceFrame;
use orskit_units::uom::si::{angle::radian, length::meter, ratio::ratio};
use orskit_units::{
    Angle, GravitationalParameter, Length, Mass, MomentOfInertia, Position, Ratio, Velocity,
    VelocityVector,
};
use thiserror::Error;

use crate::{
    CartesianCoordinates, FramedPosition, FramedVelocity, InertiaTensor, KinematicError,
    Orientation, SpacecraftProperties,
};

/// Coordinates tied to the epoch at which they are valid.
///
/// This is deliberately not a complete [`State`]: formats such as CCSDS OEM
/// provide an epoch and coordinates, but omit mass, orientation, and inertia.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoordinateSample<C> {
    epoch: Epoch,
    coordinates: C,
}

impl<C> CoordinateSample<C> {
    /// Associates native representation coordinates with an epoch.
    #[must_use]
    pub const fn new(epoch: Epoch, coordinates: C) -> Self {
        Self { epoch, coordinates }
    }

    /// Returns the sample epoch.
    #[must_use]
    pub const fn epoch(&self) -> Epoch {
        self.epoch
    }

    /// Returns the native representation coordinates.
    #[must_use]
    pub const fn coordinates(&self) -> &C {
        &self.coordinates
    }

    /// Consumes the sample and returns its native coordinates.
    #[must_use]
    pub fn into_coordinates(self) -> C {
        self.coordinates
    }
}

/// Representation-aware common contract for complete spacecraft states.
///
/// The associated [`State::Coordinates`] type prevents a Keplerian or
/// equinoctial state from pretending to contain Cartesian position and
/// velocity. Algorithms that require another representation use
/// [`StateConversion`] explicitly.
///
/// ```
/// use orskit_core::State;
///
/// fn epoch<S: State>(state: &S) -> orskit_core::Epoch {
///     state.epoch()
/// }
/// ```
pub trait State: fmt::Debug + Send + Sync {
    /// Native coordinate representation stored by this state.
    type Coordinates;

    /// Returns the epoch and native coordinates.
    fn sample(&self) -> &CoordinateSample<Self::Coordinates>;

    /// Returns the representation-independent spacecraft properties.
    fn properties(&self) -> &SpacecraftProperties;

    /// Returns the state epoch.
    fn epoch(&self) -> Epoch {
        self.sample().epoch()
    }

    /// Returns the native representation coordinates.
    fn coordinates(&self) -> &Self::Coordinates {
        self.sample().coordinates()
    }

    /// Returns the spacecraft mass.
    fn mass(&self) -> Mass {
        self.properties().mass()
    }

    /// Returns the explicit spacecraft orientation.
    fn orientation(&self) -> &Orientation {
        self.properties().orientation()
    }

    /// Returns the framed inertia tensor.
    fn inertia(&self) -> InertiaTensor {
        self.properties().inertia()
    }

    /// Returns the symmetric inertia matrix in the tensor's attached frame.
    fn inertia_matrix(&self) -> [[MomentOfInertia; 3]; 3] {
        self.inertia().matrix()
    }
}

/// Explicit conversion between complete state representations.
///
/// Conversion-specific inputs belong to [`StateConversion::Context`]. For
/// example, Keplerian-to-Cartesian conversion requires a central-body
/// gravitational parameter, while Keplerian-to-equinoctial conversion does
/// not. Conversion never stores that context in either state.
pub trait StateConversion<Target: State>: State {
    /// Additional data required only while converting.
    type Context;

    /// Converts this state into `Target`, preserving epoch and spacecraft
    /// properties.
    fn convert(&self, context: Self::Context) -> Result<Target, StateError>;
}

/// Complete state whose native coordinates are Cartesian.
#[derive(Debug, Clone, PartialEq)]
pub struct CartesianState {
    sample: CoordinateSample<CartesianCoordinates>,
    properties: SpacecraftProperties,
}

impl CartesianState {
    /// Enriches a timed Cartesian sample with explicit spacecraft properties.
    #[must_use]
    pub const fn new(
        sample: CoordinateSample<CartesianCoordinates>,
        properties: SpacecraftProperties,
    ) -> Self {
        Self { sample, properties }
    }

    /// Returns the framed Cartesian position.
    #[must_use]
    pub const fn position(&self) -> FramedPosition {
        self.sample.coordinates.position()
    }

    /// Returns the framed Cartesian velocity.
    #[must_use]
    pub const fn velocity(&self) -> FramedVelocity {
        self.sample.coordinates.velocity()
    }

    /// Returns the scalar speed.
    #[must_use]
    pub fn speed(&self) -> Velocity {
        self.velocity().speed()
    }
}

impl State for CartesianState {
    type Coordinates = CartesianCoordinates;

    fn sample(&self) -> &CoordinateSample<Self::Coordinates> {
        &self.sample
    }

    fn properties(&self) -> &SpacecraftProperties {
        &self.properties
    }
}

/// Elliptic osculating Keplerian coordinates.
///
/// Angles are inclination `i`, right ascension of the ascending node `Omega`,
/// argument of periapsis `omega`, and true anomaly `nu`. The supported regime
/// is `a > 0` and `0 <= e < 1`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeplerianCoordinates {
    frame: ReferenceFrame,
    semi_major_axis: Length,
    eccentricity: Ratio,
    inclination: Angle,
    right_ascension_of_ascending_node: Angle,
    argument_of_periapsis: Angle,
    true_anomaly: Angle,
}

impl KeplerianCoordinates {
    /// Constructs and validates elliptic osculating Keplerian coordinates.
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

    /// Returns the semi-major axis.
    #[must_use]
    pub const fn semi_major_axis(self) -> Length {
        self.semi_major_axis
    }

    /// Returns the eccentricity.
    #[must_use]
    pub const fn eccentricity(self) -> Ratio {
        self.eccentricity
    }

    /// Returns the inclination.
    #[must_use]
    pub const fn inclination(self) -> Angle {
        self.inclination
    }

    /// Returns the right ascension of the ascending node.
    #[must_use]
    pub const fn right_ascension_of_ascending_node(self) -> Angle {
        self.right_ascension_of_ascending_node
    }

    /// Returns the argument of periapsis.
    #[must_use]
    pub const fn argument_of_periapsis(self) -> Angle {
        self.argument_of_periapsis
    }

    /// Returns the true anomaly.
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

/// Complete state whose native coordinates are Keplerian.
#[derive(Debug, Clone, PartialEq)]
pub struct KeplerianState {
    sample: CoordinateSample<KeplerianCoordinates>,
    properties: SpacecraftProperties,
}

impl KeplerianState {
    /// Enriches a validated timed Keplerian sample with spacecraft properties.
    #[must_use]
    pub const fn new(
        sample: CoordinateSample<KeplerianCoordinates>,
        properties: SpacecraftProperties,
    ) -> Self {
        Self { sample, properties }
    }
}

impl State for KeplerianState {
    type Coordinates = KeplerianCoordinates;

    fn sample(&self) -> &CoordinateSample<Self::Coordinates> {
        &self.sample
    }

    fn properties(&self) -> &SpacecraftProperties {
        &self.properties
    }
}

/// Elliptic equinoctial coordinates `(a, ex, ey, hx, hy, lv)`.
///
/// Definitions are `ex=e cos(omega+Omega)`, `ey=e sin(omega+Omega)`,
/// `hx=tan(i/2) cos(Omega)`, `hy=tan(i/2) sin(Omega)`, and
/// `lv=nu+omega+Omega`. Circular and equatorial elliptic orbits remain
/// nonsingular; the exactly retrograde equatorial case is singular.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EquinoctialCoordinates {
    frame: ReferenceFrame,
    semi_major_axis: Length,
    eccentricity_x: Ratio,
    eccentricity_y: Ratio,
    inclination_x: Ratio,
    inclination_y: Ratio,
    true_longitude: Angle,
}

impl EquinoctialCoordinates {
    /// Constructs and validates elliptic equinoctial coordinates.
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

    /// Returns the semi-major axis.
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

    /// Returns the true longitude argument `lv`.
    #[must_use]
    pub const fn true_longitude(self) -> Angle {
        self.true_longitude
    }
}

/// Complete state whose native coordinates are equinoctial.
#[derive(Debug, Clone, PartialEq)]
pub struct EquinoctialState {
    sample: CoordinateSample<EquinoctialCoordinates>,
    properties: SpacecraftProperties,
}

impl EquinoctialState {
    /// Enriches a validated timed equinoctial sample with spacecraft properties.
    #[must_use]
    pub const fn new(
        sample: CoordinateSample<EquinoctialCoordinates>,
        properties: SpacecraftProperties,
    ) -> Self {
        Self { sample, properties }
    }
}

impl State for EquinoctialState {
    type Coordinates = EquinoctialCoordinates;

    fn sample(&self) -> &CoordinateSample<Self::Coordinates> {
        &self.sample
    }

    fn properties(&self) -> &SpacecraftProperties {
        &self.properties
    }
}

impl StateConversion<CartesianState> for KeplerianState {
    type Context = GravitationalParameter;

    fn convert(
        &self,
        gravitational_parameter: Self::Context,
    ) -> Result<CartesianState, StateError> {
        let coordinates = cartesian_from_keplerian(
            gravitational_parameter,
            self.sample.coordinates.validated()?,
            self.sample.coordinates.frame,
        )?;
        Ok(CartesianState::new(
            CoordinateSample::new(self.epoch(), coordinates),
            self.properties.clone(),
        ))
    }
}

impl StateConversion<EquinoctialState> for KeplerianState {
    type Context = ();

    fn convert(&self, (): Self::Context) -> Result<EquinoctialState, StateError> {
        let coordinates = self.sample.coordinates;
        let e = coordinates.eccentricity.get::<ratio>();
        let i = coordinates.inclination.get::<radian>();
        if (PI - i).abs() <= 16.0 * f64::EPSILON {
            return Err(StateError::RetrogradeEquinoctialSingularity);
        }
        let raan = coordinates
            .right_ascension_of_ascending_node
            .get::<radian>();
        let periapsis = coordinates.argument_of_periapsis.get::<radian>();
        let anomaly = coordinates.true_anomaly.get::<radian>();
        let longitude_of_periapsis = periapsis + raan;
        let inclination_scale = (i / 2.0).tan();
        let target = EquinoctialCoordinates::new(
            coordinates.frame,
            coordinates.semi_major_axis,
            Ratio::new::<ratio>(e * longitude_of_periapsis.cos()),
            Ratio::new::<ratio>(e * longitude_of_periapsis.sin()),
            Ratio::new::<ratio>(inclination_scale * raan.cos()),
            Ratio::new::<ratio>(inclination_scale * raan.sin()),
            Angle::new::<radian>(anomaly + longitude_of_periapsis),
        )?;
        Ok(EquinoctialState::new(
            CoordinateSample::new(self.epoch(), target),
            self.properties.clone(),
        ))
    }
}

impl StateConversion<KeplerianState> for EquinoctialState {
    type Context = ();

    fn convert(&self, (): Self::Context) -> Result<KeplerianState, StateError> {
        let source = self.sample.coordinates;
        let ex = source.eccentricity_x.get::<ratio>();
        let ey = source.eccentricity_y.get::<ratio>();
        let hx = source.inclination_x.get::<ratio>();
        let hy = source.inclination_y.get::<ratio>();
        let longitude_of_periapsis = ey.atan2(ex);
        let raan = hy.atan2(hx);
        let target = KeplerianCoordinates::new(
            source.frame,
            source.semi_major_axis,
            Ratio::new::<ratio>(ex.hypot(ey)),
            Angle::new::<radian>(2.0 * hx.hypot(hy).atan()),
            Angle::new::<radian>(raan),
            Angle::new::<radian>(longitude_of_periapsis - raan),
            Angle::new::<radian>(source.true_longitude.get::<radian>() - longitude_of_periapsis),
        )?;
        Ok(KeplerianState::new(
            CoordinateSample::new(self.epoch(), target),
            self.properties.clone(),
        ))
    }
}

impl StateConversion<CartesianState> for EquinoctialState {
    type Context = GravitationalParameter;

    fn convert(
        &self,
        gravitational_parameter: Self::Context,
    ) -> Result<CartesianState, StateError> {
        let keplerian: KeplerianState =
            <Self as StateConversion<KeplerianState>>::convert(self, ())?;
        <KeplerianState as StateConversion<CartesianState>>::convert(
            &keplerian,
            gravitational_parameter,
        )
    }
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
) -> Result<CartesianCoordinates, StateError> {
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
    Ok(CartesianCoordinates::new(
        FramedPosition::new(
            Position::from_metres(position[0], position[1], position[2]),
            frame,
        )?,
        FramedVelocity::new(
            VelocityVector::from_metres_per_second(velocity[0], velocity[1], velocity[2]),
            frame,
        )?,
    ))
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

/// Invalid complete-state, coordinate, or conversion input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum StateError {
    /// Mass is NaN or infinite.
    #[error("mass must be finite")]
    NonFiniteMass,
    /// Mass is zero or negative.
    #[error("mass must be strictly positive")]
    NotPositiveMass,
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
    /// Derived Cartesian coordinates failed finite-value validation.
    #[error(transparent)]
    DerivedKinematics(#[from] KinematicError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use orskit_frames::{CustomFrameId, FrameOrientation, FrameOrigin};
    use orskit_units::uom::si::{
        mass::kilogram, moment_of_inertia::kilogram_square_meter, velocity::meter_per_second,
    };

    fn properties() -> SpacecraftProperties {
        let id = CustomFrameId::new(1);
        let body = ReferenceFrame::new(FrameOrigin::Custom(id), FrameOrientation::Custom(id));
        let orientation = Orientation::identity(body, ReferenceFrame::GCRF);
        let inertia = InertiaTensor::principal(
            body,
            MomentOfInertia::new::<kilogram_square_meter>(1_000.0),
            MomentOfInertia::new::<kilogram_square_meter>(1_200.0),
            MomentOfInertia::new::<kilogram_square_meter>(800.0),
        )
        .expect("fixture inertia is physical");
        SpacecraftProperties::new(Mass::new::<kilogram>(500.0), orientation, inertia)
            .expect("fixture properties are physical")
    }

    fn earth_mu() -> GravitationalParameter {
        GravitationalParameter::from_cubic_metres_per_second_squared(3.986_004_418e14)
            .expect("Earth gravitational parameter is positive")
    }

    fn keplerian_coordinates(
        inclination: f64,
        raan: f64,
        periapsis: f64,
        anomaly: f64,
    ) -> KeplerianCoordinates {
        KeplerianCoordinates::new(
            ReferenceFrame::GCRF,
            Length::new::<meter>(7_000_000.0),
            Ratio::new::<ratio>(0.0),
            Angle::new::<radian>(inclination),
            Angle::new::<radian>(raan),
            Angle::new::<radian>(periapsis),
            Angle::new::<radian>(anomaly),
        )
        .expect("fixture coordinates are valid")
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
    fn state_trait_keeps_native_coordinates() {
        let coordinates = keplerian_coordinates(0.2, 0.3, 0.4, 0.5);
        let state = KeplerianState::new(
            CoordinateSample::new(Epoch::from_tai_seconds(1.0), coordinates),
            properties(),
        );

        assert_eq!(state.epoch(), Epoch::from_tai_seconds(1.0));
        assert_eq!(state.coordinates(), &coordinates);
        assert_eq!(state.mass(), Mass::new::<kilogram>(500.0));
    }

    #[test]
    fn keplerian_state_has_no_gravity_or_cartesian_dependency() {
        let state = KeplerianState::new(
            CoordinateSample::new(
                Epoch::from_tai_seconds(0.0),
                keplerian_coordinates(0.0, 0.0, 0.0, 0.0),
            ),
            properties(),
        );

        assert_eq!(
            state.coordinates().semi_major_axis(),
            Length::new::<meter>(7_000_000.0)
        );
        let cartesian: CartesianState = state
            .convert(earth_mu())
            .expect("conversion context supplies gravity");
        assert_eq!(cartesian.epoch(), state.epoch());
    }

    #[test]
    fn circular_equatorial_conversion_matches_analytic_vector() {
        let radius = 7_000_000.0;
        let state = KeplerianState::new(
            CoordinateSample::new(
                Epoch::from_tai_seconds(0.0),
                keplerian_coordinates(0.0, 0.0, 0.0, 0.0),
            ),
            properties(),
        );
        let cartesian: CartesianState = state.convert(earth_mu()).expect("valid conversion");
        let expected_speed = (earth_mu().as_cubic_metres_per_second_squared() / radius).sqrt();

        assert_vector_close(
            cartesian.position().value().to_metres(),
            [radius, 0.0, 0.0],
            1.0e-8,
        );
        assert_vector_close(
            cartesian.velocity().value().to_metres_per_second(),
            [0.0, expected_speed, 0.0],
            1.0e-10,
        );
        assert_eq!(cartesian.speed().get::<meter_per_second>(), expected_speed);
    }

    #[test]
    fn polar_conversion_has_expected_axes() {
        let radius = 7_000_000.0;
        let speed = (earth_mu().as_cubic_metres_per_second_squared() / radius).sqrt();
        let state = KeplerianState::new(
            CoordinateSample::new(
                Epoch::from_tai_seconds(0.0),
                keplerian_coordinates(PI / 2.0, 0.0, 0.0, PI / 2.0),
            ),
            properties(),
        );
        let cartesian: CartesianState = state.convert(earth_mu()).expect("valid conversion");

        assert_vector_close(
            cartesian.position().value().to_metres(),
            [0.0, 0.0, radius],
            1.0e-8,
        );
        assert_vector_close(
            cartesian.velocity().value().to_metres_per_second(),
            [-speed, 0.0, 0.0],
            1.0e-10,
        );
    }

    #[test]
    fn representation_conversion_is_explicit_and_agrees() {
        let coordinates = KeplerianCoordinates::new(
            ReferenceFrame::GCRF,
            Length::new::<meter>(7_200_000.0),
            Ratio::new::<ratio>(0.1),
            Angle::new::<radian>(0.7),
            Angle::new::<radian>(1.1),
            Angle::new::<radian>(0.4),
            Angle::new::<radian>(2.0),
        )
        .expect("elliptic coordinates are valid");
        let keplerian = KeplerianState::new(
            CoordinateSample::new(Epoch::from_tai_seconds(42.0), coordinates),
            properties(),
        );
        let equinoctial: EquinoctialState = keplerian.convert(()).expect("valid conversion");
        let from_keplerian: CartesianState =
            keplerian.convert(earth_mu()).expect("valid conversion");
        let from_equinoctial: CartesianState =
            equinoctial.convert(earth_mu()).expect("valid conversion");

        assert_vector_close(
            from_equinoctial.position().value().to_metres(),
            from_keplerian.position().value().to_metres(),
            1.0e-8,
        );
        assert_vector_close(
            from_equinoctial.velocity().value().to_metres_per_second(),
            from_keplerian.velocity().value().to_metres_per_second(),
            1.0e-10,
        );
        assert_eq!(equinoctial.epoch(), keplerian.epoch());
        assert_eq!(equinoctial.mass(), keplerian.mass());
    }

    #[test]
    fn invalid_coordinates_and_retrograde_conversion_are_rejected() {
        assert!(matches!(
            KeplerianCoordinates::new(
                ReferenceFrame::GCRF,
                Length::new::<meter>(7_000_000.0),
                Ratio::new::<ratio>(1.0),
                Angle::new::<radian>(0.0),
                Angle::new::<radian>(0.0),
                Angle::new::<radian>(0.0),
                Angle::new::<radian>(0.0),
            ),
            Err(StateError::EccentricityOutOfRange)
        ));

        let retrograde = KeplerianState::new(
            CoordinateSample::new(
                Epoch::from_tai_seconds(0.0),
                KeplerianCoordinates::new(
                    ReferenceFrame::GCRF,
                    Length::new::<meter>(7_000_000.0),
                    Ratio::new::<ratio>(0.1),
                    Angle::new::<radian>(PI),
                    Angle::new::<radian>(0.0),
                    Angle::new::<radian>(0.0),
                    Angle::new::<radian>(0.0),
                )
                .expect("retrograde Keplerian coordinates are valid"),
            ),
            properties(),
        );
        let result: Result<EquinoctialState, _> = retrograde.convert(());
        assert_eq!(result, Err(StateError::RetrogradeEquinoctialSingularity));
    }
}
