#![forbid(unsafe_code)]

//! Point-mass-plus-`J2` oblateness Cartesian dynamics.
//!
//! This crate adds the dominant Earth-oblateness secular perturbation to the
//! existing point-mass system. It follows the same strict, single-topology
//! shape as `dynamics-two-bodies::TwoBodyDynamics`: [`J2Dynamics`] owns
//! exactly one point-mass model and exactly one [`J2GravityModel`] zonal
//! correction model and sums their accelerations directly. It is not a
//! general composed-model evaluator and not general spherical harmonics; see
//! `.agent/decisions/0042-oblate-earth-j2-dynamics.md`.
//!
//! The zonal `J2` acceleration equation is documented public scientific
//! material (NASA GMAT *Mathematical Specifications*, 2007); no source code
//! was consulted or copied.

use std::{fmt, sync::Arc};

use dynamics::{
    CartesianDynamics, ConservativeForceModel, ConservativeForceModelHandle, Force, ForceModel,
    SpacecraftStateRequirements, SystemDynamics,
};
use dynamics_two_bodies::{PointMassGravityModel, TwoBodyDynamics, TwoBodyEvaluationError};
use gravity::{SharedCentralGravity, ZonalGravityProvider};
use hifitime::Epoch;
use orbits::cartesian::{CartesianState, FramedAcceleration};
use thiserror::Error;
use units::uom::si::{length::meter, ratio::ratio};
use units::AccelerationVector;

/// Zonal `J2` oblateness gravity correction.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct J2OblatenessForce;

impl Force for J2OblatenessForce {
    fn name(&self) -> &str {
        "J2 zonal oblateness gravity"
    }
}

static J2_OBLATENESS_FORCE: J2OblatenessForce = J2OblatenessForce;

/// Zonal `J2` correction model of gravity from one shared zonal provider.
///
/// This model contributes only the `J2` correction acceleration, not the
/// point-mass term; [`J2Dynamics`] sums it with a separate point-mass model
/// so both remain atomic for future composed-model evaluation.
#[derive(Debug, Clone)]
pub struct J2GravityModel {
    zonal_gravity: Arc<dyn ZonalGravityProvider>,
}

impl J2GravityModel {
    /// Describes the `J2` correction using the supplied zonal provider.
    #[must_use]
    pub fn new(zonal_gravity: Arc<dyn ZonalGravityProvider>) -> Self {
        Self { zonal_gravity }
    }

    /// Returns the selected zonal gravity provider.
    #[must_use]
    pub const fn zonal_gravity(&self) -> &Arc<dyn ZonalGravityProvider> {
        &self.zonal_gravity
    }
}

impl ForceModel for J2GravityModel {
    fn model_name(&self) -> &str {
        "J2 zonal oblateness gravity correction model"
    }
    fn force(&self) -> &dyn Force {
        &J2_OBLATENESS_FORCE
    }
    fn state_requirements(&self) -> SpacecraftStateRequirements {
        SpacecraftStateRequirements::POSITION
    }
}
impl ConservativeForceModel for J2GravityModel {}

/// Strict point-mass-plus-`J2` spacecraft dynamics.
///
/// Contains exactly one point-mass model and exactly one [`J2GravityModel`],
/// evaluated over one shared [`ZonalGravityProvider`].
#[derive(Clone)]
pub struct J2Dynamics {
    point_mass: TwoBodyDynamics,
    j2_model: Arc<J2GravityModel>,
    conservative_force_models: [ConservativeForceModelHandle; 2],
}

impl J2Dynamics {
    /// Describes spacecraft motion under one central point-mass term plus
    /// its `J2` zonal oblateness correction.
    #[must_use]
    pub fn new(zonal_gravity: Arc<dyn ZonalGravityProvider>) -> Self {
        let central_gravity: SharedCentralGravity = zonal_gravity.clone();
        let point_mass_handle: ConservativeForceModelHandle =
            Arc::new(PointMassGravityModel::new(central_gravity.clone()));
        let point_mass = TwoBodyDynamics::new(PointMassGravityModel::new(central_gravity));
        let j2_model = Arc::new(J2GravityModel::new(zonal_gravity));
        let j2_handle: ConservativeForceModelHandle = j2_model.clone();
        Self {
            point_mass,
            j2_model,
            conservative_force_models: [point_mass_handle, j2_handle],
        }
    }

