# Task 0022: defer language bindings

## Parity target

- Ledger row: Bindings / Python package; Bindings / JVM-language package
- Current status: Partial experimental scaffolds with CI smoke checks
- Intended status after this task: Explicitly deferred until the Rust core API
  is stable enough to support binding design and validation

## User workflow

Contributors work exclusively on the Rust workspace; CI validates only the
Rust workspace and does not require Python or Java toolchains.

## Scientific contract

- Inputs and units: Not applicable; no scientific behavior changes.
- Outputs and units: Not applicable.
- Frames/epochs/time scales: Not applicable.
- Conventions and valid regimes: Binding implementation is deferred.
- External data requirements: None.
- Errors and singularities: Not applicable.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| Project-owned CI and binding scaffolds | MIT OR Apache-2.0 project work | Existing bindings are separate workspaces and can be excluded without changing the Rust dependency graph | CI workflow, engineering guidance, README, parity ledger |

## Design

- Affected crates/layers: GitHub Actions workflow and contributor-facing
  documentation; the binding workspaces remain retained but are not built.
- Public API: No Rust API change; Python and JVM APIs are unavailable.
- Rejected alternatives: Delete the experimental binding workspaces; retain CI
  smoke checks that distract from Rust core stabilization.
- ADR required: No; this is a reversible validation-scope change.

## Validation

- Unit cases: Existing Rust workspace tests.
- Invariants/properties: Rust CI has no Python or JVM setup steps or binding
  Cargo invocations.
- Independent reference vectors: Not applicable.
- Differential/scenario tests: Not applicable.
- Tolerances and justification: Not applicable.
- Benchmarks: Not applicable.

## Completion checklist

- [x] Implementation and typed errors
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance recorded
- [x] Parity ledger updated
- [x] Relevant checks pass
- [x] Binding impact handled or explicitly deferred
