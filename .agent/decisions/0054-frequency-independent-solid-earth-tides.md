# ADR-0054: compose IERS frequency-independent solid-Earth tide coefficients

- Status: Accepted
- Date: 2026-10-11
- Owners: propagation/dynamics maintainers
- Affected parity row: Propagation / gravity fields and solid/ocean tides

## Context

Issue #15 requests relativity and gravity-tide corrections. The existing
`TimeVaryingSphericalHarmonicGravityModel` composes normalized coefficient
deltas inside the selected static field, and the Earth-orientation layer now
provides the full IERS 2010 CIO-based GCRF/ITRF2020 transform. The remaining
Earth inputs must still be supplied explicitly: Earth-centered Moon/Sun
ephemerides in coefficient axes and sourced gravitational parameters.

The static `TideSystem` is convention metadata, not a conversion operation.
Applying a second permanent-tide term to a zero-tide field would double count
the static contribution. The frequency-dependent, ocean, and pole-tide
models have separate formula/data requirements and are not part of this slice.

## Decision

1. Implement only the IERS Conventions (2010), Chapter 6, §6.2 frequency-
   independent Step 1 coefficient response. Equation 6.6 uses nominal
   degree-2 and degree-3 external-potential Love numbers. Equation 6.7 also
   includes the induced degree-4 terms from the degree-2 tide. All deltas use
   the existing fully normalized 4π convention and are composed before the
   unchanged Cartesian potential-gradient evaluator.
2. Use the selected gravity field's typed `GMₑ`, reference radius, coefficient
   frame, and tide-system metadata. Restrict this Earth-specific provider to
   Earth-centered fields with ITRF2020 coefficient axes and degree/order
   coverage no greater than 4.
3. Require caller-supplied, typed Moon and Sun gravitational parameters with
   separate source records, plus a caller-owned ephemeris and explicit
   ephemeris provenance. For every evaluation epoch, resolve exactly one state
   per body in ITRF2020. Validate body identity, epoch, complete frame,
   finite exterior distance, and the closed Hifitime coverage interval.
   Perform no network access or implicit data selection.
4. For `TideFree` static coefficients, apply the complete Step 1 response,
   including the permanent degree-2 zonal term. For `ZeroTide` coefficients,
   remove that time-independent `C̄₂₀` part from the correction using IERS
   Eqs. 6.13–6.14. Reject `MeanTide` rather than infer its external-potential
   convention. Never relabel or convert the static field's tide system.
5. Add an epoch-scoped evaluator to the existing delta-provider contract so
   the expensive ephemeris states are resolved once per gravity evaluation.
   Preserve provider failures as typed error sources instead of mapping them
   to zero deltas or missing-coefficient errors. Existing providers retain a
   default adapter and their `Option` callback behavior.
6. Do not implement Step 2 frequency-dependent corrections, ocean tides,
   pole tides, a tide force separate from the coefficient potential, data
   parsers, datasets, or language bindings.

## Alternatives considered

- **Apply an independent acceleration:** rejected because the response is a
  change to the gravity potential coefficients and must use the same
  recurrence/gradient and tide-system convention.
- **Use tide-free corrections unchanged on zero-tide coefficients:** rejected
  because it reapplies the permanent `C̄₂₀` component already represented in
  the static field.
- **Treat `MeanTide` as `ZeroTide`:** rejected because its permanent external-
  potential convention is not implemented by this coefficient-only slice.
- **Bundle Sun/Moon constants or ephemerides:** rejected because identity does
  not imply a physical parameter/data product and all scientific inputs must
  remain caller-selected and traceable.
- **Fetch data or silently extrapolate:** rejected to preserve deterministic,
  offline model evaluation and closed coverage.
- **Implement all of issue #15:** rejected because frequency-dependent,
  ocean, and pole-tide model/input requirements are distinct follow-ups.

## Consequences

- Callers can add the IERS frequency-independent solid-Earth response to
  supported tide-free or zero-tide Earth fields without replacing or mutating
  their static coefficients.
- The provider supports fully normalized coefficients through degree/order 4,
  including the small induced degree-4 response. It does not claim full
  frequency-dependent solid-Earth tide accuracy or complete issue #15 parity.
- `MeanTide`, other coefficient frames, non-Earth origins, and higher
  truncations are rejected by this model entry point.
- Caller-owned ephemeris, tide-raiser GM, frame/EOP, and coefficient data
  versions remain visible in provider provenance. No data is included.
- The existing `spherical-harmonics` opt-in facade feature remains the public
  integration boundary; no new crate or external dependency is introduced.

## Validation

- Independently evaluate the published closed-form degree-2 zonal, degree-2
  tesseral, degree-3 zonal, and degree-4 response equations.
- Verify the zero-tide correction differs only by the permanent `C̄₂₀` term;
  verify one epoch resolves one Moon and one Sun state, closed coverage and
  unsupported inputs fail explicitly, and ephemeris failures remain typed.
- Compare a composed gravity evaluation through the existing potential-
  gradient path. Label these results as analytic/invariant evidence, not as an
  external ephemeris vector or independent product validation.
- Run the Rust 1.96.1 workspace and feature checks recorded in task 0054.

## Provenance

The equations, permanent-tide handling, and tabulated nominal/induced Love
numbers follow the public [IERS Conventions (2010), Chapter 6, §§6.2.1–6.2.2,
Eqs. 6.6–6.7 and 6.13–6.14](https://iers-conventions.obspm.fr/content/chapter6/icc6.pdf).
The implementation is independently written from those public equations and
uses no source code, tests, datasets, or numerical output from another
astrodynamics library. `BodyEphemerisProvider`, Hifitime epochs, typed SI
parameters, and the existing normalized coefficient evaluator are unchanged
project contracts.
