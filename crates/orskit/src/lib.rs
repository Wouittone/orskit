//! Public facade for orskit.
//!
//! The facade is intentionally thin while the Rust core contracts are still
//! pre-alpha. It provides one stable import root for examples and downstream
//! experiments without hiding the focused crates that own domain behavior.
//!
//! ```
//! use orskit::prelude::*;
//!
//! let frame = ReferenceFrame::GCRF;
//! assert!(frame.is_inertial());
//! ```

pub use orskit_bodies as bodies;
pub use orskit_ccsds as ccsds;
pub use orskit_core as core;
pub use orskit_dynamics as dynamics;
pub use orskit_frames as frames;
pub use orskit_measurements as measurements;
pub use orskit_units as units;
pub use orskit_utils as utils;

/// Common types for small examples and user-facing workflows.
///
/// The prelude deliberately avoids glob-reexporting every crate. Prefer the
/// domain modules when a workflow needs more than these foundational identity,
/// state, propagation, measurement, and unit types.
pub mod prelude {
    pub use crate::bodies::{Body, BodyKind, BodySystem, CustomBodyId};
    pub use crate::core::{
        CartesianCoordinates, CartesianState, CoordinateSample, Epoch, EquinoctialState,
        FramedAcceleration, FramedAngularVelocity, FramedPosition, FramedVelocity, KeplerianState,
        Orbit, OrbitalConversion, OrbitalElements, Orientation, QuaternionAttitude, Spacecraft,
        SpacecraftShape, SpacecraftState, SpacecraftView, To, TryTo,
    };
    pub use crate::dynamics::{
        ConservativeForceModel, ConservativeForceModelHandle, DynamicsDescriptionError,
        EllipticTwoBodyPropagator, Force, ForceModel, GravityForce, NonConservativeForceModel,
        NonConservativeForceModelHandle, PointMassGravityModel, Propagator,
        SpacecraftStateDependencies, SystemDynamics, ThreeBodyDynamics, TwoBodyDynamics,
        TwoBodyPropagationError,
    };
    pub use crate::frames::{
        CustomFrameId, DerivedFrame, FrameDefinitionError, FrameMotion, FrameOrientation,
        FrameOrigin, ReferenceFrame,
    };
    pub use crate::measurements::{
        GroundStation, GroundStationError, MeasurementError, RangeMeasurement,
    };
    pub use crate::units::{
        Acceleration, AccelerationVector, Angle, AngularAcceleration, AngularVelocity,
        AngularVelocityVector, Area, GravitationalConstant, GravitationalParameter, Length, Mass,
        MomentOfInertia, Position, QuantityError, Ratio, Velocity, VelocityVector,
    };
}
