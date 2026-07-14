#![forbid(unsafe_code)]

//! Measurement and observation handling.
//!
//! This crate owns the participant-centric measurement model. Every range
//! observation declares its ordered signal participants, the signal event whose
//! epoch is recorded, and the scalar range convention. Ground stations are
//! represented here as parent-relative frame participants rather than in a
//! separate station domain. Clocks, corrections, and richer ground geometry
//! remain explicit future capabilities.

pub mod measurement;
pub mod participant;
pub mod station;

pub use measurement::{MeasurementError, ObservationTimeTag, RangeConvention, RangeMeasurement};
pub use participant::{ParticipantId, ParticipantIdError, SignalPath, SignalPathError};
pub use station::GroundStation;
