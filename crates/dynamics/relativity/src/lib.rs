#![forbid(unsafe_code)]

//! Explicit correction-only Schwarzschild first post-Newtonian gravity.
//!
//! [`Schwarzschild1PnCorrection`] evaluates the additional acceleration
//! contribution
//!
//! ```text
//! a_1PN = μ / (c² r³) [(4 μ/r - v²) r + 4 (r·v) v]
//! ```
//!
//! from IERS Conventions (2010), Chapter 10, §10.2, Eq. 10.8. It does **not**
//! include Newtonian monopole acceleration. Compose it explicitly with the
//! selected Newtonian gravity model; the gravitational parameter must match
//! that model.
//!
//! The equation is the test-particle Schwarzschild correction for an isolated,
//! spherical, non-rotating monopole in harmonic coordinates. The supplied
//! state must be central-body-centered, inertial, and expressed in harmonic
//! Cartesian coordinates. The correction omits central-body spin
//! (Lense–Thirring), external-body relativistic terms, and relativistic
//! couplings to nonspherical gravity. The configured expansion parameter
//! bound checks `max(v²/c², μ/(r c²))` but does not establish an accuracy claim;
//! callers must choose a bound appropriate to their application, with values
//! much smaller than one for the weak-field, slow-motion regime.
//!
//! The speed of light is the exact SI value from the project's constants.
//! Inputs and results use typed SI quantities, the evaluation epoch is explicit,
//! and the output acceleration retains the input state frame. This model has no
//! Earth-orientation or external-data dependency.
//!
//! ```
//! use dynamics_relativity::Schwarzschild1PnCorrection;
//! use dynamics::EvaluableCartesianForceModel;
//! use frames::{Body, InertialFrame, ReferenceFrame};
//! use hifitime::Epoch;
//! use units::uom::si::ratio::ratio;
//! use units::{GravitationalParameter, Ratio, Position, VelocityVector};
//! use orbits::cartesian::CartesianState;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let model = Schwarzschild1PnCorrection::new(
//!     ReferenceFrame::GCRF.origin(),
//!     InertialFrame::GCRF,
//!     GravitationalParameter::try_from(3.986_004_418e14)?,
//!     Ratio::new::<ratio>(1e-6),
//! )?;
//! let state = CartesianState::new(
//!     ReferenceFrame::GCRF,
//!     Position::from_metres(7_000_000.0, 0.0, 0.0),
//!     VelocityVector::from_metres_per_second(0.0, 7_500.0, 0.0),
//! )?;
//! let correction = model.cartesian_acceleration(Epoch::from_tai_seconds(0.0), &state)?;
//! assert_eq!(correction.frame(), state.frame());
//! # Ok(())
//! # }
//! ```

use frames::{FrameOrigin, InertialFrame};
use hifitime::Epoch;
use orbits::cartesian::{CartesianState, FramedAcceleration};
use thiserror::Error;
use units::uom::si::{ratio::ratio, velocity::meter_per_second};
use units::{AccelerationVector, GravitationalParameter, Ratio};
use utils::constants::speed_of_light;

use dynamics::{
    CartesianForceModelError, EvaluableCartesianForceModel, Force, ForceModel,
    SpacecraftStateRequirements,
};

/// Physical gravity interaction identity for relativistic contributions.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct RelativisticGravityForce;

impl Force for RelativisticGravityForce {
    fn name(&self) -> &str {
        "relativistic gravity"
    }
}

static FORCE: RelativisticGravityForce = RelativisticGravityForce;

/// Correction-only Schwarzschild first post-Newtonian monopole contribution.
///
/// Construct explicitly with the monopole origin, an inertial frame with that
/// origin, the same typed gravitational parameter used by the Newtonian model,
/// and a caller-selected maximum expansion parameter. This adds only the
/// relativistic correction, never Newtonian gravity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Schwarzschild1PnCorrection {
    origin: FrameOrigin,
    inertial_frame: InertialFrame,
    gravitational_parameter: GravitationalParameter,
    maximum_expansion_parameter: Ratio,
}

