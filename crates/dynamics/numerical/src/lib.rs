#![forbid(unsafe_code)]

//! Adaptive numerical propagation of frame- and epoch-qualified Cartesian states.
//!
//! This first numerical slice implements the Bogacki--Shampine 3(2) embedded
//! Runge--Kutta pair for non-stiff translational dynamics. The propagated
//! solution is third order and the second-order companion supplies a local
//! error estimate. Accepted steps also construct a cubic Hermite continuous
//! extension from their endpoint states and derivatives; public ephemerides
//! and event handling remain outside this crate.
//!
//! The pair and its coefficients are from P. Bogacki and L. F. Shampine,
//! ["A 3(2) pair of Runge--Kutta formulas"](https://doi.org/10.1016/0893-9659(89)90079-7),
//! *Applied Mathematics Letters* 2(4), 1989, pp. 321--325. The endpoint-value
//! and endpoint-slope cubic Hermite extension is documented for this pair by
//! L. F. Shampine, I. Gladwell, and S. Thompson,
//! [*Solving ODEs with MATLAB*](https://doi.org/10.1017/CBO9780511615542)
//! (Cambridge University Press, 2003), section 1.2.
//!
//! Run the complete typed point-mass example with:
//!
//! ```text
//! cargo run -p dynamics-numerical --example numerical_two_body
//! ```

use std::error::Error;

pub use dynamics::CartesianDynamics;
use dynamics::Propagator;
use frames::ReferenceFrame;
use hifitime::{Duration, Epoch};
use orbits::cartesian::CartesianState;
use orskit_core::{Orbit, OrbitParts};
use thiserror::Error;
use units::uom::si::{length::meter, ratio::ratio, velocity::meter_per_second};
use units::{Length, Position, Ratio, Velocity, VelocityVector};

const COMPONENT_COUNT: usize = 6;
const SAFETY_FACTOR: f64 = 0.9;
const MINIMUM_STEP_FACTOR: f64 = 0.2;
const MAXIMUM_STEP_FACTOR: f64 = 5.0;
const ERROR_ESTIMATOR_ORDER_PLUS_ONE: f64 = 3.0;

/// Typed local-error and step-control settings for Cartesian propagation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IntegrationConfiguration {
    position_absolute_tolerance: Length,
    velocity_absolute_tolerance: Velocity,
    relative_tolerance: Ratio,
    minimum_step: Duration,
    maximum_step: Duration,
    initial_step: Duration,
    max_steps: usize,
    max_rejections: usize,
}

impl IntegrationConfiguration {
    /// Builds an explicit integration configuration.
    ///
    /// Absolute tolerances scale the three position and three velocity
    /// components independently. The dimensionless relative term uses
    /// `absolute + relative * max(|initial|, |candidate|)`. The RMS of the six
    /// scaled embedded differences controls acceptance. Step durations are
    /// positive magnitudes and are signed internally for backward propagation.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        position_absolute_tolerance: Length,
        velocity_absolute_tolerance: Velocity,
        relative_tolerance: Ratio,
        minimum_step: Duration,
        maximum_step: Duration,
        initial_step: Duration,
        max_steps: usize,
        max_rejections: usize,
    ) -> Result<Self, IntegrationConfigurationError> {
        let position = position_absolute_tolerance.get::<meter>();
        let velocity = velocity_absolute_tolerance.get::<meter_per_second>();
        let relative = relative_tolerance.get::<ratio>();
        if !position.is_finite() || position <= 0.0 {
            return Err(IntegrationConfigurationError::InvalidPositionTolerance);
        }
        if !velocity.is_finite() || velocity <= 0.0 {
            return Err(IntegrationConfigurationError::InvalidVelocityTolerance);
        }
        if !relative.is_finite() || relative <= 0.0 {
            return Err(IntegrationConfigurationError::InvalidRelativeTolerance);
        }

        let minimum_seconds = positive_duration_seconds(
            minimum_step,
            IntegrationConfigurationError::InvalidMinimumStep,
        )?;
        let maximum_seconds = positive_duration_seconds(
            maximum_step,
            IntegrationConfigurationError::InvalidMaximumStep,
        )?;
        let initial_seconds = positive_duration_seconds(
            initial_step,
            IntegrationConfigurationError::InvalidInitialStep,
        )?;
        if minimum_seconds > maximum_seconds {
            return Err(IntegrationConfigurationError::InvertedStepBounds);
        }
        if !(minimum_seconds..=maximum_seconds).contains(&initial_seconds) {
            return Err(IntegrationConfigurationError::InitialStepOutsideBounds);
        }
        if max_steps == 0 {
            return Err(IntegrationConfigurationError::ZeroStepLimit);
        }
        if max_rejections == 0 {
            return Err(IntegrationConfigurationError::ZeroRejectionLimit);
        }

        Ok(Self {
            position_absolute_tolerance,
            velocity_absolute_tolerance,
            relative_tolerance,
            minimum_step,
            maximum_step,
            initial_step,
            max_steps,
            max_rejections,
        })
    }

    /// Returns the absolute tolerance applied to each position component.
    #[must_use]
    pub const fn position_absolute_tolerance(self) -> Length {
        self.position_absolute_tolerance
    }

    /// Returns the absolute tolerance applied to each velocity component.
    #[must_use]
    pub const fn velocity_absolute_tolerance(self) -> Velocity {
        self.velocity_absolute_tolerance
    }

    /// Returns the dimensionless relative tolerance.
    #[must_use]
    pub const fn relative_tolerance(self) -> Ratio {
        self.relative_tolerance
    }

    /// Returns the positive minimum ordinary step magnitude.
    #[must_use]
    pub const fn minimum_step(self) -> Duration {
        self.minimum_step
    }

    /// Returns the positive maximum step magnitude.
    #[must_use]
    pub const fn maximum_step(self) -> Duration {
        self.maximum_step
    }

    /// Returns the positive initial step magnitude.
    #[must_use]
    pub const fn initial_step(self) -> Duration {
        self.initial_step
    }

    /// Returns the maximum number of attempted steps.
    #[must_use]
    pub const fn max_steps(self) -> usize {
        self.max_steps
    }

    /// Returns the maximum number of rejected steps.
    #[must_use]
    pub const fn max_rejections(self) -> usize {
        self.max_rejections
    }
}

