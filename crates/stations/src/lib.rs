//! Ground station and geographic location utilities
//!
//! This crate provides structures and utilities for handling ground stations,
//! geographic locations, and site-specific calculations.

pub mod location;

pub use location::{GeographicLocation, LocationError};
