# OEM fuzz regressions

Every minimized input that exposes a panic, excessive resource use, or a
sequential/parallel acceptance mismatch belongs in this directory and must be
committed with a focused assertion when one is meaningful. The
`oem_conformance` integration test always feeds these files to the bounded
streaming reader, so a raw malformed byte sequence may be retained without
pretending that it is valid OEM.

Name cases after the behavior they protect, keep the exact minimized bytes,
and record the cargo-fuzz command and finding in the fixing commit. Markdown
files are ignored by the regression runner.
