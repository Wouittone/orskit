# ADR-0049: re-land general caller-supplied spherical-harmonic gravity

- Status: Accepted
- Date: 2026-10-03
- Owners: propagation/dynamics maintainers

## Scope and dependencies

- Issue: [#11](https://github.com/Wouittone/orskit/issues/11)
- Restore the implementation from commit `06c077f`, reverted by `96350b3`
  before PR #21 merged, onto main after PR #28.
- Depends on composable evaluable force models (#10) and the body-fixed
  transform provider boundary (#17).
- Preserve the ERA-only CIRS/TIRS provider from PR #21 and opt-in
  Gauss-Jackson from PR #28. Exclude #15 tide and relativity work.
- Retain [ADR-0047](0047-low-degree-spherical-harmonics-boundary.md)'s
  evaluator decision. Number this restoration ADR 0049 because
  [ADR-0048](0048-independent-opt-in-gauss-jackson.md) is Gauss-Jackson.

## Scientific/API contract

- Support caller-selected degree/order truncation, including zonal, tesseral,
  and sectorial terms, using fully normalized geodetic 4π coefficients.
- Require source/product/revision provenance, optional checksum, degree/order
  coverage, normalization, tide system, coefficient frame, and explicit
  coefficient epoch semantics.
- Require frame-transform provenance and validate each exact transform
  request/response, including epoch, time scale, frame pair, origin, and
  direction.
- Return framed SI acceleration. Do not bundle/fetch coefficient or EOP data;
  do not parse datasets; do not silently convert or double count tide
  conventions.

## Independent validation

- Compare a separately computed potential and central finite-difference
  gradient for multiple degree/order truncations, radii, axes (including both
  polar axes), and transform epochs.
- Check central and J2-only reductions against point-mass and closed-form
  equations.
- Check typed metadata, normalization, frame, coverage, missing-coefficient,
  epoch-range, and transform-response failures.

## Reconciliation and limitations

`frames-eop::Iau2000EraProvider` implements the provenance-qualified
`ReferenceFrameTransformProvider` for CIRS/TIRS only. It is not a
`BodyFixedTransformProvider` for GCRF/ITRF; applications must supply a complete,
qualified body-fixed transform rather than relabel an ERA-only rotation.

The sourced wrapper checks supplied records against a data-backed provider's
native `reference_data()` and evaluates through
`body_fixed_transform_with_provenance()`, rejecting mismatched response records
with a typed error. Data-free providers retain caller-declared convention
records. This addresses the provenance mismatch deferred by PR #21 review
comment 4173931157 without changing the shared frames contract.

The original independent potential-gradient tests remain unchanged.
Validation uses Rust 1.96.1 and includes an isolated
`orskit --no-default-features --features spherical-harmonics` check.
The full all-target nextest run is not claimed: its existing 100 MiB CCSDS
benchmark can exhaust memory. Exact current commands and results belong in
the restoration PR evidence.

Independent standard high-degree coefficient-set vectors, coefficient-file
ingestion, time-variable coefficients, tides, and full terrestrial-frame
accuracy remain follow-up work for #11; this restores the evaluator slice
without claiming full parity or closing the issue.
