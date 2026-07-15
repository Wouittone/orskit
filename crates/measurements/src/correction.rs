//! Typed additive measurement corrections and their composition.

use std::{error::Error as StdError, fmt, marker::PhantomData, ops::Div};

use frames::ReferenceFrame;
use thiserror::Error;
use units::uom::si::{length::meter, time::second};
#[cfg(feature = "azimuth-elevation")]
use units::Angle;
#[cfg(feature = "doppler")]
use units::Frequency;
#[cfg(feature = "range-rate")]
use units::Velocity;
use units::{Length, Position, Time};

#[cfg(feature = "doppler")]
use crate::DopplerMeasurement;
use crate::Measurement;
#[cfg(feature = "range")]
use crate::RangeMeasurement;
#[cfg(feature = "range-rate")]
use crate::RangeRateMeasurement;
#[cfg(feature = "azimuth-elevation")]
use crate::{AzimuthElevationMeasurement, MeasurementValues};
use crate::{Measured, MeasurementError, MeasurementQuantity, MeasurementValueError};

/// Signal-event timeline derived for one measurement evaluation.
///
/// `observation_epoch` is the immutable epoch at which the observation was
/// reported, normally a ground-reception event. `event_epochs` records the
/// separately solved emission, transit, reflection, and reception events in
/// signal-path order.
#[derive(Debug, Clone, PartialEq)]
pub struct SignalEventTimeline {
    observation_epoch: hifitime::Epoch,
    event_epochs: Vec<hifitime::Epoch>,
}

impl SignalEventTimeline {
    /// Creates an instantaneous timeline from the measurement's reported epoch.
    /// Every signal event and prediction initially use that epoch.
    #[must_use]
    pub fn instantaneous<M: Measurement>(measurement: &M) -> Self {
        let observation_epoch = measurement.epoch();
        Self {
            observation_epoch,
            event_epochs: vec![observation_epoch; measurement.path().participant_count()],
        }
    }

    /// Returns the immutable epoch at which the observation was reported.
    #[must_use]
    pub const fn observation_epoch(&self) -> hifitime::Epoch {
        self.observation_epoch
    }

    /// Returns the resolved epoch for every signal-path event.
    #[must_use]
    pub fn event_epochs(&self) -> &[hifitime::Epoch] {
        &self.event_epochs
    }

    /// Returns one signal event epoch by its path-event index.
    #[must_use]
    pub fn event_epoch(&self, index: usize) -> Option<hifitime::Epoch> {
        self.event_epochs.get(index).copied()
    }

    /// Replaces every signal-event epoch after validating path-event count.
    pub fn with_event_epochs(
        mut self,
        event_epochs: Vec<hifitime::Epoch>,
    ) -> Result<Self, SignalEventTimelineError> {
        if event_epochs.len() != self.event_epochs.len() {
            return Err(SignalEventTimelineError::EventCountMismatch {
                expected: self.event_epochs.len(),
                actual: event_epochs.len(),
            });
        }
        if let Some(index) = event_epochs
            .windows(2)
            .position(|events| events[1] < events[0])
        {
            return Err(SignalEventTimelineError::NonMonotonicEventEpoch {
                previous: index,
                current: index + 1,
            });
        }
        self.event_epochs = event_epochs;
        Ok(self)
    }
}

/// Invalid signal-event timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum SignalEventTimelineError {
    /// A propagation solver supplied a different number of epochs than signal events.
    #[error("propagation solver supplied {actual} signal-event epochs; expected {expected}")]
    EventCountMismatch {
        /// Signal-event count from the measurement path.
        expected: usize,
        /// Epochs supplied by the propagation solver.
        actual: usize,
    },
    /// Signal events are not in chronological path order.
    #[error("signal event {current} precedes event {previous}")]
    NonMonotonicEventEpoch {
        /// Index of the earlier path event.
        previous: usize,
        /// Index of the following path event that incorrectly precedes it.
        current: usize,
    },
}

/// Unit-qualified local excess light-time rate, `d(delay) / d(path length)`.
pub type SignalPropagationSlowness = <Time as Div<Length>>::Output;

