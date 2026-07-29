use std::sync::Arc;

use dynamics::{Propagator, SystemDynamics};
use dynamics_numerical::{BogackiShampine32, IntegrationConfiguration};
use dynamics_two_bodies::{PointMassGravityModel, TwoBodyDynamics};
use frames::{Body, FrameOrigin, ReferenceFrame};
use gravity::{PointMass, SharedCentralGravity};
use hifitime::{Duration, Epoch};
use orbits::cartesian::CartesianState;
use orskit_core::Orbit;
use units::uom::si::{length::meter, ratio::ratio, velocity::meter_per_second};
use units::{GravitationalParameter, Length, Position, Ratio, Velocity, VelocityVector};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let earth_gravity: SharedCentralGravity = Arc::new(PointMass::new(
        FrameOrigin::Body(Body::EARTH),
        GravitationalParameter::try_from(3.986_004_418e14)?,
    ));
    let problem = TwoBodyDynamics::new(PointMassGravityModel::new(earth_gravity));
    let configuration = IntegrationConfiguration::new(
        Length::new::<meter>(1.0e-3),
        Velocity::new::<meter_per_second>(1.0e-6),
        Ratio::new::<ratio>(1.0e-10),
        Duration::from_seconds(1.0e-6),
        Duration::from_seconds(30.0),
        Duration::from_seconds(10.0),
        100_000,
        10_000,
    )?;
    let propagator = BogackiShampine32::new(problem, configuration);

    let epoch = Epoch::from_tai_seconds(1_000.0);
    let initial = Orbit::new(
        epoch,
        CartesianState::new(
            ReferenceFrame::GCRF,
            Position::from_metres(7_000_000.0, 0.0, 0.0),
            VelocityVector::from_metres_per_second(0.0, 7_500.0, 1_000.0),
        )?,
    );
    let target = epoch + Duration::from_seconds(900.0);
    let propagated = propagator.propagate(initial, target)?;
    let [x, y, z] = propagated.as_ref().position().to_metres();
    let [vx, vy, vz] = propagated.as_ref().velocity().to_metres_per_second();

    println!("model={}", propagator.problem().name());
    println!("epoch={}", propagated.epoch());
    println!("position_m={x:.6},{y:.6},{z:.6}");
    println!("velocity_m_s={vx:.9},{vy:.9},{vz:.9}");
    Ok(())
}
