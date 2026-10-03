# Task: implement time-varying spherical-harmonic coefficient deltas

## Scope and dependency assessment

- Issue: [#31](https://github.com/Wouittone/orskit/issues/31)
- Keep the existing static `dynamics-spherical-harmonics` API and output
  behavior unchanged.
- Add explicit epoch-dependent normalized coefficient-delta composition and a
  linear secular-rate provider; keep all coefficient data caller-supplied.
- This is an opt-in capability through the existing spherical-harmonics crate
  and facade features. No new crate, dependency, parser, dataset, or binding
  surface is required.

## Scientific/API contract

- Add dimensionless normalized `ΔC̄ₙₘ`/`ΔS̄ₙₘ` to a static field's coefficients
  before the potential-gradient evaluation.
- Every provider declares provenance, maximum degree/order, normalization, tide
  system, coefficient frame, and inclusive epoch coverage with an explicit time
  scale.
- Reject metadata mismatches and missing data using named construction or
  evaluation errors. Do not extrapolate outside the declared coverage.
- Linear coefficient rates use typed SI frequency (`s⁻¹`) values and a
  reference `Epoch` inside the declared coverage. The unit central coefficient
  cannot change.
- Keep all provider and evaluation operations deterministic and offline. Do not
  claim physical-tide support from generic coefficient-delta composition.

## Completion record

- [x] Delta provider contract, metadata checks, and separate composed force
  model.
- [x] Linear secular-rate provider with SI rates and explicit reference epoch.
- [x] Bit-identical zero-delta, static-sum, independent-gradient, and typed error
  tests.
- [x] Guide, ADR-0052, provenance, and parity update.
- [x] Run and record the requested focused and workspace validation results.
- [x] Secret scan of implementation, tests, guide, and project records.
- [ ] Automated code-review/CodeQL validation.

## Validation results

Toolchain: `rustc 1.96.1 (31fca3adb 2026-06-26)`.

- `cargo fmt --all --check` — passed.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
  — passed.
- `cargo test -p dynamics-spherical-harmonics --all-features --locked` —
  passed, 15 unit tests and one doctest.
- `cargo test --workspace --doc --all-features --locked` — passed, 14 doctests.
- `cargo doc --workspace --all-features --no-deps --locked` — passed.
- `cargo check -p orskit --no-default-features --features spherical-harmonics
  --locked` — passed.
- `pwsh -NoProfile -File scripts/check_crate_diagram.ps1 -Check` — passed;
  `docs/architecture.md` matches Cargo metadata.
- `git diff --check` — passed.
- `runtime-tools-secret_scanning` — no secrets detected in modified or added
  files.
- The workspace all-target test suite was not run locally, per the issue's
  instruction to skip its known 100 MiB CCSDS benchmark OOM risk. The focused
  crate test and requested workspace doctests were run separately.

Automated code review and CodeQL validation remain to be recorded.

## Limitations

No authoritative coefficient-rate product, secular-rate reference vector,
solid/ocean/pole tide formula, or tide data is included. Numerical tests use
manufactured rates and independently evaluate the potential gradient; they
validate coefficient composition and the evaluator, not a particular gravity
model or tide product.
