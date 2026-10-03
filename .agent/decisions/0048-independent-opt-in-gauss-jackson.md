# ADR-0048: independent opt-in eighth-order Gauss-Jackson

- Status: Accepted for the limited experimental Rust surface
- Date: 2026-10-03
- Issue: #16; supersedes ADR-0041's preference for an external adapter for this capability only
- Affected parity row: numerical integration and dense ephemerides

## Context and decision

The coordinating maintenance session reports that #19 completed its reusable
external-solver gate: zero mature-path allocation passed, but equivalent-work
runtime regressions were +129.9% for two-body, +157.5% for velocity-dependent
drag, and +306.4% for dense queries. Those results fail the less-than-10%
promotion condition. They are supplied decision context, not measurements
reproduced by this change. Do not adopt the adapter or add its dependency.
Bogacki-Shampine remains unchanged, including its dense/events/variational API.

Implement an independent, fixed eighth-order summed Gauss-Jackson method
from Berry and Healy (2004), equations 38 and 52. Generate coefficients from
the backward-difference generating functions, not a borrowed implementation
or supplemental coefficient program. Use nine acceleration ordinates; native
adaptive propagation supplies the first eight intervals. Integrate sums with
compensated additions and jointly iterate position and velocity for
velocity-dependent acceleration. Preserve typed units and the complete
inertial frame/origin; dynamics validates the physical problem.

Expose only `Propagator<CartesianState>` behind `gauss-jackson` in the numerical
crate and both facades. Configuration fixes the step, startup tolerances,
corrector tolerances, iteration and total step limits. Each call starts fresh.
No order switching, step switching, public workspace, external solver, dense
output, terminal partial step, event/maneuver/STM/covariance extension is implied.
Off-grid targets fail using exact duration nanoseconds. Discontinuities need a
fresh call on each smooth interval, whose targets still must satisfy the grid.
Corrector nonconvergence is an error, never an accepted predictor fallback.

## Alternatives and restrictions

An unsummed Stormer-Cowell recurrence would not supply the requested summed
Gauss-Jackson method. Copying an astrodynamics library or the paper's
supplemental program is prohibited. Arbitrary endpoint interpolation would
hide a different accuracy contract, so dense output is deliberately absent.
The selected native starter is available and error-controlled; its lower order
requires much tighter tolerances than the desired long-arc budget. No
mid-corrector smoothing is needed when that starter resolves the history;
startup and floating-point errors can still dominate small-step convergence.

The method is not A-stable and has no mature truncation-error estimator. Fixed
point convergence is necessary, not proof of stability or accuracy. Smooth
non-stiff operational models need step-refinement evidence, especially near
periapsis. The 30 s near-circular and 15 s eccentric reduced-model steps are
tested examples, not universal recommendations.

## Evidence and known gaps

Tests cover polynomial motion, signed/zero/short arcs, typed failures and error
sources, a manufactured velocity-dependent exponential, observed order,
three-day analytical two-body phase error and invariants, and independent
Orekit 13.1.6 endpoints for reduced ISS/CRRES-like scenarios.
An independent Hipparchus 4.0.3 DOP853 endpoint also checks a three-day
point-mass plus smooth velocity-dependent linear-drag orbit.
The [evidence record](../references/gauss-jackson/README.md) gives provenance,
budgets, commands, benchmark protocol and limitations.

Real ISS/CRRES perturbed force histories, dense output, persistent history across
calls, allocation-profiler measurements, mass/attitude/bindings and external
adapter promotion remain outside this slice. No complete operational validation
or semi-analytical parity claim is made.
