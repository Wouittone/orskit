# Task: read OEM XML with bounded streaming semantics

## Parity target

- Ledger row: I/O / CCSDS orbit, attitude, tracking, and navigation messages
- Current status: Partial, OEM KVN only
- Intended status after this task: Partial, OEM KVN plus bounded blocking OEM
  3.0 XML reading

## User workflow

A caller passes any blocking `BufRead` source to `OemXmlReader` and consumes
header, segment, comment, coordinate, covariance, and segment-end events
without retaining the full message. A caller that wants an in-memory value
uses `parse_oem_xml` and receives the same `Oem` model used by KVN.

## Scientific contract

- Inputs and units: CCSDS OEM 3.0 XML; positions in kilometres, velocities in
  kilometres per second, optional accelerations in kilometres per second
  squared, and covariance terms in their CCSDS compound units.
- Outputs and units: existing typed OEM events and document values; quantities
  are converted at the same explicit serialization boundary as KVN.
- Frames/epochs/time scales: frame, centre, and time-system metadata are
  mandatory and drive the existing typed coordinate and epoch construction.
- Conventions and valid regimes: UTF-8 XML 1.0; exact CCSDS NDM namespaces;
  consistently unqualified or `ndm:`-qualified tags; OEM issue 3.
- External data requirements: none. Schema-location attributes are not
  dereferenced and no network access occurs.
- Errors and singularities: malformed XML, I/O, declaration/namespace/schema
  subset violations, invalid units or values, chronology, and every finite
  resource limit are typed errors.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| CCSDS 502.0-B-3, *Orbit Data Messages*, Issue 3, April 2023 | Public recommended standard | OEM 3.0 XML elements, units, ordering, and annex G example shape | `crates/ccsds/src/oem/xml.rs`; project XML fixture and tests |
| CCSDS 505.0-B-3, *XML Specification for Navigation Data Messages*, Issue 3, May 2023 | Public recommended standard | XML declaration, namespace, qualification, and NDM structure rules | XML decoder, namespace tests, ADR-0045 |
| `quick-xml` 0.41.0 | MIT dependency used unmodified | Pull XML tokenization only | `crates/ccsds`; isolated XML fuzz target |

## Design

- Affected crates/layers: `ccsds`, its isolated fuzz workspace, and assurance
  records.
- Public API: `OemXmlReader`, `parse_oem_xml`, and
  `parse_oem_xml_with_limits`; existing `OemEvent`, `Oem`, limits, and errors
  remain the semantic boundary.
- Rejected alternatives: document-tree parsing, runtime schema loading, a
  parallel XML model, and XML writing in this slice.
- ADR required: ADR-0045.

## Validation

- Unit cases: unqualified and qualified input, malformed markup, DTD,
  declaration, root ID and namespace diagnostics, metadata/data cardinality,
  units, terminal chronology, and inclusive finite budgets at EOF and across
  CRLF input.
- Invariants/properties: event collection produces the existing OEM model;
  coordinate chronology and metadata semantics match KVN; no events follow a
  terminal error and every accepted segment has one coherent event pair.
- Independent reference vectors: project-authored fixture follows the
  structures and element names in CCSDS 502.0-B-3 annex G and CCSDS
  505.0-B-3.
- Differential/scenario tests: event counts and collected values are checked
  from the same fixture.
- Tolerances and justification: no new numerical approximation is introduced.
- Benchmarks: existing 100 MiB benchmark remains KVN-specific; XML timing is
  deferred until an operational XML corpus exists.

## Completion checklist

- [x] Implementation and typed errors
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance recorded
- [x] Parity ledger updated
- [x] Relevant checks pass
- [x] Binding impact explicitly deferred until the Rust API stabilizes
