//! Measurement and observation handling.
//!
//! This crate owns the participant-centric measurement model. Ground stations
//! are represented here as parent-relative frame participants rather than in a
//! separate station domain. Participant paths, clocks, corrections, and richer
//! ground geometry will be added here without copying Orekit's station API.

pub mod measurement;
pub mod station;

pub use measurement::{MeasurementError, RangeMeasurement};
pub use station::{GroundStation, GroundStationError};
