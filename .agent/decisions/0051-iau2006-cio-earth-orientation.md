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

- The complete 3 x 3 matrix is checked at a 2025 epoch against the original
  ERFA/PyERFA differential fixture (1e-11 tolerance; exact PyERFA version/build
  was not retained) and at TT/UT1 MJD 53736.0 against the published ERFA v2.0.1
  `eraC2t06a` test vector (5e-12 tolerance). Direct `X`, `Y`, `s` outputs are
  checked against the pinned ERFA v2.0.1 `eraXys06a` test at that MJD using
  5e-12 rad tolerance. The public test outputs are recorded in
  `crates/frames-eop/src/cio.rs`; the latter tolerance is about 1
  microarcsecond, consistent with the precision of the coefficient tables.
- An order-sensitive polar-motion cross-term test covers the active
  TIRS-to-ITRS product and its derivative. Analytic angular velocity is
  compared with central differences while UT1-TAI, `dX`, `dY`, `xp` and `yp`
  all have nonzero slopes. Forward/reverse state round trips, inclusive
  coverage, a UT1-TAI-continuous leap-second boundary and typed failures are
  unit tested.

## Distribution limitation

The included IERS Tables 5.2a/5.2b/5.2d are essential to this implementation.
TN 36 identifies ©2010 Verlag des Bundesamts für Kartographie und Geodäsie,
Frankfurt am Main, but the official publication and site reviewed did not
provide an explicit license or permission to redistribute the tables. Their
presence in the repository is not a claim of MIT/Apache compatibility or legal
clearance. Obtain written permission or document another substantiated legal
basis before merging or distributing the bundled files.
