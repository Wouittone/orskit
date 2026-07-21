//! One synthetic Cartesian position update with the current EKF boundary.

use std::{error::Error, sync::Arc};

use bodies::Body;
use dynamics::{EllipticKeplerPropagator, PointMassGravityModel, Propagator, TwoBodyDynamics};
use frames::{FrameOrigin, ReferenceFrame};
use gravity::{CentralGravityProvider, SharedCentralGravity};
use hifitime::{Duration, Epoch};
use orbit_determination::{
    CartesianCovariance, CartesianPositionObservation, CartesianStateEstimate,
    ExtendedKalmanFilter, Orbit, OrbitDetermination, PositionCovariance, StateEstimate,
};
use orbits::cartesian::CartesianState;
use units::{GravitationalParameter, Position, VelocityVector};

#[derive(Debug)]
struct TutorialEarthGravity;

impl CentralGravityProvider for TutorialEarthGravity {
    fn origin(&self) -> FrameOrigin {
        FrameOrigin::Body(Body::EARTH)
    }

    fn parameter(&self) -> GravitationalParameter {
        GravitationalParameter::try_from(3.986_004_418e14)
            .expect("the documented Earth parameter is positive")
    }
}

fn propagator() -> EllipticKeplerPropagator {
    let gravity: SharedCentralGravity = Arc::new(TutorialEarthGravity);
    EllipticKeplerPropagator::new(TwoBodyDynamics::new(PointMassGravityModel::new(gravity)))
}

fn main() -> Result<(), Box<dyn Error>> {
    let epoch = Epoch::from_tai_seconds(0.0);
    let truth = Orbit::new(
        epoch,
        CartesianState::new(
            ReferenceFrame::GCRF,
            Position::from_metres(7_000_000.0, 0.0, 0.0),
            VelocityVector::from_metres_per_second(0.0, 7_500.0, 1_000.0),
        )?,
    );
    let observation_epoch = epoch + Duration::from_seconds(60.0);
    let truth_at_observation = propagator().propagate(truth, observation_epoch)?;
    let truth_position = truth_at_observation.as_ref().position();

    let prior_state = CartesianState::new(
        ReferenceFrame::GCRF,
        Position::from_metres(7_000_400.0, -300.0, 200.0),
        VelocityVector::from_metres_per_second(0.2, 7_499.8, 1_000.1),
    )?;
    let prior: CartesianStateEstimate = StateEstimate::new(
        Orbit::new(epoch, prior_state),
        CartesianCovariance::from_standard_deviations(
            ReferenceFrame::GCRF,
            Position::from_metres(1_000.0, 1_000.0, 1_000.0),
            VelocityVector::from_metres_per_second(10.0, 10.0, 10.0),
        )?,
    )?;
    let process_noise = CartesianCovariance::from_standard_deviations(
        ReferenceFrame::GCRF,
        Position::from_metres(0.1, 0.1, 0.1),
        VelocityVector::from_metres_per_second(0.01, 0.01, 0.01),
    )?;
    let observation = CartesianPositionObservation::new(
        observation_epoch,
        truth_position + Position::from_metres(3.0, -2.0, 1.0),
        PositionCovariance::from_standard_deviations(
            ReferenceFrame::GCRF,
            Position::from_metres(5.0, 5.0, 5.0),
        )?,
    )?;

    let mut filter = ExtendedKalmanFilter::new(propagator(), prior, process_noise)?;
    let posterior = filter.estimate(&observation)?;
    println!("posterior epoch: {}", posterior.orbit().epoch());
    println!(
        "posterior position (m): {:?}",
        posterior.orbit().as_ref().position().to_metres()
    );
    Ok(())
}
