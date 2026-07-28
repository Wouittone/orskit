#![forbid(unsafe_code)]

//! Explicit, format-neutral interchange snapshots for orskit domain values.
//!
//! This crate does not implement `Serialize` directly on domain objects.
//! Domain values contain application-owned identities and gravity-provider
//! trait objects, so callers first register stable identifiers in an
//! [`ExportContext`]. Registration prevents human-facing `Display` output or
//! process-local identities from silently becoming wire contracts. The
//! resulting owned snapshots contain raw values only at explicitly unit-named
//! serialization fields. Import resolves only caller-approved identities and
//! reconstructs values through their validated domain constructors.
//!
//! Enable `orbits` for built-in state snapshots, `two-bodies` for analytical
//! propagator snapshots, and `json` for the [`json`] encoder. The generic
//! versioned orbit envelope and downstream extension contract are always
//! available from this crate. No snapshot capability is enabled by default in
//! the public `orskit` facade.
//!
//! # Example
//!
//! ```
//! # #[cfg(feature = "orbits")]
//! # {
//! use frames::ReferenceFrame;
//! use hifitime::Epoch;
//! use orbits::cartesian::CartesianState;
//! use orskit_core::Orbit;
//! use orskit_export::{ExportContext, ImportContext, OrbitSnapshot};
//! use units::{Position, VelocityVector};
//!
//! let mut context = ExportContext::new();
//! context.register_reference_frame("gcrf", ReferenceFrame::GCRF)?;
//! let orbit = Orbit::new(
//!     Epoch::from_tai_seconds(42.0),
//!     CartesianState::new(
//!         ReferenceFrame::GCRF,
//!         Position::from_metres(7_000_000.0, 0.0, 0.0),
//!         VelocityVector::from_metres_per_second(0.0, 7_500.0, 0.0),
//!     )?,
//! );
//! let snapshot = OrbitSnapshot::try_from((&orbit, &context))?;
//!
//! assert_eq!(snapshot.schema, "orskit.orbit");
//! assert_eq!(snapshot.state.position_m, [7_000_000.0, 0.0, 0.0]);
//!
//! let mut imports = ImportContext::new();
//! imports.register_reference_frame("gcrf", ReferenceFrame::GCRF)?;
//! let restored: Orbit<CartesianState> = snapshot.try_into_orbit(&imports)?;
//! assert_eq!(restored, orbit);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! # }
//! # #[cfg(not(feature = "orbits"))]
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::{error::Error as StdError, sync::Arc};

#[cfg(feature = "two-bodies")]
use dynamics_two_bodies::{EllipticKeplerPropagator, PointMassGravityModel, TwoBodyDynamics};
#[cfg(feature = "orbits")]
use frames::InertialFrame;
use frames::{FrameOrigin, ReferenceFrame};
use gravity::SharedCentralGravity;
use hifitime::{Epoch, HifitimeError};
#[cfg(feature = "orbits")]
use orbits::{
    cartesian::CartesianState, circular::CircularState, equinoctial::EquinoctialState,
    keplerian::KeplerianState,
};
use orskit_core::Orbit;
use orskit_core::SpacecraftState;
use serde::{Deserialize, Serialize};
use thiserror::Error;
#[cfg(feature = "orbits")]
use units::uom::si::{angle::radian, length::meter, ratio::ratio};
#[cfg(feature = "orbits")]
use units::{Angle, Length, Position, Ratio, VelocityVector};

const ORBIT_SCHEMA: &str = "orskit.orbit";
#[cfg(feature = "two-bodies")]
const ELLIPTIC_KEPLER_PROPAGATOR_SCHEMA: &str = "orskit.propagator.elliptic-kepler";
const SCHEMA_VERSION: u32 = 1;

/// Caller-owned mapping from opaque scientific providers to stable export IDs.
///
/// Frames and origins compare by their exact domain identity. Shared providers
/// compare by allocation identity, mirroring element-state conversion rules
/// and preventing a numerically equal but independently selected provider from
/// being mislabeled.
#[derive(Debug, Default)]
pub struct ExportContext {
    reference_frames: Vec<(String, ReferenceFrame)>,
    frame_origins: Vec<(String, FrameOrigin)>,
    central_gravities: Vec<(String, SharedCentralGravity)>,
}

impl ExportContext {
    /// Creates an empty export context.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            reference_frames: Vec::new(),
            frame_origins: Vec::new(),
            central_gravities: Vec::new(),
        }
    }

    /// Registers a stable, non-blank ID for one exact reference-frame identity.
    ///
    /// IDs and frame identities must both be unique within a context.
    pub fn register_reference_frame(
        &mut self,
        id: impl Into<String>,
        frame: ReferenceFrame,
    ) -> Result<(), ExportError> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(ExportError::BlankReferenceFrameId);
        }
        if self
            .reference_frames
            .iter()
            .any(|(registered_id, _)| registered_id == &id)
        {
            return Err(ExportError::DuplicateReferenceFrameId { id });
        }
        if self
            .reference_frames
            .iter()
            .any(|(_, registered_frame)| *registered_frame == frame)
        {
            return Err(ExportError::DuplicateReferenceFrame);
        }
        self.reference_frames.push((id, frame));
        Ok(())
    }

    /// Registers a stable, non-blank ID for one exact frame-origin identity.
    ///
    /// IDs and origins must both be unique within a context.
    pub fn register_frame_origin(
        &mut self,
        id: impl Into<String>,
        origin: FrameOrigin,
    ) -> Result<(), ExportError> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(ExportError::BlankFrameOriginId);
        }
        if self
            .frame_origins
            .iter()
            .any(|(registered_id, _)| registered_id == &id)
        {
            return Err(ExportError::DuplicateFrameOriginId { id });
        }
        if self
            .frame_origins
            .iter()
            .any(|(_, registered_origin)| *registered_origin == origin)
        {
            return Err(ExportError::DuplicateFrameOrigin);
        }
        self.frame_origins.push((id, origin));
        Ok(())
    }

    /// Registers a stable, non-blank provider ID for shared gravity.
    ///
    /// The context retains one clone of the provider's [`Arc`]. IDs and
    /// provider allocations must both be unique within a context.
    pub fn register_central_gravity(
        &mut self,
        provider_id: impl Into<String>,
        provider: SharedCentralGravity,
    ) -> Result<(), ExportError> {
        let provider_id = provider_id.into();
        if provider_id.trim().is_empty() {
            return Err(ExportError::BlankCentralGravityId);
        }
        if self
            .central_gravities
            .iter()
            .any(|(registered_id, _)| registered_id == &provider_id)
        {
            return Err(ExportError::DuplicateCentralGravityId { id: provider_id });
        }
        if self
            .central_gravities
            .iter()
            .any(|(_, registered_provider)| Arc::ptr_eq(registered_provider, &provider))
        {
            return Err(ExportError::DuplicateCentralGravityProvider);
        }
        self.central_gravities.push((provider_id, provider));
        Ok(())
    }

    /// Returns the registered stable ID for an exact reference frame.
    pub fn reference_frame_id(&self, frame: ReferenceFrame) -> Result<&str, ExportError> {
        self.reference_frames
            .iter()
            .find_map(|(id, registered_frame)| (*registered_frame == frame).then_some(id.as_str()))
            .ok_or(ExportError::UnregisteredReferenceFrame)
    }

    /// Returns the registered stable ID for an exact frame origin.
    pub fn frame_origin_id(&self, origin: FrameOrigin) -> Result<&str, ExportError> {
        self.frame_origins
            .iter()
            .find_map(|(id, registered_origin)| {
                (*registered_origin == origin).then_some(id.as_str())
            })
            .ok_or(ExportError::UnregisteredFrameOrigin)
    }

    /// Creates a snapshot for one exactly registered gravity provider.
    pub fn central_gravity_snapshot(
        &self,
        provider: &SharedCentralGravity,
    ) -> Result<CentralGravitySnapshot, ExportError> {
        let (provider_id, _) = self
            .central_gravities
            .iter()
            .find(|(_, registered_provider)| Arc::ptr_eq(registered_provider, provider))
            .ok_or(ExportError::UnregisteredCentralGravityProvider)?;
        Ok(CentralGravitySnapshot {
            provider_id: provider_id.clone(),
            origin_id: self.frame_origin_id(provider.origin())?.to_owned(),
            gravitational_parameter_m3_s2: provider
                .parameter()
                .as_cubic_metres_per_second_squared(),
        })
    }
}

