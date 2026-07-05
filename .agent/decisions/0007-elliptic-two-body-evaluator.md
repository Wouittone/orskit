# ADR-0007: begin dynamics evaluation with explicit elliptic two-body propagation

- Status: Accepted; generalized by ADR-0009 and recomposed by ADR-0011 and ADR-0013
- Date: 2026-07-04
- Owners: orskit maintainers
- Affected parity rows: two-body propagation; dynamics composition; orbits

## Context

ADR-0006 separated dynamics descriptions from evaluation and numerical
resolution. The first executable dynamics slice should validate that boundary
without making two-body motion the general solver abstraction. Existing core
states already provide validated elliptic Keplerian elements and explicit
conversion to Cartesian coordinates when a gravitational parameter is supplied.

## Decision

1. Add `EllipticTwoBodyPropagator` as one evaluator, not as a method on
   `SystemDynamics`. ADR-0009 subsequently places it behind the generic
   representation-preserving propagator contract; ADR-0011 later changes its
   input to an epoch-specific spacecraft view with a closed orbital-state enum.
2. Require an explicit positive `GravitationalParameter`. ADR-0009 moves it
   from propagator construction into `PointMassGravityModel`; body identity
   still never selects a constant implicitly.
3. Propagate `KeplerianState` analytically by advancing mean anomaly and solving
   the elliptic Kepler equation with bounded Newton iteration. Preserve frame,
   non-anomaly elements, and all spacecraft properties.
4. Support signed Hifitime durations and target epochs. Restrict this slice to
   the existing elliptic element regime; Cartesian universal-variable,
   parabolic, and hyperbolic propagation remain separate future work.
5. Validate against analytic invariants and offline Cartesian outputs generated
   independently by Orekit and Lox. A later policy amendment permits Nyx
   public-API black-box validation only; it does not alter this implementation
   decision or allow Nyx implementation material to inform orskit.

## Alternatives considered

- Put `propagate` on `SystemDynamics`: rejected because complicated dynamics
  will need state layouts, data contexts, derivative evaluation, integrator
  policy, events, and variational equations that this analytical case does not.
- Start with a numerical integrator: rejected because it adds truncation error
  and solver configuration before the force-evaluation contract exists.
- Infer Earth's `mu` from `Body::EARTH`: rejected because body identity is not a
  physical-data model and constants must identify their source/convention.
- Implement from Orekit, Lox, or Nyx source: prohibited. Only public equations
  and independently generated black-box outputs may inform this work.

## Consequences

- The project gains a useful first solution while keeping advanced dynamics and
  numerical resolution open.
- Callers with Cartesian initial states must convert or await a future
  universal-variable evaluator.
- Elliptic singularities retain the existing Keplerian representation policy.
- Differential fixtures remain offline and deterministic.

## Validation

Tests cover analytic circular behavior, orbital invariants, reverse-time
round trips, convergence/error behavior, and Cartesian comparison against
recorded independent outputs with physical tolerances.

## Provenance

Equations come from NASA GMAT mathematical documentation. Orekit and Lox are
used only to generate black-box output for the same declared initial condition.
Nyx was not consulted while making this implementation decision. A later
maintainer-approved, isolated public-API benchmark remains validation evidence
only and did not inform this design.
