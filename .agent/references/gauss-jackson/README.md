# Independent Gauss-Jackson scientific evidence

Issue #16; ADR-0048; baseline main `b3228caee857e7c9c7f0ecc393b01f7addaa335c`.
This is an opt-in experimental eighth-order Cartesian endpoint method, not
complete operational validation or semi-analytical propagation.

## References and reproducible scenarios

Matthew M. Berry and Liam M. Healy, *Implementation of Gauss-Jackson Integration
for Orbit Propagation*, Journal of the Astronautical Sciences 52(3),
July-September 2004, pp. 331-357, <https://hdl.handle.net/1903/2202>.
Publicly accessible copyrighted paper; equations and numerical facts only.
The original code derives coefficients algebraically from generating functions.
No supplemental Lisp program, library implementation, test, figure, or
distinctive prose was copied. The PDF is not distributed.

Pages 353-354 give ISS period 92.05 min/eccentricity 0.001 and CRRES period
607.28 min/eccentricity 0.716. Our **reduced two-body** scenarios retain those
facts, not the paper's 24x24 geopotential, Jacchia-70, lunar/solar perturbations.
Both use `mu=398600441800000 m^3/s^2`,
`a=cbrt(mu*(period_s/(2*pi))^2)`, inclination 0.7 rad, RAAN 1.1 rad,
argument of periapsis 0.4 rad, initial true anomaly zero, GCRF Earth origin.
Reference epochs use Orekit J2000; Rust uses TAI seconds zero. These autonomous
models depend on elapsed SI seconds only, so no epoch/frame transform is implied.

Independent endpoints come from unmodified Orekit 13.1.6 `KeplerianPropagator`
public APIs, in the isolated original Java harness:

```powershell
gradle -p .agent\references\gauss-jackson\orekit run --quiet --no-daemon
```

It prints daily endpoints through 72 h; the 72 h values are retained in offline
Rust tests. No Java dependency is shipped in orskit.

The same harness additionally uses unmodified Hipparchus 4.0.3 DOP853 (Apache-2.0,
transitive Orekit dependency) to integrate the original perturbed model
`r''=-mu*r/|r|^3-k*v`, `k=1e-8 /s`, from the reduced ISS initial state for 72 h.
This is smooth linear drag, **not a physical atmosphere or ISS prediction**.
Maximum step is 30 s, minimum 1e-6 s, relative tolerance 1e-15, absolute
position tolerances 1e-7 and 1e-8 m with velocity thresholds 1000 times smaller.
Tenfold tightening changes the reference position by less than 13 um and velocity
by less than 16 nm/s. The tighter reference is:

```text
position_m = [-2779458.7944452288, 4714719.550021319, 3887710.583637523]
velocity_m_s = [-5447.035423406024, -5000.319688345475, 2178.4209192667627]
```

## Tests and physical budgets

Native startup settings: 1e-7 m / 1e-10 m/s absolute, 1e-15 relative,
1e-8 s minimum, 1 s maximum, 0.1 s initial, 100000 attempts / 10000 rejections.
Mature correction thresholds: 1e-8 m / 1e-11 m/s per component, 20 iterations,
1000000 total grid steps. Startup and fixed-point thresholds are not global
error bounds.

The a priori reduced-orbit numerical budget is 1 cm position and 10 um/s
velocity per component over three days: substantially below operational
state/model uncertainty, but above independent f64/reference noise. A 30 s
step passes for the near-circular case; 30 s initially failed the eccentric
case's unchanged 1 cm budget, so its step was reduced to 15 s instead of
relaxing that budget. The perturbed 30 s case passes the same budget against
the independent DOP853 endpoint.

An additional 7200 km/e=0.1 analytical comparison checks phase error, full
frame/epoch preservation, reverse recovery (2 cm / 20 um/s) and 2 ppb relative
specific-energy/angular-momentum conservation. Conserved invariants do not
replace endpoint phase checks. A harmonic oscillator step-halving test requires
at least nominal eighth-order improvement before the starter/roundoff floor;
its observed ratio is about 774, not a general superconvergence claim.
Constant acceleration checks short/startup/mature/signed arcs, and a
velocity-dependent exponential `v'=-0.001*v` checks an exact independent
manufactured solution with 0.1 mm / 0.1 um/s budgets.
An epoch-dependent `x''=2*t` cubic checks signed force epochs and rejects an
endpoint just one nanosecond off the grid. Its BS32 starter uses fixed 1 s
steps (exact for this cubic) to avoid adaptive stage-epoch quantization masking
the epoch contract; its budgets are 1e-8 m / 1e-10 m/s.

