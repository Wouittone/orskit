use std::f64::consts::{PI, TAU};

use hifitime::{Duration, Epoch};
use orskit_bodies::Body;
use orskit_core::{CoordinateSample, KeplerianCoordinates, KeplerianState, State, StateError};
use orskit_units::uom::si::{angle::radian, length::meter, ratio::ratio};
use orskit_units::{Angle, GravitationalParameter};
use thiserror::Error;

use crate::TwoBodyDynamics;

const DEFAULT_TOLERANCE_RADIANS: f64 = 1.0e-13;
const DEFAULT_MAX_ITERATIONS: usize = 32;

/// Analytical point-mass propagation for an elliptic Keplerian state.
///
/// The solution advances mean anomaly with `n = sqrt(mu / a^3)`, solves
/// `M = E - e sin(E)` by bounded Newton iteration, and converts eccentric
/// anomaly back to true anomaly. These relations follow the public
/// [NASA GMAT Mathematical Specifications](https://ntrs.nasa.gov/citations/20080031744).
///
/// This evaluator is intentionally narrower than [`crate::SystemDynamics`]:
/// it supports the existing elliptic [`KeplerianState`] regime and the single
/// point-mass force installed by [`TwoBodyDynamics::new`]. It performs no frame
/// transform and never infers a gravitational parameter from body identity.
/// Mass, orientation, and inertia are copied unchanged because this first
/// evaluator advances translational orbit elements only.
///
/// ```no_run
/// use hifitime::Duration;
/// use orskit_bodies::Body;
/// use orskit_core::KeplerianState;
/// use orskit_dynamics::EllipticTwoBodyPropagator;
/// use orskit_units::GravitationalParameter;
///
/// # fn propagate(
/// #     initial: &KeplerianState,
/// #     mu: GravitationalParameter,
/// # ) -> Result<KeplerianState, orskit_dynamics::TwoBodyPropagationError> {
/// let propagator = EllipticTwoBodyPropagator::new(Body::EARTH, mu);
/// propagator.propagate(initial, Duration::from_seconds(3_600.0))
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct EllipticTwoBodyPropagator {
    dynamics: TwoBodyDynamics,
    gravitational_parameter: GravitationalParameter,
    tolerance_radians: f64,
    max_iterations: usize,
}

impl EllipticTwoBodyPropagator {
    /// Creates an elliptic analytical propagator for one attracting body.
    #[must_use]
    pub fn new(attractor: Body, gravitational_parameter: GravitationalParameter) -> Self {
        Self {
            dynamics: TwoBodyDynamics::new(attractor),
            gravitational_parameter,
            tolerance_radians: DEFAULT_TOLERANCE_RADIANS,
            max_iterations: DEFAULT_MAX_ITERATIONS,
        }
    }

    /// Sets the anomaly-solver tolerance in radians.
    pub fn with_tolerance_radians(
        mut self,
        tolerance_radians: f64,
    ) -> Result<Self, TwoBodyPropagationError> {
        if !tolerance_radians.is_finite() || tolerance_radians <= 0.0 {
            return Err(TwoBodyPropagationError::InvalidTolerance);
        }
        self.tolerance_radians = tolerance_radians;
        Ok(self)
    }

    /// Sets the maximum anomaly-solver iteration count.
    pub fn with_max_iterations(
        mut self,
        max_iterations: usize,
    ) -> Result<Self, TwoBodyPropagationError> {
        if max_iterations == 0 {
            return Err(TwoBodyPropagationError::ZeroIterations);
        }
        self.max_iterations = max_iterations;
        Ok(self)
    }

    /// Returns the underlying two-body dynamics description.
    #[must_use]
    pub const fn dynamics(&self) -> &TwoBodyDynamics {
        &self.dynamics
    }

    /// Returns the explicit central gravitational parameter.
    #[must_use]
    pub const fn gravitational_parameter(&self) -> GravitationalParameter {
        self.gravitational_parameter
    }

