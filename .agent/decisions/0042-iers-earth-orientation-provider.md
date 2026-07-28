# ADR-0042: keep Earth orientation behind a verified high-level provider

- Status: Accepted
- Date: 2026-07-24
- Owners: orskit maintainers
- Affected parity rows: frames, transforms, and Earth orientation; explicit
  scientific data context and providers

## Context

GCRF/ITRF transformations depend on TT for the celestial model, UT1 for Earth
rotation, polar motion, and an explicit celestial convention. The existing
`FrameReferenceDataSupplier` deliberately prevents an application-facing raw
matrix API, while ADR-0039 requires exact caller-selected artifacts. A complete
IAU 2006/2000A series is large and has an authoritative maintained
implementation in the IAU SOFA ecosystem. Operational IERS file parsing is a
separate untrusted-input capability scheduled under I15.

## Decision

1. `Iers2010EarthOrientation` implements the existing supplier boundary only
   for GCRF and ITRF2020 with the CIO-based IERS Conventions (2010) procedure:
   IAU 2006 precession, IAU 2000A nutation, IAU Earth Rotation Angle, IERS TIO
   locator, and caller-supplied polar motion.
2. Construction consumes a `VerifiedArtifact` and typed samples decoded by the
   caller from those exact bytes. The artifact interval must equal the sample
   interval. There is no file discovery, network access, implicit cache, or
   operational EOP parser.
3. Samples carry an absolute Hifitime epoch, continuous UT1-TAI, and typed
   polar angles. Linear interpolation occurs in uniform TAI and rejects gaps
   larger than the caller-selected maximum.
4. The complete rotating-frame velocity term is retained. A private
   second-order 0.5-second finite-difference stencil differentiates the whole
   orientation, using one-sided stencils at coverage endpoints.
5. The celestial kernel is the unmodified pure-Rust `sofars` 0.6.1 dependency.
   Its MIT notice and bundled SOFA license/marking conditions remain in the
   dependency distribution. Project documentation identifies the derivation
   and does not imply IAU SOFA endorsement.
6. No raw transformation matrix is public.

## Alternatives considered

- Reimplementing the full IAU 2006/2000A series was rejected because it would
  duplicate a large standards kernel and create avoidable clean-room and
  maintenance risk.
- A native ERFA/SOFA binding was rejected because the scientific workspace
  forbids unsafe Rust and should not require a C toolchain for this capability.
- Parsing Finals 2000A directly in `frames` was rejected because bounded
  operational EOP ingestion belongs to I15.
- Rotating position while copying velocity was rejected as physically
  incorrect. Exposing a matrix and leaving derivative handling to callers was
  rejected by the high-level frame boundary.

## Consequences

Callers retain exact artifact provenance and explicit interpolation policy.
The provider is deterministic and offline, and state transforms include
velocity coupling. The first slice does not apply observed celestial-pole
offsets (`dX`, `dY`) or synthesize subdaily tidal/libration EOP corrections,
does not model an ITRF realization change, and relies on callers or future I15
parsers to decode and correct source products faithfully. The finite-difference
derivative has a documented numerical error budget and requires at least one
second of usable coverage around an endpoint stencil.

## Validation

The official IAU SOFA 2023-10-11 celestial-to-terrestrial numerical result is
checked as an authoritative vector. Project-authored tests check inverse
composition for position and velocity, compare transformed velocity with a
separate position finite difference, distinguish coverage and interpolation
failures, and reject unrelated frames.

## Provenance

- IERS Technical Note 36, IERS Conventions (2010), Chapter 5, updated
  10 August 2012: transformation order, ERA, polar-motion, CIO, TT, UT1, and
  IAU 2006/2000A conventions.
- IAU SOFA Collection, Issue 2023-10-11 and its validation result: maintained
  standards kernel and authoritative numerical vector.
- `sofars` 0.6.1: unmodified pure-Rust dependency, MIT plus bundled SOFA terms.
