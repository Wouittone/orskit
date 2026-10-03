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
- [x] Matrix verified against ERFA to 1e-11; finite-difference angular velocity
- [x] Guide, ADR, provenance and parity updates
- [x] Facade exposure through the existing `earth-rotation` feature
