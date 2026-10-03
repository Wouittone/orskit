# ADR-0046: implement a provenance-qualified ERA-only EOP provider

- Status: Accepted
- Date: 2026-10-03
- Owners: frames maintainers
- Affected parity rows: Geometry / frames, transforms, Earth orientation

## Context

The frames crate already defines an object-safe body-fixed transform request
and provider boundary, but applications still have to implement the Earth
rotation prerequisite themselves. A complete terrestrial/celestial
realization would also require precession-nutation/CIP motion, polar motion,
and selected conventions and corrections. Shipping those together would
expand this issue into dependent frame and force-model work (#11/#15) and
would require additional data formats and reference vectors.

## Decision

1. Add a standalone `frames-eop` crate, exposed from the `orskit` facade behind
   the opt-in `earth-rotation` feature.
2. Implement an IAU 2000 Earth Rotation Angle provider over immutable,
   caller-supplied UT1−UTC samples and mandatory EOP source/product/revision
   provenance. An optional caller-provided checksum identifies exact content.
3. Require at least two finite, strictly chronological samples at or after
   1972-01-01 UTC. Normalize epochs to UTC, use Hifitime's pinned leap-second
   table to form UT1−TAI, and linearly interpolate that continuous offset in
   TAI seconds. Coverage is inclusive at both sample endpoints and no
   extrapolation is performed.
4. Accept only the explicit GCRF/ITRF2020 frame identities at Earth's center.
   Retain epoch, time scale, direction, frames/origin, angular velocity, EOP
   provenance, convention revision, and time-scale provenance with each
   resolved transform. Include the interpolated UT1−TAI slope in angular
   velocity.
5. Label the model ERA-only. It does not implement polar motion,
   precession-nutation/CIP motion, celestial-pole offsets, tides, or
   terrestrial-realization corrections and therefore is not a complete
   precision GCRF-to-ITRF transform.
6. Do not bundle or fetch EOP data and do not add parsers, general transform
   infrastructure, harmonics, tides, or dependent issue #11/#15 work.

## Alternatives considered

- Put concrete EOP calculations in `frames`: rejected because the frames
  contract remains implementation-neutral and the provider/data implementation
  is a separately selectable crate.
- Use an unqualified UT1−UTC `f64`: rejected because public physical inputs
  must carry units.
- Interpolate UT1−UTC directly across leap seconds: rejected because the
  discontinuity would create a false angular-rate spike. Interpolating the
  continuous UT1−TAI offset is equivalent away from leap boundaries.
- Accept any inertial/terrestrial frame pair: rejected because ERA alone
  cannot silently claim other frame conventions or ITRF realizations.
- Add an EOP file reader or bundled series: deferred so applications retain
  data-loading, revision, checksum, and offline control.

## Validation

- IERS Conventions 2010 Chapter 5 §5.5.3 Eq. 5.15 evaluated at JD(UT1) =
  2451545.0 gives ERA = 280.46061837504 degrees; the independent matrix-vector
  tolerance is `2e-12` radians.
- Unit tests cover direct and inverse position/velocity transforms, the
  angular-rate velocity term, UT1−UTC interpolation slope, inclusive coverage,
  outside-coverage and frame/provenance failures, request time scale retention,
  and the 2016 leap-second boundary.
- No independent EOP series or full terrestrial/celestial reference vector is
  claimed; those remain required for broader parity.
