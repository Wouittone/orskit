# Task 0018: contributor governance and issue intake

## Parity target

- Ledger row: project governance and provenance process
- Current status: provenance and ADR policies exist under `.agent`, but public
  contributor, security, conduct, and issue-intake files are missing
- Intended status after this task: contributors can find licensing,
  provenance, security, conduct, and issue-template expectations from the
  repository root and GitHub issue UI

## User workflow

Open the repository, learn how to contribute safely, report security or
scientific-correctness concerns, and file issues that capture units, frames,
parity rows, and provenance before implementation starts.

## Scientific contract

- Inputs and units: none.
- Outputs and units: governance documentation and issue metadata.
- Frames/epochs/time scales: issue templates ask reporters to identify them
  when relevant.
- Conventions and valid regimes: pre-alpha only; no release line is supported
  for security fixes.
- External data requirements: none.
- Errors and singularities: copied implementation material, unsupported parity
  claims, and hidden provenance assumptions are rejected by policy.

## Provenance

| Reference | Class/terms | Facts used | Evidence/code affected |
| --- | --- | --- | --- |
| Original orskit project policy | Project-owned MIT/Apache-2.0 work | Contributor licensing, issue intake, security reporting, and conduct expectations | `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, `.github/ISSUE_TEMPLATE/*` |

## Design

- Affected crates/layers: repository governance docs and GitHub issue intake.
- Public API: none.
- Rejected alternatives: copy a stock code of conduct without tailoring it to
  scientific/provenance review; defer public contributor docs while `.agent`
  policies remain agent-oriented only.
- ADR required: no; this executes the existing roadmap governance item.

## Validation

- Unit cases: none.
- Invariants/properties: public docs point contributors toward provenance,
  parity, and original-work requirements; issue templates collect physical
  context and material-use intent.
- Independent reference vectors: none.
- Differential/scenario tests: none.
- Tolerances and justification: none.
- Benchmarks: none.

## Completion checklist

- [x] Public governance docs added
- [x] Issue templates added
- [x] README and roadmap updated
- [x] Provenance recorded
- [x] Parity ledger impact considered; no scientific capability row changed
- [x] Relevant checks pass
- [x] Binding impact not applicable
