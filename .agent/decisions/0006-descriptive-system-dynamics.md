# ADR-0006: separate dynamics description from evaluation and resolution

- Status: Superseded by ADR-0017
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
   trait. It exposes separate ordered collections of conservative and
   non-conservative force models, but no derivative or propagation method.
2. Define an open `ForceModel` contract and conservative/non-conservative model
   subtraits. Store plug-ins behind shared immutable trait-object handles.
   ADR-0008 subsequently separates physical `Force` identity from these model
   implementations and gives the model subtraits explicit `ForceModel` names.
3. Make the spacecraft the sole interaction target. A force declares whether
   it needs spacecraft position, speed, orientation, and inertia through
   `SpacecraftStateDependencies`; environmental bodies and other parameters are
   force-model configuration, not interaction participants.
4. Implement `TwoBodyDynamics` and `ThreeBodyDynamics` as peer implementations
   of `SystemDynamics`. They describe a spacecraft under one or two point-mass
   attractors respectively and may compose additional force-model descriptions
   in deterministic declaration order within each force class.
5. Reject a three-body description that repeats its attractor.
6. Defer general force evaluation, state derivatives, external model data,
   integration, events, and variational equations. ADR-0007 subsequently adds
   a narrow analytical elliptic two-body evaluator that consumes this
   description without defining the general resolution contract.

## Alternatives considered

- Put `derivative(epoch, state)` directly on `SystemDynamics`: rejected for now
  because the state, frame compatibility, data context, derivative layout, and
  error contract would be guessed prematurely.
- Make two-body dynamics the base trait and extend it for additional bodies:
  rejected because it repeats the architecture removed by ADR-0002.
- Use a closed force-model enum: rejected because downstream and future
  project force models must remain pluggable.
- Model force interactions as arbitrary source/target participant graphs:
  rejected because the propagated spacecraft is the interaction input; bodies,
  atmospheres, radiation sources, and other environment belong to force-model
  configuration and explicit future data providers.

## Consequences

- Applications can assemble and inspect dynamics topology now without a fake
  solver or placeholder numerical output.
- Conservative/non-conservative classification and ordering are explicit and
  stable, ready for future conservation checks and accumulation policy.
- The traits do not yet claim that a described system can be evaluated or
  propagated.
- A future evaluation API can add typed state/data contracts separately without
  changing what a system description means.

## Validation

Tests exercise both simplified implementations through `SystemDynamics`, plug
in conservative and non-conservative models, preserve declaration order,
restrict interaction dependencies to spacecraft state, and reject duplicate
attractors. No floating-point validation is claimed because this slice does
not evaluate equations.

## Provenance

This is an original architecture decision based on maintainer direction and
ADR-0002. No third-party dynamics implementation or tests were consulted.
