#![forbid(unsafe_code)]

//! Reference-frame identities for orskit.
//!
//! A frame identity is modeled as a body-backed, barycentric, or custom origin
//! plus an orientation. A [`DerivedFrame`] associates a parent-aligned custom
//! identity with a fixed origin offset expressed in its parent frame. This
//! supports caller-owned hierarchies such as an Earth-fixed ground site without
//! pretending that general frame transforms or geodesy already exist.
//! Orientations explicitly declare whether their axes are inertial,
//! non-inertial, or unspecified. Transform algorithms will be added behind
//! provider traits once their data and accuracy contracts are defined.

use std::collections::{HashMap, HashSet};
use std::{fmt, str::FromStr};

pub use bodies::{Body, BodySystem, CustomBodyId};
use thiserror::Error;
use units::Position;

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

/// Caller-assigned namespace for one logical frame catalog.
///
/// Applications should use a stable, globally unique 128-bit value, such as a
/// UUID encoded as `u128`. Catalog instances using the same namespace are
/// replicas of the same logical catalog; distinct logical catalogs must use
/// distinct namespaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FrameNamespace(u128);

impl FrameNamespace {
    /// Constructs an explicit catalog namespace.
    #[must_use]
    pub const fn new(value: u128) -> Self {
        Self(value)
    }

    /// Returns the application-assigned namespace value.
    #[must_use]
    pub const fn value(self) -> u128 {
        self.0
    }
}

/// Opaque identity issued by a [`FrameCatalog`] for one derived frame.
///
/// Identity includes both the catalog namespace and catalog-local key. Callers
/// can inspect those components but cannot construct a `FrameId` directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FrameId {
    namespace: FrameNamespace,
    local_id: u64,
}

impl FrameId {
    /// Returns the namespace of the issuing logical catalog.
    #[must_use]
    pub const fn namespace(self) -> FrameNamespace {
        self.namespace
    }

    /// Returns the catalog-local key chosen when the definition was issued.
    #[must_use]
    pub const fn local_id(self) -> u64 {
        self.local_id
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
    /// Origin issued and validated by a caller-owned frame catalog.
    Derived(FrameId),
}

/// Whether frame axes are suitable for equations requiring inertial axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FrameMotion {
    /// Axes are explicitly defined as inertial.
    Inertial,
    /// Axes rotate or otherwise vary with time.
    NonInertial,
    /// No inertial-motion claim has been supplied.
    Unspecified,
}

impl fmt::Display for FrameMotion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Inertial => "INERTIAL",
            Self::NonInertial => "NON_INERTIAL",
            Self::Unspecified => "UNSPECIFIED",
        })
    }
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
    /// Application-defined orientation with explicit motion semantics.
    Custom {
        /// Application-defined orientation identity.
        id: CustomFrameId,
        /// Whether the axes are inertial.
        motion: FrameMotion,
    },
}

impl FrameOrientation {
    /// Constructs an application-defined orientation with explicit motion.
    #[must_use]
    pub const fn custom(id: CustomFrameId, motion: FrameMotion) -> Self {
        Self::Custom { id, motion }
    }

    /// Returns the axes' declared motion semantics.
    #[must_use]
    pub const fn motion(self) -> FrameMotion {
        match self {
            Self::Icrf | Self::Gcrf | Self::Eme2000 => FrameMotion::Inertial,
            Self::Itrf(_) | Self::Teme | Self::Mod | Self::Tod | Self::Gtod => {
                FrameMotion::NonInertial
            }
            Self::Custom { motion, .. } => motion,
        }
    }

    /// Returns whether the axes are affirmatively classified as inertial.
    #[must_use]
    pub const fn is_inertial(self) -> bool {
        matches!(self.motion(), FrameMotion::Inertial)
    }
}

/// Complete reference-frame identity: origin plus orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReferenceFrame {
    origin: FrameOrigin,
    orientation: FrameOrientation,
}

/// A reference frame whose axes are affirmatively classified as inertial.
///
/// This capability proves only the axes' declared motion semantics. It does
/// not establish that the frame origin matches a particular dynamical model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InertialFrame(ReferenceFrame);

