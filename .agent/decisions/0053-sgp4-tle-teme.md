# ADR-0053: strict TLE parsing and opt-in SGP4/TEME propagation

- Status: Accepted
- Date: 2026-10-03
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

1. Add a standalone `tle` crate. It accepts exactly two 69-byte ASCII lines,
   verifies fixed columns, checksums, field syntax/ranges and catalog agreement,
   expands the standard two-digit epoch pivot, and retains the original records
   for lossless line formatting.
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

## Provenance

The strict columns and Alpha-5 behavior follow the CelesTrak TLE format and
Space-Track Alpha-5 documentation. Model conventions and expected output vectors
come from Vallado et al., *Revisiting Spacetrack Report #3*, AIAA 2006-6753,
Revision 3. The unmodified SGP4 dependency provides the propagation algorithm;
project-owned code is limited to typed input validation, setup, time/frame/unit
adaptation, and tests. See `.agent/PROVENANCE.md`.
