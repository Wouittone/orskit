# ADR-0024: share extensible central-gravity objects

- Status: Accepted
- Date: 2026-07-13
- Affected parity rows: scientific data context; orbital representations;
  propagation composition

## Context

ADR-0019 introduced core-owned string provenance and a concrete gravity-context
value. That made extension depend on adapting application catalogs into one
record shape and copied structural identity into the public model.

## Decision

1. `ScientificSource` and `CentralGravity` are object-safe `Send + Sync` traits
   implemented by applications or small built-in reference types.
2. Public state/model boundaries use `Arc<dyn CentralGravity>` and borrowed
   getters. Cloning a state increments the shared allocation count; it does not
   duplicate provenance or gravity data.
3. The shared allocation is the unforgeable scientific identity. Element
   construction and conversion require the gravity origin to match the frame;
   conversion also requires the same `Arc` allocation.
4. `PointMassGravity` and `ReferenceSource` are conveniences, not closed
   registries; downstream implementations participate in the same APIs.
5. Keplerian/equinoctial-only conversions retain the shared object without
   requesting unrelated inputs.

## Consequences

- Rich catalogs and scenario types integrate without stringly conversion or a
  process-global registry.
- Independently allocated, numerically equal gravity objects remain distinct;
  callers explicitly share one `Arc` when they mean one selection.
- Serialization will require an explicit application/catalog mapping rather
  than serializing a trait object implicitly.

## Validation

Tests cover downstream trait implementations, shared built-in provenance,
allocation identity mismatch, origin mismatch, and pointer preservation across
element representations.

## Provenance

This is original orskit API architecture. No external implementation or tests
were consulted.
