# Agent workflow

## 1. Orient

- Read `AGENTS.md`, inspect the current code/tests, and check the working tree.
- Select one capability row in `PARITY.md` and define a vertical outcome.
- Identify affected layers and verify dependency direction in `ARCHITECTURE.md`.
- Create a task brief from `templates/task.md` for non-trivial work.

## 2. Research without contaminating implementation

- Decide which public standards, papers, documentation, behavior, and datasets
  are needed.
- Classify each source under `PROVENANCE.md` before consulting implementation
  material.
- Capture conventions, equations, regimes, and test vectors—not source
  expression or upstream internal structure.
- If reference behavior is needed, preserve versioned inputs and outputs so the
  comparison is reproducible and can run without the reference installed.

## 3. Design the contract

Write down:

- physical inputs/outputs and their units, frames, epochs, and time scales;
- valid regimes, singularities, and error behavior;
- required external data and how it is supplied;
- extension points and dependency direction;
- validation vectors, invariants, and tolerances; and
- allocation/performance expectations if the path is hot.

Use `templates/adr.md` when the choice affects multiple capability families,
public compatibility, data ownership, safety, or FFI.

## 4. Implement a vertical slice

- Start with the smallest domain type or behavior that can be validated.
- Keep model logic in Rust core crates and adapters at the edges.
- Add errors and tests with the implementation, not afterward.
- Document references and conventions next to the relevant code.
- Benchmark only after the correct behavior is covered.

## 5. Review adversarially

Check for:

- implicit units, frames, epochs, time scales, constants, or datasets;
- singularities, NaNs, overflow, non-convergence, and invalid input;
- unjustified tolerances or reference values with unclear provenance;
- panics, unsafe assumptions, ABI ownership mistakes, and hidden allocation;
- accidental dependency cycles or implementation in a binding; and
- claims that exceed the supplied evidence.

Run all applicable commands in `ENGINEERING.md`. State skipped checks and why.

## 6. Close the loop

- Update the `PARITY.md` row and link concrete evidence.
- Update public docs/examples and the roadmap if sequencing changed.
- Record new cross-cutting decisions and provenance.
- Summarize what is supported, the validated regime, known gaps, and exact
  checks run.

## Scope and handoff

One task should have one clear owner and one parity outcome. Split research,
implementation, or review only when their file ownership and deliverables do
not overlap. A handoff must include the task brief, decisions, source policy,
changed files, failed experiments, and remaining checks.

Never leave placeholder methods that return physically plausible dummy values.
Use an explicit unsupported error or omit the API until it has real semantics.
