# ADR-0036: typed estimates over dedicated propagation contracts for Kalman OD

- Status: Accepted
- Date: 2026-07-18
- Affected parity rows: sequential orbit determination; third-body dynamics

## Context

Measurement families retain their own units, signal paths, frames, and timing.
At the same time, a public `usize`/matrix state contract would make the
numerical representation, rather than an implementation of `SpacecraftState`,
the lasting OD boundary. OD must also not duplicate force models or numerical
integration: orskit's `dynamics::Propagator` already separates a physical
problem in a concrete propagator. Validation needs innovations and residuals,
but normal runs must not unconditionally retain a per-step trace.

## Decision

1. `OrbitDetermination<Observation>` is the open sequential contract. A filter
   owns a selected `dynamics::Propagator`, whose concrete value owns precisely
   one physical problem. `estimate_all`
   processes ordered series without dimension parameters or an untyped
   observation enum.
2. Every filter prior, process model, observation, and secondary ephemeris
   declares one `ReferenceFrame`; the supplied models require affirmative
   inertial axes and reject mismatches.
3. `StateEstimate<S, C>` is parameterised by a `SpacecraftState` implementation
   and a domain covariance. The Cartesian implementation keeps fixed-size
   vectors and matrices private, exposing only explicit SI interoperability
   constructors on typed covariance objects.
4. `KalmanFilter` is an OD extension trait with two implementations:
   `ExtendedKalmanFilter` derives a scaled central-Jacobian transition through
   the external `finitediff` crate and uses Joseph correction;
   `UnscentedKalmanFilter`
   propagates scaled sigma points through that same contract. Neither filter
   implements an ODE solver or force model.
5. Diagnostics are observer callbacks supplied for an individual estimation
   call. The default estimation path neither allocates nor retains diagnostic
   records; observers receive prediction statistics, innovation, post-fit
   residual, and innovation covariance.
6. A restricted-three-body model accepts an application-owned
   `SecondaryBodyEphemeris`.  It neither downloads nor selects ephemeris data.

## Consequences

- Range, Doppler, angular, and catalog adapters can implement typed observation
  contracts without exposing algorithm matrices in the common estimate boundary.
- This initial slice is Cartesian-only and does not yet estimate measurement,
  force-model, or maneuver parameters; batch least squares and physical data
  providers remain separate work.
- The filter keeps all numerical matrices private. Solver accuracy, force
  composition, ephemerides, and propagated-state provenance remain the
  responsibility of the caller-selected propagator.

## Provenance

This is original orskit architecture.  NASA propagation references supply only
the public acceleration conventions.  Orekit 13.1.7 is exercised only as an
unmodified black-box endpoint reference; no source, tests, or implementation
structure informed this decision.
