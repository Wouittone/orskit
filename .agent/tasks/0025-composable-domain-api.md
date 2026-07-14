# Task: compose domain contracts independently from implementations

## Parity target

- Ledger row: Orbits; dynamics and force-model composition; two-body propagation; stable Rust facade
- Current status: Partial / Designed
- Intended status after this task: Partial, with implementation-independent public contracts

## User workflow

A mission application selects a gravity provider, state representation, and
dynamics implementation through crate or facade features, constructs an
epoch-qualified orbit from that state, and supplies independently composed
force-model descriptions to a propagation method targeting an explicit epoch.
Applications can also implement the core state and dynamics traits without
depending on Cartesian or two-body crates.

## Scientific contract

- Inputs and units: implementations retain typed physical quantities.
- Outputs and units: implementations retain typed physical quantities.
- Frames/epochs/time scales: every state declares a frame; `Orbit<S: SpacecraftState>` owns its Hifitime epoch and propagation targets an explicit Hifitime epoch.
- Conventions and valid regimes: concrete state/model crates document their regimes.
- External data requirements: model implementations own explicit provider configuration; application-owned gravity providers do not require a toolkit provenance record.
- Errors and singularities: implementation crates expose typed errors; core contracts do not erase them.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| `.agent/ARCHITECTURE.md` | Project architecture | Dependency direction and separate implementation boundaries | workspace manifests and public contracts |
| `.agent/decisions/0025-separate-propagation-method-from-problem.md` | Original project decision | Solver and physical problem are independent | dynamics propagation contract |

## Design

- Affected crates/layers: core contracts, gravity provider contract, feature-gated orbit implementations, dynamics contracts, two-body implementation, facade, CCSDS adapter.
- Public API: `SpacecraftState` is an open trait; `Orbit<S: SpacecraftState>` and `SpacecraftView<S>` compose a chosen state implementation; `gravity::CentralGravityProvider` owns gravity selection; `ComposedDynamics` assembles heterogeneous force models; `Propagator` targets an epoch.
- Rejected alternatives: a closed enum in the core, core-owned provider provenance, a generic dynamics crate that embeds one physical topology, and a facade that always links every implementation.
- ADR required: yes, ADR-0031.

## Validation

- Unit cases: custom downstream-like state/model implementations, implementation composition and feature combinations.
- Invariants/properties: orbit state and spacecraft view retain the selected state value; force declaration order is retained.
- Independent reference vectors: existing Cartesian and two-body vectors remain in their implementation crates.
- Differential/scenario tests: existing two-body propagation scenarios remain feature-gated.
- Tolerances and justification: unchanged, because the mathematical implementations move without alteration.
- Benchmarks: existing two-body benchmark moves with its implementation crate.

## Completion checklist

- [x] Implementation and typed errors
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance recorded
- [x] Parity ledger updated
- [x] Relevant checks pass
- [x] Binding impact handled or explicitly deferred
