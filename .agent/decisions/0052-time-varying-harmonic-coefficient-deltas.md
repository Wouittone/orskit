# ADR-0052: compose epoch-dependent spherical-harmonic coefficient deltas

- Status: Accepted
- Date: 2026-10-03
- Owners: propagation/dynamics maintainers
- Affected parity row: Propagation / gravity fields and solid/ocean tides

## Context

Issue #31 needs time-variable normalized coefficients without replacing or
mutating a caller's static gravity field. Delta conventions must match the
static coefficient set, and requests must be bounded by explicit provider
coverage. The force model is evaluated repeatedly by propagators, so the
composition must not alter the static model's default behavior or introduce an
extra coefficient-table allocation on every evaluation.

## Decision

1. Keep `SphericalHarmonicGravityModel` and its construction/evaluation API
   unchanged. Add `with_coefficient_deltas` that consumes a configured static
   model and returns a separate `TimeVaryingSphericalHarmonicGravityModel`.
2. Make deltas dimensionless fully normalized `ΔC̄ₙₘ` and `ΔS̄ₙₘ`. A provider
   declares its provenance, normalization, tide system, coefficient frame,
   maximum degree/order, and inclusive epoch coverage/time scale.
3. Reject inconsistent provider degree/order, reversed coverage, invalid
   provenance, convention/frame mismatch, and insufficient degree/order at
   construction. Reject out-of-coverage epochs, missing/non-finite deltas,
   non-zero central-term deltas, and non-finite sums at evaluation.
4. Add a linear secular-rate provider whose typed SI frequency values are
   multiplied by elapsed SI seconds from a declared reference epoch. Require
   that the reference epoch lies inside the declared closed coverage.
5. Evaluate each composed coefficient on demand within the existing Cartesian
   solid-harmonic recurrence. Preserve the static coefficient exactly when its
   delta is zero, including signed-zero bit patterns; do not allocate a
   time-varying coefficient table on each force evaluation.
6. Do not add tide formulas, gravity products, coefficient parsers, network
   access, or binding APIs. Deltas retain, and never convert, the static set's
   tide-system convention.

## Alternatives considered

- **Replace or mutate the static provider:** rejected because it obscures
  provenance and changes the stable static API.
- **Treat omitted deltas as zero:** rejected because missing coverage and a
  physically zero correction must remain distinguishable.
- **Precompute a coefficient table at each epoch:** rejected because arbitrary
  epoch-dependent providers can change between force evaluations and copying
  the full table adds hidden allocation to propagation.
- **Apply separate force terms for each coefficient delta:** rejected because
  coefficient composition must happen before the same potential-gradient
  evaluator and must preserve the selected tide convention.
- **Implement tides in this slice:** rejected because tide-specific physical
  models, qualified data, and independent reference vectors are not part of
  issue #31.

## Consequences

- Applications can layer caller-owned secular or other epoch-dependent
  coefficient changes over a selected static field with typed mismatch and
  coverage failures.
- Zero changes retain the static evaluator's bit-level output; non-zero changes
  use the existing potential and gradient algorithm with summed coefficients.
- The first linear provider is only a rate mechanism. It does not provide
  authoritative values or establish solid/ocean/pole tide parity.
- Python/JVM exposure remains deferred while the Rust core contracts stabilize.

## Validation

- Confirm zero-rate evaluations are bit-identical to the static model.
- Compare secularly composed acceleration bit-for-bit with a static model
  constructed from the same summed coefficients.
- Compare the composed result with an independent potential and
  finite-difference gradient.
- Test convention, frame, degree/order, provenance, coverage, missing,
  non-finite, central-term, and rate-construction failures.
- Run the Rust 1.96.1 checks listed in task 0052 and the opt-in facade check.

## Provenance

The coefficient normalization and time-variable gravity context follow
[IERS Conventions (2010), Chapter 6, §§6.1–6.3](https://iers-conventions.obspm.fr/content/chapter6/icc6.pdf).
The implementation independently evaluates the existing project recurrence
with caller-supplied dimensionless deltas; it copies no source code, data,
tests, or reference vectors. SI frequency units and Hifitime epochs use the
existing `units` and Hifitime contracts.
