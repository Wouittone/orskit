//! Open estimation contracts and feature-gated instantaneous geometric models.
//!
//! A [`ParticipantStateProvider`] joins stable participant identities to
//! frame-qualified position and velocity at an explicit epoch. A
//! [`MeasurementEstimator`] then predicts one concrete measurement type without
//! erasing its units or dimensions. The built-in estimators deliberately model
//! only instantaneous Euclidean geometry: callers remain responsible for
//! propagation, frame transforms, light time, clocks, media, and other
//! corrections.

#[cfg(feature = "light-time")]
use std::fmt;
#[cfg(feature = "light-time")]
use std::num::NonZeroUsize;
use std::{collections::BTreeMap, error::Error as StdError};

use frames::{KinematicFrameTransformProvider, ReferenceFrame};
#[cfg(feature = "light-time")]
use hifitime::Duration;
use hifitime::Epoch;
use thiserror::Error;
#[cfg(feature = "light-time")]
use units::Position;
use units::VelocityVector;

#[cfg(any(feature = "geometric-tdoa", feature = "light-time"))]
use units::uom::si::time::second;
#[cfg(any(
    feature = "geometric-range-rate",
    feature = "geometric-bistatic-range-rate",
    feature = "geometric-doppler",
    feature = "geometric-tdoa",
    feature = "geometric-fdoa",
    feature = "geometric-phase"
))]
use units::uom::si::velocity::meter_per_second as velocity_unit;
#[cfg(feature = "geometric-tdoa")]
use units::Time;
#[cfg(any(
    feature = "geometric-azimuth-elevation",
    feature = "geometric-angular-ra-dec",
    feature = "geometric-phase"
))]
use units::{uom::si::angle::radian, Angle};
#[cfg(any(
    feature = "geometric-doppler",
    feature = "geometric-fdoa",
    feature = "geometric-phase"
))]
use units::{uom::si::frequency::hertz, Frequency};
#[cfg(any(
    feature = "geometric-range",
    feature = "geometric-bistatic-range",
    feature = "geometric-turnaround-range",
    feature = "light-time"
))]
use units::{uom::si::length::meter, Length};
#[cfg(any(
    feature = "geometric-doppler",
    feature = "geometric-tdoa",
    feature = "geometric-fdoa",
    feature = "geometric-phase",
    feature = "light-time"
))]
use utils::constants::speed_of_light;

#[cfg(feature = "geometric-bistatic-range")]
use crate::BistaticRangeMeasurement;
#[cfg(feature = "geometric-bistatic-range-rate")]
use crate::BistaticRangeRateMeasurement;
#[cfg(feature = "geometric-doppler")]
use crate::DopplerMeasurement;
#[cfg(feature = "geometric-fdoa")]
use crate::FdoaMeasurement;
#[cfg(any(
    feature = "geometric-angular-ra-dec",
    feature = "geometric-bistatic-range",
    feature = "geometric-bistatic-range-rate",
    feature = "geometric-turnaround-range",
    feature = "geometric-tdoa",
    feature = "geometric-fdoa",
    feature = "geometric-phase"
))]
use crate::GroundObservationError;
#[cfg(any(
    feature = "geometric-range",
    feature = "geometric-range-rate",
    feature = "geometric-doppler",
    feature = "geometric-bistatic-range",
    feature = "geometric-bistatic-range-rate",
    feature = "geometric-turnaround-range",
    feature = "geometric-tdoa",
    feature = "geometric-fdoa",
    feature = "geometric-phase"
))]
use crate::Measured;
#[cfg(any(
    feature = "geometric-range",
    feature = "geometric-range-rate",
    feature = "geometric-azimuth-elevation",
    feature = "geometric-doppler"
))]
use crate::MeasurementError;
#[cfg(any(
    feature = "geometric-azimuth-elevation",
    feature = "geometric-angular-ra-dec"
))]
use crate::MeasurementValues;
#[cfg(feature = "geometric-phase")]
use crate::PhaseMeasurement;
#[cfg(feature = "geometric-range-rate")]
use crate::RangeRateMeasurement;
#[cfg(feature = "geometric-angular-ra-dec")]
use crate::RightAscensionDeclinationMeasurement;
#[cfg(feature = "geometric-tdoa")]
use crate::TdoaMeasurement;
#[cfg(feature = "geometric-turnaround-range")]
use crate::TurnaroundRangeMeasurement;
#[cfg(feature = "geometric-azimuth-elevation")]
use crate::{AzimuthElevationConvention, AzimuthElevationMeasurement};
use crate::{CorrectionModelChain, CorrectionModelError, SignalEventTimeline};
use crate::{GroundStation, Measurement, ParticipantId};
#[cfg(any(
    feature = "geometric-range",
    feature = "geometric-range-rate",
    feature = "geometric-azimuth-elevation",
    feature = "geometric-doppler",
    feature = "geometric-angular-ra-dec",
    feature = "geometric-bistatic-range",
    feature = "geometric-bistatic-range-rate",
    feature = "geometric-turnaround-range",
    feature = "geometric-tdoa",
    feature = "geometric-fdoa",
    feature = "geometric-phase"
))]
use crate::{MeasurementValueError, SignalPath};
#[cfg(feature = "geometric-range")]
use crate::{RangeConvention, RangeMeasurement};

/// Position and velocity of a participant in one explicitly declared frame.
///
/// Participant state and frame-transform kinematics are the same value. This
/// alias keeps the measurements vocabulary without introducing a duplicate
/// conversion boundary.
pub use frames::FrameKinematics as ParticipantKinematics;
/// Invalid [`ParticipantKinematics`] input.
pub use frames::FrameKinematicsError as ParticipantKinematicsError;

/// Resolves ground-station and spacecraft identities to state at an epoch.
///
/// Returning `Ok(None)` means this provider does not own the identity. This
/// lets [`CompositeParticipantStateProvider`] combine a station network with
/// caller-defined spacecraft ephemerides without a closed participant enum.
pub trait ParticipantStateProvider {
    /// Provider-specific failure, for example an ephemeris coverage error.
    type Error: StdError + Send + Sync + 'static;

    /// Resolves `participant` at `epoch` into the requested expression frame.
    fn state_at(
        &self,
        participant: &ParticipantId,
        epoch: Epoch,
        frame: ReferenceFrame,
    ) -> Result<Option<ParticipantKinematics>, Self::Error>;
}

impl<T: ParticipantStateProvider + ?Sized> ParticipantStateProvider for &T {
    type Error = T::Error;

    fn state_at(
        &self,
        participant: &ParticipantId,
        epoch: Epoch,
        frame: ReferenceFrame,
    ) -> Result<Option<ParticipantKinematics>, Self::Error> {
        (*self).state_at(participant, epoch, frame)
    }
}

/// Adapts a provider fixed in one source frame through an explicit transform provider.
///
/// The wrapped provider is always queried in `source_frame`; no request is
/// silently relabeled as a different frame. The transform provider owns any
/// orientation, translation, Earth-orientation, or ephemeris data required to
/// produce the caller's target frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransformingParticipantStateProvider<P, T> {
    source: P,
    source_frame: ReferenceFrame,
    transforms: T,
}

impl<P, T> TransformingParticipantStateProvider<P, T> {
    /// Creates an adapter that obtains source states in `source_frame`.
    #[must_use]
    pub const fn new(source: P, source_frame: ReferenceFrame, transforms: T) -> Self {
        Self {
            source,
            source_frame,
            transforms,
        }
    }

    /// Returns the wrapped state provider.
    #[must_use]
    pub const fn source(&self) -> &P {
        &self.source
    }

    /// Returns the source expression frame.
    #[must_use]
    pub const fn source_frame(&self) -> ReferenceFrame {
        self.source_frame
    }

    /// Returns the frame-transform provider.
    #[must_use]
    pub const fn transforms(&self) -> &T {
        &self.transforms
    }
}

impl<P: ParticipantStateProvider, T: KinematicFrameTransformProvider> ParticipantStateProvider
    for TransformingParticipantStateProvider<P, T>
{
    type Error = TransformingParticipantStateProviderError<P::Error, T::Error>;

    fn state_at(
        &self,
        participant: &ParticipantId,
        epoch: Epoch,
        frame: ReferenceFrame,
    ) -> Result<Option<ParticipantKinematics>, Self::Error> {
        let Some(source) = self
            .source
            .state_at(participant, epoch, self.source_frame)
            .map_err(TransformingParticipantStateProviderError::Source)?
        else {
            return Ok(None);
        };
        if source.frame() != self.source_frame {
            return Err(
                TransformingParticipantStateProviderError::UnexpectedSourceFrame {
                    participant: participant.clone(),
                    expected: self.source_frame,
                    actual: source.frame(),
                },
            );
        }
        let transformed = self
            .transforms
            .transform(epoch, source, frame)
            .map_err(TransformingParticipantStateProviderError::Transform)?;
        if transformed.frame() != frame {
            return Err(
                TransformingParticipantStateProviderError::UnexpectedTargetFrame {
                    participant: participant.clone(),
                    expected: frame,
                    actual: transformed.frame(),
                },
            );
        }
        Ok(Some(transformed))
    }
}

