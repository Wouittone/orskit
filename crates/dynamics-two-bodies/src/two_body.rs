use std::f64::consts::{PI, TAU};
use std::sync::Arc;

use dynamics::{PropagationState, Propagator};
use gravity::SharedCentralGravity;
use hifitime::Duration;
use orskit_core::{Orbit, OrbitParts};
use thiserror::Error;
use units::uom::si::length::meter;
use units::{Length, Position, VelocityVector};

use orbits::{
    cartesian::{CartesianState, StateError},
    circular::CircularState,
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
/// The solution uses the universal anomaly and Stumpff functions to solve the
/// elliptic universal Kepler equation, then applies the Lagrange `f` and `g`
/// functions directly to Cartesian position and velocity. These relations
/// follow the public [NASA Technical Memorandum 2004-213230](https://ntrs.nasa.gov/citations/20040084254).
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
/// Cartesian state is regular for every non-collision orbital orientation,
/// including exactly retrograde planes. Caller-facing element states still
/// retain their own conversion-chart limits when the propagated Cartesian
/// result is restored.
///
/// The solver implements [`Propagator<TwoBodyDynamics, S>`] generically for
/// every `S` whose [`PropagationState<TwoBodyDynamics>`] resolves to
/// [`CartesianState`]. It advances the resolved Cartesian state with universal
/// variables and restores the caller-selected representation. The problem owns
/// the two-body topology and gravity provider, while this type owns only
/// analytical solution settings.
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
}

impl EllipticKeplerPropagator {
    fn advance_cartesian(
        &self,
        state: CartesianState,
        problem: &TwoBodyDynamics,
        duration: Duration,
    ) -> Result<CartesianState, EllipticKeplerError> {
        let position = state.position().to_metres();
        let velocity = state.velocity().to_metres_per_second();
        let radius = norm(position);
        if !radius.is_finite() || radius == 0.0 {
            return Err(EllipticKeplerError::CartesianCollisionSingularity);
        }

        let mu = problem
            .central_gravity()
            .parameter()
            .as_cubic_metres_per_second_squared();
        let reciprocal_semi_major_axis = 2.0 / radius - dot(velocity, velocity) / mu;
        if !reciprocal_semi_major_axis.is_finite() || reciprocal_semi_major_axis <= 0.0 {
            return Err(EllipticKeplerError::NonEllipticCartesianOrbit);
        }

        let semi_major_axis = 1.0 / reciprocal_semi_major_axis;
        let (mean_anomaly, _) =
            self.phase_advance(Length::new::<meter>(semi_major_axis), problem, duration)?;
        let mean_motion = (mu / semi_major_axis.powi(3)).sqrt();
        let reduced_duration = mean_anomaly / mean_motion;
        let sqrt_mu = mu.sqrt();
        let chi = solve_universal_kepler(
            sqrt_mu * reciprocal_semi_major_axis * reduced_duration,
            reciprocal_semi_major_axis,
            radius,
            dot(position, velocity) / sqrt_mu,
            sqrt_mu * reduced_duration,
            self.tolerance_radians / reciprocal_semi_major_axis.sqrt(),
            self.max_iterations,
        )?;
        let (stumpff_c, stumpff_s) = stumpff(reciprocal_semi_major_axis * chi * chi);
        let f = 1.0 - chi * chi * stumpff_c / radius;
        let g = reduced_duration - chi.powi(3) * stumpff_s / sqrt_mu;
        let propagated_position = add(scale(position, f), scale(velocity, g));
        let propagated_radius = norm(propagated_position);
        if !propagated_radius.is_finite() || propagated_radius == 0.0 {
            return Err(EllipticKeplerError::CartesianCollisionSingularity);
        }
        let f_dot = sqrt_mu * (reciprocal_semi_major_axis * chi.powi(3) * stumpff_s - chi)
            / (propagated_radius * radius);
        let g_dot = 1.0 - chi * chi * stumpff_c / propagated_radius;
        let propagated_velocity = add(scale(position, f_dot), scale(velocity, g_dot));
        Ok(CartesianState::new(
            state.frame(),
            Position::from_metres(
                propagated_position[0],
                propagated_position[1],
                propagated_position[2],
            ),
            VelocityVector::from_metres_per_second(
                propagated_velocity[0],
                propagated_velocity[1],
                propagated_velocity[2],
            ),
        )?)
    }
}

impl PropagationState<TwoBodyDynamics> for CartesianState {
    type Resolved = Self;
    type Error = EllipticKeplerError;

