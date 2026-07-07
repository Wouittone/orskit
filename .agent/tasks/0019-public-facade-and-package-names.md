# Task 0019: public facade and package names

## Parity target

- Ledger row: Bindings / Stable public Rust facade
- Current status: Not assessed
- Intended status after this task: Partial, with a thin public `orskit` crate,
  focused crate re-exports, and a conservative prelude

## User workflow

Add one dependency on `orskit`, import `orskit::prelude::*`, and construct a
basic typed frame or state value without first learning every internal crate
name.

## Scientific contract

- Inputs and units: no new physical inputs.
- Outputs and units: no new scientific values; facade re-exports existing
  typed APIs and units unchanged.
- Frames/epochs/time scales: unchanged from the focused crates.
- Conventions and valid regimes: pre-alpha import surface only; no stability
  or parity claim beyond the re-exported crates.
- External data requirements: none.
- Errors and singularities: unchanged from the focused crates.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| Original orskit packaging policy | Project-owned MIT/Apache-2.0 work | Thin facade and namespaced packages | `crates/orskit`, `crates/utils/Cargo.toml`, ADR-0016 |

## Design

- Affected crates/layers: workspace manifest, new `orskit` facade crate,
  `orskit-utils` package rename, architecture/README/parity docs.
- Public API: `orskit::{bodies, ccsds, core, dynamics, frames, measurements,
  units, utils}` and `orskit::prelude`.
- Rejected alternatives: no facade; root-level glob re-export of every focused
  crate; stable workflow API before Rust contracts settle.
- ADR required: ADR-0016.

## Validation

- Unit cases: facade doctest imports `orskit::prelude::*` and uses
  `ReferenceFrame::gcrf()`.
- Invariants/properties: facade adds no scientific behavior and does not alter
  units, frames, or error semantics.
- Independent reference vectors: none.
- Differential/scenario tests: workspace build and tests cover dependency
  integration.
- Tolerances and justification: none.
- Benchmarks: none.

## Completion checklist

- [x] Implementation and typed errors
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance recorded
- [x] Parity ledger updated
- [x] Relevant checks pass
- [x] Binding impact explicitly deferred