    /// Propagates by a signed duration.
    pub fn propagate(
        &self,
        initial: &KeplerianState,
        duration: Duration,
    ) -> Result<KeplerianState, TwoBodyPropagationError> {
        let elapsed_seconds = duration.to_seconds();
        if !elapsed_seconds.is_finite() {
            return Err(TwoBodyPropagationError::NonFiniteDuration);
        }

        let source = initial.coordinates();
        let semi_major_axis_m = source.semi_major_axis().get::<meter>();
        let eccentricity = source.eccentricity().get::<ratio>();
        let initial_true_anomaly = source.true_anomaly().get::<radian>();
        let initial_eccentric_anomaly = true_to_eccentric(initial_true_anomaly, eccentricity);
        let initial_mean_anomaly =
            initial_eccentric_anomaly - eccentricity * initial_eccentric_anomaly.sin();
        let mean_motion = (self
            .gravitational_parameter
            .as_cubic_metres_per_second_squared()
            / semi_major_axis_m.powi(3))
        .sqrt();
        let propagated_mean_anomaly = initial_mean_anomaly + mean_motion * elapsed_seconds;
        if !propagated_mean_anomaly.is_finite() {
            return Err(TwoBodyPropagationError::NonFiniteMeanAnomaly);
        }

        let eccentric_anomaly = solve_elliptic_kepler(
            normalize_signed(propagated_mean_anomaly),
            eccentricity,
            self.tolerance_radians,
            self.max_iterations,
        )?;
        let true_anomaly = eccentric_to_true(eccentric_anomaly, eccentricity);
        let propagated = KeplerianCoordinates::new(
            source.frame(),
            source.semi_major_axis(),
            source.eccentricity(),
            source.inclination(),
            source.right_ascension_of_ascending_node(),
            source.argument_of_periapsis(),
            Angle::new::<radian>(true_anomaly),
        )?;

        Ok(KeplerianState::new(
            CoordinateSample::new(initial.epoch() + duration, propagated),
            initial.properties().clone(),
        ))
    }

    /// Propagates to an explicit target epoch.
    pub fn propagate_to(
        &self,
        initial: &KeplerianState,
        target: Epoch,
    ) -> Result<KeplerianState, TwoBodyPropagationError> {
        self.propagate(initial, target - initial.epoch())
    }
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
) -> Result<f64, TwoBodyPropagationError> {
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

    Err(TwoBodyPropagationError::DidNotConverge {
        iterations: max_iterations,
    })
}

fn normalize_signed(angle: f64) -> f64 {
    (angle + PI).rem_euclid(TAU) - PI
}

