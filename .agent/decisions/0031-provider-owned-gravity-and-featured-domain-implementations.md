# ADR-0031: keep gravity providers and domain implementations outside core

- Status: Accepted
- Date: 2026-07-14
- Affected parity rows: foundations/providers; orbits; dynamics and force-model composition; two-body propagation; stable Rust facade
- Supersedes: ADR-0016, ADR-0024, ADR-0030

## Context

ADR-0030 removed the closed spacecraft-state enum, but it still placed gravity
and scientific-source contracts in `core`, gave new internal packages an
unnecessary `orskit-` prefix, and left the public propagator accepting a
duration. Those choices made a caller-owned gravity provider appear to require
core-owned provenance and blurred the distinction between reusable contracts
and implementation selections.

## Decision

1. `core` contains only representation-neutral spacecraft and orbit contracts.
   `Orbit<S>` requires `S: SpacecraftState`.
2. `gravity` owns `CentralGravityProvider` and its shared handle. Its optional
   `point-mass` feature supplies `PointMass { origin, parameter }`; applications
   can instead supply their own provider without a provenance record imposed by
   the toolkit.
3. `orbits` owns feature-gated state implementations. The current
   `cartesian` feature exports `orbits::cartesian::CartesianState`,
   `orbits::circular::CircularState`, `orbits::keplerian::KeplerianState`, and
   `orbits::equinoctial::EquinoctialState`. No public convenience enum erases
   that selection.
4. `dynamics` remains contract-only. Each physical topology and solver is an
   implementation crate; `dynamics-two-bodies` is the current two-body
   implementation, not the dynamics architecture itself.
5. The `Propagator` contract accepts a target `Epoch`; implementations may
   derive a duration internally. Concrete providers and representations are
   enabled only through the relevant crate or facade feature.
6. New internal packages use concise domain names. `core` remains imported as
   `orskit_core` inside Rust crates because a dependency named `core` shadows
   Rust's built-in crate; facade consumers use `orskit::core`.

## Consequences

- Applications compose their gravity provider, state representation, dynamics
  topology, and solver explicitly.
- A future numerical or n-body crate can implement `dynamics` without taking a
  dependency on the analytical two-body crate.
- This is a deliberate pre-alpha breaking API change. Compatibility aliases
  would retain the implementation coupling that this decision removes.

## Validation

Compile the default workspace and all features; run the state-conversion,
two-body numerical, facade-feature, documentation, formatting, and lint suites.

## Provenance

This is original orskit API architecture. No external implementation, tests,
or source material informed the decision.
