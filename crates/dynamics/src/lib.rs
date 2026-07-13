//! Composable descriptions of spacecraft dynamics and force models.
//!
//! This crate primarily describes model topology. A force interaction targets
//! one spacecraft and may depend on its position, speed, orientation, and
//! inertia. Environmental bodies and other configuration belong to the force
//! model, not to the interaction input. The first narrow evaluator provides
//! orbit-only analytical elliptic two-body propagation; general state derivatives,
//! numerical integration, events, and variational equations remain absent. The
//! concrete [`TwoBodyDynamics`] description is deliberately strict: it contains
//! one central point-mass model and cannot be extended with additional forces.
//! Third-body descriptions remain unavailable until their ephemeris, frame, and
//! acceleration-assembly contracts are explicit.
//!
//! ```
//! use bodies::Body;
//! use dynamics::{PointMassGravityModel, SystemDynamics, TwoBodyDynamics};
//! use units::GravitationalParameter;
//!
//! let mu = GravitationalParameter::from_cubic_metres_per_second_squared(3.986_004_418e14)?;
//! let gravity = PointMassGravityModel::new(Body::EARTH, mu);
//! let model = TwoBodyDynamics::new(gravity);
//! assert_eq!(model.conservative_force_models().len(), 1);
//! assert_eq!(
//!     model.conservative_force_models()[0].force().name(),
//!     "gravity"
//! );
//! assert!(model.non_conservative_force_models().is_empty());
//! # Ok::<(), units::QuantityError>(())
//! ```

use std::{fmt, sync::Arc};

use bodies::Body;
use units::GravitationalParameter;

mod propagator;
mod two_body;

pub use propagator::Propagator;
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
/// and its sourced gravitational parameter are explicit model configuration;
/// neither value is inferred from the other.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointMassGravityModel {
    attractor: Body,
    gravitational_parameter: GravitationalParameter,
}

impl PointMassGravityModel {
    /// Describes a point-mass gravity model for the selected body.
    #[must_use]
    pub const fn new(attractor: Body, gravitational_parameter: GravitationalParameter) -> Self {
        Self {
            attractor,
            gravitational_parameter,
        }
    }

    /// Returns the configured attracting body.
    #[must_use]
    pub const fn attractor(self) -> Body {
        self.attractor
    }

    /// Returns the explicitly configured gravitational parameter.
    #[must_use]
    pub const fn gravitational_parameter(self) -> GravitationalParameter {
        self.gravitational_parameter
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

/// Strict two-body spacecraft dynamics description.
///
/// The system contains exactly the spacecraft and one configured central
/// point-mass gravity model. Additional force models would change that physical
/// topology and therefore cannot be attached to this type. General multi-force
/// and third-body propagation remain unavailable until their state and data
/// provider contracts are defined.
#[derive(Clone)]
pub struct TwoBodyDynamics {
    central_gravity_model: Arc<PointMassGravityModel>,
    conservative_force_models: [ConservativeForceModelHandle; 1],
}

impl TwoBodyDynamics {
    /// Describes spacecraft motion under exactly one central point-mass model.
    #[must_use]
    pub fn new(model: PointMassGravityModel) -> Self {
        let central_gravity_model = Arc::new(model);
        let model_handle: ConservativeForceModelHandle = central_gravity_model.clone();
        Self {
            central_gravity_model,
            conservative_force_models: [model_handle],
        }
    }

    /// Returns the configured attracting body.
    #[must_use]
    pub fn attractor(&self) -> Body {
        self.central_gravity_model.attractor()
    }

    /// Returns the sole central point-mass gravity model.
    #[must_use]
    pub fn central_gravity_model(&self) -> &PointMassGravityModel {
        &self.central_gravity_model
    }
}

impl fmt::Debug for TwoBodyDynamics {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TwoBodyDynamics")
            .field("central_gravity_model", &self.central_gravity_model)
            .finish()
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
        &[]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point_mass(body: Body, mu_m3_s2: f64) -> PointMassGravityModel {
        PointMassGravityModel::new(
            body,
            GravitationalParameter::from_cubic_metres_per_second_squared(mu_m3_s2)
                .expect("fixture gravitational parameter is positive"),
        )
    }

    fn earth_gravity() -> PointMassGravityModel {
        point_mass(Body::EARTH, 3.986_004_418e14)
    }

    #[test]
    fn two_body_preserves_exactly_one_central_point_mass_model() {
        let expected_mu = earth_gravity().gravitational_parameter();
        let dynamics = TwoBodyDynamics::new(earth_gravity());

        assert_eq!(dynamics.name(), "two-body spacecraft dynamics");
        assert_eq!(dynamics.attractor(), Body::EARTH);
        assert_eq!(dynamics.central_gravity_model().attractor(), Body::EARTH);
        assert_eq!(
            dynamics.central_gravity_model().gravitational_parameter(),
            expected_mu
        );
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
    fn state_dependencies_are_limited_to_the_spacecraft_state_contract() {
        let dependencies = SpacecraftStateDependencies::new(true, true, false, true);

        assert!(dependencies.position());
        assert!(dependencies.speed());
        assert!(!dependencies.orientation());
        assert!(dependencies.inertia());
    }
}
