# CCSDS test resources

`orekit_oem_issue839_covariance.oem` is a focused OEM fixture derived from the
covariance blocks in Orekit's `OEM-Issue839.txt` test resource:

<https://github.com/CS-SI/Orekit/blob/develop/src/test/resources/ccsds/odm/oem/OEM-Issue839.txt>

The resource retains the original covariance rows and their `RTN`/`EME2000`
frame cases, with a minimal OEM envelope so the test is self-contained. Orekit
is licensed under Apache-2.0; this fixture is retained with this attribution
for CCSDS interoperability testing.
