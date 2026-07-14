#![forbid(unsafe_code)]

//! Feature-gated public facade for orskit domain contracts and implementations.
//!
//! `core`, `frames`, and `units` are always available. Select concrete state,
//! dynamics, and I/O implementations explicitly with Cargo features; the
//! default facade intentionally links no physical model implementation.

pub use core_crate as core;
pub use frames;
pub use units;

#[cfg(feature = "bodies")]
pub use bodies;
#[cfg(feature = "ccsds")]
pub use ccsds;
#[cfg(feature = "dynamics")]
pub use dynamics;
#[cfg(feature = "two-body")]
pub use dynamics_two_body;
#[cfg(feature = "point-mass-gravity")]
pub use gravity_point_mass;
#[cfg(feature = "measurements")]
pub use measurements;
#[cfg(feature = "cartesian")]
pub use orbits_cartesian;

/// Conservative imports for selected workflow capabilities.
pub mod prelude {
    pub use crate::core::{Orbit, SpacecraftState};
    pub use crate::frames::{FrameCatalog, FrameNamespace, ReferenceFrame};
    pub use crate::units::{Length, Position, VelocityVector};

    #[cfg(feature = "bodies")]
    pub use crate::bodies::{Body, BodySystem};
    #[cfg(feature = "dynamics")]
    pub use crate::dynamics::{ComposedDynamics, Propagator};
    #[cfg(feature = "two-body")]
    pub use crate::dynamics_two_body::{EllipticKeplerPropagator, TwoBodyDynamics};
    #[cfg(feature = "point-mass-gravity")]
    pub use crate::gravity_point_mass::{PointMassGravity, ReferenceSource};
    #[cfg(feature = "measurements")]
    pub use crate::measurements::{ParticipantId, RangeMeasurement, SignalPath};
    #[cfg(feature = "cartesian")]
    pub use crate::orbits_cartesian::{CartesianState, EquinoctialState, KeplerianState};
}
