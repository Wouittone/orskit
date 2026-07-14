# ADR-0025: separate propagation method from physical problem

- Status: Superseded by ADR-0030
- Date: 2026-07-13
- Affected parity rows: dynamics composition; two-body propagation; future
  numerical propagation

## Context

The propagator trait was bounded by `ForceModel`, so an entire composed system
had to masquerade as one force. Solver names also encoded two-body topology,
conflating the physical problem with the method used to solve it. Positional
boolean state dependencies could not express velocity vectors, mass, or angular
velocity cleanly.

## Decision

1. `Propagator<Problem>` is object-safe for a fixed problem and has no
   `ForceModel` bound. A problem owns physical configuration; an implementation
   owns solution policy.
2. `TwoBodyDynamics` is the explicit current problem. The analytical
   `EllipticKeplerPropagator` implements `Propagator<TwoBodyDynamics>`.
3. Method names do not encode body topology. Future numerical or stochastic
   implementations can target compatible problem types through the same entry
   contract; this task implements none of those future methods.
4. `PointMassGravityModel` and `TwoBodyDynamics` share an application-extensible
   `Arc<dyn CentralGravity>` with element states.
5. Replace positional dependency booleans with opaque composable
   `SpacecraftStateRequirements` flags for position, velocity, mass, attitude,
   angular velocity, and inertia.
6. Three-/n-body problems remain unavailable until their provider/evaluation
   contracts exist; the generic solver boundary does not fabricate them.

## Consequences

- One call shape supports multiple solver implementations without weakening
  the scientific problem type.
- The current analytical method cannot be accidentally applied to an
  incompatible dynamics problem.
- New state requirements extend without positional constructor churn.

## Validation

Tests cover trait-object use for a fixed two-body problem, shared gravity
identity, representation preservation, and named requirement composition.

## Provenance

This is original orskit API architecture. Existing analytical reference vectors
remain unchanged; no external implementation or tests informed the boundary.
