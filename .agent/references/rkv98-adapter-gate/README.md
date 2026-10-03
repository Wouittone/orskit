# External Vern9 allocation and performance gate

This is a standalone, non-shipping Cargo workspace. It is not a member of the
orskit workspace and must not be added as a dependency of a shipped crate.
The external adaptive lane pins the released
`differential-equations-rs = 1.4.1` reusable-stepping contract. The native
comparison pins `numeris = 0.6.0` and uses its `RKV98` API unmodified.

## Scenarios and scientific policy

All arcs start from a six-component Earth-centered inertial Cartesian state
`[x, y, z, vx, vy, vz]` in metres and metres per second, at `t = 0 s`, with
`mu = 398600441800000 m^3/s^2`:

| Scenario | Inputs | Error reference |
| --- | --- | --- |
| `two-body-leo` | 500 km circular LEO; Earth radius 6,371,000 m; six-hour arc | Analytic circular two-body state |
| `leo-velocity-dependent-drag` | Same initial state and arc; central gravity plus synthetic co-rotating linear drag, `gamma = 2e-9 s^-1`, Earth spin `7.2921150e-5 rad/s` | Tighter `numeris::ode::RKV98` solve at `1e-13` absolute/relative tolerance; convergence reference, not an independent physical truth |
| `two-body-leo` dense queries | Same two-body arc; 721 query epochs every 30 s, including endpoints | Analytic circular two-body state at every query |

The drag law is a controlled velocity-dependent perturbation for measuring
state/RHS adapter cost. It is not a claim to model a particular atmosphere.
The comparison uses the same initial conditions, RHS equations, duration,
output policy, and achieved position/velocity error budget. The RKV98 and Vern9
tableaus and their controllers are different; this benchmark does not claim
they are the same method or have the same internal numerical work.

Native RKV98 uses `1e-9` absolute/relative tolerance and its native adaptive
controller. Reusable Vern9 uses `2e-10` absolute/relative tolerance and the
proportional controller with exponent 8. Dense-query Vern9 uses `2.15e-10`
tolerance through the release's dense-solve API and default controller. The
values target comparable observed errors; the raw per-sample errors are
retained so reviewers can reject a timing comparison if the achieved errors
diverge. The harness fails if either lane exceeds 2.1 mm position or
2.1e-6 m/s velocity error, or if the paired lane errors differ by more than
25%. These bounds are the evidence-comparability check for these scenarios,
not a project-wide propagation accuracy claim.

Endpoint records measure tableau initialization separately, then one
stepper/controller setup-and-propagation pass and a warm loop that resets and
reuses that Vern9 workspace. The RKV98 baseline is constructed anew per arc.
Dense-query records measure the separate dense solve and repeated query phases
using the same 30-second output epochs. `stats_alloc` reports allocator calls
and bytes for setup and warm/query regions. The runner independently samples
whole-process peak working set; it is not a per-lane allocation count and
includes loaded code/runtime.

## Reproduce

From the repository root, run:

```powershell
pwsh .agent\references\rkv98-adapter-gate\run.ps1 -Rounds 3 -SamplesPerRound 5 -Warmup 5 -Iterations 1000 -QueryRepetitions 20
```

The runner builds once with the locked external dependencies, executes
independent processes in three rounds of five samples, and saves `raw.txt` and
`metadata.json` under a timestamped `results` directory. Metadata records the
source commit/dirty state, lockfile SHA-256, solver/dependency versions, Rust
and Cargo versions, CPU/OS, power plan, and virtualization information.
Thermal conditions and host idleness cannot be established automatically; for
promotion-quality timing, repeat on an otherwise idle, thermally stable host.

The standalone harness can also be smoke-run directly:

```powershell
cargo run --release --manifest-path .agent\references\rkv98-adapter-gate\Cargo.toml --locked --features adapter -- 5 20 10
cargo fmt --manifest-path .agent\references\rkv98-adapter-gate\Cargo.toml -- --check
cargo clippy --manifest-path .agent\references\rkv98-adapter-gate\Cargo.toml --release --locked --all-targets --all-features -- -D warnings
```

