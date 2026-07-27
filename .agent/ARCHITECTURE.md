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
  membership remain independent of caller-selected reference ellipsoids.
  Ellipsoids provide typed geodetic/geocentric conversion without implying a
  rotation or ephemeris provider.
- **Frames:** lightweight identities compose a body, body-system barycenter, or
  explicit custom origin with an orientation. Caller-owned `FrameCatalog`
  values issue namespace-qualified `FrameId` identities and validated
  `DerivedFrame` definitions with fixed typed offsets; only registered parents
  form chains, without a global registry.
  Catalog-issued local East–North–Up frames retain their affirmatively
  body-fixed parent, ellipsoidal geodetic origin, and reversible position
  transform; generic non-inertial axes do not satisfy that capability.
  Orientations declare inertial, non-inertial, or unspecified motion;
  algorithms requiring inertial axes accept only an affirmative inertial
  declaration. `FrameReferenceDataSupplier` records a non-empty immutable set
  of stable data-artifact descriptors with non-blank identities and supplies
  fully resolved kinematics to a validating transform adapter; implementations
  own Earth orientation, ephemerides, coverage, interpolation, caching, and
  convention selection. Caches may retain derived values but cannot silently
  replace selected scientific data.
  `Iers2010EarthOrientation` is the first concrete implementation: it consumes
  one verified caller-selected artifact and typed UT1-TAI/polar-motion samples,
  rejects coverage and interpolation gaps, and resolves full position and
  velocity between GCRF and ITRF2020 using the CIO-based IERS 2010/IAU
  2006+2000A convention. Observed celestial-pole offsets and operational
  IERS-product parsing remain separate future slices.
  Transform-provider
  contracts therefore admit optional external adapters without a global data
  context or a public matrix API.
- **Ephemerides:** an open provider evaluates one explicit target relative to
  one explicit observer, in a complete observer-centered frame, at an absolute
  epoch. Results contain finite typed position and velocity and expose every
  verified caller-selected artifact used. The first concrete provider applies
  piecewise cubic Hermite interpolation to already-decoded samples; operational
  format readers, transforms, aberration corrections, and caches remain
  separate explicit boundaries.
- **Orbits:** frame- and epoch-qualified states, element sets, conversions,
  Jacobians, interpolation, and covariance representations.

### Dynamics and observation

- **Dynamics:** `SystemDynamics` separates ordered conservative and
  non-conservative force-model descriptions acting on a spacecraft. Evaluation,
  coupled state layout, model data, derivatives, and numerical resolution
  remain separate future contracts.
- **Forces:** the open `Force` contract identifies a physical interaction while
  the object-safe `ForceModel` contract identifies one implementation of it.
  Models declare only their dependency on spacecraft position, speed,
  orientation, and inertia. Environmental bodies and other parameters belong
  to force-model configuration and explicit future data providers. Dynamics
  composes heterogeneous model trait objects without matching model types.
- **Propagation:** `Propagator<State>` owns both its solution method and the
  physical problem it advances. `PropagationState<Problem>` lets a concrete
  propagator resolve a caller-selected state into the representation it
  advances and restore it using its owned problem. The current analytical
  implementation owns `TwoBodyDynamics`, advances epoch-qualified
  `Orbit<State>` values, and preserves the selected state representation. A
  translational propagator does not imply that attitude or other
  epoch-dependent spacecraft properties were advanced. Analytical, numerical,
  semi-analytical, and TLE
  algorithms, dense output, ephemerides, and variational equations remain
  distinct capabilities.
  The opt-in `dynamics-numerical` implementation owns one evaluable
  `CartesianDynamics`, whose typed acceleration boundary declares its frame
  and component requirements. It advances only epoch-qualified translational
  `CartesianState` values with caller-selected typed tolerances, step bounds,
  and limits; the raw six-component SI layout is private. Its optional dense
  output is an immutable, directional collection of accepted-step cubic
  Hermite segments. Typed event handlers inspect those dense states, define
  direction in increasing physical epoch, and use bounded bisection with
  deterministic simultaneous ordering. This first event slice cannot reset or
  reintegrate state; coupled spacecraft properties and variational state remain
  separate capabilities.
- **Events:** detector functions, physical-time direction, bounded root
  localization, source-preserving handlers, and deterministic simultaneous
  ordering are explicit contracts.
- **Attitude:** open `Attitude` and `SpacecraftGeometry` contracts compose
  caller-selected representations into a `SpacecraftView`; optional built-in
  quaternion attitude and standard geometry implementations are separately
  feature-gated in `core`.