/// Failure while adapting a source provider through a frame transform.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TransformingParticipantStateProviderError<P: StdError + 'static, T: StdError + 'static> {
    /// The source state provider failed.
    #[error("source participant state provider failed")]
    Source(#[source] P),
    /// The frame-transform provider failed.
    #[error("kinematic frame transform failed")]
    Transform(#[source] T),
    /// The source provider relabeled its kinematics.
    #[error("participant {participant} supplied {actual:?}, expected source frame {expected:?}")]
    UnexpectedSourceFrame {
        /// Participant whose source state was returned.
        participant: ParticipantId,
        /// Frame requested from the source provider.
        expected: ReferenceFrame,
        /// Frame attached to the returned source state.
        actual: ReferenceFrame,
    },
    /// The transform provider returned kinematics in a different target frame.
    #[error("participant {participant} transform supplied {actual:?}, expected target frame {expected:?}")]
    UnexpectedTargetFrame {
        /// Participant whose transformed state was returned.
        participant: ParticipantId,
        /// Requested transformed frame.
        expected: ReferenceFrame,
        /// Frame attached to the transformed kinematics.
        actual: ReferenceFrame,
    },
}

/// Fixed station states resolved from one or more [`GroundStation`] values.
///
/// A station is fixed in its declared parent frame and has zero velocity in
/// this first model. It intentionally does not manufacture a transform into a
/// different frame.
#[derive(Debug, Clone, Default)]
pub struct GroundStationProvider {
    stations: BTreeMap<ParticipantId, GroundStation>,
}

impl GroundStationProvider {
    /// Creates a fixed-state provider for distinct station identities.
    pub fn new(
        stations: impl IntoIterator<Item = GroundStation>,
    ) -> Result<Self, GroundStationProviderError> {
        let mut values = BTreeMap::new();
        for station in stations {
            let id = station.id().clone();
            if values.insert(id.clone(), station).is_some() {
                return Err(GroundStationProviderError::DuplicateStation { participant: id });
            }
        }
        Ok(Self { stations: values })
    }

    /// Returns the station registered for `participant`, if any.
    #[must_use]
    pub fn station(&self, participant: &ParticipantId) -> Option<&GroundStation> {
        self.stations.get(participant)
    }
}

impl ParticipantStateProvider for GroundStationProvider {
    type Error = GroundStationProviderError;

    fn state_at(
        &self,
        participant: &ParticipantId,
        _epoch: Epoch,
        frame: ReferenceFrame,
    ) -> Result<Option<ParticipantKinematics>, Self::Error> {
        let Some(station) = self.stations.get(participant) else {
            return Ok(None);
        };
        if station.parent_frame() != frame {
            return Err(GroundStationProviderError::FrameMismatch {
                participant: participant.clone(),
                expected: Box::new(station.parent_frame()),
                actual: Box::new(frame),
            });
        }
        Ok(Some(
            ParticipantKinematics::new(
                station.position_in_parent(),
                VelocityVector::from_metres_per_second(0.0, 0.0, 0.0),
                frame,
            )
            .expect("a validated station frame has finite origin coordinates"),
        ))
    }
}

/// Failure while building or querying a fixed ground-station provider.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum GroundStationProviderError {
    /// The same identity was supplied more than once.
    #[error("ground-station participant {participant} was supplied more than once")]
    DuplicateStation {
        /// Duplicate participant identity.
        participant: ParticipantId,
    },
    /// A station cannot provide state in a frame it does not define.
    #[error(
        "ground station {participant} is fixed in {expected:?}, not requested frame {actual:?}"
    )]
    FrameMismatch {
        /// Station identity.
        participant: ParticipantId,
        /// Parent frame in which the station position is expressed.
        expected: Box<ReferenceFrame>,
        /// Requested estimator frame.
        actual: Box<ReferenceFrame>,
    },
}

/// Combines two independent participant providers in left-to-right order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompositeParticipantStateProvider<L, R> {
    left: L,
    right: R,
}

impl<L, R> CompositeParticipantStateProvider<L, R> {
    /// Joins two providers. The right provider is queried only when the left
    /// provider does not own the requested participant.
    #[must_use]
    pub const fn new(left: L, right: R) -> Self {
        Self { left, right }
    }

    /// Returns the left provider.
    #[must_use]
    pub const fn left(&self) -> &L {
        &self.left
    }

    /// Returns the right provider.
    #[must_use]
    pub const fn right(&self) -> &R {
        &self.right
    }
}

impl<L: ParticipantStateProvider, R: ParticipantStateProvider> ParticipantStateProvider
    for CompositeParticipantStateProvider<L, R>
{
    type Error = CompositeParticipantStateProviderError<L::Error, R::Error>;

    fn state_at(
        &self,
        participant: &ParticipantId,
        epoch: Epoch,
        frame: ReferenceFrame,
    ) -> Result<Option<ParticipantKinematics>, Self::Error> {
        match self
            .left
            .state_at(participant, epoch, frame)
            .map_err(CompositeParticipantStateProviderError::Left)?
        {
            Some(state) => Ok(Some(state)),
            None => self
                .right
                .state_at(participant, epoch, frame)
                .map_err(CompositeParticipantStateProviderError::Right),
        }
    }
}

/// Failure from one side of a composite participant provider.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CompositeParticipantStateProviderError<L: StdError + 'static, R: StdError + 'static> {
    /// The left provider failed.
    #[error("left participant provider failed")]
    Left(#[source] L),
    /// The right provider failed.
    #[error("right participant provider failed")]
    Right(#[source] R),
}

/// Solves signal propagation for one observable using correction fields.
///
/// Implementations evaluate [`CorrectionModelChain::propagation_gradient`] at
/// frame-qualified spacetime states while integrating each signal leg. The
/// solver, rather than an individual correction, derives the resulting event
/// epochs while preserving the observation epoch.
pub trait SignalPropagationSolver<M, C: ?Sized>: std::fmt::Debug + Send + Sync {
    /// Solves signal timing for `measurement` using the supplied corrections.
    fn solve_timing(
        &self,
        measurement: &M,
        corrections: &CorrectionModelChain<M, C>,
        conditions: &C,
    ) -> Result<SignalEventTimeline, SignalPropagationError>;
}

/// Erased source error from an application-defined signal-propagation solver.
pub type SignalPropagationError = Box<dyn StdError + Send + Sync + 'static>;

/// Iteratively resolves a vacuum light-time timeline for every path leg.
///
/// The final path event is fixed at the measurement's reported epoch. Each
/// preceding event is solved backward from its receiving event using geometric
/// distance divided by the exact vacuum speed of light. On every iteration the
/// solver samples the correction chain at the geometric leg midpoint, adding
/// its excess slowness to the vacuum delay. Applications that need higher-order
/// path integration, refraction, spacecraft turnaround delay, or specialized
/// relativistic effects provide a separate [`SignalPropagationSolver`].
///
/// Participant states are requested in the measurement's declared frame. Use
/// [`TransformingParticipantStateProvider`] to make a source-frame conversion
/// explicit; this solver never assumes that distinct frame identities share
/// axes or origins.
#[cfg(feature = "light-time")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VacuumLightTimeSolver<P> {
    participants: P,
    max_iterations: NonZeroUsize,
    convergence: Duration,
}

#[cfg(feature = "light-time")]
impl<P> VacuumLightTimeSolver<P> {
    /// Creates a solver with a 16-iteration limit and one-nanosecond event-time
    /// convergence threshold.
    #[must_use]
    pub fn new(participants: P) -> Self {
        Self {
            participants,
            max_iterations: NonZeroUsize::new(16).expect("16 is non-zero"),
            convergence: Duration::from_seconds(1.0e-9),
        }
    }

    /// Replaces the convergence configuration.
    ///
    /// A positive, finite threshold is required. The convergence condition is
    /// the absolute difference between two successive emission-epoch updates.
    pub fn with_convergence(
        mut self,
        max_iterations: usize,
        convergence: Duration,
    ) -> Result<Self, VacuumLightTimeConfigurationError> {
        self.max_iterations = NonZeroUsize::new(max_iterations)
            .ok_or(VacuumLightTimeConfigurationError::ZeroIterations)?;
        let seconds = convergence.to_seconds();
        if !seconds.is_finite() || seconds <= 0.0 {
            return Err(VacuumLightTimeConfigurationError::InvalidConvergence);
        }
        self.convergence = convergence;
        Ok(self)
    }

    /// Returns the participant state provider.
    #[must_use]
    pub const fn participants(&self) -> &P {
        &self.participants
    }

    /// Returns the maximum fixed-point iterations allowed per leg.
    #[must_use]
    pub const fn max_iterations(&self) -> usize {
        self.max_iterations.get()
    }

    /// Returns the absolute event-time convergence threshold.
    #[must_use]
    pub const fn convergence(&self) -> Duration {
        self.convergence
    }
}

/// Invalid [`VacuumLightTimeSolver`] configuration.
#[cfg(feature = "light-time")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum VacuumLightTimeConfigurationError {
    /// At least one fixed-point iteration is required.
    #[error("vacuum light-time solver requires at least one iteration")]
    ZeroIterations,
    /// The convergence threshold must be finite and strictly positive.
    #[error("vacuum light-time convergence threshold must be finite and strictly positive")]
    InvalidConvergence,
}

/// Failure while resolving a vacuum light-time path.
#[cfg(feature = "light-time")]
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum VacuumLightTimeError<E: StdError + 'static> {
    /// The participant state provider failed.
    #[error("participant state provider failed for {participant}")]
    Participant {
        /// Participant whose state was requested.
        participant: ParticipantId,
        /// Provider-specific source error.
        #[source]
        source: Box<E>,
    },
    /// No provider owns a required path participant.
    #[error("no participant state provider owns {participant}")]
    MissingParticipant {
        /// Missing identity.
        participant: ParticipantId,
    },
    /// A provider did not return coordinates in the requested measurement frame.
    #[error("participant {participant} supplied {actual:?}, expected {expected:?}")]
    FrameMismatch {
        /// Participant identity.
        participant: ParticipantId,
        /// Requested measurement frame.
        expected: Box<ReferenceFrame>,
        /// Frame attached to returned kinematics.
        actual: Box<ReferenceFrame>,
    },
    /// A path leg has zero geometric length.
    #[error("signal-path leg {leg} has zero geometric length")]
    ZeroLengthLeg {
        /// Zero-based path-leg index.
        leg: usize,
    },
    /// A geometric or delay calculation became NaN or infinite.
    #[error("vacuum light-time computation produced a non-finite value")]
    NonFiniteComputation,
    /// Corrections made the total modeled propagation delay non-positive.
    #[error("signal-path leg {leg} has a non-positive modeled propagation delay")]
    NonPositiveDelay {
        /// Zero-based path-leg index.
        leg: usize,
    },
    /// A correction model rejected midpoint propagation evaluation.
    #[error("signal-propagation correction evaluation failed")]
    Correction(#[source] CorrectionModelError),
    /// A leg did not converge within the configured iteration bound.
    #[error("signal-path leg {leg} did not converge after {iterations} iterations")]
    NonConvergent {
        /// Zero-based path-leg index.
        leg: usize,
        /// Number of fixed-point updates evaluated.
        iterations: usize,
    },
}

