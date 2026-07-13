# ADR-0019: bind orbital elements to sourced gravity context

- Status: Accepted
- Date: 2026-07-13
- Affected parity rows: scientific data context; orbital representations;
  two-body propagation

## Context

Representation conversion accepted a naked gravitational parameter for every
source/target pair. It could silently use Earth gravity for a Mars-centered
state, discarded the parameter's provenance, and required an unrelated value
for identity and Keplerian/equinoctial-only conversions.

## Decision

1. `GravityContext` structurally binds frame origin, positive typed
   gravitational parameter, and normalized scientific source metadata.
2. Its opaque identity is derived from the complete structure and cannot be
   caller-forged. Equal structures compare equal; changing origin, parameter,
   or source changes identity.
3. Keplerian and equinoctial states retain that identity as the context under
   which their osculating elements are defined.
4. Cartesian-to-element conversion validates frame origin and binds the
   supplied context. Element-to-Cartesian conversion validates the stored
   identity against the supplied context.
5. Identity and Keplerian/equinoctial representation-only conversions do not
   accept gravity context. Remove the universal naked-parameter adapters.

## Consequences

- Sensitivity studies create a distinct context and explicitly reconvert
  rather than silently swapping a parameter under existing elements.
- Element/orbit values are cloneable rather than `Copy` because their identity
  retains structured provenance.
- Dynamics must own the same context it uses for conversion and evaluation.

## Validation

Tests cover matching contexts, wrong origins, changed parameter/source/origin
identity, and context-free element-only conversions while retaining existing
analytic vectors.

## Provenance

This is original orskit scientific-context architecture. Conversion equations
continue to use the public NASA GMAT reference already recorded by the project;
no external source or test implementation was consulted.
