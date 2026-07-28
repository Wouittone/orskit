# Task: maintain OEM KVN conformance evidence and bounded fuzzing

## Parity target

- Ledger row: I/O / CCSDS orbit, attitude, tracking, and navigation messages
- Current status: Partial
- Intended status after this task: Partial with a maintained OEM KVN
  conformance corpus and bounded parser fuzz target

## User workflow

A contributor can run stable semantic checks over provenance-cleared OEM KVN
fixtures, seed a bounded cargo-fuzz run from those fixtures, and preserve every
minimized discovery as an ordinary regression test.

## Scientific contract

- Inputs and units: untrusted OEM KVN bytes; valid state rows use kilometres,
  kilometres per second, and optional kilometres per second squared.
- Outputs and units: the existing typed OEM document/events and existing
  contextual parse errors.
- Frames/epochs/time scales: corpus coverage includes Earth/EME2000/UTC,
  Mars/ICRF/TAI, and RTN/EME2000 covariance axes.
- Conventions and valid regimes: the existing CCSDS 502.0-B-3 OEM KVN reader;
  this task changes no accepted syntax.
- External data requirements: none at runtime or test time.
- Errors and singularities: arbitrary bytes must not panic or exceed explicit
  parser budgets; sequential and parallel collectors must agree on acceptance.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| CCSDS 502.0-B-3, *Orbit Data Messages*, Issue 3, May 2023 | Public standard | Existing OEM KVN syntax, units, sections, and record combinations only | Project-authored `project_multisegment.oem` and its semantic tests |
| Orekit `OEM-Issue839.txt` | Apache-2.0 test resource already approved and attributed | Existing RTN and EME2000 covariance interoperability rows | Existing covariance fixture and new corpus-level semantic test |
| `libfuzzer-sys` 0.4.13 | `(MIT OR Apache-2.0) AND NCSA` developer dependency | In-process coverage-guided fuzz runner only | Isolated `crates/ccsds/fuzz` workspace |

No parser implementation, external test code, or unattributed web sample was
copied. The multi-segment fixture, harness, tests, and documentation are
original project work.

## Design

- Affected crates/layers: CCSDS test data, integration tests, isolated
  developer fuzz workspace, and evidence documentation.
- Public API: unchanged.
- Rejected alternatives: vendoring operational examples with unclear
  redistribution terms; adding fuzz dependencies to the shipping workspace;
  unbounded fuzz inputs; committing generated corpora and crash artifacts.
- ADR required: no; this applies the existing untrusted-input and provenance
  policies without changing architecture or a public contract.

## Validation

- Unit cases: project fixture segment/frame/time/acceleration semantics and
  attributed covariance-axis semantics.
- Invariants/properties: arbitrary bytes are consumed under hard limits;
  sequential and ordered-parallel modes agree on success and parsed values.
- Independent reference vectors: the existing attributed Orekit covariance
  resource.
- Differential/scenario tests: project-authored multi-segment OEM scenario.
- Tolerances and justification: exact structural equality; no approximate
  numerical comparison is introduced.
- Benchmarks: not applicable; existing OEM benchmark evidence is unchanged.

## Completion checklist

- [x] Implementation and typed errors (existing public errors unchanged)
- [x] Scientific and regression tests
- [x] Fuzz harness and reproducible workflow documentation
- [x] Provenance recorded
- [x] Parity ledger updated
- [x] Relevant checks pass
- [x] Binding impact explicitly deferred; no binding files changed