/// Failure while associating opaque domain providers with stable export IDs.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum ExportError {
    /// A reference-frame export ID contained no non-whitespace characters.
    #[error("reference-frame export ID must not be blank")]
    BlankReferenceFrameId,
    /// A reference-frame export ID was already registered.
    #[error("reference-frame export ID {id:?} is already registered")]
    DuplicateReferenceFrameId {
        /// Conflicting stable identifier.
        id: String,
    },
    /// The same exact reference frame was registered more than once.
    #[error("reference frame is already registered")]
    DuplicateReferenceFrame,
    /// An exported value referred to a frame absent from the context.
    #[error("reference frame has no export registration")]
    UnregisteredReferenceFrame,
    /// A frame-origin export ID contained no non-whitespace characters.
    #[error("frame-origin export ID must not be blank")]
    BlankFrameOriginId,
    /// A frame-origin export ID was already registered.
    #[error("frame-origin export ID {id:?} is already registered")]
    DuplicateFrameOriginId {
        /// Conflicting stable identifier.
        id: String,
    },
    /// The same exact frame origin was registered more than once.
    #[error("frame origin is already registered")]
    DuplicateFrameOrigin,
    /// An exported value referred to an origin absent from the context.
    #[error("frame origin has no export registration")]
    UnregisteredFrameOrigin,
    /// A gravity-provider export ID contained no non-whitespace characters.
    #[error("central-gravity export ID must not be blank")]
    BlankCentralGravityId,
    /// A gravity-provider export ID was already registered.
    #[error("central-gravity export ID {id:?} is already registered")]
    DuplicateCentralGravityId {
        /// Conflicting stable identifier.
        id: String,
    },
    /// The same shared provider allocation was registered more than once.
    #[error("central-gravity provider is already registered")]
    DuplicateCentralGravityProvider,
    /// An exported value referred to a provider absent from the context.
    #[error("central-gravity provider has no export registration")]
    UnregisteredCentralGravityProvider,
    /// Maximum solver iteration count could not fit the fixed-width schema.
    #[error("maximum iteration count does not fit the export schema")]
    MaximumIterationsOutOfRange,
}

/// Caller-owned stable-ID resolver used when importing snapshots.
///
/// Import never constructs a frame or scientific provider from wire data.
/// Applications register the exact live identities they accept, and imported
/// gravity metadata must match the registered provider's origin and parameter.
#[derive(Debug, Default)]
pub struct ImportContext {
    registrations: ExportContext,
}

impl ImportContext {
    /// Creates an empty import context.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            registrations: ExportContext::new(),
        }
    }

    /// Registers one accepted stable frame ID.
    pub fn register_reference_frame(
        &mut self,
        id: impl Into<String>,
        frame: ReferenceFrame,
    ) -> Result<(), ExportError> {
        self.registrations.register_reference_frame(id, frame)
    }

    /// Registers one accepted stable origin ID.
    pub fn register_frame_origin(
        &mut self,
        id: impl Into<String>,
        origin: FrameOrigin,
    ) -> Result<(), ExportError> {
        self.registrations.register_frame_origin(id, origin)
    }

    /// Registers one accepted stable central-gravity provider ID.
    pub fn register_central_gravity(
        &mut self,
        id: impl Into<String>,
        provider: SharedCentralGravity,
    ) -> Result<(), ExportError> {
        self.registrations.register_central_gravity(id, provider)
    }

    /// Resolves an accepted frame ID to its exact domain identity.
    pub fn reference_frame(&self, id: &str) -> Result<ReferenceFrame, ImportError> {
        self.registrations
            .reference_frames
            .iter()
            .find_map(|(registered_id, frame)| (registered_id == id).then_some(*frame))
            .ok_or_else(|| ImportError::UnknownReferenceFrameId { id: id.to_owned() })
    }

    /// Resolves an accepted origin ID to its exact domain identity.
    pub fn frame_origin(&self, id: &str) -> Result<FrameOrigin, ImportError> {
        self.registrations
            .frame_origins
            .iter()
            .find_map(|(registered_id, origin)| (registered_id == id).then_some(*origin))
            .ok_or_else(|| ImportError::UnknownFrameOriginId { id: id.to_owned() })
    }

    /// Resolves and verifies the live provider described by a gravity snapshot.
    pub fn central_gravity(
        &self,
        snapshot: &CentralGravitySnapshot,
    ) -> Result<SharedCentralGravity, ImportError> {
        let provider = self
            .registrations
            .central_gravities
            .iter()
            .find_map(|(registered_id, provider)| {
                (registered_id == &snapshot.provider_id).then_some(provider)
            })
            .ok_or_else(|| ImportError::UnknownCentralGravityId {
                id: snapshot.provider_id.clone(),
            })?;
        let declared_origin = self.frame_origin(&snapshot.origin_id)?;
        let actual_origin = provider.origin();
        if actual_origin != declared_origin {
            return Err(ImportError::CentralGravityOriginMismatch {
                provider_id: snapshot.provider_id.clone(),
            });
        }
        let actual_parameter = provider.parameter().as_cubic_metres_per_second_squared();
        if actual_parameter.to_bits() != snapshot.gravitational_parameter_m3_s2.to_bits() {
            return Err(ImportError::CentralGravityParameterMismatch {
                provider_id: snapshot.provider_id.clone(),
            });
        }
        Ok(provider.clone())
    }
}

