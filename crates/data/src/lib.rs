#![forbid(unsafe_code)]

//! Explicit contracts for caller-selected scientific data.
//!
//! This crate performs no network access and owns no process-global catalog.
//! Applications select an exact local artifact, declare its immutable identity
//! and time coverage, and verify its SHA-256 digest before handing its bytes to
//! a format-specific provider.
//!
//! ```
//! use orskit_data::{ArtifactCoverage, ArtifactDescriptor, Sha256Digest, VerifiedArtifact};
//!
//! let bytes = b"abc".to_vec();
//! let checksum = "ba7816bf8f01cfea414140de5dae2223\
//!                 b00361a396177a9cb410ff61f20015ad"
//!     .parse::<Sha256Digest>()?;
//! let descriptor = ArtifactDescriptor::new(
//!     "example authority",
//!     "example product",
//!     "2026-01",
//!     checksum,
//!     ArtifactCoverage::AllTime,
//! )?;
//! let verified = VerifiedArtifact::from_bytes(descriptor, bytes)?;
//!
//! assert_eq!(verified.bytes(), b"abc");
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::{
    fmt,
    fs::File,
    io::{self, Read},
    num::NonZeroU64,
    path::{Path, PathBuf},
    str::FromStr,
};

use hifitime::Epoch;
use sha2::{Digest, Sha256};
use thiserror::Error;

/// A SHA-256 content digest.
///
/// Text form is exactly 64 lowercase hexadecimal digits. Parsing also accepts
/// uppercase hexadecimal and normalizes it on display.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Sha256Digest([u8; 32]);

impl Sha256Digest {
    /// Computes the SHA-256 digest of `bytes`.
    #[must_use]
    pub fn compute(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }

    /// Returns the 32 digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for Sha256Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl fmt::Display for Sha256Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl FromStr for Sha256Digest {
    type Err = DigestParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != 64 {
            return Err(DigestParseError::InvalidLength {
                actual: value.len(),
            });
        }

        let mut digest = [0_u8; 32];
        for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
            let high =
                hex_value(pair[0]).ok_or(DigestParseError::InvalidHex { index: index * 2 })?;
            let low = hex_value(pair[1]).ok_or(DigestParseError::InvalidHex {
                index: index * 2 + 1,
            })?;
            digest[index] = (high << 4) | low;
        }
        Ok(Self(digest))
    }
}

const fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Failure while parsing a textual SHA-256 digest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum DigestParseError {
    /// SHA-256 text did not contain exactly 64 hexadecimal digits.
    #[error("SHA-256 digest must contain 64 hexadecimal digits, got {actual}")]
    InvalidLength {
        /// Actual byte length of the supplied text.
        actual: usize,
    },
    /// A non-hexadecimal byte occurred at `index`.
    #[error("SHA-256 digest contains a non-hexadecimal byte at index {index}")]
    InvalidHex {
        /// Zero-based byte index in the supplied text.
        index: usize,
    },
}

/// Inclusive time interval covered by a scientific-data artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeCoverage {
    start: Epoch,
    end: Epoch,
}

impl TimeCoverage {
    /// Constructs an inclusive interval.
    ///
    /// Returns an error when `end` precedes `start`.
    pub fn new(start: Epoch, end: Epoch) -> Result<Self, CoverageError> {
        if end < start {
            return Err(CoverageError::Reversed { start, end });
        }
        Ok(Self { start, end })
    }

    /// First covered instant.
    #[must_use]
    pub const fn start(&self) -> Epoch {
        self.start
    }

    /// Last covered instant.
    #[must_use]
    pub const fn end(&self) -> Epoch {
        self.end
    }

    /// Whether `epoch` lies in this inclusive interval.
    #[must_use]
    pub fn contains(&self, epoch: Epoch) -> bool {
        self.start <= epoch && epoch <= self.end
    }
}

/// Declared temporal coverage of an artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactCoverage {
    /// The artifact is not limited to a time interval, such as a convention
    /// document or static physical constants table.
    AllTime,
    /// The artifact covers exactly one inclusive interval.
    Interval(TimeCoverage),
}

impl ArtifactCoverage {
    /// Verifies that `epoch` is covered.
    pub fn require(&self, epoch: Epoch) -> Result<(), CoverageError> {
        match self {
            Self::AllTime => Ok(()),
            Self::Interval(coverage) if coverage.contains(epoch) => Ok(()),
            Self::Interval(coverage) => Err(CoverageError::Outside {
                epoch,
                start: coverage.start,
                end: coverage.end,
            }),
        }
    }
}

