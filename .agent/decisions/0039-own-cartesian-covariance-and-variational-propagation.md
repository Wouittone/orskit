# ADR-0039: own Cartesian covariance in orbits and integrate variational state

- Status: Accepted
- Date: 2026-07-30
- Owners: orskit maintainers
- Affected parity rows: variational equations, STM, and covariance propagation;
  Cartesian covariance mapping; sequential orbit determination

## Context

P17 requires a state transition matrix and covariance propagation. The only
Cartesian covariance type previously lived inside `orbit-determination`, which
would create a dependency cycle if numerical dynamics consumed it. It also
accepted only diagonal standard deviations publicly, while covariance
propagation necessarily creates correlated position/velocity blocks.

The numerical kernel currently advances six Cartesian components with typed
state tolerances. A correct variational slice must retain the dimensions of
the four STM blocks and must control their numerical error rather than expose
an unqualified `6 x 6` matrix.

## Decision

1. `orbits::cartesian::CartesianCovariance` is the shared domain covariance.
   It owns a frame plus unit-qualified position/position,
   position/velocity, and velocity/velocity blocks. It validates finite,
   symmetric, strictly positive-definite input. Orbit determination re-exports
   this same type and converts it to private numerical matrices internally.
2. `CartesianVariationalDynamics` extends `CartesianDynamics` with an
   acceleration Jacobian. Position partials use reciprocal square seconds and
   velocity partials use reciprocal seconds. The first implementation is the
   analytic point-mass Jacobian.
3. `BogackiShampine32::propagate_with_state_transition` advances the six state
   components and 36 STM components as one 42-component augmented system:
   `d Phi / dt = A Phi`, with `Phi(t0) = I`. The same embedded 3(2) stages
   advance both groups.
4. Local error acceptance includes every augmented component.
   `VariationalConfiguration` supplies typed absolute tolerances for the
   dimensionless, seconds, and reciprocal-seconds STM blocks; the ordinary
   relative tolerance applies to all groups.
5. `CartesianStateTransition` exposes four typed `3 x 3` blocks, one frame,
   and initial/final epochs. No public raw numerical matrix or `nalgebra` type
   is introduced.
6. Covariance propagation applies `P(t) = Phi P0 Phi^T`, symmetrizes
   floating-point roundoff, and revalidates the typed result. It adds no
   process noise.
7. The first slice excludes maneuver discontinuities, maneuver execution
   covariance, model parameters, mass/attitude sensitivities, process-noise
   integration, square-root covariance propagation, and dense STM output.

## Alternatives considered

- Keep covariance inside orbit determination: rejected because propagation
  would depend upward on estimation or introduce a duplicate covariance type.
- Expose `nalgebra::SMatrix<f64, 6, 6>`: rejected because entries have four
  distinct dimensions and numerical storage is not the domain API.
- Estimate every STM only by repeated finite-difference propagation: rejected
  because it is expensive, perturbation-dependent, and does not implement the
  requested variational equations. Central differences remain validation.
- Control steps only with the six-state error: rejected because an accepted
  state step could still carry an uncontrolled STM error.
- Add process noise implicitly: rejected because its physical model, units,
  frame, and time correlation must be selected explicitly by the caller.

## Consequences

Cartesian covariance becomes reusable across orbit, propagation, and
estimation workflows. Numerical propagation performs more derivative work and
stores a 42-component stage state when variational output is requested, while
ordinary propagation remains unchanged. Dynamics implementations must opt in
by providing valid first partial derivatives.

## Validation

Inertial motion has the closed-form block STM `[I, dt I; 0, I]` and an analytic
covariance map. Point-mass variational output is compared column-by-column
against independently perturbed central propagations. Reverse propagation,
configuration failures, frame mismatch, covariance symmetry, and positive
definiteness are tested. The example maps a 300-second Earth-orbit covariance.

## Provenance

Paul J. Huxel and Robert H. Bishop, *Navigation Algorithms for Formation
Flying Missions*, Proceedings of the 2nd International Symposium on Formation
Flying Missions and Technologies, 2004, NASA NTRS document 20060048534,
Public Use Permitted, supplies `d Phi / dt = A Phi`, `Phi(t0)=I`, and
`P = Phi P0 Phi^T + Q`. This slice uses the zero-process-noise case. No source
code, tests, implementation structure, or distinctive prose was copied.
