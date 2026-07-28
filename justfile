# Run `just` to discover the supported shortcuts. Raw Cargo equivalents remain
# documented in CONTRIBUTING.md and .agent/ENGINEERING.md.

default:
    @just --list

# Format, compile, and lint every Rust target with the locked dependency graph.
check:
    cargo fmt --all --check
    cargo check --workspace --all-targets --all-features --locked
    cargo clippy --workspace --all-targets --all-features --locked -- -D warnings -D clippy::must-use-candidate
    pwsh -NoProfile -File scripts/check_crate_diagram.ps1 -Check
    pwsh -NoProfile -File scripts/check_feature_matrix.ps1

# Run workspace tests, including doctests (which nextest does not support).
test:
    cargo nextest run --workspace --all-targets --all-features --locked
    cargo test --workspace --doc --all-features --locked

# Build all Rust API documentation without dependency docs.
docs:
    cargo doc --workspace --all-features --no-deps --locked

# Compile benchmark targets without using timings as correctness thresholds.
bench:
    cargo bench --workspace --all-features --no-run --locked

# Regenerate the Cargo-metadata-backed crate diagram.
diagram:
    pwsh -NoProfile -File scripts/check_crate_diagram.ps1

# Verify the committed crate diagram matches Cargo metadata.
diagram-check:
    pwsh -NoProfile -File scripts/check_crate_diagram.ps1 -Check

# Check the maintained representative Cargo feature combinations.
feature-matrix:
    pwsh -NoProfile -File scripts/check_feature_matrix.ps1
