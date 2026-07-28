# Continuous-integration assurance policy

The repository separates ordinary cross-platform build checks from assurance
jobs whose failure and permission boundaries need explicit interpretation.
`.github/workflows/build.yml` owns the Linux, macOS, and Windows build/test
matrix. `.github/workflows/assurance.yml` owns MSRV, dependency policy,
coverage, and documentation publication.

## Required checks and failure handling

All pull-request assurance jobs are secret-free and read-only. A reproducible
MSRV, dependency-policy, coverage-generation, or documentation-build failure is
a merge blocker. A hosted-runner, action-download, registry, advisory-database,
or GitHub Pages outage should first be retried. It is not a scientific or
software failure, but an unsuccessful job is never described as a pass.

Only maintainers may bypass a required check during a confirmed external
service incident. The pull request must name the failed service, link the
incident or job, record equivalent local evidence where possible, and open a
follow-up item to restore the check. Checks are not weakened merely to make an
incident green.

## Minimum supported Rust version

The workspace `rust-version` is the public MSRV contract. The explicit MSRV job
uses Rust 1.96.1 and checks every workspace target with all features and the
committed lockfile. A future MSRV change updates the workspace manifest,
toolchain pin, assurance workflow, release notes, and compatibility evidence in
one reviewed change.

The ordinary build matrix may use the same pinned version; the separate job is
retained so branch protection and release evidence can identify MSRV failures
without inferring them from another job.

## Dependency licenses, advisories, and sources

`cargo-deny` evaluates all normal and development dependencies:

- licenses must match the allow-list in `deny.toml`;
- known security vulnerabilities fail the advisory check;
- yanked releases fail the check;
- unmaintained or unsound direct workspace dependencies fail the check; and
- registry dependencies must come from crates.io, while new Git sources require
  an explicit policy change.

An advisory or license exception must identify the exact advisory or crate,
explain why the affected code is unreachable or the terms are acceptable,
identify an owner and removal condition, and receive maintainer review.
Security-sensitive details follow `SECURITY.md`. Network failure while updating
the RustSec database is an infrastructure failure, not evidence that no
advisory exists.

## Coverage

The coverage job runs the workspace test targets with all features under
`cargo-llvm-cov`, produces LCOV, and retains the report as a GitHub artifact for
14 days. Report generation and the instrumented tests are required to succeed.

Coverage is a review aid, not a scientific validation score. No percentage
threshold is enforced until maintainers establish a stable baseline and
document exclusions. A coverage change cannot establish or invalidate a
capability claim without the evidence required by `.agent/PARITY.md`.

Coverage stays in GitHub artifacts and uses no third-party upload token. Adding
an external coverage service requires a separate policy change covering token
scope, fork behavior, retention, service outages, and whether upload failure is
blocking.

## Documentation publication

Every pull request builds rustdoc with warnings denied. A push to `main` also
uploads the same generated workspace documentation and deploys it through the
protected `github-pages` environment. Repository administrators must select
GitHub Actions as the Pages source and may require approval on that environment.
A failed deployment leaves the previously published site intact and does not
turn a successful documentation build into a scientific validation claim.

Documentation publication uses only the job-scoped `GITHUB_TOKEN` permissions
`pages: write` and `id-token: write`; other jobs retain `contents: read`.
Pull-request code is never executed in a privileged Pages job.

## Secrets

No current CI job requires a repository secret. Secrets must never be exposed
to pull requests from forks or to a `pull_request_target` workflow that checks
out untrusted code. If a future service requires credentials, maintainers must
use a narrowly scoped environment secret, document rotation and revocation,
and keep the untrusted build/test job separate from the privileged upload or
deployment job.

## Local equivalents

The ordinary workspace commands remain documented in `CONTRIBUTING.md`.
Additional assurance checks are:

```powershell
cargo deny check advisories licenses sources --all-features --locked
cargo llvm-cov --workspace --all-targets --all-features --locked --lcov --output-path lcov.info
$env:RUSTDOCFLAGS = "-D warnings"
cargo doc --workspace --all-features --no-deps --locked
```

The hosted Pages deployment and its permission configuration cannot be
validated locally.
