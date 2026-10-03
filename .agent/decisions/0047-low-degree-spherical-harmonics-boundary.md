# ADR-0047: implement caller-supplied general spherical-harmonic gravity

- Status: Accepted
- Date: 2026-10-03
- Owners: propagation/dynamics maintainers
- Affected parity rows: Propagation / gravity fields and solid/ocean tides;
  dynamics and force-model composition

## Context

Issue #11 depends on the composable evaluable-force contract (#10) and the
body-fixed transform boundary (#17). The prior proposed ADR described only
zonal degree 2/3 as a first step; that limitation would not satisfy the issue's
general degree/order workflow. The dependent contracts now provide an
object-safe force evaluation boundary and an epoch/time-scale-qualified
inertial-to-body-fixed rotation. The gravity implementation can therefore
remain independent of both the selected coefficient supplier and frame-data
implementation.

## Decision

1. Keep the implementation in `crates/dynamics/spherical-harmonics`, with an
   opt-in `dynamics/spherical-harmonics` capability and an opt-in
   `orskit/spherical-harmonics` facade feature.
2. Accept a caller-selected immutable coefficient provider. Its metadata
   identifies source/product/revision/checksum, maximum degree/order,
   fully-normalized 4π convention, tide system, coefficient frame, and either
   time-invariant or closed-interval epoch validity. Snapshot every coefficient
   in the selected triangular truncation at model construction; an omitted
   coefficient is a coverage error, not an implicit zero.
3. Evaluate any selected degree/order pair `0 ≤ M ≤ N` for which the provider
   has coverage. Require `C̄00 = 1`, `S̄00 = 0`; support cosine and sine
   coefficients for zonal, tesseral, and sectorial terms.
4. Evaluate the standard geopotential expansion
   `V = μ/r Σₙ (R/r)ⁿ Σₘ P̄ₙₘ(sin φ)(C̄ₙₘ cos(mλ)+S̄ₙₘ sin(mλ))`,
   where `P̄00 = 1` and the associated Legendre functions are fully normalized
   geodetic 4π functions. Acceleration is the Cartesian gradient of `V`.
   Implement the normalized solid-harmonic recurrence with first-order
   Cartesian automatic differentiation so polar axes do not require longitude
   special cases.
5. Require a caller-selected `BodyFixedTransformProvider` plus non-empty
   source/revision records. Each evaluation requests the exact configured
   inertial/body-fixed frame pair, origin, epoch, time scale, and direction;
   reject a provider response that answers any other request. Rotate the
   body-fixed SI acceleration back into the input inertial frame.
6. Treat the coefficient tide system as retained descriptive metadata. This
   model performs no permanent-tide conversion and applies no solid, ocean, or
   pole tide. Consumers must not apply a permanent-tide correction already
   embedded in their selected coefficients. Physical tide models remain
   separate follow-up work (#15).
7. Do not bundle coefficients or EOP data, add parsers/network access, or
   implement time-variable gravity, tide corrections, or a frame realization.
   The selected transform's data and accuracy remain caller responsibilities.

## Alternatives considered

- **Keep only zonal terms:** rejected because it fails the general
  degree/order requirement in #11.
- **Use spherical-coordinate acceleration formulas directly:** rejected for
  the first slice because longitude and its derivatives are singular at the
  poles. The Cartesian solid-harmonic recurrence gives a continuous field and
  acceleration there.
- **Treat absent provider coefficients as zero:** rejected because missing data
  and physically zero terms must not be indistinguishable.
- **Convert tide systems in the force model:** rejected because conversion
  needs external permanent-tide conventions/data and can silently double
  count corrections. The selected system is recorded without adjustment.
- **Add a concrete GCRF/ITRF implementation or EOP dependency:** rejected
  because #17 owns only the shared rotation boundary and its concrete
  implementations/data remain separately selected.

## Consequences

- One model supports degree/order truncation, all coefficient orders,
  synthetic or mission-specific coefficient providers, and explicit field
  metadata without gravity-data packaging.
- Coefficient, frame, transform-response, and epoch-coverage mismatches fail
  with typed errors before an acceleration is accepted.
- The force contributes framed SI acceleration and composes through the
  existing `EvaluableCartesianForceModel` contract.
- The model does not claim validated gravity-data parity or full terrestrial
  frame accuracy; independent coefficient sets and qualified transforms remain
  necessary for higher-fidelity applications.

## Validation

- Compare general synthetic zonal/tesseral/sectorial expansions against a
  separately evaluated scalar potential and central finite-difference
  gradient (100 m, five-point differencing; `2e-9 m/s²` absolute or `2e-9` relative
  tolerance) across `(N,M) = (0,0), (2,0), (3,1), (5,3), (8,8), (20,13)`,
  multiple radii, Cartesian axes including both poles, and three transform
  epochs. The SI absolute tolerance bounds finite-difference cancellation for
  the Earth-scale potential while remaining below `8e-10` relative to the
  point-mass acceleration at the test radii.
- Compare the central and J2-only cases against point-mass and closed-form
  J2 acceleration equations.
- Test typed failures for normalization, source metadata, coefficient-frame
  mismatch, incomplete coverage, missing coefficients, bounded-epoch
  extrapolation, and incorrect transform responses.
- Run Rust 1.96.1 formatting, all-feature check/lint/test/doc checks and the
  explicit `orskit/spherical-harmonics` facade build.

## Provenance

The geopotential expansion and normalization conventions follow IERS
Conventions (2010), Chapter 6, §6.1. Tests independently evaluate the scalar
potential from the published expansion and finite-difference its Cartesian
gradient; no third-party implementation, source code, test vectors, or
coefficient data are copied or bundled.
