//! Measurement and observation handling.
//!
//! This crate owns the eventual participant-centric measurement model. Ground
//! assets are measurement participants rather than a separate domain crate,
//! alongside spacecraft and other observers. Participant paths, ground
//! geometry, clocks, and corrections will be designed together rather than
//! copied from Orekit's station API.

pub mod measurement;

pub use measurement::{MeasurementError, RangeMeasurement};
