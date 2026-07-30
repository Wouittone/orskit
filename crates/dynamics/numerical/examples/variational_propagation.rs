use std::sync::Arc;

use dynamics_numerical::{BogackiShampine32, IntegrationConfiguration, VariationalConfiguration};
use dynamics_two_bodies::{PointMassGravityModel, TwoBodyDynamics};
use frames::{Body, FrameOrigin, ReferenceFrame};
use gravity::{PointMass, SharedCentralGravity};
use hifitime::{Duration, Epoch};
use orbits::cartesian::{CartesianCovariance, CartesianState};
use orskit_core::Orbit;
use units::uom::si::{
    area::square_meter, length::meter, ratio::ratio, time::second, velocity::meter_per_second,
};
use units::{
    GravitationalParameter, InverseTime, Length, Position, Ratio, Time, Velocity, VelocityVector,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let earth_gravity: SharedCentralGravity = Arc::new(PointMass::new(
        FrameOrigin::Body(Body::EARTH),
        GravitationalParameter::try_from(3.986_004_418e14)?,
    ));
    let problem = TwoBodyDynamics::new(PointMassGravityModel::new(earth_gravity));
    let integration = IntegrationConfiguration::new(
        Length::new::<meter>(1.0e-3),
        Velocity::new::<meter_per_second>(1.0e-6),
        Ratio::new::<ratio>(1.0e-10),
        Duration::from_seconds(1.0e-6),
        Duration::from_seconds(10.0),
        Duration::from_seconds(1.0),
        100_000,
        10_000,
    )?;
    let variational = VariationalConfiguration::new(
        Ratio::new::<ratio>(1.0e-9),
        Time::new::<second>(1.0e-7),
        InverseTime::from_per_second(1.0e-12),
    )?;
    let propagator = BogackiShampine32::new(problem, integration);

    let epoch = Epoch::from_tai_seconds(1_000.0);
    let initial = Orbit::new(
        epoch,
        CartesianState::new(
            ReferenceFrame::GCRF,
            Position::from_metres(7_000_000.0, 0.0, 0.0),
            VelocityVector::from_metres_per_second(0.0, 7_500.0, 0.0),
        )?,
    );
    let covariance = CartesianCovariance::from_standard_deviations(
        ReferenceFrame::GCRF,
        Position::from_metres(100.0, 100.0, 100.0),
        VelocityVector::from_metres_per_second(0.1, 0.1, 0.1),
    )?;
    let result = propagator.propagate_with_covariance(
        initial,
        &covariance,
        epoch + Duration::from_seconds(300.0),
        variational,
    )?;

    println!("final_epoch={}", result.final_orbit().epoch());
    println!(
        "phi_rv_xx_s={:.9}",
        result.state_transition().position_velocity()[0][0].get::<second>()
    );
    println!(
        "position_sigma_x_m={:.6}",
        result.covariance().position_position()[0][0]
            .get::<square_meter>()
            .sqrt()
    );
    Ok(())
}