/// A location in the signal-propagation differential equation.
///
/// A propagation solver evaluates correction fields at these explicit
/// spacetime points while integrating each signal-path leg.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SignalPropagationState {
    epoch: hifitime::Epoch,
    position: Position,
    frame: ReferenceFrame,
}

impl SignalPropagationState {
    /// Creates a finite, frame-qualified propagation state.
    pub fn new(
        epoch: hifitime::Epoch,
        position: Position,
        frame: ReferenceFrame,
    ) -> Result<Self, SignalPropagationStateError> {
        if !position.is_finite() {
            return Err(SignalPropagationStateError::NonFinitePosition);
        }
        Ok(Self {
            epoch,
            position,
            frame,
        })
    }

    /// Returns the epoch at this integration point.
    #[must_use]
    pub const fn epoch(self) -> hifitime::Epoch {
        self.epoch
    }

    /// Returns the spatial point at this integration point.
    #[must_use]
    pub const fn position(self) -> Position {
        self.position
    }

    /// Returns the frame in which [`Self::position`] is expressed.
    #[must_use]
    pub const fn frame(self) -> ReferenceFrame {
        self.frame
    }
}

/// Invalid input to a signal-propagation field evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum SignalPropagationStateError {
    /// The spatial integration point has a non-finite coordinate.
    #[error("signal-propagation position must be finite")]
    NonFinitePosition,
}

/// A correction contribution to the signal-propagation differential equation.
///
/// The contained excess slowness is added to other correction contributions by
/// [`CorrectionModelChain`]. A propagation solver integrates the resulting
/// field over a signal leg; this type never sets an epoch directly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SignalPropagationGradient {
    excess_slowness: SignalPropagationSlowness,
}

impl SignalPropagationGradient {
    /// Creates one unit-qualified excess light-time rate.
    #[must_use]
    pub const fn new(excess_slowness: SignalPropagationSlowness) -> Self {
        Self { excess_slowness }
    }

    /// Returns no propagation contribution.
    #[must_use]
    pub fn zero() -> Self {
        Self::new(Time::new::<second>(0.0) / Length::new::<meter>(1.0))
    }

    /// Returns the excess light-time rate, in time per path length.
    #[must_use]
    pub const fn excess_slowness(self) -> SignalPropagationSlowness {
        self.excess_slowness
    }

    fn combined(self, other: Self) -> Self {
        Self::new(self.excess_slowness + other.excess_slowness)
    }
}

/// Physical provenance of a measurement correction.
///
/// This is deliberately an open trait. Applications can define their own
/// provenance marker and implement this trait, rather than being limited to a
/// crate-owned enumeration.
pub trait CorrectionKind: fmt::Debug + Send + Sync + 'static {}

#[cfg(any(
    feature = "clock-correction",
    feature = "troposphere-correction",
    feature = "ionosphere-correction",
    feature = "relativity-correction",
    feature = "instrument-correction"
))]
macro_rules! correction_kind {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name;

        impl CorrectionKind for $name {}
    };
}

#[cfg(feature = "clock-correction")]
correction_kind!(
    /// A station or spacecraft clock contribution.
    Clock
);
#[cfg(feature = "troposphere-correction")]
correction_kind!(
    /// A neutral-atmosphere contribution.
    Troposphere
);
#[cfg(feature = "ionosphere-correction")]
correction_kind!(
    /// An ionospheric contribution.
    Ionosphere
);
#[cfg(feature = "relativity-correction")]
correction_kind!(
    /// A relativistic signal-propagation contribution.
    Relativity
);
#[cfg(feature = "instrument-correction")]
correction_kind!(
    /// An antenna, transponder, or receiver contribution.
    Instrument
);

/// One unit-qualified additive correction and its uncertainty state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AdditiveCorrection<K: CorrectionKind, Q: MeasurementQuantity> {
    kind: K,
    value: Measured<Q>,
}

