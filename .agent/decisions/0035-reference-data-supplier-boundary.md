# ADR-0035: require explicit provenance-bearing frame reference-data suppliers

- Status: Accepted
- Date: 2026-07-16
- Affected parity rows: foundations/data context; geometry/frames, transforms,
  Earth orientation; geometry/ephemerides

## Context

Frame identities and a generic transform-provider contract made time and frame
requests explicit, but a concrete provider could still hide which Earth
orientation, ephemeris, convention, coverage, or cache policy it selected.
That conflicts with deterministic data use and prevents a scenario from
recording the exact inputs behind a transform. Treating a JPL planetary
ephemeris as the sole transform input would be scientifically incomplete:
terrestrial/celestial orientation also needs Earth-orientation data and a
declared convention set.

## Decision

1. `frames::FrameReferenceDataSupplier` is the open contract for an
   application-owned, reference-data-backed kinematic solution. It exposes an
   immutable descriptor containing authority, product, revision, and optional
   content checksum.
2. A supplier resolves one epoch-qualified `FrameKinematics` request directly
   into the requested frame. It owns all source-specific parsing, coverage,
   interpolation, cache policy, time-scale handling, rotational and
   translational terms, and numerical convention selection.
3. `ReferenceDataKinematicFrameTransform` adapts a supplier to the existing
   `KinematicFrameTransformProvider` contract. It bypasses the supplier for an
   identity request, forwards every distinct-frame request, and rejects any
   result whose frame does not equal the requested target.
4. The recommended future production implementation is a separately
   feature-gated JPL/NAIF SPK adapter using a pinned DE440-class ephemeris
   together with a pinned IERS EOP product and an explicitly named IAU/IERS
   convention. Mission-specific validated bundles and independently managed
   almanac adapters remain equally valid supplier implementations.

## Alternatives considered

- Process-global data context: rejected because loading, versioning, cache
  lifetime, and offline behavior would become ambient and irreproducible.
- A JPL-only API or a bundled DE file reader: rejected because planetary
  ephemerides do not by themselves realize terrestrial/celestial orientation,
  and because this foundational crate must not impose a data format or download
  policy.
- Public rotation matrices and vector kernels: rejected because they create a
  second low-level public API and leave velocity/origin semantics easy to omit.
- Extending only `KinematicFrameTransformProvider`: rejected because it cannot
  require data provenance from implementations that happen to use reference
  data.

## Consequences

- Algorithms keep their existing transform-provider dependency and do not need
  a source-specific data dependency.
- Applications can select a JPL/IERS, mission, or almanac implementation while
  making selected input artifacts inspectable in scenario records.
- The adapter makes no numerical frame-transform accuracy claim. A concrete
  supplier must separately document equations, data coverage, interpolation,
  accuracy evidence, and external reference vectors.

## Validation

`frames` tests prove that data-backed transforms delegate distinct-frame
requests, bypass identity requests, preserve supplier errors, and reject
mislabelled output. Full-workspace checks cover the public contract. No
external transform vector is claimed until a concrete supplier and convention
set are implemented.

## Provenance

JPL's DE440/DE441 data documentation establishes that these are planetary and
lunar ephemerides. IERS documents its Earth-orientation and ICRF/ITRF data
products. Those public facts motivate the separation of ephemeris and EOP
inputs only. No implementation source, test, data file, or transform equation
was copied.
