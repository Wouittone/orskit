# Task: verify caller-selected scientific-data artifacts

## Parity target

- Ledger row: Foundations / Explicit scientific data context and providers
- Current status: Partial
- Intended status after this task: Partial, with reusable version, digest,
  coverage, and deterministic offline-loading contracts

## User workflow

A caller declares the authority, product, immutable version, SHA-256 digest,
and temporal coverage of a local scientific-data file. The caller loads it
through an explicit byte limit and receives immutable bytes only after checksum
verification. A format-specific provider then parses those bytes and checks the
declared coverage for each requested epoch.

## Scientific contract

- Inputs and units: local bytes; Hifitime epochs for inclusive coverage.
- Outputs and units: immutable verified bytes and an inspectable descriptor.
- Frames/epochs/time scales: coverage compares absolute Hifitime instants and
  does not reinterpret civil time labels.
- Conventions and valid regimes: SHA-256 text is exactly 64 hexadecimal digits;
  one interval includes both endpoints; `AllTime` is explicit.
- External data requirements: the application selects the exact local path,
  descriptor, digest, and maximum byte count.
- Errors and singularities: blank identities, malformed digests, reversed or
  missed coverage, I/O failures, size-limit exhaustion, and checksum mismatch
  are typed.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| NIST FIPS 180-4 and CAVP secure-hashing vectors | US Government standard and validation data | SHA-256 digest definition and independent known result | `crates/data` |
| RustCrypto `sha2` 0.10.9 | MIT OR Apache-2.0 dependency | Unmodified SHA-256 implementation | `crates/data` |

## Design

- Affected crates/layers: new foundational `orskit-data`, frames supplier
  provenance, public facade, workspace documentation.
- Public API: `Sha256Digest`, `ArtifactDescriptor`, `ArtifactCoverage`,
  `TimeCoverage`, and `VerifiedArtifact`.
- Rejected alternatives: free-form optional checksum strings; a global mutable
  data catalog; implicit network/cache lookup; implementing SHA-256 locally.
- ADR required: ADR-0039.

## Validation

- Unit cases: identity and digest parsing, coverage, checksum mismatch, bounded
  local load, and I/O source preservation.
- Invariants/properties: verified bytes always match the retained descriptor;
  coverage includes both declared endpoints.
- Independent reference vectors: standard SHA-256 result for `abc`.
- Differential/scenario tests: frame supplier consumes the common descriptor.
- Tolerances and justification: exact byte digest and epoch ordering; no
  floating-point scientific model is implemented.
- Benchmarks: not required; loading is explicit setup work.

## Completion checklist

- [x] Implementation and typed errors
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance recorded
- [x] Parity ledger updated
- [x] Relevant checks pass
- [x] Binding impact explicitly deferred until the Rust API stabilizes
