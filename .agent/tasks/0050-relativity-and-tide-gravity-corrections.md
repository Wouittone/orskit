# Task: implement the independently valid #15 relativity slice

## Scope and dependency assessment

- Issue: [#15](https://github.com/Wouittone/orskit/issues/15)
- Promote the correction-only slice from [PR #24](https://github.com/Wouittone/orskit/pull/24),
  source commit `c71a9c93595d019db790c511f55d24950859c43f`, onto main
  `84b919f1e630894547e09d7881e69973fa954cc1` in [PR #29](https://github.com/Wouittone/orskit/pull/29).
  PR #24 was merged into a feature branch, not main.
- Merge current main `dc6c82c721808f3b2682b63ada4a0e804ed827e1`, retaining
  its ERA provider, Vern9 gate, opt-in Gauss–Jackson (#28), and general
  spherical harmonics (#30), alongside this correction-only slice.
- Implement the correction-only Schwarzschild 1PN monopole term.
- Defer solid-Earth, ocean, and pole-tide implementation: the harmonic model
  has no epoch-dependent coefficient-delta composition, no tide-specific
  provider/formula contracts are present, and #17 supplies UT1-based ERA only
  without polar motion. Independent tide reference vectors are also absent.
- No bundled data, parser, or implicit download.
- Python/JVM binding exposure remains deferred while the Rust core stabilizes.

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
- [x] Record exact main-based validation results below

## Source PR #24 verification evidence (historical, not a main-based rerun)

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

## Initial main-based PR #29 verification evidence (before the main refresh)

- Toolchain: `rustc 1.96.1 (31fca3adb 2026-06-26)`.
- `cargo +1.96.1 fmt --all --check` — passed.
- `cargo +1.96.1 test -p dynamics-relativity --all-features --locked -j 1` —
  passed, six unit tests and one doctest. The implementation and its tests are
  unchanged from source commit `c71a9c93595d019db790c511f55d24950859c43f`.
- A separate 70-digit decimal evaluation of the IERS equation confirms the
  non-radial SI vector; reference rounding differs by less than
  `2e-24 m/s²`, below the test's `3e-23 m/s²` comparison tolerance.
- `cargo +1.96.1 nextest run --workspace --lib --bins --tests --all-features --locked --jobs 1` —
  passed, 242 tests, zero skipped, using cargo-nextest 0.9.146 installed outside
  the repository. This is the main-based count, not the source branch's 243.
- `cargo +1.96.1 check --workspace --all-targets --all-features --locked -j 1` —
  passed.
- `cargo +1.96.1 clippy --workspace --all-targets --all-features --locked -j 1 -- -D warnings -D clippy::must-use-candidate` —
  passed.
- `cargo +1.96.1 test --workspace --doc --all-features --locked -j 1` and
  `cargo +1.96.1 doc --workspace --all-features --no-deps --locked -j 1` —
  passed (12 workspace doctests).
- `cargo +1.96.1 check -p dynamics --no-default-features --features relativity --locked -j 1` —
  passed.
- `cargo +1.96.1 check -p orskit --no-default-features --features relativity --locked -j 1` —
  passed.
- `cargo +1.96.1 check -p orskit --features "relativity,spherical-harmonics,numerical,earth-rotation" --locked -j 1` —
  passed.
- Per-package `cargo +1.96.1 check --locked --manifest-path "$manifest" --no-default-features -j 1`
  over every workspace manifest from Cargo metadata — passed; the existing
  `measurements` minimal build reports unused `fmt` and `Position` imports.
- `RUSTUP_TOOLCHAIN=1.96.1 pwsh -NoProfile -File scripts/check_crate_diagram.ps1 -Check`
  and `git diff --check` — passed.
- Full `nextest --all-targets` is intentionally not rerun here: the source
  report above records the pre-existing 100 MiB CCSDS benchmark allocation
  abort. Checking/compiling all targets is not evidence that those benchmarks
  executed successfully.
- Secret scanning of the changed files — no secrets detected.
- Automated validation was attempted: its code-review binary was unavailable,
  and CodeQL timed out. Neither is claimed as a completed clean scan.
- PR CI reports `action_required`, with no jobs executed. Required approval
  and branch rules remain unchanged; this PR is not merged.

## Current-main refresh verification evidence

Main `dc6c82c721808f3b2682b63ada4a0e804ed827e1` is the second parent of
merge commit `72ab569`. Relativity ADR/task numbering is now 0050; unrelated
Gauss–Jackson ADR-0048 and general-harmonics ADR-0049 records are unchanged.
The relativity implementation/tests and main's two new model implementations
were compared byte-for-byte with their respective pre-merge sources.

With `RUSTUP_TOOLCHAIN=1.96.1`, the following commands passed:

- `cargo fmt --all --check`.
- `cargo clippy --workspace --all-targets --all-features --locked -j 1 -- -D warnings`.
- `cargo test -p dynamics-relativity --all-features --locked -j 1` —
  six unit tests and one doctest.
- `cargo test --workspace --doc --all-features --locked -j 1` —
  14 workspace doctests.
- `cargo doc --workspace --all-features --no-deps --locked -j 1`.
- `cargo check -p orskit --no-default-features --features relativity --locked -j 1`.
- `pwsh -NoProfile -File scripts/check_crate_diagram.ps1` and
  `pwsh -NoProfile -File scripts/check_crate_diagram.ps1 -Check` —
  regenerated diagram matches locked Cargo metadata.
- `git diff --check`.

The known pre-existing 100 MiB CCSDS benchmark allocation failure was not
rerun locally; no full-suite local result is claimed. GitHub CI run
[37153049754](https://github.com/Wouittone/orskit/actions/runs/37153049754)
passed on `c0c9b29`. Secret scans found no secrets. Automated review was
unavailable and CodeQL timed out again; neither is claimed as a clean scan.
Manual review confirmed both sets of features/parity evidence, source
preservation, reference numbering, and lockfile/diagram consistency.

Issue #15 remains partially addressed: tide coefficient-delta composition,
tide-specific versioned data/formula/reference vectors, and full orientation
inputs remain missing. No tide force, Newtonian term, issue closure, or approval
rule change is introduced.
