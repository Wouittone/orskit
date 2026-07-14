# ADR-0032: use standard conversions and add the circular orbit state

- Status: Accepted
- Date: 2026-07-14
- Affected parity rows: epoch/frame-qualified Cartesian states; Keplerian,
  circular, equinoctial, and nonsingular elements; two-body propagation

## Context

The current representation slice contained Cartesian, Keplerian, and
equinoctial states, but exposed a project-specific `StateConversion` trait.
That duplicated Rust's fallible conversion vocabulary and made ordinary
representation conversion less discoverable. The pinned Orekit 13.1.7 public
orbit inventory names four primary kinematic representations: Cartesian,
circular, Keplerian, and equinoctial.

## Decision

1. Use standard `From`/`Into` only for infallible conversion and
   `TryFrom`/`TryInto` for every fallible state conversion. Remove custom
   source-side conversion traits.
2. Cartesian-to-element conversions implement `TryFrom<(CartesianState,
   SharedCentralGravity)>`; the tuple makes caller-selected gravity explicit
   at the conversion boundary. Element states retain their gravity binding and
   therefore implement `TryFrom<ElementState> for CartesianState`.
3. Add `CircularState` for elliptic circular elements `(a, ex, ey, i, Omega,
   alpha_v)`, with `ex=e cos(omega)`, `ey=e sin(omega)`, and
   `alpha_v=nu+omega`. Values are typed and frame/gravity-qualified exactly as
   the existing elliptic element states.
4. Implement the full supported conversion graph among Cartesian, circular,
   Keplerian, and equinoctial states. Conversions preserve the existing
   elliptic and retrograde-equinoctial error policy.
5. The elliptic analytical two-body solver supports `CircularState` and
   returns the same representation through the resolved-state propagation
   boundary defined by ADR-0033.

## Alternatives considered

- Retaining `StateConversion`: rejected because it duplicates standard Rust
  conversion traits without adding a domain invariant.
- Implicit or global gravity for Cartesian conversion: rejected because it
  would hide data provenance and permit wrong-body conversions.
- Copying Orekit's abstract orbit hierarchy: rejected; its public inventory
  identifies capability coverage, while orskit preserves an open Rust-native
  state contract.

## Consequences

- Client code can use standard conversion idioms and the new circular
  representation without a closed core enum.
- A Cartesian conversion must carry a shared gravity provider explicitly.
- The current scope remains elliptic only; anomaly variants, interpolation,
  field-valued states, and non-elliptic conics remain separate capabilities.

## Validation

Test every directed conversion, circular analytic vectors, signed two-body
round trips, native representation preservation, and existing independent
Cartesian endpoint vectors.

## Provenance

- Orekit 13.1.7 public `org.orekit.orbits` package and `CircularOrbit` API:
  capability inventory and circular-element terminology only.
- Existing NASA GMAT and NAIF references recorded in `PROVENANCE.md`:
  independently derived elliptic conversion equations.

No external implementation, source, or test code was consulted or copied.
