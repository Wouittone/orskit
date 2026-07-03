# Provenance and clean-room policy

This policy protects the independent character of orskit and its intended
MIT/Apache-2.0 licensing. It is an engineering policy, not legal advice.

## Default rule

All project-owned implementation, tests, examples, and documentation must be
original work contributed under MIT or Apache-2.0. A permissive upstream
license does not automatically authorize importing code into the dual-licensed
project: reuse must be deliberate, attributed, and approved first. Without
that approval, implement from public scientific descriptions and observed
behavior.

## Reference classes

| Reference | Allowed use | Prohibited use by default |
| --- | --- | --- |
| Standards, textbooks, and papers | Equations, conventions, test values, and algorithms with citation | Copying protected prose, figures, or code |
| Orekit documentation and public API behavior | Capability inventory, terminology research, black-box comparison, and independent test expectations | Translating or structurally porting source code, tests, or internal design |
| Nyx astrodynamics project material | High-level awareness of user needs and ergonomics | Copying or adapting source, tests, examples, docs, internal architecture, identifiers, or distinctive expression |
| Separately published permissive/MPL crates | Unmodified dependency use after license, API, and maintenance review | Copying dependency source into project-owned MIT/Apache-2.0 files |
| Other open-source libraries | Public behavior research and separately licensed dependencies after audit | Source reuse without explicit compatibility review and attribution |
| Public datasets | Validation when redistribution and use terms are recorded | Vendoring or redistributing data without confirmed permission |

Treat the Nyx astrodynamics implementation as out of bounds even if a mirror,
fork, or future release presents different licensing. This does not prohibit
using a separately packaged crate such as Hifitime, unmodified and under its
own compatible license, after an explicit dependency decision.

## Required research record

For every new scientific model, file format, or differential test, record:

- the exact title, authoring organization, version/date, and stable locator;
- whether it is a standard, paper, documentation, dataset, behavior sample, or
  dependency;
- its license or access terms when applicable;
- what facts were learned from it;
- what was intentionally not copied; and
- the tests or code that use those facts.

Put concise citations next to equations and test vectors. Add a row to the
ledger below when the reference affects more than one module or establishes a
parity claim.

## Project reference ledger

| Area | Reference and version | Class/terms | Permitted use | Code or evidence |
| --- | --- | --- | --- | --- |
| Project scope | Orekit public capability documentation; baseline not yet pinned | Documentation; terms to record when pinned | Capability inventory only | `.agent/PARITY.md` |
| Design inspiration boundary | Nyx; version deliberately not consulted | AGPL boundary specified by project owner | High-level awareness only; no implementation material | This policy |
| Time | [Hifitime 4.3](https://docs.rs/hifitime/4.3.0/hifitime/) | MPL-2.0 dependency | Direct, unmodified epoch/time API | `crates/core`, `crates/measurements` |
| Units | [`uom` 0.38](https://docs.rs/uom/0.38.0/uom/) | MIT OR Apache-2.0 dependency | Direct dimensional quantities | `crates/units` |
| Frame survey | [`lox-frames` 0.1.0-alpha.11](https://docs.rs/lox-frames/0.1.0-alpha.11/lox_frames/) and [ANISE](https://docs.rs/anise/latest/anise/) | MPL-2.0 dependencies; not adopted in this slice | API/capability evaluation only | `.agent/decisions/0001-foundational-types.md` |

## Dependency policy

- Confirm the license from package metadata and the upstream repository before
  adding a dependency.
- Prefer MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, or similarly
  compatible terms; escalate anything else for explicit review.
- Record feature flags and disable unnecessary defaults.
- Generate and review a dependency/license report in CI before releases.
- A dependency is not copied source, but it still forms part of the distributed
  and linked product and must be compatible with each target.

## If provenance is uncertain

Stop the affected implementation. Preserve a short factual note, replace the
questionable material with an independently derived version, and request a
maintainer review. Do not try to make copied material "different enough."
