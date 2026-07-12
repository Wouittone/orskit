# Task 0021: pinned Rust toolchain policy

## Parity target

- Ledger row: CI/MSRV/toolchain foundation
- Current status: CI used implicit stable Rust and packages did not declare a
  `rust-version`
- Intended status after this task: validated Rust toolchain pinned to 1.96.1
  across workspace metadata, bindings, and CI

## User workflow

Clone the repository and have Cargo/rustup select the same Rust toolchain used
by CI, with package metadata making the currently supported compiler explicit.

## Scientific contract

- Inputs and units: none.
- Outputs and units: reproducible compiler/tooling selection.
- Frames/epochs/time scales: none.
- Conventions and valid regimes: Rust 1.96.1 is the validated toolchain, not a
  claim that older compilers work.
- External data requirements: rustup/GitHub Actions toolchain installation.
- Errors and singularities: implicit moving-`stable` compiler behavior is
  rejected for CI and local baseline checks.

## Provenance

| Reference                        | Class/terms                   | Facts used                                                           | Evidence/code affected                        |
|----------------------------------|-------------------------------|----------------------------------------------------------------------|-----------------------------------------------|
| Local validated toolchain output | Build environment observation | `rustc 1.96.1` and `cargo 1.96.1` are the locally validated versions | `rust-toolchain.toml`, manifests, CI workflow |

## Design

- Affected crates/layers: workspace manifest, focused crate manifests, binding
  manifests, CI workflow, engineering docs.
- Public API: none.
- Rejected alternatives: leave CI on moving `stable`; declare an older MSRV
  without testing it; omit binding manifests from the policy.
- ADR required: no; this executes the roadmap CI/MSRV foundation item.

## Validation

- Unit cases: none.
- Invariants/properties: all packages declare the same validated compiler
  floor; CI installs the same toolchain.
- Independent reference vectors: none.
- Differential/scenario tests: none.
- Tolerances and justification: none.
- Benchmarks: none.

## Completion checklist

- [x] Implementation and typed errors
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance recorded
- [x] Parity ledger impact considered; no scientific capability row changed
- [x] Relevant checks pass
- [x] Binding impact handled
