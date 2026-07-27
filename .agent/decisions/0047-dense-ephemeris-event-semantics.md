# ADR-0047: use accepted-step Hermite ephemerides for immutable event search

- Status: Accepted
- Date: 2026-07-28
- Affected parity rows: numerical integration and dense ephemerides; events and
  root localization
- Related tasks: 0033, 0047, 0051

## Context

ADR-0044 deliberately delivered endpoint-only Fehlberg RK4(5). P12 must add a
continuous extension before event detectors can inspect intermediate states.
That extension must preserve exact typed endpoints and must not imply that
local endpoint error control bounds accumulated trajectory error. Event
contracts additionally need bounded failures and deterministic behavior in
both propagation directions.

## Decision

1. `generate_ephemeris` retains every accepted fifth-order endpoint and
   evaluates the dynamics derivative at each accepted step end. A cubic Hermite
   polynomial joins the two state/derivative pairs. Exact endpoint queries
   return the stored vectors directly.
2. For sufficiently smooth dynamics this continuous extension has
   `O(h^4)` interpolation error. It is a lower-order reporting extension over
   fifth-order accepted endpoints and makes no global-error guarantee.
3. Dense ephemerides retain one frame and a directional closed epoch interval.
   Out-of-range queries fail; extrapolation is never implicit.
4. Event detectors are caller-owned scalar functions over typed dense states.
   Values must be finite. Detector evaluation and handler source errors are
   preserved.
5. The first root solver is bracketed bisection. Its positive typed epoch
   tolerance, non-zero iteration limit, and non-zero event limit are required
   inputs. A returned root lies within the final bracket, whose width is no
   greater than the configured tolerance.
6. Direction means increasing or decreasing detector value with increasing
   physical epoch, not integration direction.
7. Roots within the configured epoch tolerance are simultaneous. Their
   handlers run in detector-slice order. All handlers in the group run before
   any `Stop` action terminates the search. The immutable ephemeris API supports
   only `Continue` and `Stop`; state reset belongs to a future online
   integration contract.
8. Only endpoint zeros and sign-changing roots are claimed. Grazing roots and
   multiple hidden crossings inside one accepted step remain pending.
9. Integer stage-epoch fractions use quotient/remainder arithmetic so valid
   large Hifitime durations cannot overflow. Step scaling clamps in floating
   seconds before reconstructing `Duration`. Non-finite component error scales
   are typed failures rather than false zero-error acceptance.

## Alternatives considered

- Reuse the RKF45 stage values as an undocumented interpolant: rejected because
  no selected continuous-extension coefficients or accuracy contract supports
  it.
- Store only endpoint positions and linearly interpolate: rejected because
  velocity/state continuity and useful event timing would be unnecessarily
  poor.
- Use unconstrained high-degree fitting: rejected because endpoint derivatives
  and error order would be unclear.
- Implement a Brent hybrid immediately: rejected because deterministic bounded
  bisection is smaller and sufficient for the first analytic event slice.
- Let handlers mutate/reset the trajectory: rejected because a completed
  immutable ephemeris cannot consistently replay changed dynamics.

## Consequences

Dense generation costs one additional dynamics evaluation per accepted step
and retains one compact segment per accepted step. Ordinary endpoint
propagation remains on its prior path and incurs neither cost. Event searches
are deterministic but can only find roots bracketed by accepted-step
endpoints. Higher-order method-specific extensions, online reset/reintegration,
grazing detectors, and adaptive event checking remain later work.

## Evidence

Tests cover exact dense endpoints; analytic forward/backward quadratic
trajectories; fourth-order harmonic interpolation refinement; analytic linear
event epochs in both directions; direction filtering; simultaneous ordering
and stop semantics; source preservation; bounded failures; extreme duration
fraction/scaling arithmetic; and non-finite error-scale rejection.

## Provenance

Shampine's 1986 primary paper supplies the continuous-extension purpose and
order discipline. Brent's 1971 primary paper supplies the guaranteed
sign-changing-bracket root-localization context; only project-authored
bisection is implemented. No third-party source code or astrodynamics-library
tests were consulted or copied.
