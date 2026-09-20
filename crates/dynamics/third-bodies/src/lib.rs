#![forbid(unsafe_code)]

//! Differential third-body gravitational acceleration.
//!
//! [`DifferentialThirdBodyModel`] is intended for propagation about one
//! central body. It evaluates
//! `μ₃ ((r₃ - r) / |r₃ - r|³ - r₃ / |r₃|³)`, where `r` is the spacecraft
//! position and `r₃` is the perturbing body's position, both relative to the
//! selected central body in the same inertial frame. The second term removes
//! the perturbing body's acceleration of that central origin. This crate does
//! not offer an absolute convention: use a model whose documented origin
//! matches the propagation coordinates.
//!
//! Both the ephemeris and third-body gravitational parameter are explicit
//! caller-supplied providers. No physical constants, ephemeris files, or
//! ambient data selection are bundled with this crate.
//!
//! # Provenance
//!
//! This implementation and its reference vectors are original clean-room
//! work. It consumes only the project's public body, ephemeris, gravity, and
//! dynamics contracts; no external implementation, ephemeris data set, or
//! copied test vector was used.

use std::sync::Arc;

use bodies::Body;
use dynamics::{
    CartesianForceModelError, EvaluableCartesianForceModel, Force, ForceModel,
    SpacecraftStateRequirements,
};
use frames::{FrameOrigin, InertialFrame, ReferenceFrame};
use gravity::SharedCentralGravity;
use hifitime::Epoch;
use orbits::cartesian::{
    BodyEphemerisError, BodyEphemerisProvider, CartesianState, FramedAcceleration,
};
use thiserror::Error;
use units::{AccelerationVector, Position};

/// The acceleration convention implemented by [`DifferentialThirdBodyModel`].
///
/// The model has exactly one convention to avoid accidentally applying an
/// origin-relative formula to an absolute state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThirdBodyAccelerationConvention {
    /// Acceleration relative to a central body at the state-frame origin.
    DifferentialCentralBody,
}

/// Physical interaction represented by a third-body point-mass model.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ThirdBodyGravityForce;

impl Force for ThirdBodyGravityForce {
    fn name(&self) -> &str {
        "third-body gravity"
    }
}

static THIRD_BODY_GRAVITY_FORCE: ThirdBodyGravityForce = ThirdBodyGravityForce;

/// Differential third-body evaluation or construction failure.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ThirdBodyError {
    /// The propagated central and perturbing bodies must differ.
    #[error("central body {central_body} must differ from third body {third_body}")]
    IdenticalBodies {
        /// Body at the propagated state-frame origin.
        central_body: Body,
        /// Perturbing body.
        third_body: Body,
    },
    /// The supplied gravity provider does not identify the selected third body.
    #[error("third-body gravity origin {gravity_origin:?} does not match third body {third_body}")]
    GravityOriginMismatch {
        /// Origin declared by the caller-supplied gravity provider.
        gravity_origin: FrameOrigin,
        /// Perturbing body selected for this model.
        third_body: Body,
    },
    /// Differential third-body propagation requires explicitly inertial axes.
    #[error("differential third-body acceleration requires explicitly inertial axes")]
    NonInertialFrame,
    /// The state must be centered on the model's selected central body.
    #[error("state frame origin {frame_origin:?} does not match central body {central_body}")]
    CentralOriginMismatch {
        /// Origin carried by the propagated state frame.
        frame_origin: FrameOrigin,
        /// Central body selected for this model.
        central_body: Body,
    },
    /// The caller-supplied ephemeris could not resolve the requested state.
    #[error("third-body ephemeris evaluation failed")]
    Ephemeris {
        /// Typed failure from the explicit ephemeris provider.
        #[source]
        source: BodyEphemerisError,
    },
    /// An ephemeris returned a state for a body other than the requested one.
    #[error("ephemeris returned body {actual_body} instead of requested body {requested_body}")]
    EphemerisBodyMismatch {
        /// Body requested from the provider.
        requested_body: Body,
        /// Body returned by the provider.
        actual_body: Body,
    },
    /// An ephemeris returned a state at a different epoch.
    #[error("ephemeris returned a state at an epoch other than the requested epoch")]
    EphemerisEpochMismatch,
    /// An ephemeris returned kinematics in a different frame.
    #[error(
        "ephemeris returned frame {actual_frame:?} instead of requested frame {requested_frame:?}"
    )]
    EphemerisFrameMismatch {
        /// Frame requested from the provider.
        requested_frame: Box<ReferenceFrame>,
        /// Frame returned by the provider.
        actual_frame: Box<ReferenceFrame>,
    },
    /// The provider result contained a non-finite position.
    #[error("ephemeris returned a non-finite third-body position")]
    NonFiniteEphemerisPosition,
    /// The perturbing body coincides with the central origin.
    #[error("third-body acceleration is singular when the third body is at the central origin")]
    ThirdBodyAtCentralOrigin,
    /// The spacecraft coincides with the perturbing body.
    #[error("third-body acceleration is singular when spacecraft and third body coincide")]
    ThirdBodyCollision,
    /// A finite input produced non-finite intermediate geometry.
    #[error("third-body geometry is not finite")]
    NonFiniteGeometry,
    /// The resulting third-body acceleration is not finite.
    #[error("third-body acceleration is not finite")]
    NonFiniteAcceleration,
}

