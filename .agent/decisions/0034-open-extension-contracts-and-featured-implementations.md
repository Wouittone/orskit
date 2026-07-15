# ADR-0034: open extension contracts and feature-gated built-ins

- Status: Accepted
- Date: 2026-07-14
- Supersedes: ADR-0012 closed-attitude-state decision
- Affected parity rows: attitude, observation, CCSDS I/O, bindings

## Context

The core and measurement APIs had several closed enums embedded in public
composition points: `AttitudeState`, `SpacecraftShape`, `MeasurementKind`, and
OEM covariance axes. `MeasurementQuantity` was sealed. Each choice required a
change to an orskit crate before an application could add a representation,
observable family, unit-bearing quantity, or covariance convention. Built-in
implementations also compiled as a group, even when an application required
only one of them.

## Decision

1. `Attitude`, `SpacecraftGeometry`, `MeasurementKind`, `MeasurementQuantity`,
   `CorrectionKind`, and `OemCovarianceAxes` are open public contracts.
2. `Spacecraft<G>` and `SpacecraftView<S, G, A>` retain physical frame checks
   while accepting application-owned geometry and attitude implementations.
3. `OemCovarianceFrame` type-erases an axes implementation, retains the OEM
   identifier, and preserves unknown declarations as `DeclaredCovarianceAxes`.
4. The `core` crate separately gates its quaternion-attitude and standard-shape
   implementations. The `measurements` crate separately gates every built-in
   observable family and correction-provenance marker. The facade forwards
   these selections and keeps its default implementation-free.
5. `AttitudeState` remains a feature-gated alias for `QuaternionAttitude` for
   source compatibility; it is not a closed representation boundary.

## Consequences

- Applications can use only their selected built-ins with
  `default-features = false`, or supply implementations in their own crates.
- Heterogeneous measurement routing remains object-safe, while family identity
  is no longer capped by an enum.
- OEM messages with unfamiliar covariance axes remain constructible and retain
  their declared semantics instead of being rejected solely for catalog gaps.

## Validation

Compile the workspace with all features and the `core` and `measurements`
crates with no default features. Test application-owned attitude, geometry,
measurement quantity/family, and covariance-axes implementations.
