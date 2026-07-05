//! Core state types for orskit.
//!
//! Physical scalars and vectors are strongly typed, epochs use Hifitime
//! directly, and each coordinate-dependent value carries its own frame.
//! [`SpacecraftState`] is the closed set of six-element orbital
//! representations. [`Spacecraft`] contains time-independent identity and
//! geometry; [`SpacecraftView`] composes its epoch-specific physical state.

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
    AttitudeError, AttitudeState, FramedAngularVelocity, InertiaError, InertiaTensor, Orientation,
    OrientationError, QuaternionAttitude, ShapeError, Spacecraft, SpacecraftError, SpacecraftShape,
    SpacecraftView, SpacecraftViewError,
};
pub use state::{
    CartesianState, CoordinateSample, EquinoctialState, KeplerianState, OrbitalConversion,
    OrbitalElements, SpacecraftState, StateError, To, TryTo,
};
