# Task: turn the external review into an executable improvement program

## Parity target

- Ledger row: all rows (planning only)
- Current status: mixed
- Intended status after this task: unchanged; no scientific claim follows from planning

## User workflow

A maintainer can see which review observations are already satisfied, which
actions are next, which scientific dependencies block later work, and which
binding/community tasks are deliberately parked or owner-operated.

## Scientific contract

- Inputs and units: not applicable
- Outputs and units: not applicable
- Frames/epochs/time scales: not applicable
- Conventions and valid regimes: roadmap ordering follows `ARCHITECTURE.md`
- External data requirements: none
- Errors and singularities: not applicable

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| July 2026 external review supplied by the maintainer | Review text; not implementation material | Improvement suggestions and reported gaps | `REVIEW_IMPROVEMENTS.md` |

## Design

- Affected crates/layers: repository governance only
- Public API: unchanged
- Rejected alternatives: treating stale review statements as current facts;
  reactivating bindings before core stabilization; implementing placeholders
- ADR required: no

## Validation

- Unit cases: not applicable
- Invariants/properties: every recommendation has a disposition or backlog item
- Independent reference vectors: not applicable
- Differential/scenario tests: not applicable
- Tolerances and justification: not applicable
- Benchmarks: not applicable

## Completion checklist

- [x] Review audited against the repository
- [x] Dependency-ordered checklist added
- [x] Already-satisfied and stale observations recorded
- [x] Binding work explicitly deferred
- [x] Wave 1 branches integrated and verified
