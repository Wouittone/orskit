# Deliberate Cargo feature matrix

orskit does not attempt every mathematical combination of its additive Cargo
features. The maintained matrix checks the dependency boundaries that are most
likely to regress:

| Boundary | Representative facade features |
| --- | --- |
| Implementation-neutral minimum | no default features |
| Earth orientation | `earth-orientation` |
| State and I/O | `cartesian`, `ccsds`, `tle`, `sgp4`, `ephemeris` |
| Dynamics | `two-bodies` |
| Numerical propagation | `numerical` |
| Observation | `measurements`, `measurement-range`, `measurement-geometric-range` |
| Estimation | `orbit-determination` |
| Export | `serialization`, `serialization,cartesian`, `serialization,two-bodies`, `serialization-json` |
| Feature unification | all features |

The export crate is also tested without features and with `orbits`,
`orbits,json`, and `two-bodies,json`. This proves that selecting serialization
alone does not silently select a physical state or propagator implementation.

Run the matrix with:

```powershell
pwsh -NoProfile -File scripts/check_feature_matrix.ps1
```

The normal all-feature workspace tests remain the maximal integration check.
When a feature adds a new dependency cluster or changes an implication, update
this matrix and its CI invocation. Individual measurement leaf features remain
covered by the maximal build unless they establish a distinct dependency
boundary; enumerating their power set would add cost without meaningful
coverage.
