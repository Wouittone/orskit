#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Provider-driven cannonball solar-radiation-pressure dynamics.
//!
//! The crate owns no solar ephemeris, solar-flux data, occulting-body
//! constants, or frame transformations. [`CannonballSolarRadiationPressure`]
//! requests the Sun from the caller's [`BodyEphemerisProvider`] and receives
//! reference-distance irradiance from a caller's [`SolarFluxProvider`].
//! Irradiance is scaled by the instantaneous spacecraft-to-Sun distance, and
//! the result is `flux / c * coefficient * area / mass` directed away from
//! the Sun.
//!
//! Eclipse support is intentionally limited to an explicit cylindrical
//! umbra. It is a full-light/full-shadow geometry: penumbra and solar-disc
//! dimensions are not inferred or approximated.

use std::{fmt, sync::Arc};

use dynamics::{
    CartesianForceModelError, EvaluableCartesianForceModel, Force, ForceModel,
    NonConservativeForceModel, SpacecraftStateRequirements,
};
use frames::{Body, FrameOrigin, ReferenceFrame};
use hifitime::Epoch;
use orbits::cartesian::{
    BodyEphemerisError, BodyEphemerisProvider, CartesianState, FramedAcceleration,
};
use thiserror::Error;
use units::uom::si::{area::square_meter, length::meter, mass::kilogram, ratio::ratio};
use units::{AccelerationVector, Area, Length, Mass, Ratio};

/// Speed of light in vacuum, in metres per second.
pub const SPEED_OF_LIGHT_M_PER_S: f64 = 299_792_458.0;

/// A validated positive reference-distance solar irradiance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolarFlux(f64);

impl SolarFlux {
    /// Creates irradiance in watts per square metre at the provider's
    /// reference distance.
    pub fn new_watts_per_square_meter(value: f64) -> Result<Self, SrpInputError> {
        if !value.is_finite() {
            return Err(SrpInputError::NonFiniteFlux);
        }
        if value < 0.0 {
            return Err(SrpInputError::NegativeFlux);
        }
        Ok(Self(value))
    }

    /// Returns the irradiance in watts per square metre.
    #[must_use]
    pub const fn watts_per_square_meter(self) -> f64 {
        self.0
    }
}

/// Supplies reference-distance solar irradiance for an epoch.
pub trait SolarFluxProvider: fmt::Debug + Send + Sync {
    /// Provider-specific evaluation failure.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Returns irradiance at the model's configured reference distance.
    fn flux_at(&self, epoch: Epoch) -> Result<SolarFlux, Self::Error>;
}

/// Positive, finite spacecraft reference area.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SrpArea(Area);

impl SrpArea {
    /// Creates an area in square metres.
    pub fn new(value: Area) -> Result<Self, SrpInputError> {
        let value = value.get::<square_meter>();
        if !value.is_finite() {
            return Err(SrpInputError::NonFiniteArea);
        }
        if value <= 0.0 {
            return Err(SrpInputError::NonPositiveArea);
        }
        Ok(Self(Area::new::<square_meter>(value)))
    }

    /// Returns the area.
    #[must_use]
    pub const fn value(self) -> Area {
        self.0
    }
}

/// Positive, finite spacecraft mass.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SrpMass(Mass);

impl SrpMass {
    /// Creates a mass in kilograms.
    pub fn new(value: Mass) -> Result<Self, SrpInputError> {
        let value = value.get::<kilogram>();
        if !value.is_finite() {
            return Err(SrpInputError::NonFiniteMass);
        }
        if value <= 0.0 {
            return Err(SrpInputError::NonPositiveMass);
        }
        Ok(Self(Mass::new::<kilogram>(value)))
    }

    /// Returns the mass.
    #[must_use]
    pub const fn value(self) -> Mass {
        self.0
    }
}

/// Positive, finite cannonball optical coefficient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OpticalCoefficient(Ratio);

impl OpticalCoefficient {
    /// Creates a dimensionless coefficient.
    pub fn new(value: Ratio) -> Result<Self, SrpInputError> {
        let value = value.get::<ratio>();
        if !value.is_finite() {
            return Err(SrpInputError::NonFiniteCoefficient);
        }
        if value < 0.0 {
            return Err(SrpInputError::NegativeCoefficient);
        }
        Ok(Self(Ratio::new::<ratio>(value)))
    }

