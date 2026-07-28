# ADR-0039: verify caller-selected scientific-data artifacts

- Status: Accepted
- Date: 2026-07-24
- Affected parity rows: explicit scientific data context and providers; time;
  frames and Earth orientation; ephemerides and geodesy

## Context

Provider contracts already make scientific data application-owned, but the
shared artifact record only carries optional free-form checksum text and has no
coverage contract. Format-specific providers would otherwise repeat identity,
digest, allocation, and offline-loading policy or quietly accept unverifiable
inputs.

## Decision

1. `orskit-data` owns the small, format-neutral artifact boundary:
   non-blank authority/product/version identity, a required SHA-256 content
   digest, and explicit all-time or inclusive epoch coverage.
2. `VerifiedArtifact` owns immutable bytes only after their digest matches the
   selected descriptor. Its local-file loader requires a non-zero caller
   allocation bound and performs no network or cache lookup.
3. Coverage is checked explicitly by providers at evaluation time. Loading an
   artifact does not imply that it covers every requested epoch.
4. Provider crates may re-export this vocabulary, but must not create weaker
   string-only artifact identities or ambient process-global data contexts.
5. Fetching and cache population remain separate, explicit application tooling.
   This foundational crate neither downloads nor silently replaces data.

## Consequences

- Exact bytes, identity, applicability, and allocation policy are inspectable.
- SHA-256 adds one focused MIT/Apache-2.0 RustCrypto dependency.
- Coverage remains one inclusive interval in the first slice. Disjoint
  intervals and format-specific spatial/model applicability belong to later
  provider contracts.
- This contract enables, but does not implement or validate, leap-second,
  Earth-orientation, ephemeris, gravity, atmosphere, or space-weather models.

## Validation

Tests cover the standard SHA-256 `abc` result, strict digest parsing, identity
validation, inclusive/reversed coverage, checksum mismatch, bounded local
loading, and source-preserving I/O errors. Full workspace feature and
documentation checks cover integration.

## Provenance

NIST FIPS 180-4 defines SHA-256 and message digests for change detection. The
RustCrypto `sha2` crate supplies the unmodified implementation under MIT OR
Apache-2.0. No third-party scientific data, parser, provider implementation, or
cache design was copied.
