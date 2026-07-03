# Instructions for orskit agents

These instructions apply to every contribution performed by an automated
agent. The project is pre-alpha: correctness, provenance, and coherent domain
boundaries matter more than preserving the current scaffold.

## Before changing code

1. Read `VISION.md` and `PROVENANCE.md`.
2. Locate the affected capability in `PARITY.md`.
3. Read the relevant boundaries in `ARCHITECTURE.md` and checks in
   `ENGINEERING.md`.
4. Inspect the repository and its tests. Never infer current behavior from the
   roadmap or README.
5. Write a task brief using `templates/task.md` when work spans more than one
   module, changes a public API, or introduces a numerical model.

## Non-negotiable rules

- Implement independently. Do not copy, translate, port, or mechanically
  transform code from Orekit, Lox, Nyx, or another astrodynamics library unless
  the repository has explicitly approved that exact reuse and its resulting
  licensing in writing. The default is no source reuse.
- Treat the Nyx astrodynamics implementation, tests, examples, and internal
  structure as implementation-prohibited. Separately published crates may be
  used unmodified after a license/API audit; Hifitime is an approved MPL-2.0
  dependency and the canonical orskit time API.
- Use Orekit as a capability and behavior reference, not as a source-language
  implementation to translate.
- Never claim feature parity from the existence of a type or method. A parity
  claim requires the evidence defined in `PARITY.md`.
- Make reference frame, epoch/time scale, units, and data context explicit at
  every boundary where ambiguity could change a physical result. Public
  physical values must use typed quantities; raw scalars are restricted to
  explicitly unit-named numerical, serialization, and FFI boundaries.
- Keep the scientific core safe Rust. Confine necessary `unsafe` to small,
  reviewed interoperability modules with documented safety invariants.
- Keep language bindings thin. Domain behavior belongs in Rust and must be
  testable without Python or a JVM.
- Until the Rust core contracts stabilize, restrict binding edits to the
  minimum needed to preserve compilation; do not add binding features.
- Do not trade correctness for speed without a measured error budget. Do not
  claim a performance improvement without a reproducible benchmark.
- Do not introduce hidden network access, mutable process-global scientific
  data, or silently downloaded ephemeris/Earth-orientation data.
- Prefer focused dependencies. Every dependency must have a compatible license,
  a clear purpose, and a maintenance rationale.

## Change discipline

- Deliver vertical slices: domain API, implementation, tests, documentation,
  and parity-ledger update together.
- Preserve unrelated user changes and avoid opportunistic rewrites.
- Add an architecture decision record for durable, cross-cutting choices.
- Define domain errors; do not use panics for recoverable inputs or let panics
  cross an FFI boundary.
- Document equations, conventions, authoritative references, valid ranges,
  singularities, and numerical tolerances near the implementation.
- Keep public APIs Rust-native. Similar capability to Orekit does not require a
  class-for-class Java API clone.

## Definition of done

A change is complete only when all applicable items hold:

- The relevant tests cover nominal behavior, boundary cases, failure modes,
  and at least one independent reference or invariant.
- Formatting, linting, unit/integration tests, and documentation checks pass.
- Numerical tolerances are justified and expressed in physical terms.
- Public APIs have examples and state their units, frames, time scales, and
  error behavior.
- New external material is recorded according to `PROVENANCE.md`.
- `PARITY.md` is updated with an honest status and evidence links.
- Binding surfaces are updated or explicitly deferred until the Rust API is
  stable enough to expose.
- Performance-sensitive work includes a before/after benchmark and accuracy
  comparison.

If a requested shortcut conflicts with these rules, surface the conflict and
propose the smallest compliant path.