    fn validate(&self, problem: &TwoBodyDynamics) -> Result<(), Self::Error> {
        ensure_cartesian_problem_compatible(self, problem)?;
        validate_elliptic_cartesian(self, problem)
    }

    fn resolve(self, problem: &TwoBodyDynamics) -> Result<Self::Resolved, Self::Error> {
        self.validate(problem)?;
        Ok(self)
    }

    fn restore(resolved: Self::Resolved, _problem: &TwoBodyDynamics) -> Result<Self, Self::Error> {
        Ok(resolved)
    }
}

impl PropagationState<TwoBodyDynamics> for KeplerianState {
    type Resolved = CartesianState;
    type Error = EllipticKeplerError;

    fn validate(&self, problem: &TwoBodyDynamics) -> Result<(), Self::Error> {
        ensure_problem_gravity(self.central_gravity(), problem.central_gravity())?;
        CartesianState::try_from(self)?;
        Ok(())
    }

    fn resolve(self, problem: &TwoBodyDynamics) -> Result<Self::Resolved, Self::Error> {
        self.validate(problem)?;
        Ok(self.try_into()?)
    }

    fn restore(resolved: Self::Resolved, problem: &TwoBodyDynamics) -> Result<Self, Self::Error> {
        Ok(Self::try_from((
            resolved,
            Arc::clone(problem.central_gravity()),
        ))?)
    }
}

impl PropagationState<TwoBodyDynamics> for CircularState {
    type Resolved = CartesianState;
    type Error = EllipticKeplerError;

    fn validate(&self, problem: &TwoBodyDynamics) -> Result<(), Self::Error> {
        ensure_problem_gravity(self.central_gravity(), problem.central_gravity())
    }

    fn resolve(self, problem: &TwoBodyDynamics) -> Result<Self::Resolved, Self::Error> {
        self.validate(problem)?;
        Ok(self.try_into()?)
    }

    fn restore(resolved: Self::Resolved, problem: &TwoBodyDynamics) -> Result<Self, Self::Error> {
        let keplerian =
            KeplerianState::try_from((resolved, Arc::clone(problem.central_gravity())))?;
        Ok(Self::try_from(keplerian)?)
    }
}

impl PropagationState<TwoBodyDynamics> for EquinoctialState {
    type Resolved = CartesianState;
    type Error = EllipticKeplerError;

    fn validate(&self, problem: &TwoBodyDynamics) -> Result<(), Self::Error> {
        ensure_problem_gravity(self.central_gravity(), problem.central_gravity())
    }

    fn resolve(self, problem: &TwoBodyDynamics) -> Result<Self::Resolved, Self::Error> {
        self.validate(problem)?;
        Ok(self.try_into()?)
    }

    fn restore(resolved: Self::Resolved, problem: &TwoBodyDynamics) -> Result<Self, Self::Error> {
        Ok(Self::try_from((
            resolved,
            Arc::clone(problem.central_gravity()),
        ))?)
    }
}

impl<S> Propagator<TwoBodyDynamics, S> for EllipticKeplerPropagator
where
    S: PropagationState<TwoBodyDynamics, Resolved = CartesianState>,
    EllipticKeplerError: From<S::Error>,
{
    type Error = EllipticKeplerError;

    fn propagate_resolved(
        &self,
        initial: Orbit<CartesianState>,
        problem: &TwoBodyDynamics,
        target: hifitime::Epoch,
    ) -> Result<Orbit<CartesianState>, Self::Error> {
        let OrbitParts { epoch, state } = initial.into();
        let duration = target - epoch;
        let state = self.advance_cartesian(state, problem, duration)?;
        Ok(Orbit::new(target, state))
    }
}

fn ensure_cartesian_problem_compatible(
    state: &CartesianState,
    problem: &TwoBodyDynamics,
) -> Result<(), StateError> {
    let frame_origin = state.frame().origin();
    let gravity_origin = problem.central_gravity().origin();
    if frame_origin != gravity_origin {
        return Err(StateError::CentralGravityOriginMismatch {
            gravity_origin,
            frame_origin,
        });
    }
    Ok(())
}

