//! Two-body dynamics with dimensionally typed inputs and outputs.

use orskit_core::SpacecraftState;
use orskit_units::{AccelerationVector, GravitationalParameter, VelocityVector};
use thiserror::Error;

/// Time derivative of a translational spacecraft state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TranslationalDerivative {
    position_rate: VelocityVector,
    velocity_rate: AccelerationVector,
}

impl TranslationalDerivative {
    /// Constructs a typed translational derivative.
    #[must_use]
    pub const fn new(position_rate: VelocityVector, velocity_rate: AccelerationVector) -> Self {
        Self {
            position_rate,
            velocity_rate,
        }
    }

    /// Derivative of position, which is velocity.
    #[must_use]
    pub const fn position_rate(self) -> VelocityVector {
        self.position_rate
    }

    /// Derivative of velocity, which is acceleration.
    #[must_use]
    pub const fn velocity_rate(self) -> AccelerationVector {
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
    pub fn dynamics(
        &self,
        state: &SpacecraftState,
    ) -> Result<TranslationalDerivative, DynamicsError> {
        let [x, y, z] = state.position().to_metres();
        let radius_squared = x.mul_add(x, y.mul_add(y, z * z));
        if !radius_squared.is_finite() || radius_squared <= f64::EPSILON {
            return Err(DynamicsError::UndefinedAtOrigin);
        }

        let radius_cubed = radius_squared * radius_squared.sqrt();
        let scale = -self.mu.as_cubic_metres_per_second_squared() / radius_cubed;
        let acceleration =
            AccelerationVector::from_metres_per_second_squared(scale * x, scale * y, scale * z);

        Ok(TranslationalDerivative::new(state.velocity(), acceleration))
    }
}

/// Failure to evaluate a dynamics model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum DynamicsError {
    /// Point-mass gravity is singular at the central-body origin.
    #[error("two-body gravity is undefined at the frame origin")]
    UndefinedAtOrigin,
}

#[cfg(test)]
mod tests {
    use orskit_core::frames::ReferenceFrame;
    use orskit_core::{Epoch, SpacecraftState};
    use orskit_units::uom::si::{acceleration::meter_per_second_squared, mass::kilogram};
    use orskit_units::{GravitationalParameter, Mass, Position, VelocityVector};

    use super::*;

    #[test]
    fn circular_orbit_acceleration_points_toward_origin() {
        let state = SpacecraftState::new(
            Epoch::from_tai_seconds(0.0),
            ReferenceFrame::GCRF,
            Position::from_metres(7_000_000.0, 0.0, 0.0),
            VelocityVector::from_metres_per_second(0.0, 7_546.053_290_107_542, 0.0),
            Mass::new::<kilogram>(1_000.0),
        )
        .expect("fixture is valid");
        let dynamics = TwoBodyDynamics::new(
            GravitationalParameter::from_cubic_metres_per_second_squared(3.986_004_418e14)
                .expect("Earth GM is positive and finite"),
        );

        let derivative = dynamics.dynamics(&state).expect("radius is non-zero");
        let acceleration = derivative.velocity_rate();
        assert!(
            (acceleration.x().get::<meter_per_second_squared>() + 8.134_702_893_877_55).abs()
                < 1e-12
        );
        assert_eq!(acceleration.y().get::<meter_per_second_squared>(), 0.0);
        assert_eq!(derivative.position_rate(), state.velocity());
    }

    #[test]
    fn gravity_is_undefined_at_origin() {
        let state = SpacecraftState::new(
            Epoch::from_tai_seconds(0.0),
            ReferenceFrame::GCRF,
            Position::from_metres(0.0, 0.0, 0.0),
            VelocityVector::from_metres_per_second(1.0, 0.0, 0.0),
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
