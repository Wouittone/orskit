# Task: Reproducible two-body performance comparison

## Parity target

- Ledger row: Propagation / Two-body/Keplerian propagation
- Current status: Partial
- Intended status after this task: Partial, with reproducible speed and process-memory evidence

## User workflow

Run one PowerShell command to build release-mode orskit, Lox, Orekit, and an
isolated AGPL Nyx harness and compare independent Cartesian endpoint-query
throughput and peak process working set on the local machine.

## Scientific contract

- Inputs and units: identical GCRF Cartesian position in metres and velocity in metres per second; Earth point-mass gravity
- Outputs and units: Cartesian position and velocity folded into a checksum
- Frames/epochs/time scales: geocentric inertial frame; arbitrary fixed uniform epoch; signed target offsets in seconds
- Conventions and valid regimes: bound elliptic orbit, independent point queries over plus or minus one day
- External data requirements: none
- Errors and singularities: harness fails immediately if any implementation rejects a query

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| Orekit 13.1.6 public API | Apache-2.0 public API/black-box behavior | `CartesianOrbit` and `KeplerianPropagator` public construction and endpoint propagation | Orekit benchmark harness |
| Lox 0.1.0-alpha.39 public API | MPL-2.0 dependency and public API/black-box behavior | Cartesian `Orbit`, `Vallado`, and `state_at` endpoint propagation | Isolated Lox benchmark harness |
| Nyx 2.3.1 public API | AGPL-3.0-or-later dependency; validation-only use approved by project owner | Cartesian `Orbit` and documented two-body `at_epoch` behavior | Isolated AGPL Nyx benchmark harness |

No implementation source, tests, examples, or internal design from any reference is
copied or consulted.

## Design

- Affected crates/layers: dynamics example plus external black-box reference harnesses
- Public API: none
- Rejected alternatives: Python Lox bindings, because interpreter overhead would dominate; in-process JVM measurement, because process memory would no longer be comparable
- ADR required: no; this records evidence without changing architecture

## Validation

- Unit cases: existing propagation tests
- Invariants/properties: checksum prevents unused-result elimination; existing energy/reverse-time tests
- Independent reference vectors: Orekit and Lox endpoint fixtures; Nyx output
  recorded with its failed 3,600-second accuracy comparison
- Differential/scenario tests: identical Cartesian workload and query offsets
- Tolerances and justification: existing physical endpoint tolerances remain authoritative; performance checks do not assert timing thresholds
- Benchmarks: release builds, 10,000 warm-up queries, repeated timed samples, peak whole-process working set

## Completion checklist

- [x] All project-owned benchmark harnesses compile and run
- [x] Scientific and regression tests remain green
- [x] Methodology and available local results recorded
- [x] Provenance recorded
- [x] Parity ledger updated
- [x] Relevant workspace checks pass, including isolated Lox and Nyx release builds
- [x] Binding impact explicitly deferred; no binding API changes
