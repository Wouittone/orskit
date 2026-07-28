# Migrating Orekit workflows to orskit

This guide maps supported workflows from the pinned Orekit 13.1.7 baseline to
orskit concepts. It is not a Java-class compatibility table: orskit uses Rust
ownership, traits, explicit Cargo features, typed physical quantities, and
caller-selected scientific data. Check the [parity ledger](../.agent/PARITY.md)
before assuming that an Orekit capability has an orskit equivalent.

## The central design difference

Orekit commonly obtains time scales, frames, bodies, and gravity data from a
`DataContext` and factory APIs. Orskit does not use a process-wide default data
context. Select and retain each provider explicitly:

| Orekit 13.1.7 concept | Current orskit concept | Important difference |
| --- | --- | --- |
| `AbsoluteDate` and a `TimeScale` | `hifitime::Epoch` | The epoch retains its scale; UTC leap-second limitations are recorded in the parity ledger. |
| `Frame` / `FramesFactory` | `frames::ReferenceFrame`, `FrameCatalog`, and provider traits | Built-in identities and catalogued derived frames are separate from epoch-dependent transform providers. |
| `DataContext` / `DataProvidersManager` | `orskit_data::VerifiedArtifact` plus caller-owned providers | No global default, implicit filesystem search, or network fetch occurs in algorithms. |
| `CelestialBody` / `CelestialBodyFactory` | `bodies::Body` plus `gravity::CentralGravityProvider` and `ephemeris::EphemerisProvider` | Body identity, gravity, and physical ephemeris are deliberately separate selections. |
| `OneAxisEllipsoid` / `GeodeticPoint` / `TopocentricFrame` | `ReferenceEllipsoid`, `GeodeticPosition`, and catalogued `TopocentricFrame` | ENU construction requires affirmative body-fixed parent axes. |
| `Orbit` subclasses | `Orbit<S>` with `CartesianState`, `CircularState`, `KeplerianState`, or `EquinoctialState` | The representation is a Rust type parameter and gravity-dependent elements retain their selected provider identity. |
| `SpacecraftState` | `Orbit<S>` plus separate spacecraft/attitude contracts | An orbit is not silently enlarged into mass, attitude, or additional state. |
| `Propagator` / `KeplerianPropagator` | `Propagator<S>` / `EllipticKeplerPropagator` | A propagator owns its physical problem; propagation preserves the selected state representation. |
| `OemParser` | `ccsds::OemKvnReader` | The current reader is bounded and streaming; consult parity for supported encodings and retained semantics. |
| `ObservedMeasurement` and estimator families | typed `measurements` plus `orbit_determination` filters | The current slice is intentionally narrower; participant paths, covariance, and physical propagation are explicit inputs. |

## Choose Cargo capabilities first

The `orskit` facade has no default physical model. Enable only the workflow
capabilities you need. For example:

```toml
[dependencies]
orskit = { version = "0.0.0", features = [
    "cartesian",
    "point-mass-gravity",
    "two-bodies",
] }
```

Unlike importing an Orekit package, importing a Rust module does not activate
an implementation. Feature names and supported combinations are maintained in
the [feature matrix](feature-matrix.md).

## Replace factory lookups with explicit values

An Orekit workflow might request GCRF from `FramesFactory`, obtain Earth GM
from a data-backed factory, and pass both into an orbit. In orskit, construct
or receive the exact values and providers at the application boundary:

```rust
use std::sync::Arc;

use orskit::frames::{Body, FrameOrigin, InertialFrame};
use orskit::gravity::{PointMass, SharedCentralGravity};
use orskit::orbits::keplerian::KeplerianState;
use orskit::units::uom::si::{angle::radian, length::meter, ratio::ratio};
use orskit::units::{Angle, GravitationalParameter, Length, Ratio};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let gravity: SharedCentralGravity = Arc::new(PointMass::new(
        FrameOrigin::Body(Body::EARTH),
        GravitationalParameter::try_from(3.986_004_418e14)?,
    ));
    let state = KeplerianState::new(
        InertialFrame::GCRF,
        gravity,
        Length::new::<meter>(7_200_000.0),
        Ratio::new::<ratio>(0.01),
        Angle::new::<radian>(0.3),
        Angle::new::<radian>(0.4),
        Angle::new::<radian>(0.5),
        Angle::new::<radian>(0.6),
    )?;
    let _ = state;
    Ok(())
}
```

