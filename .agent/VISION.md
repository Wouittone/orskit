# Vision and scope

## Mission

orskit will be a community-owned, production-grade astrodynamics toolkit
implemented in Rust and dual licensed under MIT or Apache-2.0. It aims for
capability-level feature parity with Orekit while providing a Rust-native API,
stronger invalid-state prevention, predictable data dependencies, and
benchmarked performance.

Python and JVM-language users should receive first-class, idiomatic bindings;
they should not have to understand Rust internals or sacrifice scientific
traceability.

## What feature parity means

Feature parity means that the major mission-analysis workflows represented in
the pinned Orekit baseline can be completed with comparable model coverage and
documented accuracy. It does not mean:

- reproducing Orekit's Java package layout or object model;
- matching undocumented implementation details;
- producing bit-identical floating-point values across different algorithms;
- shipping every upstream feature in a single release; or
- claiming equivalence without traceable validation evidence.

The exact baseline version and capability inventory must be pinned in
`PARITY.md` before a release makes an Orekit-parity claim.

## Product principles

1. **Physics is explicit.** Frames, epochs, time scales, units, conventions,
   and external data are visible in APIs and results.
2. **Correctness is evidenced.** Invariants, standards, analytic cases,
   independent datasets, and differential tests support claims.
3. **Rust shapes the design.** Ownership, traits, enums, newtypes, and typed
   errors are used where they make invalid states harder to represent. Lox is a
   useful Rust ecosystem reference without being an implementation source.
4. **Performance is measured.** Optimizations preserve a stated accuracy
   budget and are supported by reproducible benchmarks.
5. **Bindings are products.** Python and JVM APIs are versioned, tested,
   documented, and idiomatic rather than raw symbol dumps.
6. **Data use is deterministic.** Callers control data versions, loading,
   caching, and offline behavior.
7. **The implementation remains permissively reusable.** Provenance and
   dependencies preserve the project's MIT/Apache-2.0 distribution goal.

## Intended capability families

- precise time systems, calendars, leap seconds, and Earth orientation;
- reference frames, transforms, celestial bodies, geodesy, and ephemerides;
- Cartesian and element-based orbit representations and conversions;
- analytical, semi-analytical, TLE, and numerical propagation;
- gravity, drag, radiation, third-body, relativistic, tide, and maneuver
  models;
- event detection, attitudes, interpolation, covariance, and state-transition
  support;
- participant-centric ground–spacecraft and spacecraft–spacecraft
  measurements, modifiers, orbit determination, and filtering;
- mission geometry, visibility, conjunction-supporting primitives, and
  ephemeris generation;
- common CCSDS and operational astrodynamics data formats; and
- stable Rust, Python, and JVM-language interfaces.

## Non-goals

- A line-by-line port of Orekit, Lox, or Nyx.
- Compatibility with Orekit's Java API at the cost of a coherent Rust API.
- Opaque convenience APIs that silently select frames, time scales, constants,
  models, or online datasets.
- Premature support for every platform, scalar type, accelerator, or embedded
  environment before the scientific contracts are stable.
- Certification for safety-critical flight use without a separate assurance
  program.

## Success criteria

The project is successful when users can build reproducible orbit-analysis and
estimation pipelines in Rust, Python, and JVM languages; understand every
model and dataset involved; validate results against independent evidence; and
do so without accepting a copyleft obligation for their application.
