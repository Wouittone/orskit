# ADR-0015: model fixed measurement stations with parent-relative frames

- Status: Accepted
- Date: 2026-07-06
- Affected parity rows: frames and transforms; ground participants

## Context

`ReferenceFrame` identifies an origin and orientation but could not record that
an application-defined frame is based on another frame. A ground measurement
site needs at least a fixed position relative to an Earth-fixed frame before
geodetic conversion, clocks, displacement, and signal paths can be designed.

The identity is embedded in copyable state values throughout the workspace.
Making it recursively own parent frames would mix identity with transform
configuration, force allocation into every state, and make caller-controlled
hierarchies harder to inspect.

## Decision

1. `ReferenceFrame` remains the compact copyable identity carried by physical
   values.
2. `DerivedFrame` is the first explicit frame-definition value. It records a
   custom child identity, one direct parent, and a finite fixed origin offset
   expressed in the parent axes.
3. The initial derived-frame axes are parent-aligned. Their orientation and
   inertial-motion classification are inherited from the parent.
4. Hierarchies are caller-owned: a child definition can use another derived
   frame's identity as its parent, while the definitions themselves retain the
   edges and geometry. There is no ambient mutable registry.
5. `GroundStation` belongs to `orskit-measurements`. It owns a stable station
   identifier and one parent-relative frame definition. It is body-agnostic and
   does not introduce a standalone stations crate.
6. Binding changes are deferred while Rust core contracts remain unstable.

## Rejected alternatives

- Recursively store `Arc<ReferenceFrame>` parents in every identity: identity
  would become allocation-backed and lose its current `Copy` contract.
- Add a process-global frame registry: this would violate deterministic,
  caller-controlled scientific context.
- Treat a station position as geodetic latitude/longitude/height now: reference
  ellipsoids and geodetic conversion are not implemented, so this would hide a
  model choice.
- Define topocentric axes without a transform contract: parent alignment is
  explicit and honest; ENU/NED/SEZ orientations require later validated
  rotation and geodesy work.

## Consequences

- Earth-fixed and planetary sites can be represented immediately using typed
  Cartesian offsets in explicit parent frames.
- Frame parentage is inspectable without implementing coordinate transforms.
- A bare `ReferenceFrame` intentionally does not discover its ancestors; code
  performing composition must receive the corresponding definitions or a
  future immutable frame context.
- Moving stations, tectonic displacement, local topocentric orientations,
  clocks, weather, and signal corrections remain explicit future capabilities.

## Validation

Frame tests cover parent retention, inherited orientation/motion, explicit
multi-level chains, non-finite geometry, and direct self-parent rejection.
Measurement tests cover Earth-fixed and planetary stations plus invalid station
identity and geometry.

## Provenance

This is original orskit domain architecture based on the project's explicit
frame and participant requirements. No third-party implementation source,
tests, or internal structure informed the design.
