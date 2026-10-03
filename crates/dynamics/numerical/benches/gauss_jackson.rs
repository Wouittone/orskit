//! Reproducible endpoint-only accuracy/timing comparison; no speed assertion.
use std::{
    hint::black_box,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Instant,
};

use dynamics::{CartesianDynamics, Propagator};
use dynamics_numerical::{
    BogackiShampine32, GaussJackson8, GaussJacksonConfiguration, IntegrationConfiguration,
};
use dynamics_two_bodies::{EllipticKeplerPropagator, PointMassGravityModel, TwoBodyDynamics};
use frames::{Body, FrameOrigin, InertialFrame};
use gravity::{PointMass, SharedCentralGravity};
use hifitime::{Duration, Epoch};
use orbits::{
    cartesian::{CartesianState, FramedAcceleration},
    keplerian::KeplerianState,
};
use orskit_core::Orbit;
use units::uom::si::{angle::radian, length::meter, ratio::ratio, velocity::meter_per_second};
use units::{Angle, GravitationalParameter, Length, Ratio, Velocity};

#[derive(Debug)]
struct Counted<P> {
    problem: P,
    calls: Arc<AtomicUsize>,
}

impl<P: CartesianDynamics> CartesianDynamics for Counted<P> {
    type Error = P::Error;
    fn validate(&self, state: &CartesianState) -> Result<(), Self::Error> {
        self.problem.validate(state)
    }
    fn acceleration(
        &self,
        epoch: Epoch,
        state: &CartesianState,
    ) -> Result<FramedAcceleration, Self::Error> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.problem.acceleration(epoch, state)
    }
}

fn configuration(position_m: f64, velocity_m_s: f64) -> IntegrationConfiguration {
    IntegrationConfiguration::new(
        Length::new::<meter>(position_m),
        Velocity::new::<meter_per_second>(velocity_m_s),
        Ratio::new::<ratio>(1.0e-15),
        Duration::from_seconds(1.0e-8),
        Duration::from_seconds(30.0),
        Duration::from_seconds(0.1),
        5_000_000,
        100_000,
    )
    .unwrap()
}

#[allow(clippy::too_many_arguments)]
fn measure<S: Propagator<CartesianState>>(
    solver: &S,
    initial: CartesianState,
    exact: CartesianState,
    seconds: f64,
    calls: &AtomicUsize,
    label: &str,
    round: usize,
    sample: usize,
) {
    let epoch = Epoch::from_tai_seconds(0.0);
    calls.store(0, Ordering::Relaxed);
    let start = Instant::now();
    let actual = black_box(
        solver
            .propagate(
                Orbit::new(epoch, initial),
                epoch + Duration::from_seconds(seconds),
            )
            .unwrap(),
    );
    let elapsed = start.elapsed().as_secs_f64();
    let position_error = norm_difference(
        actual.as_ref().position().to_metres(),
        exact.position().to_metres(),
    );
    let velocity_error = norm_difference(
        actual.as_ref().velocity().to_metres_per_second(),
        exact.velocity().to_metres_per_second(),
    );
    println!("{label},{round},{sample},{seconds:.0},{elapsed:.9},{},{position_error:.9e},{velocity_error:.9e}",
        calls.load(Ordering::Relaxed));
}

fn norm_difference(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.into_iter()
        .zip(b)
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f64>()
        .sqrt()
}

fn main() {
    // A harness-free benchmark has no libtest cases. Nextest probes all
    // targets with --list; discovery must not execute the timed workload.
    if std::env::args().any(|argument| argument == "--list") {
        return;
    }
    println!("solver_scenario,round,sample,arc_s,elapsed_s,rhs_calls,position_error_m,velocity_error_m_s");
    for (name, period_minutes, eccentricity, step) in [
        ("reduced_iss", 92.05, 0.001, 30.0),
        ("reduced_crres", 607.28, 0.716, 15.0),
    ] {
        let gravity: SharedCentralGravity = Arc::new(PointMass::new(
            FrameOrigin::Body(Body::EARTH),
            GravitationalParameter::try_from(3.986_004_418e14).unwrap(),
        ));
        let a = (3.986_004_418e14 * (period_minutes * 60.0 / std::f64::consts::TAU).powi(2)).cbrt();
        let initial: CartesianState = KeplerianState::new(
            InertialFrame::GCRF,
            Arc::clone(&gravity),
            Length::new::<meter>(a),
            Ratio::new::<ratio>(eccentricity),
            Angle::new::<radian>(0.7),
            Angle::new::<radian>(1.1),
            Angle::new::<radian>(0.4),
            Angle::new::<radian>(0.0),
        )
        .unwrap()
        .try_into()
        .unwrap();
        let problem = TwoBodyDynamics::new(PointMassGravityModel::new(gravity));
        let reference = EllipticKeplerPropagator::new(problem.clone());
        let calls = Arc::new(AtomicUsize::new(0));
        let gj_settings = GaussJacksonConfiguration::new(
            Duration::from_seconds(step),
            configuration(1.0e-7, 1.0e-10),
            Length::new::<meter>(1.0e-8),
            Velocity::new::<meter_per_second>(1.0e-11),
            20,
            1_000_000,
        )
        .unwrap();
        let start = Instant::now();
        let gj = GaussJackson8::new(
            Counted {
                problem: problem.clone(),
                calls: Arc::clone(&calls),
            },
            gj_settings,
        );
        eprintln!(
            "{name}: construction_s={:.9},solver_inline_bytes={}",
            start.elapsed().as_secs_f64(),
            std::mem::size_of_val(&gj)
        );
        let native = BogackiShampine32::new(
            Counted {
                problem,
                calls: Arc::clone(&calls),
            },
            configuration(1.0e-7, 1.0e-10),
        );
        for round in 1..=3 {
            for sample in 1..=3 {
                for seconds in [8.0 * step, 259_200.0] {
                    let epoch = Epoch::from_tai_seconds(0.0);
                    let exact = *reference
                        .propagate(
                            Orbit::new(epoch, initial),
                            epoch + Duration::from_seconds(seconds),
                        )
                        .unwrap()
                        .as_ref();
                    measure(
                        &gj,
                        initial,
                        exact,
                        seconds,
                        &calls,
                        &format!("gj8_{name}"),
                        round,
                        sample,
                    );
                    if seconds == 259_200.0 {
                        measure(
                            &native,
                            initial,
                            exact,
                            seconds,
                            &calls,
                            &format!("bs32_{name}"),
                            round,
                            sample,
                        );
                    }
                }
            }
        }
    }
}