This extra construction is intentional: the gravity source and frame cannot
change because a global context was replaced elsewhere in the process.

## Transform frames through a selected provider

`ReferenceFrame` is an identity, not an implicit transform engine.
`FrameCatalog` validates caller-owned parent/child definitions; it does not
perform epoch-dependent transforms. Earth-orientation transforms use a
caller-created `Iers2010EarthOrientation` backed by a checksum-verified
artifact and decoded typed samples. Pass that supplier to the high-level
reference-data adapter; do not expect a frame identity to load EOP
automatically.

The current Earth-orientation slice is GCRF to/from ITRF2020 and does not yet
cover Orekit's complete frame tree, EOP ingestion, celestial-pole offsets, or
all ITRF realizations.

## Propagate without mutable initial-state configuration

Orekit propagators contain an initial state and expose target-date propagation.
The orskit `Propagator<S>` contract instead receives an `Orbit<S>` and a target
`Epoch`. This keeps the input explicit and permits one immutable propagator
configuration to serve independent calls:

```rust
use orskit::core::{Epoch, Orbit};
use orskit::dynamics::{EllipticKeplerPropagator, Propagator, TwoBodyDynamics};
use orskit::dynamics::PointMassGravityModel;
use orskit::orbits::keplerian::KeplerianState;

fn propagate(
    orbit: Orbit<KeplerianState>,
    gravity: orskit::gravity::SharedCentralGravity,
    target: Epoch,
) -> Result<Orbit<KeplerianState>, Box<dyn std::error::Error>> {
    let dynamics = TwoBodyDynamics::new(PointMassGravityModel::new(gravity));
    let propagator = EllipticKeplerPropagator::new(dynamics);
    Ok(propagator.propagate(orbit, target)?)
}
```

Do not translate Orekit event handlers, dense ephemeris generation, numerical
force-model configuration, or reset semantics into this analytical slice.
Use `dynamics::numerical` for the supported Cartesian adaptive propagation,
dense ephemeris, and immutable bracketed-event boundary. Reset/reintegration
semantics and Orekit's broader force-model ecosystem remain roadmap items.

## Treat parsing and persistence as boundaries

For OEM input, use the bounded streaming reader and handle events or build an
`OemSegment` deliberately. For application persistence, enable
`serialization`, register stable frame/provider IDs in an `ExportContext`, and
use a separate `ImportContext` containing only identities and live providers
the application trusts. Snapshot IDs are not `Display` strings and snapshots
do not recreate provider implementations.

## Error and validation migration

- Replace Java exceptions with the concrete Rust `Result<_, Error>` returned
  by each boundary; retain source errors instead of converting them to text.
- Convert raw values into typed `units` quantities at I/O boundaries.
- Treat frame origin, axes capability, epoch scale, provider identity,
  coverage, and checksum failures as contract errors rather than defaults.
- Do not infer parity from a similarly named type. The parity ledger names
  validation evidence and the unsupported remainder for every domain.

## Capabilities without a current migration path

There is not yet a supported equivalent for Orekit's high-fidelity numerical
force-model ecosystem, event-driven reset/reintegration, maneuvers, attitude
propagation, most CCSDS/GNSS formats, batch least squares, broad measurement
corrections, mission geometry, or language bindings. The supported
`dynamics::numerical` dense/event slice and fixed-model `dynamics::sgp4`
implementation are narrower than their full Orekit counterparts. Remaining
gaps are tracked explicitly in the roadmap and parity ledger.
