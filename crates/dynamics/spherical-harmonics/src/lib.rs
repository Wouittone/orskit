#![forbid(unsafe_code)]

//! A deliberately small, explicit spherical-harmonic gravity boundary.
//!
//! [`SphericalHarmonicGravityModel`] accepts coefficient data and an
//! epoch-qualified body-fixed rotation from the caller. This crate does not
//! load gravity files, select a tide system, or provide Earth-orientation
//! data. The first numerical slice evaluates only the fully-normalized 4π
//! zonal coefficients C̄₂₀ (J₂) and, optionally, C̄₃₀ (J₃), plus C̄₀₀ = 1.
//! All tesseral and sectorial coefficients are rejected. See
//! `.agent/decisions/0047-low-degree-spherical-harmonics-boundary.md`.

use std::{fmt, sync::Arc};

use dynamics::{
    CartesianForceModelError, EvaluableCartesianForceModel, Force, ForceModel,
    SpacecraftStateRequirements,
};
use frames::{
    BodyFixedTransformDirection, BodyFixedTransformProvider, BodyFixedTransformRequest,
    FrameOrigin, InertialFrame, ReferenceFrame,
};
use hifitime::{Epoch, TimeScale};
use orbits::cartesian::{CartesianState, FramedAcceleration};
use thiserror::Error;
use units::uom::si::length::meter;
use units::{AccelerationVector, GravitationalParameter, Length};

/// Normalization convention of the supplied harmonic coefficients.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoefficientNormalization {
    /// Geodesy fully-normalized (4π) coefficients.
    FullyNormalized4Pi,
}

/// Tide-system label attached to a coefficient set.
///
/// The label is intentionally not interpreted: converting between tide
/// systems requires caller-supplied permanent-tide corrections and data
/// provenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TideSystem {
    TideFree,
    ZeroTide,
    MeanTide,
}

/// One dimensionless, normalized coefficient pair.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HarmonicCoefficient {
    pub cosine: f64,
    pub sine: f64,
}

/// Caller-owned coefficient/data contract.
pub trait HarmonicCoefficientProvider: fmt::Debug + Send + Sync {
    fn normalization(&self) -> CoefficientNormalization;
    fn tide_system(&self) -> TideSystem;
    fn maximum_degree(&self) -> u32;
    fn maximum_order(&self) -> u32;
    /// Returns a coefficient, or `None` when the supplied set omits it.
    fn coefficient(&self, degree: u32, order: u32) -> Option<HarmonicCoefficient>;
}

/// Model configuration and physical metadata.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SphericalHarmonicField {
    pub gravitational_parameter: GravitationalParameter,
    pub reference_radius: Length,
    pub origin: FrameOrigin,
    pub inertial_frame: InertialFrame,
    pub body_fixed_frame: ReferenceFrame,
    pub time_scale: TimeScale,
    pub maximum_degree: u32,
    pub maximum_order: u32,
}

/// An evaluable low-degree spherical-harmonic gravity contribution.
#[derive(Clone)]
pub struct SphericalHarmonicGravityModel {
    field: SphericalHarmonicField,
    c20: f64,
    c30: f64,
    coefficients: Arc<dyn HarmonicCoefficientProvider>,
    transform_provider: Arc<dyn BodyFixedTransformProvider>,
}

impl fmt::Debug for SphericalHarmonicGravityModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SphericalHarmonicGravityModel")
            .field("field", &self.field)
            .field("c20", &self.c20)
            .field("c30", &self.c30)
            .finish_non_exhaustive()
    }
}

/// Gravity force identity.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct SphericalHarmonicGravity;

impl Force for SphericalHarmonicGravity {
    fn name(&self) -> &str {
        "caller-supplied spherical-harmonic gravity"
    }
}

static FORCE: SphericalHarmonicGravity = SphericalHarmonicGravity;