    /// Returns the coefficient.
    #[must_use]
    pub const fn value(self) -> Ratio {
        self.0
    }
}

/// Validated spacecraft properties used by the cannonball model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SrpSpacecraft {
    /// Reference area exposed to the radiation.
    pub area: SrpArea,
    /// Spacecraft mass.
    pub mass: SrpMass,
    /// Cannonball optical coefficient.
    pub coefficient: OpticalCoefficient,
}

/// Eclipse geometry selected explicitly by the caller.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EclipseGeometry {
    /// Full cylindrical umbra behind a spherical occulting body.
    CylindricalUmbra {
        /// Radius of the occulting body in metres.
        occulting_body_radius: Length,
    },
}

/// Construction-time input failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum SrpInputError {
    /// Irradiance was not finite.
    #[error("solar flux must be finite")]
    NonFiniteFlux,
    /// Irradiance was negative.
    #[error("solar flux must not be negative")]
    NegativeFlux,
    /// Area was not finite.
    #[error("SRP area must be finite")]
    NonFiniteArea,
    /// Area was zero or negative.
    #[error("SRP area must be positive")]
    NonPositiveArea,
    /// Mass was not finite.
    #[error("SRP mass must be finite")]
    NonFiniteMass,
    /// Mass was zero or negative.
    #[error("SRP mass must be positive")]
    NonPositiveMass,
    /// Optical coefficient was not finite.
    #[error("optical coefficient must be finite")]
    NonFiniteCoefficient,
    /// Optical coefficient was negative.
    #[error("optical coefficient must not be negative")]
    NegativeCoefficient,
    /// Reference distance was not finite.
    #[error("solar-flux reference distance must be finite")]
    NonFiniteReferenceDistance,
    /// Reference distance was zero or negative.
    #[error("solar-flux reference distance must be positive")]
    NonPositiveReferenceDistance,
    /// Occulting-body radius was invalid.
    #[error("occulting-body radius must be finite and positive")]
    InvalidOccultingBodyRadius,
}

