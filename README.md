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

> **Status: pre-alpha.** The repository currently contains an early workspace
> scaffold with typed units, celestial-body and frame identities, a closed
> six-element Cartesian/Keplerian/equinoctial state enum, epoch-qualified
> orbits, orbital conversion traits, time-independent spacecraft definitions,
> and epoch-specific views
> with attitude and angular velocity,
> streaming CCSDS OEM KVN ingestion, composable dynamics descriptions, a
> minimal range measurement, representation-preserving analytical elliptic
> two-body propagation for all current state types, and experimental binding
> adapters. General composed-force/numerical propagation,
> complete CCSDS coverage, and complete measurement-participant modeling are
> intentionally not implemented yet. Fixed ground stations can now be defined
> through parent-relative frames, but transforms, geodesy, clocks, and signal
> paths are absent. It is
> not suitable for scientific or operational use and does not have Orekit
> parity.

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
| `crates/units` | `uom`-backed physical quantities and typed Cartesian vectors |
| `crates/bodies` | Planet, moon, dwarf-planet, custom-body, and explicit body-system identities |
| `crates/frames` | Reference-frame identities plus caller-owned, parent-relative fixed frame definitions |
| `crates/core` | Six-element orbital states, epoch-qualified orbits, time-independent spacecraft identity/geometry, and complete physical views |
| `crates/dynamics` | System/force-model composition plus orbit-only analytical elliptic point-mass propagation |
| `crates/ccsds` | Blocking/Tokio streaming and Rayon collection for CCSDS OEM KVN coordinates |
| `crates/measurements` | Typed measurements and fixed ground-station participants built on parent-relative frames |
| `crates/utils` | Typed sourced constants; package boundary remains transitional |
| `bindings/python` | Experimental PyO3 binding workspace |
| `bindings/java` | Experimental native C ABI and Java FFM build workspace |

See the [capability parity ledger](.agent/PARITY.md) for an honest accounting of
what exists and what still needs to be researched, designed, and validated.

## Build the current scaffold

The core crates form a Cargo workspace:

```powershell
cargo build --workspace
cargo test --workspace
```

### Stream a CCSDS OEM

The current I/O slice emits timed coordinates without retaining a complete message:

```rust
use std::{fs::File, io::BufReader};
use orskit_ccsds::{OemEvent, OemKvnReader};

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
velocity. Define the time-independent `Spacecraft` from identity and geometry,
convert the OEM coordinates to `CartesianState`, then combine them with those
missing values in a `SpacecraftView`. This is presently
CCSDS 502.0-B-3 OEM KVN coordinate
ingestion only. XML,
covariance, OPM/OMM/OCM, attitude, and tracking messages remain explicit gaps.

The binding projects are separate workspaces for now.

### Python

Install [uv](https://docs.astral.sh/uv/), then from PowerShell:

```powershell
Push-Location bindings/python
uv run --with maturin maturin develop
Pop-Location
```

### JVM languages

The current experiment is the native side of a future Java Foreign Function &
Memory API package:

```powershell
cargo test --manifest-path bindings/java/Cargo.toml
```

The safe JVM wrapper and complete Gradle packaging are still planned work.
Platform/toolchain support will be documented and tested before the bindings
are presented as a stable package.

## Contributing

Before implementing a model, read the [agent and contributor
instructions](.agent/AGENTS.md) and the [provenance
policy](.agent/PROVENANCE.md). Work should advance a specific row in
the [parity ledger](.agent/PARITY.md) with tests, references, stated
tolerances, and honest known gaps.

The immediate priorities are listed in [the roadmap](.agent/ROADMAP.md).

## License

orskit is intended to be licensed, at your option, under either:

- the [MIT License](LICENSE-MIT); or
- the [Apache License, Version 2.0](LICENSE-APACHE).