#[cfg(feature = "light-time")]
impl<P: ParticipantStateProvider + fmt::Debug + Send + Sync, M: Measurement, C: ?Sized>
    SignalPropagationSolver<M, C> for VacuumLightTimeSolver<P>
{
    fn solve_timing(
        &self,
        measurement: &M,
        corrections: &CorrectionModelChain<M, C>,
        conditions: &C,
    ) -> Result<SignalEventTimeline, SignalPropagationError> {
        let path = measurement.path();
        let frame = measurement.frame();
        let mut events = vec![measurement.epoch(); path.participant_count()];

        for leg in (0..path.participant_count() - 1).rev() {
            let receiver_epoch = events[leg + 1];
            let receiver = self
                .state(
                    path.participant(leg + 1).expect("valid signal path"),
                    receiver_epoch,
                    frame,
                )
                .map_err(|error| Box::new(error) as SignalPropagationError)?;
            events[leg] = self
                .solve_leg(
                    measurement,
                    corrections,
                    conditions,
                    leg,
                    path.participant(leg).expect("valid signal path"),
                    receiver,
                    receiver_epoch,
                    frame,
                )
                .map_err(|error| Box::new(error) as SignalPropagationError)?;
        }

        SignalEventTimeline::instantaneous(measurement)
            .with_event_epochs(events)
            .map_err(|error| Box::new(error) as SignalPropagationError)
    }
}

#[cfg(feature = "light-time")]
impl<P: ParticipantStateProvider> VacuumLightTimeSolver<P> {
    fn state(
        &self,
        participant: &ParticipantId,
        epoch: Epoch,
        frame: ReferenceFrame,
    ) -> Result<ParticipantKinematics, VacuumLightTimeError<P::Error>> {
        let state = self
            .participants
            .state_at(participant, epoch, frame)
            .map_err(|source| VacuumLightTimeError::Participant {
                participant: participant.clone(),
                source: Box::new(source),
            })?
            .ok_or_else(|| VacuumLightTimeError::MissingParticipant {
                participant: participant.clone(),
            })?;
        if state.frame() != frame {
            return Err(VacuumLightTimeError::FrameMismatch {
                participant: participant.clone(),
                expected: Box::new(frame),
                actual: Box::new(state.frame()),
            });
        }
        Ok(state)
    }

    #[allow(clippy::too_many_arguments)]
    fn solve_leg<M: Measurement, C: ?Sized>(
        &self,
        measurement: &M,
        corrections: &CorrectionModelChain<M, C>,
        conditions: &C,
        leg: usize,
        emitter: &ParticipantId,
        receiver: ParticipantKinematics,
        receiver_epoch: Epoch,
        frame: ReferenceFrame,
    ) -> Result<Epoch, VacuumLightTimeError<P::Error>> {
        let mut emission_epoch = receiver_epoch;
        let convergence_seconds = self.convergence.to_seconds();

        for _ in 0..self.max_iterations.get() {
            let source = self.state(emitter, emission_epoch, frame)?;
            let source_position = source.position().to_metres();
            let receiver_position = receiver.position().to_metres();
            let displacement =
                std::array::from_fn(|axis| receiver_position[axis] - source_position[axis]);
            let distance_metres = dot(displacement, displacement).sqrt();
            if !distance_metres.is_finite() {
                return Err(VacuumLightTimeError::NonFiniteComputation);
            }
            if distance_metres == 0.0 {
                return Err(VacuumLightTimeError::ZeroLengthLeg { leg });
            }

            let distance = Length::new::<meter>(distance_metres);
            let midpoint_position = Position::from_metres(
                0.5 * (source_position[0] + receiver_position[0]),
                0.5 * (source_position[1] + receiver_position[1]),
                0.5 * (source_position[2] + receiver_position[2]),
            );
            let midpoint_epoch = emission_epoch
                + Duration::from_seconds((receiver_epoch - emission_epoch).to_seconds() * 0.5);
            let gradient = corrections
                .propagation_gradient(
                    measurement,
                    crate::SignalPropagationState::new(midpoint_epoch, midpoint_position, frame)
                        .expect("midpoint of finite positions is finite"),
                    conditions,
                )
                .map_err(VacuumLightTimeError::Correction)?;
            let delay_seconds = (distance / speed_of_light()).get::<second>()
                + (gradient.excess_slowness() * distance).get::<second>();
            if !delay_seconds.is_finite() {
                return Err(VacuumLightTimeError::NonFiniteComputation);
            }
            if delay_seconds <= 0.0 {
                return Err(VacuumLightTimeError::NonPositiveDelay { leg });
            }
            let next_epoch = receiver_epoch - Duration::from_seconds(delay_seconds);
            if (next_epoch - emission_epoch).to_seconds().abs() <= convergence_seconds {
                return Ok(next_epoch);
            }
            emission_epoch = next_epoch;
        }
        Err(VacuumLightTimeError::NonConvergent {
            leg,
            iterations: self.max_iterations.get(),
        })
    }
}

/// Predicts one concrete measurement type at an explicit epoch.
///
/// The generic `M` keeps a prediction's unit-bearing values and dimensions in
/// its concrete measurement implementation. Downstream crates may implement
/// this contract for application-defined observable families.
pub trait MeasurementEstimator<M: Measurement> {
    /// Estimator-specific prediction failure.
    type Error: StdError + Send + Sync + 'static;

    /// Predicts `measurement` at `epoch` while retaining its path, frame, and
    /// declared conventions. Prediction uncertainty is explicitly unknown
    /// until a state/correction uncertainty model is supplied.
    fn predict(&self, measurement: &M, epoch: Epoch) -> Result<M, Self::Error>;

    /// Predicts using a solved signal-event timeline.
    ///
    /// The default delegates to [`Self::predict`] at the immutable observation
    /// epoch. Estimators that model a multi-leg light-time path override this
    /// method and use [`SignalEventTimeline::event_epochs`] for each path
    /// event, while retaining the observation epoch on their returned
    /// measurement.
    fn predict_with_events(
        &self,
        measurement: &M,
        timeline: &SignalEventTimeline,
    ) -> Result<M, Self::Error> {
        self.predict(measurement, timeline.observation_epoch())
    }

    /// Solves signal propagation, predicts, then applies value-domain effects.
    ///
    /// The propagation solver integrates correction gradients and may produce
    /// emission or transit events distinct from the observation's reported
    /// epoch, for example for atmospheric or relativistic propagation delay.
    /// The same ordered correction chain then applies its value-domain effects
    /// and must preserve the observation epoch. A force model remains part of the
    /// [`ParticipantStateProvider`] that resolves spacecraft state at the
    /// requested epoch.
    fn predict_with_models<C: ?Sized, S: SignalPropagationSolver<M, C>>(
        &self,
        measurement: &M,
        propagation_solver: &S,
        correction_models: &CorrectionModelChain<M, C>,
        conditions: &C,
    ) -> Result<MeasurementPrediction<M>, MeasurementModelEstimationError<Self::Error>> {
        let timeline = propagation_solver
            .solve_timing(measurement, correction_models, conditions)
            .map_err(MeasurementModelEstimationError::Propagation)?;
        let predicted = self
            .predict_with_events(measurement, &timeline)
            .map_err(|error| MeasurementModelEstimationError::Estimation(Box::new(error)))?;
        verify_observation_epoch(
            &predicted,
            timeline.observation_epoch(),
            ObservationEpochStage::Estimator,
        )?;
        let corrected = correction_models
            .apply(predicted, &timeline, conditions)
            .map_err(MeasurementModelEstimationError::Correction)?;
        verify_observation_epoch(
            &corrected,
            timeline.observation_epoch(),
            ObservationEpochStage::Correction,
        )?;
        Ok(MeasurementPrediction {
            timeline,
            measurement: corrected,
        })
    }
}

/// A predicted measurement together with its solved signal-event timeline.
#[derive(Debug, Clone, PartialEq)]
pub struct MeasurementPrediction<M> {
    timeline: SignalEventTimeline,
    measurement: M,
}

impl<M> MeasurementPrediction<M> {
    /// Returns the signal-event timeline resolved before prediction.
    #[must_use]
    pub const fn timeline(&self) -> &SignalEventTimeline {
        &self.timeline
    }

    /// Returns the immutable epoch at which the observation was reported.
    #[must_use]
    pub const fn observation_epoch(&self) -> Epoch {
        self.timeline.observation_epoch()
    }

    /// Returns the predicted, value-corrected measurement.
    #[must_use]
    pub const fn measurement(&self) -> &M {
        &self.measurement
    }
}

fn verify_observation_epoch<M: Measurement, E: StdError + 'static>(
    measurement: &M,
    expected: Epoch,
    stage: ObservationEpochStage,
) -> Result<(), MeasurementModelEstimationError<E>> {
    let actual = measurement.epoch();
    (actual == expected).then_some(()).ok_or(
        MeasurementModelEstimationError::ObservationEpochMismatch {
            stage,
            expected,
            actual,
        },
    )
}

/// Stage that returned a measurement with rewritten observation provenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservationEpochStage {
    /// The estimator returned an epoch other than the observation epoch.
    Estimator,
    /// A value-domain correction changed the observation epoch.
    Correction,
}

/// Failure while solving signal events, predicting an observable, or applying value
/// corrections.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum MeasurementModelEstimationError<E: StdError + 'static> {
    /// The signal-propagation solver failed.
    #[error("signal-propagation solve failed")]
    Propagation(#[source] SignalPropagationError),
    /// Geometric or application-specific prediction failed.
    #[error("measurement prediction failed")]
    Estimation(#[source] Box<E>),
    /// A type-matching correction model rejected the predicted measurement.
    #[error("measurement correction model failed")]
    Correction(#[source] CorrectionModelError),
    /// An estimator or value correction changed the observation epoch.
    #[error("{stage:?} returned epoch {actual:?}; expected {expected:?}")]
    ObservationEpochMismatch {
        /// Stage that returned the invalid epoch.
        stage: ObservationEpochStage,
        /// Immutable epoch at which the observation was reported.
        expected: Epoch,
        /// Epoch returned by the stage.
        actual: Epoch,
    },
}

/// An instantaneous Euclidean geometric estimator backed by participant state.
///
/// It evaluates every path leg at the requested epoch in the measurement's
/// declared frame. It does not solve light time or apply physical corrections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeometricEstimator<P> {
    participants: P,
}

impl<P> GeometricEstimator<P> {
    /// Creates an estimator from a caller-selected participant provider.
    #[must_use]
    pub const fn new(participants: P) -> Self {
        Self { participants }
    }

    /// Returns the participant provider.
    #[must_use]
    pub const fn participants(&self) -> &P {
        &self.participants
    }

    /// Adds an emitted carrier frequency for carrier-based observables.
    #[cfg(any(
        feature = "geometric-doppler",
        feature = "geometric-fdoa",
        feature = "geometric-phase"
    ))]
    #[must_use]
    pub const fn with_carrier_frequency(
        self,
        carrier_frequency: Frequency,
    ) -> CarrierGeometricEstimator<P> {
        CarrierGeometricEstimator {
            geometry: self,
            carrier_frequency,
        }
    }
}

