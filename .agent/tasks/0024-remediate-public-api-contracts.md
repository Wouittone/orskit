# Task 0024: remediate public API contracts

## Parity target

- Ledger rows: foundations/frames/orbits; propagation/dynamics; observation;
  CCSDS I/O; stable public Rust facade.
- Current status: Partial or Designed APIs admit ambiguous identities, invalid
  physical combinations, lossy ingestion, and topology claims that constructors
  do not preserve.
- Intended status after this task: the same honest parity statuses, backed by
  stricter public contracts and regression evidence for every reviewed misuse.

## User workflow

Build an orbit, spacecraft view, dynamics problem, observation, or OEM stream
through APIs that either preserve the required scientific context and source
provenance or reject the operation with a typed error before returning a
plausible result.

## Scientific contract

- Inputs and units: retain `uom` quantities and unit-named numerical solver
  boundaries; gravity conversion uses a positive typed parameter bound to an
  explicit frame origin and caller-supplied provenance.
- Outputs and units: existing typed state and observation quantities; lossy
  projections must be explicit rather than implicit conversions.
- Frames/epochs/time scales: osculating element conversion requires affirmative
  inertial axes; derived identities are resolved through collision-checked
  definitions; attitudes name moving/reference/expression frames; observation
  time-tag semantics are explicit; OEM epochs are strictly ordered per segment.
- Conventions and valid regimes: elliptic element conventions remain; zero-time
  propagation is identity; long-arc propagation is accepted only within a
  documented error policy; range leg and correction conventions are explicit.
- External data requirements: no hidden data or network access. Third-body and
  general force evaluation remain unavailable until explicit providers exist.
- Errors and singularities: typed errors cover frame/context mismatch,
  conflicting identity definitions, invalid geometry, unsupported propagation
  spans/regimes, decoder limits, duplicate/reversed epochs, and incomplete
  observation topology.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| Existing project ADRs, tasks, and adversarial review | Original project work | Public contracts must enforce their documented invariants | All affected crates and regression tests |
| CCSDS 502.0-B-3, May 2023 | Public standard already recorded in `PROVENANCE.md` | Ordered OEM records, comments, metadata, and bounded parsing | `crates/ccsds` |
| NASA GMAT Mathematical Specifications (2007) | US Government work already recorded in `PROVENANCE.md` | Elliptic anomaly propagation and valid frame/regime assumptions | `crates/core`, `crates/dynamics` |

No implementation material from Orekit, Lox, Nyx, or another astrodynamics
project is used.

## Design

- Affected crates/layers: `bodies`, `frames`, `core`, `dynamics`,
  `measurements`, `ccsds`, the public facade/package manifests, and linked
  project policy/evidence.
- Public API: deliberate pre-alpha breaking changes to conversion context,
  frame definitions, spacecraft construction, attitude rates, observations,
  decoder events/limits, force requirements, propagation problems, and package
  import roots.
- Rejected alternatives: document caller responsibility while retaining
  constructors that certify invalid states; silently discard source data;
  preserve misleading type names for compatibility; infer scientific data.
- ADR required: yes. Each durable boundary change updates or supersedes the
  relevant existing ADR in the same focused commit.

## Validation

- Unit cases: one regression test per adversarial misuse, plus nominal
  constructor/conversion/parser behavior.
- Invariants/properties: zero-duration identity; frame/context agreement;
  collision-free identities; topology preservation; monotonic OEM epochs;
  mode-equivalent decoder limits and ordered events.
- Independent reference vectors: retain all existing orbit and propagation
  vectors; no new parity claim is introduced.
- Differential/scenario tests: preserve existing Orekit/Lox endpoint evidence
  and add end-to-end OEM/observation context scenarios.
- Tolerances and justification: retain current validated orbital tolerances;
  long-duration acceptance receives an explicit phase-error bound or typed
  rejection.
- Benchmarks: rerun relevant OEM and propagation benchmarks only if the fixes
  materially alter hot-path allocation or arithmetic.

## Completion checklist

- [ ] Implementation and typed errors
- [ ] Scientific and regression tests
- [ ] Rustdoc/examples
- [ ] Provenance recorded
- [ ] Parity ledger updated
- [ ] Relevant checks pass
- [ ] Binding impact handled or explicitly deferred
