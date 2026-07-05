use std::{hint::black_box, time::Instant};

use hifitime::{Duration, Epoch};
use orskit_bodies::Body;
use orskit_core::frames::{CustomFrameId, FrameOrientation, FrameOrigin, ReferenceFrame};
use orskit_core::{
    AttitudeState, CartesianState, FramedAngularVelocity, InertiaTensor, Orientation, Spacecraft,
    SpacecraftShape, SpacecraftState, SpacecraftView,
};
use orskit_dynamics::{EllipticTwoBodyPropagator, PointMassGravityModel, Propagator};
use orskit_units::uom::si::{mass::kilogram, moment_of_inertia::kilogram_square_meter};
use orskit_units::{
    AngularVelocityVector, GravitationalParameter, Mass, MomentOfInertia, Position, VelocityVector,
};

const DEFAULT_ITERATIONS: usize = 1_000_000;
const WARMUP_ITERATIONS: usize = 10_000;

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

    let initial = InitialCondition::new();
    let initial_view = initial.view();
    let gravity = PointMassGravityModel::new(
        Body::EARTH,
        GravitationalParameter::from_cubic_metres_per_second_squared(3.986_004_418e14)
            .expect("positive gravitational parameter"),
    );
    let propagator = EllipticTwoBodyPropagator::new();

    let _warmup_checksum = run_queries(
        black_box(&propagator),
        black_box(&initial_view),
        black_box(&gravity),
        WARMUP_ITERATIONS,
    );

    let started = Instant::now();
    let checksum = run_queries(
        black_box(&propagator),
        black_box(&initial_view),
        black_box(&gravity),
        iterations,
    );
    let elapsed_ns = started.elapsed().as_nanos();

    println!(
        "implementation=orskit iterations={iterations} elapsed_ns={elapsed_ns} checksum={checksum:.17e}"
    );
}

fn run_queries(
    propagator: &EllipticTwoBodyPropagator,
    initial: &SpacecraftView<'_>,
    gravity: &PointMassGravityModel,
    iterations: usize,
) -> f64 {
    let mut checksum = 0.0;
    for index in 0..iterations {
        let elapsed_seconds = query_offset_seconds(index);
        let state = propagator
            .propagate(
                black_box(initial),
                black_box(gravity),
                Duration::from_seconds(black_box(elapsed_seconds)),
            )
            .expect("benchmark query must remain in the supported elliptic regime");
        let cartesian = match state.state() {
            SpacecraftState::Cartesian(state) => state,
            _ => unreachable!("the propagator preserves the Cartesian variant"),
        };
        let position = cartesian.position().to_metres();
        let velocity = cartesian.velocity().to_metres_per_second();
        checksum += position[0] * 1.0e-6 + position[2] * 2.0e-6 + velocity[1] * 1.0e-3;
    }
    black_box(checksum)
}

fn query_offset_seconds(index: usize) -> f64 {
    ((index.wrapping_mul(104_729) % 172_800) as f64) - 86_400.0
}

struct InitialCondition {
    spacecraft: Spacecraft,
    state: CartesianState,
    inertia: InertiaTensor,
    attitude: AttitudeState,
}

impl InitialCondition {
    fn new() -> Self {
        let frame = ReferenceFrame::GCRF;
        let state = CartesianState::new(
            frame,
            Position::from_metres(
                -6_547_737.711_811_969,
                1_403_357.008_528_988_8,
                3_236_397.558_481_829,
            ),
            VelocityVector::from_metres_per_second(
                -3_483.367_356_322_263,
                -5_479.766_927_646_723,
                -3_108.644_196_877_947_3,
            ),
        )
        .expect("finite benchmark state");

        let id = CustomFrameId::new(91);
        let body = ReferenceFrame::new(FrameOrigin::Custom(id), FrameOrientation::Custom(id));
        let attitude = AttitudeState::new(
            Orientation::identity(body, ReferenceFrame::GCRF),
            FramedAngularVelocity::new(
                AngularVelocityVector::from_radians_per_second(0.0, 0.0, 0.0),
                body,
            )
            .expect("finite angular velocity"),
        )
        .expect("consistent attitude");
        let inertia = InertiaTensor::principal(
            body,
            MomentOfInertia::new::<kilogram_square_meter>(800.0),
            MomentOfInertia::new::<kilogram_square_meter>(900.0),
            MomentOfInertia::new::<kilogram_square_meter>(1_000.0),
        )
        .expect("physical benchmark inertia");
        Self {
            spacecraft: Spacecraft::new("BENCHMARK-SC", SpacecraftShape::Point)
                .expect("valid spacecraft"),
            state,
            inertia,
            attitude,
        }
    }

    fn view(&self) -> SpacecraftView<'_> {
        SpacecraftView::new(
            &self.spacecraft,
            Epoch::from_tai_seconds(0.0),
            Mass::new::<kilogram>(500.0),
            self.state.into(),
            self.inertia,
            self.attitude.clone(),
        )
        .expect("physical benchmark spacecraft view")
    }
}
