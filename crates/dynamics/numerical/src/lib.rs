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
//! cargo run -p dynamics-numerical --example event_detection
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

/// Boxed failure returned by an application-defined event detector or handler.
pub type EventCallbackError = Box<dyn Error + Send + Sync + 'static>;

/// Required sign change as propagation advances.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventDirection {
    /// Accept rising and falling sign changes.
    Any,
    /// Accept negative-to-positive changes in propagation order.
    Rising,
    /// Accept positive-to-negative changes in propagation order.
    Falling,
}

impl EventDirection {
    fn accepts(self, crossing: Self) -> bool {
        self == Self::Any || self == crossing
    }
}

/// Action requested after an event occurrence is reported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventAction {
    /// Continue from the accepted step without modifying the state.
    Continue,
    /// Stop after every occurrence simultaneous with this one is dispatched.
    Stop,
}

/// Application-defined switching function evaluated on dense Cartesian states.
///
/// The returned [`Ratio`] must be finite and dimensionless. The detector owns
/// any physical normalization needed to produce it. This slice detects roots
/// bracketed by a sign change or lying exactly on a checked-step endpoint;
/// unbracketed grazing roots are not claimed.
pub trait EventDetector: std::fmt::Debug + Send + Sync {
    /// Stable human-readable detector name retained in occurrences.
    fn name(&self) -> &str;

    /// Selects which propagation-order sign changes trigger this detector.
    fn direction(&self) -> EventDirection {
        EventDirection::Any
    }

    /// Evaluates the signed dimensionless switching function.
    fn value(&self, state: &Orbit<CartesianState>) -> Result<Ratio, EventCallbackError>;
}

/// Application callback invoked in deterministic occurrence order.
pub trait EventHandler {
    /// Handles one localized event.
    fn handle(&mut self, occurrence: &EventOccurrence) -> Result<EventAction, EventCallbackError>;
}

impl<F> EventHandler for F
where
    F: FnMut(&EventOccurrence) -> Result<EventAction, EventCallbackError>,
{
    fn handle(&mut self, occurrence: &EventOccurrence) -> Result<EventAction, EventCallbackError> {
        self(occurrence)
    }
}

/// Bounded event scanning and root-localization settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EventConfiguration {
    maximum_check_interval: Duration,
    time_tolerance: Duration,
    max_root_iterations: usize,
    max_events: usize,
}

impl EventConfiguration {
    /// Creates explicit event scanning and localization settings.
    ///
    /// Accepted integration steps are capped by `maximum_check_interval`.
    /// Callers must choose it small enough that each continuous detector has at
    /// most one sign change per interval. Roots within `time_tolerance` are
    /// treated as simultaneous and dispatched in detector registration order.
    pub fn new(
        maximum_check_interval: Duration,
        time_tolerance: Duration,
        max_root_iterations: usize,
        max_events: usize,
    ) -> Result<Self, EventConfigurationError> {
        positive_duration_seconds(
            maximum_check_interval,
            IntegrationConfigurationError::InvalidMaximumStep,
        )
        .map_err(|_| EventConfigurationError::InvalidMaximumCheckInterval)?;
        positive_duration_seconds(
            time_tolerance,
            IntegrationConfigurationError::InvalidMinimumStep,
        )
        .map_err(|_| EventConfigurationError::InvalidTimeTolerance)?;
        if time_tolerance.to_seconds() > maximum_check_interval.to_seconds() {
            return Err(EventConfigurationError::ToleranceExceedsCheckInterval);
        }
        if max_root_iterations == 0 {
            return Err(EventConfigurationError::ZeroRootIterations);
        }
        if max_events == 0 {
            return Err(EventConfigurationError::ZeroEventLimit);
        }
        Ok(Self {
            maximum_check_interval,
            time_tolerance,
            max_root_iterations,
            max_events,
        })
    }

    /// Returns the maximum interval over which one sign change is assumed.
    #[must_use]
    pub const fn maximum_check_interval(self) -> Duration {
        self.maximum_check_interval
    }

    /// Returns the time tolerance used for roots and simultaneity.
    #[must_use]
    pub const fn time_tolerance(self) -> Duration {
        self.time_tolerance
    }

    /// Returns the maximum bisection iterations for one root.
    #[must_use]
    pub const fn max_root_iterations(self) -> usize {
        self.max_root_iterations
    }

    /// Returns the maximum number of dispatched occurrences.
    #[must_use]
    pub const fn max_events(self) -> usize {
        self.max_events
    }
}

/// Invalid event scanning or localization configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum EventConfigurationError {
    /// Maximum check interval is not positive and finite.
    #[error("maximum event check interval must be a positive finite duration")]
    InvalidMaximumCheckInterval,
    /// Root time tolerance is not positive and finite.
    #[error("event time tolerance must be a positive finite duration")]
    InvalidTimeTolerance,
    /// Root tolerance is larger than the maximum check interval.
    #[error("event time tolerance must not exceed the maximum check interval")]
    ToleranceExceedsCheckInterval,
    /// Bisection iteration limit is zero.
    #[error("maximum root-localization iterations must be non-zero")]
    ZeroRootIterations,
    /// Event occurrence limit is zero.
    #[error("maximum event occurrences must be non-zero")]
    ZeroEventLimit,
}

/// One localized detector occurrence.
#[derive(Debug, Clone, PartialEq)]
pub struct EventOccurrence {
    detector_index: usize,
    detector_name: Box<str>,
    epoch: Epoch,
    state: CartesianState,
    crossing: EventDirection,
}

impl EventOccurrence {
    /// Returns the detector's registration index.
    #[must_use]
    pub const fn detector_index(&self) -> usize {
        self.detector_index
    }

