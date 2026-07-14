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
2. `FrameId` contains a catalog namespace, a non-reusable process-local issuing
   authority ID, and a local ID. Separate catalog instances never issue equal
   identities, even when callers reuse a namespace/local ID. Exhausting the
   checked issuer space is a typed construction error rather than wraparound.
3. Catalogs accept explicit non-derived roots. Derived parents must already
   exist in the same catalog, rejecting foreign parents and making cycles
   unconstructable.
4. Identical redefinition is idempotent; conflicting redefinition is an error.
5. `GroundStation` composes `ParticipantId` with a validated `DerivedFrame` and
   does not accept raw identity/parent/offset constructor parts.
6. No global definition registry is introduced; the only process-global state
   is an atomic issuing-authority counter. `ReferenceFrame` stays copyable and
   hashable, and scientific definitions remain caller-owned.

## Validation

Tests cover namespace collisions, conflicts, foreign/unknown parents,
registered chains, invalid roots/geometry, and station composition.

## Provenance

This is original orskit frame-identity architecture. No external implementation
or tests were consulted.