/// Failure while resolving or reconstructing a versioned snapshot.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ImportError {
    /// The snapshot names another schema family.
    #[error("expected snapshot schema {expected:?}, found {actual:?}")]
    SchemaMismatch {
        /// Expected schema identifier.
        expected: &'static str,
        /// Identifier found in the snapshot.
        actual: String,
    },
    /// The schema version is unsupported.
    #[error("unsupported {schema:?} schema version {actual}; expected {expected}")]
    UnsupportedSchemaVersion {
        /// Schema identifier.
        schema: &'static str,
        /// Supported version.
        expected: u32,
        /// Version found in the snapshot.
        actual: u32,
    },
    /// An orbit epoch could not be parsed by Hifitime.
    #[error("invalid snapshot epoch")]
    InvalidEpoch {
        /// Hifitime parse failure.
        #[source]
        source: HifitimeError,
    },
    /// A stable frame ID has no caller registration.
    #[error("unknown reference-frame import ID {id:?}")]
    UnknownReferenceFrameId {
        /// Missing stable ID.
        id: String,
    },
    /// A stable origin ID has no caller registration.
    #[error("unknown frame-origin import ID {id:?}")]
    UnknownFrameOriginId {
        /// Missing stable ID.
        id: String,
    },
    /// A stable provider ID has no caller registration.
    #[error("unknown central-gravity import ID {id:?}")]
    UnknownCentralGravityId {
        /// Missing stable ID.
        id: String,
    },
    /// Registered provider origin differs from the declared origin ID.
    #[error("central-gravity provider {provider_id:?} does not match its declared origin")]
    CentralGravityOriginMismatch {
        /// Stable provider ID.
        provider_id: String,
    },
    /// Registered provider parameter differs from the serialized value.
    #[error("central-gravity provider {provider_id:?} does not match its declared parameter")]
    CentralGravityParameterMismatch {
        /// Stable provider ID.
        provider_id: String,
    },
    /// A representation or implementation discriminator is unsupported.
    #[error("expected {expected:?}, found {actual:?}")]
    DiscriminatorMismatch {
        /// Expected discriminator.
        expected: &'static str,
        /// Snapshot discriminator.
        actual: String,
    },
    /// A resolved frame does not have affirmatively inertial axes.
    #[error("snapshot state requires an inertial frame")]
    NonInertialFrame {
        /// Frame capability failure.
        #[source]
        source: frames::InertialFrameError,
    },
    /// A state or propagator constructor rejected imported values.
    #[error("snapshot values violate the live domain contract")]
    Domain {
        /// Original typed domain failure.
        #[source]
        source: Box<dyn StdError + Send + Sync>,
    },
    /// A fixed-width wire integer cannot fit this platform's domain type.
    #[error("snapshot integer does not fit the live domain type")]
    IntegerOutOfRange,
}

impl ImportError {
    #[cfg(feature = "orbits")]
    fn domain(source: impl StdError + Send + Sync + 'static) -> Self {
        Self::Domain {
            source: Box::new(source),
        }
    }
}

/// Serializable representation supplied by one concrete spacecraft state.
///
/// Applications may implement this trait for their own [`SpacecraftState`]
/// types and choose an owned Serde-compatible snapshot type. Raw physical
/// scalars in that type should use unit-qualified field names.
pub trait ExportableState: SpacecraftState {
    /// Owned, serializable representation of this state.
    type Snapshot: Serialize;

    /// Creates a state snapshot using explicit provider registrations.
    fn export_snapshot(&self, context: &ExportContext) -> Result<Self::Snapshot, ExportError>;
}

/// Reconstruction supplied by one concrete spacecraft-state implementation.
///
/// Implementations must validate snapshot values through their normal domain
/// constructors rather than assigning fields directly.
pub trait ImportableState: SpacecraftState + Sized {
    /// Owned snapshot accepted by this implementation.
    type Snapshot;

    /// Reconstructs a validated state using caller-approved identities.
    fn import_snapshot(
        snapshot: Self::Snapshot,
        context: &ImportContext,
    ) -> Result<Self, ImportError>;
}

/// Versioned, epoch-qualified envelope for one caller-selected state snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrbitSnapshot<S> {
    /// Schema family identifier.
    pub schema: String,
    /// Version of the schema family.
    pub schema_version: u32,
    /// Hifitime Gregorian epoch text including its time scale.
    pub epoch: String,
    /// Representation-specific state snapshot.
    pub state: S,
}

impl<'orbit, 'context, State> TryFrom<(&'orbit Orbit<State>, &'context ExportContext)>
    for OrbitSnapshot<State::Snapshot>
where
    State: ExportableState,
{
    type Error = ExportError;

    fn try_from(
        (orbit, context): (&'orbit Orbit<State>, &'context ExportContext),
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: ORBIT_SCHEMA.to_owned(),
            schema_version: SCHEMA_VERSION,
            epoch: orbit.epoch().to_string(),
            state: orbit.as_ref().export_snapshot(context)?,
        })
    }
}

impl<S> OrbitSnapshot<S> {
    /// Imports an orbit after validating schema, epoch, and state values.
    pub fn try_into_orbit<State>(self, context: &ImportContext) -> Result<Orbit<State>, ImportError>
    where
        State: ImportableState<Snapshot = S>,
    {
        validate_schema(&self.schema, self.schema_version, ORBIT_SCHEMA)?;
        let epoch = self
            .epoch
            .parse::<Epoch>()
            .map_err(|source| ImportError::InvalidEpoch { source })?;
        Ok(Orbit::new(
            epoch,
            State::import_snapshot(self.state, context)?,
        ))
    }
}

fn validate_schema(schema: &str, version: u32, expected: &'static str) -> Result<(), ImportError> {
    if schema != expected {
        return Err(ImportError::SchemaMismatch {
            expected,
            actual: schema.to_owned(),
        });
    }
    if version != SCHEMA_VERSION {
        return Err(ImportError::UnsupportedSchemaVersion {
            schema: expected,
            expected: SCHEMA_VERSION,
            actual: version,
        });
    }
    Ok(())
}

/// Gravity context embedded in element-state and propagator snapshots.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CentralGravitySnapshot {
    /// Stable caller-assigned provider identity.
    pub provider_id: String,
    /// Stable caller-assigned identity for the provider's frame origin.
    pub origin_id: String,
    /// Provider gravitational parameter in cubic metres per square second.
    pub gravitational_parameter_m3_s2: f64,
}

/// Exported Cartesian state in SI units.
#[cfg(feature = "orbits")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CartesianStateSnapshot {
    /// State representation discriminator.
    pub representation: String,
    /// Reference-frame identity.
    pub frame: String,
    /// Position components `(x, y, z)` in metres.
    pub position_m: [f64; 3],
    /// Velocity components `(vx, vy, vz)` in metres per second.
    pub velocity_m_s: [f64; 3],
}

#[cfg(feature = "orbits")]
impl ExportableState for CartesianState {
    type Snapshot = CartesianStateSnapshot;

    fn export_snapshot(&self, context: &ExportContext) -> Result<Self::Snapshot, ExportError> {
        Ok(CartesianStateSnapshot {
            representation: "cartesian".to_owned(),
            frame: context.reference_frame_id(self.frame())?.to_owned(),
            position_m: self.position().to_metres(),
            velocity_m_s: self.velocity().to_metres_per_second(),
        })
    }
}

#[cfg(feature = "orbits")]
impl ImportableState for CartesianState {
    type Snapshot = CartesianStateSnapshot;

    fn import_snapshot(
        snapshot: Self::Snapshot,
        context: &ImportContext,
    ) -> Result<Self, ImportError> {
        validate_discriminator(&snapshot.representation, "cartesian")?;
        CartesianState::new(
            context.reference_frame(&snapshot.frame)?,
            Position::from_metres(
                snapshot.position_m[0],
                snapshot.position_m[1],
                snapshot.position_m[2],
            ),
            VelocityVector::from_metres_per_second(
                snapshot.velocity_m_s[0],
                snapshot.velocity_m_s[1],
                snapshot.velocity_m_s[2],
            ),
        )
        .map_err(ImportError::domain)
    }
}