impl InertialFrame {
    /// Solar-system barycentric ICRF.
    pub const ICRF: Self = Self(ReferenceFrame::ICRF);
    /// Geocentric Celestial Reference Frame.
    pub const GCRF: Self = Self(ReferenceFrame::GCRF);
    /// Geocentric Earth Mean Equator and Equinox of J2000.
    pub const EME2000: Self = Self(ReferenceFrame::EME2000);

    /// Returns the reference frame carrying the inertial-axis declaration.
    #[must_use]
    pub const fn reference_frame(self) -> ReferenceFrame {
        self.0
    }
}

impl TryFrom<ReferenceFrame> for InertialFrame {
    type Error = InertialFrameError;

    fn try_from(frame: ReferenceFrame) -> Result<Self, Self::Error> {
        if frame.is_inertial() {
            Ok(Self(frame))
        } else {
            Err(InertialFrameError::NotExplicitlyInertial { frame })
        }
    }
}

impl From<InertialFrame> for ReferenceFrame {
    fn from(frame: InertialFrame) -> Self {
        frame.reference_frame()
    }
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

    /// Returns the axes' declared motion semantics.
    #[must_use]
    pub const fn motion(self) -> FrameMotion {
        self.orientation.motion()
    }

    /// Returns whether the axes are affirmatively classified as inertial.
    #[must_use]
    pub const fn is_inertial(self) -> bool {
        self.orientation.is_inertial()
    }
}

/// Caller-owned registry for validated parent-relative frame definitions.
///
/// The catalog is the only issuer of [`FrameId`] values. Roots must be supplied
/// explicitly, and a derived parent must already be registered in this exact
/// logical catalog. Because definitions can only reference existing parents
/// and cannot be changed under an issued ID, cycles are unrepresentable.
#[derive(Debug)]
pub struct FrameCatalog {
    namespace: FrameNamespace,
    roots: HashSet<ReferenceFrame>,
    definitions: HashMap<u64, DerivedFrame>,
}

impl FrameCatalog {
    /// Creates a catalog with the explicit reference frames it may derive from.
    pub fn new(
        namespace: FrameNamespace,
        roots: impl IntoIterator<Item = ReferenceFrame>,
    ) -> Result<Self, FrameDefinitionError> {
        let roots: HashSet<_> = roots.into_iter().collect();
        if let Some(root) = roots
            .iter()
            .copied()
            .find(|root| matches!(root.origin(), FrameOrigin::Derived(_)))
        {
            return Err(FrameDefinitionError::DerivedFrameCannotBeRoot { frame: root });
        }
        Ok(Self {
            namespace,
            roots,
            definitions: HashMap::new(),
        })
    }

    /// Returns this logical catalog's explicit namespace.
    #[must_use]
    pub const fn namespace(&self) -> FrameNamespace {
        self.namespace
    }

    /// Issues or idempotently retrieves a parent-aligned derived frame.
    ///
    /// Reusing `local_id` for a different definition is rejected. The parent
    /// must be an explicit root or an earlier definition from this catalog.
    pub fn define_parent_aligned(
        &mut self,
        local_id: u64,
        parent: ReferenceFrame,
        origin_offset: Position,
    ) -> Result<DerivedFrame, FrameDefinitionError> {
        if !origin_offset.is_finite() {
            return Err(FrameDefinitionError::NonFiniteOriginOffset);
        }
        self.validate_parent(parent)?;

        let id = FrameId {
            namespace: self.namespace,
            local_id,
        };
        let candidate = DerivedFrame {
            reference_frame: ReferenceFrame::new(FrameOrigin::Derived(id), parent.orientation()),
            parent,
            origin_offset,
        };
        if let Some(existing) = self.definitions.get(&local_id) {
            return if *existing == candidate {
                Ok(*existing)
            } else {
                Err(FrameDefinitionError::ConflictingRedefinition { id })
            };
        }
        self.definitions.insert(local_id, candidate);
        Ok(candidate)
    }

