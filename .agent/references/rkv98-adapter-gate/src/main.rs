//! Isolated allocation and performance gate for the released reusable Vern9 API.
#![forbid(unsafe_code)]

use std::hint::black_box;
use std::time::Instant;

use numeris::ode::{AdaptiveSettings, RKAdaptive, RKV98};
use numeris::Vector;
use stats_alloc::{Region, StatsAlloc, INSTRUMENTED_SYSTEM};

#[global_allocator]
static GLOBAL: &StatsAlloc<std::alloc::System> = &INSTRUMENTED_SYSTEM;

const MU_M3_S2: f64 = 398_600_441_800_000.0;
const EARTH_RADIUS_M: f64 = 6_371_000.0;
const ALTITUDE_M: f64 = 500_000.0;
const EARTH_ROTATION_RAD_S: f64 = 7.292_115_0e-5;
const DRAG_RATE_S_INV: f64 = 2.0e-9;
const DURATION_S: f64 = 6.0 * 3_600.0;
const BASELINE_TOLERANCE: f64 = 1.0e-9;
#[cfg(feature = "adapter")]
const VERN9_TOLERANCE: f64 = 2.0e-10;
#[cfg(feature = "adapter")]
const DENSE_VERN9_TOLERANCE: f64 = 2.15e-10;
const REFERENCE_TOLERANCE: f64 = 1.0e-13;
#[cfg(feature = "adapter")]
const DENSE_QUERY_INTERVAL_S: f64 = 30.0;
const MAX_POSITION_ERROR_M: f64 = 2.1e-3;
const MAX_VELOCITY_ERROR_M_S: f64 = 2.1e-6;
#[cfg(feature = "adapter")]
const MAX_ERROR_DIFFERENCE_FRACTION: f64 = 0.25;
type State = [f64; 6];

#[derive(Clone, Copy)]
struct Scenario {
    name: &'static str,
    drag_rate_s_inv: f64,
}

const TWO_BODY: Scenario = Scenario {
    name: "two-body-leo",
    drag_rate_s_inv: 0.0,
};
const VELOCITY_DEPENDENT: Scenario = Scenario {
    name: "leo-velocity-dependent-drag",
    drag_rate_s_inv: DRAG_RATE_S_INV,
};

fn initial_state() -> State {
    let radius = EARTH_RADIUS_M + ALTITUDE_M;
    let speed = (MU_M3_S2 / radius).sqrt();
    [radius, 0.0, 0.0, 0.0, speed, 0.0]
}

fn derivative(state: &[f64], scenario: Scenario) -> State {
    let [x, y, z, vx, vy, vz] = [state[0], state[1], state[2], state[3], state[4], state[5]];
    let radius_squared = x * x + y * y + z * z;
    let gravity_scale = -MU_M3_S2 / (radius_squared * radius_squared.sqrt());
    let atmosphere_vx = -EARTH_ROTATION_RAD_S * y;
    let atmosphere_vy = EARTH_ROTATION_RAD_S * x;
    let relative_vx = vx - atmosphere_vx;
    let relative_vy = vy - atmosphere_vy;
    let relative_vz = vz;
    [
        vx,
        vy,
        vz,
        gravity_scale * x - scenario.drag_rate_s_inv * relative_vx,
        gravity_scale * y - scenario.drag_rate_s_inv * relative_vy,
        gravity_scale * z - scenario.drag_rate_s_inv * relative_vz,
    ]
}

fn analytic_two_body(duration_s: f64) -> State {
    let initial = initial_state();
    let radius = initial[0];
    let angular_rate = (MU_M3_S2 / radius.powi(3)).sqrt();
    let angle = angular_rate * duration_s;
    [
        radius * angle.cos(),
        radius * angle.sin(),
        0.0,
        -radius * angular_rate * angle.sin(),
        radius * angular_rate * angle.cos(),
        0.0,
    ]
}

fn errors(actual: &[f64], expected: &[f64]) -> (f64, f64) {
    let position = (0..3)
        .map(|index| (actual[index] - expected[index]).powi(2))
        .sum::<f64>()
        .sqrt();
    let velocity = (3..6)
        .map(|index| (actual[index] - expected[index]).powi(2))
        .sum::<f64>()
        .sqrt();
    (position, velocity)
}

