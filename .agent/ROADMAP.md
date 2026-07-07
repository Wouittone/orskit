# Delivery roadmap

The roadmap orders risk; it is not a promise of dates. Milestones should ship
small vertical slices rather than constructing every type before any workflow
works.

## Milestone 0 — trustworthy foundation

- Keep the pinned Orekit baseline and versioned `PARITY.md` inventory current
  as reference releases and evidence change.
- Keep the provenance policy, contributor licensing process, security policy,
  code of conduct, issue templates, and architecture decision records current
  as governance requirements evolve.
- Curate the public `orskit` facade into stable, documented workflows once the
  Rust core contracts settle.
- Establish CI for formatting, Clippy, tests, docs, dependency licenses,
  advisories, MSRV, and native binding smoke tests.
- Define units, time/frame association, error, tolerance, data-context, and
  serialization policies.
- Publish a minimal frame- and unit-safe spacecraft-state workflow with
  traceable validation, without pre-empting the dynamics architecture.

**Exit gate:** a new contributor can reproduce checks offline, trace every
reference, and run one scientifically meaningful scenario without ambiguous
units, frame, or epoch.

## Milestone 1 — time, frames, bodies, and orbit state

- Implement precise instants/durations and initial time-scale conversions.
- Introduce explicit data contexts and versioned leap/Earth-orientation inputs.
- Implement inertial/terrestrial frame transforms and transform composition.
- Implement body/ellipsoid/geodetic primitives.
- Implement epoch/frame-qualified Cartesian and primary orbital element types,
  conversions, anomalies, and interpolation.

**Exit gate:** independently validated state conversion between terrestrial and
inertial frames across representative epochs and orbit regimes.

## Milestone 2 — advanced dynamics and propagation architecture

- Extend the initial description-only system/force-model contracts into
  composable translational, rotational, mass, multi-body, and variational
  evaluation contracts before selecting a resolver.
- Add force-model composition covering gravity models (point mass, harmonics,
  third bodies, relativity, and tides), aerodynamic models, radiation-pressure
  models, and maneuvers. Two-body motion is a validation case, not the
  architecture.
- Add numerical integrator abstraction, dense output, ephemeris generation,
  event detection, and deterministic simultaneous-event handling.
- Add spacecraft mass and initial maneuver support.
- Establish accuracy-plus-performance benchmark scenarios.

**Exit gate:** reproducible LEO and deep-space scenarios with explicit model
data, bounded errors, events, and benchmark baselines.

## Milestone 3 — operational propagators and attitude

- Implement TLE parsing and independently validated SGP4 behavior.
- Add selected analytical and semi-analytical families based on user demand.
- Add attitude state, providers, interpolation, and attitude-dependent force
  models.
- Add variational equations, state-transition matrices, and covariance
  propagation.

**Exit gate:** validated operational propagation workflows and sensitivity
outputs across documented regimes.

## Milestone 4 — observations and estimation

- Extend the initial parent-relative ground-station type into typed
  participant/time-aware models.
- Model ordered participant paths that support ground–spacecraft,
  spacecraft–spacecraft, and multi-leg observations without a separate station
  subsystem or an Orekit-shaped station API.
- Add ground geometry, participant clocks, environmental corrections, and
  major measurement types inside the measurement domain.
- Add measurement generation, parameter selection/scaling, batch least squares,
  and sequential filters.
- Validate with synthetic recovery and independently sourced scenarios.

**Exit gate:** simulate, perturb, and recover an orbit with inspectable residuals
and covariance behavior.

## Milestone 5 — data formats and mission workflows

- Implement prioritized CCSDS messages and operational formats using conformance
  corpora and fuzzing.
- Add visibility, eclipse, occultation, field-of-view, and access workflows.
- Build explicit fetch/cache tooling for public scientific datasets without
  adding implicit network behavior to algorithms.

**Exit gate:** ingest, analyze, estimate, and export a representative mission
scenario with versioned inputs.

## Milestone 6 — first-class bindings

Binding feature work is deliberately deferred while Rust core contracts are
unstable; earlier milestones make only compilation-preserving adapter edits.
Stabilization happens here:

- curate a stable Rust facade;
- provide idiomatic Python classes, arrays, exceptions, packaging, and docs;
- version the native ABI and provide safe JVM wrappers around FFM;
- test cross-language numerical agreement, ownership, threading, and failures;
- publish supported platform and compatibility matrices.

**Exit gate:** the same documented scenario runs from Rust, Python, and a JVM
language with equivalent model choices and tolerance-consistent results.

## Milestone 7 — parity release and 1.0 hardening

- Close or explicitly publish gaps against the pinned Orekit baseline.
- Perform API, safety, dependency, provenance, and numerical assurance reviews.
- Stabilize compatibility, deprecation, data-versioning, and release policies.
- Publish the parity evidence package and reproducible benchmark suite.

**Exit gate:** every claimed capability is `Validated` in `PARITY.md`; remaining
differences are named, scoped, and documented rather than hidden by a percentage.

## Prioritization rule

Within a milestone, choose work by this order:

1. scientific correctness or provenance risk;
2. foundations that unblock multiple validated vertical slices;
3. real user workflows and interoperability;
4. measurable performance bottlenecks; then
5. convenience and breadth.

Do not implement a broad surface of placeholders to make the ledger look full.