impl Schwarzschild1PnCorrection {
    /// Creates a correction with explicit central-body, frame, gravity, and
    /// weak-field/slow-motion regime bound.
    ///
    /// `maximum_expansion_parameter` must be in `(0, 1)` and bounds
    /// `max(v²/c², μ/(r c²))` at evaluation. A smaller value gives a more
    /// conservative regime gate but is not itself an error estimate.
    pub fn new(
        origin: FrameOrigin,
        inertial_frame: InertialFrame,
        gravitational_parameter: GravitationalParameter,
        maximum_expansion_parameter: Ratio,
    ) -> Result<Self, ConfigurationError> {
        let frame_origin = inertial_frame.reference_frame().origin();
        if origin != frame_origin {
            return Err(ConfigurationError::OriginMismatch {
                gravity_origin: origin,
                frame_origin,
            });
        }
        let maximum = maximum_expansion_parameter.get::<ratio>();
        if !maximum.is_finite() || maximum <= 0.0 || maximum >= 1.0 {
            return Err(ConfigurationError::InvalidExpansionParameterBound { maximum });
        }
        Ok(Self {
            origin,
            inertial_frame,
            gravitational_parameter,
            maximum_expansion_parameter,
        })
    }

    /// Returns the configured central monopole origin.
    #[must_use]
    pub const fn origin(self) -> FrameOrigin {
        self.origin
    }

    /// Returns the configured inertial frame.
    #[must_use]
    pub const fn inertial_frame(self) -> InertialFrame {
        self.inertial_frame
    }

    /// Returns the monopole parameter used by this correction.
    #[must_use]
    pub const fn gravitational_parameter(self) -> GravitationalParameter {
        self.gravitational_parameter
    }

    /// Returns the configured maximum post-Newtonian expansion parameter.
    #[must_use]
    pub const fn maximum_expansion_parameter(self) -> Ratio {
        self.maximum_expansion_parameter
    }

    fn validate_state(&self, state: &CartesianState) -> Result<(), EvaluationError> {
        if state.frame().origin() != self.origin {
            return Err(EvaluationError::OriginMismatch {
                expected: self.origin,
                actual: state.frame().origin(),
            });
        }
        if state.frame() != self.inertial_frame.reference_frame() {
            return Err(EvaluationError::FrameMismatch {
                expected: Box::new(self.inertial_frame.reference_frame()),
                actual: Box::new(state.frame()),
            });
        }
        Ok(())
    }

    fn evaluate(&self, state: &CartesianState) -> Result<FramedAcceleration, EvaluationError> {
        self.validate_state(state)?;
        let position = state.position().to_metres();
        let velocity = state.velocity().to_metres_per_second();
        let radius_squared = dot(position, position);
        if !radius_squared.is_finite() {
            return Err(EvaluationError::NonFiniteGeometry);
        }
        if radius_squared == 0.0 {
            return Err(EvaluationError::SingularPosition);
        }
        let radius = radius_squared.sqrt();
        let speed_squared = dot(velocity, velocity);
        let position_velocity_dot = dot(position, velocity);
        if !radius.is_finite() || !speed_squared.is_finite() || !position_velocity_dot.is_finite() {
            return Err(EvaluationError::NonFiniteGeometry);
        }

        let mu = self
            .gravitational_parameter
            .as_cubic_metres_per_second_squared();
        let c = speed_of_light().get::<meter_per_second>();
        let c_squared = c * c;
        let expansion_parameter = (speed_squared / c_squared).max((mu / radius) / c_squared);
        let maximum = self.maximum_expansion_parameter.get::<ratio>();
        if !expansion_parameter.is_finite() {
            return Err(EvaluationError::NonFiniteGeometry);
        }
        if expansion_parameter > maximum {
            return Err(EvaluationError::OutsideConfiguredRegime {
                expansion_parameter,
                maximum,
            });
        }

        let scale = ((mu / radius_squared) / radius) / c_squared;
        let radial_factor = 4.0 * mu / radius - speed_squared;
        let correction: [f64; 3] = std::array::from_fn(|axis| {
            scale
                * radial_factor
                    .mul_add(position[axis], 4.0 * position_velocity_dot * velocity[axis])
        });
        if correction.iter().any(|component| !component.is_finite()) {
            return Err(EvaluationError::NonFiniteAcceleration);
        }

        FramedAcceleration::new(
            AccelerationVector::from_metres_per_second_squared(
                correction[0],
                correction[1],
                correction[2],
            ),
            state.frame(),
        )
        .map_err(|_| EvaluationError::NonFiniteAcceleration)
    }
}

impl ForceModel for Schwarzschild1PnCorrection {
    fn model_name(&self) -> &str {
        "Schwarzschild first post-Newtonian monopole correction"
    }

    fn force(&self) -> &dyn Force {
        &FORCE
    }

