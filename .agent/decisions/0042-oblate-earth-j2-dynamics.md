# ADR-0042: strict point-mass-plus-J2 Cartesian dynamics as the first oblateness slice

- Status: Accepted
- Date: 2026-09-20
- Owners: propagation/dynamics maintainers
- Affected parity rows: Propagation / gravity fields and solid/ocean tides;
  Propagation / dynamics and force-model composition

## Context

`PARITY.md` records gravity fields as `Not assessed`: orskit has only a
point-mass `CentralGravityProvider`/`PointMassGravityModel`/`TwoBodyDynamics`
triple. `FORCE_MODELS.md`'s suggested implementation order places "point-mass
and third-body attraction, J2 then full spherical harmonics" first. Third-body
attraction needs body ephemerides, which `PARITY.md` records as pending
(`bodies`/`frames` ephemeris work). General spherical harmonics need a
degree/order coefficient provider and a body-fixed frame/rotation supplier,
which also do not exist yet. J2 (the dominant oblateness term) needs neither:
its zonal, axisymmetric acceleration depends only on Cartesian position and an
explicit equatorial radius/J2 coefficient, evaluated directly in the same
inertial frame already used for point-mass dynamics, provided that frame's
polar axis is declared to coincide with the body's oblateness axis (true for
GCRF and Earth's mean rotation axis to the accuracy this term claims).

`dynamics/core` currently exposes only descriptive `ForceModel`/
`ConservativeForceModel` markers (no evaluation method) and a purely
descriptive `ComposedDynamics`; there is no generic "sum evaluable force
models" evaluator yet. `dynamics/two-bodies` instead implements one strict,
named `CartesianDynamics` (`TwoBodyDynamics`) that owns exactly one physical
topology. Building a generic composed-evaluation layer is a larger,
separately scoped task (tracked by GitHub Issue #10) that needs to resolve
how heterogeneous typed force models combine their accelerations, error
behavior, and partial derivatives without an unsafe downcast.

## Decision

Add a second strict, named topology, `J2Dynamics`, in a new
`dynamics/harmonics` sub-crate, following the exact shape of
`TwoBodyDynamics`: it owns exactly one point-mass model and one J2 oblateness
model, and implements `CartesianDynamics` by summing their accelerations
directly (no generic composition layer). It:

- requires an explicit `ZonalGravityProvider` (extends
  `gravity::CentralGravityProvider` with equatorial radius and a dimensionless
  J2 coefficient);
- validates the same inertial-frame and origin-match invariants as
  `TwoBodyDynamics`, plus documents (and, where mechanically checkable,
  asserts) that the frame's declared inertial axes are assumed coincident
  with the body's oblateness axis;
- computes the standard closed-form J2 zonal acceleration term and adds it to
  the existing point-mass acceleration; and
- returns a typed error distinguishing point-mass singularities from a
  non-finite J2 contribution.

This is deliberately not "general spherical harmonics" (no degree/order
input, no tesseral/sectorial terms, no body-fixed rotation) and not a
composed multi-model evaluator. Both remain future work once their
prerequisite data/frame infrastructure and the composition-evaluation
architecture exist.

## Alternatives considered

- **Wait for the generic composed-evaluation architecture before adding any
  second gravity term.** Rejected: that architecture is a larger, separately
  scoped decision (how heterogeneous force models sum accelerations and
  partials without downcasting), and blocking the dominant, most commonly
  requested oblateness correction on it delays real scientific value for no
  correctness gain; `TwoBodyDynamics` already establishes the
  "strict named topology" precedent this follows.
- **Implement full normalized spherical harmonics immediately.** Rejected:
  it requires a degree/order coefficient provider, a body-fixed frame
  transform, and normalization-convention decisions that do not exist yet;
  J2 alone captures the dominant secular perturbation and is independently
  well-documented, letting this slice ship without that infrastructure.
- **Fold J2 into `dynamics/two-bodies` as an optional field on
  `TwoBodyDynamics`.** Rejected: `TwoBodyDynamics` is documented as "exactly
  one point-mass model"; overloading it would blur that contract and force
  every point-mass caller to reason about an oblateness flag. A distinct
  named type keeps both contracts exact.

## Consequences

- A new crate, `dynamics-harmonics`, and a new provider trait,
  `gravity::ZonalGravityProvider`, are added to the public surface, gated by
  Cargo features off by default.
- `J2Dynamics` cannot yet be combined with third-body, drag, or SRP terms
  without hand-writing another strict topology; general composition remains
  tracked in GitHub Issue #10.
- The inertial-axis/oblateness-axis coincidence assumption is a named,
  documented limitation, not silently hidden; general body-fixed rotation is
  deferred, not assumed away permanently.

## Validation

- Independent reference vector: a J2 acceleration value computed from the
  closed-form equation with WGS84-class constants at an explicit test state,
  computed independently of the Rust implementation, checked at a tight
  relative tolerance.
- Invariant tests: J2 acceleration vanishes when the provider's J2
  coefficient is zero (reduces exactly to the existing point-mass term); the
  z-component is odd under negating the state's z-coordinate while the
  horizontal components are unchanged (axisymmetric zonal-term property).
- Typed-error tests: non-inertial frame, origin mismatch, and zero-radius
  singularity are rejected the same way as `TwoBodyDynamics`.

## Provenance

- NASA GMAT *Mathematical Specifications* (2007), already an accepted
  provenance reference for orbit-state work in this project: zonal
  geopotential/J2 acceleration equation and its physical interpretation.
- IERS Conventions (2010), Chapter 6, already cited in `FORCE_MODELS.md`:
  geopotential expansion and tide-free/zero-tide convention warning (recorded
  so the J2 value's convention is stated, not assumed).
