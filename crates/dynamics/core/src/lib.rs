#![forbid(unsafe_code)]

//! Composable contracts for spacecraft dynamics.
//!
//! This crate contains no concrete force or propagation implementation.
//! Applications assemble force models through [`ComposedDynamics`] and select
//! a solver and state implementation from dedicated crates.

use std::{fmt, sync::Arc};

use hifitime::Epoch;
use orbits::cartesian::{CartesianState, FramedAcceleration};
use thiserror::Error;
use units::{InverseTime, InverseTimeSquared};

mod propagator;

pub use propagator::{PropagationState, Propagator};

/// Spacecraft-state components required to evaluate a force model.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpacecraftStateRequirements(u8);

impl SpacecraftStateRequirements {
    pub const NONE: Self = Self(0);
    pub const POSITION: Self = Self(1 << 0);
    pub const VELOCITY: Self = Self(1 << 1);
    pub const MASS: Self = Self(1 << 2);
    pub const ATTITUDE: Self = Self(1 << 3);
    pub const ANGULAR_VELOCITY: Self = Self(1 << 4);
    pub const INERTIA: Self = Self(1 << 5);

    /// Combines two sets of requirements.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Returns whether every component in `required` is present.
    #[must_use]
    pub const fn contains(self, required: Self) -> bool {
        self.0 & required.0 == required.0
    }
}

/// Open descriptive identity for a physical interaction.
pub trait Force: fmt::Debug + Send + Sync {
    /// Returns a stable human-readable force-family name for diagnostics.
    fn name(&self) -> &str;
}

/// Common descriptive contract for one implementation of a physical force.
pub trait ForceModel: fmt::Debug + Send + Sync {
    /// Returns a stable human-readable implementation name for diagnostics.
    fn model_name(&self) -> &str;
    /// Returns the physical interaction modeled by this implementation.
    fn force(&self) -> &dyn Force;
    /// Returns the spacecraft-state components required by this model.
    fn state_requirements(&self) -> SpacecraftStateRequirements;
}

/// Description of a potential-derived force-model contribution.
pub trait ConservativeForceModel: ForceModel {}

/// Description of a non-conservative force-model contribution.
pub trait NonConservativeForceModel: ForceModel {}

/// Shared handle for a conservative force-model description.
pub type ConservativeForceModelHandle = Arc<dyn ConservativeForceModel + Send + Sync + 'static>;
/// Shared handle for a non-conservative force-model description.
pub type NonConservativeForceModelHandle =
    Arc<dyn NonConservativeForceModel + Send + Sync + 'static>;

/// Description of a spacecraft dynamical system and its force composition.
pub trait SystemDynamics: fmt::Debug + Send + Sync {
    /// Returns a stable human-readable system name for diagnostics.
    fn name(&self) -> &str;
    /// Returns conservative force models in declaration order.
    fn conservative_force_models(&self) -> &[ConservativeForceModelHandle];
    /// Returns non-conservative force models in declaration order.
    fn non_conservative_force_models(&self) -> &[NonConservativeForceModelHandle];
}

/// Evaluable translational dynamics for one Cartesian state.
///
/// The input state carries its reference frame and `epoch` identifies the
/// instant of evaluation. The returned acceleration must carry the same frame.
/// Implementations own or borrow every immutable provider they require; this
/// contract performs no ambient data lookup.
pub trait CartesianDynamics: fmt::Debug + Send + Sync {
    /// Typed model/provider error.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Validates the state frame, origin, and model data before integration.
    fn validate(&self, state: &CartesianState) -> Result<(), Self::Error>;

    /// Evaluates acceleration at `epoch`, expressed in the state's frame.
    fn acceleration(
        &self,
        epoch: Epoch,
        state: &CartesianState,
    ) -> Result<FramedAcceleration, Self::Error>;
}

/// Cartesian acceleration partial derivatives for variational equations.
///
/// `position[row][column]` is `d acceleration_row / d position_column`
/// in reciprocal square seconds. `velocity[row][column]` is
/// `d acceleration_row / d velocity_column` in reciprocal seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CartesianAccelerationJacobian {
    position: [[InverseTimeSquared; 3]; 3],
    velocity: [[InverseTime; 3]; 3],
}

