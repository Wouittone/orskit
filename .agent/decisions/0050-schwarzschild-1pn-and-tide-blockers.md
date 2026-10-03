# ADR-0050: add correction-only Schwarzschild 1PN gravity and defer tides

- Status: Accepted
- Date: 2026-10-03
- Owners: propagation/dynamics maintainers
- Affected parity rows: Propagation / gravity fields and solid/ocean tides;
  Propagation / third-body, drag, atmosphere, radiation, relativity

## Context

Issue #15 requests first-post-Newtonian relativistic acceleration and
solid-Earth, ocean, and pole-tide corrections. The current main harmonic model
accepts caller-selected general spherical-harmonic coefficients with
source/revision, normalization, tide-system, frame, and epoch metadata plus
provenance-qualified epoch transforms; it does not
accept epoch-varying coefficient deltas. Its acceleration is the gradient of that
selected potential, so a tide modification belongs inside this model boundary
or an equivalent coefficient-potential composition, not as an unrelated
additive force.

The current Earth-orientation implementation is explicitly ERA-only and
accepts UT1 data. It does not provide polar motion, precession/nutation, or a
complete terrestrial realization. No solid/ocean/pole tide data provider,
convention-specific formula contract, or independent tide reference vectors
are currently available.

## Decision

1. Add an opt-in `dynamics-relativity` model for the correction-only
   Schwarzschild test-particle 1PN monopole term from IERS Conventions (2010),
   Chapter 10, §10.2, Eq. 10.8.
2. Require an explicit gravity origin, matching inertial frame, typed
   gravitational parameter, and caller-selected maximum weak-field/
   slow-motion expansion parameter. Use the exact SI speed of light.
3. Return only framed SI correction acceleration. Never include the
   Newtonian monopole term; callers must explicitly compose this with a
   Newtonian model using the same `μ`, origin, and harmonic-coordinate frame.
4. State the test-particle, harmonic-coordinate, isolated spherical
   non-rotating monopole assumptions and reject zero radius, frame/origin
   mismatch, non-finite geometry, and states outside the configured regime.
5. Do not implement solid-Earth, ocean, or pole-tide corrections in this
   slice. They require distinct caller-owned, versioned inputs and
   convention-specific evaluation of time-varying potential coefficients.
   Pole tide additionally requires polar-motion inputs that #17 does not
   provide. No tide term may silently alter the harmonic model's tide-system
   metadata or be layered on as a separate acceleration.

## Alternatives considered

- **Add a generic tide-delta callback without physical providers:** rejected;
  it would create an unvalidated placeholder and would not compute a tide.
- **Add tide acceleration on top of static harmonics:** rejected because tide
  corrections modify the gravity potential and could double count terms
  already embedded in the selected coefficient set.
- **Treat ERA as complete Earth orientation:** rejected; the #17 provider
  omits polar motion and other required orientation terms.
- **Combine the 1PN term with Newtonian acceleration in one model:** rejected
  because that hides composition and can double count central gravity.

## Consequences

- The 1PN correction can be independently selected and composed through the
  existing evaluable Cartesian-force contract.
- It is not a complete relativistic model: spin, external-body terms, and
  relativistic coupling to nonspherical gravity are deferred.
- Solid, ocean, and pole-tide work remains blocked on suitable in-model
  coefficient-delta composition, explicit source/version/convention and
  frame/time semantics, the missing polar-motion input for pole tide, and
  independent reference vectors for each implemented slice.

## Validation

- Compare a non-radial SI acceleration vector against an independent
  evaluation of the published IERS equation.
- Cover correction-only output, frame/origin mismatch, zero-radius
  singularity, state requirements, and caller-selected regime rejection.
- Run the Rust 1.96.1 workspace format, all-feature check/lint/tests/docs,
  MSRV, and explicit public-facade feature checks.

## Provenance

IERS Conventions (2010), Chapter 10, §10.2, Eq. 10.8 supplies the
Schwarzschild correction equation and its harmonic-coordinate assumptions.
The exact speed of light comes from the BIPM SI definition of the metre. No
code, tests, data, or distinctive prose from another astrodynamics library is
used.