fn positive_duration_seconds(
    duration: Duration,
    error: IntegrationConfigurationError,
) -> Result<f64, IntegrationConfigurationError> {
    let seconds = duration.to_seconds();
    if !seconds.is_finite() || seconds <= 0.0 {
        return Err(error);
    }
    Ok(seconds)
}

/// Invalid numerical integration configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum IntegrationConfigurationError {
    /// Position absolute tolerance is not positive and finite.
    #[error("position absolute tolerance must be positive and finite")]
    InvalidPositionTolerance,
    /// Velocity absolute tolerance is not positive and finite.
    #[error("velocity absolute tolerance must be positive and finite")]
    InvalidVelocityTolerance,
    /// Relative tolerance is not positive and finite.
    #[error("relative tolerance must be positive and finite")]
    InvalidRelativeTolerance,
    /// Minimum step magnitude is not positive and finite.
    #[error("minimum step must be a positive finite duration")]
    InvalidMinimumStep,
    /// Maximum step magnitude is not positive and finite.
    #[error("maximum step must be a positive finite duration")]
    InvalidMaximumStep,
    /// Initial step magnitude is not positive and finite.
    #[error("initial step must be a positive finite duration")]
    InvalidInitialStep,
    /// Minimum step exceeds maximum step.
    #[error("minimum step must not exceed maximum step")]
    InvertedStepBounds,
    /// Initial step is outside the inclusive step bounds.
    #[error("initial step must lie within the inclusive step bounds")]
    InitialStepOutsideBounds,
    /// Attempt limit is zero.
    #[error("maximum attempted steps must be non-zero")]
    ZeroStepLimit,
    /// Rejection limit is zero.
    #[error("maximum rejected steps must be non-zero")]
    ZeroRejectionLimit,
}

/// Adaptive Bogacki--Shampine 3(2) Cartesian propagator.
///
/// This propagator owns one immutable, evaluable physical problem and explicit
/// local-error settings. It supports forward and backward propagation and
/// never steps across the requested epoch. It is intended for smooth,
/// non-stiff dynamics; configured local tolerances do not bound accumulated
/// global error or physical model error.
#[derive(Debug, Clone)]
pub struct BogackiShampine32<P> {
    problem: P,
    configuration: IntegrationConfiguration,
}

impl<P> BogackiShampine32<P> {
    /// Selects an evaluable problem and validated numerical configuration.
    #[must_use]
    pub const fn new(problem: P, configuration: IntegrationConfiguration) -> Self {
        Self {
            problem,
            configuration,
        }
    }

    /// Returns the owned evaluable problem.
    #[must_use]
    pub const fn problem(&self) -> &P {
        &self.problem
    }

    /// Returns the local-error and step-control configuration.
    #[must_use]
    pub const fn configuration(&self) -> IntegrationConfiguration {
        self.configuration
    }
}

impl<P> Propagator<CartesianState> for BogackiShampine32<P>
where
    P: CartesianDynamics,
{
    type Error = NumericalPropagationError<P::Error>;

    fn propagate(
        &self,
        initial: Orbit<CartesianState>,
        target: Epoch,
    ) -> Result<Orbit<CartesianState>, Self::Error> {
        let OrbitParts { epoch, state } = initial.into();
        self.problem
            .validate(&state)
            .map_err(NumericalPropagationError::Dynamics)?;
        if target == epoch {
            return Ok(Orbit::new(epoch, state));
        }

        let (state, _) = self.integrate(epoch, state, target)?;
        Ok(Orbit::new(target, state))
    }
}

