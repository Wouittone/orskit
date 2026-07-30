# ADR-0040: prescribe attitude before rotational dynamics

- Status: Accepted
- Date: 2026-07-30
- Owners: orskit maintainers
- Affected parity rows: attitude providers and interpolation;
  attitude-dependent forces; mass-qualified maneuvers
- Extends: ADR-0034 and ADR-0038

## Context

The core already owns framed orientation, body angular velocity, inertia,
spacecraft/body identity, and the open `Attitude` representation contract.
It does not own an epoch-dependent provider. The numerical kernel propagates
Cartesian translation and has no accepted quaternion/angular-rate state
layout, rotational tolerance, torque evaluator, or dense attitude extension.

P15 deliberately rejects body-fixed thrust because a force direction cannot
be interpreted in Cartesian axes without attitude. A useful POC can resolve
prescribed attitudes without prematurely claiming torque-driven rotational
dynamics.

## Decision

1. The dedicated `attitude` crate owns the open `AttitudeProvider<S>` contract
   and built-in providers. `core` retains representation and frame invariants.
   A provider consumes an epoch-qualified `Orbit<S>`, owns every law/sample/data
   dependency, and returns an owned `Attitude`.
2. `FixedAttitudeProvider` accepts only zero body angular velocity and returns
   one constant attitude after checking its reference frame against the orbit.
   `TabulatedAttitudeProvider` requires a
   non-empty, strictly increasing table with common source/target frames and
   one opaque body ownership capability. Its coverage is closed and it never
   extrapolates.
3. Tabulated orientation uses shortest-arc unit-quaternion SLERP. Equivalent
   quaternion signs do not select a long or spurious rotation. Body angular
   velocity is linearly interpolated component-wise in the common body axes.
   This first provider does not claim derivative consistency between the SLERP
   curve and the independently supplied angular-rate samples.
4. `Orientation` exposes only domain-level framed operations needed by the
   workflow: checked SLERP and typed force rotation. It does not expose a raw
   rotation matrix as a second numerical API.
5. Finite thrust explicitly selects `ThrustFrame::Reference` or
   `ThrustFrame::Body`. The existing propagation entry point accepts only
   reference-frame burns and returns a typed provider-required error for body
   burns. The opt-in attitude-aware entry point requires a provider.
6. During every Runge--Kutta stage of an active body burn, the numerical
   adapter constructs that stage's epoch-qualified Cartesian orbit, evaluates
   the provider, verifies body ownership and target frame, rotates the typed
   force, and adds `F/m(t)` to base acceleration. Provider errors remain
   source-preserving.
7. Fixed and tabulated providers require no external dataset. Provider
   implementations needing ephemerides, frame transforms, or guidance laws
   must own those explicit dependencies in later slices.
8. Quaternion/angular-rate integration, torque providers, pointing laws,
   derivative-consistent higher-order attitude interpolation, attitude
   ephemerides, body-fixed impulses, and coupled attitude/mass/variational
   states remain deferred.

## Alternatives considered

- Put providers in `core`: rejected because `core` is the
  representation-neutral contract layer and concrete provider selection
  belongs in a focused implementation crate.
- Infer an attitude from the orbit: rejected because no universal pointing law
  exists and doing so would hide a physical model.
- Add a raw quaternion callback to the maneuver API: rejected because it loses
  body ownership, reference frame, angular state, coverage, and typed errors.
- Integrate rotational dynamics immediately: rejected because quaternion
  manifold error control, torque evaluation, inertia evolution, and dense
  interpolation need an independently evidenced contract.
- Extrapolate the nearest table interval: rejected because attitude outside
  declared coverage is a data failure, not an interpolation result.

## Consequences

The POC can execute genuine attitude-dependent force evaluation and reuse the
P15 mass/audit workflow without changing the public Cartesian propagator state.
Applications can implement their own providers now. Full P16 rotational
dynamics remains a separate vertical slice rather than being implied by a
prescribed attitude.

## Validation

Provider tests cover frame checks, table ordering, closed coverage, shortest
arc/sign equivalence, midpoint rotation, and angular-rate interpolation.
Numerical tests rotate body `+x` thrust through a fixed 90-degree attitude,
compare with the existing analytic variable-mass velocity budget, count
provider evaluations across Runge--Kutta stages, recover the state in reverse,
and retain a tabulated-coverage failure as the nested source. A runnable
body-fixed maneuver example crosses the provider, orientation, maneuver, mass,
and numerical layers.

## Provenance

Ken Shoemake, *Animating Rotation with Quaternion Curves*, Computer Graphics
19(3), 1985, DOI 10.1145/325334.325242, establishes spherical interpolation
of unit quaternions for rotations. Only the public interpolation concept and
equations were consulted. No source code, tests, examples, figures, or
distinctive prose were copied. The existing `nalgebra` dependency supplies the
unmodified numerical quaternion kernel.
