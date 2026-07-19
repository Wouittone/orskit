use std::fmt;

use frames::ReferenceFrame;
use hifitime::Epoch;
use units::Position;

use crate::{OrbitDeterminationError, PositionCovariance};

/// Typed Cartesian position observation contract for the supplied filters.
pub trait CartesianObservation: fmt::Debug + Send + Sync {
    /// Epoch at which the observation is evaluated.
    fn epoch(&self) -> Epoch;
    /// Frame in which the observation is expressed.
    fn frame(&self) -> ReferenceFrame;
    /// Observed Cartesian position.
    fn position(&self) -> Position;
    /// Positive-definite position covariance.
    fn covariance(&self) -> &PositionCovariance;
}

/// Cartesian position observation with unit-qualified position and covariance.
#[derive(Debug, Clone, PartialEq)]
pub struct CartesianPositionObservation {
    epoch: Epoch,
    position: Position,
    covariance: PositionCovariance,
}

impl CartesianPositionObservation {
    /// Creates a position observation. The covariance supplies its frame.
    pub fn new(
        epoch: Epoch,
        position: Position,
        covariance: PositionCovariance,
    ) -> Result<Self, OrbitDeterminationError> {
        if !position.is_finite() {
            return Err(OrbitDeterminationError::NonFinite { what: "position" });
        }
        Ok(Self {
            epoch,
            position,
            covariance,
        })
    }
}

impl CartesianObservation for CartesianPositionObservation {
    fn epoch(&self) -> Epoch {
        self.epoch
    }
    fn frame(&self) -> ReferenceFrame {
        self.covariance.frame()
    }
    fn position(&self) -> Position {
        self.position
    }
    fn covariance(&self) -> &PositionCovariance {
        &self.covariance
    }
}
