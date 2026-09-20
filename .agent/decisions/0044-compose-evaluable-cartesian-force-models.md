# ADR-0044: compose evaluable Cartesian force models

- Status: Accepted
- Date: 2026-09-20
- Owners: dynamics/propagation maintainers
- Affected parity rows: Propagation / dynamics and force-model composition

## Context

`dynamics-core` already describes force composition through `ForceModel`,
`ConservativeForceModel`/`NonConservativeForceModel`, and the purely
descriptive `ComposedDynamics` (ADR-0006, ADR-0008); none of these expose an
evaluation method. Concrete Cartesian topologies (`TwoBodyDynamics`,
`J2Dynamics`) each implement `CartesianDynamics` directly as one strict,
named, hand-written system (ADR-0017, ADR-0042). Adding a third topology
(for example third-body, drag, or radiation pressure) still requires a new
hand-written struct that sums its own hard-coded set of terms; ADR-0042
explicitly deferred "how heterogeneous typed force models combine their
accelerations, error behavior, and partial derivatives without an unsafe
downcast" to this issue (GitHub Issue #10).

An evaluation contract usable across heterogeneous, independently typed
force-model implementations must be object-safe: callers need one ordered,
homogeneous collection (`Vec<Arc<dyn Trait>>`) holding arbitrary concrete
model types. Associated `Error` types on `CartesianDynamics` make that trait
itself unsuitable as the element type of such a collection once model types
differ, because a trait object fixes one concrete `Error` type. Composed
Jacobian evaluation is additionally unsafe to offer unconditionally: if only
some constituent models can supply an acceleration Jacobian, silently
omitting the rest produces a Jacobian that does not correspond to the actual
summed acceleration function.

## Decision

1. Add `EvaluableCartesianForceModel: ForceModel`, an object-safe contract
   with `validate_cartesian` and `cartesian_acceleration` methods matching
   `CartesianDynamics`'s state/epoch/frame shape. Its error type is
   `CartesianForceModelError`, a struct that erases a model's typed failure
   behind `Box<dyn std::error::Error + Send + Sync>` while preserving the
   failing model's name and the original error as `Error::source`. This
   follows the project's existing domain-error policy: an error is erased
   only at a genuine object-safe boundary, matching the precedent already
   used by `measurements::CorrectionModelError` and
   `orbit_determination::OrbitDeterminationError::Propagation`.
2. Add `EvaluableCartesianVariationalForceModel: EvaluableCartesianForceModel`
   with a `cartesian_acceleration_jacobian` method, implemented only by
   models that can honestly supply partials for every state they accept.
3. Add `ComposedCartesianDynamics`: an ordered `Vec` of
   `EvaluableCartesianForceModelHandle` implementing `CartesianDynamics`.
   Validation and evaluation iterate the vector in declaration order and sum
   accelerations with the same typed `AccelerationVector` addition used
   elsewhere; the first failing model's error is returned immediately
   (wrapped in `CartesianForceModelError`, not retried, not reordered). A
   model returning an acceleration in a different frame than the input state,
   or an empty composition, is a distinct typed error.
4. Add `ComposedCartesianVariationalDynamics`: the same ordered-composition
   shape, but its `Vec` holds
   `EvaluableCartesianVariationalForceModelHandle`, so every constituent
   model is guaranteed Jacobian-capable. It implements both
   `CartesianDynamics` and `CartesianVariationalDynamics`, summing
   accelerations and acceleration Jacobians component-wise. This is the
   "where architecture permits" half of the contract: a composed Jacobian is
   only ever produced when every summed contribution actually supplies one,
   never by silently skipping non-variational models.
5. Add `CartesianDynamicsForceModel<D>`, a generic adapter from any existing
   whole `D: CartesianDynamics` (and, where `D: CartesianVariationalDynamics`,
   also Jacobian-capable) into one named, described
   `EvaluableCartesianForceModel` contribution. This lets an existing strict
   topology such as `TwoBodyDynamics` or `J2Dynamics` participate in a
   composed system as a single ordered contribution without changing its own
   public API, and is the only supported way `dynamics-core` composes an
   external whole system today.
6. `TwoBodyDynamics` and `J2Dynamics` are unchanged: neither their public API
   nor their evaluated values are touched by this ADR.

## Alternatives considered

