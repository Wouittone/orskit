//! Composable descriptions of spacecraft dynamics and force models.
//!
//! This crate describes model topology only. A force interaction targets one
//! spacecraft and may depend on its position, speed, orientation, and inertia.
//! Environmental bodies and other configuration belong to the force model,
//! not to the interaction input. State derivatives, numerical integration,
//! propagation, events, and variational equations remain deliberately absent.
//!
//! ```
//! use orskit_bodies::Body;
//! use orskit_dynamics::{SystemDynamics, ThreeBodyDynamics};
//!
//! let model = ThreeBodyDynamics::new(Body::EARTH, Body::MOON)?;
//! assert_eq!(model.conservative_forces().len(), 2);
//! assert!(model.non_conservative_forces().is_empty());
//! # Ok::<(), orskit_dynamics::DynamicsDescriptionError>(())
//! ```

use std::{fmt, sync::Arc};

use orskit_bodies::Body;
use thiserror::Error;

/// Spacecraft-state components a force interaction is allowed to inspect.
///
/// This value describes access only; it does not evaluate the force. A future
/// evaluator will map these requirements to an explicit spacecraft-state
/// representation before invoking a model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpacecraftStateDependencies {
    position: bool,
    speed: bool,
    orientation: bool,
    inertia: bool,
}

impl SpacecraftStateDependencies {
    /// No spacecraft-state component is required.
    pub const NONE: Self = Self::new(false, false, false, false);
    /// Only spacecraft position is required.
    pub const POSITION: Self = Self::new(true, false, false, false);
    /// Every currently defined spacecraft-state component is required.
    pub const ALL: Self = Self::new(true, true, true, true);

    /// Defines the spacecraft-state components required by a force model.
    #[must_use]
    pub const fn new(position: bool, speed: bool, orientation: bool, inertia: bool) -> Self {
        Self {
            position,
            speed,
            orientation,
            inertia,
        }
    }

    /// Returns whether spacecraft position is required.
    #[must_use]
    pub const fn position(self) -> bool {
        self.position
    }

    /// Returns whether spacecraft speed is required.
    #[must_use]
    pub const fn speed(self) -> bool {
        self.speed
    }

    /// Returns whether spacecraft orientation is required.
    #[must_use]
    pub const fn orientation(self) -> bool {
        self.orientation
    }

    /// Returns whether spacecraft inertia is required.
    #[must_use]
    pub const fn inertia(self) -> bool {
        self.inertia
    }
}

/// Common descriptive contract for a force acting on a spacecraft.
///
/// The force model owns all environmental configuration. Its interaction with
/// the propagated object is restricted to the spacecraft-state components
/// declared by [`ForceModel::state_dependencies`]. No evaluation method is
/// defined until the state representation and data context are designed.
pub trait ForceModel: fmt::Debug + Send + Sync {
    /// Returns a stable human-readable model name for diagnostics.
    fn name(&self) -> &str;

    /// Returns the spacecraft-state components inspected by this model.
    fn state_dependencies(&self) -> SpacecraftStateDependencies;
}

/// Description of a conservative force contribution.
///
/// Conservative and non-conservative models are stored separately so future
/// evaluators can select appropriate conservation checks and accumulation
/// policies without inspecting strings or a closed model enum.
pub trait ConservativeForce: ForceModel {}

/// Description of a non-conservative force contribution.
pub trait NonConservativeForce: ForceModel {}

/// Shared handle for a pluggable conservative force description.
pub type ConservativeForceHandle = Arc<dyn ConservativeForce + Send + Sync + 'static>;

/// Shared handle for a pluggable non-conservative force description.
pub type NonConservativeForceHandle = Arc<dyn NonConservativeForce + Send + Sync + 'static>;

/// Description of a spacecraft dynamical system and its force composition.
///
/// Implementations preserve declaration order independently within the
/// conservative and non-conservative collections. Evaluation, state
/// derivatives, and numerical resolution belong to future contracts.
pub trait SystemDynamics: fmt::Debug + Send + Sync {
    /// Returns a stable human-readable system name for diagnostics.
    fn name(&self) -> &str;