Configuration rejection, off-grid targets, step limits, non-inertial axes,
acceleration-frame mismatch, preserved dynamics error sources and correction
nonconvergence are tested. There is no dense interpolation to validate;
off-grid queries explicitly fail. Public configuration rustdoc is a doctest.

## Local benchmark: 2026-10-03 Windows x86_64

AMD Ryzen 7 3800X; Rust 1.96.1 (`31fca3adb`), Cargo benchmark profile, active
`target-cpu=native` flags. Cargo.lock SHA-256:
`86f32d6a71cf4347a9058b5564d4ac8989c35d587baf0b118fb11aa85434c3c0`.
Shared host; background load, thermal/power
state and peak process memory were not controlled. Build time is excluded.
Atomic RHS counters impose the same per-evaluation instrumentation on both
methods. Every output is consumed with `black_box` and compared in SI norms.

```powershell
$env:CARGO_BUILD_JOBS='1'
cargo bench -p dynamics-numerical --features gauss-jackson --bench gauss_jackson --locked
```

Three rounds of three samples interleave full-arc Gauss-Jackson/native endpoints.
All samples, including the first, are in
[`2026-10-03-windows.csv`](2026-10-03-windows.csv). Each call starts fresh history.
Eight-step starter-only arcs are timed separately; full arcs include startup
and 8632 / 17272 mature steps, respectively. Reusing a solver value does not
reuse history. No dense/output storage is requested from either solver.

| Three-day endpoint workload | Median elapsed | Position error norm | Velocity error norm | RHS calls |
| --- | ---: | ---: | ---: | ---: |
| Gauss-Jackson, reduced ISS, 30 s | 10.4863 ms | 1.014469757e-5 m | 1.159961164e-8 m/s | 29290 |
| Native BS32, reduced ISS | 2.0363839 s | 6.726656722e-3 m | 7.661170013e-6 m/s | 12405352 |
| Gauss-Jackson, reduced CRRES, 15 s | 16.9656 ms | 3.773192061e-5 m | 7.374094513e-9 m/s | 43700 |
| Native BS32, reduced CRRES | 0.4987430 s | 2.718989581e-5 m | 7.561067965e-9 m/s | 3195444 |

Native full arcs use 1e-7 m / 1e-10 m/s / 1e-15 relative, maximum step 30 s,
initial 0.1 s, minimum 1e-8 s, 5000000 attempts / 100000 rejections. Gauss-Jackson
uses those same native startup tolerances with maximum starter step 30 s
(tests use a stricter 1 s cap), then its configured fixed mature grid.
Both methods meet the 1 cm / 10 um/s norm budget here, but achieved errors and
algorithmic work are **not identical**, especially for the near-circular case.
This is a local same-budget accuracy/timing comparison, not an equivalent-work
external-adapter promotion test or a portable speed guarantee.

Cold construction measured about 0.8 us per solver; inline counted-solver
storage is 400 bytes. This is not heap size or peak memory. Starter-only
median timings are 1.8192 ms / 1.2381 ms, with 11613 / 7885 RHS evaluations.
Arrays and compensated sums contain no allocating mature-path operations by
inspection; no allocation profiler or retained/peak-memory measurement was
performed, and application dynamics can allocate. Native behavior is untouched;
do not infer a default-path regression or promote an external adapter.

## Validation record

### Review correction: checked-state acceptance

The original benchmark CSV above is historical evidence before PR review.
Review identified an extra acceleration evaluation and unchecked state correction
after the fixed-point threshold test. Acceptance now retains the checked
candidate and its producing acceleration history, with compensated sums included
in the iteration itself. A smooth, ill-conditioned velocity-coupled regression
demonstrates that the removed extra correction could exceed both thresholds.
Convergence still does not imply accuracy or stability for that artificial force.

Post-fix commands (Rust 1.96.1, serial Cargo builds):

- `cargo test -p dynamics-numerical --all-features --locked gauss_jackson -- --nocapture`:
  all 10 Gauss-Jackson tests passed, including the independent long-arc budgets.