impl<K: CorrectionKind, Q: MeasurementQuantity> AdditiveCorrection<K, Q> {
    /// Creates an additive correction. Its sign is applied algebraically.
    #[must_use]
    pub const fn new(kind: K, value: Measured<Q>) -> Self {
        Self { kind, value }
    }

    /// Returns the correction's physical provenance category.
    #[must_use]
    pub const fn kind(&self) -> &K {
        &self.kind
    }

    /// Returns the unit-qualified correction and uncertainty state.
    #[must_use]
    pub const fn value(&self) -> Measured<Q> {
        self.value
    }
}

/// A correction applicable to one concrete measurement implementation.
///
/// Generic composition keeps corrections dimension-safe: a range correction
/// cannot be accidentally added to Doppler or angle data. Trait objects such
/// as `Box<dyn MeasurementCorrection<RangeMeasurement>>` compose heterogeneous
/// correction models for one observable family.
pub trait MeasurementCorrection<M>: fmt::Debug + Send + Sync {
    /// Applies this correction while retaining the observation metadata.
    fn apply(&self, measurement: &M) -> Result<M, CorrectionError>;
}

/// An epoch- and conditions-aware correction model for one observable type.
///
/// `C` is application-owned changing state such as weather, media parameters,
/// or an instrument calibration snapshot. Models may contribute to the
/// signal-propagation equation, then apply a value-domain correction.
pub trait MeasurementCorrectionModel<M, C: ?Sized>: fmt::Debug + Send + Sync {
    /// Evaluates this model's local signal-propagation contribution.
    ///
    /// The solver integrates these gradients over each leg to derive event
    /// epochs. The default has no propagation effect, so ordinary value-only
    /// corrections need not implement it.
    fn propagation_gradient(
        &self,
        _measurement: &M,
        _state: SignalPropagationState,
        _conditions: &C,
    ) -> Result<SignalPropagationGradient, CorrectionModelError> {
        Ok(SignalPropagationGradient::zero())
    }

    /// Evaluates and applies this model to one predicted measurement.
    ///
    /// The boxed source preserves application-defined model failures while
    /// allowing an ordered chain to hold heterogeneous model implementations.
    fn apply_model(
        &self,
        measurement: &M,
        timeline: &SignalEventTimeline,
        conditions: &C,
    ) -> Result<M, CorrectionModelError>;
}

/// Erased source error from an application-defined correction model.
pub type CorrectionModelError = Box<dyn StdError + Send + Sync + 'static>;

impl<M, C: ?Sized, T: MeasurementCorrection<M>> MeasurementCorrectionModel<M, C> for T {
    fn apply_model(
        &self,
        measurement: &M,
        _timeline: &SignalEventTimeline,
        _conditions: &C,
    ) -> Result<M, CorrectionModelError> {
        MeasurementCorrection::apply(self, measurement).map_err(|error| Box::new(error) as _)
    }
}

/// An ordered chain of state-aware correction models for one observable type.
#[derive(Debug)]
pub struct CorrectionModelChain<M, C: ?Sized> {
    models: Vec<Box<dyn MeasurementCorrectionModel<M, C>>>,
    conditions: PhantomData<fn(&C)>,
}

impl<M, C: ?Sized> Default for CorrectionModelChain<M, C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M, C: ?Sized> CorrectionModelChain<M, C> {
    /// Creates an empty correction-model chain.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            models: Vec::new(),
            conditions: PhantomData,
        }
    }

    /// Appends a model, preserving the declared application order.
    pub fn push(&mut self, model: impl MeasurementCorrectionModel<M, C> + 'static) {
        self.models.push(Box::new(model));
    }

    /// Returns the number of models in the chain.
    #[must_use]
    pub fn len(&self) -> usize {
        self.models.len()
    }

    /// Returns whether this chain has no models.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.models.is_empty()
    }
}

impl<M, C: ?Sized> CorrectionModelChain<M, C> {
    /// Sums local propagation contributions from every correction model.
    pub fn propagation_gradient(
        &self,
        measurement: &M,
        state: SignalPropagationState,
        conditions: &C,
    ) -> Result<SignalPropagationGradient, CorrectionModelError> {
        self.models
            .iter()
            .try_fold(SignalPropagationGradient::zero(), |gradient, model| {
                model
                    .propagation_gradient(measurement, state, conditions)
                    .map(|contribution| gradient.combined(contribution))
            })
    }
}

