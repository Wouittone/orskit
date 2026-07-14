# Engineering standard

## Scientific conventions

- Use `f64` as the baseline scalar until a capability explicitly requires a
  generic scalar or automatic differentiation.
- Every public physical value must carry a compile-time quantity type. Use
  `uom` with SI storage internally and typed conversion units at boundaries.
  Raw `f64` is permitted only inside numerical kernels or at unit-named FFI,
  parsing, and serialization adapters; it must not become a domain field.
- Attach a frame directly to every coordinate-dependent public value and use
  Hifitime epochs for state time. Do not assume position, velocity, attitude,
  inertia, covariance, or derivatives share a frame merely because they belong
  to one state; do not accept naked six-element vectors.
- State angle ranges, longitude sign, axis order, rotation convention, anomaly
  convention, and derivative ordering.
- State model validity ranges and singularities. Return a typed error or use a
  nonsingular representation rather than silently degrading.
- Cite constants and equations. Version convention-sensitive datasets and
  models.

## Numerical work

Each numerical algorithm must document:

- the equation or standard being implemented;
- inputs, outputs, dimensions, and coordinate representation;
- conditioning and known singularities;
- absolute/relative tolerances and their physical interpretation;
- convergence and iteration limits;
- deterministic behavior and floating-point caveats; and
- an independent validation strategy.

Avoid exact equality for computed floating-point values. A tolerance must be
derived from the model, reference accuracy, and use case—not copied from a
passing test. Preserve accuracy checks when optimizing.

## Rust API and safety

- Format with stable `rustfmt`; lint with Clippy and treat project-code warnings
  as failures in CI.
- Public items require rustdoc and examples when usage is not obvious.
- Use `Result` for recoverable failures and meaningful error enums for domain
  failures. Avoid stringly typed errors in the scientific core.
- Validate invariants at construction. Make invalid combinations difficult to
  express with types.
- Every active Rust crate uses `#![forbid(unsafe_code)]`; unsafe code is not an
  accepted implementation technique in the scientific workspace. Disabled
  binding prototypes stay outside the dependency graph and require an explicit
  binding task plus policy review before modification or re-enablement.
- Do not panic across FFI or on untrusted data. Avoid hidden allocations in
  hot loops and expose reusable workspaces only when profiling justifies them.
- Declare and test the minimum supported Rust version before the first public
  release; do not raise it accidentally.

## Test hierarchy

1. **Unit tests:** formulas, constructors, conversions, and errors.
2. **Property/invariant tests:** round trips, transform composition, conserved
   quantities, symmetry, monotonicity, and dimensional expectations.
3. **Reference-vector tests:** standards, papers, or independently generated
   datasets with provenance and tolerances.
4. **Differential tests:** public behavior of an independently implemented
   reference, with version, inputs, and comparison policy recorded.
5. **Scenario tests:** end-to-end mission workflows crossing domain crates.
6. **Binding tests:** package/import/load, ownership, errors, arrays, threading,
   and parity with the Rust API.

Regression tests should first fail for the reported defect. Avoid snapshots for
floating-point results unless the snapshot format includes units, conventions,
and an intentional tolerance-aware comparison.

## Performance

- Establish correctness before optimizing.
- Benchmark representative small and large cases with fixed datasets and
  toolchain metadata.
- Measure throughput, latency, allocations, and—where relevant—parallel scaling.
- Compare numerical error as well as wall-clock performance.
- Keep benchmark baselines out of correctness tests; use them to detect and
  investigate regressions, not to promise identical timings.

## Data and parsing

- Parsing is untrusted-input handling: bound allocations, report line/field
  context, reject invalid values, and fuzz important formats.
- Do not download scientific data implicitly. Support caller-controlled local
  stores and explicit fetch tooling separately from algorithms.
- Record source, version, checksum, coverage interval, and conventions for
  every loaded dataset where the format permits.

## Dependency and feature discipline

- Keep the default feature set useful, deterministic, and free of network
  behavior.
- Gate heavy formats, parallelism, and bindings behind purposeful features or
  separate crates.
- Audit duplicate dependencies, security advisories, and licenses in CI.
- Avoid a catch-all `utils` dumping ground; place concepts with the domain that
  owns their invariants.

## Baseline checks

The currently validated Rust toolchain and package `rust-version` are pinned to
Rust 1.96.1. A lower MSRV may be declared only after it is tested in CI against
the full Rust workspace.

Run the checks applicable to a change from the repository root:

```powershell
cargo fmt --all --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo nextest run --workspace --locked
cargo doc --workspace --no-deps --locked
```

Python and JVM binding workspaces are intentionally disabled while the Rust
core API stabilizes. They are excluded from local baseline checks and CI. When
binding work resumes, add package-level smoke tests and platform/toolchain
support before treating either package as available.
