# ADR-0026: catalog derived-frame identities

- Status: Accepted
- Date: 2026-07-13
- Affected parity rows: frames and transforms; ground participants

## Context

`DerivedFrame` used a caller-supplied `CustomFrameId` but did not include its
parent/definition identity. Independent definitions could collide, and
`GroundStation` constructed frames from raw ID, parent, and offset parts.

## Decision

1. Caller-owned `FrameCatalog` is the only issuer of general `FrameId` values
   for derived frames.
2. `FrameId` contains a catalog namespace, local ID, and deterministic opaque
   definition tag. Replicas issuing the same definition agree; replicas that
   reuse a namespace/local ID for conflicting geometry cannot compare equal.
3. Catalogs accept explicit non-derived roots. Derived parents must already
   exist in the same catalog, rejecting foreign parents and making cycles
   unconstructable.
4. Identical redefinition is idempotent; conflicting redefinition is an error.
5. `GroundStation` composes `ParticipantId` with a validated `DerivedFrame` and
   does not accept raw identity/parent/offset constructor parts.
6. No global registry is introduced; `ReferenceFrame` stays copyable/hashable.

## Validation

Tests cover namespace collisions, conflicts, foreign/unknown parents,
registered chains, invalid roots/geometry, and station composition.

## Provenance

This is original orskit frame-identity architecture. No external implementation
or tests were consulted.
