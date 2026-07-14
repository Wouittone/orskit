# ADR-0022: preserve OEM source records and provenance

- Status: Accepted
- Date: 2026-07-13
- Affected parity rows: CCSDS orbit messages

## Context

The decoder accepted comments and segment metadata but projected coordinate
events down to bare samples. Callers could not associate a sample with its
segment metadata or source line, and collected documents discarded accepted
record ordering and comment location.

## Decision

1. Every segment has a stable source-order `OemSegmentId`.
2. Segment start/end, comments, and coordinate samples identify their segment.
3. `OemComment` retains section, line, and text. `OemSample` retains line and a
   shared immutable `OemSegmentContext` containing segment metadata.
4. Streaming modes preserve exact event order. Collected segments retain an
   ordered record view while also exposing convenient coordinate/comment views.
5. Blocking, async, sequential, and parallel modes expose equivalent semantic
   provenance.

## Consequences

- Accepted source information is not silently discarded.
- Shared metadata uses `Arc`; samples do not clone segment strings.
- XML, covariance, writing, and full semantic round trips remain future work.

## Validation

Tests cover header/metadata/data comment location, interleaved record order,
segment IDs, metadata pointer sharing, and mode-equivalent events/documents.

## Provenance

This is original orskit ingestion architecture implementing the source-order
and segment semantics of CCSDS 502.0-B-3 already recorded in project
provenance. No external implementation or tests were consulted.
