# Task: prescribe attitude and resolve body-fixed finite thrust

## Parity target

- Ledger row: attitude / rotations, angular states, and attitude providers;
  propagation / maneuvers, mass, and finite/impulsive burns
- Current status: Partial, with framed attitude values but no providers,
  interpolation, or attitude-dependent force evaluation.
- Intended status after this task: Partial, with fixed and bounded tabulated
  quaternion providers, shortest-arc interpolation, and body-fixed finite
  thrust resolved at every numerical stage.

## User workflow

A caller constructs a fixed or tabulated body-to-orbit attitude provider,
declares a constant finite-burn force in spacecraft body axes, propagates it
with the existing mass-qualified maneuver workflow, and inspects the rotated
orbit change, remaining mass, and execution log.

## Scientific contract

- Inputs and units: Hifitime sample/burn epochs; dimensionless quaternion
  components; typed body angular velocity, force, mass, and mass flow.
- Outputs and units: providers return a validated `QuaternionAttitude`;
  maneuver propagation returns the existing `CartesianMassState` and audit
  log.
- Frames/epochs/time scales: providers consume a complete
  `Orbit<S>` and return body-to-orbit-frame orientation. Tables have explicit
  closed coverage and one body ownership capability. Body thrust is rotated
  into the propagated Cartesian frame at every Runge--Kutta stage epoch.
- Conventions and valid regimes: quaternion components use scalar/i/j/k order.
  A fixed provider requires exactly zero body angular velocity.
  Tabulated orientation follows the shortest unit-quaternion arc; equivalent
  `q` and `-q` endpoints represent one rotation. Angular velocity is linearly
  interpolated in shared body axes and is not claimed to be the derivative of
  the SLERP curve. Body thrust is prescribed, not torque-driven steering.
- External data requirements: none for fixed/tabulated providers. Samples and
  base-dynamics data are caller-owned; there is no network or ambient lookup.
- Errors and singularities: invalid sample ordering, frame/body disagreement,
  interpolation failure, missing coverage, provider failure, body/reference
  mismatch, mass exhaustion, and numerical failure are typed and
  source-preserving.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| Ken Shoemake, [*Animating Rotation with Quaternion Curves*](https://doi.org/10.1145/325334.325242), Computer Graphics 19(3), 1985 | Copyrighted primary paper; interpolation concept/equations only | Unit-quaternion spherical interpolation for rotations | `Orientation::slerp`; `TabulatedAttitudeProvider`; sign-equivalence and midpoint tests |
| Goebel and Katz, *Fundamentals of Electric Propulsion*, 2008, chapter 2 | Previously recorded public JPL technical book | Instantaneous `F/m` acceleration and constant mass-flow law | Existing maneuver kernel with rotated force input |

No external source code, tests, examples, or distinctive prose was copied.
`nalgebra` remains the previously approved numerical dependency and supplies
the unit-quaternion interpolation kernel.

## Design

- Affected crates/layers: `core`; new `attitude` implementation crate;
  `dynamics-numerical`; feature-gated `dynamics` and `orskit` facades; task,
  decision, parity, architecture, and provenance evidence.
- Public API: `FramedForce`, `Orientation::slerp`, `Orientation::rotate_force`,
  `AttitudeProvider`, `FixedAttitudeProvider`, `AttitudeSample`,
  `TabulatedAttitudeProvider`, `ThrustFrame`,
  `ConstantThrustManeuver::body_fixed`, and
  `propagate_with_attitude_maneuvers` with typed errors.
- Rejected alternatives: hidden attitude inference; raw rotation matrices;
  extrapolating tables; treating body components as inertial; integrating
  quaternion/torque states before their tolerance and dense-output contracts
  exist; forcing provider implementations into representation-neutral `core`.
- ADR required: ADR-0040 records provider ownership, interpolation,
  stage-sampling, and the prescribed-versus-dynamic attitude boundary.

## Validation

- Unit cases: fixed-provider frame rejection; table ordering and coverage;
  equivalent quaternion signs; SLERP midpoint; angular-rate interpolation;
  body-provider/reference-frame mismatches and provider-required behavior.
- Invariants/properties: orientation endpoints retain frames; SLERP returns a
  unit quaternion; a 90-degree body-to-reference rotation maps body `+x`
  thrust to reference `+y`; body-fixed forward/reverse propagation recovers
  mass and state.
- Independent reference vectors: identity-to-180-degree rotation has a
  90-degree midpoint and maps the unit x-axis to y, directly derived from the
  quaternion rotation convention.
- Differential/scenario tests: the existing closed-form variable-mass burn is
  repeated with a fixed 90-degree attitude; provider call counting proves
  evaluation across adaptive Runge--Kutta stages. A one-sample table proves
  coverage failure remains nested as its typed source.
- Tolerances and justification: rotated zero components are bounded at
  `2e-12 m/s`; the non-zero velocity retains the existing variable-mass
  `2e-8 m/s` global error budget. Reverse recovery uses `5e-9 m/s`.
- Benchmarks: no performance claim; provider dispatch and interpolation remain
  future profiling targets.

## Completion checklist

- [x] Implementation and typed errors
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance recorded
- [x] Parity ledger updated
- [x] Relevant checks pass
- [x] Binding impact explicitly deferred; no binding files changed
