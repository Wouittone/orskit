use std::{error::Error as StdError, num::NonZeroU64};

use frames::ReferenceFrame;
use hifitime::{Duration, Epoch};
use orbits::cartesian::{CartesianState, StateError};
use orskit_core::Orbit;
use thiserror::Error;

use crate::StateVector;

/// Directional epoch interval covered by one dense ephemeris.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EphemerisInterval {
    start: Epoch,
    end: Epoch,
}

impl EphemerisInterval {
    pub(crate) const fn new(start: Epoch, end: Epoch) -> Self {
        Self { start, end }
    }

    /// Initial epoch in propagation order.
    #[must_use]
    pub const fn start(self) -> Epoch {
        self.start
    }

    /// Final epoch in propagation order.
    #[must_use]
    pub const fn end(self) -> Epoch {
        self.end
    }

    /// Whether `epoch` lies in the closed interval, independent of direction.
    #[must_use]
    pub fn contains(self, epoch: Epoch) -> bool {
        let (lower, upper) = ordered_epochs(self.start, self.end);
        epoch >= lower && epoch <= upper
    }
}

/// Accepted-step dense Cartesian ephemeris.
///
/// Each segment is a cubic Hermite continuous extension through the accepted
/// fifth-order RKF45 endpoints and their dynamics derivatives. The extension
/// is endpoint-consistent and has interpolation error `O(h^4)` for a smooth
/// state trajectory. It does not raise the RKF45 endpoint method's order or
/// provide a global trajectory-error bound.
#[derive(Debug, Clone)]
pub struct DenseEphemeris {
    interval: EphemerisInterval,
    frame: ReferenceFrame,
    initial: StateVector,
    segments: Vec<DenseSegment>,
}

impl DenseEphemeris {
    pub(crate) fn new(
        interval: EphemerisInterval,
        frame: ReferenceFrame,
        initial: StateVector,
        segments: Vec<DenseSegment>,
    ) -> Self {
        Self {
            interval,
            frame,
            initial,
            segments,
        }
    }

    /// Covered directional interval.
    #[must_use]
    pub const fn interval(&self) -> EphemerisInterval {
        self.interval
    }

    /// Frame shared by every ephemeris state.
    #[must_use]
    pub fn frame(&self) -> ReferenceFrame {
        self.frame
    }

    /// Number of accepted RKF45 steps retained as dense segments.
    #[must_use]
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    /// Interpolates the Cartesian state at an epoch in the closed interval.
    pub fn state_at(&self, epoch: Epoch) -> Result<Orbit<CartesianState>, DenseOutputError> {
        if !self.interval.contains(epoch) {
            return Err(DenseOutputError::OutsideInterval {
                epoch,
                interval: self.interval,
            });
        }
        if epoch == self.interval.start {
            return typed_orbit(epoch, self.frame, self.initial);
        }
        let segment = self
            .segments
            .iter()
            .find(|segment| segment.contains(epoch))
            .ok_or(DenseOutputError::MissingSegment { epoch })?;
        typed_orbit(epoch, self.frame, segment.interpolate(epoch))
    }

