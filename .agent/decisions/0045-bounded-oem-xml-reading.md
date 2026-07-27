# ADR-0045: share OEM semantics across bounded KVN and XML readers

- Status: Accepted
- Date: 2026-07-28
- Owners: orskit maintainers
- Affected parity rows: CCSDS orbit, attitude, tracking, and navigation
  messages

## Context

I10 requires blocking OEM XML ingestion without creating a second OEM domain
model or weakening the finite resource contracts established for KVN. CCSDS
502.0-B-3 defines OEM content and its XML representation. CCSDS 505.0-B-3
defines the NDM/XML declaration, namespace, root, header/body/segment
structure, and qualified and unqualified schema forms.

Loading an XML document tree would make memory proportional to the entire
message and discard the existing event workflow. General XML facilities such
as DTDs and processing instructions are unnecessary for OEM and enlarge the
attack surface.

## Decision

1. `OemXmlReader<R: BufRead>` is a blocking pull iterator that emits the
   existing `OemEvent` variants. `parse_oem_xml` is only a collecting
   convenience over that iterator, so XML and KVN share `Oem`, metadata,
   coordinates, comments, covariance, chronology, frames, epochs, and typed
   errors.
2. The reader uses `quick-xml` 0.41 with default features disabled. It retains
   one parser buffer plus the fields for the current state or covariance; it
   never builds a document tree.
3. Existing `OemDecoderLimits` bound physical line length, section bytes and
   lines, and total document bytes and lines. XML additionally has a fixed
   element-depth limit of 32 and a finite semantic-record count derived from
   the document-line budget. Scalar text is bounded before accumulation.
4. The accepted envelope is UTF-8 XML 1.0, OEM 3.0, and consistently
   unqualified or `ndm:`-qualified elements. The root must declare the exact
   XML Schema Instance and CCSDS NDM namespaces. Optional
   `xsi:noNamespaceSchemaLocation` values are treated as identifiers only;
   the parser performs no network access and does not fetch or execute a
   schema.
5. DTDs, processing instructions, undeclared entities, nested scalar content,
   unknown elements/attributes, inconsistent prefixes, and unsupported OEM
   fields are rejected. Predefined XML entities and character references are
   decoded locally.
6. One OEM document has exactly one header and body, and each `<segment>` maps
   to exactly one metadata/data pair and therefore exactly one
   `SegmentStart`/`SegmentEnd` event pair. Structural, resource, semantic, and
   chronology errors terminate iteration after the error item; callers never
   observe events decoded beyond an invalid prefix.
7. XML writing, lossless lexical preservation, full XSD validation, combined
   `<ndm>` documents, and message families other than OEM remain outside I10.
   Writing and semantic round trips belong to I11.

## Alternatives considered

- A DOM or serde-derived document model was rejected because it duplicates
  OEM semantics and requires whole-document allocation.
- Runtime XSD validation was rejected because it adds a large dependency and
  can imply ambient schema resolution. The typed parser validates the bounded
  supported subset directly.
- A format-specific XML output model was rejected because I10 does not
  authorize writing and I11 must first define lossless behavior.

## Consequences

Callers can stream CCSDS OEM XML through the same event workflow used for KVN
with finite resource use and no hidden I/O. This is a supported subset, not a
claim that arbitrary NDM/XML documents are schema-valid. Fields not represented
by the current OEM semantic model, including `CLASSIFICATION`,
`REF_FRAME_EPOCH`, and covariance comments, fail explicitly rather than being
silently discarded.

## Validation

Project-authored unqualified and derived qualified fixtures cover headers,
metadata, comments, states with optional acceleration, and covariance.
Negative cases cover declarations, namespaces, mixed qualification, malformed
markup, DTDs, root diagnostics, section cardinality, terminal chronology,
units, and inclusive line/resource limits at EOF and across CRLF input. An
isolated cargo-fuzz target consumes arbitrary bytes with a 256 KiB harness cap
and finite decoder budgets.

## Provenance

- CCSDS 502.0-B-3, *Orbit Data Messages*, Issue 3, April 2023: OEM 3.0 XML
  structure, field order, state-vector and covariance element names, units,
  and annex G example shape.
- CCSDS 505.0-B-3, *XML Specification for Navigation Data Messages*, Issue 3,
  May 2023: XML 1.0 UTF-8 declaration, NDM namespaces, qualified/unqualified
  forms, and header/body/segment structure.
- `quick-xml` 0.41.0: unmodified MIT-licensed streaming XML dependency. No
  dependency source or third-party parser implementation was copied.
