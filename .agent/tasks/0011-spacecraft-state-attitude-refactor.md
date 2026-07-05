# Task 0011: separate orbital state from spacecraft attitude

- Status: Superseded in part by task 0012
- Parity rows: Orbits / Cartesian states; Orbits / orbital elements; Attitude /
  rotations and angular states; Propagation / two-body propagation

## Goal

Represent the supported six-element orbital alternatives with a closed
`SpacecraftState` enum, expose symmetric target-side (`From`, `TryFrom`) and
source-side (`To`, `TryTo`) conversion APIs, and compose that orbital state
with epoch, mass, inertia, and an `Attitude` implementation in a spacecraft
snapshot.

## Physical contract

- Cartesian elements: position `(x, y, z)` and velocity `(vx, vy, vz)` in one
  explicit frame.
- Keplerian elements: `(a, e, i, Omega, omega, nu)` in the existing bound
  elliptic regime.
- Equinoctial elements: `(a, ex, ey, hx, hy, lv)` in the existing convention.
- Representation-changing conversions take an explicit positive central-body
  gravitational parameter. They remain fallible for invalid Cartesian conics
  and the retrograde equinoctial singularity.
- Attitude exposes an explicit framed rotation and an angular-velocity vector.
  The inertia tensor and angular velocity are expressed in the attitude body
  frame.
- A spacecraft snapshot owns one epoch, positive mass, orbital state, inertia
  tensor, and attitude value.

## Public API plan

- Add concrete six-element `CartesianState`, `KeplerianState`, and
  `EquinoctialState` values and the closed `SpacecraftState` enum.
- Add `To` as the source-side counterpart of standard `From`, and `TryTo` as
  the source-side counterpart of standard `TryFrom`.
- Use `From`/`To` for infallible enum wrapping and `TryFrom`/`TryTo` with an
  `OrbitalConversion` context for representation changes.
- Add `OrbitalElements`, implemented by every concrete state and the enum, to
  request any supported representation.
- Add `Attitude`, `AttitudeState`, framed angular velocity, and an initial
  spacecraft snapshot (later corrected by task 0012).

## Evidence

- Constructor and frame-invariant tests for all state and attitude values.
- `From`/`To` and `TryFrom`/`TryTo` tests across every supported pair.
- Analytic circular and polar conversion checks plus round trips.
- Spacecraft composition tests for mass and rigid-body frame consistency.
- Workspace formatting, checks, Clippy, tests, and documentation.

Validation completed with `cargo test --workspace --locked`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`, and
`RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked`. The Java
binding tests and Clippy pass. The Python binding compiles and passes Clippy in
PyO3 interpreter-free `abi3-py312` check mode; a local import smoke test still
requires an installed Python interpreter.

## Provenance

No new scientific model or external implementation source is introduced. The
existing independently implemented conversion equations and references in
`PROVENANCE.md` remain the basis for this API refactor.
