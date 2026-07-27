# Task: propagate strict TLE records with independently validated SGP4

## Parity target

- Ledger row: Propagation / TLE and SGP4
- Current status: strict TLE format only
- Intended status after this task: Partial, with typed SGP4 propagation and
  explicit TEME output

## User workflow

A caller obtains an epoch-qualified `Sgp4Elements` domain state, directly or
through the optional strict-TLE adapter, then passes it to the stateless
`Sgp4Propagator` through the common `Propagator` trait with a typed target
`Epoch`. The result is an `Orbit<CartesianState>` in `ReferenceFrame::TEME`.

## Scientific contract

- Inputs: validated model-specific `Sgp4Elements` and one target epoch.
- Outputs: metres and metres per second in geocentric TEME.
- Model: the unmodified `sgp4` 2.4 dependency, configured with WGS-72 and its
  AFSPC-compatible epoch, sidereal-time, and propagation modes. The adapter
  accepts the distributed-data ephemeris type `0`; explicit legacy selectors
  for SGP, SGP4, SDP4, SGP8, or SDP8 are rejected until their distinct
  semantics are implemented.
- Time: elapsed SI minutes from the TLE UTC epoch. Across a UTC leap insertion
  this differs from a convention that treats every UTC civil day as exactly
  1,440 minutes.
- Errors: initialization-domain, propagation-divergence, and invalid Cartesian
  output failures remain typed and preserve their sources.
- Limits: no TEME conversion, covariance propagation, maneuvers, catalog
  policy, operational accuracy guarantee, or separate decay classifier.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| Vallado, Crawford, Hujsak, and Kelso, *Revisiting Spacetrack Report #3*, AIAA-2006-6753, Revision 3 | Published technical paper and public verification data | WGS-72/AFSPC compatibility convention and near-Earth/deep-space position/velocity verification vectors | independent acceptance tests; ADR-0046 |
| `sgp4` 2.4 | MIT separately licensed dependency | Public typed initialization/propagation API and declared TEME output units | `crates/tle`; facade feature |

The dependency is used unmodified as a black box. Its implementation source,
tests, and examples are not copied, translated, or used to design project
code. Acceptance values come only from the published Revision 3 material.

## Design

- Keep the project strict parser as the only public TLE format boundary.
- Keep the TLE conversion at the I/O boundary; `dynamics-sgp4` has no
  `TwoLineElement` dependency.
- Implement `Propagator<Sgp4Elements, CartesianState>` with no configurable
  force-model or gravity selection; never invoke the dependency's TLE parser.
- Add no parallel public numeric state type.
- Keep the facade feature opt-in and preserve default-build isolation.
- ADR required: ADR-0046.

## Validation

- Published near-Earth and deep-space verification cases at multiple elapsed
  times, with tolerances stated beside the evidence.
- Exact epoch/frame/unit assertions.
- Unsupported legacy model selection is covered at initialization. An extreme
  but validated drag term and extrapolation interval exercise dependency
  propagation failure and source preservation. Dependency initialization
  wrappers remain defensive because no constructible failure was identified
  after the strict parser's domain checks.
- Default and TLE-enabled feature checks, tests, docs, and strict Clippy.

## Completion checklist

- [x] Implementation and typed errors
- [x] Published verification evidence
- [x] Rustdoc/example
- [x] Provenance ledger updated
- [x] Parity/roadmap updated
- [x] Relevant checks pass
- [x] Binding impact explicitly deferred until the Rust API stabilizes

The unstable core model is intentionally Rust-only. Python and JVM binding
work remains deferred under the project-wide core-stabilization policy.
