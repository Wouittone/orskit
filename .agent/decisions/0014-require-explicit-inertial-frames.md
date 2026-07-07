# ADR-0014: require affirmative inertial frame semantics

- Status: Accepted
- Date: 2026-07-05
- Affected parity rows: frames and transforms; orbital representations;
  two-body propagation

## Context

Orbital conversion and analytical propagation rejected a blacklist of known
rotating or date-dependent orientations. `Custom` and any future orientation
variant therefore passed as inertial without carrying evidence for that claim.

## Decision

1. Every frame orientation exposes `FrameMotion`: `Inertial`, `NonInertial`,
   or `Unspecified`.
2. Custom orientations carry their motion classification explicitly.
3. Algorithms whose equations require inertial axes accept only an affirmative
   `Inertial` value. `Unspecified` is rejected rather than guessed.
4. Built-in inertial classification remains a property of the orientation;
   frame transforms and data-dependent realization accuracy remain future
   provider responsibilities.
5. Spacecraft body axes are custom non-inertial orientations.

## Consequences

- New and custom orientations cannot become propagation-compatible by default.
- Applications can declare a custom inertial orientation without adding a
  built-in enum variant.
- Existing direct construction of `FrameOrientation::Custom(id)` changes to a
  motion-bearing form during the pre-alpha API phase.
- This classification does not implement transforms or prove that two frames
  are aligned at a given epoch.

## Validation

Frame tests cover built-in and custom classifications. State-conversion and
two-body tests cover rejection of unspecified/non-inertial axes and acceptance
of explicitly inertial custom axes.

## Provenance

This is original orskit domain architecture refining the already documented
inertial-axis precondition. No external implementation informed the design.
