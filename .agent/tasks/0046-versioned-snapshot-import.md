# Task: import versioned orbit and propagator snapshots

## Parity target

- Ledger row: stable public Rust facade
- Current status: Partial
- Intended status after this task: Partial, with validated bidirectional snapshots

## User workflow

A Rust caller decodes an owned snapshot, registers the exact live frames,
origins, and gravity providers it trusts, and reconstructs an orbit state or
analytical propagator through the normal validated domain constructors.

## Scientific contract

- Inputs and units: versioned snapshots whose raw fields retain explicit SI,
  ratio, or radian names.
- Outputs and units: typed orbit states and analytical propagator settings.
- Frames/epochs/time scales: the epoch is parsed by Hifitime; stable frame IDs
  resolve only through caller registrations.
- Conventions and valid regimes: representation and implementation
  discriminators are exact, versioned contracts.
- External data requirements: opaque providers are supplied by the caller and
  checked against serialized origin and gravitational-parameter metadata.
- Errors and singularities: malformed schemas, epochs, identities, metadata,
  discriminators, and domain values return typed `ImportError` variants.

## Provenance

No external scientific equations, test vectors, source, or file-format
specifications are used. Serde and Hifitime provide parsing infrastructure;
domain validation remains in the existing orskit constructors.

## Design

- Affected crates/layers: `orskit-export`, the public facade, documentation,
  and parity evidence.
- Public API: `ImportContext`, `ImportableState`, `ImportError`,
  `OrbitSnapshot::try_into_orbit`, and
  `EllipticKeplerPropagatorSnapshot::try_into_propagator`.
- Rejected alternatives: trusting serialized provider parameters; globally
  resolving IDs; bypassing constructors; silently accepting future schemas.
- ADR required: amendment to ADR-0038.

## Validation

- Unit cases: application-defined state extension, all built-in state
  representations, analytical propagator settings, schema and epoch failures.
- Invariants/properties: epoch, representation, coordinates, provider
  allocation identity, and numerical settings survive a round trip.
- Independent reference vectors: not applicable; no numerical model is added.
- Differential/scenario tests: JSON encode/decode plus reconstruction.
- Tolerances and justification: exact comparisons apply because snapshots copy
  stored values and reconstruction uses the same typed constructors.
- Benchmarks: not required; persistence is not a declared hot path.

## Completion checklist

- [x] Implementation and typed errors
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance recorded
- [x] Parity ledger updated
- [x] Relevant checks pass
- [x] Binding impact explicitly deferred until the Rust API stabilizes
