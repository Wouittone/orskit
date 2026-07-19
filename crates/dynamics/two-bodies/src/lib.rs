#![forbid(unsafe_code)]

//! Two-body dynamics implementation and its elliptic Kepler solver.
//!
//! The reusable force-model and propagation contracts are provided by
//! `dynamics`; this crate selects one physical topology and orbit
//! implementations explicitly.

use std::{fmt, sync::Arc};

use dynamics::{
    ConservativeForceModel, ConservativeForceModelHandle, Force, ForceModel,
    SpacecraftStateRequirements, SystemDynamics,
};
use gravity::SharedCentralGravity;

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

#[cfg(test)]
mod tests {
    use super::*;
    use dynamics::Propagator;
    use frames::{Body, FrameOrigin, InertialFrame};
    use gravity::CentralGravityProvider;
    use hifitime::{Duration, Epoch};
    use orbits::{
        cartesian::CartesianState, circular::CircularState, equinoctial::EquinoctialState,
        keplerian::KeplerianState,
    };
    use orskit_core::Orbit;
    use std::sync::Arc;
    use units::uom::si::{angle::radian, length::meter, ratio::ratio};
    use units::{Angle, GravitationalParameter, Length, Position, Ratio, VelocityVector};

    #[derive(Debug)]
    struct TestGravity;

    impl CentralGravityProvider for TestGravity {
        fn origin(&self) -> frames::FrameOrigin {
            FrameOrigin::Body(Body::EARTH)
        }
        fn parameter(&self) -> GravitationalParameter {
            GravitationalParameter::try_from(3.986_004_418e14).expect("positive parameter")
        }
    }

    fn problem() -> (SharedCentralGravity, TwoBodyDynamics) {
        let gravity: SharedCentralGravity = Arc::new(TestGravity);
        let dynamics = TwoBodyDynamics::new(PointMassGravityModel::new(gravity.clone()));
        (gravity, dynamics)
    }

    fn keplerian(gravity: SharedCentralGravity) -> KeplerianState {
        KeplerianState::new(
            InertialFrame::GCRF,
            gravity,
            Length::new::<meter>(7_200_000.0),
            Ratio::new::<ratio>(0.1),
            Angle::new::<radian>(0.7),
            Angle::new::<radian>(1.1),
            Angle::new::<radian>(0.4),
            Angle::new::<radian>(2.0),
        )
        .expect("valid elliptic state")
    }

    #[test]
    fn propagates_each_supported_representation_to_the_requested_epoch() {
        let (gravity, problem) = problem();
        let target = Epoch::from_tai_seconds(4_600.0);
        let solver = EllipticKeplerPropagator::new(problem.clone());
        let keplerian = keplerian(gravity.clone());
        let circular = CircularState::try_from(keplerian.clone()).expect("conversion");
        let equinoctial = EquinoctialState::try_from(keplerian.clone()).expect("conversion");
        let cartesian: CartesianState = keplerian.clone().try_into().expect("conversion");

        assert_eq!(
            solver
                .propagate(
                    Orbit::new(Epoch::from_tai_seconds(1_000.0), circular),
                    target
                )
                .expect("circular propagation")
                .epoch(),
            target,
        );
        assert_eq!(
            solver
                .propagate(
                    Orbit::new(Epoch::from_tai_seconds(1_000.0), keplerian),
                    target
                )
                .expect("Keplerian propagation")
                .epoch(),
            target,
        );
        assert_eq!(
            solver
                .propagate(
                    Orbit::new(Epoch::from_tai_seconds(1_000.0), equinoctial),
                    target
                )
                .expect("equinoctial propagation")
                .epoch(),
            target,
        );
        assert_eq!(
            solver
                .propagate(
                    Orbit::new(Epoch::from_tai_seconds(1_000.0), cartesian),
                    target
                )
                .expect("Cartesian propagation")
                .epoch(),
            target,
        );
    }

    #[test]
    fn target_epoch_replaces_duration_at_the_public_boundary() {
        let (gravity, problem) = problem();
        let initial = Orbit::new(Epoch::from_tai_seconds(1_000.0), keplerian(gravity));
        let target = initial.epoch() + Duration::from_seconds(900.0);
        let result = EllipticKeplerPropagator::new(problem.clone())
            .propagate(initial, target)
            .expect("propagation");
        assert_eq!(result.epoch(), target);
    }

    #[test]
    fn cartesian_state_is_not_a_core_requirement() {
        let state = CartesianState::new(
            frames::ReferenceFrame::GCRF,
            Position::from_metres(7_000_000.0, 0.0, 0.0),
            VelocityVector::from_metres_per_second(0.0, 7_500.0, 0.0),
        )
        .expect("finite Cartesian state");
        assert_eq!(state.frame(), frames::ReferenceFrame::GCRF);
    }
}
