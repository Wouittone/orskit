# ADR-0008: separate physical forces from force-model implementations

- Status: Accepted
- Date: 2026-07-04
- Owners: orskit maintainers
- Affected parity rows: dynamics composition; force models

## Context

ADR-0006 established open plug-in contracts and conservative/non-conservative
composition, but its type names conflated a physical force with the model used
to approximate that force. Point-mass, spherical-harmonic, irregular-body, and
time-variable gravity are models of gravity; they are not four independent
physical forces.

The composition boundary must accept heterogeneous third-party and future
models without knowing their concrete types or encoding every model in a
project-owned enum.

## Decision

1. `Force` is an open object-safe description of a physical interaction family.
2. `ForceModel` is an open object-safe description of one implementation of a
   force. It exposes `model_name`, `force`, and spacecraft-state dependencies.
3. `ConservativeForceModel` and `NonConservativeForceModel` classify model
   implementations and remain separate ordered collections on `SystemDynamics`.
4. Model handles use trait objects. `ForceModel::force` returns `&dyn Force`
   rather than an associated type, closed enum, or value requiring downcasting.
   This preserves object safety and permits arbitrary heterogeneous model
   combinations.
5. `GravityForce` identifies gravity. `PointMassGravityModel` is its first model
   implementation. Future spherical-harmonic, irregular-body, and time-variable
   gravity implementations report the same physical force.
6. The capability inventory lists physical forces separately from their model
   implementations and supporting environmental/data submodels.

## Consequences

- System composition depends only on model traits, never model-specific fields.
- Diagnostics can state both the physical force and the selected model.
- New force families and new models remain downstream-extensible.
- The pre-alpha API receives deliberate breaking renames from “force” handles
  and collection methods to “force model” handles and methods.
- Potential-derived versus non-conservative classification remains a property
  of the configured model and its assumptions, not merely its force-family name.

## Alternatives considered

- A closed `ForceKind` or `ForceModelKind` enum: rejected because every new
  downstream family or model would require changing orskit.
- An associated `type Force` on `ForceModel`: rejected because models with
  different force types could not share one object-safe collection.
- Downcast model objects to discover their force: rejected because composition
  would rely on model specifics.
- Keep a single name: rejected because “gravity” and “point-mass gravity model”
  answer different questions.

## Validation

Tests compose custom model implementations through trait-object handles, verify
separate force and model names, preserve conservative/non-conservative ordering,
and retain the existing two-/three-body and propagation behavior.

## Provenance

This is an original architecture decision based on maintainer direction and
the existing orskit dynamics boundary. No third-party implementation or test
material was consulted.
