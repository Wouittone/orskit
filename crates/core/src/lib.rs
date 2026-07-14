#![forbid(unsafe_code)]

//! Implementation-neutral domain contracts for orskit.
//!
//! Physical scalars and vectors are strongly typed, epochs use Hifitime
//! directly, and each coordinate-dependent value carries its own frame.
//! [`SpacecraftState`] is an open frame-qualified state contract and
//! [`Orbit`] qualifies any selected state with an epoch. [`Spacecraft`]
//! contains time-independent identity and geometry; [`SpacecraftView`]
//! composes it with a chosen epoch-specific state implementation.

mod orbit;
mod spacecraft;

pub use frames;
pub use hifitime::Epoch;
pub use orbit::{Orbit, SpacecraftState};
pub use spacecraft::{
    Attitude, AttitudeError, BodyAngularVelocity, InertiaError, InertiaTensor, Orientation,
    OrientationError, ShapeError, Spacecraft, SpacecraftBodyFrame, SpacecraftError,
    SpacecraftGeometry, SpacecraftView, SpacecraftViewError,
};
#[cfg(feature = "quaternion-attitude")]
pub use spacecraft::{AttitudeState, QuaternionAttitude};
#[cfg(feature = "standard-shapes")]
pub use spacecraft::{CuboidShape, SpacecraftShape, SphereShape};
pub use units;
