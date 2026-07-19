# Task: benchmark one Cartesian position EKF correction against Orekit

## Parity target

- Ledger row: Estimation / batch least squares and sequential filters.
- Current status: Partial.
- Intended status after this task: Partial with reproducible performance evidence for one shared sequential-OD scenario.

## User workflow

Run the PowerShell harness to build and time release-mode orskit and the
isolated Orekit public-API harness over the same Cartesian position correction.

## Scientific contract

- Inputs and units: GCRF Cartesian SI prior and position observation; explicit Earth `mu`.
- Outputs and units: checksum of posterior Cartesian metres.
- Frames/epochs/time scales: GCRF and J2000/TAI-equivalent epoch-zero workload.
- Conventions and valid regimes: point-mass bound elliptic two-body propagation; independent filters per query.
- External data requirements: none.
- Errors and singularities: each harness exits non-zero on invalid construction or correction failure.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| Orekit 13.1.6 public API | Apache-2.0 public API / black-box execution | `KalmanEstimator`, `KeplerianPropagatorBuilder`, `ConstantProcessNoise`, and `Position` construction | isolated Java benchmark harness |

No Orekit source, tests, examples, or implementation structure is copied.

## Validation

- Benchmarks: three 10,000-query release samples recorded in `.agent/references/orbit-determination/benchmark/results/2026-07-20-windows.md`.
- Independent scenario check: checksums agree to displayed precision for the common workload.

## Completion checklist

- [x] Reproducible harnesses
- [x] Public-API reference comparison
- [x] Provenance recorded
- [x] Results recorded
