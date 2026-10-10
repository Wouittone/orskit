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
two data lines or a preceding printable, nonblank ASCII name plus the two data
lines (3LE), with LF or CRLF input. Both plain names and `0 `-prefixed names are
retained exactly. It reports malformed input through `TleError`, including
line, column, and field context where applicable.

The two-digit epoch pivot is `57..=99` → 1957–1999 and `00..=56` → 2000–2056.
Day-of-year begins at 1 and is bounded by that year's calendar length.
Alpha-5 catalog IDs are decoded to numeric IDs; formatting still preserves the
original five catalog columns, checksums, signed zero spellings, and all other
record columns exactly. The line separator emitted by `Display` is LF.

The pinned, MIT-licensed `sgp4` 2.4.0 public `Elements::from_tle` API decodes
and stores the orbital fields, catalog identity, and UTC calendar timestamp.
`TwoLineElement` adds stricter lexical/range checks than the dependency, plus
a source-preserving text adapter: **the dependency has no TLE text writer**.
`Display` writes the validated original 2LE/3LE records; it does not generate
new records from edited orbital elements. This retains the existing lossless
writing contract without inventing another orbital-data model.

Blank implied-exponent fields retain their previous accepted meaning of zero:
only the temporary dependency input is normalized (and its checksum updated).
The original lines are never normalized. Epoch fractions are converted to exact
integer nanoseconds at 1e-8-day resolution (864 microseconds), replacing the
dependency's floating-point calendar fraction. Its re-exported Chrono calendar
handles day-of-year/leap-year conversion; Hifitime remains the propagation
epoch/time-scale API. No fixed month-length tables or new calendar dependency
are introduced.

## Serde data versus TLE text

`TwoLineElement` implements serde `Serialize` and `Deserialize` using
`object_name` (optional), `line_one`, and `line_two`. Reads re-run all validation;
unknown fields, invalid names, malformed columns, and wrong checksums fail.
This representation preserves both names and original numeric spelling.

```rust
use tle::TwoLineElement;

let source = "0 NOAA 14\n\
              1 23455U 94089A   97320.90946019  .00000140  00000-0  10191-3 0  2621\n\
              2 23455  99.0090 272.6745 0008546 223.1686 136.8816 14.11711747148495";
let tle: TwoLineElement = source.parse()?;
let json = serde_json::to_string(&tle)?;
let restored: TwoLineElement = serde_json::from_str(&json)?;
assert_eq!(restored.to_string(), source);
# Ok::<(), Box<dyn std::error::Error>>(())
```

`AsRef<sgp4::Elements>` exposes a borrowed external element representation.
Its own serde representation is OMM data (for example, `NORAD_CAT_ID`, `EPOCH`,
and expanded `OBJECT_ID`), **not TLE text**. Serializing it does not preserve
source spellings; arbitrary OMM-to-TLE generation is not provided. The existing
`international_designator()` accessor still returns the original short TLE
designator, rather than the dependency's expanded OMM designator.

Deliberate compatibility changes: combined text input now accepts 3LE in
addition to 2LE, serde source-record data is supported, and `epoch_year()` is
no longer a `const fn` because it reads the external calendar type. Existing
fixed-column errors, catalog/pivot/range checks, lossless formatting, and
ephemeris-type restrictions remain unchanged.

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

Parser malformed-input, serde/2LE/3LE round-trip, calendar boundary, and named
propagation regressions are under `crates/tle/src/lib.rs` and `crates/tle/tests`.
The scoped fuzz workspace and corpus were removed at the reviewer's request;
the malformed-checksum regression remains as an ordinary integration test.
Provenance is recorded in `.agent/PROVENANCE.md`; the design boundary is ADR-0053.
