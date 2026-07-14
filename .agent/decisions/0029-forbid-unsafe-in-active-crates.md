# ADR-0029: forbid unsafe code in active crates

- Status: Accepted
- Date: 2026-07-13
- Affected parity rows: engineering safety

## Decision

Every active workspace crate declares `#![forbid(unsafe_code)]`. Unsafe code is
not an available implementation technique in the scientific workspace.
Disabled binding prototypes remain outside the dependency graph and are not
modified or re-enabled by this task.

## Consequences

- The compiler rejects unsafe functions, blocks, traits, and implementations in
  every active crate, including code introduced by future contributors.
- Any future FFI re-enablement requires an explicit binding task and a new
  architecture/policy decision rather than silently weakening crate policy.

## Provenance

This is original orskit engineering policy.
