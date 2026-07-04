//! Reference-frame identities for orskit.
//!
//! A frame is modeled as a body-backed, barycentric, or custom origin plus an
//! orientation. Transform algorithms will be added behind provider traits once
//! their data and accuracy contracts are defined; state values can already
//! carry an unambiguous frame identity.

use std::{fmt, str::FromStr};

pub use orskit_bodies::{Body, BodySystem, CustomBodyId};
use thiserror::Error;

/// Typed identifier reserved for application-defined frame components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CustomFrameId(u64);

impl CustomFrameId {
    /// Constructs an application-defined identifier.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the application-defined numeric identifier.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Origin of a reference frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FrameOrigin {
    /// Center of mass of an explicitly linked body system.
    Barycenter(BodySystem),
    /// Center of mass of one celestial body.
    Body(Body),
    /// Application-defined origin.
    Custom(CustomFrameId),
}

/// Orientation of a reference frame's axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FrameOrientation {
    /// International Celestial Reference Frame.
    Icrf,
    /// Geocentric Celestial Reference Frame.
    Gcrf,
    /// Earth Mean Equator and Equinox of J2000.
    Eme2000,
    /// International Terrestrial Reference Frame realization identified by year.
    Itrf(u16),
    /// True Equator, Mean Equinox frame.
    Teme,
    /// Mean equator and equinox of date.
    Mod,
    /// True equator and equinox of date.
    Tod,
    /// Greenwich true-of-date rotating frame.
    Gtod,
    /// Application-defined orientation.
    Custom(CustomFrameId),
}

/// Complete reference-frame identity: origin plus orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReferenceFrame {
    origin: FrameOrigin,
    orientation: FrameOrientation,
}

impl ReferenceFrame {
    /// Solar-system barycentric ICRF.
    pub const ICRF: Self = Self::new(
        FrameOrigin::Barycenter(BodySystem::SOLAR_SYSTEM),
        FrameOrientation::Icrf,
    );
    /// Geocentric Celestial Reference Frame.
    pub const GCRF: Self = Self::new(FrameOrigin::Body(Body::EARTH), FrameOrientation::Gcrf);
    /// Geocentric Earth Mean Equator and Equinox of J2000.
    pub const EME2000: Self = Self::new(FrameOrigin::Body(Body::EARTH), FrameOrientation::Eme2000);
    /// Geocentric ITRF2020 terrestrial frame.
    pub const ITRF2020: Self =
        Self::new(FrameOrigin::Body(Body::EARTH), FrameOrientation::Itrf(2020));
    /// Geocentric True Equator, Mean Equinox frame.
    pub const TEME: Self = Self::new(FrameOrigin::Body(Body::EARTH), FrameOrientation::Teme);

    /// Constructs a frame from an explicit origin and orientation.
    #[must_use]
    pub const fn new(origin: FrameOrigin, orientation: FrameOrientation) -> Self {
        Self {
            origin,
            orientation,
        }
    }

    /// Returns the frame origin.
    #[must_use]
    pub const fn origin(self) -> FrameOrigin {
        self.origin
    }

    /// Returns the frame orientation.
    #[must_use]
    pub const fn orientation(self) -> FrameOrientation {
        self.orientation
    }
}

impl fmt::Display for ReferenceFrame {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match *self {
            Self::ICRF => "ICRF",
            Self::GCRF => "GCRF",
            Self::EME2000 => "EME2000",
            Self::ITRF2020 => "ITRF2020",
            Self::TEME => "TEME",
            _ => return write!(formatter, "{}/{}", self.origin, self.orientation),
        };
        formatter.write_str(name)
    }
}

impl FromStr for ReferenceFrame {
    type Err = FrameParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_uppercase().as_str() {
            "ICRF" => Ok(Self::ICRF),
            "GCRF" => Ok(Self::GCRF),
            "EME2000" | "J2000" => Ok(Self::EME2000),
            "ITRF2020" | "ITRF-2020" => Ok(Self::ITRF2020),
            "TEME" => Ok(Self::TEME),
            _ => Err(FrameParseError),
        }
    }
}