impl<M: Clone, C: ?Sized> CorrectionModelChain<M, C> {
    /// Applies every model in order using the solved event timeline and condition state.
    pub fn apply(
        &self,
        measurement: &M,
        timeline: &SignalEventTimeline,
        conditions: &C,
    ) -> Result<M, CorrectionModelError> {
        self.models
            .iter()
            .try_fold(measurement.clone(), |value, model| {
                model.apply_model(&value, timeline, conditions)
            })
    }
}

/// An ordered composition of corrections for one measurement implementation.
#[derive(Debug, Default)]
pub struct CorrectionChain<M> {
    corrections: Vec<Box<dyn MeasurementCorrection<M>>>,
}

impl<M> CorrectionChain<M> {
    /// Creates an empty correction chain.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            corrections: Vec::new(),
        }
    }

    /// Appends one correction, preserving application order.
    pub fn push(&mut self, correction: impl MeasurementCorrection<M> + 'static) {
        self.corrections.push(Box::new(correction));
    }

    /// Returns the number of composed corrections.
    #[must_use]
    pub fn len(&self) -> usize {
        self.corrections.len()
    }

    /// Returns whether no correction is configured.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.corrections.is_empty()
    }
}

impl<M: Clone> CorrectionChain<M> {
    /// Applies every correction in insertion order.
    pub fn apply(&self, measurement: &M) -> Result<M, CorrectionError> {
        self.corrections
            .iter()
            .try_fold(measurement.clone(), |value, correction| {
                correction.apply(&value)
            })
    }
}

#[cfg(any(feature = "range", feature = "range-rate", feature = "doppler"))]
macro_rules! scalar_correction {
    ($quantity:ty, $measurement:ty) => {
        impl<K: CorrectionKind> MeasurementCorrection<$measurement>
            for AdditiveCorrection<K, $quantity>
        {
            fn apply(&self, measurement: &$measurement) -> Result<$measurement, CorrectionError> {
                let value = measurement.value().corrected(self.value)?;
                measurement.with_value(value).map_err(Into::into)
            }
        }
    };
}

#[cfg(feature = "range")]
scalar_correction!(Length, RangeMeasurement);
#[cfg(feature = "range-rate")]
scalar_correction!(Velocity, RangeRateMeasurement);
#[cfg(feature = "doppler")]
scalar_correction!(Frequency, DopplerMeasurement);

/// A two-axis angular correction for an azimuth/elevation observation.
#[cfg(feature = "azimuth-elevation")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AzimuthElevationCorrection<K: CorrectionKind> {
    kind: K,
    values: MeasurementValues<Angle, 2>,
}

#[cfg(feature = "azimuth-elevation")]
impl<K: CorrectionKind> AzimuthElevationCorrection<K> {
    /// Creates a correction with azimuth/elevation values and their `2 × 2` covariance matrix.
    #[must_use]
    pub const fn new(kind: K, values: MeasurementValues<Angle, 2>) -> Self {
        Self { kind, values }
    }

    /// Returns the physical provenance category.
    #[must_use]
    pub const fn kind(&self) -> &K {
        &self.kind
    }
}

#[cfg(feature = "azimuth-elevation")]
impl<K: CorrectionKind> MeasurementCorrection<AzimuthElevationMeasurement>
    for AzimuthElevationCorrection<K>
{
    fn apply(
        &self,
        measurement: &AzimuthElevationMeasurement,
    ) -> Result<AzimuthElevationMeasurement, CorrectionError> {
        let values = (*measurement.values()).corrected(self.values)?;
        measurement.with_values(values).map_err(Into::into)
    }
}

