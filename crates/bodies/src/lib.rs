//! Celestial-body and body-system identities for orskit.
//!
//! This crate deliberately contains identity and classification only. Physical
//! constants, ephemerides, shapes, and rotation models require separately
//! sourced data and do not belong in a body identifier.
//!
//! ```
//! use bodies::{Body, BodySystem};
//!
//! assert!(BodySystem::EARTH_MOON.contains(Body::EARTH));
//! assert!(BodySystem::EARTH_MOON.contains(Body::MOON));
//! ```

use std::{fmt, str::FromStr};

use thiserror::Error;

const SOLAR_SYSTEM_BODIES: &[Body] = &[
    Body::SUN,
    Body::MERCURY,
    Body::VENUS,
    Body::EARTH,
    Body::MOON,
    Body::MARS,
    Body::JUPITER,
    Body::SATURN,
    Body::URANUS,
    Body::NEPTUNE,
    Body::PLUTO,
];
const EARTH_MOON_BODIES: &[Body] = &[Body::EARTH, Body::MOON];

/// Stable identifier for an application-defined celestial body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CustomBodyId(u64);

impl CustomBodyId {
    /// Constructs an application-defined body identifier.
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

/// Broad classification of a celestial body.
///
/// The built-in Solar System classification follows
/// [IAU Resolution B5](https://www.iau.org/static/resolutions/Resolution_GA26-5-6.pdf):
/// eight planets are distinct from dwarf planets such as Pluto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BodyKind {
    /// A star.
    Star,
    /// One of the eight IAU planets in the Solar System.
    Planet,
    /// A natural satellite.
    Moon,
    /// A dwarf planet.
    DwarfPlanet,
    /// Another kind of celestial body.
    Other,
}

impl fmt::Display for BodyKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Star => "STAR",
            Self::Planet => "PLANET",
            Self::Moon => "MOON",
            Self::DwarfPlanet => "DWARF PLANET",
            Self::Other => "BODY",
        })
    }
}

/// Identity of a celestial body independently of any frame or physical model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Body {
    identity: BodyIdentity,
    kind: BodyKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum BodyIdentity {
    Sun,
    Mercury,
    Venus,
    Earth,
    Moon,
    Mars,
    Jupiter,
    Saturn,
    Uranus,
    Neptune,
    Pluto,
    Custom(CustomBodyId),
}

impl Body {
    /// The Sun.
    pub const SUN: Self = Self::known(BodyIdentity::Sun, BodyKind::Star);
    /// Mercury.
    pub const MERCURY: Self = Self::known(BodyIdentity::Mercury, BodyKind::Planet);
    /// Venus.
    pub const VENUS: Self = Self::known(BodyIdentity::Venus, BodyKind::Planet);
    /// Earth.
    pub const EARTH: Self = Self::known(BodyIdentity::Earth, BodyKind::Planet);
    /// Earth's Moon.
    pub const MOON: Self = Self::known(BodyIdentity::Moon, BodyKind::Moon);
    /// Mars.
    pub const MARS: Self = Self::known(BodyIdentity::Mars, BodyKind::Planet);
    /// Jupiter.
    pub const JUPITER: Self = Self::known(BodyIdentity::Jupiter, BodyKind::Planet);
    /// Saturn.
    pub const SATURN: Self = Self::known(BodyIdentity::Saturn, BodyKind::Planet);
    /// Uranus.
    pub const URANUS: Self = Self::known(BodyIdentity::Uranus, BodyKind::Planet);
    /// Neptune.
    pub const NEPTUNE: Self = Self::known(BodyIdentity::Neptune, BodyKind::Planet);
    /// Pluto.
    pub const PLUTO: Self = Self::known(BodyIdentity::Pluto, BodyKind::DwarfPlanet);

    const fn known(identity: BodyIdentity, kind: BodyKind) -> Self {
        Self { identity, kind }
    }

    /// Constructs an application-defined body with an explicit classification.
    ///
    /// This supports planets and moons outside the small built-in catalogue
    /// without assigning them an implicit physical model.
    #[must_use]
    pub const fn custom(id: CustomBodyId, kind: BodyKind) -> Self {
        Self {
            identity: BodyIdentity::Custom(id),
            kind,
        }
    }

    /// Returns this body's broad classification.
    #[must_use]
    pub const fn kind(self) -> BodyKind {
        self.kind
    }

    /// Returns the identifier when this is an application-defined body.
    #[must_use]
    pub const fn custom_id(self) -> Option<CustomBodyId> {
        match self.identity {
            BodyIdentity::Custom(id) => Some(id),
            _ => None,
        }
    }
}

impl fmt::Display for Body {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.identity {
            BodyIdentity::Sun => formatter.write_str("SUN"),
            BodyIdentity::Mercury => formatter.write_str("MERCURY"),
            BodyIdentity::Venus => formatter.write_str("VENUS"),
            BodyIdentity::Earth => formatter.write_str("EARTH"),
            BodyIdentity::Moon => formatter.write_str("MOON"),
            BodyIdentity::Mars => formatter.write_str("MARS"),
            BodyIdentity::Jupiter => formatter.write_str("JUPITER"),
            BodyIdentity::Saturn => formatter.write_str("SATURN"),
            BodyIdentity::Uranus => formatter.write_str("URANUS"),
            BodyIdentity::Neptune => formatter.write_str("NEPTUNE"),
            BodyIdentity::Pluto => formatter.write_str("PLUTO"),
            BodyIdentity::Custom(id) => {
                write!(formatter, "CUSTOM {}({})", self.kind, id.value())
            }
        }
    }
}

