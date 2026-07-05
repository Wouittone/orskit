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
| Orbits | Epoch/frame-qualified Cartesian states | Partial | Invariant-bearing API; units/frame/time tests | `CartesianState` stores `(x, y, z, vx, vy, vz)` in an explicit frame; `SpacecraftState` closes the supported representation set; `Orbit` qualifies a representation with its epoch; time-independent `Spacecraft` identity/geometry and complete `SpacecraftView` physical data remain separate; bound osculating conversion has typed frame/degeneracy/conic errors |
| Orbits | Keplerian, circular, equinoctial, and nonsingular elements | Partial | Round trips across regimes; singularity policy | Six-element elliptic `KeplerianState` and `(a, ex, ey, hx, hy, lv)` `EquinoctialState` implement `OrbitalElements`; `From`/`To` enum wrapping and contextual `TryFrom`/`TryTo` conversions cover every current pair; analytic circular/polar vectors, representation round trips, and singularity errors pass; other conics/anomalies remain pending |
| Orbits | Anomalies, Jacobians, interpolation, covariance mapping | Not assessed | Analytic/reference comparisons and round-trip bounds | None |
| Propagation | Dynamics and force-model composition | Designed | Multi-model topology; explicit participants/data needs; coupled-state evaluation scenarios | `Force` identifies an open physical family while object-safe `ForceModel` implementations compose heterogeneously in separate conservative/non-conservative collections; validated force/model identity, two-/three-body topology and plug-in ordering; evaluation/data/state contracts pending |
| Propagation | Two-body/Keplerian propagation | Partial | Analytic orbit scenarios; conservation/error budget | `Propagator<ForceModel>` analytically advances epoch-qualified `Orbit` values under `PointMassGravityModel`, preserving the orbital enum variant without claiming attitude or rigid-body evolution; every representation passes circular/invariant/reverse-time evidence and Orekit 13.1.6 plus Lox 0.1.0-alpha.39 Cartesian endpoint comparisons. A reproducible release-mode benchmark records orskit, Orekit, Lox, and isolated Nyx 2.3.1 speed and peak process working set. Nyx `Orbit::at_epoch` fails the shared 3,600-second Cartesian accuracy case by 4,377 km and is therefore reported but not accepted as validation evidence. Universal-variable/other conics, events, and ephemerides remain pending |
| Propagation | Numerical integration and dense ephemerides | Not assessed | Integrator order/error tests; independent scenarios | Dependencies selected; no public propagator |
| Propagation | Gravity fields and solid/ocean tides | Not assessed | Published vectors; degree/order and convention tests | None |
| Propagation | Third-body, drag, atmosphere, radiation, relativity | Not assessed | Model-specific vectors and combined scenario | None |
| Propagation | Maneuvers, mass, and finite/impulsive burns | Not assessed | Conservation and event-timing scenarios | None |
| Propagation | Events and root localization | Not assessed | Direction/grazing/simultaneous-event tests | None |
| Propagation | TLE/SGP4 and analytical propagator families | Not assessed | Standard verification cases and format round trips | None |
| Propagation | Semi-analytical propagation | Not assessed | Long-arc reference scenarios with error budgets | None |
| Propagation | Variational equations, STM, and covariance propagation | Not assessed | Finite-difference/analytic sensitivities | None |
| Attitude | Rotations, angular states, and attitude providers | Partial | Composition, interpolation, and reference scenarios | Closed `AttitudeState` currently wraps `QuaternionAttitude`, exposing framed orientation angles and body-frame angular velocity without generics or trait objects; angular-velocity/inertia body-frame consistency is validated; providers, interpolation, and dynamics remain pending |
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
