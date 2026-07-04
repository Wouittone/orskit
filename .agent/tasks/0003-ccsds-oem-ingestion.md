# Task: stream CCSDS OEM states into orskit domain types

## Parity target

- Ledger row: I/O / CCSDS orbit, attitude, tracking, and navigation messages
- Current status: Not assessed
- Intended status after this task: Partial

## User workflow

Read a CCSDS 502.0-B-3 OEM in KVN form from a blocking or Tokio buffered
reader and consume each ephemeris point as an orskit Cartesian state without
loading the complete message. For in-memory large messages, parse state lines
in deterministic order with Rayon and collect an OEM document.

## Scientific contract

- Inputs and units: OEM KVN position in km, velocity in km/s, optional
  acceleration in km/s2.
- Outputs and units: `orskit-units` quantities stored in SI through a
  `CartesianCoordinates` value that can be explicitly enriched into a state.
- Frames/epochs/time scales: every state component carries the frame composed
  from `CENTER_NAME` and `REF_FRAME`; epochs are Hifitime `Epoch` values parsed
  using the segment `TIME_SYSTEM`.
- Conventions and valid regimes: CCSDS 502.0-B-3 OEM KVN, including multiple
  segments and optional acceleration. Initially supported absolute time systems
  are those Hifitime can represent directly.
- External data requirements: none; no data or leap-second files are downloaded.
- Errors and singularities: malformed lines, missing metadata, unsupported time
  systems/frames, non-finite values, covariance blocks, and I/O failures return
  typed errors with line context.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| CCSDS 502.0-B-3, Orbit Data Messages, Issue 3, May 2023 | Public standard | OEM KVN sections, required metadata, state record units and ordering | `orskit-ccsds` parser and tests |
| SANA Orbit Centers, Time Systems, and Celestial Body Reference Frames registries | Public registries | Names at frame/time parsing boundaries | `orskit-frames`, `orskit-ccsds` |
| `ccsds-ndm` 0.0.9 public API and package metadata | MPL-2.0 dependency evaluation only | Broad NDM support; whole-string/file APIs and owned physical types | ADR-0003 |
| `lox-odm` 0.1.0-alpha.3 public API and package metadata | MPL-2.0 dependency evaluation only | ODM scope, alpha/MSRV status, Lox frame/time coupling | ADR-0003 |

No implementation source, tests, or prose are copied from another
astrodynamics project or parser.

## Design

- Affected crates/layers: `orskit-core`, `orskit-frames`, new edge-layer
  `orskit-ccsds` crate.
- Public API: `CartesianCoordinates`; frame component parsing; blocking and async OEM
  event readers; sequential and parallel document parsers.
- Rejected alternatives: directly expose `ccsds-ndm` or `lox-odm` message types;
  materialize all input before parsing; add mass not present in OEM to create a
  `SpacecraftState`.
- ADR required: yes, ADR-0003.

## Validation

- Unit cases: known frame aliases, single/multiple OEM segments, optional
  acceleration, UTC/TAI epochs, malformed and unsupported inputs.
- Invariants/properties: sequential, Tokio, and Rayon paths produce the same
  ordered domain states.
- Independent reference vectors: CCSDS 502.0-B-3 OEM example-shaped records;
  values are checked at the documented km and km/s conversion boundary.
- Differential/scenario tests: compare all parser modes over the same message.
- Tolerances and justification: parsing and decimal-to-SI conversion use exact
  equality only for values exactly representable in the fixtures; no numerical
  model is introduced.
- Benchmarks: Criterion throughput for approximately 100 MiB OEM input on the
  streaming, sequential collection, and parallel collection paths; recorded in
  `.agent/benchmarks/2026-07-04-oem-kvn-100-mib.md`.

## Completion checklist

- [x] Implementation and typed errors
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance recorded
- [x] Parity ledger updated
- [x] Relevant checks pass
- [x] Binding impact handled or explicitly deferred: no new binding surface while
  the Rust CCSDS contract is partial
