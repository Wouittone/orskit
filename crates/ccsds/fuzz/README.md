# Bounded OEM parser fuzzing

This cargo-fuzz workspace is isolated from the orskit workspace and release
dependency graph. `libfuzzer-sys` is used only by this developer harness under
its `(MIT OR Apache-2.0) AND NCSA` license.

Use a nightly Rust toolchain, install cargo-fuzz, and run the target from the
repository root:

```powershell
cargo install cargo-fuzz --version 0.13.2 --locked
New-Item -ItemType Directory -Force crates/ccsds/fuzz/corpus/oem_kvn
Copy-Item crates/ccsds/testdata/*.oem crates/ccsds/fuzz/corpus/oem_kvn/
Push-Location crates/ccsds
cargo +nightly fuzz run oem_kvn fuzz/corpus/oem_kvn -- -max_len=262144 -timeout=10
cargo +nightly fuzz run oem_xml fuzz/corpus/oem_xml -- -max_len=262144 -timeout=10
Pop-Location
```

The harness independently rejects inputs above 256 KiB and configures finite
line, section, document-byte, and line-count limits. It consumes arbitrary
bytes through the blocking reader. UTF-8 inputs additionally require the
sequential and ordered-parallel collectors to agree on acceptance and, for
accepted documents, on the complete parsed value.

The `oem_xml` target consumes arbitrary bytes through the bounded blocking XML
event reader and also exercises the collecting API for UTF-8 inputs. Seed its
corpus with project-authored XML under `../testdata/`; minimized XML findings
belong in `../testdata/fuzz-regressions/xml/`.

Do not commit generated corpora, artifacts, or coverage data. For every
finding, minimize it with `cargo +nightly fuzz tmin`, copy the exact input to
`../testdata/fuzz-regressions/`, and add a focused assertion to
`../tests/oem_conformance.rs` when the expected error or semantic result is
stable. This turns each discovery into an ordinary regression test. Record
`rustc +nightly -Vv` with the finding so the run can be reproduced.
