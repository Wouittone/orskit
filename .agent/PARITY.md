# Capability parity ledger

This ledger prevents aspirational scope from becoming an unsupported parity
claim. It tracks capability families, not one-to-one Java classes.

## Reference baseline

- **Orekit version:** 13.1.7.
- **Orekit release date:** 2026-07-03.
- **Baseline pinned on:** 2026-07-07.
- **Inventory revision:** `orekit-13.1.7-2026-07-07`.
- **Baseline record:** `.agent/baselines/orekit-13.1.7.md`.
- **Inventory method:** public documentation and public behavior only, under
  `PROVENANCE.md`.

Do not change any row to `Validated` or publish a percentage of Orekit parity
unless the row links to acceptance evidence for this pinned baseline. Evidence
generated against an older Orekit release remains useful only when it is
explicitly labeled with that older version.

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
| Foundations | Units, dimensions, constants, and numerical policies | Partial | Typed/convention-explicit APIs; sourced constants; dimensional tests | `units`; `uom` compile-fail example; typed constants |
| Foundations | Time scales, calendars, durations, leap seconds | Partial | Standard vectors; leap-boundary tests; scale round trips | Hifitime 4.3 adopted directly; orskit validation pending |
| Foundations | Explicit scientific data context and providers | Partial | Version/checksum behavior; offline deterministic scenario | Object-safe `ScientificSource` and `CentralGravity` traits let applications share immutable catalog/model objects through `Arc`; built-in reference types cover small workflows, while general providers and checksum/version policies remain pending |
| Geometry | Frames, transforms, Earth orientation | Partial | Transform composition/inverse; independent frame vectors | Affirmative motion composes with caller-owned `FrameCatalog`; namespace-qualified `FrameId` prevents cross-catalog collisions, definitions reject conflicts/foreign parents and form registered acyclic chains; transform evaluation is not implemented |
| Geometry | Celestial bodies, ephemerides, ellipsoids, geodesy | Partial | Standard geodetic vectors; ephemeris comparison | `bodies` provides classified immutable body identities, validated explicit body-system membership, and custom identities; masses, shapes, rotation, ephemerides, and geodesy remain pending |
| Orbits | Epoch/frame-qualified Cartesian states | Partial | Invariant-bearing API; units/frame/time tests | `CartesianState` stays gravity-independent; element states share an `Arc<dyn CentralGravity>`; `Spacecraft` geometry uses sealed validated sphere/cuboid payloads; `Orbit` qualifies the closed representation set with its epoch; conversion has typed gravity/frame/degeneracy/conic errors |
| Orbits | Keplerian, circular, equinoctial, and nonsingular elements | Partial | Round trips across regimes; singularity policy | Six-element elliptic `KeplerianState` and `(a, ex, ey, hx, hy, lv)` `EquinoctialState` carry an opaque `InertialFrame` capability, so non-inertial or unspecified axes cannot enter element APIs; `From`/`To` enum wrapping and contextual conversions cover every current pair; analytic circular/polar vectors, representation round trips, and singularity errors pass; other conics/anomalies remain pending |
| Orbits | Anomalies, Jacobians, interpolation, covariance mapping | Not assessed | Analytic/reference comparisons and round-trip bounds | None |
| Propagation | Dynamics and force-model composition | Designed | Multi-model topology; explicit participants/data needs; coupled-state evaluation scenarios | `Force` identifies an open physical family while object-safe model contracts remain available for future composition; concrete `TwoBodyDynamics` preserves exactly one central point-mass model. General composition and third-body APIs are withheld until their evaluation, ephemeris, frame, provenance, and state contracts exist |
| Propagation | Two-body/Keplerian propagation | Partial | Analytic orbit scenarios; conservation/error budget | `EllipticKeplerPropagator` targets strict `TwoBodyDynamics`, uses exact duration nanoseconds, rejects spans outside a configurable phase-error budget, preserves zero identity, and advances equinoctial longitude without a singular Keplerian intermediate. Existing reference evidence remains; numerical/stochastic methods, other conics, events, and ephemerides remain pending |
| Propagation | Numerical integration and dense ephemerides | Not assessed | Integrator order/error tests; independent scenarios | Dependencies selected; no public propagator |
| Propagation | Gravity fields and solid/ocean tides | Not assessed | Published vectors; degree/order and convention tests | None |
| Propagation | Third-body, drag, atmosphere, radiation, relativity | Not assessed | Model-specific vectors and combined scenario | None |
| Propagation | Maneuvers, mass, and finite/impulsive burns | Not assessed | Conservation and event-timing scenarios | None |
| Propagation | Events and root localization | Not assessed | Direction/grazing/simultaneous-event tests | None |
| Propagation | TLE/SGP4 and analytical propagator families | Not assessed | Standard verification cases and format round trips | None |
| Propagation | Semi-analytical propagation | Not assessed | Long-arc reference scenarios with error budgets | None |
| Propagation | Variational equations, STM, and covariance propagation | Not assessed | Finite-difference/analytic sensitivities | None |
| Attitude | Rotations, angular states, and attitude providers | Partial | Composition, interpolation, and reference scenarios | `Spacecraft` owns its non-inertial body frame; `BodyAngularVelocity` names body and relative/reference frames; views validate body-attitude-inertia agreement and attitude/orbit reference agreement; providers, interpolation, and dynamics remain pending |
| Observation | Ground participants, displacement, clocks, weather | Partial | Frame/time-aware ground-observer scenarios | `measurements::GroundStation` owns a validated `ParticipantId` shared with observation paths and a parent-relative fixed Cartesian frame; geodetic conversion, local topocentric axes, displacement, clocks, and weather remain pending |
| Observation | Range, range-rate, angles, Doppler, GNSS, inter-satellite | Partial | Per-type reference vectors and participant timing | Range values own an ordered participant path, explicit epoch tag, scalar convention, measurement frame, typed range, and explicit optional typed uncertainty; light time, corrections, and other measurement families remain pending |
| Observation | Measurement modifiers and corrections | Not assessed | Model-specific correction vectors | None |
| Estimation | Parameter drivers and measurement generation | Not assessed | Parameter scaling/selection and simulation scenarios | None |
| Estimation | Batch least squares and sequential filters | Not assessed | Synthetic recovery, covariance, and independent cases | None |
| Mission analysis | Visibility, eclipse, occultation, FOV, access | Not assessed | Geometry edge cases and event scenarios | None |
| I/O | CCSDS orbit, attitude, tracking, and navigation messages | Partial | Conformance corpus; lossless semantic round trips | OEM KVN modes use finite budgets, enforce segment chronology, and preserve source-order segment IDs, line/section-qualified comments, sample lines, shared metadata, and ordered records; explicit enrichment supplies physical state properties absent from OEM; XML, covariance, other message families, conformance corpus, and writing remain pending |
| I/O | TLE, SP3, RINEX, gravity, EOP, ephemeris, space weather | Not assessed | Format-specific conformance and malformed-input tests | None |
| Bindings | Stable public Rust facade | Not assessed | Coherent documented workflow API | No public Rust facade is provided; applications import focused crates directly. |
| Bindings | Python package | Not assessed | Build/import smoke tests; typed API/error parity | Experimental PyO3 workspace retained but disabled while the Rust core API stabilizes; no CI validation currently runs |
| Bindings | JVM-language package | Not assessed | Native load/FFM smoke tests; ownership/error parity | Experimental C ABI/FFM workspace retained but disabled while the Rust core API stabilizes; no CI validation currently runs |

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
