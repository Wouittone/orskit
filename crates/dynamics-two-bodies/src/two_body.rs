use std::f64::consts::{PI, TAU};
use std::sync::Arc;

use dynamics::Propagator;
use gravity::SharedCentralGravity;
use hifitime::Duration;
use orskit_core::Orbit;
use thiserror::Error;
use units::uom::si::{angle::radian, length::meter, ratio::ratio};
use units::{Angle, Length};

use orbits::{
    cartesian::{CartesianState, StateError},
    equinoctial::EquinoctialState,
    keplerian::KeplerianState,
};

use crate::TwoBodyDynamics;

const DEFAULT_TOLERANCE_RADIANS: f64 = 1.0e-13;
const DEFAULT_MAX_ITERATIONS: usize = 32;
const DEFAULT_PHASE_ERROR_BUDGET_RADIANS: f64 = 1.0e-10;
const NANOSECONDS_PER_SECOND: u128 = 1_000_000_000;
const MAX_EXACT_F64_INTEGER: u128 = 1_u128 << f64::MANTISSA_DIGITS;

/// Analytical point-mass propagation for bound elliptic spacecraft states.
///
/// The solution advances mean anomaly with `n = sqrt(mu / a^3)`, solves
/// `M = E - e sin(E)` by bounded Newton iteration, and converts eccentric
/// anomaly back to true anomaly. These relations follow the public
/// [NASA GMAT Mathematical Specifications](https://ntrs.nasa.gov/citations/20080031744).
/// Hifitime supplies an exact signed nanosecond count `N`; this solver computes
/// `dt = N / 10^9` without first rounding the complete duration to `f64`.
/// Before range reduction, it estimates a conservative floating-point phase
/// bound `B_phi = |n| B_dt + |dt| B_n + B_mul + B_reduce` for
/// `Delta M = n dt` and rejects requests exceeding the configured budget.
/// Here `B_dt` bounds nanosecond-to-seconds conversion, `B_n` bounds evaluation
/// of `sqrt(mu / a^3)`, and the remaining terms bound multiplication and
/// periodic reduction. The estimate depends on `|N|`, so acceptance is
/// symmetric for equal forward and backward durations.
///
/// Equinoctial states are advanced without conversion through Keplerian
/// inclination. With eccentric longitude `F`, mean longitude is solved from
/// `L = F - ex sin(F) + ey cos(F)` while `a`, `ex`, `ey`, `hx`, and `hy` stay
/// constant. This keeps every finite supported `hx`/`hy` valid near the
/// retrograde singular limit of the intermediate Keplerian representation.
///
/// The solver implements [`Propagator<TwoBodyDynamics, KeplerianState>`],
/// [`Propagator<TwoBodyDynamics, EquinoctialState>`], and
/// [`Propagator<TwoBodyDynamics, CartesianState>`]; the problem owns the
/// two-body topology and gravity provider, while this type owns only analytical
/// solution settings. A different compatible solver can implement the same
/// propagation contract without changing the problem type.
///
/// The input state representation is selected at the trait boundary and is
/// preserved. This evaluator returns only an epoch-qualified orbit and makes
/// no claim about spacecraft mass, inertia, or attitude at the resulting epoch.
#[derive(Debug, Clone)]
pub struct EllipticKeplerPropagator {
    tolerance_radians: f64,
    max_iterations: usize,
    phase_error_budget_radians: f64,
}

impl Default for EllipticKeplerPropagator {
    fn default() -> Self {
        Self::new()
    }
}

impl EllipticKeplerPropagator {
    /// Creates an elliptic analytical propagator.
    ///
    /// Defaults are a `1e-13` radian anomaly residual tolerance, 32 Newton
    /// iterations, and a `1e-10` radian floating phase-error budget.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            tolerance_radians: DEFAULT_TOLERANCE_RADIANS,
            max_iterations: DEFAULT_MAX_ITERATIONS,
            phase_error_budget_radians: DEFAULT_PHASE_ERROR_BUDGET_RADIANS,
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

