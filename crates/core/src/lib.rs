//! Core state types for orskit.
//!
//! Physical scalars and vectors are strongly typed, epochs use Hifitime
//! directly, and each coordinate-dependent value carries its own frame.
//! [`State`] is the common physical contract implemented by [`CartesianState`],
//! [`KeplerianState`], and [`EquinoctialState`].

mod kinematics;
mod spacecraft;
mod state;

pub use hifitime::Epoch;
pub use kinematics::{
    CartesianCoordinates, FramedAcceleration, FramedPosition, FramedVelocity, KinematicError,
};
pub use orskit_frames as frames;
pub use orskit_units as units;
pub use spacecraft::{
    InertiaError, InertiaTensor, Orientation, OrientationError, SpacecraftProperties,
};
pub use state::{
    CartesianState, CoordinateSample, EquinoctialCoordinates, EquinoctialState,
    KeplerianCoordinates, KeplerianState, State, StateConversion, StateError,
};
