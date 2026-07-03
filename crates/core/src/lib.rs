//! Core state types for orskit.
//!
//! Physical scalars and vectors are strongly typed, epochs use Hifitime
//! directly, and every translational state carries a reference frame.

mod spacecraft;

pub use hifitime::Epoch;
pub use orskit_frames as frames;
pub use orskit_units as units;
pub use spacecraft::{
    InertiaError, InertiaTensor, Orientation, OrientationError, SpacecraftState, StateError,
};
