# Task: expose dense Cartesian ephemerides and bracketed events

## Parity target

- Ledger rows: propagation / numerical integration and dense ephemerides;
  propagation / events and root localization
- Current status: numerical propagation `Partial`; events `Not assessed`
- Intended status after this task: both `Partial`, with public accepted-step
  dense output, bracketed direction-aware events, deterministic dispatch, and
  stop handlers. Reset handlers and unbracketed grazing detection remain
  pending.

## User workflow

A caller numerically propagates one frame-qualified Cartesian orbit, evaluates
the retained accepted-step continuous extension at any covered epoch, registers
heterogeneous dimensionless event detectors, receives localized occurrences in
deterministic propagation order, and can continue or stop through one handler.

## Scientific contract

- Inputs and units: P11 Cartesian states and integration settings; event
  maximum-check and root tolerances are `hifitime::Duration`; each detector
  owns its physical normalization and returns a finite dimensionless `Ratio`.
- Outputs and units: immutable `CartesianEphemeris` samples remain
  `Orbit<CartesianState>` in the propagated frame; occurrences retain detector
  identity, epoch, Cartesian state, and propagation-order crossing direction.
- Frames/epochs/time scales: every interpolated state retains the one input
  frame; segment and occurrence epochs use Hifitime; reverse arcs expose
  chronological coverage while preserving reverse propagation order.
- Conventions and valid regimes: cubic Hermite output is valid only on retained
  accepted steps. Bisection localizes one bracketed sign change per detector
  and maximum-check interval. Rising/falling is defined as propagation
  advances, so it reverses for a reverse arc. Roots within the configured time
  tolerance are simultaneous and dispatch by detector registration index.
- External data requirements: unchanged from the selected
  `CartesianDynamics`; event evaluation introduces none.
- Errors and singularities: outside coverage, non-finite interpolation or
  switching values, blank detector names, detector/handler failures,
  root-iteration exhaustion, and event-limit exhaustion are typed. A stop
  truncates dense coverage at the first simultaneous root in propagation
  order. State reset is not supported.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| P11 Bogacki--Shampine and *Solving ODEs with MATLAB* references recorded in task 0036 | Previously recorded primary paper/textbook | Accepted-step endpoint values/slopes and cubic Hermite continuous extension | Public ephemeris and detector state evaluation |
| ADR-0037 and existing orskit numerical policy | Original project architecture | Typed dimensionless switching values, bracketed localization, direction, propagation-time ordering, deterministic simultaneous policy | Event contracts and tests |

Root localization uses independently written bounded bisection. No new
external source code, test implementation, dataset, or scientific model was
consulted.

## Design

- Affected crates/layers: `dynamics-numerical`; feature-gated `dynamics` and
  `orskit` facade exports; architecture and evidence documentation.
- Public API: `CartesianEphemeris`, `DensePropagation`, `DenseOutputError`,
  `EventDetector`, `EventHandler`, `EventConfiguration`, `EventDirection`,
  `EventAction`, `EventOccurrence`, `EventPropagation`, and event error
  variants on `NumericalPropagationError`.
- Rejected alternatives: reintegration for arbitrary epochs; raw state arrays;
  event values with implicit physical units; open-ended root iteration;
  processing only the first simultaneous detector; state reset without a
  discontinuity/reinitialization contract.
- ADR required: no new ADR; this executes the accepted P12 decisions left open
  by ADR-0037 and records the handler choice in this task.

## Validation

- Unit cases: invalid event configuration, outside dense coverage, blank/error
  and non-finite detectors, handler failure, root-iteration exhaustion, and
  event-limit exhaustion.
- Invariants/properties: dense endpoint identity; exact quadratic interior;
  reverse chronological coverage; one report for a shared step boundary;
  stopped coverage truncation.
- Independent reference vectors: P11 analytical/Orekit two-body endpoint
  remains in the shared integrator path; no new physical model is introduced.
- Differential/scenario tests: exact inertial motion supplies known position
  and epoch roots; forward/reverse direction and ordering are checked.
- Tolerances and justification: constant-acceleration dense samples use
  roundoff-scale budgets. Event roots use explicit time tolerances; analytic
  stop tests require the localized epoch/state within 1 nanosecond and 1
  nanometre respectively.
- Benchmarks: no performance claim; ephemeris/event allocation and evaluation
  benchmarks remain future work if profiles identify a bottleneck.

## Completion checklist

- [x] Implementation and typed errors
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance reviewed; no new external material
- [x] Parity ledger updated
- [x] Relevant checks pass
- [x] Binding impact explicitly deferred; no binding files changed
