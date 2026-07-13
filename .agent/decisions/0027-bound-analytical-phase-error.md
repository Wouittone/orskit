# ADR-0027: bound analytical propagation phase error

- Status: Accepted
- Date: 2026-07-13
- Affected parity rows: two-body propagation

## Context

The analytical propagator rounded a Hifitime duration through `f64` seconds and
accepted arbitrary finite spans without an accuracy contract. Equinoctial
states passed through a singular Keplerian intermediate, so valid large finite
`hx/hy` values could fail, even at zero duration.

## Decision

1. Read signed exact total nanoseconds and split whole/subsecond components
   before floating-point phase evaluation.
2. Estimate a conservative IEEE-754 phase/reduction error from absolute span;
   positive and negative durations share the same bound.
3. Expose a configurable unit-named angular budget and return typed
   `AccuracyBudgetExceeded` rather than silently exceed it.
4. Zero duration validates the problem relationship and returns the original
   orbit exactly.
5. Advance equinoctial mean longitude directly, preserving `a, ex, ey, hx, hy`
   without conversion through Keplerian inclination.

## Consequences

- Very long spans such as `10^12 s` are rejected under the default budget.
- Callers may deliberately select another documented budget.
- This remains an elliptic analytical method, not a promise of arbitrary-span
  or arbitrary-conic propagation.

## Validation

Tests cover nanosecond sensitivity, symmetric deterministic rejection, exact
zero identity, `hx = 1e16`, representation preservation, reverse propagation,
and unchanged Orekit/Lox 3,600-second reference vectors.

## Provenance

Equations continue to use the public NASA GMAT reference already recorded by
the project. Error-bound and API design are original orskit work; no external
implementation or tests were consulted.
