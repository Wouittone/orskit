# CCSDS OEM conformance resources

`orekit_oem_issue839_covariance.oem` is a focused OEM fixture derived from the
covariance blocks in Orekit's `OEM-Issue839.txt` test resource:

<https://github.com/CS-SI/Orekit/blob/develop/src/test/resources/ccsds/odm/oem/OEM-Issue839.txt>

The resource retains the original covariance rows and their `RTN`/`EME2000`
frame cases, with a minimal OEM envelope so the test is self-contained. Orekit
is licensed under Apache-2.0; this fixture is retained with this attribution
for CCSDS interoperability testing.

`project_multisegment.oem` is original orskit test data released under the
repository's MIT/Apache-2.0 terms. It combines two segments, Earth and Mars
origins, EME2000 and ICRF orientations, UTC and TAI epochs, optional
interpolation metadata, comments, and a state with acceleration. It is
operationally representative synthetic data, not a claimed mission product.

These two files form the maintained OEM KVN conformance corpus. The first
provides attributed interoperability evidence from a permissively licensed
implementation; the second covers supported standard combinations without
redistributing an OEM copied from a public website or standard. The integration
tests in `../tests/oem_conformance.rs` pin their expected semantics.

`project_oem_3_0.xml` is original orskit test data following the OEM XML
structure and element names specified by CCSDS 502.0-B-3 and the declaration,
namespace, and unqualified form specified by CCSDS 505.0-B-3. It covers
comments, optional acceleration, covariance, unit attributes, and the shared
event/document semantics. The test suite derives a consistently
`ndm:`-qualified form from the same data rather than copying a standard annex
example.

Raw minimized fuzz findings belong in `fuzz-regressions/`; see
`../fuzz/README.md` for the bounded harness and preservation workflow.
