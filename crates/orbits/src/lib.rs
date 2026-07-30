#![forbid(unsafe_code)]

//! Feature-gated orbit state implementations.
//!
//! Applications select concrete representations through modules such as
//! [`cartesian`] and [`keplerian`], while `core::SpacecraftState` remains the
//! implementation-neutral contract.

#[cfg(feature = "cartesian")]
mod covariance;
#[cfg(feature = "cartesian")]
mod kinematics;
#[cfg(feature = "cartesian")]
mod state;

/// Cartesian position/velocity state and coordinate types.
#[cfg(feature = "cartesian")]
pub mod cartesian {
    pub use crate::covariance::{CartesianCovariance, CartesianCovarianceError};
    pub use crate::kinematics::{
        CartesianCoordinates, FramedAcceleration, FramedPosition, FramedVelocity, KinematicError,
    };
    pub use crate::state::{
        CartesianState, CartesianStateWithGravity, CoordinateSample, StateError,
    };
}

/// Elliptic circular state representation.
#[cfg(feature = "cartesian")]
pub mod circular {
    pub use crate::state::{CircularState, StateError};
}

/// Classical elliptic Keplerian state representation.
#[cfg(feature = "cartesian")]
pub mod keplerian {
    pub use crate::state::{KeplerianState, StateError};
}

/// Elliptic equinoctial state representation.
#[cfg(feature = "cartesian")]
pub mod equinoctial {
    pub use crate::state::{EquinoctialState, StateError};
}
