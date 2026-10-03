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
diverge. The harness checks paired errors in combined mode, and the runner
enforces the same 25% relative-difference limit across the six isolated modes.
Either lane also fails if it exceeds 2.1 mm position or 2.1e-6 m/s velocity
error. These bounds are the evidence-comparability check for these scenarios,
not a project-wide propagation accuracy claim.

Endpoint records measure tableau initialization separately, then one
stepper/controller setup-and-propagation pass and a warm loop that resets and
reuses that Vern9 workspace. The RKV98 baseline is constructed anew per arc.
Dense-query records measure the separate dense solve and repeated query phases
using the same 30-second output epochs. `stats_alloc` reports allocation and
reallocation counts and bytes for setup and warm/query regions. Mature endpoint
propagation and repeated dense queries fail the run if they allocate or
reallocate. Each benchmark mode runs in a separate process, isolating native
and Vern9 lane footprints. After all benchmark regions finish, each process
flushes a completion marker and waits for the runner. The runner captures
Windows `Process.PeakWorkingSet64` while the process is held at this
post-measurement handshake, then releases it. This captures the
OS-maintained high-water working set after measurement without sampling fast
processes or extending the timed regions. It is a whole-process footprint
(including runtime and code), not retained heap memory or a per-phase heap
measurement.

## Reproduce

From the repository root, run:

```powershell
pwsh .agent\references\rkv98-adapter-gate\run.ps1 -Rounds 3 -SamplesPerRound 5 -Warmup 5 -Iterations 1000 -QueryRepetitions 20
```

The runner builds once with the locked external dependencies, then executes
each native/Vern9 scenario and dense-query lane in its own process for each
sample (six modes per sample), verifies paired position/velocity errors within
25%, and requires the post-measurement memory-capture handshake. It saves
`raw.txt` and `metadata.json` under a timestamped `results` directory. Metadata
records the source commit and dirty
state, SHA-256 hashes of `Cargo.toml`, `Cargo.lock`, `src/main.rs`, and
`run.ps1`, solver/dependency versions, Rust and Cargo versions, CPU/OS, power
plan, and virtualization information. Dirty source is permitted because the
input hashes identify the exact harness inputs used. Thermal conditions and
host idleness cannot be established automatically; for promotion-quality
timing, repeat on an otherwise idle, thermally stable host.

The standalone harness can also be smoke-run directly:

```powershell
cargo run --release --manifest-path .agent\references\rkv98-adapter-gate\Cargo.toml --locked --features adapter -- 5 20 10
cargo fmt --manifest-path .agent\references\rkv98-adapter-gate\Cargo.toml -- --check
cargo clippy --manifest-path .agent\references\rkv98-adapter-gate\Cargo.toml --release --locked --all-targets --all-features -- -D warnings
```

The three executable arguments are warm-up arc count, measured arc count, and
dense-query repetitions. An optional fourth mode argument selects
`native-two-body`, `vern9-two-body`, `native-drag`, `vern9-drag`,
`native-dense`, or `vern9-dense`; the default `all` mode runs both lanes and
checks paired error comparability in one process. The runner adds
`hold-for-peak` as a fifth argument to enable the completion handshake. Direct
harness runs omit it and do not wait for runner input. Per-sample elapsed
totals are divided by their iteration/query counts when comparing medians.
Setup and dense-query records remain separate from mature endpoint
propagation.

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

Raw samples: [`results/run-20261003-183015/raw.txt`](results/run-20261003-183015/raw.txt);
host/toolchain record and source hashes:
[`results/run-20261003-183015/metadata.json`](results/run-20261003-183015/metadata.json).
Earlier runs are retained but superseded. The first corrected run addressed an
allocating dense Vern9 RHS callback; this run additionally isolates all six
native/Vern9 workload modes in separate processes, records reallocation
statistics, captures the OS high-water metric at a completion handshake, and
checks paired physical errors in each sample. The callback copies directly
into its provided output slice. Each timing and process-peak figure below is a
median of 15 samples per lane. Elapsed totals are normalized per arc or query.

| Workload | Native RKV98 | Reusable Vern9 | Vern9 regression | Position error, native / Vern9 | Velocity error, native / Vern9 | Median peak process working set, native / Vern9 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Two-body LEO endpoint | 60.349 us/arc | 144.798 us/arc | +139.9% | 1.646 / 1.653 mm | 1.879 / 1.759 um/s | 4,280,320 / 4,546,560 bytes |
| Velocity-dependent LEO endpoint | 59.189 us/arc | 143.708 us/arc | +142.8% | 1.644 / 1.639 mm | 1.878 / 1.754 um/s | 4,280,320 / 4,571,136 bytes |
| Dense query, 721 outputs | 83.821 ns/query | 298.162 ns/query | +255.7% | 1.901 / 1.547 mm max | 1.919 / 1.666 um/s max | 4,415,488 / 4,759,552 bytes |

Across all 15 samples, mature native and reusable Vern9 endpoint regions and
both repeated dense-query regions recorded **0 allocations, 0 allocated bytes,
0 reallocations, and 0 bytes reallocated**. Reusable endpoint workspace setup
made 9 allocations / 1,110 bytes and 0 reallocations. Vern9 tableau
initialization made 3,038 allocations / 168,709 bytes and 1,918 reallocations /
23,267 net bytes reallocated. Dense-solve setup made 289 allocations /
154,288 bytes and 5 reallocations / 26,784 net bytes reallocated for Vern9,
versus 104 allocations / 112,064 bytes and 20 reallocations / 10,912 net bytes
reallocated for native RKV98. Repeated queries were allocation- and
reallocation-free.

The process-peak figures are OS-maintained `Process.PeakWorkingSet64` values
captured at the post-measurement completion handshake while each isolated mode
process is held open. They measure whole-process footprint, including
code/runtime and setup/reference work; they are not exact retained-heap or
phase-specific memory measurements. The runner checked every paired position
and velocity error against its 25% comparability limit. Paired endpoint
position errors differ by less than 0.5% and endpoint velocity errors by less
than 7%. Vern9's dense-query maximum position and velocity errors are 18.6%
and 13.2% smaller than native.

The runtime requirement is **not met**: matched-error Vern9 was 2.40x and 2.43x
slower on the endpoint scenarios and 3.56x slower per dense query, versus the
allowed maximum of 1.10x. Mature allocation and reallocation freedom satisfies
only the allocation half of ADR-0041. The machine was a Windows 11 / Ryzen 7
3800X host on the High performance power plan; idleness and thermal conditions
were not independently verified. These are local negative observations, not
portable timing or retained-memory claims. Do not promote the adapter.

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
