# Elliptic two-body comparison case

This directory records black-box comparisons for the first orskit two-body
solution. Reference harnesses are isolated from project implementation and use
only published APIs. Their output is copied into an offline Rust regression
test; the external tools are never required by normal builds or CI.

## Shared input

| Quantity | Value |
| --- | ---: |
| Semi-major axis | 7,200,000 m |
| Eccentricity | 0.1 |
| Inclination | 0.7 rad |
| Right ascension of ascending node | 1.1 rad |
| Argument of periapsis | 0.4 rad |
| Initial true anomaly | 2.0 rad |
| Elapsed time | 3,600 s |
| Gravitational parameter | Explicit per comparison; see below |
| Coordinate axes | Orekit/orskit GCRF; Lox ICRF, using the same numerical element axes without a frame transform |

## Outputs

All Cartesian values use SI units.

| Implementation | Position `(x, y, z)` m | Velocity `(x, y, z)` m/s | Status |
| --- | --- | --- | --- |
| Orekit 13.1.6 `KeplerianPropagator` | `(4863976.030492352, 4133125.6430910705, -2072064.3510849578)` | `(-3449.464728617805, 5450.5641610648245, 4671.788819571301)` | Generated 2026-07-04 with `orekit/` harness and `mu = 398600441800000 m^3/s^2` |
| Lox 0.1.0-alpha.39 `Vallado` | `(4863976.12851866, 4133125.488197863, -2072064.4838470626)` | `(-3449.4645190814335, 5450.564272952743, 4671.788705029827)` | Generated 2026-07-04 with `lox/` harness and Lox Earth `mu = 398600435507022.7 m^3/s^2` |
| Nyx | — | — | Not consulted: repository policy prohibits using Nyx implementation material, docs, tests, or examples |

Each regression uses the gravitational parameter recorded in its row. This is
essential: Lox's built-in Earth value differs from the explicit Orekit value by
about 15.8 parts per billion. Both regressions use a tolerance of 1 micrometre
in position and 1 nanometre per second in velocity. It covers independent
implementation and f64 ordering differences while remaining far below
operational orbit-state uncertainties.

This validates propagation and element/Cartesian conversion, not a GCRF/ICRF
frame transform: each tool receives the same numeric orientation relative to
its inertial axes, and no transform is requested. The absolute epoch is likewise
tool-specific because the autonomous point-mass problem depends only on the
3,600-second interval.

## Reproduction

Run the Orekit harness with Gradle 9.2.1 or newer:

```powershell
gradle -p .agent/references/two-body/orekit run --quiet
```

Run the Lox harness with the pinned public package:

```powershell
uv run --with lox-space==0.1.0a39 python .agent/references/two-body/lox/reference.py
```

A Nyx value may be added if supplied independently by a maintainer without
exposing prohibited Nyx material to this implementation effort.
