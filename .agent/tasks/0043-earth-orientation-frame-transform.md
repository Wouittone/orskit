# Task: transform GCRF and ITRF2020 states with verified Earth orientation

## Parity target

- Ledger row: Geometry / Frames, transforms, Earth orientation
- Current status: Partial, with an abstract reference-data supplier only
- Intended status after this task: Partial, with one concrete independently
  validated IERS 2010 GCRF/ITRF2020 position-and-velocity provider

## User workflow

A caller verifies an exact local Earth-orientation artifact, decodes it to
typed UT1-TAI and polar-motion samples, selects a maximum linear-interpolation
span, and constructs `Iers2010EarthOrientation`. Through the existing
`ReferenceDataKinematicFrameTransform`, the caller transforms finite position
and velocity between GCRF and ITRF2020 at a covered epoch without receiving or
managing a raw matrix.

## Scientific contract

- Inputs and units: Hifitime sample/evaluation epochs; typed time for UT1-TAI;
  typed angles for IERS `xp`/`yp`; typed position and velocity.
- Outputs and units: finite typed position and velocity in the requested
  GCRF or ITRF2020 frame.
- Frames/epochs/time scales: GCRF and ITRF2020 only; TT drives precession and
  nutation; UT1 drives ERA; interpolation uses uniform TAI.
- Conventions and valid regimes: CIO-based IERS 2010, IAU 2006 precession,
  IAU 2000A nutation, IERS TIO locator, linear EOP interpolation, 0.5-second
  second-order rotation derivative. Observed `dX`/`dY` and subdaily tidal or
  libration corrections are not synthesized; callers supply any such
  corrections required by their selected EOP product.
- External data requirements: one caller-selected `VerifiedArtifact` whose
  exact interval equals the strictly increasing decoded sample interval.
- Errors and singularities: non-finite samples, insufficient/non-increasing
  samples, mismatched/outside coverage, excessive interpolation gaps,
  unavailable derivative stencil, unsupported frames, and non-finite output
  are typed.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| IERS Technical Note 36, IERS Conventions (2010), Chapter 5, update 2012-08-10 | Public authoritative convention | `Q R W`/inverse semantics, CIO procedure, TT, UT1, ERA, polar motion, TIO locator, IAU 2006/2000A selection | `crates/frames/src/earth_orientation.rs`; ADR-0042 |
| IAU SOFA Collection Issue 2023-10-11 | Authoritative software and validation values; SOFA terms | Standards kernel behavior and one numerical celestial-to-terrestrial result | frame reference-vector test; ADR-0042 |
| `sofars` 0.6.1 | MIT plus bundled SOFA terms | Unmodified safe pure-Rust IAU 2006/2000A computation | `crates/frames`; `Cargo.lock` |

## Design

- Affected crates/layers: `frames`, workspace dependency metadata, facade by
  existing crate re-export, project architecture/provenance/parity records.
- Public API: `EarthOrientationSample`, `EarthOrientationConvention`,
  `Iers2010EarthOrientation`, and `EarthOrientationError`.
- Rejected alternatives: public raw matrices; position-only rotation; native
  C FFI; a project-owned full IAU series; implicit data loading; operational
  Finals 2000A parsing in the frame crate.
- ADR required: ADR-0042.

## Validation

- Unit cases: sample/provider invariants, coverage, interpolation gap, and
  unsupported-frame failures.
- Invariants/properties: GCRF-to-ITRF2020-to-GCRF composition restores full
  position and velocity.
- Independent reference vectors: official IAU SOFA Issue 2023-10-11
  celestial-to-terrestrial result.
- Differential/scenario tests: transformed velocity agrees with a separate
  finite difference of transformed positions.
- Tolerances and justification: 0.2 nanometre for the one-metre SOFA direction
  vector; 3 nanometres and 3 picometres/second for inverse composition;
  2 micrometres/second for the surface-radius velocity derivative, bounding
  the differing second-order stencils and floating-point evaluation.
- Benchmarks: not required for this correctness slice.

## Completion checklist

- [x] Implementation and typed errors
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance recorded
- [x] Parity ledger updated
- [x] Relevant checks pass
- [x] Binding impact explicitly deferred until the Rust API stabilizes
