# Curating approachable issues

This guide helps maintainers apply GitHub's `good first issue` and `help wanted`
labels consistently. Repository text can make work ready; creating or applying
hosted labels is an owner-operated action.

## `good first issue`

Use this label only when a contributor can finish the issue without making a
new architecture or scientific-policy decision. The issue should:

- describe one bounded outcome and name the relevant files or crate boundary;
- link the prerequisite project documentation and accepted design;
- state acceptance checks and, for physical behavior, the approved reference,
  units, frames, epochs, valid regime, and tolerance;
- identify likely failure cases and explicitly list work that is out of scope;
- have no unresolved provenance, licensing, or external-data question; and
- avoid bindings while the Rust core stabilization gate is active.

Good candidates include a missing test for an already specified invariant, a
small documentation correction, a parser diagnostic with an existing contract,
or a self-contained refactor with unchanged behavior. A new force model,
coordinate convention, estimator design, dependency, or parity-status decision
is not a first issue merely because its code might be short.

## `help wanted`

Use this label for a bounded, accepted outcome where outside expertise or
implementation capacity would help, but prior project experience may be
needed. Before labeling it, maintainers should settle architectural ownership,
provenance, validation criteria, and dependencies. Add a task brief for work
that crosses crates, changes public APIs, or introduces a numerical model.

Both labels can be present when the issue meets the stricter `good first issue`
criteria. Remove either label if review uncovers a policy decision or hidden
dependency, and record the blocker in the issue.

## Maintainer checklist

- [ ] The issue maps to an existing parity row or explicitly says it is
      documentation/tooling work.
- [ ] Acceptance criteria are observable and no parity outcome is promised by
      compilation alone.
- [ ] Relevant tests and raw Cargo commands are named.
- [ ] References and data have known terms, or no external material is needed.
- [ ] The scope excludes adjacent cleanup and parked bindings.
- [ ] A maintainer is available to answer domain questions and review promptly.

After merge, close the issue only when its acceptance checks pass and its
documentation, provenance record, and parity evidence are consistent. The
maintainer who merges the change applies any resulting hosted labels or
milestones; contributors should not be asked to simulate those actions in
repository files.
