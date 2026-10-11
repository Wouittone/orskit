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
come from the unmodified, pinned `erfa` 0.2.1 dependency (`pn_matrix_06a`,
`bpn_to_xy` and `S06`, full IAU 2000A nutation with IAU 2006 adjustments
and frame bias), `R` is the Earth Rotation Angle above and
`W` uses the TIO locator `s'` with caller-supplied polar motion. Callers
provide versioned UT1-UTC, `xp`, `yp`, `dX` and `dY` samples; values are
linearly interpolated with closed coverage and no extrapolation. The returned
transform carries the EOP/dependency provenance and rate-consistent angular
velocity. `s` is evaluated with the model's uncorrected `X/Y`; caller `dX/dY`
corrections are applied to the pole, preserving the existing convention.

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
full-model comparison uses 1e-14 rad tolerances for `X/Y`, 1e-18 rad for `s`,
and the existing 5e-12 matrix tolerance. The angular velocity is checked against central differences
at three 2025 epochs with nonzero slopes in UT1-TAI, `dX`, `dY`, `xp` and `yp`;
a 2016 leap-boundary case verifies UT1-TAI continuity and inclusive sample
endpoints. The redundant unknown-version 2025 PyERFA matrix fixture was removed.

### Celestial derivative strategy

The dependency exposes values but no analytical derivatives. The adapter uses
an original sixth-order centered TT derivative:

```text
df/dt = [45(f(t+h)-f(t-h)) - 9(f(t+2h)-f(t-2h))
         + (f(t+3h)-f(t-3h))] / (60h), h = 512 SI seconds
```

Pair differences avoid cancellation of the secular offset. Only the smooth
celestial model is sampled; EOP slopes, ERA, TIO locator and the matrix product
rule remain analytic. The TT J2000 split preserves subday resolution without
subtracting two large Julian dates. Samples do not cross EOP knots or coverage
boundaries, and UTC leaps do not enter the stencil.

A 257-epoch sweep from 1972-01-01 to 2100-01-01 TT compares against both
256 s refinement and a separate fourth-order 1024 s stencil. Maximum
`X/Y/s` rate differences are `[4.665e-20, 8.801e-19, 3.512e-21]` rad/s;
test bounds are `[2e-18, 2e-18, 2e-20]` rad/s, conservatively below
1e-10 m/s at a 10,000 km lever arm. Six legacy analytic-evaluator regression
observations show maximum position/rate shifts of 5.846e-12 rad and
2.744e-17 rad/s when moving from rounded IERS tables to the full model.
These are numerical convergence/regression evidence, not operational
Earth-orientation accuracy guarantees or a new restriction on caller coverage.

### Dependency licensing and maintenance

The owner approved unmodified dependency replacement after verification.
The three unverified IERS table files and their evaluator have been
deliberately replaced. No table redistribution permission is inferred for
historical commits.

`erfa` 0.2.1 is separately MPL-2.0 licensed, with an ERFA BSD-style notice
in its actual combined LICENSE; the embedded arrays are in MPL-marked
software files. orskit does not relicense or copy them. Binary distributors,
including downstream users of `earth-rotation`, must retain
[`crates/frames-eop/NOTICE`](../../crates/frames-eop/NOTICE), the ERFA
attribution/disclaimer, and the dependency's MPL source-availability notices.
The full source and license are linked there. No SOFA endorsement is claimed.

**Maintenance rationale:** the dependency's upstream is not archived,
but its last push/release is from November 2022. Active maintenance has not been
verified or claimed. Active releases were a preference, not a publication
requirement. The pinned implementation of the fixed IAU standard is accepted
on the verified license/API, independent reference cases, derivative
convergence and complete Rust 1.96.1 test gates. Inactivity retains a risk of
delayed fixes and future compiler compatibility; updates require renewed
review and validation. Tests do not guarantee absence of unknown defects. The
[exact artifact audit](../../.agent/PROVENANCE.md#erfa-dependency-replacement-review-2026-10-10)
records hashes, public APIs, license terms, rejected alternatives and evidence.

Tides and equinox-based or truncated variants are out of scope. See
[ADR-0051](../../.agent/decisions/0051-iau2006-cio-earth-orientation.md).