fn assert_error_budget(label: &str, error: (f64, f64)) {
    assert!(
        error.0 <= MAX_POSITION_ERROR_M && error.1 <= MAX_VELOCITY_ERROR_M_S,
        "{label} exceeded the physical error budget: position={} m, velocity={} m/s",
        error.0,
        error.1
    );
}

#[cfg(feature = "adapter")]
fn assert_error_match(native: (f64, f64), vern9: (f64, f64)) {
    for (name, native_value, vern9_value) in [
        ("position", native.0, vern9.0),
        ("velocity", native.1, vern9.1),
    ] {
        let scale = native_value.max(vern9_value);
        assert!(
            scale == 0.0
                || (native_value - vern9_value).abs()
                    <= MAX_ERROR_DIFFERENCE_FRACTION * scale,
            "Vern9 {name} error differs from the native result by more than {MAX_ERROR_DIFFERENCE_FRACTION:.0}%: native={native_value}, Vern9={vern9_value}"
        );
    }
}

#[derive(Clone, Copy)]
struct RunSummary {
    endpoint: State,
    rhs: usize,
    accepted: usize,
    rejected: usize,
}

fn run_native(initial: State, scenario: Scenario, tolerance: f64, dense: bool) -> RunSummary {
    let settings = AdaptiveSettings {
        abs_tol: tolerance,
        rel_tol: tolerance,
        dense_output: dense,
        ..Default::default()
    };
    let solution = RKV98::integrate(
        0.0,
        DURATION_S,
        &Vector::from_array(initial),
        |_time, state: &Vector<f64, 6>| Vector::from_array(derivative(state.as_slice(), scenario)),
        &settings,
    )
    .expect("native RKV98 integration must succeed");
    RunSummary {
        endpoint: std::array::from_fn(|index| solution.y[index]),
        rhs: solution.evals,
        accepted: solution.accepted,
        rejected: solution.rejected,
    }
}

#[cfg(feature = "adapter")]
fn run_reusable_vern9(
    stepper: &mut differential_equations::stepping::ExplicitRungeKuttaStepper<'_>,
    controller: &mut differential_equations::stepping::AdaptiveController,
    tolerances: &differential_equations::tolerances::Tolerances,
    initial: State,
    scenario: Scenario,
) -> RunSummary {
    use differential_equations::stepping::{integrate_rk, ObserverAction};
    use differential_equations::tolerances::ErrorNorm;

    stepper.reset(0.0, &initial).expect("same-sized reset");
    stepper.clear_statistics();
    controller.reset(60.0).expect("valid controller reset");
    let mut rhs = |_time: f64, state: &[f64], output: &mut [f64]| {
        output.copy_from_slice(&derivative(state, scenario));
        Ok::<_, std::convert::Infallible>(())
    };
    let mut norm = |old: &[f64], candidate: &[f64], error: &[f64]| {
        Ok::<_, std::convert::Infallible>(
            tolerances
                .error_norm(old, candidate, error, ErrorNorm::Rms)
                .expect("validated tolerances"),
        )
    };
    integrate_rk(
        stepper,
        controller,
        DURATION_S,
        &[],
        2_000_000,
        &mut rhs,
        &mut norm,
        &mut |_| Ok(ObserverAction::Continue),
    )
    .expect("reusable Vern9 integration must succeed");
    let stats = stepper.statistics();
    let mut endpoint = [0.0; 6];
    stepper
        .copy_state_into(&mut endpoint)
        .expect("same-sized output");
    RunSummary {
        endpoint,
        rhs: stats.rhs_evaluations,
        accepted: stats.accepted_steps,
        rejected: stats.rejected_steps,
    }
}

