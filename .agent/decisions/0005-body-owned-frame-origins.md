# ADR-0005: body identities own celestial frame origins

- Status: Accepted
- Date: 2026-07-04
- Owners: orskit maintainers
- Affected parity rows: celestial bodies; frames/transforms; CCSDS messages

## Context

`orskit-frames` initially encoded the Sun, planets, Moon, and two barycenters as
unrelated `FrameOrigin` variants. That makes bodies frame implementation
details, provides no reusable planet/moon identity, and cannot answer which
bodies belong to a barycentric system.

Frame identities are copied into every coordinate-dependent value, so their
foundational representation should remain immutable, hashable, and cheap to
copy. Identity must also remain separate from masses, gravity fields,
ephemerides, shapes, and rotation models, all of which have data provenance and
epoch requirements of their own.

## Decision

1. Add `orskit-bodies` below `orskit-frames` in the dependency graph. It owns
   `Body`, broad `BodyKind`, application-defined identifiers, and `BodySystem`.
2. Model a frame origin as `FrameOrigin::Body(Body)` or
   `FrameOrigin::Barycenter(BodySystem)`. Retain `FrameOrigin::Custom` for
   origins that are not celestial bodies, such as a spacecraft-defined origin.
3. Make body-system membership explicit through a validated static body slice.
   A system has a non-blank name and at least two distinct members. Static
   membership keeps `BodySystem`, `FrameOrigin`, and `ReferenceFrame` immutable,
   allocation-free, `Copy`, and hashable.
4. Define built-in Solar System and Earth-Moon systems. Their member lists are
   inspectable, so barycenter names are no longer disconnected labels.
5. Keep this crate identity-only. A `BodySystem` does not compute a barycenter
   and supplies no implicit mass, state, ephemeris, reference ellipsoid, or
   rotation model.

## Alternatives considered

- Keep celestial variants in `FrameOrigin`: rejected because it reverses the
  intended bodies-to-frames dependency and prevents reuse outside frame APIs.
- Add only a generic numeric body identifier: rejected for this slice because
  common identities and classifications would remain implicit and parsing
  would require an ambient registry.
- Store body-system membership in `Vec` or `Arc<[Body]>`: this would support
  runtime-sized ownership but would make every frame identity allocation-aware
  and remove its current `Copy` contract. A future dynamic registry can map to
  stable identities if real workflows require it.
- Attach gravitational parameters or ephemerides to `Body`: rejected because
  those are model/data selections rather than identity and often vary by
  source, version, and epoch.

## Consequences

- Coordinate values now identify celestial origins through reusable body
  domain types.
- Algorithms can inspect barycentric membership but still must request the
  physical data required to calculate or transform that barycenter.
- Adding a custom body does not create an implicit physical model.
- Custom body systems currently require static membership to preserve the
  lightweight frame value contract.

## Validation

Body tests cover classification and system invariants. Frame tests prove that
ICRF and Earth-centered built-ins still round-trip, and that an Earth-Moon
barycentric origin exposes both linked bodies. CCSDS tests retain center-name
parsing and Earth-fixed origin validation.

## Provenance

The distinction between individual bodies and barycenters follows public NAIF
SPICE identity documentation. Planet and dwarf-planet classifications follow
NASA Science and IAU Resolution B5. No third-party implementation source or
tests were consulted.
