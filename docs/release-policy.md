# Pre-1.0 Rust release policy

orskit is pre-alpha and has no published release. This policy governs future
Rust crate releases until the maintainers explicitly declare a 1.0 stability
contract. It does not reactivate or version the parked Python and JVM bindings.

## Versioning

The workspace follows Semantic Versioning with the additional expectations
that apply before 1.0:

- `0.MINOR.0` may contain breaking public-API, behavior, feature, data, or MSRV
  changes. Release notes must identify each break and its migration path.
- `0.MINOR.PATCH` is reserved for backward-compatible fixes, documentation,
  validation evidence, and additive changes that do not invalidate an existing
  supported workflow.
- The current `0.0.0` version means unreleased scaffolding. The first public
  release will choose an intentional `0.MINOR.0` version; it will not imply
  scientific validation or Orekit parity.

Where practical, deprecate an API for at least one minor release before
removing it. Maintainers may make an immediate breaking correction when the old
contract is physically ambiguous, unsound, or likely to produce a wrong
scientific result; the release notes must explain why compatibility was unsafe.

All publishable Rust crates currently share one workspace version. Release them
in lockstep unless an accepted ADR establishes independent versioning and a
dependency-compatible publishing order. Feature flags are part of the public
Rust compatibility surface: removing a feature or changing its meaning is a
breaking change.

## Changelog and release notes

Every user-visible pull request adds an entry under `CHANGELOG.md`'s
`Unreleased` section in `Added`, `Changed`, `Deprecated`, `Removed`, `Fixed`, or
`Security`, unless the pull request itself is release bookkeeping. At release,
move those entries under a dated version heading and add comparison links.

Release notes summarize outcomes rather than commit titles and include:

- supported end-to-end workflows and explicit known gaps;
- breaking API, feature, behavior, data, or MSRV changes with migration advice;
- the Rust toolchain and platform validation actually run;
- scientific models or formats added, their supported regimes, provenance,
  validation evidence, tolerances, and unresolved limitations;
- material dependency, license, advisory, or security changes; and
- benchmark methodology and accuracy alongside any performance claim.

Do not describe a capability as validated merely because it compiles, has an
API, or appears in a release. Parity claims must cite the evidence required by
`.agent/PARITY.md` for the pinned baseline.

## Release authority

Releases are explicit maintainer operations. CI may build and dry-run packages,
but it must not publish to crates.io or create a final release without a
maintainer selecting the version and approving the evidence. Before publishing,
maintainers verify the locked workspace checks, documentation, crate metadata,
dependency licenses/advisories, changelog, parity ledger, provenance ledger,
and package order. A release tag and crate artifacts must correspond to the
same reviewed commit.

Security-sensitive fixes may use a coordinated private process described in
`SECURITY.md`; publish only the detail appropriate after remediation.