impl<P> BogackiShampine32<P>
where
    P: CartesianDynamics,
{
    fn integrate(
        &self,
        initial_epoch: Epoch,
        initial_state: CartesianState,
        target: Epoch,
    ) -> Result<(CartesianState, IntegrationStatistics), NumericalPropagationError<P::Error>> {
        let total_seconds = (target - initial_epoch).to_seconds();
        if !total_seconds.is_finite() {
            return Err(NumericalPropagationError::NonFiniteDuration);
        }
        let direction = total_seconds.signum();
        let total_magnitude = total_seconds.abs();
        let minimum_step = self.configuration.minimum_step.to_seconds();
        let maximum_step = self.configuration.maximum_step.to_seconds();
        let mut step_magnitude = self
            .configuration
            .initial_step
            .to_seconds()
            .min(total_magnitude);
        let mut elapsed = 0.0;
        let mut values = state_to_array(initial_state);
        let frame = initial_state.frame();
        let mut statistics = IntegrationStatistics::default();

        while elapsed < total_magnitude {
            if statistics.attempted_steps >= self.configuration.max_steps {
                return Err(NumericalPropagationError::StepLimitExceeded {
                    attempted: statistics.attempted_steps,
                });
            }
            let remaining = total_magnitude - elapsed;
            let proposed_magnitude = step_magnitude.min(remaining);
            if proposed_magnitude == 0.0 || !proposed_magnitude.is_finite() {
                return Err(NumericalPropagationError::StepUnderflow);
            }
            let signed_step = direction * proposed_magnitude;
            let signed_elapsed = direction * elapsed;
            let step = self.step(initial_epoch, signed_elapsed, signed_step, frame, values)?;
            statistics.attempted_steps += 1;

            let error_norm =
                scaled_rms_error(values, step.candidate, step.error, self.configuration);
            if !error_norm.is_finite() {
                return Err(NumericalPropagationError::NonFiniteErrorEstimate);
            }
            let factor = step_factor(error_norm);
            if error_norm <= 1.0 {
                let dense = DenseStep::new(values, step.candidate, step.k1, step.k4, signed_step);
                debug_assert!(dense.endpoint_error() <= 32.0 * f64::EPSILON);
                values = step.candidate;
                elapsed = if proposed_magnitude == remaining {
                    total_magnitude
                } else {
                    elapsed + proposed_magnitude
                };
                statistics.accepted_steps += 1;
                step_magnitude = (proposed_magnitude * factor).clamp(minimum_step, maximum_step);
            } else {
                statistics.rejected_steps += 1;
                if statistics.rejected_steps > self.configuration.max_rejections {
                    return Err(NumericalPropagationError::RejectionLimitExceeded {
                        rejected: statistics.rejected_steps,
                    });
                }
                let reduced = proposed_magnitude * factor.min(1.0);
                if reduced < minimum_step {
                    return Err(NumericalPropagationError::MinimumStepExhausted {
                        required_seconds: reduced,
                        minimum_seconds: minimum_step,
                    });
                }
                step_magnitude = reduced;
            }
        }

        Ok((array_to_state(frame, values)?, statistics))
    }

    fn step(
        &self,
        initial_epoch: Epoch,
        elapsed_seconds: f64,
        step_seconds: f64,
        frame: ReferenceFrame,
        y: [f64; COMPONENT_COUNT],
    ) -> Result<EmbeddedStep, NumericalPropagationError<P::Error>> {
        // Bogacki--Shampine 3(2), advanced with the third-order solution.
        let k1 = self.derivative(initial_epoch, elapsed_seconds, frame, y)?;
        let k2_state = combine(y, step_seconds, &[(0.5, k1)]);
        let k2 = self.derivative(
            initial_epoch,
            elapsed_seconds + 0.5 * step_seconds,
            frame,
            k2_state,
        )?;
        let k3_state = combine(y, step_seconds, &[(0.75, k2)]);
        let k3 = self.derivative(
            initial_epoch,
            elapsed_seconds + 0.75 * step_seconds,
            frame,
            k3_state,
        )?;
        let candidate = combine(
            y,
            step_seconds,
            &[(2.0 / 9.0, k1), (1.0 / 3.0, k2), (4.0 / 9.0, k3)],
        );
        let k4 = self.derivative(
            initial_epoch,
            elapsed_seconds + step_seconds,
            frame,
            candidate,
        )?;
        let embedded = combine(
            y,
            step_seconds,
            &[(7.0 / 24.0, k1), (0.25, k2), (1.0 / 3.0, k3), (0.125, k4)],
        );
        let error = std::array::from_fn(|index| candidate[index] - embedded[index]);
        if candidate
            .into_iter()
            .chain(error)
            .any(|value| !value.is_finite())
        {
            return Err(NumericalPropagationError::NonFiniteState);
        }
        Ok(EmbeddedStep {
            candidate,
            error,
            k1,
            k4,
        })
    }

    fn derivative(
        &self,
        initial_epoch: Epoch,
        elapsed_seconds: f64,
        frame: ReferenceFrame,
        values: [f64; COMPONENT_COUNT],
    ) -> Result<[f64; COMPONENT_COUNT], NumericalPropagationError<P::Error>> {
        let state = array_to_state(frame, values)?;
        let epoch = initial_epoch + Duration::from_seconds(elapsed_seconds);
        let acceleration = self
            .problem
            .acceleration(epoch, &state)
            .map_err(NumericalPropagationError::Dynamics)?;
        if acceleration.frame() != frame {
            return Err(NumericalPropagationError::AccelerationFrameMismatch {
                state_frame: Box::new(frame),
                acceleration_frame: Box::new(acceleration.frame()),
            });
        }
        let [ax, ay, az] = acceleration.value().to_metres_per_second_squared();
        let derivative = [values[3], values[4], values[5], ax, ay, az];
        if derivative.into_iter().any(|value| !value.is_finite()) {
            return Err(NumericalPropagationError::NonFiniteDerivative);
        }
        Ok(derivative)
    }
}