/// Failure from constructing or querying a time-coverage interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum CoverageError {
    /// The interval end precedes its start.
    #[error("scientific-data coverage ends at {end}, before its start at {start}")]
    Reversed {
        /// Requested first instant.
        start: Epoch,
        /// Requested last instant.
        end: Epoch,
    },
    /// A requested instant is outside an artifact's inclusive coverage.
    #[error("epoch {epoch} is outside scientific-data coverage [{start}, {end}]")]
    Outside {
        /// Requested instant.
        epoch: Epoch,
        /// First covered instant.
        start: Epoch,
        /// Last covered instant.
        end: Epoch,
    },
}

/// Immutable identity and applicability of one scientific-data artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactDescriptor {
    authority: String,
    product: String,
    version: String,
    checksum: Sha256Digest,
    coverage: ArtifactCoverage,
}

impl ArtifactDescriptor {
    /// Constructs a complete artifact descriptor.
    ///
    /// Authority, product, and version must each contain non-whitespace text.
    pub fn new(
        authority: impl Into<String>,
        product: impl Into<String>,
        version: impl Into<String>,
        checksum: Sha256Digest,
        coverage: ArtifactCoverage,
    ) -> Result<Self, ArtifactDescriptorError> {
        let descriptor = Self {
            authority: authority.into(),
            product: product.into(),
            version: version.into(),
            checksum,
            coverage,
        };
        for (field, value) in [
            (ArtifactIdentityField::Authority, &descriptor.authority),
            (ArtifactIdentityField::Product, &descriptor.product),
            (ArtifactIdentityField::Version, &descriptor.version),
        ] {
            if value.trim().is_empty() {
                return Err(ArtifactDescriptorError::BlankIdentity { field });
            }
        }
        Ok(descriptor)
    }

    /// Publishing organization or application that supplied the data.
    #[must_use]
    pub fn authority(&self) -> &str {
        &self.authority
    }

    /// Product or data-family identity.
    #[must_use]
    pub fn product(&self) -> &str {
        &self.product
    }

    /// Immutable release, issue, or application-defined version.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Expected SHA-256 content digest.
    #[must_use]
    pub const fn checksum(&self) -> Sha256Digest {
        self.checksum
    }

    /// Declared temporal coverage.
    #[must_use]
    pub const fn coverage(&self) -> ArtifactCoverage {
        self.coverage
    }
}

/// One required artifact-identity field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactIdentityField {
    /// Publishing authority.
    Authority,
    /// Product or data family.
    Product,
    /// Immutable version or revision.
    Version,
}

impl fmt::Display for ArtifactIdentityField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Authority => "authority",
            Self::Product => "product",
            Self::Version => "version",
        })
    }
}

/// Invalid artifact identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum ArtifactDescriptorError {
    /// A required identity field contained no non-whitespace text.
    #[error("scientific-data artifact has a blank {field}")]
    BlankIdentity {
        /// Invalid field.
        field: ArtifactIdentityField,
    },
}

/// Immutable bytes whose content matches their declared digest.
#[derive(Debug)]
pub struct VerifiedArtifact {
    descriptor: ArtifactDescriptor,
    bytes: Box<[u8]>,
}

impl VerifiedArtifact {
    /// Verifies owned bytes against `descriptor`.
    pub fn from_bytes(
        descriptor: ArtifactDescriptor,
        bytes: Vec<u8>,
    ) -> Result<Self, ArtifactLoadError> {
        let actual = Sha256Digest::compute(&bytes);
        if actual != descriptor.checksum {
            return Err(ArtifactLoadError::ChecksumMismatch {
                expected: descriptor.checksum,
                actual,
            });
        }
        Ok(Self {
            descriptor,
            bytes: bytes.into_boxed_slice(),
        })
    }

    /// Loads and verifies one explicitly selected local file.
    ///
    /// At most `maximum_bytes` plus one sentinel byte is read, preventing an
    /// untrusted or changing file from causing an unbounded allocation.
    pub fn load(
        path: impl AsRef<Path>,
        descriptor: ArtifactDescriptor,
        maximum_bytes: NonZeroU64,
    ) -> Result<Self, ArtifactLoadError> {
        let path = path.as_ref();
        let file = File::open(path).map_err(|source| ArtifactLoadError::Open {
            path: path.to_owned(),
            source,
        })?;
        let mut bytes = Vec::new();
        file.take(maximum_bytes.get().saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|source| ArtifactLoadError::Read {
                path: path.to_owned(),
                source,
            })?;
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum_bytes.get() {
            return Err(ArtifactLoadError::MaximumSizeExceeded {
                maximum_bytes: maximum_bytes.get(),
            });
        }
        Self::from_bytes(descriptor, bytes)
    }

    /// Verified artifact identity, version, digest, and coverage.
    #[must_use]
    pub const fn descriptor(&self) -> &ArtifactDescriptor {
        &self.descriptor
    }

