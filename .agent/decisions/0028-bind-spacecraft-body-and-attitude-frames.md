# ADR-0028: bind spacecraft body and attitude frames

- Status: Accepted
- Date: 2026-07-13
- Affected parity rows: spacecraft state; attitude

## Context

Angular velocity named only an expression frame. `Spacecraft` did not own a
body frame, and `SpacecraftView` did not check attitude reference against orbit.

## Decision

1. `Spacecraft` owns an affirmatively non-inertial body frame.
2. `BodyAngularVelocity` means body relative to reference, expressed in body
   axes, and names both frames.
3. `QuaternionAttitude` requires orientation endpoints to match both rate
   endpoints.
4. `SpacecraftView` requires spacecraft body, attitude moving, and inertia
   frames to match, and attitude reference to equal the orbit coordinate frame.
5. No transform or convention is inferred.

## Validation

Tests cover invalid body motion, rate endpoint mismatches, body/inertia
mismatch, and attitude/orbit reference mismatch.

## Provenance

This is original orskit attitude-boundary design. No external implementation
or tests were consulted.
