# ADR-0046: use an unmodified SGP4 dependency behind the strict TLE boundary

- Status: Accepted
- Date: 2026-07-24
- Owners: orskit maintainers
- Affected parity row: TLE and SGP4 propagation

## Context

The project owns a strict, fixed-column TLE parser but deliberately does not
own an SGP4 implementation. Reimplementing or translating a reference
implementation would add avoidable numerical and provenance risk. SGP4 output
also needs an explicit frame contract because TEME is neither GCRF nor a
terrestrial frame.

## Decision

1. `dynamics-sgp4` uses the MIT-licensed `sgp4` 2.4 crate unmodified and as a
   black box. Dependency implementation source, tests, and examples are out of
   bounds for project implementation work.
2. The project parser remains authoritative, but it is not a propagation
   state. Its optional adapter maps validated columns into an epoch-qualified
   `Sgp4Elements`; the dynamics crate never depends on `TwoLineElement` and
   never calls the dependency's TLE parser.
3. TLE propagation selects WGS-72 and the dependency's AFSPC-compatible epoch,
   sidereal-time, and propagation behavior to match the published verification
   convention. The adapter accepts distributed-data ephemeris type `0`.
   Explicit nonzero legacy model selectors are rejected with a typed error
   rather than silently evaluated by a different model.
4. The stateless, non-configurable `Sgp4Propagator` implements
   `Propagator<Sgp4Elements, CartesianState>`. The input `Orbit` supplies the
   element epoch and the target is a typed `Epoch`. Position and velocity cross
   the dependency boundary in documented kilometres and kilometres per second
   and are immediately converted to project unit types.
5. Every returned state is explicitly tagged `ReferenceFrame::TEME`. This
   feature performs no TEME-to-GCRF/ITRF conversion.
6. Elapsed time is the signed Hifitime duration divided by sixty SI seconds.
   A target spanning a UTC leap insertion therefore follows physical elapsed
   time, not a fixed 1,440-minute-per-UTC-civil-day convention.
7. Dependency initialization and propagation failures are exposed as typed
   sources. No additional decay classification is inferred.

## Alternatives considered

- Owning an SGP4 implementation was rejected because an established compatible
  dependency gives a smaller, auditable boundary and avoids source-derived
  provenance.
- Reusing the dependency's TLE parser was rejected because it would create two
  public parsing contracts and bypass the strict format guarantees.
- Returning raw arrays or a TLE-specific Cartesian type was rejected because
  existing typed orbit, unit, epoch, and frame contracts already represent the
  result.
- Labeling TEME inertial was rejected: its axes are time-dependent and the
  frame catalog therefore classifies it non-inertial.

## Validation

Acceptance is independent of the dependency's own tests. Project-authored
tests compare representative near-Earth and deep-space predictions against
the public numerical verification results accompanying Vallado et al.,
*Revisiting Spacetrack Report #3*, AIAA-2006-6753, Revision 3.

## Consequences

Users gain a small typed TLE-to-domain-state adapter and a format-independent
SGP4-to-TEME propagator without a second parser or raw numeric public API.
Deep-space cases supported by the dependency are available,
but only representative published cases are locked down. TLE age, atmospheric
model limits, maneuvers, reentry/decay policy, covariance, catalog operations,
and frame conversion remain caller concerns.
