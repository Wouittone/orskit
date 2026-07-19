use std::{error::Error, fmt};

use crate::EstimationObserver;

/// Common sequential orbit-determination boundary.
pub trait OrbitDetermination<Observation>: fmt::Debug + Send + Sync {
    /// State returned after assimilating an observation.
    type Estimate;
    /// Error produced by this implementation.
    type Error: Error + Send + Sync + 'static;

    /// Propagates the current estimate and assimilates one observation.
    fn estimate(&mut self, observation: &Observation) -> Result<Self::Estimate, Self::Error>;

    /// Processes ordered observations and returns every posterior.
    fn estimate_all<'a>(
        &mut self,
        observations: impl IntoIterator<Item = &'a Observation>,
    ) -> Result<Vec<Self::Estimate>, Self::Error>
    where
        Observation: 'a,
    {
        observations
            .into_iter()
            .map(|observation| self.estimate(observation))
            .collect()
    }
}

/// A sequential Kalman implementation with optional caller-owned diagnostics.
pub trait KalmanFilter<Observation>: OrbitDetermination<Observation> {
    /// Processes one observation and emits diagnostics only to `observer`.
    fn estimate_with_observer(
        &mut self,
        observation: &Observation,
        observer: &mut dyn EstimationObserver,
    ) -> Result<Self::Estimate, Self::Error>;
}
