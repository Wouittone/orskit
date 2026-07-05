# ADR-0010: compose spacecraft snapshots from orbit and attitude

- Status: Superseded by ADR-0011
- Date: 2026-07-05
- Supersedes: ADR-0004 state/property composition
- Affected parity rows: Cartesian states; Keplerian/equinoctial elements;
  rotations and angular states; two-body propagation

## Context

ADR-0004 duplicated epoch, mass, inertia, and orientation inside every orbital
representation. That made a coordinate representation pretend to be a whole
spacecraft and forced every conversion to copy unrelated rigid-body data.
Attitude also exposed only a rotation and omitted angular velocity.

## Decision

1. `CartesianState`, `KeplerianState`, and `EquinoctialState` contain their six
   characteristic orbital elements plus the frame required to interpret them.
2. `SpacecraftState` is the closed enum of those supported representations.
3. Standard `From` and the symmetric `To` trait wrap concrete states in the
   enum. Representation changes use standard `TryFrom` and symmetric `TryTo`
   with explicit `OrbitalConversion` context containing the gravitational
   parameter.
4. `OrbitalElements` lets any concrete state or `SpacecraftState` provide any
   supported representation without implying that all representations are
   stored.
5. `Attitude` exposes a framed orientation and framed angular velocity.
   `AttitudeState` is the initial immutable implementation.
6. `Spacecraft<A>` composes epoch, positive mass, `SpacecraftState`, inertia,
   and an attitude implementation. Its constructor validates rigid-body frame
   consistency.
7. Format ingestion continues to return timed coordinate samples when the
   source does not provide complete spacecraft data.

## Consequences

- Orbital conversion no longer carries epoch, mass, or rigid-body properties.
- The enum makes currently supported state alternatives explicit and easy to
  match while concrete values remain independently testable.
- New state representations require extending the enum and conversion matrix;
  that is intentional while the supported set is small and closed.
- Attitude models can vary behind a trait without weakening the spacecraft
  snapshot's physical invariants.

## Validation

Task 0011 records the conversion matrix, analytic and round-trip tests,
spacecraft construction checks, and workspace validation.
