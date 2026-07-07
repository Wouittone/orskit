# ADR-0016: expose a thin public facade and namespace package names

- Status: Accepted
- Date: 2026-07-07
- Affected parity rows: stable public Rust facade; package naming

## Context

The workspace had focused packages named `orskit-*`, but the transitional
utility crate was still published as `utils`, and there was no package named
`orskit`. Users had to discover individual internal crate boundaries before
they could write even a small example.

The Rust core is still pre-alpha, so a broad stable workflow API would be a
false promise. The project still needs a clear public entrypoint and a package
naming rule that avoids generic crate names.

## Decision

1. Add a root `orskit` crate as a thin facade over the focused crates.
2. Re-export focused crates as named modules such as `orskit::core` and
   `orskit::frames` instead of moving domain behavior into the facade.
3. Provide a conservative `orskit::prelude` for examples and early user
   workflows.
4. Rename the transitional package `utils` to `orskit-utils`.
5. Keep the facade pre-alpha: it establishes the import root and direction but
   does not claim a stable 1.0 API.

## Alternatives considered

- Keep only focused crates: this preserves internal clarity but leaves users
  without the public `orskit` package promised by the roadmap.
- Move all exports directly into the facade root: this would create avoidable
  name collisions and obscure which crate owns each domain.
- Stabilize a high-level workflow API now: the core contracts are still
  evolving, so this would freeze the wrong surface.

## Consequences

- Examples can start from `orskit::prelude::*` or explicit `orskit::*` modules.
- Focused crates remain independently usable and retain domain ownership.
- Future facade curation can add higher-level workflows without breaking the
  current module re-export direction.
- The package namespace no longer contains the generic `utils` package name.

## Validation

Workspace formatting, check, tests, and documentation should include the new
facade crate. A facade doctest verifies that the prelude exposes a basic frame
workflow.

## Provenance

This is original orskit packaging architecture. It does not derive from another
astrodynamics project's source, examples, tests, or internal structure.