/// Instantaneous geometry plus a declared carrier frequency.
#[cfg(any(
    feature = "geometric-doppler",
    feature = "geometric-fdoa",
    feature = "geometric-phase"
))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CarrierGeometricEstimator<P> {
    geometry: GeometricEstimator<P>,
    carrier_frequency: Frequency,
}

#[cfg(any(
    feature = "geometric-doppler",
    feature = "geometric-fdoa",
    feature = "geometric-phase"
))]
impl<P> CarrierGeometricEstimator<P> {
    /// Returns the geometric participant link.
    #[must_use]
    pub const fn geometry(&self) -> &GeometricEstimator<P> {
        &self.geometry
    }

    /// Returns the emitted carrier frequency.
    #[must_use]
    pub const fn carrier_frequency(&self) -> Frequency {
        self.carrier_frequency
    }
}

/// Failure while predicting instantaneous geometric observables.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum GeometricEstimationError<E: StdError + 'static> {
    /// A provider could not resolve one participant state.
    #[error("participant state provider failed for {participant}")]
    Participant {
        /// Participant whose state was requested.
        participant: ParticipantId,
        /// Provider-specific source failure.
        #[source]
        source: Box<E>,
    },
    /// No joined provider owns this path participant.
    #[error("no participant state provider owns {participant}")]
    MissingParticipant {
        /// Missing identity.
        participant: ParticipantId,
    },
    /// A provider returned coordinates in a frame other than the requested frame.
    #[error("participant {participant} supplied {actual:?}, expected {expected:?}")]
    FrameMismatch {
        /// Participant identity.
        participant: ParticipantId,
        /// Frame declared by the measurement.
        expected: Box<ReferenceFrame>,
        /// Frame attached to returned kinematics.
        actual: Box<ReferenceFrame>,
    },
    /// A geometric path leg has zero length, so its direction is undefined.
    #[error("signal-path leg {leg} has zero geometric length")]
    ZeroLengthLeg {
        /// Zero-based path-leg index.
        leg: usize,
    },
    /// An intermediate geometric computation became NaN or infinite.
    #[error("instantaneous geometric computation produced a non-finite value")]
    NonFiniteGeometry,
    /// The observer lies at the selected frame origin, so radial vertical is undefined.
    #[error("observer is at the measurement-frame origin; local vertical is undefined")]
    UndefinedLocalVertical,
    /// The observer lies on the selected frame's z axis, so local north/east is undefined.
    #[error("observer lies on the measurement-frame z axis; local north/east is undefined")]
    UndefinedNorthEastAxes,
    /// A carrier-based observable requires a finite, positive carrier frequency.
    #[error("carrier frequency must be finite and strictly positive")]
    InvalidCarrierFrequency,
    /// A direct-reception observable's path endpoint disagrees with its named receiver.
    #[error("signal path ends at {actual}, but this observable names receiver {expected}")]
    ReceiverMismatch {
        /// Receiver named by the observable.
        expected: ParticipantId,
        /// Final signal-path participant.
        actual: ParticipantId,
    },
    /// Constructing the unit-qualified predicted value failed.
    #[cfg(any(
        feature = "geometric-range",
        feature = "geometric-range-rate",
        feature = "geometric-azimuth-elevation",
        feature = "geometric-doppler",
        feature = "geometric-angular-ra-dec",
        feature = "geometric-bistatic-range",
        feature = "geometric-bistatic-range-rate",
        feature = "geometric-turnaround-range",
        feature = "geometric-tdoa",
        feature = "geometric-fdoa",
        feature = "geometric-phase"
    ))]
    #[error(transparent)]
    InvalidValue(#[from] MeasurementValueError),
    /// The generated value violates the concrete measurement's semantic contract.
    #[cfg(any(
        feature = "geometric-range",
        feature = "geometric-range-rate",
        feature = "geometric-azimuth-elevation",
        feature = "geometric-doppler"
    ))]
    #[error(transparent)]
    InvalidMeasurement(#[from] MeasurementError),
    /// The generated value violates a ground-observation semantic contract.
    #[cfg(any(
        feature = "geometric-angular-ra-dec",
        feature = "geometric-bistatic-range",
        feature = "geometric-bistatic-range-rate",
        feature = "geometric-turnaround-range",
        feature = "geometric-tdoa",
        feature = "geometric-fdoa",
        feature = "geometric-phase"
    ))]
    #[error(transparent)]
    InvalidGroundObservation(#[from] GroundObservationError),
}

#[cfg(any(
    feature = "geometric-range",
    feature = "geometric-range-rate",
    feature = "geometric-azimuth-elevation",
    feature = "geometric-doppler",
    feature = "geometric-angular-ra-dec",
    feature = "geometric-bistatic-range",
    feature = "geometric-bistatic-range-rate",
    feature = "geometric-turnaround-range",
    feature = "geometric-phase"
))]
#[derive(Debug, Clone, Copy)]
struct PathLeg {
    #[cfg(any(
        feature = "geometric-range-rate",
        feature = "geometric-bistatic-range-rate",
        feature = "geometric-doppler",
        feature = "geometric-azimuth-elevation",
        feature = "geometric-angular-ra-dec"
    ))]
    displacement_metres: [f64; 3],
    #[cfg(any(
        feature = "geometric-range-rate",
        feature = "geometric-bistatic-range-rate",
        feature = "geometric-doppler"
    ))]
    relative_velocity_metres_per_second: [f64; 3],
    length_metres: f64,
}

#[cfg(any(
    feature = "geometric-range",
    feature = "geometric-range-rate",
    feature = "geometric-azimuth-elevation",
    feature = "geometric-doppler",
    feature = "geometric-angular-ra-dec",
    feature = "geometric-bistatic-range",
    feature = "geometric-bistatic-range-rate",
    feature = "geometric-turnaround-range",
    feature = "geometric-phase"
))]
type PathGeometry = (Vec<ParticipantKinematics>, Vec<PathLeg>);

#[cfg(any(
    feature = "geometric-range",
    feature = "geometric-range-rate",
    feature = "geometric-azimuth-elevation",
    feature = "geometric-doppler",
    feature = "geometric-angular-ra-dec",
    feature = "geometric-bistatic-range",
    feature = "geometric-bistatic-range-rate",
    feature = "geometric-turnaround-range",
    feature = "geometric-tdoa",
    feature = "geometric-fdoa",
    feature = "geometric-phase"
))]
impl<P: ParticipantStateProvider> GeometricEstimator<P> {
    fn state(
        &self,
        participant: &ParticipantId,
        epoch: Epoch,
        frame: ReferenceFrame,
    ) -> Result<ParticipantKinematics, GeometricEstimationError<P::Error>> {
        let state = self
            .participants
            .state_at(participant, epoch, frame)
            .map_err(|source| GeometricEstimationError::Participant {
                participant: participant.clone(),
                source: Box::new(source),
            })?
            .ok_or_else(|| GeometricEstimationError::MissingParticipant {
                participant: participant.clone(),
            })?;
        if state.frame() != frame {
            return Err(GeometricEstimationError::FrameMismatch {
                participant: participant.clone(),
                expected: Box::new(frame),
                actual: Box::new(state.frame()),
            });
        }
        Ok(state)
    }

