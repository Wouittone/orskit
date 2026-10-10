# Provenance and clean-room policy

This policy protects the independent character of orskit and its intended
MIT/Apache-2.0 licensing. It is an engineering policy, not legal advice.

## Default rule

All project-owned implementation, tests, examples, and documentation must be
original work contributed under MIT or Apache-2.0. A permissive upstream
license does not automatically authorize importing code into the dual-licensed
project: reuse must be deliberate, attributed, and approved first. Without
that approval, implement from public scientific descriptions and observed
behavior.

## Reference classes

| Reference | Allowed use | Prohibited use by default |
| --- | --- | --- |
| Standards, textbooks, and papers | Equations, conventions, test values, and algorithms with citation | Copying protected prose, figures, or code |
| Orekit documentation and public API behavior | Capability inventory, terminology research, black-box comparison, and independent test expectations | Translating or structurally porting source code, tests, or internal design |
| Lox documentation and public API behavior | Rust API and capability research, ergonomic inspiration, and external dependency evaluation | Translating or structurally porting source code, tests, or internal design |
| Nyx astrodynamics project material | Public API documentation and unmodified black-box execution for validation/benchmarks only | Copying or adapting source, tests, examples, internal architecture, identifiers, or distinctive expression; linking Nyx into any orskit crate |
| Separately published permissive/MPL crates | Unmodified dependency use after license, API, and maintenance review | Copying dependency source into project-owned MIT/Apache-2.0 files |
| Other open-source libraries | Public behavior research and separately licensed dependencies after audit | Source reuse without explicit compatibility review and attribution |
| Public datasets | Validation when redistribution and use terms are recorded | Vendoring or redistributing data without confirmed permission |

Treat the Nyx astrodynamics implementation as out of bounds even if a mirror,
fork, or future release presents different licensing. Maintainer-approved
validation may use Nyx's public API documentation and execute an unmodified
release in a separately licensed external harness. Such a harness must remain
outside the workspace and distribution, and may not inform orskit design or
implementation. This does not prohibit using a separately packaged crate such
as Hifitime, unmodified and under its own compatible license, after an explicit
dependency decision.

## Required research record

For every new scientific model, file format, or differential test, record:

- the exact title, authoring organization, version/date, and stable locator;
- whether it is a standard, paper, documentation, dataset, behavior sample, or
  dependency;
- its license or access terms when applicable;
- what facts were learned from it;
- what was intentionally not copied; and
- the tests or code that use those facts.

Put concise citations next to equations and test vectors. Add a row to the
ledger below when the reference affects more than one module or establishes a
parity claim.

## Project reference ledger