/// Error returned by elliptic analytical two-body propagation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum TwoBodyPropagationError {
    /// The elapsed duration could not be represented as a finite number of seconds.
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
    /// The propagated coordinates failed core state validation.
    #[error(transparent)]
    InvalidState(#[from] StateError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use orskit_core::frames::{CustomFrameId, FrameOrientation, FrameOrigin, ReferenceFrame};
    use orskit_core::{
        CartesianState, InertiaTensor, Orientation, SpacecraftProperties, StateConversion,
    };
    use orskit_units::uom::si::{
        mass::kilogram, moment_of_inertia::kilogram_square_meter, ratio::ratio,
    };
    use orskit_units::{Length, Mass, MomentOfInertia, Ratio};

    fn earth_mu() -> GravitationalParameter {
        GravitationalParameter::from_cubic_metres_per_second_squared(3.986_004_418e14)
            .expect("Earth gravitational parameter is positive")
    }

    fn lox_earth_mu() -> GravitationalParameter {
        GravitationalParameter::from_cubic_metres_per_second_squared(3.986_004_355_070_227e14)
            .expect("Lox Earth gravitational parameter is positive")
    }

    fn properties() -> SpacecraftProperties {
        let id = CustomFrameId::new(11);
        let body_frame = ReferenceFrame::new(FrameOrigin::Custom(id), FrameOrientation::Custom(id));
        let orientation = Orientation::identity(body_frame, ReferenceFrame::GCRF);
        let inertia = InertiaTensor::principal(
            body_frame,
            MomentOfInertia::new::<kilogram_square_meter>(800.0),
            MomentOfInertia::new::<kilogram_square_meter>(900.0),
            MomentOfInertia::new::<kilogram_square_meter>(1_000.0),
        )
        .expect("fixture inertia is physical");
        SpacecraftProperties::new(Mass::new::<kilogram>(500.0), orientation, inertia)
            .expect("fixture properties are physical")
    }

    fn state(eccentricity: f64, true_anomaly: f64) -> KeplerianState {
        let coordinates = KeplerianCoordinates::new(
            ReferenceFrame::GCRF,
            Length::new::<meter>(7_200_000.0),
            Ratio::new::<ratio>(eccentricity),
            Angle::new::<radian>(0.7),
            Angle::new::<radian>(1.1),
            Angle::new::<radian>(0.4),
            Angle::new::<radian>(true_anomaly),
        )
        .expect("fixture orbit is elliptic");
        KeplerianState::new(
            CoordinateSample::new(Epoch::from_tai_seconds(1_000.0), coordinates),
            properties(),
        )
    }

    fn assert_vector_close(actual: [f64; 3], expected: [f64; 3], tolerance: f64) {
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!(
                (actual - expected).abs() <= tolerance,
                "actual {actual} differs from expected {expected} by more than {tolerance}"
            );
        }
    }

    #[test]
    fn circular_half_period_reaches_opposite_state() {
        let initial = state(0.0, 0.0);
        let propagator = EllipticTwoBodyPropagator::new(Body::EARTH, earth_mu());
        let radius = initial.coordinates().semi_major_axis().get::<meter>();
        let half_period =
            PI * (radius.powi(3) / earth_mu().as_cubic_metres_per_second_squared()).sqrt();
        let propagated = propagator
            .propagate(&initial, Duration::from_seconds(half_period))
            .expect("circular propagation converges");
        let cartesian: CartesianState = propagated
            .convert(earth_mu())
            .expect("propagated elements convert");
        let initial_cartesian: CartesianState = initial
            .convert(earth_mu())
            .expect("initial elements convert");

        assert_vector_close(
            cartesian.position().value().to_metres(),
            initial_cartesian
                .position()
                .value()
                .to_metres()
                .map(|value| -value),
            // Hifitime stores the computed half-period at nanosecond resolution;
            // 0.5 ns at LEO speed corresponds to roughly 4 micrometres.
            1.0e-5,
        );
        assert_vector_close(
            cartesian.velocity().value().to_metres_per_second(),
            initial_cartesian
                .velocity()
                .value()
                .to_metres_per_second()
                .map(|value| -value),
            1.0e-8,
        );
    }

    #[test]
    fn propagation_preserves_invariants_and_properties() {
        let initial = state(0.2, 1.3);
        let propagated = EllipticTwoBodyPropagator::new(Body::EARTH, earth_mu())
            .propagate(&initial, Duration::from_seconds(1_800.0))
            .expect("elliptic propagation converges");

        assert_eq!(
            propagated.coordinates().semi_major_axis(),
            initial.coordinates().semi_major_axis()
        );
        assert_eq!(
            propagated.coordinates().eccentricity(),
            initial.coordinates().eccentricity()
        );
        assert_eq!(
            propagated.coordinates().inclination(),
            initial.coordinates().inclination()
        );
        assert_eq!(
            propagated.coordinates().right_ascension_of_ascending_node(),
            initial.coordinates().right_ascension_of_ascending_node()
        );
        assert_eq!(
            propagated.coordinates().argument_of_periapsis(),
            initial.coordinates().argument_of_periapsis()
        );
        assert_eq!(propagated.properties(), initial.properties());
    }

    #[test]
    fn signed_duration_round_trip_recovers_cartesian_state() {
        let initial = state(0.6, 2.1);
        let propagator = EllipticTwoBodyPropagator::new(Body::EARTH, earth_mu());
        let forward = propagator
            .propagate(&initial, Duration::from_seconds(3_600.0))
            .expect("forward propagation converges");
        let recovered = propagator
            .propagate(&forward, Duration::from_seconds(-3_600.0))
            .expect("backward propagation converges");
        let initial_cartesian: CartesianState = initial.convert(earth_mu()).expect("conversion");
        let recovered_cartesian: CartesianState =
            recovered.convert(earth_mu()).expect("conversion");

        assert_vector_close(
            recovered_cartesian.position().value().to_metres(),
            initial_cartesian.position().value().to_metres(),
            1.0e-8,
        );
        assert_vector_close(
            recovered_cartesian
                .velocity()
                .value()
                .to_metres_per_second(),
            initial_cartesian.velocity().value().to_metres_per_second(),
            1.0e-10,
        );
    }

    #[test]
    fn target_epoch_matches_duration_propagation() {
        let initial = state(0.1, 2.0);
        let duration = Duration::from_seconds(900.0);
        let propagator = EllipticTwoBodyPropagator::new(Body::EARTH, earth_mu());
        let by_duration = propagator
            .propagate(&initial, duration)
            .expect("duration propagation converges");
        let by_epoch = propagator
            .propagate_to(&initial, initial.epoch() + duration)
            .expect("target propagation converges");

        assert_eq!(by_epoch, by_duration);
    }

    #[test]
    fn invalid_solver_configuration_is_rejected() {
        let propagator = EllipticTwoBodyPropagator::new(Body::EARTH, earth_mu());
        assert!(matches!(
            propagator.clone().with_tolerance_radians(0.0),
            Err(TwoBodyPropagationError::InvalidTolerance)
        ));
        assert!(matches!(
            propagator.with_max_iterations(0),
            Err(TwoBodyPropagationError::ZeroIterations)
        ));
    }

    #[test]
    fn iteration_limit_is_reported() {
        let initial = state(0.99, 0.2);
        let propagator = EllipticTwoBodyPropagator::new(Body::EARTH, earth_mu())
            .with_max_iterations(1)
            .expect("one iteration is valid configuration");
        assert!(matches!(
            propagator.propagate(&initial, Duration::from_seconds(4_000.0)),
            Err(TwoBodyPropagationError::DidNotConverge { iterations: 1 })
        ));
    }

    #[test]
    fn matches_orekit_13_1_6_black_box_reference() {
        // Generated by the isolated harness under
        // `.agent/references/two-body/orekit` using KeplerianPropagator.
        let expected_position_m = [
            4.863_976_030_492_352e6,
            4.133_125_643_091_070_5e6,
            -2.072_064_351_084_958e6,
        ];
        let expected_velocity_m_s = [
            -3.449_464_728_617_805e3,
            5.450_564_161_064_824_5e3,
            4.671_788_819_571_301e3,
        ];
        let initial = state(0.1, 2.0);
        let propagated = EllipticTwoBodyPropagator::new(Body::EARTH, earth_mu())
            .propagate(&initial, Duration::from_seconds(3_600.0))
            .expect("reference case converges");
        let cartesian: CartesianState = propagated
            .convert(earth_mu())
            .expect("reference result converts");

        assert_vector_close(
            cartesian.position().value().to_metres(),
            expected_position_m,
            1.0e-6,
        );
        assert_vector_close(
            cartesian.velocity().value().to_metres_per_second(),
            expected_velocity_m_s,
            1.0e-9,
        );
    }

    #[test]
    fn matches_lox_0_1_0_alpha_39_black_box_reference() {
        // Generated by the isolated public-API harness under
        // `.agent/references/two-body/lox` using Vallado propagation. Lox's
        // built-in Earth origin supplies the gravitational parameter recorded
        // by `lox_earth_mu`, so this case deliberately differs from Orekit's.
        let expected_position_m = [
            4.863_976_128_518_66e6,
            4.133_125_488_197_863e6,
            -2.072_064_483_847_062_6e6,
        ];
        let expected_velocity_m_s = [
            -3.449_464_519_081_433_5e3,
            5.450_564_272_952_743e3,
            4.671_788_705_029_827e3,
        ];
        let initial = state(0.1, 2.0);
        let propagated = EllipticTwoBodyPropagator::new(Body::EARTH, lox_earth_mu())
            .propagate(&initial, Duration::from_seconds(3_600.0))
            .expect("reference case converges");
        let cartesian: CartesianState = propagated
            .convert(lox_earth_mu())
            .expect("reference result converts");

        assert_vector_close(
            cartesian.position().value().to_metres(),
            expected_position_m,
            1.0e-6,
        );
        assert_vector_close(
            cartesian.velocity().value().to_metres_per_second(),
            expected_velocity_m_s,
            1.0e-9,
        );
    }
}
