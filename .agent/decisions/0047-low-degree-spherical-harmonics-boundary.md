# ADR-0047: caller-supplied low-degree spherical-harmonic gravity boundary

- Status: Proposed
- Date: 2026-09-20
- Owners: propagation/dynamics maintainers

## Decision

Add `crates/dynamics/spherical-harmonics` as an isolated first vertical slice.
The crate accepts a caller-owned coefficient provider and a caller-owned
`frames::BodyFixedTransformProvider`; it never downloads gravity data or
selects Earth-orientation data. Coefficients are explicitly labeled as
fully-normalized geodesy 4π values and carry an explicit tide-system label.

The numerical implementation is intentionally restricted to zonal degree/order
`(2,0)` and optional `(3,0)`, with `C̄00 = 1`. It evaluates the central,
J2, and J3 terms in body-fixed Cartesian coordinates and rotates acceleration
back to the configured inertial frame. Nonzero tesseral/sectorial coefficients,
unsupported degree/order coverage, non-finite values, frame-origin mismatch,
and zero-radius evaluation are rejected.

## Exact limitation

This is not a general spherical-harmonic evaluator: no tesseral or sectorial
terms, latitude recurrences, tide corrections, time-variable coefficients,
coefficient-file parsing, or built-in Earth data are included. The facade
should expose the provider/data contract and require applications to document
their coefficient provenance and tide-system interpretation.

## Validation

The crate includes an independently calculated J2/J3 reference vector,
zonal parity invariants, point-mass reduction, and unsupported-tesseral
validation. Parent integration must add the crate to the workspace and facade
only after reviewing the dependency boundary.
