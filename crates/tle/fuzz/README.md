# Bounded TLE parser checks

This isolated cargo-fuzz workspace is developer tooling and is not part of the
shipping orskit dependency graph. Its input limit is 140 bytes: two 69-byte TLE
lines plus one line separator. The parser itself requires exact fixed-width
ASCII input and performs no input-sized allocation before those checks.

From `crates/tle`, run:

```powershell
cargo install cargo-fuzz --locked
cargo +nightly fuzz run two_line_element -- -max_len=140 -timeout=10
```

For every discovery, minimize the input, copy it unchanged into
`../testdata/parser-regressions/`, and add a focused semantic assertion to the
ordinary integration test when applicable. Record the nightly compiler and
command in the fixing change. Generated corpora and artifacts remain untracked.
