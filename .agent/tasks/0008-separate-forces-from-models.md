# Task: separate physical forces from model implementations

## Parity target

- Ledger row: Propagation / Dynamics and force-model composition
- Current status: Designed
- Intended status after this task: Designed, with corrected force/model boundary

## User workflow

Define a model for a physical force, combine it with any other conservative or
non-conservative model behind object-safe trait handles, and inspect both the
physical force family and the selected model without matching on model types.

## Scientific contract

- Inputs and units: descriptive model configuration only; no numerical input.
- Outputs and units: ordered heterogeneous force-model descriptions; no
  acceleration, torque, or mass-rate output yet.
- Frames/epochs/time scales: deferred to evaluation contracts.
- Conventions and valid regimes: a force is a physical interaction family; a
  force model is one approximation/implementation of that force.
- External data requirements: remain model configuration or explicit future
  data providers.
- Errors and singularities: unchanged; this slice evaluates no equations.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| Maintainer direction and existing ADR-0006 | Original project design | Physical force identity must not be conflated with model implementation | `orskit-dynamics`, force catalogue, ADR-0008 |

No third-party implementation material is used.

## Design

- Affected crates/layers: `orskit-dynamics` and agent architecture documents.
- Public API: open `Force`; object-safe `ForceModel`; renamed conservative and
  non-conservative model traits/handles/collections; `GravityForce` and
  `PointMassGravityModel`.
- Rejected alternatives: closed force/model enums; associated model force type
  that prevents heterogeneous trait objects; downcasting model implementations;
  retaining force/model names as synonyms.
- ADR required: yes, ADR-0008.

## Validation

- Unit cases: point-mass model reports gravity force; mixed custom models retain
  force/model identities and declaration order.
- Invariants/properties: `SystemDynamics` stores only model trait objects and
  never branches on concrete model types.
- Independent reference vectors: not applicable to a descriptive API refactor.
- Differential/scenario tests: existing two-body propagation remains green.
- Tolerances and justification: not applicable.
- Benchmarks: not applicable; no evaluation hot path changes.

## Completion checklist

- [x] Implementation and typed errors: no new recoverable failure mode
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance recorded
- [x] Parity ledger updated
- [x] Relevant checks pass
- [x] Binding impact handled or explicitly deferred: bindings remain disabled
  and do not expose the pre-alpha dynamics traits
