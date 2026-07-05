# ADR-0013: propagate epoch-qualified orbits, not complete spacecraft views

- Status: Accepted
- Date: 2026-07-05
- Supersedes: ADR-0011 propagation decision
- Affected parity rows: orbital states; two-body propagation; attitude

## Context

`SpacecraftView` states that its orbit, mass, inertia, and attitude are valid at
one epoch. The translational two-body propagator advanced that epoch and orbit
while copying attitude unchanged. For a nonzero angular velocity, the result
was a type-valid but physically stale complete view.

## Decision

1. Introduce `Orbit` as an epoch plus the closed `SpacecraftState` enum.
2. `Propagator<Model>` accepts and returns `Orbit`, preserving its native state
   variant. Translational propagation makes no claim about mass, inertia, or
   attitude at the target epoch.
3. `SpacecraftView` composes an `Orbit` with mass, inertia, and attitude that
   callers assert are valid at the same epoch.
4. Remove the view helper that replaced only epoch and orbit while preserving
   all other epoch-dependent values.
5. Future coupled propagators may return complete views only when their state
   contract and models actually evolve every included quantity.

## Consequences

- A translational result cannot masquerade as a fully propagated spacecraft.
- Orbit propagation no longer borrows spacecraft identity or allocates
  unrelated rigid-body values.
- Workflows must explicitly obtain or propagate attitude and other physical
  properties before composing a target-epoch `SpacecraftView`.
- The public pre-alpha constructor and propagator APIs change deliberately.

## Validation

Existing point-mass invariant, reverse-time, representation, and independent
endpoint tests now operate on `Orbit`. Core view tests verify explicit orbit
composition.

## Provenance

This is original orskit domain architecture. No external implementation or
reference design informed the decision.
