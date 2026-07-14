use std::{hint::black_box, sync::Arc, time::Instant};

use bodies::Body;
use core_crate::frames::{FrameOrigin, ReferenceFrame};
use core_crate::{Orbit, SharedCentralGravity, SharedScientificSource};
use dynamics::Propagator;
use dynamics_two_body::{EllipticKeplerPropagator, PointMassGravityModel, TwoBodyDynamics};
use gravity_point_mass::{PointMassGravity, ReferenceSource};
use hifitime::{Duration, Epoch};
use orbits_cartesian::{CartesianState, SpacecraftState};
use units::{GravitationalParameter, Position, VelocityVector};

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

    let initial = initial_orbit();
    let source: SharedScientificSource = Arc::new(
        ReferenceSource::new(
            "International Earth Rotation and Reference Systems Service",
            "IERS Conventions",
            "2010",
            "IERS Technical Note 36",
        )
        .expect("complete benchmark source"),
    );
    let gravity: SharedCentralGravity = Arc::new(
        PointMassGravity::new(
            FrameOrigin::Body(Body::EARTH),
            GravitationalParameter::from_cubic_metres_per_second_squared(3.986_004_418e14)
                .expect("positive gravitational parameter"),
            source,
        )
        .expect("complete benchmark source"),
    );
    let problem = TwoBodyDynamics::new(PointMassGravityModel::new(gravity));
    let propagator = EllipticKeplerPropagator::new();

    let _warmup_checksum = run_queries(
        black_box(&propagator),
        black_box(initial.clone()),
        black_box(&problem),
        WARMUP_ITERATIONS,
    );

    let started = Instant::now();
    let checksum = run_queries(
        black_box(&propagator),
        black_box(initial),
        black_box(&problem),
        iterations,
    );
    let elapsed_ns = started.elapsed().as_nanos();

    println!(
        "implementation=orskit iterations={iterations} elapsed_ns={elapsed_ns} checksum={checksum:.17e}"
    );
}

fn run_queries(
    propagator: &EllipticKeplerPropagator,
    initial: Orbit<SpacecraftState>,
    problem: &TwoBodyDynamics,
    iterations: usize,
) -> f64 {
    let mut checksum = 0.0;
    for index in 0..iterations {
        let elapsed_seconds = query_offset_seconds(index);
        let state = propagator
            .propagate(
                black_box(initial.clone()),
                black_box(problem),
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

fn initial_orbit() -> Orbit<SpacecraftState> {
    let state = CartesianState::new(
        ReferenceFrame::GCRF,
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
    Orbit::new(Epoch::from_tai_seconds(0.0), state.into())
}
