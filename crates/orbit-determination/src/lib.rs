#![forbid(unsafe_code)]

//! Sequential orbit determination over orskit's propagation contracts.
//!
//! # What an application provides
//!
//! To run the current Cartesian filters, provide:
//!
//! 1. an [`Orbit`] containing an [`orbits::cartesian::CartesianState`] and a
//!    matching [`CartesianCovariance`];
//! 2. a [`dynamics::Propagator`] which owns its physical problem, such as an
//!    `EllipticKeplerPropagator` constructed with a
//!    `dynamics::TwoBodyDynamics`;
//! 3. positive-definite process noise and one or more [`CartesianObservation`]
//!    values; and
//! 4. either [`ExtendedKalmanFilter`] or [`UnscentedKalmanFilter`].
//!
//! A custom position observation implements [`CartesianObservation`]; future
//! range or angular filters will introduce their own typed observation contracts
//! rather than forcing values through a public numerical vector.
//!
//! Filters deliberately do not define force-model or integration traits.  They
//! invoke the caller-selected [`dynamics::Propagator`] for every propagation,
//! leaving two-body, three-body, numerical, and operational propagators in
//! their dedicated dynamics crates. [`EstimationObserver`] is optional and is
//! never retained by a filter, so normal production estimation stores no
//! diagnostic history.

mod covariance;
mod diagnostics;
mod error;
mod extended;
mod filter;
mod numerical;
mod observation;
mod unscented;

pub use covariance::{
    CartesianCovariance, CartesianStateEstimate, PositionCovariance, StateCovariance, StateEstimate,
};
pub use diagnostics::{CorrectionEvent, EstimationObserver, PredictionEvent};
pub use error::OrbitDeterminationError;
pub use extended::ExtendedKalmanFilter;
pub use filter::{KalmanFilter, OrbitDetermination};
pub use observation::{CartesianObservation, CartesianPositionObservation};
pub use orskit_core::{Orbit, SpacecraftState};
pub use unscented::{UnscentedConfiguration, UnscentedKalmanFilter};

#[cfg(test)]
mod tests;
