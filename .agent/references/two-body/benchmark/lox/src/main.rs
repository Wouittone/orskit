use std::{hint::black_box, time::Instant};

use lox_space::prelude::*;

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

    let epoch = Time::<Tai>::default();
    let initial = Orbit::<Cartesian, Tai, Earth, Icrf>::new(
        Cartesian::new(
            Distance::new(-6_547_737.711_811_969),
            Distance::new(1_403_357.008_528_988_8),
            Distance::new(3_236_397.558_481_829),
            Velocity::new(-3_483.367_356_322_263),
            Velocity::new(-5_479.766_927_646_723),
            Velocity::new(-3_108.644_196_877_947_3),
        ),
        epoch,
        Earth,
        Icrf,
    );
    let propagator = Vallado::new(initial);

    let _warmup_checksum = run_queries(black_box(&propagator), epoch, WARMUP_ITERATIONS);
    let started = Instant::now();
    let checksum = run_queries(black_box(&propagator), epoch, iterations);
    let elapsed_ns = started.elapsed().as_nanos();

    println!(
        "implementation=lox iterations={iterations} elapsed_ns={elapsed_ns} checksum={checksum:.17e}"
    );
}

fn run_queries(propagator: &Vallado<Tai, Earth, Icrf>, epoch: Time<Tai>, iterations: usize) -> f64 {
    let mut checksum = 0.0;
    for index in 0..iterations {
        let elapsed_seconds = query_offset_seconds(index);
        let target = epoch + TimeDelta::from_seconds(black_box(elapsed_seconds));
        let state = propagator
            .state_at(black_box(target))
            .expect("benchmark query must propagate");
        let position = state.position();
        let velocity = state.velocity();
        checksum += position.x * 1.0e-6 + position.z * 2.0e-6 + velocity.y * 1.0e-3;
    }
    black_box(checksum)
}

fn query_offset_seconds(index: usize) -> i64 {
    (index.wrapping_mul(104_729) % 172_800) as i64 - 86_400
}
