# Orekit capability baseline: 13.1.7

This file pins the Orekit release used for orskit's capability inventory. It is
not an implementation source and does not authorize translating Orekit source,
tests, examples, internal structure, or distinctive prose.

## Baseline identity

- **Reference project:** Orekit Java
- **Pinned version:** 13.1.7
- **Release date:** 2026-07-03
- **Pinned by:** orskit maintainers
- **Pinned on:** 2026-07-07
- **Inventory revision:** `orekit-13.1.7-2026-07-07`
- **Primary capability sources:**
  - <https://www.orekit.org/download.html>
  - <https://www.orekit.org/news.html>
  - <https://www.orekit.org/doc-javadoc.html>
  - <https://www.orekit.org/site-orekit-13.1.7/apidocs/index.html>
- **License/terms recorded for inventory use:** Orekit is Apache-2.0; Orekit
  website pages are CC BY 3.0 unless otherwise noted by the site.

## Allowed use

- Inventory public capability families and public API behavior.
- Name gaps in `.agent/PARITY.md` against this pinned release.
- Build black-box comparison harnesses that exercise public APIs.
- Cite public documentation for terminology and acceptance criteria.

## Prohibited use

- Reading, copying, translating, or structurally porting Orekit source,
  examples, tests, or internal implementation design into orskit-owned files.
- Treating a matching type name or module name as parity evidence.
- Reporting a project-wide parity percentage without a published weighting
  method.

## Versioning rules

1. `.agent/PARITY.md` records this pinned baseline and inventory revision.
2. New or changed parity rows must state whether evidence is:
   - current for this baseline;
   - older-but-still-labeled evidence that must be refreshed; or
   - independent evidence not derived from Orekit.
3. Existing fixtures generated against Orekit 13.1.6 remain valid only as
   explicitly labeled historical evidence until regenerated or reviewed against
   13.1.7.
4. Refreshing the baseline requires a new baseline file, a provenance update,
   and a parity-ledger revision note. Do not silently retarget `latest` links.

## Initial inventory families

The parity ledger groups capabilities by domain rather than mirroring Java
packages. The first pass uses these public Orekit capability areas as inventory
inputs:

- time, frames, bodies, geodesy, and scientific data context;
- orbit/state representations, conversions, anomalies, and interpolation;
- analytical, numerical, and semi-analytical propagation;
- gravity, atmosphere, radiation, maneuvers, relativity, tides, and empirical
  force families;
- events, ephemerides, covariance, variational equations, and estimation;
- attitude states/providers and attitude-dependent force/torque effects;
- observation participants, measurements, modifiers, and orbit determination;
- operational formats and mission-analysis workflows.

This is intentionally an inventory surface, not a design mandate. orskit's
architecture remains Rust-native and evidence-driven.
