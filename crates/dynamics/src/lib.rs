#![forbid(unsafe_code)]

//! Feature-gated dynamics capabilities.
//!
//! Core force and propagation contracts are always available. Enable the
//! `numerical` feature for adaptive Cartesian integration or `two-bodies` for
//! point-mass dynamics and the analytical elliptic Kepler propagator.

pub use dynamics_core::*;

#[cfg(feature = "numerical")]
pub use dynamics_numerical::{
    AdaptiveRungeKuttaConfig, AdaptiveRungeKuttaFehlberg, AdaptiveStepBounds, AdaptiveStepLimits,
    CartesianDynamics, CartesianTolerances, DenseEphemeris, DenseOutputError, EphemerisInterval,
    EventAction, EventDetector, EventDirection, EventOccurrence, EventSearchConfig,
    EventSearchConfigError, EventSearchError, EventSearchOutcome, EventStage,
    NumericalPropagationError, NumericalPropagatorBuildError, StepBoundsError, ToleranceError,
};

#[cfg(feature = "numerical")]
pub mod numerical {
    //! Adaptive translational Cartesian propagation.

    pub use dynamics_numerical::*;
}

#[cfg(feature = "sgp4")]
pub use sgp4::{Sgp4Elements, Sgp4ElementsError, Sgp4Error, Sgp4Propagator};

#[cfg(feature = "sgp4")]
pub mod sgp4;

#[cfg(feature = "two-bodies")]
pub use dynamics_two_bodies::{EllipticKeplerPropagator, PointMassGravityModel, TwoBodyDynamics};

#[cfg(feature = "two-bodies")]
pub mod two_bodies {
    //! Point-mass two-body dynamics capability.

    pub use dynamics_two_bodies::{
        EllipticKeplerPropagator, PointMassGravityModel, TwoBodyDynamics,
    };
}
