//! Composable descriptions of spacecraft dynamics and force models.
//!
//! This crate primarily describes model topology. A force interaction targets
//! one spacecraft and may depend on its position, speed, orientation, and
//! inertia. Environmental bodies and other configuration belong to the force
//! model, not to the interaction input. The first narrow evaluator provides
//! analytical elliptic two-body propagation; general state derivatives,
//! numerical integration, events, and variational equations remain absent.
//!
//! ```
//! use orskit_bodies::Body;
//! use orskit_dynamics::{SystemDynamics, ThreeBodyDynamics};
//!
//! let model = ThreeBodyDynamics::new(Body::EARTH, Body::MOON)?;
//! assert_eq!(model.conservative_force_models().len(), 2);
//! assert_eq!(
//!     model.conservative_force_models()[0].force().name(),
//!     "gravity"
//! );
//! assert!(model.non_conservative_force_models().is_empty());
//! # Ok::<(), orskit_dynamics::DynamicsDescriptionError>(())
//! ```

use std::{fmt, sync::Arc};

use orskit_bodies::Body;
use thiserror::Error;

mod two_body;

pub use two_body::{EllipticTwoBodyPropagator, TwoBodyPropagationError};

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

/// Open descriptive identity for a physical force family.
///
/// A force is the physical interaction, such as gravity. A [`ForceModel`] is a
/// selected implementation or approximation of that force, such as a
/// point-mass gravity model. Keeping this trait object-safe allows downstream
/// crates to introduce new force families without changing a closed enum.
pub trait Force: fmt::Debug + Send + Sync {
    /// Returns a stable human-readable force-family name for diagnostics.
    fn name(&self) -> &str;
}

/// Common descriptive contract for a model of a force acting on a spacecraft.
///
/// The force model owns all environmental configuration. Its interaction with
/// the propagated object is restricted to the spacecraft-state components
/// declared by [`ForceModel::state_dependencies`]. No evaluation method is
/// defined until the state representation and data context are designed.
pub trait ForceModel: fmt::Debug + Send + Sync {
    /// Returns a stable human-readable implementation name for diagnostics.
    fn model_name(&self) -> &str;

    /// Returns the physical force modeled by this implementation.
    ///
    /// Returning a trait object rather than an associated type keeps
    /// [`ForceModel`] object-safe so heterogeneous implementations can be
    /// composed in one dynamics description.
    fn force(&self) -> &dyn Force;

    /// Returns the spacecraft-state components inspected by this model.
    fn state_dependencies(&self) -> SpacecraftStateDependencies;
}

/// Description of a potential-derived force-model contribution.
///
/// Conservative and non-conservative models are stored separately so future
/// evaluators can select appropriate conservation checks and accumulation
/// policies without inspecting strings or a closed model enum.
pub trait ConservativeForceModel: ForceModel {}

/// Description of a non-conservative force-model contribution.
pub trait NonConservativeForceModel: ForceModel {}

/// Shared handle for a pluggable conservative force-model description.
pub type ConservativeForceModelHandle = Arc<dyn ConservativeForceModel + Send + Sync + 'static>;

/// Shared handle for a pluggable non-conservative force-model description.
pub type NonConservativeForceModelHandle =
    Arc<dyn NonConservativeForceModel + Send + Sync + 'static>;

/// Description of a spacecraft dynamical system and its force composition.
///
/// Implementations preserve declaration order independently within the
/// conservative and non-conservative collections. Evaluation, state
/// derivatives, and numerical resolution belong to future contracts.
pub trait SystemDynamics: fmt::Debug + Send + Sync {
    /// Returns a stable human-readable system name for diagnostics.
    fn name(&self) -> &str;

    /// Returns conservative force models in declaration order.
    fn conservative_force_models(&self) -> &[ConservativeForceModelHandle];

    /// Returns non-conservative force models in declaration order.
    fn non_conservative_force_models(&self) -> &[NonConservativeForceModelHandle];
}

/// Physical gravitational interaction.
///
/// Point-mass, spherical-harmonic, irregular-body, and time-variable gravity
/// are model implementations of this same force family.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct GravityForce;

impl Force for GravityForce {
    fn name(&self) -> &str {
        "gravity"
    }
}

static GRAVITY_FORCE: GravityForce = GravityForce;

