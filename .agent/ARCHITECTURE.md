# Target architecture

This is the target shape, not a claim about the current pre-alpha scaffold.
Crate boundaries should evolve toward it through tested vertical slices.

## Dependency direction

Dependencies flow downward only:

```text
public facade and language bindings
                  |
       mission workflows and I/O
                  |
 measurements, estimation, propagation, events, attitude
                  |
         orbits, frames, bodies
                  |
       time, math, units, data
```

Lower layers must not depend on bindings, file formats, application workflows,
or process-global configuration. Cycles between domain crates indicate a
missing abstraction or an incorrectly placed type.

## Domain boundaries

### Foundations

- **Math and units:** `uom` quantities, typed Cartesian values, interpolation,
  root finding, integration, linear algebra adapters, numerical tolerances.
- **Time:** Hifitime's `Epoch` and `Duration` types are used directly for
  instants, durations, calendars, time scales, leap seconds, and conversion.
- **Data:** explicit, versioned access to Earth orientation, gravity,
  atmosphere, ephemeris, and space-weather inputs.

### Physical model

- **Bodies:** reusable celestial-body identities and explicit body-system
  membership, followed by reference ellipsoids, geodetic conversion, rotation,
  and ephemeris providers. Identity does not imply a physical-data model.
- **Frames:** origins compose a body, body-system barycenter, or explicit custom
  origin with an orientation. Transform-provider contracts admit optional
  external adapters for kinematic transforms, Earth orientation, and transform
  composition.
- **Orbits:** frame- and epoch-qualified states, element sets, conversions,
  Jacobians, interpolation, and covariance representations.

### Dynamics and observation

- **Forces:** composable acceleration and mass-flow models with declared data
  requirements.
- **Propagation:** analytical, numerical, semi-analytical, and TLE propagators;
  dense output; ephemerides; variational equations.
- **Events:** detector functions, direction, root localization, handlers, and
  deterministic simultaneous-event policy.
- **Attitude:** rotations, angular derivatives, attitude providers, and
  spacecraft geometry.
- **Measurements:** typed observations, participants, timing, modifiers,
  uncertainties, ground assets, spacecraft links, and correction models. A
  ground observer is a measurement participant, not a separate top-level
  domain or crate.
- **Estimation:** parameters, residuals, least squares, filters, covariance,
  and state-transition/sensitivity machinery.

### Edges

- **I/O:** standards and file formats translate to domain types; domain types
  never depend on parsers.
- **Facade:** a curated `orskit` crate re-exports stable workflows without
  flattening important domain distinctions.
- **Bindings:** Python and JVM adapters translate errors, ownership, arrays,
  callbacks, and asynchronous work without reimplementing physics.

## Core data contracts

- A complete physical `State` has an epoch, positive mass, orientation, inertia
  matrix, and one native coordinate representation. Cartesian, Keplerian,
  equinoctial, and future representations implement the same trait without
  embedding one another. Algorithms obtain another representation through an
  explicit conversion trait and supply conversion-only context there. Each
  position, velocity, acceleration, orientation, inertia tensor, covariance, and other
  coordinate-dependent value carries the frame information needed to interpret
  it; a state does not imply that its components share a frame.
- File formats that omit physical properties yield values such as
  `CoordinateSample<CartesianCoordinates>`, not fabricated complete states.
  Callers enrich them with explicit `SpacecraftProperties` at the workflow boundary.
- Every public physical scalar and vector uses a typed quantity. `uom` is the
  canonical dimensional system and SI is its storage baseline. Raw scalars may
  appear only at explicitly unit-named numerical, serialization, and FFI
  boundaries; angles are typed rather than documented by convention alone.
- Hifitime is used directly as the canonical epoch/time implementation. orskit
  does not wrap it in a weaker numeric time type.
- UTC is a civil representation, not a uniform integration coordinate.
- Frame transforms support position, velocity, and—where required—acceleration
  consistently.
- Algorithms declare their frame compatibility. Until supplied with a transform
  provider, an algorithm rejects combinations it cannot evaluate rather than
  narrowing what a `SpacecraftState` may represent.
- External scientific data enters through an immutable `DataContext`-style
  value or trait-backed provider. Algorithms declare their required data.
- Constants identify their convention and source; there is no anonymous
  "Earth constant" shared across incompatible models.
- Covariance and Jacobian values identify their parameterization, ordering,
  frame, and units.

## API strategy

- Prefer small cohesive types, enums for closed physical alternatives, and
  traits for genuine extension points.
- Use builders when construction has many optional model choices; validate at
  construction so propagation does not repeatedly discover configuration
  errors.
- Separate immutable model configuration from mutable integration workspace.
- Make expensive allocation and data loading observable to callers.
- Use typed domain errors and preserve error sources.
- Do not expose dependency-specific types in stable public APIs unless the
  dependency is an intentional part of the compatibility contract. Hifitime
  and `uom` are intentional foundational contracts.

## Concurrency and determinism

Domain models should be immutable and shareable where practical. Caches must
have deterministic semantics and explicit invalidation/versioning. Parallel
algorithms must document reduction order and expected floating-point drift.
There is no ambient mutable "current data context."

## FFI architecture

- Keep a narrow interoperability layer above stable Rust domain APIs.
- JVM bindings should use a versioned C-compatible ABI suitable for the Java
  Foreign Function & Memory API; Python may use PyO3 above the same domain API.
- Prefer opaque handles and owned result buffers over exposing Rust layouts.
- Define ownership, lifetime, thread-safety, nullability, and error behavior
  for every exported symbol.
- Catch panics before every foreign boundary and convert them to structured
  errors.
- Generate or verify binding declarations where possible and test package
  installation, not only compilation.

## Crate evolution

The current `bodies`, `core`, `frames`, `measurements`, `units`, and `utils`
crates are an initial scaffold. Generic names such as `core` and `utils` are transitional.
New boundaries should use namespaced package names such as `orskit-time` and
`orskit-frames`. Split crates only when the domain boundary and dependency
direction are clear; do not create one crate per noun pre-emptively.

Likely long-term packages include a public `orskit` facade and focused packages
for time/data, frames/bodies, orbits, propagation/forces/events, attitude,
measurements/estimation, I/O, and FFI. The exact split requires architecture
decision records and evidence from real vertical slices.

The initial split now includes `orskit-units` for typed quantities,
`orskit-bodies` for celestial and body-system identities, `orskit-frames` for
body-backed frame identity, and a trait-based state model with Cartesian,
elliptic Keplerian, and elliptic equinoctial representations.
`lox-frames` is scientifically promising but still alpha; ANISE is mature but
owns a broader almanac/orbit context than this foundational slice needs. Keep
the boundary adapter-friendly and revisit both for transform providers rather
than leaking either API into spacecraft state.

There is deliberately no dynamics/propagation crate yet. Its design must cover
composable multi-body and coupled dynamics, force models, events, integration,
and variational equations; a two-body example must not become the organizing
abstraction. Likewise there is no `stations` crate: ground and spacecraft
participants belong to the measurement topology and estimation workflows.