    /// Returns conservative forces in declaration order.
    fn conservative_forces(&self) -> &[ConservativeForceHandle];

    /// Returns non-conservative forces in declaration order.
    fn non_conservative_forces(&self) -> &[NonConservativeForceHandle];
}

/// Point-mass gravity exerted by one configured attracting body.
///
/// Only spacecraft position is an interaction dependency. The attracting body
/// is model configuration; its gravitational parameter and ephemeris remain
/// explicit future data requirements rather than properties inferred here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointMassGravity {
    attractor: Body,
}

impl PointMassGravity {
    /// Describes point-mass gravity from the selected body.
    #[must_use]
    pub const fn new(attractor: Body) -> Self {
        Self { attractor }
    }

    /// Returns the configured attracting body.
    #[must_use]
    pub const fn attractor(self) -> Body {
        self.attractor
    }
}

impl ForceModel for PointMassGravity {
    fn name(&self) -> &str {
        "point-mass gravity"
    }

    fn state_dependencies(&self) -> SpacecraftStateDependencies {
        SpacecraftStateDependencies::POSITION
    }
}

impl ConservativeForce for PointMassGravity {}

/// Simplified two-body spacecraft dynamics description.
///
/// The two bodies are the spacecraft and one configured point-mass attractor.
/// Additional conservative and non-conservative force descriptions may be
/// attached without changing the system contract.
#[derive(Debug, Clone)]
pub struct TwoBodyDynamics {
    attractor: Body,
    conservative_forces: Vec<ConservativeForceHandle>,
    non_conservative_forces: Vec<NonConservativeForceHandle>,
}

impl TwoBodyDynamics {
    /// Describes spacecraft motion under one point-mass attractor.
    #[must_use]
    pub fn new(attractor: Body) -> Self {
        Self {
            attractor,
            conservative_forces: vec![Arc::new(PointMassGravity::new(attractor))],
            non_conservative_forces: Vec::new(),
        }
    }

    /// Returns the configured attracting body.
    #[must_use]
    pub const fn attractor(&self) -> Body {
        self.attractor
    }

    /// Adds a conservative force description in declaration order.
    #[must_use]
    pub fn with_conservative_force(mut self, force: ConservativeForceHandle) -> Self {
        self.conservative_forces.push(force);
        self
    }

    /// Adds a non-conservative force description in declaration order.
    #[must_use]
    pub fn with_non_conservative_force(mut self, force: NonConservativeForceHandle) -> Self {
        self.non_conservative_forces.push(force);
        self
    }
}

impl SystemDynamics for TwoBodyDynamics {
    fn name(&self) -> &str {
        "two-body spacecraft dynamics"
    }

    fn conservative_forces(&self) -> &[ConservativeForceHandle] {
        &self.conservative_forces
    }

    fn non_conservative_forces(&self) -> &[NonConservativeForceHandle] {
        &self.non_conservative_forces
    }
}

/// Simplified three-body spacecraft dynamics description.
///
/// The three bodies are the spacecraft and two distinct point-mass attractors.
/// The description does not select restricted/full equations, ephemerides, or
/// a numerical resolution method.
#[derive(Debug, Clone)]
pub struct ThreeBodyDynamics {
    attractors: [Body; 2],
    conservative_forces: Vec<ConservativeForceHandle>,
    non_conservative_forces: Vec<NonConservativeForceHandle>,
}

impl ThreeBodyDynamics {
    /// Describes spacecraft motion under two distinct point-mass attractors.
    pub fn new(first: Body, second: Body) -> Result<Self, DynamicsDescriptionError> {
        if first == second {
            return Err(DynamicsDescriptionError::DuplicateAttractor(first));
        }
        Ok(Self {
            attractors: [first, second],
            conservative_forces: vec![
                Arc::new(PointMassGravity::new(first)),
                Arc::new(PointMassGravity::new(second)),
            ],
            non_conservative_forces: Vec::new(),
        })
    }

    /// Returns the two configured attracting bodies.
    #[must_use]
    pub const fn attractors(&self) -> [Body; 2] {
        self.attractors
    }

    /// Adds a conservative force description in declaration order.
    #[must_use]
    pub fn with_conservative_force(mut self, force: ConservativeForceHandle) -> Self {
        self.conservative_forces.push(force);
        self
    }

