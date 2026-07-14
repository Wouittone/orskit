# ADR-0018: qualify range observations

- Status: Accepted
- Date: 2026-07-13
- Affected parity rows: ground participants; range observations

## Context

The original `RangeMeasurement` stored only an epoch, scalar range, and
uncertainty. It could not say who exchanged the signal, which signal event
owned the epoch, or whether the scalar meant full path length or a conventional
one-way equivalent. Reversing participants or time-tag semantics produced the
same value.

## Decision

1. Every range observation owns an ordered `SignalPath` of validated
   `ParticipantId` values.
2. `ObservationTimeTag` identifies transmit, receive, or a validated
   intermediate participant event.
3. `RangeConvention` distinguishes full path length from a two-leg returning
   path's one-way equivalent, and construction validates the topology.
4. `GroundStation` owns the same `ParticipantId` type used in signal paths.
5. Remove the ambiguous three-argument range constructor without a
   compatibility escape hatch during the pre-alpha API phase.

## Consequences

- Participant order, epoch semantics, and scalar convention are part of value
  identity.
- The API still does not infer light time, turnaround time, clocks,
  corrections, or media models.
- Future measurement types can reuse participant and path identity without a
  station-centric hierarchy.

## Validation

Tests distinguish reversed and differently tagged paths, reject invalid event
indices and round-trip conventions, and cover a station-spacecraft-station
path.

## Provenance

This is original orskit measurement-domain design. No third-party source or
test was consulted.
