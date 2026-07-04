# ADR-0006: separate dynamics description from evaluation and resolution

- Status: Accepted
- Date: 2026-07-04
- Owners: orskit maintainers
- Affected parity rows: dynamics composition; propagation; force models

## Context

ADR-0002 removed a premature two-body orbit crate because advanced dynamics
must eventually cover composed forces, coupled translational/rotational/mass
states, events, integration, and variational equations. The project is now
ready to establish the first boundary, but the state-vector, data-provider,
derivative, and numerical-resolution contracts are not yet designed.

Force composition and system topology can be modeled independently from those
later choices. Doing so allows simplified two- and three-body configurations to
be ordinary implementations rather than the root abstraction.

## Decision

1. Add an `orskit-dynamics` crate with a description-only `SystemDynamics`
   trait. It exposes participant identity and an ordered collection of force
   models, but no derivative or propagation method.
2. Define an open `ForceModel` trait whose associated participant type and
   `ForceInteraction` declare source/target roles. Store plug-ins behind shared
   immutable trait-object handles.
3. Keep the participant type associated rather than fixing it to celestial
   bodies. The initial simplified models use `Body`; future coupled spacecraft
   or estimation systems may use richer participant identities.
4. Implement `TwoBodyDynamics` and `ThreeBodyDynamics` as peer implementations
   of `SystemDynamics`. Each begins with a mutual point-mass gravity
   description and may compose additional force descriptions in deterministic
   declaration order.
5. Validate that simplified-system participants are distinct and that every
   plugged force source/target belongs to the containing system.
6. Defer force evaluation, state derivatives, epochs, frames, external model
   data, integration, propagation, events, and variational equations. Their
   future contract will consume descriptions rather than being embedded in
   them.

## Alternatives considered

- Put `derivative(epoch, state)` directly on `SystemDynamics`: rejected for now
  because the state, frame compatibility, data context, derivative layout, and
  error contract would be guessed prematurely.
- Make two-body dynamics the base trait and extend it for additional bodies:
  rejected because it repeats the architecture removed by ADR-0002.
- Use a closed force-model enum: rejected because downstream and future
  project force models must remain pluggable.
- Let force models introduce undeclared participants: rejected because it makes
  the system boundary impossible to inspect or validate.

## Consequences

- Applications can assemble and inspect dynamics topology now without a fake
  solver or placeholder numerical output.
- Force-model ordering is explicit and stable, ready for a future documented
  accumulation policy.
- The traits do not yet claim that a described system can be evaluated or
  propagated.
- A future evaluation API can add typed state/data contracts separately without
  changing what a system description means.

## Validation

Tests exercise both simplified implementations through `SystemDynamics`, plug
in a custom force model, preserve declaration order, and reject duplicate or
external bodies. No floating-point validation is claimed because this slice
does not evaluate equations.

## Provenance

This is an original architecture decision based on maintainer direction and
ADR-0002. No third-party dynamics implementation or tests were consulted.