    /// Sets the maximum estimated floating phase error in radians.
    ///
    /// This budget covers mean-motion evaluation, duration conversion, phase
    /// multiplication, and periodic range reduction. It does not replace the
    /// separately configured anomaly-solver residual tolerance.
    ///
    /// ```
    /// use dynamics_two_bodies::EllipticKeplerPropagator;
    ///
    /// let propagator = EllipticKeplerPropagator::new()
    ///     .with_phase_error_budget_radians(1.0e-8)?;
    /// assert_eq!(propagator.phase_error_budget_radians(), 1.0e-8);
    /// # Ok::<(), dynamics_two_bodies::EllipticKeplerError>(())
    /// ```
    pub fn with_phase_error_budget_radians(
        mut self,
        phase_error_budget_radians: f64,
    ) -> Result<Self, EllipticKeplerError> {
        if !phase_error_budget_radians.is_finite() || phase_error_budget_radians <= 0.0 {
            return Err(EllipticKeplerError::InvalidPhaseErrorBudget);
        }
        self.phase_error_budget_radians = phase_error_budget_radians;
        Ok(self)
    }

    /// Returns the configured floating phase-error budget in radians.
    #[must_use]
    pub const fn phase_error_budget_radians(&self) -> f64 {
        self.phase_error_budget_radians
    }

