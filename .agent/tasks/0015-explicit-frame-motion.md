# Task 0015: require affirmative inertial frame semantics

## Parity target

- Ledger row: Geometry / Frames, transforms, Earth orientation
- Current status: Partial
- Intended status after this task: Partial with explicit inertial eligibility
  for orbital conversion and analytical propagation

## User workflow

Use built-in or explicitly classified custom frame orientations and receive a
typed rejection whenever an orbital algorithm cannot establish inertial axes.

## Scientific contract

- Inputs and units: framed orbital coordinates and states; no raw quantities.
- Outputs and units: unchanged orbital representations and propagated orbits.
- Frames/epochs/time scales: orientation motion is `Inertial`, `NonInertial`,
  or `Unspecified`; algorithms requiring inertial axes accept only `Inertial`.
- Conventions and valid regimes: built-in ICRF, GCRF, and EME2000 orientations
  are inertial for the current point-mass/orbital-element contracts.
- External data requirements: none.
- Errors and singularities: non-inertial and unspecified custom orientations
  are rejected before orbital conversion or propagation.

## Provenance

No new external implementation material is used. The existing inertial-frame
restriction is made explicit in orskit's own frame identity contract.

## Design

- Affected crates/layers: frames, core state conversion, dynamics, minimal
  binding construction compatibility, architecture documentation.
- Public API: `FrameMotion`; motion-bearing custom orientations; affirmative
  `motion()` and `is_inertial()` queries.
- Rejected alternatives: a blacklist of known rotating frames; treating every
  custom orientation as inertial; silently transforming frames.
- ADR required: ADR-0014.

## Validation

- Unit cases: built-in classification and custom classification.
- Invariants/properties: only affirmative inertial axes pass conversion and
  propagation guards.
- Independent reference vectors: existing two-body vectors remain unchanged.
- Differential/scenario tests: existing representation propagation suite.
- Tolerances and justification: no numerical tolerance change.
- Benchmarks: no performance claim; classification is constant-time.

## Completion checklist

- [x] Implementation and typed errors
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance recorded
- [x] Parity ledger updated
- [x] Relevant checks pass
- [x] Binding impact handled with minimal custom body-frame construction changes
