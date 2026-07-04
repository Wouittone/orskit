# ADR-0001: foundational time, units, frames, and state types

- Status: Accepted; frame-origin ownership and state composition superseded in
  part by ADR-0005 and ADR-0004 respectively
- Date: 2026-07-04
- Owners: orskit maintainers
- Affected parity rows: foundations/time/units, geometry/frames,
  orbits/Cartesian states, attitude

## Context

orskit needs foundational types before propagation algorithms can have sound
contracts. Raw floating-point fields would repeat a major class of unit and
context errors. Time and frame implementations are also expensive to build and
maintain, so existing crates were evaluated before choosing project boundaries.

## Decision

1. Use [Hifitime 4.3](https://docs.rs/hifitime/4.3.0/hifitime/) directly for
   epochs and time operations. It remains an unmodified MPL-2.0 dependency; no
   weaker numeric time wrapper is introduced.
2. Use [`uom` 0.38](https://docs.rs/uom/0.38.0/uom/) directly for compile-time
   dimensional quantities. orskit-owned semantic vector and state types compose
   these quantities.
3. Own the small frame identity contract in `orskit-frames`: a frame is an
   origin plus an orientation. ADR-0005 subsequently moved celestial-body and
   body-system identity into `orskit-bodies`, which frame origins compose. Keep
   future transform calculation behind adapter-friendly provider contracts.
4. Define spacecraft state as an epoch, independently framed position and
   velocity, and positive mass. This initial composition was superseded by
   ADR-0004, which requires orientation and inertia in every complete `State`
   and separates incomplete Cartesian coordinates from complete states.
5. Permit raw scalars only at explicitly unit-named numerical, serialization,
   parsing, and FFI boundaries.

## Alternatives considered

- [`lox-frames` 0.1.0-alpha.11](https://docs.rs/lox-frames/0.1.0-alpha.11/lox_frames/)
  has an attractive, focused frame model and may become a transform-provider
  adapter. Its current alpha API and coupling to the wider Lox type ecosystem
  make it too unstable to embed in every orskit state today.
- [ANISE](https://docs.rs/anise/latest/anise/) has a mature frame/ephemeris
  engine and may become an almanac-backed adapter. Its broader state and data
  context would make the basic spacecraft value depend on much more than frame
  identity.
- Hand-written numeric units were rejected because they would duplicate a
  mature dimensional type system and make compound-unit arithmetic fragile.
- A bare frame string or integer was rejected because it cannot distinguish an
  origin from an orientation and makes invalid combinations easy to hide.

## Consequences

- Unit mistakes become type errors across Rust domain APIs.
- Hifitime and `uom` are intentional public compatibility dependencies.
- FFI bindings must name their raw units and convert immediately to typed
  values.
- Actual frame transforms remain incomplete, but every coordinate-dependent
  state value carries frame identity while providers are evaluated
  independently. Individual algorithms reject unsupported cross-frame inputs.
- MPL-2.0 dependency notices must remain available in distributed dependency
  metadata; project-owned source remains MIT/Apache-2.0.

## Validation

- Compile-fail documentation proves incompatible quantities cannot be added.
- Unit tests cover vector dimensions, frame parsing, state invariants,
  quaternion normalization, inertia positive definiteness and principal-moment
  triangle inequalities.
- Rust, Python, and C-ABI/JVM workspaces compile from the same domain state.

## Provenance

Only public crate documentation and package metadata were used for this
dependency decision. No source from Orekit, Nyx astrodynamics, Lox, or ANISE was
copied or translated.