/// Exported circular elements in SI/radian units.
#[cfg(feature = "orbits")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CircularStateSnapshot {
    /// State representation discriminator.
    pub representation: String,
    /// Inertial reference-frame identity.
    pub frame: String,
    /// Explicit gravity context used to interpret the elements.
    pub central_gravity: CentralGravitySnapshot,
    /// Semi-major axis in metres.
    pub semi_major_axis_m: f64,
    /// `e cos(omega)`.
    pub eccentricity_x: f64,
    /// `e sin(omega)`.
    pub eccentricity_y: f64,
    /// Inclination in radians.
    pub inclination_rad: f64,
    /// Right ascension of the ascending node in radians.
    pub right_ascension_of_ascending_node_rad: f64,
    /// True latitude argument in radians.
    pub true_latitude_argument_rad: f64,
}

#[cfg(feature = "orbits")]
impl ExportableState for CircularState {
    type Snapshot = CircularStateSnapshot;

    fn export_snapshot(&self, context: &ExportContext) -> Result<Self::Snapshot, ExportError> {
        Ok(CircularStateSnapshot {
            representation: "circular".to_owned(),
            frame: context.reference_frame_id(self.frame())?.to_owned(),
            central_gravity: context.central_gravity_snapshot(self.central_gravity())?,
            semi_major_axis_m: self.semi_major_axis().get::<meter>(),
            eccentricity_x: self.eccentricity_x().get::<ratio>(),
            eccentricity_y: self.eccentricity_y().get::<ratio>(),
            inclination_rad: self.inclination().get::<radian>(),
            right_ascension_of_ascending_node_rad: self
                .right_ascension_of_ascending_node()
                .get::<radian>(),
            true_latitude_argument_rad: self.true_latitude_argument().get::<radian>(),
        })
    }
}

#[cfg(feature = "orbits")]
impl ImportableState for CircularState {
    type Snapshot = CircularStateSnapshot;

    fn import_snapshot(
        snapshot: Self::Snapshot,
        context: &ImportContext,
    ) -> Result<Self, ImportError> {
        validate_discriminator(&snapshot.representation, "circular")?;
        let frame = inertial_frame(context.reference_frame(&snapshot.frame)?)?;
        CircularState::new(
            frame,
            context.central_gravity(&snapshot.central_gravity)?,
            Length::new::<meter>(snapshot.semi_major_axis_m),
            Ratio::new::<ratio>(snapshot.eccentricity_x),
            Ratio::new::<ratio>(snapshot.eccentricity_y),
            Angle::new::<radian>(snapshot.inclination_rad),
            Angle::new::<radian>(snapshot.right_ascension_of_ascending_node_rad),
            Angle::new::<radian>(snapshot.true_latitude_argument_rad),
        )
        .map_err(ImportError::domain)
    }
}

/// Exported classical Keplerian elements in SI/radian units.
#[cfg(feature = "orbits")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeplerianStateSnapshot {
    /// State representation discriminator.
    pub representation: String,
    /// Inertial reference-frame identity.
    pub frame: String,
    /// Explicit gravity context used to interpret the elements.
    pub central_gravity: CentralGravitySnapshot,
    /// Semi-major axis in metres.
    pub semi_major_axis_m: f64,
    /// Scalar eccentricity.
    pub eccentricity: f64,
    /// Inclination in radians.
    pub inclination_rad: f64,
    /// Right ascension of the ascending node in radians.
    pub right_ascension_of_ascending_node_rad: f64,
    /// Argument of periapsis in radians.
    pub argument_of_periapsis_rad: f64,
    /// True anomaly in radians.
    pub true_anomaly_rad: f64,
}

#[cfg(feature = "orbits")]
impl ExportableState for KeplerianState {
    type Snapshot = KeplerianStateSnapshot;

    fn export_snapshot(&self, context: &ExportContext) -> Result<Self::Snapshot, ExportError> {
        Ok(KeplerianStateSnapshot {
            representation: "keplerian".to_owned(),
            frame: context.reference_frame_id(self.frame())?.to_owned(),
            central_gravity: context.central_gravity_snapshot(self.central_gravity())?,
            semi_major_axis_m: self.semi_major_axis().get::<meter>(),
            eccentricity: self.eccentricity().get::<ratio>(),
            inclination_rad: self.inclination().get::<radian>(),
            right_ascension_of_ascending_node_rad: self
                .right_ascension_of_ascending_node()
                .get::<radian>(),
            argument_of_periapsis_rad: self.argument_of_periapsis().get::<radian>(),
            true_anomaly_rad: self.true_anomaly().get::<radian>(),
        })
    }
}

#[cfg(feature = "orbits")]
impl ImportableState for KeplerianState {
    type Snapshot = KeplerianStateSnapshot;

    fn import_snapshot(
        snapshot: Self::Snapshot,
        context: &ImportContext,
    ) -> Result<Self, ImportError> {
        validate_discriminator(&snapshot.representation, "keplerian")?;
        let frame = inertial_frame(context.reference_frame(&snapshot.frame)?)?;
        KeplerianState::new(
            frame,
            context.central_gravity(&snapshot.central_gravity)?,
            Length::new::<meter>(snapshot.semi_major_axis_m),
            Ratio::new::<ratio>(snapshot.eccentricity),
            Angle::new::<radian>(snapshot.inclination_rad),
            Angle::new::<radian>(snapshot.right_ascension_of_ascending_node_rad),
            Angle::new::<radian>(snapshot.argument_of_periapsis_rad),
            Angle::new::<radian>(snapshot.true_anomaly_rad),
        )
        .map_err(ImportError::domain)
    }
}

/// Exported equinoctial elements in SI/radian units.
#[cfg(feature = "orbits")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EquinoctialStateSnapshot {
    /// State representation discriminator.
    pub representation: String,
    /// Inertial reference-frame identity.
    pub frame: String,
    /// Explicit gravity context used to interpret the elements.
    pub central_gravity: CentralGravitySnapshot,
    /// Semi-major axis in metres.
    pub semi_major_axis_m: f64,
    /// `e cos(omega + Omega)`.
    pub eccentricity_x: f64,
    /// `e sin(omega + Omega)`.
    pub eccentricity_y: f64,
    /// `tan(i / 2) cos(Omega)`.
    pub inclination_x: f64,
    /// `tan(i / 2) sin(Omega)`.
    pub inclination_y: f64,
    /// True longitude in radians.
    pub true_longitude_rad: f64,
}

#[cfg(feature = "orbits")]
impl ExportableState for EquinoctialState {
    type Snapshot = EquinoctialStateSnapshot;

