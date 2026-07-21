# Task: Make governance, developer tooling, and current Rust workflows approachable

## Parity target

- Ledger row: Rust facade, analytical propagation, and sequential filters are
  documented, but this task changes no capability status.
- Current status: Partial.
- Intended status after this task: Partial, with clearer reproducible entry
  points and no new parity claim.

## User workflow

A contributor can select an appropriately scoped issue, reproduce the pinned
Rust environment, discover the standard checks, understand crate dependency
direction, run current two-body and Cartesian OD examples, and implement an
application-specific propagation pair. Maintainers can review changes and
prepare pre-1.0 release notes against explicit policy.

## Scientific contract

- Inputs and units: tutorials use typed SI position, velocity, gravitational
  parameter, and covariance standard deviations.
- Outputs and units: propagated and estimated Cartesian state remains typed;
  example display labels raw boundary values in metres and metres per second.
- Frames/epochs/time scales: tutorials use Earth-centered GCRF and explicit
  Hifitime TAI epochs/absolute targets.
- Conventions and valid regimes: two-body examples are limited to bound
  elliptic point-mass motion; OD uses the current sequential Cartesian position
  boundary and diagonal public covariance constructors.
- External data requirements: none; the tutorial selects a conventional Earth
  gravitational parameter explicitly and downloads no scientific data.
- Errors and singularities: guides name finite-state, frame/origin,
  positive-definite covariance, elliptic-regime, convergence, and phase-budget
  failures and preserve typed `Result` handling.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| IERS Conventions (2010), Table 1.1 | Public scientific convention | Conventional Earth gravitational parameter used illustratively | `docs/tutorials/two-body-propagation.md`; `two_body_propagation` example |
| Cargo metadata emitted by pinned Cargo | Project build metadata | Current normal workspace dependency edges and optionality | `docs/architecture.md`; `scripts/check_crate_diagram.ps1` |

No external implementation source, tests, or distinctive prose were used.

## Design

- Affected crates/layers: repository governance/tooling and compiled examples
  in `dynamics-core`, `dynamics-two-bodies`, and `orbit-determination`.
- Public API: unchanged.
- Rejected alternatives: prose-only snippets were rejected because all-target
  checks would not detect API drift; editor-wide formatting preferences were
  rejected in favor of toolchain discovery and recommendations.
- ADR required: no; these files expose existing contracts and reversible
  development workflow, without changing a durable domain boundary.

## Validation

- Unit cases: no production behavior changed.
- Invariants/properties: the custom example checks its deterministic endpoint.
- Independent reference vectors: none; examples explicitly make no validation
  claim.
- Differential/scenario tests: examples compile under Cargo all-target checks
  and the two tutorials can be run directly.
- Tolerances and justification: tutorials report existing solver settings and
  distinguish numerical tolerance from model error.
- Benchmarks: no performance claim; the task runner compiles maintained
  benchmark targets without setting timing thresholds.

## Completion checklist

- [x] Implementation and typed errors
- [x] Scientific and regression tests (compiled examples; no production change)
- [x] Rustdoc/examples
- [x] Provenance recorded locally in the tutorial and this task brief
- [x] Parity ledger reviewed; no status change is warranted
- [x] Relevant checks pass
- [x] Binding impact explicitly deferred; no binding files changed