/// Failure while applying a correction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum CorrectionError {
    /// Arithmetic produced a non-finite corrected value or uncertainty.
    #[error(transparent)]
    InvalidValue(#[from] MeasurementValueError),
    /// The corrected data no longer meet its observable's semantic contract.
    #[error(transparent)]
    InvalidMeasurement(#[from] MeasurementError),
}

#[cfg(all(
    test,
    feature = "range",
    feature = "range-rate",
    feature = "azimuth-elevation",
    feature = "doppler",
    feature = "clock-correction",
    feature = "troposphere-correction",
    feature = "instrument-correction"
))]
mod tests {
    use frames::ReferenceFrame;
    use hifitime::{Duration, Epoch};
    use units::uom::si::{
        angle::radian, frequency::hertz, length::meter, velocity::meter_per_second,
    };
    use units::{Angle, AngularVariance, Frequency, Length, Position, Time, Velocity};

    use super::*;
    use crate::{
        AzimuthElevationConvention, Measurement, MeasurementUncertaintyInput, MeasurementValues,
        ParticipantId, RangeConvention, SignalPath,
    };

    fn path() -> SignalPath {
        let station = ParticipantId::new("DSS-14").expect("station ID");
        let spacecraft = ParticipantId::new("SC-01").expect("spacecraft ID");
        SignalPath::new(vec![station, spacecraft]).expect("signal path")
    }

    fn length(value: f64, standard_deviation: Option<f64>) -> Measured<Length> {
        Measured::new(
            [Length::new::<meter>(value)],
            standard_deviation
                .map(|value| MeasurementUncertaintyInput::Scalar(Length::new::<meter>(value))),
        )
        .expect("finite length")
    }

    fn angles(
        azimuth: f64,
        elevation: f64,
        covariance: Option<[[f64; 2]; 2]>,
    ) -> MeasurementValues<Angle, 2> {
        MeasurementValues::new(
            [
                Angle::new::<radian>(azimuth),
                Angle::new::<radian>(elevation),
            ],
            covariance.map(|entries| {
                MeasurementUncertaintyInput::Covariance(
                    entries.map(|row| row.map(AngularVariance::from_square_radians)),
                )
            }),
        )
        .expect("angles")
    }

    #[test]
    fn correction_chain_preserves_units_and_combines_known_errors() {
        let range = RangeMeasurement::new(
            path(),
            Epoch::from_tai_seconds(0.0),
            ReferenceFrame::ITRF2020,
            RangeConvention::PathLength,
            length(100.0, Some(3.0)),
        )
        .expect("range");
        let mut corrections = CorrectionChain::new();
        corrections.push(AdditiveCorrection::new(Troposphere, length(2.0, Some(4.0))));

        let corrected = corrections.apply(&range).expect("corrected range");
        assert_eq!(corrected.path(), range.path());
        assert_eq!(corrected.epoch(), range.epoch());
        assert_eq!(corrected.frame(), range.frame());
        assert_eq!(corrected.value().value(), Length::new::<meter>(102.0));
        assert_eq!(
            corrected.value().standard_error(),
            Some(Length::new::<meter>(5.0))
        );
    }

    #[test]
    fn unknown_correction_error_remains_explicitly_unknown() {
        let range = RangeMeasurement::new(
            path(),
            Epoch::from_tai_seconds(0.0),
            ReferenceFrame::ITRF2020,
            RangeConvention::PathLength,
            length(100.0, Some(3.0)),
        )
        .expect("range");
        let correction = AdditiveCorrection::new(
            Clock,
            Measured::new([Length::new::<meter>(2.0)], None).expect("correction"),
        );

        let corrected = correction.apply(&range).expect("corrected range");
        assert_eq!(corrected.value().error(), None);
    }

