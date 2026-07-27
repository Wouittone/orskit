# ADR-0044: use endpoint-only Fehlberg RK4(5) for the first numerical slice

- Status: Accepted
- Date: 2026-07-24
- Owners: orskit maintainers
- Affected parity rows: numerical integration and dense ephemerides; dynamics
  and force-model composition

## Context

ADR-0037 requires numerical propagation to own an evaluable physical problem,
retain typed state/tolerances, and keep raw integration layouts private. P11
needs a primary-source embedded method, exact target handling, and honest
separation from P12 dense output and events. The descriptive `SystemDynamics`
contract cannot evaluate derivatives, and a generic public ODE vector would
erase units, frames, and component identity.

## Decision

1. The first implementation is an opt-in `dynamics-numerical` crate.
   `CartesianDynamics` is the evaluable boundary: it declares one
   `ReferenceFrame`, declares its state-component requirements, and evaluates
   a typed `AccelerationVector` from an epoch-qualified typed
   `CartesianState`. A model needing mass, attitude, angular velocity, or
   inertia is rejected when the propagator is constructed.
2. `AdaptiveRungeKuttaFehlberg` implements
   `Propagator<CartesianState>` and owns its dynamics. Its six-component SI
   state array is private.
3. The integrator uses Fehlberg RK4(5) formula 2, Table III of NASA TR R-315.
   Its nodes are `(0, 1/4, 3/8, 12/13, 1, 1/2)`. Stage rows are:
   `(1/4)`; `(3/32, 9/32)`; `(1932/2197, -7200/2197, 7296/2197)`;
   `(439/216, -8, 3680/513, -845/4104)`; and
   `(-8/27, 2, -3544/2565, 1859/4104, -11/40)`.
   The fourth-order weights are
   `(25/216, 0, 1408/2565, 2197/4104, -1/5, 0)` and fifth-order weights are
   `(16/135, 0, 6656/12825, 28561/56430, -9/50, 2/55)`.
   The fifth-order estimate is accepted.
4. Local error is the root-mean-square of six component errors divided by
   `absolute + relative * max(|initial|, |candidate|)`. Position and velocity
   have separate typed absolute tolerances. Relative tolerance is
   dimensionless.
5. The next-step factor is `0.9 * error^(-1/5)`, clamped to `[0.2, 5]`.
   Step bounds are positive Hifitime durations; accepted-step and total
   rejected-step limits are explicit non-zero integers. A rejected attempt
   does not modify the accepted state or epoch.
6. Forward and backward steps are signed. No accepted step crosses the target;
   the final remainder is attempted directly and the returned orbit carries
   the exact requested Hifitime epoch. Stage epochs use exact rational
   fractions of Hifitime's integer nanoseconds and truncate a fractional
   nanosecond toward zero symmetrically in either direction. State-stage
   arithmetic still uses the complete signed step in seconds; dynamics with
   meaningful sub-nanosecond time variation are unsupported.
7. This method controls a local embedded error estimate. It does not promise a
   global trajectory-error bound. Determinism holds for identical inputs on
   one supported platform/toolchain, subject to ordinary floating-point
   differences across targets.
8. Contrary to the original ADR-0037 item 5, P11 does not implement even an
   internal dense extension. P12 must select a primary-source continuous
   extension compatible with its endpoint method and validate dense endpoints,
   interiors, and event roots before adding those capabilities.

## Alternatives considered

- Dormand-Prince 5(4) was a viable primary-source method, but the public-domain
  NASA Fehlberg report provides a compact six-stage pair and directly
  satisfies this slice's provenance constraint.
- A generic `Vec<f64>` ODE solver was rejected because it creates an untyped
  parallel API.
- Adding acceleration evaluation to descriptive `ForceModel` was rejected
  because individual force descriptions do not establish a complete,
  frame-compatible evaluable problem.
- Producing dense output now was rejected because P12 explicitly owns the
  continuous extension and event semantics.

## Consequences

Cartesian users can integrate caller-owned acceleration models with explicit
frames, epochs, tolerances, bounds, and limits. Coupled state, other
representations, dense output, events, and global-error guarantees remain
absent. The classical RKF pair is not FSAL and performs six acceleration
evaluations per attempt.

## Validation

Tests cover invalid configuration, unsupported component requirements, exact
forward/backward polynomial solutions, observed fifth-order refinement,
adaptive rejection, frame mismatch, provider-source preservation, non-finite
derivatives, minimum-step/step/rejection exhaustion, and comparison with the
analytical two-body propagator at its separately Orekit-validated scenario.

## Provenance

Erwin Fehlberg, *Low-Order Classical Runge-Kutta Formulas with Stepsize
Control and Their Application to Some Heat Transfer Problems*, NASA TR R-315,
July 1969, especially RK4(5) formula 2 in Table III. NASA NTRS identifies it as
US Government work with public use permitted. Coefficients and the embedded
error concept were used; no source code or third-party implementation was
consulted or copied.
