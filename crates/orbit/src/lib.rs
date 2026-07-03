//! Orbital propagation and dynamics.
//!
//! This crate provides orbital propagation capabilities using ODE solvers,
//! including gravity modeling and perturbation handling.

pub mod propagator;

pub use propagator::{DynamicsError, TranslationalDerivative, TwoBodyDynamics};
