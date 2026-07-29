#![forbid(unsafe_code)]

//! Feature-gated dynamics capabilities.
//!
//! Core force and propagation contracts are always available. Enable
//! `numerical` for adaptive Cartesian propagation and `two-bodies` for
//! point-mass dynamics and the analytical elliptic Kepler propagator.

pub use dynamics_core::*;

#[cfg(feature = "numerical")]
pub use dynamics_numerical::{
    BogackiShampine32, IntegrationConfiguration, IntegrationConfigurationError,
    NumericalPropagationError,
};
#[cfg(feature = "two-bodies")]
pub use dynamics_two_bodies::{
    EllipticKeplerPropagator, PointMassGravityModel, TwoBodyDynamics, TwoBodyEvaluationError,
};

#[cfg(feature = "numerical")]
pub mod numerical {
    //! Adaptive Cartesian numerical propagation.

    pub use dynamics_numerical::{
        BogackiShampine32, IntegrationConfiguration, IntegrationConfigurationError,
        NumericalPropagationError,
    };
}

#[cfg(feature = "two-bodies")]
pub mod two_bodies {
    //! Point-mass two-body dynamics capability.

    pub use dynamics_two_bodies::{
        EllipticKeplerPropagator, PointMassGravityModel, TwoBodyDynamics, TwoBodyEvaluationError,
    };
}
