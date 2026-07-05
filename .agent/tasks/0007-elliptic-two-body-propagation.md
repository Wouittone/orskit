# Task: implement the first elliptic two-body solution

## Parity target

- Ledger row: Propagation / Two-body/Keplerian propagation
- Current status: Not assessed
- Intended status after this task: Partial

## User workflow

Construct a simple point-mass two-body propagator with an attracting body and
explicit gravitational parameter, propagate a complete elliptic Keplerian
spacecraft state by a signed duration or to a target epoch, and convert the
result to Cartesian coordinates for comparison with independent tools.

## Scientific contract

- Inputs and units: `KeplerianState`, positive typed gravitational parameter in
  `m^3/s^2`, and Hifitime `Duration` or target `Epoch`.
- Outputs and units: a new `KeplerianState` at the requested epoch; semi-major
  axis, eccentricity, inclination, right ascension, argument of periapsis, and
  spacecraft properties are preserved while true anomaly advances.
- Frames/epochs/time scales: the input coordinate frame is preserved. Duration
  arithmetic uses Hifitime; no time-scale conversion or frame transform occurs.
- Conventions and valid regimes: osculating elliptic elements (`a > 0`,
  `0 <= e < 1`) and true anomaly, matching the existing core state contract.
  Mean anomaly advances at `sqrt(mu/a^3)` and Kepler's elliptic equation is
  solved by bounded Newton iteration.
- External data requirements: the caller supplies `mu`; no body constant is
  inferred from the attracting-body identity.
- Errors and singularities: non-finite duration and failure to converge within
  the documented iteration limit are typed errors. Invalid elements remain
  rejected by `KeplerianCoordinates`.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| NASA GMAT Mathematical Specifications, 2007 | US Government technical documentation | Elliptic mean/eccentric/true anomaly relations and mean motion | `crates/dynamics/src/two_body.rs` |
| Orekit 13.1.6 `KeplerianPropagator` public API/black-box output | Public behavior documentation; Apache-2.0 project | Independent propagation output only | reference fixture and comparison test |
| Lox `Vallado` in `lox-space` 0.1.0-alpha.39 public API/black-box output | Public behavior documentation; MPL-2.0 package | Independent Cartesian propagation output only, with its built-in Earth gravitational parameter recorded explicitly | reference fixture and comparison test |
| Nyx 2.3.1 public API/black-box output | AGPL-3.0-or-later; validation-only use approved later by the project owner | No implementation material used; later output did not meet the shared Cartesian tolerance | Historical implementation remained independent; later evidence is recorded in task 0010 |

No source, tests, examples, or internal structure are copied from any reference
implementation.

## Design

- Affected crates/layers: `orskit-dynamics`, `orskit-core` public state usage,
  provenance/parity/architecture documentation, and offline comparison data.
- Public API: `EllipticTwoBodyPropagator` and `TwoBodyPropagationError`.
- Rejected alternatives: a dynamics-wide solver trait in this first slice;
  implicit body `mu`; numerical integration for an analytically solvable case;
  Cartesian universal-variable support before its broader conic contract is
  designed; embedding propagation into `StateConversion`.
- ADR required: yes, ADR-0007.

## Validation

- Unit cases: circular half-period, full-period identity, signed-duration round
  trip, invariant elements/properties, target-epoch equivalence, and invalid
  duration/convergence behavior.
- Invariants/properties: specific orbital elements other than anomaly remain
  unchanged; forward then backward propagation recovers the initial Cartesian
  state within a stated physical tolerance.
- Independent reference vectors: one non-circular inclined Earth orbit compared
  in Cartesian metres and metres per second with Orekit and Lox outputs.
- Differential/scenario tests: offline Orekit and Lox fixture tests. A later,
  isolated Nyx black-box run is recorded in task 0010 with its failed Cartesian
  accuracy result; it is not a regression fixture or implementation input.
- Tolerances and justification: anomaly solver residual at most `1e-13 rad`;
  reference Cartesian tolerance set from observed independent-tool agreement
  plus f64 conversion error and recorded beside the fixture.
- Benchmarks: deferred until multiple propagator approaches exist or profiling
  identifies this scalar solution as material.

## Completion checklist

- [x] Implementation and typed errors
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance recorded
- [x] Parity ledger updated
- [x] Relevant checks pass
- [x] Binding impact handled or explicitly deferred: bindings remain disabled
  while the Rust contract is partial
