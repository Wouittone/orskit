# Task: parse and format strict Two-Line Element sets

## Parity target

- Ledger row: I/O — TLE, SP3, RINEX, gravity, EOP, ephemeris, space weather
- Current status: Not assessed
- Intended status after this task: Partial, with bounded strict TLE text support

## User workflow

A caller supplies two complete TLE data lines, receives a validated owned
mean-element record with unit-qualified accessors, and formats it back to a
canonical checksummed two-line representation.

## Scientific contract

- Inputs and units: exactly two 69-byte ASCII fixed-column TLE lines.
- Outputs and units: TLE mean elements in degrees, revolutions/day and its
  format-defined derivatives, dimensionless eccentricity, and B* in inverse
  Earth radii.
- Frames/epochs/time scales: epoch is the TLE UTC year/day representation.
  No coordinate frame is fabricated; TEME arises only from future SGP4 output.
- Conventions and valid regimes: conventional 1957 two-digit-year pivot,
  epoch day zero retained, standard modulo-10 checksum, matching numeric or
  Alpha-5 catalog identifiers, canonical left-justified designator output.
- External data requirements: none.
- Errors and singularities: typed line, field, column, checksum, range, and
  cross-line identity errors; input line count and line size are bounded.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| CelesTrak, *NORAD Two-Line Element Set Format*, updated 2022-07-01 | Public format documentation | Fixed columns, field units, checksum, NOAA 14 example | `crates/tle` |
| T. S. Kelso, *FAQs: Two-Line Element Set Format*, Satellite Times 4(3), 1998; page updated 2025-01-27 | Public technical documentation | Character grammar, ranges, UTC epoch/year pivot, derivative scaling, B* units, padding | `crates/tle` |
| Space-Track.org, *Basic Description of the TLE Format / Alpha-5*, accessed 2026-07-24 | Public USSPACECOM service documentation | Alpha-5 mapping and current checksum convention | `crates/tle` |
| `libfuzzer-sys` 0.4.13 | `(MIT OR Apache-2.0) AND NCSA` developer dependency | Bounded in-process parser inputs only | isolated `crates/tle/fuzz` workspace |

No implementation source, third-party tests, or parser structure was consulted
or copied.

## Design

- Affected crates/layers: new `tle` operational-format crate, isolated
  parser-quality workspace, `orskit` facade, docs, provenance, parity, and
  roadmap.
- Public API: `TwoLineElement`, `TleError`, `TleLine`, `TleField`; standard
  `FromStr` and `Display`.
- Rejected alternatives: adding TLE to CCSDS; converting mean elements to
  osculating `Orbit`; permissive token splitting; storing unchecked source.
- ADR required: ADR-0040.

## Validation

- Unit cases: published NOAA 14 record, Alpha-5 examples, checksum, length,
  non-ASCII, fixed-column, range, mismatched-object, line-count failures, and a
  committed minimized-input regression path.
- Invariants/properties: parse–format–parse preserves the validated record;
  canonical output is exactly 69 bytes per line with valid checksums.
- Independent reference vectors: CelesTrak NOAA 14 and Space-Track Alpha-5
  mappings.
- Differential/scenario tests: facade feature compilation.
- Tolerances and justification: none; fixed decimal fields are retained as
  scaled integers and compared exactly.
- Benchmarks: not required for two bounded 69-byte lines.

## Completion checklist

- [x] Implementation and typed errors
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance recorded
- [x] Parity ledger updated
- [x] Relevant checks pass
- [x] Binding impact explicitly deferred until the Rust API stabilizes
