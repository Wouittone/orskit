# Task: implement the independently valid #15 relativity slice

## Scope and dependency assessment

- Issue: [#15](https://github.com/Wouittone/orskit/issues/15)
- Stack on the general spherical-harmonics branch / PR #22. Do not alter its
  base branch or create a commit/PR before integration approval.
- Implement the correction-only Schwarzschild 1PN monopole term.
- Defer solid-Earth, ocean, and pole-tide implementation: the harmonic model
  has no epoch-dependent coefficient-delta composition, no tide-specific
  provider/formula contracts are present, and #17 supplies UT1-based ERA only
  without polar motion. Independent tide reference vectors are also absent.
- No bundled data, parser, or implicit download.

## Scientific contract

- Use the IERS Conventions (2010) Chapter 10 §10.2 Eq. 10.8 correction for a
  test particle around an isolated, spherical, non-rotating monopole in
  harmonic coordinates.
- Inputs and outputs are typed SI quantities; input state and returned
  acceleration retain one explicitly selected inertial frame and origin.
- Configure the central parameter and maximum
  `max(v²/c², μ/(r c²))` regime bound explicitly.
- Return only the 1PN correction, never Newtonian attraction.
- Reject frame/origin mismatch, zero radius, non-finite geometry/results, and
  out-of-bound evaluation.
- Do not claim spin, external-body, nonspherical relativistic couplings, full
  Earth orientation, or tide parity.

## Completion record

- [x] Correction-only 1PN force model and opt-in dynamics/facade features
- [x] IERS independent reference-vector and regime/error tests
- [x] Relativity guide, ADR, provenance and parity updates
- [x] Tide prerequisites and blockers documented; no placeholder tide model
- [x] Rust 1.96.1 format, MSRV, all-feature check/lint/tests/docs and facade checks
- [x] Record exact validation results below

## Verification evidence

- Toolchain: `rustc 1.96.1 (31fca3adb 2026-06-26)`.
- `cargo +1.96.1 fmt --all --check` — passed.
- `cargo +1.96.1 check --workspace --all-targets --all-features --locked -j 1` — passed.
- `cargo +1.96.1 clippy --workspace --all-targets --all-features --locked -j 1 -- -D warnings -D clippy::must-use-candidate` — passed.
- `cargo +1.96.1 nextest run --workspace --lib --bins --tests --all-features --locked --jobs 1` — passed, 243 tests.
- `cargo +1.96.1 test --workspace --doc --all-features --locked -j 1` — passed.
- `cargo +1.96.1 doc --workspace --all-features --no-deps --locked -j 1` — passed.
- `cargo +1.96.1 check -p dynamics --features relativity --locked -j 1` — passed.
- `cargo +1.96.1 check -p orskit --features "relativity,spherical-harmonics,numerical" --locked -j 1` — passed.
- `pwsh -File scripts\check_crate_diagram.ps1 -Check` — passed.
- `git diff --check` — passed.
- `cargo +1.96.1 nextest run --workspace --all-targets --all-features --locked --jobs 1` — did not complete: the existing `ccsds::bench/oem::sequential_collect` 100 MiB benchmark requested a 738,197,504-byte allocation and aborted. The 243 workspace unit/integration tests passed using the `--lib --bins --tests` target selection.

The independent non-radial IERS reference vector is tested in the new crate.
Tides remain intentionally unimplemented pending the providers, semantics,
and independent references described above.
