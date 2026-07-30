use std::convert::Infallible;

use attitude::FixedAttitudeProvider;
use dynamics_numerical::{
    BogackiShampine32, CartesianDynamics, CartesianMassState, ConstantThrustManeuver,
    IntegrationConfiguration, ManeuverSchedule, ThrustVector,
};
use frames::{CustomFrameId, FrameMotion, FrameOrientation, FrameOrigin, ReferenceFrame};
use hifitime::{Duration, Epoch};
use orbits::cartesian::{CartesianState, FramedAcceleration};
use orskit_core::{
    BodyAngularVelocity, Orbit, Orientation, OrientationQuaternion, QuaternionAttitude,
    SpacecraftBodyFrame,
};
use units::uom::si::{
    length::meter, mass::kilogram, mass_rate::kilogram_per_second, ratio::ratio,
    velocity::meter_per_second,
};
use units::{
    AccelerationVector, AngularVelocityVector, Length, Mass, MassRate, Position, Ratio, Velocity,
    VelocityVector,
};

#[derive(Debug, Clone, Copy)]
struct InertialMotion;

impl CartesianDynamics for InertialMotion {
    type Error = Infallible;

    fn validate(&self, _state: &CartesianState) -> Result<(), Self::Error> {
        Ok(())
    }

    fn acceleration(
        &self,
        _epoch: Epoch,
        state: &CartesianState,
    ) -> Result<FramedAcceleration, Self::Error> {
        Ok(FramedAcceleration::new(
            AccelerationVector::from_metres_per_second_squared(0.0, 0.0, 0.0),
            state.frame(),
        )
        .expect("zero acceleration is finite"))
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let id = CustomFrameId::new(42);
    let body = SpacecraftBodyFrame::new(
        "poc-spacecraft".to_owned(),
        ReferenceFrame::new(
            FrameOrigin::Custom(id),
            FrameOrientation::custom(id, FrameMotion::NonInertial),
        ),
    )?;
    let orientation = Orientation::try_from(OrientationQuaternion {
        source_frame: body.reference_frame(),
        target_frame: ReferenceFrame::GCRF,
        components: [
            Ratio::new::<ratio>(std::f64::consts::FRAC_1_SQRT_2),
            Ratio::new::<ratio>(0.0),
            Ratio::new::<ratio>(0.0),
            Ratio::new::<ratio>(std::f64::consts::FRAC_1_SQRT_2),
        ],
    })?;
    let rate = BodyAngularVelocity::new(
        AngularVelocityVector::from_radians_per_second(0.0, 0.0, 0.0),
        body.clone(),
        ReferenceFrame::GCRF,
    )?;
    let provider = FixedAttitudeProvider::new(QuaternionAttitude::new(orientation, rate)?)?;

    let integration = IntegrationConfiguration::new(
        Length::new::<meter>(1.0e-6),
        Velocity::new::<meter_per_second>(1.0e-9),
        Ratio::new::<ratio>(1.0e-11),
        Duration::from_seconds(1.0e-6),
        Duration::from_seconds(2.0),
        Duration::from_seconds(0.25),
        10_000,
        1_000,
    )?;
    let propagator = BogackiShampine32::new(InertialMotion, integration);
    let epoch = Epoch::from_tai_seconds(1_000.0);
    let initial = CartesianMassState::new(
        Orbit::new(
            epoch,
            CartesianState::new(
                ReferenceFrame::GCRF,
                Position::from_metres(0.0, 0.0, 0.0),
                VelocityVector::from_metres_per_second(0.0, 0.0, 0.0),
            )?,
        ),
        Mass::new::<kilogram>(100.0),
    )?;
    let burn = ConstantThrustManeuver::body_fixed(
        "body +x burn",
        epoch,
        epoch + Duration::from_seconds(10.0),
        body,
        ThrustVector::from_newtons(100.0, 0.0, 0.0),
        MassRate::new::<kilogram_per_second>(1.0),
    )?;
    let schedule = ManeuverSchedule::new(vec![], vec![burn])?;
    let result = propagator.propagate_with_attitude_maneuvers(
        initial,
        epoch + Duration::from_seconds(10.0),
        &schedule,
        &provider,
    )?;
    let [vx, vy, vz] = result
        .final_state()
        .orbit()
        .as_ref()
        .velocity()
        .to_metres_per_second();

    println!("final_velocity_m_s={vx:.9},{vy:.9},{vz:.9}");
    println!(
        "final_mass_kg={:.6}",
        result.final_state().mass().get::<kilogram>()
    );
    Ok(())
}
