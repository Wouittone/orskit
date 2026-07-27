# orskit

orskit is an open-source astrodynamics toolkit being built in Rust. Its goal is
capability-level feature parity with [Orekit](https://www.orekit.org/), paired
with a Rust-native API, explicit physical context, and benchmarked performance.
Python and JVM-language bindings are planned as first-class interfaces.

The project uses three reference projects deliberately: Orekit for capability
coverage, [Lox](https://github.com/lox-space/lox) for modern Rust astrodynamics
design ideas, and Nyx for high-level ergonomics. orskit remains an independent
implementation: their source is not copied, translated, or adapted into
project-owned code. That code is intended to remain available under either the
MIT or Apache-2.0 license.

> **Status: pre-alpha.** The workspace has typed units; explicit body, frame,
> orbit, dynamics, and scientific-data contracts; Cartesian and primary
> elliptic element representations; verified Earth-orientation transforms,
> WGS 84 geodesy and local ENU frames; caller-selected sampled ephemerides;
> streaming OEM KVN/XML and strict TLE ingestion; analytical elliptic two-body
> and adaptive Cartesian propagation with optional dense output and events;
> Cartesian sequential
> orbit-determination filters; and versioned snapshot exchange. General
> high-fidelity force coverage, complete
> CCSDS coverage, broader operational propagation policy, clocks and complete participant paths,
> mission geometry, and stable bindings remain roadmap work. The project is not
> suitable for scientific or operational use and does not have Orekit parity.

Rust core correctness is the current priority. Python and JVM bindings are
planned, but feature work on them is deferred until the core contracts settle.

## Direction

orskit is designed around a few hard requirements:

- frames, epochs, time scales, units, constants, and model data are explicit;
- every public physical value is dimensionally typed rather than documented by
  convention alone;
- numerical claims are backed by traceable references and error budgets;
- the scientific implementation remains independent and permissively licensed;
- safe Rust owns domain behavior while FFI layers remain thin;
- users interact through one small, high-level domain API; vector/matrix
  kernels remain implementation details rather than a second public API;
- optimizations are judged on accuracy and reproducible measurements; and
- scientific datasets are versioned and caller-controlled, never silently
  downloaded by an algorithm.

The intended scope includes precise time and frames, celestial bodies and
ephemerides, orbit representations, analytical and numerical propagation,
force models, events, attitudes, measurements, estimation, mission geometry,
operational data formats, and Rust/Python/JVM APIs.

The project handbook in [`.agent/`](.agent/) defines the architecture,
clean-room provenance policy, quality standard, capability ledger, and staged
roadmap. Start with [`.agent/README.md`](.agent/README.md).

## Current workspace

| Path | Current role |
| --- | --- |
| `crates/orskit` | Feature-gated public facade: contracts by default, selected implementations on demand |
| `crates/data` | Verified caller-selected scientific-data identities, SHA-256 digests, time coverage, and bounded offline loading |
| `crates/units` | `uom`-backed physical quantities and typed Cartesian vectors |
| `crates/bodies` | Body identities plus caller-selected reference ellipsoids and geodetic/geocentric conversion |
| `crates/frames` | Reference-frame identities, caller-owned derived frames, and an opt-in verified IERS 2010 GCRF/ITRF2020 transform |
| `crates/core` | Open state and generic orbit contracts plus spacecraft identity/geometry and complete physical views |
| `crates/orbits` | Feature-gated state representations; `cartesian` provides Cartesian, elliptic circular, Keplerian, and equinoctial states |
| `crates/gravity` | Gravity-provider contract; the `point-mass` feature provides an immutable point-mass provider |
| `crates/dynamics` | Core force-model/propagation contracts, with opt-in `two-bodies` point-mass dynamics and analytical elliptic Kepler propagation |
| `crates/dynamics/numerical` | Opt-in adaptive Fehlberg RK4(5) Cartesian propagation with immutable dense output and bounded event localization |
| `crates/dynamics/sgp4` | Stateless, non-configurable WGS-72/AFSPC SGP4 propagation from model-specific mean elements to Cartesian TEME |
| `crates/ephemeris` | Caller-selected physical ephemeris contracts and verified-artifact-backed sampled interpolation |
| `crates/orbit-determination` | Open sequential OD contracts plus Cartesian extended and unscented Kalman filters over caller-selected propagators |
| `crates/ccsds` | Bounded blocking OEM KVN/XML streaming plus Tokio KVN streaming and Rayon KVN collection |
| `crates/export` | Opt-in, versioned Serde snapshots and validated reconstruction for orbit states and analytical-propagator configuration; JSON is separately feature-gated |
| `crates/tle` | Strict, bounded NORAD TLE parsing and canonical formatting, plus opt-in conversion to the separate SGP4 domain state |
| `crates/measurements` | Typed measurements and fixed ground-station participants built on parent-relative frames |
| `crates/utils` | Typed sourced constants; package boundary remains transitional |
| `bindings/python` | Disabled experimental PyO3 binding workspace |
| `bindings/java` | Disabled experimental native C ABI and Java FFM build workspace |

See the [capability parity ledger](.agent/PARITY.md) for an honest accounting of
what exists and what still needs to be researched, designed, and validated.

## Build the current scaffold

The core crates form a Cargo workspace. The currently validated Rust toolchain
is pinned in `rust-toolchain.toml` and package metadata. Install
[`cargo-nextest`](https://nexte.st/) before running the test commands.
The repository also provides a [`justfile`](justfile) with discoverable
`check`, `test`, `docs`, and `bench` shortcuts; raw Cargo commands remain in
[CONTRIBUTING.md](CONTRIBUTING.md).

```powershell
cargo build --workspace
cargo nextest run --workspace --all-targets --all-features --locked
# cargo-nextest does not support doctests on stable Rust.
cargo test --workspace --doc --all-features --locked
```

Small Rust examples can import the focused crates directly:

The Cargo package `core` exposes the Rust library `orskit_core`,
avoiding a collision with Rust's built-in `core` crate.

```rust
use frames::ReferenceFrame;

let frame = ReferenceFrame::GCRF;
assert!(frame.is_inertial());
```

The public facade's `serialization` feature exposes format-neutral, owned
snapshots without changing domain objects or attempting to serialize
application-owned provider trait objects. `serialization-json` additionally
provides JSON encoding and decoding. Element-state and propagator snapshots
require callers to register stable IDs for their selected central-gravity
providers; snapshot fields containing raw physical values name their SI or
radian units explicitly. Import resolves only caller-approved frame, origin,
and provider identities, checks the declared schema and provider metadata, and
reconstructs values through the live domain constructors. Serialization follows
the selected facade capabilities and does not silently enable Cartesian or
two-body implementations.

### Stream a CCSDS OEM

The current I/O slice emits timed coordinates without retaining a complete message:

```rust
use std::{fs::File, io::BufReader};
use ccsds::{OemEvent, OemKvnReader};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let reader = OemKvnReader::new(BufReader::new(File::open("orbit.oem")?));
    for event in reader {
        if let OemEvent::Coordinates(coordinates) = event? {
            println!(
                "{}: {:?}",
                coordinates.epoch(),
                coordinates.coordinates().position()
            );
        }
    }
    Ok(())
}
```

OEM supplies timed coordinates but not mass, inertia, attitude, or angular
velocity. Enable the facade `cartesian` feature (or depend on `orbits` with its
`cartesian` feature) to convert OEM coordinates to `CartesianState`, then
combine them with those missing values in a `SpacecraftView<CartesianState>`.
This is presently CCSDS 502.0-B-3 OEM KVN ingestion and bounded blocking OEM
3.0 XML ingestion, including typed Cartesian covariance records. XML writing,
OPM/OMM/OCM, attitude, and tracking messages remain explicit gaps.

The Python and JVM binding experiments are currently disabled while the Rust
core API is stabilized. Their separate workspaces remain in the repository but
are not built or tested in CI. Platform/toolchain support and stable package
workflows will be documented when binding work resumes.

## Contributing

Start with [CONTRIBUTING.md](CONTRIBUTING.md). Before implementing a model,
read the [agent instructions](.agent/AGENTS.md) and the [provenance
policy](.agent/PROVENANCE.md). Work should advance a specific row in the
[parity ledger](.agent/PARITY.md) with tests, references, stated tolerances,
and honest known gaps.

The immediate priorities are listed in [the roadmap](.agent/ROADMAP.md).

Current developer and API guides include:

- the [Cargo-metadata-backed crate diagram](docs/architecture.md);
- [elliptic two-body propagation](docs/tutorials/two-body-propagation.md);
- [Cartesian position orbit determination](docs/tutorials/cartesian-orbit-determination.md);
- [custom propagation pairs](docs/guides/custom-propagation.md);
- the [Orekit workflow migration guide](docs/orekit-migration.md);
- and the [pre-1.0 release policy](docs/release-policy.md) and [changelog](CHANGELOG.md).

## License

orskit is intended to be licensed, at your option, under either:

- the [MIT License](LICENSE-MIT); or
- the [Apache License, Version 2.0](LICENSE-APACHE).
