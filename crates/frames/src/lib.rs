#![forbid(unsafe_code)]

//! Reference-frame identities for orskit.
//!
//! A frame identity is modeled as a body-backed, barycentric, or custom origin
//! plus an orientation. A [`DerivedFrame`] associates a parent-aligned custom
//! identity with a fixed origin offset expressed in its parent frame.
//! [`TopocentricFrame`] adds caller-selected ellipsoid geometry for a local
//! East–North–Up frame under an affirmatively body-fixed parent.
//! Neither type implies an epoch-dependent transform.
//! Orientations explicitly declare whether their axes are inertial,
//! non-inertial, or unspecified. Kinematic transform providers make their
//! epoch and data dependencies explicit. A
//! [`FrameReferenceDataSupplier`] carries the selected reference-data identity
//! and can back a transform provider without making this foundational crate
//! load files, fetch data, or select an Earth-orientation model implicitly.

#[cfg(feature = "earth-orientation")]
mod earth_orientation;

use std::collections::{HashMap, HashSet};
use std::error::Error as StdError;
use std::sync::atomic::{AtomicU64, Ordering};
use std::{fmt, str::FromStr};

pub use bodies::{Body, BodySystem, CustomBodyId, GeodeticPosition, ReferenceEllipsoid};
#[cfg(feature = "earth-orientation")]
pub use earth_orientation::{
    EarthOrientationConvention, EarthOrientationError, EarthOrientationSample,
    Iers2010EarthOrientation,
};
use hifitime::Epoch;
pub use orskit_data::ArtifactDescriptor as ReferenceDataDescriptor;
use thiserror::Error;
use units::uom::si::angle::radian;
use units::{Position, VelocityVector};

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
/// UUID encoded as `u128`. Each catalog instance is nevertheless a distinct
/// issuing authority; recreating a catalog does not recreate issued IDs.
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
/// Identity includes the catalog namespace, an unforgeable process-local
/// issuing-authority token, and the catalog-local key. Callers can inspect the
/// namespace/key but cannot construct a `FrameId` directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FrameId {
    namespace: FrameNamespace,
    issuer_id: u64,
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
    /// Application-defined axes fixed to their frame-origin body.
    BodyFixed(CustomFrameId),
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

    /// Constructs application-defined axes affirmatively fixed to their body.
    #[must_use]
    pub const fn body_fixed(id: CustomFrameId) -> Self {
        Self::BodyFixed(id)
    }

    /// Returns the axes' declared motion semantics.
    #[must_use]
    pub const fn motion(self) -> FrameMotion {
        match self {
            Self::Icrf | Self::Gcrf | Self::Eme2000 => FrameMotion::Inertial,
            Self::Itrf(_)
            | Self::Teme
            | Self::Mod
            | Self::Tod
            | Self::Gtod
            | Self::BodyFixed(_) => FrameMotion::NonInertial,
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

    /// Whether these axes are affirmatively fixed to their origin body.
    ///
    /// ITRF realizations are Earth-specific. Other bodies require an explicit
    /// application-defined body-fixed orientation.
    #[must_use]
    pub const fn is_body_fixed(self) -> bool {
        matches!(
            (self.origin, self.orientation),
            (FrameOrigin::Body(Body::EARTH), FrameOrientation::Itrf(_))
                | (FrameOrigin::Body(_), FrameOrientation::BodyFixed(_))
        )
    }

    /// Returns whether the axes are affirmatively classified as inertial.
    #[must_use]
    pub const fn is_inertial(self) -> bool {
        self.orientation.is_inertial()
    }
}

/// Finite position and velocity expressed in one declared reference frame.
///
/// This is the data boundary for a [`KinematicFrameTransformProvider`]. It
/// carries no implicit origin, orientation, epoch, or Earth-orientation data;
/// callers supply the epoch to each transform evaluation explicitly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameKinematics {
    position: Position,
    velocity: VelocityVector,
    frame: ReferenceFrame,
}

impl FrameKinematics {
    /// Creates finite kinematics expressed in `frame`.
    pub fn new(
        position: Position,
        velocity: VelocityVector,
        frame: ReferenceFrame,
    ) -> Result<Self, FrameKinematicsError> {
        if !position.is_finite() {
            return Err(FrameKinematicsError::NonFinitePosition);
        }
        if !velocity.is_finite() {
            return Err(FrameKinematicsError::NonFiniteVelocity);
        }
        Ok(Self {
            position,
            velocity,
            frame,
        })
    }

    /// Returns the expressed position.
    #[must_use]
    pub const fn position(self) -> Position {
        self.position
    }

    /// Returns the expressed velocity.
    #[must_use]
    pub const fn velocity(self) -> VelocityVector {
        self.velocity
    }

    /// Returns the expression frame.
    #[must_use]
    pub const fn frame(self) -> ReferenceFrame {
        self.frame
    }
}

/// Invalid kinematic transform input or output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum FrameKinematicsError {
    /// A position component is NaN or infinite.
    #[error("frame kinematics position components must be finite")]
    NonFinitePosition,
    /// A velocity component is NaN or infinite.
    #[error("frame kinematics velocity components must be finite")]
    NonFiniteVelocity,
}