    /// Verified immutable content.
    #[must_use]
    pub const fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Failure while loading or verifying a selected local artifact.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ArtifactLoadError {
    /// The selected path could not be opened.
    #[error("failed to open scientific-data artifact {}", path.display())]
    Open {
        /// Selected local path.
        path: PathBuf,
        /// Filesystem failure.
        #[source]
        source: io::Error,
    },
    /// The selected file could not be read.
    #[error("failed to read scientific-data artifact {}", path.display())]
    Read {
        /// Selected local path.
        path: PathBuf,
        /// Filesystem failure.
        #[source]
        source: io::Error,
    },
    /// The selected file exceeded the caller's allocation limit.
    #[error("scientific-data artifact exceeds the {maximum_bytes}-byte limit")]
    MaximumSizeExceeded {
        /// Caller-selected maximum.
        maximum_bytes: u64,
    },
    /// Loaded content did not match the selected immutable digest.
    #[error("scientific-data checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch {
        /// Digest declared by the caller.
        expected: Sha256Digest,
        /// Digest calculated from the loaded content.
        actual: Sha256Digest,
    },
}

#[cfg(test)]
mod tests {
    use std::error::Error as _;

    use super::*;

    const ABC_SHA256: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    fn descriptor(bytes: &[u8]) -> ArtifactDescriptor {
        ArtifactDescriptor::new(
            "test authority",
            "test product",
            "test version",
            Sha256Digest::compute(bytes),
            ArtifactCoverage::AllTime,
        )
        .expect("valid descriptor")
    }

    #[test]
    fn sha256_matches_the_standard_abc_vector() {
        assert_eq!(Sha256Digest::compute(b"abc").to_string(), ABC_SHA256);
        assert_eq!(
            ABC_SHA256.parse::<Sha256Digest>(),
            Ok(Sha256Digest::compute(b"abc"))
        );
    }

    #[test]
    fn digest_text_is_exact_and_reports_the_bad_position() {
        assert_eq!(
            "00".parse::<Sha256Digest>(),
            Err(DigestParseError::InvalidLength { actual: 2 })
        );
        let invalid = format!("{}z", &ABC_SHA256[..63]);
        assert_eq!(
            invalid.parse::<Sha256Digest>(),
            Err(DigestParseError::InvalidHex { index: 63 })
        );
    }

    #[test]
    fn descriptors_reject_blank_identity_fields() {
        assert_eq!(
            ArtifactDescriptor::new(
                " ",
                "product",
                "version",
                Sha256Digest::compute(b""),
                ArtifactCoverage::AllTime,
            ),
            Err(ArtifactDescriptorError::BlankIdentity {
                field: ArtifactIdentityField::Authority,
            })
        );
    }

    #[test]
    fn time_coverage_is_inclusive_and_rejects_reversal() {
        let start = Epoch::from_tai_seconds(10.0);
        let end = Epoch::from_tai_seconds(20.0);
        let coverage = TimeCoverage::new(start, end).expect("ordered coverage");
        assert!(coverage.contains(start));
        assert!(coverage.contains(end));
        assert_eq!(
            ArtifactCoverage::Interval(coverage).require(Epoch::from_tai_seconds(9.0)),
            Err(CoverageError::Outside {
                epoch: Epoch::from_tai_seconds(9.0),
                start,
                end,
            })
        );
        assert_eq!(
            TimeCoverage::new(end, start),
            Err(CoverageError::Reversed {
                start: end,
                end: start
            })
        );
    }

    #[test]
    fn owned_bytes_require_the_declared_digest() {
        let error = VerifiedArtifact::from_bytes(descriptor(b"expected"), b"actual".to_vec())
            .expect_err("different bytes");
        assert!(matches!(error, ArtifactLoadError::ChecksumMismatch { .. }));
    }

    #[test]
    fn local_loading_is_bounded_and_offline() {
        let path = std::env::temp_dir().join(format!(
            "orskit-data-{}-bounded-artifact.bin",
            std::process::id()
        ));
        std::fs::write(&path, b"abc").expect("temporary test artifact");

        let loaded = VerifiedArtifact::load(
            &path,
            descriptor(b"abc"),
            NonZeroU64::new(3).expect("non-zero"),
        )
        .expect("bounded verified load");
        assert_eq!(loaded.bytes(), b"abc");
        assert!(matches!(
            VerifiedArtifact::load(
                &path,
                descriptor(b"abc"),
                NonZeroU64::new(2).expect("non-zero"),
            ),
            Err(ArtifactLoadError::MaximumSizeExceeded { maximum_bytes: 2 })
        ));

        std::fs::remove_file(path).expect("remove temporary test artifact");
    }

    #[test]
    fn local_open_failures_preserve_the_io_source() {
        let path = std::env::temp_dir().join(format!(
            "orskit-data-{}-missing-artifact.bin",
            std::process::id()
        ));
        let error =
            VerifiedArtifact::load(path, descriptor(b""), NonZeroU64::new(1).expect("non-zero"))
                .expect_err("missing path");

        assert!(matches!(error, ArtifactLoadError::Open { .. }));
        assert!(error.source().is_some());
    }
}
