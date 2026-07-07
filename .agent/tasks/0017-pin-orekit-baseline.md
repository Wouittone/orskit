# Task 0017: pin Orekit parity baseline

## Parity target

- Ledger row: all parity-ledger rows through the reference-baseline metadata
- Current status: Orekit baseline not pinned
- Intended status after this task: baseline pinned to Orekit 13.1.7 with a
  versioned inventory revision and refresh rules

## User workflow

Review the parity ledger and know exactly which Orekit release defines the
capability inventory, which evidence is current, and which older comparison
fixtures still need refresh before any `Validated` claim.

## Scientific contract

- Inputs and units: none.
- Outputs and units: versioned reference metadata and parity-ledger rules.
- Frames/epochs/time scales: none.
- Conventions and valid regimes: capability inventory only; no scientific
  algorithm or transform is implemented.
- External data requirements: public Orekit release, download, news, and
  Javadoc pages.
- Errors and singularities: drift-prone `latest` links and unpinned parity
  claims are rejected by policy.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| Orekit download/news/Javadoc pages, 13.1.7 | Public project documentation; Apache-2.0 project, CC BY 3.0 website pages | latest release, release date, API documentation version list | `.agent/baselines/orekit-13.1.7.md`, `.agent/PARITY.md`, `.agent/PROVENANCE.md` |

## Design

- Affected crates/layers: `.agent` governance and parity docs only.
- Public API: none.
- Rejected alternatives: keep `not yet pinned`; pin to the older 13.1.6
  fixture version; use `site-orekit-latest` as a moving inventory target.
- ADR required: no; this is the direct execution of Milestone 0's first
  roadmap item under the existing provenance policy.

## Validation

- Unit cases: none.
- Invariants/properties: parity baseline is explicit and no `latest` Javadoc
  link remains for the force-model inventory.
- Independent reference vectors: none.
- Differential/scenario tests: none.
- Tolerances and justification: none.
- Benchmarks: none.

## Completion checklist

- [x] Implementation and typed errors
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance recorded
- [x] Parity ledger updated
- [x] Relevant checks pass
- [x] Binding impact handled or explicitly deferred