impl FromStr for Body {
    type Err = BodyParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match normalized_name(value).as_str() {
            "SUN" | "HELIOCENTER" => Ok(Self::SUN),
            "MERCURY" => Ok(Self::MERCURY),
            "VENUS" => Ok(Self::VENUS),
            "EARTH" | "GEOCENTER" => Ok(Self::EARTH),
            "MOON" | "SELENOCENTER" => Ok(Self::MOON),
            "MARS" => Ok(Self::MARS),
            "JUPITER" => Ok(Self::JUPITER),
            "SATURN" => Ok(Self::SATURN),
            "URANUS" => Ok(Self::URANUS),
            "NEPTUNE" => Ok(Self::NEPTUNE),
            "PLUTO" => Ok(Self::PLUTO),
            _ => Err(BodyParseError),
        }
    }
}

/// Named collection of bodies whose center of mass can define a frame origin.
///
/// Membership is explicit and inspectable. It is an identity relationship,
/// not a gravity or ephemeris model: computing a barycenter still requires
/// caller-selected masses and states for the represented bodies.
/// This preserves the body/barycenter distinction documented by
/// [NAIF SPICE](https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/FORTRAN/req/naif_ids.html).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BodySystem {
    name: &'static str,
    bodies: &'static [Body],
}

impl BodySystem {
    /// The Solar System members represented by this foundational body catalogue.
    pub const SOLAR_SYSTEM: Self = Self {
        name: "SOLAR SYSTEM",
        bodies: SOLAR_SYSTEM_BODIES,
    };

    /// The Earth-Moon system.
    pub const EARTH_MOON: Self = Self {
        name: "EARTH-MOON",
        bodies: EARTH_MOON_BODIES,
    };

    /// Defines a custom static body system.
    ///
    /// A barycentric system must have a non-blank name and at least two
    /// distinct bodies. Static storage keeps frame identities immutable,
    /// allocation-free, and usable as constants.
    pub fn new(name: &'static str, bodies: &'static [Body]) -> Result<Self, BodySystemError> {
        if name.trim().is_empty() {
            return Err(BodySystemError::EmptyName);
        }
        if bodies.len() < 2 {
            return Err(BodySystemError::TooFewBodies);
        }
        for (index, body) in bodies.iter().enumerate() {
            if bodies[index + 1..].contains(body) {
                return Err(BodySystemError::DuplicateBody(*body));
            }
        }
        Ok(Self { name, bodies })
    }

    /// Returns the system name without the `BARYCENTER` suffix.
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Returns the explicitly linked bodies in this system.
    #[must_use]
    pub const fn bodies(self) -> &'static [Body] {
        self.bodies
    }

    /// Returns whether the body is explicitly linked to this system.
    #[must_use]
    pub fn contains(self, body: Body) -> bool {
        self.bodies.contains(&body)
    }
}

impl fmt::Display for BodySystem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name)
    }
}

/// Invalid custom body-system definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum BodySystemError {
    /// System names must contain a non-whitespace character.
    #[error("body-system name must not be blank")]
    EmptyName,
    /// A barycenter requires at least two bodies.
    #[error("a body system must contain at least two bodies")]
    TooFewBodies,
    /// A body may occur only once in a system.
    #[error("body system contains duplicate body {0}")]
    DuplicateBody(Body),
}

/// Error returned when a built-in body name is unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("unknown celestial body")]
pub struct BodyParseError;

fn normalized_name(value: &str) -> String {
    value
        .trim()
        .to_ascii_uppercase()
        .replace(['-', '_'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_body_classification_is_explicit() {
        assert_eq!(Body::SUN.kind(), BodyKind::Star);
        assert_eq!(Body::EARTH.kind(), BodyKind::Planet);
        assert_eq!(Body::MOON.kind(), BodyKind::Moon);
        assert_eq!(Body::PLUTO.kind(), BodyKind::DwarfPlanet);

        let custom_moon = Body::custom(CustomBodyId::new(42), BodyKind::Moon);
        assert_eq!(custom_moon.kind(), BodyKind::Moon);
        assert_eq!(custom_moon.custom_id(), Some(CustomBodyId::new(42)));
    }

    #[test]
    fn earth_moon_system_links_both_bodies() {
        assert_eq!(BodySystem::EARTH_MOON.bodies(), &[Body::EARTH, Body::MOON]);
        assert!(BodySystem::EARTH_MOON.contains(Body::EARTH));
        assert!(BodySystem::EARTH_MOON.contains(Body::MOON));
        assert!(!BodySystem::EARTH_MOON.contains(Body::MARS));
    }

    #[test]
    fn invalid_custom_systems_are_rejected() {
        const DUPLICATE: &[Body] = &[Body::EARTH, Body::EARTH];

        assert_eq!(
            BodySystem::new("", &[Body::EARTH, Body::MOON]),
            Err(BodySystemError::EmptyName)
        );
        assert_eq!(
            BodySystem::new("EARTH", &[Body::EARTH]),
            Err(BodySystemError::TooFewBodies)
        );
        assert_eq!(
            BodySystem::new("DUPLICATE", DUPLICATE),
            Err(BodySystemError::DuplicateBody(Body::EARTH))
        );
    }

    #[test]
    fn registered_body_aliases_parse() {
        assert_eq!("GEOCENTER".parse(), Ok(Body::EARTH));
        assert_eq!("SELENOCENTER".parse(), Ok(Body::MOON));
        assert_eq!("HELIOCENTER".parse(), Ok(Body::SUN));
    }
}
