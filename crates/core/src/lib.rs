//! Core state types for orskit.
//!
//! Physical scalars and vectors are strongly typed, epochs use Hifitime
//! directly, and each coordinate-dependent value carries its own frame.

mod kinematics;
mod spacecraft;

pub use hifitime::Epoch;
pub use kinematics::{FramedAcceleration, FramedPosition, FramedVelocity, KinematicError};
pub use orskit_frames as frames;
pub use orskit_units as units;
pub use spacecraft::{
    InertiaError, InertiaTensor, Orientation, OrientationError, SpacecraftState, StateError,
};
