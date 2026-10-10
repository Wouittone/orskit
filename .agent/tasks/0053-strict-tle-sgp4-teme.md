# Task 0053: strict TLE parsing and SGP4 propagation to TEME

## Scope

- Issue: [#32](https://github.com/Wouittone/orskit/issues/32)
- Add strict TLE/3LE parsing with typed errors, validated serde, and lossless
  formatting using the approved external parser/element storage.
- Add opt-in Vallado-convention SGP4/SDP4 propagation to SI Cartesian TEME.
- Keep TLE I/O and propagation in separate crate boundaries; do not expose a
  new default facade feature.
- Do not add TEME transforms, legacy SGP/SDP modes, decay policy, or bundled
  downloadable data.

## Scientific and format contract

- Fixed 69-byte ASCII line records, checksums, columns, field ranges, matching
  catalog identity, Alpha-5 numbers, and the 1957 epoch-year pivot are checked
  before accepting a record.
- The parser retains exact source lines; output line contents are not rewritten.
- The feature-gated TLE adapter supports ephemeris type 0 only.
- SGP4 uses WGS-72 and AFSPC-compatible conventions through the unmodified
  `sgp4` 2.4.0 MIT dependency. The project does not copy or translate its source.
- Model output has an explicit TEME frame, SI typed position/velocity, and a
  Hifitime epoch. TLE epoch fractional day is converted exactly at its
  1e-8-day field precision.
- Near-Earth and deep-space published vectors are compared at 1 m and 1 mm/s
  tolerances. The evidence is limited to these vectors; it is not an operational
  or full-parity claim.

## Completion checklist

- [x] Add the strict `tle` crate, typed errors, source-preserving formatting,
  epoch pivot, checksum/range checks, and retained regression test.
- [x] Address PR #37 owner review: use `sgp4::Elements::from_tle` and
  `sgp4::Elements`, add named 3LE and serde round trips, remove scale constants
  and custom calendar tables, and remove the scoped fuzz files.
- [x] Add WGS-72 SGP4/SDP4 support behind dynamics and facade feature gates;
  preserve `Propagator<CartesianState>` seed semantics.
- [x] Add near-Earth/deep-space Vallado verification vectors and UTC/TEME/SI
  assertions.
- [x] Add the TLE/SGP4 guide, ADR-0053, and update architecture, parity, and
  provenance records.
- [x] Complete requested Rust 1.96.1 validation and record exact results below
  for the parent's PR publication.

## PR #37 review validation (2026-10-10, Windows, Rust 1.96.1)

All final commands below passed in the assigned `bot/tle-sgp4` checkout:

- `cargo +1.96.1 fmt --all --check`: passed.
- `cargo +1.96.1 check --workspace --all-targets --all-features --locked`: passed.
- `cargo +1.96.1 clippy --workspace --all-targets --all-features --locked -- -D warnings -D clippy::must-use-candidate`: passed.
- `cargo +1.96.1 nextest run --workspace --all-targets --all-features --locked --test-threads 1 --status-level fail --final-status-level fail`: passed, 282 tests across 40 binaries, zero skipped. Sequential execution limits benchmark memory contention; the full target set, including benchmarks, is retained.
- `cargo +1.96.1 test --workspace --doc --all-features --locked`: passed, 16 doctests.
- `cargo +1.96.1 doc --workspace --all-features --no-deps --locked`: passed.
- `cargo +1.96.1 check --workspace --no-default-features --locked`: passed; two pre-existing unused imports in `measurements/src/estimation.rs` remain untouched.
- `cargo +1.96.1 test -p tle --no-default-features --locked`: passed, 10 tests and one doctest.
- `cargo +1.96.1 test -p tle --no-default-features --features sgp4 --locked`: passed, 16 tests and one doctest.
- `cargo +1.96.1 test -p dynamics-sgp4 --no-default-features --locked`: passed, five tests and one doctest.
- `cargo +1.96.1 check -p orskit --no-default-features --features tle --locked`: passed.
- `cargo +1.96.1 check -p orskit --no-default-features --features sgp4 --locked`: passed.
- `pwsh -NoProfile -File scripts\check_crate_diagram.ps1 -Check`: passed.
- `git --no-pager diff --check`: passed.

Parser-only testing initially exposed an ungated SGP4 integration target;
its manifest now requires the `sgp4` feature, and both feature configurations
pass. No change to propagation equations, typed units, frame semantics, or
independent Vallado expected vectors was needed.

Rust Analyzer and docs.rs MCP tools were not exposed in this session. Published
docs.rs API/feature/validation documentation and installed package metadata/
license were inspected directly; actual PR #37 diff/reviews/comments were
retrieved with `gh`. No review reply, resolution, merge, rebase, or push was
performed. Parent publication remains pending; no unresolved dependency choice
requires user input. The text writer deliberately preserves accepted records,
not arbitrary edited OMM elements (the approved dependency has no TLE writer).

## References and non-reuse

See the SGP4 and TLE entries in `.agent/PROVENANCE.md`. The strict validation
and lossless-writing adapters are original project code; the approved external
dependency parses/stores the orbital data and supplies propagation. It is
linked unmodified; no implementation source, tests, or examples from it or
another astrodynamics library are copied.
