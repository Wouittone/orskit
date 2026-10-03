# Caller-supplied spherical-harmonic gravity

`dynamics-spherical-harmonics` evaluates a static geopotential through the
caller-selected degree and order. Enable the `dynamics/spherical-harmonics`
feature (or the public facade's `spherical-harmonics` feature), then provide:

- a fully normalized 4π coefficient provider, with source/product/revision,
  optional checksum, degree/order coverage, tide system, coefficient frame,
  and epoch semantics;
- the body's `μ` and harmonic reference radius, with explicit origin and
  inertial/body-fixed frame identities;
- a body-fixed transform provider plus source records for its frame data and
  conventions; and
- an evaluation epoch, passed through the Cartesian force-model interface.

The coefficient provider must return every coefficient in the selected
triangular region. Return explicit zero coefficients when appropriate;
`None` means that the source does not cover that term and model construction
fails. `C̄00 = 1` and `S̄00 = 0` define the central term. The selected truncation
requires `maximum_order <= maximum_degree`.

For fully normalized geodetic coefficients (`P̄00 = 1`), the model evaluates

```text
V = μ/r Σₙ (R/r)ⁿ Σₘ P̄ₙₘ(sin φ)
      [C̄ₙₘ cos(m λ) + S̄ₙₘ sin(m λ)]
a = ∇V
```

Here `μ` is in m³/s², `R` and `r` are in metres, the coefficients are
dimensionless, and acceleration is returned as a typed SI vector in the input
state frame. The normalized solid-harmonic recurrence is differentiated in
Cartesian coordinates; no longitude is required at the poles. The formula,
associated-Legendre normalization, and tide-system terminology follow IERS
Conventions (2010), Chapter 6, §6.1.
The evaluator assumes the finite expansion is valid at the requested position.
It does not infer a body's mass boundary from the reference radius or enforce
an external-field radius; that validity range depends on the selected field.

## Epoch and frame behavior

`SphericalHarmonicField::time_scale` selects the scale used in each exact
`BodyFixedTransformRequest`. A `TimeInvariant` coefficient set is independent
of epoch. `ValidBetween` sets inclusive endpoints and a time scale; evaluation
outside them returns `CoefficientEpochOutOfRange` without extrapolation.
Coefficient frame identity must equal the configured body-fixed frame, and
the inertial frame, coefficient frame, and gravity origin must be compatible.
At every evaluation the returned transform request is checked against the
requested epoch, time scale, frames, origin, and direction. Acceleration is
computed in body-fixed axes and rotated back to the input inertial axes.

Frame-transform provenance is supplied explicitly alongside the transform
provider because the shared frame contract deliberately does not select EOP
data or conventions. For `frames-eop::Iau2000EraProvider`, callers can copy its
`reference_data()` identities into `SourcedBodyFixedTransformProvider`. That
provider currently implements ERA only and does not include polar motion,
precession-nutation/CIP motion, or terrestrial-realization corrections; its
frame labels do not imply a complete precision transformation.

## Tide and data policy

`TideSystem` records the convention embedded in the chosen gravity
coefficients. This model never converts tide systems and never applies
permanent, solid-Earth, ocean, or pole tides. Select coefficient data and any
separately configured tide model consistently so that a permanent-tide
correction is not applied twice. Time-variable coefficients and tide forces
are outside this implementation slice (#15).

No gravity coefficients or Earth-orientation series are bundled, parsed, or
downloaded. Applications control data loading, checksums, revisions, offline
behavior, and accuracy selection. Missing/unsupported metadata, frames, or
coefficient coverage produce typed errors rather than fallback values.

The validation tests compare the implementation with an independently
evaluated potential and finite-difference gradient over multiple degree/order
truncations, axes, radii, and epochs; the zonal case is also checked against
the closed-form J2 equation. These checks validate the evaluator, not a
particular external gravity product or a complete terrestrial frame.

See [ADR-0047](../../.agent/decisions/0047-low-degree-spherical-harmonics-boundary.md),
[the provenance ledger](../../.agent/PROVENANCE.md), and
[the parity ledger](../../.agent/PARITY.md) for the evidence and declared
limitations.
