# ADR-0033: resolve caller state before propagation

- Status: Accepted
- Date: 2026-07-14
- Affected parity rows: two-body propagation; future numerical and analytical
  propagation

## Context

An analytical propagator may only advance a particular element set, while its
public callers need to retain their selected spacecraft-state representation.
Putting representation conversion into individual propagator methods would
duplicate the boundary and invite special methods such as
`propagate_keplerian`. That would leave future propagators with inconsistent
conversion, validation, and restoration behavior.

## Decision

1. `dynamics::PropagationState<Problem>` resolves one caller-selected state
   type into the representation a concrete propagator advances, then restores
   that caller type using the propagator-owned problem.
2. `Propagator<State>` owns both its algorithm and one physical problem. Each
   implementation performs resolution, advancement, and restoration internally;
   callers pass only an epoch-qualified orbit and target epoch.
3. The elliptic two-body solver resolves every supported caller state to
   Cartesian state and advances that one state with universal variables.
   Cartesian state is regular for every non-collision orbital orientation,
   including exact retrograde planes; element callers retain their own
   restoration-chart policies.
4. Resolution receives the propagator-owned physical problem, so gravity-identity and
   origin validation occurs before solving, including zero-duration requests.

## Alternatives considered

- Per-propagator public conversion methods: rejected because they fragment a
  common API and duplicate validation.
- A closed enum of state representations: rejected because `SpacecraftState`
  remains application-extensible.
- Always resolving through Keplerian or prograde equinoctial elements:
  rejected because each has an inclination-chart singularity.

## Consequences

- Future analytical and numerical propagators reuse one typed conversion and
  restoration flow while choosing their own valid target state.
- State/problem compatibility errors remain typed and occur before numerical
  propagation.
- Extension authors implement `PropagationState` and the common `Propagator`
  contract for scientifically valid combinations rather than adding special
  public propagation methods or solver-specific target traits.

## Validation

The `dynamics` contract test verifies that a propagator owns and uses one
physical problem. The two-body suite
checks an application-defined Cartesian-resolving state, all four supported
representations, exact retrograde Cartesian propagation, zero-duration
validation, shared gravity identity, and independent reference vectors.

## Provenance

This is original orskit API architecture. NASA/TM-2004-213230 supplies the
public universal-variable and Lagrange-coefficient equations; Orekit/Lox
black-box vectors validate results only. No external implementation, tests, or
source material informed this boundary.
