#![forbid(unsafe_code)]

//! Composable descriptions of spacecraft dynamics and force models.
//!
//! This crate primarily describes model topology. A force interaction targets
//! one spacecraft and declares its required position, velocity, mass, attitude,
//! angular velocity, and inertia components. Environmental bodies and other
//! configuration belong to the force model, not to the interaction input. The
//! first narrow evaluator provides
//! orbit-only analytical elliptic two-body propagation; general state derivatives,
//! numerical integration, events, and variational equations remain absent. The
//! concrete [`TwoBodyDynamics`] description is deliberately strict: it contains
//! one central point-mass model and cannot be extended with additional forces.
//! Third-body descriptions remain unavailable until their ephemeris, frame, and
//! acceleration-assembly contracts are explicit.
//!
//! ```
//! use std::sync::Arc;
//!
//! use bodies::Body;
//! use core_crate::frames::FrameOrigin;
//! use core_crate::{
//!     PointMassGravity, ReferenceSource, SharedCentralGravity, SharedScientificSource,
//! };
//! use dynamics::{PointMassGravityModel, SystemDynamics, TwoBodyDynamics};
//! use units::GravitationalParameter;
//!
//! let mu = GravitationalParameter::from_cubic_metres_per_second_squared(3.986_004_418e14)
//!     .expect("positive gravitational parameter");
//! let source: SharedScientificSource = Arc::new(
//!     ReferenceSource::new("IERS", "IERS Conventions", "2010", "IERS TN 36")
//!         .expect("complete source record"),
//! );
//! let central_gravity: SharedCentralGravity = Arc::new(
//!     PointMassGravity::new(FrameOrigin::Body(Body::EARTH), mu, source)
//!         .expect("complete source record"),
//! );
//! let gravity = PointMassGravityModel::new(central_gravity);
//! let model = TwoBodyDynamics::new(gravity);
//! assert_eq!(model.conservative_force_models().len(), 1);
//! assert_eq!(
//!     model.conservative_force_models()[0].force().name(),
//!     "gravity"
//! );
//! assert!(model.non_conservative_force_models().is_empty());
//! ```

use std::{fmt, sync::Arc};

use core_crate::SharedCentralGravity;

mod propagator;
mod two_body;

pub use propagator::Propagator;
pub use two_body::{EllipticKeplerError, EllipticKeplerPropagator};

/// Spacecraft-state components required to evaluate a force model.
///
/// Requirements compose through [`Self::union`] and are queried through
/// [`Self::contains`]. The opaque representation permits future state
/// components without adding positional constructor arguments.
///
/// ```
/// use dynamics::SpacecraftStateRequirements;
///
/// let requirements = SpacecraftStateRequirements::POSITION
///     .union(SpacecraftStateRequirements::VELOCITY);
/// assert!(requirements.contains(SpacecraftStateRequirements::POSITION));
/// assert!(!requirements.contains(SpacecraftStateRequirements::MASS));
/// ```
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpacecraftStateRequirements(u8);

impl SpacecraftStateRequirements {
    /// No spacecraft-state component is required.
    pub const NONE: Self = Self(0);
    /// The framed spacecraft position vector is required.
    pub const POSITION: Self = Self(1 << 0);
    /// The framed spacecraft velocity vector is required.
    pub const VELOCITY: Self = Self(1 << 1);
    /// The current spacecraft mass is required.
    pub const MASS: Self = Self(1 << 2);
    /// The current spacecraft attitude is required.
    pub const ATTITUDE: Self = Self(1 << 3);
    /// The framed spacecraft angular-velocity vector is required.
    pub const ANGULAR_VELOCITY: Self = Self(1 << 4);
    /// The framed spacecraft inertia tensor is required.
    pub const INERTIA: Self = Self(1 << 5);

    /// Combines two sets of spacecraft-state requirements.
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
/// The force model owns all environmental configuration and declares the
/// spacecraft-state components required by its evaluation. No evaluation
/// method is defined until the state representation and data context are
/// designed.
pub trait ForceModel: fmt::Debug + Send + Sync {
    /// Returns a stable human-readable implementation name for diagnostics.
    fn model_name(&self) -> &str;

    /// Returns the physical force modeled by this implementation.
    ///
    /// Returning a trait object rather than an associated type keeps
    /// [`ForceModel`] object-safe so heterogeneous implementations can be
    /// composed in one dynamics description.
    fn force(&self) -> &dyn Force;

