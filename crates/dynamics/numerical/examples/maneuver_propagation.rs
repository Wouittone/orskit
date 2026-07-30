use std::sync::Arc;

use dynamics_numerical::{
    BogackiShampine32, CartesianMassState, ConstantThrustManeuver, ImpulsiveManeuver,
    IntegrationConfiguration, ManeuverSchedule, ThrustVector,
};
use dynamics_two_bodies::{PointMassGravityModel, TwoBodyDynamics};
use frames::{Body, FrameOrigin, ReferenceFrame};
use gravity::{PointMass, SharedCentralGravity};
use hifitime::{Duration, Epoch};
use orbits::cartesian::CartesianState;
use orskit_core::Orbit;
use units::uom::si::{
    length::meter, mass::kilogram, mass_rate::kilogram_per_second, ratio::ratio,
    velocity::meter_per_second,
};
use units::{
    GravitationalParameter, Length, Mass, MassRate, Position, Ratio, Velocity, VelocityVector,
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
    let propagator = BogackiShampine32::new(problem, integration);

    let epoch = Epoch::from_tai_seconds(1_000.0);
    let initial = CartesianMassState::new(
        Orbit::new(
            epoch,
            CartesianState::new(
                ReferenceFrame::GCRF,
                Position::from_metres(7_000_000.0, 0.0, 0.0),
                VelocityVector::from_metres_per_second(0.0, 7_500.0, 0.0),
            )?,
        ),
        Mass::new::<kilogram>(500.0),
    )?;
    let finite = ConstantThrustManeuver::new(
        "orbit-raise burn",
        epoch + Duration::from_seconds(60.0),
        epoch + Duration::from_seconds(120.0),
        ReferenceFrame::GCRF,
        ThrustVector::from_newtons(0.0, 20.0, 0.0),
        MassRate::new::<kilogram_per_second>(0.02),
    )?;
    let trim = ImpulsiveManeuver::new(
        "trim impulse",
        epoch + Duration::from_seconds(180.0),
        ReferenceFrame::GCRF,
        VelocityVector::from_metres_per_second(0.0, 0.25, 0.0),
        Mass::new::<kilogram>(0.05),
    )?;
    let schedule = ManeuverSchedule::new(vec![trim], vec![finite])?;
    let result = propagator.propagate_with_maneuvers(
        initial,
        epoch + Duration::from_seconds(300.0),
        &schedule,
    )?;

    let final_state = result.final_state();
    let [vx, vy, vz] = final_state
        .orbit()
        .as_ref()
        .velocity()
        .to_metres_per_second();
    println!("final_epoch={}", final_state.orbit().epoch());
    println!("final_mass_kg={:.6}", final_state.mass().get::<kilogram>());
    println!("final_velocity_m_s={vx:.6},{vy:.6},{vz:.6}");
    for execution in result.executions() {
        println!(
            "execution={} kind={:?} start={} end={}",
            execution.name(),
            execution.kind(),
            execution.start(),
            execution.end()
        );
    }
    Ok(())
}
