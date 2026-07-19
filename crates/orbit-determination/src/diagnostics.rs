use crate::{CartesianStateEstimate, PositionCovariance};
use units::Position;

/// Optional, caller-owned sink for estimation diagnostics.
///
/// The filter does not install or retain an observer. Call
/// [`crate::KalmanFilter::estimate_with_observer`] only for validation or
/// plotting runs.
pub trait EstimationObserver {
    /// Called after propagation and before correction.
    fn on_prediction(&mut self, event: PredictionEvent<'_>);
    /// Called after correction.
    fn on_correction(&mut self, event: CorrectionEvent<'_>);
}

/// Pre-correction diagnostic event.
#[derive(Debug)]
pub struct PredictionEvent<'a> {
    /// Predicted state estimate.
    pub estimate: &'a CartesianStateEstimate,
}

/// Post-correction diagnostic event for a Cartesian position observation.
#[derive(Debug)]
pub struct CorrectionEvent<'a> {
    /// Observation minus predicted position.
    pub innovation: Position,
    /// Observation minus corrected position.
    pub residual: Position,
    /// Innovation covariance.
    pub innovation_covariance: PositionCovariance,
    /// Corrected posterior estimate.
    pub estimate: &'a CartesianStateEstimate,
}
