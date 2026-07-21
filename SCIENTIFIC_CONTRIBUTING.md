# Contributing scientific evidence without writing Rust

orskit welcomes domain experts, analysts, operators, technical writers, and
standards specialists. A useful contribution does not need to contain code.
Scientific review is especially valuable while the Rust core contracts are
still taking shape.

Start with the [project scope](.agent/VISION.md), [capability ledger](.agent/PARITY.md),
and [clean-room policy](.agent/PROVENANCE.md). The ledger distinguishes an
implemented prototype from a validated capability and identifies the evidence
still needed.

## Standards and literature research

An issue or review can establish the scientific contract before anyone writes
code. Include:

- the exact title, authoring organization or authors, revision/date, and a
  stable locator;
- the document's license or access terms, especially for redistributable
  tables and test data;
- the equations, conventions, valid ranges, singularities, and units relevant
  to the proposed behavior;
- the frame, origin, epoch, time scale, and external-data assumptions; and
- ambiguities or conflicts between revisions that require a maintainer choice.

Summarize facts in your own words and quote only what is necessary. Do not copy
source, tests, examples, figures, or distinctive prose from Orekit, Lox, Nyx,
or another implementation. Public standards, textbooks, papers, and public API
behavior may inform an independent implementation under the rules in
`.agent/PROVENANCE.md`.

## Reference vectors and scenarios

A strong reference-vector contribution is small enough to inspect and complete
enough to reproduce. Provide:

1. input values with units, ordering, frames, origins, epochs, and time scales;
2. the expected output and its precision or uncertainty;
3. the reference version and exact procedure that produced it;
4. an accuracy tolerance justified by the reference and intended use, not by
   the current implementation; and
5. edge cases or failure cases near the model's supported boundary.

Prefer published standards, analytic invariants, government or institutional
datasets, or an independently implemented tool. If a comparison uses another
program, record its version, configuration, and only its observed public
inputs/outputs. Nyx may be executed only as an unmodified black box in the
separate validation boundary described by the provenance policy.

Do not attach a dataset until redistribution is permitted. When redistribution
is unclear, link to the authoritative source and provide a checksum and
retrieval instructions instead. Record coverage intervals, conventions, and
any transformation applied to the original data. Never include credentials,
licensed mission data, export-controlled material, or personal data.

## Review-only contributions

You can contribute by opening a capability or provenance issue, reviewing a
task brief or ADR, checking a tutorial's scientific assumptions, or reproducing
a published result. A useful review names the exact statement or scenario and
separates:

- a correctness defect;
- an undocumented limitation;
- missing validation evidence; and
- a feature request beyond the current contract.

When reporting a numerical disagreement, include both results, units, reference
frames, epochs/time scales, software and data versions, configuration, and a
minimal input. Do not infer a cause from the numerical difference alone.

Use the repository's capability-request or provenance-question issue form.
Maintainers will translate accepted evidence into a task brief and link it from
the parity ledger. Applying hosted labels and accepting a parity claim remain
maintainer actions.