- **Measurements:** an object-safe `Measurement` trait composes heterogeneous
  observations without erasing their dimensions. Concrete range, range-rate,
  azimuth/elevation, and Doppler implementations each retain an explicit
  signal path, epoch, frame, typed values, and an explicit known-or-unknown
  error; no shared observation-context object or time tag can assign metadata
  to another measurement.
  One-value observations use a scalar standard error; multi-value observations
  accept a typed positive-definite covariance matrix and retain only its lower
  triangular Cholesky matrix. Construction validates finite, exactly symmetric
  input through strict factorization without an arbitrary tolerance. Generic
  correction chains apply only to their matching implementation, with typed
  additive corrections and retained open physical-provenance types. Known scalar
  errors combine by root-sum-of-squares and stored lower matrices as
  `L₁L₁ᵀ + L₂L₂ᵀ`, while an unknown input remains unknown.
  Built-in observable families and correction-provenance markers each have an
  explicit `measurements` feature, so `default-features = false` can retain
  only application-selected implementations. Ground-observation data types
  include azimuth/elevation, right ascension/declination, range/range-rate,
  Doppler, bistatic and turn-around range, TDOA, FDOA, and carrier phase;
  `MeasurementEstimator<M>` composes predictions for one concrete measurement
  type with a caller-selected `ParticipantStateProvider`; fixed
  `GroundStationProvider` values and generic composite providers link one or
  many stations with application-owned spacecraft ephemerides. The optional,
  per-observable instantaneous-geometric implementations retain the requested
  epoch and frame, require the provider to make frame resolution explicit, and
  mark predicted uncertainty unknown. A single `CorrectionModelChain<M, C>`
  has an optional local, frame-qualified spacetime propagation-gradient field
  and value-domain effects. `SignalPropagationSolver<M, C>` evaluates and
  integrates the correction field over each signal leg to derive a
  `SignalEventTimeline` while preserving the measurement's reported epoch;
  corrections never set epochs directly. This makes
  physical media or relativistic propagation delay correction behavior rather
  than clock bias or a second model family. The default instantaneous estimator
  uses the reported epoch; a multi-leg light-time estimator may use every event
  epoch. Value-domain corrections must preserve the reported epoch.
  Force models remain within the state provider and its propagator rather than
  creating a measurements-to-dynamics dependency. The feature-gated
  `VacuumLightTimeSolver` resolves every path leg backward from the reported
  reception epoch with the exact vacuum light speed and one midpoint sample of
  the correction-gradient field; its fixed-point tolerance is explicitly
  configured and it reports non-convergence. `frames::FrameKinematics` and a
  `KinematicFrameTransformProvider` make transformation epochs, data, and
  output frames explicit; the measurements adapter never relabels coordinates.
  The concrete verified IERS 2010 GCRF/ITRF2020 provider can satisfy this
  boundary; displacement, weather, higher-order media integration, turnaround
  delay, and physical correction models remain separate implementations.
  `GroundStation` owns a parent-relative fixed frame; geodetic conversion,
  displacement, topocentric-frame construction,
  clocks, weather inputs, light-time solving, and physical correction-model
  evaluation remain future contracts. A ground observer is not a separate
  top-level domain or crate.
- **Estimation:** parameters, residuals, least squares, filters, covariance,
  and state-transition/sensitivity machinery.

### Edges

- **I/O:** standards and file formats translate to domain types; domain types
  never depend on parsers.
- **Facade:** a curated `orskit` crate re-exports core contracts by default and
  concrete capabilities only through explicit feature gates.
- **Bindings:** Python and JVM adapters translate errors, ownership, arrays,
  callbacks, and asynchronous work without reimplementing physics.

## Core data contracts

- `SpacecraftState` is an open, frame-qualified contract. `Orbit<S>` is bound
  to `S: SpacecraftState`, so every epoch-qualified orbit is a valid spacecraft
  state. Concrete representations are selected through the feature-gated
  `orbits` crate (`orbits::cartesian`, `orbits::keplerian`, and
  `orbits::equinoctial` today); applications can provide another implementation
  without changing core APIs. There is no closed convenience enum in a public
  contract.
- `Orbit<S>` composes an epoch with its caller-selected state representation.
  `Spacecraft` contains time-independent identity, an opaque spacecraft-owned
  non-inertial body-frame capability, and body geometry.
  `SpacecraftView<S, G, A>` borrows it while owning an `Orbit<S>`, an open
  caller-selected `A: Attitude`, positive mass, and framed inertia; `G:
  SpacecraftGeometry` identifies the borrowed spacecraft geometry. The
  optional `QuaternionAttitude` implementation remains available as the
  compatibility alias `AttitudeState` when the `quaternion-attitude` feature
  is selected.
  Position, velocity, acceleration, orientation, inertia tensor, covariance,
  and every other coordinate-dependent value carries the frame information
  needed to interpret it. Cartesian coordinates remain gravity-independent;
  osculating element alternatives share an application-extensible
  `Arc<dyn gravity::CentralGravityProvider>` binding origin and parameter.
  The provider is selected by the application or a feature in the dedicated
  `gravity` crate; lower-level
  format coordinates may remain separate until validated into that state.
