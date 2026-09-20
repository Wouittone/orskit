# ADR-0043: require an explicit body ephemeris provider

- Status: Accepted
- Date: 2026-09-20
- Affected parity rows: foundations/data context; geometry/celestial bodies
  and ephemerides; propagation/dynamics and force-model composition

## Context

Third-body acceleration and other geometry workflows need celestial-body
position and velocity at a precise epoch in the same complete reference frame
as their other states. Body identity alone must not select an ephemeris, while
placing data lookup in dynamics would introduce ambient scientific state and
couple force models to a file format or network service. The boundary must
also accept heterogeneous application implementations without erasing units,
body identity, epoch, frame, or actionable failures.

## Decision

1. The feature-gated Cartesian `orbits` layer owns
   `BodyEphemerisProvider`, an object-safe, `Debug + Send + Sync` contract.
   Every request names a `Body`, Hifitime `Epoch`, and complete
   `ReferenceFrame`.
2. Successful evaluations return `BodyState`, which retains that body and epoch
   with finite, typed `FrameKinematics`. Providers must return the requested
   body, epoch, and frame rather than silently transforming, relabeling, or
   substituting them.
3. `BodyEphemerisError` distinguishes unsupported bodies, unsupported frames,
   coverage gaps, and provider evaluation failures. The last preserves its
   concrete source behind a boxed error at this genuine object-safe extension
   boundary.
4. Applications construct and pass providers explicitly. Implementations own
   data loading, interpolation, caching, coverage, and provenance; they may not
   select or download scientific data through ambient process state.
5. This decision adds no ephemeris implementation, data source, gravitational
   parameter, acceleration assembly convention, or force model.

## Alternatives considered

- Put the provider in `bodies`: rejected because `bodies` intentionally owns
  identity and classification only, and depending on framed typed kinematics
  would invert the existing `frames -> bodies` dependency.
- Put the provider in dynamics: rejected because ephemeris state is reusable
  orbit/geometry data and must not imply a force model.
- Return raw arrays or unframed positions: rejected because units, velocity,
  frame origin/orientation, and epoch could be omitted or confused.
- Let each implementation expose an associated error: rejected because users
  could not compose arbitrary provider implementations behind one plain trait
  object. A contextual domain error with a boxed source preserves both object
  safety and the implementation failure.

## Consequences

- Third-body work can depend on one caller-selected provider without adding
  data or network dependencies to dynamics.
- A concrete ephemeris adapter remains responsible for documenting its data
  identity, coverage, interpolation, accuracy, and resource behavior.
- Consumers must enforce the documented result-identity invariant at their
  trust boundary; the future third-body adapter will report a provider that
  returns a mismatched body, epoch, or frame rather than using that state.

## Validation

Unit tests exercise the contract through `Box<dyn BodyEphemerisProvider>`,
verify typed body/epoch/frame/position/velocity retention, cover unsupported
body, frame, and epoch failures, and confirm that evaluation error sources are
preserved. No numerical ephemeris or third-body accuracy claim is made.

## Provenance

This is an original project API decision based on the existing explicit-data,
typed-state, and object-safe extension policies. No external source,
implementation, test vector, or data set was consulted.