    /// Finds and handles bracketed detector roots over accepted dense segments.
    ///
    /// Detector direction is defined with increasing physical epoch, even for
    /// a backward-generated ephemeris. Roots separated by no more than the
    /// configured epoch tolerance are simultaneous; their handlers run in
    /// detector-slice order. Every handler in that simultaneous group runs
    /// before a `Stop` action terminates the search.
    ///
    /// The detector slice is homogeneous: every detector has the same concrete
    /// type and detector-specific error type.
    pub fn find_events<D: EventDetector>(
        &self,
        detectors: &mut [D],
        config: EventSearchConfig,
    ) -> Result<EventSearchOutcome, EventSearchError<D::Error>> {
        let mut events = Vec::new();
        let mut last_epochs = vec![None; detectors.len()];
        let mut pending = Vec::new();

        for (segment_index, segment) in self.segments.iter().enumerate() {
            let start = segment.start;
            let end = segment.end;
            let start_state = segment.state_at(start, self.frame)?;
            let end_state = segment.state_at(end, self.frame)?;
            let mut candidates = std::mem::take(&mut pending);
            for (detector_index, detector) in detectors.iter_mut().enumerate() {
                let start_value = evaluate(detector, detector_index, &start_state)?;
                let end_value = evaluate(detector, detector_index, &end_state)?;
                let Some(direction) = crossing_direction(start, start_value, end, end_value) else {
                    continue;
                };
                if !detector.direction().accepts(direction) {
                    continue;
                }
                let root = localize_root(
                    segment,
                    self.frame,
                    detector,
                    detector_index,
                    start,
                    start_value,
                    end,
                    end_value,
                    config,
                )?;
                if last_epochs[detector_index].is_some_and(|last| last == root.epoch()) {
                    continue;
                }
                candidates.push((detector_index, direction, root));
            }

            candidates.sort_by(|left, right| {
                propagation_order(
                    left.2.epoch(),
                    right.2.epoch(),
                    self.interval.start <= self.interval.end,
                )
                .then_with(|| left.0.cmp(&right.0))
            });
            candidates
                .dedup_by(|left, right| left.0 == right.0 && left.2.epoch() == right.2.epoch());

            while let Some((_, _, first)) = candidates.first() {
                let group_epoch = first.epoch();
                let group_len = candidates
                    .iter()
                    .take_while(|(_, _, state)| {
                        duration_magnitude_nanoseconds(state.epoch() - group_epoch)
                            <= duration_magnitude_nanoseconds(config.epoch_tolerance)
                    })
                    .count();
                let has_next_segment = segment_index + 1 < self.segments.len();
                if has_next_segment
                    && group_len == candidates.len()
                    && duration_magnitude_nanoseconds(end - group_epoch)
                        <= duration_magnitude_nanoseconds(config.epoch_tolerance)
                {
                    pending = candidates;
                    break;
                }
                let mut group: Vec<_> = candidates.drain(..group_len).collect();
                group.sort_by_key(|candidate| candidate.0);
                let remaining = config
                    .maximum_events
                    .get()
                    .saturating_sub(events.len() as u64);
                if group.len() as u64 > remaining {
                    return Err(EventSearchError::EventLimitExceeded {
                        maximum: config.maximum_events,
                    });
                }
                let mut stop = false;
                for (detector_index, direction, state) in group {
                    let occurrence = EventOccurrence {
                        detector_index,
                        direction,
                        state,
                    };
                    let action =
                        detectors[detector_index]
                            .handle(&occurrence)
                            .map_err(|source| EventSearchError::Detector {
                                detector_index,
                                stage: EventStage::Handler,
                                source: Box::new(source),
                            })?;
                    last_epochs[detector_index] = Some(occurrence.epoch());
                    events.push(occurrence);
                    stop |= action == EventAction::Stop;
                }
                if stop {
                    return Ok(EventSearchOutcome {
                        events,
                        stopped: true,
                    });
                }
            }
        }

        Ok(EventSearchOutcome {
            events,
            stopped: false,
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DenseSegment {
    start: Epoch,
    end: Epoch,
    start_state: StateVector,
    end_state: StateVector,
    start_derivative: StateVector,
    end_derivative: StateVector,
}

impl DenseSegment {
    pub(crate) const fn new(
        start: Epoch,
        end: Epoch,
        start_state: StateVector,
        end_state: StateVector,
        start_derivative: StateVector,
        end_derivative: StateVector,
    ) -> Self {
        Self {
            start,
            end,
            start_state,
            end_state,
            start_derivative,
            end_derivative,
        }
    }

    fn contains(&self, epoch: Epoch) -> bool {
        let (lower, upper) = ordered_epochs(self.start, self.end);
        epoch >= lower && epoch <= upper
    }

    fn interpolate(&self, epoch: Epoch) -> StateVector {
        if epoch == self.start {
            return self.start_state;
        }
        if epoch == self.end {
            return self.end_state;
        }
        let step_seconds = (self.end - self.start).to_seconds();
        let theta = (epoch - self.start).to_seconds() / step_seconds;
        let theta_squared = theta * theta;
        let theta_cubed = theta_squared * theta;
        let h00 = 2.0 * theta_cubed - 3.0 * theta_squared + 1.0;
        let h10 = theta_cubed - 2.0 * theta_squared + theta;
        let h01 = -2.0 * theta_cubed + 3.0 * theta_squared;
        let h11 = theta_cubed - theta_squared;
        std::array::from_fn(|component| {
            h00 * self.start_state[component]
                + h10 * step_seconds * self.start_derivative[component]
                + h01 * self.end_state[component]
                + h11 * step_seconds * self.end_derivative[component]
        })
    }

    fn state_at(
        &self,
        epoch: Epoch,
        frame: ReferenceFrame,
    ) -> Result<Orbit<CartesianState>, DenseOutputError> {
        typed_orbit(epoch, frame, self.interpolate(epoch))
    }
}

/// Dense-output query failure.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum DenseOutputError {
    /// Requested epoch lies outside the closed ephemeris interval.
    #[error("epoch {epoch} lies outside dense ephemeris interval {interval:?}")]
    OutsideInterval {
        /// Requested epoch.
        epoch: Epoch,
        /// Available interval.
        interval: EphemerisInterval,
    },
    /// Internal accepted-step coverage was incomplete.
    #[error("no dense segment covers in-range epoch {epoch}")]
    MissingSegment {
        /// Requested epoch.
        epoch: Epoch,
    },
    /// Interpolation could not reconstruct a valid typed state.
    #[error(transparent)]
    InvalidState(#[from] StateError),
}

/// Physical-time direction of a detector crossing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventDirection {
    /// Accept both increasing and decreasing crossings.
    Any,
    /// Accept only values increasing through zero with epoch.
    Increasing,
    /// Accept only values decreasing through zero with epoch.
    Decreasing,
}

impl EventDirection {
    fn accepts(self, actual: Self) -> bool {
        self == Self::Any || self == actual
    }
}

/// Event handler instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventAction {
    /// Continue searching the remaining dense interval.
    Continue,
    /// Stop after every handler in the simultaneous group has run.
    Stop,
}

/// Scalar event function and handler.
pub trait EventDetector {
    /// Detector-specific evaluation or handler failure.
    type Error: StdError + Send + Sync + 'static;

    /// Crossing direction accepted by this detector.
    fn direction(&self) -> EventDirection {
        EventDirection::Any
    }

    /// Signed scalar event function; a finite zero denotes an event.
    fn value(&mut self, state: &Orbit<CartesianState>) -> Result<f64, Self::Error>;

    /// Handles one localized event.
    ///
    /// The default keeps searching. State reset is deliberately absent from
    /// this immutable-ephemeris slice.
    fn handle(&mut self, _event: &EventOccurrence) -> Result<EventAction, Self::Error> {
        Ok(EventAction::Continue)
    }
}

/// Bounded event-search controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventSearchConfig {
    epoch_tolerance: Duration,
    maximum_iterations: NonZeroU64,
    maximum_events: NonZeroU64,
}

impl EventSearchConfig {
    /// Constructs a search with positive epoch tolerance and explicit limits.
    pub fn new(
        epoch_tolerance: Duration,
        maximum_iterations: NonZeroU64,
        maximum_events: NonZeroU64,
    ) -> Result<Self, EventSearchConfigError> {
        if epoch_tolerance <= Duration::ZERO {
            return Err(EventSearchConfigError::NonPositiveEpochTolerance);
        }
        Ok(Self {
            epoch_tolerance,
            maximum_iterations,
            maximum_events,
        })
    }

