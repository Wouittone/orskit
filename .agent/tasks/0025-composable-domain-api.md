# Task: compose domain contracts independently from implementations

## Parity target

- Ledger row: Orbits; dynamics and force-model composition; two-body propagation; stable Rust facade
- Current status: Partial / Designed
- Intended status after this task: Partial, with implementation-independent public contracts

## User workflow

A mission application selects the state and dynamics implementations it needs
through facade features, constructs an epoch-qualified orbit from that state,
and supplies independently composed force-model descriptions to a propagation
method. Applications can also implement the core state and force traits without
depending on Cartesian or two-body crates.

## Scientific contract

- Inputs and units: implementations retain typed physical quantities.
- Outputs and units: implementations retain typed physical quantities.
- Frames/epochs/time scales: every state declares a frame; `Orbit` owns its Hifitime epoch.
- Conventions and valid regimes: concrete state/model crates document their regimes.
- External data requirements: model implementations own explicit provider configuration.
- Errors and singularities: implementation crates expose typed errors; core contracts do not erase them.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| `.agent/ARCHITECTURE.md` | Project architecture | Dependency direction and separate implementation boundaries | workspace manifests and public contracts |
| `.agent/decisions/0025-separate-propagation-method-from-problem.md` | Original project decision | Solver and physical problem are independent | dynamics propagation contract |

## Design

- Affected crates/layers: core contracts, dynamics contracts, Cartesian orbit implementation, two-body implementation, facade, CCSDS adapter.
- Public API: `SpacecraftState` becomes an open trait; `Orbit<S>` and `SpacecraftView<S>` compose a chosen state implementation; `ComposedDynamics` assembles heterogeneous force models.
- Rejected alternatives: a closed enum in the core and a facade that always links every implementation.
- ADR required: yes, ADR-0030.

## Validation

- Unit cases: custom downstream-like state/model implementations, implementation composition and feature combinations.
- Invariants/properties: orbit state and spacecraft view retain the selected state value; force declaration order is retained.
- Independent reference vectors: existing Cartesian and two-body vectors remain in their implementation crates.
- Differential/scenario tests: existing two-body propagation scenarios remain feature-gated.
- Tolerances and justification: unchanged, because the mathematical implementations move without alteration.
- Benchmarks: existing two-body benchmark moves with its implementation crate.

## Completion checklist

- [ ] Implementation and typed errors
- [ ] Scientific and regression tests
- [ ] Rustdoc/examples
- [ ] Provenance recorded
- [ ] Parity ledger updated
- [ ] Relevant checks pass
- [ ] Binding impact handled or explicitly deferred
