# Schwarzschild 1PN gravity correction

Enable the opt-in `relativity` feature on `dynamics` or `orskit`; it is not
enabled by default. The facades expose `dynamics::relativity` and
`orskit::prelude::Schwarzschild1PnCorrection`, respectively.

`dynamics-relativity::Schwarzschild1PnCorrection` evaluates only the
first-post-Newtonian correction for an isolated, spherical, non-rotating
monopole in harmonic coordinates:

```text
a_1PN = μ / (c² r³) [(4 μ/r - v²) r + 4 (r·v) v]
```

This is the test-particle equation in IERS Conventions (2010), Chapter 10,
§10.2, Eq. 10.8. Position and velocity are Cartesian vectors in metres and
metres per second, `μ` is in m³/s², `c` is the exact speed of light, and the
result is a typed SI acceleration in the state frame.

Construction explicitly selects the monopole origin, a matching inertial
frame, the central gravitational parameter, and a maximum expansion parameter
in `(0, 1)`. At evaluation the model checks
`max(v²/c², μ/(r c²))` against that configured bound. The bound is a
weak-field/slow-motion regime gate, not a truncation-error estimate; choose a
value much smaller than one appropriate to the application.

The model never includes Newtonian gravity. Add it explicitly alongside the
matching point-mass or spherical-harmonic model; both must use the same `μ`,
origin, and harmonic-coordinate frame. The formula omits body spin
(Lense–Thirring), external-body relativistic terms, and relativistic
couplings to nonspherical gravity. It is not a complete relativistic
many-body or Earth-orientation solution. The correction is velocity-dependent
and does not currently provide variational acceleration partials.

No relativistic model data is bundled or fetched. An independent vector test
uses the IERS equation with explicitly declared SI constants. See
[ADR-0050](../../.agent/decisions/0050-schwarzschild-1pn-and-tide-blockers.md)
and [the provenance ledger](../../.agent/PROVENANCE.md).