    /// Returns the detector name captured at propagation time.
    #[must_use]
    pub fn detector_name(&self) -> &str {
        &self.detector_name
    }

    /// Returns the localized epoch.
    #[must_use]
    pub const fn epoch(&self) -> Epoch {
        self.epoch
    }

    /// Returns the interpolated Cartesian state.
    #[must_use]
    pub const fn state(&self) -> CartesianState {
        self.state
    }

    /// Returns the actual sign-change direction in propagation order.
    #[must_use]
    pub const fn crossing(&self) -> EventDirection {
        self.crossing
    }

    /// Returns the occurrence as an epoch-qualified orbit.
    #[must_use]
    pub const fn orbit(&self) -> Orbit<CartesianState> {
        Orbit::new(self.epoch, self.state)
    }
}

/// Immutable dense Cartesian output over one completed propagation arc.
#[derive(Debug, Clone)]
pub struct CartesianEphemeris {
    initial_epoch: Epoch,
    initial_state: CartesianState,
    final_epoch: Epoch,
    segments: Vec<EphemerisSegment>,
}

impl CartesianEphemeris {
    /// Returns the epoch at which integration began.
    #[must_use]
    pub const fn initial_epoch(&self) -> Epoch {
        self.initial_epoch
    }

    /// Returns the final target or stop epoch.
    #[must_use]
    pub const fn final_epoch(&self) -> Epoch {
        self.final_epoch
    }

    /// Returns the common Cartesian reference frame.
    #[must_use]
    pub const fn frame(&self) -> ReferenceFrame {
        self.initial_state.frame()
    }

    /// Returns the number of accepted dense segments.
    #[must_use]
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    /// Evaluates the accepted continuous extension without reintegration.
    pub fn state_at(&self, epoch: Epoch) -> Result<Orbit<CartesianState>, DenseOutputError> {
        if epoch == self.initial_epoch {
            return Ok(Orbit::new(epoch, self.initial_state));
        }
        let (coverage_start, coverage_end) = ordered_epochs(self.initial_epoch, self.final_epoch);
        if epoch < coverage_start || epoch > coverage_end {
            return Err(DenseOutputError::OutsideCoverage {
                requested: epoch,
                coverage_start,
                coverage_end,
            });
        }
        for segment in &self.segments {
            if segment.contains(epoch) {
                return segment.evaluate(epoch);
            }
        }
        Err(DenseOutputError::UnrepresentedEpoch { requested: epoch })
    }
}

/// Dense-output query failure.
#[derive(Debug, Clone, Copy, PartialEq, Error)]
pub enum DenseOutputError {
    /// Requested epoch is outside the completed propagation arc.
    #[error(
        "requested epoch {requested} is outside dense coverage [{coverage_start}, {coverage_end}]"
    )]
    OutsideCoverage {
        /// Requested epoch.
        requested: Epoch,
        /// Earliest covered epoch.
        coverage_start: Epoch,
        /// Latest covered epoch.
        coverage_end: Epoch,
    },
    /// A covered epoch could not be associated with a retained segment.
    #[error("requested epoch {requested} is not represented by a dense segment")]
    UnrepresentedEpoch {
        /// Requested epoch.
        requested: Epoch,
    },
    /// Interpolation produced a non-finite Cartesian state.
    #[error("dense interpolation produced a non-finite Cartesian state")]
    NonFiniteState,
}

/// Final state plus dense output for a completed propagation.
#[derive(Debug, Clone)]
pub struct DensePropagation {
    final_orbit: Orbit<CartesianState>,
    ephemeris: CartesianEphemeris,
}

impl DensePropagation {
    /// Returns the final target orbit.
    #[must_use]
    pub const fn final_orbit(&self) -> &Orbit<CartesianState> {
        &self.final_orbit
    }

    /// Returns the immutable dense ephemeris.
    #[must_use]
    pub const fn ephemeris(&self) -> &CartesianEphemeris {
        &self.ephemeris
    }

    /// Consumes the result into its final orbit and ephemeris.
    #[must_use]
    pub fn into_parts(self) -> (Orbit<CartesianState>, CartesianEphemeris) {
        (self.final_orbit, self.ephemeris)
    }
}

/// Final state, dense output, and event log from event-aware propagation.
#[derive(Debug, Clone)]
pub struct EventPropagation {
    final_orbit: Orbit<CartesianState>,
    ephemeris: CartesianEphemeris,
    occurrences: Vec<EventOccurrence>,
    stopped: bool,
}

impl EventPropagation {
    /// Returns the target or handler-stop orbit.
    #[must_use]
    pub const fn final_orbit(&self) -> &Orbit<CartesianState> {
        &self.final_orbit
    }

    /// Returns dense output truncated at the final orbit.
    #[must_use]
    pub const fn ephemeris(&self) -> &CartesianEphemeris {
        &self.ephemeris
    }

    /// Returns occurrences in deterministic dispatch order.
    #[must_use]
    pub fn occurrences(&self) -> &[EventOccurrence] {
        &self.occurrences
    }

    /// Returns whether a handler stopped propagation before the target.
    #[must_use]
    pub const fn stopped(&self) -> bool {
        self.stopped
    }
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

        let result = self.integrate_with(epoch, state, target, None, |_| {
            Ok(StepObservation::Continue)
        })?;
        Ok(Orbit::new(result.epoch, result.state))
    }
}

