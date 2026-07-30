# ADR-0038: schedule explicit mass-qualified maneuvers

- Status: Accepted
- Date: 2026-07-30
- Owners: orskit maintainers
- Extended by: ADR-0040 prescribed attitude and body-fixed finite thrust
- Affected parity rows: maneuvers, mass, and finite/impulsive burns;
  numerical integration and dense ephemerides

## Context

P11 and P12 provide adaptive Cartesian propagation, dense output, and
continue/stop event handlers. They deliberately do not define a generic
coupled state layout or event-driven state reset. P15 needs a useful maneuver
workflow without implying that attitude steering, generic mass dynamics, or a
seven-component error-controlled state already exists.

Spacecraft mass is epoch-dependent and must stay positive. Finite thrust
acceleration depends on instantaneous mass, while an impulse creates explicit
velocity and mass discontinuities. Maneuver vectors also require a declared
frame; silently treating a body-fixed vector as inertial would be incorrect.

## Decision

1. The first maneuver slice uses `CartesianMassState`, which composes an
   epoch-qualified Cartesian orbit with a finite, strictly positive typed
   mass. It is a workflow state rather than a replacement for
   `SpacecraftView`.
2. `ImpulsiveManeuver` declares an epoch, reference frame, typed
   delta-velocity, and typed propellant consumption. These are independent
   caller-selected inputs; this slice does not infer propellant from specific
   impulse. Forward propagation applies the jump, and reverse propagation
   applies its exact inverse.
3. `ConstantThrustManeuver` declares a chronological interval, a constant
   force vector in the propagated Cartesian frame, and a positive constant
   mass-flow magnitude. During the burn,
   `m(t) = m(reference) - mass_flow * (t - reference)` and the numerical
   translational stages add `thrust / m(t)` to the selected base dynamics.
   Linear mass evolution is exact for this model and is not assigned a
   fabricated numerical mass tolerance.
4. Finite burns may touch at an endpoint but may not overlap. Impulses may
   occur during a burn and split its execution into auditable arcs.
   Simultaneous impulses preserve registration order.
5. Maneuver boundaries split propagation into ordinary adaptive Cartesian
   segments. A schedule returns the final mass-qualified state plus a
   propagation-ordered execution log. It does not claim a discontinuous dense
   ephemeris.
6. A non-zero arc beginning or ending at an impulse interprets the initial
   state as pre-impulse when moving forward and post-impulse when moving
   backward. A zero-duration request is identity.
7. Every scheduled maneuver must use the propagated frame. Mass exhaustion,
   frame disagreement, invalid schedules, non-finite inputs, base-dynamics
   errors, and numerical failures remain typed and source-preserving.
8. Attitude-resolved/body-fixed thrust, throttle or steering laws,
   detector-triggered reset handlers, dry-mass/tank models, overlapping thrust
   composition, and dense mass ephemerides remain later capabilities.

## Alternatives considered

- Generalize the numerical kernel immediately to an open coupled state:
  rejected for this slice because attitude, mass tolerances, variational
  groups, and reset semantics still need their own contracts.
- Represent a finite maneuver as constant acceleration: rejected because it
  hides the acceleration change caused by propellant consumption.
- Infer inertial thrust from a body-frame vector without attitude: rejected
  because the transformation is physically undefined.
- Reuse P12 stop handlers as state resets: rejected because P12 intentionally
  has no discontinuity/reinitialization contract.
- Retain one dense ephemeris across impulses: rejected because a single
  continuous interpolant cannot represent the velocity and mass jumps.

## Consequences

The POC supports meaningful orbit changes under caller-selected base dynamics,
exact constant-rate mass depletion, scheduled impulses, reverse propagation,
and deterministic audit output. Its deliberate model bounds remain visible in
the type names and documentation. A later generic coupled-state design can
reuse the public maneuver descriptions while replacing the segmented
execution kernel.

## Validation

Tests compare a one-dimensional constant-thrust burn against the closed-form
variable-mass velocity and position, exercise forward/reverse finite and
impulsive round trips, verify mixed burn/impulse order, check initial-boundary
and zero-duration policy, and cover overlap, frame, and mass-exhaustion
failures. A point-mass example demonstrates a finite orbit-raise burn followed
by a trim impulse.

## Provenance

Dan M. Goebel and Ira Katz, *Fundamentals of Electric Propulsion: Ion and Hall
Thrusters*, JPL Space Science and Technology Series, 2008, chapter 2,
documents thrust as spacecraft force, instantaneous-mass acceleration, and
propellant mass-rate evolution. Only equations and physical definitions were
consulted; no implementation, tests, examples, or distinctive prose were
copied.