    fn export_snapshot(&self, context: &ExportContext) -> Result<Self::Snapshot, ExportError> {
        Ok(EquinoctialStateSnapshot {
            representation: "equinoctial".to_owned(),
            frame: context.reference_frame_id(self.frame())?.to_owned(),
            central_gravity: context.central_gravity_snapshot(self.central_gravity())?,
            semi_major_axis_m: self.semi_major_axis().get::<meter>(),
            eccentricity_x: self.eccentricity_x().get::<ratio>(),
            eccentricity_y: self.eccentricity_y().get::<ratio>(),
            inclination_x: self.inclination_x().get::<ratio>(),
            inclination_y: self.inclination_y().get::<ratio>(),
            true_longitude_rad: self.true_longitude().get::<radian>(),
        })
    }
}

#[cfg(feature = "orbits")]
impl ImportableState for EquinoctialState {
    type Snapshot = EquinoctialStateSnapshot;

    fn import_snapshot(
        snapshot: Self::Snapshot,
        context: &ImportContext,
    ) -> Result<Self, ImportError> {
        validate_discriminator(&snapshot.representation, "equinoctial")?;
        let frame = inertial_frame(context.reference_frame(&snapshot.frame)?)?;
        EquinoctialState::new(
            frame,
            context.central_gravity(&snapshot.central_gravity)?,
            Length::new::<meter>(snapshot.semi_major_axis_m),
            Ratio::new::<ratio>(snapshot.eccentricity_x),
            Ratio::new::<ratio>(snapshot.eccentricity_y),
            Ratio::new::<ratio>(snapshot.inclination_x),
            Ratio::new::<ratio>(snapshot.inclination_y),
            Angle::new::<radian>(snapshot.true_longitude_rad),
        )
        .map_err(ImportError::domain)
    }
}

#[cfg(feature = "orbits")]
fn inertial_frame(frame: ReferenceFrame) -> Result<InertialFrame, ImportError> {
    InertialFrame::try_from(frame).map_err(|source| ImportError::NonInertialFrame { source })
}

#[cfg(feature = "orbits")]
fn validate_discriminator(actual: &str, expected: &'static str) -> Result<(), ImportError> {
    if actual == expected {
        Ok(())
    } else {
        Err(ImportError::DiscriminatorMismatch {
            expected,
            actual: actual.to_owned(),
        })
    }
}

/// Versioned export of an analytical elliptic Kepler propagator.
#[cfg(feature = "two-bodies")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EllipticKeplerPropagatorSnapshot {
    /// Schema family identifier.
    pub schema: String,
    /// Version of the schema family.
    pub schema_version: u32,
    /// Propagator implementation discriminator.
    pub propagator: String,
    /// Physical-problem discriminator.
    pub dynamics: String,
    /// Explicit gravity context owned by the physical problem.
    pub central_gravity: CentralGravitySnapshot,
    /// Anomaly solver tolerance in radians.
    pub tolerance_rad: f64,
    /// Maximum anomaly solver iteration count.
    pub max_iterations: u64,
    /// Maximum estimated floating phase error in radians.
    pub phase_error_budget_rad: f64,
}

#[cfg(feature = "two-bodies")]
impl TryFrom<(&EllipticKeplerPropagator, &ExportContext)> for EllipticKeplerPropagatorSnapshot {
    type Error = ExportError;

    fn try_from(
        (propagator, context): (&EllipticKeplerPropagator, &ExportContext),
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: ELLIPTIC_KEPLER_PROPAGATOR_SCHEMA.to_owned(),
            schema_version: SCHEMA_VERSION,
            propagator: "elliptic_kepler".to_owned(),
            dynamics: "two_body".to_owned(),
            central_gravity: context
                .central_gravity_snapshot(propagator.dynamics().central_gravity())?,
            tolerance_rad: propagator.tolerance().get::<radian>(),
            max_iterations: u64::try_from(propagator.max_iterations())
                .map_err(|_| ExportError::MaximumIterationsOutOfRange)?,
            phase_error_budget_rad: propagator.phase_error_budget().get::<radian>(),
        })
    }
}

#[cfg(feature = "two-bodies")]
impl EllipticKeplerPropagatorSnapshot {
    /// Reconstructs a validated propagator using caller-approved gravity.
    pub fn try_into_propagator(
        self,
        context: &ImportContext,
    ) -> Result<EllipticKeplerPropagator, ImportError> {
        validate_schema(
            &self.schema,
            self.schema_version,
            ELLIPTIC_KEPLER_PROPAGATOR_SCHEMA,
        )?;
        validate_discriminator(&self.propagator, "elliptic_kepler")?;
        validate_discriminator(&self.dynamics, "two_body")?;
        let max_iterations =
            usize::try_from(self.max_iterations).map_err(|_| ImportError::IntegerOutOfRange)?;
        EllipticKeplerPropagator::new(TwoBodyDynamics::new(PointMassGravityModel::new(
            context.central_gravity(&self.central_gravity)?,
        )))
        .with_tolerance(Angle::new::<radian>(self.tolerance_rad))
        .map_err(ImportError::domain)?
        .with_max_iterations(max_iterations)
        .map_err(ImportError::domain)?
        .with_phase_error_budget(Angle::new::<radian>(self.phase_error_budget_rad))
        .map_err(ImportError::domain)
    }
}