| Area | Reference and version | Class/terms | Permitted use | Code or evidence |
| --- | --- | --- | --- | --- |
| Project scope | [Orekit 13.1.7 release/download/Javadoc pages](https://www.orekit.org/download.html) | Apache-2.0 project; website pages CC BY 3.0 unless otherwise noted | Versioned capability inventory only; no source, tests, examples, internal structure, or distinctive prose | `.agent/baselines/orekit-13.1.7.md`; `.agent/PARITY.md` |
| Rust design reference | [Lox](https://github.com/lox-space/lox) | MPL-2.0 project documentation | High-level API and architecture research only | `.agent/ARCHITECTURE.md` |
| Nyx validation boundary | [Nyx 2.3.1 public API](https://docs.rs/nyx-space/2.3.1/nyx_space/) and [license statement](https://docs.rs/crate/nyx-space/2.3.1) | AGPL-3.0-or-later; validation-only use approved by project owner | Public API/black-box validation and benchmarks only; no implementation or design use | `.agent/references/two-body/benchmark/nyx`; task 0010 |
| Time | [Hifitime 4.3](https://docs.rs/hifitime/4.3.0/hifitime/) | MPL-2.0 dependency | Direct, unmodified epoch/time API | `crates/core`, `crates/measurements` |
| Ground-observation capability inventory | [Orekit 13.1 measurements package](https://www.orekit.org/static/apidocs/org/orekit/estimation/measurements/package-summary.html), [FDOA API](https://www.orekit.org/static/apidocs/org/orekit/estimation/measurements/FDOA.html), and [TDOA API](https://www.orekit.org/static/apidocs/org/orekit/estimation/measurements/TDOA.html) | Apache-2.0 project public API documentation | Measurement-family names, participant roles, arrival-difference sign conventions, and public unit labels only; no source, tests, examples, or prediction implementation material | `crates/measurements/src/ground.rs`; `.agent/PARITY.md` |
| Instantaneous radiometric geometry | [JPL DESCANSO *Radiometric Tracking Techniques for Deep-Space Navigation*, Chapter 3](https://descanso.jpl.nasa.gov/monograph/series1/Descanso1_C03.pdf) | Public technical monograph | Range as line-of-sight distance and Doppler/range-rate as line-of-sight relative-velocity concepts only; no source, tests, examples, or detailed signal-processing/model implementation material | `crates/measurements/src/estimation.rs`; `.agent/ARCHITECTURE.md`; `.agent/PARITY.md` |
| Vacuum light-time and station frame context | [JPL DESCANSO *Radiometric Tracking Techniques for Deep-Space Navigation*, Chapter 3](https://descanso.jpl.nasa.gov/monograph/series1/Descanso1_C03.pdf) and [BIPM definition of the metre](https://www.bipm.org/en/si-base-units/metre) | Public technical monograph and SI definition | Ordered signal events, range as propagated signal distance, station-coordinate frame context, and the exact vacuum light-speed constant only; no source, tests, examples, detailed signal-processing, Earth-orientation, or media-model implementation material | `crates/frames/src/lib.rs`; `crates/measurements/src/estimation.rs`; task 0027 |
| Units | [`uom` 0.38](https://docs.rs/uom/0.38.0/uom/) | MIT OR Apache-2.0 dependency | Direct dimensional quantities | `crates/units` |
| Frame survey | [`lox-frames` 0.1.0-alpha.11](https://docs.rs/lox-frames/0.1.0-alpha.11/lox_frames/) and [ANISE](https://docs.rs/anise/latest/anise/) | MPL-2.0 dependencies; not adopted in this slice | API/capability evaluation only | `.agent/decisions/0001-foundational-types.md` |
| Body and barycenter identities | [NAIF SPICE *NAIF Integer ID codes*, revision 2021-12-10](https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/FORTRAN/req/naif_ids.html), [NASA Science *About the Planets*](https://science.nasa.gov/solar-system/planets/), and [IAU Resolution B5 (2006)](https://www.iau.org/static/resolutions/Resolution_GA26-5-6.pdf) | US Government API/science documentation and public scientific resolution | Body-versus-barycenter semantics, common Solar System identity names, eight-planet and Pluto classification only | `crates/bodies`, `crates/frames`; `.agent/decisions/0005-body-owned-frame-origins.md` |
| CCSDS OEM | [CCSDS 502.0-B-3, *Orbit Data Messages*, Issue 3, May 2023](https://ccsds.org/Pubs/502x0b3e1.pdf) and [SANA navigation registries](https://sanaregistry.org/r/orbit_centers/) | Public standard and normative registries | OEM KVN syntax, units, section semantics, and registered identifiers only | `crates/ccsds`; `.agent/tasks/0003-ccsds-oem-ingestion.md` |
| OEM covariance interoperability fixture | [Orekit `OEM-Issue839.txt`](https://github.com/CS-SI/Orekit/blob/develop/src/test/resources/ccsds/odm/oem/OEM-Issue839.txt) | Apache-2.0 source test resource; retained with attribution | The original covariance rows and `RTN`/`EME2000` cases in a minimal, self-contained OEM test resource; no Orekit parser or implementation code | `crates/ccsds/testdata/orekit_oem_issue839_covariance.oem`; `crates/ccsds/testdata/README.md`; `crates/ccsds/src/oem.rs` |
| CCSDS Rust dependency survey | [`ccsds-ndm` 0.0.9](https://crates.io/crates/ccsds-ndm/0.0.9) and [`lox-odm` 0.1.0-alpha.3](https://crates.io/crates/lox-odm/0.1.0-alpha.3) | MPL-2.0 packages; not adopted | Public API, package metadata, feature and domain-boundary evaluation only | `.agent/decisions/0003-own-streaming-ccsds-ingestion.md` |
| Orbit state representations | [NASA GMAT Mathematical Specifications (2007)](https://ntrs.nasa.gov/citations/20080031744), [NAIF CSPICE `CONICS`](https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/C/cspice/conics_c.html), [NAIF CSPICE `OSCLTX`](https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/C/cspice/oscltx_c.html), [Orekit 13.1.7 orbit package](https://orekit.org/static/apidocs/org/orekit/orbits/package-summary.html), and [Orekit 13.1.7 `CircularOrbit` API](https://orekit.org/static/apidocs/org/orekit/orbits/CircularOrbit.html) | US Government technical/API documentation; Orekit public behavior documentation | State/element conventions, valid regimes, circular-element terminology, inverse sanity-check policy, and independent analytic behavior only | `crates/orbits/src/state.rs`; ADR-0032; task 0026 |
| Elliptic two-body propagation | [NASA GMAT Mathematical Specifications (2007)](https://ntrs.nasa.gov/citations/20080031744), [NASA/TM-2004-213230 *Orbit Propagation*](https://ntrs.nasa.gov/citations/20040084254), [Orekit 13.1.6 `KeplerianPropagator`](https://www.orekit.org/static/apidocs/org/orekit/propagation/analytical/KeplerianPropagator.html), [Orekit 13.1.6 `CartesianOrbit`](https://www.orekit.org/static/apidocs/org/orekit/orbits/CartesianOrbit.html), [Lox 0.1.0-alpha.39 `Vallado`](https://docs.rs/lox-space/0.1.0-alpha.39/lox_space/prelude/struct.Vallado.html), and [Nyx 2.3.1 `Orbit::at_epoch`](https://docs.rs/nyx-space/2.3.1/nyx_space/cosmic/type.Orbit.html) | US Government equations; Apache-2.0, MPL-2.0, and isolated AGPL-3.0-or-later public API/black-box behavior | Universal-variable/Stumpff and Lagrange `f`/`g` equations; independent Cartesian output; public Cartesian endpoint-query performance workload only. Performance harnesses use pinned dependencies unmodified; no reference source, tests, examples, or internal design were consulted. Nyx default/premium features are disabled and no orskit crate depends on it | `crates/dynamics/core/src/propagator.rs`; `crates/dynamics/two-bodies`; `.agent/references/two-body`; ADR-0033; task 0026 |
| Adaptive Cartesian numerical propagation | [P. Bogacki and L. F. Shampine, *A 3(2) pair of Runge--Kutta formulas*, Applied Mathematics Letters 2(4), 1989](https://doi.org/10.1016/0893-9659(89)90079-7) and [L. F. Shampine, I. Gladwell, and S. Thompson, *Solving ODEs with MATLAB*, Cambridge University Press, 2003, section 1.2](https://doi.org/10.1017/CBO9780511615542) | Copyrighted primary numerical paper and textbook; equations, coefficients, error-scaling facts, and interpolation description only | Four-stage embedded 3(2) pair advanced with the third-order solution; component-scaled local-error control; cubic Hermite continuous extension using accepted endpoint values and slopes. No source code, test implementation, or distinctive prose was copied | `crates/dynamics/core`; `crates/dynamics/numerical`; `crates/dynamics/two-bodies`; ADR-0037; task 0036 |
| Spacecraft thrust and mass evolution | Dan M. Goebel and Ira Katz, [*Fundamentals of Electric Propulsion: Ion and Hall Thrusters*](https://descanso.jpl.nasa.gov/SciTechBook/series1/Goebel_02_Chap2_thruster.pdf), JPL Space Science and Technology Series, 2008, chapter 2 | Public US Government/JPL technical book; equations and physical definitions only | Thrust as force on instantaneous spacecraft mass, propellant mass-rate evolution, and the variable-mass relation used for independent analytic validation. No source code, tests, examples, or distinctive prose was copied | `crates/dynamics/numerical/src/maneuver.rs`; ADR-0038; task 0038 |
| Prescribed attitude interpolation and body-fixed thrust | Ken Shoemake, [*Animating Rotation with Quaternion Curves*](https://doi.org/10.1145/325334.325242), *Computer Graphics* 19(3), 1985 | Copyrighted primary paper; interpolation concept and equations only | Unit quaternions represent rotations and spherical interpolation follows the unit-sphere arc. The implementation uses the unmodified `nalgebra` kernel behind framed domain operations; no source code, tests, examples, figures, or distinctive prose was copied | `crates/core/src/spacecraft.rs`; `crates/attitude`; `crates/dynamics/numerical/src/maneuver.rs`; ADR-0040; task 0040 |
| Cartesian variational and covariance propagation | Paul J. Huxel and Robert H. Bishop, [*Navigation Algorithms for Formation Flying Missions*](https://ntrs.nasa.gov/citations/20060048534), Proceedings of the 2nd International Symposium on Formation Flying Missions and Technologies, 2004, NASA NTRS 20060048534 | Public US Government-sponsored conference paper; Public Use Permitted; equations only | State transition as `d Phi/dt = A Phi`, identity initial condition, and covariance map `Phi P Phi^T + Q`; the implemented first slice selects `Q = 0`. No source code, tests, implementation structure, or distinctive prose was copied | `crates/orbits/src/covariance.rs`; `crates/dynamics/core`; `crates/dynamics/numerical/src/variational.rs`; ADR-0039; task 0039 |
| Sequential Cartesian orbit determination | [orskit propagation contracts](../crates/dynamics/core/src/propagator.rs), [finitediff 0.2.0](https://crates.io/crates/finitediff/0.2.0), and [Orekit 13.1.6 public API](https://www.orekit.org/static/apidocs/) | Original project contract; MIT OR Apache-2.0 dependency; Apache-2.0 public API/black-box behavior | The OD boundary delegates physical propagation to one caller-selected propagator that owns its physical problem. `finitediff` supplies the unmodified central-Jacobian algorithm. The isolated Orekit public-API benchmark compares one Cartesian position EKF correction; no Orekit source, tests, examples, data, or implementation structure was used by OD | `crates/orbit-determination`; `.agent/references/orbit-determination`; ADR-0036; tasks 0030 and 0031 |
| Reference-data-backed frame transforms | [JPL DE440/DE441 description](https://ssd.jpl.nasa.gov/doc/de440_de441.html), [NAIF SPK required reading](https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/C/req/spk.html), and [IERS data products](https://www.iers.org/IERS/EN/DataProducts/data.html) | JPL DE440/DE441 are planetary and lunar ephemerides; NAIF SPK is an ephemeris-kernel format containing position/velocity segments; IERS publishes Earth-orientation and ICRF/ITRF data products. These facts establish that a production terrestrial/celestial supplier must identify separately selected ephemeris and Earth-orientation inputs. No transform equations, source code, tests, or data files were copied | `crates/frames/src/lib.rs`; ADR-0035; task 0029 |
| Earth rotation angle | [IERS Conventions (2010), Chapter 5, §5.5.3, Eq. 5.15](https://iers-conventions.obspm.fr/content/chapter5/icc5.pdf) and [Hifitime 4.3 `Epoch`](https://docs.rs/hifitime/4.3.0/hifitime/struct.Epoch.html) | Public IERS scientific standard and the already-approved MPL-2.0 time library | IAU 2000 ERA equation and reference constants; Hifitime UTC/TAI conversions and leap-second table only. Implementation is independently written. The J2000 value is evaluated independently from the published equation; no EOP data, implementation source, or tests were copied | `crates/frames-eop/src/lib.rs`; ADR-0046; task 0046; `docs/guides/earth-orientation.md` |
| Force and torque capability inventory | [Orekit 13.1.7 force packages](https://www.orekit.org/site-orekit-13.1.7/apidocs/org/orekit/forces/package-summary.html), [IERS Conventions 2010 Chapter 6](https://iers-conventions.obspm.fr/content/chapter6/icc6.pdf), [NASA GMAT force-model documentation](https://documentation.help/GMAT/Propagator.html), and [NASA attitude-control survey](https://s3vi.ndc.nasa.gov/ssri-kb/static/resources/A%20Brief%20Survey%20of%20Attitude%20Control%20Systems%20for%20Small%20Satellites%20u.pdf) | Public behavior/capability documentation, public standards, and US Government technical documentation | Capability names, model-family boundaries, tide-convention warning, and disturbance-torque categories only; no implementation material | `.agent/FORCE_MODELS.md` |

| External ODE solver performance evaluation | [`Wouittone/differential-equations-rs` issue #40](https://github.com/Wouittone/differential-equations-rs/issues/40) and [PR #50](https://github.com/Wouittone/differential-equations-rs/pull/50) | Same-author, MIT OR Apache-2.0 sibling project; public issue/PR findings | Representative six-hour LEO RKV98 wall-clock/accuracy comparison and controller-arithmetic cost share, used only as performance facts to justify not adopting the crate as the default numerical backend | `.agent/decisions/0041-defer-external-ode-solver-adoption.md`; GitHub Issue #8 |
| External reusable Vern9 allocation/performance gate | [`differential-equations-rs` 1.4.1](https://crates.io/crates/differential-equations-rs/1.4.1), [`numeris` 0.6.0](https://crates.io/crates/numeris/0.6.0), and [`stats_alloc` 0.1.10](https://crates.io/crates/stats_alloc/0.1.10) | Isolated MIT OR Apache-2.0 and MIT dependencies; public APIs and versioned package contracts | Reusable workspace allocation guarantees and unmodified native RKV98 comparison behavior; no dependency source was copied. Fixed LEO, synthetic velocity-dependent perturbation, and equal 30-second dense-query samples are benchmark inputs, not claimed reference trajectories beyond the stated analytic/convergence checks | `.agent/references/rkv98-adapter-gate`; ADR-0041; GitHub Issue #19 |
| Earth-oblateness J2 gravity correction | [NASA GMAT *Mathematical Specifications* (2007)](https://ntrs.nasa.gov/citations/20080031744), already an accepted reference in this ledger, and [IERS Conventions 2010, Chapter 6](https://iers-conventions.obspm.fr/content/chapter6/icc6.pdf) | US Government technical documentation; public standard | Closed-form zonal J2 perturbing-acceleration equation and geopotential tide-system convention naming only; no source code, tests, or distinctive prose was copied | `crates/gravity/src/lib.rs`; `crates/dynamics/harmonics`; ADR-0042; GitHub Issue #9 |
| General spherical-harmonic geopotential | [IERS Conventions (2010), Chapter 6, §6.1, *Geopotential*](https://www.iers.org/SharedDocs/Publikationen/EN/IERS/Publications/tn/TechnNote36/tn36_6.pdf?__blob=publicationFile&v=2) | Public scientific standard | Geopotential expansion, fully normalized geodetic 4π associated-Legendre convention (`P̄00 = 1`), coefficient/tide-system terminology, and acceleration as the Cartesian gradient of positive geopotential. The recurrence and Cartesian differentiation are independently derived; no code, test vectors, or coefficient data were copied | `crates/dynamics/spherical-harmonics/src/lib.rs`; `docs/guides/spherical-harmonics.md`; ADR-0047; ADR-0049; GitHub Issue #11 |

## Gauss-Jackson research record

Gauss-Jackson (#16): Matthew M. Berry and Liam M. Healy, *Implementation of
Gauss-Jackson Integration for Orbit Propagation*, Journal of the Astronautical
Sciences 52(3), July-September 2004, pp. 331-357,
<https://hdl.handle.net/1903/2202>. Publicly accessible copyrighted paper:
equations 38/52, backward-difference concepts, stability restrictions and
published period/eccentricity facts only. Neither the supplemental Lisp source
nor any astrodynamics integrator implementation was consulted or copied.
Original generating-function coefficient derivation and tests live in
`crates/dynamics/numerical`; evidence is in `references/gauss-jackson`.
Orekit 13.1.6 (Apache-2.0) was executed unmodified through public APIs in the
isolated Java harness for independent numeric endpoints only. Its transitive
Hipparchus 4.0.3 (Apache-2.0) DOP853 public API independently integrates an
original point-mass plus linear-drag scenario; tenfold tighter local tolerances
check the reference floor. Neither is linked into orskit. No external
library source, tests, examples, internal structure or distinctive prose was
used. No dataset or paper text is redistributed.

| Schwarzschild first post-Newtonian acceleration | [IERS Conventions (2010), Chapter 10, §10.2, Eq. 10.8](https://apps.dtic.mil/sti/html/tr/ADA535671/index.html) and [BIPM definition of the metre](https://www.bipm.org/en/si-base-units/metre) | Public scientific standard and SI definition | The test-particle Schwarzschild correction in harmonic coordinates, isolated spherical monopole assumptions, and exact speed of light. The correction is independently implemented and checked against a separately evaluated SI vector; no source code or test material is copied | `crates/dynamics/relativity/src/lib.rs`; `docs/guides/relativity.md`; ADR-0050; task 0050 |
| CIO-based GCRF-to-ITRF transform | [IERS Conventions (2010), Chapter 5, §5.4-5.5](https://iers-conventions.obspm.fr/content/chapter5/icc5.pdf); [`erfa` 0.2.1](https://docs.rs/erfa/0.2.1/erfa/) | Scientific equations/conventions only; unmodified MPL-2.0 dependency with packaged ERFA BSD-style notice. The owner approved dependency replacement after verification; no IERS table redistribution permission is inferred. | Original rotation chain and derivative stencil; dependency public API supplies full IAU 2006/2000A `X/Y/s`. The three unverified bundled tables and their evaluator have been deliberately replaced. No dependency implementation, coefficients, test code or distinctive prose was copied into project-owned implementation. | `crates/frames-eop/src/cio.rs`; `crates/frames-eop/NOTICE`; `docs/guides/earth-orientation.md`; ADR-0051; task 0051 |
| CIO independent reference vectors | [ERFA v2.0.1 test suite, pinned commit `9915ba38c9365f8b0738269b8c2ac1fdd5f8dee3`, `src/t_erfa_c.c`](https://github.com/liberfa/erfa/blob/9915ba38c9365f8b0738269b8c2ac1fdd5f8dee3/src/t_erfa_c.c) and [IERS Conventions (2010), Chapter 5](https://iers-conventions.obspm.fr/content/chapter5/icc5.pdf) | Public validation outputs and scientific standard; numerical values only | Direct `X`, `Y`, `s` values from the ERFA `eraXys06a` case and the matrix from its `eraC2t06a` case at TT/UT1 MJD 53736.0. Expected values are recorded with their routine, input epoch, and upstream revision; no ERFA implementation or test code is copied. The redundant unknown-version 2025 PyERFA matrix fixture was removed; finite-difference and round-trip tests at 2025 epochs remain. | `crates/frames-eop/src/cio.rs`; `docs/guides/earth-orientation.md`; ADR-0051; task 0051 |

## CIO reference output record

The reference producer is the unmodified ERFA v2.0.1 public validation suite,
`src/t_erfa_c.c`, pinned above (both cases dated 2013-08-07). No locally run
generator is claimed: these are published expected numerical outputs, verified
against that exact upstream commit, rather than outputs inferred from orskit.
`eraXys06a` takes TT JD `(2400000.5, 53736.0)` and returns `X`, `Y`, `s`
in radians. `eraC2t06a` takes the same split JD for both TT and UT1,
`xp = 2.55060238e-7 rad`, `yp = 1.860359247e-6 rad`, and returns the
GCRS-to-ITRS column-vector matrix. The Rust tests contain rounded-to-f64
expected outputs and independent request construction (TT-TAI = 32.184 s
and leap-aware UT1-UTC samples). These original tests use public numerical
values only, not upstream test logic. The implementation now links the separate
unmodified `erfa` Rust dependency; no ERFA implementation/test code is copied
into project-owned files. The former 2025 fixture lacked a recoverable producer version and
was removed rather than relabeled as a v2.0.1 result.

## IERS table distribution review (2026-10-10)

Exact primary sources examined:

- [TN 36 PDF](https://iers-conventions.obspm.fr/content/tn36.pdf), front matter,
  physical PDF page 2: copyright identifies Verlag des Bundesamts für
  Kartographie und Geodäsie, Frankfurt am Main 2010; provides IERS Central
  Bureau contact `central_bureau@iers.org`. No affirmative table redistribution
  grant was located. Download SHA-256:
  `fdb5b74c5a6135c9d20fd99191741e8b62ac8d712eb96f046e7eeb8fb4236f51`.
- [Official packaged v1.0.0](https://iers-conventions.obspm.fr/packaged_versions/iersconventions_v1_0_0.tar.gz),
  SHA-256 `99a67ce5b1432362ea175142c7054aee66092bdc3655e40641a618eec567498d`:
  examined its member list and the three `2010_official/chapter5/additional_info/`
  table files. No package-level `LICENSE`/`COPYING` or general `README` was
  listed (an unrelated software-specific README exists); no license,
  copyright or redistribution notice appeared in those three table texts.
  This is evidence of no located grant, not a conclusion about all package
  members or the legal status of numbers.
- [Centre home page](https://iers-conventions.obspm.fr/) and
  [Chapter 5 page](https://iers-conventions.obspm.fr/chapter5.php): identify the
  official packaged edition, distinguish the updated pages as under
  development, and attribute Tables 5.2a/5.2b/5.2d to N. Capitaine (footnote
  4). These availability/attribution statements are not redistribution terms.
- [Table 5.2a](https://iers-conventions.obspm.fr/content/chapter5/additional_info/tab5.2a.txt),
  [Table 5.2b](https://iers-conventions.obspm.fr/content/chapter5/additional_info/tab5.2b.txt),
  [Table 5.2d](https://iers-conventions.obspm.fr/content/chapter5/additional_info/tab5.2d.txt):
  scientific descriptions, numerical coefficients, argument multipliers,
  units and block counts; no explicit redistribution license located.

**Material distinction:** individual equations, coefficients and physical facts
are not the same thing as copyrighted prose, code or a potentially protected
selection/arrangement/database. Absence of an open license does not establish
that numerical facts require one. The actual bundled `.dat` files, however,
retain source-derived table-title prose, column headers, coefficient ordering
and numerical rows. Most explanatory prose and block headings were omitted;
no IERS program, PDF, figures or archive packaging is shipped, and the Rust
implementation is original. The legal review must address the actual copied
collection and headers, not conflate them with implementation code or declare
all numbers copyrighted.

**Historical owner decision requested before publishing PR #38:** approve and record a
specific basis for distributing these three actual `.dat` artifacts under the
project's intended distribution, covering their source-derived headers and
any applicable collection/database rights. Either obtain an applicable
written grant through IERS Central Bureau (request clarification of the table
contributor/publisher's rights and commercial redistribution conditions), or
record qualified legal review supporting numerical-facts reuse and identifying
any required header replacement/formatting/attribution changes. Mere public
access or silence is not clearance. Until this decision is recorded, keep
publication/merge blocked. No critical tables were removed. If neither basis
can be substantiated, a separately approved caller-supplied-table design is a
concrete fallback, not an implicit download or silent deletion in that fix.

The owner subsequently selected **unmodified licensed dependency replacement**
on 2026-10-10. The three obsolete `.dat` files were explicitly removed with
`apply_patch` as part of that source replacement. This changes the current
distribution; it does not retroactively license those files in Git history.

## ERFA dependency replacement review (2026-10-10)

Approved use: unmodified dependency only, after verifying license and public
API; no copying of dependency source into orskit implementation. The choice is
`erfa = "=0.2.1"`, defaults disabled (the crate defines no feature flags).
The Cargo-generated lockfile adds only `erfa`, `thiserror` 1.0.69 and its
derive macro; existing package versions are unchanged.

Exact artifacts:

- [Registry version metadata](https://crates.io/api/v1/crates/erfa/0.2.1):
  MPL-2.0, published 2022-11-22, edition 2021, no declared MSRV.
  Archive SHA-256 from registry and Cargo.lock:
  `f63880def87bd7d612b89f9046f5db0961949417f88d831d24596eae555915e5`.
  The downloaded package's `.cargo_vcs_info.json` identifies source commit
  `83190c4a59f61ca6e6e3958592d581ccae20481c` in `cjordan/rust-erfa/erfa`.
- Actual packaged `LICENSE` (not just metadata):
  SHA-256 `161eb77d5438af7810961555787d2ec80c6ad10e179d761f30dac5c760de99ca`.
  Contains Christopher H. Jordan's 2022 MPL-2.0 notice/full text and a
  NumFOCUS 2013-2021 ERFA BSD-style notice for adapted docstrings.
  It is one combined file; there is no packaged `LICENSE-ERFA`.
  The upstream repository also contains a separate ERFA notice, but that
  older 2013-2014 copyright is not substituted for the actual package notice.
- `src/prenut/nut00a.rs` SHA-256
  `8d46651c6319fc308e6c4c131a403e16505a34aeff5c5ccdb0eee4382bf596c3`:
  MPL-marked code including 678 luni-solar and 687 planetary terms.
- `src/time/s06.rs` SHA-256
  `7bff002e50c1cf80d65bc8e49ae451b5a3f53f014c6d3689ae594d7d9725ab02`:
  MPL-marked code including six polynomial coefficients and all 66
  periodic terms in blocks 33/3/25/4/1. These arrays form part of the
  separately licensed software, not downloaded IERS table collections.

License assessment: MPL is file-level copyleft, not MIT/Apache or a permissive
relicensing grant. Unmodified linking in a larger MIT/Apache work is permitted;
MPL source availability and notice duties remain for the dependency.
The ERFA notice permits source/binary redistribution with attribution,
disclaimer retention and no endorsement. `crates/frames-eop/NOTICE` reproduces
the packaged ERFA notice and gives the immutable MPL source/combined-license
locators for binary recipients. Distributors must carry that notice and make
the pinned source available as described; do not relabel dependency code or
its embedded arrays as MIT/Apache. No SOFA endorsement is claimed.

Public APIs verified against versioned docs:
[`pn_matrix_06a`](https://docs.rs/erfa/0.2.1/erfa/prenut/fn.pn_matrix_06a.html),
[`bpn_to_xy`](https://docs.rs/erfa/0.2.1/erfa/prenut/fn.bpn_to_xy.html),
[`S06`](https://docs.rs/erfa/0.2.1/erfa/time/fn.S06.html).
Inputs are two-part TT Julian dates and outputs are CIP coordinates and CIO
locator in radians. The adapter uses the optimal J2000 split, full 2000A
(not 2000B/truncated) nutation, IAU 2006 adjustments and frame bias.
It computes `s` from the uncorrected model `X/Y`, preserving the previous
caller pole-offset convention. Free core nutation remains excluded from the
model; caller EOP corrections remain necessary.

Audit scope was package metadata/licenses, marked coefficient files/term
counts, production safety and public API/model coverage, not source reuse or
architecture translation. Production sources have no unsafe blocks or FFI;
all 56 unsafe occurrences are in the dependency's cfg(test) ERFA-C comparison
module, not linked into orskit. Rust 1.96.1 compatibility is established by
workspace and minimal-feature checks, not a fabricated upstream MSRV.
docs.rs MCP was unavailable; versioned public docs were used directly.

Derivative evidence: no analytical derivative API exists. An original
sixth-order centered stencil in smooth TT time uses 512 s spacing and paired
differences; its Taylor truncation is O(h^6). It never samples EOP across
coverage boundaries, knots or UTC leap seconds. EOP/ERA/TIO and matrix-chain
derivatives remain analytic. A 257-epoch 1972-2100 sweep compares against
256 s refinement and an independent fourth-order 1024 s stencil: maxima
`[4.6643281e-20, 8.8001077e-19, 3.5111362e-21]` rad/s, bounded by
`[2e-18, 2e-18, 2e-20]`. At a 10,000 km lever arm this is a conservative
sub-1e-10 m/s celestial velocity contribution. These are convergence bounds,
not a proof of global model accuracy at arbitrarily remote epochs.

Six original pre-replacement analytic-evaluator regression observations
at TT J2000 offsets -10227.5/0/2191.5/9131.5/18262.5/36524.5 days give maximum
coordinate shift `5.8458031e-12` rad and rate shift `2.7433534e-17` rad/s.
The full model avoids the old X/Y table rounding; the independently published
ERFA reference now passes at 1e-14 rad for X/Y and 1e-18 rad for s.
The 5e-12 full-matrix tolerance and existing changing-EOP velocity/leap/frame
regressions are retained. Legacy observations are not mislabeled as external
reference vectors and do not redistribute the original coefficient collection.

Maintenance review and acceptance rationale: upstream
[`cjordan/rust-erfa`](https://github.com/cjordan/rust-erfa) is not archived,
with zero open issues observed, but last push is 2022-11-23; active maintenance
has **not** been established. This is a mature pinned implementation of a fixed
standard, not an actively released crate. The repository requires license,
API and maintenance **review**, not active releases or an automatic ban on
dormant libraries. The owner's clarification confirms that maintenance was a
preference, not an additional publication gate.

Unmodified `erfa` 0.2.1 is accepted for this narrow use: exact release/source
hash pinning prevents unnoticed upstream changes; IAU 2006/2000A is a fixed
standard; full-series API and embedded software licensing are verified; the
independent ERFA values, derivative convergence/regressions and all 272
workspace tests establish the exercised behavior on Rust 1.96.1. Upstream
inactivity remains a risk of delayed bug fixes and future compiler
compatibility, not evidence of correctness or a reason to claim active
maintenance. Any update requires renewed license/API review and these gates;
future discovered scientific or compatibility defects must be addressed
before shipping affected behavior. Tests support this bounded acceptance,
not a guarantee that no unknown defect exists.

No unresolved scientific, provenance or safe-Rust gate was identified for
this replacement. MPL source availability and ERFA notice retention remain
ordinary distributor obligations, not waived requirements. Historical table
files remain uncleared in Git history; current package contents exclude them.
Alternatives reviewed, not adopted: `erfars` requires C/FFI; `rfa` 0.5.9 is
LGPL-3.0-or-later; `sofars` 0.6.0's nominal MIT metadata omits additional SOFA
terms in its actual LICENSE; `oxiephemeris-bodies` 0.1.1 explicitly imports
IERS electronic tables without a verified separate table grant; current
`lox-frames` has MPL-marked coefficient arrays but its package omits a license
file and a separately verified ERFA/table licensing chain was not established.
None is presented as a verified actively maintained permissive replacement.

## Dependency policy

- Confirm the license from package metadata and the upstream repository before
  adding a dependency.
- Prefer MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, or similarly
  compatible terms; escalate anything else for explicit review.
- Record feature flags and disable unnecessary defaults.
- Generate and review a dependency/license report in CI before releases.
- A dependency is not copied source, but it still forms part of the distributed
  and linked product and must be compatible with each target.

## If provenance is uncertain

Stop the affected implementation. Preserve a short factual note, replace the
questionable material with an independently derived version, and request a
maintainer review. Do not try to make copied material "different enough."