/// A point-mass third-body contribution relative to a selected central body.
///
/// `central_body` specifies the required `FrameOrigin::Body` of every
/// propagated state. `third_body_gravity` supplies the perturbing body's
/// sourced standard gravitational parameter and must declare that body's
/// origin. `ephemeris` is requested explicitly at each evaluation in the
/// state frame; its returned body, epoch, and frame are verified before use.
#[derive(Debug)]
pub struct DifferentialThirdBodyModel {
    central_body: Body,
    third_body: Body,
    third_body_gravity: SharedCentralGravity,
    ephemeris: Arc<dyn BodyEphemerisProvider>,
}

impl DifferentialThirdBodyModel {
    /// Builds a differential third-body model from explicit data providers.
    pub fn new(
        central_body: Body,
        third_body: Body,
        third_body_gravity: SharedCentralGravity,
        ephemeris: Arc<dyn BodyEphemerisProvider>,
    ) -> Result<Self, ThirdBodyError> {
        if central_body == third_body {
            return Err(ThirdBodyError::IdenticalBodies {
                central_body,
                third_body,
            });
        }
        let gravity_origin = third_body_gravity.origin();
        if gravity_origin != FrameOrigin::Body(third_body) {
            return Err(ThirdBodyError::GravityOriginMismatch {
                gravity_origin,
                third_body,
            });
        }
        Ok(Self {
            central_body,
            third_body,
            third_body_gravity,
            ephemeris,
        })
    }

    /// Returns the central body fixed at the propagated frame origin.
    #[must_use]
    pub const fn central_body(&self) -> Body {
        self.central_body
    }

    /// Returns the perturbing body.
    #[must_use]
    pub const fn third_body(&self) -> Body {
        self.third_body
    }

    /// Returns the selected acceleration convention.
    #[must_use]
    pub const fn convention(&self) -> ThirdBodyAccelerationConvention {
        ThirdBodyAccelerationConvention::DifferentialCentralBody
    }

    /// Returns the caller-supplied, sourced third-body gravity provider.
    #[must_use]
    pub const fn third_body_gravity(&self) -> &SharedCentralGravity {
        &self.third_body_gravity
    }

    /// Returns the caller-supplied ephemeris provider.
    #[must_use]
    pub const fn ephemeris(&self) -> &Arc<dyn BodyEphemerisProvider> {
        &self.ephemeris
    }

    fn validate_state(&self, state: &CartesianState) -> Result<(), ThirdBodyError> {
        InertialFrame::try_from(state.frame()).map_err(|_| ThirdBodyError::NonInertialFrame)?;
        let frame_origin = state.frame().origin();
        if frame_origin != FrameOrigin::Body(self.central_body) {
            return Err(ThirdBodyError::CentralOriginMismatch {
                frame_origin,
                central_body: self.central_body,
            });
        }
        Ok(())
    }

