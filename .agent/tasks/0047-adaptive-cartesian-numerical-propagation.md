# Task: propagate translational Cartesian states with adaptive RKF4(5)

## Parity target

- Ledger row: Propagation / Numerical integration and dense ephemerides
- Current status: Designed
- Intended status after this task: Partial, with endpoint-only adaptive
  Cartesian propagation; dense ephemerides remain P12

## User workflow

A caller implements `CartesianDynamics` for one explicit frame, returning
typed acceleration at each Hifitime epoch. The caller selects typed position,
velocity, and relative tolerances; positive step bounds; and non-zero solver
limits. `AdaptiveRungeKuttaFehlberg` then propagates an
`Orbit<CartesianState>` forward or backward to the exact requested epoch.

## Scientific contract

- Inputs and units: typed Cartesian position/velocity; typed acceleration from
  the dynamics; `Length`, `Velocity`, and `Ratio` tolerances; Hifitime step
  durations.
- Outputs and units: `Orbit<CartesianState>` in the same declared frame at the
  exact target epoch.
- Frames/epochs/time scales: state and dynamics frames must be identical;
  elapsed Hifitime durations are uniform and signed; rational stage-epoch
  offsets truncate fractional nanoseconds toward zero in either direction.
- Conventions and valid regimes: Fehlberg RK4(5) formula 2, fifth-order
  accepted estimate, six-component scaled RMS local error, controller safety
  0.9 and clamp `[0.2, 5]`; Cartesian translation only.
- External data requirements: entirely owned/borrowed by the caller's
  immutable dynamics implementation; no ambient lookup or network access.
- Errors and singularities: invalid tolerances/bounds, unsupported component
  requirements, frame mismatch, provider failure, non-finite
  state/derivative/error, minimum step, accepted-step limit, and rejected-step
  limit are typed. Point-mass collision behavior belongs to the model.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| Erwin Fehlberg, NASA TR R-315, July 1969 | Primary US Government technical report; public use permitted | RK4(5) formula 2 Table III nodes, stages, embedded weights, and error-control concept | `crates/dynamics/numerical`; ADR-0044 |
| Existing analytical two-body solver and its Orekit 13.1.6 black-box evidence | Original implementation with provenance-cleared behavior comparison | Independent endpoint and physical conservation baseline | numerical point-mass scenario |

## Design

- Affected crates/layers: new `dynamics-numerical`; `dynamics` and `orskit`
  opt-in features; architecture/parity/provenance/roadmap records.
- Public API: `CartesianDynamics`, typed tolerance/bounds/limits/configuration,
  `AdaptiveRungeKuttaFehlberg`, and typed build/propagation errors.
- Rejected alternatives: public numeric vectors; changing descriptive force
  traits into evaluators; coupled-state placeholders; dense output/events.
- ADR required: ADR-0044.

## Validation

- Unit cases: configuration, requirements, frames, provider/non-finite
  failures, and every solver exhaustion category.
- Invariants/properties: exact target epoch; polynomial solution
  forward/backward; rejection leaves accepted state unchanged.
- Independent reference vectors: numerical point-mass endpoint compared with
  the analytical solver and the existing Orekit 13.1.6 scenario result.
- Differential/scenario tests: harmonic oscillator analytic solution and
  point-mass analytical comparison.
- Tolerances and justification: polynomial error below 20 picometres and
  2 picometres/second; harmonic solution below 2 nanometres and
  2 nanometres/second; the point-mass scenario uses 0.1 mm,
  0.1 micrometre/second, and `1e-13` configured scales and stays within
  2 centimetres and 20 micrometres/second of the independently validated
  analytical endpoint.
- Benchmarks: deferred until representative numerical force assemblies exist.

## Completion checklist

- [x] Implementation and typed errors
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance recorded
- [x] Parity ledger updated
- [x] Relevant checks pass
- [x] Binding impact explicitly deferred until the Rust API stabilizes
