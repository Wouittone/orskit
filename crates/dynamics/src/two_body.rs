use std::f64::consts::{PI, TAU};
use std::sync::Arc;

use core_crate::{KeplerianState, Orbit, SharedCentralGravity, SpacecraftState, StateError};
use hifitime::Duration;
use thiserror::Error;
use units::uom::si::{angle::radian, length::meter, ratio::ratio};
use units::Angle;

use crate::{Propagator, TwoBodyDynamics};

const DEFAULT_TOLERANCE_RADIANS: f64 = 1.0e-13;
const DEFAULT_MAX_ITERATIONS: usize = 32;

/// Analytical point-mass propagation for bound elliptic spacecraft states.
///
/// The solution advances mean anomaly with `n = sqrt(mu / a^3)`, solves
/// `M = E - e sin(E)` by bounded Newton iteration, and converts eccentric
/// anomaly back to true anomaly. These relations follow the public
/// [NASA GMAT Mathematical Specifications](https://ntrs.nasa.gov/citations/20080031744).
///
/// The solver implements [`Propagator<TwoBodyDynamics>`]; the problem owns the
/// two-body topology and gravity provider, while this type owns only analytical
/// solution settings. A different compatible solver can implement the same
/// propagation contract without changing the problem type.
///
/// The input [`SpacecraftState`] variant is preserved. This translational
/// evaluator returns only an epoch-qualified orbit and makes no claim about
/// spacecraft mass, inertia, or attitude at the resulting epoch.
#[derive(Debug, Clone)]
pub struct EllipticKeplerPropagator {
    tolerance_radians: f64,
    max_iterations: usize,
}

impl Default for EllipticKeplerPropagator {
    fn default() -> Self {
        Self::new()
    }
}

impl EllipticKeplerPropagator {
    /// Creates an elliptic analytical propagator with documented defaults.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            tolerance_radians: DEFAULT_TOLERANCE_RADIANS,
            max_iterations: DEFAULT_MAX_ITERATIONS,
        }
    }

    /// Sets the anomaly-solver tolerance in radians.
    pub fn with_tolerance_radians(
        mut self,
        tolerance_radians: f64,
    ) -> Result<Self, EllipticKeplerError> {
        if !tolerance_radians.is_finite() || tolerance_radians <= 0.0 {
            return Err(EllipticKeplerError::InvalidTolerance);
        }
        self.tolerance_radians = tolerance_radians;
        Ok(self)
    }

    /// Sets the maximum anomaly-solver iteration count.
    pub fn with_max_iterations(
        mut self,
        max_iterations: usize,
    ) -> Result<Self, EllipticKeplerError> {
        if max_iterations == 0 {
            return Err(EllipticKeplerError::ZeroIterations);
        }
        self.max_iterations = max_iterations;
        Ok(self)
    }

    fn propagate_keplerian(
        &self,
        initial: KeplerianState,
        problem: &TwoBodyDynamics,
        duration: Duration,
    ) -> Result<KeplerianState, EllipticKeplerError> {
        ensure_problem_gravity(initial.central_gravity(), problem.central_gravity())?;
        let elapsed_seconds = duration.to_seconds();
        if !elapsed_seconds.is_finite() {
            return Err(EllipticKeplerError::NonFiniteDuration);
        }

        let semi_major_axis_m = initial.semi_major_axis().get::<meter>();
        let eccentricity = initial.eccentricity().get::<ratio>();
        let initial_true_anomaly = initial.true_anomaly().get::<radian>();
        let initial_eccentric_anomaly = true_to_eccentric(initial_true_anomaly, eccentricity);
        let initial_mean_anomaly =
            initial_eccentric_anomaly - eccentricity * initial_eccentric_anomaly.sin();
        let mean_motion = (problem
            .central_gravity()
            .gravitational_parameter()
            .as_cubic_metres_per_second_squared()
            / semi_major_axis_m.powi(3))
        .sqrt();
        let propagated_mean_anomaly = initial_mean_anomaly + mean_motion * elapsed_seconds;
        if !propagated_mean_anomaly.is_finite() {
            return Err(EllipticKeplerError::NonFiniteMeanAnomaly);
        }

        let eccentric_anomaly = solve_elliptic_kepler(
            normalize_signed(propagated_mean_anomaly),
            eccentricity,
            self.tolerance_radians,
            self.max_iterations,
        )?;
        let true_anomaly = eccentric_to_true(eccentric_anomaly, eccentricity);
        Ok(KeplerianState::new(
            initial.inertial_frame(),
            initial.central_gravity().clone(),
            initial.semi_major_axis(),
            initial.eccentricity(),
            initial.inclination(),
            initial.right_ascension_of_ascending_node(),
            initial.argument_of_periapsis(),
            Angle::new::<radian>(true_anomaly),
        )?)
    }

    fn propagate_state(
        &self,
        initial: SpacecraftState,
        problem: &TwoBodyDynamics,
        duration: Duration,
    ) -> Result<SpacecraftState, EllipticKeplerError> {
        match initial {
            SpacecraftState::Keplerian(state) => {
                Ok(self.propagate_keplerian(state, problem, duration)?.into())
            }
            SpacecraftState::Equinoctial(state) => {
                ensure_problem_gravity(state.central_gravity(), problem.central_gravity())?;
                let keplerian = state.to_keplerian()?;
                let propagated = self.propagate_keplerian(keplerian, problem, duration)?;
                Ok(SpacecraftState::Equinoctial(propagated.to_equinoctial()?))
            }
            SpacecraftState::Cartesian(state) => {
                let keplerian = state.to_keplerian(problem.central_gravity())?;
                let propagated = self.propagate_keplerian(keplerian, problem, duration)?;
                Ok(SpacecraftState::Cartesian(
                    propagated.to_cartesian(problem.central_gravity())?,
                ))
            }
        }
    }
}

