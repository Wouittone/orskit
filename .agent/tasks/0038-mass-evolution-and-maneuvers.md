# Task: implement mass evolution and scheduled maneuvers

## Parity target

- Ledger row: propagation / maneuvers, mass, and finite/impulsive burns
- Current status: Not assessed
- Intended status after this task: Partial, with frame-explicit scheduled
  impulses, constant-frame finite thrust, exact constant-rate mass evolution,
  reverse execution, and an audit log.

## User workflow

A caller combines an epoch-qualified Cartesian orbit with positive spacecraft
mass, schedules a finite orbit-raise burn and an impulsive trim, propagates
them over caller-selected base dynamics, and inspects the final orbit, remaining
mass, and deterministic execution record.

## Scientific contract

- Inputs and units: typed `Mass`, `MassRate`, force components in
  `ThrustVector`, velocity components in `VelocityVector`, Hifitime epochs, and
  the P11 typed numerical configuration.
- Outputs and units: `CartesianMassState` retains `Orbit<CartesianState>` and
  positive mass; each execution records propagation-order start/end epochs and
  mass before/after.
- Frames/epochs/time scales: every maneuver declares the one propagated
  Cartesian frame. Finite intervals are chronological absolute epochs;
  execution follows forward or reverse propagation order.
- Conventions and valid regimes: impulses are instantaneous forward-time
  velocity/mass jumps whose inverse is used in reverse. Finite thrust is
  constant in the declared Cartesian frame, mass flow is constant and
  positive, `dm/dt = -q`, and stage acceleration adds `F/m(t)`. Finite burns
  cannot overlap; attitude and steering are not inferred.
- External data requirements: unchanged from the selected base dynamics.
  Maneuvers introduce no dataset or network behavior.
- Errors and singularities: blank/non-finite/invalid maneuver inputs,
  overlapping burns, frame mismatch, zero or negative mass, mass exhaustion,
  non-finite combined acceleration, base-provider failure, and numerical
  segment failure are typed. There is no dense-output claim across
  discontinuities.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| Dan M. Goebel and Ira Katz, [*Fundamentals of Electric Propulsion: Ion and Hall Thrusters*](https://descanso.jpl.nasa.gov/SciTechBook/series1/Goebel_02_Chap2_thruster.pdf), JPL Space Science and Technology Series, 2008, chapter 2 | Public US Government/JPL technical book | Thrust as force on instantaneous spacecraft mass; propellant consumption changes total mass; thrust and mass-flow relationship | `ConstantThrustManeuver`, stage acceleration, exact mass law, analytic validation |
| ADR-0037 and P11/P12 numerical implementation | Original project architecture and previously recorded numerical references | Typed segmented propagation, exact target boundaries, forward/reverse behavior, discontinuity caution | schedule execution and adaptive coast/burn segments |

Only equations and public physical definitions were consulted. No external
source code, tests, examples, or distinctive prose were copied. The analytic
one-dimensional validation was independently derived from `dv/dt = F/m(t)`
with `m(t) = m0 - q t`.

## Design

- Affected crates/layers: `units`; `dynamics-numerical`; feature-gated
  `dynamics` and `orskit` facade exports; architecture, parity, decision, and
  provenance evidence.
- Public API: `ThrustVector`, `CartesianMassState`, `ImpulsiveManeuver`,
  `ConstantThrustManeuver`, `ManeuverSchedule`, `ManeuverExecution`,
  `ManeuverExecutionKind`, `ManeuverPropagation`, and typed maneuver errors.
- Rejected alternatives: raw force arrays; constant acceleration that ignores
  mass depletion; implicit body-to-inertial steering; overlapping finite
  burns; P12 reset semantics without a discontinuity contract; one continuous
  dense ephemeris across impulses.
- ADR required: ADR-0038 records the scoped mass law, frame policy, boundary
  convention, reverse behavior, and deferred coupled-state features.

## Validation

- Unit cases: input validation, finite-burn overlap, wrong frame, mass
  exhaustion, initial-epoch impulse, and zero-duration identity.
- Invariants/properties: exact mass accounting; deterministic mixed execution
  order; impulsive inverse; finite-burn forward/reverse recovery.
- Independent reference vectors: the variable-mass straight-line equations
  from the cited JPL propulsion definitions supply a closed-form analytic
  velocity and independently integrated position.
- Differential/scenario tests: one-dimensional zero-base-dynamics burn
  compares numerical position/velocity against the analytic solution; a
  point-mass Earth example combines one finite and one impulsive maneuver.
- Tolerances and justification: the ten-second analytic burn uses strict
  `1e-9 m` and `1e-12 m/s` local absolute tolerances. Expected global
  differences are bounded to `2e-7 m` and `2e-8 m/s`; the reverse round trip
  uses tighter observed roundoff/error-controller budgets. Mass is evaluated
  by the exact linear law and compared at floating-point roundoff scale.
- Benchmarks: no performance claim; segmented schedule overhead remains a
  future profiling target.

## Completion checklist

- [x] Implementation and typed errors
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance recorded
- [x] Parity ledger updated
- [x] Relevant checks pass
- [x] Binding impact explicitly deferred; no binding files changed