    #[cfg(any(
        feature = "geometric-range",
        feature = "geometric-range-rate",
        feature = "geometric-azimuth-elevation",
        feature = "geometric-doppler",
        feature = "geometric-angular-ra-dec",
        feature = "geometric-bistatic-range",
        feature = "geometric-bistatic-range-rate",
        feature = "geometric-turnaround-range",
        feature = "geometric-phase"
    ))]
    fn path_states(
        &self,
        path: &SignalPath,
        epoch: Epoch,
        frame: ReferenceFrame,
    ) -> Result<Vec<ParticipantKinematics>, GeometricEstimationError<P::Error>> {
        path.participants()
            .iter()
            .map(|participant| self.state(participant, epoch, frame))
            .collect()
    }

    #[cfg(any(
        feature = "geometric-range",
        feature = "geometric-range-rate",
        feature = "geometric-azimuth-elevation",
        feature = "geometric-doppler",
        feature = "geometric-angular-ra-dec",
        feature = "geometric-bistatic-range",
        feature = "geometric-bistatic-range-rate",
        feature = "geometric-turnaround-range",
        feature = "geometric-phase"
    ))]
    fn legs(
        &self,
        path: &SignalPath,
        epoch: Epoch,
        frame: ReferenceFrame,
    ) -> Result<PathGeometry, GeometricEstimationError<P::Error>> {
        let states = self.path_states(path, epoch, frame)?;
        let legs = states
            .windows(2)
            .enumerate()
            .map(|(index, pair)| {
                let first = pair[0].position().to_metres();
                let endpoint = pair[1].position().to_metres();
                let displacement = std::array::from_fn(|axis| endpoint[axis] - first[axis]);
                let length = dot(displacement, displacement).sqrt();
                if !length.is_finite() {
                    return Err(GeometricEstimationError::NonFiniteGeometry);
                }
                if length == 0.0 {
                    return Err(GeometricEstimationError::ZeroLengthLeg { leg: index });
                }
                #[cfg(any(
                    feature = "geometric-range-rate",
                    feature = "geometric-bistatic-range-rate",
                    feature = "geometric-doppler"
                ))]
                let first_velocity = pair[0].velocity().to_metres_per_second();
                #[cfg(any(
                    feature = "geometric-range-rate",
                    feature = "geometric-bistatic-range-rate",
                    feature = "geometric-doppler"
                ))]
                let second_velocity = pair[1].velocity().to_metres_per_second();
                Ok(PathLeg {
                    #[cfg(any(
                        feature = "geometric-range-rate",
                        feature = "geometric-bistatic-range-rate",
                        feature = "geometric-doppler",
                        feature = "geometric-azimuth-elevation",
                        feature = "geometric-angular-ra-dec"
                    ))]
                    displacement_metres: displacement,
                    #[cfg(any(
                        feature = "geometric-range-rate",
                        feature = "geometric-bistatic-range-rate",
                        feature = "geometric-doppler"
                    ))]
                    relative_velocity_metres_per_second: std::array::from_fn(|axis| {
                        second_velocity[axis] - first_velocity[axis]
                    }),
                    length_metres: length,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok((states, legs))
    }

    #[cfg(any(
        feature = "geometric-range",
        feature = "geometric-bistatic-range",
        feature = "geometric-turnaround-range",
        feature = "geometric-phase"
    ))]
    fn path_length(
        &self,
        path: &SignalPath,
        epoch: Epoch,
        frame: ReferenceFrame,
    ) -> Result<f64, GeometricEstimationError<P::Error>> {
        let (_, legs) = self.legs(path, epoch, frame)?;
        let length = legs.iter().map(|leg| leg.length_metres).sum::<f64>();
        if length.is_finite() {
            Ok(length)
        } else {
            Err(GeometricEstimationError::NonFiniteGeometry)
        }
    }

    /// Computes geometric path length with every participant evaluated at its
    /// own solved signal-event epoch.
    #[cfg(feature = "geometric-range")]
    fn path_length_at_events(
        &self,
        path: &SignalPath,
        timeline: &SignalEventTimeline,
        frame: ReferenceFrame,
    ) -> Result<f64, GeometricEstimationError<P::Error>> {
        let states = path
            .participants()
            .iter()
            .zip(timeline.event_epochs())
            .map(|(participant, epoch)| self.state(participant, *epoch, frame))
            .collect::<Result<Vec<_>, _>>()?;
        let length = states
            .windows(2)
            .enumerate()
            .map(|(index, pair)| {
                let first = pair[0].position().to_metres();
                let endpoint = pair[1].position().to_metres();
                let displacement = std::array::from_fn(|axis| endpoint[axis] - first[axis]);
                let leg_length = dot(displacement, displacement).sqrt();
                if !leg_length.is_finite() {
                    return Err(GeometricEstimationError::NonFiniteGeometry);
                }
                if leg_length == 0.0 {
                    return Err(GeometricEstimationError::ZeroLengthLeg { leg: index });
                }
                Ok(leg_length)
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .sum::<f64>();
        if length.is_finite() {
            Ok(length)
        } else {
            Err(GeometricEstimationError::NonFiniteGeometry)
        }
    }

    #[cfg(any(
        feature = "geometric-range-rate",
        feature = "geometric-bistatic-range-rate",
        feature = "geometric-doppler"
    ))]
    fn path_range_rate(
        &self,
        path: &SignalPath,
        epoch: Epoch,
        frame: ReferenceFrame,
    ) -> Result<f64, GeometricEstimationError<P::Error>> {
        let (_, legs) = self.legs(path, epoch, frame)?;
        let rate = legs
            .iter()
            .map(|leg| {
                dot(
                    leg.displacement_metres,
                    leg.relative_velocity_metres_per_second,
                ) / leg.length_metres
            })
            .sum::<f64>();
        if rate.is_finite() {
            Ok(rate)
        } else {
            Err(GeometricEstimationError::NonFiniteGeometry)
        }
    }

    #[cfg(any(
        feature = "geometric-azimuth-elevation",
        feature = "geometric-angular-ra-dec"
    ))]
    fn final_reception_leg(
        &self,
        path: &SignalPath,
        epoch: Epoch,
        frame: ReferenceFrame,
    ) -> Result<(ParticipantKinematics, [f64; 3], f64), GeometricEstimationError<P::Error>> {
        let (states, legs) = self.legs(path, epoch, frame)?;
        let observer = *states
            .last()
            .expect("a signal path always has two participants");
        let leg = *legs.last().expect("a signal path always has one leg");
        Ok((
            observer,
            leg.displacement_metres.map(|value| -value),
            leg.length_metres,
        ))
    }

    #[cfg(any(
        feature = "geometric-tdoa",
        feature = "geometric-fdoa",
        feature = "geometric-phase"
    ))]
    fn direct_reception_states(
        &self,
        path: &SignalPath,
        receiver: &ParticipantId,
        epoch: Epoch,
        frame: ReferenceFrame,
    ) -> Result<(ParticipantKinematics, ParticipantKinematics), GeometricEstimationError<P::Error>>
    {
        let actual = path
            .participant(path.participant_count() - 1)
            .expect("a signal path always has a final participant");
        if actual != receiver {
            return Err(GeometricEstimationError::ReceiverMismatch {
                expected: receiver.clone(),
                actual: actual.clone(),
            });
        }
        let source = path
            .participant(0)
            .expect("a signal path always has a first participant");
        Ok((
            self.state(source, epoch, frame)?,
            self.state(receiver, epoch, frame)?,
        ))
    }

    #[cfg(any(feature = "geometric-tdoa", feature = "geometric-fdoa"))]
    fn direct_range(
        &self,
        source: ParticipantKinematics,
        receiver: ParticipantKinematics,
        leg: usize,
    ) -> Result<(f64, f64), GeometricEstimationError<P::Error>> {
        let displacement = std::array::from_fn(|axis| {
            receiver.position().to_metres()[axis] - source.position().to_metres()[axis]
        });
        let length = dot(displacement, displacement).sqrt();
        if !length.is_finite() {
            return Err(GeometricEstimationError::NonFiniteGeometry);
        }
        if length == 0.0 {
            return Err(GeometricEstimationError::ZeroLengthLeg { leg });
        }
        let velocity = std::array::from_fn(|axis| {
            receiver.velocity().to_metres_per_second()[axis]
                - source.velocity().to_metres_per_second()[axis]
        });
        let rate = dot(displacement, velocity) / length;
        if !rate.is_finite() {
            return Err(GeometricEstimationError::NonFiniteGeometry);
        }
        Ok((length, rate))
    }
}

#[cfg(any(
    feature = "geometric-range",
    feature = "geometric-bistatic-range",
    feature = "geometric-turnaround-range"
))]
fn predicted_length<E: StdError + 'static>(
    value: f64,
) -> Result<Measured<Length>, GeometricEstimationError<E>> {
    Ok(Measured::new([Length::new::<meter>(value)], None)?)
}

#[cfg(any(
    feature = "geometric-range-rate",
    feature = "geometric-bistatic-range-rate"
))]
fn predicted_velocity<E: StdError + 'static>(
    value: f64,
) -> Result<Measured<units::Velocity>, GeometricEstimationError<E>> {
    Ok(Measured::new(
        [units::Velocity::new::<velocity_unit>(value)],
        None,
    )?)
}

#[cfg(any(feature = "geometric-doppler", feature = "geometric-fdoa"))]
fn predicted_frequency<E: StdError + 'static>(
    value: f64,
) -> Result<Measured<Frequency>, GeometricEstimationError<E>> {
    Ok(Measured::new([Frequency::new::<hertz>(value)], None)?)
}

#[cfg(feature = "geometric-phase")]
fn predicted_angle<E: StdError + 'static>(
    value: f64,
) -> Result<Measured<Angle>, GeometricEstimationError<E>> {
    Ok(Measured::new([Angle::new::<radian>(value)], None)?)
}

#[cfg(feature = "geometric-tdoa")]
fn predicted_time<E: StdError + 'static>(
    value: f64,
) -> Result<Measured<Time>, GeometricEstimationError<E>> {
    Ok(Measured::new([Time::new::<second>(value)], None)?)
}

#[cfg(any(
    feature = "geometric-azimuth-elevation",
    feature = "geometric-angular-ra-dec"
))]
fn predicted_angles<E: StdError + 'static>(
    first: f64,
    second_value: f64,
) -> Result<MeasurementValues<Angle, 2>, GeometricEstimationError<E>> {
    Ok(MeasurementValues::new(
        [
            Angle::new::<radian>(first),
            Angle::new::<radian>(second_value),
        ],
        None,
    )?)
}

#[cfg(feature = "geometric-range")]
impl<P: ParticipantStateProvider> MeasurementEstimator<RangeMeasurement> for GeometricEstimator<P> {
    type Error = GeometricEstimationError<P::Error>;

    fn predict(
        &self,
        measurement: &RangeMeasurement,
        epoch: Epoch,
    ) -> Result<RangeMeasurement, Self::Error> {
        let mut length = self.path_length(measurement.path(), epoch, measurement.frame())?;
        if measurement.convention() == RangeConvention::RoundTripOneWayEquivalent {
            length *= 0.5;
        }
        Ok(RangeMeasurement::new(
            measurement.path().clone(),
            epoch,
            measurement.frame(),
            measurement.convention(),
            predicted_length(length)?,
        )?)
    }

    fn predict_with_events(
        &self,
        measurement: &RangeMeasurement,
        timeline: &SignalEventTimeline,
    ) -> Result<RangeMeasurement, Self::Error> {
        let mut length =
            self.path_length_at_events(measurement.path(), timeline, measurement.frame())?;
        if measurement.convention() == RangeConvention::RoundTripOneWayEquivalent {
            length *= 0.5;
        }
        Ok(measurement.with_value(predicted_length(length)?)?)
    }
}

#[cfg(feature = "geometric-range-rate")]
impl<P: ParticipantStateProvider> MeasurementEstimator<RangeRateMeasurement>
    for GeometricEstimator<P>
{
    type Error = GeometricEstimationError<P::Error>;

    fn predict(
        &self,
        measurement: &RangeRateMeasurement,
        epoch: Epoch,
    ) -> Result<RangeRateMeasurement, Self::Error> {
        Ok(RangeRateMeasurement::new(
            measurement.path().clone(),
            epoch,
            measurement.frame(),
            predicted_velocity(self.path_range_rate(
                measurement.path(),
                epoch,
                measurement.frame(),
            )?)?,
        ))
    }
}

#[cfg(feature = "geometric-azimuth-elevation")]
impl<P: ParticipantStateProvider> MeasurementEstimator<AzimuthElevationMeasurement>
    for GeometricEstimator<P>
{
    type Error = GeometricEstimationError<P::Error>;

    fn predict(
        &self,
        measurement: &AzimuthElevationMeasurement,
        epoch: Epoch,
    ) -> Result<AzimuthElevationMeasurement, Self::Error> {
        let (observer, direction, length) =
            self.final_reception_leg(measurement.path(), epoch, measurement.frame())?;
        let up = normalize(observer.position().to_metres())
            .ok_or(GeometricEstimationError::UndefinedLocalVertical)?;
        let east = normalize([-up[1], up[0], 0.0])
            .ok_or(GeometricEstimationError::UndefinedNorthEastAxes)?;
        let north = cross(up, east);
        let line_of_sight = direction.map(|value| value / length);
        let azimuth = dot(line_of_sight, east)
            .atan2(dot(line_of_sight, north))
            .rem_euclid(std::f64::consts::TAU);
        let elevation = dot(line_of_sight, up).clamp(-1.0, 1.0).asin();
        Ok(AzimuthElevationMeasurement::new(
            measurement.path().clone(),
            epoch,
            measurement.frame(),
            AzimuthElevationConvention::ClockwiseFromNorthAboveHorizon,
            predicted_angles(azimuth, elevation)?,
        )?)
    }
}