impl<P> BogackiShampine32<P>
where
    P: CartesianDynamics,
{
    /// Propagates to `target` and retains every accepted dense-output segment.
    pub fn propagate_with_ephemeris(
        &self,
        initial: Orbit<CartesianState>,
        target: Epoch,
    ) -> Result<DensePropagation, NumericalPropagationError<P::Error>> {
        let OrbitParts { epoch, state } = initial.into();
        self.problem
            .validate(&state)
            .map_err(NumericalPropagationError::Dynamics)?;
        let initial_state = state;
        let mut segments = Vec::new();
        let result = self.integrate_with(epoch, state, target, None, |step| {
            segments.push(step.segment(1.0));
            Ok(StepObservation::Continue)
        })?;
        let final_orbit = Orbit::new(result.epoch, result.state);
        let ephemeris = CartesianEphemeris {
            initial_epoch: epoch,
            initial_state,
            final_epoch: result.epoch,
            segments,
        };
        Ok(DensePropagation {
            final_orbit,
            ephemeris,
        })
    }

    /// Propagates with dense-output event detection and handler dispatch.
    ///
    /// Each accepted step is capped by
    /// [`EventConfiguration::maximum_check_interval`]. Within a step, one
    /// bracketed root per detector is localized by bisection. Occurrences are
    /// ordered by propagation time; roots within the configured time tolerance
    /// are dispatched by detector registration index. If any handler in a
    /// simultaneous group requests [`EventAction::Stop`], every occurrence in
    /// that group is dispatched before the ephemeris is truncated at the first
    /// root in propagation order. State reset is deliberately not supported by
    /// this first event slice.
    pub fn propagate_with_events(
        &self,
        initial: Orbit<CartesianState>,
        target: Epoch,
        detectors: &[&dyn EventDetector],
        event_configuration: EventConfiguration,
        handler: &mut dyn EventHandler,
    ) -> Result<EventPropagation, NumericalPropagationError<P::Error>> {
        let OrbitParts { epoch, state } = initial.into();
        self.problem
            .validate(&state)
            .map_err(NumericalPropagationError::Dynamics)?;
        for (detector_index, detector) in detectors.iter().enumerate() {
            if detector.name().trim().is_empty() {
                return Err(NumericalPropagationError::InvalidDetectorName { detector_index });
            }
        }

        let initial_state = state;
        let mut segments = Vec::new();
        let mut occurrences = Vec::new();
        let mut last_occurrence_epochs = vec![None; detectors.len()];
        let maximum_step = event_configuration.maximum_check_interval.to_seconds();
        let result = self.integrate_with(epoch, state, target, Some(maximum_step), |step| {
            process_events(
                step,
                detectors,
                event_configuration,
                handler,
                &mut last_occurrence_epochs,
                &mut occurrences,
                &mut segments,
            )
        })?;
        let final_orbit = Orbit::new(result.epoch, result.state);
        let ephemeris = CartesianEphemeris {
            initial_epoch: epoch,
            initial_state,
            final_epoch: result.epoch,
            segments,
        };
        Ok(EventPropagation {
            final_orbit,
            ephemeris,
            occurrences,
            stopped: result.stopped,
        })
    }

    #[cfg(test)]
    fn integrate(
        &self,
        initial_epoch: Epoch,
        initial_state: CartesianState,
        target: Epoch,
    ) -> Result<(CartesianState, IntegrationStatistics), NumericalPropagationError<P::Error>> {
        let result = self.integrate_with(initial_epoch, initial_state, target, None, |_| {
            Ok(StepObservation::Continue)
        })?;
        Ok((result.state, result.statistics))
    }

    fn integrate_with<F>(
        &self,
        initial_epoch: Epoch,
        initial_state: CartesianState,
        target: Epoch,
        maximum_step_override: Option<f64>,
        mut observe_step: F,
    ) -> Result<IntegrationResult, NumericalPropagationError<P::Error>>
    where
        F: FnMut(AcceptedDenseStep) -> Result<StepObservation, NumericalPropagationError<P::Error>>,
    {
        let total_seconds = (target - initial_epoch).to_seconds();
        if !total_seconds.is_finite() {
            return Err(NumericalPropagationError::NonFiniteDuration);
        }
        if total_seconds == 0.0 {
            return Ok(IntegrationResult {
                state: initial_state,
                epoch: initial_epoch,
                #[cfg(test)]
                statistics: IntegrationStatistics::default(),
                stopped: false,
            });
        }
        let direction = total_seconds.signum();
        let total_magnitude = total_seconds.abs();
        let minimum_step = self.configuration.minimum_step.to_seconds();
        let maximum_step = maximum_step_override
            .unwrap_or(f64::INFINITY)
            .min(self.configuration.maximum_step.to_seconds());
        let mut step_magnitude = self
            .configuration
            .initial_step
            .to_seconds()
            .min(maximum_step)
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
                let end_elapsed = if proposed_magnitude == remaining {
                    total_magnitude
                } else {
                    elapsed + proposed_magnitude
                };
                let start_epoch = initial_epoch + Duration::from_seconds(direction * elapsed);
                let end_epoch = if end_elapsed == total_magnitude {
                    target
                } else {
                    initial_epoch + Duration::from_seconds(direction * end_elapsed)
                };
                let accepted_step = AcceptedDenseStep {
                    start_epoch,
                    end_epoch,
                    frame,
                    dense,
                };
                statistics.accepted_steps += 1;
                if let StepObservation::Stop {
                    epoch: stop_epoch,
                    values: stop_values,
                } = observe_step(accepted_step)?
                {
                    return Ok(IntegrationResult {
                        state: array_to_state(frame, stop_values)?,
                        epoch: stop_epoch,
                        #[cfg(test)]
                        statistics,
                        stopped: true,
                    });
                }
                values = step.candidate;
                elapsed = end_elapsed;
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

        Ok(IntegrationResult {
            state: array_to_state(frame, values)?,
            epoch: target,
            #[cfg(test)]
            statistics,
            stopped: false,
        })
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

#[derive(Debug, Clone, Copy)]
struct IntegrationResult {
    state: CartesianState,
    epoch: Epoch,
    #[cfg(test)]
    statistics: IntegrationStatistics,
    stopped: bool,
}

#[derive(Debug, Clone, Copy)]
enum StepObservation {
    Continue,
    Stop {
        epoch: Epoch,
        values: [f64; COMPONENT_COUNT],
    },
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

#[derive(Debug, Clone, Copy)]
struct AcceptedDenseStep {
    start_epoch: Epoch,
    end_epoch: Epoch,
    frame: ReferenceFrame,
    dense: DenseStep,
}

impl AcceptedDenseStep {
    fn epoch_at_fraction(self, fraction: f64) -> Epoch {
        if fraction == 0.0 {
            self.start_epoch
        } else if fraction == 1.0 {
            self.end_epoch
        } else {
            self.start_epoch + Duration::from_seconds(self.dense.step_seconds * fraction)
        }
    }

    fn orbit_at_fraction(self, fraction: f64) -> Result<Orbit<CartesianState>, DenseOutputError> {
        let values = self.dense.evaluate(fraction);
        let state = dense_array_to_state(self.frame, values)?;
        Ok(Orbit::new(self.epoch_at_fraction(fraction), state))
    }

    fn segment(self, end_fraction: f64) -> EphemerisSegment {
        EphemerisSegment {
            start_epoch: self.start_epoch,
            end_epoch: self.epoch_at_fraction(end_fraction),
            end_fraction,
            frame: self.frame,
            dense: self.dense,
        }
    }
}

#[derive(Debug, Clone)]
struct EphemerisSegment {
    start_epoch: Epoch,
    end_epoch: Epoch,
    end_fraction: f64,
    frame: ReferenceFrame,
    dense: DenseStep,
}

impl EphemerisSegment {
    fn contains(&self, epoch: Epoch) -> bool {
        let (start, end) = ordered_epochs(self.start_epoch, self.end_epoch);
        epoch >= start && epoch <= end
    }

    fn evaluate(&self, epoch: Epoch) -> Result<Orbit<CartesianState>, DenseOutputError> {
        let fraction = if epoch == self.start_epoch {
            0.0
        } else if epoch == self.end_epoch {
            self.end_fraction
        } else {
            ((epoch - self.start_epoch).to_seconds() / self.dense.step_seconds)
                .clamp(0.0, self.end_fraction)
        };
        let state = dense_array_to_state(self.frame, self.dense.evaluate(fraction))?;
        Ok(Orbit::new(epoch, state))
    }
}

fn ordered_epochs(left: Epoch, right: Epoch) -> (Epoch, Epoch) {
    if left <= right {
        (left, right)
    } else {
        (right, left)
    }
}

fn dense_array_to_state(
    frame: ReferenceFrame,
    values: [f64; COMPONENT_COUNT],
) -> Result<CartesianState, DenseOutputError> {
    CartesianState::new(
        frame,
        Position::from_metres(values[0], values[1], values[2]),
        VelocityVector::from_metres_per_second(values[3], values[4], values[5]),
    )
    .map_err(|_| DenseOutputError::NonFiniteState)
}

#[derive(Debug, Clone)]
struct RootCandidate {
    fraction: f64,
    occurrence: EventOccurrence,
    values: [f64; COMPONENT_COUNT],
}

#[allow(clippy::too_many_arguments)]
fn process_events<E>(
    step: AcceptedDenseStep,
    detectors: &[&dyn EventDetector],
    configuration: EventConfiguration,
    handler: &mut dyn EventHandler,
    last_occurrence_epochs: &mut [Option<Epoch>],
    occurrences: &mut Vec<EventOccurrence>,
    segments: &mut Vec<EphemerisSegment>,
) -> Result<StepObservation, NumericalPropagationError<E>>
where
    E: Error + Send + Sync + 'static,
{
    let mut roots = Vec::new();
    for (detector_index, detector) in detectors.iter().enumerate() {
        let start = evaluate_detector::<E>(*detector, detector_index, step, 0.0)?;
        let end = evaluate_detector::<E>(*detector, detector_index, step, 1.0)?;
        let Some(crossing) = crossing_direction(start, end) else {
            continue;
        };
        if !detector.direction().accepts(crossing) {
            continue;
        }
        let fraction =
            localize_root::<E>(*detector, detector_index, step, start, end, configuration)?;
        let orbit = step
            .orbit_at_fraction(fraction)
            .map_err(NumericalPropagationError::DenseOutput)?;
        if last_occurrence_epochs[detector_index] == Some(orbit.epoch()) {
            continue;
        }
        roots.push(RootCandidate {
            fraction,
            values: state_to_array(*orbit.as_ref()),
            occurrence: EventOccurrence {
                detector_index,
                detector_name: detector.name().into(),
                epoch: orbit.epoch(),
                state: *orbit.as_ref(),
                crossing,
            },
        });
    }

    roots.sort_by(|left, right| left.fraction.total_cmp(&right.fraction));
    let simultaneous_fraction_tolerance =
        configuration.time_tolerance.to_seconds() / step.dense.step_seconds.abs();
    let mut cursor = 0;
    while cursor < roots.len() {
        let group_fraction = roots[cursor].fraction;
        let canonical = roots[cursor].clone();
        let mut group_end = cursor + 1;
        while group_end < roots.len()
            && (roots[group_end].fraction - group_fraction).abs() <= simultaneous_fraction_tolerance
        {
            group_end += 1;
        }
        roots[cursor..group_end].sort_by_key(|candidate| candidate.occurrence.detector_index);

        let mut stop_requested = false;
        for candidate in &roots[cursor..group_end] {
            if occurrences.len() >= configuration.max_events {
                return Err(NumericalPropagationError::EventLimitExceeded {
                    maximum: configuration.max_events,
                });
            }
            let action = handler.handle(&candidate.occurrence).map_err(|source| {
                NumericalPropagationError::EventHandler {
                    detector_index: candidate.occurrence.detector_index,
                    source,
                }
            })?;
            last_occurrence_epochs[candidate.occurrence.detector_index] =
                Some(candidate.occurrence.epoch);
            occurrences.push(candidate.occurrence.clone());
            stop_requested |= action == EventAction::Stop;
        }
        if stop_requested {
            segments.push(step.segment(canonical.fraction));
            return Ok(StepObservation::Stop {
                epoch: canonical.occurrence.epoch,
                values: canonical.values,
            });
        }
        cursor = group_end;
    }

    segments.push(step.segment(1.0));
    Ok(StepObservation::Continue)
}

fn evaluate_detector<E>(
    detector: &dyn EventDetector,
    detector_index: usize,
    step: AcceptedDenseStep,
    fraction: f64,
) -> Result<f64, NumericalPropagationError<E>>
where
    E: Error + Send + Sync + 'static,
{
    let orbit = step
        .orbit_at_fraction(fraction)
        .map_err(NumericalPropagationError::DenseOutput)?;
    let value = detector
        .value(&orbit)
        .map_err(|source| NumericalPropagationError::EventDetector {
            detector_index,
            source,
        })?
        .get::<ratio>();
    if !value.is_finite() {
        return Err(NumericalPropagationError::NonFiniteEventValue { detector_index });
    }
    Ok(value)
}

fn crossing_direction(start: f64, end: f64) -> Option<EventDirection> {
    if start == 0.0 && end == 0.0 {
        None
    } else if start <= 0.0 && end >= 0.0 {
        Some(EventDirection::Rising)
    } else if start >= 0.0 && end <= 0.0 {
        Some(EventDirection::Falling)
    } else {
        None
    }
}

fn localize_root<E>(
    detector: &dyn EventDetector,
    detector_index: usize,
    step: AcceptedDenseStep,
    start_value: f64,
    end_value: f64,
    configuration: EventConfiguration,
) -> Result<f64, NumericalPropagationError<E>>
where
    E: Error + Send + Sync + 'static,
{
    if start_value == 0.0 {
        return Ok(0.0);
    }
    if end_value == 0.0 {
        return Ok(1.0);
    }
    let fraction_tolerance =
        configuration.time_tolerance.to_seconds() / step.dense.step_seconds.abs();
    let mut left = 0.0;
    let mut right = 1.0;
    let mut left_value = start_value;
    for _ in 0..configuration.max_root_iterations {
        if right - left <= fraction_tolerance {
            return Ok(0.5 * (left + right));
        }
        let middle = 0.5 * (left + right);
        let middle_value = evaluate_detector::<E>(detector, detector_index, step, middle)?;
        if middle_value == 0.0 {
            return Ok(middle);
        }
        if left_value.is_sign_negative() == middle_value.is_sign_negative() {
            left = middle;
            left_value = middle_value;
        } else {
            right = middle;
        }
    }
    Err(NumericalPropagationError::EventRootNotConverged {
        detector_index,
        iterations: configuration.max_root_iterations,
    })
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
    /// An application detector has no usable diagnostic identity.
    #[error("event detector {detector_index} has a blank name")]
    InvalidDetectorName {
        /// Detector registration index.
        detector_index: usize,
    },
    /// Dense interpolation failed during propagation or event localization.
    #[error("dense-output evaluation failed")]
    DenseOutput(#[source] DenseOutputError),
    /// An application detector returned an error.
    #[error("event detector {detector_index} evaluation failed")]
    EventDetector {
        /// Detector registration index.
        detector_index: usize,
        /// Application error.
        #[source]
        source: EventCallbackError,
    },
    /// An application detector returned NaN or infinity.
    #[error("event detector {detector_index} returned a non-finite switching value")]
    NonFiniteEventValue {
        /// Detector registration index.
        detector_index: usize,
    },
    /// Bisection did not meet the configured time tolerance.
    #[error("event detector {detector_index} root did not converge in {iterations} iterations")]
    EventRootNotConverged {
        /// Detector registration index.
        detector_index: usize,
        /// Completed bisection iterations.
        iterations: usize,
    },
    /// An application event handler returned an error.
    #[error("handler for event detector {detector_index} failed")]
    EventHandler {
        /// Detector registration index.
        detector_index: usize,
        /// Application error.
        #[source]
        source: EventCallbackError,
    },
    /// The bounded event log is full.
    #[error("maximum event occurrence count {maximum} exhausted")]
    EventLimitExceeded {
        /// Configured maximum event count.
        maximum: usize,
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

    #[derive(Debug)]
    struct TimeDetector {
        name: &'static str,
        root: Epoch,
        direction: EventDirection,
    }

    impl EventDetector for TimeDetector {
        fn name(&self) -> &str {
            self.name
        }

        fn direction(&self) -> EventDirection {
            self.direction
        }

        fn value(&self, state: &Orbit<CartesianState>) -> Result<Ratio, EventCallbackError> {
            Ok(Ratio::new::<ratio>(
                (state.epoch() - self.root).to_seconds(),
            ))
        }
    }

    #[derive(Debug)]
    struct PositionXDetector {
        name: &'static str,
        threshold_metres: f64,
        direction: EventDirection,
    }

    impl EventDetector for PositionXDetector {
        fn name(&self) -> &str {
            self.name
        }

        fn direction(&self) -> EventDirection {
            self.direction
        }

        fn value(&self, state: &Orbit<CartesianState>) -> Result<Ratio, EventCallbackError> {
            Ok(Ratio::new::<ratio>(
                state.as_ref().position().to_metres()[0] - self.threshold_metres,
            ))
        }
    }

    #[derive(Debug)]
    struct GrazingTimeDetector {
        root: Epoch,
    }

    impl EventDetector for GrazingTimeDetector {
        fn name(&self) -> &str {
            "grazing"
        }

        fn value(&self, state: &Orbit<CartesianState>) -> Result<Ratio, EventCallbackError> {
            let offset = (state.epoch() - self.root).to_seconds();
            Ok(Ratio::new::<ratio>(offset * offset))
        }
    }

    #[derive(Debug)]
    struct InvalidEventDetector {
        name: &'static str,
        fails: bool,
    }

    impl EventDetector for InvalidEventDetector {
        fn name(&self) -> &str {
            self.name
        }

        fn value(&self, _state: &Orbit<CartesianState>) -> Result<Ratio, EventCallbackError> {
            if self.fails {
                Err(Box::new(FixtureModelError))
            } else {
                Ok(Ratio::new::<ratio>(f64::NAN))
            }
        }
    }

    fn event_configuration(
        maximum_check_seconds: f64,
        tolerance_seconds: f64,
        max_root_iterations: usize,
        max_events: usize,
    ) -> EventConfiguration {
        EventConfiguration::new(
            Duration::from_seconds(maximum_check_seconds),
            Duration::from_seconds(tolerance_seconds),
            max_root_iterations,
            max_events,
        )
        .expect("valid event configuration")
    }

    fn inertial_propagator() -> BogackiShampine32<ConstantAcceleration> {
        BogackiShampine32::new(
            ConstantAcceleration {
                value: AccelerationVector::from_metres_per_second_squared(0.0, 0.0, 0.0),
            },
            configuration(1.0e-9, 1.0e-12, 1.0e-12, 1.0e-6, 20.0, 7.0),
        )
    }

    #[test]
    fn dense_ephemeris_reproduces_endpoints_and_analytic_interior() {
        let propagator = BogackiShampine32::new(
            ConstantAcceleration {
                value: AccelerationVector::from_metres_per_second_squared(2.0, -1.0, 0.5),
            },
            configuration(1.0e-9, 1.0e-12, 1.0e-12, 1.0e-6, 20.0, 7.0),
        );
        let epoch = Epoch::from_tai_seconds(1_000.0);
        let initial_state = state([10.0, -4.0, 8.0], [3.0, 2.0, -1.0]);
        let target = epoch + Duration::from_seconds(100.0);
        let result = propagator
            .propagate_with_ephemeris(Orbit::new(epoch, initial_state), target)
            .expect("dense propagation");
        assert!(result.ephemeris().segment_count() > 1);
        assert_eq!(
            result.ephemeris().state_at(epoch).expect("initial sample"),
            Orbit::new(epoch, initial_state)
        );
        assert_eq!(
            result.ephemeris().state_at(target).expect("final sample"),
            result.final_orbit().clone()
        );

        let midpoint = result
            .ephemeris()
            .state_at(epoch + Duration::from_seconds(50.0))
            .expect("interior sample");
        assert_vector_close(
            midpoint.as_ref().position().to_metres(),
            [2_660.0, -1_154.0, 583.0],
            3.0e-10,
        );
        assert_vector_close(
            midpoint.as_ref().velocity().to_metres_per_second(),
            [103.0, -48.0, 24.0],
            3.0e-12,
        );
        assert!(matches!(
            result
                .ephemeris()
                .state_at(epoch - Duration::from_seconds(1.0)),
            Err(DenseOutputError::OutsideCoverage { .. })
        ));
    }

    #[test]
    fn reverse_dense_ephemeris_uses_chronological_coverage() {
        let propagator = inertial_propagator();
        let initial_epoch = Epoch::from_tai_seconds(10.0);
        let target = Epoch::from_tai_seconds(0.0);
        let result = propagator
            .propagate_with_ephemeris(
                Orbit::new(initial_epoch, state([10.0, 0.0, 0.0], [1.0, 0.0, 0.0])),
                target,
            )
            .expect("reverse dense propagation");
        let middle = result
            .ephemeris()
            .state_at(Epoch::from_tai_seconds(5.0))
            .expect("reverse interior sample");
        assert_vector_close(
            middle.as_ref().position().to_metres(),
            [5.0, 0.0, 0.0],
            1.0e-12,
        );
        assert_eq!(result.ephemeris().initial_epoch(), initial_epoch);
        assert_eq!(result.ephemeris().final_epoch(), target);
    }

    #[test]
    fn event_localization_uses_dense_state_and_stops_with_truncated_coverage() {
        let propagator = inertial_propagator();
        let epoch = Epoch::from_tai_seconds(0.0);
        let detector = PositionXDetector {
            name: "x=5m",
            threshold_metres: 5.0,
            direction: EventDirection::Rising,
        };
        let mut dispatched = Vec::new();
        let mut handler =
            |occurrence: &EventOccurrence| -> Result<EventAction, EventCallbackError> {
                dispatched.push(occurrence.clone());
                Ok(EventAction::Stop)
            };
        let result = propagator
            .propagate_with_events(
                Orbit::new(epoch, state([0.0, 0.0, 0.0], [1.0, 0.0, 0.0])),
                epoch + Duration::from_seconds(10.0),
                &[&detector],
                event_configuration(2.0, 1.0e-9, 64, 10),
                &mut handler,
            )
            .expect("event-aware propagation");
        assert!(result.stopped());
        assert_eq!(result.occurrences(), dispatched);
        assert_eq!(result.occurrences().len(), 1);
        assert!(
            (result.final_orbit().epoch() - (epoch + Duration::from_seconds(5.0)))
                .to_seconds()
                .abs()
                <= 1.0e-9
        );
        assert_vector_close(
            result.final_orbit().as_ref().position().to_metres(),
            [5.0, 0.0, 0.0],
            1.0e-9,
        );
        assert!(matches!(
            result
                .ephemeris()
                .state_at(epoch + Duration::from_seconds(6.0)),
            Err(DenseOutputError::OutsideCoverage { .. })
        ));
    }

    #[test]
    fn direction_is_defined_in_forward_and_reverse_propagation_order() {
        let propagator = inertial_propagator();
        let root = Epoch::from_tai_seconds(5.0);
        let rising = TimeDetector {
            name: "rising",
            root,
            direction: EventDirection::Rising,
        };
        let falling = TimeDetector {
            name: "falling",
            root,
            direction: EventDirection::Falling,
        };
        let mut continue_handler =
            |_occurrence: &EventOccurrence| -> Result<EventAction, EventCallbackError> {
                Ok(EventAction::Continue)
            };

        let forward = propagator
            .propagate_with_events(
                Orbit::new(
                    Epoch::from_tai_seconds(0.0),
                    state([0.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
                ),
                Epoch::from_tai_seconds(10.0),
                &[&rising, &falling],
                event_configuration(2.0, 1.0e-9, 64, 10),
                &mut continue_handler,
            )
            .expect("forward events");
        assert_eq!(forward.occurrences().len(), 1);
        assert_eq!(forward.occurrences()[0].detector_name(), "rising");
        assert_eq!(forward.occurrences()[0].crossing(), EventDirection::Rising);

        let reverse = propagator
            .propagate_with_events(
                Orbit::new(
                    Epoch::from_tai_seconds(10.0),
                    state([10.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
                ),
                Epoch::from_tai_seconds(0.0),
                &[&rising, &falling],
                event_configuration(2.0, 1.0e-9, 64, 10),
                &mut continue_handler,
            )
            .expect("reverse events");
        assert_eq!(reverse.occurrences().len(), 1);
        assert_eq!(reverse.occurrences()[0].detector_name(), "falling");
        assert_eq!(reverse.occurrences()[0].crossing(), EventDirection::Falling);
    }

    #[test]
    fn simultaneous_events_dispatch_in_registration_order_before_stop() {
        let propagator = inertial_propagator();
        let epoch = Epoch::from_tai_seconds(0.0);
        let first = TimeDetector {
            name: "first",
            root: epoch + Duration::from_seconds(5.0),
            direction: EventDirection::Any,
        };
        let second = TimeDetector {
            name: "second",
            root: epoch + Duration::from_seconds(5.0),
            direction: EventDirection::Any,
        };
        let mut dispatch_order = Vec::new();
        let mut handler =
            |occurrence: &EventOccurrence| -> Result<EventAction, EventCallbackError> {
                dispatch_order.push(occurrence.detector_index());
                Ok(if occurrence.detector_index() == 0 {
                    EventAction::Stop
                } else {
                    EventAction::Continue
                })
            };
        let result = propagator
            .propagate_with_events(
                Orbit::new(epoch, state([0.0, 0.0, 0.0], [1.0, 0.0, 0.0])),
                epoch + Duration::from_seconds(10.0),
                &[&first, &second],
                event_configuration(3.0, 1.0e-9, 64, 10),
                &mut handler,
            )
            .expect("simultaneous events");
        assert_eq!(dispatch_order, vec![0, 1]);
        assert_eq!(
            result
                .occurrences()
                .iter()
                .map(EventOccurrence::detector_index)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
        assert!(result.stopped());
    }

    #[test]
    fn events_follow_propagation_time_before_registration_order() {
        let propagator = inertial_propagator();
        let early = TimeDetector {
            name: "early",
            root: Epoch::from_tai_seconds(3.0),
            direction: EventDirection::Any,
        };
        let late = TimeDetector {
            name: "late",
            root: Epoch::from_tai_seconds(7.0),
            direction: EventDirection::Any,
        };
        let mut handler =
            |_occurrence: &EventOccurrence| -> Result<EventAction, EventCallbackError> {
                Ok(EventAction::Continue)
            };
        let forward = propagator
            .propagate_with_events(
                Orbit::new(
                    Epoch::from_tai_seconds(0.0),
                    state([0.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
                ),
                Epoch::from_tai_seconds(10.0),
                &[&late, &early],
                event_configuration(10.0, 1.0e-9, 64, 10),
                &mut handler,
            )
            .expect("forward ordering");
        assert_eq!(
            forward
                .occurrences()
                .iter()
                .map(EventOccurrence::detector_name)
                .collect::<Vec<_>>(),
            vec!["early", "late"]
        );

        let reverse = propagator
            .propagate_with_events(
                Orbit::new(
                    Epoch::from_tai_seconds(10.0),
                    state([10.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
                ),
                Epoch::from_tai_seconds(0.0),
                &[&early, &late],
                event_configuration(10.0, 1.0e-9, 64, 10),
                &mut handler,
            )
            .expect("reverse ordering");
        assert_eq!(
            reverse
                .occurrences()
                .iter()
                .map(EventOccurrence::detector_name)
                .collect::<Vec<_>>(),
            vec!["late", "early"]
        );
    }

    #[test]
    fn shared_step_boundary_root_is_reported_once() {
        let propagator = inertial_propagator();
        let detector = TimeDetector {
            name: "boundary",
            root: Epoch::from_tai_seconds(2.0),
            direction: EventDirection::Any,
        };
        let mut handler =
            |_occurrence: &EventOccurrence| -> Result<EventAction, EventCallbackError> {
                Ok(EventAction::Continue)
            };
        let result = propagator
            .propagate_with_events(
                Orbit::new(
                    Epoch::from_tai_seconds(0.0),
                    state([0.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
                ),
                Epoch::from_tai_seconds(6.0),
                &[&detector],
                event_configuration(2.0, 1.0e-9, 64, 10),
                &mut handler,
            )
            .expect("boundary event");
        assert_eq!(result.occurrences().len(), 1);
        assert_eq!(result.occurrences()[0].epoch(), detector.root);
    }

    #[test]
    fn unbracketed_grazing_root_is_not_claimed() {
        let propagator = inertial_propagator();
        let detector = GrazingTimeDetector {
            root: Epoch::from_tai_seconds(5.0),
        };
        let mut handler =
            |_occurrence: &EventOccurrence| -> Result<EventAction, EventCallbackError> {
                Ok(EventAction::Continue)
            };
        let result = propagator
            .propagate_with_events(
                Orbit::new(
                    Epoch::from_tai_seconds(0.0),
                    state([0.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
                ),
                Epoch::from_tai_seconds(10.0),
                &[&detector],
                event_configuration(2.0, 1.0e-9, 64, 10),
                &mut handler,
            )
            .expect("grazing scan");
        assert!(result.occurrences().is_empty());
        assert!(!result.stopped());
    }

    #[test]
    fn event_configuration_and_callback_failures_are_typed() {
        assert_eq!(
            EventConfiguration::new(
                Duration::from_seconds(1.0),
                Duration::from_seconds(2.0),
                1,
                1,
            ),
            Err(EventConfigurationError::ToleranceExceedsCheckInterval)
        );
        let propagator = inertial_propagator();
        let epoch = Epoch::from_tai_seconds(0.0);
        let initial = Orbit::new(epoch, state([0.0, 0.0, 0.0], [1.0, 0.0, 0.0]));
        let failing = InvalidEventDetector {
            name: "failing",
            fails: true,
        };
        let mut handler =
            |_occurrence: &EventOccurrence| -> Result<EventAction, EventCallbackError> {
                Ok(EventAction::Continue)
            };
        let error = propagator
            .propagate_with_events(
                initial.clone(),
                epoch + Duration::from_seconds(1.0),
                &[&failing],
                event_configuration(1.0, 1.0e-6, 32, 10),
                &mut handler,
            )
            .expect_err("detector must fail");
        assert!(matches!(
            error,
            NumericalPropagationError::EventDetector {
                detector_index: 0,
                ..
            }
        ));
        assert!(std::error::Error::source(&error).is_some());

        let non_finite = InvalidEventDetector {
            name: "non-finite",
            fails: false,
        };
        let result = propagator.propagate_with_events(
            initial,
            epoch + Duration::from_seconds(1.0),
            &[&non_finite],
            event_configuration(1.0, 1.0e-6, 32, 10),
            &mut handler,
        );
        assert!(matches!(
            result,
            Err(NumericalPropagationError::NonFiniteEventValue { detector_index: 0 })
        ));
    }

    #[test]
    fn root_iteration_event_limit_and_handler_errors_are_bounded() {
        let propagator = inertial_propagator();
        let epoch = Epoch::from_tai_seconds(0.0);
        let root = TimeDetector {
            name: "non-midpoint",
            root: epoch + Duration::from_seconds(0.3),
            direction: EventDirection::Any,
        };
        let mut continue_handler =
            |_occurrence: &EventOccurrence| -> Result<EventAction, EventCallbackError> {
                Ok(EventAction::Continue)
            };
        let result = propagator.propagate_with_events(
            Orbit::new(epoch, state([0.0, 0.0, 0.0], [1.0, 0.0, 0.0])),
            epoch + Duration::from_seconds(1.0),
            &[&root],
            event_configuration(1.0, 1.0e-9, 1, 10),
            &mut continue_handler,
        );
        assert!(matches!(
            result,
            Err(NumericalPropagationError::EventRootNotConverged {
                detector_index: 0,
                iterations: 1
            })
        ));

        let first = TimeDetector {
            name: "first",
            root: epoch + Duration::from_seconds(0.25),
            direction: EventDirection::Any,
        };
        let second = TimeDetector {
            name: "second",
            root: epoch + Duration::from_seconds(0.75),
            direction: EventDirection::Any,
        };
        let result = propagator.propagate_with_events(
            Orbit::new(epoch, state([0.0, 0.0, 0.0], [1.0, 0.0, 0.0])),
            epoch + Duration::from_seconds(1.0),
            &[&first, &second],
            event_configuration(1.0, 1.0e-9, 64, 1),
            &mut continue_handler,
        );
        assert!(matches!(
            result,
            Err(NumericalPropagationError::EventLimitExceeded { maximum: 1 })
        ));

        let mut failing_handler =
            |_occurrence: &EventOccurrence| -> Result<EventAction, EventCallbackError> {
                Err(Box::new(FixtureModelError))
            };
        let error = propagator
            .propagate_with_events(
                Orbit::new(epoch, state([0.0, 0.0, 0.0], [1.0, 0.0, 0.0])),
                epoch + Duration::from_seconds(1.0),
                &[&first],
                event_configuration(1.0, 1.0e-9, 64, 10),
                &mut failing_handler,
            )
            .expect_err("handler must fail");
        assert!(matches!(
            error,
            NumericalPropagationError::EventHandler {
                detector_index: 0,
                ..
            }
        ));
        assert!(std::error::Error::source(&error).is_some());
    }

    #[test]
    fn typed_acceleration_dimension_is_preserved() {
        let acceleration = Acceleration::new::<meter_per_second_squared>(1.0);
        let vector = AccelerationVector::new(acceleration, acceleration, acceleration);
        assert_eq!(vector.to_metres_per_second_squared(), [1.0, 1.0, 1.0]);
    }
}
