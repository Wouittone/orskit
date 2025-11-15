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

Requires Java 25+ and Gradle 9+:

```bash
cd bindings/java
./gradlew build          # Unix/Linux/macOS
gradlew.bat build        # Windows

# Or with locally installed Gradle
gradle build
```

#### Java Build Configuration

The Java bindings use Kotlin-based Gradle with modern best practices:
- **Gradle 9.0+**: Latest Gradle version with performance improvements
- **Kotlin DSL**: Type-safe build configuration (`build.gradle.kts`)
- **Configuration Cache**: Faster build times with incremental caching
- **Build Cache**: Reusable build outputs across machines
- **JaCoCo**: Code coverage reporting
- **Parallel Builds**: Multi-threaded compilation
- **Java 25 Preview Features**: Support for latest Java features

#### Build Properties Optimization

The `gradle.properties` file includes:
- **Daemon Settings**: Long-lived Gradle daemon for faster builds
- **JVM Configuration**: Optimized heap size (4GB max) and G1 garbage collector
- **Parallel Execution**: Automatic worker detection for concurrent builds
- **Caching**: Configuration cache and build cache enabled for incremental builds
- **Feature Preview**: Auto-download of Java installations

#### Gradle Wrapper

The project includes `gradlew` (Unix/Linux/macOS) and `gradlew.bat` (Windows) scripts for reproducible builds without installing Gradle locally.

## Dependencies

### Core Library
- **[nalgebra](https://github.com/dimforge/nalgebra)** (0.34): Linear algebra for vectors and matrices
- **[ode-solvers](https://github.com/ivan-pi/ode_solvers)** (0.6): Numerical ODE integration (RK4, Dopri5, Dop853)
- **[hifitime](https://github.com/nyx-space/hifitime)** (4.1): High-fidelity time handling for precise orbital mechanics

### Python Bindings
- **[PyO3](https://github.com/PyO3/pyo3)** (0.21): Python-Rust interop

### Java Bindings
- **[Java 25+](https://openjdk.org/)**: For FFM API support
- **[Gradle 9+](https://gradle.org/)**: Build automation with Kotlin DSL

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