The three executable arguments are warm-up arc count, measured arc count, and
dense-query repetitions. Per-sample elapsed totals are divided by their
iteration/query counts when comparing medians. Setup and dense-query records
remain separate from mature endpoint propagation.

## Promotion decision

ADR-0041 requires zero allocations in the mature reusable workspace and less
than 10% runtime regression against the native baseline at equivalent
scientific error and output policy. Meeting the allocation condition alone is
not sufficient. Absolute timings are local observations, not portable
performance promises.

## Recorded evidence (2026-10-03)

Command:

```powershell
pwsh .agent\references\rkv98-adapter-gate\run.ps1 -Rounds 3 -SamplesPerRound 5 -Warmup 5 -Iterations 1000 -QueryRepetitions 20
```

Raw samples: [`results/run-20261003-175341/raw.txt`](results/run-20261003-175341/raw.txt);
host/toolchain record: [`results/run-20261003-175341/metadata.json`](results/run-20261003-175341/metadata.json).
The earlier runs are retained for provenance, but are superseded: review found
that the dense Vern9 RHS callback allocated an output array on every RHS
evaluation. The callback now copies directly into its provided output slice;
the decision below uses only this corrected 3 x 5 run.
Each timing below is a median of 15 process samples, normalized per arc or
query. Endpoint position errors differ by less than 0.5% and endpoint velocity
errors by less than 7%; Vern9's dense-query maximum position and velocity
errors are respectively 18.6% and 13.2% smaller than native.

| Workload | Native RKV98 | Reusable Vern9 | Vern9 regression | Position error, native / Vern9 | Velocity error, native / Vern9 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Two-body LEO endpoint | 84.921 us/arc | 207.656 us/arc | +144.5% | 1.646 / 1.653 mm | 1.879 / 1.759 um/s |
| Velocity-dependent LEO endpoint | 86.445 us/arc | 211.852 us/arc | +145.1% | 1.644 / 1.639 mm | 1.878 / 1.754 um/s |
| Dense query, 721 outputs | 107.039 ns/query | 436.720 ns/query | +308.0% | 1.901 / 1.547 mm max | 1.919 / 1.666 um/s max |

Mature reusable Vern9 workspace propagation made **0 allocations / 0 bytes**
in all endpoint samples. Workspace setup made 9 allocations / 1,110 bytes;
first-use Vern9 tableau initialization, recorded separately, made 3,038
allocations / 168,709 bytes. The dense-query solve and retained output made
289 allocations / 154,288 bytes for Vern9 versus 104 / 112,064 bytes for native
RKV98. Repeated dense queries made no allocations in either lane. The largest
sampled whole-process working set was 4,923,392 bytes.

The runtime requirement is **not met**: matched-error Vern9 was 2.45x slower for
endpoint propagation and 4.08x slower per dense query, versus the allowed
maximum of 1.10x. Zero mature-loop allocations satisfy only the allocation half
of ADR-0041. The machine was a Windows 11 / Ryzen 7 3800X host on the High
performance power plan; idleness and thermal conditions were not independently
verified. Raw sample ranges show timing variability, so this is local negative
evidence, not a portable timing claim or promotion evidence. Do not promote the
adapter.

## Provenance

- `differential-equations-rs 1.4.1` (MIT OR Apache-2.0), pinned as an isolated
  dependency; the published reusable `ExplicitRungeKuttaStepper` API is used
  without copying its implementation.
- `numeris 0.6.0` (MIT), pinned as an isolated native RKV98 comparison
  dependency; no implementation source is copied.
- `stats_alloc 0.1.10` (MIT OR Apache-2.0), used only for allocation
  instrumentation.
- No crate in the orskit workspace depends on any of these benchmark
  dependencies.