/// Point-mass model of gravity from one configured attracting body.
///
/// Only spacecraft position is an interaction dependency. The attracting body
/// is model configuration; its gravitational parameter and ephemeris remain
/// explicit future data requirements rather than properties inferred here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointMassGravityModel {
    attractor: Body,
}

impl PointMassGravityModel {
    /// Describes a point-mass gravity model for the selected body.
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

impl ForceModel for PointMassGravityModel {
    fn model_name(&self) -> &str {
        "point-mass gravity model"
    }

    fn force(&self) -> &dyn Force {
        &GRAVITY_FORCE
    }

    fn state_dependencies(&self) -> SpacecraftStateDependencies {
        SpacecraftStateDependencies::POSITION
    }
}

impl ConservativeForceModel for PointMassGravityModel {}

/// Simplified two-body spacecraft dynamics description.
///
/// The two bodies are the spacecraft and one configured point-mass attractor.
/// Additional conservative and non-conservative force-model implementations
/// may be attached without changing the system contract.
#[derive(Debug, Clone)]
pub struct TwoBodyDynamics {
    attractor: Body,
    conservative_force_models: Vec<ConservativeForceModelHandle>,
    non_conservative_force_models: Vec<NonConservativeForceModelHandle>,
}

impl TwoBodyDynamics {
    /// Describes spacecraft motion under one point-mass attractor.
    #[must_use]
    pub fn new(attractor: Body) -> Self {
        Self {
            attractor,
            conservative_force_models: vec![Arc::new(PointMassGravityModel::new(attractor))],
            non_conservative_force_models: Vec::new(),
        }
    }

    /// Returns the configured attracting body.
    #[must_use]
    pub const fn attractor(&self) -> Body {
        self.attractor
    }

    /// Adds a conservative force-model description in declaration order.
    #[must_use]
    pub fn with_conservative_force_model(mut self, model: ConservativeForceModelHandle) -> Self {
        self.conservative_force_models.push(model);
        self
    }

    /// Adds a non-conservative force-model description in declaration order.
    #[must_use]
    pub fn with_non_conservative_force_model(
        mut self,
        model: NonConservativeForceModelHandle,
    ) -> Self {
        self.non_conservative_force_models.push(model);
        self
    }
}

impl SystemDynamics for TwoBodyDynamics {
    fn name(&self) -> &str {
        "two-body spacecraft dynamics"
    }

    fn conservative_force_models(&self) -> &[ConservativeForceModelHandle] {
        &self.conservative_force_models
    }

    fn non_conservative_force_models(&self) -> &[NonConservativeForceModelHandle] {
        &self.non_conservative_force_models
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
    conservative_force_models: Vec<ConservativeForceModelHandle>,
    non_conservative_force_models: Vec<NonConservativeForceModelHandle>,
}

impl ThreeBodyDynamics {
    /// Describes spacecraft motion under two distinct point-mass attractors.
    pub fn new(first: Body, second: Body) -> Result<Self, DynamicsDescriptionError> {
        if first == second {
            return Err(DynamicsDescriptionError::DuplicateAttractor(first));
        }
        Ok(Self {
            attractors: [first, second],
            conservative_force_models: vec![
                Arc::new(PointMassGravityModel::new(first)),
                Arc::new(PointMassGravityModel::new(second)),
            ],
            non_conservative_force_models: Vec::new(),
        })
    }

    /// Returns the two configured attracting bodies.
    #[must_use]
    pub const fn attractors(&self) -> [Body; 2] {
        self.attractors
    }

    /// Adds a conservative force-model description in declaration order.
    #[must_use]
    pub fn with_conservative_force_model(mut self, model: ConservativeForceModelHandle) -> Self {
        self.conservative_force_models.push(model);
        self
    }

    /// Adds a non-conservative force-model description in declaration order.
    #[must_use]
    pub fn with_non_conservative_force_model(
        mut self,
        model: NonConservativeForceModelHandle,
    ) -> Self {
        self.non_conservative_force_models.push(model);
        self
    }
}

impl SystemDynamics for ThreeBodyDynamics {
    fn name(&self) -> &str {
        "three-body spacecraft dynamics"
    }

    fn conservative_force_models(&self) -> &[ConservativeForceModelHandle] {
        &self.conservative_force_models
    }