#[cfg(feature = "geometric-angular-ra-dec")]
impl<P: ParticipantStateProvider> MeasurementEstimator<RightAscensionDeclinationMeasurement>
    for GeometricEstimator<P>
{
    type Error = GeometricEstimationError<P::Error>;

    fn predict(
        &self,
        measurement: &RightAscensionDeclinationMeasurement,
        epoch: Epoch,
    ) -> Result<RightAscensionDeclinationMeasurement, Self::Error> {
        let (_, direction, length) =
            self.final_reception_leg(measurement.path(), epoch, measurement.frame())?;
        let right_ascension = direction[1]
            .atan2(direction[0])
            .rem_euclid(std::f64::consts::TAU);
        let declination = direction[2].atan2(direction[0].hypot(direction[1]));
        if !right_ascension.is_finite() || !declination.is_finite() || !length.is_finite() {
            return Err(GeometricEstimationError::NonFiniteGeometry);
        }
        Ok(RightAscensionDeclinationMeasurement::new(
            measurement.path().clone(),
            epoch,
            measurement.frame(),
            measurement.convention(),
            predicted_angles(right_ascension, declination)?,
        )?)
    }
}

#[cfg(feature = "geometric-bistatic-range")]
impl<P: ParticipantStateProvider> MeasurementEstimator<BistaticRangeMeasurement>
    for GeometricEstimator<P>
{
    type Error = GeometricEstimationError<P::Error>;

    fn predict(
        &self,
        measurement: &BistaticRangeMeasurement,
        epoch: Epoch,
    ) -> Result<BistaticRangeMeasurement, Self::Error> {
        Ok(BistaticRangeMeasurement::new(
            measurement.path().clone(),
            epoch,
            measurement.frame(),
            measurement.stations().clone(),
            predicted_length(self.path_length(measurement.path(), epoch, measurement.frame())?)?,
        ))
    }
}

#[cfg(feature = "geometric-bistatic-range-rate")]
impl<P: ParticipantStateProvider> MeasurementEstimator<BistaticRangeRateMeasurement>
    for GeometricEstimator<P>
{
    type Error = GeometricEstimationError<P::Error>;

    fn predict(
        &self,
        measurement: &BistaticRangeRateMeasurement,
        epoch: Epoch,
    ) -> Result<BistaticRangeRateMeasurement, Self::Error> {
        Ok(BistaticRangeRateMeasurement::new(
            measurement.path().clone(),
            epoch,
            measurement.frame(),
            measurement.stations().clone(),
            predicted_velocity(self.path_range_rate(
                measurement.path(),
                epoch,
                measurement.frame(),
            )?)?,
        ))
    }
}

#[cfg(feature = "geometric-turnaround-range")]
impl<P: ParticipantStateProvider> MeasurementEstimator<TurnaroundRangeMeasurement>
    for GeometricEstimator<P>
{
    type Error = GeometricEstimationError<P::Error>;

    fn predict(
        &self,
        measurement: &TurnaroundRangeMeasurement,
        epoch: Epoch,
    ) -> Result<TurnaroundRangeMeasurement, Self::Error> {
        Ok(TurnaroundRangeMeasurement::new(
            measurement.path().clone(),
            epoch,
            measurement.frame(),
            measurement.stations().clone(),
            predicted_length(self.path_length(measurement.path(), epoch, measurement.frame())?)?,
        ))
    }
}

#[cfg(feature = "geometric-tdoa")]
impl<P: ParticipantStateProvider> MeasurementEstimator<TdoaMeasurement> for GeometricEstimator<P> {
    type Error = GeometricEstimationError<P::Error>;

    fn predict(
        &self,
        measurement: &TdoaMeasurement,
        epoch: Epoch,
    ) -> Result<TdoaMeasurement, Self::Error> {
        let (source, primary) = self.direct_reception_states(
            measurement.path(),
            measurement.stations().primary(),
            epoch,
            measurement.frame(),
        )?;
        let secondary = self.state(
            measurement.stations().secondary(),
            epoch,
            measurement.frame(),
        )?;
        let (primary_range, _) = self.direct_range(source, primary, 0)?;
        let (secondary_range, _) = self.direct_range(source, secondary, 1)?;
        let seconds = (primary_range - secondary_range) / speed_of_light().get::<velocity_unit>();
        Ok(TdoaMeasurement::new(
            measurement.path().clone(),
            epoch,
            measurement.frame(),
            measurement.stations().clone(),
            predicted_time(seconds)?,
        ))
    }
}

#[cfg(feature = "geometric-doppler")]
impl<P: ParticipantStateProvider> MeasurementEstimator<DopplerMeasurement>
    for CarrierGeometricEstimator<P>
{
    type Error = GeometricEstimationError<P::Error>;

    fn predict(
        &self,
        measurement: &DopplerMeasurement,
        epoch: Epoch,
    ) -> Result<DopplerMeasurement, Self::Error> {
        let carrier = self.carrier_frequency.get::<hertz>();
        if !carrier.is_finite() || carrier <= 0.0 {
            return Err(GeometricEstimationError::InvalidCarrierFrequency);
        }
        let shift = -carrier
            * self
                .geometry
                .path_range_rate(measurement.path(), epoch, measurement.frame())?
            / speed_of_light().get::<velocity_unit>();
        Ok(DopplerMeasurement::new(
            measurement.path().clone(),
            epoch,
            measurement.frame(),
            predicted_frequency(shift)?,
        ))
    }
}

#[cfg(feature = "geometric-fdoa")]
impl<P: ParticipantStateProvider> MeasurementEstimator<FdoaMeasurement> for GeometricEstimator<P> {
    type Error = GeometricEstimationError<P::Error>;

    fn predict(
        &self,
        measurement: &FdoaMeasurement,
        epoch: Epoch,
    ) -> Result<FdoaMeasurement, Self::Error> {
        let carrier = measurement.emitter_frequency().get::<hertz>();
        if !carrier.is_finite() || carrier <= 0.0 {
            return Err(GeometricEstimationError::InvalidCarrierFrequency);
        }
        let (source, primary) = self.direct_reception_states(
            measurement.path(),
            measurement.stations().primary(),
            epoch,
            measurement.frame(),
        )?;
        let secondary = self.state(
            measurement.stations().secondary(),
            epoch,
            measurement.frame(),
        )?;
        let (_, primary_rate) = self.direct_range(source, primary, 0)?;
        let (_, secondary_rate) = self.direct_range(source, secondary, 1)?;
        let difference =
            -carrier * (primary_rate - secondary_rate) / speed_of_light().get::<velocity_unit>();
        Ok(FdoaMeasurement::new(
            measurement.path().clone(),
            epoch,
            measurement.frame(),
            measurement.stations().clone(),
            measurement.emitter_frequency(),
            predicted_frequency(difference)?,
        ))
    }
}

#[cfg(feature = "geometric-phase")]
impl<P: ParticipantStateProvider> MeasurementEstimator<PhaseMeasurement> for GeometricEstimator<P> {
    type Error = GeometricEstimationError<P::Error>;

    fn predict(
        &self,
        measurement: &PhaseMeasurement,
        epoch: Epoch,
    ) -> Result<PhaseMeasurement, Self::Error> {
        let carrier = measurement.carrier_frequency().get::<hertz>();
        if !carrier.is_finite() || carrier <= 0.0 {
            return Err(GeometricEstimationError::InvalidCarrierFrequency);
        }
        let (_, receiver) = self.direct_reception_states(
            measurement.path(),
            measurement.receiver(),
            epoch,
            measurement.frame(),
        )?;
        let _ = receiver;
        let phase = (self.path_length(measurement.path(), epoch, measurement.frame())? * carrier
            / speed_of_light().get::<velocity_unit>()
            * std::f64::consts::TAU)
            .rem_euclid(std::f64::consts::TAU);
        Ok(PhaseMeasurement::new(
            measurement.path().clone(),
            epoch,
            measurement.frame(),
            measurement.receiver().clone(),
            measurement.carrier_frequency(),
            predicted_angle(phase)?,
        ))
    }
}

#[cfg(any(
    feature = "geometric-range",
    feature = "geometric-range-rate",
    feature = "geometric-azimuth-elevation",
    feature = "geometric-doppler",
    feature = "geometric-angular-ra-dec",
    feature = "geometric-bistatic-range",
    feature = "geometric-bistatic-range-rate",
    feature = "geometric-turnaround-range",
    feature = "geometric-tdoa",
    feature = "geometric-fdoa",
    feature = "geometric-phase",
    feature = "light-time"
))]
fn dot(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0].mul_add(right[0], left[1].mul_add(right[1], left[2] * right[2]))
}

#[cfg(feature = "geometric-azimuth-elevation")]
fn cross(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [
        left[1].mul_add(right[2], -left[2] * right[1]),
        left[2].mul_add(right[0], -left[0] * right[2]),
        left[0].mul_add(right[1], -left[1] * right[0]),
    ]
}

#[cfg(feature = "geometric-azimuth-elevation")]
fn normalize(values: [f64; 3]) -> Option<[f64; 3]> {
    let norm = dot(values, values).sqrt();
    (norm.is_finite() && norm > 0.0).then(|| values.map(|value| value / norm))
}

#[cfg(test)]
mod tests {
    use super::*;
    use frames::{FrameCatalog, FrameKinematics, FrameNamespace, KinematicFrameTransformProvider};
    use units::uom::si::length::meter;

    fn id(value: &str) -> ParticipantId {
        value.parse().expect("participant ID")
    }

    #[test]
    fn fixed_station_provider_is_explicit_about_its_parent_frame() {
        let frame = FrameCatalog::new(FrameNamespace::new(91), [ReferenceFrame::ITRF2020])
            .expect("catalog")
            .define_parent_aligned(
                1,
                ReferenceFrame::ITRF2020,
                Position::from_metres(6_378_137.0, 0.0, 0.0),
            )
            .expect("finite station frame");
        let provider = GroundStationProvider::new([GroundStation::new(id("DSS-14"), frame)])
            .expect("one station");
        let state = provider
            .state_at(
                &id("DSS-14"),
                Epoch::from_tai_seconds(0.0),
                ReferenceFrame::ITRF2020,
            )
            .expect("station parent frame")
            .expect("station exists");
        assert_eq!(
            state.position().x(),
            units::Length::new::<meter>(6_378_137.0)
        );
        assert!(matches!(
            provider.state_at(
                &id("DSS-14"),
                Epoch::from_tai_seconds(0.0),
                ReferenceFrame::GCRF
            ),
            Err(GroundStationProviderError::FrameMismatch { .. })
        ));
    }

