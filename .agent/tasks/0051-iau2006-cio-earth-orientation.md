# Task: implement the full IAU 2006/2000A CIO Earth-orientation transform

- Issue: [#17](https://github.com/Wouittone/orskit/issues/17) (stage 2)
- ADR: [ADR-0051](../decisions/0051-iau2006-cio-earth-orientation.md)
- Supersedes the incomplete cloud-agent draft PR #33.

## Scope

- `frames_eop::Iau2006CioProvider` for GCRF to/from ITRF2020 using caller-owned
  UT1-UTC, polar-motion and celestial-pole-offset samples.
- Full IAU 2006/2000A model through unmodified `erfa` 0.2.1; no bundled
  IERS table files after the owner-approved dependency replacement.
- No tides, no implicit data download, no bindings.

## Completion record

- [x] CIP `X, Y, s`, ERA, TIO locator and polar motion; product-rule rotation
  rate with validated sixth-order celestial derivatives and analytic EOP/ERA/TIO rates
- [x] Active TIRS-to-ITRS polar-motion order and derivative cross terms tested
- [x] Direct `X`, `Y`, `s` and full matrix compared with pinned ERFA v2.0.1 test vectors at MJD 53736.0; redundant unknown-version 2025 matrix fixture removed
- [x] UT1-TAI continuity and inclusive coverage tested across the 2016 leap boundary; changing-slope derivatives compared with finite differences at three 2025 epochs
- [x] Guide, ADR, provenance and parity updates
- [x] Facade exposure through the existing `earth-rotation` feature
- [x] Unverified bundled IERS tables/evaluator deliberately replaced with an
  unmodified licensed dependency; actual MPL/ERFA package notices verified
- [x] Maintenance review recorded: `erfa` has no release/push since November
  2022; accepted as a pinned fixed-standard dependency with verified API,
  independent reference tests and full Rust 1.96.1 gates, without claiming
  active maintenance

## PR #38 follow-up validation (2026-10-10)

Existing checkout `bot/iers-cio-earth-orientation`, Windows, Rust 1.96.1:

| Exact command | Result |
| --- | --- |
| `cargo +1.96.1 nextest run --workspace --all-targets --all-features --locked` | Passed: 270 tests, 0 skipped, including all three 100 MiB CCSDS benchmark targets |
| `cargo +1.96.1 test -p frames-eop --all-features --locked` | Passed: 21 unit tests and 1 doctest |
| `cargo +1.96.1 fmt --all --check` | Passed |
| `git diff --check` | Passed |
| `cargo +1.96.1 clippy --workspace --all-targets --all-features --locked -- -D warnings -D clippy::must-use-candidate` | Passed |
| `cargo +1.96.1 check --workspace --all-targets --all-features --locked` | Passed |
| `cargo +1.96.1 check -p orskit --no-default-features --features earth-rotation --locked` | Passed |
| `cargo +1.96.1 test --workspace --doc --all-features --locked` | Passed: 14 doctests |
| `cargo +1.96.1 doc --workspace --all-features --no-deps --locked` with `RUSTDOCFLAGS=-D warnings` | Passed |

Rust Analyzer and docs.rs MCP tools were unavailable in this session.
Reference-version ambiguity was resolved by removing the redundant fixture,
not by claiming to have regenerated it. The pinned published ERFA cases and
three-epoch changing-EOP derivative coverage remain. Publication remains
blocked on the narrow [owner decision](../PROVENANCE.md#iers-table-distribution-review-2026-10-10)
about the actual bundled table files; passing tests do not resolve that issue.

## Owner-approved dependency replacement (2026-10-10)

Replaced exactly the three obsolete coefficient files and the original table
evaluator with pinned `erfa` 0.2.1. No dependency source was copied or modified.
The actual package combines MPL-2.0 and ERFA BSD-style attribution terms;
the license/array hashes, safety/API audit and alternative review are in
[PROVENANCE](../PROVENANCE.md#erfa-dependency-replacement-review-2026-10-10).
`NOTICE` supplies binary attribution and immutable source locators.

Independent pinned reference cases and changing-EOP/leap/polar-order tests
remain. `X/Y/s` reference tolerances improve to 1e-14/1e-14/1e-18 rad.
The sixth-order 512 s TT derivative agrees with refinement and an independent
fourth-order stencil within 2e-18/2e-18/2e-20 rad/s over 257 epochs (1972-2100).
Maximum six-epoch shifts from the former analytic table evaluator are
5.846e-12 rad and 2.744e-17 rad/s. This is numerical evidence, not a claim
that table redistribution was retroactively authorized.

Maintenance clarification: active maintenance is not established, but the
owner confirms it was a preference rather than a hard gate. Repository policy
requires review, not automatically excluding dormant standardized libraries.
The fixed-standard pinned dependency is accepted with the verified
license/API, independent reference tests and complete Rust 1.96.1 gates.
Upstream inactivity is retained as a future fixes/compatibility risk. No actual
scientific, provenance or safe-Rust blocker remains for this replacement.
Binary notice/source-availability obligations still apply. Do not push.

### Replacement validation results

All commands ran in the existing `bot/iers-cio-earth-orientation` worktree
with Rust 1.96.1; no push, merge, rebase or other worktree changes occurred.

| Exact command | Result |
| --- | --- |
| `cargo +1.96.1 check -p frames-eop` | Passed; Cargo generated the reviewed lockfile additions |
| `cargo +1.96.1 test -p frames-eop --locked -- --nocapture` | Passed: 23 unit tests and 1 doctest; printed derivative/regression maxima |
| `cargo +1.96.1 fmt --all --check` | Passed |
| `cargo +1.96.1 check --workspace --all-targets --all-features --locked` | Passed |
| `cargo +1.96.1 clippy --workspace --all-targets --all-features --locked -- -D warnings -D clippy::must-use-candidate` | Passed |
| `cargo +1.96.1 nextest run --workspace --all-targets --all-features --locked` | Passed: 272 tests, including the three 100 MiB CCSDS benchmark targets |
| `cargo +1.96.1 test --workspace --doc --all-features --locked` | Passed |
| `cargo +1.96.1 doc --workspace --all-features --no-deps --locked` with `RUSTDOCFLAGS=-D warnings` | Passed |
| `cargo +1.96.1 check -p orskit --no-default-features --features earth-rotation --locked` | Passed |
| `cargo +1.96.1 test -p orskit --no-default-features --features earth-rotation --locked` | Passed (facade has no unit tests; minimal feature and dependencies compiled) |
| `pwsh -NoProfile -File scripts\check_crate_diagram.ps1 -Check` | Passed; no workspace-diagram changes required |
| `cargo +1.96.1 package -p frames-eop --list --allow-dirty --locked` | Passed; includes `NOTICE`, no obsolete IERS tables |
| `cargo +1.96.1 nextest list --workspace --all-targets --all-features --locked --message-format json` | Passed; confirms 272-test inventory |
| `git diff --check` | Passed |

All requested numerical/build/documentation gates pass. They support the
bounded maintenance acceptance above; they do not imply active maintenance or
waive dependency distributor license obligations.