    #[test]
    fn concrete_observables_share_only_the_object_safe_metadata_contract() {
        let range = RangeMeasurement::new(
            path(),
            Epoch::from_tai_seconds(0.0),
            ReferenceFrame::ITRF2020,
            RangeConvention::PathLength,
            length(1.0, None),
        )
        .expect("range");
        let range_rate = RangeRateMeasurement::new(
            path(),
            Epoch::from_tai_seconds(0.0),
            ReferenceFrame::ITRF2020,
            Measured::new(
                [Velocity::new::<meter_per_second>(1.0)],
                Some(MeasurementUncertaintyInput::Scalar(Velocity::new::<
                    meter_per_second,
                >(0.1))),
            )
            .expect("range rate"),
        );
        let doppler = DopplerMeasurement::new(
            path(),
            Epoch::from_tai_seconds(0.0),
            ReferenceFrame::ITRF2020,
            Measured::new(
                [Frequency::new::<hertz>(10.0)],
                Some(MeasurementUncertaintyInput::Scalar(
                    Frequency::new::<hertz>(0.1),
                )),
            )
            .expect("Doppler"),
        );
        let angle_measurement = AzimuthElevationMeasurement::new(
            path(),
            Epoch::from_tai_seconds(0.0),
            ReferenceFrame::ITRF2020,
            AzimuthElevationConvention::ClockwiseFromNorthAboveHorizon,
            angles(1.0, 0.5, Some([[0.1, 0.05], [0.05, 0.2]])),
        )
        .expect("angles");

        let measurements: [&dyn Measurement; 4] =
            [&range, &range_rate, &doppler, &angle_measurement];
        assert_eq!(
            measurements.map(|measurement| measurement.kind().name()),
            ["range", "range-rate", "doppler", "azimuth-elevation"]
        );
    }

    #[test]
    fn angular_correction_retains_only_the_combined_lower_triangular_matrix() {
        let measurement = AzimuthElevationMeasurement::new(
            path(),
            Epoch::from_tai_seconds(0.0),
            ReferenceFrame::ITRF2020,
            AzimuthElevationConvention::ClockwiseFromNorthAboveHorizon,
            angles(1.0, 0.5, Some([[1.0, 0.0], [0.0, 1.0]])),
        )
        .expect("angles");
        let correction = AzimuthElevationCorrection::new(
            Troposphere,
            angles(0.1, 0.0, Some([[3.0, 0.0], [0.0, 3.0]])),
        );

        let corrected = correction.apply(&measurement).expect("corrected angles");
        let error = corrected.values().error().expect("known covariance");
        let lower = error
            .lower_triangular_matrix()
            .expect("stored lower matrix");
        assert_eq!(lower[0][1], Angle::new::<radian>(0.0));
        assert_eq!(lower[1][0], Angle::new::<radian>(0.0));
        assert!(lower[0][0] > Angle::new::<radian>(1.0));
        assert!(lower[1][1] > Angle::new::<radian>(1.0));
    }

    #[test]
    fn angular_corrections_cannot_silently_cross_declared_bounds() {
        let angle_measurement = AzimuthElevationMeasurement::new(
            path(),
            Epoch::from_tai_seconds(0.0),
            ReferenceFrame::ITRF2020,
            AzimuthElevationConvention::ClockwiseFromNorthAboveHorizon,
            angles(6.0, 0.0, None),
        )
        .expect("angles");
        let correction = AzimuthElevationCorrection::new(Instrument, angles(0.5, 0.0, None));

        assert_eq!(
            correction.apply(&angle_measurement),
            Err(CorrectionError::InvalidMeasurement(
                MeasurementError::AzimuthOutOfRange
            ))
        );
    }

    #[test]
    fn correction_provenance_is_open_to_downstream_models() {
        #[derive(Debug, Clone, Copy)]
        struct SolarPlasma;

        impl CorrectionKind for SolarPlasma {}

        let range = RangeMeasurement::new(
            path(),
            Epoch::from_tai_seconds(0.0),
            ReferenceFrame::ITRF2020,
            RangeConvention::PathLength,
            length(100.0, None),
        )
        .expect("range");
        let correction = AdditiveCorrection::new(SolarPlasma, length(1.0, None));

        assert_eq!(
            correction
                .apply(&range)
                .expect("corrected range")
                .value()
                .value(),
            Length::new::<meter>(101.0)
        );
    }

