#![forbid(unsafe_code)]

//! Composable, participant-centric, unit-qualified measurement data.
//!
//! Concrete observation types implement [`Measurement`] and retain their
//! dimensions; correction chains compose only with the matching implementation.
//! Every observable and correction carries typed values plus either a scalar
//! standard error (for one-value observations), a typed positive-definite
//! covariance input decomposed into a retained lower-triangular matrix (for
//! multi-value observations), or explicitly unknown error.

pub mod correction;
pub mod estimation;
#[cfg(any(
    feature = "angular-ra-dec",
    feature = "bistatic-range",
    feature = "bistatic-range-rate",
    feature = "turnaround-range",
    feature = "tdoa",
    feature = "fdoa",
    feature = "phase"
))]
pub mod ground;
pub mod measurement;
pub mod participant;
pub mod station;

#[cfg(feature = "azimuth-elevation")]
pub use correction::AzimuthElevationCorrection;
#[cfg(feature = "clock-correction")]
pub use correction::Clock;
#[cfg(feature = "instrument-correction")]
pub use correction::Instrument;
#[cfg(feature = "ionosphere-correction")]
pub use correction::Ionosphere;
#[cfg(feature = "relativity-correction")]
pub use correction::Relativity;
#[cfg(feature = "troposphere-correction")]
pub use correction::Troposphere;
pub use correction::{
    AdditiveCorrection, CorrectionChain, CorrectionError, CorrectionKind, CorrectionModelChain,
    CorrectionModelError, MeasurementCorrection, MeasurementCorrectionModel, SignalEventTimeline,
    SignalEventTimelineError, SignalPropagationGradient, SignalPropagationSlowness,
    SignalPropagationState, SignalPropagationStateError,
};
#[cfg(any(
    feature = "geometric-doppler",
    feature = "geometric-fdoa",
    feature = "geometric-phase"
))]
pub use estimation::CarrierGeometricEstimator;
pub use estimation::{
    CompositeParticipantStateProvider, CompositeParticipantStateProviderError,
    GeometricEstimationError, GeometricEstimator, GroundStationProvider,
    GroundStationProviderError, MeasurementEstimator, MeasurementModelEstimationError,
    MeasurementPrediction, ObservationEpochStage, ParticipantKinematics,
    ParticipantKinematicsError, ParticipantStateProvider, SignalPropagationError,
    SignalPropagationSolver, TransformingParticipantStateProvider,
    TransformingParticipantStateProviderError,
};
#[cfg(feature = "light-time")]
pub use estimation::{
    VacuumLightTimeConfigurationError, VacuumLightTimeError, VacuumLightTimeSolver,
};
#[cfg(any(
    feature = "angular-ra-dec",
    feature = "bistatic-range",
    feature = "bistatic-range-rate",
    feature = "turnaround-range",
    feature = "tdoa",
    feature = "fdoa",
    feature = "phase"
))]
pub use ground::GroundObservationError;
#[cfg(any(
    feature = "bistatic-range",
    feature = "bistatic-range-rate",
    feature = "turnaround-range",
    feature = "tdoa",
    feature = "fdoa"
))]
pub use ground::GroundStationPair;
#[cfg(feature = "bistatic-range")]
pub use ground::{BistaticRangeKind, BistaticRangeMeasurement};
#[cfg(feature = "bistatic-range-rate")]
pub use ground::{BistaticRangeRateKind, BistaticRangeRateMeasurement};
#[cfg(feature = "fdoa")]
pub use ground::{FdoaKind, FdoaMeasurement};
#[cfg(feature = "phase")]
pub use ground::{PhaseKind, PhaseMeasurement};
#[cfg(feature = "angular-ra-dec")]
pub use ground::{
    RightAscensionDeclinationConvention, RightAscensionDeclinationKind,
    RightAscensionDeclinationMeasurement,
};
#[cfg(feature = "tdoa")]
pub use ground::{TdoaKind, TdoaMeasurement};
#[cfg(feature = "turnaround-range")]
pub use ground::{TurnaroundRangeKind, TurnaroundRangeMeasurement};
#[cfg(feature = "azimuth-elevation")]
pub use measurement::{
    AzimuthElevationConvention, AzimuthElevationKind, AzimuthElevationMeasurement,
};
#[cfg(feature = "doppler")]
pub use measurement::{DopplerKind, DopplerMeasurement};
pub use measurement::{
    Measured, Measurement, MeasurementError, MeasurementKind, MeasurementQuantity,
    MeasurementUncertainty, MeasurementUncertaintyError, MeasurementUncertaintyInput,
    MeasurementValueError, MeasurementValues,
};
#[cfg(feature = "range")]
pub use measurement::{RangeConvention, RangeKind, RangeMeasurement};
#[cfg(feature = "range-rate")]
pub use measurement::{RangeRateKind, RangeRateMeasurement};
pub use participant::{ParticipantId, ParticipantIdError, SignalPath, SignalPathError};
pub use station::GroundStation;
