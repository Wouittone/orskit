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
- [ ] Run and record all requested validation results.
- [x] Secret scan of implementation, tests, guide, and project records.
- [ ] Automated code-review/CodeQL validation.

## Validation results

Pending final Rust 1.96.1 checks and automated validation. The focused test
command already run during implementation was:

- `cargo test -p dynamics-spherical-harmonics --all-features --locked` — passed,
  15 unit tests and one doctest.

## Limitations

No authoritative coefficient-rate product, secular-rate reference vector,
solid/ocean/pole tide formula, or tide data is included. Numerical tests use
manufactured rates and independently evaluate the potential gradient; they
validate coefficient composition and the evaluator, not a particular gravity
model or tide product.