/// Construction failures are reported before an integrator can use the model.
#[derive(Debug, Error)]
pub enum ConstructionError {
    #[error("gravitational parameter must be finite and positive")]
    InvalidGravitationalParameter,
    #[error("reference radius must be finite and positive")]
    InvalidReferenceRadius,
    #[error("inertial and body-fixed frames must have the same configured origin")]
    FrameOriginMismatch,
    #[error("only maximum degree 2 or 3 and order 0 are supported")]
    UnsupportedDegreeOrder,
    #[error("coefficient provider normalization must be fully-normalized 4π")]
    UnsupportedNormalization,
    #[error("coefficient provider degree/order coverage is insufficient")]
    InsufficientCoverage,
    #[error("coefficient C({degree},{order}) is non-finite")]
    NonFiniteCoefficient { degree: u32, order: u32 },
    #[error("coefficient S({degree},{order}) must be zero for this zonal slice")]
    NonZeroSineCoefficient { degree: u32, order: u32 },
    #[error("tesseral/sectorial coefficient C/S({degree},{order}) is unsupported")]
    NonZeroTesseralCoefficient { degree: u32, order: u32 },
    #[error("C(0,0) must equal 1 for the central term")]
    InvalidCentralCoefficient,
    #[error("degree-one coefficients must be absent or zero")]
    NonZeroDegreeOne,
    #[error("body-fixed transform request is invalid: {0}")]
    InvalidTransformRequest(#[from] frames::BodyFixedTransformError),
}

/// Evaluation failures retain provider and physical singularity causes.
#[derive(Debug, Error)]
pub enum EvaluationError {
    #[error("Cartesian state frame does not match the configured inertial frame")]
    FrameMismatch,
    #[error("Cartesian state origin does not match the gravity field origin")]
    OriginMismatch,
    #[error("position radius must be finite and non-zero")]
    SingularPosition,
    #[error("body-fixed transform provider failed: {0}")]
    TransformProvider(#[from] frames::BodyFixedTransformProviderError),
    #[error("body-fixed transform response did not match the requested transform")]
    TransformResponseMismatch,
    #[error("body-fixed acceleration is not finite")]
    NonFiniteAcceleration,
}

impl SphericalHarmonicGravityModel {
    /// Validates and constructs a model from caller-owned providers.
    pub fn new(
        field: SphericalHarmonicField,
        coefficients: Arc<dyn HarmonicCoefficientProvider>,
        transform_provider: Arc<dyn BodyFixedTransformProvider>,
    ) -> Result<Self, ConstructionError> {
        if !field
            .gravitational_parameter
            .as_cubic_metres_per_second_squared()
            .is_finite()
            || field
                .gravitational_parameter
                .as_cubic_metres_per_second_squared()
                <= 0.0
        {
            return Err(ConstructionError::InvalidGravitationalParameter);
        }
        if !field.reference_radius.get::<meter>().is_finite()
            || field.reference_radius.get::<meter>() <= 0.0
        {
            return Err(ConstructionError::InvalidReferenceRadius);
        }
        if field.inertial_frame.reference_frame().origin() != field.origin
            || field.body_fixed_frame.origin() != field.origin
        {
            return Err(ConstructionError::FrameOriginMismatch);
        }
        if !(field.maximum_degree == 2 || field.maximum_degree == 3) || field.maximum_order != 0 {
            return Err(ConstructionError::UnsupportedDegreeOrder);
        }
        if coefficients.normalization() != CoefficientNormalization::FullyNormalized4Pi
            || coefficients.maximum_order() < field.maximum_order
            || coefficients.maximum_degree() < field.maximum_degree
        {
            return Err(
                if coefficients.normalization() != CoefficientNormalization::FullyNormalized4Pi {
                    ConstructionError::UnsupportedNormalization
                } else {
                    ConstructionError::InsufficientCoverage
                },
            );
        }
        let coefficient = |degree, order| {
            coefficients
                .coefficient(degree, order)
                .unwrap_or(HarmonicCoefficient {
                    cosine: 0.0,
                    sine: 0.0,
                })
        };
        for degree in 0..=field.maximum_degree {
            let value = coefficient(degree, 0);
            if !value.cosine.is_finite() || !value.sine.is_finite() {
                return Err(ConstructionError::NonFiniteCoefficient { degree, order: 0 });
            }
            if value.sine != 0.0 {
                return Err(ConstructionError::NonZeroSineCoefficient { degree, order: 0 });
            }
        }
        if coefficient(0, 0).cosine != 1.0 {
            return Err(ConstructionError::InvalidCentralCoefficient);
        }
        if coefficient(1, 0).cosine != 0.0 {
            return Err(ConstructionError::NonZeroDegreeOne);
        }
        for degree in 0..=field.maximum_degree {
            for order in 1..=coefficients.maximum_order() {
                let value = coefficient(degree, order);
                if !value.cosine.is_finite() || !value.sine.is_finite() {
                    return Err(ConstructionError::NonFiniteCoefficient { degree, order });
                }
                if value.cosine != 0.0 || value.sine != 0.0 {
                    return Err(ConstructionError::NonZeroTesseralCoefficient { degree, order });
                }
            }
        }
        BodyFixedTransformRequest::new(
            Epoch::from_tai_seconds(0.0),
            field.time_scale,
            field.inertial_frame,
            field.body_fixed_frame,
            BodyFixedTransformDirection::InertialToBodyFixed,
        )?;
        Ok(Self {
            field,
            c20: coefficient(2, 0).cosine,
            c30: if field.maximum_degree == 3 {
                coefficient(3, 0).cosine
            } else {
                0.0
            },
            coefficients,
            transform_provider,
        })
    }

