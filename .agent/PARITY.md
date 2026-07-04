# Capability parity ledger

This ledger prevents aspirational scope from becoming an unsupported parity
claim. It tracks capability families, not one-to-one Java classes.

## Reference baseline

- **Orekit version:** not yet pinned.
- **Baseline date:** not yet pinned.
- **Inventory method:** public documentation and public behavior only, under
  `PROVENANCE.md`.

Pin these before changing any row to `Validated` or publishing a percentage of
Orekit parity.

## Status vocabulary

- `Not assessed` — inventory or requirements are incomplete.
- `Researched` — scope, references, conventions, and acceptance evidence are
  recorded.
- `Designed` — public contract and architecture decision are accepted.
- `Partial` — useful implementation exists but acceptance evidence or material
  sub-capabilities are missing.
- `Validated` — acceptance criteria pass with linked evidence for the pinned
  baseline.

`Validated` is never inferred from compilation, a demo, or a type name.

## Ledger

| Domain | Capability family | Status | Acceptance evidence required | Current evidence |
| --- | --- | --- | --- | --- |
| Foundations | Units, dimensions, constants, and numerical policies | Partial | Typed/convention-explicit APIs; sourced constants; dimensional tests | `orskit-units`; `uom` compile-fail example; typed constants |
| Foundations | Time scales, calendars, durations, leap seconds | Partial | Standard vectors; leap-boundary tests; scale round trips | Hifitime 4.3 adopted directly; orskit validation pending |
| Foundations | Explicit scientific data context and providers | Not assessed | Version/checksum behavior; offline deterministic scenario | None |
| Geometry | Frames, transforms, Earth orientation | Partial | Transform composition/inverse; independent frame vectors | Typed origin/orientation identities compose body-backed or barycentric origins; transforms not implemented |
| Geometry | Celestial bodies, ephemerides, ellipsoids, geodesy | Partial | Standard geodetic vectors; ephemeris comparison | `orskit-bodies` provides classified immutable body identities, validated explicit body-system membership, and custom identities; masses, shapes, rotation, ephemerides, and geodesy remain pending |
| Orbits | Epoch/frame-qualified Cartesian states | Partial | Invariant-bearing API; units/frame/time tests | Representation-aware `State` contract composes an epoch and native coordinates with explicit mass, orientation, and inertia; `CartesianState` alone exposes position, velocity, and speed |
| Orbits | Keplerian, circular, equinoctial, and nonsingular elements | Partial | Round trips across regimes; singularity policy | Elliptic osculating `KeplerianState` and `(a, ex, ey, hx, hy, lv)` `EquinoctialState` store only native coordinates; `StateConversion` accepts gravitational parameter only for conversion to Cartesian; analytic circular/polar vectors, representation agreement, and explicit singularity errors; hyperbolic/parabolic and other anomaly types pending |
| Orbits | Anomalies, Jacobians, interpolation, covariance mapping | Not assessed | Analytic/reference comparisons and round-trip bounds | None |
| Propagation | Dynamics and force-model composition | Designed | Multi-model topology; explicit participants/data needs; coupled-state evaluation scenarios | Description-only `SystemDynamics` and open `ForceModel` contracts; validated two-/three-body topology and plug-in ordering; evaluation/data/state contracts pending |
| Propagation | Two-body/Keplerian propagation | Not assessed | Analytic orbit scenarios; conservation/error budget | Two-body topology can now be described, but no dynamics evaluator or propagator exists |
| Propagation | Numerical integration and dense ephemerides | Not assessed | Integrator order/error tests; independent scenarios | Dependencies selected; no public propagator |
| Propagation | Gravity fields and solid/ocean tides | Not assessed | Published vectors; degree/order and convention tests | None |
| Propagation | Third-body, drag, atmosphere, radiation, relativity | Not assessed | Model-specific vectors and combined scenario | None |
| Propagation | Maneuvers, mass, and finite/impulsive burns | Not assessed | Conservation and event-timing scenarios | None |
| Propagation | Events and root localization | Not assessed | Direction/grazing/simultaneous-event tests | None |
| Propagation | TLE/SGP4 and analytical propagator families | Not assessed | Standard verification cases and format round trips | None |
| Propagation | Semi-analytical propagation | Not assessed | Long-arc reference scenarios with error budgets | None |
| Propagation | Variational equations, STM, and covariance propagation | Not assessed | Finite-difference/analytic sensitivities | None |
| Attitude | Rotations, angular states, and attitude providers | Partial | Composition, interpolation, and reference scenarios | Validated orientation with explicit source/target frames and inertia with its own expression frame |
| Observation | Ground participants, displacement, clocks, weather | Not assessed | Frame/time-aware ground-observer scenarios | Ownership assigned to `measurements`; participant API not designed |
| Observation | Range, range-rate, angles, Doppler, GNSS, inter-satellite | Partial | Per-type reference vectors and participant timing | Typed range measurement only |
| Observation | Measurement modifiers and corrections | Not assessed | Model-specific correction vectors | None |
| Estimation | Parameter drivers and measurement generation | Not assessed | Parameter scaling/selection and simulation scenarios | None |
| Estimation | Batch least squares and sequential filters | Not assessed | Synthetic recovery, covariance, and independent cases | None |
| Mission analysis | Visibility, eclipse, occultation, FOV, access | Not assessed | Geometry edge cases and event scenarios | None |
| I/O | CCSDS orbit, attitude, tracking, and navigation messages | Partial | Conformance corpus; lossless semantic round trips | CCSDS 502.0-B-3 OEM KVN blocking/Tokio event ingestion and ordered Rayon collection into typed Cartesian coordinates; explicit enrichment supplies physical state properties absent from OEM; XML, covariance, other message families, conformance corpus, and writing remain pending |
| I/O | TLE, SP3, RINEX, gravity, EOP, ephemeris, space weather | Not assessed | Format-specific conformance and malformed-input tests | None |
| Bindings | Stable public Rust facade | Not assessed | Coherent documented workflow API | Independent scaffold crates only |
| Bindings | Python package | Partial | Build/import smoke tests; typed API/error parity | PyO3 orbital-state scaffold |
| Bindings | JVM-language package | Partial | Native load/FFM smoke tests; ownership/error parity | C ABI orbital-state scaffold |

## Validation record requirements

Every `Validated` row must link to:

- a scoped capability specification;
- authoritative scientific/format references and provenance;
- unit, invariant, reference-vector, and applicable differential tests;
- stated tolerances and supported regimes;
- benchmark evidence for performance claims;
- known gaps compared with the pinned baseline; and
- Rust/binding API documentation where applicable.

Split a row when its sub-capabilities can no longer be honestly represented by
one status. Never average statuses into a project-wide parity percentage without
a published weighting method.
