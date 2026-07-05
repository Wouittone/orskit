# Task 0014: separate translational propagation from complete spacecraft views

## Parity target

- Ledger row: Propagation / Two-body/Keplerian propagation
- Current status: Partial
- Intended status after this task: Partial with an orbit-only propagation
  contract that does not imply unmodeled attitude evolution

## User workflow

Construct an epoch-qualified `Orbit`, propagate it with an explicit force model,
then independently compose the resulting orbit with physical properties that
are valid at the target epoch when a complete `SpacecraftView` is required.

## Scientific contract

- Inputs and units: `Orbit`, signed Hifitime duration or target epoch, and an
  explicit `PointMassGravityModel`.
- Outputs and units: an `Orbit` at the requested epoch, preserving the native
  Cartesian, Keplerian, or equinoctial representation.
- Frames/epochs/time scales: unchanged from task 0009.
- Conventions and valid regimes: autonomous elliptic point-mass translation.
- External data requirements: none.
- Errors and singularities: unchanged from the existing two-body evaluator.

## Provenance

No new scientific source is required. This task corrects an original orskit
domain boundary; equations and reference vectors remain those recorded for
task 0009.

## Design

- Affected crates/layers: `orskit-core`, `orskit-dynamics`, benchmark and
  architecture documentation.
- Public API: add `Orbit { epoch, state }`; make `Propagator<Model>` consume and
  return `Orbit`; make `SpacecraftView` compose an `Orbit` and remove its
  orbit-only replacement helper.
- Rejected alternatives: silently copying attitude to a new epoch; rejecting
  nonzero angular velocity only; making the analytical translational
  propagator responsible for attitude dynamics.
- ADR required: ADR-0013.

## Validation

- Unit cases: duration and target-epoch propagation, variant preservation.
- Invariants/properties: orbital invariants and signed-duration round trip.
- Independent reference vectors: unchanged Orekit and Lox endpoints.
- Differential/scenario tests: all three orbital representations.
- Tolerances and justification: unchanged from task 0009.
- Benchmarks: retain the same orbit-query workload without constructing
  unrelated rigid-body data.

## Completion checklist

- [x] Implementation and typed errors
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance recorded
- [x] Parity ledger updated
- [x] Relevant checks pass
- [x] Binding impact explicitly deferred; bindings do not expose propagation