- `cargo clippy -p dynamics-numerical --all-targets --all-features --locked -- -D warnings -D clippy::must-use-candidate`:
  passed.
- `cargo nextest run --workspace --all-targets --all-features --locked --test-threads 2 --status-level fail --final-status-level fail`:
  235 tests passed across 34 binaries.
- `cargo test -p dynamics-numerical --doc --all-features --locked`:
  one doctest passed.
- `cargo bench -p dynamics-numerical --features gauss-jackson --bench gauss_jackson --locked`:
  all 54 samples completed; the unchanged physical budgets still pass.

Post-fix three-day Gauss-Jackson position/velocity norm errors are
6.426431948e-6 m / 7.379947423e-9 m/s (reduced ISS) and
3.742562121e-5 m / 7.313233925e-9 m/s (reduced CRRES), with 20563 / 26433 RHS
evaluations. Median elapsed times on the same uncontrolled host were
7.3381 / 12.4632 ms; the paired native medians were 1.7207932 / 0.4412556 s
with unchanged native errors. These are not replacement portable baselines;
the original CSV is deliberately retained rather than overwritten.

### Initial implementation checks

All Rust checks use the pinned/MSRV Rust 1.96.1. The initial unrestricted parallel
link failed from Windows memory exhaustion (`LNK1102` / `0xc000012d`), not a
source diagnostic; serial Cargo builds resolved it.

| Exact command | Result |
| --- | --- |
| `cargo fmt --all --check` | Passed |
| `cargo check --workspace --all-targets --all-features --locked` | Passed |
| `cargo +1.96.1 check --workspace --all-targets --all-features --locked` | Passed: explicit pinned/MSRV toolchain |
| `cargo +1.96.1 check --workspace --all-targets --locked` | Passed: default features |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings -D clippy::must-use-candidate` | Passed after fixing new lint findings |
| `$env:CARGO_BUILD_JOBS='1'; cargo nextest run --workspace --all-targets --all-features --locked --test-threads 2 --status-level fail --final-status-level fail` | Passed: 234 tests, 34 binaries |
| `cargo nextest run -p dynamics-numerical --all-targets --no-default-features --features gauss-jackson --locked --test-threads 2 --status-level fail --final-status-level fail` | Passed: 42 tests, 6 binaries |
| `cargo nextest run --workspace --lib --tests --locked --test-threads 2 --status-level fail --final-status-level fail` | Passed: 219 default-feature tests, 22 binaries |
| `cargo +1.96.1 check --workspace --lib --no-default-features --locked` | Passed with two pre-existing unused-import warnings in measurements |
| `cargo +1.96.1 check -p orskit --all-targets --no-default-features --features gauss-jackson --locked` | Passed: isolated facade feature/MSRV |
| `cargo +1.96.1 check --workspace --all-targets --no-default-features --locked` | Failed on pre-existing ungated CCSDS OEM benchmark import of the disabled `parallel` API |
| `cargo test --workspace --doc --all-features --locked` | Passed: 11 doctests |
| `cargo doc --workspace --all-features --no-deps --locked` | Passed |
| `cargo doc -p dynamics-numerical --no-default-features --no-deps --locked` | Completed with a pre-existing maneuver rustdoc link to an attitude-gated method |
| `cargo bench --workspace --all-features --no-run --locked` | Passed: all 24 benchmark targets compiled |
| `cargo bench -p dynamics-numerical --features gauss-jackson --bench gauss_jackson --locked` | Passed: 54 timing/accuracy samples |
| `gradle -p .agent\references\gauss-jackson\orekit run --quiet --no-daemon` | Passed: independent analytical and perturbed vectors generated |
| `pwsh -NoProfile -File scripts\check_crate_diagram.ps1 -Check` | Failed on pre-existing layer-map omissions: atmosphere, dynamics-drag/harmonics/spherical-harmonics/srp/third-bodies; no crate/dependency was added by this change |
| `git diff --check` | Passed |

Remaining limitations: real data-backed perturbed operational scenarios,
dense interpolation, persistent history, adaptive mature-step error control,
other orders, events/maneuvers/STM/covariance, allocation profiling and language
bindings are not provided. No commit, push, merge or pull request was made.
