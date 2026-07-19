# Task: establish a frame-explicit sequential orbit-determination slice

## Parity target

- Ledger row: Estimation / batch least squares and sequential filters.
- Current status: Not assessed.
- Intended status after this task: Partial, limited to Cartesian extended and
  unscented Kalman filtering with built-in Cartesian position observations.

## User workflow

An application creates an `Orbit<CartesianState>` prior with a
`CartesianCovariance`, constructs an implementation of
`dynamics::Propagator<CartesianState>` that owns its physical problem, and processes one
or an ordered series of epoch-qualified `CartesianObservation` values through
either `ExtendedKalmanFilter` or `UnscentedKalmanFilter`. It may supply an
`EstimationObserver` for a validation run; production calls retain no
diagnostic history.

## Scientific contract

- Inputs and units: public state is `Orbit<CartesianState>`, not a coordinate
  vector.  The Cartesian covariance and position covariance are domain objects
  with typed standard-deviation constructors; position, velocity,
  gravity parameter, and integration scaling remain typed.
- Outputs and units: posterior `Orbit<CartesianState>` and domain covariance at
  the observation epoch.
- Frames/epochs/time scales: all states, observations, and propagated outputs
  declare the same `ReferenceFrame`; epochs are `hifitime::Epoch` and are
  never relabelled.
- Conventions and valid regimes: Cartesian state is regular for all
  non-collision orientations. The selected propagator owns
  force-model, ephemeris, integration, and data-provenance conventions.
- External data requirements: none are selected or downloaded by OD.
- Errors and singularities: non-finite values, non-PD covariances, non-inertial
  or mismatched frames, zero primary separation, and singular innovation
  covariance, propagator failure, and invalid UKF scaling are explicit errors.
  Solver accuracy and collision
  policy belong to the selected propagator.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| nalgebra 0.35 | Apache-2.0 dependency | Unmodified fixed-size linear algebra and Cholesky factorization | `orbit-determination` dependency |
| orskit `dynamics::Propagator` | Original project contract | A propagator owns its physical problem and algorithm | `orbit-determination` filter boundary |
| finitediff 0.2.0 | MIT OR Apache-2.0 dependency | Unmodified central-Jacobian algorithm for the EKF transition calculation | `orbit-determination` dependency |

No external implementation, source, tests, examples, or datasets are copied.

## Design

- Affected crates/layers: standalone `orbit-determination` implementation,
  public facade feature, and project capability records.
- Public API: `OrbitDetermination`, `KalmanFilter`, generic
  `StateEstimate<S, C>`, state-domain covariance contracts,
  `ExtendedKalmanFilter`, `UnscentedKalmanFilter`, typed Cartesian observation
  contracts, and opt-in `EstimationObserver`.
- Rejected alternatives: a measurements-to-dynamics dependency, hidden frame
  selection, a global ephemeris/data context, and separate two-/three-body
  filters with duplicated covariance code.
- ADR required: ADR-0036 records the reusable filtering boundary and explicit
  secondary ephemeris requirement.

## Validation

- Unit cases: typed construction, one-or-many observations, opt-in observer
  callbacks with innovation/residuals, covariance checks, and both filters
  over an `EllipticKeplerPropagator` owning `TwoBodyDynamics`.
- Invariants/properties: all propagated states and state-transition entries
  remain finite.
- Independent reference vectors: the selected two-body propagator retains its
  independent Orekit validation in `dynamics/two-bodies`; OD verifies that it
  uses that public contract rather than reimplementing propagation.
- Differential/scenario tests: common `KalmanFilter` contract runs EKF and UKF
  against the same prior, observation, problem, and propagator.
- Tolerances and justification: EKF normalizes Cartesian position by 10,000 km
  and velocity by 10 km/s before delegating central-Jacobian differentiation to
  `finitediff`; propagator accuracy is selected and validated by its dedicated
  crate.
- Benchmarks: deferred; no performance claim is made.

## Completion checklist

- [x] Implementation and typed errors
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance recorded
- [x] Parity ledger updated
- [x] Relevant checks pass
- [x] Binding impact explicitly deferred while the public Rust API is partial