    #[cfg(feature = "light-time")]
    #[test]
    fn transforming_provider_requires_and_verifies_an_explicit_target_frame() {
        #[derive(Debug)]
        struct TestTransform;

        impl KinematicFrameTransformProvider for TestTransform {
            type Error = std::convert::Infallible;

            fn transform(
                &self,
                epoch: Epoch,
                state: FrameKinematics,
                target: ReferenceFrame,
            ) -> Result<FrameKinematics, Self::Error> {
                assert_eq!(epoch, Epoch::from_tai_seconds(20.0));
                let position = state.position().to_metres();
                Ok(FrameKinematics::new(
                    Position::from_metres(position[0] + 100.0, position[1], position[2]),
                    state.velocity(),
                    target,
                )
                .expect("finite transformed state"))
            }
        }

        let frame = FrameCatalog::new(FrameNamespace::new(93), [ReferenceFrame::ITRF2020])
            .expect("catalog")
            .define_parent_aligned(
                1,
                ReferenceFrame::ITRF2020,
                Position::from_metres(6_378_137.0, 0.0, 0.0),
            )
            .expect("station frame");
        let source = GroundStationProvider::new([GroundStation::new(id("DSS-14"), frame)])
            .expect("station provider");
        let provider = TransformingParticipantStateProvider::new(
            source,
            ReferenceFrame::ITRF2020,
            TestTransform,
        );

        let transformed = provider
            .state_at(
                &id("DSS-14"),
                Epoch::from_tai_seconds(20.0),
                ReferenceFrame::GCRF,
            )
            .expect("explicit transform")
            .expect("station state");
        assert_eq!(transformed.frame(), ReferenceFrame::GCRF);
        assert_eq!(
            transformed.position().x(),
            units::Length::new::<meter>(6_378_237.0)
        );
    }

    #[cfg(all(feature = "light-time", feature = "range"))]
    #[test]
    fn vacuum_light_time_solves_moving_emitter_and_preserves_reported_epoch() {
        use std::convert::Infallible;

        use crate::{
            MeasurementCorrectionModel, SignalPropagationGradient, SignalPropagationState,
        };

        #[derive(Debug)]
        struct LinearProvider {
            emitter: ParticipantId,
            receiver: ParticipantId,
        }

        impl ParticipantStateProvider for LinearProvider {
            type Error = Infallible;

            fn state_at(
                &self,
                participant: &ParticipantId,
                epoch: Epoch,
                frame: ReferenceFrame,
            ) -> Result<Option<ParticipantKinematics>, Self::Error> {
                let elapsed = (epoch - Epoch::from_tai_seconds(1_000.0)).to_seconds();
                let state = if participant == &self.emitter {
                    Some(
                        ParticipantKinematics::new(
                            Position::from_metres(299_792_458.0 + 10.0 * elapsed, 0.0, 0.0),
                            VelocityVector::from_metres_per_second(10.0, 0.0, 0.0),
                            frame,
                        )
                        .expect("finite emitter"),
                    )
                } else if participant == &self.receiver {
                    Some(
                        ParticipantKinematics::new(
                            Position::from_metres(0.0, 0.0, 0.0),
                            VelocityVector::from_metres_per_second(0.0, 0.0, 0.0),
                            frame,
                        )
                        .expect("finite receiver"),
                    )
                } else {
                    None
                };
                Ok(state)
            }
        }

        let emitter = id("SC-01");
        let receiver = id("DSS-14");
        let epoch = Epoch::from_tai_seconds(1_000.0);
        let measurement = RangeMeasurement::new(
            SignalPath::new(vec![emitter.clone(), receiver.clone()]).expect("path"),
            epoch,
            ReferenceFrame::GCRF,
            RangeConvention::PathLength,
            Measured::new([units::Length::new::<meter>(0.0)], None).expect("range value"),
        )
        .expect("range");
        let participants = LinearProvider { emitter, receiver };
        let solver = VacuumLightTimeSolver::new(participants);
        let corrections = CorrectionModelChain::<RangeMeasurement, ()>::new();
        let timeline = solver
            .solve_timing(&measurement, &corrections, &())
            .expect("light-time solution");

        assert_eq!(timeline.observation_epoch(), epoch);
        assert_eq!(timeline.event_epoch(1), Some(epoch));
        let emission = timeline.event_epoch(0).expect("emission event");
        assert!(emission < epoch);
        assert!(((epoch - emission).to_seconds() - 0.999_999_967).abs() < 1.0e-9);

        let non_convergent = VacuumLightTimeSolver::new(LinearProvider {
            emitter: id("SC-01"),
            receiver: id("DSS-14"),
        })
        .with_convergence(1, Duration::from_seconds(1.0e-9))
        .expect("configuration")
        .solve_timing(&measurement, &corrections, &())
        .expect_err("one iteration cannot converge moving-emitter solution");
        assert!(matches!(
            non_convergent.downcast_ref::<VacuumLightTimeError<Infallible>>(),
            Some(VacuumLightTimeError::NonConvergent {
                leg: 0,
                iterations: 1
            })
        ));

        #[derive(Debug)]
        struct DoubleVacuumSlowness;

        impl MeasurementCorrectionModel<RangeMeasurement, ()> for DoubleVacuumSlowness {
            fn propagation_gradient(
                &self,
                _measurement: &RangeMeasurement,
                _state: SignalPropagationState,
                _conditions: &(),
            ) -> Result<SignalPropagationGradient, CorrectionModelError> {
                Ok(SignalPropagationGradient::new(
                    units::Time::new::<second>(1.0) / units::Length::new::<meter>(299_792_458.0),
                ))
            }

            fn apply_model(
                &self,
                measurement: RangeMeasurement,
                _timeline: &SignalEventTimeline,
                _conditions: &(),
            ) -> Result<RangeMeasurement, CorrectionModelError> {
                Ok(measurement)
            }
        }

        let mut delayed_corrections = CorrectionModelChain::new();
        delayed_corrections.push(Box::new(DoubleVacuumSlowness));
        let delayed = VacuumLightTimeSolver::new(LinearProvider {
            emitter: id("SC-01"),
            receiver: id("DSS-14"),
        })
        .solve_timing(&measurement, &delayed_corrections, &())
        .expect("corrected light-time solution");
        assert!(
            (epoch - delayed.event_epoch(0).expect("emission event")).to_seconds() > 1.9,
            "one added vacuum slowness should approximately double this one-second path delay"
        );
    }

