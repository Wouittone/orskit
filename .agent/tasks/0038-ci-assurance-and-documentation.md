# Task: establish CI assurance and documentation publication

## Parity target

- Ledger row: all rows (repository assurance only)
- Current status: mixed
- Intended status after this task: unchanged; CI evidence does not establish a
  scientific capability

## User workflow

A contributor receives independently named CI results for the documented MSRV,
dependency licenses and advisories, instrumented coverage, and warning-free
documentation. Reviewed `main` documentation is published through GitHub Pages
without exposing secrets to pull-request code.

## Scientific contract

- Inputs and units: not applicable
- Outputs and units: CI results, LCOV artifact, and generated rustdoc
- Frames/epochs/time scales: not applicable
- Conventions and valid regimes: the committed lockfile and all workspace
  features define the checked dependency/build graph
- External data requirements: crates.io metadata and the RustSec advisory
  database are fetched by the dependency-policy job; no scientific data
- Errors and singularities: deterministic check failures block merging;
  hosted-service incidents follow `docs/ci-policy.md`

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| GitHub Pages custom-workflow documentation | GitHub service documentation | Pages artifact, deployment permission, and environment requirements | `.github/workflows/assurance.yml`; `docs/ci-policy.md` |
| cargo-deny documentation and action | MIT OR Apache-2.0 tool/action documentation | License, advisory, and source checks | `deny.toml`; `.github/workflows/assurance.yml` |
| cargo-llvm-cov documentation | Apache-2.0 OR MIT tool documentation | LLVM coverage invocation and LCOV output | `.github/workflows/assurance.yml` |

No scientific source, implementation, or reference vector is used.

## Design

- Affected crates/layers: repository CI and contributor policy only
- Public API: unchanged
- Rejected alternatives: inferring MSRV from the ordinary matrix; publishing
  from pull requests; granting workflow-wide Pages permissions; requiring an
  external coverage token; treating coverage percentage as validation
- ADR required: no; this applies existing governance and release policy

## Validation

- Unit cases: not applicable
- Invariants/properties: pull-request jobs have read-only permissions and no
  secrets; only the main-branch Pages job receives publication permissions
- Independent reference vectors: not applicable
- Differential/scenario tests: local Cargo metadata, docs, and configuration
  validation where tools are available; first hosted jobs remain CI evidence
- Tolerances and justification: not applicable
- Benchmarks: not applicable

## Completion checklist

- [x] Explicit MSRV job
- [x] Dependency license/advisory/source policy
- [x] Coverage generation and retained artifact
- [x] Warning-free documentation build and main-only Pages publication
- [x] Failure, exception, permission, and secret policies documented
- [x] Roadmap evidence updated
- [x] Relevant local validation recorded; first hosted execution remains pending
- [x] Binding impact explicitly deferred; no binding files changed
