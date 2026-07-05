// SPDX-License-Identifier: AGPL-3.0-or-later

use std::{hint::black_box, time::Instant};

use anise::frames::Frame;
use nyx_space::{cosmic::Orbit, time::Epoch};

const DEFAULT_ITERATIONS: usize = 1_000_000;
const WARMUP_ITERATIONS: usize = 10_000;
const EARTH_NAIF_ID: i32 = 399;
const EARTH_MU_KM3_S2: f64 = 398_600.441_8;

fn main() {
    let argument = std::env::args().nth(1);
    if argument.as_deref() == Some("--reference") {
        print_reference();
        return;
    }
    let iterations = argument
        .map(|value| {
            value
                .parse()
                .expect("iterations must be a positive integer")
        })
        .unwrap_or(DEFAULT_ITERATIONS);
    assert!(iterations > 0, "iterations must be positive");

    let initial = initial_orbit();
    let epoch = initial.epoch;

    let _warmup_checksum = run_queries(black_box(&initial), epoch, WARMUP_ITERATIONS);
    let started = Instant::now();
    let checksum = run_queries(black_box(&initial), epoch, iterations);
    let elapsed_ns = started.elapsed().as_nanos();

    println!(
        "implementation=nyx iterations={iterations} elapsed_ns={elapsed_ns} checksum={checksum:.17e}"
    );
}

fn initial_orbit() -> Orbit {
    let frame = Frame::from_ephem_j2000(EARTH_NAIF_ID).with_mu_km3_s2(EARTH_MU_KM3_S2);
    Orbit::cartesian(
        -6_547.737_711_811_969,
        1_403.357_008_528_989,
        3_236.397_558_481_829,
        -3.483_367_356_322_263,
        -5.479_766_927_646_723,
        -3.108_644_196_877_947_3,
        Epoch::from_tai_seconds(0.0),
        frame,
    )
}

fn print_reference() {
    let initial = initial_orbit();
    let propagated = initial
        .at_epoch(initial.epoch + nyx_space::time::Duration::from_seconds(3_600.0))
        .expect("reference query must propagate");
    println!("mu_m3_s2={:.17e}", EARTH_MU_KM3_S2 * 1.0e9);
    println!(
        "position_m={:.17e},{:.17e},{:.17e}",
        propagated.radius_km.x * 1.0e3,
        propagated.radius_km.y * 1.0e3,
        propagated.radius_km.z * 1.0e3
    );
    println!(
        "velocity_m_s={:.17e},{:.17e},{:.17e}",
        propagated.velocity_km_s.x * 1.0e3,
        propagated.velocity_km_s.y * 1.0e3,
        propagated.velocity_km_s.z * 1.0e3
    );
}

fn run_queries(initial: &Orbit, epoch: Epoch, iterations: usize) -> f64 {
    let mut checksum = 0.0;
    for index in 0..iterations {
        let target = epoch + nyx_space::time::Duration::from_seconds(query_offset_seconds(index));
        let state = initial
            .at_epoch(black_box(target))
            .expect("benchmark query must propagate");
        checksum += state.radius_km.x * 1.0e-3 + state.radius_km.z * 2.0e-3 + state.velocity_km_s.y;
    }
    black_box(checksum)
}

fn query_offset_seconds(index: usize) -> f64 {
    ((index.wrapping_mul(104_729) % 172_800) as f64) - 86_400.0
}
