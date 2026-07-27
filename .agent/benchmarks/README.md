# Maintained benchmark suite

This directory defines the maintained performance workloads that already exist
in orskit. The suite measures current behavior; it does not establish feature
parity or promise portable timing.

## Workloads

| Name | Timed operation | Correctness evidence | Historical result |
| --- | --- | --- | --- |
| `oem` | Generate and parse just over 100 MiB of deterministic OEM KVN through streaming, sequential collection, and ordered Rayon collection | The `ccsds` test suite checks parser modes, source order, units, chronology, budgets, malformed input, and the attributed covariance fixture | [`2026-07-04-oem-kvn-100-mib.md`](2026-07-04-oem-kvn-100-mib.md) |
| `two-body` | Repeated independent Cartesian endpoint queries over deterministic signed offsets in plus or minus one day | The `dynamics-two-bodies` test suite checks analytic cases, conserved quantities, signed-time recovery, and pinned independent endpoints | [`two-body 2026-07-04 Windows result`](../references/two-body/benchmark/results/2026-07-04-windows.md) |
| `od` | Repeated independent one-observation Cartesian position EKF corrections | The `orbit-determination` test suite checks deterministic EKF/UKF recovery and covariance behavior. The benchmark checksum is only an anti-elision/scenario-consistency check, not accuracy evidence | [`OD 2026-07-20 Windows result`](../references/orbit-determination/benchmark/results/2026-07-20-windows.md) |

The external Orekit, Lox, and Nyx runners remain isolated under
`.agent/references/`. They are optional because their toolchains or dependencies
may require network access and because Nyx has the stricter provenance boundary
recorded in `PROVENANCE.md`.

## Run protocol

Run correctness before timing and retain the generated directory as the raw
record:

```powershell
pwsh .agent/benchmarks/run.ps1 -Phase accuracy
pwsh .agent/benchmarks/run.ps1 -Phase timing -Quick
```

`-Workload oem,two-body,od` selects a subset. Timing defaults to project-owned,
offline-capable Rust implementations. `-IncludeReferences` additionally runs
the isolated comparison harnesses and may need Gradle, Java, and cached or
downloadable pinned dependencies. Without `-OutputDirectory`, results go under
the ignored Cargo `target/benchmark-runs/` tree.

Accuracy and timing are deliberately separate phases:

1. Accuracy is asserted by normal Rust tests with physical tolerances and
   provenance. A failing accuracy phase invalidates the timing result.
2. Timing uses release/benchmark profiles, fixed inputs, warm-up, repeated
   samples, and checksums that force result consumption. Checksums never replace
   physical assertions.
3. `metadata.json` records UTC time, commit and dirty state, Cargo.lock SHA-256,
   selected workloads/phases, sample controls, OS/architecture/CPU identifiers,
   logical processor count, PowerShell, Cargo, and verbose Rust compiler data.
4. For a publishable run, use an otherwise idle host, record power/thermal and
   virtualization conditions that are not discoverable automatically, retain
   raw output, and repeat the same command after the candidate change.

The OEM Criterion harness reports distributions and byte throughput. The
two-body comparison interleaves implementations and also samples process
working set. The maintained Rust-only OD run records each harness-reported
elapsed time; the optional reference runner repeats both implementations.
Build time is excluded everywhere.

## Baseline and regression review policy

The linked records are historical local baselines, not acceptance thresholds.
Never compare their absolute times across different machines, toolchains,
power modes, virtualization environments, or materially different background
loads.

Performance review uses paired evidence on a controlled host:

- compare the same commit/toolchain/profile/workload controls, changing only
  the candidate code;
- inspect all raw samples, medians/distributions, checksums, and applicable
  working-set observations rather than selecting the best run;
- rerun when a directional change exceeds that host's observed run-to-run
  noise or Criterion reports a statistically significant change;
- investigate algorithmic work, allocations, data volume, CPU frequency,
  thermal throttling, and background load before calling a change a regression;
- accept a slowdown only with an explicit correctness, maintainability, or
  capability rationale and preserve both before/after records; and
- never weaken an accuracy assertion to recover performance.

There is intentionally no universal percentage threshold. A project CI host
may later maintain its own statistically characterized alert band, but that
local policy must remain an investigation trigger rather than a cross-machine
performance promise.

The `Scheduled benchmark evidence` workflow runs the project-owned accuracy
phase followed by quick timing samples each Monday and on manual dispatch. It
retains raw output and metadata for 30 days. A timing change never fails that
workflow by comparison with another run; maintainers inspect paired evidence
under the policy above.

## Deterministic invariant strategy

Current invariant coverage uses fixed, reviewable case matrices instead of a
new property-test dependency. The supported regimes are narrow enough to cover
representative low/high eccentricity, prograde/near-retrograde inclination,
signed propagation durations, and multiple coordinate signs deterministically.
`proptest` can be reconsidered when supported conic/frame/model domains become
large enough that shrinking generated failures adds material value; any such
dependency still requires the license and maintenance rationale mandated by
`PROVENANCE.md`.