/// Resolves kinematics between explicitly declared reference frames.
///
/// Implementations own all required Earth-orientation, ephemeris, rotation,
/// translation, and velocity-transform data. They must return kinematics in
/// `target`; consumers verify that result rather than trusting a provider's
/// declaration.
pub trait KinematicFrameTransformProvider: fmt::Debug + Send + Sync {
    /// Provider-specific failure, for example missing Earth-orientation data.
    type Error: StdError + Send + Sync + 'static;

    /// Transforms `kinematics` at `epoch` into `target`.
    fn transform(
        &self,
        epoch: Epoch,
        kinematics: FrameKinematics,
        target: ReferenceFrame,
    ) -> Result<FrameKinematics, Self::Error>;
}

/// Supplies reference-data-backed kinematic frame solutions.
///
/// This is an application-controlled data boundary: implementations own data
/// loading, coverage checks, interpolation, caching, and the selected
/// conventions. Each result must include all origin translation, orientation,
/// and velocity terms needed for the requested frame conversion. The trait
/// deliberately returns high-level [`FrameKinematics`] rather than exposing a
/// public matrix kernel.
///
/// A recommended production implementation combines a pinned JPL/NAIF SPK
/// planetary ephemeris (for example DE440) with a pinned IERS Earth-orientation
/// series and one declared IAU/IERS convention set. JPL ephemerides alone do
/// not realize the terrestrial-to-celestial orientation; implementations must
/// report every selected input through [`Self::reference_data`]. Other valid
/// implementations include a mission-specific validated data bundle or an
/// adapter to an independently managed almanac.
pub trait FrameReferenceDataSupplier: fmt::Debug + Send + Sync {
    /// Failure returned for missing data, unsupported frames, invalid coverage,
    /// or supplier-specific evaluation errors.
    type Error: StdError + Send + Sync + 'static;

    /// Returns every immutable reference-data artifact selected by this supplier.
    ///
    /// Implementations must return at least one descriptor for a distinct-frame
    /// request. A production terrestrial/celestial supplier normally reports
    /// separate ephemeris, Earth-orientation, and convention artifacts.
    /// The returned artifact set and every descriptor value must remain stable for
    /// the supplier's lifetime; an implementation may cache derived evaluations,
    /// but it must not silently replace its selected scientific data.
    fn reference_data(&self) -> &[ReferenceDataDescriptor];

    /// Resolves `kinematics` at `epoch` into `target` using the selected data.
    fn transform_kinematics(
        &self,
        epoch: Epoch,
        kinematics: FrameKinematics,
        target: ReferenceFrame,
    ) -> Result<FrameKinematics, Self::Error>;
}

/// [`KinematicFrameTransformProvider`] backed by one explicit reference-data supplier.
///
/// Construct this adapter with [`From`]. It preserves an identity transform
/// without querying the supplier, delegates every distinct-frame request to
/// the supplier, and verifies that the returned kinematics carry the requested
/// target frame. Use [`AsRef`] to inspect the supplier's provenance after
/// construction.
#[derive(Debug)]
pub struct ReferenceDataKinematicFrameTransform<S> {
    supplier: S,
}

impl<S> From<S> for ReferenceDataKinematicFrameTransform<S> {
    fn from(supplier: S) -> Self {
        Self { supplier }
    }
}

impl<S> AsRef<S> for ReferenceDataKinematicFrameTransform<S> {
    fn as_ref(&self) -> &S {
        &self.supplier
    }
}

impl<S: FrameReferenceDataSupplier> KinematicFrameTransformProvider
    for ReferenceDataKinematicFrameTransform<S>
{
    type Error = ReferenceDataKinematicFrameTransformError<S::Error>;

    fn transform(
        &self,
        epoch: Epoch,
        kinematics: FrameKinematics,
        target: ReferenceFrame,
    ) -> Result<FrameKinematics, Self::Error> {
        if kinematics.frame() == target {
            return Ok(kinematics);
        }
        let reference_data = self.supplier.reference_data();
        if reference_data.is_empty() {
            return Err(ReferenceDataKinematicFrameTransformError::MissingReferenceData);
        }
        let transformed = self
            .supplier
            .transform_kinematics(epoch, kinematics, target)
            .map_err(
                |source| ReferenceDataKinematicFrameTransformError::Supplier {
                    source: Box::new(source),
                },
            )?;
        if transformed.frame() != target {
            return Err(
                ReferenceDataKinematicFrameTransformError::OutputFrameMismatch {
                    expected: target,
                    actual: transformed.frame(),
                },
            );
        }
        Ok(transformed)
    }
}

/// Failure from [`ReferenceDataKinematicFrameTransform`].
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ReferenceDataKinematicFrameTransformError<E: StdError + Send + Sync + 'static> {
    /// A distinct-frame request was attempted without any declared reference data.
    #[error("reference-data supplier declared no reference-data artifacts")]
    MissingReferenceData,
    /// The selected reference-data supplier could not evaluate the request.
    #[error("reference-data supplier failed to resolve the requested frame transform")]
    Supplier {
        /// Supplier-specific source failure. This allocation occurs only on an
        /// error path, keeping successful transformation results compact.
        #[source]
        source: Box<E>,
    },
    /// The supplier did not return kinematics in the requested target frame.
    #[error("reference-data supplier returned {actual:?}, expected {expected:?}")]
    OutputFrameMismatch {
        /// Target frame requested from the transform provider.
        expected: ReferenceFrame,
        /// Frame carried by the supplier's returned kinematics.
        actual: ReferenceFrame,
    },
}

