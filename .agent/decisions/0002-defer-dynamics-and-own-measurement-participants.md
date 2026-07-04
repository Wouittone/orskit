# ADR-0002: defer dynamics and keep participants in measurements

- Status: Accepted
- Date: 2026-07-04
- Owners: orskit maintainers
- Affected parity rows: propagation, observation, estimation

## Context

The initial scaffold contained a two-body `orbit` crate and a standalone
`stations` crate. Both boundaries were premature. orskit needs advanced,
composable dynamics rather than an architecture grown outward from a two-body
example. Measurements may involve ground assets, one or more spacecraft, or
other participants, so a station-only domain creates the wrong ownership and
encourages copying Orekit's station-centric API.

## Decision

1. Remove the provisional dynamics/orbit implementation and its crate. Design
   propagation later around multi-model dynamics, force composition, coupled
   states, integration, events, and variational equations.
2. Remove the standalone `stations` crate. Ground assets, spacecraft, clocks,
   signal paths, and corrections are all measurement-participant concerns and
   belong in `measurements` (or lower geometry/data abstractions shared by it).
3. Do not preserve placeholder APIs merely for structural continuity. Add
   participant and dynamics types only with a validated vertical workflow.
4. Treat Orekit's behavior and capability coverage as reference evidence, not
   its station object model as an API template.

## Consequences

- The workspace temporarily has no propagation implementation.
- `measurements` currently exposes only a typed range value; participant paths
  and ground geometry remain intentionally undesigned.
- Future range, Doppler, angular, inter-satellite, and multi-leg observations
  can share one participant/path abstraction.
- Future two-body support will be one model and validation regime within a
  broader dynamics system.

## Validation

- The workspace and binding adapters compile without `orbit` or `stations`
  dependencies.
- Documentation and the parity ledger make both deferrals explicit rather than
  claiming placeholder capability.

## Provenance

This is an orskit ownership decision prompted by maintainer review. No external
implementation source was consulted.
