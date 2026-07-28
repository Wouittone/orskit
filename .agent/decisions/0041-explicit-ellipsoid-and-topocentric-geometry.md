# ADR-0041: keep ellipsoid geometry separate from frame identity

- Status: Accepted
- Date: 2026-07-24
- Affected parity rows: bodies/ellipsoids/geodesy; frames; ground participants

## Context

Body identity does not select a shape, datum, rotation, or ephemeris. Geodetic
coordinates require an ellipsoid, while local topocentric axes additionally
require a body-fixed parent frame and an explicit longitude at a pole. Placing
all of this in either `bodies` or `frames` would create a misleading implicit
model or reverse the intended dependency direction.

## Decision

1. `bodies::ReferenceEllipsoid` binds caller-selected oblate geometry to a
   `Body`; WGS 84 is the first sourced built-in.
2. `GeodeticPosition` uses east-positive longitude in `[-π, π]`, ellipsoidal
   latitude in `[-π/2, π/2]`, and ellipsoidal height along the outward normal.
3. The ellipsoid performs EPSG 9602 conversion in conventional body-fixed
   geocentric axes. Exact body-center input and exact polar-axis inverse input
   are typed errors because coordinates or longitude are not unique.
4. `FrameCatalog` issues East–North–Up identities only from a registered,
   affirmatively body-fixed parent matching the ellipsoid body. Earth ITRF
   realizations carry that meaning directly; other bodies require an explicit
   application-defined body-fixed orientation. Merely non-inertial frames,
   including TEME and generic custom axes, are rejected.
   The resulting frame owns reversible position transforms but implies no
   epoch-dependent terrestrial/celestial rotation.
5. At a geodetic pole, forward conversion and topocentric construction require
   the caller's explicit longitude; no hidden meridian is selected.

## Alternatives considered

- Attaching WGS 84 directly to `Body::EARTH` was rejected because identity must
  not select physical data.
- Returning a naked rotation matrix was rejected because matrices are an
  internal numerical representation, not the supported domain API.
- Canonicalizing polar longitude to zero was rejected because it silently
  chooses east and north directions.
- Putting topocentric construction in `bodies` was rejected because only
  `frames` owns catalogued frame identities and parent validation.

## Consequences

Ellipsoid conversion remains reusable without a frame catalog. Topocentric
frames are explicit leaf geometry above a selected body-fixed parent.
Orthometric/geoid heights, displacement, Earth orientation, station wiring,
and other ellipsoids remain separate future evidence slices.

## Validation

NGA parameter assertions pin WGS 84. EPSG 9602's published North Sea vector
checks forward conversion and a reverse round trip. EPSG 9836's published
origin/point pair checks East–North–Up conversion and exact inverse recovery.
Tests also cover center, polar-axis, body mismatch, and inertial-parent errors.

## Provenance

Only NGA's published WGS 84 defining parameters and the public EPSG registry
equations/examples for methods 9602 and 9836 were used. No external
implementation source or tests were consulted or copied.
