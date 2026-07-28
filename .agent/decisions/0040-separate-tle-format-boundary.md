# ADR-0040: keep TLE text validation separate from SGP4 propagation

- Status: Accepted
- Date: 2026-07-24
- Owners: orskit maintainers
- Affected parity rows: TLE/SGP4; operational data formats

## Context

A TLE is a legacy fixed-width representation of mean elements intended for a
matching SGP4-family model. Treating those fields as an osculating Keplerian
orbit would silently change their meaning, while placing the parser in the
CCSDS crate would misclassify the format. The same validated record must later
serve an independently evidenced SGP4 implementation.

## Decision

1. A dedicated `tle` crate owns strict fixed-column parsing, checksums,
   Alpha-5 identifiers, range checks, and canonical formatting.
2. Parsed values remain TLE mean-element data with unit-qualified accessors.
   They are not converted into an `Orbit` and do not claim a coordinate frame.
3. Parsing accepts exactly two 69-byte ASCII lines. It validates all fixed
   separators, both checksums, and the cross-line catalog identity before
   constructing the immutable record.
4. Fixed decimal values are stored as scaled integers so canonical formatting
   and parse–format–parse evidence are exact.
5. SGP4 propagation is a later vertical slice that will consume this record,
   define TEME output explicitly, and use published verification cases.

## Alternatives considered

- Put TLE under `ccsds`: rejected because TLE is not a CCSDS message.
- Convert immediately to `KeplerianState`: rejected because TLE fields are
  model-specific mean elements, not generic osculating elements.
- Combine parsing with SGP4: rejected to keep format correctness independently
  testable and to avoid an unevidenced propagation placeholder.
- Preserve unchecked source text: rejected because downstream propagation must
  not repeatedly discover malformed fixed-column input.

## Consequences

The parser is deterministic, bounded, and has no data or network dependency.
Canonical formatting may normalize permitted leading spaces or zeroes,
exponent signs on zero, and international-designator justification. Three-line
records with a common name, bulk catalogs, OMM conversion, and SGP4 remain
future work.

## Validation

Published format examples, Alpha-5 mappings, malformed inputs, checksum cases,
and exact semantic round trips exercise the boundary. Facade feature checks
ensure TLE remains opt-in.

## Provenance

Only the public CelesTrak TLE documentation, T. S. Kelso's public format FAQ,
and Space-Track's public TLE/Alpha-5 documentation were consulted. No parser or
SGP4 implementation source was used.
