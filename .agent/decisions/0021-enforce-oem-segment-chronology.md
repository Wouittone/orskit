# ADR-0021: enforce OEM segment chronology

- Status: Accepted
- Date: 2026-07-13
- Affected parity rows: CCSDS orbit messages

## Context

The OEM reader accepted duplicate and decreasing sample epochs within one
segment. Such a document could not safely support interpolation or ephemeris
semantics, yet all decoder modes returned it as valid typed coordinates.

## Decision

1. State epochs must be strictly increasing within each OEM segment.
2. One `SegmentChronology` validator defines the rule for blocking, async,
   sequential, and parallel modes.
3. Streaming modes validate before emitting a sample. Parallel mode parses in
   parallel but validates in deterministic source order before assembly.
4. Chronology resets at each segment boundary.
5. Errors report both epochs and both source lines.

## Consequences

- Invalid samples are never emitted before the chronology failure.
- Separate segments may start at the same or an earlier epoch.
- The large-file benchmark uses increasing nanosecond epochs rather than an
  invalid repeated fixture.

## Validation

Tests cover duplicate, reversed, cross-segment reset, invalid-sample emission,
and mode-equivalent errors.

## Provenance

This is original validation around the chronological OEM segment semantics in
CCSDS 502.0-B-3, already recorded in project provenance. No external source or
test implementation was consulted.