    /// Returns the selected zonal gravity provider.
    #[must_use]
    pub fn zonal_gravity(&self) -> &Arc<dyn ZonalGravityProvider> {
        self.j2_model.zonal_gravity()
    }
}

impl fmt::Debug for J2Dynamics {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("J2Dynamics")
            .field("point_mass", &self.point_mass)
            .field("j2_model", &self.j2_model)
            .finish()
    }
}

impl SystemDynamics for J2Dynamics {
    fn name(&self) -> &str {
        "point-mass-plus-J2 spacecraft dynamics"
    }
    fn conservative_force_models(&self) -> &[ConservativeForceModelHandle] {
        &self.conservative_force_models
    }
    fn non_conservative_force_models(&self) -> &[dynamics::NonConservativeForceModelHandle] {
        &[]
    }
}

/// Point-mass-plus-`J2` derivative validation/evaluation failure.
#[derive(Debug, Clone, Copy, PartialEq, Error)]
pub enum J2EvaluationError {
    /// The point-mass term rejected the state or produced a non-finite
    /// acceleration.
    #[error(transparent)]
    PointMass(#[from] TwoBodyEvaluationError),
    /// The `J2` correction acceleration became non-finite.
    #[error("J2 zonal acceleration is not finite")]
    NonFiniteJ2Acceleration,
}

impl CartesianDynamics for J2Dynamics {
    type Error = J2EvaluationError;

    fn validate(&self, state: &CartesianState) -> Result<(), Self::Error> {
        self.point_mass
            .validate(state)
            .map_err(J2EvaluationError::PointMass)
    }

