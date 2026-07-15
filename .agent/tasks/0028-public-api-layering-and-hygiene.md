# Task: establish public API layering and hygiene rules

## Parity target

- Ledger row: cross-cutting API architecture
- Current status: the facade and focused crates exist, but the public-surface
  policy was implicit and recent audits found custom conversion and cloning
  convenience APIs.
- Intended status after this task: repository guidance explicitly requires one
  small high-level domain API over private vector/matrix kernels, with explicit
  ownership and standard trait conventions.

## User workflow

Users select domain capabilities through the `orskit` facade and focused
domain crates. They do not need to select, call, or depend on a separate
vector/matrix algorithm API.

## Scientific contract

- Inputs and units: public domain inputs remain typed quantities.
- Outputs and units: public domain outputs remain typed quantities.
- Frames/epochs/time scales: remain explicit at domain boundaries.
- Conventions and valid regimes: remain owned by the high-level model.
- External data requirements: remain explicit model configuration.
- Errors and singularities: remain typed high-level errors.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| Project architecture | project-owned | Public API and performance policy | README and `.agent` guidance |

## Design

- Affected crates/layers: repository documentation and all future public APIs.
- Public API: one domain-oriented surface; internal vector/matrix kernels are
  implementation details. Standard traits, borrowing, and ownership transfer
  take precedence over custom conversion and clone-based convenience methods.
- Rejected alternatives: exposing a separate low-level API or relying on
  undocumented contributor convention.
- ADR required: no; this documents and enforces the existing architecture.

## Validation

- Unit cases: not applicable; no runtime behavior changes.
- Invariants/properties: audit the public surface for hidden cloning,
  allocation, low-level representation leakage, raw domain scalars, and
  duplicate trait contracts.
- Independent reference vectors: not applicable.
- Differential/scenario tests: not applicable.
- Tolerances and justification: not applicable.
- Benchmarks: not applicable; this task makes no performance claim.

## Completion checklist

- [x] Repository-level API layering policy documented
- [x] Public-surface audit completed and reported
- [ ] Code remediation: deliberately deferred until the reported findings are
  prioritized