- File formats that omit physical properties yield values such as
  `CoordinateSample<CartesianCoordinates>`, not fabricated complete states.
  Callers enrich them into an explicit `Spacecraft` at the workflow boundary.
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
  narrowing what a `SpacecraftState` may represent. Unknown or future frame
  orientations never become inertial merely because they are absent from a
  non-inertial blacklist.
- External scientific data enters through a verified `orskit-data` artifact
  and an immutable trait-backed provider. Artifact authority, product, version,
  SHA-256 content digest, coverage, and caller-selected allocation limit are
  explicit; algorithms declare their required data and perform no implicit
  network or cache lookup.
- Constants identify their convention and source; there is no anonymous
  "Earth constant" shared across incompatible models.
- Covariance and Jacobian values identify their parameterization, ordering,
  frame, and units.

## API strategy

- Prefer small cohesive types, enums for closed physical alternatives, and
  traits for genuine extension points.
- Present one high-level, domain-oriented API to users. Algorithms may use
  vector/matrix kernels internally for performance, but those kernels are not
  a second supported public surface.
- Use builders when construction has many optional model choices; validate at
  construction so propagation does not repeatedly discover configuration
  errors.
- Separate immutable model configuration from mutable integration workspace.
- Make expensive allocation and data loading observable to callers.
- Prefer borrowed access and ownership transfer over cloned return values. Use
  standard traits for conversions and access when they express the operation;
  do not add public convenience wrappers that duplicate them.
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

Focused Cargo packages retain concise domain names. The public facade is the
`orskit` package; internal implementation packages use names such as `orbits`,
`gravity`, and the `dynamics/two-bodies` sub-crate because the workspace already establishes
their provenance. Rust's built-in `core` crate remains reserved, so the core
contract library is imported internally as `orskit_core` and re-exported to
facade users as `orskit::core`.
Split crates only when the domain boundary and dependency direction are clear;
do not create one crate per noun pre-emptively.

Likely long-term focused packages include time/data, frames/bodies, orbits,
propagation/forces/events, attitude, measurements/estimation, I/O, and FFI.
The exact split requires architecture decision records and evidence from real
vertical slices.

The initial split includes `units` for typed quantities, `bodies` for celestial
and body-system identities, `frames` for body-backed frame identity,
implementation-neutral `core` contracts, the feature-gated `orbits` crate for
Cartesian, elliptic circular, elliptic Keplerian, and elliptic equinoctial
six-element representations, and the `gravity` crate for independently selected gravity
providers. Its `point-mass` feature is optional; no scientific provenance
record is imposed on application-owned providers.
`lox-frames` is scientifically promising but still alpha; ANISE is mature but
owns a broader almanac/orbit context than this foundational slice needs. Keep
the boundary adapter-friendly and revisit both for transform providers rather
than leaking either API into spacecraft state.

`dynamics/core` describes named spacecraft-state requirements, open
conservative/non-conservative force-model contracts, `ComposedDynamics`, and
the generic `PropagationState<Problem>`/`Propagator<State>` boundary. The
`dynamics` facade exposes those contracts and feature-gates sub-crates.
Concrete `TwoBodyDynamics`, its point-mass model, and `EllipticKeplerPropagator`
live in the opt-in `dynamics/two-bodies` sub-crate. It depends explicitly on the
`orbits` Cartesian feature. The
solver accepts a target epoch, derives its internal duration, resolves an
epoch-qualified `Orbit<S>` into Cartesian state, advances it with universal
variables, then restores and returns the same selected representation.
Cartesian state is the shared non-singular resolved representation except at
physical zero-radius collision; caller-selected element charts retain their
own conversion singularities.
Complete spacecraft views are composed separately from properties known to be
valid at the propagated epoch.
The `tle` crate is an operational-format boundary: it validates and formats
TLE records without owning propagation. Its optional adapter converts validated
columns into an epoch-qualified `Sgp4Elements` domain state. The separate
`dynamics` crate's gated `sgp4` module owns a stateless, non-configurable
`Propagator<Sgp4Elements, CartesianState>` backed by an unmodified black-box
dependency configured for WGS-72/AFSPC compatibility. It returns the existing
typed Cartesian orbit in explicit TEME axes. TLE age policy, frame conversion,
covariance, maneuvers, and operational accuracy remain outside that propagator.
General force evaluation remains distinct from the first caller-implemented
Cartesian acceleration boundary. Numerical propagation currently covers
adaptive translational Cartesian state with optional immutable dense output
and bounded event localization; coupled rotational, mass, variational,
reset/reintegration, and grazing/multiple-root capabilities remain deferred.
Third-body descriptions remain unavailable until their ephemeris,
frame, provenance, and acceleration-assembly contracts exist.
There is no `stations` crate: ground and spacecraft participants belong to the
measurement topology and estimation workflows.
