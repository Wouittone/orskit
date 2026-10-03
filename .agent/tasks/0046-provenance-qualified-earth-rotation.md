# Task: add a provenance-qualified ERA-only Earth-rotation provider

## Parity target

- Ledger row: Geometry / Frames, transforms, Earth orientation.
- Status remains Partial: implement one caller-data-backed CIRS/TIRS Earth-spin
  slice, not a complete GCRF-to-ITRF realization.
- Dependent #11/#15 work is excluded.

## Scientific contract

- Implement the IAU 2000 Earth Rotation Angle (ERA) equation from IERS
  Conventions 2010 Chapter 5 §5.5.3 Eq. 5.15 using caller-supplied UT1−UTC.
- The supported requests are same-origin Earth-centered CIRS↔TIRS only.
  Represent them with a same-origin frame-transform contract that does not
  imply an inertial source frame. Keep request epoch and time scale, frame
  pair/origin, relative angular velocity, data revision/checksum, convention,
  and time-scale provenance observable with the result.
- Samples are finite, strictly chronological UTC epochs from 1972-01-01 onward.
  Require at least two samples, interpolate continuous UT1−TAI over elapsed
  TAI seconds, include its derivative in angular velocity, use closed coverage,
  and reject extrapolation.
- Do not bundle or retrieve data. No parser, polar motion, precession-nutation,
  CIP/pole offsets, tides, terrestrial-realization corrections, harmonics, or
  dependent issue implementation is in scope.

## Validation

- Independent standard vector: JD(UT1) = 2451545.0 produces ERA =
  280.46061837504 degrees; test with a `2e-12` radian matrix tolerance.
- Validate forward and reverse Cartesian position and velocity, including the
  omega-cross-position term and interpolated angular-rate correction.
- Validate exact coverage endpoints and out-of-coverage failure, EOP
  chronology/finite-value/provenance checks, unsupported frame/convention,
  alternate request time scale, and leap-second continuity.
- The separate provider limitation must remain visible in crate/API docs and
  `.agent/PARITY.md`.

## Completion record

- [x] Standalone implementation crate and opt-in facade feature
- [x] Independent ERA vector and transform/invariant tests
- [x] Typed errors and stable provenance records
- [x] Public example and Earth-orientation guide
- [x] Provenance, decision, and parity records
- [x] Rust 1.96.1 workspace checks

## PR #21 scope-review correction

On 2026-10-03, the fetched `origin/main` and live GitHub `main` both resolved
to `b3228caee857e7c9c7f0ecc393b01f7addaa335c`. At PR head
`9aaebf0f1e48423b1b5b121e86905e746e0c28d3`, the #11 implementation commit
`06c077f` remained in the comparison diff despite merge commit `94b5aee`.
The #11 patch was removed from the working tree without rewriting history:
the existing low-degree gravity boundary and ADR were restored, and the
general evaluator, added facade feature, guide, gravity record (now
[ADR-0049](../decisions/0049-general-spherical-harmonics-gravity.md)), and related ledger
changes were removed. The #17 provider and shared frame contracts remain.

Review comments 4173908679 and 4173931140 are accepted as scope corrections.
Comment 4173931157 identifies a valid provenance mismatch in the removed
`SourcedBodyFixedTransformProvider`; that wrapper is not retained or expanded
in this #17-only change. Any general-gravity provenance fix belongs to #11.

The gravity record's original workspace `nextest` evidence described the separate #11 work, not
validation of this corrected comparison. The corrected #17-only tree was
validated on Rust 1.96.1:

- `cargo fmt --all --check` — passed.
- `cargo nextest run --workspace --all-features --locked` — passed: 235 tests, 0 skipped.
- `cargo check --workspace --all-targets --all-features --locked` — passed.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings -D clippy::must-use-candidate` — passed.
- `cargo test --workspace --doc --all-features --locked -j 1` — passed: 11 doctests.
- `cargo doc --workspace --all-features --no-deps --locked -j 1` — passed.
- `cargo check -p orskit --no-default-features --locked` — passed.
- `.\scripts\check_crate_diagram.ps1 -Check` — passed.
- `git diff --check` — passed.
- Comparison with the verified main tree — no remaining changes to
  `crates/dynamics`, ADR-0047, the gravity record, or the spherical-harmonics guide;
  the facade retains only its #17 `earth-rotation` additions.

## Parent issue disposition

This PR delivers the first ERA-only CIRS/TIRS slice. It does not complete
issue #17's inertial-to-body-fixed provider workflow or close the issue; the
celestial-intermediate and polar-motion composition remains follow-up work
tracked on issue #17.
