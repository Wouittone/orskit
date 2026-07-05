# ADR-0011: separate spacecraft definition from epoch-specific views

- Status: Superseded in part by ADR-0012
- Date: 2026-07-05
- Supersedes: ADR-0010 spacecraft composition
- Affected parity rows: Cartesian states; rotations and angular states;
  two-body propagation

## Context

ADR-0010 made `Spacecraft<A>` generic over attitude and stored epoch, mass,
orbit, inertia, and attitude directly on it. Attitude was singled out as a
generic parameter without a coherent reason, and the object mixed permanent
spacecraft identity with quantities that may vary by epoch.

## Decision

1. `Spacecraft` is non-generic and contains only time-independent identity and
   body geometry. The initial geometry alternatives are point, sphere, and
   body-axis-aligned cuboid.
2. `SpacecraftView<'a>` borrows a `Spacecraft` and an `Attitude` trait object.
   It contains the epoch-specific mass, orbital state, inertia, and attitude.
3. The view validates positive finite mass and requires inertia and angular
   velocity to use the attitude body frame.
4. `Propagator<Model>` accepts and returns `SpacecraftView` values, preserving
   the borrowed spacecraft and attitude while changing only quantities its
   model advances.
5. Neither the spacecraft definition nor its view is generic over a physical
   representation.

## Consequences

- One spacecraft definition can have many views across epochs without
  duplicating identity or geometry.
- Mass depletion, articulated inertia, and attitude evolution can later
  produce new views without mutating the time-independent object.
- Borrowed views make the lifetime relationship explicit and avoid allocation
  or trait-object ownership policy in the physical core.

## Validation

Task 0012 covers definition/shape validation, view frame invariants, borrowed
identity preservation through propagation, bindings compilation, and full
workspace checks.
