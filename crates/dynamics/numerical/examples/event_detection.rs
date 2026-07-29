use std::sync::Arc;

use dynamics_numerical::{
    BogackiShampine32, EventAction, EventCallbackError, EventConfiguration, EventDetector,
    EventDirection, EventOccurrence, IntegrationConfiguration,
};
use dynamics_two_bodies::{PointMassGravityModel, TwoBodyDynamics};
use frames::{Body, FrameOrigin, ReferenceFrame};
use gravity::{PointMass, SharedCentralGravity};
use hifitime::{Duration, Epoch};
use orbits::cartesian::CartesianState;
use orskit_core::Orbit;
use units::uom::si::{length::meter, ratio::ratio, velocity::meter_per_second};
use units::{GravitationalParameter, Length, Position, Ratio, Velocity, VelocityVector};

#[derive(Debug)]
struct TargetEpoch {
    epoch: Epoch,
}

impl EventDetector for TargetEpoch {
    fn name(&self) -> &str {
        "demonstration epoch"
    }

    fn direction(&self) -> EventDirection {
        EventDirection::Rising
    }

    fn value(&self, state: &Orbit<CartesianState>) -> Result<Ratio, EventCallbackError> {
        Ok(Ratio::new::<ratio>(
            (state.epoch() - self.epoch).to_seconds(),
        ))
    }
}

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
        Duration::from_seconds(30.0),
        Duration::from_seconds(10.0),
        100_000,
        10_000,
    )?;
    let events = EventConfiguration::new(
        Duration::from_seconds(60.0),
        Duration::from_seconds(1.0e-6),
        64,
        16,
    )?;
    let propagator = BogackiShampine32::new(problem, integration);

    let epoch = Epoch::from_tai_seconds(1_000.0);
    let initial = Orbit::new(
        epoch,
        CartesianState::new(
            ReferenceFrame::GCRF,
            Position::from_metres(7_000_000.0, 0.0, 0.0),
            VelocityVector::from_metres_per_second(0.0, 7_500.0, 1_000.0),
        )?,
    );
    let detector = TargetEpoch {
        epoch: epoch + Duration::from_seconds(450.0),
    };
    let mut handler = |occurrence: &EventOccurrence| -> Result<EventAction, EventCallbackError> {
        println!(
            "event={} epoch={}",
            occurrence.detector_name(),
            occurrence.epoch()
        );
        Ok(EventAction::Stop)
    };
    let result = propagator.propagate_with_events(
        initial,
        epoch + Duration::from_seconds(900.0),
        &[&detector],
        events,
        &mut handler,
    )?;

    let midpoint = result
        .ephemeris()
        .state_at(epoch + Duration::from_seconds(225.0))?;
    println!("stopped={}", result.stopped());
    println!(
        "dense_midpoint_x_m={:.6}",
        midpoint.as_ref().position().to_metres()[0]
    );
    Ok(())
}
