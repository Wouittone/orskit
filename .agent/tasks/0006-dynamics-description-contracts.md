# Task: describe composable system dynamics without selecting a resolver

## Parity target

- Ledger row: Propagation / Dynamics and force-model composition
- Current status: Not assessed
- Intended status after this task: Designed

## User workflow

Describe the participants in a dynamical system, inspect the ordered force
models acting between them, plug in additional force-model descriptions, and
represent simplified two- and three-body systems through the same extensible
contract. Do not evaluate derivatives or propagate a state yet.

## Scientific contract

- Inputs and units: body identities and force source/target relationships only;
  no numerical physical inputs in this slice.
- Outputs and units: immutable model topology; no derivative, acceleration, or
  propagated state output.
- Frames/epochs/time scales: deliberately deferred to a future evaluation
  contract, where they must be explicit alongside the evaluated state.
- Conventions and valid regimes: two- and three-body descriptions contain two
  or three distinct bodies respectively and begin with mutual point-mass
  gravity topology. This does not select restricted/full equations or a
  numerical solution method.
- External data requirements: none. Gravity parameters, ephemerides, frame
  transforms, and other model data are not inferred from body identity.
- Errors and singularities: duplicate participants and force models that refer
  to bodies outside the described system are typed construction errors.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| Maintainer design direction and ADR-0002 | Project-owned architecture decision | Advanced dynamics must compose models and must not grow outward from a two-body kernel | `crates/dynamics`; ADR-0006 |

No third-party implementation, equations, tests, or internal structure were
consulted. This slice contains software contracts rather than a numerical
scientific model.

## Design

- Affected crates/layers: new `orskit-dynamics` domain crate; workspace and
  architecture documentation. Bindings remain unchanged.
- Public API: `SystemDynamics`, `ForceModel`, `ForceInteraction`,
  `ForceModelHandle`, `MutualPointMassGravity`, `TwoBodyDynamics`,
  `ThreeBodyDynamics`, and `DynamicsDescriptionError`.
- Rejected alternatives: derivative evaluation before defining state/data
  contracts; a two-body-specific dynamics trait; an enum closing the force
  model set; implicit bodies referenced only by force models; placeholder
  propagation methods.
- ADR required: yes, ADR-0006.

## Validation

- Unit cases: two-/three-body trait implementations, force plug-in order,
  duplicate bodies, and external force participants.
- Invariants/properties: every force source/target belongs to the containing
  simplified system; declared model order is preserved.
- Independent reference vectors: not applicable without numerical evaluation.
- Differential/scenario tests: not applicable in this descriptive slice.
- Tolerances and justification: not applicable; no floating-point operations.
- Benchmarks: not required; model construction is not a profiled hot path.

## Completion checklist

- [x] Implementation and typed errors
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance recorded
- [x] Parity ledger updated
- [x] Relevant checks pass
- [x] Binding impact handled or explicitly deferred
