#![forbid(unsafe_code)]

//! Strongly typed physical quantities used by orskit.
//!
//! Scalar quantities use [`uom`] so dimensional mistakes are compile-time
//! errors. Cartesian values wrap three quantities of the same dimension, which
//! prevents positions, velocities, and accelerations from being interchanged.
//!
//! ```compile_fail
//! use units::{Length, Mass};
//! use units::uom::si::{length::meter, mass::kilogram};
//!
//! let distance = Length::new::<meter>(1.0);
//! let mass = Mass::new::<kilogram>(1.0);
//! let _invalid = distance + mass;
//! ```

mod astronomy;
mod vector;

pub use astronomy::{GravitationalConstant, GravitationalParameter, QuantityError};
pub use uom;
pub use uom::si::f64::{
    Acceleration, Angle, AngularAcceleration, AngularVelocity, Area, Length, Mass, MomentOfInertia,
    Ratio, Velocity,
};
pub use vector::{AccelerationVector, AngularVelocityVector, Position, VelocityVector};
