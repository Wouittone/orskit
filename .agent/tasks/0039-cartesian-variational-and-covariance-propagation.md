# Task: implement Cartesian variational and covariance propagation

## Parity target

- Ledger rows: propagation / variational equations, STM, and covariance
  propagation; orbits / anomalies, Jacobians, interpolation, covariance mapping
- Current status: both Not assessed
- Intended status after this task: both Partial, with a typed Cartesian
  covariance domain object, analytic point-mass acceleration partials, an
  error-controlled 42-component state/STM integration, and zero-process-noise
  covariance mapping.

## User workflow

A caller supplies a frame-qualified Cartesian orbit and correlated typed
covariance, selects a dynamics model with acceleration partial derivatives,
propagates to a target epoch, and inspects the final orbit, four
dimension-qualified STM blocks, and mapped covariance.

## Scientific contract

- Inputs and units: Cartesian position/velocity and P11 integration settings;
  a same-frame `CartesianCovariance`; typed absolute STM tolerances using
  `Ratio`, `Time`, and `InverseTime`.
- Outputs and units: `CartesianStateTransition` contains dimensionless
  position/position and velocity/velocity blocks, a seconds-valued
  position/velocity block, and a reciprocal-seconds velocity/position block.
  `CovariancePropagation` returns the mapped unit-qualified covariance.
- Frames/epochs/time scales: the state, covariance, acceleration Jacobian, and
  STM share one Cartesian frame. The STM records both Hifitime epochs.
- Conventions and valid regimes: state order is
  `[x, y, z, vx, vy, vz]`; rows are final components and columns initial
  components. `d Phi/dt = A Phi`, `Phi(t0)=I`. The point-mass position
  Jacobian is `mu (3 r r^T / |r|^5 - I / |r|^3)` and its velocity block is
  zero. Covariance maps as `Phi P0 Phi^T` with no process noise.
- External data requirements: unchanged from the selected dynamics provider.
- Errors and singularities: ordinary numerical errors, non-finite
  acceleration partials, invalid STM tolerances, state/covariance frame
  mismatch, and invalid mapped covariance are typed. Point-mass collision
  remains singular.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| Paul J. Huxel and Robert H. Bishop, [*Navigation Algorithms for Formation Flying Missions*](https://ntrs.nasa.gov/citations/20060048534), Proceedings of the 2nd International Symposium on Formation Flying Missions and Technologies, 2004, NASA NTRS 20060048534 | Public US Government-sponsored conference paper; Public Use Permitted | `d Phi/dt = A Phi`, identity initial condition, covariance map `Phi P Phi^T + Q` | augmented variational kernel, zero-process-noise covariance mapping, analytic inertial validation |
| Existing point-mass equations and NASA GMAT reference recorded in task 0036 / project provenance | Previously approved equations and public technical documentation | Differentiate `a = -mu r / |r|^3` to obtain the analytic acceleration Jacobian | `TwoBodyDynamics::acceleration_jacobian`, central finite-difference validation |
| P11 Bogacki--Shampine primary references | Previously recorded primary numerical paper/textbook | Apply the same embedded 3(2) stages and component-scaled error controller to the augmented system | 42-component state/STM integration |

No external code, tests, implementation structure, or distinctive prose was
copied. The point-mass Jacobian and inertial covariance expectations were
independently derived and checked against central perturbations.

## Design

- Affected crates/layers: `units`; `orbits`; `dynamics-core`;
  `dynamics-two-bodies`; `dynamics-numerical`; `orbit-determination`;
  feature-gated facades and evidence.
- Public API: `InverseTime`, `InverseTimeSquared`,
  `CartesianCovariance`, `CartesianAccelerationJacobian`,
  `CartesianVariationalDynamics`, `VariationalConfiguration`,
  `CartesianStateTransition`, `VariationalPropagation`,
  `CovariancePropagation`, and typed errors.
- Rejected alternatives: covariance owned by estimation; public raw matrices;
  finite-difference-only production STM; state-only error acceptance; implicit
  process noise.
- ADR required: ADR-0039 records covariance ownership, typed block dimensions,
  augmented error control, and deferred process-noise/maneuver behavior.

## Validation

- Unit cases: covariance construction/asymmetry/indefiniteness, invalid
  variational tolerances, and covariance frame mismatch.
- Invariants/properties: zero-duration identity; forward/reverse inertial STM;
  covariance symmetry and positivity after mapping.
- Independent reference vectors: inertial motion supplies
  `[I, dt I; 0, I]` and closed-form position/velocity covariance blocks.
- Differential/scenario tests: every point-mass STM column is compared with a
  central finite difference of two independently propagated initial states; a
  point-mass example maps a 300-second covariance.
- Tolerances and justification: inertial results use roundoff-scale budgets.
  Point-mass perturbations are `1 m` and `1 mm/s`, large enough to dominate
  the configured propagation noise while remaining in the local linear
  regime; STM agreement is bounded to `2e-5` for position rows and `2e-8` for
  velocity rows.
- Benchmarks: no performance claim; augmented-stage cost should be profiled
  before workspace or parallel optimizations.

## Completion checklist

- [x] Implementation and typed errors
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance recorded
- [x] Parity ledger updated
- [x] Relevant checks pass
- [x] Binding impact explicitly deferred; no binding files changed
