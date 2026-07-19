use std::error::Error;

use orbits::cartesian::StateError;
use thiserror::Error;

/// Error from OD validation, propagation, or Kalman correction.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum OrbitDeterminationError {
    /// A scalar, state, or matrix component is NaN or infinite.
    #[error("{what} must contain only finite values")]
    NonFinite { what: &'static str },
    /// Domain objects do not share one frame.
    #[error("estimate, covariance, observation, and propagated state frames must match")]
    FrameMismatch,
    /// A covariance is not symmetric.
    #[error("{what} must be symmetric")]
    NotSymmetric { what: &'static str },
    /// A covariance cannot be factored.
    #[error("{what} must be positive definite")]
    NotPositiveDefinite { what: &'static str },
    /// The innovation covariance cannot be solved.
    #[error("innovation covariance is singular")]
    SingularInnovationCovariance,
    /// Unscented-transform scaling parameters are invalid.
    #[error("unscented-transform parameters must define a positive finite scaling")]
    InvalidUnscentedConfiguration,
    /// An internally reconstructed Cartesian state was invalid.
    #[error("propagation produced an invalid Cartesian state")]
    InvalidCartesianState(#[source] StateError),
    /// The application-selected propagator failed.
    #[error("application-selected propagator failed")]
    Propagation(#[source] Box<dyn Error + Send + Sync + 'static>),
}

impl OrbitDeterminationError {
    pub(crate) fn propagation(error: impl Error + Send + Sync + 'static) -> Self {
        Self::Propagation(Box::new(error))
    }
}
