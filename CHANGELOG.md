# Changelog

All notable changes to orskit's Rust crates will be documented in this file.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and versioning follows the project's [pre-1.0 release policy](docs/release-policy.md).

## [Unreleased]

### Added

- Caller-selected physical ephemeris contracts and verified-artifact-backed
  cubic-Hermite interpolation with explicit target, observer, frame, epoch,
  coverage, and interpolation failures.
- Opt-in strict Two-Line Element parsing and canonical formatting with fixed
  69-column validation, standard checksums, Alpha-5 identifiers, typed errors,
  and exact semantic round-trip evidence.
- A stateless, non-configurable SGP4 implementation of the common propagator
  trait, advancing model-specific mean elements to typed Cartesian TEME states
  independently of the optional TLE conversion boundary.
- Bounded blocking OEM 3.0 XML streaming and collection over the existing
  typed OEM semantic model, with strict namespace, structure, unit, chronology,
  and resource-limit validation.
- Optional immutable cubic-Hermite dense output and bounded event localization
  for adaptive Cartesian propagation, including physical-time direction and
  deterministic simultaneous-event handling.

- Contributor and maintainer guidance for scientific, provenance, and
  approachable-issue work.
- Reproducible developer shortcuts, editor recommendations, and a pinned
  development container.
- Maintained crate architecture documentation and current propagation and
  orbit-determination tutorials.
- Opt-in, versioned Serde snapshots for current orbit representations and
  analytical Kepler propagator configuration, with separately gated JSON
  encoding/decoding, explicit provider registrations, and validated import
  through the live domain constructors.
- IERS/BIPM-backed leap-boundary validation, focused public API doctests, a
  maintained feature matrix, and cost-conscious Linux/macOS/Windows CI.
- Explicit scientific-data artifact identity, SHA-256 verification, time
  coverage, and bounded caller-selected offline loading.
- Scheduled accuracy-first benchmark evidence with retained raw metadata and no
  noisy cross-machine timing threshold.
- A provenance-cleared OEM KVN conformance corpus and isolated, bounded fuzz
  harness with a regression-preservation workflow.
- Caller-selected reference ellipsoids, WGS 84 geodetic/geocentric conversion,
  and catalog-issued local East–North–Up frames with explicit singularities.
- A concept-level Orekit 13.1.7 migration guide that distinguishes supported
  Rust workflows from capability gaps.

### Changed

- Unified new conversion and transform APIs around existing standard/domain
  traits: TLE and snapshot export use `TryFrom`, Cartesian state values bridge
  losslessly through `FrameKinematics`, ephemeris states expose that boundary
  without discarding query context, and topocentric frames implement the common
  kinematic transform provider.
- Clarified the README's current OEM covariance support.
- Hardened numerical propagation against extreme-duration arithmetic and
  non-finite error-normalization scales.

There are no published releases yet.

[Unreleased]: https://github.com/Wouittone/orskit/commits/main