    fn state_requirements(&self) -> SpacecraftStateRequirements {
        SpacecraftStateRequirements::POSITION.union(SpacecraftStateRequirements::VELOCITY)
    }
}

impl EvaluableCartesianForceModel for Schwarzschild1PnCorrection {
    fn validate_cartesian(&self, state: &CartesianState) -> Result<(), CartesianForceModelError> {
        self.validate_state(state)
            .map_err(|source| CartesianForceModelError::new(self.model_name(), source))
    }

    fn cartesian_acceleration(
        &self,
        _epoch: Epoch,
        state: &CartesianState,
    ) -> Result<FramedAcceleration, CartesianForceModelError> {
        self.evaluate(state)
            .map_err(|source| CartesianForceModelError::new(self.model_name(), source))
    }
}

/// Invalid Schwarzschild correction configuration.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConfigurationError {
    /// The configured inertial frame does not share the monopole origin.
    #[error("gravity origin {gravity_origin} does not match inertial-frame origin {frame_origin}")]
    OriginMismatch {
        /// Origin of the selected monopole.
        gravity_origin: FrameOrigin,
        /// Origin carried by the selected inertial frame.
        frame_origin: FrameOrigin,
    },
    /// The maximum weak-field/slow-motion expansion parameter is not in `(0, 1)`.
    #[error(
        "maximum post-Newtonian expansion parameter must be finite and in (0, 1), got {maximum}"
    )]
    InvalidExpansionParameterBound {
        /// Rejected dimensionless bound.
        maximum: f64,
    },
}

/// Recoverable state and evaluation failures.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EvaluationError {
    /// The propagated state is centered on another origin.
    #[error("Cartesian state origin {actual} does not match configured gravity origin {expected}")]
    OriginMismatch {
        /// Configured central origin.
        expected: FrameOrigin,
        /// Actual state origin.
        actual: FrameOrigin,
    },
    /// The propagated state uses another or non-inertial frame.
    #[error("Cartesian state frame {actual} does not match configured inertial frame {expected}")]
    FrameMismatch {
        /// Configured inertial frame.
        expected: Box<frames::ReferenceFrame>,
        /// Actual state frame.
        actual: Box<frames::ReferenceFrame>,
    },
    /// The state is at the monopole center.
    #[error("Schwarzschild correction is singular at zero radius")]
    SingularPosition,
    /// Finite state components produced non-finite geometry.
    #[error("Schwarzschild correction geometry is not finite")]
    NonFiniteGeometry,
    /// The state exceeds the explicitly configured post-Newtonian validity bound.
    #[error("post-Newtonian expansion parameter {expansion_parameter} exceeds configured maximum {maximum}")]
    OutsideConfiguredRegime {
        /// `max(v²/c², μ/(r c²))` for the rejected state.
        expansion_parameter: f64,
        /// Configured maximum dimensionless expansion parameter.
        maximum: f64,
    },
    /// The computed correction is non-finite.
    #[error("Schwarzschild correction acceleration is not finite")]
    NonFiniteAcceleration,
}

