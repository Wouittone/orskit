#![forbid(unsafe_code)]

//! Concrete point-mass two-body dynamics and its elliptic Kepler solver.
//!
//! The reusable force-model and propagation contracts are provided by
//! `orskit-dynamics`; this crate selects one physical topology and one state
//! implementation explicitly.

use std::{fmt, sync::Arc};

use core_crate::SharedCentralGravity;
use dynamics::{
    ConservativeForceModel, ConservativeForceModelHandle, Force, ForceModel,
    SpacecraftStateRequirements, SystemDynamics,
};

mod two_body;

pub use two_body::{EllipticKeplerError, EllipticKeplerPropagator};

/// Physical gravitational interaction.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct GravityForce;

impl Force for GravityForce {
    fn name(&self) -> &str {
        "gravity"
    }
}

static GRAVITY_FORCE: GravityForce = GravityForce;

/// Point-mass model of gravity from one shared, sourced gravity provider.
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

    /// Returns the selected gravity provider.
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

/// Strict two-body spacecraft dynamics containing exactly one point-mass model.
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

    /// Returns the selected gravity provider.
    #[must_use]
    pub fn central_gravity(&self) -> &SharedCentralGravity {
        self.central_gravity_model.central_gravity()
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
    fn non_conservative_force_models(&self) -> &[dynamics::NonConservativeForceModelHandle] {
        &[]
    }
}
