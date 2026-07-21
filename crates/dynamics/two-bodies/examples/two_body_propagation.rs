//! Minimal, physically explicit elliptic two-body propagation.

use std::{error::Error, sync::Arc};

use bodies::Body;
use dynamics::Propagator;
use dynamics_two_bodies::{EllipticKeplerPropagator, PointMassGravityModel, TwoBodyDynamics};
use frames::{FrameOrigin, ReferenceFrame};
use gravity::{PointMass, SharedCentralGravity};
use hifitime::{Duration, Epoch};
use orbits::cartesian::CartesianState;
use orskit_core::Orbit;
use units::{GravitationalParameter, Position, VelocityVector};

fn main() -> Result<(), Box<dyn Error>> {
    // Conventional Earth GM from IERS Conventions (2010), Table 1.1, converted
    // from 398_600.4418 km^3/s^2 to SI. The application selects this source.
    let earth_mu = GravitationalParameter::try_from(3.986_004_418e14)?;
    let gravity: SharedCentralGravity =
        Arc::new(PointMass::new(FrameOrigin::Body(Body::EARTH), earth_mu));
    let dynamics = TwoBodyDynamics::new(PointMassGravityModel::new(gravity));
    let propagator = EllipticKeplerPropagator::new(dynamics);

    let state = CartesianState::new(
        ReferenceFrame::GCRF,
        Position::from_metres(7_000_000.0, 0.0, 0.0),
        VelocityVector::from_metres_per_second(0.0, 7_500.0, 1_000.0),
    )?;
    // Epoch is exactly zero seconds on Hifitime's TAI timeline. Propagation
    // uses a uniform TAI duration; no implicit UTC conversion is performed.
    let initial = Orbit::new(Epoch::from_tai_seconds(0.0), state);
    let target = initial.epoch() + Duration::from_seconds(900.0);

    let propagated = propagator.propagate(initial, target)?;
    println!("epoch: {}", propagated.epoch());
    println!(
        "position (m): {:?}",
        propagated.as_ref().position().to_metres()
    );
    println!(
        "velocity (m/s): {:?}",
        propagated.as_ref().velocity().to_metres_per_second()
    );
    Ok(())
}