    fn evaluate(
        &self,
        epoch: Epoch,
        state: &CartesianState,
    ) -> Result<FramedAcceleration, ThirdBodyError> {
        self.validate_state(state)?;

        let body_state = self
            .ephemeris
            .state_at(self.third_body, epoch, state.frame())
            .map_err(|source| ThirdBodyError::Ephemeris { source })?;
        if body_state.body() != self.third_body {
            return Err(ThirdBodyError::EphemerisBodyMismatch {
                requested_body: self.third_body,
                actual_body: body_state.body(),
            });
        }
        if body_state.epoch() != epoch {
            return Err(ThirdBodyError::EphemerisEpochMismatch);
        }
        let kinematics = body_state.kinematics();
        if kinematics.frame() != state.frame() {
            return Err(ThirdBodyError::EphemerisFrameMismatch {
                requested_frame: Box::new(state.frame()),
                actual_frame: Box::new(kinematics.frame()),
            });
        }
        let third_position = kinematics.position();
        if !third_position.is_finite() {
            return Err(ThirdBodyError::NonFiniteEphemerisPosition);
        }

        let relative_to_third = subtract(third_position, state.position());
        let direct = inverse_cube_unit_vector(third_position).map_err(|non_finite| {
            if non_finite {
                ThirdBodyError::NonFiniteGeometry
            } else {
                ThirdBodyError::ThirdBodyAtCentralOrigin
            }
        })?;
        let spacecraft = inverse_cube_unit_vector(relative_to_third).map_err(|non_finite| {
            if non_finite {
                ThirdBodyError::NonFiniteGeometry
            } else {
                ThirdBodyError::ThirdBodyCollision
            }
        })?;
        let mu = self
            .third_body_gravity
            .parameter()
            .as_cubic_metres_per_second_squared();
        let components: [f64; 3] =
            std::array::from_fn(|index| mu * (spacecraft[index] - direct[index]));
        if !components.iter().all(|component| component.is_finite()) {
            return Err(ThirdBodyError::NonFiniteAcceleration);
        }
        FramedAcceleration::new(
            AccelerationVector::from_metres_per_second_squared(
                components[0],
                components[1],
                components[2],
            ),
            state.frame(),
        )
        .map_err(|_| ThirdBodyError::NonFiniteAcceleration)
    }
}

impl ForceModel for DifferentialThirdBodyModel {
    fn model_name(&self) -> &str {
        "differential third-body gravity model"
    }

    fn force(&self) -> &dyn Force {
        &THIRD_BODY_GRAVITY_FORCE
    }

    fn state_requirements(&self) -> SpacecraftStateRequirements {
        SpacecraftStateRequirements::POSITION
    }
}

impl EvaluableCartesianForceModel for DifferentialThirdBodyModel {
    fn validate_cartesian(&self, state: &CartesianState) -> Result<(), CartesianForceModelError> {
        self.validate_state(state)
            .map_err(|source| CartesianForceModelError::new(self.model_name(), source))
    }

    fn cartesian_acceleration(
        &self,
        epoch: Epoch,
        state: &CartesianState,
    ) -> Result<FramedAcceleration, CartesianForceModelError> {
        self.evaluate(epoch, state)
            .map_err(|source| CartesianForceModelError::new(self.model_name(), source))
    }
}

fn subtract(left: Position, right: Position) -> Position {
    let [left_x, left_y, left_z] = left.to_metres();
    let [right_x, right_y, right_z] = right.to_metres();
    Position::from_metres(left_x - right_x, left_y - right_y, left_z - right_z)
}

