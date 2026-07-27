# Task: validate time boundaries and portable public workflows

## Parity target

- Ledger rows: time scales/calendars/durations; stable public Rust facade
- Current status: Partial
- Intended status after this task: Partial with authoritative leap-boundary
  evidence, executable API examples, a deliberate feature matrix, and
  cross-platform CI

## User workflow

A contributor can run representative feature combinations locally, compile
focused examples for the core state/propagation/orbit/OD contracts, and rely on
CI coverage across Linux, macOS, and Windows. The project directly validates
Hifitime UTC/TAI behavior at the 2015 and 2016 leap-second boundaries without
introducing a weaker time wrapper.

## Scientific contract

- Inputs and units: Gregorian UTC instants and exact SI-second elapsed time.
- Outputs and units: corresponding TAI Gregorian instants and exact Hifitime
  durations.
- Frames/epochs/time scales: UTC, TAI, TT, TDB, ET, GPST, GST, and BDT;
  orbit examples retain explicit GCRF and TAI epochs.
- Conventions and valid regimes: IERS-announced positive leap seconds at the
  end of 2015-06-30 and 2016-12-31.
- External data requirements: none at runtime; fixed authoritative bulletin
  facts are test evidence.
- Errors and singularities: no new public error boundary. Hifitime 4.3 does
  not retain `23:59:60` as an instant distinct from `23:59:59`; a regression
  test and the parity ledger record this limitation.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| IERS Bulletin C 49, 2015-01-05 | Public authoritative bulletin | Positive leap second at the end of 2015-06-30 and UTC-TAI change effective 2015-07-01 | `crates/core/tests/time_scales.rs` |
| IERS Bulletin C 52, 2016-07-06 | Public authoritative bulletin | Positive leap second at the end of 2016-12-31 and UTC-TAI change effective 2017-01-01 | `crates/core/tests/time_scales.rs` |
| BIPM leap-second table through 2018 | Public authoritative metrology table | 2015/2017 effective dates and TAI-UTC = 37 seconds after the 2016 insertion | `crates/core/tests/time_scales.rs` |

No external implementation or test source was copied.

## Design

- Affected crates/layers: test-only time evidence in `core`; rustdoc in
  `core`, `dynamics-core`, `orbits`, and `orbit-determination`; CI/tooling.
- Public API: unchanged.
- Rejected alternatives: a new time wrapper; every mathematical feature power
  set; tripling formatting/Clippy/docs CI work on every host.
- ADR required: no; existing architecture and feature contracts are applied.

## Validation

- Unit cases: two UTC leap boundaries and their TAI representations.
- Invariants/properties: elapsed physical time counts the inserted second
  after converting civil UTC endpoints to uniform TAI; one fractional instant
  round-trips internally through supported scales. The latter is not an
  independent accuracy claim for modeled ET/TDB conversions.
- Independent reference vectors: IERS Bulletin C 49/C 52 and BIPM table.
- Differential/scenario tests: four focused doctests and representative
  facade/export feature combinations.
- Tolerances and justification: authoritative UTC/TAI boundary endpoints and
  elapsed nanoseconds use exact equality. The other-scale case checks only
  internal round-trip identity for one instant; it makes no external accuracy
  or model-error claim.
- Benchmarks: not applicable.

## Completion checklist

- [x] Implementation and typed errors (no new error boundary)
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance recorded
- [x] Parity ledger updated
- [x] Relevant local checks pass; the first hosted macOS/Windows matrix run is
  pending CI execution
- [x] Binding impact explicitly deferred; no binding files changed
