# Contributing to orskit

orskit is pre-alpha scientific infrastructure. Contributions are welcome, but
the bar for provenance and physical semantics is intentionally high: a feature
is not considered present until its units, frames, epochs, data needs,
tolerances, and evidence are inspectable.

## Contribution license

By submitting a contribution, you agree that your contribution is your original
work or is material you are authorized to contribute, and that it may be
distributed under orskit's dual license:

- MIT; or
- Apache-2.0.

Do not submit source code, tests, examples, figures, or distinctive prose copied
or translated from Orekit, Lox, Nyx, or another astrodynamics project unless the
repository has explicitly accepted that exact reuse and its license terms in
writing.

## Before opening a change

1. Read `.agent/PROVENANCE.md` and `.agent/PARITY.md`.
2. Identify the capability row the change advances.
3. For scientific behavior, record public references, valid regimes, units,
   frame/time-scale assumptions, error behavior, and tolerances.
4. For durable architecture choices, add or update an ADR in
   `.agent/decisions/`.
5. For multi-file or cross-crate work, add a task brief in `.agent/tasks/`.

## Change expectations

- Prefer small vertical slices over broad placeholder surfaces.
- Add tests with the behavior, including failure modes and at least one
  invariant or independent reference where applicable.
- Keep domain behavior in safe Rust core crates; bindings should remain thin.
- Never add hidden network access or process-global scientific data.
- Update `.agent/PARITY.md` honestly. `Validated` requires the evidence listed
  in that file.
- Update `.agent/PROVENANCE.md` when a new reference, dataset, dependency, or
  black-box behavior sample affects a claim.

## Local checks

Use the checks that match the files touched. For Rust core changes, the default
set is:

```powershell
cargo fmt --all --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo doc --workspace --all-features --no-deps
```

Documentation-only changes should at least pass `git diff --check` and should
be reviewed for stale links, unsupported parity claims, and provenance gaps.

## Pull request checklist

- [ ] The change has one clear semantic outcome.
- [ ] The affected parity row and known gaps are updated.
- [ ] New references or datasets are recorded in provenance docs.
- [ ] Relevant checks were run, or skipped checks are explained.
- [ ] The contribution is original or explicitly approved compatible material.
