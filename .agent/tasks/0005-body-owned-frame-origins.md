# Task: add body-owned frame origins and linked barycentric systems

## Parity target

- Ledger row: Geometry / Celestial bodies, ephemerides, ellipsoids, geodesy
- Current status: Not assessed
- Intended status after this task: Partial

## User workflow

Identify planets, moons, the Sun, dwarf planets, and application-defined
celestial bodies independently of reference frames. Construct a frame whose
origin is either one body or the barycenter of an inspectable body system.

## Scientific contract

- Inputs and units: identity values only; this slice accepts no physical
  scalars or unit-bearing values.
- Outputs and units: immutable body, classification, system-membership, and
  frame-origin identities; no numerical output.
- Frames/epochs/time scales: `FrameOrigin::Body` and
  `FrameOrigin::Barycenter` compose body identities into a reference frame;
  body identities themselves have no epoch or orientation.
- Conventions and valid regimes: the built-in catalogue distinguishes the Sun,
  eight IAU planets, Earth's Moon, and Pluto as a dwarf planet. Application
  bodies carry an explicit broad classification. A barycentric system contains
  at least two distinct bodies.
- External data requirements: none. Identity membership does not imply masses,
  ephemerides, shapes, rotations, or a barycenter computation model.
- Errors and singularities: blank system names, fewer than two bodies, and
  duplicate members are typed construction errors.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| NAIF SPICE *NAIF Integer ID codes*, revision 2021-12-10 | US Government API documentation | Bodies and system barycenters are distinct ephemeris-object identities; Earth-Moon and Solar System barycenter terminology | `crates/bodies`, `crates/frames` |
| NASA Science *About the Planets* | US Government public science documentation | Eight-planet catalogue and Pluto dwarf-planet classification | `Body`, `BodyKind` |
| IAU Resolution B5, 2006 | Public scientific resolution | Formal planet/dwarf-planet classification boundary | `BodyKind` documentation |

No source, tests, examples, or internal structure were copied from another
astrodynamics implementation.

## Design

- Affected crates/layers: new foundational `orskit-bodies`; `orskit-frames`;
  CCSDS adapter; compilation-only downstream consumers; handbook and README.
- Public API: `Body`, `BodyKind`, `CustomBodyId`, `BodySystem`,
  `BodySystemError`, and body-backed `FrameOrigin::{Body, Barycenter}`.
- Rejected alternatives: retaining one frame-origin variant per planet;
  disconnected barycenter labels; embedding masses or ephemerides in identity
  types; heap-owned system membership that would make frame identities
  allocation-bearing and non-`Copy`.
- ADR required: yes, ADR-0005.

## Validation

- Unit cases: built-in body classification, aliases, custom moon identity, and
  invalid custom systems.
- Invariants/properties: Earth-Moon barycenter links exactly Earth and Moon;
  built-in reference frames retain round-trip parsing.
- Independent reference vectors: not applicable to identity-only types.
- Differential/scenario tests: CCSDS non-geocentric center parsing and
  Earth-only orientation rejection continue through body-backed origins.
- Tolerances and justification: not applicable; no floating-point operations.
- Benchmarks: not required; identities are immutable scalar/static-slice values.

## Completion checklist

- [x] Implementation and typed errors
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance recorded
- [x] Parity ledger updated
- [x] Relevant checks pass
- [x] Binding impact handled or explicitly deferred
