# ADR-0020: bound OEM decoder resources

- Status: Accepted
- Date: 2026-07-13
- Affected parity rows: CCSDS orbit messages

## Context

OEM event readers limited one physical line but allowed an unbounded number of
comments and unbounded accumulated header, metadata, or data work. Calling the
API “bounded memory” therefore described only one allocation, not the decoder's
actual resource contract.

## Decision

1. Every blocking, async, sequential, and parallel OEM KVN entrypoint uses an
   `OemDecoderLimits` value.
2. Finite defaults bound line content bytes, section content bytes/lines, and
   whole-document content bytes/lines. Whole-document counters never reset, so
   they also bound the number of segments and records a collector can retain.
   Callers may select other non-zero finite limits.
   `usize::MAX` is rejected because saturating counters could never exceed it.
3. LF and CRLF terminators are excluded consistently from byte accounting.
4. Counters reset only at validated header, metadata, and data boundaries.
5. Limit failures report the limit kind, section, source line, configured
   limit, and observed value.
6. Streaming readers fail as soon as an unterminated line cannot possibly fit
   the configured content limit, without waiting for newline or EOF.

## Consequences

- Endless short comments, repeated small segments, and other
  low-byte/high-line inputs terminate with a typed error.
- Existing large-file workloads remain supported by documented finite defaults.
- Chronology and semantic-provenance preservation are separate contracts.

## Validation

Tests cover exact inclusive boundaries, endless-comment exhaustion, section
reset, non-resetting document exhaustion across decoder modes, invalid zero
limits, and LF/CRLF equivalence.

## Provenance

This is original defensive parser design applied to the project's independent
CCSDS 502.0-B-3 reader. No external implementation or test was consulted.
