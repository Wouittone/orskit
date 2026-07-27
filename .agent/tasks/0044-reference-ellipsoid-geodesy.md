# Task: add explicit reference-ellipsoid and topocentric geometry

## Parity target

- Ledger row: Geometry / celestial bodies, ephemerides, ellipsoids, geodesy
- Current status: Partial, with body identity only
- Intended status after this task: Partial, with validated WGS 84 conversion
  and local East–North–Up construction

## User workflow

A caller selects a reference ellipsoid, validates longitude/latitude/height,
converts it to or from conventional body-fixed geocentric coordinates, and
registers a local East–North–Up frame under a caller-owned frame catalog.

## Scientific contract

- Inputs and units: typed angle longitude/latitude, typed ellipsoidal height,
  typed semi-major axis and inverse flattening, and typed Cartesian positions.
- Outputs and units: typed geodetic or geocentric positions and a catalogued
  East–North–Up frame with reversible position conversion.
- Frames/epochs/time scales: geocentric axes use `+Z` north, `+X` at zero
  longitude/equator, `+Y` at 90° east/equator. A topocentric parent is an
  affirmatively body-fixed frame for the ellipsoid body; generic non-inertial
  axes such as TEME are insufficient. No epoch transform is implied.
- Conventions and valid regimes: east-positive longitude `[-π, π]`, geodetic
  latitude `[-π/2, π/2]`, ellipsoidal rather than orthometric height, oblate
  ellipsoids with positive axis and inverse flattening greater than one.
- External data requirements: caller-selected ellipsoid; WGS 84 constants are
  embedded from NGA's defining parameters.
- Errors and singularities: invalid ellipsoid/coordinate values, body mismatch,
  parent without body-fixed semantics, body center, and exact polar-axis
  inverse longitude.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| NGA WGS 84 defining parameters | US Government public technical data | Semi-major axis and inverse flattening | `ReferenceEllipsoid::wgs84` |
| EPSG method 9602 / Guidance Note 7-2 | IOGP/EPSG public registry method | Geographic/geocentric equations, axes, and North Sea example | `crates/bodies/src/geodesy.rs` |
| EPSG method 9836 / Guidance Note 7-2 | IOGP/EPSG public registry method | East–North–Up equations and example | `frames::TopocentricFrame` |

No source implementation or third-party test was copied.

## Design

- Affected crates/layers: `bodies`, `frames`, facade prelude, architecture and
  evidence documentation.
- Public API: `ReferenceEllipsoid`, `GeodeticPosition`, associated errors,
  `TopocentricFrame`, and `FrameCatalog::define_topocentric_enu`.
- Rejected alternatives: implicit Earth ellipsoid; public matrix API; hidden
  polar meridian; geodesy-to-frames dependency reversal.
- ADR required: ADR-0041.

## Validation

- Unit cases: WGS 84 parameters, EPSG 9602/9836 vectors, reverse round trips,
  invalid ellipsoid/coordinates, center/pole singularities, body and parent
  validation.
- Invariants/properties: accepted geodetic values are finite and in range;
  local/parent transforms are inverses within floating-point roundoff.
- Independent reference vectors: EPSG methods 9602 and 9836.
- Differential/scenario tests: facade and rustdoc compilation.
- Tolerances and justification: 1 cm for EPSG 9602 rounded centimetre output;
  2 mm for EPSG 9836 millimetre-rounded input/output; sub-millimetre reverse
  height and `1e-12 rad` angular round trip for the same non-singular point.
- Benchmarks: not required for scalar setup geometry.

## Completion checklist

- [x] Implementation and typed errors
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance recorded
- [x] Parity ledger updated
- [x] Relevant checks pass
- [x] Binding impact explicitly deferred until the Rust API stabilizes