fn state_to_array(state: CartesianState) -> [f64; COMPONENT_COUNT] {
    let [x, y, z] = state.position().to_metres();
    let [vx, vy, vz] = state.velocity().to_metres_per_second();
    [x, y, z, vx, vy, vz]
}

fn array_to_state<E>(
    frame: ReferenceFrame,
    values: [f64; COMPONENT_COUNT],
) -> Result<CartesianState, NumericalPropagationError<E>>
where
    E: Error + Send + Sync + 'static,
{
    CartesianState::new(
        frame,
        Position::from_metres(values[0], values[1], values[2]),
        VelocityVector::from_metres_per_second(values[3], values[4], values[5]),
    )
    .map_err(|_| NumericalPropagationError::NonFiniteState)
}

fn combine(
    initial: [f64; COMPONENT_COUNT],
    step: f64,
    terms: &[(f64, [f64; COMPONENT_COUNT])],
) -> [f64; COMPONENT_COUNT] {
    std::array::from_fn(|component| {
        terms
            .iter()
            .fold(initial[component], |value, (weight, derivative)| {
                (step * weight).mul_add(derivative[component], value)
            })
    })
}

fn scaled_rms_error(
    initial: [f64; COMPONENT_COUNT],
    candidate: [f64; COMPONENT_COUNT],
    error: [f64; COMPONENT_COUNT],
    configuration: IntegrationConfiguration,
) -> f64 {
    let position_absolute = configuration.position_absolute_tolerance.get::<meter>();
    let velocity_absolute = configuration
        .velocity_absolute_tolerance
        .get::<meter_per_second>();
    let relative = configuration.relative_tolerance.get::<ratio>();
    let sum = (0..COMPONENT_COUNT).fold(0.0, |sum, index| {
        let absolute = if index < 3 {
            position_absolute
        } else {
            velocity_absolute
        };
        let scale = absolute + relative * initial[index].abs().max(candidate[index].abs());
        (error[index] / scale).mul_add(error[index] / scale, sum)
    });
    (sum / COMPONENT_COUNT as f64).sqrt()
}

fn step_factor(error_norm: f64) -> f64 {
    if error_norm == 0.0 {
        return MAXIMUM_STEP_FACTOR;
    }
    (SAFETY_FACTOR * error_norm.powf(-1.0 / ERROR_ESTIMATOR_ORDER_PLUS_ONE))
        .clamp(MINIMUM_STEP_FACTOR, MAXIMUM_STEP_FACTOR)
}

#[derive(Debug, Clone, Copy)]
struct EmbeddedStep {
    candidate: [f64; COMPONENT_COUNT],
    error: [f64; COMPONENT_COUNT],
    k1: [f64; COMPONENT_COUNT],
    k4: [f64; COMPONENT_COUNT],
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct IntegrationStatistics {
    attempted_steps: usize,
    accepted_steps: usize,
    rejected_steps: usize,
}

/// One accepted-step cubic Hermite continuous extension.
#[derive(Debug, Clone, Copy)]
struct DenseStep {
    start: [f64; COMPONENT_COUNT],
    end: [f64; COMPONENT_COUNT],
    start_derivative: [f64; COMPONENT_COUNT],
    end_derivative: [f64; COMPONENT_COUNT],
    step_seconds: f64,
}

impl DenseStep {
    fn new(
        start: [f64; COMPONENT_COUNT],
        end: [f64; COMPONENT_COUNT],
        start_derivative: [f64; COMPONENT_COUNT],
        end_derivative: [f64; COMPONENT_COUNT],
        step_seconds: f64,
    ) -> Self {
        Self {
            start,
            end,
            start_derivative,
            end_derivative,
            step_seconds,
        }
    }

    fn evaluate(self, fraction: f64) -> [f64; COMPONENT_COUNT] {
        debug_assert!((0.0..=1.0).contains(&fraction));
        let squared = fraction * fraction;
        let cubed = squared * fraction;
        let h00 = 2.0 * cubed - 3.0 * squared + 1.0;
        let h10 = cubed - 2.0 * squared + fraction;
        let h01 = -2.0 * cubed + 3.0 * squared;
        let h11 = cubed - squared;
        std::array::from_fn(|index| {
            h00 * self.start[index]
                + h10 * self.step_seconds * self.start_derivative[index]
                + h01 * self.end[index]
                + h11 * self.step_seconds * self.end_derivative[index]
        })
    }

