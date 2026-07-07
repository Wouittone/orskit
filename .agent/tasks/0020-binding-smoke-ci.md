# Task 0020: native binding smoke checks in CI

## Parity target

- Ledger row: Bindings / Python package; Bindings / JVM-language package
- Current status: Partial, with local experimental native adapters
- Intended status after this task: Partial, with CI smoke checks for native
  binding workspaces and namespaced dependency manifests

## User workflow

Push to `main` or open a pull request and have CI catch native Python/JVM
adapter compilation regressions, including stale internal package names.

## Scientific contract

- Inputs and units: no new scientific inputs.
- Outputs and units: no new scientific outputs.
- Frames/epochs/time scales: unchanged from the Rust core APIs exposed by the
  experimental adapters.
- Conventions and valid regimes: compilation smoke checks only; no binding
  feature stabilization claim.
- External data requirements: GitHub Actions Python 3.12 for PyO3 discovery;
  no scientific datasets.
- Errors and singularities: stale package names and lockfile drift are rejected
  by locked CI checks.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| Original orskit CI policy | Project-owned MIT/Apache-2.0 work | Binding smoke checks should compile native adapters without adding binding features | `.github/workflows/build.yml`, binding manifests |

## Design

- Affected crates/layers: GitHub Actions workflow; `bindings/python` and
  `bindings/java` manifests and lockfiles.
- Public API: none.
- Rejected alternatives: stabilize binding features now; run only the core
  workspace and leave bindings unchecked; keep stale un-namespaced dependency
  aliases.
- ADR required: no; this executes the existing roadmap CI item.

## Validation

- Unit cases: native Java binding cargo tests.
- Invariants/properties: binding manifests depend on namespaced internal
  packages; CI uses `--locked`.
- Independent reference vectors: none.
- Differential/scenario tests: none.
- Tolerances and justification: none.
- Benchmarks: none.

## Completion checklist

- [x] Implementation and typed errors
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance recorded
- [x] Parity ledger updated
- [x] Relevant checks pass
- [x] Binding impact handled