impl fmt::Display for FrameOrigin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Barycenter(system) => write!(formatter, "{system} BARYCENTER"),
            Self::Body(body) => body.fmt(formatter),
            Self::Custom(id) => write!(formatter, "CUSTOM({})", id.value()),
        }
    }
}

impl FromStr for FrameOrigin {
    type Err = FrameOriginParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match normalized_name(value).as_str() {
            "SOLAR SYSTEM BARYCENTER" | "SSB" => Ok(Self::Barycenter(BodySystem::SOLAR_SYSTEM)),
            "EARTH MOON BARYCENTER" | "EARTH BARYCENTER" | "EMB" => {
                Ok(Self::Barycenter(BodySystem::EARTH_MOON))
            }
            _ => value
                .parse::<Body>()
                .map(Self::Body)
                .map_err(|_| FrameOriginParseError),
        }
    }
}

impl fmt::Display for FrameOrientation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Icrf => formatter.write_str("ICRF"),
            Self::Gcrf => formatter.write_str("GCRF"),
            Self::Eme2000 => formatter.write_str("EME2000"),
            Self::Itrf(year) => write!(formatter, "ITRF{year}"),
            Self::Teme => formatter.write_str("TEME"),
            Self::Mod => formatter.write_str("MOD"),
            Self::Tod => formatter.write_str("TOD"),
            Self::Gtod => formatter.write_str("GTOD"),
            Self::Custom(id) => write!(formatter, "CUSTOM({})", id.value()),
        }
    }
}

impl FromStr for FrameOrientation {
    type Err = FrameOrientationParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let normalized = normalized_name(value);
        match normalized.as_str() {
            "ICRF" => Ok(Self::Icrf),
            "GCRF" => Ok(Self::Gcrf),
            "EME2000" | "J2000" => Ok(Self::Eme2000),
            "TEME" => Ok(Self::Teme),
            "MOD" => Ok(Self::Mod),
            "TOD" => Ok(Self::Tod),
            "GTOD" => Ok(Self::Gtod),
            _ => normalized
                .strip_prefix("ITRF")
                .and_then(|year| year.trim().parse::<u16>().ok())
                .map(Self::Itrf)
                .ok_or(FrameOrientationParseError),
        }
    }
}

fn normalized_name(value: &str) -> String {
    value
        .trim()
        .to_ascii_uppercase()
        .replace(['-', '_'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Error returned when a built-in frame name is unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("unknown reference frame")]
pub struct FrameParseError;

/// Error returned when a built-in frame origin name is unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("unknown reference frame origin")]
pub struct FrameOriginParseError;

/// Error returned when a built-in frame orientation name is unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("unknown reference frame orientation")]
pub struct FrameOrientationParseError;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_frames_round_trip_through_names() {
        for frame in [
            ReferenceFrame::ICRF,
            ReferenceFrame::GCRF,
            ReferenceFrame::EME2000,
            ReferenceFrame::ITRF2020,
            ReferenceFrame::TEME,
        ] {
            assert_eq!(frame.to_string().parse(), Ok(frame));
        }
    }

    #[test]
    fn j2000_alias_resolves_to_eme2000() {
        assert_eq!("J2000".parse(), Ok(ReferenceFrame::EME2000));
    }

    #[test]
    fn ccsds_components_form_non_geocentric_frames() {
        let origin: FrameOrigin = "MARS".parse().expect("known SANA center name");
        let orientation: FrameOrientation = "ITRF-2014".parse().expect("known realization syntax");

        assert_eq!(origin, FrameOrigin::Body(Body::MARS));
        assert_eq!(orientation, FrameOrientation::Itrf(2014));
        assert_eq!(
            ReferenceFrame::new(origin, FrameOrientation::Icrf).to_string(),
            "MARS/ICRF"
        );
    }

    #[test]
    fn barycentric_origins_retain_system_membership() {
        let origin: FrameOrigin = "EMB".parse().expect("known barycenter name");
        let FrameOrigin::Barycenter(system) = origin else {
            panic!("EMB must be a barycentric origin");
        };

        assert_eq!(system, BodySystem::EARTH_MOON);
        assert_eq!(system.bodies(), &[Body::EARTH, Body::MOON]);
    }
}