    fn endpoint_error(self) -> f64 {
        self.evaluate(0.0)
            .into_iter()
            .zip(self.start)
            .chain(self.evaluate(1.0).into_iter().zip(self.end))
            .map(|(actual, expected)| {
                let scale = expected.abs().max(1.0);
                (actual - expected).abs() / scale
            })
            .fold(0.0, f64::max)
    }
}

/// Adaptive propagation failure.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum NumericalPropagationError<E>
where
    E: Error + Send + Sync + 'static,
{
    /// The evaluable physical problem or one of its providers failed.
    #[error("Cartesian dynamics evaluation failed")]
    Dynamics(#[source] E),
    /// Target-minus-initial duration cannot be represented as a finite kernel interval.
    #[error("propagation duration is not finite")]
    NonFiniteDuration,
    /// A stage state became NaN or infinite.
    #[error("numerical stage produced a non-finite Cartesian state")]
    NonFiniteState,
    /// A stage derivative became NaN or infinite.
    #[error("dynamics evaluation produced a non-finite derivative")]
    NonFiniteDerivative,
    /// The embedded local-error estimate became NaN or infinite.
    #[error("embedded local-error estimate is not finite")]
    NonFiniteErrorEstimate,
    /// An evaluator returned acceleration in a different frame.
    #[error("acceleration frame {acceleration_frame} does not match state frame {state_frame}")]
    AccelerationFrameMismatch {
        /// Frame carried by the stage state.
        state_frame: Box<ReferenceFrame>,
        /// Frame carried by the returned acceleration.
        acceleration_frame: Box<ReferenceFrame>,
    },
    /// Floating-point stepping cannot make progress.
    #[error("step size underflow prevents progress toward the target epoch")]
    StepUnderflow,
    /// Error control requires a step below the configured minimum.
    #[error(
        "error control requires {required_seconds} s, below configured minimum {minimum_seconds} s"
    )]
    MinimumStepExhausted {
        /// Proposed reduced step magnitude in seconds.
        required_seconds: f64,
        /// Configured minimum step magnitude in seconds.
        minimum_seconds: f64,
    },
    /// Attempted-step limit was exhausted.
    #[error("maximum attempted-step count exhausted after {attempted} attempts")]
    StepLimitExceeded {
        /// Number of attempted steps.
        attempted: usize,
    },
    /// Rejected-step limit was exhausted.
    #[error("maximum rejected-step count exhausted after {rejected} rejections")]
    RejectionLimitExceeded {
        /// Number of rejected steps.
        rejected: usize,
    },
}

#[cfg(test)]
mod tests {
    use std::{convert::Infallible, sync::Arc};

    use dynamics_two_bodies::{
        EllipticKeplerPropagator, PointMassGravityModel, TwoBodyDynamics, TwoBodyEvaluationError,
    };
    use frames::{Body, FrameOrientation, FrameOrigin, InertialFrame};
    use gravity::{PointMass, SharedCentralGravity};
    use orbits::{cartesian::FramedAcceleration, keplerian::KeplerianState};
    use units::uom::si::{acceleration::meter_per_second_squared, angle::radian};
    use units::{Acceleration, AccelerationVector, Angle, GravitationalParameter};

    use super::*;

    #[derive(Debug, Clone, Copy)]
    struct ConstantAcceleration {
        value: AccelerationVector,
    }

    impl CartesianDynamics for ConstantAcceleration {
        type Error = Infallible;

        fn validate(&self, _state: &CartesianState) -> Result<(), Self::Error> {
            Ok(())
        }

        fn acceleration(
            &self,
            _epoch: Epoch,
            state: &CartesianState,
        ) -> Result<FramedAcceleration, Self::Error> {
            Ok(FramedAcceleration::new(self.value, state.frame())
                .expect("test acceleration is finite"))
        }
    }

    #[derive(Debug, Clone, Copy)]
    struct HarmonicOscillator;

    impl CartesianDynamics for HarmonicOscillator {
        type Error = Infallible;

        fn validate(&self, _state: &CartesianState) -> Result<(), Self::Error> {
            Ok(())
        }

