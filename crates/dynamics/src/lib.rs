#![forbid(unsafe_code)]

//! Feature-gated dynamics capabilities.
//!
//! Core force and propagation contracts are always available. Enable
//! `numerical` for adaptive Cartesian propagation and `two-bodies` for
//! point-mass dynamics and the analytical elliptic Kepler propagator.
//! Independently opt in to `gauss-jackson` for fixed-step long arcs without
//! dense output; the native adaptive method is unchanged. The `sgp4` feature
//! adds model-specific mean-element propagation to TEME.

pub use dynamics_core::*;

#[cfg(feature = "sgp4")]
pub use dynamics_sgp4::{Sgp4Elements, Sgp4ElementsError, Sgp4Error, Sgp4Propagator};

#[cfg(feature = "sgp4")]
pub mod sgp4 {
    //! Analytical SGP4/SDP4 propagation using WGS-72 constants.

    pub use dynamics_sgp4::{Sgp4Elements, Sgp4ElementsError, Sgp4Error, Sgp4Propagator};
}

#[cfg(feature = "gauss-jackson")]
pub use dynamics_numerical::{
    GaussJackson8, GaussJacksonConfiguration, GaussJacksonConfigurationError,
    GaussJacksonPropagationError,
};

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
#[cfg(feature = "relativity")]
pub use dynamics_relativity::{
    ConfigurationError as RelativityConfigurationError,
    EvaluationError as RelativityEvaluationError, RelativisticGravityForce,
    Schwarzschild1PnCorrection,
};
#[cfg(feature = "spherical-harmonics")]
pub use dynamics_spherical_harmonics::{
    CoefficientEpochSemantics, CoefficientNormalization,
    ConstructionError as SphericalHarmonicConstructionError,
    EvaluationError as SphericalHarmonicEvaluationError, HarmonicCoefficient,
    HarmonicCoefficientMetadata, HarmonicCoefficientProvider, ProvenanceField, ProvenanceProvider,
    SourcedBodyFixedTransformProvider, SphericalHarmonicField, SphericalHarmonicGravity,
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

    #[cfg(feature = "gauss-jackson")]
    pub use dynamics_numerical::{
        GaussJackson8, GaussJacksonConfiguration, GaussJacksonConfigurationError,
        GaussJacksonPropagationError,
    };

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

#[cfg(feature = "relativity")]
pub mod relativity {
    //! Correction-only Schwarzschild first post-Newtonian gravity.

    pub use dynamics_relativity::{
        ConfigurationError, EvaluationError, RelativisticGravityForce, Schwarzschild1PnCorrection,
    };
}

#[cfg(feature = "spherical-harmonics")]
pub mod spherical_harmonics {
    //! Caller-supplied general spherical-harmonic gravity capability.

    pub use dynamics_spherical_harmonics::{
        CoefficientEpochSemantics, CoefficientNormalization,
        ConstructionError as SphericalHarmonicConstructionError,
        EvaluationError as SphericalHarmonicEvaluationError, HarmonicCoefficient,
        HarmonicCoefficientMetadata, HarmonicCoefficientProvider, ProvenanceField,
        ProvenanceProvider, SourcedBodyFixedTransformProvider, SphericalHarmonicField,
        SphericalHarmonicGravity, SphericalHarmonicGravityModel, TideSystem,
    };
}