    /// Root bracket-width tolerance.
    #[must_use]
    pub const fn epoch_tolerance(self) -> Duration {
        self.epoch_tolerance
    }

    /// Maximum bisection iterations per root.
    #[must_use]
    pub const fn maximum_iterations(self) -> NonZeroU64 {
        self.maximum_iterations
    }

    /// Maximum handled events in one search.
    #[must_use]
    pub const fn maximum_events(self) -> NonZeroU64 {
        self.maximum_events
    }
}

/// Invalid event-search configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum EventSearchConfigError {
    /// Epoch tolerance was zero or negative.
    #[error("event root epoch tolerance must be positive")]
    NonPositiveEpochTolerance,
}

/// One localized event.
#[derive(Debug, Clone)]
pub struct EventOccurrence {
    detector_index: usize,
    direction: EventDirection,
    state: Orbit<CartesianState>,
}

impl EventOccurrence {
    /// Detector position in the supplied slice.
    #[must_use]
    pub const fn detector_index(&self) -> usize {
        self.detector_index
    }

    /// Physical-time crossing direction.
    #[must_use]
    pub const fn direction(&self) -> EventDirection {
        self.direction
    }

    /// Localized epoch.
    #[must_use]
    pub const fn epoch(&self) -> Epoch {
        self.state.epoch()
    }

