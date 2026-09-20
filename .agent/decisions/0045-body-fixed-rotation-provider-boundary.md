# ADR-0045: require explicit body-fixed rotation provider boundary

- Status: Accepted
- Date: 2026-09-20
- Owners: frames maintainers
- Affected parity rows: Geometry / frames, transforms, Earth orientation;
  Propagation / gravity fields and solid/ocean tides

## Context

General spherical harmonics, tides, drag, and ground observation workflows need
positions and velocities converted between inertial and body-fixed axes. The
existing high-level frame transform boundary allows complete kinematic
solutions, but it does not expose the narrower prerequisite needed by force and
geometry implementations: an epoch-dependent rotation with explicit direction
and body angular velocity. Implementing that as bundled Earth-orientation data
inside `frames` would make scientific inputs ambient, while implementing
gravity or drag now would mix this prerequisite with model families that have
their own data and validation requirements.

## Decision

Add a frames-layer-only body-fixed rotation boundary:

1. `BodyFixedTransformRequest` names the epoch, explicit Hifitime time scale,
   inertial frame, body-fixed frame, and transform direction.
2. Construction requires same-origin frames, an affirmative inertial frame on
   one side, and non-inertial axes on the body-fixed side.
3. `DirectionCosineMatrix` validates finite, orthonormal, right-handed
   matrices.
4. `BodyFixedTransform` carries the validated request, rotation direction, and
   body angular velocity relative to inertial axes expressed in body-fixed axes,
   and applies the correct velocity cross term.
5. `BodyFixedTransformProvider` is object-safe and returns a standardized typed
   provider error, so applications can supply EOP/convention-backed
   implementations without generics or a global data context.
6. `BodyFixedKinematicFrameTransform` adapts this provider to the existing
   `KinematicFrameTransformProvider` while verifying that provider output
   answers the exact request.

No Earth-orientation data, concrete EOP parser, spherical harmonics, tides,
drag, dynamics composition, workspace facade, or orbit ephemeris behavior is
added by this decision.

## Alternatives considered

- Extend only `FrameReferenceDataSupplier`: rejected because high-level
  kinematic suppliers do not make the rotation direction or angular velocity
  contract explicit enough for force-model prerequisites.
- Use a generic associated-error provider: rejected because downstream
  heterogeneous provider storage needs an object-safe boundary without naming a
  concrete associated error type.
- Add a concrete IERS/IAU implementation immediately: rejected because data
  selection, interpolation, coverage, and reference-vector validation belong to
  a separate implementation slice.
- Let gravity-field code own ad hoc rotation matrices: rejected because it
  would duplicate frame validation and make velocity terms easy to omit.

## Consequences

- Callers must supply a provider and chosen time scale explicitly before
  converting inertial/body-fixed position-velocity kinematics.
- Body-fixed conversion is limited to same-origin frame pairs. Translation,
  ephemeris, geodesy, displacement, tides, and gravity-field coefficient data
  remain separate contracts.
- Concrete providers can be feature-gated later and validated against
  independent Earth-orientation transform vectors without changing the frames
  contract.

## Validation

`frames` unit tests cover explicit request time-scale conversion, origin and
axis rejection, rotation-matrix validation, object-safe provider dispatch,
angular-velocity velocity terms, inverse recovery, and adapter behavior that
does not relabel frames. Rustdoc demonstrates constructing a data-free
transform value while keeping the provider/data implementation external.
