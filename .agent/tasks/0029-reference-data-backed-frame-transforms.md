# Task: establish reference-data-backed frame-transform suppliers

## Parity target

- Ledger rows: Foundations / Explicit scientific data context and providers;
  Geometry / Frames, transforms, Earth orientation; Geometry / Celestial
  bodies, ephemerides, ellipsoids, geodesy.
- Current status: Partial; transforms have an explicit provider boundary, but
  no source identity or data-backed adapter.
- Intended status after this task: Partial; a provenance-bearing supplier can
  back frame transforms while concrete Earth-orientation equations and data
  readers remain separately selectable implementations.

## User workflow

An application loads and validates its own reference-data bundle, implements
`FrameReferenceDataSupplier`, and converts it with
`ReferenceDataKinematicFrameTransform::from`. Estimation or propagation APIs
then consume the standard `KinematicFrameTransformProvider` contract without
receiving a hidden global data context. The adapter exposes its supplier through
`AsRef`, so every borrowed reference-data artifact record remains available for
scenario provenance.

## Scientific contract

- Inputs and units: Hifitime epochs, finite typed Cartesian position and
  velocity, explicit source and target frames, and a supplier-selected data
  bundle.
- Outputs and units: finite typed kinematics in the requested target frame.
- Frames/epochs/time scales: every request names an epoch and both frames; the
  supplier owns all selected time-scale conversion, orientation, translation,
  and velocity terms.
- Conventions and valid regimes: the adapter makes no Earth-orientation or
  ephemeris approximation. A supplier must define its IAU/IERS convention,
  interpolation, coverage, and extrapolation policy; its declared artifact set
  remains stable for the supplier lifetime.
- External data requirements: applications own data loading and versions. The
  recommended production stack combines a pinned JPL/NAIF SPK planetary
  ephemeris (such as DE440), a pinned IERS EOP product, and one declared
  IAU/IERS convention set. JPL ephemerides alone do not realize the ITRF/ICRF
  orientation.
- Errors and singularities: supplier coverage, format, and evaluation errors
  are retained as sources; a supplier result in any frame other than the
  requested target is rejected.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| [JPL DE440/DE441](https://ssd.jpl.nasa.gov/doc/de440_de441.html) and [NAIF SPK](https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/C/req/spk.html) | US Government data-product and format documentation | DE440/DE441 are planetary/lunar ephemerides; SPK segments provide position/velocity data over finite coverage | supplier recommendation and provenance policy |
| [IERS data products](https://www.iers.org/IERS/EN/DataProducts/data.html) | International scientific-service data documentation | IERS supplies Earth orientation and ICRF/ITRF products | EOP requirement for terrestrial/celestial supplier |

No external implementation, source code, test, or data artifact was copied.

## Design

- Affected crates/layers: `frames`, public facade re-export, architecture,
  parity, and provenance records.
- Public API: `ReferenceDataDescriptor`, `FrameReferenceDataSupplier`, and
  `ReferenceDataKinematicFrameTransform`.
- Rejected alternatives: global data context; a JPL-only transform claim;
  exposing rotations/matrices as a second public numerical API; or treating an
  ephemeris as sufficient Earth-orientation data.
- ADR required: yes; ADR-0035 records the generic data-boundary decision before adding a
  concrete reader or numerical convention implementation.

## Validation

- Unit cases: identity requests do not load data; distinct-frame requests
  delegate; absent or incomplete provenance fails before supplier evaluation;
  source errors survive; wrong-frame supplier output fails.
- Invariants/properties: a distinct-frame request requires at least one
  borrowed immutable provenance record with non-blank identity fields;
  successful adapter output always carries the requested frame.
- Independent reference vectors: deferred until a concrete JPL/IERS supplier
  and convention set are selected.
- Differential/scenario tests: deferred until the selected supplier can be
  compared with independently generated frame vectors.
- Tolerances and justification: none at this abstraction boundary.
- Benchmarks: deferred; no performance claim is made.

## Completion checklist

- [x] Implementation and typed errors
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance recorded
- [x] Parity ledger updated
- [x] Relevant checks pass
- [x] Binding impact handled or explicitly deferred: bindings remain deferred