    /// Dense Cartesian state at the localized epoch.
    #[must_use]
    pub const fn state(&self) -> &Orbit<CartesianState> {
        &self.state
    }
}

/// Completed event-search report.
#[derive(Debug, Clone)]
pub struct EventSearchOutcome {
    events: Vec<EventOccurrence>,
    stopped: bool,
}

impl EventSearchOutcome {
    /// Events in propagation order, with simultaneous events in detector order.
    #[must_use]
    pub fn events(&self) -> &[EventOccurrence] {
        &self.events
    }

    /// Whether a handler requested termination.
    #[must_use]
    pub const fn stopped(&self) -> bool {
        self.stopped
    }
}

/// Detector callback phase that failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventStage {
    /// Scalar event-function evaluation.
    Evaluation,
    /// Event handler invocation.
    Handler,
}

/// Failure during dense event search.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EventSearchError<E: StdError + Send + Sync + 'static> {
    /// Dense state evaluation failed.
    #[error(transparent)]
    DenseOutput(#[from] DenseOutputError),
    /// A detector callback failed.
    #[error("event detector {detector_index} failed during {stage:?}")]
    Detector {
        /// Detector position in the supplied slice.
        detector_index: usize,
        /// Callback phase.
        stage: EventStage,
        /// Preserved detector-specific source.
        #[source]
        source: Box<E>,
    },
    /// A detector returned NaN or infinity.
    #[error("event detector {detector_index} returned a non-finite value at {epoch}")]
    NonFiniteValue {
        /// Detector position in the supplied slice.
        detector_index: usize,
        /// Evaluation epoch.
        epoch: Epoch,
    },
    /// A bracket could not reach the configured epoch tolerance.
    #[error("event detector {detector_index} exhausted root iteration limit {maximum}")]
    RootIterationLimitExceeded {
        /// Detector position in the supplied slice.
        detector_index: usize,
        /// Configured iteration limit.
        maximum: NonZeroU64,
    },
    /// The configured handled-event limit was reached.
    #[error("event search exhausted event limit {maximum}")]
    EventLimitExceeded {
        /// Configured event limit.
        maximum: NonZeroU64,
    },
}

