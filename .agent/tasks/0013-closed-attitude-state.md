# Task 0013: reconcile orbital and attitude state representations

- Status: Complete
- Parity rows: Attitude / rotations and angular states; Propagation / two-body
  propagation

## Goal

Replace the open attitude trait with a closed `AttitudeState` enum analogous to
`SpacecraftState`, remove `dyn Attitude`, and make epoch-specific spacecraft
views own their angular state directly.

## Evidence

- Quaternion attitude construction and frame validation.
- Attitude enum angle and angular-speed access.
- Spacecraft views own and preserve attitude values.
- Propagation preserves attitude equality across every orbital representation.
- Workspace and binding formatting, tests, Clippy, and documentation checks.

Validation passed with workspace tests, Clippy with warnings denied, rustdoc
with warnings denied, Java binding tests/Clippy, and Python binding Clippy in
interpreter-free `abi3-py312` mode.