    #[cfg(all(
        feature = "geometric-range",
        feature = "geometric-range-rate",
        feature = "geometric-azimuth-elevation",
        feature = "geometric-doppler",
        feature = "geometric-angular-ra-dec",
        feature = "geometric-bistatic-range",
        feature = "geometric-bistatic-range-rate",
        feature = "geometric-turnaround-range",
        feature = "geometric-tdoa",
        feature = "geometric-fdoa",
        feature = "geometric-phase"
    ))]
    mod predictions {
        use std::convert::Infallible;

        use hifitime::Duration;
        use units::uom::si::{
            angle::radian, frequency::hertz, length::meter, time::second,
            velocity::meter_per_second,
        };
        use units::{Angle, Frequency, Length, Time, Velocity};

        use super::*;
        use crate::{
            GroundStationPair, MeasurementCorrectionModel, RightAscensionDeclinationConvention,
            SignalPath, SignalPropagationGradient, SignalPropagationState,
        };

        #[derive(Debug, Clone)]
        struct SpacecraftProvider {
            id: ParticipantId,
            state: ParticipantKinematics,
        }

        impl ParticipantStateProvider for SpacecraftProvider {
            type Error = Infallible;

            fn state_at(
                &self,
                participant: &ParticipantId,
                epoch: Epoch,
                _frame: ReferenceFrame,
            ) -> Result<Option<ParticipantKinematics>, Self::Error> {
                let elapsed = (epoch - Epoch::from_tai_seconds(123.0)).to_seconds();
                let position = self.state.position().to_metres();
                let velocity = self.state.velocity().to_metres_per_second();
                let propagated = ParticipantKinematics::new(
                    Position::from_metres(
                        position[0] + elapsed * velocity[0],
                        position[1] + elapsed * velocity[1],
                        position[2] + elapsed * velocity[2],
                    ),
                    self.state.velocity(),
                    self.state.frame(),
                )
                .expect("finite spacecraft state");
                Ok((participant == &self.id).then_some(propagated))
            }
        }

        fn path(values: &[ParticipantId]) -> SignalPath {
            SignalPath::new(values.to_vec()).expect("signal path")
        }

        fn scalar_length(value: f64) -> Measured<Length> {
            Measured::new([Length::new::<meter>(value)], None).expect("length")
        }

        fn scalar_velocity(value: f64) -> Measured<Velocity> {
            Measured::new([Velocity::new::<meter_per_second>(value)], None).expect("velocity")
        }

        fn scalar_frequency(value: f64) -> Measured<Frequency> {
            Measured::new([Frequency::new::<hertz>(value)], None).expect("frequency")
        }

        fn scalar_time(value: f64) -> Measured<Time> {
            Measured::new([Time::new::<second>(value)], None).expect("time")
        }

        fn scalar_angle(value: f64) -> Measured<Angle> {
            Measured::new([Angle::new::<radian>(value)], None).expect("angle")
        }

        fn angles() -> MeasurementValues<Angle, 2> {
            MeasurementValues::new([Angle::new::<radian>(0.0), Angle::new::<radian>(0.0)], None)
                .expect("angles")
        }

        #[derive(Debug)]
        struct PropagationDelay(SignalPropagationGradient);

        impl MeasurementCorrectionModel<RangeMeasurement, ()> for PropagationDelay {
            fn propagation_gradient(
                &self,
                _measurement: &RangeMeasurement,
                _state: SignalPropagationState,
                _conditions: &(),
            ) -> Result<SignalPropagationGradient, CorrectionModelError> {
                Ok(self.0)
            }

            fn apply_model(
                &self,
                measurement: RangeMeasurement,
                _timeline: &SignalEventTimeline,
                _conditions: &(),
            ) -> Result<RangeMeasurement, CorrectionModelError> {
                Ok(measurement)
            }
        }

        #[derive(Debug)]
        struct ConstantGradientPropagation {
            state: SignalPropagationState,
            path_length: Length,
        }

        impl SignalPropagationSolver<RangeMeasurement, ()> for ConstantGradientPropagation {
            fn solve_timing(
                &self,
                measurement: &RangeMeasurement,
                corrections: &CorrectionModelChain<RangeMeasurement, ()>,
                conditions: &(),
            ) -> Result<SignalEventTimeline, SignalPropagationError> {
                let gradient =
                    corrections.propagation_gradient(measurement, self.state, conditions)?;
                let delay = gradient.excess_slowness() * self.path_length;
                let duration = Duration::from_seconds(delay.get::<second>());
                let event_epochs = measurement
                    .path()
                    .participants()
                    .iter()
                    .enumerate()
                    .map(|(index, _)| {
                        if index + 1 == measurement.path().participant_count() {
                            measurement.epoch()
                        } else {
                            measurement.epoch() - duration
                        }
                    })
                    .collect();
                SignalEventTimeline::instantaneous(measurement)
                    .with_event_epochs(event_epochs)
                    .map_err(|error| Box::new(error) as SignalPropagationError)
            }
        }

        #[test]
        fn geometric_estimators_join_ground_stations_and_spacecraft_for_every_observable() {
            let primary = id("DSS-14");
            let secondary = id("DSS-25");
            let spacecraft = id("SC-01");
            let mut catalog =
                FrameCatalog::new(FrameNamespace::new(92), [ReferenceFrame::ITRF2020])
                    .expect("catalog");
            let primary_frame = catalog
                .define_parent_aligned(
                    1,
                    ReferenceFrame::ITRF2020,
                    Position::from_metres(6_378_137.0, 0.0, 0.0),
                )
                .expect("primary frame");
            let secondary_frame = catalog
                .define_parent_aligned(
                    2,
                    ReferenceFrame::ITRF2020,
                    Position::from_metres(0.0, 6_378_137.0, 0.0),
                )
                .expect("secondary frame");
            let stations = GroundStationProvider::new([
                GroundStation::new(primary.clone(), primary_frame),
                GroundStation::new(secondary.clone(), secondary_frame),
            ])
            .expect("distinct stations");
            let spacecraft_provider = SpacecraftProvider {
                id: spacecraft.clone(),
                state: ParticipantKinematics::new(
                    Position::from_metres(7_000_000.0, 100_000.0, 1_000.0),
                    VelocityVector::from_metres_per_second(0.0, 50.0, 0.0),
                    ReferenceFrame::ITRF2020,
                )
                .expect("spacecraft state"),
            };
            let estimator = GeometricEstimator::new(CompositeParticipantStateProvider::new(
                stations,
                spacecraft_provider,
            ));
            let epoch = Epoch::from_tai_seconds(123.0);
            let downlink = path(&[spacecraft.clone(), primary.clone()]);
            let bistatic = path(&[primary.clone(), spacecraft.clone(), secondary.clone()]);
            let pair = GroundStationPair::new(primary.clone(), secondary.clone()).expect("pair");

            let range = RangeMeasurement::new(
                downlink.clone(),
                epoch,
                ReferenceFrame::ITRF2020,
                RangeConvention::PathLength,
                scalar_length(0.0),
            )
            .expect("range");
            let predicted_range = estimator.predict(&range, epoch).expect("predicted range");
            assert_eq!(predicted_range.epoch(), epoch);
            assert!(predicted_range.value().value().get::<meter>() > 0.0);
            assert_eq!(predicted_range.value().error(), None);
            let reverse_range = RangeMeasurement::new(
                path(&[primary.clone(), spacecraft.clone()]),
                epoch,
                ReferenceFrame::ITRF2020,
                RangeConvention::PathLength,
                scalar_length(0.0),
            )
            .expect("reverse range");
            let predicted_reverse = estimator
                .predict(&reverse_range, epoch)
                .expect("predicted reverse range");
            assert!(
                (predicted_reverse.value().value().get::<meter>()
                    - predicted_range.value().value().get::<meter>())
                .abs()
                    <= 1.0e-9,
                "instantaneous one-leg geometric range must be symmetric"
            );
            let propagation_solver = ConstantGradientPropagation {
                state: SignalPropagationState::new(
                    epoch,
                    Position::from_metres(7_000_000.0, 100_000.0, 1_000.0),
                    ReferenceFrame::ITRF2020,
                )
                .expect("finite propagation state"),
                path_length: Length::new::<meter>(0.0),
            };
            let correction_models = CorrectionModelChain::<RangeMeasurement, ()>::new();
            let unmodified = estimator
                .predict_with_models(&range, &propagation_solver, &correction_models, &())
                .expect("prediction with empty model chains");
            assert_eq!(unmodified.observation_epoch(), epoch);
            assert_eq!(unmodified.measurement().epoch(), epoch);
            assert_eq!(unmodified.measurement(), &predicted_range);

            let delay = Time::new::<second>(2.5);
            let mut propagation_corrections = CorrectionModelChain::new();
            propagation_corrections.push(Box::new(PropagationDelay(
                SignalPropagationGradient::new(delay / Length::new::<meter>(1.0)),
            )));
            let delayed_propagation_solver = ConstantGradientPropagation {
                state: propagation_solver.state,
                path_length: Length::new::<meter>(1.0),
            };
            let delayed = estimator
                .predict_with_models(
                    &range,
                    &delayed_propagation_solver,
                    &propagation_corrections,
                    &(),
                )
                .expect("prediction at propagation-delayed epoch");
            assert_eq!(delayed.observation_epoch(), epoch);
            assert_eq!(delayed.measurement().epoch(), epoch);
            assert_ne!(
                delayed.measurement().value().value(),
                predicted_range.value().value()
            );
            assert_eq!(
                delayed.timeline().event_epochs(),
                &[epoch - Duration::from_seconds(delay.get::<second>()), epoch]
            );

            let range_rate = RangeRateMeasurement::new(
                downlink.clone(),
                Epoch::from_tai_seconds(0.0),
                ReferenceFrame::ITRF2020,
                scalar_velocity(0.0),
            );
            assert!(estimator
                .predict(&range_rate, epoch)
                .expect("predicted range rate")
                .value()
                .value()
                .get::<meter_per_second>()
                .is_finite());

            let azimuth_elevation = AzimuthElevationMeasurement::new(
                downlink.clone(),
                Epoch::from_tai_seconds(0.0),
                ReferenceFrame::ITRF2020,
                AzimuthElevationConvention::ClockwiseFromNorthAboveHorizon,
                angles(),
            )
            .expect("angles");
            assert_eq!(
                estimator
                    .predict(&azimuth_elevation, epoch)
                    .expect("predicted azimuth/elevation")
                    .values()
                    .error(),
                None
            );

            let right_ascension_declination = RightAscensionDeclinationMeasurement::new(
                downlink.clone(),
                Epoch::from_tai_seconds(0.0),
                ReferenceFrame::ITRF2020,
                RightAscensionDeclinationConvention::Equatorial,
                angles(),
            )
            .expect("right ascension/declination");
            assert_eq!(
                estimator
                    .predict(&right_ascension_declination, epoch)
                    .expect("predicted right ascension/declination")
                    .values()
                    .error(),
                None
            );

            let bistatic_range = BistaticRangeMeasurement::new(
                bistatic.clone(),
                Epoch::from_tai_seconds(0.0),
                ReferenceFrame::ITRF2020,
                pair.clone(),
                scalar_length(0.0),
            );
            assert!(
                estimator
                    .predict(&bistatic_range, epoch)
                    .expect("predicted bistatic range")
                    .value()
                    .value()
                    .get::<meter>()
                    > 0.0
            );

            let bistatic_range_rate = BistaticRangeRateMeasurement::new(
                bistatic.clone(),
                Epoch::from_tai_seconds(0.0),
                ReferenceFrame::ITRF2020,
                pair.clone(),
                scalar_velocity(0.0),
            );
            assert!(estimator
                .predict(&bistatic_range_rate, epoch)
                .expect("predicted bistatic range rate")
                .value()
                .value()
                .get::<meter_per_second>()
                .is_finite());

            let turnaround = TurnaroundRangeMeasurement::new(
                bistatic.clone(),
                Epoch::from_tai_seconds(0.0),
                ReferenceFrame::ITRF2020,
                pair.clone(),
                scalar_length(0.0),
            );
            assert!(
                estimator
                    .predict(&turnaround, epoch)
                    .expect("predicted turnaround range")
                    .value()
                    .value()
                    .get::<meter>()
                    > 0.0
            );

            let tdoa = TdoaMeasurement::new(
                downlink.clone(),
                Epoch::from_tai_seconds(0.0),
                ReferenceFrame::ITRF2020,
                pair.clone(),
                scalar_time(0.0),
            );
            assert!(estimator
                .predict(&tdoa, epoch)
                .expect("predicted TDOA")
                .value()
                .value()
                .get::<second>()
                .is_finite());

            let fdoa = FdoaMeasurement::new(
                downlink.clone(),
                Epoch::from_tai_seconds(0.0),
                ReferenceFrame::ITRF2020,
                pair.clone(),
                Frequency::new::<hertz>(8.4e9),
                scalar_frequency(0.0),
            );
            assert!(estimator
                .predict(&fdoa, epoch)
                .expect("predicted FDOA")
                .value()
                .value()
                .get::<hertz>()
                .is_finite());

            let phase = PhaseMeasurement::new(
                downlink.clone(),
                Epoch::from_tai_seconds(0.0),
                ReferenceFrame::ITRF2020,
                primary.clone(),
                Frequency::new::<hertz>(8.4e9),
                scalar_angle(0.0),
            );
            let predicted_phase = estimator.predict(&phase, epoch).expect("predicted phase");
            assert!((0.0..std::f64::consts::TAU)
                .contains(&predicted_phase.value().value().get::<radian>()));

            let doppler = DopplerMeasurement::new(
                downlink,
                Epoch::from_tai_seconds(0.0),
                ReferenceFrame::ITRF2020,
                scalar_frequency(0.0),
            );
            let carrier_estimator =
                estimator.with_carrier_frequency(Frequency::new::<hertz>(8.4e9));
            assert!(carrier_estimator
                .predict(&doppler, epoch)
                .expect("predicted Doppler")
                .value()
                .value()
                .get::<hertz>()
                .is_finite());
        }
    }
}