    /// Returns a definition only when its ID belongs to and exists in this catalog.
    #[must_use]
    pub fn definition(&self, id: FrameId) -> Option<DerivedFrame> {
        (id.namespace == self.namespace)
            .then(|| self.definitions.get(&id.local_id).copied())
            .flatten()
    }

    fn validate_parent(&self, parent: ReferenceFrame) -> Result<(), FrameDefinitionError> {
        match parent.origin() {
            FrameOrigin::Derived(parent_id) => {
                if parent_id.namespace != self.namespace {
                    return Err(FrameDefinitionError::ForeignDerivedParent { parent_id });
                }
                if self
                    .definitions
                    .get(&parent_id.local_id)
                    .is_some_and(|definition| definition.reference_frame() == parent)
                {
                    Ok(())
                } else {
                    Err(FrameDefinitionError::UnknownDerivedParent { parent_id })
                }
            }
            _ if self.roots.contains(&parent) => Ok(()),
            _ => Err(FrameDefinitionError::UnknownRootParent { parent }),
        }
    }
}

/// A catalog-issued frame whose axes are aligned with a direct parent frame.
///
/// `origin_offset` is the vector from the parent origin to the derived origin,
/// expressed in the parent axes. The derived frame inherits the parent's
/// orientation identity and motion classification. This type records hierarchy
/// and fixed geometry only; it performs no coordinate transformation.
///
/// ```
/// use frames::{FrameCatalog, FrameNamespace, ReferenceFrame};
/// use units::Position;
///
/// let mut catalog = FrameCatalog::new(
///     FrameNamespace::new(0x4f52534b4954),
///     [ReferenceFrame::ITRF2020],
/// )?;
/// let site = catalog.define_parent_aligned(
///     42,
///     ReferenceFrame::ITRF2020,
///     Position::from_metres(6_378_137.0, 0.0, 0.0),
/// )?;
/// assert_eq!(site.parent(), ReferenceFrame::ITRF2020);
/// # Ok::<(), frames::FrameDefinitionError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DerivedFrame {
    reference_frame: ReferenceFrame,
    parent: ReferenceFrame,
    origin_offset: Position,
}

impl DerivedFrame {
    /// Returns the opaque catalog-issued frame identity.
    #[must_use]
    pub const fn id(self) -> FrameId {
        match self.reference_frame.origin() {
            FrameOrigin::Derived(id) => id,
            _ => unreachable!(),
        }
    }

    /// Returns the identity carried by coordinate-dependent values.
    #[must_use]
    pub const fn reference_frame(self) -> ReferenceFrame {
        self.reference_frame
    }

    /// Returns the direct parent frame.
    #[must_use]
    pub const fn parent(self) -> ReferenceFrame {
        self.parent
    }

    /// Returns the parent-to-child origin offset in the parent axes.
    #[must_use]
    pub const fn origin_offset(self) -> Position {
        self.origin_offset
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
            Self::Derived(id) => write!(formatter, "DERIVED({id})"),
        }
    }
}

impl fmt::Display for FrameId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:032x}:{}",
            self.namespace.value(),
            self.local_id
        )
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
            Self::Custom { id, motion } => {
                write!(formatter, "CUSTOM({},{motion})", id.value())
            }
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

/// Invalid parent-relative frame definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum FrameDefinitionError {
    /// At least one parent-frame offset component is NaN or infinite.
    #[error("derived-frame origin offset must be finite")]
    NonFiniteOriginOffset,
    /// A derived frame cannot bypass catalog validation by being declared a root.
    #[error("derived frame {frame} cannot be registered as a catalog root")]
    DerivedFrameCannotBeRoot {
        /// Rejected root frame.
        frame: ReferenceFrame,
    },
    /// The requested non-derived parent was not declared as a catalog root.
    #[error("frame {parent} is not a known root of this frame catalog")]
    UnknownRootParent {
        /// Rejected parent frame.
        parent: ReferenceFrame,
    },
    /// A derived parent belongs to a different catalog namespace.
    #[error("derived parent {parent_id} belongs to a foreign frame catalog")]
    ForeignDerivedParent {
        /// Foreign parent identity.
        parent_id: FrameId,
    },
    /// A same-namespace derived parent is not registered with this catalog.
    #[error("derived parent {parent_id} is not registered in this frame catalog")]
    UnknownDerivedParent {
        /// Unknown parent identity.
        parent_id: FrameId,
    },
    /// An issued local key was reused for a different immutable definition.
    #[error("frame {id} is already defined differently")]
    ConflictingRedefinition {
        /// Conflicting issued identity.
        id: FrameId,
    },
}

