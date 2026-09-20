#![forbid(unsafe_code)]

//! Feature-gated dynamics capabilities.
//!
//! Core force and propagation contracts are always available. Enable
//! `numerical` for adaptive Cartesian propagation and `two-bodies` for
//! point-mass dynamics and the analytical elliptic Kepler propagator.

pub use dynamics_core::*;

#[cfg(feature = "drag")]
pub use dynamics_drag::{
    AtmosphereRelativeVelocity, AtmosphereRelativeVelocityProvider, AtmosphericDragForce,
    CannonballDragModel, DragArea, DragCoefficient, DragEvaluationError, DragInputError, DragMass,
    RelativeVelocityError,
};
#[cfg(feature = "harmonics")]
pub use dynamics_harmonics::{J2Dynamics, J2EvaluationError, J2GravityModel, J2OblatenessForce};
#[cfg(feature = "attitude")]
pub use dynamics_numerical::{AttitudeManeuverDynamicsError, AttitudeManeuverPropagationError};
#[cfg(feature = "numerical")]
pub use dynamics_numerical::{
    BogackiShampine32, CartesianEphemeris, CartesianMassState, CartesianStateTransition,
    ConstantThrustManeuver, CovariancePropagation, DenseOutputError, DensePropagation, EventAction,
    EventCallbackError, EventConfiguration, EventConfigurationError, EventDetector, EventDirection,
    EventHandler, EventOccurrence, EventPropagation, ImpulsiveManeuver, IntegrationConfiguration,
    IntegrationConfigurationError, ManeuverConfigurationError, ManeuverDynamicsError,
    ManeuverExecution, ManeuverExecutionKind, ManeuverPropagation, ManeuverPropagationError,
    ManeuverSchedule, NumericalPropagationError, ThrustFrame, ThrustVector,
    VariationalConfiguration, VariationalConfigurationError, VariationalPropagation,
    VariationalPropagationError,
};
#[cfg(feature = "spherical-harmonics")]
pub use dynamics_spherical_harmonics::{
    CoefficientNormalization, ConstructionError as SphericalHarmonicConstructionError,
    EvaluationError as SphericalHarmonicEvaluationError, HarmonicCoefficient,
    HarmonicCoefficientProvider, SphericalHarmonicField, SphericalHarmonicGravity,
    SphericalHarmonicGravityModel, TideSystem,
};
#[cfg(feature = "srp")]
pub use dynamics_srp::{
    CannonballSolarRadiationPressure, EclipseGeometry, OpticalCoefficient, SolarFlux,
    SolarFluxProvider, SrpArea, SrpEvaluationError, SrpInputError, SrpMass, SrpSpacecraft,
    SPEED_OF_LIGHT_M_PER_S,
};
#[cfg(feature = "third-bodies")]
pub use dynamics_third_bodies::{
    DifferentialThirdBodyModel, ThirdBodyAccelerationConvention, ThirdBodyError,
    ThirdBodyGravityForce,
};
#[cfg(feature = "two-bodies")]
pub use dynamics_two_bodies::{
    EllipticKeplerPropagator, PointMassGravityModel, TwoBodyDynamics, TwoBodyEvaluationError,
};

#[cfg(feature = "numerical")]
pub mod numerical {
    //! Adaptive Cartesian numerical propagation.

    #[cfg(feature = "attitude")]
    pub use dynamics_numerical::{AttitudeManeuverDynamicsError, AttitudeManeuverPropagationError};
    pub use dynamics_numerical::{
        BogackiShampine32, CartesianEphemeris, CartesianMassState, CartesianStateTransition,
        ConstantThrustManeuver, CovariancePropagation, DenseOutputError, DensePropagation,
        EventAction, EventCallbackError, EventConfiguration, EventConfigurationError,
        EventDetector, EventDirection, EventHandler, EventOccurrence, EventPropagation,
        ImpulsiveManeuver, IntegrationConfiguration, IntegrationConfigurationError,
        ManeuverConfigurationError, ManeuverDynamicsError, ManeuverExecution,
        ManeuverExecutionKind, ManeuverPropagation, ManeuverPropagationError, ManeuverSchedule,
        NumericalPropagationError, ThrustFrame, ThrustVector, VariationalConfiguration,
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

#[cfg(feature = "harmonics")]
pub mod harmonics {
    //! Point-mass-plus-`J2` oblateness gravity dynamics capability.

    pub use dynamics_harmonics::{
        J2Dynamics, J2EvaluationError, J2GravityModel, J2OblatenessForce,
    };
}

#[cfg(feature = "drag")]
pub mod drag {
    //! Cannonball atmospheric-drag capability.

    pub use dynamics_drag::{
        AtmosphereRelativeVelocity, AtmosphereRelativeVelocityProvider, AtmosphericDragForce,
        CannonballDragModel, DragArea, DragCoefficient, DragEvaluationError, DragInputError,
        DragMass, RelativeVelocityError,
    };
}

#[cfg(feature = "third-bodies")]
pub mod third_bodies {
    //! Differential third-body point-mass gravity capability.

    pub use dynamics_third_bodies::{
        DifferentialThirdBodyModel, ThirdBodyAccelerationConvention, ThirdBodyError,
        ThirdBodyGravityForce,
    };
}

#[cfg(feature = "srp")]
pub mod srp {
    //! Cannonball solar-radiation-pressure and eclipse capability.

    pub use dynamics_srp::{
        CannonballSolarRadiationPressure, EclipseGeometry, OpticalCoefficient, SolarFlux,
        SolarFluxProvider, SrpArea, SrpEvaluationError, SrpInputError, SrpMass, SrpSpacecraft,
        SPEED_OF_LIGHT_M_PER_S,
    };
}

#[cfg(feature = "spherical-harmonics")]
pub mod spherical_harmonics {
    //! Caller-supplied low-degree zonal spherical-harmonic gravity capability.

    pub use dynamics_spherical_harmonics::{
        CoefficientNormalization, ConstructionError as SphericalHarmonicConstructionError,
        EvaluationError as SphericalHarmonicEvaluationError, HarmonicCoefficient,
        HarmonicCoefficientProvider, SphericalHarmonicField, SphericalHarmonicGravity,
        SphericalHarmonicGravityModel, TideSystem,
    };
}
