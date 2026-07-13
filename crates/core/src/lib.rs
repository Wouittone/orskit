//! Core state types for orskit.
//!
//! Physical scalars and vectors are strongly typed, epochs use Hifitime
//! directly, and each coordinate-dependent value carries its own frame.
//! [`SpacecraftState`] is the closed set of six-element orbital
//! representations and [`Orbit`] qualifies one with an epoch. [`Spacecraft`]
//! contains time-independent identity and geometry; [`SpacecraftView`]
//! composes a complete epoch-specific physical state.

mod gravity;
mod kinematics;
mod spacecraft;
mod state;

pub use frames;
pub use gravity::{GravityContext, GravityContextId, ScientificSource, ScientificSourceError};
pub use hifitime::Epoch;
pub use kinematics::{
    CartesianCoordinates, FramedAcceleration, FramedPosition, FramedVelocity, KinematicError,
};
pub use spacecraft::{
    AttitudeError, AttitudeState, FramedAngularVelocity, InertiaError, InertiaTensor, Orientation,
    OrientationError, QuaternionAttitude, ShapeError, Spacecraft, SpacecraftError, SpacecraftShape,
    SpacecraftView, SpacecraftViewError,
};
pub use state::{
    CartesianState, CoordinateSample, EquinoctialState, KeplerianState, Orbit, SpacecraftState,
    StateError, To,
};
pub use units;