fn print_endpoint_lanes(scenario: Scenario, reference: State, warmup: usize, measured: usize) {
    let initial = initial_state();
    for _ in 0..warmup {
        black_box(run_native(initial, scenario, BASELINE_TOLERANCE, false));
    }
    let startup_region = Region::new(GLOBAL);
    let startup_started = Instant::now();
    let startup = run_native(initial, scenario, BASELINE_TOLERANCE, false);
    let startup_elapsed_ns = startup_started.elapsed().as_nanos();
    let startup_allocations = startup_region.change();
    let region = Region::new(GLOBAL);
    let started = Instant::now();
    let mut result = startup;
    for _ in 0..measured {
        result = black_box(run_native(initial, scenario, BASELINE_TOLERANCE, false));
    }
    let elapsed_ns = started.elapsed().as_nanos();
    let allocations = region.change();
    let (position_error, velocity_error) = errors(&result.endpoint, &reference);
    let native_errors = (position_error, velocity_error);
    assert_error_budget("native RKV98 endpoint", native_errors);
    println!(
        "record=endpoint scenario={} lane=native-rkv98 tolerance={BASELINE_TOLERANCE:.0e} warmup={warmup} samples={measured} startup_elapsed_ns={startup_elapsed_ns} mature_elapsed_ns={elapsed_ns} rhs_per_arc={} accepted_per_arc={} rejected_per_arc={} position_error_m={position_error:.9e} velocity_error_m_s={velocity_error:.9e} startup_allocations={} startup_bytes={} mature_allocations={} mature_bytes={}",
        scenario.name,
        result.rhs,
        result.accepted,
        result.rejected,
        startup_allocations.allocations,
        startup_allocations.bytes_allocated,
        allocations.allocations,
        allocations.bytes_allocated,
    );

    #[cfg(feature = "adapter")]
    print_vern9_endpoint(
        scenario,
        reference,
        initial,
        warmup,
        measured,
        native_errors,
    );
}

#[cfg(feature = "adapter")]
fn print_vern9_tableau_initialization() {
    use differential_equations::solvers::explicit::Vern9;

    let region = Region::new(GLOBAL);
    let started = Instant::now();
    let tableau = Vern9.tableau().expect("Vern9 tableau");
    let elapsed_ns = started.elapsed().as_nanos();
    let allocations = region.change();
    black_box(tableau);
    println!(
        "record=tableau-initialization lane=vern9 elapsed_ns={elapsed_ns} allocations={} allocated_bytes={}",
        allocations.allocations, allocations.bytes_allocated
    );
}

#[cfg(feature = "adapter")]
fn print_vern9_endpoint(
    scenario: Scenario,
    reference: State,
    initial: State,
    warmup: usize,
    measured: usize,
    native_errors: (f64, f64),
) {
    use differential_equations::solvers::explicit::Vern9;
    use differential_equations::stepping::{
        AdaptiveController, ControllerConfig, ExplicitRungeKuttaStepper,
    };
    use differential_equations::tolerances::Tolerances;

    let mut startup_state = initial;
    let startup_region = Region::new(GLOBAL);
    let startup_started = Instant::now();
    let tableau = Vern9.tableau().expect("Vern9 tableau");
    let tolerances =
        Tolerances::scalar(6, VERN9_TOLERANCE, VERN9_TOLERANCE).expect("valid tolerances");
    let mut startup_stepper =
        ExplicitRungeKuttaStepper::from_buffer(tableau, 0.0, &mut startup_state)
            .expect("valid stepper");
    let mut startup_controller = AdaptiveController::new(
        ControllerConfig::proportional(8).expect("valid controller"),
        60.0,
    )
    .expect("valid controller");
    let startup = run_reusable_vern9(
        &mut startup_stepper,
        &mut startup_controller,
        &tolerances,
        initial,
        scenario,
    );
    let startup_elapsed_ns = startup_started.elapsed().as_nanos();
    let startup_allocations = startup_region.change();
    drop(startup_stepper);

    let mut state = initial;
    let mut stepper =
        ExplicitRungeKuttaStepper::from_buffer(tableau, 0.0, &mut state).expect("valid stepper");
    let mut controller = AdaptiveController::new(
        ControllerConfig::proportional(8).expect("valid controller"),
        60.0,
    )
    .expect("valid controller");
    for _ in 0..warmup {
        black_box(run_reusable_vern9(
            &mut stepper,
            &mut controller,
            &tolerances,
            initial,
            scenario,
        ));
    }
    let region = Region::new(GLOBAL);
    let started = Instant::now();
    let mut result = startup;
    for _ in 0..measured {
        result = black_box(run_reusable_vern9(
            &mut stepper,
            &mut controller,
            &tolerances,
            initial,
            scenario,
        ));
    }
    let elapsed_ns = started.elapsed().as_nanos();
    let allocations = region.change();
    let (position_error, velocity_error) = errors(&result.endpoint, &reference);
    let vern9_errors = (position_error, velocity_error);
    assert_error_budget("reusable Vern9 endpoint", vern9_errors);
    assert_error_match(native_errors, vern9_errors);
    println!(
        "record=endpoint scenario={} lane=reusable-vern9 tolerance={VERN9_TOLERANCE:.0e} controller=proportional-p8 warmup={warmup} samples={measured} startup_elapsed_ns={startup_elapsed_ns} mature_elapsed_ns={elapsed_ns} rhs_per_arc={} accepted_per_arc={} rejected_per_arc={} position_error_m={position_error:.9e} velocity_error_m_s={velocity_error:.9e} startup_allocations={} startup_bytes={} mature_allocations={} mature_bytes={}",
        scenario.name,
        result.rhs,
        result.accepted,
        result.rejected,
        startup_allocations.allocations,
        startup_allocations.bytes_allocated,
        allocations.allocations,
        allocations.bytes_allocated,
    );
}

