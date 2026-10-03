# Task: implement general caller-supplied spherical-harmonic gravity

## Scope and dependencies

- Issue: [#11](https://github.com/Wouittone/orskit/issues/11)
- Stacked on `wouittone-provenance-qualified-eop-provider` (#17 / PR #21).
- Depends on composable evaluable force models (#10) and the body-fixed
  transform provider boundary (#17).
- Excludes #15 tide and relativity work. No changes to #17 files.

## Scientific/API contract

- Support caller-selected degree/order truncation, including zonal, tesseral,
  and sectorial terms, using fully normalized geodetic 4π coefficients.
- Require source/product/revision provenance, optional checksum, degree/order
  coverage, normalization, tide system, coefficient frame, and explicit
  coefficient epoch semantics.
- Require frame-transform provenance and validate each exact transform
  request/response, including epoch, time scale, frame pair, origin, and
  direction.
- Return framed SI acceleration. Do not bundle/fetch coefficient or EOP data;
  do not parse datasets; do not silently convert or double count tide
  conventions.

## Independent validation

- Compare a separately computed potential and central finite-difference
  gradient for multiple degree/order truncations, radii, axes (including both
  polar axes), and transform epochs.
- Check central and J2-only reductions against point-mass and closed-form
  equations.
- Check typed metadata, normalization, frame, coverage, missing-coefficient,
  epoch-range, and transform-response failures.

## Completion record

- [x] General normalized degree/order evaluator and caller provider contracts
- [x] Frame/epoch and coefficient provenance and typed validity errors
- [x] Opt-in dynamics and public facade feature
- [x] Independent multi-truncation/axis/radius/epoch and J2 tests
- [x] User guide, ADR, provenance, and parity updates
- [x] Rust 1.96.1 workspace checks (record exact results below)
- [x] Record final checks and remaining limitations

## Verification evidence

- Toolchain: `rustc 1.96.1 (31fca3adb 2026-06-26)`.
- `cargo fmt --all --check` — passed.
- `cargo check --workspace --all-targets --all-features --locked` — passed.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings -D clippy::must-use-candidate` — passed.
- `cargo nextest run --workspace --all-targets --all-features --locked` — passed.
- `cargo test --workspace --doc --all-features --locked` — passed.
- `cargo doc --workspace --all-features --no-deps --locked` — passed.
- `cargo check -p orskit --features spherical-harmonics --locked` — passed.

The issue is already tracked in the roadmap project as P1 / In Progress in the
M2 milestone, with its dependencies and target date. This implementation is
intentionally stacked on PR #21. No PR is merged as part of this task.
