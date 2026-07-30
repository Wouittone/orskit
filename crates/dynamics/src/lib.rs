#![forbid(unsafe_code)]

//! Feature-gated dynamics capabilities.
//!
//! Core force and propagation contracts are always available. Enable
//! `numerical` for adaptive Cartesian propagation and `two-bodies` for
//! point-mass dynamics and the analytical elliptic Kepler propagator.

pub use dynamics_core::*;

#[cfg(feature = "numerical")]
pub use dynamics_numerical::{
    BogackiShampine32, CartesianEphemeris, CartesianMassState, CartesianStateTransition,
    ConstantThrustManeuver, CovariancePropagation, DenseOutputError, DensePropagation, EventAction,
    EventCallbackError, EventConfiguration, EventConfigurationError, EventDetector, EventDirection,
    EventHandler, EventOccurrence, EventPropagation, ImpulsiveManeuver, IntegrationConfiguration,
    IntegrationConfigurationError, ManeuverConfigurationError, ManeuverDynamicsError,
    ManeuverExecution, ManeuverExecutionKind, ManeuverPropagation, ManeuverPropagationError,
    ManeuverSchedule, NumericalPropagationError, ThrustVector, VariationalConfiguration,
    VariationalConfigurationError, VariationalPropagation, VariationalPropagationError,
};
#[cfg(feature = "two-bodies")]
pub use dynamics_two_bodies::{
    EllipticKeplerPropagator, PointMassGravityModel, TwoBodyDynamics, TwoBodyEvaluationError,
};

#[cfg(feature = "numerical")]
pub mod numerical {
    //! Adaptive Cartesian numerical propagation.

    pub use dynamics_numerical::{
        BogackiShampine32, CartesianEphemeris, CartesianMassState, CartesianStateTransition,
        ConstantThrustManeuver, CovariancePropagation, DenseOutputError, DensePropagation,
        EventAction, EventCallbackError, EventConfiguration, EventConfigurationError,
        EventDetector, EventDirection, EventHandler, EventOccurrence, EventPropagation,
        ImpulsiveManeuver, IntegrationConfiguration, IntegrationConfigurationError,
        ManeuverConfigurationError, ManeuverDynamicsError, ManeuverExecution,
        ManeuverExecutionKind, ManeuverPropagation, ManeuverPropagationError, ManeuverSchedule,
        NumericalPropagationError, ThrustVector, VariationalConfiguration,
        VariationalConfigurationError, VariationalPropagation, VariationalPropagationError,
    };
}

#[cfg(feature = "two-bodies")]
pub mod two_bodies {
    //! Point-mass two-body dynamics capability.

    pub use dynamics_two_bodies::{
        EllipticKeplerPropagator, PointMassGravityModel, TwoBodyDynamics, TwoBodyEvaluationError,
    };
}
