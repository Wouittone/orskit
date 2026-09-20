# ADR-0041: keep differential-equations-rs out of the default propagation path

- Status: Accepted
- Date: 2026-09-20
- Owners: propagation/dynamics maintainers
- Affected parity rows: Propagation / numerical integration and dense
  ephemerides; Propagation / two-body/Keplerian propagation

## Context

`differential-equations-rs` (same author, MIT OR Apache-2.0, workspace at
`Wouittone/differential-equations-rs`) is a general-purpose Rust ODE toolkit
offering explicit/implicit Runge-Kutta families (Verner, Tsit5, high-order
pairs), extrapolation methods, Rosenbrock/implicit solvers for stiff systems,
and a dedicated Gauss-Jackson multistep predictor-corrector. It represents
state as `Vec<f64>`/`ndarray` with a SciML-style `f(du, u, p, t)` calling
convention and a builder-driven `OdeProblem`.

orskit's own `dynamics-numerical` crate implements a hand-rolled, typed,
frame/epoch-qualified `BogackiShampine32` propagator over `CartesianDynamics`.
It is order 3(2), with cubic Hermite dense output, event bisection, and a
42-component variational (state + STM) extension. It does not offer
Gauss-Jackson, high-order embedded pairs (order 7+), extrapolation, or
implicit/stiff solvers.

A companion evaluation (`Wouittone/differential-equations-rs` issue #40,
"External solver evaluation: SatKit replacement regresses runtime") swapped a
native fixed-size-array RKV98 kernel in the sibling SatKit project for a
`differential-equations-rs`-backed adapter. For a representative six-hour LEO
case, the swap regressed wall-clock runtime by about 37.6% (1.423 ms native
versus 1.959 ms adapted) while preserving trajectories within about 1.3 um
position / 1.4 nm/s velocity. Both runs took the same accepted/rejected step
counts and RHS evaluation counts, so the regression is adapter/allocation
overhead (heap-backed `ndarray`/`Vec<f64>` state and per-step conversions),
not algorithmic cost. A separate diagnostic in that repository (PR #50) found
controller/step-size arithmetic itself is cheap (about 2.6% of inclusive
stage/controller time), reinforcing that the cost sits in state
representation and adapter plumbing, not the numerical method.

orskit's engineering standard forbids hidden allocation in hot loops and
requires that public APIs not leak dependency-specific types unless the
dependency is an intentional foundational contract (Hifitime, `uom`). A
per-step-allocating external adapter would violate both constraints if wired
into the default Cartesian propagation call path.

## Decision

Do not add `differential-equations-rs` as a workspace dependency of any
shipped orskit crate yet, and do not replace `BogackiShampine32` as the
default numerical propagator. Instead:

1. Keep it available only as an external, unlinked validation/reference
   harness (mirroring the existing Nyx validation boundary in
   `PROVENANCE.md`): run it out-of-tree to generate independent
   high-order/extrapolation reference trajectories for accuracy
   cross-checks, never as a linked dependency of a distributed crate.
2. Track, as a separate, explicitly gated follow-up (GitHub Issue #16), an opt-in
   adapter crate for capabilities orskit cannot cheaply reproduce in-house:
   Gauss-Jackson (a classic operational astrodynamics propagator family) and
   very-high-order/extrapolation methods for deep-space or long-arc accuracy
   scenarios. That adapter must not become the default propagator.
3. Gate any future adoption of that adapter on a reusable-workspace,
   allocation-free (or amortized-allocation) integration proven by a
   benchmark using the same methodology as the SatKit evaluation (fixed
   representative scenario, wall-clock plus accuracy comparison, accepted
   here as a reused reference on a `differential-equations-rs`-hosted
   scenario since it does not depend on orskit implementation), with an
   explicit performance budget (regression less than 10% versus the native
   baseline it would sit beside) before promotion beyond an explicitly
   opt-in feature.
4. Never expose `differential-equations-rs` types (`OdeProblem`, `ndarray`
   arrays, its error types) in any orskit public API; any adapter converts to
   and from orskit's typed `Orbit<CartesianState>` boundary only.

## Alternatives considered

- **Replace `BogackiShampine32` outright.** Rejected: the SatKit evaluation
  is direct evidence of a ~37.6% regression for a representative LEO
  workload with no offsetting accuracy or capability gain for that solver
  order; this would be an unjustified performance regression on the hot
  propagation path.
- **Reimplement Gauss-Jackson and high-order RK/extrapolation from scratch.**
  Rejected for now: this duplicates substantial, already-reviewed numerical
  work in a sibling project the same author maintains, and the FORCE_MODELS
  and PARITY backlog is already large; an adapter is preferred once its
  overhead is bounded.
- **Ignore the crate entirely.** Rejected: its Gauss-Jackson and
  extrapolation families cover real capability gaps (Milestone 3's
  "selected analytical and semi-analytical families" and higher-accuracy
  deep-space scenarios) that are otherwise unplanned work.

## Consequences

- No immediate dependency, licensing, or Cargo.lock churn in orskit.
- The path to Gauss-Jackson and higher-order solvers is recorded but blocked
  on a concrete performance gate rather than left implicit.
- Validation work can already use the external crate out-of-tree without
  waiting on the adapter.
- GitHub Issue #16 must be revisited if `differential-equations-rs`
  publishes a lower-overhead state representation (for example a
  fixed-size/stack-backed state option) that could close the measured gap.

## Validation

No code changes accompany this decision. The cited SatKit evaluation
(issue #40) and controller-arithmetic diagnostic (PR #50) are the accepted
evidence. Any future adapter must add its own benchmark reusing that
methodology before this ADR's performance gate is considered satisfied.

## Provenance

- `Wouittone/differential-equations-rs` issue #40 ("External solver
  evaluation: SatKit replacement regresses runtime") and PR #50 ("perf:
  profile controller arithmetic against RKV98"): same-author, MIT OR
  Apache-2.0 sibling project; public issue/PR findings used for the
  performance facts above only, no source code copied.
- `.agent/PROVENANCE.md` Nyx validation-boundary precedent for keeping an
  external comparison harness out of the distributed workspace.