    fn non_conservative_force_models(&self) -> &[NonConservativeForceModelHandle] {
        &self.non_conservative_force_models
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
    struct PotentialForce;

    impl Force for PotentialForce {
        fn name(&self) -> &str {
            "test potential"
        }
    }

    static POTENTIAL_FORCE: PotentialForce = PotentialForce;

    #[derive(Debug)]
    struct PositionPotentialModel;

    impl ForceModel for PositionPotentialModel {
        fn model_name(&self) -> &str {
            "position potential model"
        }

        fn force(&self) -> &dyn Force {
            &POTENTIAL_FORCE
        }

        fn state_dependencies(&self) -> SpacecraftStateDependencies {
            SpacecraftStateDependencies::POSITION
        }
    }

    impl ConservativeForceModel for PositionPotentialModel {}

    #[derive(Debug)]
    struct AerodynamicForce;

    impl Force for AerodynamicForce {
        fn name(&self) -> &str {
            "aerodynamic force"
        }
    }

    static AERODYNAMIC_FORCE: AerodynamicForce = AerodynamicForce;

    #[derive(Debug)]
    struct AerodynamicDragModel;

    impl ForceModel for AerodynamicDragModel {
        fn model_name(&self) -> &str {
            "isotropic drag model"
        }

        fn force(&self) -> &dyn Force {
            &AERODYNAMIC_FORCE
        }

        fn state_dependencies(&self) -> SpacecraftStateDependencies {
            SpacecraftStateDependencies::ALL
        }
    }

    impl NonConservativeForceModel for AerodynamicDragModel {}

    #[test]
    fn two_body_is_a_system_dynamics_implementation() {
        let dynamics = TwoBodyDynamics::new(Body::EARTH);

        assert_eq!(dynamics.name(), "two-body spacecraft dynamics");
        assert_eq!(dynamics.attractor(), Body::EARTH);
        assert_eq!(dynamics.conservative_force_models().len(), 1);
        assert!(dynamics.non_conservative_force_models().is_empty());
        assert_eq!(
            dynamics.conservative_force_models()[0].force().name(),
            "gravity"
        );
        assert_eq!(
            dynamics.conservative_force_models()[0].model_name(),
            "point-mass gravity model"
        );
        assert_eq!(
            dynamics.conservative_force_models()[0].state_dependencies(),
            SpacecraftStateDependencies::POSITION
        );
    }

    #[test]
    fn three_body_configures_two_independent_attractors() {
        let dynamics = ThreeBodyDynamics::new(Body::EARTH, Body::MOON)
            .expect("Earth and Moon are distinct attractors");

        assert_eq!(dynamics.attractors(), [Body::EARTH, Body::MOON]);
        assert_eq!(dynamics.conservative_force_models().len(), 2);
        assert_eq!(
            dynamics.conservative_force_models()[0].state_dependencies(),
            SpacecraftStateDependencies::POSITION
        );
        assert_eq!(
            dynamics.conservative_force_models()[1].state_dependencies(),
            SpacecraftStateDependencies::POSITION
        );
        assert!(dynamics
            .conservative_force_models()
            .iter()
            .all(|model| model.force().name() == "gravity"));
    }

    #[test]
    fn heterogeneous_force_models_are_split_and_ordered_without_downcasting() {
        let dynamics = TwoBodyDynamics::new(Body::EARTH)
            .with_conservative_force_model(Arc::new(PositionPotentialModel))
            .with_non_conservative_force_model(Arc::new(AerodynamicDragModel));

        assert_eq!(dynamics.conservative_force_models().len(), 2);
        assert_eq!(
            dynamics.conservative_force_models()[1].force().name(),
            "test potential"
        );
        assert_eq!(
            dynamics.conservative_force_models()[1].model_name(),
            "position potential model"
        );
        assert_eq!(dynamics.non_conservative_force_models().len(), 1);
        assert_eq!(
            dynamics.non_conservative_force_models()[0].force().name(),
            "aerodynamic force"
        );
        assert_eq!(
            dynamics.non_conservative_force_models()[0].model_name(),
            "isotropic drag model"
        );
        assert_eq!(
            dynamics.non_conservative_force_models()[0].state_dependencies(),
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
