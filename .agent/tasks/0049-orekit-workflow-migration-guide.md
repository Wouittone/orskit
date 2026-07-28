# Task: publish an Orekit workflow migration guide

## Parity target

- Ledger row: stable public Rust facade
- Current status: Partial
- Intended status after this task: Partial, with a concept-level migration guide

## User workflow

An Orekit user can identify the corresponding supported orskit concepts,
understand deliberate architectural differences, select Cargo capabilities,
and recognize workflows that do not yet have a supported migration path.

## Scientific contract

The guide introduces no numerical behavior. It preserves the project rules
that units, epochs, frames, providers, data identity, and valid regimes remain
explicit and that unsupported parity is never implied by similar names.

## Provenance

Orekit 13.1.7 public API documentation is used only for public concept names
and workflow behavior. No source code, test, example, implementation structure,
or distinctive prose is used.

## Design

- Affected layers: user documentation and roadmap evidence.
- Public API: none.
- Rejected alternative: a one-for-one Java class table that would obscure
  Rust ownership and falsely imply unsupported parity.
- ADR required: no.

## Validation

- Links and type names are checked against current documentation and facade
  exports.
- Named facade capabilities are checked by the all-feature documentation build;
  the standalone guide examples remain illustrative until the release guide
  gains a Markdown snippet harness.
- The parity ledger remains the authority for capability status.

## Completion checklist

- [x] Concept and workflow mappings documented
- [x] Deliberate architectural differences explained
- [x] Unsupported workflows stated
- [x] Provenance recorded
- [x] Roadmap updated
- [x] Documentation checks pass
- [x] Binding work remains deferred
