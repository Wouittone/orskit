//! Measurement data structures

/// Represents a measurement or observation
#[derive(Debug, Clone)]
pub struct Measurement {
    /// Time of measurement (MJD or similar)
    pub time: f64,
    /// Measurement value
    pub value: f64,
    /// Measurement uncertainty (1-sigma)
    pub uncertainty: f64,
}

impl Measurement {
    /// Create a new measurement
    pub fn new(time: f64, value: f64, uncertainty: f64) -> Self {
        Self {
            time,
            value,
            uncertainty,
        }
    }
}
