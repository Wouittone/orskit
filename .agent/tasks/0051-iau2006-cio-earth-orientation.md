# Task: implement the full IAU 2006/2000A CIO Earth-orientation transform

- Issue: [#17](https://github.com/Wouittone/orskit/issues/17) (stage 2)
- ADR: [ADR-0051](../decisions/0051-iau2006-cio-earth-orientation.md)
- Supersedes the incomplete cloud-agent draft PR #33.

## Scope

- `frames_eop::Iau2006CioProvider` for GCRF to/from ITRF2020 using caller-owned
  UT1-UTC, polar-motion and celestial-pole-offset samples.
- IERS 2010 Tables 5.2a/5.2b/5.2d bundled as data files.
- No tides, no implicit data download, no bindings.

## Completion record

- [x] CIP `X, Y, s`, ERA, TIO locator and polar motion; analytic rotation rate
- [x] Active TIRS-to-ITRS polar-motion order and derivative cross terms tested
- [x] Direct `X`, `Y`, `s` and full matrix compared with pinned ERFA v2.0.1 test vectors at MJD 53736.0; redundant unknown-version 2025 matrix fixture removed
- [x] UT1-TAI continuity and inclusive coverage tested across the 2016 leap boundary; changing-slope derivatives compared with finite differences at three 2025 epochs
- [x] Guide, ADR, provenance and parity updates
- [x] Facade exposure through the existing `earth-rotation` feature
- [ ] Redistribution rights for bundled IERS coefficient tables substantiated (blocker: permission/license not found; see PROVENANCE and ADR-0051)

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
