//! Two-body dynamics with dimensionally typed and framed inputs and outputs.

use orskit_core::frames::ReferenceFrame;
use orskit_core::{FramedAcceleration, FramedVelocity, SpacecraftState};
use orskit_units::{AccelerationVector, GravitationalParameter};
use thiserror::Error;

/// Time derivative of a translational spacecraft state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TranslationalDerivative {
    position_rate: FramedVelocity,
    velocity_rate: FramedAcceleration,
}

impl TranslationalDerivative {
    /// Constructs a typed and framed translational derivative.
    #[must_use]
    pub const fn new(position_rate: FramedVelocity, velocity_rate: FramedAcceleration) -> Self {
        Self {
            position_rate,
            velocity_rate,
        }
    }

    /// Derivative of position, which is velocity.
    #[must_use]
    pub const fn position_rate(self) -> FramedVelocity {
        self.position_rate
    }

    /// Derivative of velocity, which is acceleration.
    #[must_use]
    pub const fn velocity_rate(self) -> FramedAcceleration {
        self.velocity_rate
    }
}

/// Point-mass two-body gravitational dynamics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TwoBodyDynamics {
    mu: GravitationalParameter,
}

impl TwoBodyDynamics {
    /// Constructs the model with an explicit central-body gravitational
    /// parameter. No celestial body or constant is selected implicitly.
    #[must_use]
    pub const fn new(mu: GravitationalParameter) -> Self {
        Self { mu }
    }

    /// Returns the configured gravitational parameter.
    #[must_use]
    pub const fn gravitational_parameter(self) -> GravitationalParameter {
        self.mu
    }

    /// Evaluates `dr/dt = v` and `dv/dt = -mu*r/|r|^3`.
    ///
    /// This initial kernel has no frame-transform provider, so it requires the
    /// position and velocity to already use the same frame. The state type
    /// itself imposes no such restriction.
    pub fn dynamics(
        &self,
        state: &SpacecraftState,
    ) -> Result<TranslationalDerivative, DynamicsError> {
        let position = state.position();
        let velocity = state.velocity();
        if position.frame() != velocity.frame() {
            return Err(DynamicsError::FrameMismatch {
                position: position.frame(),
                velocity: velocity.frame(),
            });
        }

        let [x, y, z] = position.value().to_metres();
        let radius_squared = x.mul_add(x, y.mul_add(y, z * z));
        if !radius_squared.is_finite() || radius_squared <= f64::EPSILON {
            return Err(DynamicsError::UndefinedAtOrigin);
        }

        let radius_cubed = radius_squared * radius_squared.sqrt();
        let scale = -self.mu.as_cubic_metres_per_second_squared() / radius_cubed;
        let acceleration =
            AccelerationVector::from_metres_per_second_squared(scale * x, scale * y, scale * z);
        let acceleration = FramedAcceleration::new(acceleration, position.frame())
            .map_err(|_| DynamicsError::NonFiniteOutput)?;

        Ok(TranslationalDerivative::new(velocity, acceleration))
    }
}

/// Failure to evaluate a dynamics model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum DynamicsError {
    /// This kernel cannot combine different kinematic frames without a
    /// transform provider.
    #[error("two-body dynamics requires matching position and velocity frames")]
    FrameMismatch {
        /// Frame attached to the position.
        position: ReferenceFrame,
        /// Frame attached to the velocity.
        velocity: ReferenceFrame,
    },
    /// Point-mass gravity is singular at the position frame's origin.
    #[error("two-body gravity is undefined at the position frame origin")]
    UndefinedAtOrigin,
    /// A finite input unexpectedly produced a non-finite acceleration.
    #[error("two-body dynamics produced a non-finite acceleration")]
    NonFiniteOutput,
}

#[cfg(test)]
mod tests {
    use orskit_core::frames::ReferenceFrame;
    use orskit_core::{Epoch, FramedPosition, FramedVelocity, SpacecraftState};
    use orskit_units::uom::si::{acceleration::meter_per_second_squared, mass::kilogram};
    use orskit_units::{GravitationalParameter, Mass, Position, VelocityVector};

    use super::*;

    fn state(position_frame: ReferenceFrame, velocity_frame: ReferenceFrame) -> SpacecraftState {
        SpacecraftState::new(
            Epoch::from_tai_seconds(0.0),
            FramedPosition::new(Position::from_metres(7_000_000.0, 0.0, 0.0), position_frame)
                .expect("fixture position is finite"),
            FramedVelocity::new(
                VelocityVector::from_metres_per_second(0.0, 7_546.053_290_107_542, 0.0),
                velocity_frame,
            )
            .expect("fixture velocity is finite"),
            Mass::new::<kilogram>(1_000.0),
        )
        .expect("fixture is valid")
    }

    fn earth_dynamics() -> TwoBodyDynamics {
        TwoBodyDynamics::new(
            GravitationalParameter::from_cubic_metres_per_second_squared(3.986_004_418e14)
                .expect("Earth GM is positive and finite"),
        )
    }

    #[test]
    fn circular_orbit_acceleration_points_toward_origin() {
        let state = state(ReferenceFrame::GCRF, ReferenceFrame::GCRF);
        let derivative = earth_dynamics()
            .dynamics(&state)
            .expect("frames match and radius is non-zero");
        let acceleration = derivative.velocity_rate();

        assert_eq!(acceleration.frame(), ReferenceFrame::GCRF);
        assert!(
            (acceleration.value().x().get::<meter_per_second_squared>() + 8.134_702_893_877_55)
                .abs()
                < 1e-12
        );
        assert_eq!(
            acceleration.value().y().get::<meter_per_second_squared>(),
            0.0
        );
        assert_eq!(derivative.position_rate(), state.velocity());
    }

    #[test]
    fn state_may_hold_frames_that_this_kernel_cannot_combine() {
        let state = state(ReferenceFrame::GCRF, ReferenceFrame::EME2000);

        assert_eq!(
            earth_dynamics().dynamics(&state),
            Err(DynamicsError::FrameMismatch {
                position: ReferenceFrame::GCRF,
                velocity: ReferenceFrame::EME2000,
            })
        );
    }

    #[test]
    fn gravity_is_undefined_at_origin() {
        let state = SpacecraftState::new(
            Epoch::from_tai_seconds(0.0),
            FramedPosition::new(Position::from_metres(0.0, 0.0, 0.0), ReferenceFrame::GCRF)
                .expect("origin is a finite position"),
            FramedVelocity::new(
                VelocityVector::from_metres_per_second(1.0, 0.0, 0.0),
                ReferenceFrame::GCRF,
            )
            .expect("fixture velocity is finite"),
            Mass::new::<kilogram>(1.0),
        )
        .expect("a state may be constructed at an origin");
        let dynamics = TwoBodyDynamics::new(
            GravitationalParameter::from_cubic_metres_per_second_squared(1.0)
                .expect("fixture GM is valid"),
        );

        assert_eq!(
            dynamics.dynamics(&state),
            Err(DynamicsError::UndefinedAtOrigin)
        );
    }
}
