# ADR-0030: compose domain contracts independently from implementations

- Status: Accepted
- Date: 2026-07-14
- Affected parity rows: orbits; dynamics and force-model composition; two-body propagation; stable Rust facade
- Supersedes: ADR-0025

## Context

The core crate exposed a closed `SpacecraftState` enum containing Cartesian and
element implementations, while the dynamics crate exposed a concrete
point-mass/two-body topology beside its reusable force contracts. This tied
users to implementations they had not selected, prevented application-defined
state representations, and made the facade link all currently available
capabilities.

## Decision

1. `orskit-core` owns open, implementation-neutral contracts: `SpacecraftState`,
   generic epoch-qualified `Orbit<S>`, generic `SpacecraftView<S>`, scientific
   provenance, gravity selection, and attitude/spacecraft definitions.
2. `orskit-dynamics` owns only open force-model, dynamics-topology, and generic
   propagation contracts. `ComposedDynamics` is the standard ordered assembly
   of heterogeneous conservative and non-conservative model handles.
3. Cartesian, Keplerian, and equinoctial state representations live in
   `orskit-orbits-cartesian`. Point-mass gravity topology and the elliptic
   Kepler solution live in `orskit-dynamics-two-body`.
4. The `orskit` facade exposes implementation crates only through explicit
   Cargo features. `default` is intentionally empty; `cartesian` and
   `two-body` select the current implementations, and the latter implies the
   former.
5. File-format adapters depend on the concrete representation they decode, not
   on a closed core enum.

## Consequences

- Applications can use a custom state or force-model implementation through
  the same core contracts.
- Feature selection makes concrete model costs and capabilities visible in the
  dependency graph.
- This is a pre-alpha breaking API change. Compatibility aliases would hide
  the old closed model and are deliberately not retained.

## Validation

Compile and test the core-only workspace subset, each implementation feature,
the facade feature combinations, and all existing Cartesian/two-body numerical
tests after relocation.

## Provenance

This is original orskit API architecture. No external implementation, tests,
or source material informed the change.
