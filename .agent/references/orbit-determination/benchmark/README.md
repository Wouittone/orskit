# Cartesian position EKF benchmark

This benchmark compares one deterministic sequential-OD correction using only
public APIs: an epoch-zero Cartesian prior, a three-axis position observation,
and point-mass two-body propagation. Each query constructs a fresh filter,
assimilates the same observation, and contributes the posterior position to a
checksum. It measures the complete one-step solve, including estimator/filter
construction; it does not claim model, measurement-family, or accuracy parity.

Both implementations use `mu = 3.986004415e14 m^3/s^2`, a diagonal prior
covariance of `1e6` in Cartesian SI coordinates, diagonal process noise of
`1e-8`, and 5 m one-sigma Cartesian position noise. Orskit uses its current
finite-difference EKF transition; Orekit uses `KalmanEstimator` with
`KeplerianPropagatorBuilder`, `ConstantProcessNoise`, and `Position`.

Run from the repository root:

```powershell
pwsh .agent/references/orbit-determination/benchmark/run.ps1
```

The Orekit harness is an Apache-2.0 public-API black-box comparison only. No
Orekit implementation, test, or example source is reused by the Rust code.
