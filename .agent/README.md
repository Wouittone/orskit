# orskit agent handbook

This directory is the durable operating handbook for humans and coding agents
working on orskit. It records the constraints that must survive individual
issues, sessions, and contributors.

## Reading order

1. [`AGENTS.md`](AGENTS.md) — mandatory instructions for every change.
2. [`VISION.md`](VISION.md) — mission, scope, and non-goals.
3. [`PROVENANCE.md`](PROVENANCE.md) — clean-room and licensing rules.
4. [`ARCHITECTURE.md`](ARCHITECTURE.md) — target boundaries and data model.
5. [`FORCE_MODELS.md`](FORCE_MODELS.md) — dynamics effect and dependency inventory.
6. [`ENGINEERING.md`](ENGINEERING.md) — implementation and validation bar.
7. [`PARITY.md`](PARITY.md) — capability inventory and evidence ledger.
8. [`ROADMAP.md`](ROADMAP.md) — delivery order and release gates.
9. [`WORKFLOW.md`](WORKFLOW.md) — repeatable task lifecycle.

Accepted cross-cutting choices live in [`decisions/`](decisions/).

The templates in [`templates/`](templates/) are starting points for scoped
work and architecture decisions.

## Sources of truth

| Question | Source |
| --- | --- |
| What are we building? | `VISION.md` |
| May this reference be used? | `PROVENANCE.md` |
| Where should this code live? | `ARCHITECTURE.md` |
| Which force and torque families should dynamics cover? | `FORCE_MODELS.md` |
| What quality is required? | `ENGINEERING.md` |
| Is a capability complete? | `PARITY.md` plus linked evidence |
| What comes next? | `ROADMAP.md` |
| How should an agent execute a task? | `WORKFLOW.md` |

When the documents disagree, stop and repair the inconsistency in the same
change. Do not silently choose the most convenient interpretation.
