#![forbid(unsafe_code)]

//! Feature-gated public facade for orskit domain contracts and implementations.
//!
//! `core`, `frames`, and `units` are always available. Select concrete state,
//! attitude, geometry, measurement, dynamics, and I/O implementations
//! explicitly with Cargo features; the default facade intentionally links no
//! physical model implementation. The `serialization` feature provides
//! format-neutral owned snapshots and validated reconstruction, while
//! `serialization-json` adds JSON encoding and decoding.
//!
//! This is the sole user-facing API layer. Domain types and operations stay
//! here and in the focused crates it re-exports; vector/matrix kernels and
//! algorithm workspaces remain implementation details except at explicitly
//! named interoperability boundaries.

pub use frames;
pub use orskit_core as core;
pub use orskit_data as data;
pub use units;

#[cfg(feature = "bodies")]
pub use bodies;
#[cfg(feature = "ccsds")]
pub use ccsds;
#[cfg(feature = "dynamics")]
pub use dynamics;
#[cfg(feature = "ephemeris")]
pub use ephemeris;
#[cfg(feature = "point-mass-gravity")]
pub use gravity;
#[cfg(feature = "measurements")]
pub use measurements;
#[cfg(feature = "orbit-determination")]
pub use orbit_determination;
#[cfg(feature = "cartesian")]
pub use orbits;
#[cfg(feature = "serialization")]
pub use orskit_export as export;
#[cfg(feature = "tle")]
pub use tle;

/// Conservative imports for selected workflow capabilities.
pub mod prelude {
    pub use crate::core::{Orbit, SpacecraftState};
    pub use crate::data::{
        ArtifactCoverage, ArtifactDescriptor, Sha256Digest, TimeCoverage, VerifiedArtifact,
    };
    pub use crate::frames::{
        FrameCatalog, FrameNamespace, GeodeticPosition, ReferenceEllipsoid, ReferenceFrame,
        TopocentricFrame,
    };
    pub use crate::units::{Length, Position, VelocityVector};

    #[cfg(feature = "bodies")]
    pub use crate::bodies::{Body, BodySystem};
    #[cfg(feature = "dynamics")]
    pub use crate::dynamics::{ComposedDynamics, PropagationState, Propagator};
    #[cfg(feature = "two-bodies")]
    pub use crate::dynamics::{EllipticKeplerPropagator, TwoBodyDynamics};
    #[cfg(feature = "sgp4")]
    pub use crate::dynamics::{Sgp4Elements, Sgp4ElementsError, Sgp4Error, Sgp4Propagator};
    #[cfg(feature = "ephemeris")]
    pub use crate::ephemeris::{EphemerisProvider, EphemerisQuery, EphemerisState};
    #[cfg(feature = "serialization")]
    pub use crate::export::{
        ExportContext, ExportableState, ImportContext, ImportableState, OrbitSnapshot,
    };
    #[cfg(feature = "point-mass-gravity")]
    pub use crate::gravity::PointMass;
    #[cfg(feature = "measurement-range")]
    pub use crate::measurements::RangeMeasurement;
    #[cfg(feature = "measurement-estimation")]
    pub use crate::measurements::{
        CorrectionModelChain, MeasurementEstimator, ParticipantStateProvider,
        SignalPropagationSolver,
    };
    #[cfg(feature = "measurements")]
    pub use crate::measurements::{Measurement, ParticipantId, SignalPath};
    #[cfg(feature = "orbit-determination")]
    pub use crate::orbit_determination::{
        CartesianCovariance, CartesianObservation, CartesianPositionObservation,
        CartesianStateEstimate, EstimationObserver, ExtendedKalmanFilter, KalmanFilter,
        OrbitDetermination, PositionCovariance, StateEstimate, UnscentedConfiguration,
        UnscentedKalmanFilter,
    };
    #[cfg(feature = "cartesian")]
    pub use crate::orbits::{
        cartesian::CartesianState, circular::CircularState, equinoctial::EquinoctialState,
        keplerian::KeplerianState,
    };
    #[cfg(feature = "sgp4")]
    pub use crate::tle::Sgp4ConversionError;
    #[cfg(feature = "tle")]
    pub use crate::tle::TwoLineElement;
}
