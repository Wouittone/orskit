# Opt-in fixed-step Gauss-Jackson propagation

Enable `orskit`'s `gauss-jackson` feature (or the same feature on `dynamics` /
`dynamics-numerical`) and explicitly select `GaussJackson8`. The existing
`BogackiShampine32` algorithm, configuration, dense output and defaults do not
change. No external solver is linked.

Construct `GaussJacksonConfiguration` with a positive fixed `Duration`, native
`IntegrationConfiguration` for startup, positive typed `Length` / `Velocity`
corrector-change thresholds, and nonzero correction/step limits. Then construct
`GaussJackson8::new(problem, configuration)` and call the standard
`Propagator<CartesianState>::propagate(initial, target)` contract. The
configuration constructor's rustdoc includes a compiled example. The
[benchmark executable](../../crates/dynamics/numerical/benches/gauss_jackson.rs)
is a complete typed two-body usage example.

## Scientific and lifecycle contract

Only eighth order is supported. Each propagation call creates its own nine-point
acceleration history with eight native Bogacki-Shampine intervals in the
propagation direction. Shorter arcs use only that explicitly configured starter.
The starter's local tolerances must be much tighter than the desired long-arc
error. History is not shared between calls: querying many endpoints separately
repeats startup and propagation.

The mature method integrates Cartesian position in metres, velocity in metres
per second and acceleration in metres per second squared. The complete initial
frame, including origin, is preserved, not relabelled or transformed. Only
affirmatively inertial axes are accepted; the owned dynamics also validates its
origin/data/model compatibility and must return acceleration in that same frame.
Epoch arithmetic uses Hifitime elapsed durations, not UTC calendar differences.
Uniform grid epochs are formed with integer nanoseconds, in either direction.
The target must lie exactly on the initial-epoch grid. There is no partial final
step and no dense output or interpolation. The final orbit carries the exact
requested epoch. Zero duration validates the frame/problem before returning.

Startup, dynamics, non-finite stages and acceleration-frame errors retain the
existing numerical error and its source. Off-grid targets, non-inertial axes,
work-limit exhaustion and corrector nonconvergence have named errors.
Failed corrections do not produce successful predictor-only results.

## Method, error and stability

Berry and Healy, *Implementation of Gauss-Jackson Integration for Orbit
Propagation*, J. Astronautical Sciences 52(3), 2004, pp. 331-357,
[public paper](https://hdl.handle.net/1903/2202), equations 38 and 52, supply the
summed Adams velocity and Gauss-Jackson position operators.

With backward difference `x`, define `q(x)=x/[-log(1-x)]`. The regular velocity
correction is `(q-1)/x`, and position correction is `(q*q-1+x)/(x*x)`.
Truncate these series through `x^8` and convert differences into acceleration
ordinates with the binomial identity. First and second integration sums retain
their initial conditions. The implementation advances the second sum as position
increments to avoid subtracting large positions, with compensated accumulation.
The predictor extrapolates the degree-eight acceleration history; the corrector
jointly iterates both position and velocity, including velocity-dependent forces.

Corrector thresholds bound per-component iteration changes; they are **not**
local truncation-error tolerances, global accuracy certificates, or physical
model uncertainties. There is no adaptive mature-step controller. Smooth
non-stiff forces are required across the entire history. The method is not
A-stable; large steps, fast periapsis motion or stiff velocity dependence can
be unstable even when an iteration converges. Small steps may instead encounter
roundoff and startup error floors. Compare step-halved trajectories and an
independent reference before selecting an operational configuration.

Restart after a force discontinuity, impulse, frame change, or step change.
This slice does not integrate maneuvers, events, attitude/mass, STM or covariance,
and has no language-binding surface.

## Evidence

The [scientific and benchmark record](../../.agent/references/gauss-jackson/README.md)
includes three-day analytical and independent black-box cases and limitations.
The paper's ISS/CRRES periods/eccentricities define reduced two-body examples;
the paper's perturbed geopotential/atmosphere/lunar/solar scenarios are **not**
reproduced. This remains an experimental opt-in method, not operational
certification or a new default.
