# Task: propagate every state representation with an explicit force model

- Status: Superseded in part by task 0014

## Parity target

- Ledger row: Propagation / Two-body/Keplerian propagation
- Current status: Partial
- Intended status after this task: Partial with representation-independent
  point-mass propagation evidence

## User workflow

Select an explicit point-mass gravity model, propagate a Cartesian, Keplerian,
or equinoctial state through the same `Propagator<State, ForceModel>` contract,
and receive the result in the input state's native representation.

## Scientific contract

- Inputs and units: complete state, signed Hifitime duration or target epoch,
  and `PointMassGravityModel` containing an attractor and positive typed `mu` in
  `m^3/s^2`.
- Outputs and units: a complete state of the same Rust/native representation at
  the requested epoch.
- Frames/epochs/time scales: coordinates must share one frame whose origin is
  the modeled attractor; no frame or time-scale transformation occurs.
- Conventions and valid regimes: autonomous Newtonian point-mass motion in the
  existing elliptic regime. Cartesian conversion uses osculating classical
  elements with explicit circular/equatorial conventions.
- External data requirements: none after the caller constructs the model; body
  identity never selects `mu` implicitly.
- Errors and singularities: mismatched Cartesian component frames, known
  non-inertial axes, degenerate or non-elliptic Cartesian states,
  model/frame-origin mismatch, invalid solver configuration, and iteration
  failure are typed errors.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| NASA GMAT Mathematical Specifications (2007) | US Government technical documentation | Elliptic anomaly evolution | `crates/dynamics/src/two_body.rs` |
| NAIF CSPICE `oscltx_c` public API documentation | US Government API documentation | State/element inputs, singular conventions, inertial-frame restriction, inverse sanity-check policy | `crates/core/src/state.rs` |
| Orekit 13.1.6 `KeplerianPropagator` black-box output | Apache-2.0 public behavior | Independent Cartesian endpoint | differential tests |
| Lox 0.1.0-alpha.39 `Vallado` black-box output | MPL-2.0 public behavior | Independent Cartesian endpoint | differential tests |
| Nyx 2.3.1 public API/black-box output | AGPL-3.0-or-later; validation-only use approved later by the project owner | No implementation material used; later output did not meet the shared Cartesian tolerance | Historical implementation remained independent; later evidence is recorded in task 0010 |

No reference implementation source, tests, examples, or internal structure are
copied or translated.

## Design

- Affected crates/layers: `orskit-core`, `orskit-dynamics`, reference evidence,
  parity/provenance/architecture documentation.
- Public API: object-safe fixed-pair `Propagator<State, Model>` trait;
  `PointMassGravityModel` with explicit `mu`; Cartesian-to-Keplerian and
  Cartesian-to-equinoctial `StateConversion` implementations.
- Rejected alternatives: propagation methods on `State`; a closed state/model
  enum; returning Cartesian regardless of input; model downcasting; implicit
  body constants; claiming general multi-force numerical propagation.
- ADR required: yes, ADR-0009.

## Validation

- Unit cases: signed duration, target epoch, model/frame mismatch, invalid
  Cartesian regimes, circular/equatorial conversion conventions.
- Invariants/properties: native output type, epoch/properties preservation,
  cross-representation Cartesian agreement, forward/backward recovery.
- Independent reference vectors: the recorded non-circular inclined Earth case
  from Orekit and Lox, each with its explicit `mu`.
- Differential/scenario tests: Cartesian endpoints from all three input/output
  representations compared to each independent reference.
- Tolerances and justification: retain the recorded 1 micrometre position and
  1 nanometre/second velocity reference tolerances; conversion round trips use
  separately documented floating-point bounds.
- Benchmarks: the reproducible three-implementation Cartesian endpoint harness
  and local measurements are tracked separately in task 0010.

## Completion checklist

- [x] Implementation and typed errors
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance recorded
- [x] Parity ledger updated
- [x] Relevant checks pass
- [x] Binding impact handled or explicitly deferred: bindings remain disabled
  and do not expose the pre-alpha dynamics API
