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
   locator `s` from the series of IERS Tables 5.2a, 5.2b and 5.2d (IAU 2006
   precession with IAU 2000A_R06 nutation), including the `t^j` blocks,
   polynomial parts and 14 fundamental arguments. The tables are bundled as
   plain-text data under `crates/frames-eop/data/iers-2010`.
3. Take only UT1-UTC, polar motion `xp, yp` and celestial-pole offsets
   `dX, dY` from caller-supplied, versioned samples (linearly interpolated,
   closed coverage, no extrapolation); the TIO locator `s'` uses the IERS
   secular rate. No file or network access.
4. Accept only the GCRF/ITRF2020 pair (either direction) and return the
   rotation plus the body angular velocity from the analytic derivative of
   the chain. Each transform borrows the EOP provenance records.
5. Leave solid-Earth, ocean and pole-tide corrections to the tide slice, and
   leave equinox-based and truncated variants out.

## Consequences

- `Iau2000EraProvider` remains for ERA-only use; the new provider is the
  frame-level prerequisite for #15 and #32.
- Accuracy is that of the IERS tabulated series; libration and ocean-tide
  terms in UT1 and polar motion must be present in the caller's EOP data.

## Verification

- The complete 3 x 3 matrix is checked to 1e-11 against an independent
  ERFA/SOFA (`pyerfa`) evaluation of `xys06a`, `c2ixys`, `era00`, `sp00`,
  `pom00` and `c2tcio`; CIP `X`, `Y`, `s` agree to about 3e-13 rad.
- The analytic angular velocity is compared with a central finite
  difference of the rotation; forward/reverse state round trips, coverage,
  leap-second behavior and typed failures are unit tested.