    #[test]
    fn correction_model_chain_evaluates_multiple_models_against_epoch_conditions() {
        #[derive(Debug)]
        struct Conditions {
            epoch: Epoch,
            clock_bias: Length,
            media_bias: Length,
        }

        #[derive(Debug)]
        enum Model {
            Clock,
            Media,
        }

        impl MeasurementCorrectionModel<RangeMeasurement, Conditions> for Model {
            fn apply_model(
                &self,
                measurement: &RangeMeasurement,
                timeline: &SignalEventTimeline,
                conditions: &Conditions,
            ) -> Result<RangeMeasurement, CorrectionModelError> {
                assert_eq!(timeline.observation_epoch(), conditions.epoch);
                let bias = match self {
                    Self::Clock => conditions.clock_bias,
                    Self::Media => conditions.media_bias,
                };
                let correction = Measured::new([bias], None)
                    .map_err(|error| Box::new(error) as CorrectionModelError)?;
                let value = measurement
                    .value()
                    .corrected(correction)
                    .map_err(|error| Box::new(error) as CorrectionModelError)?;
                measurement
                    .with_value(value)
                    .map_err(|error| Box::new(error) as CorrectionModelError)
            }
        }

        let epoch = Epoch::from_tai_seconds(123.0);
        let range = RangeMeasurement::new(
            path(),
            epoch,
            ReferenceFrame::ITRF2020,
            RangeConvention::PathLength,
            length(100.0, None),
        )
        .expect("range");
        let conditions = Conditions {
            epoch,
            clock_bias: Length::new::<meter>(2.0),
            media_bias: Length::new::<meter>(3.0),
        };
        let mut models = CorrectionModelChain::new();
        models.push(Model::Clock);
        models.push(Model::Media);

        let timeline = SignalEventTimeline::instantaneous(&range);
        assert_eq!(
            models
                .apply(&range, &timeline, &conditions)
                .expect("modelled range")
                .value()
                .value(),
            Length::new::<meter>(105.0)
        );
    }

    #[test]
    fn correction_model_chain_composes_spatiotemporal_propagation_gradients() {
        #[derive(Debug)]
        struct PropagationDelay(SignalPropagationGradient);

        impl MeasurementCorrectionModel<RangeMeasurement, ()> for PropagationDelay {
            fn propagation_gradient(
                &self,
                _measurement: &RangeMeasurement,
                state: SignalPropagationState,
                _conditions: &(),
            ) -> Result<SignalPropagationGradient, CorrectionModelError> {
                assert_eq!(state.epoch(), Epoch::from_tai_seconds(123.0));
                assert_eq!(state.position(), Position::from_metres(1.0, 2.0, 3.0));
                assert_eq!(state.frame(), ReferenceFrame::ITRF2020);
                Ok(self.0)
            }

            fn apply_model(
                &self,
                measurement: &RangeMeasurement,
                _timeline: &SignalEventTimeline,
                _conditions: &(),
            ) -> Result<RangeMeasurement, CorrectionModelError> {
                Ok(measurement.clone())
            }
        }

        let actual_epoch = Epoch::from_tai_seconds(123.0);
        let range = RangeMeasurement::new(
            path(),
            actual_epoch,
            ReferenceFrame::ITRF2020,
            RangeConvention::PathLength,
            length(100.0, None),
        )
        .expect("range");
        let delay =
            SignalPropagationGradient::new(Time::new::<second>(2.5) / Length::new::<meter>(1.0));
        let mut models = CorrectionModelChain::new();
        models.push(PropagationDelay(delay));

        let state = SignalPropagationState::new(
            actual_epoch,
            Position::from_metres(1.0, 2.0, 3.0),
            ReferenceFrame::ITRF2020,
        )
        .expect("finite propagation state");
        let gradient = models
            .propagation_gradient(&range, state, &())
            .expect("propagation gradient");
        assert_eq!(
            gradient.excess_slowness() * Length::new::<meter>(1.0),
            Time::new::<second>(2.5)
        );
        assert!(matches!(
            SignalEventTimeline::instantaneous(&range).with_event_epochs(vec![
                actual_epoch,
                actual_epoch - Duration::from_seconds(1.0),
            ]),
            Err(SignalEventTimelineError::NonMonotonicEventEpoch { .. })
        ));
    }
}
