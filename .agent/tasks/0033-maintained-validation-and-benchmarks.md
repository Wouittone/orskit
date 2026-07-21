# Task: maintain validation invariants and benchmark evidence

## Parity target

- Ledger rows: frames/transforms; orbit representations; two-body propagation;
  instantaneous range; CCSDS OEM; sequential orbit determination.
- Current status: Partial with isolated examples, benchmarks, and selected
  invariant tests.
- Intended status after this task: Partial with maintained benchmark
  methodology, reproducibility records, baseline review policy, and broader
  deterministic invariant evidence. No row becomes Validated.

## User workflow

Run accuracy separately from timing for any subset of the existing OEM,
two-body, and Cartesian-position OD workloads, retain machine/toolchain
metadata, and review local before/after results without a cross-machine limit.

## Scientific contract

- Inputs and units: existing explicit OEM KVN kilometres/kilometres per second;
  GCRF Cartesian SI orbit and OD states; typed frame kinematics and range.
- Outputs and units: existing typed parse/state/filter outputs; benchmark
  throughput, elapsed nanoseconds, process working set where supported, and
  anti-elision checksums.
- Frames/epochs/time scales: existing OEM metadata; explicit GCRF/ITRF2020 and
  Hifitime epochs in test scenarios.
- Conventions and valid regimes: current bound elliptic representation and
  point-mass propagation regime; instantaneous one-leg path-length range only.
- External data requirements: none for project-owned accuracy and timing;
  optional isolated public-API comparisons retain their pinned dependencies.
- Errors and singularities: existing typed parser, state, propagator, and
  estimator failures; element tests avoid the documented equinoctial
  retrograde singularity.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| Existing project scientific contracts, attributed fixtures, and isolated benchmark records | Original/previously recorded project evidence | Existing supported workloads, regimes, tolerances, and provenance boundaries | Tests and `.agent/benchmarks` methodology |

No new external scientific material, datasets, implementation source, or
dependency is introduced, so the project provenance ledger does not change.

## Design

- Affected crates/layers: test-only coverage in `frames`, `orbits`,
  `dynamics-two-bodies`, and `measurements`; maintained harness under `.agent`.
- Public API: unchanged.
- Rejected alternatives: timing assertions in correctness tests; portable
  percentage thresholds; random generation without a failure-space benefit;
  linking isolated reference implementations into the workspace.
- ADR required: no; this applies the existing engineering policy without a
  durable public/API architecture decision.

## Validation

- Unit cases: existing workload package suites remain the accuracy gates.
- Invariants/properties: affine frame transform direct/composed/inverse;
  representation round trips through Cartesian physical state across four
  elliptic regimes; two-body specific energy and angular-momentum conservation
  across four signed-duration cases; reversed instantaneous one-leg range.
- Independent reference vectors: existing attributed OEM covariance and pinned
  Orekit/Lox two-body endpoint fixtures.
- Differential/scenario tests: existing optional isolated runners; current
  deterministic EKF/UKF recovery scenarios.
- Tolerances and justification: orbit round trips use 1 micrometre and 1
  nanometre/second budgets; two-body invariants allow 2e-13 relative numerical
  drift; range reversal allows 1 nanometre. These are deterministic floating-
  point budgets well below the fidelity of the current physical models.
- Benchmarks: unified OEM/two-body/OD runner writes raw output plus commit,
  lockfile, toolchain, host, and run-control metadata.

## Completion checklist

- [x] Implementation and typed errors (public implementation unchanged)
- [x] Scientific and regression tests
- [x] Rustdoc/examples (public API unchanged; maintained methodology documented)
- [x] Provenance recorded (no new external material)
- [x] Parity ledger updated
- [x] Relevant checks pass
- [x] Binding impact explicitly deferred; no binding files changed
