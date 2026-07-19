#![forbid(unsafe_code)]

//! Feature-gated dynamics capabilities.
//!
//! Core force and propagation contracts are always available. Enable the
//! `two-bodies` feature to add point-mass two-body dynamics and the analytical
//! elliptic Kepler propagator.

pub use dynamics_core::*;

#[cfg(feature = "two-bodies")]
pub use dynamics_two_bodies::{EllipticKeplerPropagator, PointMassGravityModel, TwoBodyDynamics};

#[cfg(feature = "two-bodies")]
pub mod two_bodies {
    //! Point-mass two-body dynamics capability.

    pub use dynamics_two_bodies::{
        EllipticKeplerPropagator, PointMassGravityModel, TwoBodyDynamics,
    };
}