    /// Returns the spacecraft-state components required by this model.
    fn state_requirements(&self) -> SpacecraftStateRequirements;
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

/// Point-mass model of gravity from one shared, sourced gravity provider.
///
/// Only spacecraft position is an interaction dependency. The attracting body
/// or barycenter origin, gravitational parameter, and provenance remain bound
/// together; none is inferred from another field.
#[derive(Debug, Clone)]
pub struct PointMassGravityModel {
    central_gravity: SharedCentralGravity,
}

impl PointMassGravityModel {
    /// Describes point-mass gravity using the supplied sourced provider.
    #[must_use]
    pub fn new(central_gravity: SharedCentralGravity) -> Self {
        Self { central_gravity }
    }

    /// Returns the sourced central-gravity provider used by this model.
    #[must_use]
    pub const fn central_gravity(&self) -> &SharedCentralGravity {
        &self.central_gravity
    }
}

impl ForceModel for PointMassGravityModel {
    fn model_name(&self) -> &str {
        "point-mass gravity model"
    }

    fn force(&self) -> &dyn Force {
        &GRAVITY_FORCE
    }

    fn state_requirements(&self) -> SpacecraftStateRequirements {
        SpacecraftStateRequirements::POSITION
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

    /// Returns the sourced gravity provider of the central point-mass model.
    #[must_use]
    pub fn central_gravity(&self) -> &SharedCentralGravity {
        self.central_gravity_model.central_gravity()
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
    use std::sync::Arc;

    use bodies::Body;
    use core_crate::frames::FrameOrigin;
    use core_crate::{CentralGravity, ScientificSource};
    use units::GravitationalParameter;

    #[derive(Debug)]
    struct TestSource {
        product: String,
    }

    impl ScientificSource for TestSource {
        fn authority(&self) -> &str {
            "orskit test"
        }

        fn product(&self) -> &str {
            &self.product
        }

        fn version_or_scenario(&self) -> &str {
            "test scenario"
        }

        fn locator(&self) -> &str {
            "crates/dynamics/src/lib.rs"
        }
    }

    #[derive(Debug)]
    struct TestCentralGravity {
        origin: FrameOrigin,
        gravitational_parameter: GravitationalParameter,
        source: TestSource,
    }

    impl CentralGravity for TestCentralGravity {
        fn origin(&self) -> FrameOrigin {
            self.origin
        }

        fn gravitational_parameter(&self) -> GravitationalParameter {
            self.gravitational_parameter
        }

        fn source(&self) -> &dyn ScientificSource {
            &self.source
        }
    }

    fn point_mass(body: Body, mu_m3_s2: f64) -> PointMassGravityModel {
        let gravitational_parameter =
            GravitationalParameter::from_cubic_metres_per_second_squared(mu_m3_s2)
                .expect("fixture gravitational parameter is positive");
        PointMassGravityModel::new(Arc::new(TestCentralGravity {
            origin: FrameOrigin::Body(body),
            gravitational_parameter,
            source: TestSource {
                product: format!("{body} point-mass fixture"),
            },
        }))
    }

    fn earth_gravity() -> PointMassGravityModel {
        point_mass(Body::EARTH, 3.986_004_418e14)
    }

    #[test]
    fn two_body_preserves_exactly_one_central_point_mass_model() {
        let gravity = earth_gravity();
        let expected_gravity = gravity.central_gravity().clone();
        let dynamics = TwoBodyDynamics::new(gravity);

        assert_eq!(dynamics.name(), "two-body spacecraft dynamics");
        assert!(Arc::ptr_eq(dynamics.central_gravity(), &expected_gravity));
        assert!(Arc::ptr_eq(
            dynamics.central_gravity_model().central_gravity(),
            &expected_gravity
        ));
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
            dynamics.conservative_force_models()[0].state_requirements(),
            SpacecraftStateRequirements::POSITION
        );
    }

    #[test]
    fn state_requirements_compose_without_positional_booleans() {
        let requirements = SpacecraftStateRequirements::POSITION
            .union(SpacecraftStateRequirements::VELOCITY)
            .union(SpacecraftStateRequirements::MASS)
            .union(SpacecraftStateRequirements::ANGULAR_VELOCITY);

        assert!(requirements.contains(SpacecraftStateRequirements::POSITION));
        assert!(requirements.contains(SpacecraftStateRequirements::VELOCITY));
        assert!(requirements.contains(
            SpacecraftStateRequirements::POSITION.union(SpacecraftStateRequirements::MASS)
        ));
        assert!(requirements.contains(SpacecraftStateRequirements::NONE));
        assert!(!requirements.contains(SpacecraftStateRequirements::ATTITUDE));
        assert!(!requirements.contains(SpacecraftStateRequirements::INERTIA));
    }
}