        fn acceleration(
            &self,
            _epoch: Epoch,
            state: &CartesianState,
        ) -> Result<FramedAcceleration, Self::Error> {
            let [x, y, z] = state.position().to_metres();
            Ok(FramedAcceleration::new(
                AccelerationVector::from_metres_per_second_squared(-x, -y, -z),
                state.frame(),
            )
            .expect("finite harmonic acceleration"))
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
    #[error("fixture model failure")]
    struct FixtureModelError;

    #[derive(Debug)]
    struct FailingDynamics;

    impl CartesianDynamics for FailingDynamics {
        type Error = FixtureModelError;

        fn validate(&self, _state: &CartesianState) -> Result<(), Self::Error> {
            Ok(())
        }

        fn acceleration(
            &self,
            _epoch: Epoch,
            _state: &CartesianState,
        ) -> Result<FramedAcceleration, Self::Error> {
            Err(FixtureModelError)
        }
    }

    #[derive(Debug)]
    struct WrongFrameDynamics;

    impl CartesianDynamics for WrongFrameDynamics {
        type Error = Infallible;

        fn validate(&self, _state: &CartesianState) -> Result<(), Self::Error> {
            Ok(())
        }

        fn acceleration(
            &self,
            _epoch: Epoch,
            _state: &CartesianState,
        ) -> Result<FramedAcceleration, Self::Error> {
            Ok(FramedAcceleration::new(
                AccelerationVector::from_metres_per_second_squared(0.0, 0.0, 0.0),
                ReferenceFrame::EME2000,
            )
            .expect("finite acceleration"))
        }
    }

    fn configuration(
        position_metres: f64,
        velocity_metres_per_second: f64,
        relative: f64,
        minimum_seconds: f64,
        maximum_seconds: f64,
        initial_seconds: f64,
    ) -> IntegrationConfiguration {
        IntegrationConfiguration::new(
            Length::new::<meter>(position_metres),
            Velocity::new::<meter_per_second>(velocity_metres_per_second),
            Ratio::new::<ratio>(relative),
            Duration::from_seconds(minimum_seconds),
            Duration::from_seconds(maximum_seconds),
            Duration::from_seconds(initial_seconds),
            100_000,
            10_000,
        )
        .expect("valid fixture configuration")
    }

    fn state(position: [f64; 3], velocity: [f64; 3]) -> CartesianState {
        CartesianState::new(
            ReferenceFrame::GCRF,
            Position::from_metres(position[0], position[1], position[2]),
            VelocityVector::from_metres_per_second(velocity[0], velocity[1], velocity[2]),
        )
        .expect("finite fixture state")
    }

    fn assert_vector_close(actual: [f64; 3], expected: [f64; 3], tolerance: f64) {
        for (actual, expected) in actual.into_iter().zip(expected) {
            assert!(
                (actual - expected).abs() <= tolerance,
                "{actual:.17e} differs from {expected:.17e} by more than {tolerance:.3e}"
            );
        }
    }

    #[test]
    fn configuration_rejects_invalid_tolerances_bounds_and_limits() {
        let base = || {
            (
                Length::new::<meter>(1.0),
                Velocity::new::<meter_per_second>(1.0),
                Ratio::new::<ratio>(1.0e-6),
                Duration::from_seconds(0.1),
                Duration::from_seconds(10.0),
                Duration::from_seconds(1.0),
            )
        };
        let (_, velocity, relative, minimum, maximum, initial) = base();
        assert_eq!(
            IntegrationConfiguration::new(
                Length::new::<meter>(0.0),
                velocity,
                relative,
                minimum,
                maximum,
                initial,
                1,
                1,
            ),
            Err(IntegrationConfigurationError::InvalidPositionTolerance)
        );
        let (position, velocity, relative, _, _, _) = base();
        assert_eq!(
            IntegrationConfiguration::new(
                position,
                velocity,
                relative,
                Duration::from_seconds(2.0),
                Duration::from_seconds(1.0),
                Duration::from_seconds(1.5),
                1,
                1,
            ),
            Err(IntegrationConfigurationError::InvertedStepBounds)
        );
        let (position, velocity, relative, minimum, maximum, initial) = base();
        assert_eq!(
            IntegrationConfiguration::new(
                position, velocity, relative, minimum, maximum, initial, 0, 1,
            ),
            Err(IntegrationConfigurationError::ZeroStepLimit)
        );
    }

    #[test]
    fn constant_acceleration_is_exact_forward_and_backward() {
        let acceleration = AccelerationVector::from_metres_per_second_squared(2.0, -1.0, 0.5);
        let propagator = BogackiShampine32::new(
            ConstantAcceleration {
                value: acceleration,
            },
            configuration(1.0e-9, 1.0e-12, 1.0e-12, 1.0e-6, 20.0, 7.0),
        );
        let epoch = Epoch::from_tai_seconds(1_000.0);
        let initial_state = state([10.0, -4.0, 8.0], [3.0, 2.0, -1.0]);
        let target = epoch + Duration::from_seconds(100.0);
        let propagated = propagator
            .propagate(Orbit::new(epoch, initial_state), target)
            .expect("forward propagation");
        assert_eq!(propagated.epoch(), target);
        assert_vector_close(
            propagated.as_ref().position().to_metres(),
            [10_310.0, -4_804.0, 2_408.0],
            2.0e-10,
        );
        assert_vector_close(
            propagated.as_ref().velocity().to_metres_per_second(),
            [203.0, -98.0, 49.0],
            2.0e-12,
        );

        let recovered = propagator
            .propagate(propagated, epoch)
            .expect("backward propagation");
        assert_eq!(recovered.epoch(), epoch);
        assert_vector_close(
            recovered.as_ref().position().to_metres(),
            initial_state.position().to_metres(),
            2.0e-9,
        );
        assert_vector_close(
            recovered.as_ref().velocity().to_metres_per_second(),
            initial_state.velocity().to_metres_per_second(),
            2.0e-11,
        );
    }

    #[test]
    fn zero_duration_validates_without_evaluating_derivatives() {
        let propagator = BogackiShampine32::new(
            FailingDynamics,
            configuration(1.0, 1.0, 1.0e-6, 0.1, 10.0, 1.0),
        );
        let epoch = Epoch::from_tai_seconds(42.0);
        let initial = state([1.0, 2.0, 3.0], [4.0, 5.0, 6.0]);
        let result = propagator
            .propagate(Orbit::new(epoch, initial), epoch)
            .expect("zero-duration validation");
        assert_eq!(result.epoch(), epoch);
        assert_eq!(result.as_ref(), &initial);
    }

    #[test]
    fn dynamics_error_retains_its_source() {
        let propagator = BogackiShampine32::new(
            FailingDynamics,
            configuration(1.0, 1.0, 1.0e-6, 0.1, 10.0, 1.0),
        );
        let error = propagator
            .propagate(
                Orbit::new(
                    Epoch::from_tai_seconds(0.0),
                    state([1.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
                ),
                Epoch::from_tai_seconds(1.0),
            )
            .expect_err("model must fail");
        assert!(matches!(
            error,
            NumericalPropagationError::Dynamics(FixtureModelError)
        ));
        assert!(std::error::Error::source(&error).is_some());
    }

    #[test]
    fn acceleration_frame_mismatch_is_rejected() {
        let propagator = BogackiShampine32::new(
            WrongFrameDynamics,
            configuration(1.0, 1.0, 1.0e-6, 0.1, 10.0, 1.0),
        );
        let result = propagator.propagate(
            Orbit::new(
                Epoch::from_tai_seconds(0.0),
                state([1.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
            ),
            Epoch::from_tai_seconds(1.0),
        );
        assert!(matches!(
            result,
            Err(NumericalPropagationError::AccelerationFrameMismatch { .. })
        ));
    }

    #[test]
    fn tight_tolerance_rejects_without_mutating_the_accepted_state() {
        let propagator = BogackiShampine32::new(
            HarmonicOscillator,
            configuration(1.0e-10, 1.0e-10, 1.0e-12, 1.0e-8, 10.0, 10.0),
        );
        let epoch = Epoch::from_tai_seconds(0.0);
        let initial = state([1.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
        let (result, statistics) = propagator
            .integrate(epoch, initial, epoch + Duration::from_seconds(10.0))
            .expect("adaptive propagation");
        assert!(statistics.rejected_steps > 0);
        assert!((result.position().to_metres()[0] - 10.0_f64.cos()).abs() < 2.0e-8);
        assert!((result.velocity().to_metres_per_second()[0] + 10.0_f64.sin()).abs() < 2.0e-8);
    }

    #[test]
    fn minimum_step_and_attempt_limits_are_typed_failures() {
        let strict = IntegrationConfiguration::new(
            Length::new::<meter>(1.0e-15),
            Velocity::new::<meter_per_second>(1.0e-15),
            Ratio::new::<ratio>(1.0e-15),
            Duration::from_seconds(10.0),
            Duration::from_seconds(10.0),
            Duration::from_seconds(10.0),
            10,
            10,
        )
        .expect("valid strict configuration");
        let epoch = Epoch::from_tai_seconds(0.0);
        let initial = state([1.0, 0.0, 0.0], [0.0, 0.0, 0.0]);
        let result = BogackiShampine32::new(HarmonicOscillator, strict).propagate(
            Orbit::new(epoch, initial),
            epoch + Duration::from_seconds(10.0),
        );
        assert!(matches!(
            result,
            Err(NumericalPropagationError::MinimumStepExhausted { .. })
        ));

        let limited = IntegrationConfiguration::new(
            Length::new::<meter>(1.0),
            Velocity::new::<meter_per_second>(1.0),
            Ratio::new::<ratio>(1.0e-6),
            Duration::from_seconds(1.0),
            Duration::from_seconds(1.0),
            Duration::from_seconds(1.0),
            1,
            1,
        )
        .expect("valid limited configuration");
        let result = BogackiShampine32::new(
            ConstantAcceleration {
                value: AccelerationVector::from_metres_per_second_squared(0.0, 0.0, 0.0),
            },
            limited,
        )
        .propagate(
            Orbit::new(epoch, initial),
            epoch + Duration::from_seconds(3.0),
        );
        assert!(matches!(
            result,
            Err(NumericalPropagationError::StepLimitExceeded { attempted: 1 })
        ));
    }

    #[test]
    fn observed_global_order_is_three() {
        fn error_for_step(step_seconds: f64) -> f64 {
            let configuration = configuration(
                1.0e20,
                1.0e20,
                1.0e-15,
                step_seconds / 100.0,
                step_seconds,
                step_seconds,
            );
            let epoch = Epoch::from_tai_seconds(0.0);
            let result = BogackiShampine32::new(HarmonicOscillator, configuration)
                .propagate(
                    Orbit::new(epoch, state([1.0, 0.0, 0.0], [0.0, 0.0, 0.0])),
                    epoch + Duration::from_seconds(1.0),
                )
                .expect("fixed maximum-step propagation");
            let position_error = result.as_ref().position().to_metres()[0] - 1.0_f64.cos();
            let velocity_error =
                result.as_ref().velocity().to_metres_per_second()[0] + 1.0_f64.sin();
            position_error.hypot(velocity_error)
        }

        let coarse = error_for_step(0.2);
        let fine = error_for_step(0.1);
        let convergence_ratio = coarse / fine;
        assert!(
            (6.0..=10.0).contains(&convergence_ratio),
            "expected third-order ratio near 8, observed {convergence_ratio}"
        );
    }

    #[test]
    fn dense_extension_reproduces_endpoints_and_quadratic_solution() {
        let start = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let end = [6.0, 8.0, 10.0, 6.0, 7.0, 8.0];
        let start_derivative = [4.0, 5.0, 6.0, 2.0, 2.0, 2.0];
        let end_derivative = [6.0, 7.0, 8.0, 2.0, 2.0, 2.0];
        let dense = DenseStep::new(start, end, start_derivative, end_derivative, 1.0);
        assert_eq!(dense.evaluate(0.0), start);
        assert_eq!(dense.evaluate(1.0), end);
        assert_vector_close(
            dense.evaluate(0.5)[0..3].try_into().expect("three values"),
            [3.25, 4.75, 6.25],
            4.0e-15,
        );
    }

    fn earth_problem() -> (SharedCentralGravity, TwoBodyDynamics) {
        let parameter =
            GravitationalParameter::try_from(3.986_004_418e14).expect("positive Earth parameter");
        let gravity: SharedCentralGravity =
            Arc::new(PointMass::new(FrameOrigin::Body(Body::EARTH), parameter));
        let problem = TwoBodyDynamics::new(PointMassGravityModel::new(Arc::clone(&gravity)));
        (gravity, problem)
    }

    fn external_fixture_initial(gravity: SharedCentralGravity) -> CartesianState {
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
        .expect("valid elliptic fixture")
        .try_into()
        .expect("Cartesian conversion")
    }

    #[test]
    fn two_body_matches_analytical_and_recorded_orekit_endpoint() {
        let (gravity, problem) = earth_problem();
        let initial_state = external_fixture_initial(gravity);
        let epoch = Epoch::from_tai_seconds(0.0);
        let target = epoch + Duration::from_seconds(3_600.0);
        let numerical = BogackiShampine32::new(
            problem.clone(),
            configuration(1.0e-3, 1.0e-6, 1.0e-11, 1.0e-6, 30.0, 10.0),
        )
        .propagate(Orbit::new(epoch, initial_state), target)
        .expect("numerical propagation");
        let analytical = EllipticKeplerPropagator::new(problem)
            .propagate(Orbit::new(epoch, initial_state), target)
            .expect("analytical propagation");

        assert_vector_close(
            numerical.as_ref().position().to_metres(),
            analytical.as_ref().position().to_metres(),
            0.2,
        );
        assert_vector_close(
            numerical.as_ref().velocity().to_metres_per_second(),
            analytical.as_ref().velocity().to_metres_per_second(),
            2.0e-4,
        );
        assert_vector_close(
            numerical.as_ref().position().to_metres(),
            [
                4.863_976_030_492_352e6,
                4.133_125_643_091_070_5e6,
                -2.072_064_351_084_958e6,
            ],
            0.2,
        );
        assert_vector_close(
            numerical.as_ref().velocity().to_metres_per_second(),
            [
                -3.449_464_728_617_805e3,
                5.450_564_161_064_824_5e3,
                4.671_788_819_571_301e3,
            ],
            2.0e-4,
        );
    }

    #[test]
    fn two_body_rejects_non_inertial_frames_and_wrong_origins() {
        let (_, problem) = earth_problem();
        let terrestrial = CartesianState::new(
            ReferenceFrame::ITRF2020,
            Position::from_metres(7.0e6, 0.0, 0.0),
            VelocityVector::from_metres_per_second(0.0, 7.5e3, 0.0),
        )
        .expect("finite state");
        assert_eq!(
            problem.validate(&terrestrial),
            Err(TwoBodyEvaluationError::NonInertialFrame)
        );

        let mars_frame = ReferenceFrame::new(FrameOrigin::Body(Body::MARS), FrameOrientation::Gcrf);
        let wrong_origin = CartesianState::new(
            mars_frame,
            Position::from_metres(7.0e6, 0.0, 0.0),
            VelocityVector::from_metres_per_second(0.0, 7.5e3, 0.0),
        )
        .expect("finite state");
        assert!(matches!(
            problem.validate(&wrong_origin),
            Err(TwoBodyEvaluationError::GravityOriginMismatch { .. })
        ));
    }

    #[test]
    fn typed_acceleration_dimension_is_preserved() {
        let acceleration = Acceleration::new::<meter_per_second_squared>(1.0);
        let vector = AccelerationVector::new(acceleration, acceleration, acceleration);
        assert_eq!(vector.to_metres_per_second_squared(), [1.0, 1.0, 1.0]);
    }
}