    #[must_use]
    pub const fn field(&self) -> SphericalHarmonicField {
        self.field
    }

    /// Returns the declared coefficient normalization.
    #[must_use]
    pub fn normalization(&self) -> CoefficientNormalization {
        self.coefficients.normalization()
    }

    /// Returns the caller-declared tide-system label without converting it.
    #[must_use]
    pub fn tide_system(&self) -> TideSystem {
        self.coefficients.tide_system()
    }

    /// Returns the caller-owned coefficient provider used to construct this model.
    #[must_use]
    pub fn coefficients(&self) -> &Arc<dyn HarmonicCoefficientProvider> {
        &self.coefficients
    }

    fn evaluate(
        &self,
        epoch: Epoch,
        state: &CartesianState,
    ) -> Result<FramedAcceleration, EvaluationError> {
        if state.frame().origin() != self.field.origin {
            return Err(EvaluationError::OriginMismatch);
        }
        if state.frame() != self.field.inertial_frame.reference_frame() {
            return Err(EvaluationError::FrameMismatch);
        }
        let request = BodyFixedTransformRequest::new(
            epoch,
            self.field.time_scale,
            self.field.inertial_frame,
            self.field.body_fixed_frame,
            BodyFixedTransformDirection::InertialToBodyFixed,
        )
        .map_err(|_| EvaluationError::TransformResponseMismatch)?;
        let transform = self.transform_provider.body_fixed_transform(request)?;
        if transform.request() != request {
            return Err(EvaluationError::TransformResponseMismatch);
        }
        let p = state.position().to_metres();
        let b = transform.rotation().rows();
        let x = b[0][0] * p[0] + b[0][1] * p[1] + b[0][2] * p[2];
        let y = b[1][0] * p[0] + b[1][1] * p[1] + b[1][2] * p[2];
        let z = b[2][0] * p[0] + b[2][1] * p[1] + b[2][2] * p[2];
        let r2 = x.mul_add(x, y.mul_add(y, z * z));
        let r = r2.sqrt();
        if !r.is_finite() || r == 0.0 {
            return Err(EvaluationError::SingularPosition);
        }
        let mu = self
            .field
            .gravitational_parameter
            .as_cubic_metres_per_second_squared();
        let radius = self.field.reference_radius.get::<meter>();
        let j2 = -5.0_f64.sqrt() * self.c20;
        let j3 = -7.0_f64.sqrt() * self.c30;
        let mut a = [-mu * x / (r2 * r), -mu * y / (r2 * r), -mu * z / (r2 * r)];
        let q = z * z / r2;
        let j2_factor = 1.5 * mu * j2 * radius * radius / (r2 * r2 * r);
        a[0] += j2_factor * x * (-1.0 + 5.0 * q);
        a[1] += j2_factor * y * (-1.0 + 5.0 * q);
        a[2] += j2_factor * z * (-3.0 + 5.0 * q);
        let j3_factor = 0.5 * mu * j3 * radius * radius * radius;
        a[0] += j3_factor * x * (-15.0 * z / r.powi(7) + 35.0 * z.powi(3) / r.powi(9));
        a[1] += j3_factor * y * (-15.0 * z / r.powi(7) + 35.0 * z.powi(3) / r.powi(9));
        a[2] +=
            j3_factor * (3.0 / r.powi(5) - 30.0 * z * z / r.powi(7) + 35.0 * z.powi(4) / r.powi(9));
        if a.iter().any(|value| !value.is_finite()) {
            return Err(EvaluationError::NonFiniteAcceleration);
        }
        let at = transform.rotation();
        let rows = at.rows();
        let inertial = [
            rows[0][0] * a[0] + rows[1][0] * a[1] + rows[2][0] * a[2],
            rows[0][1] * a[0] + rows[1][1] * a[1] + rows[2][1] * a[2],
            rows[0][2] * a[0] + rows[1][2] * a[1] + rows[2][2] * a[2],
        ];
        FramedAcceleration::new(
            AccelerationVector::from_metres_per_second_squared(
                inertial[0],
                inertial[1],
                inertial[2],
            ),
            state.frame(),
        )
        .map_err(|_| EvaluationError::NonFiniteAcceleration)
    }
}

impl ForceModel for SphericalHarmonicGravityModel {
    fn model_name(&self) -> &str {
        "low-degree spherical-harmonic gravity model"
    }
    fn force(&self) -> &dyn Force {
        &FORCE
    }
    fn state_requirements(&self) -> SpacecraftStateRequirements {
        SpacecraftStateRequirements::POSITION
    }
}

impl EvaluableCartesianForceModel for SphericalHarmonicGravityModel {
    fn validate_cartesian(&self, state: &CartesianState) -> Result<(), CartesianForceModelError> {
        let error = if state.frame().origin() != self.field.origin {
            EvaluationError::OriginMismatch
        } else if state.frame() != self.field.inertial_frame.reference_frame() {
            EvaluationError::FrameMismatch
        } else {
            return Ok(());
        };
        Err(CartesianForceModelError::new(self.model_name(), error))
    }