fn inverse_cube_unit_vector(position: Position) -> Result<[f64; 3], bool> {
    let components = position.to_metres();
    let radius_squared = components
        .iter()
        .fold(0.0, |sum, component| component.mul_add(*component, sum));
    if !radius_squared.is_finite() {
        return Err(true);
    }
    let radius = radius_squared.sqrt();
    if radius == 0.0 {
        return Err(false);
    }
    let inverse_radius_cubed = 1.0 / (radius_squared * radius);
    let result = components.map(|component| component * inverse_radius_cubed);
    if result.iter().all(|component| component.is_finite()) {
        Ok(result)
    } else {
        Err(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use frames::FrameKinematics;
    use gravity::CentralGravityProvider;
    use std::error::Error as _;
    use units::{GravitationalParameter, VelocityVector};

    #[derive(Debug)]
    struct TestGravity {
        origin: FrameOrigin,
        parameter: GravitationalParameter,
    }

    impl CentralGravityProvider for TestGravity {
        fn origin(&self) -> FrameOrigin {
            self.origin
        }

        fn parameter(&self) -> GravitationalParameter {
            self.parameter
        }
    }

    #[derive(Debug)]
    struct FixedEphemeris {
        state: orbits::cartesian::BodyState,
    }

    impl BodyEphemerisProvider for FixedEphemeris {
        fn state_at(
            &self,
            _body: Body,
            _epoch: Epoch,
            _frame: ReferenceFrame,
        ) -> Result<orbits::cartesian::BodyState, BodyEphemerisError> {
            Ok(self.state)
        }
    }

    fn model(third_position: Position) -> DifferentialThirdBodyModel {
        let epoch = Epoch::from_tai_seconds(120.0);
        let ephemeris = Arc::new(FixedEphemeris {
            state: orbits::cartesian::BodyState::new(
                Body::MOON,
                epoch,
                FrameKinematics::new(
                    third_position,
                    VelocityVector::from_metres_per_second(0.0, 0.0, 0.0),
                    ReferenceFrame::GCRF,
                )
                .expect("finite ephemeris kinematics"),
            ),
        });
        DifferentialThirdBodyModel::new(
            Body::EARTH,
            Body::MOON,
            Arc::new(TestGravity {
                origin: FrameOrigin::Body(Body::MOON),
                parameter: GravitationalParameter::try_from(4.0).expect("positive parameter"),
            }),
            ephemeris,
        )
        .expect("valid model")
    }

    fn state(position: Position) -> CartesianState {
        CartesianState::new(
            ReferenceFrame::GCRF,
            position,
            VelocityVector::from_metres_per_second(0.0, 0.0, 0.0),
        )
        .expect("finite state")
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() <= 1e-15,
            "actual {actual:.17e} differs from expected {expected:.17e}"
        );
    }

    #[test]
    fn computes_independently_calculated_differential_reference_vector() {
        let model = model(Position::from_metres(4.0, 3.0, 0.0));
        let acceleration = model
            .cartesian_acceleration(
                Epoch::from_tai_seconds(120.0),
                &state(Position::from_metres(1.0, -2.0, 0.0)),
            )
            .expect("evaluation succeeds");

        // Independent scalar calculation: R=(4,3,0), R-r=(3,5,0), μ=4.
        let expected_x = 4.0 * (3.0 / 34.0_f64.powf(1.5) - 4.0 / 25.0_f64.powf(1.5));
        let expected_y = 4.0 * (5.0 / 34.0_f64.powf(1.5) - 3.0 / 25.0_f64.powf(1.5));
        let [actual_x, actual_y, actual_z] = acceleration.value().to_metres_per_second_squared();
        assert_close(actual_x, expected_x);
        assert_close(actual_y, expected_y);
        assert_close(actual_z, 0.0);
        assert_eq!(acceleration.frame(), ReferenceFrame::GCRF);
    }

    #[test]
    fn differential_acceleration_vanishes_at_the_central_origin() {
        let acceleration = model(Position::from_metres(4.0, -3.0, 2.0))
            .cartesian_acceleration(
                Epoch::from_tai_seconds(120.0),
                &state(Position::from_metres(0.0, 0.0, 0.0)),
            )
            .expect("evaluation succeeds")
            .value()
            .to_metres_per_second_squared();

        assert_eq!(acceleration, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn construction_rejects_unsourced_or_mismatched_gravity_origin() {
        let ephemeris = Arc::new(FixedEphemeris {
            state: orbits::cartesian::BodyState::new(
                Body::MOON,
                Epoch::from_tai_seconds(120.0),
                FrameKinematics::new(
                    Position::from_metres(1.0, 0.0, 0.0),
                    VelocityVector::from_metres_per_second(0.0, 0.0, 0.0),
                    ReferenceFrame::GCRF,
                )
                .expect("finite kinematics"),
            ),
        });
        let result = DifferentialThirdBodyModel::new(
            Body::EARTH,
            Body::MOON,
            Arc::new(TestGravity {
                origin: FrameOrigin::Body(Body::EARTH),
                parameter: GravitationalParameter::try_from(1.0).expect("positive parameter"),
            }),
            ephemeris,
        );

        assert!(matches!(
            result,
            Err(ThirdBodyError::GravityOriginMismatch {
                gravity_origin: FrameOrigin::Body(Body::EARTH),
                third_body: Body::MOON
            })
        ));
    }

    #[test]
    fn validation_rejects_noninertial_and_wrong_origin_states() {
        let model = model(Position::from_metres(4.0, 3.0, 0.0));
        let noninertial = CartesianState::new(
            ReferenceFrame::ITRF2020,
            Position::from_metres(1.0, 0.0, 0.0),
            VelocityVector::from_metres_per_second(0.0, 0.0, 0.0),
        )
        .expect("finite state");
        let noninertial_error = model
            .validate_cartesian(&noninertial)
            .expect_err("non-inertial state must fail");
        assert!(matches!(
            noninertial_error
                .source()
                .and_then(|source| source.downcast_ref::<ThirdBodyError>()),
            Some(ThirdBodyError::NonInertialFrame)
        ));

        let wrong_origin = CartesianState::new(
            ReferenceFrame::ICRF,
            Position::from_metres(1.0, 0.0, 0.0),
            VelocityVector::from_metres_per_second(0.0, 0.0, 0.0),
        )
        .expect("finite state");
        let wrong_origin_error = model
            .validate_cartesian(&wrong_origin)
            .expect_err("wrong origin state must fail");
        assert!(matches!(
            wrong_origin_error
                .source()
                .and_then(|source| source.downcast_ref::<ThirdBodyError>()),
            Some(ThirdBodyError::CentralOriginMismatch { .. })
        ));
    }

    #[test]
    fn ephemeris_result_identity_and_singularities_are_rejected() {
        let epoch = Epoch::from_tai_seconds(120.0);
        let wrong_body_model = DifferentialThirdBodyModel::new(
            Body::EARTH,
            Body::MOON,
            Arc::new(TestGravity {
                origin: FrameOrigin::Body(Body::MOON),
                parameter: GravitationalParameter::try_from(1.0).expect("positive parameter"),
            }),
            Arc::new(FixedEphemeris {
                state: orbits::cartesian::BodyState::new(
                    Body::SUN,
                    epoch,
                    FrameKinematics::new(
                        Position::from_metres(2.0, 0.0, 0.0),
                        VelocityVector::from_metres_per_second(0.0, 0.0, 0.0),
                        ReferenceFrame::GCRF,
                    )
                    .expect("finite kinematics"),
                ),
            }),
        )
        .expect("valid model");
        let wrong_body_error = wrong_body_model
            .cartesian_acceleration(epoch, &state(Position::from_metres(0.0, 0.0, 0.0)))
            .expect_err("mismatched body must fail");
        assert!(matches!(
            wrong_body_error
                .source()
                .and_then(|source| source.downcast_ref::<ThirdBodyError>()),
            Some(ThirdBodyError::EphemerisBodyMismatch { .. })
        ));

        let central_origin_error = model(Position::from_metres(0.0, 0.0, 0.0))
            .cartesian_acceleration(epoch, &state(Position::from_metres(1.0, 0.0, 0.0)))
            .expect_err("third body at central origin must fail");
        assert!(matches!(
            central_origin_error
                .source()
                .and_then(|source| source.downcast_ref::<ThirdBodyError>()),
            Some(ThirdBodyError::ThirdBodyAtCentralOrigin)
        ));
        let collision_error = model(Position::from_metres(1.0, 0.0, 0.0))
            .cartesian_acceleration(epoch, &state(Position::from_metres(1.0, 0.0, 0.0)))
            .expect_err("spacecraft/third-body collision must fail");
        assert!(matches!(
            collision_error
                .source()
                .and_then(|source| source.downcast_ref::<ThirdBodyError>()),
            Some(ThirdBodyError::ThirdBodyCollision)
        ));
    }
}
