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
| Sequential Cartesian orbit determination | [orskit propagation contracts](../../crates/dynamics/core/src/propagator.rs), [finitediff 0.2.0](https://crates.io/crates/finitediff/0.2.0), and [Orekit 13.1.6 public API](https://www.orekit.org/static/apidocs/) | Original project contract; MIT OR Apache-2.0 dependency; Apache-2.0 public API/black-box behavior | The OD boundary delegates physical propagation to one caller-selected propagator that owns its physical problem. `finitediff` supplies the unmodified central-Jacobian algorithm. The isolated Orekit public-API benchmark compares one Cartesian position EKF correction; no Orekit source, tests, examples, data, or implementation structure was used by OD | `crates/orbit-determination`; `.agent/references/orbit-determination`; ADR-0036; tasks 0030 and 0031 |
| Reference-data-backed frame transforms | [JPL DE440/DE441 description](https://ssd.jpl.nasa.gov/doc/de440_de441.html), [NAIF SPK required reading](https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/C/req/spk.html), and [IERS data products](https://www.iers.org/IERS/EN/DataProducts/data.html) | JPL DE440/DE441 are planetary and lunar ephemerides; NAIF SPK is an ephemeris-kernel format containing position/velocity segments; IERS publishes Earth-orientation and ICRF/ITRF data products. These facts establish that a production terrestrial/celestial supplier must identify separately selected ephemeris and Earth-orientation inputs. No transform equations, source code, tests, or data files were copied | `crates/frames/src/lib.rs`; ADR-0035; task 0029 |
| Force and torque capability inventory | [Orekit 13.1.7 force packages](https://www.orekit.org/site-orekit-13.1.7/apidocs/org/orekit/forces/package-summary.html), [IERS Conventions 2010 Chapter 6](https://iers-conventions.obspm.fr/content/chapter6/icc6.pdf), [NASA GMAT force-model documentation](https://documentation.help/GMAT/Propagator.html), and [NASA attitude-control survey](https://s3vi.ndc.nasa.gov/ssri-kb/static/resources/A%20Brief%20Survey%20of%20Attitude%20Control%20Systems%20for%20Small%20Satellites%20u.pdf) | Public behavior/capability documentation, public standards, and US Government technical documentation | Capability names, model-family boundaries, tide-convention warning, and disturbance-torque categories only; no implementation material | `.agent/FORCE_MODELS.md` |

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