fn validate_elliptic_cartesian(
    state: &CartesianState,
    problem: &TwoBodyDynamics,
) -> Result<(), EllipticKeplerError> {
    frames::InertialFrame::try_from(state.frame())
        .map_err(|_| StateError::CartesianFrameNotExplicitlyInertial)?;
    let radius = norm(state.position().to_metres());
    if !radius.is_finite() || radius == 0.0 {
        return Err(EllipticKeplerError::CartesianCollisionSingularity);
    }
    let velocity_squared = dot(
        state.velocity().to_metres_per_second(),
        state.velocity().to_metres_per_second(),
    );
    let mu = problem
        .central_gravity()
        .parameter()
        .as_cubic_metres_per_second_squared();
    let reciprocal_semi_major_axis = 2.0 / radius - velocity_squared / mu;
    if !reciprocal_semi_major_axis.is_finite() || reciprocal_semi_major_axis <= 0.0 {
        return Err(EllipticKeplerError::NonEllipticCartesianOrbit);
    }
    Ok(())
}

fn solve_universal_kepler(
    mut chi: f64,
    reciprocal_semi_major_axis: f64,
    radius: f64,
    radial_velocity_factor: f64,
    scaled_duration: f64,
    tolerance: f64,
    max_iterations: usize,
) -> Result<f64, EllipticKeplerError> {
    for _ in 0..max_iterations {
        let z = reciprocal_semi_major_axis * chi * chi;
        let (stumpff_c, stumpff_s) = stumpff(z);
        let residual = radial_velocity_factor * chi * chi * stumpff_c
            + (1.0 - reciprocal_semi_major_axis * radius) * chi.powi(3) * stumpff_s
            + radius * chi
            - scaled_duration;
        let derivative = radial_velocity_factor * chi * (1.0 - z * stumpff_s)
            + (1.0 - reciprocal_semi_major_axis * radius) * chi * chi * stumpff_c
            + radius;
        if !residual.is_finite() || !derivative.is_finite() || derivative == 0.0 {
            return Err(EllipticKeplerError::UniversalKeplerNonFinite);
        }
        let update = residual / derivative;
        chi -= update;
        if update.abs() <= tolerance {
            return Ok(chi);
        }
    }
    Err(EllipticKeplerError::DidNotConverge {
        iterations: max_iterations,
    })
}

fn stumpff(z: f64) -> (f64, f64) {
    if z.abs() < 1.0e-8 {
        let z_squared = z * z;
        (
            0.5 - z / 24.0 + z_squared / 720.0 - z_squared * z / 40_320.0,
            1.0 / 6.0 - z / 120.0 + z_squared / 5_040.0 - z_squared * z / 362_880.0,
        )
    } else {
        let root_z = z.sqrt();
        (
            (1.0 - root_z.cos()) / z,
            (root_z - root_z.sin()) / (root_z * z),
        )
    }
}

fn dot(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0].mul_add(right[0], left[1].mul_add(right[1], left[2] * right[2]))
}

fn norm(vector: [f64; 3]) -> f64 {
    dot(vector, vector).sqrt()
}

fn scale(vector: [f64; 3], factor: f64) -> [f64; 3] {
    vector.map(|component| component * factor)
}

