# ADR-0009: use a representation-preserving state/model propagator contract

- Status: Superseded in part by ADR-0010 and ADR-0013
- Date: 2026-07-04
- Owners: orskit maintainers
- Affected parity rows: two-body propagation; state representations; dynamics
  composition

## Context

The first analytical evaluator in ADR-0007 is an inherent method limited to
`KeplerianState` and owns body/`mu` configuration separately from the
`PointMassGravityModel`. The state layer now has Cartesian, Keplerian, and
equinoctial representations, while ADR-0008 makes force models the explicit
implementation boundary.

A common propagation contract must preserve native state representation and
remain extensible across both state and force-model types without adding
propagation to `State` or inspecting concrete models at runtime.

## Decision

1. Define `Propagator<S, M>` where `S: State` and `M: ForceModel`. For a fixed
   state/model pair it is object-safe, returns `S`, and supports signed duration
   and target-epoch propagation.
2. Pass the force model explicitly to each propagation call. The propagator
   owns numerical policy; the model owns physical configuration.
3. Store the explicit positive gravitational parameter in
   `PointMassGravityModel` beside its attractor. Body identity never selects a
   constant.
4. Implement the elliptic analytical propagator for Cartesian, Keplerian, and
   equinoctial states with `PointMassGravityModel`. Preserve the caller's native
   representation, epoch semantics, mass, orientation, and inertia.
5. Reuse explicit `StateConversion` boundaries rather than embedding one state
   representation inside another. Add Cartesian-to-Keplerian/equinoctial
   conversions with documented elliptic and singularity conventions.
6. Reject a state whose coordinate-frame origin is not the point-mass
   attractor. Do not perform an implicit frame transform.
7. Keep this trait distinct from `SystemDynamics`: one-model analytical
   propagation does not define derivative accumulation, integration, events,
   or arbitrary composed-force resolution.

## Consequences

- One call shape works for every existing state representation and the existing
  evaluable force model.
- Future propagators can implement only scientifically valid state/model pairs;
  unsupported combinations fail at compile time rather than by downcast.
- Cartesian input remains restricted to bound elliptic motion in this
  propagator even though Cartesian coordinates can represent other conics.
- `PointMassGravityModel`, `TwoBodyDynamics`, `ThreeBodyDynamics`, and the first
  propagator receive deliberate pre-alpha constructor/API changes.
- General composed-force numerical propagation remains a future vertical slice.

## Alternatives considered

- Put `propagate` on `State`: rejected because representation is data, not a
  dynamics algorithm or physical model.
- Use `dyn State`/`dyn ForceModel` with downcasts: rejected because behavior
  would depend on runtime model specifics and lose native output typing.
- Use a closed state/model enum: rejected because it blocks downstream models.
- Return only Cartesian state: rejected because it erases native representation
  and repeats the state/coordinate confusion removed earlier.
- Keep `mu` on the propagator: rejected because the force model would not fully
  identify the physical point-mass configuration used by propagation.

## Validation

Tests exercise all three state/model trait implementations, Cartesian
conversion singularities, native-type/property preservation, cross-
representation agreement, and the existing Orekit/Lox black-box endpoints.

## Provenance

The contract is original orskit architecture. Cartesian conversion conventions
are checked against public NAIF CSPICE behavior documentation; propagation is
validated through recorded Orekit and Lox black-box outputs. A later isolated
Nyx public-API comparison is validation-only and did not inform this contract.
