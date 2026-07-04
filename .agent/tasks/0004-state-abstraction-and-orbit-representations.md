# Task: define a common state abstraction with three orbit representations

## Parity target

- Ledger rows: Orbits / epoch-frame-qualified Cartesian states; Orbits /
  Keplerian, circular, equinoctial, and nonsingular elements
- Current status: Partial; Not assessed
- Intended status after this task: Partial; Partial

## User workflow

Construct a complete spacecraft state with an epoch, positive mass, explicit
orientation, framed inertia, and native Cartesian, elliptic Keplerian, or
elliptic equinoctial coordinates. Consume shared physical properties through
one `State` trait and request another coordinate representation through an
explicit conversion trait.

## Scientific contract

- Inputs and units: typed SI-backed length, velocity, mass, inertia, angle,
  ratio, and gravitational-parameter values.
- Outputs and units: native typed coordinates through the common trait;
  Cartesian states expose framed position, velocity, and speed. Explicit
  conversion produces a target state while preserving epoch and spacecraft
  properties.
- Frames/epochs/time scales: Hifitime `Epoch`; every coordinate representation
  carries an orskit `ReferenceFrame`; orientation and inertia retain their own
  frame identities.
- Conventions and valid regimes: osculating elliptic Keplerian elements with
  true anomaly; equinoctial `(a, ex, ey, hx, hy, lv)` using
  `ex=e cos(omega+Omega)`, `ey=e sin(omega+Omega)`,
  `hx=tan(i/2) cos(Omega)`, `hy=tan(i/2) sin(Omega)`, and
  `lv=nu+omega+Omega`. Equinoctial states support circular/equatorial elliptic
  cases but not the exactly retrograde equatorial singularity.
- External data requirements: the caller supplies the central body's positive
  gravitational parameter only to conversion into Cartesian representation; no
  state stores it and no implicit body constant is selected.
- Errors and singularities: non-finite values, non-positive semi-major axis,
  eccentricity outside `[0,1)`, inclination outside `[0,pi]`, and non-finite
  derived Cartesian coordinates are typed construction errors.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| NASA GMAT Mathematical Specifications, 2007 | US Government work, public use | State representations and Cartesian/Keplerian conversion conventions | `crates/core/src/state.rs` |
| NAIF SPICE `CONICS` public documentation | US Government documentation | Conic element ordering, units, and state-vector semantics | Keplerian API docs and tests |
| Orekit 12.0.2 `EquinoctialOrbit` public API documentation | Public behavior documentation only | Equinoctial element definitions and elliptic/singularity regime | Equinoctial API docs and tests |

No source, tests, examples, or internal structure are copied from Orekit,
GMAT, SPICE, Lox, or Nyx.

## Design

- Affected crates/layers: `orskit-core`; `orskit-ccsds` adapter; compilation-only
  binding adjustments; handbook and public README.
- Public API: representation-aware `State` and `StateConversion<Target>` traits,
  `CoordinateSample<C>`, `SpacecraftProperties`, distinct Cartesian,
  Keplerian, and equinoctial coordinate types, and their concrete states.
- Rejected alternatives: inheritance-shaped enum hierarchy; optional physical
  properties in the trait; fabricating OEM mass/attitude/inertia; exposing naked
  six-element arrays; storing gravitational parameters or independent
  Cartesian values inside element representations.
- ADR required: yes, ADR-0004.

## Validation

- Unit cases: mass and element validation; equatorial circular and polar
  Keplerian vectors; circular/equatorial equinoctial state.
- Invariants/properties: all representations expose identical physical
  properties; Keplerian/equinoctial conversion preserves Cartesian position and
  velocity within physically stated tolerances.
- Independent reference vectors: analytic circular-orbit vectors and vis-viva
  speed.
- Differential/scenario tests: representation round trips through independently
  expressed formulas; CCSDS coordinates enrichment into a full state.
- Tolerances and justification: metre and micrometre-per-second-scale bounds for
  deterministic f64 representation conversions at LEO scale.
- Benchmarks: not required until conversion profiling identifies a hot path.

## Completion checklist

- [x] Implementation and typed errors
- [x] Scientific and regression tests
- [x] Rustdoc/examples
- [x] Provenance recorded
- [x] Parity ledger updated
- [x] Relevant checks pass
- [x] Binding impact handled or explicitly deferred
