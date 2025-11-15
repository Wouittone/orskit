# orskit - Orbital Mechanics Library in Rust

A Rust library for orbital mechanics calculations, inspired by [Orekit](https://www.orekit.org/). orskit provides orbital propagation, station geometry, and measurement handling with language bindings for Python and Java.

## Structure

### Core Library (`crates/`)

- **`core`**: Core orbital mechanics structures and utilities
- **`orbit`**: Orbital propagation using numerical ODE solvers (RK4, Dopri5, Dop853)
- **`stations`**: Ground station and geographic location utilities
- **`measurements`**: Measurement and observation handling
- **`utils`**: Common constants and utility functions

### Language Bindings (`bindings/`)

- **`python/`**: Python bindings using PyO3 (for Python 3.8+)
- **`java/`**: Java FFM (Foreign Function & Memory) bindings for Java 25+

## Building

### Rust Library

```bash
cargo build --release
```

### Python Bindings

```bash
cd bindings/python
pip install maturin
maturin develop
```

### Java Bindings

Requires Java 25+ and Gradle:

```bash
cd bindings/java
gradle build
```

## Dependencies

### Core Library
- **[nalgebra](https://github.com/dimforge/nalgebra)** (0.34): Linear algebra for vectors and matrices
- **[ode-solvers](https://github.com/ivan-pi/ode_solvers)** (0.6): Numerical ODE integration (RK4, Dopri5, Dop853)
- **[hifitime](https://github.com/nyx-space/hifitime)** (4.1): High-fidelity time handling for precise orbital mechanics

### Python Bindings
- **[PyO3](https://github.com/PyO3/pyo3)** (0.21): Python-Rust interop

### Java Bindings
- **Java 25+**: For FFM API support

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
