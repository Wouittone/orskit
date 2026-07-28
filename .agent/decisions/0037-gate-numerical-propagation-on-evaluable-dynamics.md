# ADR-0037: gate numerical propagation on evaluable dynamics

- Status: Accepted; dense-output item 5 superseded by ADR-0044
- Date: 2026-07-21
- Owners: orskit maintainers
- Affected parity rows: numerical integration and dense ephemerides; dynamics
  and force-model composition; events and root localization

## Context

`Propagator<State>` already establishes that a concrete propagator owns one
physical problem, and `PropagationState<Problem>` resolves and restores the
caller's representation. The current `SystemDynamics` and `ForceModel` traits
describe force composition and required spacecraft properties, but deliberately
do not evaluate derivatives. There is also no accepted layout for coupled
translation, mass, attitude, variational, or covariance components.

An adaptive integrator cannot be correct in isolation from those boundaries.
Publishing a raw `Vec<f64>` ODE interface would lose units, frames, component
identity, and model requirements. Implementing a stepping kernel before
selecting a documented embedded method and continuous extension would also
make later dense output and events either inconsistent or duplicative.

## Decision

1. A numerical propagator continues to implement `Propagator<State>` and owns
   its immutable evaluable physical problem. The evaluable problem is distinct
   from descriptive `SystemDynamics`: it validates topology/data coverage and
   computes derivatives for a declared resolved-state layout.
2. State adaptation owns the mapping between typed domain state and a private,
   contiguous SI integration layout. Public APIs never accept a naked numeric
   vector. Each component group declares its physical dimension and required
   absolute tolerance. The first vertical slice supports Cartesian position
   and velocity only; model requirements for mass, attitude, angular velocity,
   or inertia are rejected at construction until those groups are designed.
3. Configuration uses finite, strictly positive typed absolute tolerances
   (`Length` for position and `Velocity` for velocity), a finite positive
   dimensionless relative tolerance, typed minimum/maximum/initial
   `hifitime::Duration` bounds, and finite step/rejection limits. A candidate
   step is accepted from an embedded local-error estimate using component-wise
   scales `absolute + relative * max(|initial|, |candidate|)` and one documented
   aggregate norm. P11 must state the pair, orders, tableau, controller,
   safety/clamp factors, and local-versus-global error interpretation.
4. Forward and backward propagation are supported. Accepted steps may not
   cross the target epoch; the final result is evaluated at exactly the target.
   Rejected steps do not mutate accepted state or observable solver output.
   Identical inputs, configuration, providers, and target produce the same
   step decisions on one supported platform/toolchain, subject to documented
   floating-point caveats.
5. Superseded by ADR-0044. P11 implements endpoint propagation without a dense
   extension. P12 owns selection and validation of a continuous extension
   before exposing dense ephemerides or event integration.
6. Event detectors in P12 evaluate typed states obtained from dense output.
   A detector owns any physical normalization needed to return a finite signed,
   dimensionless switching value. Direction is a closed rising/falling/any
   choice. Root localization is bracketed within accepted steps; simultaneous
   roots are ordered first by propagation time and then detector registration
   order, with reverse propagation using its propagation-time ordering.
   Handlers and stopping/reset semantics remain a P12 API decision.
7. Recoverable failures are typed: invalid configuration, unsupported state or
   model requirements, frame/origin/data incompatibility, provider failure,
   non-finite state/derivative/error estimate, minimum-step exhaustion,
   step/rejection-limit exhaustion, and dense/event evaluation failure.
   Provider and state-adaptation errors retain their `Error::source`; the
   numerical kernel does not panic for these conditions.
8. P11 cannot begin with a public implementation until it selects the exact
   embedded method and dense extension from a primary numerical reference,
   records coefficient provenance, and supplies validation independent from
   the new kernel. Required evidence includes analytic polynomial or
   manufactured solutions, observed convergence order, forward/backward and
   rejection cases, conservation/error-budget comparison with the existing
   analytical two-body solver, and at least one provenance-cleared external
   scenario. P12 adds dense interpolation and analytic event-root evidence.

## Alternatives considered

- Add a generic public ODE solver over `Vec<f64>`: rejected because it creates
  a second untyped scientific API and cannot enforce state/model compatibility.
- Treat descriptive `ForceModel` values as derivative evaluators: rejected
  because description, provider/data resolution, and numerical evaluation have
  different responsibilities and error contracts.
- Implement a translational RK kernel now and attach dense output later:
  rejected because the continuous extension is part of method selection and
  event correctness, not an interchangeable afterthought.
- Design every coupled component before translation: rejected because it would
  delay a useful vertical slice and invent contracts for mass/attitude/STM
  propagation without their model and validation requirements.
- Use an unstable trait alias for repeated evaluator bounds: rejected because
  the workspace targets stable Rust and a named semantic trait is clearer.

## Consequences

- P11 has a small first slice but cannot bypass units, frames, data, or error
  scaling to get a generic integrator compiling quickly.
- State layout and derivative evaluation require one more accepted contract
  before implementation. This is deliberate: current force traits remain
  honest descriptions rather than placeholder numerical models.
- Dense output is designed with the embedded method, while public ephemerides,
  event handlers, and simultaneous-event behavior remain independently testable
  P12 capability.
- Later component groups can extend the private layout and tolerance schema
  without changing the high-level `Propagator<State>` call shape.

## Validation

P11 must test configuration failures, exact target handling, forward/backward
integration, accepted/rejected-step state transitions, non-finite derivative
failures, step/iteration exhaustion, analytic solutions, observed method order,
and a two-body accuracy/conservation budget. P12 must test dense endpoints and
interior accuracy, rising/falling/grazing behavior, roots at step boundaries,
and deterministic simultaneous roots. No implementation or validation claim is
made by this ADR.

## Provenance

This decision uses original orskit architecture in `ARCHITECTURE.md`,
`ENGINEERING.md`, ADR-0033, and the external-review roadmap. No external source,
implementation, coefficient table, or test vector was consulted. P11 must
record its primary numerical references and any independent validation data in
`PROVENANCE.md` before adding a kernel.