impl Propagator<TwoBodyDynamics> for EllipticKeplerPropagator {
    type Error = EllipticKeplerError;

    fn propagate(
        &self,
        initial: Orbit,
        problem: &TwoBodyDynamics,
        duration: Duration,
    ) -> Result<Orbit, Self::Error> {
        let state = self.propagate_state(initial.state(), problem, duration)?;
        Ok(Orbit::new(initial.epoch() + duration, state))
    }
}

fn ensure_problem_gravity(
    state: &SharedCentralGravity,
    problem: &SharedCentralGravity,
) -> Result<(), EllipticKeplerError> {
    if !Arc::ptr_eq(state, problem) {
        return Err(EllipticKeplerError::CentralGravityMismatch);
    }
    Ok(())
}

fn true_to_eccentric(true_anomaly: f64, eccentricity: f64) -> f64 {
    let scale = (1.0 - eccentricity * eccentricity).sqrt();
    (scale * true_anomaly.sin()).atan2(eccentricity + true_anomaly.cos())
}

fn eccentric_to_true(eccentric_anomaly: f64, eccentricity: f64) -> f64 {
    let scale = (1.0 - eccentricity * eccentricity).sqrt();
    (scale * eccentric_anomaly.sin()).atan2(eccentric_anomaly.cos() - eccentricity)
}

fn solve_elliptic_kepler(
    mean_anomaly: f64,
    eccentricity: f64,
    tolerance: f64,
    max_iterations: usize,
) -> Result<f64, EllipticKeplerError> {
    let mut eccentric_anomaly = if eccentricity < 0.8 {
        mean_anomaly
    } else {
        PI.copysign(mean_anomaly)
    };

    for _ in 0..max_iterations {
        let residual = eccentric_anomaly - eccentricity * eccentric_anomaly.sin() - mean_anomaly;
        let derivative = 1.0 - eccentricity * eccentric_anomaly.cos();
        let correction = residual / derivative;
        eccentric_anomaly -= correction;
        if correction.abs() <= tolerance {
            let final_residual =
                eccentric_anomaly - eccentricity * eccentric_anomaly.sin() - mean_anomaly;
            if final_residual.abs() <= tolerance {
                return Ok(eccentric_anomaly);
            }
        }
    }

    Err(EllipticKeplerError::DidNotConverge {
        iterations: max_iterations,
    })
}

fn normalize_signed(angle: f64) -> f64 {
    (angle + PI).rem_euclid(TAU) - PI
}