fn dot(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0].mul_add(right[0], left[1].mul_add(right[1], left[2] * right[2]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use frames::{Body, FrameOrientation, ReferenceFrame};
    use std::error::Error as _;
    use units::{Position, VelocityVector};

    fn model(maximum: f64) -> Schwarzschild1PnCorrection {
        Schwarzschild1PnCorrection::new(
            ReferenceFrame::GCRF.origin(),
            InertialFrame::GCRF,
            GravitationalParameter::try_from(3.986_004_418e14).unwrap(),
            Ratio::new::<ratio>(maximum),
        )
        .unwrap()
    }

    fn state(position: [f64; 3], velocity: [f64; 3]) -> CartesianState {
        CartesianState::new(
            ReferenceFrame::GCRF,
            Position::from_metres(position[0], position[1], position[2]),
            VelocityVector::from_metres_per_second(velocity[0], velocity[1], velocity[2]),
        )
        .unwrap()
    }

    #[test]
    fn matches_independent_iers_schwarzschild_reference_vector() {
        let result = model(1e-6)
            .cartesian_acceleration(
                Epoch::from_tai_seconds(0.0),
                &state(
                    [7_000_000.0, -1_200_000.0, 2_000_000.0],
                    [1_200.0, 7_400.0, -900.0],
                ),
            )
            .unwrap();
        assert_eq!(result.frame(), ReferenceFrame::GCRF);
        let acceleration = result.value().to_metres_per_second_squared();
        let expected = [
            1.217_444_092_338_802e-8,
            -2.852_915_220_469_650_3e-9,
            3.603_562_958_057_28e-9,
        ];
        for (actual, expected) in acceleration.into_iter().zip(expected) {
            assert!((actual - expected).abs() < 3e-23, "{actual} != {expected}");
        }
    }

    #[test]
    fn static_state_has_a_purely_radial_monopole_correction() {
        let result = model(1e-6)
            .cartesian_acceleration(
                Epoch::from_tai_seconds(0.0),
                &state([7e6, 0.0, 0.0], [0.0, 0.0, 0.0]),
            )
            .unwrap()
            .value()
            .to_metres_per_second_squared();
        assert!(result[0] > 0.0);
        assert_eq!(result[1], 0.0);
        assert_eq!(result[2], 0.0);
        assert!(result[0] < 3e-8);
    }

    #[test]
    fn rejects_origin_frame_and_singularity_mismatches() {
        let model = model(1e-6);
        let mars_origin = FrameOrigin::Body(Body::MARS);
        let wrong_origin = CartesianState::new(
            ReferenceFrame::new(mars_origin, FrameOrientation::Gcrf),
            Position::from_metres(7e6, 0.0, 0.0),
            VelocityVector::from_metres_per_second(0.0, 7_500.0, 0.0),
        )
        .unwrap();
        let error = model.validate_cartesian(&wrong_origin).unwrap_err();
        assert!(matches!(
            error
                .source()
                .and_then(|source| source.downcast_ref::<EvaluationError>()),
            Some(EvaluationError::OriginMismatch { .. })
        ));

        let wrong_frame = CartesianState::new(
            ReferenceFrame::EME2000,
            Position::from_metres(7e6, 0.0, 0.0),
            VelocityVector::from_metres_per_second(0.0, 7_500.0, 0.0),
        )
        .unwrap();
        let error = model.validate_cartesian(&wrong_frame).unwrap_err();
        assert!(matches!(
            error
                .source()
                .and_then(|source| source.downcast_ref::<EvaluationError>()),
            Some(EvaluationError::FrameMismatch { .. })
        ));

        let zero = state([0.0; 3], [0.0; 3]);
        let error = model.cartesian_acceleration(Epoch::from_tai_seconds(0.0), &zero);
        assert!(matches!(
            error
                .unwrap_err()
                .source()
                .and_then(|source| source.downcast_ref()),
            Some(EvaluationError::SingularPosition)
        ));

        let invalid = Schwarzschild1PnCorrection::new(
            mars_origin,
            InertialFrame::GCRF,
            GravitationalParameter::try_from(4.0e14).unwrap(),
            Ratio::new::<ratio>(1e-6),
        );
        assert!(matches!(
            invalid,
            Err(ConfigurationError::OriginMismatch { .. })
        ));

        let invalid_bound = Schwarzschild1PnCorrection::new(
            ReferenceFrame::GCRF.origin(),
            InertialFrame::GCRF,
            GravitationalParameter::try_from(4.0e14).unwrap(),
            Ratio::new::<ratio>(0.0),
        );
        assert!(matches!(
            invalid_bound,
            Err(ConfigurationError::InvalidExpansionParameterBound { .. })
        ));
    }

    #[test]
    fn rejects_non_finite_intermediate_geometry() {
        let large_finite_state = state([f64::MAX, 0.0, 0.0], [0.0; 3]);
        let error =
            model(1e-6).cartesian_acceleration(Epoch::from_tai_seconds(0.0), &large_finite_state);
        assert!(matches!(
            error
                .unwrap_err()
                .source()
                .and_then(|source| source.downcast_ref()),
            Some(EvaluationError::NonFiniteGeometry)
        ));
    }

    #[test]
    fn rejects_states_outside_the_configured_post_newtonian_regime() {
        let error = model(1e-10).cartesian_acceleration(
            Epoch::from_tai_seconds(0.0),
            &state([7e6, 0.0, 0.0], [0.0, 7_500.0, 0.0]),
        );
        assert!(matches!(
            error
                .unwrap_err()
                .source()
                .and_then(|source| source.downcast_ref()),
            Some(EvaluationError::OutsideConfiguredRegime { .. })
        ));
    }

    #[test]
    fn state_requirements_include_position_and_velocity() {
        let requirements = model(1e-6).state_requirements();
        assert!(requirements.contains(SpacecraftStateRequirements::POSITION));
        assert!(requirements.contains(SpacecraftStateRequirements::VELOCITY));
    }
}
