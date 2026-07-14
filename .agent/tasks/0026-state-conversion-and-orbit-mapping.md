# Task: complete state conversion traits and epoch-preserving orbit mapping

## Parity target

- Ledger rows: Orbits / epoch-frame-qualified Cartesian states; Orbits /
  Keplerian, circular, equinoctial, and nonsingular elements; Propagation /
  Two-body/Keplerian propagation.
- Current status: Partial.
- Intended status after this task: Partial, with Cartesian, elliptic circular,
  elliptic Keplerian, and elliptic equinoctial representations available
  through standard Rust conversions.

## User workflow

An application constructs an `Orbit<S>`, uses `TryFrom`/`TryInto` to select a
different orbit representation while retaining the same Hifitime epoch, and
provides an explicit shared gravity provider with Cartesian inputs. Every
`Propagator<Problem, S>::propagate` first uses `PropagationState<Problem>` to
resolve `S` into the implementation's target state, calls
`propagate_resolved` on the common `Propagator` trait, and restores `S`; the
elliptic two-body propagator thus preserves Cartesian, circular, Keplerian, or
equinoctial output representation. Its one analytical resolved state is
Cartesian, advanced through universal variables and Lagrange `f`/`g`
coefficients; Cartesian is regular except at physical zero-radius collision.

## Scientific contract

- Inputs and units: the existing typed SI-backed element and Cartesian values;
  Cartesian/element conversions accept a caller-selected shared gravity
  provider.
- Outputs and units: converted typed coordinates; mapping changes only the
  representation and preserves the orbit epoch.
- Frames/epochs/time scales: existing conversion frame and gravity-origin
  validation remains in force; `Orbit` preserves its Hifitime epoch exactly.
- Conventions and valid regimes: elliptic circular elements are `(a, ex, ey,
  i, Omega, alpha_v)`, where `ex=e cos(omega)`, `ey=e sin(omega)`, and
  `alpha_v=nu+omega`; existing elliptic Keplerian and equinoctial conventions
  remain unchanged.
- External data requirements: gravity is explicit for conversions touching
  Cartesian coordinates; element-to-element conversions require no new data.
- Errors and singularities: conversion `StateError` values remain sources of
  typed propagation errors; the universal Cartesian kernel rejects only
  non-elliptic energy and physical zero-radius collision. Element restoration
  retains the caller representation's declared chart singularities.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| Orekit 13.1.7 `org.orekit.orbits` package and `CircularOrbit` public API | Public behavior and capability documentation | Four primary representations and circular-element terminology only | `crates/orbits`, `.agent/PROVENANCE.md` |
| NASA/TM-2004-213230, *Orbit Propagation* | Public NASA technical memorandum | Universal functions plus Lagrange `f`/`g` propagation equations only | `crates/dynamics-two-bodies`, `.agent/PROVENANCE.md` |
| Existing orbit-state references in task 0004 | Project design and recorded scientific sources | Explicit gravity context, elliptic conventions, and singularity policy | `crates/orbits`, `crates/core`, `crates/dynamics-two-bodies` |

No external implementation, source, or test material is used by this API
completion task.

## Design

- Affected crates/layers: core orbit contract, orbit implementations, and the
  two-body analytical implementation.
- Public API: standard `TryFrom`/`TryInto` conversions; `Orbit::map_state` and
`Orbit::try_map_state`; `CircularState`; generic
  `PropagationState<Problem>` and `Propagator::propagate_resolved`, with
  problem-context restoration for element states.
- Rejected alternatives: implicit global gravity, a closed state enum, or a
  core dependency on concrete orbit implementations.
- ADR required: ADR-0033 records the reusable resolved-state propagation
  boundary.

## Validation

- Unit cases: every supported directed representation conversion, circular
  element construction, and exact epoch preservation through successful and
  fallible orbit mapping.
- Invariants/properties: converted Cartesian endpoints agree with existing
  conversion paths; mapping never changes the epoch.
- Independent reference vectors: existing Orekit and Lox two-body vectors.
- Differential/scenario tests: the shared dynamics contract proves resolution
  and restoration; an application-defined state resolves to Cartesian through
  the generic propagator; Cartesian and circular two-body propagation use
  standard conversions and preserve their representations; exact retrograde
  Cartesian propagation is finite.
- Tolerances and justification: the existing anomaly tolerance is scaled by
  `sqrt(a)` for universal-anomaly Newton updates; existing phase-error budgets
  remain in radians.
- Benchmarks: not required; no performance claim is made for the new numerical
  kernel.

## Completion checklist

- [x] Implementation and typed errors
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance recorded
- [x] Parity ledger updated
- [x] Relevant checks pass
- [x] Binding impact handled or explicitly deferred: bindings remain deferred
  while the Rust API is partial
