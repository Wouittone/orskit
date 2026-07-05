# Two-body speed and process-memory comparison

This harness compares one narrow end-user workload across orskit, Orekit
13.1.6, Lox 0.1.0-alpha.39, and Nyx 2.3.1. It is evidence for this workload,
not a general ranking of the libraries.

Each executable receives the same Earth-centered inertial Cartesian state and
performs independent point-mass endpoint queries at deterministic signed time
offsets spanning plus or minus one day. Setup and 10,000 warm-up queries are
excluded from each implementation's internal timer. Every result contributes
to a checksum so the compiler cannot discard the work.

The comparison includes the public Cartesian-call cost:

- orskit `EllipticTwoBodyPropagator` converts Cartesian to elliptic elements,
  advances mean anomaly, and converts the result back for every query;
- Orekit `KeplerianPropagator` is constructed from a `CartesianOrbit` and
  returns Cartesian position/velocity from each propagated state;
- Lox `Vallado` accepts and returns Cartesian orbits using its documented
  universal-variable formulation;
- Nyx `Orbit::at_epoch` applies its documented two-body mean-longitude shift.

Orskit, Orekit, and Nyx use `mu = 398600441800000 m^3/s^2`. The pinned Lox `Earth`
type supplies `mu = 398600435507022.7 m^3/s^2`; the differing constant is
retained because Lox's typed `Vallado` public constructor obtains point-mass
properties from its origin. Correctness is evaluated by the separate fixture
for each explicit constant.

The PowerShell runner builds release binaries, interleaves repeated samples,
and polls each direct process's working set. `peak_working_set_bytes` is the
largest whole-process resident working set observed at the configured polling
interval. It is not an allocation count. In particular, Orekit's number
includes the JVM and Lox/orskit/Nyx include their native runtime and loaded code.

Run from the repository root:

```powershell
pwsh .agent/references/two-body/benchmark/run.ps1 | Set-Content benchmark.csv
```

Useful controls are `-Iterations`, `-Samples`, `-PollMilliseconds`, and
`-Implementations`. Build time is never included. The first run needs access
to Maven Central and crates.io for the pinned reference dependencies; later
runs can reuse their normal Gradle and Cargo caches.

For a publishable comparison, run on an otherwise idle machine, record the
CPU, OS, Rust, Java, and Gradle versions, retain every raw sample, and report
the median throughput plus the maximum observed peak working set. Re-run the
existing endpoint comparisons whenever the algorithms or constants change;
speed without the accuracy evidence is not accepted here.

## Nyx isolation and accuracy

Nyx is AGPL-3.0-or-later. Its harness is therefore a separately licensed Cargo
workspace outside the orskit workspace and distribution. Default features are
disabled so the premium feature is not enabled. No orskit crate depends on
Nyx, and no Nyx source, tests, examples, or internal design are consulted.

Nyx 2.3.1's public `Orbit::at_epoch` result does not pass the shared
3,600-second endpoint comparison: it differs from the matching Orekit/orskit
state by about 4,377 km and 4.424 km/s. Its timing remains useful as a measured
public-API behavior, but it is explicitly disqualified as correctness evidence
for this scenario. An alternative independently supplied executable may still
be selected with `-NyxExecutable`.
