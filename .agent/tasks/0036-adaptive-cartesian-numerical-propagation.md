# Task: implement adaptive Cartesian numerical propagation

## Parity target

- Ledger row: propagation / numerical integration and dense ephemerides
- Current status: Designed
- Intended status after this task: Partial, with an adaptive Cartesian
  propagator and internal dense steps; public ephemerides and events remain
  pending.

## User workflow

A caller selects an immutable evaluable Cartesian dynamics problem, supplies
typed position/velocity local tolerances and typed step bounds, and propagates
an epoch-qualified Cartesian orbit forward or backward to an absolute target.
The first concrete problem is central point-mass two-body motion.

## Scientific contract

- Inputs and units: Cartesian position in metres and velocity in metres per
  second; acceleration returned as metres per second squared; typed absolute
  `Length` and `Velocity` tolerances; dimensionless relative tolerance;
  `hifitime::Duration` step magnitudes.
- Outputs and units: `Orbit<CartesianState>` in the input frame at the exact
  requested `Epoch`; raw six-component SI arrays are private to the kernel.
- Frames/epochs/time scales: every stage state carries its reference frame;
  every evaluation receives a Hifitime epoch; acceleration carries and must
  match the stage frame. Point-mass evaluation requires explicitly inertial
  axes and a frame origin matching its gravity provider.
- Conventions and valid regimes: Bogacki--Shampine 3(2), advanced with the
  third-order result and controlled by the embedded second-order difference;
  non-stiff smooth translational Cartesian dynamics only. Cubic Hermite
  continuous output is valid on one accepted closed step.
- External data requirements: none for the included point-mass evaluator; the
  caller supplies the immutable sourced gravity provider.
- Errors and singularities: invalid tolerance/step configuration, model or
  provider failure, frame/origin mismatch, point-mass collision, non-finite
  duration/state/derivative/error, step underflow, minimum-step exhaustion,
  and step/rejection limits are typed recoverable errors.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| P. Bogacki and L. F. Shampine, *A 3(2) pair of Runge--Kutta formulas*, Applied Mathematics Letters 2(4), 1989, DOI 10.1016/0893-9659(89)90079-7 | Copyrighted primary numerical paper; equations and coefficients only | Four-stage embedded 3(2) tableau, higher-order local extrapolation, FSAL endpoint derivative | `crates/dynamics/numerical` kernel and order tests |
| [L. F. Shampine, I. Gladwell, and S. Thompson, *Solving ODEs with MATLAB*, Cambridge University Press, 2003, section 1.2](https://doi.org/10.1017/CBO9780511615542) | Copyrighted numerical textbook; equations and method description only | Component scaling, local-error acceptance, and cubic Hermite extension from endpoint values/slopes for BS 3(2) | Configuration/error controller, private dense step, manufactured tests |
| Existing Orekit 13.1.6 black-box two-body fixture | Previously approved behavior sample | Independent 3,600-second Cartesian endpoint for the existing elliptic scenario | Numerical two-body regression |

No external source code, test implementation, or distinctive prose was copied.

## Design

- Affected crates/layers: `CartesianDynamics` contract in `dynamics-core`;
  point-mass evaluator in `dynamics-two-bodies`; new safe-Rust
  `dynamics-numerical` implementation crate; feature-gated `dynamics` and
  `orskit` facade exports; workspace metadata and evidence documentation.
- Public API: `CartesianDynamics`, `IntegrationConfiguration`,
  `BogackiShampine32`, typed configuration/propagation errors, and the
  `TwoBodyDynamics` evaluator implementation.
- Rejected alternatives: a public `Vec<f64>` ODE solver; untyped tolerance
  arrays; fixed-step RK4 without an error estimate; public dense ephemerides
  before event semantics; silently accepting acceleration in another frame.
- ADR required: no new ADR; this implements the accepted ADR-0037 first slice.

## Validation

- Unit cases: configuration failures, zero duration, model source chaining,
  frame mismatch, minimum-step and attempt-limit exhaustion.
- Invariants/properties: constant-acceleration forward/backward recovery;
  rejected attempts do not advance accepted state; dense endpoint identity;
  two-body frame/origin validation.
- Independent reference vectors: recorded Orekit 13.1.6 two-body endpoint.
- Differential/scenario tests: numerical versus the independently implemented
  analytical two-body propagator over 3,600 seconds.
- Tolerances and justification: manufactured constant-acceleration tests use
  roundoff-scale budgets; observed-order refinement must produce the expected
  approximately eightfold third-order error reduction; the numerical
  two-body endpoint stays within 0.2 m and 0.2 mm/s of both analytical and
  recorded external endpoints under 1 mm, 1 micrometre/second, and `1e-11`
  local scaling. These are regression budgets, not global-error guarantees.
- Benchmarks: no performance claim; benchmark expansion waits for higher-order
  method selection or evidence that this POC is performance-sensitive.

## Completion checklist

- [x] Implementation and typed errors
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance recorded
- [x] Parity ledger updated
- [x] Relevant checks pass
- [x] Binding impact explicitly deferred; no binding files changed