/// Error returned by analytical elliptic Kepler propagation.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EllipticKeplerError {
    /// The elapsed duration could not be represented as finite seconds.
    #[error("propagation duration must be finite")]
    NonFiniteDuration,
    /// Mean anomaly overflowed after applying the requested duration.
    #[error("propagated mean anomaly must be finite")]
    NonFiniteMeanAnomaly,
    /// The configured anomaly tolerance was not positive and finite.
    #[error("anomaly tolerance must be positive and finite")]
    InvalidTolerance,
    /// At least one anomaly-solver iteration is required.
    #[error("maximum anomaly iterations must be greater than zero")]
    ZeroIterations,
    /// Newton iteration did not meet the configured residual tolerance.
    #[error("elliptic Kepler solver did not converge within {iterations} iterations")]
    DidNotConverge {
        /// Iteration limit that was exhausted.
        iterations: usize,
    },
    /// An element state is bound to a different gravity provider than the
    /// explicit two-body problem, even if their numeric parameters match.
    #[error("orbital elements and two-body problem use different central-gravity providers")]
    CentralGravityMismatch,
    /// The propagated orbital state failed validation.
    #[error(transparent)]
    InvalidState(#[from] StateError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use bodies::Body;
    use core_crate::frames::{FrameOrigin, InertialFrame};
    use core_crate::{CartesianState, CentralGravity, ScientificSource};
    use hifitime::Epoch;
    use units::{GravitationalParameter, Length, Ratio};

    use crate::PointMassGravityModel;

    #[derive(Debug)]
    struct TestSource;

    impl ScientificSource for TestSource {
        fn authority(&self) -> &str {
            "orskit test"
        }

        fn product(&self) -> &str {
            "two-body propagation fixture"
        }

        fn version_or_scenario(&self) -> &str {
            "test scenario"
        }

        fn locator(&self) -> &str {
            "crates/dynamics/src/two_body.rs"
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

    fn earth_mu() -> GravitationalParameter {
        GravitationalParameter::from_cubic_metres_per_second_squared(3.986_004_418e14)
            .expect("Earth gravitational parameter is positive")
    }

    fn lox_earth_mu() -> GravitationalParameter {
        GravitationalParameter::from_cubic_metres_per_second_squared(3.986_004_355_070_227e14)
            .expect("Lox Earth gravitational parameter is positive")
    }

    fn gravity(origin: FrameOrigin, mu: GravitationalParameter) -> SharedCentralGravity {
        Arc::new(TestCentralGravity {
            origin,
            gravitational_parameter: mu,
            source: TestSource,
        })
    }

    fn earth_gravity(mu: GravitationalParameter) -> SharedCentralGravity {
        gravity(FrameOrigin::Body(Body::EARTH), mu)
    }

    fn problem(central_gravity: &SharedCentralGravity) -> TwoBodyDynamics {
        TwoBodyDynamics::new(PointMassGravityModel::new(central_gravity.clone()))
    }

    fn orbit(
        central_gravity: &SharedCentralGravity,
        eccentricity: f64,
        true_anomaly: f64,
    ) -> KeplerianState {
        KeplerianState::new(
            InertialFrame::GCRF,
            central_gravity.clone(),
            Length::new::<meter>(7_200_000.0),
            Ratio::new::<ratio>(eccentricity),
            Angle::new::<radian>(0.7),
            Angle::new::<radian>(1.1),
            Angle::new::<radian>(0.4),
            Angle::new::<radian>(true_anomaly),
        )
        .expect("fixture orbit is elliptic")
    }

    fn initial(
        central_gravity: &SharedCentralGravity,
        eccentricity: f64,
        true_anomaly: f64,
    ) -> Orbit {
        Orbit::new(
            Epoch::from_tai_seconds(1_000.0),
            orbit(central_gravity, eccentricity, true_anomaly).into(),
        )
    }

    fn cartesian(state: SpacecraftState, central_gravity: &SharedCentralGravity) -> CartesianState {
        match state {
            SpacecraftState::Cartesian(state) => state,
            SpacecraftState::Keplerian(state) => {
                state.to_cartesian(central_gravity).expect("conversion")
            }
            SpacecraftState::Equinoctial(state) => {
                state.to_cartesian(central_gravity).expect("conversion")
            }
        }
    }

    fn assert_vector_close(actual: [f64; 3], expected: [f64; 3], tolerance: f64) {
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!(
                (actual - expected).abs() <= tolerance,
                "actual {actual} differs from expected {expected} by more than {tolerance}"
            );
        }
    }

    fn propagate_all_representations(
        mu: GravitationalParameter,
        duration: Duration,
    ) -> [CartesianState; 3] {
        let central_gravity = earth_gravity(mu);
        let problem = problem(&central_gravity);
        let propagator = EllipticKeplerPropagator::new();
        let keplerian = initial(&central_gravity, 0.1, 2.0);
        let state = match keplerian.state() {
            SpacecraftState::Keplerian(state) => state,
            _ => unreachable!(),
        };
        let equinoctial = Orbit::new(
            keplerian.epoch(),
            state.clone().to_equinoctial().expect("conversion").into(),
        );
        let cartesian_orbit = Orbit::new(
            keplerian.epoch(),
            state
                .to_cartesian(&central_gravity)
                .expect("conversion")
                .into(),
        );

        [keplerian, equinoctial, cartesian_orbit].map(|initial| {
            let propagated = propagator
                .propagate(initial, &problem, duration)
                .expect("propagation converges");
            cartesian(propagated.state(), &central_gravity)
        })
    }

    fn assert_all_representations_match_reference(
        mu: GravitationalParameter,
        expected_position_m: [f64; 3],
        expected_velocity_m_s: [f64; 3],
    ) {
        for result in propagate_all_representations(mu, Duration::from_seconds(3_600.0)) {
            assert_vector_close(result.position().to_metres(), expected_position_m, 1.0e-6);
            assert_vector_close(
                result.velocity().to_metres_per_second(),
                expected_velocity_m_s,
                1.0e-9,
            );
        }
    }

    #[test]
    fn circular_half_period_reaches_opposite_state() {
        let central_gravity = earth_gravity(earth_mu());
        let problem = problem(&central_gravity);
        let initial = initial(&central_gravity, 0.0, 0.0);
        let initial_orbit = match initial.state() {
            SpacecraftState::Keplerian(state) => state,
            _ => unreachable!(),
        };
        let radius = initial_orbit.semi_major_axis().get::<meter>();
        let half_period =
            PI * (radius.powi(3) / earth_mu().as_cubic_metres_per_second_squared()).sqrt();
        let propagated = EllipticKeplerPropagator::new()
            .propagate(
                initial.clone(),
                &problem,
                Duration::from_seconds(half_period),
            )
            .expect("circular propagation converges");
        let propagated_cartesian = cartesian(propagated.state(), &central_gravity);
        let initial_cartesian = cartesian(initial.state(), &central_gravity);

        assert_vector_close(
            propagated_cartesian.position().to_metres(),
            initial_cartesian.position().to_metres().map(|value| -value),
            1.0e-5,
        );
        assert_vector_close(
            propagated_cartesian.velocity().to_metres_per_second(),
            initial_cartesian
                .velocity()
                .to_metres_per_second()
                .map(|value| -value),
            1.0e-8,
        );
    }

    #[test]
    fn propagation_preserves_variant_and_orbital_invariants() {
        let central_gravity = earth_gravity(earth_mu());
        let problem = problem(&central_gravity);
        let initial = initial(&central_gravity, 0.2, 1.3);
        let propagated = EllipticKeplerPropagator::new()
            .propagate(initial.clone(), &problem, Duration::from_seconds(1_800.0))
            .expect("elliptic propagation converges");
        let (before, after) = match (initial.state(), propagated.state()) {
            (SpacecraftState::Keplerian(before), SpacecraftState::Keplerian(after)) => {
                (before, after)
            }
            _ => panic!("propagator must preserve state variant"),
        };

        assert_eq!(after.semi_major_axis(), before.semi_major_axis());
        assert_eq!(after.eccentricity(), before.eccentricity());
        assert_eq!(after.inclination(), before.inclination());
        assert_eq!(
            after.right_ascension_of_ascending_node(),
            before.right_ascension_of_ascending_node()
        );
        assert_eq!(
            after.argument_of_periapsis(),
            before.argument_of_periapsis()
        );
        assert_eq!(
            propagated.epoch(),
            initial.epoch() + Duration::from_seconds(1_800.0)
        );
    }

    #[test]
    fn signed_duration_round_trip_recovers_cartesian_state() {
        let central_gravity = earth_gravity(earth_mu());
        let problem = problem(&central_gravity);
        let initial = initial(&central_gravity, 0.6, 2.1);
        let propagator = EllipticKeplerPropagator::new();
        let forward = propagator
            .propagate(initial.clone(), &problem, Duration::from_seconds(3_600.0))
            .expect("forward propagation converges");
        let recovered = propagator
            .propagate(forward, &problem, Duration::from_seconds(-3_600.0))
            .expect("backward propagation converges");
        let initial_cartesian = cartesian(initial.state(), &central_gravity);
        let recovered_cartesian = cartesian(recovered.state(), &central_gravity);

        assert_vector_close(
            recovered_cartesian.position().to_metres(),
            initial_cartesian.position().to_metres(),
            1.0e-8,
        );
        assert_vector_close(
            recovered_cartesian.velocity().to_metres_per_second(),
            initial_cartesian.velocity().to_metres_per_second(),
            1.0e-10,
        );
    }

    #[test]
    fn target_epoch_matches_duration_propagation() {
        let central_gravity = earth_gravity(earth_mu());
        let problem = problem(&central_gravity);
        let initial = initial(&central_gravity, 0.1, 2.0);
        let duration = Duration::from_seconds(900.0);
        let target = initial.epoch() + duration;
        let concrete = EllipticKeplerPropagator::new();
        let propagator: &dyn Propagator<TwoBodyDynamics, Error = EllipticKeplerError> = &concrete;
        let by_duration = propagator
            .propagate(initial.clone(), &problem, duration)
            .expect("duration propagation converges");
        let by_epoch = propagator
            .propagate_to(initial, &problem, target)
            .expect("target propagation converges");

        assert_eq!(by_epoch.epoch(), by_duration.epoch());
        assert_eq!(by_epoch.state(), by_duration.state());
    }

    #[test]
    fn invalid_solver_configuration_and_iteration_limit_are_reported() {
        let propagator = EllipticKeplerPropagator::new();
        assert!(matches!(
            propagator.clone().with_tolerance_radians(0.0),
            Err(EllipticKeplerError::InvalidTolerance)
        ));
        assert!(matches!(
            propagator.clone().with_max_iterations(0),
            Err(EllipticKeplerError::ZeroIterations)
        ));
        let one_iteration = propagator
            .with_max_iterations(1)
            .expect("one iteration is valid");
        let central_gravity = earth_gravity(earth_mu());
        let problem = problem(&central_gravity);
        let difficult = initial(&central_gravity, 0.99, 0.2);
        assert!(matches!(
            one_iteration.propagate(difficult, &problem, Duration::from_seconds(4_000.0)),
            Err(EllipticKeplerError::DidNotConverge { iterations: 1 })
        ));
    }

    #[test]
    fn element_gravity_identity_must_match_the_explicit_problem() {
        let state_gravity = earth_gravity(earth_mu());
        let numerically_equal_but_distinct_gravity = earth_gravity(earth_mu());
        let initial = initial(&state_gravity, 0.1, 2.0);
        let problem = problem(&numerically_equal_but_distinct_gravity);

        assert!(matches!(
            EllipticKeplerPropagator::new().propagate(
                initial,
                &problem,
                Duration::from_seconds(60.0),
            ),
            Err(EllipticKeplerError::CentralGravityMismatch)
        ));
    }

    #[test]
    fn cartesian_origin_must_match_the_explicit_problem() {
        let earth_gravity = earth_gravity(earth_mu());
        let earth_elements = orbit(&earth_gravity, 0.1, 2.0);
        let cartesian_state = earth_elements
            .to_cartesian(&earth_gravity)
            .expect("fixture conversion");
        let initial = Orbit::new(Epoch::from_tai_seconds(1_000.0), cartesian_state.into());
        let mars_gravity = gravity(FrameOrigin::Body(Body::MARS), earth_mu());
        let problem = problem(&mars_gravity);

        assert!(matches!(
            EllipticKeplerPropagator::new().propagate(
                initial,
                &problem,
                Duration::from_seconds(60.0),
            ),
            Err(EllipticKeplerError::InvalidState(_))
        ));
    }

    #[test]
    fn matches_orekit_13_1_6_black_box_reference() {
        assert_all_representations_match_reference(
            earth_mu(),
            [
                4.863_976_030_492_352e6,
                4.133_125_643_091_070_5e6,
                -2.072_064_351_084_958e6,
            ],
            [
                -3.449_464_728_617_805e3,
                5.450_564_161_064_824_5e3,
                4.671_788_819_571_301e3,
            ],
        );
    }

    #[test]
    fn matches_lox_0_1_0_alpha_39_black_box_reference() {
        assert_all_representations_match_reference(
            lox_earth_mu(),
            [
                4.863_976_128_518_66e6,
                4.133_125_488_197_863e6,
                -2.072_064_483_847_062_6e6,
            ],
            [
                -3.449_464_519_081_433_5e3,
                5.450_564_272_952_743e3,
                4.671_788_705_029_827e3,
            ],
        );
    }
}
