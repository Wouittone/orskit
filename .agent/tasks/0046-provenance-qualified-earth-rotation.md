# Task: add a provenance-qualified ERA-only Earth-rotation provider

## Parity target

- Ledger row: Geometry / Frames, transforms, Earth orientation.
- Status remains Partial: implement one caller-data-backed Earth-spin slice,
  not a complete GCRF-to-ITRF realization.
- Dependent #11/#15 work is excluded.

## Scientific contract

- Implement the IAU 2000 Earth Rotation Angle (ERA) equation from IERS
  Conventions 2010 Chapter 5 §5.5.3 Eq. 5.15 using caller-supplied UT1−UTC.
- The supported request is same-origin Earth-centered GCRF/ITRF2020 only.
  Keep request epoch and time scale, direction, frame pair/origin, body angular
  velocity, data revision/checksum, convention, and time-scale provenance
  observable with the result.
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
- Validate direct and inverse Cartesian position and velocity, including the
  omega-cross-position term and interpolated angular-rate correction.
- Validate exact coverage endpoints and out-of-coverage failure, EOP
  chronology/finite-value/provenance checks, unsupported frame/convention,
  alternate request time scale, and leap-second continuity.
- The separate provider limitation must remain visible in crate/API docs,
  `.agent/PROVENANCE.md`, and `.agent/PARITY.md`.

## Completion record

- [x] Standalone implementation crate and opt-in facade feature
- [x] Independent ERA vector and transform/invariant tests
- [x] Typed errors and stable provenance records
- [x] Public example and Earth-orientation guide
- [x] Provenance, decision, and parity records
- [x] Rust 1.96.1 workspace checks
