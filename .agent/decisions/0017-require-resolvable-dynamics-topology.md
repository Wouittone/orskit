# ADR-0017: require resolvable dynamics topology

- Status: Accepted
- Date: 2026-07-13
- Affected parity rows: dynamics composition; third-body propagation

## Context

`TwoBodyDynamics` and `ThreeBodyDynamics` named physical topologies while also
allowing arbitrary force models to be appended. The three-body description did
not identify ephemerides, frames, restricted/full equations, or an acceleration
assembly contract. Construction therefore certified names that the stored data
could not preserve or evaluate.

## Decision

1. `TwoBodyDynamics` owns exactly one central `PointMassGravityModel` and does
   not expose additive force-model builders.
2. Remove the incomplete `ThreeBodyDynamics` API. Third-body dynamics return
   only after typed ephemeris, frame, data-provenance, and evaluation contracts
   exist.
3. Retain the open force/model description traits for future, honestly named
   composition APIs; they are not a license to weaken concrete topology names.

## Consequences

- A `TwoBodyDynamics` value always means the spacecraft plus one central point
  mass.
- Callers cannot construct a plausible but scientifically unresolved
  three-body description.
- Future general composition should use a topology-neutral name and must carry
  the providers required to evaluate every model.

## Validation

Tests verify that construction preserves and exposes exactly one central model
and that no additional conservative or non-conservative collection is present.

## Provenance

This is original orskit boundary design. No third-party implementation or test
was consulted.
