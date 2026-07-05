# Task 0012: split spacecraft definition from epoch-specific view

- Status: Superseded in part by task 0013
- Parity rows: Orbits / Cartesian states; Attitude / angular states;
  Propagation / two-body propagation

## Goal

Correct the spacecraft ownership boundary introduced by task 0011. Keep
identity and geometry time-independent, move epoch/mass/orbit/inertia/attitude
to a separate borrowed view, and remove the attitude generic parameter.

## Evidence

- Time-independent spacecraft ID and point/sphere/cuboid shape tests.
- Epoch-specific mass, orbit, inertia, and attitude view tests.
- Rigid-body frame validation through the initial attitude abstraction (later
  replaced by the closed enum in task 0013).
- Propagation retains the spacecraft definition, attitude, mass, and inertia.
- Workspace and binding formatting, tests, Clippy, and documentation checks.

Validation passed with workspace tests, Clippy with warnings denied, rustdoc
with warnings denied, Java binding tests/Clippy, and Python binding Clippy in
interpreter-free `abi3-py312` mode.
