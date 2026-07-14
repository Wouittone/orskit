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
| Foundations | Explicit scientific data context and providers | Partial | Version/checksum behavior; offline deterministic scenario | `gravity::CentralGravityProvider` lets applications share their selected origin/parameter provider through `Arc`; the feature-gated `gravity::PointMass` is a small built-in provider, while general catalog and checksum/version policies remain pending |
| Geometry | Frames, transforms, Earth orientation | Partial | Transform composition/inverse; independent frame vectors | Affirmative motion composes with caller-owned `FrameCatalog`; namespace-qualified `FrameId` prevents cross-catalog collisions, definitions reject conflicts/foreign parents and form registered acyclic chains; transform evaluation is not implemented |
| Geometry | Celestial bodies, ephemerides, ellipsoids, geodesy | Partial | Standard geodetic vectors; ephemeris comparison | `bodies` provides classified immutable body identities, validated explicit body-system membership, and custom identities; masses, shapes, rotation, ephemerides, and geodesy remain pending |
| Orbits | Epoch/frame-qualified Cartesian states | Partial | Invariant-bearing API; units/frame/time tests | `orskit_core` exposes open `SpacecraftState` and constrained `Orbit<S: SpacecraftState>` contracts; `orbits` with its `cartesian` feature supplies gravity-independent `CartesianState` plus element states sharing an `Arc<dyn CentralGravityProvider>`; conversion has typed gravity/frame/degeneracy/conic errors |
| Orbits | Keplerian, circular, equinoctial, and nonsingular elements | Partial | Round trips across regimes; singularity policy | Elliptic `CircularState` `(a, ex, ey, i, Omega, alpha_v)`, six-element `KeplerianState`, and `(a, ex, ey, hx, hy, lv)` `EquinoctialState` carry an opaque `InertialFrame` capability, so non-inertial or unspecified axes cannot enter element APIs; standard `TryFrom`/`TryInto` cover every supported Cartesian, circular, Keplerian, and equinoctial conversion, with explicit shared gravity only for Cartesian input; `Orbit::try_map_state` preserves epoch; analytic circular/polar vectors, representation round trips, and singularity errors pass; other conics/anomalies remain pending |
| Orbits | Anomalies, Jacobians, interpolation, covariance mapping | Not assessed | Analytic/reference comparisons and round-trip bounds | None |
| Propagation | Dynamics and force-model composition | Partial | Multi-model topology; explicit participants/data needs; coupled-state evaluation scenarios | `Force` identifies an open physical family, object-safe model contracts compose heterogeneous models in ordered `ComposedDynamics`, and concrete model topologies live in dedicated implementation crates. General evaluation and third-body APIs remain withheld until their ephemeris, frame, provenance, and state contracts exist |
| Propagation | Two-body/Keplerian propagation | Partial | Analytic orbit scenarios; conservation/error budget | Feature-gated `dynamics-two-bodies` supplies strict `TwoBodyDynamics` and `EllipticKeplerPropagator`; the shared `PropagationState<Problem>` contract resolves a caller-selected `Orbit<S>` to Cartesian state before every solver runs and restores it afterward. The current solver applies universal-variable Lagrange `f`/`g` propagation to Cartesian, circular, Keplerian, or equinoctial callers, derives exact internal duration nanoseconds, and accepts exactly retrograde Cartesian planes; element-chart restoration retains its declared singularity policy |
| Propagation | Numerical integration and dense ephemerides | Not assessed | Integrator order/error tests; independent scenarios | Dependencies selected; no public propagator |
| Propagation | Gravity fields and solid/ocean tides | Not assessed | Published vectors; degree/order and convention tests | None |
| Propagation | Third-body, drag, atmosphere, radiation, relativity | Not assessed | Model-specific vectors and combined scenario | None |
| Propagation | Maneuvers, mass, and finite/impulsive burns | Not assessed | Conservation and event-timing scenarios | None |
| Propagation | Events and root localization | Not assessed | Direction/grazing/simultaneous-event tests | None |
| Propagation | TLE/SGP4 and analytical propagator families | Not assessed | Standard verification cases and format round trips | None |
| Propagation | Semi-analytical propagation | Not assessed | Long-arc reference scenarios with error budgets | None |
| Propagation | Variational equations, STM, and covariance propagation | Not assessed | Finite-difference/analytic sensitivities | None |
| Attitude | Rotations, angular states, and attitude providers | Partial | Composition, interpolation, and reference scenarios | `Spacecraft` and `SpacecraftView<S, G, A>` use open `SpacecraftGeometry` and `Attitude` contracts, while `quaternion-attitude` and `standard-shapes` separately gate built-in implementations; opaque body-frame capabilities and view validation preserve body-attitude-inertia and attitude/orbit frame agreement; providers, interpolation, and dynamics remain pending |
| Observation | Ground participants, displacement, clocks, weather | Partial | Frame/time-aware ground-observer scenarios | `measurements::GroundStation` owns a validated `ParticipantId` shared with observation paths and a parent-relative fixed Cartesian frame; geodetic conversion, local topocentric axes, displacement, clocks, and weather remain pending |
| Observation | Range, range-rate, angles, Doppler, GNSS, inter-satellite | Partial | Per-type reference vectors and participant timing | Object-safe `Measurement` composition exposes open `MeasurementKind` and `MeasurementQuantity` contracts. Separately feature-gated ground-observation data types cover range, range rate, azimuth/elevation, right ascension/declination, Doppler, bistatic range/range-rate, turn-around range, TDOA, FDOA, and carrier phase. Every value is a `uom` quantity with an explicit known-or-unknown error: scalar standard error for one-value observations and a typed positive-definite covariance input decomposed into a retained lower-triangular matrix for multi-value observations. GNSS, inter-satellite links, physical light-time evaluation, and measurement prediction remain pending |
| Observation | Measurement modifiers and corrections | Partial | Model-specific correction vectors | Ordered generic correction chains accept only corrections for the matching concrete observable. Unit-qualified additive corrections carry open `CorrectionKind` provenance types, with built-in clock, troposphere, ionosphere, relativity, and instrument markers each separately feature-gated alongside downstream extension points; they combine known scalar errors by root-sum-of-squares and stored lower matrices as `L₁L₁ᵀ + L₂L₂ᵀ` before strict decomposition; atmospheric, clock, relativistic, and instrumental model evaluation remain pending |
| Estimation | Parameter drivers and measurement generation | Not assessed | Parameter scaling/selection and simulation scenarios | None |
| Estimation | Batch least squares and sequential filters | Not assessed | Synthetic recovery, covariance, and independent cases | None |
| Mission analysis | Visibility, eclipse, occultation, FOV, access | Not assessed | Geometry edge cases and event scenarios | None |
| I/O | CCSDS orbit, attitude, tracking, and navigation messages | Partial | Conformance corpus; lossless semantic round trips | OEM KVN modes use finite budgets, enforce segment chronology, and preserve source-order segment IDs, line/section-qualified comments, sample lines, shared metadata, ordered records, and Cartesian covariance matrices. OEM lower triangles construct unit-qualified position, position/velocity, and velocity entries in open covariance-axes declarations, with catalogued reference and RTN implementations plus preserved application-defined identifiers; explicit enrichment supplies physical state properties absent from OEM; XML, other message families, broader conformance corpus, and writing remain pending |
| I/O | TLE, SP3, RINEX, gravity, EOP, ephemeris, space weather | Not assessed | Format-specific conformance and malformed-input tests | None |
| Bindings | Stable public Rust facade | Partial | Coherent documented workflow API | The `orskit` facade always exposes implementation-neutral core contracts and gates Cartesian, dynamics, two-body, CCSDS, bodies, and measurements implementations behind explicit Cargo features; each domain crate remains independently usable. |
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
