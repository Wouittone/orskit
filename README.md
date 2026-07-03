# orskit

orskit is an open-source astrodynamics toolkit being built in Rust. Its goal is
capability-level feature parity with [Orekit](https://www.orekit.org/), paired
with a Rust-native API, explicit physical context, and benchmarked performance.
Python and JVM-language bindings are planned as first-class interfaces.

The project takes high-level inspiration from the ergonomics of the Nyx space
libraries, but it is an independent implementation: Nyx source is not copied,
translated, or adapted. Project-owned code is intended to remain available
under either the MIT or Apache-2.0 license.

> **Status: pre-alpha.** The repository currently contains an early workspace
> scaffold and a first typed spacecraft-state, frame identity, two-body
> dynamics, station, measurement, Python, and native JVM-FFM slice. It is not
> yet suitable for scientific or operational use, and it does not currently
> have Orekit parity.

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
| `crates/frames` | Reference-frame origin/orientation identities |
| `crates/core` | Frame- and epoch-qualified spacecraft state with optional orientation/inertia |
| `crates/orbit` | Typed two-body dynamics scaffold |
| `crates/stations` | Typed, validated geographic-location scaffold |
| `crates/measurements` | Typed range-measurement scaffold |
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