/// A reference frame does not satisfy an affirmative inertial-axis requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum InertialFrameError {
    /// The orientation is non-inertial or has unspecified motion semantics.
    #[error("reference frame {frame} does not have affirmatively inertial axes")]
    NotExplicitlyInertial {
        /// Rejected reference frame.
        frame: ReferenceFrame,
    },
}

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

    #[test]
    fn inertial_eligibility_is_affirmative() {
        assert_eq!(ReferenceFrame::ICRF.motion(), FrameMotion::Inertial);
        assert_eq!(ReferenceFrame::GCRF.motion(), FrameMotion::Inertial);
        assert_eq!(ReferenceFrame::EME2000.motion(), FrameMotion::Inertial);
        assert_eq!(ReferenceFrame::ITRF2020.motion(), FrameMotion::NonInertial);
        assert_eq!(ReferenceFrame::TEME.motion(), FrameMotion::NonInertial);

        let id = CustomFrameId::new(42);
        let unspecified = FrameOrientation::custom(id, FrameMotion::Unspecified);
        let inertial = FrameOrientation::custom(id, FrameMotion::Inertial);
        assert!(!unspecified.is_inertial());
        assert!(inertial.is_inertial());
        assert_eq!(unspecified.to_string(), "CUSTOM(42,UNSPECIFIED)");
    }

    #[test]
    fn inertial_frame_capability_rejects_terrestrial_and_unspecified_axes() {
        assert_eq!(
            InertialFrame::try_from(ReferenceFrame::GCRF),
            Ok(InertialFrame::GCRF)
        );

        assert_eq!(
            InertialFrame::try_from(ReferenceFrame::ITRF2020),
            Err(InertialFrameError::NotExplicitlyInertial {
                frame: ReferenceFrame::ITRF2020,
            })
        );

        let id = CustomFrameId::new(43);
        let unspecified = ReferenceFrame::new(
            FrameOrigin::Body(Body::EARTH),
            FrameOrientation::custom(id, FrameMotion::Unspecified),
        );
        assert_eq!(
            InertialFrame::try_from(unspecified),
            Err(InertialFrameError::NotExplicitlyInertial { frame: unspecified })
        );
    }

    #[test]
    fn parent_aligned_frame_retains_parent_and_typed_offset() {
        let offset = Position::from_metres(6_378_137.0, 0.0, 0.0);
        let mut catalog = FrameCatalog::new(FrameNamespace::new(1), [ReferenceFrame::ITRF2020])
            .expect("root catalog");
        let site = catalog
            .define_parent_aligned(1001, ReferenceFrame::ITRF2020, offset)
            .expect("finite fixed site");

        assert_eq!(site.parent(), ReferenceFrame::ITRF2020);
        assert_eq!(site.origin_offset(), offset);
        assert_eq!(site.id().namespace(), FrameNamespace::new(1));
        assert_eq!(site.id().local_id(), 1001);
        assert_eq!(
            site.reference_frame().origin(),
            FrameOrigin::Derived(site.id())
        );
        assert_eq!(
            site.reference_frame().orientation(),
            ReferenceFrame::ITRF2020.orientation()
        );
        assert_eq!(site.reference_frame().motion(), FrameMotion::NonInertial);
    }

    #[test]
    fn catalog_namespaces_prevent_same_local_id_collisions() {
        let mut left = FrameCatalog::new(FrameNamespace::new(10), [ReferenceFrame::ITRF2020])
            .expect("left catalog");
        let mut right = FrameCatalog::new(FrameNamespace::new(11), [ReferenceFrame::ITRF2020])
            .expect("right catalog");
        let define = |catalog: &mut FrameCatalog| {
            catalog
                .define_parent_aligned(
                    7,
                    ReferenceFrame::ITRF2020,
                    Position::from_metres(1.0, 2.0, 3.0),
                )
                .expect("valid definition")
        };
        let left_frame = define(&mut left);
        let right_frame = define(&mut right);

        assert_ne!(left_frame.id(), right_frame.id());
        assert_ne!(left_frame.reference_frame(), right_frame.reference_frame());
    }

    #[test]
    fn derived_frames_form_only_registered_acyclic_parent_chains() {
        let namespace = FrameNamespace::new(20);
        let mut catalog =
            FrameCatalog::new(namespace, [ReferenceFrame::ITRF2020]).expect("root catalog");
        let site = catalog
            .define_parent_aligned(
                1001,
                ReferenceFrame::ITRF2020,
                Position::from_metres(6_378_137.0, 0.0, 0.0),
            )
            .expect("site frame");
        let instrument = catalog
            .define_parent_aligned(
                1002,
                site.reference_frame(),
                Position::from_metres(0.0, 0.0, 2.0),
            )
            .expect("instrument frame");

        assert_eq!(instrument.parent(), site.reference_frame());
        assert_eq!(catalog.definition(site.id()), Some(site));
        assert_eq!(catalog.definition(instrument.id()), Some(instrument));
        assert_eq!(
            instrument.reference_frame().orientation(),
            FrameOrientation::Itrf(2020)
        );

        let mut same_namespace_other_instance =
            FrameCatalog::new(namespace, [ReferenceFrame::ITRF2020]).expect("separate replica");
        assert_eq!(
            same_namespace_other_instance.define_parent_aligned(
                1002,
                site.reference_frame(),
                Position::from_metres(0.0, 0.0, 2.0),
            ),
            Err(FrameDefinitionError::UnknownDerivedParent {
                parent_id: site.id(),
            })
        );
    }

    #[test]
    fn catalog_rejects_foreign_parents_and_conflicting_redefinitions() {
        let mut first = FrameCatalog::new(FrameNamespace::new(30), [ReferenceFrame::ITRF2020])
            .expect("first catalog");
        let parent = first
            .define_parent_aligned(
                1,
                ReferenceFrame::ITRF2020,
                Position::from_metres(1.0, 0.0, 0.0),
            )
            .expect("parent frame");
        assert_eq!(
            first.define_parent_aligned(
                1,
                ReferenceFrame::ITRF2020,
                Position::from_metres(2.0, 0.0, 0.0),
            ),
            Err(FrameDefinitionError::ConflictingRedefinition { id: parent.id() })
        );

        let mut second = FrameCatalog::new(FrameNamespace::new(31), [ReferenceFrame::ITRF2020])
            .expect("second catalog");
        assert_eq!(
            second.define_parent_aligned(
                2,
                parent.reference_frame(),
                Position::from_metres(0.0, 0.0, 1.0),
            ),
            Err(FrameDefinitionError::ForeignDerivedParent {
                parent_id: parent.id(),
            })
        );
        assert!(matches!(
            FrameCatalog::new(FrameNamespace::new(32), [parent.reference_frame()]),
            Err(FrameDefinitionError::DerivedFrameCannotBeRoot { frame })
                if frame == parent.reference_frame()
        ));
    }

    #[test]
    fn catalog_rejects_unknown_roots_and_invalid_geometry() {
        let mut catalog = FrameCatalog::new(FrameNamespace::new(40), [ReferenceFrame::ITRF2020])
            .expect("root catalog");
        assert_eq!(
            catalog.define_parent_aligned(
                1,
                ReferenceFrame::ITRF2020,
                Position::from_metres(f64::NAN, 0.0, 0.0),
            ),
            Err(FrameDefinitionError::NonFiniteOriginOffset)
        );
        assert_eq!(
            catalog.define_parent_aligned(
                2,
                ReferenceFrame::GCRF,
                Position::from_metres(0.0, 0.0, 0.0),
            ),
            Err(FrameDefinitionError::UnknownRootParent {
                parent: ReferenceFrame::GCRF,
            })
        );
    }
}