/// Transform provider that accepts only an identity transform.
///
/// This is useful when an API requires an explicit transform boundary but a
/// workflow has already selected one common frame. It never treats distinct
/// frame identities as equivalent.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct IdentityKinematicFrameTransform;

impl KinematicFrameTransformProvider for IdentityKinematicFrameTransform {
    type Error = IdentityKinematicFrameTransformError;

    fn transform(
        &self,
        _epoch: Epoch,
        kinematics: FrameKinematics,
        target: ReferenceFrame,
    ) -> Result<FrameKinematics, Self::Error> {
        if kinematics.frame() == target {
            Ok(kinematics)
        } else {
            Err(IdentityKinematicFrameTransformError::FrameMismatch {
                from: kinematics.frame(),
                target,
            })
        }
    }
}

/// Failure from [`IdentityKinematicFrameTransform`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum IdentityKinematicFrameTransformError {
    /// An identity transform was requested for distinct frame identities.
    #[error("identity transform cannot convert {from:?} into {target:?}")]
    FrameMismatch {
        /// Frame carried by the input kinematics.
        from: ReferenceFrame,
        /// Requested result frame.
        target: ReferenceFrame,
    },
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
    issuer_id: u64,
    roots: HashSet<ReferenceFrame>,
    definitions: HashMap<u64, DerivedFrame>,
    topocentric_definitions: HashMap<u64, TopocentricFrame>,
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
            issuer_id: next_catalog_issuer()?,
            roots,
            definitions: HashMap::new(),
            topocentric_definitions: HashMap::new(),
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
            issuer_id: self.issuer_id,
            local_id,
        };
        let candidate = DerivedFrame {
            reference_frame: ReferenceFrame::new(FrameOrigin::Derived(id), parent.orientation()),
            parent,
            origin_offset,
        };
        if self.topocentric_definitions.contains_key(&local_id) {
            return Err(FrameDefinitionError::ConflictingRedefinition { id });
        }
        if let Some(existing) = self.definitions.get(&local_id) {
            return if *existing == candidate {
                Ok(*existing)
            } else {
                Err(FrameDefinitionError::ConflictingRedefinition { id: existing.id() })
            };
        }
        self.definitions.insert(local_id, candidate);
        Ok(candidate)
    }

    /// Issues or idempotently retrieves an East–North–Up topocentric frame.
    ///
    /// `parent` must be a registered, affirmatively body-fixed frame whose
    /// conventional geocentric axes match `ellipsoid`: `+Z` is north, `+X`
    /// crosses the equator at zero longitude, and `+Y` crosses it at 90° east.
    /// The geodetic origin's longitude therefore fixes the east/north axes even
    /// at a pole.
    pub fn define_topocentric_enu(
        &mut self,
        local_id: u64,
        parent: ReferenceFrame,
        ellipsoid: ReferenceEllipsoid,
        origin: GeodeticPosition,
    ) -> Result<TopocentricFrame, FrameDefinitionError> {
        self.validate_parent(parent)?;
        if parent.origin() != FrameOrigin::Body(ellipsoid.body()) {
            return Err(FrameDefinitionError::EllipsoidBodyMismatch {
                parent,
                ellipsoid_body: ellipsoid.body(),
            });
        }
        if !parent.is_body_fixed() {
            return Err(FrameDefinitionError::TopocentricParentMustBeBodyFixed { parent });
        }

        let id = FrameId {
            namespace: self.namespace,
            issuer_id: self.issuer_id,
            local_id,
        };
        let candidate = TopocentricFrame {
            reference_frame: ReferenceFrame::new(
                FrameOrigin::Derived(id),
                FrameOrientation::body_fixed(CustomFrameId::new(local_id)),
            ),
            parent,
            origin,
            origin_offset: ellipsoid.to_geocentric(origin),
        };
        if self.definitions.contains_key(&local_id) {
            return Err(FrameDefinitionError::ConflictingRedefinition { id });
        }
        if let Some(existing) = self.topocentric_definitions.get(&local_id) {
            return if *existing == candidate {
                Ok(*existing)
            } else {
                Err(FrameDefinitionError::ConflictingRedefinition { id })
            };
        }
        self.topocentric_definitions.insert(local_id, candidate);
        Ok(candidate)
    }

    /// Returns a definition only when its ID belongs to and exists in this catalog.
    #[must_use]
    pub fn definition(&self, id: FrameId) -> Option<DerivedFrame> {
        (id.namespace == self.namespace && id.issuer_id == self.issuer_id)
            .then(|| self.definitions.get(&id.local_id).copied())
            .flatten()
            .filter(|definition| definition.id() == id)
    }

    /// Returns a topocentric definition only when its ID belongs to this
    /// catalog.
    #[must_use]
    pub fn topocentric_definition(&self, id: FrameId) -> Option<TopocentricFrame> {
        (id.namespace == self.namespace && id.issuer_id == self.issuer_id)
            .then(|| self.topocentric_definitions.get(&id.local_id).copied())
            .flatten()
            .filter(|definition| definition.id() == id)
    }

    fn validate_parent(&self, parent: ReferenceFrame) -> Result<(), FrameDefinitionError> {
        match parent.origin() {
            FrameOrigin::Derived(parent_id) => {
                if parent_id.namespace != self.namespace || parent_id.issuer_id != self.issuer_id {
                    return Err(FrameDefinitionError::ForeignDerivedParent { parent_id });
                }
                let registered = self
                    .definitions
                    .get(&parent_id.local_id)
                    .is_some_and(|definition| definition.reference_frame() == parent)
                    || self
                        .topocentric_definitions
                        .get(&parent_id.local_id)
                        .is_some_and(|definition| definition.reference_frame() == parent);
                if registered {
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

/// Catalog-issued local East–North–Up frame at one geodetic origin.
///
/// East is tangent to increasing east-positive longitude, north is tangent to
/// increasing geodetic latitude, and up follows the outward ellipsoid normal.
/// Position transforms are affine: parent coordinates are translated by the
/// geocentric origin and then rotated. No Earth-orientation or epoch transform
/// is implied; `parent` is already a caller-selected body-fixed frame.
///
/// ```
/// use bodies::{GeodeticPosition, ReferenceEllipsoid};
/// use frames::{FrameCatalog, FrameNamespace, ReferenceFrame};
/// use units::uom::si::{angle::degree, length::meter};
/// use units::{Angle, Length, Position};
///
/// let origin = GeodeticPosition::new(
///     Angle::new::<degree>(5.0),
///     Angle::new::<degree>(55.0),
///     Length::new::<meter>(200.0),
/// )?;
/// let mut catalog = FrameCatalog::new(
///     FrameNamespace::new(1),
///     [ReferenceFrame::ITRF2020],
/// )?;
/// let enu = catalog.define_topocentric_enu(
///     7,
///     ReferenceFrame::ITRF2020,
///     ReferenceEllipsoid::wgs84(),
///     origin,
/// )?;
///
/// let local = enu.to_topocentric_position(enu.origin_offset());
/// assert_eq!(local, Position::from_metres(0.0, 0.0, 0.0));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TopocentricFrame {
    reference_frame: ReferenceFrame,
    parent: ReferenceFrame,
    origin: GeodeticPosition,
    origin_offset: Position,
}

impl TopocentricFrame {
    /// Returns the catalog-issued identity.
    #[must_use]
    pub const fn id(self) -> FrameId {
        match self.reference_frame.origin() {
            FrameOrigin::Derived(id) => id,
            _ => unreachable!(),
        }
    }

    /// Returns the local frame identity carried by local coordinates.
    #[must_use]
    pub const fn reference_frame(self) -> ReferenceFrame {
        self.reference_frame
    }

    /// Returns the body-fixed geocentric parent.
    #[must_use]
    pub const fn parent(self) -> ReferenceFrame {
        self.parent
    }

    /// Returns the geodetic origin, including its explicit polar meridian.
    #[must_use]
    pub const fn origin(self) -> GeodeticPosition {
        self.origin
    }

    /// Returns the parent-to-local-origin vector in parent axes.
    #[must_use]
    pub const fn origin_offset(self) -> Position {
        self.origin_offset
    }

    /// Converts a position in parent axes to local `(east, north, up)`.
    #[must_use]
    pub fn to_topocentric_position(self, parent_position: Position) -> Position {
        let [east, north, up] =
            self.rotate_to_topocentric((parent_position - self.origin_offset).to_metres());
        Position::from_metres(east, north, up)
    }

    /// Converts local `(east, north, up)` to a position in parent axes.
    #[must_use]
    pub fn to_parent_position(self, topocentric_position: Position) -> Position {
        let [x, y, z] = self.rotate_to_parent(topocentric_position.to_metres());
        let [origin_x, origin_y, origin_z] = self.origin_offset.to_metres();
        Position::from_metres(origin_x + x, origin_y + y, origin_z + z)
    }

    fn rotate_to_topocentric(self, [x, y, z]: [f64; 3]) -> [f64; 3] {
        let longitude = self.origin.longitude().get::<radian>();
        let latitude = self.origin.latitude().get::<radian>();
        let (sin_longitude, cos_longitude) = longitude.sin_cos();
        let (sin_latitude, cos_latitude) = latitude.sin_cos();

        [
            -x * sin_longitude + y * cos_longitude,
            -x * sin_latitude * cos_longitude - y * sin_latitude * sin_longitude + z * cos_latitude,
            x * cos_latitude * cos_longitude + y * cos_latitude * sin_longitude + z * sin_latitude,
        ]
    }

    fn rotate_to_parent(self, [east, north, up]: [f64; 3]) -> [f64; 3] {
        let longitude = self.origin.longitude().get::<radian>();
        let latitude = self.origin.latitude().get::<radian>();
        let (sin_longitude, cos_longitude) = longitude.sin_cos();
        let (sin_latitude, cos_latitude) = latitude.sin_cos();

        [
            -east * sin_longitude - north * sin_latitude * cos_longitude
                + up * cos_latitude * cos_longitude,
            east * cos_longitude - north * sin_latitude * sin_longitude
                + up * cos_latitude * sin_longitude,
            north * cos_latitude + up * sin_latitude,
        ]
    }
}

impl KinematicFrameTransformProvider for TopocentricFrame {
    type Error = TopocentricTransformError;

    fn transform(
        &self,
        _epoch: Epoch,
        kinematics: FrameKinematics,
        target: ReferenceFrame,
    ) -> Result<FrameKinematics, Self::Error> {
        let source = kinematics.frame();
        if source == target {
            return Ok(kinematics);
        }

        let (position, velocity) = if source == self.parent && target == self.reference_frame {
            let position = self.to_topocentric_position(kinematics.position());
            let velocity = self.rotate_to_topocentric(kinematics.velocity().to_metres_per_second());
            (
                position,
                VelocityVector::from_metres_per_second(velocity[0], velocity[1], velocity[2]),
            )
        } else if source == self.reference_frame && target == self.parent {
            let position = self.to_parent_position(kinematics.position());
            let velocity = self.rotate_to_parent(kinematics.velocity().to_metres_per_second());
            (
                position,
                VelocityVector::from_metres_per_second(velocity[0], velocity[1], velocity[2]),
            )
        } else {
            return Err(TopocentricTransformError::UnsupportedTransform {
                frames: Box::new((source, target)),
            });
        };

        Ok(FrameKinematics::new(position, velocity, target)?)
    }
}

/// Failure while applying a catalog-issued topocentric transform.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum TopocentricTransformError {
    /// The requested frames are not this definition's parent/local pair.
    #[error("unsupported topocentric transform {frames:?}")]
    UnsupportedTransform {
        /// Requested input and output frames.
        frames: Box<(ReferenceFrame, ReferenceFrame)>,
    },
    /// Rotated kinematics became non-finite.
    #[error("topocentric transform produced invalid kinematics")]
    InvalidKinematics(#[from] FrameKinematicsError),
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
            "{:032x}:{:016x}:{}",
            self.namespace.value(),
            self.issuer_id,
            self.local_id,
        )
    }
}

static NEXT_CATALOG_ISSUER: AtomicU64 = AtomicU64::new(1);

fn next_catalog_issuer() -> Result<u64, FrameDefinitionError> {
    NEXT_CATALOG_ISSUER
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .map_err(|_| FrameDefinitionError::IssuerSpaceExhausted)
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
            Self::BodyFixed(id) => write!(formatter, "BODY_FIXED({})", id.value()),
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
    /// This process exhausted the non-reusable catalog issuing-authority space.
    #[error("frame catalog issuing-authority space is exhausted")]
    IssuerSpaceExhausted,
    /// At least one parent-frame offset component is NaN or infinite.
    #[error("derived-frame origin offset must be finite")]
    NonFiniteOriginOffset,
    /// The ellipsoid body did not match the body-centered parent origin.
    #[error("ellipsoid body {ellipsoid_body} does not match topocentric parent {parent}")]
    EllipsoidBodyMismatch {
        /// Rejected parent.
        parent: ReferenceFrame,
        /// Body modeled by the ellipsoid.
        ellipsoid_body: Body,
    },
    /// Topocentric axes require an affirmatively body-fixed parent.
    #[error("topocentric parent {parent} is not affirmatively body-fixed")]
    TopocentricParentMustBeBodyFixed {
        /// Rejected parent.
        parent: ReferenceFrame,
    },
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
    /// A derived parent belongs to a different catalog issuing authority.
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
    use orskit_data::{ArtifactCoverage, Sha256Digest};
    use units::uom::si::{angle::degree, length::meter, ratio::ratio};
    use units::{Angle, Length, Ratio};

    fn descriptor() -> ReferenceDataDescriptor {
        ReferenceDataDescriptor::new(
            "test authority",
            "test reference data",
            "test revision",
            Sha256Digest::compute(b"test reference data"),
            ArtifactCoverage::AllTime,
        )
        .expect("valid test reference-data descriptor")
    }

    #[derive(Debug)]
    struct OffsetSupplier {
        reference_data: Vec<ReferenceDataDescriptor>,
    }

    impl FrameReferenceDataSupplier for OffsetSupplier {
        type Error = std::convert::Infallible;

        fn reference_data(&self) -> &[ReferenceDataDescriptor] {
            &self.reference_data
        }

        fn transform_kinematics(
            &self,
            _epoch: Epoch,
            kinematics: FrameKinematics,
            target: ReferenceFrame,
        ) -> Result<FrameKinematics, Self::Error> {
            let position = kinematics.position().to_metres();
            Ok(FrameKinematics::new(
                Position::from_metres(position[0] + 100.0, position[1], position[2]),
                kinematics.velocity(),
                target,
            )
            .expect("finite transformed state"))
        }
    }

    #[derive(Debug)]
    struct CanonicalOffsetSupplier {
        reference_data: Vec<ReferenceDataDescriptor>,
    }

    impl CanonicalOffsetSupplier {
        fn origin(frame: ReferenceFrame) -> ([f64; 3], [f64; 3]) {
            match frame {
                ReferenceFrame::ITRF2020 => ([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
                ReferenceFrame::GCRF => ([128.0, -64.0, 32.0], [8.0, -4.0, 2.0]),
                ReferenceFrame::EME2000 => ([-16.0, 8.0, -4.0], [-1.0, 0.5, -0.25]),
                _ => unreachable!("the deterministic test supplier supports three frames"),
            }
        }
    }

    impl FrameReferenceDataSupplier for CanonicalOffsetSupplier {
        type Error = std::convert::Infallible;

        fn reference_data(&self) -> &[ReferenceDataDescriptor] {
            &self.reference_data
        }

        fn transform_kinematics(
            &self,
            _epoch: Epoch,
            kinematics: FrameKinematics,
            target: ReferenceFrame,
        ) -> Result<FrameKinematics, Self::Error> {
            let (source_position, source_velocity) = Self::origin(kinematics.frame());
            let (target_position, target_velocity) = Self::origin(target);
            let position = kinematics.position().to_metres();
            let velocity = kinematics.velocity().to_metres_per_second();
            Ok(FrameKinematics::new(
                Position::from_metres(
                    position[0] + source_position[0] - target_position[0],
                    position[1] + source_position[1] - target_position[1],
                    position[2] + source_position[2] - target_position[2],
                ),
                VelocityVector::from_metres_per_second(
                    velocity[0] + source_velocity[0] - target_velocity[0],
                    velocity[1] + source_velocity[1] - target_velocity[1],
                    velocity[2] + source_velocity[2] - target_velocity[2],
                ),
                target,
            )
            .expect("deterministic affine transform remains finite"))
        }
    }

    #[derive(Debug)]
    struct MislabelledSupplier {
        reference_data: Vec<ReferenceDataDescriptor>,
    }

    impl FrameReferenceDataSupplier for MislabelledSupplier {
        type Error = std::convert::Infallible;

        fn reference_data(&self) -> &[ReferenceDataDescriptor] {
            &self.reference_data
        }

        fn transform_kinematics(
            &self,
            _epoch: Epoch,
            kinematics: FrameKinematics,
            _target: ReferenceFrame,
        ) -> Result<FrameKinematics, Self::Error> {
            Ok(kinematics)
        }
    }

    #[derive(Debug, Error)]
    #[error("reference data do not cover the requested epoch")]
    struct CoverageError;

    #[derive(Debug)]
    struct RejectingSupplier {
        reference_data: Vec<ReferenceDataDescriptor>,
    }

    impl FrameReferenceDataSupplier for RejectingSupplier {
        type Error = CoverageError;

        fn reference_data(&self) -> &[ReferenceDataDescriptor] {
            &self.reference_data
        }

        fn transform_kinematics(
            &self,
            _epoch: Epoch,
            _kinematics: FrameKinematics,
            _target: ReferenceFrame,
        ) -> Result<FrameKinematics, Self::Error> {
            Err(CoverageError)
        }
    }

    #[derive(Debug)]
    struct EmptySupplier;

    impl FrameReferenceDataSupplier for EmptySupplier {
        type Error = std::convert::Infallible;

        fn reference_data(&self) -> &[ReferenceDataDescriptor] {
            &[]
        }

        fn transform_kinematics(
            &self,
            _epoch: Epoch,
            _kinematics: FrameKinematics,
            _target: ReferenceFrame,
        ) -> Result<FrameKinematics, Self::Error> {
            unreachable!("the adapter must reject an empty reference-data set first")
        }
    }

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
    fn conflicting_replicas_cannot_issue_equal_frame_identities() {
        let namespace = FrameNamespace::new(12);
        let mut left =
            FrameCatalog::new(namespace, [ReferenceFrame::ITRF2020]).expect("left catalog");
        let mut right =
            FrameCatalog::new(namespace, [ReferenceFrame::ITRF2020]).expect("right catalog");
        let left_frame = left
            .define_parent_aligned(
                7,
                ReferenceFrame::ITRF2020,
                Position::from_metres(1.0, 2.0, 3.0),
            )
            .expect("left definition");
        let right_frame = right
            .define_parent_aligned(
                7,
                ReferenceFrame::ITRF2020,
                Position::from_metres(4.0, 5.0, 6.0),
            )
            .expect("right definition");

        assert_ne!(left_frame.id(), right_frame.id());
        assert_ne!(left_frame.reference_frame(), right_frame.reference_frame());
        assert_eq!(left.definition(right_frame.id()), None);
    }

    #[test]
    fn separate_catalog_instances_are_distinct_issuing_authorities() {
        let namespace = FrameNamespace::new(13);
        let mut left =
            FrameCatalog::new(namespace, [ReferenceFrame::ITRF2020]).expect("left catalog");
        let mut right =
            FrameCatalog::new(namespace, [ReferenceFrame::ITRF2020]).expect("right catalog");
        let define = |catalog: &mut FrameCatalog| {
            catalog
                .define_parent_aligned(
                    7,
                    ReferenceFrame::ITRF2020,
                    Position::from_metres(1.0, 2.0, 3.0),
                )
                .expect("valid definition")
        };

        assert_ne!(define(&mut left).id(), define(&mut right).id());
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
            Err(FrameDefinitionError::ForeignDerivedParent {
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

    #[test]
    fn identity_kinematic_transform_never_equates_distinct_frames() {
        let state = FrameKinematics::new(
            Position::from_metres(1.0, 2.0, 3.0),
            VelocityVector::from_metres_per_second(4.0, 5.0, 6.0),
            ReferenceFrame::ITRF2020,
        )
        .expect("finite state");
        let transform = IdentityKinematicFrameTransform;

        assert_eq!(
            transform.transform(
                Epoch::from_tai_seconds(0.0),
                state,
                ReferenceFrame::ITRF2020
            ),
            Ok(state)
        );
        assert_eq!(
            transform.transform(Epoch::from_tai_seconds(0.0), state, ReferenceFrame::GCRF),
            Err(IdentityKinematicFrameTransformError::FrameMismatch {
                from: ReferenceFrame::ITRF2020,
                target: ReferenceFrame::GCRF,
            })
        );
    }

    #[test]
    fn kinematic_transform_composition_and_inverse_preserve_state() {
        let transform: ReferenceDataKinematicFrameTransform<_> = CanonicalOffsetSupplier {
            reference_data: vec![descriptor()],
        }
        .into();
        let epoch = Epoch::from_tai_seconds(42.0);

        for (position, velocity) in [
            ([1.0, 2.0, 3.0], [4.0, 5.0, 6.0]),
            ([-7_000_000.0, 125.5, 9.25], [-1.0, 7_500.0, 0.5]),
            ([0.0, -0.5, 1_024.0], [0.25, -0.125, 64.0]),
        ] {
            let initial = FrameKinematics::new(
                Position::from_metres(position[0], position[1], position[2]),
                VelocityVector::from_metres_per_second(velocity[0], velocity[1], velocity[2]),
                ReferenceFrame::ITRF2020,
            )
            .expect("finite deterministic state");
            let intermediate = transform
                .transform(epoch, initial, ReferenceFrame::GCRF)
                .expect("first transform");
            let composed = transform
                .transform(epoch, intermediate, ReferenceFrame::EME2000)
                .expect("composed transform");
            let direct = transform
                .transform(epoch, initial, ReferenceFrame::EME2000)
                .expect("direct transform");
            let recovered = transform
                .transform(epoch, composed, ReferenceFrame::ITRF2020)
                .expect("inverse transform");

            assert_eq!(composed, direct);
            assert_eq!(recovered, initial);
        }
    }

    #[test]
    fn reference_data_adapter_delegates_distinct_frames_and_preserves_provenance() {
        let supplier = OffsetSupplier {
            reference_data: vec![
                descriptor(),
                ReferenceDataDescriptor::new(
                    "test convention authority",
                    "test convention",
                    "test convention revision",
                    Sha256Digest::compute(b"test convention"),
                    ArtifactCoverage::AllTime,
                )
                .expect("valid convention descriptor"),
            ],
        };
        let transform: ReferenceDataKinematicFrameTransform<_> = supplier.into();
        assert_eq!(
            transform.as_ref().reference_data()[0].product(),
            "test reference data"
        );
        assert_eq!(transform.as_ref().reference_data().len(), 2);
        let state = FrameKinematics::new(
            Position::from_metres(1.0, 2.0, 3.0),
            VelocityVector::from_metres_per_second(4.0, 5.0, 6.0),
            ReferenceFrame::ITRF2020,
        )
        .expect("finite state");

        let transformed = transform
            .transform(Epoch::from_tai_seconds(42.0), state, ReferenceFrame::GCRF)
            .expect("supplier resolves distinct frames");

        assert_eq!(transformed.frame(), ReferenceFrame::GCRF);
        assert_eq!(
            transformed.position(),
            Position::from_metres(101.0, 2.0, 3.0)
        );
        assert_eq!(transformed.velocity(), state.velocity());
    }

    #[test]
    fn reference_data_adapter_preserves_identity_without_loading_data() {
        let transform: ReferenceDataKinematicFrameTransform<_> = RejectingSupplier {
            reference_data: vec![descriptor()],
        }
        .into();
        let state = FrameKinematics::new(
            Position::from_metres(1.0, 2.0, 3.0),
            VelocityVector::from_metres_per_second(4.0, 5.0, 6.0),
            ReferenceFrame::GCRF,
        )
        .expect("finite state");

        assert!(matches!(
            transform.transform(Epoch::from_tai_seconds(42.0), state, ReferenceFrame::GCRF),
            Ok(returned) if returned == state
        ));
    }

    #[test]
    fn reference_data_adapter_rejects_supplier_output_in_the_wrong_frame() {
        let transform: ReferenceDataKinematicFrameTransform<_> = MislabelledSupplier {
            reference_data: vec![descriptor()],
        }
        .into();
        let state = FrameKinematics::new(
            Position::from_metres(1.0, 2.0, 3.0),
            VelocityVector::from_metres_per_second(4.0, 5.0, 6.0),
            ReferenceFrame::ITRF2020,
        )
        .expect("finite state");

        assert!(matches!(
            transform.transform(Epoch::from_tai_seconds(42.0), state, ReferenceFrame::GCRF),
            Err(
                ReferenceDataKinematicFrameTransformError::OutputFrameMismatch {
                    expected: ReferenceFrame::GCRF,
                    actual: ReferenceFrame::ITRF2020,
                }
            )
        ));
    }

    #[test]
    fn reference_data_adapter_preserves_supplier_errors() {
        let transform: ReferenceDataKinematicFrameTransform<_> = RejectingSupplier {
            reference_data: vec![descriptor()],
        }
        .into();
        let state = FrameKinematics::new(
            Position::from_metres(1.0, 2.0, 3.0),
            VelocityVector::from_metres_per_second(4.0, 5.0, 6.0),
            ReferenceFrame::ITRF2020,
        )
        .expect("finite state");

        assert!(matches!(
            transform.transform(Epoch::from_tai_seconds(42.0), state, ReferenceFrame::GCRF),
            Err(ReferenceDataKinematicFrameTransformError::Supplier { .. })
        ));
    }

    #[test]
    fn reference_data_adapter_rejects_distinct_frames_without_declared_data() {
        let transform: ReferenceDataKinematicFrameTransform<_> = EmptySupplier.into();
        let state = FrameKinematics::new(
            Position::from_metres(1.0, 2.0, 3.0),
            VelocityVector::from_metres_per_second(4.0, 5.0, 6.0),
            ReferenceFrame::ITRF2020,
        )
        .expect("finite state");

        assert!(matches!(
            transform.transform(Epoch::from_tai_seconds(42.0), state, ReferenceFrame::GCRF),
            Err(ReferenceDataKinematicFrameTransformError::MissingReferenceData)
        ));
    }

    #[test]
    fn epsg_topocentric_vector_matches_and_round_trips() {
        let origin = GeodeticPosition::new(
            Angle::new::<degree>(5.0),
            Angle::new::<degree>(55.0),
            Length::new::<meter>(200.0),
        )
        .expect("EPSG origin");
        let mut catalog = FrameCatalog::new(FrameNamespace::new(50), [ReferenceFrame::ITRF2020])
            .expect("topocentric catalog");
        let frame = catalog
            .define_topocentric_enu(
                1,
                ReferenceFrame::ITRF2020,
                ReferenceEllipsoid::wgs84(),
                origin,
            )
            .expect("valid ENU definition");
        let parent_position = Position::from_metres(3_771_793.968, 140_253.342, 5_124_304.349);
        let local = frame.to_topocentric_position(parent_position);
        let [east, north, up] = local.to_metres();

        assert!((east - -189_013.869).abs() <= 0.002);
        assert!((north - -128_642.040).abs() <= 0.002);
        assert!((up - -4_220.171).abs() <= 0.002);
        let round_trip = frame.to_parent_position(local).to_metres();
        for (actual, expected) in round_trip.into_iter().zip(parent_position.to_metres()) {
            assert!((actual - expected).abs() < 1.0e-9);
        }

        let parent_kinematics = FrameKinematics::new(
            parent_position,
            VelocityVector::from_metres_per_second(10.0, 20.0, 30.0),
            frame.parent(),
        )
        .expect("finite parent kinematics");
        let local_kinematics = frame
            .transform(
                Epoch::from_tai_seconds(42.0),
                parent_kinematics,
                frame.reference_frame(),
            )
            .expect("parent-to-topocentric transform");
        let restored = frame
            .transform(
                Epoch::from_tai_seconds(84.0),
                local_kinematics,
                frame.parent(),
            )
            .expect("topocentric-to-parent transform");
        assert_eq!(restored.frame(), frame.parent());
        for (actual, expected) in restored
            .position()
            .to_metres()
            .into_iter()
            .zip(parent_kinematics.position().to_metres())
        {
            assert!((actual - expected).abs() < 1.0e-9);
        }
        for (actual, expected) in restored
            .velocity()
            .to_metres_per_second()
            .into_iter()
            .zip(parent_kinematics.velocity().to_metres_per_second())
        {
            assert!((actual - expected).abs() < 1.0e-12);
        }
    }

    #[test]
    fn topocentric_definition_requires_matching_body_fixed_parent() {
        let origin = GeodeticPosition::new(
            Angle::new::<degree>(0.0),
            Angle::new::<degree>(0.0),
            Length::new::<meter>(0.0),
        )
        .expect("equatorial origin");
        let mars_ellipsoid = ReferenceEllipsoid::new(
            Body::MARS,
            Length::new::<meter>(3_396_190.0),
            Ratio::new::<ratio>(169.894_447_223_612),
        )
        .expect("valid test ellipsoid");
        let mars_fixed = ReferenceFrame::new(
            FrameOrigin::Body(Body::MARS),
            FrameOrientation::body_fixed(CustomFrameId::new(51)),
        );
        let mut catalog = FrameCatalog::new(
            FrameNamespace::new(51),
            [
                ReferenceFrame::ITRF2020,
                ReferenceFrame::GCRF,
                ReferenceFrame::TEME,
                mars_fixed,
            ],
        )
        .expect("catalog roots");

        assert_eq!(
            catalog.define_topocentric_enu(1, ReferenceFrame::ITRF2020, mars_ellipsoid, origin,),
            Err(FrameDefinitionError::EllipsoidBodyMismatch {
                parent: ReferenceFrame::ITRF2020,
                ellipsoid_body: Body::MARS,
            })
        );
        assert_eq!(
            catalog.define_topocentric_enu(
                2,
                ReferenceFrame::GCRF,
                ReferenceEllipsoid::wgs84(),
                origin,
            ),
            Err(FrameDefinitionError::TopocentricParentMustBeBodyFixed {
                parent: ReferenceFrame::GCRF,
            })
        );
        assert_eq!(
            catalog.define_topocentric_enu(
                3,
                ReferenceFrame::TEME,
                ReferenceEllipsoid::wgs84(),
                origin,
            ),
            Err(FrameDefinitionError::TopocentricParentMustBeBodyFixed {
                parent: ReferenceFrame::TEME,
            })
        );

        let mars = catalog
            .define_topocentric_enu(4, mars_fixed, mars_ellipsoid, origin)
            .expect("explicit Mars body-fixed parent");
        assert_eq!(mars.parent(), mars_fixed);
        assert_eq!(
            mars.reference_frame().orientation().motion(),
            FrameMotion::NonInertial
        );
    }
}
