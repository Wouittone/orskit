# ERA-only Earth rotation

`frames-eop::Iau2000EraProvider` is a small, explicit Earth-spin provider over
caller-supplied Earth-orientation samples. It is not a full high-precision
GCRF-to-ITRF implementation. The provider omits polar motion,
precession-nutation/Celestial Intermediate Pole motion, celestial-pole offsets,
tides, and terrestrial-realization corrections. Its GCRF/ITRF2020 frame labels
identify the accepted request pair, not a claim that those omitted terms have
been applied.

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
rate and is retained as the returned body angular velocity, so the frame
provider's velocity transform includes a rate-consistent `ω × r` term.
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
