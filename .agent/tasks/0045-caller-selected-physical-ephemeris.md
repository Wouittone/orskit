# Task: evaluate caller-selected physical ephemerides

## Parity target

- Ledger row: Geometry — celestial bodies, ephemerides, ellipsoids, geodesy
- Current status: Partial
- Intended status after this task: Partial, with an evidenced physical
  ephemeris-provider and interpolation slice

## User workflow

A caller authenticates a selected local ephemeris artifact, supplies decoded
position/velocity samples for one target relative to one observer in one
complete frame, and evaluates a finite typed state at an explicitly selected
epoch within both artifact and interpolation coverage.

## Scientific contract

- Inputs and units: absolute `hifitime::Epoch` samples with typed metre
  positions and metre/second velocities.
- Outputs and units: finite geometric Cartesian target position and velocity
  relative to the selected observer, using the same typed SI quantities.
- Frames/epochs/time scales: every query carries target, observer, complete
  origin-and-axes `ReferenceFrame`, and absolute epoch. The frame origin must
  be the observer. The independent JPL vector uses ICRF axes and TDB epochs.
- Conventions and valid regimes: piecewise two-endpoint cubic Hermite
  interpolation; sample epochs increase strictly and may be unequally spaced.
- External data requirements: caller-owned `VerifiedArtifact` with immutable
  authority/product/version, SHA-256 digest, and declared coverage. No network,
  global cache, or implicit artifact selection.
- Errors and singularities: typed invalid frame origin, non-finite state,
  insufficient/unordered samples, artifact coverage, interpolation coverage,
  target/observer mismatch, frame mismatch, and non-finite interpolation
  errors.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| NASA/JPL Horizons System manual 4.98d, 2025-11-21 | US Government technical documentation/service | Geometric target-relative Cartesian vector semantics, selectable centers, ICRF axes, TDB time, and state output units | `crates/ephemeris`; independent fixture |
| NASA/JPL Horizons API 1.2 response, retrieved 2026-07-24 | Public US Government behavior sample | DE441 Moon relative Earth geometric ICRF states at three TDB epochs | `crates/ephemeris/testdata`; midpoint validation |
| NAIF, *SPK Required Reading*, toolkit N0067 documentation | US Government technical documentation | Discrete position/velocity states, target/center/frame/coverage semantics, and joint Hermite interpolation with derivative velocity | `crates/ephemeris` |

No JPL/NAIF implementation source, toolkit code, SPK reader, or third-party
test implementation was consulted or copied.

## Design

- Affected crates/layers: new `ephemeris` physical-model crate, `orskit`
  facade, workspace diagrams, docs, provenance, parity, and roadmap.
- Public API: `EphemerisProvider`, `EphemerisQuery`, `EphemerisState`,
  `EphemerisSample`, `CubicHermiteEphemeris`, and typed errors.
- Rejected alternatives: hide a global ephemeris context; couple ephemerides
  to body identity; return untyped six-element arrays; parse SPK in this slice;
  silently transform between frames; claim that provenance bytes prove a
  caller's decoding.
- ADR required: ADR-0043.

## Validation

- Unit cases: exact endpoints, target/observer and frame mismatch, non-finite
  samples, insufficient/duplicate samples, artifact coverage, interpolation
  coverage, and provenance exposure.
- Invariants/properties: returned state reproduces its complete query; cubic
  position and derivative velocity reproduce both endpoints exactly.
- Independent reference vectors: JPL Horizons DE441 Moon relative Earth
  geometric ICRF vectors at 2026-01-01 00:00, 00:01, and 00:02 TDB.
- Differential/scenario tests: interpolate the independently supplied middle
  vector from the two outer Horizons vectors.
- Tolerances and justification: 1 mm position and 1 µm/s velocity, far above
  observed rounding-scale residuals while remaining negligible relative to
  the 120-second source interval and Horizons output precision.
- Benchmarks: not required for a bounded two-sample cubic evaluation.

## Completion checklist

- [x] Implementation and typed errors
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance recorded
- [x] Parity ledger updated
- [x] Relevant checks pass
- [x] Binding impact explicitly deferred until the Rust API stabilizes
