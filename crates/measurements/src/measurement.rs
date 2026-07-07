//! Typed measurement data structures.

use hifitime::Epoch;
use orskit_units::uom::si::length::meter;
use orskit_units::Length;
use thiserror::Error;

/// A scalar range observation and its one-sigma uncertainty.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RangeMeasurement {
    epoch: Epoch,
    range: Length,
    uncertainty: Length,
}

impl RangeMeasurement {
    /// Constructs a finite non-negative range observation with positive uncertainty.
    pub fn new(epoch: Epoch, range: Length, uncertainty: Length) -> Result<Self, MeasurementError> {
        let range_m = range.get::<meter>();
        let uncertainty_m = uncertainty.get::<meter>();
        if !range_m.is_finite() || !uncertainty_m.is_finite() {
            return Err(MeasurementError::NonFinite);
        }
        if range_m < 0.0 {
            return Err(MeasurementError::NegativeRange);
        }
        if uncertainty_m <= 0.0 {
            return Err(MeasurementError::NotPositiveUncertainty);
        }
        Ok(Self {
            epoch,
            range,
            uncertainty,
        })
    }

    /// Returns the observation epoch.
    #[must_use]
    pub const fn epoch(self) -> Epoch {
        self.epoch
    }

    /// Returns the measured range.
    #[must_use]
    pub const fn range(self) -> Length {
        self.range
    }

    /// Returns the one-sigma range uncertainty.
    #[must_use]
    pub const fn uncertainty(self) -> Length {
        self.uncertainty
    }
}

/// Invalid measurement input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum MeasurementError {
    /// The measurement or uncertainty is NaN or infinite.
    #[error("measurement values must be finite")]
    NonFinite,
    /// A geometric range cannot be negative.
    #[error("measurement range must be non-negative")]
    NegativeRange,
    /// One-sigma uncertainty is zero or negative.
    #[error("measurement uncertainty must be strictly positive")]
    NotPositiveUncertainty,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_uncertainty_must_be_positive() {
        assert_eq!(
            RangeMeasurement::new(
                Epoch::from_tai_seconds(0.0),
                Length::new::<meter>(1.0),
                Length::new::<meter>(0.0),
            ),
            Err(MeasurementError::NotPositiveUncertainty)
        );
    }

    #[test]
    fn range_must_not_be_negative() {
        assert_eq!(
            RangeMeasurement::new(
                Epoch::from_tai_seconds(0.0),
                Length::new::<meter>(-1.0),
                Length::new::<meter>(1.0),
            ),
            Err(MeasurementError::NegativeRange)
        );
    }
}