    fn propagate_keplerian(
        &self,
        initial: KeplerianState,
        problem: &TwoBodyDynamics,
        duration: Duration,
    ) -> Result<KeplerianState, EllipticKeplerError> {
        ensure_problem_gravity(initial.central_gravity(), problem.central_gravity())?;
        let eccentricity = initial.eccentricity().get::<ratio>();
        let initial_true_anomaly = initial.true_anomaly().get::<radian>();
        let initial_eccentric_anomaly = true_to_eccentric(initial_true_anomaly, eccentricity);
        let initial_mean_anomaly =
            initial_eccentric_anomaly - eccentricity * initial_eccentric_anomaly.sin();
        let (phase_advance, _) =
            self.phase_advance(initial.semi_major_axis(), problem, duration)?;
        let propagated_mean_anomaly = normalize_signed(initial_mean_anomaly + phase_advance);

        let eccentric_anomaly = solve_elliptic_kepler(
            propagated_mean_anomaly,
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

    fn phase_advance(
        &self,
        semi_major_axis: Length,
        problem: &TwoBodyDynamics,
        duration: Duration,
    ) -> Result<(f64, f64), EllipticKeplerError> {
        let mean_motion = (problem
            .central_gravity()
            .parameter()
            .as_cubic_metres_per_second_squared()
            / semi_major_axis.get::<meter>().powi(3))
        .sqrt();
        if !mean_motion.is_finite() || mean_motion <= 0.0 {
            return Err(EllipticKeplerError::InvalidMeanMotion);
        }
        let exact_duration = ExactNanosecondDuration::from(duration);
        let phase_magnitude = mean_motion * exact_duration.seconds_magnitude;
        if !phase_magnitude.is_finite() {
            return Err(EllipticKeplerError::NonFiniteMeanAnomaly);
        }

        let estimated_phase_error_radians = phase_error_bound_radians(
            mean_motion,
            phase_magnitude,
            exact_duration.seconds_magnitude,
            exact_duration.seconds_error_bound,
        );
        if estimated_phase_error_radians > self.phase_error_budget_radians {
            return Err(EllipticKeplerError::AccuracyBudgetExceeded {
                estimated_phase_error_radians,
                budget_radians: self.phase_error_budget_radians,
            });
        }

        let signed_phase = if exact_duration.is_negative {
            -phase_magnitude
        } else {
            phase_magnitude
        };
        Ok((
            normalize_signed(signed_phase),
            estimated_phase_error_radians,
        ))
    }

    fn propagate_equinoctial(
        &self,
        initial: EquinoctialState,
        problem: &TwoBodyDynamics,
        duration: Duration,
    ) -> Result<EquinoctialState, EllipticKeplerError> {
        ensure_problem_gravity(initial.central_gravity(), problem.central_gravity())?;
        let ex = initial.eccentricity_x().get::<ratio>();
        let ey = initial.eccentricity_y().get::<ratio>();
        let eccentricity_complement = (1.0 - ex.hypot(ey).powi(2)).sqrt();
        let (initial_true_longitude, input_reduction_error) =
            self.normalize_input_angle(initial.true_longitude().get::<radian>())?;
        let true_projection = ex * initial_true_longitude.cos() + ey * initial_true_longitude.sin();
        let true_cross = ex * initial_true_longitude.sin() - ey * initial_true_longitude.cos();
        let initial_eccentric_longitude = initial_true_longitude
            - 2.0 * true_cross.atan2(1.0 + eccentricity_complement + true_projection);
        let initial_mean_longitude = initial_eccentric_longitude
            - ex * initial_eccentric_longitude.sin()
            + ey * initial_eccentric_longitude.cos();
        let (phase_advance, duration_phase_error) =
            self.phase_advance(initial.semi_major_axis(), problem, duration)?;
        let combined_phase_error = input_reduction_error + duration_phase_error;
        if combined_phase_error > self.phase_error_budget_radians {
            return Err(EllipticKeplerError::AccuracyBudgetExceeded {
                estimated_phase_error_radians: combined_phase_error,
                budget_radians: self.phase_error_budget_radians,
            });
        }
        let propagated_mean_longitude = normalize_signed(initial_mean_longitude + phase_advance);
        let eccentric_longitude = solve_equinoctial_kepler(
            propagated_mean_longitude,
            ex,
            ey,
            self.tolerance_radians,
            self.max_iterations,
        )?;
        let eccentric_projection = ex * eccentric_longitude.cos() + ey * eccentric_longitude.sin();
        let eccentric_cross = ex * eccentric_longitude.sin() - ey * eccentric_longitude.cos();
        let true_longitude = eccentric_longitude
            + 2.0 * eccentric_cross.atan2(1.0 + eccentricity_complement - eccentric_projection);

        Ok(EquinoctialState::new(
            initial.inertial_frame(),
            initial.central_gravity().clone(),
            initial.semi_major_axis(),
            initial.eccentricity_x(),
            initial.eccentricity_y(),
            initial.inclination_x(),
            initial.inclination_y(),
            Angle::new::<radian>(normalize_signed(true_longitude)),
        )?)
    }

    fn normalize_input_angle(&self, angle_radians: f64) -> Result<(f64, f64), EllipticKeplerError> {
        let estimated_phase_error_radians = range_reduction_error_bound_radians(angle_radians);
        if estimated_phase_error_radians > self.phase_error_budget_radians {
            return Err(EllipticKeplerError::AccuracyBudgetExceeded {
                estimated_phase_error_radians,
                budget_radians: self.phase_error_budget_radians,
            });
        }
        Ok((
            normalize_signed(angle_radians),
            estimated_phase_error_radians,
        ))
    }
}

impl Propagator<TwoBodyDynamics, KeplerianState> for EllipticKeplerPropagator {
    type Error = EllipticKeplerError;

    fn propagate(
        &self,
        initial: Orbit<KeplerianState>,
        problem: &TwoBodyDynamics,
        target: hifitime::Epoch,
    ) -> Result<Orbit<KeplerianState>, Self::Error> {
        ensure_problem_gravity(initial.state().central_gravity(), problem.central_gravity())?;
        let duration = target - initial.epoch();
        if duration.total_nanoseconds() == 0 {
            return Ok(initial);
        }
        let state = self.propagate_keplerian(initial.into_state(), problem, duration)?;
        Ok(Orbit::new(target, state))
    }
}

impl Propagator<TwoBodyDynamics, EquinoctialState> for EllipticKeplerPropagator {
    type Error = EllipticKeplerError;

    fn propagate(
        &self,
        initial: Orbit<EquinoctialState>,
        problem: &TwoBodyDynamics,
        target: hifitime::Epoch,
    ) -> Result<Orbit<EquinoctialState>, Self::Error> {
        ensure_problem_gravity(initial.state().central_gravity(), problem.central_gravity())?;
        let duration = target - initial.epoch();
        if duration.total_nanoseconds() == 0 {
            return Ok(initial);
        }
        let state = self.propagate_equinoctial(initial.into_state(), problem, duration)?;
        Ok(Orbit::new(target, state))
    }
}

impl Propagator<TwoBodyDynamics, CartesianState> for EllipticKeplerPropagator {
    type Error = EllipticKeplerError;

    fn propagate(
        &self,
        initial: Orbit<CartesianState>,
        problem: &TwoBodyDynamics,
        target: hifitime::Epoch,
    ) -> Result<Orbit<CartesianState>, Self::Error> {
        ensure_cartesian_problem_compatible(&initial.state(), problem)?;
        let duration = target - initial.epoch();
        if duration.total_nanoseconds() == 0 {
            return Ok(initial);
        }
        let state = initial.into_state();
        let keplerian = state.to_keplerian(problem.central_gravity())?;
        let propagated = self.propagate_keplerian(keplerian, problem, duration)?;
        let state = propagated.to_cartesian(problem.central_gravity())?;
        Ok(Orbit::new(target, state))
    }
}

fn ensure_cartesian_problem_compatible(
    state: &CartesianState,
    problem: &TwoBodyDynamics,
) -> Result<(), EllipticKeplerError> {
    let frame_origin = state.frame().origin();
    let gravity_origin = problem.central_gravity().origin();
    if frame_origin != gravity_origin {
        return Err(StateError::CentralGravityOriginMismatch {
            gravity_origin,
            frame_origin,
        }
        .into());
    }
    Ok(())
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

#[derive(Debug, Clone, Copy)]
struct ExactNanosecondDuration {
    is_negative: bool,
    seconds_magnitude: f64,
    seconds_error_bound: f64,
}

impl From<Duration> for ExactNanosecondDuration {
    fn from(duration: Duration) -> Self {
        let total_nanoseconds = duration.total_nanoseconds();
        let magnitude_nanoseconds = total_nanoseconds.unsigned_abs();
        let whole_seconds = magnitude_nanoseconds / NANOSECONDS_PER_SECOND;
        let subsecond_nanoseconds = magnitude_nanoseconds % NANOSECONDS_PER_SECOND;
        let whole_seconds_f64 = whole_seconds as f64;
        let whole_seconds_error = if whole_seconds <= MAX_EXACT_F64_INTEGER {
            0.0
        } else {
            0.5 * positive_ulp(whole_seconds_f64)
        };
        let fractional_seconds = subsecond_nanoseconds as f64 / NANOSECONDS_PER_SECOND as f64;
        let fractional_seconds_error = if subsecond_nanoseconds == 0 {
            0.0
        } else {
            0.5 * positive_ulp(fractional_seconds)
        };
        let seconds_magnitude = whole_seconds_f64 + fractional_seconds;
        let addition_error = if subsecond_nanoseconds == 0 {
            0.0
        } else {
            0.5 * positive_ulp(seconds_magnitude)
        };

        Self {
            is_negative: total_nanoseconds < 0,
            seconds_magnitude,
            seconds_error_bound: whole_seconds_error + fractional_seconds_error + addition_error,
        }
    }
}

fn phase_error_bound_radians(
    mean_motion_radians_per_second: f64,
    phase_magnitude_radians: f64,
    duration_seconds_magnitude: f64,
    duration_error_seconds: f64,
) -> f64 {
    let duration_contribution = mean_motion_radians_per_second.abs() * duration_error_seconds;
    // Two multiplications for a^3, one division, and one square root are
    // covered by a deliberately conservative eight-epsilon relative bound.
    let mean_motion_contribution =
        8.0 * f64::EPSILON * mean_motion_radians_per_second.abs() * duration_seconds_magnitude;
    let multiplication_rounding = 0.5 * positive_ulp(phase_magnitude_radians);
    let reduction_contribution = range_reduction_error_bound_radians(phase_magnitude_radians);

    duration_contribution
        + mean_motion_contribution
        + multiplication_rounding
        + reduction_contribution
        + positive_ulp(TAU)
}

fn range_reduction_error_bound_radians(angle_radians: f64) -> f64 {
    let magnitude = angle_radians.abs();
    if magnitude <= PI {
        0.0
    } else {
        let tau_multiples = (magnitude / TAU).floor() + 1.0;
        positive_ulp(magnitude) + tau_multiples * 0.5 * positive_ulp(TAU) + positive_ulp(TAU)
    }
}

fn positive_ulp(value: f64) -> f64 {
    let magnitude = value.abs();
    if magnitude == 0.0 {
        f64::from_bits(1)
    } else if magnitude.is_finite() {
        f64::from_bits(magnitude.to_bits() + 1) - magnitude
    } else {
        f64::INFINITY
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

fn solve_equinoctial_kepler(
    mean_longitude: f64,
    eccentricity_x: f64,
    eccentricity_y: f64,
    tolerance: f64,
    max_iterations: usize,
) -> Result<f64, EllipticKeplerError> {
    let eccentricity = eccentricity_x.hypot(eccentricity_y);
    let longitude_of_periapsis = eccentricity_y.atan2(eccentricity_x);
    let mean_anomaly = normalize_signed(mean_longitude - longitude_of_periapsis);
    let target_mean_longitude = mean_anomaly + longitude_of_periapsis;
    let mut eccentric_longitude = if eccentricity < 0.8 {
        target_mean_longitude
    } else {
        PI.copysign(mean_anomaly) + longitude_of_periapsis
    };

    for _ in 0..max_iterations {
        let residual = eccentric_longitude - eccentricity_x * eccentric_longitude.sin()
            + eccentricity_y * eccentric_longitude.cos()
            - target_mean_longitude;
        let derivative = 1.0
            - eccentricity_x * eccentric_longitude.cos()
            - eccentricity_y * eccentric_longitude.sin();
        let correction = residual / derivative;
        eccentric_longitude -= correction;
        if correction.abs() <= tolerance {
            let final_residual = eccentric_longitude - eccentricity_x * eccentric_longitude.sin()
                + eccentricity_y * eccentric_longitude.cos()
                - target_mean_longitude;
            if final_residual.abs() <= tolerance {
                return Ok(normalize_signed(eccentric_longitude));
            }
        }
    }

    Err(EllipticKeplerError::DidNotConverge {
        iterations: max_iterations,
    })
}

fn normalize_signed(angle: f64) -> f64 {
    if (-PI..=PI).contains(&angle) {
        angle
    } else {
        let reduced = angle.rem_euclid(TAU);
        if reduced > PI {
            reduced - TAU
        } else {
            reduced
        }
    }
}

/// Error returned by analytical elliptic Kepler propagation.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EllipticKeplerError {
    /// Mean motion could not be represented as a positive finite value.
    #[error("mean motion must be representable as a positive finite value")]
    InvalidMeanMotion,
    /// Mean anomaly overflowed after applying the requested duration.
    #[error("propagated mean anomaly must be finite")]
    NonFiniteMeanAnomaly,
    /// The configured anomaly tolerance was not positive and finite.
    #[error("anomaly tolerance must be positive and finite")]
    InvalidTolerance,
    /// The configured phase-error budget was not positive and finite.
    #[error("phase-error budget must be positive and finite")]
    InvalidPhaseErrorBudget,
    /// The estimated floating phase error exceeds the declared budget.
    #[error(
        "estimated phase error {estimated_phase_error_radians:e} rad exceeds budget {budget_radians:e} rad"
    )]
    AccuracyBudgetExceeded {
        /// Conservative bound for mean-motion evaluation, duration conversion,
        /// multiplication, and periodic range reduction.
        estimated_phase_error_radians: f64,
        /// Maximum phase error accepted by the configured solver.
        budget_radians: f64,
    },
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
    use frames::{FrameOrigin, InertialFrame};
    use gravity::CentralGravityProvider;
    use hifitime::Epoch;
    use orbits::{
        cartesian::CartesianState, equinoctial::EquinoctialState, keplerian::KeplerianState,
    };
    use orskit_core::{Orbit, SpacecraftState as SpacecraftStateContract};
    use units::{GravitationalParameter, Length, Ratio};

    use crate::PointMassGravityModel;

    #[derive(Debug)]
    struct TestCentralGravity {
        origin: FrameOrigin,
        parameter: GravitationalParameter,
    }

    impl CentralGravityProvider for TestCentralGravity {
        fn origin(&self) -> FrameOrigin {
            self.origin
        }

        fn parameter(&self) -> GravitationalParameter {
            self.parameter
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
            parameter: mu,
        })
    }

    #[derive(Debug, Clone, PartialEq)]
    enum SpacecraftState {
        Cartesian(CartesianState),
        Keplerian(KeplerianState),
        Equinoctial(EquinoctialState),
    }

    impl SpacecraftStateContract for SpacecraftState {
        fn frame(&self) -> frames::ReferenceFrame {
            match self {
                Self::Cartesian(state) => state.frame(),
                Self::Keplerian(state) => state.frame(),
                Self::Equinoctial(state) => state.frame(),
            }
        }
    }

    impl From<CartesianState> for SpacecraftState {
        fn from(value: CartesianState) -> Self {
            Self::Cartesian(value)
        }
    }
    impl From<KeplerianState> for SpacecraftState {
        fn from(value: KeplerianState) -> Self {
            Self::Keplerian(value)
        }
    }
    impl From<EquinoctialState> for SpacecraftState {
        fn from(value: EquinoctialState) -> Self {
            Self::Equinoctial(value)
        }
    }

    trait LegacyPropagator {
        fn propagate_for_test(
            &self,
            initial: Orbit<SpacecraftState>,
            problem: &TwoBodyDynamics,
            duration: Duration,
        ) -> Result<Orbit<SpacecraftState>, EllipticKeplerError>;
        fn propagate_to(
            &self,
            initial: Orbit<SpacecraftState>,
            problem: &TwoBodyDynamics,
            target: Epoch,
        ) -> Result<Orbit<SpacecraftState>, EllipticKeplerError>;
    }

    impl LegacyPropagator for EllipticKeplerPropagator {
        fn propagate_for_test(
            &self,
            initial: Orbit<SpacecraftState>,
            problem: &TwoBodyDynamics,
            duration: Duration,
        ) -> Result<Orbit<SpacecraftState>, EllipticKeplerError> {
            let target = initial.epoch() + duration;
            Self::propagate_to(self, initial, problem, target)
        }

        fn propagate_to(
            &self,
            initial: Orbit<SpacecraftState>,
            problem: &TwoBodyDynamics,
            target: Epoch,
        ) -> Result<Orbit<SpacecraftState>, EllipticKeplerError> {
            let epoch = initial.epoch();
            match initial.into_state() {
                SpacecraftState::Cartesian(state) => {
                    <Self as Propagator<TwoBodyDynamics, CartesianState>>::propagate(
                        self,
                        Orbit::new(epoch, state),
                        problem,
                        target,
                    )
                    .map(|orbit| {
                        Orbit::new(
                            orbit.epoch(),
                            SpacecraftState::Cartesian(orbit.into_state()),
                        )
                    })
                }
                SpacecraftState::Keplerian(state) => {
                    <Self as Propagator<TwoBodyDynamics, KeplerianState>>::propagate(
                        self,
                        Orbit::new(epoch, state),
                        problem,
                        target,
                    )
                    .map(|orbit| {
                        Orbit::new(
                            orbit.epoch(),
                            SpacecraftState::Keplerian(orbit.into_state()),
                        )
                    })
                }
                SpacecraftState::Equinoctial(state) => {
                    <Self as Propagator<TwoBodyDynamics, EquinoctialState>>::propagate(
                        self,
                        Orbit::new(epoch, state),
                        problem,
                        target,
                    )
                    .map(|orbit| {
                        Orbit::new(
                            orbit.epoch(),
                            SpacecraftState::Equinoctial(orbit.into_state()),
                        )
                    })
                }
            }
        }
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
    ) -> Orbit<SpacecraftState> {
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
                .propagate_for_test(initial, &problem, duration)
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
            .propagate_for_test(
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
            .propagate_for_test(initial.clone(), &problem, Duration::from_seconds(1_800.0))
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
            .propagate_for_test(initial.clone(), &problem, Duration::from_seconds(3_600.0))
            .expect("forward propagation converges");
        let recovered = propagator
            .propagate_for_test(forward, &problem, Duration::from_seconds(-3_600.0))
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
        let by_duration = concrete
            .propagate_for_test(initial.clone(), &problem, duration)
            .expect("duration propagation converges");
        let by_epoch = concrete
            .propagate_to(initial, &problem, target)
            .expect("target propagation converges");

        assert_eq!(by_epoch.epoch(), by_duration.epoch());
        assert_eq!(by_epoch.state(), by_duration.state());
    }

    #[test]
    fn zero_duration_is_exact_identity_for_every_representation() {
        let central_gravity = earth_gravity(earth_mu());
        let problem = problem(&central_gravity);
        let keplerian = initial(&central_gravity, 0.2, 1.3);
        let elements = match keplerian.state() {
            SpacecraftState::Keplerian(state) => state,
            _ => unreachable!(),
        };
        let equinoctial = Orbit::new(
            keplerian.epoch(),
            elements
                .clone()
                .to_equinoctial()
                .expect("fixture conversion")
                .into(),
        );
        let cartesian = Orbit::new(
            keplerian.epoch(),
            elements
                .to_cartesian(&central_gravity)
                .expect("fixture conversion")
                .into(),
        );
        let propagator = EllipticKeplerPropagator::new();

        for initial in [keplerian, equinoctial, cartesian] {
            let propagated = propagator
                .propagate_for_test(
                    initial.clone(),
                    &problem,
                    Duration::from_total_nanoseconds(0),
                )
                .expect("zero propagation succeeds");
            assert_eq!(propagated, initial);
        }
    }

    #[test]
    fn zero_duration_still_validates_gravity_identity() {
        let state_gravity = earth_gravity(earth_mu());
        let distinct_gravity = earth_gravity(earth_mu());
        let initial = initial(&state_gravity, 0.2, 1.3);
        let problem = problem(&distinct_gravity);

        assert!(matches!(
            EllipticKeplerPropagator::new().propagate_for_test(
                initial,
                &problem,
                Duration::from_total_nanoseconds(0),
            ),
            Err(EllipticKeplerError::CentralGravityMismatch)
        ));
    }

    #[test]
    fn exact_nanosecond_duration_changes_the_propagated_phase() {
        let central_gravity = earth_gravity(earth_mu());
        let problem = problem(&central_gravity);
        let initial = initial(&central_gravity, 0.2, 1.3);
        let whole_seconds = 3_600_i128 * NANOSECONDS_PER_SECOND as i128;
        let propagator = EllipticKeplerPropagator::new();
        let at_whole_second = propagator
            .propagate_for_test(
                initial.clone(),
                &problem,
                Duration::from_total_nanoseconds(whole_seconds),
            )
            .expect("whole-second propagation succeeds");
        let one_nanosecond_later = propagator
            .propagate_for_test(
                initial,
                &problem,
                Duration::from_total_nanoseconds(whole_seconds + 1),
            )
            .expect("nanosecond-resolved propagation succeeds");
        let anomaly = |orbit: Orbit<SpacecraftState>| match orbit.state() {
            SpacecraftState::Keplerian(state) => state.true_anomaly().get::<radian>(),
            _ => unreachable!(),
        };

        assert_ne!(anomaly(at_whole_second), anomaly(one_nanosecond_later));
    }

    #[test]
    fn long_duration_budget_rejection_is_deterministic_and_symmetric() {
        const LONG_DURATION_NANOSECONDS: i128 =
            1_000_000_000_000_i128 * NANOSECONDS_PER_SECOND as i128;

        let central_gravity = earth_gravity(earth_mu());
        let problem = problem(&central_gravity);
        let initial = initial(&central_gravity, 0.2, 1.3);
        let propagator = EllipticKeplerPropagator::new();
        let rejected_bound = |nanoseconds| match propagator.propagate_for_test(
            initial.clone(),
            &problem,
            Duration::from_total_nanoseconds(nanoseconds),
        ) {
            Err(EllipticKeplerError::AccuracyBudgetExceeded {
                estimated_phase_error_radians,
                budget_radians,
            }) => (estimated_phase_error_radians, budget_radians),
            result => panic!("expected accuracy-budget rejection, got {result:?}"),
        };

        let forward = rejected_bound(LONG_DURATION_NANOSECONDS);
        let backward = rejected_bound(-LONG_DURATION_NANOSECONDS);
        assert_eq!(forward, backward);
        assert!(forward.0 > forward.1);

        let relaxed = EllipticKeplerPropagator::new()
            .with_phase_error_budget_radians(1.0e-5)
            .expect("positive finite budget");
        assert_eq!(relaxed.phase_error_budget_radians(), 1.0e-5);
        relaxed
            .propagate_for_test(
                initial,
                &problem,
                Duration::from_total_nanoseconds(LONG_DURATION_NANOSECONDS),
            )
            .expect("declared relaxed budget accepts the same duration");
    }

    #[test]
    fn direct_equinoctial_step_preserves_large_finite_inclination_components() {
        let central_gravity = earth_gravity(earth_mu());
        let problem = problem(&central_gravity);
        let state = EquinoctialState::new(
            InertialFrame::GCRF,
            central_gravity,
            Length::new::<meter>(7_200_000.0),
            Ratio::new::<ratio>(0.1),
            Ratio::new::<ratio>(0.05),
            Ratio::new::<ratio>(1.0e16),
            Ratio::new::<ratio>(0.0),
            Angle::new::<radian>(2.0),
        )
        .expect("large finite hx is a valid equinoctial state");
        let initial_longitude = state.true_longitude();
        let propagated = EllipticKeplerPropagator::new()
            .propagate_for_test(
                Orbit::new(
                    Epoch::from_tai_seconds(1_000.0),
                    SpacecraftState::from(state),
                ),
                &problem,
                Duration::from_total_nanoseconds(60 * NANOSECONDS_PER_SECOND as i128),
            )
            .expect("direct equinoctial propagation avoids retrograde conversion");
        let propagated = match propagated.state() {
            SpacecraftState::Equinoctial(state) => state,
            _ => panic!("propagator must preserve the equinoctial variant"),
        };

        assert!(Arc::ptr_eq(
            propagated.central_gravity(),
            problem.central_gravity()
        ));
        assert_eq!(propagated.semi_major_axis().get::<meter>(), 7_200_000.0);
        assert_eq!(propagated.eccentricity_x().get::<ratio>(), 0.1);
        assert_eq!(propagated.eccentricity_y().get::<ratio>(), 0.05);
        assert_eq!(propagated.inclination_x().get::<ratio>(), 1.0e16);
        assert_eq!(propagated.inclination_y().get::<ratio>(), 0.0);
        assert_ne!(propagated.true_longitude(), initial_longitude);
    }

    #[test]
    fn large_input_longitude_is_rejected_when_reduction_exceeds_budget() {
        let central_gravity = earth_gravity(earth_mu());
        let problem = problem(&central_gravity);
        let state = EquinoctialState::new(
            InertialFrame::GCRF,
            central_gravity,
            Length::new::<meter>(7_200_000.0),
            Ratio::new::<ratio>(0.0),
            Ratio::new::<ratio>(0.0),
            Ratio::new::<ratio>(0.0),
            Ratio::new::<ratio>(0.0),
            Angle::new::<radian>((1_u64 << 54) as f64),
        )
        .expect("finite longitude is representable by the state type");

        assert!(matches!(
            EllipticKeplerPropagator::new().propagate_for_test(
                Orbit::new(
                    Epoch::from_tai_seconds(1_000.0),
                    SpacecraftState::from(state)
                ),
                &problem,
                Duration::from_seconds(1_000.0),
            ),
            Err(EllipticKeplerError::AccuracyBudgetExceeded { .. })
        ));
    }

    #[test]
    fn input_and_duration_phase_errors_share_one_budget() {
        let central_gravity = earth_gravity(earth_mu());
        let problem = problem(&central_gravity);
        let duration = Duration::from_seconds(100_000_000.0);
        let longitude = 1_000_000.0;
        let loose = EllipticKeplerPropagator::new()
            .with_phase_error_budget_radians(1.0)
            .expect("loose finite budget");
        let (_, input_error) = loose
            .normalize_input_angle(longitude)
            .expect("loose input reduction");
        let (_, duration_error) = loose
            .phase_advance(Length::new::<meter>(7_200_000.0), &problem, duration)
            .expect("loose duration phase");
        let combined = input_error + duration_error;
        let budget = (combined + input_error.max(duration_error)) / 2.0;
        assert!(input_error < budget && duration_error < budget && combined > budget);

        let state = EquinoctialState::new(
            InertialFrame::GCRF,
            central_gravity,
            Length::new::<meter>(7_200_000.0),
            Ratio::new::<ratio>(0.0),
            Ratio::new::<ratio>(0.0),
            Ratio::new::<ratio>(0.0),
            Ratio::new::<ratio>(0.0),
            Angle::new::<radian>(longitude),
        )
        .expect("finite equinoctial state");
        let propagator = EllipticKeplerPropagator::new()
            .with_phase_error_budget_radians(budget)
            .expect("derived positive budget");

        assert!(matches!(
            propagator.propagate_for_test(
                Orbit::new(Epoch::from_tai_seconds(1_000.0), SpacecraftState::from(state)),
                &problem,
                duration,
            ),
            Err(EllipticKeplerError::AccuracyBudgetExceeded {
                estimated_phase_error_radians,
                budget_radians,
            }) if estimated_phase_error_radians > budget_radians
        ));
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
        assert!(matches!(
            propagator.clone().with_phase_error_budget_radians(0.0),
            Err(EllipticKeplerError::InvalidPhaseErrorBudget)
        ));
        assert!(matches!(
            propagator.clone().with_phase_error_budget_radians(f64::NAN),
            Err(EllipticKeplerError::InvalidPhaseErrorBudget)
        ));
        let one_iteration = propagator
            .with_max_iterations(1)
            .expect("one iteration is valid");
        let central_gravity = earth_gravity(earth_mu());
        let problem = problem(&central_gravity);
        let difficult = initial(&central_gravity, 0.99, 0.2);
        assert!(matches!(
            one_iteration.propagate_for_test(difficult, &problem, Duration::from_seconds(4_000.0)),
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
            EllipticKeplerPropagator::new().propagate_for_test(
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
        let initial = Orbit::new(
            Epoch::from_tai_seconds(1_000.0),
            SpacecraftState::from(cartesian_state),
        );
        let mars_gravity = gravity(FrameOrigin::Body(Body::MARS), earth_mu());
        let problem = problem(&mars_gravity);

        assert!(matches!(
            EllipticKeplerPropagator::new().propagate_for_test(
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