- **Give `CartesianDynamics` a default-erased error variant instead of adding
  a parallel evaluable contract.** Rejected: it would force every existing
  and future strict topology to erase its own precise typed error even when
  used alone, defeating the typed-error policy for the common single-system
  case.
- **Classify composed models into conservative/non-conservative buckets, like
  `SystemDynamics`/`ComposedDynamics`.** Rejected for this slice: the issue
  asks for "one ordered collection", classification adds combinatorial marker
  traits and handle types without changing how accelerations sum, and the
  existing descriptive `ComposedDynamics` already owns that classification
  role. A future ADR may revisit this if conservation-checking or
  accumulation-policy features need it.
- **Let a composed system produce a Jacobian by summing only the constituent
  models that support it, ignoring the rest.** Rejected: this silently
  produces a Jacobian inconsistent with the actual summed acceleration
  function, which is worse than refusing to compile a mixed composition at
  all. `ComposedCartesianVariationalDynamics`'s separate, stricter handle
  type makes this a compile-time impossibility instead of a numerical
  correctness bug.
- **Implement `EvaluableCartesianForceModel` directly on the existing
  individual model types (`PointMassGravityModel`, `J2GravityModel`) in their
  home crates (`dynamics-two-bodies`, `dynamics-harmonics`).** Deferred: this
  would let composition combine individual atomic terms (rather than whole
  strict topologies) and is likely valuable future work, but it is additive
  source-code work in crates outside this issue's `dynamics-core`
  composition-ownership boundary. The generic `CartesianDynamicsForceModel<D>`
  adapter delivers the ordered, object-safe composition contract now without
  that additional, separately reviewable change.
- **Add a dev-dependency from `dynamics-core` back onto `dynamics-two-bodies`
  and `dynamics-harmonics` to validate composition against the literal
  `TwoBodyDynamics`/`J2Dynamics` types.** Rejected after attempting it: both
  crates depend on `dynamics-core` as a normal dependency, and Cargo compiles
  a second, type-incompatible copy of `dynamics-core` across that cyclic
  edge (its `CartesianDynamics` trait is a distinct type from the one under
  test), so `TwoBodyDynamics: CartesianDynamics` fails to resolve inside
  `dynamics-core`'s own test binary. Validation instead uses independently
  coded test doubles reproducing the same public closed-form point-mass and
  `J2` equations already used and cited by `dynamics-two-bodies` and
  `dynamics-harmonics`.

## Consequences

- Future force families (third-body, drag, radiation pressure) can be added
  as new `EvaluableCartesianForceModel` implementations and combined with
  existing systems through `ComposedCartesianDynamics` without a new
  hand-written strict-topology struct for every combination.
- Composition failures are always typed and always name the failing model;
  no `String`/opaque failure is introduced, and no failure is coupled to a
  specific numerical integrator.
- A composed Jacobian is only ever available when every constituent model
  supports it; adding a single non-variational model to a variational
  composition is a compile-time type error, not a silent numerical omission.
- Actual per-model composition (as opposed to whole-system wrapping) for
  `PointMassGravityModel`/`J2GravityModel` remains future work in their own
  crates, tracked as a follow-up rather than bundled into this ADR.

## Validation

`crates/dynamics/core/src/evaluable.rs` tests cover: wrapping a whole
`CartesianDynamics` system preserves its direct acceleration; composing a
point-mass model and an independently coded `J2` correction model reproduces
the same closed-form sum `J2Dynamics` computes; two-model declaration order
does not change the summed result; repeated evaluation is deterministic;
mixed conservative (point mass) and non-conservative (linear drag) models sum
correctly; a failing model surfaces a named, sourced typed error; an empty
composition is rejected; a model returning an acceleration in the wrong frame
is rejected; and a composed variational system sums both accelerations and
acceleration Jacobians against an independently computed analytic reference.

## Provenance

This is an original architecture decision based on ADR-0006, ADR-0008,
ADR-0017, ADR-0042, and the project's domain-error policy in
`ENGINEERING.md`. The point-mass and zonal `J2` closed-form equations used
only in test doubles are the same public equations (NASA Technical
Memorandum 2004-213230; NASA GMAT *Mathematical Specifications*, 2007)
already cited by `dynamics-two-bodies` and `dynamics-harmonics`. No
third-party implementation or test material was consulted.