    /// Adds a non-conservative force description in declaration order.
    #[must_use]
    pub fn with_non_conservative_force(mut self, force: NonConservativeForceHandle) -> Self {
        self.non_conservative_forces.push(force);
        self
    }
}

impl SystemDynamics for ThreeBodyDynamics {
    fn name(&self) -> &str {
        "three-body spacecraft dynamics"
    }

    fn conservative_forces(&self) -> &[ConservativeForceHandle] {
        &self.conservative_forces
    }

    fn non_conservative_forces(&self) -> &[NonConservativeForceHandle] {
        &self.non_conservative_forces
    }
}

/// Invalid dynamics description.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum DynamicsDescriptionError {
    /// A three-body description repeated the same attracting body.
    #[error("three-body dynamics contains duplicate attractor {0}")]
    DuplicateAttractor(Body),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct PositionPotential;

    impl ForceModel for PositionPotential {
        fn name(&self) -> &str {
            "position potential"
        }

        fn state_dependencies(&self) -> SpacecraftStateDependencies {
            SpacecraftStateDependencies::POSITION
        }
    }

    impl ConservativeForce for PositionPotential {}

    #[derive(Debug)]
    struct AerodynamicDrag;

    impl ForceModel for AerodynamicDrag {
        fn name(&self) -> &str {
            "aerodynamic drag"
        }

        fn state_dependencies(&self) -> SpacecraftStateDependencies {
            SpacecraftStateDependencies::ALL
        }
    }

    impl NonConservativeForce for AerodynamicDrag {}

    #[test]
    fn two_body_is_a_system_dynamics_implementation() {
        let dynamics = TwoBodyDynamics::new(Body::EARTH);

        assert_eq!(dynamics.name(), "two-body spacecraft dynamics");
        assert_eq!(dynamics.attractor(), Body::EARTH);
        assert_eq!(dynamics.conservative_forces().len(), 1);
        assert!(dynamics.non_conservative_forces().is_empty());
        assert_eq!(
            dynamics.conservative_forces()[0].state_dependencies(),
            SpacecraftStateDependencies::POSITION
        );
    }

    #[test]
    fn three_body_configures_two_independent_attractors() {
        let dynamics = ThreeBodyDynamics::new(Body::EARTH, Body::MOON)
            .expect("Earth and Moon are distinct attractors");

        assert_eq!(dynamics.attractors(), [Body::EARTH, Body::MOON]);
        assert_eq!(dynamics.conservative_forces().len(), 2);
        assert_eq!(
            dynamics.conservative_forces()[0].state_dependencies(),
            SpacecraftStateDependencies::POSITION
        );
        assert_eq!(
            dynamics.conservative_forces()[1].state_dependencies(),
            SpacecraftStateDependencies::POSITION
        );
    }

    #[test]
    fn conservative_and_non_conservative_forces_are_split_and_ordered() {
        let dynamics = TwoBodyDynamics::new(Body::EARTH)
            .with_conservative_force(Arc::new(PositionPotential))
            .with_non_conservative_force(Arc::new(AerodynamicDrag));

        assert_eq!(dynamics.conservative_forces().len(), 2);
        assert_eq!(
            dynamics.conservative_forces()[1].name(),
            "position potential"
        );
        assert_eq!(dynamics.non_conservative_forces().len(), 1);
        assert_eq!(
            dynamics.non_conservative_forces()[0].state_dependencies(),
            SpacecraftStateDependencies::ALL
        );
    }

    #[test]
    fn state_dependencies_are_limited_to_the_spacecraft_state_contract() {
        let dependencies = SpacecraftStateDependencies::new(true, true, false, true);

        assert!(dependencies.position());
        assert!(dependencies.speed());
        assert!(!dependencies.orientation());
        assert!(dependencies.inertia());
    }

    #[test]
    fn duplicate_three_body_attractors_are_rejected() {
        assert!(matches!(
            ThreeBodyDynamics::new(Body::EARTH, Body::EARTH),
            Err(DynamicsDescriptionError::DuplicateAttractor(Body::EARTH))
        ));
    }
}
