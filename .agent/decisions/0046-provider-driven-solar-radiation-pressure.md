# ADR-0046: provider-driven solar radiation pressure

- Status: Proposed
- Date: 2026-09-20
- Affected parity rows: propagation/dynamics and force-model composition;
  geometry/celestial bodies and ephemerides

## Decision

The dedicated `dynamics-srp` crate implements a cannonball direct-solar
radiation-pressure `EvaluableCartesianForceModel`. It receives a caller-owned
Sun `BodyEphemerisProvider`, a caller-owned reference-distance
`SolarFluxProvider`, spacecraft area/mass/optical coefficient, and an explicit
eclipse geometry. No ephemeris, solar-flux data, body radius, or frame
transformation is bundled.

The initial geometry is a cylindrical umbra: a state is fully eclipsed only
when it is behind the configured occulting-body origin along the body-to-Sun
line and its perpendicular distance is no greater than the configured body
radius. Penumbra and solar-disc angular radius are deliberately not modeled.
The state must use explicitly inertial axes and be centered on the occulting
body; provider results are checked for requested body, epoch, and frame.

## Consequences

The parent dynamics facade must add the crate as a workspace member and expose
it behind an SRP feature. Applications must select and document the flux
reference distance and provider data provenance. A future conical-shadow model
can be added as another explicit geometry variant without changing provider
contracts.
