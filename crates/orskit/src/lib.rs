#![forbid(unsafe_code)]

//! Thin public facade for the focused orskit crates.
//!
//! Domain crates remain independently usable; this crate provides one stable
//! discovery/import root without moving ownership into a monolith.
//!
//! ```
//! use orskit::prelude::*;
//!
//! let mut catalog = FrameCatalog::new(FrameNamespace::new(1), [ReferenceFrame::ITRF2020])?;
//! let site = catalog.define_parent_aligned(
//!     1,
//!     ReferenceFrame::ITRF2020,
//!     Position::from_metres(6_378_137.0, 0.0, 0.0),
//! )?;
//! assert_eq!(site.parent(), ReferenceFrame::ITRF2020);
//! # Ok::<(), orskit::frames::FrameDefinitionError>(())
//! ```

pub use bodies;
pub use ccsds;
pub use core_crate as core;
pub use dynamics;
pub use frames;
pub use measurements;
pub use units;

/// Conservative imports for common workflow construction.
pub mod prelude {
    pub use crate::bodies::{Body, BodySystem};
    pub use crate::core::{CartesianState, Orbit, SpacecraftState};
    pub use crate::dynamics::{Propagator, TwoBodyDynamics};
    pub use crate::frames::{FrameCatalog, FrameNamespace, ReferenceFrame};
    pub use crate::measurements::{ParticipantId, RangeMeasurement, SignalPath};
    pub use crate::units::{Length, Position, VelocityVector};
}
