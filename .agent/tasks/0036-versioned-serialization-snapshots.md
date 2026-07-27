# Task: export versioned orbit and propagator snapshots

## Parity target

- Ledger row: stable public Rust facade
- Current status: Partial
- Intended status after this task: Partial, with an opt-in serialization boundary

## User workflow

A Rust caller selects a concrete orbit representation, registers a stable ID
for any application-owned gravity provider, creates an owned snapshot, and
serializes it with Serde or the opt-in JSON encoder. The same workflow exports
the physical problem and numerical configuration of the analytical elliptic
Kepler propagator.

## Scientific contract

- Inputs and units: typed Cartesian, circular, Keplerian, and equinoctial
  states; typed analytical-propagator settings.
- Outputs and units: owned snapshots whose raw scalar fields explicitly name
  metres, metres per second, radians, ratios, or cubic metres per square second.
- Frames/epochs/time scales: caller-registered stable frame identities and
  Hifitime epoch text, including its time scale, are retained.
- Conventions and valid regimes: snapshots preserve the selected state
  representation and its existing domain invariants.
- External data requirements: frames, opaque central-gravity providers, and
  provider origins require caller-assigned stable IDs; providers are matched
  by shared-allocation identity.
- Errors and singularities: blank, duplicate, or missing provider
  registrations return `ExportError`; existing state construction retains its
  own validation errors.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| Serde 1.0.228 | MIT OR Apache-2.0 dependency | Format-neutral Rust data-model serialization | `crates/export` |
| serde_json 1.0.150 | MIT OR Apache-2.0 dependency | Opt-in JSON encoding | `crates/export` |

No external scientific equations, test vectors, source, or file-format
specifications are used.

## Design

- Affected crates/layers: workspace manifest, `orskit-export`, analytical
  two-body configuration accessors, and the `orskit` facade.
- Public API: `ExportContext`, `ExportableState`, versioned state/propagator
  snapshots, and optional JSON helpers.
- Rejected alternatives: deriving Serde traits on domain objects; serializing
  trait-object internals; silently identifying numerically equal providers.
- ADR required: ADR-0038.

## Validation

- Unit cases: each built-in state representation, propagator configuration,
  frame/provider registration failures, feature isolation, and compact JSON.
- Invariants/properties: selected representation, frame, epoch/time scale,
  provider identity, and unit-qualified raw values are preserved.
- Independent reference vectors: not applicable; no numerical model is added.
- Differential/scenario tests: facade feature compilation and snapshot JSON.
- Tolerances and justification: exact comparisons are appropriate because
  snapshots copy already validated stored scalar values without computation.
- Benchmarks: not required; export is not a declared hot-path capability.

## Completion checklist

- [x] Implementation and typed errors
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance recorded
- [x] Parity ledger updated
- [x] Relevant checks pass
- [x] Binding impact explicitly deferred until the Rust API stabilizes