impl CartesianAccelerationJacobian {
    /// Creates finite, frame-basis Cartesian acceleration partials.
    pub fn new(
        position: [[InverseTimeSquared; 3]; 3],
        velocity: [[InverseTime; 3]; 3],
    ) -> Result<Self, CartesianAccelerationJacobianError> {
        if position
            .iter()
            .flatten()
            .any(|value| !value.as_per_square_second().is_finite())
            || velocity
                .iter()
                .flatten()
                .any(|value| !value.as_per_second().is_finite())
        {
            return Err(CartesianAccelerationJacobianError::NonFinite);
        }
        Ok(Self { position, velocity })
    }

    /// Returns acceleration partials with respect to position.
    #[must_use]
    pub const fn position(self) -> [[InverseTimeSquared; 3]; 3] {
        self.position
    }

    /// Returns acceleration partials with respect to velocity.
    #[must_use]
    pub const fn velocity(self) -> [[InverseTime; 3]; 3] {
        self.velocity
    }
}

/// Evaluable Cartesian dynamics with first state partial derivatives.
pub trait CartesianVariationalDynamics: CartesianDynamics {
    /// Evaluates the acceleration Jacobian at one epoch-qualified state.
    fn acceleration_jacobian(
        &self,
        epoch: Epoch,
        state: &CartesianState,
    ) -> Result<CartesianAccelerationJacobian, Self::Error>;
}

/// Invalid Cartesian acceleration Jacobian.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CartesianAccelerationJacobianError {
    /// At least one partial derivative is NaN or infinite.
    #[error("Cartesian acceleration Jacobian entries must be finite")]
    NonFinite,
}

/// Ordered heterogeneous force-model composition for one dynamical system.
#[derive(Debug, Clone, Default)]
pub struct ComposedDynamics {
    name: String,
    conservative_force_models: Vec<ConservativeForceModelHandle>,
    non_conservative_force_models: Vec<NonConservativeForceModelHandle>,
}

impl ComposedDynamics {
    /// Creates an empty, named dynamics description.
    #[must_use]
    pub fn new(name: String) -> Self {
        Self {
            name,
            conservative_force_models: Vec::new(),
            non_conservative_force_models: Vec::new(),
        }
    }

    /// Appends a conservative model, preserving declaration order.
    #[must_use]
    pub fn with_conservative(mut self, model: ConservativeForceModelHandle) -> Self {
        self.conservative_force_models.push(model);
        self
    }

    /// Appends a non-conservative model, preserving declaration order.
    #[must_use]
    pub fn with_non_conservative(mut self, model: NonConservativeForceModelHandle) -> Self {
        self.non_conservative_force_models.push(model);
        self
    }
}

impl SystemDynamics for ComposedDynamics {
    fn name(&self) -> &str {
        &self.name
    }

    fn conservative_force_models(&self) -> &[ConservativeForceModelHandle] {
        &self.conservative_force_models
    }

    fn non_conservative_force_models(&self) -> &[NonConservativeForceModelHandle] {
        &self.non_conservative_force_models
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestForce(&'static str);
    impl Force for TestForce {
        fn name(&self) -> &str {
            self.0
        }
    }
    #[derive(Debug)]
    struct Conservative(TestForce);
    impl ForceModel for Conservative {
        fn model_name(&self) -> &str {
            "conservative test model"
        }
        fn force(&self) -> &dyn Force {
            &self.0
        }
        fn state_requirements(&self) -> SpacecraftStateRequirements {
            SpacecraftStateRequirements::POSITION
        }
    }
    impl ConservativeForceModel for Conservative {}

    #[test]
    fn heterogeneous_models_compose_in_declaration_order() {
        let dynamics = ComposedDynamics::new("test system".to_owned())
            .with_conservative(Arc::new(Conservative(TestForce("gravity"))));
        assert_eq!(dynamics.name(), "test system");
        assert_eq!(dynamics.conservative_force_models().len(), 1);
        assert_eq!(
            dynamics.conservative_force_models()[0].force().name(),
            "gravity"
        );
    }
}
