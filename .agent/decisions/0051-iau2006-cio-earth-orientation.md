# ADR-0051: add a CIO-based IAU 2006/2000A GCRF-to-ITRF2020 provider

- Status: Accepted
- Date: 2026-10-03
- Owners: frames/Earth-orientation maintainers
- Affected parity rows: Geometry / frames, transforms, Earth orientation

## Context

[#17](https://github.com/Wouittone/orskit/issues/17) asks for a
provenance-qualified Earth-orientation transform. ADR-0046 delivered only the
IAU 2000 Earth Rotation Angle (CIRS to TIRS). Tides, high-accuracy
measurement prediction and pole-tide corrections need the full
celestial-to-terrestrial chain, including polar motion.

## Decision

1. Add `frames_eop::Iau2006CioProvider`, a caller-owned provider of the
   complete IERS Conventions (2010) Chapter 5 CIO-based chain
   `[GCRS to ITRS] = W(t) R(t) Q(t)`.
2. Evaluate the celestial intermediate pole coordinates `X`, `Y` and the CIO
   locator `s` through the unmodified, pinned `erfa` 0.2.1 dependency:
   `pn_matrix_06a`, `bpn_to_xy`, and `S06`. These use full IAU 2000A
   luni-solar (678 terms) and planetary (687 terms) nutation with IAU 2006
   adjustments, frame bias/precession, and the 66-term CIO series.
   The original bundled IERS tables and their evaluator are deliberately
   replaced, not redistributed under an assumed data license.
3. Take only UT1-UTC, polar motion `xp, yp` and celestial-pole offsets
   `dX, dY` from caller-supplied, versioned samples (linearly interpolated,
   closed coverage, no extrapolation); the TIO locator `s'` uses the IERS
   secular rate. No file or network access.
4. Accept only the GCRF/ITRF2020 pair (either direction) and return the
   rotation plus the body angular velocity from the product-rule derivative
   of the chain. EOP, ERA and TIO rates remain analytic; smooth celestial
   `X/Y/s` rates use a sixth-order centered TT stencil with 512 s spacing.
   Each transform borrows EOP, dependency and convention provenance records.
5. Leave solid-Earth, ocean and pole-tide corrections to the tide slice, and
   leave equinox-based and truncated variants out.

## Consequences

- `Iau2000EraProvider` remains for ERA-only use; the new provider is the
  frame-level prerequisite for #15 and #32.
- Accuracy is that of the full ERFA IAU 2006/2000A model; libration and ocean-tide
  terms in UT1 and polar motion must be present in the caller's EOP data.

## Verification

- The complete 3 x 3 matrix is checked at TT/UT1 MJD 53736.0 against the
  published ERFA v2.0.1 `eraC2t06a` test vector (5e-12 tolerance).
  Direct `X`, `Y`, `s` outputs are
  checked against the pinned ERFA v2.0.1 `eraXys06a` test at that MJD using
  1e-14 rad tolerance for `X/Y` and 1e-18 rad for `s`. The public test
  outputs are recorded in `crates/frames-eop/src/cio.rs`.
- An order-sensitive polar-motion cross-term test covers the active
  TIRS-to-ITRS product and its derivative. Angular velocity is
  compared with central differences at three 2025 epochs while UT1-TAI,
  `dX`, `dY`, `xp` and `yp` all have nonzero slopes. Forward/reverse state
  round trips, inclusive coverage, a UT1-TAI-continuous leap-second boundary
  and typed failures are unit tested.
- The redundant 2025 PyERFA matrix fixture with an unknown generator version
  was removed. Only pinned public reference outputs establish the numerical
  reference claim; the 2025 finite-difference and round-trip coverage remains.
- A 257-epoch TT sweep from 1972 to 2100 compares the sixth-order 512 s
  rates with both a refined 256 s stencil and a separately implemented
  fourth-order 1024 s stencil: maxima `[4.665e-20, 8.801e-19, 3.512e-21]`
  rad/s for `X/Y/s`. Regression observations of the former original
  analytic evaluator at six epochs retain values within 6e-12 rad and rates
  within 4e-17 rad/s; they are not independent ERFA reference vectors.

## Dependency and distribution review (2026-10-10)

The owner approved unmodified dependency replacement after license/API
verification. `erfa`'s actual package contains MPL-2.0 for Rust code and
the ERFA BSD-style notice for its adapted documentation. All embedded
nutation/CIO arrays reside in MPL-marked files; no separately unlicensed
IERS data file is consumed or bundled. See `crates/frames-eop/NOTICE` for
binary attribution and source-availability obligations, and the
[exact artifact audit](../PROVENANCE.md#erfa-dependency-replacement-review-2026-10-10).
The dependency contains no production unsafe Rust or FFI; its upstream
test-only C comparisons are not built into orskit.

Maintenance caveat: the registry release and upstream push are from November
2022. The repository is not archived and had zero open issues at review, but
this is **not evidence of active maintenance**. The owner's clarification
confirms that active maintenance was a preference, not a hard publication gate.
The repository requires a maintenance review, which is satisfied by accepting
the pinned fixed-standard implementation with verified licensing/API,
independent reference tests, derivative convergence and the full Rust 1.96.1
workspace gates. Inactivity is an explicitly retained future bug-fix/compiler
compatibility risk; updates require renewed review and validation.
The reviewed actively developed alternatives either have additional SOFA
license restrictions or lack a verified coefficient licensing chain; they
were not silently substituted. No IERS table redistribution clearance is
claimed for historical commits. No unresolved scientific, provenance or
safe-Rust gate remains for the current replacement.
