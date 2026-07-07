# Task 0016: parent-relative frames and measurement stations

## Parity target

- Ledger rows: Geometry / Frames, transforms, Earth orientation; Observation /
  Ground participants, displacement, clocks, weather
- Current status: frames Partial; ground participants Not assessed
- Intended status after this task: both Partial, with explicit fixed
  parent-relative geometry and a first measurement-owned participant

## User workflow

Create an Earth or planetary ground station at a typed Cartesian position in an
explicit parent frame, inspect its local frame identity, and use that identity
as the parent of another fixed frame such as an instrument mount.

## Scientific contract

- Inputs and units: finite typed position vector from parent origin to child
  origin, expressed in parent axes.
- Outputs and units: a parent-relative frame definition and measurement-owned
  ground station retaining the same typed position.
- Frames/epochs/time scales: the relation is fixed and epoch-independent in the
  parent axes; derived axes are aligned with and inherit motion from the parent.
- Conventions and valid regimes: Cartesian fixed offsets only; no geodetic or
  topocentric interpretation.
- External data requirements: none.
- Errors and singularities: empty station IDs, non-finite offsets, and direct
  self-parent definitions are rejected.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| Original orskit design | Project-owned MIT/Apache-2.0 work | Explicit caller-owned hierarchy and participant boundary | `orskit-frames`, `orskit-measurements`, ADR-0015 |

## Design

- Affected crates/layers: frames and measurements; architecture/parity docs.
- Public API: `DerivedFrame`, `FrameDefinitionError`, `GroundStation`,
  `GroundStationError`.
- Rejected alternatives: recursive identity allocation, global registry,
  premature geodetic/topocentric models, standalone stations crate.
- ADR required: ADR-0015.

## Validation

- Unit cases: Earth-fixed station, planetary station, parent chain, invalid
  identity, non-finite position, direct self-parent.
- Invariants/properties: child orientation and motion equal the parent's;
  position remains expressed in the declared parent.
- Independent reference vectors: none; no numerical transform is claimed.
- Differential/scenario tests: station-to-instrument parent chain.
- Tolerances and justification: none; geometry is retained without computation.
- Benchmarks: none; no performance claim.

## Completion checklist

- [x] Implementation and typed errors
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance recorded
- [x] Parity ledger updated
- [x] Relevant checks pass
- [x] Binding impact explicitly deferred by user direction
