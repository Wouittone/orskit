# Security policy

orskit is pre-alpha and not suitable for operational flight dynamics, safety of
life, or production mission assurance workflows.

## Supported versions

Only the current `main` branch is considered for security fixes. No release line
is supported yet.

## Reporting a vulnerability

Please report security issues privately instead of opening a public issue when
the report could enable exploitation. If the repository owner has not published
a dedicated security contact yet, use GitHub's private vulnerability reporting
for this repository when available; otherwise contact the maintainer through a
non-public channel and include enough detail to reproduce the issue.

Useful reports include:

- affected commit, crate, binding, or workflow;
- reproduction steps or a minimal proof of concept;
- expected impact and attacker assumptions;
- whether the issue affects scientific correctness, memory safety, supply
  chain integrity, CI credentials, generated artifacts, or bindings.

## Security scope

Security reports are especially relevant for:

- memory safety across FFI boundaries;
- panics or undefined behavior reachable from untrusted inputs;
- parser denial-of-service behavior in operational formats;
- dependency, build, release, and CI supply-chain risks;
- hidden network access or implicit scientific-data downloads;
- incorrect provenance or licensing that could compromise redistribution.

Scientific correctness bugs that could produce materially wrong physical
results should be reported even when they are not traditional security issues.
They will be tracked as correctness defects with the same bias toward clear
reproduction and evidence.