fn evaluate<D: EventDetector>(
    detector: &mut D,
    detector_index: usize,
    state: &Orbit<CartesianState>,
) -> Result<f64, EventSearchError<D::Error>> {
    let value = detector
        .value(state)
        .map_err(|source| EventSearchError::Detector {
            detector_index,
            stage: EventStage::Evaluation,
            source: Box::new(source),
        })?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(EventSearchError::NonFiniteValue {
            detector_index,
            epoch: state.epoch(),
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn localize_root<D: EventDetector>(
    segment: &DenseSegment,
    frame: ReferenceFrame,
    detector: &mut D,
    detector_index: usize,
    mut left_epoch: Epoch,
    mut left_value: f64,
    mut right_epoch: Epoch,
    right_value: f64,
    config: EventSearchConfig,
) -> Result<Orbit<CartesianState>, EventSearchError<D::Error>> {
    if left_value == 0.0 {
        return segment.state_at(left_epoch, frame).map_err(Into::into);
    }
    if right_value == 0.0 {
        return segment.state_at(right_epoch, frame).map_err(Into::into);
    }
    for _ in 0..config.maximum_iterations.get() {
        if duration_magnitude_nanoseconds(right_epoch - left_epoch)
            <= duration_magnitude_nanoseconds(config.epoch_tolerance)
        {
            return segment
                .state_at(midpoint(left_epoch, right_epoch), frame)
                .map_err(Into::into);
        }
        let middle_epoch = midpoint(left_epoch, right_epoch);
        let middle_state = segment.state_at(middle_epoch, frame)?;
        let middle_value = evaluate(detector, detector_index, &middle_state)?;
        if middle_value == 0.0 {
            return Ok(middle_state);
        }
        if same_sign(left_value, middle_value) {
            left_epoch = middle_epoch;
            left_value = middle_value;
        } else {
            right_epoch = middle_epoch;
        }
    }
    if duration_magnitude_nanoseconds(right_epoch - left_epoch)
        <= duration_magnitude_nanoseconds(config.epoch_tolerance)
    {
        return segment
            .state_at(midpoint(left_epoch, right_epoch), frame)
            .map_err(Into::into);
    }
    Err(EventSearchError::RootIterationLimitExceeded {
        detector_index,
        maximum: config.maximum_iterations,
    })
}

fn crossing_direction(
    first_epoch: Epoch,
    first_value: f64,
    second_epoch: Epoch,
    second_value: f64,
) -> Option<EventDirection> {
    if first_value != 0.0 && second_value != 0.0 && same_sign(first_value, second_value) {
        return None;
    }
    if first_value == 0.0 && second_value == 0.0 {
        return None;
    }
    let (earlier, later) = if first_epoch <= second_epoch {
        (first_value, second_value)
    } else {
        (second_value, first_value)
    };
    if later > earlier {
        Some(EventDirection::Increasing)
    } else {
        Some(EventDirection::Decreasing)
    }
}

fn same_sign(left: f64, right: f64) -> bool {
    left.is_sign_positive() == right.is_sign_positive()
}

fn midpoint(left: Epoch, right: Epoch) -> Epoch {
    left + Duration::from_total_nanoseconds((right - left).total_nanoseconds() / 2)
}

fn ordered_epochs(first: Epoch, second: Epoch) -> (Epoch, Epoch) {
    if first <= second {
        (first, second)
    } else {
        (second, first)
    }
}

fn propagation_order(left: Epoch, right: Epoch, forward: bool) -> std::cmp::Ordering {
    if forward {
        left.cmp(&right)
    } else {
        right.cmp(&left)
    }
}

fn duration_magnitude_nanoseconds(duration: Duration) -> u128 {
    nanosecond_magnitude(duration.total_nanoseconds())
}

fn nanosecond_magnitude(nanoseconds: i128) -> u128 {
    nanoseconds.unsigned_abs()
}

fn typed_orbit(
    epoch: Epoch,
    frame: ReferenceFrame,
    state: StateVector,
) -> Result<Orbit<CartesianState>, DenseOutputError> {
    Ok(Orbit::new(
        epoch,
        CartesianState::new(
            frame,
            units::Position::from_metres(state[0], state[1], state[2]),
            units::VelocityVector::from_metres_per_second(state[3], state[4], state[5]),
        )?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_magnitude_supports_minimum_nanoseconds() {
        assert_eq!(nanosecond_magnitude(i128::MIN), 1_u128 << 127);
        let minimum = Duration::from_total_nanoseconds(i128::MIN);
        assert_eq!(
            duration_magnitude_nanoseconds(minimum),
            minimum.total_nanoseconds().unsigned_abs()
        );
    }
}
