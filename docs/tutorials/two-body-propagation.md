# Elliptic two-body propagation

This tutorial advances one Earth-centered Cartesian orbit by 900 seconds with
orskit's current analytical two-body solver. The complete source is the
compiled [`two_body_propagation` example](../../crates/dynamics/two-bodies/examples/two_body_propagation.rs).

Run it from the repository root:

```powershell
cargo run -p dynamics-two-bodies --example two_body_propagation --features point-mass --locked
```

## Physical contract

The example makes every physical selection at construction:

- position is in metres and velocity is in metres per second;
- the frame is Earth-centered GCRF, whose axes are explicitly inertial;
- the initial epoch is zero seconds on Hifitime's TAI timeline, and the target
  is exactly 900 SI seconds later;
- the application selects Earth as the gravity origin and
  `3.986004418e14 m^3/s^2` as its gravitational parameter, the conventional
  value from [IERS Conventions (2010), Table 1.1](https://iers-conventions.obspm.fr/content/chapter1/icc1.pdf);
  and
- no ephemeris, Earth-orientation data, atmosphere, third body, or online data
  source is used.

`PointMass` is a gravity provider, `PointMassGravityModel` places that provider
in a strict `TwoBodyDynamics` problem, and `EllipticKeplerPropagator` owns the
problem and the analytical solution settings. `propagate` accepts an absolute
target epoch and returns the same state representation at that epoch.

## Valid regime and errors

This solver supports bound elliptic motion under exactly one point mass. A
Cartesian input must have finite coordinates, nonzero radius, negative
specific orbital energy, an explicitly inertial frame, and a frame origin that
matches the gravity provider. It does not support parabolic/hyperbolic motion,
collisions, maneuvers, drag, harmonics, third bodies, mass or attitude
propagation, events, or numerical integration.

Construction and propagation return typed errors. The example uses `?`, so an
invalid gravitational parameter, state, frame/problem combination, solver
configuration, convergence failure, or phase-accuracy-budget failure is
reported rather than silently accepted. The default solver uses a
`1e-13 rad` anomaly residual tolerance, at most 32 Newton iterations, and a
`1e-10 rad` floating phase-error budget. Those numerical settings do not model
the error introduced by the two-body physical approximation.

The printed result is a usage demonstration, not an operational prediction or
independent validation vector. See the [parity ledger](../../.agent/PARITY.md)
for the solver's validation evidence and remaining propagation gaps.
