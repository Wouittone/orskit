# TLE parser fuzzing

This standalone cargo-fuzz workspace is not part of the shipping dependency
graph. The target accepts arbitrary bytes, parses only UTF-8 text, and retains
small regression inputs in `corpus/two_line_element`.

From this directory, run:

```sh
cargo install cargo-fuzz --locked
cargo +nightly fuzz run two_line_element -- -max_len=140 -timeout=10
```

Minimize discoveries and retain them in the corpus. Generated artifacts and
corpora from local fuzz runs must not be committed.
