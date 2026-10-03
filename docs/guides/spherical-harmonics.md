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
data or conventions. When a provider exposes `reference_data()`, the supplied
records must match them exactly. Evaluation uses
`body_fixed_transform_with_provenance()` and rejects response provenance that
differs from the selected records. Data-free providers may retain caller-declared
records identifying their convention. `frames-eop::Iau2000EraProvider` implements
`ReferenceFrameTransformProvider` for the ERA-only CIRS/TIRS pair, not
`BodyFixedTransformProvider` for GCRF/ITRF. It cannot be passed directly to
`SourcedBodyFixedTransformProvider`. A caller-selected complete body-fixed
provider may incorporate ERA and retain its `reference_data()` identities
alongside sources for the other transformation components. ERA alone does not
include polar motion, precession-nutation/CIP motion, or terrestrial-realization
corrections; do not relabel its CIRS/TIRS rotation as GCRF/ITRF.

## Time-varying coefficient deltas

`SphericalHarmonicGravityModel::with_coefficient_deltas` composes a separate
epoch-dependent provider over the original static coefficient snapshot. The
provider returns dimensionless, fully normalized `ΔC̄ₙₘ` and `ΔS̄ₙₘ` values;
its metadata declares normalization, tide system, coefficient frame,
maximum degree/order, source/product/revision, and an inclusive epoch-coverage
interval with an explicit time scale. Composition rejects convention, frame,
provenance, and truncation mismatches at construction. An evaluation outside
the declared interval, or a missing/non-finite coefficient delta, returns a
typed error rather than extrapolating or substituting zero. `C̄₀₀` and `S̄₀₀`
cannot be changed.

The built-in `LinearSecularRateProvider` evaluates
`ΔC̄ₙₘ = Ċ̄ₙₘ (t - t₀)` and the corresponding sine expression. Rates are
typed SI frequencies (`s⁻¹`), elapsed time is measured from the explicit
Hifitime reference epoch in SI seconds, and that epoch must lie inside the
provider's closed coverage. This is a general first-order secular-rate
mechanism, not a published gravity rate product or a tide model. Applications
must supply source-backed rates and select those consistent with the static
coefficient tide system.

```rust
use std::sync::Arc;

use dynamics_spherical_harmonics::{
    CoefficientDeltaCoverage, CoefficientNormalization, HarmonicCoefficientDeltaMetadata,
    HarmonicCoefficientRate, LinearSecularRateProvider, SphericalHarmonicGravityModel,
    TideSystem,
};
use frames::{ReferenceDataDescriptor, ReferenceFrame};
use hifitime::{Epoch, TimeScale};
use units::uom::si::frequency::hertz;
use units::Frequency;

# fn compose(
#     static_model: SphericalHarmonicGravityModel,
# ) -> Result<(), Box<dyn std::error::Error>> {
let metadata = HarmonicCoefficientDeltaMetadata {
    source: ReferenceDataDescriptor {
        authority: "mission gravity-data provider".into(),
        product: "secular coefficient rates".into(),
        revision: "release-1".into(),
        checksum: None,
    },
    normalization: CoefficientNormalization::FullyNormalized4Pi,
    tide_system: TideSystem::TideFree,
    coefficient_frame: ReferenceFrame::ITRF2020,
    maximum_degree: 2,
    maximum_order: 0,
    coverage: CoefficientDeltaCoverage {
        start: Epoch::from_tai_seconds(0.0),
        end: Epoch::from_tai_seconds(31_557_600.0),
        time_scale: TimeScale::TAI,
    },
};
let zero_rate = HarmonicCoefficientRate {
    cosine: Frequency::new::<hertz>(0.0),
    sine: Frequency::new::<hertz>(0.0),
};
let provider = LinearSecularRateProvider::new(
    metadata,
    Epoch::from_tai_seconds(0.0),
    vec![vec![zero_rate], vec![zero_rate], vec![zero_rate]],
)?;
let time_varying = static_model.with_coefficient_deltas(Arc::new(provider))?;
# let _ = time_varying;
# Ok(())
# }
```

## Tide and data policy

`TideSystem` records the convention embedded in the chosen gravity
coefficients. This model never converts tide systems and never applies
permanent, solid-Earth, ocean, or pole tides. Select coefficient data and any
separately configured tide model consistently so that a permanent-tide
correction is not applied twice. Time-varying normalized coefficient deltas are
available through the explicit provider contract above; solid-Earth, ocean, and
pole-tide formulas, data, and force implementations remain outside this slice
(#15).

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
[ADR-0049](../../.agent/decisions/0049-general-spherical-harmonics-gravity.md),
[the provenance ledger](../../.agent/PROVENANCE.md), and
[the parity ledger](../../.agent/PARITY.md) for the evidence and declared
limitations. See [ADR-0052](../../.agent/decisions/0052-time-varying-harmonic-coefficient-deltas.md)
and [task 0052](../../.agent/tasks/0052-time-varying-harmonic-coefficient-deltas.md)
for the composition decision and validation record.
