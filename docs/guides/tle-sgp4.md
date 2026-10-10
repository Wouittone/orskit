# Strict TLE parsing and SGP4 propagation

Enable `orskit`'s `sgp4` feature to access both `orskit::tle` and the
`orskit::dynamics::sgp4` propagator. The parser is also available independently
through the `tle` crate, with no propagation feature enabled.

```rust
use dynamics::{sgp4::Sgp4Propagator, Propagator};
use hifitime::Duration;
use tle::TwoLineElement;

let elements = TwoLineElement::parse(
    "1 00005U 58002B   00179.78495062  .00000023  00000-0  28098-4 0  4753",
    "2 00005  34.2682 348.7242 1859667 331.7664  19.3264 10.82419157413667",
)?;
let propagator = Sgp4Propagator::try_from(&elements)?;
let initial = propagator.initial_orbit();
let target = initial.epoch() + Duration::from_seconds(360.0 * 60.0);
let state = propagator.propagate(initial, target)?;
assert_eq!(state.as_ref().frame(), frames::ReferenceFrame::TEME);
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Parsing contract

`TwoLineElement::parse` takes exactly two 69-byte ASCII records without line
terminators. It validates line number, fixed spaces, field syntax and ranges,
checksums, and matching catalog numbers. The combined `FromStr` parser accepts
exactly two lines. It reports malformed input through `TleError`, including
line, column, and field context where applicable.

The two-digit epoch pivot is `57..=99` → 1957–1999 and `00..=56` → 2000–2056.
Day-of-year begins at 1 and is bounded by that year's calendar length.
Alpha-5 catalog IDs are decoded to numeric IDs; formatting still preserves the
original five catalog columns, checksums, signed zero spellings, and all other
record columns exactly. The line separator emitted by `Display` is LF.

## Propagation contract

The `sgp4` feature accepts distributed-data ephemeris type 0. It initializes
the Vallado SGP4/SDP4 model with WGS-72 constants and AFSPC-compatible epoch,
sidereal-time, and propagation conventions using the unmodified `sgp4` 2.4.0
dependency. Model output is geocentric TEME position and velocity, converted
from kilometres and kilometres per second to metres and metres per second.
Hifitime epochs are used at both boundaries; the input TLE epoch is UTC.
Target propagation uses elapsed SI time from the TLE epoch.

The propagator owns its mean elements and epoch. Its `initial_orbit()` result is
the seed to pass to `Propagator<CartesianState>`; a different seed is rejected
with a typed error. Use `state_at(epoch)` for direct absolute-epoch queries.
Dependency initialization and propagation failures are retained as typed
sources. Unsupported ephemeris selectors fail during conversion. No separate
decay classification or implicit extrapolation policy is added.

TEME is not relabelled as GCRF, ITRF, or another frame. This feature provides no
TEME transformation, covariance propagation, maneuver support, or bundled
ephemeris data.

## Validation and limits

Near-Earth and deep-space cases from Vallado et al., *Revisiting Spacetrack
Report #3*, AIAA 2006-6753, Revision 3, compare against the published TEME
vectors with 1 m position and 1 mm/s velocity tolerances. The vectors are
rounded in their published representation; the tolerance applies only to these
fixtures. It is not an operational accuracy guarantee or a claim of complete
SGP4 verification-corpus coverage.

Parser malformed-input regression coverage is under `crates/tle/tests`; the
standalone fuzz target and command are documented in `crates/tle/fuzz/README.md`.
Provenance is recorded in `.agent/PROVENANCE.md`; the design boundary is ADR-0053.
