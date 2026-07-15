# ADR-0012: represent attitude with a closed state enum

- Status: Superseded by ADR-0034
- Date: 2026-07-05
- Supersedes: ADR-0011 attitude trait-object decision
- Affected parity rows: rotations and angular states; two-body propagation

## Context

ADR-0011 removed the attitude generic parameter but retained a borrowed
`dyn Attitude`. Orbital state already uses the closed `SpacecraftState` enum,
and there is no current requirement that makes attitude an open extension
point while orbit representations remain closed.

## Decision

1. Remove the `Attitude` trait and all `dyn Attitude` use.
2. Define `QuaternionAttitude` as the validated concrete representation of a
   framed quaternion orientation and body angular velocity.
3. Define the closed `AttitudeState` enum, initially with a `Quaternion`
   variant, mirroring `SpacecraftState` and its concrete representations.
4. `SpacecraftView` owns both its `SpacecraftState` and `AttitudeState`; it
   borrows only the time-independent `Spacecraft` definition.
5. Future attitude representations extend the enum and explicit conversion
   surface when a real use case requires them.

## Consequences

- The spacecraft physical state has one consistent closed-representation
  strategy for translation and rotation.
- Views are cloneable values without generic parameters, trait objects,
  allocation, or attitude lifetime coupling.
- Extending attitude is intentionally a core API change, matching orbital
  representation evolution.

## Validation

Task 0013 covers attitude construction, view ownership, propagation
preservation, adapters, and workspace checks.
