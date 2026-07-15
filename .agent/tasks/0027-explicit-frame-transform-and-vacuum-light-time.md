# Task 0027: explicit frame-transform boundary and vacuum light time

## Parity target

- Ledger rows: Geometry / Frames, transforms, Earth orientation; Observation /
  Range, range-rate, angles, Doppler, GNSS, inter-satellite.
- Current status: Partial.
- Intended status after this task: Partial, with an explicit transform-provider
  boundary and a feature-gated, iterated vacuum timing solution for ordered
  signal paths.

## User workflow

An application holds participant state in a declared source frame, supplies a
`KinematicFrameTransformProvider` that owns all needed transform data, wraps
the state provider with `TransformingParticipantStateProvider`, and selects
`VacuumLightTimeSolver`. The solver fixes the final signal event at the
measurement's reported epoch and solves each earlier event backward. The
returned `SignalEventTimeline` remains separate from observation provenance.

## Scientific contract

- Inputs and units: finite typed Cartesian position and velocity, explicit
  source/target frames, Hifitime epochs, and the exact SI vacuum speed of
  light.
- Outputs and units: a frame-qualified transformed state or a monotonic signal
  event timeline. The predicted measurement still owns its reported epoch.
- Frames/epochs/time scales: every transform is given an epoch and must return
  the requested target frame. The built-in identity implementation refuses
  distinct frames. Light-time events are solved backward from reception.
- Conventions and valid regimes: one midpoint correction-gradient sample is
  used for each vacuum leg during every fixed-point update. This is not an
  Earth-orientation model, a refractive ray trace, a transponder model, or a
  claim of high-order integration accuracy.
- External data requirements: concrete transforms own their selected Earth
  orientation, ephemeris, rotation, and translation data; the built-in solver
  needs no network data.
- Errors and singularities: non-finite states, transform output-frame mismatch,
  absent participants, zero-length legs, non-positive modeled delay, and
  non-convergence are explicit errors.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| JPL DESCANSO *Radiometric Tracking Techniques for Deep-Space Navigation*, Chapter 3 | Public technical monograph | Ordered link events, range as signal-path distance, and station reference-frame context only | `crates/measurements/src/estimation.rs`; `.agent/PROVENANCE.md` |
| BIPM definition of the metre | SI definition | Exact vacuum speed of light | existing `utils::constants::speed_of_light` |

No external implementation, source code, tests, or examples were used.

## Design

- Affected crates/layers: `frames`, `measurements`, public facade features, and
  the architecture, parity, and provenance records.
- Public API: `FrameKinematics`, `KinematicFrameTransformProvider`,
  `IdentityKinematicFrameTransform`, `TransformingParticipantStateProvider`,
  and feature-gated `VacuumLightTimeSolver`.
- Rejected alternatives: relabel coordinate values as transformed; add a hidden
  global Earth-orientation model; make a correction directly rewrite epochs;
  or imply a media/relativity implementation from a vacuum timing solver.
- ADR required: no; this is a narrow, documented extension of the existing
  provider and signal-timeline contracts.

## Validation

- Unit cases: identity transforms reject distinct frames; transformed station
  output carries the requested target frame; a moving emitter produces an
  earlier emission event while retaining the reporting epoch.
- Invariants/properties: source and target frames are checked at both provider
  boundaries; event epochs are monotonic; a bounded fixed-point loop either
  converges or reports an error.
- Independent reference vectors: none; no numerical accuracy claim beyond the
  stated vacuum fixed-point equation is made.
- Differential/scenario tests: feature-isolated `light-time` compilation plus
  full-feature measurements and frames tests.
- Tolerances and justification: default convergence is one nanosecond, the
  representable Hifitime event-time granularity used by this workspace.
- Benchmarks: not required; no throughput claim is made.

## Completion checklist

- [x] Implementation and typed errors
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance recorded
- [x] Parity ledger updated
- [x] Relevant checks pass
- [x] Binding impact handled or explicitly deferred: bindings remain deferred
