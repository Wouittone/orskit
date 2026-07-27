# Task: add dense Cartesian ephemerides and deterministic events

## Parity target

- Ledger row: Propagation / Numerical integration and dense ephemerides;
  Propagation / Events and root localization
- Current status: endpoint RKF45 propagation is `Partial`; events are `Not
  assessed`
- Intended status after this task: both are `Partial` with one translational
  Cartesian vertical slice

## User workflow

A caller generates an immutable, directional dense ephemeris from the existing
adaptive Cartesian propagator, queries a typed state anywhere in its closed
interval, and searches that trajectory with caller-owned scalar detectors.
Direction filtering, localization bounds, handler actions, and simultaneous
event order are explicit.

## Scientific contract

- Inputs and units: the existing frame-qualified `Orbit<CartesianState>`;
  target `Epoch`; typed root epoch tolerance; non-zero iteration/event limits;
  detector values are caller-defined finite signed scalars.
- Outputs and units: typed Cartesian orbit states and event occurrences. The
  dense kernel privately uses six SI components.
- Frames/epochs/time scales: all states retain the dynamics frame and Hifitime
  epoch. An `EphemerisInterval` preserves propagation direction. Event
  direction is defined with increasing physical epoch, independently of search
  direction.
- Conventions and valid regimes: accepted fifth-order RKF45 endpoints are
  joined by cubic Hermite polynomials using dynamics derivatives at both ends.
  This continuous extension is endpoint-consistent and fourth-order accurate
  on smooth trajectories (`O(h^4)` interpolation error); it does not strengthen
  the endpoint solver's global-error guarantee. Event search detects bracketed
  sign changes or exact endpoint zeros within each accepted step. Grazing roots
  and multiple even-count roots inside one step are outside this slice.
- External data requirements: exactly those owned by the caller's
  `CartesianDynamics`; none are loaded by dense output or event search.
- Errors and singularities: out-of-interval queries, invalid typed states,
  non-finite detector values, detector/handler sources, root-iteration
  exhaustion, event-limit exhaustion, and all existing numerical failures are
  typed. Every handler in a simultaneous group runs in detector-slice order
  before a `Stop` action terminates the immutable search. State resets are not
  supported.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| Lawrence F. Shampine, “Some Practical Runge-Kutta Formulas,” *Mathematics of Computation* 46(173), 1986, DOI 10.1090/S0025-5718-1986-0815836-3 | Primary numerical paper; published mathematical description | Continuous extensions permit output between accepted Runge--Kutta endpoints and require an explicit interpolation-order contract | `crates/dynamics/numerical/src/dense.rs`; dense-order test |
| Richard P. Brent, “An algorithm with guaranteed convergence for finding a zero of a function,” *The Computer Journal* 14(4), 1971, DOI 10.1093/comjnl/14.4.422 | Primary numerical paper | Maintaining a sign-changing bracket gives a guaranteed root-localization path; this slice deliberately uses bounded bisection only, not Brent interpolation | `crates/dynamics/numerical/src/dense.rs`; event-root tests |
| Erwin Fehlberg, NASA TR R-315, July 1969 | Primary US Government technical report; public use permitted | Existing accepted fifth-order RKF45 endpoint and embedded error estimate | `crates/dynamics/numerical/src/lib.rs`; ADR-0044 |

No source code, tests, examples, or distinctive implementation structure from
another astrodynamics library was consulted or copied. The Hermite polynomial,
bisection loop, tests, and event API are project-authored.

## Design

- Affected crates/layers: `dynamics-numerical`; `dynamics` facade re-export
  proposed in integration notes.
- Public API: `DenseEphemeris`, `EphemerisInterval`, dense query errors;
  `EventDetector`, direction/action/configuration, occurrence/outcome, and
  typed search errors; `AdaptiveRungeKuttaFehlberg::generate_ephemeris`.
- Rejected alternatives: unconstrained polynomial interpolation without
  derivative matching; exposing private RK stages; silently sampling detector
  functions on an arbitrary grid; unbounded root iterations; mutable
  reset-state handlers over an already-generated ephemeris; claiming grazing
  detection.
- ADR required: ADR-0047.

## Validation

- Unit cases: exact dense endpoints, outside-interval query, invalid event
  configuration, detector failure, non-finite detector output, root/event
  limits, handler stop.
- Invariants/properties: interval and frame retention; exact accepted
  endpoints; deterministic detector-index order for simultaneous roots;
  physical-time direction is invariant under forward/backward generation.
- Independent reference vectors: analytic constant-acceleration position and
  velocity throughout forward and backward intervals.
- Differential/scenario tests: harmonic oscillator step refinement exhibits
  the expected fourth-order dense interpolation trend; linear forward/backward
  crossings have analytic root epochs.
- Tolerances and justification: analytic dense states use picometre-scale
  floating-point budgets; event roots use a typed one-nanosecond bracket width.
  Bisection's final epoch error is at most the retained bracket width.
- Benchmarks: deferred; this slice makes no performance claim.

## Completion checklist

- [x] Implementation and typed errors
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance recorded
- [x] Parity ledger update proposed in integration notes
- [x] Relevant focused checks pass
- [x] Binding impact explicitly deferred until the Rust event API gains
  reset-state semantics and broader detector evidence
