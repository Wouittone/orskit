# Task: audit Rust API quality and gate numerical propagation

## Parity target

- Ledger row: foundations / units, dimensions, constants, and numerical
  policies; propagation / numerical integration and dense ephemerides; events
  and root localization
- Current status: foundations `Partial`; numerical propagation and events `Not
  assessed`
- Intended status after this task: foundations remain `Partial`; numerical
  propagation becomes `Designed`; events remain `Not assessed`

## User workflow

A Rust caller receives source-preserving domain errors, compiler guidance when
an owned result is accidentally discarded, and exhaustive matching for enums
that intentionally describe a closed set. A contributor implementing numerical
propagation has a dependency-ordered contract and validation gate, without a
premature public ODE API or unsupported propagation claim.

## Scientific contract

- Inputs and units: P11 accepts an epoch-qualified, frame-qualified state and
  typed absolute tolerances per physical component plus a dimensionless
  relative tolerance. Step bounds use `hifitime::Duration`.
- Outputs and units: propagation restores the caller's domain state; raw SI
  component arrays remain private to the integration kernel.
- Frames/epochs/time scales: the problem validates frame compatibility before
  integration; Hifitime epochs remain public and elapsed integration duration
  must use a uniform scale rather than UTC arithmetic.
- Conventions and valid regimes: the first slice is translational Cartesian
  propagation. Unsupported mass, attitude, inertia, or angular-velocity model
  requirements fail during construction rather than being silently omitted.
- External data requirements: every evaluable model owns or borrows its
  immutable providers; no ambient data or network access is permitted.
- Errors and singularities: invalid tolerances and step bounds, unsupported
  state requirements, non-finite derivatives, step underflow, exhausted step
  or rejection limits, provider failures, and event-function failures are typed
  recoverable errors with source chaining where applicable.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| Existing orskit propagation contracts and ADR-0033 | Original project architecture | A propagator owns its physical problem and resolves/restores the caller-selected state | ADR-0037 |
| Existing `ENGINEERING.md` numerical policy | Original project policy | Typed quantities, explicit tolerances, deterministic behavior, and independent validation are mandatory | ADR-0037 |

No Runge--Kutta tableau, dense-output polynomial, third-party implementation,
or external test vector was consulted or selected in this design slice. P11
must add the exact primary numerical reference and coefficient provenance
before implementing a method.

## Design

- Affected crates/layers: repository engineering policy; closed public enums
  in `frames`, `ccsds`, and `measurements`; `orbit-determination` bounds;
  future `dynamics` numerical implementation
- Public API: eight intentionally closed enums now permit exhaustive matching;
  no numerical propagation API is added
- Rejected alternatives: blanket removal or addition of `#[non_exhaustive]`;
  annotating every `Result` constructor despite `Result` already being
  `must_use`; unstable trait aliases; a generic public `Vec<f64>` ODE boundary;
  an RK kernel detached from an evaluable physical problem
- ADR required: yes, ADR-0037

### Q10 error audit

- Public recoverable paths use named errors. Existing contextual wrappers in
  frames, measurements, OD, two-body propagation, and OEM retain their
  `#[source]`/transparent chains.
- OEM epoch decoding was the one source-erasing boundary found: `InvalidEpoch`
  now retains Hifitime's parsing error and has a regression test.
- `Box<dyn Error + Send + Sync>` remains only at application-selected
  propagator/model extension boundaries where a single concrete error type
  cannot be named.
- Closed constructor/conversion errors in `units`, `core`, and `orbits` remain
  exhaustive; evolving workflow/parser/provider errors remain
  `#[non_exhaustive]`. Unit parse-error structs remain closed.

### Q11 `non_exhaustive` audit

Removed `#[non_exhaustive]` where adding a variant would change a deliberately
closed meaning and downstream exhaustive handling should fail to compile:

- `FrameMotion` (affirmative inertial, non-inertial, or unspecified);
- `ReferenceDataDescriptorField` (the fields of one concrete descriptor);
- `OemSection`, `CartesianCovarianceEntry`, and `OemRecordRef` (closed OEM KVN
  structure/dimensional categories); and
- `RangeConvention`, `AzimuthElevationConvention`, and
  `RightAscensionDeclinationConvention` (the conventions currently accepted by
  their corresponding validated measurement types).

Retained `#[non_exhaustive]` on evolving identity/catalog enums
(`FrameOrigin`, `FrameOrientation`, `BodyKind`), supported-protocol catalogs
(`OemTimeSystem`, `OemLimitKind`, `OemEvent`), and every currently marked public
workflow/parser/provider error. Those boundaries can gain capabilities or
failure modes without changing the meaning of existing variants.

### Q12 `must_use` audit

Clippy's `must_use_candidate` audit found two omissions: the infallible
`GroundStation::new` constructor and `OemSegment::records` iterator. Both are
now annotated. Existing constructors returning `Result` need no redundant
annotation, mutating `push` methods must remain discardable, and borrowed scalar
getters were already annotated. The lint is now denied by the documented and CI
Clippy command.

### Q13 bound audit

Four OD filter implementations repeated `Debug + Send + Sync` after
`Propagator<CartesianState>`, whose super-traits already impose those bounds.
The repetitions were removed. Remaining bounds state independent requirements;
introducing a helper trait would hide diagnostics rather than improve them, so
no trait alias or new super-trait was added.

## Validation

- Unit cases: OEM invalid-epoch error retains a source
- Invariants/properties: the stricter Clippy invocation reports every future
  unannotated `must_use` candidate; existing enum and filter tests cover the
  unchanged semantics after attribute/bound cleanup
- Independent reference vectors: required by P11 before implementation; none
  claimed by this design-only slice
- Differential/scenario tests: P11 requires analytic constant-derivative and
  convergence-order cases, two-body comparison against the independently
  implemented analytical propagator, and an external provenance-cleared
  scenario; P12 adds analytic dense-output/event-root cases
- Tolerances and justification: ADR-0037 defines the typed scaling contract;
  P11 must choose physical defaults only from a named scenario/error budget
- Benchmarks: deferred until the algorithm and correctness evidence exist

## Numerical implementation gate

No RK kernel is implemented in this task. The current `SystemDynamics` and
`ForceModel` traits are descriptions, not derivative evaluators; no accepted
coupled-state layout exists; T11 validation evidence is not present on this
branch; and no embedded pair or continuous extension has been selected with
primary-reference provenance. P11 may start only after those inputs are merged
or completed in its own vertical slice.

## Completion checklist

- [x] Error policy documented and source chains audited
- [x] One source-erasing error boundary fixed and tested
- [x] `#[non_exhaustive]` audited case by case
- [x] `#[must_use]` candidates audited and enforced
- [x] Repeated trait bounds simplified on stable Rust
- [x] Numerical propagation contract and design gate recorded
- [x] Rustdoc/examples unaffected; task and ADR document the public intent
- [x] Provenance recorded
- [x] Parity ledger updated without an implementation claim
- [x] Relevant checks pass
- [x] Bindings explicitly untouched and deferred
