# ADR-0043: require caller-selected physical ephemeris providers

- Status: Accepted
- Date: 2026-07-24
- Owners: orskit maintainers
- Affected parity rows: scientific data contexts; celestial bodies and
  ephemerides; dynamics and third-body force models

## Context

Body identities intentionally carry no physical trajectory. Frame identities
name origins and axes but do not evaluate their relative motion. Future
third-body forces, observations, and mission geometry need position and
velocity for one target relative to one observer at an exact epoch, with
inspectable data provenance and no implicit network or process-global data.

Format parsing is a separate concern. Pulling SPK decoding into the first
provider contract would combine data authentication, operational file
semantics, interpolation, and the high-level query API before each boundary
has independent evidence.

## Decision

1. A dedicated `ephemeris` crate owns the open `EphemerisProvider` contract.
   Each query explicitly identifies target body, observer body, complete
   reference frame, and absolute `hifitime::Epoch`.
2. The query frame's origin must be the observer body. Providers never infer
   an origin or silently transform axes. Results repeat the complete query and
   contain finite typed `Position` and `VelocityVector` values.
3. Providers expose every `orskit-data::ArtifactDescriptor` used for
   evaluation. Concrete providers own caller-selected `VerifiedArtifact`
   values; no algorithm downloads, replaces, or finds data through a mutable
   global cache.
4. The first concrete provider accepts already-decoded, strictly ordered
   position/velocity samples and performs piecewise cubic Hermite
   interpolation. Position and its velocity derivative are interpolated
   together. Both declared artifact coverage and actual sample bracketing are
   checked and reported as distinct typed errors.
5. Supplying decoded samples is an explicit trust boundary: SHA-256 proves the
   retained source bytes, while the caller remains responsible for decoding
   those bytes correctly. The provider does not falsely claim to validate a
   format it does not parse.
6. SPK and other ephemeris readers, aberration corrections, body-system
   barycenters, frame transformations, uncertainty, acceleration, higher-order
   interpolation windows, and discontinuous coverage remain separate slices.

## Alternatives considered

- Attach ephemerides to `Body`: rejected because identity must not select a
  physical model or dataset.
- Return a raw six-element SI array: rejected because position and velocity
  dimensions would be interchangeable and frame/query context could be lost.
- Use only `ReferenceFrame` origin instead of an observer field: rejected
  because target-relative-to-observer is a first-class ephemeris path and the
  duplicated declarations can be checked for consistency.
- Add an SPK reader now: rejected because the provider and interpolation slice
  needs no file reader, and unevidenced partial SPK support would broaden the
  operational-format attack surface.
- Recompute or fetch missing samples automatically: rejected because it would
  violate deterministic caller-owned data selection.

## Consequences

Applications can inject a concrete provider or implement the open trait while
retaining provider-specific errors. A sampled provider is small and useful for
validated scenarios, but callers must explicitly decode authenticated source
bytes. One provider instance covers one target/observer/frame path and one
continuous sample interval.

Future third-body and measurement code can depend on this contract without
depending on SPK, JPL products, or a global almanac. A future multi-segment or
format-backed provider can retain the same query/result boundary and add its
own typed format, coverage, and interpolation errors.

## Validation

The checked-in NASA/JPL Horizons API response is authenticated by a fixed
SHA-256 digest. Two outer DE441 Moon-relative-Earth geometric ICRF vectors at
2026-01-01 00:00 and 00:02 TDB drive the provider; the independent 00:01
Horizons vector validates interpolated position within 1 mm and velocity
within 1 µm/s. Unit tests also cover finite-state, frame-origin, path, frame,
ordering, artifact-coverage, interpolation-coverage, and exact-endpoint
behavior.

## Provenance

NASA/JPL Horizons manual 4.98d and an API 1.2 behavior sample establish the
target/observer/frame/time/unit semantics and validation vectors. NAIF *SPK
Required Reading* establishes the documented use of joint Hermite
position/velocity interpolation for unequally spaced states. No toolkit source,
SPK reader, third-party implementation, or third-party test was copied.
