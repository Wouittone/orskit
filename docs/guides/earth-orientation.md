# ERA-only Earth rotation

`frames-eop::Iau2000EraProvider` is a small, explicit Earth-spin provider over
caller-supplied Earth-orientation samples. It is not a full high-precision
GCRF-to-ITRF implementation. ERA relates the Celestial Intermediate Reference
System (CIRS) to the Terrestrial Intermediate Reference System (TIRS), so the
provider's typed results are CIRS↔TIRS only. It does not label an ERA-only
rotation as GCRF↔ITRF2020. The full transformation additionally requires
celestial-intermediate and polar-motion rotations. This provider omits polar
motion, precession-nutation/Celestial Intermediate Pole motion, celestial-pole
offsets, tides, and terrestrial-realization corrections.

The provider implements the IAU 2000 Earth Rotation Angle from IERS
Conventions 2010 Chapter 5 §5.5.3 Eq. 5.15:

```text
ERA = 2π (0.7790572732640 + 1.00273781191135448 × (JD_UT1 - 2451545.0))
```

It forms `UT1−TAI` from each caller-supplied `UT1−UTC` sample and Hifitime's
UTC/TAI leap-second table, then linearly interpolates that continuous offset
in elapsed TAI seconds. Requests are converted to TAI for coverage lookup, so
instants remain distinct across leap seconds. ERA is evaluated using elapsed
TAI seconds relative to J2000 noon plus the interpolated `UT1−TAI` offset.
The segment derivative scales the nominal ERA
rate and is retained as the returned TIRS-relative-to-CIRS angular velocity,
so the frame provider's velocity transform includes a rate-consistent
`ω × r` term.
Samples are accepted only from 1972-01-01 UTC onward, must be finite,
strictly ordered, and must include at least two records. Coverage includes both
endpoints; extrapolation is an error.

Construct the provider with the selected EOP authority, product, immutable
revision, and optional checksum. Each transform result borrows the EOP,
convention, and Hifitime time-scale provenance records from that provider.
The provider does no file or network I/O and retains its selected records for
its lifetime. The crate-level API example shows construction and a request.

The independent J2000 check uses JD(UT1) = 2451545.0 and the published IERS
equation, yielding ERA = 280.46061837504 degrees. Unit tests also exercise
forward/reverse state transforms, the angular-rate term, time-scale
conversion, closed coverage, provenance/frame errors, and the 2016 leap
second. These tests validate this ERA-only slice; they do not validate a full
terrestrial/celestial orientation solution.

References and scope are recorded in [the provenance ledger](../../.agent/PROVENANCE.md),
[ADR-0046](../../.agent/decisions/0046-implement-era-only-eop-provider.md),
and the [capability parity ledger](../../.agent/PARITY.md).

## Full CIO-based GCRF to ITRF2020 transform

`frames_eop::Iau2006CioProvider` implements the complete IERS Conventions (2010)
Chapter 5 chain `W(t) R(t) Q(t)` for the `GCRF` and `ITRF2020` pair, in either
direction. The celestial intermediate pole `X`, `Y` and the CIO locator `s`
come from Tables 5.2a, 5.2b and 5.2d (bundled in
`crates/frames-eop/data/iers-2010`), `R` is the Earth Rotation Angle above and
`W` uses the TIO locator `s'` with caller-supplied polar motion. Callers
provide versioned UT1-UTC, `xp`, `yp`, `dX` and `dY` samples; values are
linearly interpolated with closed coverage and no extrapolation. The returned
transform carries the EOP provenance and the analytic angular velocity.

For active column-vector rotations, the TIRS-to-ITRS polar-motion product is
`Rx(yp) Ry(xp) Rz(-s')`; its derivative uses the same ordered product rule.
This is the reverse product order from applying the individual active
rotations as `Rz(-s') Ry(xp) Rx(yp)`. The convention is grounded in the
IERS Chapter 5 `W R Q` chain and is checked by an order-sensitive cross-term
test and the independent full-matrix vector below.

Validation uses the pinned ERFA v2.0.1 validation cases in `src/t_erfa_c.c` at
TT/UT1 MJD 53736.0: direct `X`, `Y`, `s` comparisons and a full `eraC2t06a`
matrix check. The ERFA source revision, inputs and expected values are recorded
in the tests and [provenance ledger](../../.agent/PROVENANCE.md). The current
tabulated-series comparison uses a 5e-12 rad tolerance (about 1 microarcsecond)
to account for the precision represented by the bundled IERS coefficient
tables. The analytic angular velocity is checked against central differences
at three 2025 epochs with nonzero slopes in UT1-TAI, `dX`, `dY`, `xp` and `yp`;
a 2016 leap-boundary case verifies UT1-TAI continuity and inclusive sample
endpoints. The redundant unknown-version 2025 PyERFA matrix fixture was removed.

**Redistribution blocker:** IERS Technical Note 36 identifies ©2010 Verlag des
Bundesamts für Kartographie und Geodäsie, Frankfurt am Main, but the official
publication and site reviewed do not state an applicable open-data license or
permission to redistribute Tables 5.2a/5.2b/5.2d. The files are retained in
this PR rather than silently removed, but their inclusion is not cleared for
release. This is not a finding that individual numerical facts require a
license: the review must distinguish facts from the copied table titles,
column headers and collection as distributed. The
[provenance research record](../../.agent/PROVENANCE.md#iers-table-distribution-review-2026-10-10)
lists exact sources and the narrow owner decision required before publication.

Tides and equinox-based or truncated variants are out of scope. See
[ADR-0051](../../.agent/decisions/0051-iau2006-cio-earth-orientation.md).