#[cfg(feature = "adapter")]
fn print_dense_query_lanes(query_repetitions: usize) {
    use differential_equations::ndarray::{array, ArrayView1, ArrayViewMut1};
    use differential_equations::solvers::explicit::Vern9;
    use differential_equations::{solve, OdeProblem, SaveMode, SolveOptions};

    let scenario = TWO_BODY;
    let initial = initial_state();
    let query_count = (DURATION_S / DENSE_QUERY_INTERVAL_S) as usize + 1;
    let query_times: Vec<f64> = (0..query_count)
        .map(|index| index as f64 * DENSE_QUERY_INTERVAL_S)
        .collect();

    let settings = AdaptiveSettings {
        abs_tol: BASELINE_TOLERANCE,
        rel_tol: BASELINE_TOLERANCE,
        dense_output: true,
        ..Default::default()
    };
    let startup_region = Region::new(GLOBAL);
    let startup_started = Instant::now();
    let native_solution = RKV98::integrate(
        0.0,
        DURATION_S,
        &Vector::from_array(initial),
        |_time, state: &Vector<f64, 6>| Vector::from_array(derivative(state.as_slice(), scenario)),
        &settings,
    )
    .expect("native dense integration must succeed");
    let setup_elapsed_ns = startup_started.elapsed().as_nanos();
    let setup_allocations = startup_region.change();
    let native_stats = (
        native_solution.evals,
        native_solution.accepted,
        native_solution.rejected,
    );

    let native_errors = query_times
        .iter()
        .fold((0.0_f64, 0.0_f64), |maximum, &time| {
            let state = RKV98::interpolate(time, &native_solution)
                .expect("native dense query must be covered");
            let error = errors(state.as_slice(), &analytic_two_body(time));
            (maximum.0.max(error.0), maximum.1.max(error.1))
        });
    assert_error_budget("native RKV98 dense trajectory", native_errors);
    let query_region = Region::new(GLOBAL);
    let query_started = Instant::now();
    let mut checksum = 0.0;
    for _ in 0..query_repetitions {
        for &time in &query_times {
            let state = RKV98::interpolate(time, &native_solution)
                .expect("native dense query must be covered");
            for (component, value) in state.iter().enumerate() {
                checksum += value * (component + 1) as f64;
            }
            black_box(state);
        }
    }
    let query_elapsed_ns = query_started.elapsed().as_nanos();
    let query_allocations = query_region.change();
    black_box(checksum);
    println!(
        "record=dense-query scenario=two-body-leo lane=native-rkv98 tolerance={BASELINE_TOLERANCE:.0e} interval_s={DENSE_QUERY_INTERVAL_S} query_count={query_count} repetitions={query_repetitions} setup_elapsed_ns={setup_elapsed_ns} query_elapsed_ns={query_elapsed_ns} rhs_per_arc={} accepted_per_arc={} rejected_per_arc={} max_position_error_m={:.9e} max_velocity_error_m_s={:.9e} setup_allocations={} setup_bytes={} query_allocations={} query_bytes={} checksum={checksum:.12e}",
        native_stats.0,
        native_stats.1,
        native_stats.2,
        native_errors.0,
        native_errors.1,
        setup_allocations.allocations,
        setup_allocations.bytes_allocated,
        query_allocations.allocations,
        query_allocations.bytes_allocated,
    );

    let startup_region = Region::new(GLOBAL);
    let startup_started = Instant::now();
    let problem = OdeProblem::builder()
        .initial_state(array![
            initial[0], initial[1], initial[2], initial[3], initial[4], initial[5]
        ])
        .time_span((0.0, DURATION_S))
        .parameters(scenario)
        .build_with_in_place_rhs(
            |mut output: ArrayViewMut1<'_, f64>,
             state: ArrayView1<'_, f64>,
             scenario: &Scenario,
             _time: f64| {
                let dy = derivative(state.as_slice().expect("contiguous state"), *scenario);
                output.assign(&array![dy[0], dy[1], dy[2], dy[3], dy[4], dy[5]]);
            },
        );
    let options = SolveOptions::new()
        .with_tolerances(DENSE_VERN9_TOLERANCE, DENSE_VERN9_TOLERANCE)
        .with_save(SaveMode::Endpoints)
        .with_dense_output(true);
    let vern9_solution = solve(&problem, Vern9, &options).expect("Vern9 dense solve must succeed");
    let setup_elapsed_ns = startup_started.elapsed().as_nanos();
    let setup_allocations = startup_region.change();
    let stats = vern9_solution.stats();
    let mut state = [0.0; 6];
    let query_errors = query_times
        .iter()
        .fold((0.0_f64, 0.0_f64), |maximum, &time| {
            vern9_solution
                .try_interpolate_into(time, &mut state)
                .expect("Vern9 dense query must be covered");
            let error = errors(&state, &analytic_two_body(time));
            (maximum.0.max(error.0), maximum.1.max(error.1))
        });
    assert_error_budget("Vern9 dense trajectory", query_errors);
    assert_error_match(native_errors, query_errors);
    let query_region = Region::new(GLOBAL);
    let query_started = Instant::now();
    let mut checksum = 0.0;
    for _ in 0..query_repetitions {
        for &time in &query_times {
            vern9_solution
                .try_interpolate_into(time, &mut state)
                .expect("Vern9 dense queries must be covered");
            for (component, value) in state.iter().enumerate() {
                checksum += value * (component + 1) as f64;
            }
            black_box(state);
        }
    }
    let query_elapsed_ns = query_started.elapsed().as_nanos();
    let query_allocations = query_region.change();
    black_box(checksum);
    println!(
        "record=dense-query scenario=two-body-leo lane=reusable-vern9 controller=solver-default tolerance={DENSE_VERN9_TOLERANCE:.2e} interval_s={DENSE_QUERY_INTERVAL_S} query_count={query_count} repetitions={query_repetitions} setup_elapsed_ns={setup_elapsed_ns} query_elapsed_ns={query_elapsed_ns} rhs_per_arc={} accepted_per_arc={} rejected_per_arc={} max_position_error_m={:.9e} max_velocity_error_m_s={:.9e} setup_allocations={} setup_bytes={} query_allocations={} query_bytes={} checksum={checksum:.12e}",
        stats.rhs_evaluations,
        stats.accepted_steps,
        stats.rejected_steps,
        query_errors.0,
        query_errors.1,
        setup_allocations.allocations,
        setup_allocations.bytes_allocated,
        query_allocations.allocations,
        query_allocations.bytes_allocated,
    );
}

fn parse_arg(index: usize, default: usize) -> usize {
    std::env::args()
        .nth(index)
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn main() {
    let warmup = parse_arg(1, 5);
    let measured = parse_arg(2, 20);
    let initial = initial_state();
    #[cfg(feature = "adapter")]
    let query_repetitions = parse_arg(3, 10);
    #[cfg(feature = "adapter")]
    print_vern9_tableau_initialization();
    let reference_drag =
        run_native(initial, VELOCITY_DEPENDENT, REFERENCE_TOLERANCE, false).endpoint;
    for scenario in [TWO_BODY, VELOCITY_DEPENDENT] {
        let reference = if scenario.drag_rate_s_inv == 0.0 {
            analytic_two_body(DURATION_S)
        } else {
            reference_drag
        };
        print_endpoint_lanes(scenario, reference, warmup, measured);
    }
    #[cfg(feature = "adapter")]
    print_dense_query_lanes(query_repetitions);
    #[cfg(not(feature = "adapter"))]
    println!("dense_query_lanes=skipped reason=\"build with --features adapter\"");
}
