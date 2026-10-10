# Task 0053: strict TLE parsing and SGP4 propagation to TEME

## Scope

- Issue: [#32](https://github.com/Wouittone/orskit/issues/32)
- Add strict TLE parsing with typed errors and lossless formatting.
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
  epoch pivot, checksum/range checks, regression test, and fuzz target.
- [x] Add WGS-72 SGP4/SDP4 support behind dynamics and facade feature gates;
  preserve `Propagator<CartesianState>` seed semantics.
- [x] Add near-Earth/deep-space Vallado verification vectors and UTC/TEME/SI
  assertions.
- [x] Add the TLE/SGP4 guide, ADR-0053, and update architecture, parity, and
  provenance records.
- [ ] Complete requested Rust 1.96.1 validation and record exact results in the
  PR description.

## References and non-reuse

See the SGP4 and TLE entries in `.agent/PROVENANCE.md`. The parser and adapter
are original project code. The separately licensed propagation dependency is
linked unmodified; no implementation source, tests, or examples from it or
another astrodynamics library are copied.