/// Evaluation failure from the SRP geometry or its providers.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SrpEvaluationError<FE>
where
    FE: std::error::Error + Send + Sync + 'static,
{
    /// State axes are not explicitly inertial.
    #[error("SRP requires explicitly inertial axes")]
    NonInertialFrame,
    /// State origin is not the occulting body.
    #[error("state frame origin {frame_origin:?} does not match occulting body {occulting_body}")]
    OriginMismatch {
        /// Origin carried by the state frame.
        frame_origin: FrameOrigin,
        /// Required occulting body.
        occulting_body: Body,
    },
    /// Ephemeris provider failed.
    #[error("Sun ephemeris evaluation failed")]
    Ephemeris(#[source] BodyEphemerisError),
    /// Ephemeris returned a different body.
    #[error("ephemeris returned {actual_body}, expected {requested_body}")]
    EphemerisBodyMismatch {
        /// Body requested from the provider.
        requested_body: Body,
        /// Body returned by the provider.
        actual_body: Body,
    },
    /// Ephemeris returned a different epoch.
    #[error("ephemeris returned a state at a different epoch")]
    EphemerisEpochMismatch,
    /// Ephemeris returned a different frame.
    #[error("ephemeris returned frame {actual_frame:?}, expected {expected_frame:?}")]
    EphemerisFrameMismatch {
        /// Frame returned by the provider.
        actual_frame: Box<ReferenceFrame>,
        /// Frame requested from the provider.
        expected_frame: Box<ReferenceFrame>,
    },
    /// Ephemeris position was not finite.
    #[error("Sun ephemeris position is not finite")]
    NonFiniteEphemerisPosition,
    /// Flux provider failed.
    #[error("solar flux evaluation failed")]
    Flux(#[source] FE),
    /// Spacecraft-to-Sun distance is singular or invalid.
    #[error("spacecraft-to-Sun distance must be finite and positive")]
    InvalidSunDistance,
    /// Computed acceleration was not finite.
    #[error("solar radiation pressure acceleration is not finite")]
    NonFiniteAcceleration,
}

/// Physical direct solar-radiation-pressure interaction.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct SolarRadiationPressureForce;

impl Force for SolarRadiationPressureForce {
    fn name(&self) -> &str {
        "solar radiation pressure"
    }
}

static SRP_FORCE: SolarRadiationPressureForce = SolarRadiationPressureForce;

/// Cannonball direct-Sun radiation-pressure model.
#[derive(Debug)]
pub struct CannonballSolarRadiationPressure<F> {
    occulting_body: Body,
    reference_distance_m: f64,
    eclipse: EclipseGeometry,
    ephemeris: Arc<dyn BodyEphemerisProvider>,
    flux: Arc<F>,
    area: SrpArea,
    mass: SrpMass,
    coefficient: OpticalCoefficient,
}

impl<F> CannonballSolarRadiationPressure<F>
where
    F: SolarFluxProvider,
{
    /// Constructs a model with explicit ephemeris, flux, spacecraft, and
    /// eclipse inputs.
    pub fn new(
        occulting_body: Body,
        reference_distance: Length,
        eclipse: EclipseGeometry,
        ephemeris: Arc<dyn BodyEphemerisProvider>,
        flux: Arc<F>,
        spacecraft: SrpSpacecraft,
    ) -> Result<Self, SrpInputError> {
        let reference_distance_m = reference_distance.get::<meter>();
        if !reference_distance_m.is_finite() {
            return Err(SrpInputError::NonFiniteReferenceDistance);
        }
        if reference_distance_m <= 0.0 {
            return Err(SrpInputError::NonPositiveReferenceDistance);
        }
        let radius = match eclipse {
            EclipseGeometry::CylindricalUmbra {
                occulting_body_radius,
            } => occulting_body_radius.get::<meter>(),
        };
        if !radius.is_finite() || radius <= 0.0 {
            return Err(SrpInputError::InvalidOccultingBodyRadius);
        }
        Ok(Self {
            occulting_body,
            reference_distance_m,
            eclipse,
            ephemeris,
            flux,
            area: spacecraft.area,
            mass: spacecraft.mass,
            coefficient: spacecraft.coefficient,
        })
    }

    fn evaluate_typed(
        &self,
        epoch: Epoch,
        state: &CartesianState,
    ) -> Result<FramedAcceleration, SrpEvaluationError<F::Error>> {
        self.validate_state(state)?;
        let sun = self
            .ephemeris
            .state_at(Body::SUN, epoch, state.frame())
            .map_err(SrpEvaluationError::Ephemeris)?;
        if sun.body() != Body::SUN {
            return Err(SrpEvaluationError::EphemerisBodyMismatch {
                requested_body: Body::SUN,
                actual_body: sun.body(),
            });
        }
        if sun.epoch() != epoch {
            return Err(SrpEvaluationError::EphemerisEpochMismatch);
        }
        if sun.kinematics().frame() != state.frame() {
            return Err(SrpEvaluationError::EphemerisFrameMismatch {
                actual_frame: Box::new(sun.kinematics().frame()),
                expected_frame: Box::new(state.frame()),
            });
        }
        let sun_position = sun.kinematics().position().to_metres();
        if !sun_position.iter().all(|component| component.is_finite()) {
            return Err(SrpEvaluationError::NonFiniteEphemerisPosition);
        }
        let spacecraft_position = state.position().to_metres();
        let to_sun = [
            sun_position[0] - spacecraft_position[0],
            sun_position[1] - spacecraft_position[1],
            sun_position[2] - spacecraft_position[2],
        ];
        let distance_squared = to_sun.iter().map(|value| value * value).sum::<f64>();
        let distance = distance_squared.sqrt();
        if !distance.is_finite() || distance <= 0.0 {
            return Err(SrpEvaluationError::InvalidSunDistance);
        }
        let flux = self.flux.flux_at(epoch).map_err(SrpEvaluationError::Flux)?;
        let shadow = self.in_umbra(spacecraft_position, sun_position);
        let light_fraction = if shadow { 0.0 } else { 1.0 };
        let scale = light_fraction * flux.watts_per_square_meter() / SPEED_OF_LIGHT_M_PER_S
            * self.coefficient.value().get::<ratio>()
            * self.area.value().get::<square_meter>()
            / self.mass.value().get::<kilogram>()
            * (self.reference_distance_m / distance).powi(2)
            / distance;
        let acceleration = AccelerationVector::from_metres_per_second_squared(
            -scale * to_sun[0],
            -scale * to_sun[1],
            -scale * to_sun[2],
        );
        if !acceleration.is_finite() {
            return Err(SrpEvaluationError::NonFiniteAcceleration);
        }
        FramedAcceleration::new(acceleration, state.frame())
            .map_err(|_| SrpEvaluationError::NonFiniteAcceleration)
    }

    fn validate_state(&self, state: &CartesianState) -> Result<(), SrpEvaluationError<F::Error>> {
        if !state.frame().is_inertial() {
            return Err(SrpEvaluationError::NonInertialFrame);
        }
        if state.frame().origin() != FrameOrigin::Body(self.occulting_body) {
            return Err(SrpEvaluationError::OriginMismatch {
                frame_origin: state.frame().origin(),
                occulting_body: self.occulting_body,
            });
        }
        Ok(())
    }

    fn in_umbra(&self, spacecraft: [f64; 3], sun: [f64; 3]) -> bool {
        let sun_distance = sun.iter().map(|value| value * value).sum::<f64>().sqrt();
        let axis = [
            sun[0] / sun_distance,
            sun[1] / sun_distance,
            sun[2] / sun_distance,
        ];
        let axial = spacecraft[0] * axis[0] + spacecraft[1] * axis[1] + spacecraft[2] * axis[2];
        if axial >= 0.0 {
            return false;
        }
        let perpendicular = [
            spacecraft[0] - axial * axis[0],
            spacecraft[1] - axial * axis[1],
            spacecraft[2] - axial * axis[2],
        ];
        let radius = match self.eclipse {
            EclipseGeometry::CylindricalUmbra {
                occulting_body_radius,
            } => occulting_body_radius.get::<meter>(),
        };
        perpendicular.iter().map(|value| value * value).sum::<f64>() <= radius * radius
    }
}

impl<F> ForceModel for CannonballSolarRadiationPressure<F>
where
    F: SolarFluxProvider,
{
    fn model_name(&self) -> &str {
        "cannonball solar radiation pressure model"
    }

    fn force(&self) -> &dyn Force {
        &SRP_FORCE
    }

    fn state_requirements(&self) -> SpacecraftStateRequirements {
        SpacecraftStateRequirements::POSITION
    }
}

impl<F> NonConservativeForceModel for CannonballSolarRadiationPressure<F> where F: SolarFluxProvider {}

impl<F> EvaluableCartesianForceModel for CannonballSolarRadiationPressure<F>
where
    F: SolarFluxProvider + 'static,
{
    fn validate_cartesian(&self, state: &CartesianState) -> Result<(), CartesianForceModelError> {
        self.validate_state(state)
            .map_err(|source| CartesianForceModelError::new(self.model_name(), source))
    }

    fn cartesian_acceleration(
        &self,
        epoch: Epoch,
        state: &CartesianState,
    ) -> Result<FramedAcceleration, CartesianForceModelError> {
        self.evaluate_typed(epoch, state)
            .map_err(|source| CartesianForceModelError::new(self.model_name(), source))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use frames::FrameKinematics;
    use orbits::cartesian::BodyState;
    use std::io;
    use units::uom::si::{area::square_meter, length::meter, mass::kilogram, ratio::ratio};
    use units::{Position, VelocityVector};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
    #[error("test provider failure")]
    struct TestError;

    #[derive(Debug)]
    struct FixedEphemeris {
        position: Position,
    }

    impl BodyEphemerisProvider for FixedEphemeris {
        fn state_at(
            &self,
            body: Body,
            epoch: Epoch,
            frame: ReferenceFrame,
        ) -> Result<BodyState, BodyEphemerisError> {
            Ok(BodyState::new(
                body,
                epoch,
                FrameKinematics::new(
                    self.position,
                    VelocityVector::from_metres_per_second(0.0, 0.0, 0.0),
                    frame,
                )
                .map_err(|error| BodyEphemerisError::Evaluation {
                    body,
                    epoch: Box::new(epoch),
                    frame,
                    source: Box::new(io::Error::new(
                        io::ErrorKind::InvalidData,
                        error.to_string(),
                    )),
                })?,
            ))
        }
    }

    #[derive(Debug)]
    struct FixedFlux(SolarFlux);

    impl SolarFluxProvider for FixedFlux {
        type Error = TestError;

        fn flux_at(&self, _epoch: Epoch) -> Result<SolarFlux, Self::Error> {
            Ok(self.0)
        }
    }

    fn model(sun_x: f64) -> CannonballSolarRadiationPressure<FixedFlux> {
        CannonballSolarRadiationPressure::new(
            Body::EARTH,
            Length::new::<meter>(1.0e11),
            EclipseGeometry::CylindricalUmbra {
                occulting_body_radius: Length::new::<meter>(1.0),
            },
            Arc::new(FixedEphemeris {
                position: Position::from_metres(sun_x, 0.0, 0.0),
            }),
            Arc::new(FixedFlux(
                SolarFlux::new_watts_per_square_meter(1361.0).expect("flux"),
            )),
            SrpSpacecraft {
                area: SrpArea::new(Area::new::<square_meter>(2.0)).expect("area"),
                mass: SrpMass::new(Mass::new::<kilogram>(100.0)).expect("mass"),
                coefficient: OpticalCoefficient::new(Ratio::new::<ratio>(1.0))
                    .expect("coefficient"),
            },
        )
        .expect("model")
    }

    fn state(x: f64, y: f64) -> CartesianState {
        CartesianState::new(
            ReferenceFrame::GCRF,
            Position::from_metres(x, y, 0.0),
            VelocityVector::from_metres_per_second(0.0, 0.0, 0.0),
        )
        .expect("state")
    }

    #[test]
    fn reference_equation_points_away_from_sun() {
        let acceleration = model(1.0e11)
            .cartesian_acceleration(Epoch::from_tai_seconds(0.0), &state(0.0, 0.0))
            .expect("acceleration")
            .value()
            .to_metres_per_second_squared();
        let expected = 1361.0 / SPEED_OF_LIGHT_M_PER_S * 2.0 / 100.0;
        assert!((acceleration[0] + expected).abs() < 1.0e-18);
        assert_eq!(acceleration[1], 0.0);
    }

    #[test]
    fn inverse_square_scaling_is_invariant_to_reference_distance() {
        let near = model(1.0e11)
            .cartesian_acceleration(Epoch::from_tai_seconds(0.0), &state(0.0, 0.0))
            .expect("near")
            .value()
            .to_metres_per_second_squared();
        let far = model(2.0e11)
            .cartesian_acceleration(Epoch::from_tai_seconds(0.0), &state(0.0, 0.0))
            .expect("far")
            .value()
            .to_metres_per_second_squared();
        assert!((near[0] / far[0] - 4.0).abs() < 1.0e-12);
    }

    #[test]
    fn cylindrical_umbra_zeroes_acceleration_without_affecting_flux_provider() {
        let acceleration = model(1.0e11)
            .cartesian_acceleration(Epoch::from_tai_seconds(0.0), &state(-2.0, 0.0))
            .expect("eclipsed")
            .value()
            .to_metres_per_second_squared();
        assert_eq!(acceleration, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn mismatched_origin_is_rejected() {
        let error = model(1.0e11)
            .cartesian_acceleration(
                Epoch::from_tai_seconds(0.0),
                &CartesianState::new(
                    ReferenceFrame::ICRF,
                    Position::from_metres(0.0, 0.0, 0.0),
                    VelocityVector::from_metres_per_second(0.0, 0.0, 0.0),
                )
                .expect("state"),
            )
            .expect_err("origin mismatch");
        assert!(error.to_string().contains("failed to evaluate"));
    }
}
