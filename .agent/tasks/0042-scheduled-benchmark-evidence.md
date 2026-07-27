# Task: retain scheduled benchmark evidence

## Parity target

- Ledger rows: existing OEM, two-body propagation, and sequential OD benchmark
  evidence
- Current status: Partial with a reproducible local harness
- Intended status after this task: Partial with periodic raw CI evidence and no
  cross-machine performance gate

## User workflow

Maintainers can dispatch a Linux benchmark run or let the weekly schedule run.
The workflow first executes each workload's correctness tests, then records
quick timing samples and full reproducibility metadata as a retained artifact.

## Scientific contract

- Inputs and units: unchanged deterministic OEM, two-body, and Cartesian OD
  workloads.
- Outputs and units: existing raw timing output, checksums, and host/toolchain
  metadata.
- Frames/epochs/time scales: unchanged from task 0035.
- Conventions and valid regimes: project-owned implementations only; reference
  runners remain opt-in and are not downloaded by scheduled CI.
- External data requirements: none beyond locked Rust dependencies.
- Errors and singularities: correctness or harness failures fail the job;
  timing differences do not.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| Existing project benchmark policy and harness | Original project material | Workloads, accuracy-first sequencing, metadata, and non-threshold review policy | `.github/workflows/benchmarks.yml` |

No new scientific reference, implementation source, dataset, or dependency is
introduced.

## Design

- Affected crates/layers: GitHub Actions and benchmark documentation only.
- Public API: unchanged.
- Rejected alternatives: pull-request timing thresholds; comparison with
  historical results from another host; scheduled download of reference
  implementations; treating anti-elision checksums as scientific validation.
- ADR required: no.

## Validation

- Unit cases: the workflow invokes the existing package accuracy suites.
- Invariants/properties: timing starts only after correctness succeeds.
- Independent reference vectors: existing task-0035 evidence is unchanged.
- Differential/scenario tests: not added.
- Tolerances and justification: no timing tolerance or regression threshold.
- Benchmarks: weekly and manually dispatched quick samples, retained for 30
  days with metadata and raw output.

## Completion checklist

- [x] Implementation and typed errors (no public implementation changed)
- [x] Scientific and regression tests (existing accuracy phase)
- [x] Documentation
- [x] Provenance recorded (no new material)
- [x] Roadmap updated
- [x] Relevant local harness checks pass
- [x] Binding impact explicitly deferred; no binding files changed
