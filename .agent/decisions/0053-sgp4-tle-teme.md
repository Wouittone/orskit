# ADR-0053: strict TLE parsing and opt-in SGP4/TEME propagation

- Status: Accepted
- Date: 2026-10-03
- Review amendment: 2026-10-10 (PR #37, owner-requested parser/serde reuse)
- Owners: propagation and I/O maintainers
- Affected parity rows: Propagation / TLE/SGP4; I/O / TLE

## Context

Issue #32 requests a strict TLE parser and SGP4/SDP4 propagation to a
frame-qualified TEME state. SGP4/SDP4 contains substantial model-specific
numerics, and the repository prohibits copying or translating external
implementation source. The current `Propagator<State>` trait advances and
returns the same state representation, while TLE mean elements are not
Cartesian state.

## Decision

1. Add a standalone `tle` crate using the already approved, unmodified MIT
   `sgp4` 2.4.0 `Elements::from_tle` parser and `Elements` storage. A strict
   validation adapter checks ASCII, columns, checksums, lexical/range rules,
   catalog agreement, and the standard epoch pivot. Accept two 69-byte data
   lines with an optional preceding name (3LE). Retain the original records
   for lossless writing; the dependency has no TLE text writer. Serde source
   records deserialize through the same validation. The external elements'
   separate serde format is OMM, not TLE text.
2. Keep parsing and propagation separate. The optional `tle/sgp4` adapter
   converts distributed-data ephemeris type 0 records into a configured
   propagator; other ephemeris types fail with a typed conversion error.
3. Put propagation in the separately feature-gated `dynamics-sgp4` crate. It
   uses `sgp4` 2.4.0 unmodified as a separately licensed MIT dependency,
   configured for WGS-72 and AFSPC-compatible epoch, sidereal-time, and
   propagation conventions. No dependency source, test, or example is copied
   or translated. The project tests use the paper's published verification
   values independently of the dependency's tests.
4. The model owns its TLE epoch and mean elements. `initial_orbit()` returns the
   Cartesian SGP4 state at that epoch. The current common trait is
   `Propagator<CartesianState>`; it verifies that its input seed matches this
   model at the seed epoch before evaluating the requested target. `state_at`
   offers direct absolute-epoch queries without a seed.
5. Model input angles and mean motion use typed quantities; B* is an explicitly
   unit-named serialization/model boundary scalar in inverse Earth radii.
   Dependency outputs in km and km/s are converted immediately to typed SI
   units. Every returned state is explicitly tagged `ReferenceFrame::TEME`.
   Epochs use Hifitime and the parsed TLE epoch is constructed from UTC year,
   day-of-year, and the exact 1e-8-day fractional resolution.
   Chrono (already re-exported by `sgp4`) handles the civil calendar. Correct
   the dependency's floating-point day fraction to integer TLE ticks before
   conversion to Hifitime; never approximate a UTC ordinal with elapsed SI
   days across leap seconds. Blank implied-exponent zero fields are normalized
   only in temporary parser input, preserving their original text on writing.
6. Do not implement TEME conversions, other legacy ephemeris types, a separate
   decay classifier, covariance, maneuvers, or an operational accuracy claim.
   Dependency propagation and initialization failures remain typed and retain
   their error sources.
7. Keep the `tle` parser and SGP4 propagation opt-in on the `orskit` facade;
   neither changes its default feature set.

## Alternatives considered

- **Translate a public or third-party SGP4 implementation:** rejected because
  the source reuse policy disallows source-derived implementation work.
- **Put SGP4 inside `tle`:** rejected because file-format I/O must not own
  propagation and the dynamics layer must not depend on a parser.
- **Allow any Cartesian seed to be ignored by the model:** rejected; checking
  that the input matches the TLE-defined solution preserves the common trait's
  meaning and reports mismatches rather than silently discarding state.
- **Normalize formatted TLE lines:** rejected because it is not lossless for
  records containing alternative valid spellings such as signed zero.
- **Convert TEME into GCRF or ITRF:** deferred until independently validated
  transformation data and conventions are available.

## Validation and tolerance

Project-authored tests use the published Vallado near-Earth and deep-space
verification cases at +360 and -5184 minutes from epoch. Rounded published
vectors are compared with 1 m position and 1 mm/s velocity tolerances.
Additional tests cover exact UTC epoch conversion, frame and unit conversion,
seed mismatch, strict parser failures, round trips, epoch pivot/range behavior,
Alpha-5 IDs, and a retained malformed-input regression. These results validate
the selected vectors and adapter, not full model parity or operational use.
The dependency documents reference-CelesTrak comparisons and benchmark error
bounds; these are upstream evidence, not a project certification. Rust 1.96.1
is verified locally because `sgp4` declares no MSRV. New tests cover plain and
`0 `-prefixed names, CRLF, strict serde validation, OMM/source-format distinction,
blank zero/signed zero preservation, leap-day/year-end/pivot calendar boundaries,
and identical model output after named serde round trips. Remove the scoped
fuzz workspace and corpus per owner review; retain the checksum regression.

## Provenance

The strict columns and Alpha-5 behavior follow the CelesTrak TLE format and
Space-Track Alpha-5 documentation. Model conventions and expected output vectors
come from Vallado et al., *Revisiting Spacetrack Report #3*, AIAA 2006-6753,
Revision 3. The unmodified SGP4 dependency provides the propagation algorithm;
project-owned code is limited to strict format validation, lossless source-record
writing, typed input validation, setup, time/frame/unit adaptation, and tests.
The owner explicitly approved TLE parser/element dependency reuse in PR #37.
The same maintained package avoids another orbital library/algorithm/license
boundary. Serde and serde_json use MIT OR Apache-2.0; Chrono already exists in
the dependency graph under MIT OR Apache-2.0. Only public API/metadata/behavior
were used to author the adapter; no dependency source, tests, or examples were
copied. See `.agent/PROVENANCE.md` and the guide for deliberate compatibility
changes and writing limits.
