//! Repeated independent Cartesian position EKF corrections for cross-project
//! timing. This is a narrow, deterministic workload, not an accuracy claim.

use std::{hint::black_box, sync::Arc, time::Instant};

use bodies::Body;
use dynamics::{EllipticKeplerPropagator, PointMassGravityModel, TwoBodyDynamics};
use frames::{FrameOrigin, ReferenceFrame};
use gravity::{CentralGravityProvider, SharedCentralGravity};
use hifitime::Epoch;
use orbit_determination::{
    CartesianCovariance, CartesianPositionObservation, CartesianStateEstimate,
    ExtendedKalmanFilter, Orbit, OrbitDetermination, PositionCovariance, StateEstimate,
};
use orbits::cartesian::CartesianState;
use units::{GravitationalParameter, Position, VelocityVector};

const DEFAULT_ITERATIONS: usize = 10_000;
const WARMUP_ITERATIONS: usize = 100;
const EARTH_MU: f64 = 3.986_004_415e14;

#[derive(Debug)]
struct EarthGravity;

impl CentralGravityProvider for EarthGravity {
    fn origin(&self) -> FrameOrigin {
        FrameOrigin::Body(Body::EARTH)
    }

    fn parameter(&self) -> GravitationalParameter {
        GravitationalParameter::try_from(EARTH_MU).expect("positive gravitational parameter")
    }
}

fn main() {
    let iterations = std::env::args()
        .nth(1)
        .map(|value| {
            value
                .parse()
                .expect("iterations must be a positive integer")
        })
        .unwrap_or(DEFAULT_ITERATIONS);
    assert!(iterations > 0, "iterations must be positive");

    run_queries(WARMUP_ITERATIONS);
    let started = Instant::now();
    let checksum = run_queries(iterations);
    let elapsed_ns = started.elapsed().as_nanos();
    println!(
        "implementation=orskit-ekf-position iterations={iterations} elapsed_ns={elapsed_ns} checksum={checksum:.17e}"
    );
}

fn run_queries(iterations: usize) -> f64 {
    (0..iterations)
        .map(|_| {
            let posterior = ExtendedKalmanFilter::new(propagator(), prior(), process_noise())
                .expect("valid filter")
                .estimate(&observation())
                .expect("benchmark correction must succeed");
            let position = posterior.orbit().as_ref().position().to_metres();
            black_box(position[0] * 1.0e-6 + position[1] * 2.0e-6 + position[2] * 3.0e-6)
        })
        .sum()
}

fn propagator() -> EllipticKeplerPropagator {
    let gravity: SharedCentralGravity = Arc::new(EarthGravity);
    EllipticKeplerPropagator::new(TwoBodyDynamics::new(PointMassGravityModel::new(gravity)))
}

fn prior() -> CartesianStateEstimate {
    let state = CartesianState::new(
        ReferenceFrame::GCRF,
        Position::from_metres(6_999_600.0, 350.0, -250.0),
        VelocityVector::from_metres_per_second(0.0, 7_546.0, 0.0),
    )
    .expect("finite state");
    StateEstimate::new(
        Orbit::new(Epoch::from_tai_seconds(0.0), state),
        CartesianCovariance::from_standard_deviations(
            ReferenceFrame::GCRF,
            Position::from_metres(1_000.0, 1_000.0, 1_000.0),
            VelocityVector::from_metres_per_second(1_000.0, 1_000.0, 1_000.0),
        )
        .expect("positive covariance"),
    )
    .expect("matching frames")
}

fn process_noise() -> CartesianCovariance {
    CartesianCovariance::from_standard_deviations(
        ReferenceFrame::GCRF,
        Position::from_metres(1.0e-4, 1.0e-4, 1.0e-4),
        VelocityVector::from_metres_per_second(1.0e-4, 1.0e-4, 1.0e-4),
    )
    .expect("positive process noise")
}

fn observation() -> CartesianPositionObservation {
    CartesianPositionObservation::new(
        Epoch::from_tai_seconds(0.0),
        Position::from_metres(7_000_010.0, -10.0, 5.0),
        PositionCovariance::from_standard_deviations(
            ReferenceFrame::GCRF,
            Position::from_metres(5.0, 5.0, 5.0),
        )
        .expect("positive observation covariance"),
    )
    .expect("valid observation")
}