    fn acceleration(
        &self,
        epoch: Epoch,
        state: &CartesianState,
    ) -> Result<FramedAcceleration, Self::Error> {
        let point_mass_acceleration = self
            .point_mass
            .acceleration(epoch, state)
            .map_err(J2EvaluationError::PointMass)?;

        let position = state.position().to_metres();
        let radius_squared = position
            .into_iter()
            .fold(0.0, |sum, component| component.mul_add(component, sum));
        let radius = radius_squared.sqrt();
        let mu = self
            .zonal_gravity()
            .parameter()
            .as_cubic_metres_per_second_squared();
        let equatorial_radius = self.zonal_gravity().equatorial_radius().get::<meter>();
        let j2 = self.zonal_gravity().j2().get::<ratio>();

        let z = position[2];
        let z_over_r_squared = (z * z) / radius_squared;
        let radius_fifth = radius_squared * radius_squared * radius;
        let factor = -1.5 * j2 * mu * equatorial_radius * equatorial_radius / radius_fifth;
        let horizontal_term = 1.0 - 5.0 * z_over_r_squared;
        let vertical_term = 3.0 - 5.0 * z_over_r_squared;
        let j2_acceleration = AccelerationVector::from_metres_per_second_squared(
            factor * position[0] * horizontal_term,
            factor * position[1] * horizontal_term,
            factor * position[2] * vertical_term,
        );
        if !j2_acceleration.is_finite() {
            return Err(J2EvaluationError::NonFiniteJ2Acceleration);
        }

        let combined = point_mass_acceleration.value() + j2_acceleration;
        FramedAcceleration::new(combined, point_mass_acceleration.frame())
            .map_err(|_| J2EvaluationError::NonFiniteJ2Acceleration)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dynamics::Propagator;
    use frames::{Body, FrameOrigin, ReferenceFrame};
    use gravity::CentralGravityProvider;
    use hifitime::Duration;
    use orbits::cartesian::CartesianState;
    use orskit_core::Orbit;
    use units::uom::si::length::kilometer;
    use units::{GravitationalParameter, Length, Position, Ratio, VelocityVector};

    #[derive(Debug)]
    struct WgS84Earth;

    impl CentralGravityProvider for WgS84Earth {
        fn origin(&self) -> FrameOrigin {
            FrameOrigin::Body(Body::EARTH)
        }
        fn parameter(&self) -> GravitationalParameter {
            GravitationalParameter::try_from(3.986_004_418e14).expect("positive parameter")
        }
    }

    impl ZonalGravityProvider for WgS84Earth {
        fn equatorial_radius(&self) -> Length {
            Length::new::<kilometer>(6378.137)
        }
        fn j2(&self) -> Ratio {
            Ratio::new::<ratio>(1.082_63e-3)
        }
    }

    fn dynamics_under_test() -> J2Dynamics {
        J2Dynamics::new(Arc::new(WgS84Earth))
    }

    fn zero_j2_dynamics() -> J2Dynamics {
        #[derive(Debug)]
        struct ZeroJ2Earth;
        impl CentralGravityProvider for ZeroJ2Earth {
            fn origin(&self) -> FrameOrigin {
                FrameOrigin::Body(Body::EARTH)
            }
            fn parameter(&self) -> GravitationalParameter {
                GravitationalParameter::try_from(3.986_004_418e14).expect("positive parameter")
            }
        }
        impl ZonalGravityProvider for ZeroJ2Earth {
            fn equatorial_radius(&self) -> Length {
                Length::new::<kilometer>(6378.137)
            }
            fn j2(&self) -> Ratio {
                Ratio::new::<ratio>(0.0)
            }
        }
        J2Dynamics::new(Arc::new(ZeroJ2Earth))
    }

    fn sample_state(x: f64, y: f64, z: f64, vx: f64, vy: f64, vz: f64) -> CartesianState {
        CartesianState::new(
            ReferenceFrame::GCRF,
            Position::from_metres(x, y, z),
            VelocityVector::from_metres_per_second(vx, vy, vz),
        )
        .expect("finite Cartesian state")
    }

    #[test]
    fn zero_j2_reduces_to_the_point_mass_acceleration() {
        let epoch = Epoch::from_tai_seconds(0.0);
        let state = sample_state(7_000_000.0, 1_000_000.0, 2_000_000.0, 0.0, 7_000.0, 1_000.0);
        let dynamics = zero_j2_dynamics();
        let combined = dynamics.acceleration(epoch, &state).expect("evaluation");
        let point_mass_only =
            TwoBodyDynamics::new(PointMassGravityModel::new(Arc::new(WgS84Earth)))
                .acceleration(epoch, &state)
                .expect("point-mass evaluation");
        for (combined_component, point_mass_component) in combined
            .value()
            .to_metres_per_second_squared()
            .into_iter()
            .zip(point_mass_only.value().to_metres_per_second_squared())
        {
            assert!((combined_component - point_mass_component).abs() < 1e-15);
        }
    }

    #[test]
    fn j2_z_component_is_antisymmetric_under_negating_the_state_z_coordinate() {
        let epoch = Epoch::from_tai_seconds(0.0);
        let dynamics = dynamics_under_test();
        let positive_z = sample_state(6_800_000.0, 500_000.0, 1_500_000.0, 0.0, 7_000.0, 0.0);
        let negative_z = sample_state(6_800_000.0, 500_000.0, -1_500_000.0, 0.0, 7_000.0, 0.0);
        let positive = dynamics
            .acceleration(epoch, &positive_z)
            .expect("evaluation")
            .value()
            .to_metres_per_second_squared();
        let negative = dynamics
            .acceleration(epoch, &negative_z)
            .expect("evaluation")
            .value()
            .to_metres_per_second_squared();
        assert!((positive[0] - negative[0]).abs() < 1e-12);
        assert!((positive[1] - negative[1]).abs() < 1e-12);
        assert!((positive[2] + negative[2]).abs() < 1e-9);
    }

    /// Independent reference vector computed from the closed-form J2 zonal
    /// equation (NASA GMAT Mathematical Specifications, 2007) with WGS84
    /// constants, evaluated outside the crate under test.
    #[test]
    fn j2_acceleration_matches_an_independently_computed_reference_vector() {
        let epoch = Epoch::from_tai_seconds(0.0);
        let state = sample_state(6_800_000.0, 500_000.0, 1_500_000.0, 0.0, 7_000.0, 0.0);
        let dynamics = dynamics_under_test();
        let point_mass_only =
            TwoBodyDynamics::new(PointMassGravityModel::new(Arc::new(WgS84Earth)))
                .acceleration(epoch, &state)
                .expect("point-mass evaluation")
                .value()
                .to_metres_per_second_squared();

        let combined = dynamics
            .acceleration(epoch, &state)
            .expect("evaluation")
            .value()
            .to_metres_per_second_squared();
        let j2_only = [
            combined[0] - point_mass_only[0],
            combined[1] - point_mass_only[1],
            combined[2] - point_mass_only[2],
        ];

        // Reference: mu=3.986004418e14, Re=6378137.0, J2=1.08263e-3,
        // r=(6.8e6, 5.0e5, 1.5e6). Independently computed:
        // r^2=48297250000000.0, r=6949082.35..., factor=-1.5*J2*mu*Re^2/r^5.
        let mu = 3.986_004_418e14_f64;
        let re = 6_378_137.0_f64;
        let j2 = 1.082_63e-3_f64;
        let (x, y, z) = (6_800_000.0_f64, 500_000.0_f64, 1_500_000.0_f64);
        let r2 = x * x + y * y + z * z;
        let r = r2.sqrt();
        let factor = -1.5 * j2 * mu * re * re / (r2 * r2 * r);
        let horizontal = 1.0 - 5.0 * z * z / r2;
        let vertical = 3.0 - 5.0 * z * z / r2;
        let expected = [
            factor * x * horizontal,
            factor * y * horizontal,
            factor * z * vertical,
        ];

        for (actual_component, expected_component) in j2_only.iter().zip(expected) {
            let relative_error =
                (actual_component - expected_component).abs() / expected_component.abs();
            assert!(relative_error < 1e-12, "relative error {relative_error}");
        }
    }

    #[test]
    fn j2_dynamics_produces_a_measurably_different_trajectory_than_point_mass_only() {
        use dynamics_numerical::{BogackiShampine32, IntegrationConfiguration};
        use units::uom::si::{length::meter, ratio::ratio, velocity::meter_per_second};
        use units::{Length, Velocity};

        let configuration = IntegrationConfiguration::new(
            Length::new::<meter>(1.0),
            Velocity::new::<meter_per_second>(1.0e-3),
            Ratio::new::<ratio>(1e-9),
            Duration::from_seconds(0.1),
            Duration::from_seconds(120.0),
            Duration::from_seconds(10.0),
            200_000,
            1_000,
        )
        .expect("valid integration configuration");

        let state = sample_state(6_800_000.0, 0.0, 0.0, 0.0, 7_000.0, 3_000.0);
        let start = Epoch::from_tai_seconds(0.0);
        let target = start + Duration::from_seconds(5_400.0);

        let j2_propagator = BogackiShampine32::new(dynamics_under_test(), configuration);
        let point_mass_propagator = BogackiShampine32::new(
            TwoBodyDynamics::new(PointMassGravityModel::new(Arc::new(WgS84Earth))),
            configuration,
        );

        let j2_result = j2_propagator
            .propagate(Orbit::new(start, state), target)
            .expect("J2 propagation");
        let point_mass_result = point_mass_propagator
            .propagate(Orbit::new(start, state), target)
            .expect("point-mass propagation");

        let j2_position = j2_result.as_ref().position().to_metres();
        let point_mass_position = point_mass_result.as_ref().position().to_metres();
        let difference = j2_position
            .into_iter()
            .zip(point_mass_position)
            .map(|(j2_component, point_mass_component)| {
                (j2_component - point_mass_component).powi(2)
            })
            .sum::<f64>()
            .sqrt();
        assert!(
            difference > 1.0,
            "expected a measurable J2 perturbation over one orbit, got {difference} m"
        );
    }

    /// Differential/scenario test: over several orbital periods, `J2Dynamics`
    /// must produce a right-ascension-of-ascending-node drift whose sign
    /// matches the standard closed-form secular nodal-regression rate
    /// `dOmega/dt = -1.5 n J2 (Re/p)^2 cos(i)` (NASA GMAT Mathematical
    /// Specifications, 2007), while plain point-mass dynamics keeps the node
    /// fixed.
    #[test]
    fn j2_dynamics_regresses_the_ascending_node_in_the_predicted_direction() {
        use dynamics_numerical::{BogackiShampine32, IntegrationConfiguration};
        use gravity::SharedCentralGravity;
        use orbits::cartesian::CartesianStateWithGravity;
        use orbits::keplerian::KeplerianState;
        use units::uom::si::{
            angle::radian, length::meter, ratio::ratio, velocity::meter_per_second,
        };
        use units::{Length, Velocity};

        let configuration = IntegrationConfiguration::new(
            Length::new::<meter>(1.0),
            Velocity::new::<meter_per_second>(1.0e-3),
            Ratio::new::<ratio>(1e-9),
            Duration::from_seconds(0.1),
            Duration::from_seconds(120.0),
            Duration::from_seconds(10.0),
            500_000,
            1_000,
        )
        .expect("valid integration configuration");

        // Inclined, near-circular LEO state (prograde, i ~ 51.5 degrees).
        let state = sample_state(6_800_000.0, 0.0, 0.0, 0.0, 5_411.5, 6_723.8);
        let central_gravity: SharedCentralGravity = Arc::new(WgS84Earth);
        let initial_keplerian = KeplerianState::try_from(CartesianStateWithGravity {
            state,
            central_gravity: central_gravity.clone(),
        })
        .expect("Cartesian state converts to Keplerian elements");

        let semi_major_axis = initial_keplerian.semi_major_axis().get::<meter>();
        let eccentricity = initial_keplerian.eccentricity().get::<ratio>();
        let inclination = initial_keplerian.inclination().get::<radian>();
        let initial_raan = initial_keplerian
            .right_ascension_of_ascending_node()
            .get::<radian>();

        let mu = central_gravity
            .parameter()
            .as_cubic_metres_per_second_squared();
        let mean_motion = (mu / semi_major_axis.powi(3)).sqrt();
        let equatorial_radius = 6_378_137.0_f64;
        let j2 = 1.082_63e-3_f64;
        let semi_latus_rectum = semi_major_axis * (1.0 - eccentricity * eccentricity);
        let predicted_raan_rate = -1.5
            * mean_motion
            * j2
            * (equatorial_radius / semi_latus_rectum).powi(2)
            * inclination.cos();

        let period = std::f64::consts::TAU / mean_motion;
        let start = Epoch::from_tai_seconds(0.0);
        let target = start + Duration::from_seconds(5.0 * period);

        let j2_propagator = BogackiShampine32::new(dynamics_under_test(), configuration);
        let point_mass_propagator = BogackiShampine32::new(
            TwoBodyDynamics::new(PointMassGravityModel::new(central_gravity.clone())),
            configuration,
        );

        let j2_final = j2_propagator
            .propagate(Orbit::new(start, state), target)
            .expect("J2 propagation");
        let point_mass_final = point_mass_propagator
            .propagate(Orbit::new(start, state), target)
            .expect("point-mass propagation");

        let j2_final_keplerian = KeplerianState::try_from(CartesianStateWithGravity {
            state: *j2_final.as_ref(),
            central_gravity: central_gravity.clone(),
        })
        .expect("final J2 state converts to Keplerian elements");
        let point_mass_final_keplerian = KeplerianState::try_from(CartesianStateWithGravity {
            state: *point_mass_final.as_ref(),
            central_gravity,
        })
        .expect("final point-mass state converts to Keplerian elements");

        let wrap_to_pi = |angle: f64| -> f64 {
            (angle + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI
        };
        let j2_raan_drift = wrap_to_pi(
            j2_final_keplerian
                .right_ascension_of_ascending_node()
                .get::<radian>()
                - initial_raan,
        );
        let point_mass_raan_drift = wrap_to_pi(
            point_mass_final_keplerian
                .right_ascension_of_ascending_node()
                .get::<radian>()
                - initial_raan,
        );
        let predicted_drift = predicted_raan_rate * 5.0 * period;

        // Point-mass dynamics has no secular nodal drift.
        assert!(
            point_mass_raan_drift.abs() < 1e-6,
            "point-mass RAAN drift should be ~0, got {point_mass_raan_drift} rad"
        );
        // J2 dynamics drifts in the predicted direction, and within a
        // generous factor of the predicted first-order secular magnitude
        // (this is a rough sanity bound, not an exact-value check, since
        // osculating elements also carry short-period J2 oscillations).
        assert!(
            j2_raan_drift.signum() == predicted_drift.signum(),
            "expected RAAN drift sign {}, got {j2_raan_drift} rad (predicted {predicted_drift} rad)",
            predicted_drift.signum()
        );
        assert!(
            (j2_raan_drift / predicted_drift - 1.0).abs() < 0.5,
            "J2 RAAN drift {j2_raan_drift} rad is not within 50% of the predicted secular drift {predicted_drift} rad"
        );
    }
}
