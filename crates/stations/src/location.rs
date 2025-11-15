//! Geographic location structures and utilities

/// Represents a ground station or geographic location
#[derive(Debug, Clone, Copy)]
pub struct GeographicLocation {
    /// Latitude in radians
    pub latitude: f64,
    /// Longitude in radians
    pub longitude: f64,
    /// Altitude in meters above ellipsoid
    pub altitude: f64,
}

impl GeographicLocation {
    /// Create a new geographic location
    pub fn new(latitude: f64, longitude: f64, altitude: f64) -> Self {
        Self {
            latitude,
            longitude,
            altitude,
        }
    }
}
