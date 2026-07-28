use std::{error::Error, fmt};

use crate::EstimationObserver;

/// Common sequential orbit-determination boundary.
///
/// `estimate_all` preserves observation order and stops at the first error.
///
/// ```
/// use std::convert::Infallible;
///
/// use orbit_determination::OrbitDetermination;
///
/// #[derive(Debug, Default)]
/// struct RunningTotal(i32);
///
/// impl OrbitDetermination<i32> for RunningTotal {
///     type Estimate = i32;
///     type Error = Infallible;
///
///     fn estimate(&mut self, observation: &i32) -> Result<Self::Estimate, Self::Error> {
///         self.0 += observation;
///         Ok(self.0)
///     }
/// }
///
/// let observations = [2, 3, 5];
/// let estimates = RunningTotal::default().estimate_all(&observations)?;
/// assert_eq!(estimates, [2, 5, 10]);
/// # Ok::<(), Infallible>(())
/// ```
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
