#![forbid(unsafe_code)]

//! Feature-gated public facade for orskit domain contracts and implementations.
//!
//! `core`, `frames`, and `units` are always available. Select concrete state,
//! attitude, geometry, measurement, dynamics, and I/O implementations
//! explicitly with Cargo features; the default facade intentionally links no
//! physical model implementation.

pub use frames;
pub use orskit_core as core;
pub use units;

#[cfg(feature = "bodies")]
pub use bodies;
#[cfg(feature = "ccsds")]
pub use ccsds;
#[cfg(feature = "dynamics")]
pub use dynamics;
#[cfg(feature = "two-bodies")]
pub use dynamics_two_bodies;
#[cfg(feature = "point-mass-gravity")]
pub use gravity;
#[cfg(feature = "measurements")]
pub use measurements;
#[cfg(feature = "cartesian")]
pub use orbits;

/// Conservative imports for selected workflow capabilities.
pub mod prelude {
    pub use crate::core::{Orbit, SpacecraftState};
    pub use crate::frames::{FrameCatalog, FrameNamespace, ReferenceFrame};
    pub use crate::units::{Length, Position, VelocityVector};

    #[cfg(feature = "bodies")]
    pub use crate::bodies::{Body, BodySystem};
    #[cfg(feature = "dynamics")]
    pub use crate::dynamics::{ComposedDynamics, PropagationState, Propagator};
    #[cfg(feature = "two-bodies")]
    pub use crate::dynamics_two_bodies::{EllipticKeplerPropagator, TwoBodyDynamics};
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
    #[cfg(feature = "cartesian")]
    pub use crate::orbits::{
        cartesian::CartesianState, circular::CircularState, equinoctial::EquinoctialState,
        keplerian::KeplerianState,
    };
}