    fn cartesian_acceleration(
        &self,
        epoch: Epoch,
        state: &CartesianState,
    ) -> Result<FramedAcceleration, CartesianForceModelError> {
        self.evaluate(epoch, state)
            .map_err(|error| CartesianForceModelError::new(self.model_name(), error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use frames::{BodyFixedTransform, DirectionCosineMatrix};
    use units::{Position, VelocityVector};

    #[derive(Debug)]
    struct Coefficients {
        degree: u32,
        c20: f64,
        c30: f64,
    }
    impl HarmonicCoefficientProvider for Coefficients {
        fn normalization(&self) -> CoefficientNormalization {
            CoefficientNormalization::FullyNormalized4Pi
        }
        fn tide_system(&self) -> TideSystem {
            TideSystem::TideFree
        }
        fn maximum_degree(&self) -> u32 {
            self.degree
        }
        fn maximum_order(&self) -> u32 {
            0
        }
        fn coefficient(&self, degree: u32, order: u32) -> Option<HarmonicCoefficient> {
            Some(HarmonicCoefficient {
                cosine: match (degree, order) {
                    (0, 0) => 1.0,
                    (2, 0) => self.c20,
                    (3, 0) => self.c30,
                    _ => 0.0,
                },
                sine: 0.0,
            })
        }
    }

    #[derive(Debug)]
    struct IdentityTransform;
    impl BodyFixedTransformProvider for IdentityTransform {
        fn body_fixed_transform(
            &self,
            request: BodyFixedTransformRequest,
        ) -> Result<frames::BodyFixedTransform, frames::BodyFixedTransformProviderError> {
            BodyFixedTransform::new(
                request,
                DirectionCosineMatrix::identity(),
                units::AngularVelocityVector::from_radians_per_second(0.0, 0.0, 0.0),
            )
            .map_err(|error| {
                frames::BodyFixedTransformProviderError::InvalidTransform {
                    source: Box::new(error),
                }
            })
        }
    }

    fn model(degree: u32) -> SphericalHarmonicGravityModel {
        SphericalHarmonicGravityModel::new(
            SphericalHarmonicField {
                gravitational_parameter: GravitationalParameter::try_from(3.986004418e14).unwrap(),
                reference_radius: Length::new::<meter>(6378137.0),
                origin: ReferenceFrame::GCRF.origin(),
                inertial_frame: InertialFrame::GCRF,
                body_fixed_frame: ReferenceFrame::ITRF2020,
                time_scale: TimeScale::TAI,
                maximum_degree: degree,
                maximum_order: 0,
            },
            Arc::new(Coefficients {
                degree,
                c20: -1.08262668e-3 / 5.0_f64.sqrt(),
                c30: 2.5324105e-6 / 7.0_f64.sqrt(),
            }),
            Arc::new(IdentityTransform),
        )
        .unwrap()
    }

    fn state(z: f64) -> CartesianState {
        CartesianState::new(
            ReferenceFrame::GCRF,
            Position::from_metres(7e6, -1.2e6, z),
            VelocityVector::from_metres_per_second(0.0, 0.0, 0.0),
        )
        .unwrap()
    }

    #[test]
    fn reference_vector_includes_j2_and_j3() {
        let a = model(3)
            .cartesian_acceleration(Epoch::from_tai_seconds(0.0), &state(2e6))
            .unwrap()
            .value()
            .to_metres_per_second_squared();
        assert!((a[0] - -6.951695164310216).abs() < 1e-12);
        assert!((a[1] - 1.1917191710246084).abs() < 1e-12);
        assert!((a[2] - -1.9910268010315095).abs() < 1e-12);
    }

    #[test]
    fn zonal_symmetry_has_odd_vertical_component() {
        let positive = model(2)
            .cartesian_acceleration(Epoch::from_tai_seconds(0.0), &state(2e6))
            .unwrap()
            .value()
            .to_metres_per_second_squared();
        let negative = model(2)
            .cartesian_acceleration(Epoch::from_tai_seconds(0.0), &state(-2e6))
            .unwrap()
            .value()
            .to_metres_per_second_squared();
        assert!((positive[0] - negative[0]).abs() < 1e-12);
        assert!((positive[1] - negative[1]).abs() < 1e-12);
        assert!((positive[2] + negative[2]).abs() < 1e-12);
    }

    #[test]
    fn zero_zonal_coefficients_reduce_to_point_mass() {
        let mut zero = model(2);
        zero.c20 = 0.0;
        let a = zero
            .cartesian_acceleration(Epoch::from_tai_seconds(0.0), &state(2e6))
            .unwrap()
            .value()
            .to_metres_per_second_squared();
        let r = (7e6_f64.powi(2) + 1.2e6_f64.powi(2) + 2e6_f64.powi(2)).sqrt();
        assert!((a[0] + 3.986004418e14 * 7e6 / r.powi(3)).abs() < 1e-12);
    }

    #[test]
    fn unsupported_tesseral_data_is_rejected() {
        #[derive(Debug)]
        struct Tesseral;
        impl HarmonicCoefficientProvider for Tesseral {
            fn normalization(&self) -> CoefficientNormalization {
                CoefficientNormalization::FullyNormalized4Pi
            }
            fn tide_system(&self) -> TideSystem {
                TideSystem::TideFree
            }
            fn maximum_degree(&self) -> u32 {
                2
            }
            fn maximum_order(&self) -> u32 {
                1
            }
            fn coefficient(&self, degree: u32, order: u32) -> Option<HarmonicCoefficient> {
                Some(HarmonicCoefficient {
                    cosine: f64::from((degree == 0 && order == 0) || (degree == 2 && order == 1)),
                    sine: 0.0,
                })
            }
        }
        let result = SphericalHarmonicGravityModel::new(
            SphericalHarmonicField {
                gravitational_parameter: GravitationalParameter::try_from(1.0).unwrap(),
                reference_radius: Length::new::<meter>(1.0),
                origin: ReferenceFrame::GCRF.origin(),
                inertial_frame: InertialFrame::GCRF,
                body_fixed_frame: ReferenceFrame::ITRF2020,
                time_scale: TimeScale::TAI,
                maximum_degree: 2,
                maximum_order: 0,
            },
            Arc::new(Tesseral),
            Arc::new(IdentityTransform),
        );
        assert!(
            matches!(
                result,
                Err(ConstructionError::NonZeroTesseralCoefficient { .. })
            ),
            "{result:?}"
        );
    }
}
