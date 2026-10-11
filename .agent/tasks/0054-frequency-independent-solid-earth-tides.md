# Task 0054: implement frequency-independent solid-Earth tides

## Scope

- Issue: [#15](https://github.com/Wouittone/orskit/issues/15)
- Implement IERS Conventions (2010), Chapter 6, §6.2 Step 1 as normalized
  coefficient deltas integrated with `TimeVaryingSphericalHarmonicGravityModel`.
- Cover frequency-independent degree-2/3 solid-Earth response and the induced
  degree-4 terms only.
- Use explicit ITRF2020 Sun/Moon ephemerides, typed gravitational parameters,
  source records, and bounded Hifitime coverage; do not load or fetch data.
- Preserve permanent-tide conventions: full Step 1 for `TideFree`, remove
  permanent C̄₂₀ from the delta for `ZeroTide`, explicitly reject `MeanTide`.
- No new package or external dependency, no binding surface, no separate
  additive acceleration, and no full #15 completion claim.

## Scientific/API contract

- Equation 6.6 applies `k_nm/(2n+1) Σⱼ (3 GMⱼ/GMₑ)(Rₑ/rⱼ)ⁿ⁺¹
  P̄_nm(sin φⱼ) exp(-i m λⱼ)` for degrees 2 and 3. Equation 6.7 adds the
  induced degree-4 terms using `k₂m(+)`, the degree-2 Legendre functions and
  `(Rₑ/rⱼ)³`.
- Coefficients are dimensionless and fully normalized 4π, `P̄₀₀ = 1`.
  Love numbers are the nominal values from IERS Table 6.3.
- `GMₑ` and `Rₑ` come from the selected static gravity field; Moon/Sun `GM`
  and each source record are caller-supplied.
- The provider requests Moon/Sun center states in Earth-centered ITRF2020 for
  the exact covered epoch, validates body/epoch/frame/exterior distance, and
  resolves both states once per composed force evaluation.
- Errors preserve ephemeris provider failures. No omitted state is converted
  to zero, and no coverage extrapolation occurs.
- Analytic closed-form and invariant checks are not external product
  validation. No independently sourced tide vector is claimed.

## Completion checklist

- [x] Add the IERS frequency-independent coefficient provider and compose it
  inside the existing potential-gradient model.
- [x] Add typed, provenance-bearing Sun/Moon gravitational parameters and
  explicit ephemeris/frame/epoch/coverage validation.
- [x] Handle permanent tide per IERS Eqs. 6.13–6.14 and reject unsupported
  `MeanTide` convention.
- [x] Add closed-form/invariant tests for degree-2 zonal/tesseral, degree-3
  zonal, degree-4 induced response, permanent C̄₂₀, error propagation, and
  single-epoch ephemeris resolution.
- [x] Integrate through existing dynamics/orskit spherical-harmonics features;
  add API guide, ADR-0054, provenance, architecture, and parity updates.
- [x] Keep the full IERS Earth-orientation foundation status current.
- [x] Run the required Rust 1.96.1 workspace and package-feature validation
  below and record exact results.
- [x] Commit the completed scope; do not push or create a PR.

## Validation record

Validated with Rust 1.96.1:

- `cargo +1.96.1 fmt --all -- --check` — passed.
- `cargo +1.96.1 check --workspace --all-targets --all-features --locked` —
  passed.
- `cargo +1.96.1 clippy --workspace --all-targets --all-features --locked --
  -D warnings -D clippy::must-use-candidate` — passed.
- `cargo +1.96.1 nextest run --workspace --all-targets --all-features
  --locked` — passed, 309 tests passed, 0 skipped.
- `cargo +1.96.1 test --workspace --doc --all-features --locked` — passed.
- `$env:RUSTDOCFLAGS = '-D warnings'; cargo +1.96.1 doc --workspace
  --all-features --no-deps --locked; $result = $LASTEXITCODE; Remove-Item
  Env:RUSTDOCFLAGS; exit $result` — passed.
- For each package returned by `cargo +1.96.1 metadata --locked --no-deps
  --format-version 1`, `cargo +1.96.1 check --locked --manifest-path
  <package-manifest> --no-default-features` — passed. The existing
  `measurements` unused-import warnings remain; they are unrelated to this
  change.
- `pwsh -NoProfile -File scripts\check_crate_diagram.ps1` followed by
  `pwsh -NoProfile -File scripts\check_crate_diagram.ps1 -Check` — passed;
  the diagram was regenerated because the local `bodies` dependency changed
  Cargo metadata. The documented `just diagram` wrapper was unavailable in
  this environment, so the same script was invoked directly.
- `cargo +1.96.1 test -p dynamics-spherical-harmonics --all-features --locked`
  — passed, 26 unit tests and 2 doctests.
- `git diff --check` — passed; Git emitted only configured CRLF conversion
  notices for existing LF files.

The solid-tide numerical tests are independently reduced closed-form and
invariant checks of IERS equations, not externally sourced ephemeris/output
vectors.

## Explicit remaining #15 work

- Frequency-dependent solid-Earth corrections (IERS §6.2 Step 2).
- Ocean tide corrections and sourced ocean-tide models.
- Pole tides and their polar-motion inputs.
- Independent external solid-tide product/output reference vectors.
- Additional relativistic corrections beyond the already bounded correction-
  only Schwarzschild 1PN monopole slice.

No issue closure, network-data behavior, or complete tide/parity claim is
introduced.
