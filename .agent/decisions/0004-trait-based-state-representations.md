# ADR-0004: compose complete states from physical properties and orbit representations

- Status: Accepted
- Date: 2026-07-04
- Owners: orskit maintainers
- Affected parity rows: Cartesian states; Keplerian/equinoctial elements;
  CCSDS orbit messages; bindings

## Context

The initial core used `SpacecraftState` for mass plus Cartesian kinematics and
made orientation and inertia optional. This cannot express a uniform contract
across Cartesian, Keplerian, and equinoctial representations, and it conflates
an OEM ephemeris point with a complete physical spacecraft state. Rust has no
class inheritance; a nominal base struct alone would not provide polymorphic
behavior.

## Decision

1. Define a representation-aware `State` trait with an associated native
   coordinate type. Every state exposes an epoch, positive mass, orientation,
   framed inertia, and its own coordinates; Cartesian position, velocity, and
   speed remain Cartesian-specific operations.
2. Store mass, orientation, and inertia in validated `SpacecraftProperties` and
   compose those properties into each concrete state. Epoch remains with the
   timed orbit representation.
3. Implement concrete `CartesianState`, elliptic `KeplerianState`, and elliptic
   `EquinoctialState` over distinct coordinate types. Element states contain no
   Cartesian cache and do not depend on Cartesian coordinates.
4. Use true anomaly/true longitude in this first slice. Keplerian states accept
   `0 <= e < 1`; equinoctial states use `(a, ex, ey, hx, hy, lv)` and support
   circular and equatorial elliptic orbits.
5. Define `StateConversion<Target>` for explicit representation conversion.
   Conversion-only inputs use its associated `Context`: a positive
   gravitational parameter is supplied only when converting Keplerian or
   equinoctial state to Cartesian state and is never stored in a state. No body
   constant or frame transform is selected implicitly.
6. Keep coordinate representations independent of time. `CoordinateSample<C>`
   associates any coordinates with an epoch and is the timed value used by
   CCSDS OEM. It is deliberately not a `State` until callers enrich it with
   explicit `SpacecraftProperties`.

## Alternatives considered

- A Java-style base-class hierarchy: Rust traits plus composition express the
  common contract without inheritance or heap allocation.
- One enum containing every representation: useful for closed serialization,
  but it prevents downstream state representations from implementing the same
  behavior.
- Optional mass/orientation/inertia in `State`: rejected because it weakens the
  user-requested physical contract and pushes absence handling into every
  consumer.
- Default OEM properties: rejected because OEM does not contain these values;
  fabricated physical data would be silently wrong.
- Cache Cartesian coordinates inside element states: rejected because it
  conflates representations, requires unrelated conversion context during
  construction, and permits duplicated coordinates to drift apart.

## Consequences

- Generic algorithms can accept `impl State` and constrain the associated
  coordinate type or an explicit `StateConversion` implementation according to
  what they actually need.
- Constructing a complete state is more explicit and intentionally requires
  orientation and inertia.
- CCSDS APIs return coordinates until caller-supplied spacecraft properties are
  available.
- The first element slice is elliptic/osculating only; hyperbolic, parabolic,
  mean-anomaly, and derivative-bearing elements remain future work.

## Validation

Analytic circular and polar vectors validate axis/rotation conventions.
Keplerian-to-equinoctial-to-Cartesian comparisons validate representation
agreement with metre-scale and sub-millimetre-per-second tolerances at LEO
scale. Constructor tests cover singularities and non-finite inputs.

## Provenance

- NASA GMAT Mathematical Specifications, 2007: public US Government technical
  documentation; equations and conventions only.
- NAIF SPICE `CONICS` documentation: public US Government API documentation;
  element/state semantics only.
- Orekit 12.0.2 `EquinoctialOrbit` public API documentation: behavior and
  convention research only; no source or tests consulted.