fn add(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|index| left[index] + right[index])
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
    /// The Cartesian state is at the point-mass collision singularity.
    #[error("Cartesian two-body propagation is singular at zero radius")]
    CartesianCollisionSingularity,
    /// Cartesian state energy does not describe a bound elliptic orbit.
    #[error("Cartesian state must describe a bound elliptic orbit")]
    NonEllipticCartesianOrbit,
    /// Universal-variable Kepler evaluation became non-finite.
    #[error("universal-variable Kepler evaluation became non-finite")]
    UniversalKeplerNonFinite,
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
    use units::uom::si::{angle::radian, length::meter, ratio::ratio};
    use units::{Angle, GravitationalParameter, Length, Position, Ratio, VelocityVector};

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
        GravitationalParameter::try_from(3.986_004_418e14)
            .expect("Earth gravitational parameter is positive")
    }

    fn lox_earth_mu() -> GravitationalParameter {
        GravitationalParameter::try_from(3.986_004_355_070_227e14)
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
        Circular(CircularState),
        Keplerian(KeplerianState),
        Equinoctial(EquinoctialState),
    }

    impl SpacecraftStateContract for SpacecraftState {
        fn frame(&self) -> frames::ReferenceFrame {
            match self {
                Self::Cartesian(state) => state.frame(),
                Self::Circular(state) => state.frame(),
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
    impl From<CircularState> for SpacecraftState {
        fn from(value: CircularState) -> Self {
            Self::Circular(value)
        }
    }
    impl From<EquinoctialState> for SpacecraftState {
        fn from(value: EquinoctialState) -> Self {
            Self::Equinoctial(value)
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    struct CartesianWrapper(CartesianState);

    impl SpacecraftStateContract for CartesianWrapper {
        fn frame(&self) -> frames::ReferenceFrame {
            self.0.frame()
        }
    }

    impl PropagationState<TwoBodyDynamics> for CartesianWrapper {
        type Resolved = CartesianState;
        type Error = EllipticKeplerError;

        fn validate(&self, problem: &TwoBodyDynamics) -> Result<(), Self::Error> {
            ensure_cartesian_problem_compatible(&self.0, problem)?;
            validate_elliptic_cartesian(&self.0, problem)
        }

        fn resolve(self, problem: &TwoBodyDynamics) -> Result<Self::Resolved, Self::Error> {
            self.validate(problem)?;
            Ok(self.0)
        }

        fn restore(
            resolved: Self::Resolved,
            _problem: &TwoBodyDynamics,
        ) -> Result<Self, Self::Error> {
            Ok(Self(resolved))
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
            let OrbitParts { epoch, state } = initial.into();
            match state {
                SpacecraftState::Cartesian(state) => {
                    <Self as Propagator<TwoBodyDynamics, CartesianState>>::propagate(
                        self,
                        Orbit::new(epoch, state),
                        problem,
                        target,
                    )
                    .map(|orbit| {
                        let OrbitParts { epoch, state } = orbit.into();
                        Orbit::new(epoch, SpacecraftState::Cartesian(state))
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
                        let OrbitParts { epoch, state } = orbit.into();
                        Orbit::new(epoch, SpacecraftState::Keplerian(state))
                    })
                }
                SpacecraftState::Circular(state) => {
                    <Self as Propagator<TwoBodyDynamics, CircularState>>::propagate(
                        self,
                        Orbit::new(epoch, state),
                        problem,
                        target,
                    )
                    .map(|orbit| {
                        let OrbitParts { epoch, state } = orbit.into();
                        Orbit::new(epoch, SpacecraftState::Circular(state))
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
                        let OrbitParts { epoch, state } = orbit.into();
                        Orbit::new(epoch, SpacecraftState::Equinoctial(state))
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

    fn cartesian(state: SpacecraftState) -> CartesianState {
        match state {
            SpacecraftState::Cartesian(state) => state,
            SpacecraftState::Circular(state) => state.try_into().expect("conversion"),
            SpacecraftState::Keplerian(state) => state.try_into().expect("conversion"),
            SpacecraftState::Equinoctial(state) => state.try_into().expect("conversion"),
        }
    }

    #[test]
    fn generic_propagator_accepts_an_application_state_resolving_to_cartesian() {
        let central_gravity = earth_gravity(earth_mu());
        let problem = problem(&central_gravity);
        let state: CartesianState = orbit(&central_gravity, 0.1, 2.0)
            .try_into()
            .expect("fixture conversion");

        let propagated = EllipticKeplerPropagator::new()
            .propagate(
                Orbit::new(Epoch::from_tai_seconds(1_000.0), CartesianWrapper(state)),
                &problem,
                Epoch::from_tai_seconds(1_900.0),
            )
            .expect("generic application state propagates");

        assert_eq!(propagated.epoch(), Epoch::from_tai_seconds(1_900.0));
        assert!(propagated.as_ref().0.position().is_finite());
        assert!(propagated.as_ref().0.velocity().is_finite());
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
    ) -> [CartesianState; 4] {
        let central_gravity = earth_gravity(mu);
        let problem = problem(&central_gravity);
        let propagator = EllipticKeplerPropagator::new();
        let keplerian = initial(&central_gravity, 0.1, 2.0);
        let state = match keplerian.as_ref() {
            SpacecraftState::Keplerian(state) => state,
            _ => unreachable!(),
        };
        let equinoctial = Orbit::new(
            keplerian.epoch(),
            EquinoctialState::try_from(state.clone())
                .expect("conversion")
                .into(),
        );
        let circular = Orbit::new(
            keplerian.epoch(),
            CircularState::try_from(state.clone())
                .expect("conversion")
                .into(),
        );
        let cartesian_orbit = Orbit::new(
            keplerian.epoch(),
            CartesianState::try_from(state).expect("conversion").into(),
        );

        [keplerian, circular, equinoctial, cartesian_orbit].map(|initial| {
            let propagated = propagator
                .propagate_for_test(initial, &problem, duration)
                .expect("propagation converges");
            cartesian(propagated.as_ref().clone())
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
        let initial_orbit = match initial.as_ref() {
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
        let propagated_cartesian = cartesian(propagated.as_ref().clone());
        let initial_cartesian = cartesian(initial.as_ref().clone());

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
        let (before, after) = match (initial.as_ref(), propagated.as_ref()) {
            (SpacecraftState::Keplerian(before), SpacecraftState::Keplerian(after)) => {
                (before, after)
            }
            _ => panic!("propagator must preserve state variant"),
        };

        assert!(
            (after.semi_major_axis().get::<meter>() - before.semi_major_axis().get::<meter>())
                .abs()
                <= 1.0e-6
        );
        assert!(
            (after.eccentricity().get::<ratio>() - before.eccentricity().get::<ratio>()).abs()
                <= 1.0e-14
        );
        assert!(
            (after.inclination().get::<radian>() - before.inclination().get::<radian>()).abs()
                <= 1.0e-14
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
        let initial_cartesian = cartesian(initial.as_ref().clone());
        let recovered_cartesian = cartesian(recovered.as_ref().clone());

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
        assert_eq!(by_epoch.as_ref(), by_duration.as_ref());
    }

    #[test]
    fn zero_duration_is_exact_identity_for_every_representation() {
        let central_gravity = earth_gravity(earth_mu());
        let problem = problem(&central_gravity);
        let keplerian = initial(&central_gravity, 0.2, 1.3);
        let elements = match keplerian.as_ref() {
            SpacecraftState::Keplerian(state) => state,
            _ => unreachable!(),
        };
        let equinoctial = Orbit::new(
            keplerian.epoch(),
            EquinoctialState::try_from(elements.clone())
                .expect("fixture conversion")
                .into(),
        );
        let circular = Orbit::new(
            keplerian.epoch(),
            CircularState::try_from(elements.clone())
                .expect("fixture conversion")
                .into(),
        );
        let cartesian = Orbit::new(
            keplerian.epoch(),
            CartesianState::try_from(elements)
                .expect("fixture conversion")
                .into(),
        );
        let propagator = EllipticKeplerPropagator::new();

        for initial in [keplerian, circular, equinoctial, cartesian] {
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
        let anomaly = |orbit: Orbit<SpacecraftState>| match orbit.as_ref() {
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
    fn cartesian_universal_propagation_supports_an_exactly_retrograde_plane() {
        let central_gravity = earth_gravity(earth_mu());
        let problem = problem(&central_gravity);
        let retrograde = KeplerianState::new(
            InertialFrame::GCRF,
            central_gravity,
            Length::new::<meter>(7_200_000.0),
            Ratio::new::<ratio>(0.1),
            Angle::new::<radian>(PI),
            Angle::new::<radian>(0.7),
            Angle::new::<radian>(0.4),
            Angle::new::<radian>(2.0),
        )
        .expect("retrograde Keplerian state");
        let state: CartesianState = retrograde.try_into().expect("Cartesian conversion");
        let propagated = EllipticKeplerPropagator::new()
            .propagate(
                Orbit::new(Epoch::from_tai_seconds(1_000.0), state),
                &problem,
                Epoch::from_tai_seconds(1_060.0),
            )
            .expect("universal Cartesian propagation");
        assert!(propagated.as_ref().position().is_finite());
        assert!(propagated.as_ref().velocity().is_finite());
    }

    #[test]
    fn universal_cartesian_kernel_rejects_collision_and_non_elliptic_states() {
        let central_gravity = earth_gravity(earth_mu());
        let problem = problem(&central_gravity);
        let epoch = Epoch::from_tai_seconds(1_000.0);
        let target = Epoch::from_tai_seconds(1_060.0);
        let collision = CartesianState::new(
            frames::ReferenceFrame::GCRF,
            Position::from_metres(0.0, 0.0, 0.0),
            VelocityVector::from_metres_per_second(0.0, 1.0, 0.0),
        )
        .expect("finite Cartesian collision state");
        let escape = CartesianState::new(
            frames::ReferenceFrame::GCRF,
            Position::from_metres(7_000_000.0, 0.0, 0.0),
            VelocityVector::from_metres_per_second(0.0, 12_000.0, 0.0),
        )
        .expect("finite Cartesian escape state");
        let propagator = EllipticKeplerPropagator::new();

        assert!(matches!(
            propagator.propagate(Orbit::new(epoch, collision), &problem, target),
            Err(EllipticKeplerError::CartesianCollisionSingularity)
        ));
        assert!(matches!(
            propagator.propagate(Orbit::new(epoch, escape), &problem, target),
            Err(EllipticKeplerError::NonEllipticCartesianOrbit)
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
        let cartesian_state = CartesianState::try_from(earth_elements).expect("fixture conversion");
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