/// JSON encoding and decoding for snapshots.
#[cfg(feature = "json")]
pub mod json {
    pub use serde_json::{from_slice, from_str, to_string, to_string_pretty};
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(feature = "json", feature = "two-bodies"))]
    use dynamics_two_bodies::{PointMassGravityModel, TwoBodyDynamics};
    #[cfg(feature = "orbits")]
    use frames::InertialFrame;
    use frames::{Body, FrameOrigin, ReferenceFrame};
    use gravity::{PointMass, SharedCentralGravity};
    use hifitime::Epoch;
    use units::GravitationalParameter;
    #[cfg(feature = "orbits")]
    use units::{Angle, Length, Position, Ratio, VelocityVector};

    fn earth_gravity() -> SharedCentralGravity {
        Arc::new(PointMass::new(
            FrameOrigin::Body(Body::EARTH),
            GravitationalParameter::try_from(3.986_004_418e14).expect("positive parameter"),
        ))
    }

    #[cfg(feature = "orbits")]
    fn registered_context(gravity: SharedCentralGravity) -> ExportContext {
        let mut context = ExportContext::new();
        context
            .register_reference_frame("gcrf", ReferenceFrame::GCRF)
            .expect("unique frame registration");
        context
            .register_frame_origin("earth", FrameOrigin::Body(Body::EARTH))
            .expect("unique origin registration");
        context
            .register_central_gravity("earth", gravity)
            .expect("unique gravity registration");
        context
    }

    #[derive(Debug, PartialEq)]
    struct ApplicationState(ReferenceFrame);

    impl SpacecraftState for ApplicationState {
        fn frame(&self) -> ReferenceFrame {
            self.0
        }
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct ApplicationSnapshot {
        frame_id: String,
    }

    impl ExportableState for ApplicationState {
        type Snapshot = ApplicationSnapshot;

        fn export_snapshot(&self, context: &ExportContext) -> Result<Self::Snapshot, ExportError> {
            Ok(ApplicationSnapshot {
                frame_id: context.reference_frame_id(self.frame())?.to_owned(),
            })
        }
    }

    impl ImportableState for ApplicationState {
        type Snapshot = ApplicationSnapshot;

        fn import_snapshot(
            snapshot: Self::Snapshot,
            context: &ImportContext,
        ) -> Result<Self, ImportError> {
            Ok(Self(context.reference_frame(&snapshot.frame_id)?))
        }
    }

    #[test]
    fn generic_orbit_envelope_supports_application_states_without_builtin_features() {
        let mut context = ExportContext::new();
        context
            .register_reference_frame("application-gcrf", ReferenceFrame::GCRF)
            .expect("unique registration");
        let orbit = Orbit::new(
            Epoch::from_tai_seconds(42.0),
            ApplicationState(ReferenceFrame::GCRF),
        );

        let snapshot =
            OrbitSnapshot::try_from((&orbit, &context)).expect("registered application state");

        assert_eq!(snapshot.schema, "orskit.orbit");
        assert_eq!(snapshot.state.frame_id, "application-gcrf");

        let mut import_context = ImportContext::new();
        import_context
            .register_reference_frame("application-gcrf", ReferenceFrame::GCRF)
            .expect("unique registration");
        let imported: Orbit<ApplicationState> = snapshot
            .try_into_orbit(&import_context)
            .expect("registered application state");
        assert_eq!(imported, orbit);
    }

    #[cfg(feature = "orbits")]
    #[test]
    fn cartesian_orbit_export_is_si_and_needs_no_gravity_registration() {
        let orbit = Orbit::new(
            Epoch::from_tai_seconds(42.0),
            CartesianState::new(
                ReferenceFrame::GCRF,
                Position::from_metres(1.0, 2.0, 3.0),
                VelocityVector::from_metres_per_second(4.0, 5.0, 6.0),
            )
            .expect("finite state"),
        );

        let mut context = ExportContext::new();
        context
            .register_reference_frame("gcrf", ReferenceFrame::GCRF)
            .expect("unique registration");
        let snapshot = OrbitSnapshot::try_from((&orbit, &context)).expect("Cartesian export");

        assert_eq!(snapshot.schema, "orskit.orbit");
        assert_eq!(snapshot.epoch, "1900-01-01T00:00:42 TAI");
        assert_eq!(snapshot.state.frame, "gcrf");
        assert_eq!(snapshot.state.position_m, [1.0, 2.0, 3.0]);
        assert_eq!(snapshot.state.velocity_m_s, [4.0, 5.0, 6.0]);
    }

    #[cfg(feature = "orbits")]
    #[test]
    fn element_export_requires_the_exact_registered_provider() {
        let gravity = earth_gravity();
        let state = KeplerianState::new(
            InertialFrame::GCRF,
            Arc::clone(&gravity),
            Length::new::<meter>(7_200_000.0),
            Ratio::new::<ratio>(0.1),
            Angle::new::<radian>(0.7),
            Angle::new::<radian>(1.1),
            Angle::new::<radian>(0.4),
            Angle::new::<radian>(2.0),
        )
        .expect("valid elements");
        let orbit = Orbit::new(Epoch::from_tai_seconds(1_000.0), state);

        let mut context = ExportContext::new();
        context
            .register_reference_frame("gcrf", ReferenceFrame::GCRF)
            .expect("unique registration");
        assert_eq!(
            OrbitSnapshot::try_from((&orbit, &context)),
            Err(ExportError::UnregisteredCentralGravityProvider)
        );
        context
            .register_frame_origin("earth", FrameOrigin::Body(Body::EARTH))
            .expect("unique registration");
        context
            .register_central_gravity("iers-2010-earth", gravity)
            .expect("unique registration");
        let snapshot = OrbitSnapshot::try_from((&orbit, &context)).expect("registered export");
        assert_eq!(
            snapshot.state.central_gravity.provider_id,
            "iers-2010-earth"
        );
        assert_eq!(snapshot.state.central_gravity.origin_id, "earth");
        assert_eq!(snapshot.state.semi_major_axis_m, 7_200_000.0);
        assert_eq!(snapshot.state.eccentricity, 0.1);
        assert_eq!(snapshot.state.inclination_rad, 0.7);
        assert_eq!(snapshot.state.right_ascension_of_ascending_node_rad, 1.1);
        assert_eq!(snapshot.state.argument_of_periapsis_rad, 0.4);
        assert_eq!(snapshot.state.true_anomaly_rad, 2.0);
        #[cfg(feature = "json")]
        assert_eq!(
            serde_json::to_value(&snapshot.state).expect("finite snapshot"),
            serde_json::json!({
                "representation": "keplerian",
                "frame": "gcrf",
                "central_gravity": {
                    "provider_id": "iers-2010-earth",
                    "origin_id": "earth",
                    "gravitational_parameter_m3_s2": 3.986_004_418e14
                },
                "semi_major_axis_m": 7_200_000.0,
                "eccentricity": 0.1,
                "inclination_rad": 0.7,
                "right_ascension_of_ascending_node_rad": 1.1,
                "argument_of_periapsis_rad": 0.4,
                "true_anomaly_rad": 2.0
            })
        );
    }

    #[cfg(feature = "orbits")]
    #[test]
    fn every_element_representation_exports_its_declared_coordinates() {
        let gravity = earth_gravity();
        let context = registered_context(Arc::clone(&gravity));

        let circular = CircularState::new(
            InertialFrame::GCRF,
            Arc::clone(&gravity),
            Length::new::<meter>(7_100_000.0),
            Ratio::new::<ratio>(0.01),
            Ratio::new::<ratio>(-0.02),
            Angle::new::<radian>(0.3),
            Angle::new::<radian>(0.4),
            Angle::new::<radian>(0.5),
        )
        .expect("valid circular state");
        let circular = circular
            .export_snapshot(&context)
            .expect("registered provider");
        assert_eq!(circular.representation, "circular");
        assert_eq!(circular.frame, "gcrf");
        assert_eq!(circular.central_gravity.provider_id, "earth");
        assert_eq!(circular.central_gravity.origin_id, "earth");
        assert_eq!(circular.semi_major_axis_m, 7_100_000.0);
        assert_eq!(circular.eccentricity_x, 0.01);
        assert_eq!(circular.eccentricity_y, -0.02);
        assert_eq!(circular.inclination_rad, 0.3);
        assert_eq!(circular.right_ascension_of_ascending_node_rad, 0.4);
        assert_eq!(circular.true_latitude_argument_rad, 0.5);
        #[cfg(feature = "json")]
        assert_eq!(
            serde_json::to_value(&circular).expect("finite snapshot"),
            serde_json::json!({
                "representation": "circular",
                "frame": "gcrf",
                "central_gravity": {
                    "provider_id": "earth",
                    "origin_id": "earth",
                    "gravitational_parameter_m3_s2": 3.986_004_418e14
                },
                "semi_major_axis_m": 7_100_000.0,
                "eccentricity_x": 0.01,
                "eccentricity_y": -0.02,
                "inclination_rad": 0.3,
                "right_ascension_of_ascending_node_rad": 0.4,
                "true_latitude_argument_rad": 0.5
            })
        );

        let equinoctial = EquinoctialState::new(
            InertialFrame::GCRF,
            gravity,
            Length::new::<meter>(7_200_000.0),
            Ratio::new::<ratio>(0.03),
            Ratio::new::<ratio>(0.04),
            Ratio::new::<ratio>(0.05),
            Ratio::new::<ratio>(-0.06),
            Angle::new::<radian>(0.7),
        )
        .expect("valid equinoctial state");
        let equinoctial = equinoctial
            .export_snapshot(&context)
            .expect("registered provider");
        assert_eq!(equinoctial.representation, "equinoctial");
        assert_eq!(equinoctial.frame, "gcrf");
        assert_eq!(equinoctial.central_gravity.provider_id, "earth");
        assert_eq!(equinoctial.central_gravity.origin_id, "earth");
        assert_eq!(equinoctial.semi_major_axis_m, 7_200_000.0);
        assert_eq!(equinoctial.eccentricity_x, 0.03);
        assert_eq!(equinoctial.eccentricity_y, 0.04);
        assert_eq!(equinoctial.inclination_x, 0.05);
        assert_eq!(equinoctial.inclination_y, -0.06);
        assert_eq!(equinoctial.true_longitude_rad, 0.7);
        #[cfg(feature = "json")]
        assert_eq!(
            serde_json::to_value(&equinoctial).expect("finite snapshot"),
            serde_json::json!({
                "representation": "equinoctial",
                "frame": "gcrf",
                "central_gravity": {
                    "provider_id": "earth",
                    "origin_id": "earth",
                    "gravitational_parameter_m3_s2": 3.986_004_418e14
                },
                "semi_major_axis_m": 7_200_000.0,
                "eccentricity_x": 0.03,
                "eccentricity_y": 0.04,
                "inclination_x": 0.05,
                "inclination_y": -0.06,
                "true_longitude_rad": 0.7
            })
        );
    }

    #[cfg(feature = "orbits")]
    #[test]
    fn every_builtin_state_imports_through_domain_constructors() {
        let gravity = earth_gravity();
        let export_context = registered_context(Arc::clone(&gravity));
        let mut import_context = ImportContext::new();
        import_context
            .register_reference_frame("gcrf", ReferenceFrame::GCRF)
            .expect("unique frame registration");
        import_context
            .register_frame_origin("earth", FrameOrigin::Body(Body::EARTH))
            .expect("unique origin registration");
        import_context
            .register_central_gravity("earth", Arc::clone(&gravity))
            .expect("unique gravity registration");

        let cartesian = CartesianState::new(
            ReferenceFrame::GCRF,
            Position::from_metres(1.0, 2.0, 3.0),
            VelocityVector::from_metres_per_second(4.0, 5.0, 6.0),
        )
        .expect("valid Cartesian state");
        let circular = CircularState::new(
            InertialFrame::GCRF,
            Arc::clone(&gravity),
            Length::new::<meter>(7_100_000.0),
            Ratio::new::<ratio>(0.01),
            Ratio::new::<ratio>(-0.02),
            Angle::new::<radian>(0.3),
            Angle::new::<radian>(0.4),
            Angle::new::<radian>(0.5),
        )
        .expect("valid circular state");
        let keplerian = KeplerianState::new(
            InertialFrame::GCRF,
            Arc::clone(&gravity),
            Length::new::<meter>(7_200_000.0),
            Ratio::new::<ratio>(0.1),
            Angle::new::<radian>(0.7),
            Angle::new::<radian>(1.1),
            Angle::new::<radian>(0.4),
            Angle::new::<radian>(2.0),
        )
        .expect("valid Keplerian state");
        let equinoctial = EquinoctialState::new(
            InertialFrame::GCRF,
            gravity,
            Length::new::<meter>(7_300_000.0),
            Ratio::new::<ratio>(0.03),
            Ratio::new::<ratio>(0.04),
            Ratio::new::<ratio>(0.05),
            Ratio::new::<ratio>(-0.06),
            Angle::new::<radian>(0.7),
        )
        .expect("valid equinoctial state");

        macro_rules! assert_round_trip {
            ($state:expr, $state_type:ty) => {{
                let orbit = Orbit::new(Epoch::from_tai_seconds(1_000.0), $state);
                let snapshot = OrbitSnapshot::try_from((&orbit, &export_context)).expect("export");
                let imported: Orbit<$state_type> = snapshot
                    .try_into_orbit(&import_context)
                    .expect("validated import");
                assert_eq!(imported, orbit);
            }};
        }

        assert_round_trip!(cartesian, CartesianState);
        assert_round_trip!(circular, CircularState);
        assert_round_trip!(keplerian, KeplerianState);
        assert_round_trip!(equinoctial, EquinoctialState);
    }

    #[cfg(feature = "orbits")]
    #[test]
    fn import_rejects_untrusted_schema_identity_and_values() {
        let mut context = ImportContext::new();
        context
            .register_reference_frame("gcrf", ReferenceFrame::GCRF)
            .expect("unique frame registration");
        let snapshot = OrbitSnapshot {
            schema: ORBIT_SCHEMA.to_owned(),
            schema_version: SCHEMA_VERSION,
            epoch: "1900-01-01T00:00:42 TAI".to_owned(),
            state: CartesianStateSnapshot {
                representation: "cartesian".to_owned(),
                frame: "gcrf".to_owned(),
                position_m: [1.0, 2.0, 3.0],
                velocity_m_s: [4.0, 5.0, 6.0],
            },
        };

        let mut wrong_schema = snapshot.clone();
        wrong_schema.schema = "other.orbit".to_owned();
        assert!(matches!(
            wrong_schema.try_into_orbit::<CartesianState>(&context),
            Err(ImportError::SchemaMismatch { .. })
        ));

        let mut wrong_version = snapshot.clone();
        wrong_version.schema_version += 1;
        assert!(matches!(
            wrong_version.try_into_orbit::<CartesianState>(&context),
            Err(ImportError::UnsupportedSchemaVersion { .. })
        ));

        let mut invalid_epoch = snapshot.clone();
        invalid_epoch.epoch = "not an epoch".to_owned();
        assert!(matches!(
            invalid_epoch.try_into_orbit::<CartesianState>(&context),
            Err(ImportError::InvalidEpoch { .. })
        ));

        let mut unknown_frame = snapshot.clone();
        unknown_frame.state.frame = "unknown".to_owned();
        assert!(matches!(
            unknown_frame.try_into_orbit::<CartesianState>(&context),
            Err(ImportError::UnknownReferenceFrameId { .. })
        ));

        let mut wrong_representation = snapshot.clone();
        wrong_representation.state.representation = "keplerian".to_owned();
        assert!(matches!(
            wrong_representation.try_into_orbit::<CartesianState>(&context),
            Err(ImportError::DiscriminatorMismatch { .. })
        ));

        let mut invalid_values = snapshot;
        invalid_values.state.position_m[0] = f64::NAN;
        assert!(matches!(
            invalid_values.try_into_orbit::<CartesianState>(&context),
            Err(ImportError::Domain { .. })
        ));
    }

    #[test]
    fn context_rejects_ambiguous_registrations() {
        let gravity = earth_gravity();
        let mut context = ExportContext::new();
        assert_eq!(
            context.register_reference_frame("  ", ReferenceFrame::GCRF),
            Err(ExportError::BlankReferenceFrameId)
        );
        context
            .register_reference_frame("gcrf", ReferenceFrame::GCRF)
            .expect("first frame registration");
        assert_eq!(
            context.register_reference_frame("gcrf-copy", ReferenceFrame::GCRF),
            Err(ExportError::DuplicateReferenceFrame)
        );
        assert_eq!(
            context.register_reference_frame("gcrf", ReferenceFrame::EME2000),
            Err(ExportError::DuplicateReferenceFrameId {
                id: "gcrf".to_owned()
            })
        );
        assert_eq!(
            context.register_frame_origin("  ", FrameOrigin::Body(Body::EARTH)),
            Err(ExportError::BlankFrameOriginId)
        );
        context
            .register_frame_origin("earth", FrameOrigin::Body(Body::EARTH))
            .expect("first origin registration");
        assert_eq!(
            context.frame_origin_id(FrameOrigin::Body(Body::EARTH)),
            Ok("earth")
        );
        assert_eq!(
            context.register_frame_origin("earth-copy", FrameOrigin::Body(Body::EARTH)),
            Err(ExportError::DuplicateFrameOrigin)
        );
        assert_eq!(
            context.register_frame_origin("earth", FrameOrigin::Body(Body::MOON)),
            Err(ExportError::DuplicateFrameOriginId {
                id: "earth".to_owned()
            })
        );
        assert_eq!(
            context.register_central_gravity("  ", Arc::clone(&gravity)),
            Err(ExportError::BlankCentralGravityId)
        );
        context
            .register_central_gravity("earth", Arc::clone(&gravity))
            .expect("first registration");
        assert_eq!(
            context.register_central_gravity("earth-copy", gravity),
            Err(ExportError::DuplicateCentralGravityProvider)
        );
        assert_eq!(
            context.register_central_gravity("earth", earth_gravity()),
            Err(ExportError::DuplicateCentralGravityId {
                id: "earth".to_owned()
            })
        );

        let unresolved_gravity = earth_gravity();
        let mut unresolved_context = ExportContext::new();
        unresolved_context
            .register_central_gravity("unresolved-earth", Arc::clone(&unresolved_gravity))
            .expect("unique provider registration");
        assert_eq!(
            unresolved_context.central_gravity_snapshot(&unresolved_gravity),
            Err(ExportError::UnregisteredFrameOrigin)
        );
    }

    #[cfg(all(feature = "json", feature = "two-bodies"))]
    #[test]
    fn propagator_configuration_encodes_as_versioned_json() {
        let gravity = earth_gravity();
        let dynamics = TwoBodyDynamics::new(PointMassGravityModel::new(Arc::clone(&gravity)));
        let propagator = EllipticKeplerPropagator::new(dynamics)
            .with_tolerance(Angle::new::<radian>(1.0e-12))
            .expect("positive tolerance")
            .with_max_iterations(48)
            .expect("positive iterations")
            .with_phase_error_budget(Angle::new::<radian>(1.0e-9))
            .expect("positive budget");
        let mut context = ExportContext::new();
        context
            .register_frame_origin("earth", FrameOrigin::Body(Body::EARTH))
            .expect("unique registration");
        context
            .register_central_gravity("mission-earth", gravity)
            .expect("unique registration");

        let snapshot = EllipticKeplerPropagatorSnapshot::try_from((&propagator, &context))
            .expect("registered export");
        let value: serde_json::Value = serde_json::from_str(
            &json::to_string(&snapshot).expect("finite values serialize to JSON"),
        )
        .expect("valid JSON");

        assert_eq!(value["schema"], "orskit.propagator.elliptic-kepler");
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["propagator"], "elliptic_kepler");
        assert_eq!(value["dynamics"], "two_body");
        assert_eq!(value["central_gravity"]["provider_id"], "mission-earth");
        assert_eq!(value["central_gravity"]["origin_id"], "earth");
        assert_eq!(
            value["central_gravity"]["gravitational_parameter_m3_s2"],
            3.986_004_418e14
        );
        assert_eq!(value["tolerance_rad"], 1.0e-12);
        assert_eq!(value["max_iterations"], 48);
        assert_eq!(value["phase_error_budget_rad"], 1.0e-9);

        let encoded = json::to_string(&snapshot).expect("finite values serialize to JSON");
        let decoded: EllipticKeplerPropagatorSnapshot =
            json::from_str(&encoded).expect("snapshot deserializes");
        let mut import_context = ImportContext::new();
        import_context
            .register_frame_origin("earth", FrameOrigin::Body(Body::EARTH))
            .expect("unique registration");
        import_context
            .register_central_gravity(
                "mission-earth",
                Arc::clone(propagator.dynamics().central_gravity()),
            )
            .expect("unique registration");
        let mut mismatched = decoded.clone();
        mismatched.central_gravity.gravitational_parameter_m3_s2 += 1.0;
        assert!(matches!(
            mismatched.try_into_propagator(&import_context),
            Err(ImportError::CentralGravityParameterMismatch { .. })
        ));

        let imported = decoded
            .try_into_propagator(&import_context)
            .expect("validated propagator import");
        assert_eq!(imported.tolerance(), propagator.tolerance());
        assert_eq!(imported.max_iterations(), propagator.max_iterations());
        assert_eq!(
            imported.phase_error_budget(),
            propagator.phase_error_budget()
        );
        assert!(Arc::ptr_eq(
            imported.dynamics().central_gravity(),
            propagator.dynamics().central_gravity()
        ));
    }

    #[cfg(all(feature = "json", feature = "orbits"))]
    #[test]
    fn orbit_json_names_every_raw_value_unit() {
        let orbit = Orbit::new(
            Epoch::from_gregorian_utc(2017, 1, 1, 0, 0, 0, 123),
            CartesianState::new(
                ReferenceFrame::GCRF,
                Position::from_metres(1.0, 2.0, 3.0),
                VelocityVector::from_metres_per_second(4.0, 5.0, 6.0),
            )
            .expect("finite state"),
        );
        let mut context = ExportContext::new();
        context
            .register_reference_frame("gcrf", ReferenceFrame::GCRF)
            .expect("unique registration");
        let snapshot = OrbitSnapshot::try_from((&orbit, &context)).expect("Cartesian export");
        let value: serde_json::Value =
            serde_json::from_str(&json::to_string(&snapshot).expect("finite snapshot"))
                .expect("valid JSON");

        assert_eq!(value["schema"], "orskit.orbit");
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["epoch"], "2017-01-01T00:00:00.000000123 UTC");
        assert_eq!(value["state"]["representation"], "cartesian");
        assert_eq!(value["state"]["frame"], "gcrf");
        assert_eq!(
            value["state"]["position_m"],
            serde_json::json!([1.0, 2.0, 3.0])
        );
        assert_eq!(
            value["state"]["velocity_m_s"],
            serde_json::json!([4.0, 5.0, 6.0])
        );

        let encoded = json::to_string(&snapshot).expect("finite snapshot");
        let decoded: OrbitSnapshot<CartesianStateSnapshot> =
            json::from_str(&encoded).expect("snapshot deserializes");
        let mut imports = ImportContext::new();
        imports
            .register_reference_frame("gcrf", ReferenceFrame::GCRF)
            .expect("unique registration");
        let restored: Orbit<CartesianState> = decoded
            .try_into_orbit(&imports)
            .expect("validated reconstruction");
        assert_eq!(restored, orbit);
    }
}
