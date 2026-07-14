#![forbid(unsafe_code)]

//! Composable contracts for spacecraft dynamics.
//!
//! This crate contains no concrete force or propagation implementation.
//! Applications assemble force models through [`ComposedDynamics`] and select
//! a solver and state implementation from dedicated crates.

use std::{fmt, sync::Arc};

mod propagator;

pub use propagator::Propagator;

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
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
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
        let dynamics = ComposedDynamics::new("test system")
            .with_conservative(Arc::new(Conservative(TestForce("gravity"))));
        assert_eq!(dynamics.name(), "test system");
        assert_eq!(dynamics.conservative_force_models().len(), 1);
        assert_eq!(
            dynamics.conservative_force_models()[0].force().name(),
            "gravity"
        );
    }
}
