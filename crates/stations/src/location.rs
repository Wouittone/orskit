//! Typed geodetic locations.

use orskit_units::uom::si::{angle::radian, length::meter};
use orskit_units::{Angle, Length};
use thiserror::Error;

/// Geodetic location relative to a reference ellipsoid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeographicLocation {
    latitude: Angle,
    longitude: Angle,
    altitude: Length,
}

impl GeographicLocation {
    /// Constructs a location with latitude in `[-pi/2, pi/2]` and longitude in
    /// `[-pi, pi]`. Altitude may be negative but must be finite.
    pub fn new(latitude: Angle, longitude: Angle, altitude: Length) -> Result<Self, LocationError> {
        let latitude_rad = latitude.get::<radian>();
        let longitude_rad = longitude.get::<radian>();
        if !latitude_rad.is_finite()
            || !longitude_rad.is_finite()
            || !altitude.get::<meter>().is_finite()
        {
            return Err(LocationError::NonFinite);
        }
        if !(-std::f64::consts::FRAC_PI_2..=std::f64::consts::FRAC_PI_2).contains(&latitude_rad) {
            return Err(LocationError::LatitudeOutOfRange);
        }
        if !(-std::f64::consts::PI..=std::f64::consts::PI).contains(&longitude_rad) {
            return Err(LocationError::LongitudeOutOfRange);
        }
        Ok(Self {
            latitude,
            longitude,
            altitude,
        })
    }

    /// Returns the geodetic latitude.
    #[must_use]
    pub const fn latitude(self) -> Angle {
        self.latitude
    }

    /// Returns the geodetic longitude.
    #[must_use]
    pub const fn longitude(self) -> Angle {
        self.longitude
    }

    /// Returns altitude above the associated reference ellipsoid.
    #[must_use]
    pub const fn altitude(self) -> Length {
        self.altitude
    }
}

/// Invalid geodetic location.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum LocationError {
    /// One or more values are NaN or infinite.
    #[error("location values must be finite")]
    NonFinite,
    /// Latitude lies outside `[-pi/2, pi/2]`.
    #[error("latitude must be within [-pi/2, pi/2]")]
    LatitudeOutOfRange,
    /// Longitude lies outside `[-pi, pi]`.
    #[error("longitude must be within [-pi, pi]")]
    LongitudeOutOfRange,
}

#[cfg(test)]
mod tests {
    use orskit_units::uom::si::{angle::degree, length::meter};

    use super::*;

    #[test]
    fn location_rejects_invalid_latitude() {
        assert_eq!(
            GeographicLocation::new(
                Angle::new::<degree>(91.0),
                Angle::new::<degree>(0.0),
                Length::new::<meter>(0.0),
            ),
            Err(LocationError::LatitudeOutOfRange)
        );
    }
}
