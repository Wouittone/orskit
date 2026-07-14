#![forbid(unsafe_code)]

//! Cartesian and elliptic element implementations of the open orskit state
//! contract.
//!
//! These types are intentionally separate from `orskit-core`: applications
//! that use another state representation need not link Cartesian conversion
//! equations or their gravity-specific validation.

mod kinematics;
mod state;

pub use kinematics::{
    CartesianCoordinates, FramedAcceleration, FramedPosition, FramedVelocity, KinematicError,
};
pub use state::{
    CartesianState, CoordinateSample, EquinoctialState, KeplerianState, SpacecraftState,
    StateError, To,
};
