//! CCSDS 502.0-B-3 OEM KVN reader.

use std::{fmt, io::BufRead, str::FromStr, sync::Arc};

use frames::{Body, FrameOrientation, FrameOrigin, ReferenceFrame};
use orbits::cartesian::{
    CartesianCoordinates, CoordinateSample, FramedAcceleration, FramedPosition, FramedVelocity,
    KinematicError,
};
use orskit_core::Epoch;
use thiserror::Error;
use units::uom::si::area::square_kilometer;
use units::{
    AccelerationVector, Area, Position, PositionVelocityCovariance, VelocityVariance,
    VelocityVector,
};

#[cfg(feature = "parallel")]
use rayon::prelude::*;
#[cfg(feature = "async")]
use tokio::io::{AsyncBufRead, AsyncBufReadExt};

const DEFAULT_MAX_LINE_BYTES: usize = 64 * 1024;
const DEFAULT_MAX_SECTION_BYTES: usize = 256 * 1024 * 1024;
const DEFAULT_MAX_SECTION_LINES: usize = 4_000_000;
const DEFAULT_MAX_DOCUMENT_BYTES: usize = 512 * 1024 * 1024;
const DEFAULT_MAX_DOCUMENT_LINES: usize = 8_000_000;

/// OEM KVN section in which a decoder resource limit was reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum OemSection {
    /// Message header, through its `META_START` marker.
    Header,
    /// Segment metadata, through its `META_STOP` marker.
    Metadata,
    /// Segment ephemeris data, through the next `META_START` marker or EOF.
    Data,
}

impl fmt::Display for OemSection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Header => "header",
            Self::Metadata => "metadata",
            Self::Data => "data",
        })
    }
}

/// Kind of bounded OEM decoder resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum OemLimitKind {
    /// Content bytes in one physical line, excluding LF or CRLF.
    LineBytes,
    /// Cumulative content bytes in one header, metadata, or data section.
    SectionBytes,
    /// Physical lines in one header, metadata, or data section.
    SectionLines,
    /// Cumulative content bytes in the complete document.
    DocumentBytes,
    /// Cumulative physical lines in the complete document.
    DocumentLines,
}

impl fmt::Display for OemLimitKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::LineBytes => "line bytes",
            Self::SectionBytes => "section bytes",
            Self::SectionLines => "section lines",
            Self::DocumentBytes => "document bytes",
            Self::DocumentLines => "document lines",
        })
    }
}

/// Finite allocation and work limits shared by every OEM KVN decoder mode.
///
/// Byte limits count source content only; LF and CRLF terminators do not count.
/// Section counters reset after each structural section boundary; document
/// counters never reset. Document line/byte budgets also bound the possible
/// number of segments and records because every one consumes source lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OemDecoderLimits {
    max_line_bytes: usize,
    max_section_bytes: usize,
    max_section_lines: usize,
    max_document_bytes: usize,
    max_document_lines: usize,
}

impl OemDecoderLimits {
    /// Constructs non-zero decoder limits.
    pub fn new(
        max_line_bytes: usize,
        max_section_bytes: usize,
        max_section_lines: usize,
        max_document_bytes: usize,
        max_document_lines: usize,
    ) -> Result<Self, OemDecoderLimitsError> {
        for (kind, value) in [
            (OemLimitKind::LineBytes, max_line_bytes),
            (OemLimitKind::SectionBytes, max_section_bytes),
            (OemLimitKind::SectionLines, max_section_lines),
            (OemLimitKind::DocumentBytes, max_document_bytes),
            (OemLimitKind::DocumentLines, max_document_lines),
        ] {
            if value == 0 {
                return Err(OemDecoderLimitsError::Zero { kind });
            }
            if value == usize::MAX {
                return Err(OemDecoderLimitsError::Unbounded { kind });
            }
        }
        Ok(Self {
            max_line_bytes,
            max_section_bytes,
            max_section_lines,
            max_document_bytes,
            max_document_lines,
        })
    }

    /// Returns the maximum content bytes in one physical line.
    #[must_use]
    pub const fn max_line_bytes(self) -> usize {
        self.max_line_bytes
    }

    /// Returns the maximum cumulative content bytes in one section.
    #[must_use]
    pub const fn max_section_bytes(self) -> usize {
        self.max_section_bytes
    }

    /// Returns the maximum physical lines in one section.
    #[must_use]
    pub const fn max_section_lines(self) -> usize {
        self.max_section_lines
    }

    /// Returns the maximum cumulative content bytes in the complete document.
    #[must_use]
    pub const fn max_document_bytes(self) -> usize {
        self.max_document_bytes
    }

    /// Returns the maximum cumulative physical lines in the complete document.
    #[must_use]
    pub const fn max_document_lines(self) -> usize {
        self.max_document_lines
    }
}

impl Default for OemDecoderLimits {
    fn default() -> Self {
        Self {
            max_line_bytes: DEFAULT_MAX_LINE_BYTES,
            max_section_bytes: DEFAULT_MAX_SECTION_BYTES,
            max_section_lines: DEFAULT_MAX_SECTION_LINES,
            max_document_bytes: DEFAULT_MAX_DOCUMENT_BYTES,
            max_document_lines: DEFAULT_MAX_DOCUMENT_LINES,
        }
    }
}

/// Invalid OEM decoder-limit configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum OemDecoderLimitsError {
    /// A decoder limit was configured as zero.
    #[error("OEM decoder {kind} limit must be non-zero")]
    Zero {
        /// Invalid limit kind.
        kind: OemLimitKind,
    },
    /// A saturating counter could never exceed the configured maximum.
    #[error("OEM decoder {kind} limit must be less than usize::MAX")]
    Unbounded {
        /// Invalid limit kind.
        kind: OemLimitKind,
    },
}

/// Absolute CCSDS time systems supported by the OEM reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum OemTimeSystem {
    /// Coordinated Universal Time.
    Utc,
    /// International Atomic Time.
    Tai,
    /// Terrestrial Time.
    Tt,
    /// Barycentric Dynamical Time.
    Tdb,
    /// GPS system time, represented by Hifitime's `GPST` scale.
    Gps,
}

impl OemTimeSystem {
    fn hifitime_suffix(self) -> &'static str {
        match self {
            Self::Utc => "UTC",
            Self::Tai => "TAI",
            Self::Tt => "TT",
            Self::Tdb => "TDB",
            Self::Gps => "GPST",
        }
    }
}

impl fmt::Display for OemTimeSystem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Utc => "UTC",
            Self::Tai => "TAI",
            Self::Tt => "TT",
            Self::Tdb => "TDB",
            Self::Gps => "GPS",
        };
        formatter.write_str(value)
    }
}

impl FromStr for OemTimeSystem {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_uppercase().as_str() {
            "UTC" => Ok(Self::Utc),
            "TAI" => Ok(Self::Tai),
            "TT" => Ok(Self::Tt),
            "TDB" => Ok(Self::Tdb),
            "GPS" | "GPST" => Ok(Self::Gps),
            _ => Err(()),
        }
    }
}

/// Stable zero-based identifier for an OEM segment in source order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OemSegmentId(usize);

impl OemSegmentId {
    /// Returns the segment's zero-based source-order index.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// One accepted OEM comment with its source provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OemComment {
    segment_id: Option<OemSegmentId>,
    section: OemSection,
    source_line: usize,
    text: String,
}

impl OemComment {
    /// Returns the containing segment, or `None` for a header comment.
    #[must_use]
    pub const fn segment_id(&self) -> Option<OemSegmentId> {
        self.segment_id
    }

    /// Returns the structural section containing the comment.
    #[must_use]
    pub const fn section(&self) -> OemSection {
        self.section
    }

    /// Returns the one-based physical source line.
    #[must_use]
    pub const fn source_line(&self) -> usize {
        self.source_line
    }

    /// Returns the comment text after the `COMMENT` marker.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }
}

/// OEM file header.
#[derive(Debug, Clone, PartialEq)]
pub struct OemHeader {
    version: String,
    creation_date: Epoch,
    originator: String,
    message_id: Option<String>,
    comments: Vec<OemComment>,
}

impl OemHeader {
    /// Returns the CCSDS OEM format version exactly as declared.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Returns the UTC creation instant.
    #[must_use]
    pub const fn creation_date(&self) -> Epoch {
        self.creation_date
    }

    /// Returns the creating agency or operator.
    #[must_use]
    pub fn originator(&self) -> &str {
        &self.originator
    }

    /// Returns the optional message identifier.
    #[must_use]
    pub fn message_id(&self) -> Option<&str> {
        self.message_id.as_deref()
    }

    /// Returns header comments in source order.
    #[must_use]
    pub fn comments(&self) -> &[OemComment] {
        &self.comments
    }
}

/// Metadata applying to one OEM ephemeris segment.
#[derive(Debug, Clone, PartialEq)]
pub struct OemMetadata {
    object_name: String,
    object_id: String,
    frame: ReferenceFrame,
    time_system: OemTimeSystem,
    start_time: Epoch,
    usable_start_time: Option<Epoch>,
    usable_stop_time: Option<Epoch>,
    stop_time: Epoch,
    interpolation: Option<String>,
    interpolation_degree: Option<u8>,
    comments: Vec<OemComment>,
}

impl OemMetadata {
    /// Returns the object name.
    #[must_use]
    pub fn object_name(&self) -> &str {
        &self.object_name
    }

    /// Returns the object identifier.
    #[must_use]
    pub fn object_id(&self) -> &str {
        &self.object_id
    }

    /// Returns the frame composed from `CENTER_NAME` and `REF_FRAME`.
    #[must_use]
    pub const fn frame(&self) -> ReferenceFrame {
        self.frame
    }

    /// Returns the declared time system.
    #[must_use]
    pub const fn time_system(&self) -> OemTimeSystem {
        self.time_system
    }

    /// Returns the segment start time.
    #[must_use]
    pub const fn start_time(&self) -> Epoch {
        self.start_time
    }

    /// Returns the optional usable start time.
    #[must_use]
    pub const fn usable_start_time(&self) -> Option<Epoch> {
        self.usable_start_time
    }

    /// Returns the optional usable stop time.
    #[must_use]
    pub const fn usable_stop_time(&self) -> Option<Epoch> {
        self.usable_stop_time
    }

    /// Returns the segment stop time.
    #[must_use]
    pub const fn stop_time(&self) -> Epoch {
        self.stop_time
    }

    /// Returns the declared interpolation method, if present.
    #[must_use]
    pub fn interpolation(&self) -> Option<&str> {
        self.interpolation.as_deref()
    }

    /// Returns the declared interpolation degree, if present.
    #[must_use]
    pub const fn interpolation_degree(&self) -> Option<u8> {
        self.interpolation_degree
    }

    /// Returns metadata comments in source order.
    #[must_use]
    pub fn comments(&self) -> &[OemComment] {
        &self.comments
    }
}

/// Immutable identity and metadata shared by all records in one OEM segment.
///
/// Cloning this value shares the validated metadata allocation rather than
/// duplicating it, so streaming samples retain the exact segment context that
/// was active when their source line was accepted.
#[derive(Debug, Clone, PartialEq)]
pub struct OemSegmentContext {
    id: OemSegmentId,
    metadata: Arc<OemMetadata>,
}

impl OemSegmentContext {
    /// Returns the segment's stable source-order identifier.
    #[must_use]
    pub const fn id(&self) -> OemSegmentId {
        self.id
    }

    /// Returns the immutable metadata applying to the segment.
    #[must_use]
    pub fn metadata(&self) -> &OemMetadata {
        &self.metadata
    }
}

/// One typed OEM Cartesian sample with source and segment provenance.
#[derive(Debug, Clone, PartialEq)]
pub struct OemSample {
    context: OemSegmentContext,
    source_line: usize,
    sample: CoordinateSample<CartesianCoordinates>,
}

/// Axes in which an OEM Cartesian covariance matrix is expressed.
///
/// Applications can supply an implementation for an axes convention not
/// represented by this crate. The identifier preserves the `COV_REF_FRAME`
/// declaration used on the wire.
pub trait OemCovarianceAxes: fmt::Debug + Send + Sync + 'static {
    /// Returns the declared OEM `COV_REF_FRAME` identifier.
    fn identifier(&self) -> &str;

    /// Returns a catalogued Cartesian reference frame when these axes have one.
    fn reference_frame(&self) -> Option<ReferenceFrame> {
        None
    }
}

/// Catalogued Cartesian axes declared for an OEM covariance matrix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceCovarianceAxes {
    frame: ReferenceFrame,
    identifier: String,
}

impl ReferenceCovarianceAxes {
    /// Constructs catalogued Cartesian covariance axes.
    #[must_use]
    pub fn new(frame: ReferenceFrame) -> Self {
        Self {
            identifier: frame.orientation().to_string(),
            frame,
        }
    }
}

impl OemCovarianceAxes for ReferenceCovarianceAxes {
    fn identifier(&self) -> &str {
        &self.identifier
    }

    fn reference_frame(&self) -> Option<ReferenceFrame> {
        Some(self.frame)
    }
}

/// Local radial, transverse, normal covariance axes.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RtnCovarianceAxes;

impl OemCovarianceAxes for RtnCovarianceAxes {
    fn identifier(&self) -> &str {
        "RTN"
    }
}

/// Covariance axes preserved from an OEM declaration not catalogued by this crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredCovarianceAxes {
    identifier: String,
}

impl DeclaredCovarianceAxes {
    /// Preserves an application-defined OEM covariance-axes identifier.
    #[must_use]
    pub fn new(identifier: impl Into<String>) -> Self {
        Self {
            identifier: identifier.into(),
        }
    }
}

impl OemCovarianceAxes for DeclaredCovarianceAxes {
    fn identifier(&self) -> &str {
        &self.identifier
    }
}

/// Type-erased, extensible covariance axes attached to an OEM covariance matrix.
#[derive(Clone)]
pub struct OemCovarianceFrame {
    axes: Arc<dyn OemCovarianceAxes>,
}

impl OemCovarianceFrame {
    /// Constructs a covariance-axes declaration from an application-owned implementation.
    #[must_use]
    pub fn new(axes: impl OemCovarianceAxes) -> Self {
        Self {
            axes: Arc::new(axes),
        }
    }

    /// Returns the underlying covariance-axes contract.
    #[must_use]
    pub fn axes(&self) -> &dyn OemCovarianceAxes {
        self.axes.as_ref()
    }

    /// Returns the declared OEM `COV_REF_FRAME` identifier.
    #[must_use]
    pub fn identifier(&self) -> &str {
        self.axes.identifier()
    }

    /// Returns a catalogued Cartesian reference frame when these axes have one.
    #[must_use]
    pub fn reference_frame(&self) -> Option<ReferenceFrame> {
        self.axes.reference_frame()
    }
}

impl fmt::Debug for OemCovarianceFrame {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OemCovarianceFrame")
            .field("identifier", &self.identifier())
            .field("reference_frame", &self.reference_frame())
            .finish()
    }
}

impl PartialEq for OemCovarianceFrame {
    fn eq(&self, other: &Self) -> bool {
        self.identifier() == other.identifier() && self.reference_frame() == other.reference_frame()
    }
}

impl Eq for OemCovarianceFrame {}

/// One unit-qualified entry in a Cartesian position/velocity covariance matrix.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum CartesianCovarianceEntry {
    /// Position/position covariance in square length.
    Position(Area),
    /// Position/velocity covariance in square length per time.
    PositionVelocity(PositionVelocityCovariance),
    /// Velocity/velocity covariance in square velocity.
    Velocity(VelocityVariance),
}

/// One OEM Cartesian covariance matrix with source and segment provenance.
///
/// OEM supplies the lower triangle in km-based units. The reader retains a
/// symmetric `6 × 6` matrix whose entries retain their physical dimensions.
#[derive(Debug, Clone, PartialEq)]
pub struct OemCartesianCovariance {
    context: OemSegmentContext,
    source_line: usize,
    epoch: Epoch,
    frame: OemCovarianceFrame,
    matrix: [[CartesianCovarianceEntry; 6]; 6],
}

impl OemCartesianCovariance {
    /// Returns the containing segment identifier.
    #[must_use]
    pub const fn segment_id(&self) -> OemSegmentId {
        self.context.id()
    }

    /// Returns the one-based source line containing this covariance epoch.
    #[must_use]
    pub const fn source_line(&self) -> usize {
        self.source_line
    }

    /// Returns the immutable context shared by the containing segment.
    #[must_use]
    pub const fn context(&self) -> &OemSegmentContext {
        &self.context
    }

    /// Returns the covariance epoch.
    #[must_use]
    pub const fn epoch(&self) -> Epoch {
        self.epoch
    }

    /// Returns the covariance reference axes.
    #[must_use]
    pub const fn frame(&self) -> &OemCovarianceFrame {
        &self.frame
    }

    /// Returns the symmetric, unit-qualified Cartesian covariance matrix.
    #[must_use]
    pub const fn matrix(&self) -> &[[CartesianCovarianceEntry; 6]; 6] {
        &self.matrix
    }
}

impl OemSample {
    /// Returns the containing segment identifier.
    #[must_use]
    pub const fn segment_id(&self) -> OemSegmentId {
        self.context.id()
    }

    /// Returns the one-based physical source line containing the state record.
    #[must_use]
    pub const fn source_line(&self) -> usize {
        self.source_line
    }

    /// Returns the immutable context shared by the segment's records.
    #[must_use]
    pub const fn context(&self) -> &OemSegmentContext {
        &self.context
    }

    /// Returns the immutable metadata applying to this sample.
    #[must_use]
    pub fn metadata(&self) -> &OemMetadata {
        self.context.metadata()
    }

    /// Returns the sample epoch in the metadata's declared time system.
    #[must_use]
    pub fn epoch(&self) -> Epoch {
        self.sample.epoch()
    }

    /// Returns the typed Cartesian coordinates.
    #[must_use]
    pub fn coordinates(&self) -> &CartesianCoordinates {
        self.sample.coordinates()
    }

    /// Returns the underlying typed coordinate sample.
    #[must_use]
    pub const fn coordinate_sample(&self) -> &CoordinateSample<CartesianCoordinates> {
        &self.sample
    }
}

/// Borrowed record from a collected OEM segment, in original source order.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum OemRecordRef<'a> {
    /// A metadata- or data-section comment.
    Comment(&'a OemComment),
    /// A typed Cartesian state record.
    Coordinates(&'a OemSample),
    /// A Cartesian covariance matrix.
    Covariance(&'a OemCartesianCovariance),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OemRecordIndex {
    Comment(usize),
    Coordinates(usize),
    Covariance(usize),
}

/// One collected OEM segment.
#[derive(Debug, Clone, PartialEq)]
pub struct OemSegment {
    context: OemSegmentContext,
    coordinates: Vec<OemSample>,
    covariances: Vec<OemCartesianCovariance>,
    comments: Vec<OemComment>,
    record_order: Vec<OemRecordIndex>,
}

impl OemSegment {
    /// Returns the segment's stable source-order identifier.
    #[must_use]
    pub const fn id(&self) -> OemSegmentId {
        self.context.id()
    }

    /// Returns the immutable context shared by the segment's records.
    #[must_use]
    pub const fn context(&self) -> &OemSegmentContext {
        &self.context
    }

    /// Returns the segment metadata.
    #[must_use]
    pub fn metadata(&self) -> &OemMetadata {
        self.context.metadata()
    }

    /// Returns timed ephemeris coordinates in source order.
    #[must_use]
    pub fn coordinates(&self) -> &[OemSample] {
        &self.coordinates
    }

    /// Returns Cartesian covariance matrices in source order.
    #[must_use]
    pub fn covariances(&self) -> &[OemCartesianCovariance] {
        &self.covariances
    }

    /// Returns metadata and data comments in source order.
    #[must_use]
    pub fn comments(&self) -> &[OemComment] {
        &self.comments
    }

    /// Iterates comments and coordinate records in original source order.
    ///
    /// Metadata comments precede data-section records. Data comments remain
    /// interleaved with the coordinate lines they surrounded in the source.
    pub fn records(
        &self,
    ) -> impl DoubleEndedIterator<Item = OemRecordRef<'_>> + ExactSizeIterator + '_ {
        self.record_order.iter().map(|record| match *record {
            OemRecordIndex::Comment(index) => OemRecordRef::Comment(&self.comments[index]),
            OemRecordIndex::Coordinates(index) => {
                OemRecordRef::Coordinates(&self.coordinates[index])
            }
            OemRecordIndex::Covariance(index) => OemRecordRef::Covariance(&self.covariances[index]),
        })
    }
}

/// A collected OEM document.
#[derive(Debug, Clone, PartialEq)]
pub struct Oem {
    header: OemHeader,
    segments: Vec<OemSegment>,
}

impl Oem {
    /// Returns the message header.
    #[must_use]
    pub const fn header(&self) -> &OemHeader {
        &self.header
    }

    /// Returns all message segments in source order.
    #[must_use]
    pub fn segments(&self) -> &[OemSegment] {
        &self.segments
    }
}

/// Event emitted by a streaming OEM reader.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum OemEvent {
    /// The validated message header.
    Header(OemHeader),
    /// The beginning of an identified segment and its validated metadata.
    SegmentStart(OemSegmentContext),
    /// One data-section comment at its original source position.
    Comment(OemComment),
    /// One typed, timed Cartesian ephemeris point with source provenance.
    Coordinates(Box<OemSample>),
    /// One Cartesian covariance matrix.
    Covariance(Box<OemCartesianCovariance>),
    /// The end of the identified segment.
    SegmentEnd(OemSegmentId),
}

/// Error returned while decoding or collecting OEM KVN.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum OemError {
    /// Reading the source failed.
    #[error("I/O error after OEM line {line}: {source}")]
    Io {
        /// Last source line reached.
        line: usize,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// A configured decoder resource limit was exceeded.
    #[error(
        "OEM {section} {kind} limit exceeded at line {line}: configured {configured}, observed {observed}"
    )]
    ResourceLimitExceeded {
        /// Source line that crossed the limit.
        line: usize,
        /// Structural section containing that line.
        section: OemSection,
        /// Resource whose limit was crossed.
        kind: OemLimitKind,
        /// Configured finite limit.
        configured: usize,
        /// Observed value when the error was reported.
        observed: usize,
    },
    /// The KVN source was not UTF-8 text.
    #[error("OEM line {line} is not valid UTF-8")]
    InvalidUtf8 {
        /// Source line.
        line: usize,
    },
    /// A required field was absent.
    #[error("missing required {section} field {field} at OEM line {line}")]
    MissingField {
        /// Source line where the section ended.
        line: usize,
        /// Header or metadata.
        section: &'static str,
        /// Required field name.
        field: &'static str,
    },
    /// A keyword or structural marker was not valid in the current section.
    #[error("unexpected OEM content at line {line} in {section}: {content}")]
    UnexpectedContent {
        /// Source line.
        line: usize,
        /// Parser section.
        section: &'static str,
        /// Offending content.
        content: String,
    },
    /// The OEM version is not supported by this reader.
    #[error("unsupported CCSDS OEM version {value} at line {line}")]
    UnsupportedVersion {
        /// Source line.
        line: usize,
        /// Declared version.
        value: String,
    },
    /// A frame center is not represented by orskit's built-in identities.
    #[error("unsupported OEM CENTER_NAME {value} at line {line}")]
    UnsupportedCenter {
        /// Source line.
        line: usize,
        /// Declared center.
        value: String,
    },
    /// A frame orientation is not represented by orskit's built-in identities.
    #[error("unsupported OEM REF_FRAME {value} at line {line}")]
    UnsupportedFrame {
        /// Source line.
        line: usize,
        /// Declared orientation.
        value: String,
    },
    /// A center cannot be combined with the declared frame orientation.
    #[error("OEM CENTER_NAME {center} is incompatible with REF_FRAME {frame} at line {line}")]
    IncompatibleFrameCenter {
        /// Source line.
        line: usize,
        /// Declared center.
        center: String,
        /// Declared frame.
        frame: String,
    },
    /// The time system needs a model not supplied to this reader.
    #[error("unsupported OEM TIME_SYSTEM {value} at line {line}")]
    UnsupportedTimeSystem {
        /// Source line.
        line: usize,
        /// Declared time system.
        value: String,
    },
    /// An epoch was invalid in its declared time system.
    #[error("invalid {time_system} OEM epoch {value} at line {line}")]
    InvalidEpoch {
        /// Source line.
        line: usize,
        /// Epoch text.
        value: String,
        /// Declared time system.
        time_system: OemTimeSystem,
    },
    /// A scalar state component was invalid.
    #[error("invalid OEM state field {field}={value} at line {line}")]
    InvalidNumber {
        /// Source line.
        line: usize,
        /// Field name.
        field: &'static str,
        /// Field text.
        value: String,
    },
    /// The state record has neither seven nor ten fields.
    #[error("OEM state at line {line} has {actual} fields; expected 7 or 10")]
    InvalidStateFieldCount {
        /// Source line.
        line: usize,
        /// Observed field count.
        actual: usize,
    },
    /// Domain validation rejected a decoded vector.
    #[error("invalid OEM state at line {line}: {source}")]
    InvalidState {
        /// Source line.
        line: usize,
        /// Domain validation error.
        #[source]
        source: KinematicError,
    },
    /// A covariance entry was not finite numeric text.
    #[error("invalid OEM covariance entry {value} at line {line}")]
    InvalidCovarianceEntry {
        /// Source line.
        line: usize,
        /// Source text.
        value: String,
    },
    /// One covariance row did not contain its required lower-triangle entries.
    #[error("OEM covariance row {row} at line {line} has {actual} entries; expected {expected}")]
    InvalidCovarianceRow {
        /// Source line.
        line: usize,
        /// Zero-based covariance row.
        row: usize,
        /// Observed count.
        actual: usize,
        /// Required count.
        expected: usize,
    },
    /// Metadata times are not ordered consistently.
    #[error("OEM metadata times are not ordered at line {line}")]
    InvalidTimeRange {
        /// Source line where metadata ended.
        line: usize,
    },
    /// A state epoch is outside its segment bounds.
    #[error("OEM state epoch at line {line} is outside its segment START_TIME/STOP_TIME")]
    StateOutsideSegment {
        /// Source line.
        line: usize,
    },
    /// State epochs in one segment did not increase strictly in source order.
    #[error(
        "OEM state epoch {current_epoch} at line {current_line} is not later than {previous_epoch} at line {previous_line}"
    )]
    NonIncreasingStateEpoch {
        /// Source line of the preceding state.
        previous_line: usize,
        /// Epoch of the preceding state.
        previous_epoch: Epoch,
        /// Source line of the state that violated ordering.
        current_line: usize,
        /// Epoch of the state that violated ordering.
        current_epoch: Epoch,
    },
    /// A segment contained no ephemeris states.
    #[error("OEM segment ending at line {line} contains no state records")]
    EmptySegment {
        /// Source line where the segment ended.
        line: usize,
    },
    /// The event stream could not form one complete document.
    #[error("invalid OEM event order: {message}")]
    InvalidEventOrder {
        /// Ordering failure.
        message: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoundedLine {
    Line,
    Eof,
    TooLong { observed: usize },
}

fn line_content_bytes(source: &str) -> usize {
    match source.strip_suffix('\n') {
        Some(line) => line.strip_suffix('\r').unwrap_or(line).len(),
        None => source.len(),
    }
}

fn read_bounded_line<R: BufRead>(
    reader: &mut R,
    buffer: &mut Vec<u8>,
    max_bytes: usize,
) -> std::io::Result<BoundedLine> {
    buffer.clear();
    let buffer_limit = max_bytes.saturating_add(2);
    let mut raw_bytes = 0usize;
    let mut last_byte = None;
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return Ok(if raw_bytes == 0 {
                BoundedLine::Eof
            } else if raw_bytes > max_bytes {
                BoundedLine::TooLong {
                    observed: raw_bytes,
                }
            } else {
                BoundedLine::Line
            });
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let take = newline.map_or(available.len(), |index| index + 1);
        let byte_before_newline = newline.and_then(|index| {
            if index == 0 {
                last_byte
            } else {
                Some(available[index - 1])
            }
        });
        let remaining = buffer_limit.saturating_sub(buffer.len());
        buffer.extend_from_slice(&available[..take.min(remaining)]);
        raw_bytes = raw_bytes.saturating_add(take);
        last_byte = Some(available[take - 1]);
        reader.consume(take);
        if newline.is_some() {
            let terminator_bytes = if byte_before_newline == Some(b'\r') {
                2
            } else {
                1
            };
            let observed = raw_bytes.saturating_sub(terminator_bytes);
            return Ok(if observed > max_bytes {
                BoundedLine::TooLong { observed }
            } else {
                BoundedLine::Line
            });
        }
        if raw_bytes > max_bytes.saturating_add(1)
            || (raw_bytes > max_bytes && last_byte != Some(b'\r'))
        {
            return Ok(BoundedLine::TooLong {
                observed: raw_bytes,
            });
        }
    }
}

#[cfg(feature = "async")]
async fn read_bounded_line_async<R: AsyncBufRead + Unpin>(
    reader: &mut R,
    buffer: &mut Vec<u8>,
    max_bytes: usize,
) -> std::io::Result<BoundedLine> {
    buffer.clear();
    let buffer_limit = max_bytes.saturating_add(2);
    let mut raw_bytes = 0usize;
    let mut last_byte = None;
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            return Ok(if raw_bytes == 0 {
                BoundedLine::Eof
            } else if raw_bytes > max_bytes {
                BoundedLine::TooLong {
                    observed: raw_bytes,
                }
            } else {
                BoundedLine::Line
            });
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let take = newline.map_or(available.len(), |index| index + 1);
        let byte_before_newline = newline.and_then(|index| {
            if index == 0 {
                last_byte
            } else {
                Some(available[index - 1])
            }
        });
        let remaining = buffer_limit.saturating_sub(buffer.len());
        buffer.extend_from_slice(&available[..take.min(remaining)]);
        raw_bytes = raw_bytes.saturating_add(take);
        last_byte = Some(available[take - 1]);
        reader.consume(take);
        if newline.is_some() {
            let terminator_bytes = if byte_before_newline == Some(b'\r') {
                2
            } else {
                1
            };
            let observed = raw_bytes.saturating_sub(terminator_bytes);
            return Ok(if observed > max_bytes {
                BoundedLine::TooLong { observed }
            } else {
                BoundedLine::Line
            });
        }
        if raw_bytes > max_bytes.saturating_add(1)
            || (raw_bytes > max_bytes && last_byte != Some(b'\r'))
        {
            return Ok(BoundedLine::TooLong {
                observed: raw_bytes,
            });
        }
    }
}

/// Blocking, bounded-memory OEM KVN event reader.
///
/// The reader reuses one line buffer. Returned events own all data needed after
/// the next iteration.
pub struct OemKvnReader<R> {
    reader: R,
    decoder: Decoder,
    chronology: SegmentChronology,
    buffer: Vec<u8>,
    limits: OemDecoderLimits,
    finished: bool,
}

impl<R: BufRead> OemKvnReader<R> {
    /// Constructs a reader over any blocking buffered source.
    #[must_use]
    pub fn new(reader: R) -> Self {
        Self::with_limits(reader, OemDecoderLimits::default())
    }

    /// Constructs a reader with caller-selected finite decoder limits.
    #[must_use]
    pub fn with_limits(reader: R, limits: OemDecoderLimits) -> Self {
        Self {
            reader,
            decoder: Decoder::new(limits),
            chronology: SegmentChronology::default(),
            buffer: Vec::new(),
            limits,
            finished: false,
        }
    }
}

impl<R: BufRead> Iterator for OemKvnReader<R> {
    type Item = Result<OemEvent, OemError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }

        loop {
            match read_bounded_line(
                &mut self.reader,
                &mut self.buffer,
                self.limits.max_line_bytes,
            ) {
                Ok(BoundedLine::Eof) => {
                    self.finished = true;
                    return match self.decoder.finish() {
                        Ok(Some(event)) => {
                            Some(validate_event_chronology(event, &mut self.chronology))
                        }
                        Ok(None) => None,
                        Err(error) => Some(Err(error)),
                    };
                }
                Ok(BoundedLine::TooLong { observed }) => {
                    self.finished = true;
                    return Some(Err(OemError::ResourceLimitExceeded {
                        line: self.decoder.line + 1,
                        section: self.decoder.section(),
                        kind: OemLimitKind::LineBytes,
                        configured: self.limits.max_line_bytes,
                        observed,
                    }));
                }
                Ok(BoundedLine::Line) => {
                    let Ok(line) = std::str::from_utf8(&self.buffer) else {
                        self.finished = true;
                        return Some(Err(OemError::InvalidUtf8 {
                            line: self.decoder.line + 1,
                        }));
                    };
                    match self.decoder.push_line(line) {
                        Ok(Some(output)) => {
                            let result = decode_output(output, &mut self.chronology);
                            if matches!(&result, Err(OemError::NonIncreasingStateEpoch { .. })) {
                                self.finished = true;
                            }
                            return Some(result);
                        }
                        Ok(None) => {}
                        Err(error) => {
                            self.finished = true;
                            return Some(Err(error));
                        }
                    }
                }
                Err(source) => {
                    self.finished = true;
                    return Some(Err(OemError::Io {
                        line: self.decoder.line,
                        source,
                    }));
                }
            }
        }
    }
}

/// Tokio bounded-memory OEM KVN event reader.
#[cfg(feature = "async")]
pub struct AsyncOemKvnReader<R> {
    reader: R,
    decoder: Decoder,
    chronology: SegmentChronology,
    buffer: Vec<u8>,
    limits: OemDecoderLimits,
    finished: bool,
}

#[cfg(feature = "async")]
impl<R: AsyncBufRead + Unpin> AsyncOemKvnReader<R> {
    /// Constructs a reader over any Tokio buffered source.
    #[must_use]
    pub fn new(reader: R) -> Self {
        Self::with_limits(reader, OemDecoderLimits::default())
    }

    /// Constructs a reader with caller-selected finite decoder limits.
    #[must_use]
    pub fn with_limits(reader: R, limits: OemDecoderLimits) -> Self {
        Self {
            reader,
            decoder: Decoder::new(limits),
            chronology: SegmentChronology::default(),
            buffer: Vec::new(),
            limits,
            finished: false,
        }
    }

    /// Reads and decodes the next event.
    pub async fn next_event(&mut self) -> Option<Result<OemEvent, OemError>> {
        if self.finished {
            return None;
        }

        loop {
            match read_bounded_line_async(
                &mut self.reader,
                &mut self.buffer,
                self.limits.max_line_bytes,
            )
            .await
            {
                Ok(BoundedLine::Eof) => {
                    self.finished = true;
                    return match self.decoder.finish() {
                        Ok(Some(event)) => {
                            Some(validate_event_chronology(event, &mut self.chronology))
                        }
                        Ok(None) => None,
                        Err(error) => Some(Err(error)),
                    };
                }
                Ok(BoundedLine::TooLong { observed }) => {
                    self.finished = true;
                    return Some(Err(OemError::ResourceLimitExceeded {
                        line: self.decoder.line + 1,
                        section: self.decoder.section(),
                        kind: OemLimitKind::LineBytes,
                        configured: self.limits.max_line_bytes,
                        observed,
                    }));
                }
                Ok(BoundedLine::Line) => {
                    let Ok(line) = std::str::from_utf8(&self.buffer) else {
                        self.finished = true;
                        return Some(Err(OemError::InvalidUtf8 {
                            line: self.decoder.line + 1,
                        }));
                    };
                    match self.decoder.push_line(line) {
                        Ok(Some(output)) => {
                            let result = decode_output(output, &mut self.chronology);
                            if matches!(&result, Err(OemError::NonIncreasingStateEpoch { .. })) {
                                self.finished = true;
                            }
                            return Some(result);
                        }
                        Ok(None) => {}
                        Err(error) => {
                            self.finished = true;
                            return Some(Err(error));
                        }
                    }
                }
                Err(source) => {
                    self.finished = true;
                    return Some(Err(OemError::Io {
                        line: self.decoder.line,
                        source,
                    }));
                }
            }
        }
    }
}

/// Parses and collects an OEM KVN document sequentially.
///
/// Use [`OemKvnReader`] when the complete document does not need to be retained.
pub fn parse_oem_kvn(input: &str) -> Result<Oem, OemError> {
    parse_oem_kvn_with_limits(input, OemDecoderLimits::default())
}

/// Parses and collects an OEM KVN document with explicit decoder limits.
pub fn parse_oem_kvn_with_limits(input: &str, limits: OemDecoderLimits) -> Result<Oem, OemError> {
    collect_document(OemKvnReader::with_limits(
        std::io::Cursor::new(input.as_bytes()),
        limits,
    ))
}

/// Parses and collects an in-memory OEM KVN document with ordered Rayon state
/// conversion.
///
/// Structural scanning remains sequential. Only independent state records are
/// converted in parallel, after their segment frame and time system are known.
#[cfg(feature = "parallel")]
pub fn parse_oem_kvn_parallel(input: &str) -> Result<Oem, OemError> {
    parse_oem_kvn_parallel_with_limits(input, OemDecoderLimits::default())
}

/// Parses and collects an in-memory OEM KVN document in parallel with explicit
/// decoder limits.
#[cfg(feature = "parallel")]
pub fn parse_oem_kvn_parallel_with_limits(
    input: &str,
    limits: OemDecoderLimits,
) -> Result<Oem, OemError> {
    let mut decoder = Decoder::new(limits);
    let mut layout = Vec::new();
    let mut states = Vec::new();

    for line in input.lines() {
        if let Some(output) = decoder.push_line(line)? {
            match output {
                DecoderOutput::Event(event) => layout.push(ParallelLayout::Event(event)),
                DecoderOutput::State(raw) => {
                    let index = states.len();
                    states.push(raw);
                    layout.push(ParallelLayout::State(index));
                }
            }
        }
    }
    if let Some(event) = decoder.finish()? {
        layout.push(ParallelLayout::Event(Box::new(event)));
    }

    let parsed: Vec<Result<OemSample, OemError>> = states.par_iter().map(parse_raw_state).collect();
    let mut parsed = parsed.into_iter().map(Some).collect::<Vec<_>>();
    let mut chronology = SegmentChronology::default();
    let events = layout
        .into_iter()
        .map(|item| -> Result<OemEvent, OemError> {
            let event = match item {
                ParallelLayout::Event(event) => *event,
                ParallelLayout::State(index) => {
                    let sample = parsed[index].take().ok_or(OemError::InvalidEventOrder {
                        message: "parallel state layout was consumed more than once",
                    })??;
                    OemEvent::Coordinates(Box::new(sample))
                }
            };
            validate_event_chronology(event, &mut chronology)
        });
    collect_document(events)
}

#[cfg(feature = "parallel")]
enum ParallelLayout {
    Event(Box<OemEvent>),
    State(usize),
}

fn collect_document(
    events: impl IntoIterator<Item = Result<OemEvent, OemError>>,
) -> Result<Oem, OemError> {
    let mut header = None;
    let mut segments = Vec::new();
    let mut active: Option<OemSegment> = None;

    for event in events {
        match event? {
            OemEvent::Header(value) if header.is_none() && active.is_none() => header = Some(value),
            OemEvent::SegmentStart(context) if header.is_some() && active.is_none() => {
                let comments = context.metadata().comments().to_vec();
                let record_order = (0..comments.len()).map(OemRecordIndex::Comment).collect();
                active = Some(OemSegment {
                    context,
                    coordinates: Vec::new(),
                    covariances: Vec::new(),
                    comments,
                    record_order,
                });
            }
            OemEvent::Comment(comment) => {
                let segment = active.as_mut().ok_or(OemError::InvalidEventOrder {
                    message: "comment outside a segment",
                })?;
                if comment.segment_id() != Some(segment.id()) {
                    return Err(OemError::InvalidEventOrder {
                        message: "comment segment identifier does not match active segment",
                    });
                }
                let index = segment.comments.len();
                segment.comments.push(comment);
                segment.record_order.push(OemRecordIndex::Comment(index));
            }
            OemEvent::Coordinates(coordinates) => {
                let segment = active.as_mut().ok_or(OemError::InvalidEventOrder {
                    message: "state outside a segment",
                })?;
                if coordinates.segment_id() != segment.id() {
                    return Err(OemError::InvalidEventOrder {
                        message: "state segment identifier does not match active segment",
                    });
                }
                let index = segment.coordinates.len();
                segment.coordinates.push(*coordinates);
                segment
                    .record_order
                    .push(OemRecordIndex::Coordinates(index));
            }
            OemEvent::Covariance(covariance) => {
                let segment = active.as_mut().ok_or(OemError::InvalidEventOrder {
                    message: "covariance outside a segment",
                })?;
                if covariance.segment_id() != segment.id() {
                    return Err(OemError::InvalidEventOrder {
                        message: "covariance segment identifier does not match active segment",
                    });
                }
                let index = segment.covariances.len();
                segment.covariances.push(*covariance);
                segment.record_order.push(OemRecordIndex::Covariance(index));
            }
            OemEvent::SegmentEnd(id) => {
                let segment = active.take().ok_or(OemError::InvalidEventOrder {
                    message: "segment end without segment start",
                })?;
                if id != segment.id() {
                    return Err(OemError::InvalidEventOrder {
                        message: "segment end identifier does not match active segment",
                    });
                }
                segments.push(segment);
            }
            _ => {
                return Err(OemError::InvalidEventOrder {
                    message: "header or segment marker out of order",
                });
            }
        }
    }

    if active.is_some() {
        return Err(OemError::InvalidEventOrder {
            message: "unterminated segment",
        });
    }
    Ok(Oem {
        header: header.ok_or(OemError::InvalidEventOrder {
            message: "missing header event",
        })?,
        segments,
    })
}

#[derive(Default)]
struct SegmentChronology {
    previous: Option<(usize, Epoch)>,
}

impl SegmentChronology {
    const fn reset(&mut self) {
        self.previous = None;
    }

    fn observe(&mut self, line: usize, epoch: Epoch) -> Result<(), OemError> {
        if let Some((previous_line, previous_epoch)) = self.previous {
            if epoch <= previous_epoch {
                return Err(OemError::NonIncreasingStateEpoch {
                    previous_line,
                    previous_epoch,
                    current_line: line,
                    current_epoch: epoch,
                });
            }
        }
        self.previous = Some((line, epoch));
        Ok(())
    }
}

fn validate_event_chronology(
    event: OemEvent,
    chronology: &mut SegmentChronology,
) -> Result<OemEvent, OemError> {
    match &event {
        OemEvent::Header(_) | OemEvent::SegmentStart(_) | OemEvent::SegmentEnd(_) => {
            chronology.reset();
        }
        OemEvent::Comment(_) | OemEvent::Covariance(_) => {}
        OemEvent::Coordinates(sample) => {
            chronology.observe(sample.source_line(), sample.epoch())?;
        }
    }
    Ok(event)
}

fn decode_output(
    output: DecoderOutput<'_>,
    chronology: &mut SegmentChronology,
) -> Result<OemEvent, OemError> {
    let event = match output {
        DecoderOutput::Event(event) => *event,
        DecoderOutput::State(raw) => OemEvent::Coordinates(Box::new(parse_raw_state(&raw)?)),
    };
    validate_event_chronology(event, chronology)
}

struct Decoder {
    line: usize,
    phase: Phase,
    limits: OemDecoderLimits,
    section_bytes: usize,
    section_lines: usize,
    document_bytes: usize,
    document_lines: usize,
    header: HeaderBuilder,
    metadata: MetadataBuilder,
    next_segment_index: usize,
    current_segment_id: Option<OemSegmentId>,
    current_context: Option<OemSegmentContext>,
    current_state_count: usize,
    covariance: Option<CovarianceBuilder>,
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new(OemDecoderLimits::default())
    }
}

#[derive(Debug, Clone, Copy, Default)]
enum Phase {
    #[default]
    Header,
    Metadata,
    Data,
    Covariance,
    Done,
}

enum DecoderOutput<'a> {
    Event(Box<OemEvent>),
    State(RawState<'a>),
}

#[derive(Clone)]
struct RawState<'a> {
    text: &'a str,
    line: usize,
    context: OemSegmentContext,
}

impl Decoder {
    fn new(limits: OemDecoderLimits) -> Self {
        Self {
            line: 0,
            phase: Phase::Header,
            limits,
            section_bytes: 0,
            section_lines: 0,
            document_bytes: 0,
            document_lines: 0,
            header: HeaderBuilder::default(),
            metadata: MetadataBuilder::default(),
            next_segment_index: 0,
            current_segment_id: None,
            current_context: None,
            current_state_count: 0,
            covariance: None,
        }
    }

    fn begin_segment(&mut self) -> Result<OemSegmentId, OemError> {
        let index = self.next_segment_index;
        self.next_segment_index = index.checked_add(1).ok_or(OemError::InvalidEventOrder {
            message: "OEM segment identifier space exhausted",
        })?;
        let id = OemSegmentId(index);
        self.current_segment_id = Some(id);
        Ok(id)
    }

    const fn section(&self) -> OemSection {
        match self.phase {
            Phase::Header => OemSection::Header,
            Phase::Metadata => OemSection::Metadata,
            Phase::Data | Phase::Covariance | Phase::Done => OemSection::Data,
        }
    }

    fn account_line(&mut self, source: &str) -> Result<(), OemError> {
        let section = self.section();
        let bytes = line_content_bytes(source);
        if bytes > self.limits.max_line_bytes {
            return Err(OemError::ResourceLimitExceeded {
                line: self.line,
                section,
                kind: OemLimitKind::LineBytes,
                configured: self.limits.max_line_bytes,
                observed: bytes,
            });
        }

        self.section_bytes = self.section_bytes.saturating_add(bytes);
        self.section_lines = self.section_lines.saturating_add(1);
        self.document_bytes = self.document_bytes.saturating_add(bytes);
        self.document_lines = self.document_lines.saturating_add(1);
        if self.section_bytes > self.limits.max_section_bytes {
            return Err(OemError::ResourceLimitExceeded {
                line: self.line,
                section,
                kind: OemLimitKind::SectionBytes,
                configured: self.limits.max_section_bytes,
                observed: self.section_bytes,
            });
        }
        if self.section_lines > self.limits.max_section_lines {
            return Err(OemError::ResourceLimitExceeded {
                line: self.line,
                section,
                kind: OemLimitKind::SectionLines,
                configured: self.limits.max_section_lines,
                observed: self.section_lines,
            });
        }
        if self.document_bytes > self.limits.max_document_bytes {
            return Err(OemError::ResourceLimitExceeded {
                line: self.line,
                section,
                kind: OemLimitKind::DocumentBytes,
                configured: self.limits.max_document_bytes,
                observed: self.document_bytes,
            });
        }
        if self.document_lines > self.limits.max_document_lines {
            return Err(OemError::ResourceLimitExceeded {
                line: self.line,
                section,
                kind: OemLimitKind::DocumentLines,
                configured: self.limits.max_document_lines,
                observed: self.document_lines,
            });
        }
        Ok(())
    }

    const fn reset_section_counters(&mut self) {
        self.section_bytes = 0;
        self.section_lines = 0;
    }

    fn push_line<'a>(&mut self, source: &'a str) -> Result<Option<DecoderOutput<'a>>, OemError> {
        self.line += 1;
        self.account_line(source)?;
        let line = source.trim();
        if line.is_empty() {
            return Ok(None);
        }

        match self.phase {
            Phase::Header => {
                if line == "META_START" {
                    let header = std::mem::take(&mut self.header).finish(self.line)?;
                    self.begin_segment()?;
                    self.phase = Phase::Metadata;
                    self.reset_section_counters();
                    Ok(Some(DecoderOutput::Event(Box::new(OemEvent::Header(
                        header,
                    )))))
                } else {
                    self.header.push(line, self.line)?;
                    Ok(None)
                }
            }
            Phase::Metadata => {
                if line == "META_STOP" {
                    let metadata = std::mem::take(&mut self.metadata).finish(self.line)?;
                    let id = self.current_segment_id.ok_or(OemError::InvalidEventOrder {
                        message: "metadata completed without an active segment identifier",
                    })?;
                    let context = OemSegmentContext {
                        id,
                        metadata: Arc::new(metadata),
                    };
                    self.current_context = Some(context.clone());
                    self.current_state_count = 0;
                    self.phase = Phase::Data;
                    self.reset_section_counters();
                    Ok(Some(DecoderOutput::Event(Box::new(
                        OemEvent::SegmentStart(context),
                    ))))
                } else {
                    let id = self.current_segment_id.ok_or(OemError::InvalidEventOrder {
                        message: "metadata encountered without an active segment identifier",
                    })?;
                    self.metadata.push(line, self.line, id)?;
                    Ok(None)
                }
            }
            Phase::Data => {
                if line == "META_START" {
                    if self.current_state_count == 0 {
                        return Err(OemError::EmptySegment { line: self.line });
                    }
                    let id = self
                        .current_context
                        .take()
                        .ok_or(OemError::InvalidEventOrder {
                            message: "segment ended without immutable segment context",
                        })?
                        .id();
                    self.current_segment_id = None;
                    self.begin_segment()?;
                    self.phase = Phase::Metadata;
                    self.reset_section_counters();
                    return Ok(Some(DecoderOutput::Event(Box::new(OemEvent::SegmentEnd(
                        id,
                    )))));
                }
                if line == "COVARIANCE_START" {
                    self.phase = Phase::Covariance;
                    self.covariance = None;
                    return Ok(None);
                }
                if let Some(comment) = comment_value(line) {
                    let id = self.current_segment_id.ok_or(OemError::InvalidEventOrder {
                        message: "data comment encountered without an active segment identifier",
                    })?;
                    return Ok(Some(DecoderOutput::Event(Box::new(OemEvent::Comment(
                        OemComment {
                            segment_id: Some(id),
                            section: OemSection::Data,
                            source_line: self.line,
                            text: comment.to_owned(),
                        },
                    )))));
                }
                if line.contains('=') || line.ends_with("_STOP") {
                    return Err(OemError::UnexpectedContent {
                        line: self.line,
                        section: "data",
                        content: line.to_owned(),
                    });
                }
                let context =
                    self.current_context
                        .as_ref()
                        .ok_or_else(|| OemError::UnexpectedContent {
                            line: self.line,
                            section: "data",
                            content: "missing active segment metadata".to_owned(),
                        })?;
                self.current_state_count += 1;
                Ok(Some(DecoderOutput::State(RawState {
                    text: line,
                    line: self.line,
                    context: context.clone(),
                })))
            }
            Phase::Covariance => {
                if line == "COVARIANCE_STOP" {
                    let covariance = self
                        .covariance
                        .take()
                        .ok_or(OemError::MissingField {
                            line: self.line,
                            section: "covariance",
                            field: "EPOCH",
                        })?
                        .finish()?;
                    self.phase = Phase::Data;
                    let event = OemEvent::Covariance(Box::new(covariance));
                    return Ok(Some(DecoderOutput::Event(Box::new(event))));
                }
                if let Some((key, value)) = line.split_once('=') {
                    let key = key.trim();
                    let value = value.trim();
                    if key == "EPOCH" {
                        let previous = self
                            .covariance
                            .take()
                            .map(CovarianceBuilder::finish)
                            .transpose()?;
                        let context =
                            self.current_context
                                .as_ref()
                                .ok_or(OemError::InvalidEventOrder {
                                    message: "covariance encountered without segment context",
                                })?;
                        let epoch =
                            parse_epoch(value, context.metadata().time_system(), self.line)?;
                        self.covariance =
                            Some(CovarianceBuilder::new(context.clone(), self.line, epoch));
                        return Ok(previous.map(|covariance| {
                            DecoderOutput::Event(Box::new(OemEvent::Covariance(Box::new(
                                covariance,
                            ))))
                        }));
                    }
                    if key == "COV_REF_FRAME" {
                        let builder = self.covariance.as_mut().ok_or_else(|| {
                            OemError::UnexpectedContent {
                                line: self.line,
                                section: "covariance",
                                content: line.to_owned(),
                            }
                        })?;
                        builder.set_frame(value, self.line)?;
                        return Ok(None);
                    }
                }
                let builder =
                    self.covariance
                        .as_mut()
                        .ok_or_else(|| OemError::UnexpectedContent {
                            line: self.line,
                            section: "covariance",
                            content: line.to_owned(),
                        })?;
                builder.push_row(line, self.line)?;
                Ok(None)
            }
            Phase::Done => Err(OemError::UnexpectedContent {
                line: self.line,
                section: "end of message",
                content: line.to_owned(),
            }),
        }
    }

    fn finish(&mut self) -> Result<Option<OemEvent>, OemError> {
        match self.phase {
            Phase::Data => {
                if self.current_state_count == 0 {
                    return Err(OemError::EmptySegment { line: self.line });
                }
                self.phase = Phase::Done;
                let id = self
                    .current_context
                    .take()
                    .ok_or(OemError::InvalidEventOrder {
                        message: "segment ended without immutable segment context",
                    })?
                    .id();
                self.current_segment_id = None;
                Ok(Some(OemEvent::SegmentEnd(id)))
            }
            Phase::Covariance => Err(OemError::UnexpectedContent {
                line: self.line,
                section: "covariance",
                content: "end of input before COVARIANCE_STOP".to_owned(),
            }),
            Phase::Done => Ok(None),
            Phase::Header => Err(OemError::UnexpectedContent {
                line: self.line,
                section: "header",
                content: "end of input before META_START".to_owned(),
            }),
            Phase::Metadata => Err(OemError::UnexpectedContent {
                line: self.line,
                section: "metadata",
                content: "end of input before META_STOP".to_owned(),
            }),
        }
    }
}

struct CovarianceBuilder {
    context: OemSegmentContext,
    source_line: usize,
    epoch: Epoch,
    frame: Option<OemCovarianceFrame>,
    rows: Vec<Vec<f64>>,
}

impl CovarianceBuilder {
    const fn new(context: OemSegmentContext, source_line: usize, epoch: Epoch) -> Self {
        Self {
            context,
            source_line,
            epoch,
            frame: None,
            rows: Vec::new(),
        }
    }

    fn set_frame(&mut self, value: &str, line: usize) -> Result<(), OemError> {
        if self.frame.is_some() {
            return Err(OemError::UnexpectedContent {
                line,
                section: "covariance",
                content: "duplicate COV_REF_FRAME".to_owned(),
            });
        }
        self.frame = Some(parse_covariance_frame(value, self.context.metadata()));
        Ok(())
    }

    fn push_row(&mut self, line: &str, source_line: usize) -> Result<(), OemError> {
        let row = self.rows.len();
        if row == 6 {
            return Err(OemError::UnexpectedContent {
                line: source_line,
                section: "covariance",
                content: line.to_owned(),
            });
        }
        let values = line
            .split_ascii_whitespace()
            .map(|value| {
                value
                    .parse::<f64>()
                    .ok()
                    .filter(|value| value.is_finite())
                    .ok_or_else(|| OemError::InvalidCovarianceEntry {
                        line: source_line,
                        value: value.to_owned(),
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let expected = row + 1;
        if values.len() != expected {
            return Err(OemError::InvalidCovarianceRow {
                line: source_line,
                row,
                actual: values.len(),
                expected,
            });
        }
        self.rows.push(values);
        Ok(())
    }

    fn finish(self) -> Result<OemCartesianCovariance, OemError> {
        if self.rows.len() != 6 {
            return Err(OemError::InvalidCovarianceRow {
                line: self.source_line,
                row: self.rows.len(),
                actual: self.rows.len(),
                expected: 6,
            });
        }
        let frame = self.frame.ok_or(OemError::MissingField {
            line: self.source_line,
            section: "covariance",
            field: "COV_REF_FRAME",
        })?;
        let mut matrix = std::array::from_fn(|row| {
            std::array::from_fn(|column| cartesian_covariance_entry(row, column, 0.0))
        });
        for (row, values) in self.rows.iter().enumerate() {
            let (earlier_rows, current_and_later) = matrix.split_at_mut(row);
            let current_row = &mut current_and_later[0];
            for (column, value) in values.iter().copied().enumerate() {
                let entry = cartesian_covariance_entry(row, column, value);
                current_row[column] = entry;
                if column != row {
                    earlier_rows[column][row] = entry;
                }
            }
        }
        Ok(OemCartesianCovariance {
            context: self.context,
            source_line: self.source_line,
            epoch: self.epoch,
            frame,
            matrix,
        })
    }
}

fn parse_covariance_frame(value: &str, metadata: &OemMetadata) -> OemCovarianceFrame {
    if value.eq_ignore_ascii_case("RTN") {
        return OemCovarianceFrame::new(RtnCovarianceAxes);
    }
    match value.parse::<FrameOrientation>() {
        Ok(orientation) => OemCovarianceFrame::new(ReferenceCovarianceAxes::new(
            ReferenceFrame::new(metadata.frame().origin(), orientation),
        )),
        Err(_) => OemCovarianceFrame::new(DeclaredCovarianceAxes::new(value)),
    }
}

fn cartesian_covariance_entry(
    row: usize,
    column: usize,
    value_in_square_kilometres: f64,
) -> CartesianCovarianceEntry {
    match (row < 3, column < 3) {
        (true, true) => CartesianCovarianceEntry::Position(Area::new::<square_kilometer>(
            value_in_square_kilometres,
        )),
        (false, false) => CartesianCovarianceEntry::Velocity(
            VelocityVariance::from_square_metres_per_square_second(
                value_in_square_kilometres * 1_000_000.0,
            ),
        ),
        _ => CartesianCovarianceEntry::PositionVelocity(
            PositionVelocityCovariance::from_square_metres_per_second(
                value_in_square_kilometres * 1_000_000.0,
            ),
        ),
    }
}

#[derive(Default)]
struct HeaderBuilder {
    version: Option<String>,
    creation_date: Option<String>,
    originator: Option<String>,
    message_id: Option<String>,
    comments: Vec<OemComment>,
}

impl HeaderBuilder {
    fn push(&mut self, line: &str, number: usize) -> Result<(), OemError> {
        if let Some(comment) = comment_value(line) {
            self.comments.push(OemComment {
                segment_id: None,
                section: OemSection::Header,
                source_line: number,
                text: comment.to_owned(),
            });
            return Ok(());
        }
        let (key, value) = assignment(line, number, "header")?;
        match key {
            "CCSDS_OEM_VERS" => set_once(&mut self.version, value, key, number, "header"),
            "CREATION_DATE" => set_once(&mut self.creation_date, value, key, number, "header"),
            "ORIGINATOR" => set_once(&mut self.originator, value, key, number, "header"),
            "MESSAGE_ID" => set_once(&mut self.message_id, value, key, number, "header"),
            _ => Err(OemError::UnexpectedContent {
                line: number,
                section: "header",
                content: line.to_owned(),
            }),
        }
    }

    fn finish(self, line: usize) -> Result<OemHeader, OemError> {
        let version = required(self.version, line, "header", "CCSDS_OEM_VERS")?;
        if !matches!(version.as_str(), "1.0" | "2.0" | "3.0") {
            return Err(OemError::UnsupportedVersion {
                line,
                value: version,
            });
        }
        let creation_text = required(self.creation_date, line, "header", "CREATION_DATE")?;
        let creation_date = parse_epoch(&creation_text, OemTimeSystem::Utc, line)?;
        Ok(OemHeader {
            version,
            creation_date,
            originator: required(self.originator, line, "header", "ORIGINATOR")?,
            message_id: self.message_id,
            comments: self.comments,
        })
    }
}

#[derive(Default)]
struct MetadataBuilder {
    object_name: Option<String>,
    object_id: Option<String>,
    center_name: Option<String>,
    ref_frame: Option<String>,
    time_system: Option<String>,
    start_time: Option<String>,
    usable_start_time: Option<String>,
    usable_stop_time: Option<String>,
    stop_time: Option<String>,
    interpolation: Option<String>,
    interpolation_degree: Option<String>,
    comments: Vec<OemComment>,
}

impl MetadataBuilder {
    fn push(
        &mut self,
        line: &str,
        number: usize,
        segment_id: OemSegmentId,
    ) -> Result<(), OemError> {
        if let Some(comment) = comment_value(line) {
            self.comments.push(OemComment {
                segment_id: Some(segment_id),
                section: OemSection::Metadata,
                source_line: number,
                text: comment.to_owned(),
            });
            return Ok(());
        }
        let (key, value) = assignment(line, number, "metadata")?;
        let slot = match key {
            "OBJECT_NAME" => &mut self.object_name,
            "OBJECT_ID" => &mut self.object_id,
            "CENTER_NAME" => &mut self.center_name,
            "REF_FRAME" => &mut self.ref_frame,
            "TIME_SYSTEM" => &mut self.time_system,
            "START_TIME" => &mut self.start_time,
            "USEABLE_START_TIME" | "USABLE_START_TIME" => &mut self.usable_start_time,
            "USEABLE_STOP_TIME" | "USABLE_STOP_TIME" => &mut self.usable_stop_time,
            "STOP_TIME" => &mut self.stop_time,
            "INTERPOLATION" => &mut self.interpolation,
            "INTERPOLATION_DEGREE" => &mut self.interpolation_degree,
            _ => {
                return Err(OemError::UnexpectedContent {
                    line: number,
                    section: "metadata",
                    content: line.to_owned(),
                });
            }
        };
        set_once(slot, value, key, number, "metadata")
    }

    fn finish(self, line: usize) -> Result<OemMetadata, OemError> {
        let center_name = required(self.center_name, line, "metadata", "CENTER_NAME")?;
        let origin =
            center_name
                .parse::<FrameOrigin>()
                .map_err(|_| OemError::UnsupportedCenter {
                    line,
                    value: center_name.clone(),
                })?;
        let ref_frame = required(self.ref_frame, line, "metadata", "REF_FRAME")?;
        let orientation =
            ref_frame
                .parse::<FrameOrientation>()
                .map_err(|_| OemError::UnsupportedFrame {
                    line,
                    value: ref_frame.clone(),
                })?;
        let earth_only = matches!(
            orientation,
            FrameOrientation::Gcrf
                | FrameOrientation::Itrf(_)
                | FrameOrientation::Teme
                | FrameOrientation::Mod
                | FrameOrientation::Tod
                | FrameOrientation::Gtod
        );
        if earth_only && origin != FrameOrigin::Body(Body::EARTH) {
            return Err(OemError::IncompatibleFrameCenter {
                line,
                center: center_name,
                frame: ref_frame,
            });
        }
        let time_text = required(self.time_system, line, "metadata", "TIME_SYSTEM")?;
        let time_system =
            time_text
                .parse::<OemTimeSystem>()
                .map_err(|()| OemError::UnsupportedTimeSystem {
                    line,
                    value: time_text,
                })?;

        let start_time = parse_epoch(
            &required(self.start_time, line, "metadata", "START_TIME")?,
            time_system,
            line,
        )?;
        let stop_time = parse_epoch(
            &required(self.stop_time, line, "metadata", "STOP_TIME")?,
            time_system,
            line,
        )?;
        let usable_start_time = self
            .usable_start_time
            .map(|value| parse_epoch(&value, time_system, line))
            .transpose()?;
        let usable_stop_time = self
            .usable_stop_time
            .map(|value| parse_epoch(&value, time_system, line))
            .transpose()?;
        if start_time > stop_time
            || usable_start_time.is_some_and(|value| value < start_time || value > stop_time)
            || usable_stop_time.is_some_and(|value| value < start_time || value > stop_time)
            || matches!((usable_start_time, usable_stop_time), (Some(start), Some(stop)) if start > stop)
        {
            return Err(OemError::InvalidTimeRange { line });
        }
        let interpolation_degree = self
            .interpolation_degree
            .map(|value| {
                value.parse::<u8>().map_err(|_| OemError::InvalidNumber {
                    line,
                    field: "INTERPOLATION_DEGREE",
                    value,
                })
            })
            .transpose()?;

        Ok(OemMetadata {
            object_name: required(self.object_name, line, "metadata", "OBJECT_NAME")?,
            object_id: required(self.object_id, line, "metadata", "OBJECT_ID")?,
            frame: ReferenceFrame::new(origin, orientation),
            time_system,
            start_time,
            usable_start_time,
            usable_stop_time,
            stop_time,
            interpolation: self.interpolation,
            interpolation_degree,
            comments: self.comments,
        })
    }
}

fn parse_raw_state(raw: &RawState<'_>) -> Result<OemSample, OemError> {
    let metadata = raw.context.metadata();
    let sample = parse_state_line(
        raw.text,
        raw.line,
        metadata.frame,
        metadata.time_system,
        metadata.start_time,
        metadata.stop_time,
    )?;
    Ok(OemSample {
        context: raw.context.clone(),
        source_line: raw.line,
        sample,
    })
}

fn parse_state_line(
    line: &str,
    number: usize,
    frame: ReferenceFrame,
    time_system: OemTimeSystem,
    start_time: Epoch,
    stop_time: Epoch,
) -> Result<CoordinateSample<CartesianCoordinates>, OemError> {
    let mut fields = line.split_ascii_whitespace();
    let epoch_text = fields.next().ok_or(OemError::InvalidStateFieldCount {
        line: number,
        actual: 0,
    })?;
    let epoch = parse_epoch(epoch_text, time_system, number)?;
    if epoch < start_time || epoch > stop_time {
        return Err(OemError::StateOutsideSegment { line: number });
    }

    let names = [
        "X", "Y", "Z", "X_DOT", "Y_DOT", "Z_DOT", "X_DDOT", "Y_DDOT", "Z_DDOT",
    ];
    let mut values = [0.0; 9];
    let mut value_count = 0;
    for value in fields {
        if value_count == values.len() {
            return Err(OemError::InvalidStateFieldCount {
                line: number,
                actual: value_count + 2,
            });
        }
        values[value_count] = value
            .parse::<f64>()
            .ok()
            .filter(|parsed| parsed.is_finite())
            .ok_or_else(|| OemError::InvalidNumber {
                line: number,
                field: names[value_count],
                value: value.to_owned(),
            })?;
        value_count += 1;
    }
    if value_count != 6 && value_count != 9 {
        return Err(OemError::InvalidStateFieldCount {
            line: number,
            actual: value_count + 1,
        });
    }

    let position = FramedPosition::new(
        Position::from_metres(
            values[0] * 1_000.0,
            values[1] * 1_000.0,
            values[2] * 1_000.0,
        ),
        frame,
    )
    .map_err(|source| OemError::InvalidState {
        line: number,
        source,
    })?;
    let velocity = FramedVelocity::new(
        VelocityVector::from_metres_per_second(
            values[3] * 1_000.0,
            values[4] * 1_000.0,
            values[5] * 1_000.0,
        ),
        frame,
    )
    .map_err(|source| OemError::InvalidState {
        line: number,
        source,
    })?;
    let mut coordinates = CartesianCoordinates::new(position, velocity);
    if value_count == 9 {
        let acceleration = FramedAcceleration::new(
            AccelerationVector::from_metres_per_second_squared(
                values[6] * 1_000.0,
                values[7] * 1_000.0,
                values[8] * 1_000.0,
            ),
            frame,
        )
        .map_err(|source| OemError::InvalidState {
            line: number,
            source,
        })?;
        coordinates = coordinates.with_acceleration(acceleration);
    }
    Ok(CoordinateSample::new(epoch, coordinates))
}

fn parse_epoch(value: &str, time_system: OemTimeSystem, line: usize) -> Result<Epoch, OemError> {
    Epoch::from_str(&format!(
        "{} {}",
        value.trim(),
        time_system.hifitime_suffix()
    ))
    .map_err(|_| OemError::InvalidEpoch {
        line,
        value: value.to_owned(),
        time_system,
    })
}

fn assignment<'a>(
    line: &'a str,
    number: usize,
    section: &'static str,
) -> Result<(&'a str, &'a str), OemError> {
    let (key, value) = line
        .split_once('=')
        .ok_or_else(|| OemError::UnexpectedContent {
            line: number,
            section,
            content: line.to_owned(),
        })?;
    let key = key.trim();
    let value = value.trim();
    if key.is_empty() || value.is_empty() {
        return Err(OemError::UnexpectedContent {
            line: number,
            section,
            content: line.to_owned(),
        });
    }
    Ok((key, value))
}

fn comment_value(line: &str) -> Option<&str> {
    let tail = line.strip_prefix("COMMENT")?;
    if tail.is_empty() {
        return Some("");
    }
    tail.strip_prefix('=')
        .or_else(|| tail.strip_prefix(char::is_whitespace))
        .map(str::trim)
}

fn set_once(
    slot: &mut Option<String>,
    value: &str,
    key: &str,
    line: usize,
    section: &'static str,
) -> Result<(), OemError> {
    if slot.is_some() {
        return Err(OemError::UnexpectedContent {
            line,
            section,
            content: format!("duplicate {key}"),
        });
    }
    *slot = Some(value.to_owned());
    Ok(())
}

fn required(
    value: Option<String>,
    line: usize,
    section: &'static str,
    field: &'static str,
) -> Result<String, OemError> {
    value.ok_or(OemError::MissingField {
        line,
        section,
        field,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use frames::{CustomFrameId, FrameMotion, FrameOrientation};
    use orbits::cartesian::CartesianState;
    use orskit_core::{
        AttitudeState, BodyAngularVelocity, InertiaTensor, Orbit, Orientation, Spacecraft,
        SpacecraftBodyFrame, SpacecraftShape, SpacecraftView,
    };
    use units::uom::si::{mass::kilogram, moment_of_inertia::kilogram_square_meter};
    use units::{AngularVelocityVector, Mass, MomentOfInertia};

    const SAMPLE: &str = "CCSDS_OEM_VERS = 3.0\n\
CREATION_DATE = 2024-01-01T00:00:00\n\
ORIGINATOR = ORSKIT\n\
MESSAGE_ID = TEST-1\n\
META_START\n\
OBJECT_NAME = TEST SAT\n\
OBJECT_ID = 2024-001A\n\
CENTER_NAME = EARTH\n\
REF_FRAME = EME2000\n\
TIME_SYSTEM = UTC\n\
START_TIME = 2024-01-01T00:00:00\n\
STOP_TIME = 2024-01-01T00:10:00\n\
INTERPOLATION = LAGRANGE\n\
INTERPOLATION_DEGREE = 7\n\
META_STOP\n\
2024-01-01T00:00:00 7000 0 0 0 7.5 0\n\
2024-01-01T00:01:00 6990 450 0 -0.5 7.48 0 0.001 0 0\n\
META_START\n\
OBJECT_NAME = MARS TEST\n\
OBJECT_ID = MARS-1\n\
CENTER_NAME = MARS\n\
REF_FRAME = ICRF\n\
TIME_SYSTEM = TAI\n\
START_TIME = 2024-01-01T00:00:00\n\
STOP_TIME = 2024-01-01T00:01:00\n\
META_STOP\n\
2024-01-01T00:00:00 1 2 3 4 5 6\n";

    const ISSUE_839_COVARIANCE: &str =
        include_str!("../testdata/orekit_oem_issue839_covariance.oem");

    fn limits(
        max_line_bytes: usize,
        max_section_bytes: usize,
        max_section_lines: usize,
    ) -> OemDecoderLimits {
        OemDecoderLimits::new(
            max_line_bytes,
            max_section_bytes,
            max_section_lines,
            DEFAULT_MAX_DOCUMENT_BYTES,
            DEFAULT_MAX_DOCUMENT_LINES,
        )
        .expect("test decoder limits are non-zero")
    }

    fn limits_with_document(
        max_document_bytes: usize,
        max_document_lines: usize,
    ) -> OemDecoderLimits {
        OemDecoderLimits::new(
            DEFAULT_MAX_LINE_BYTES,
            DEFAULT_MAX_SECTION_BYTES,
            DEFAULT_MAX_SECTION_LINES,
            max_document_bytes,
            max_document_lines,
        )
        .expect("test decoder limits are non-zero")
    }

    fn maximum_section_totals(input: &str) -> (usize, usize) {
        let mut phase = Phase::Header;
        let mut bytes = 0usize;
        let mut lines = 0usize;
        let mut maximum_bytes = 0usize;
        let mut maximum_lines = 0usize;

        for line in input.lines() {
            bytes += line.len();
            lines += 1;
            let boundary = match phase {
                Phase::Header if line.trim() == "META_START" => {
                    phase = Phase::Metadata;
                    true
                }
                Phase::Metadata if line.trim() == "META_STOP" => {
                    phase = Phase::Data;
                    true
                }
                Phase::Data if line.trim() == "META_START" => {
                    phase = Phase::Metadata;
                    true
                }
                _ => false,
            };
            if boundary {
                maximum_bytes = maximum_bytes.max(bytes);
                maximum_lines = maximum_lines.max(lines);
                bytes = 0;
                lines = 0;
            }
        }
        (maximum_bytes.max(bytes), maximum_lines.max(lines))
    }

    fn duplicate_epoch_input() -> String {
        SAMPLE.replacen("2024-01-01T00:01:00 6990", "2024-01-01T00:00:00 6990", 1)
    }

    fn commented_input() -> String {
        SAMPLE
            .replacen(
                "ORIGINATOR = ORSKIT\n",
                "ORIGINATOR = ORSKIT\nCOMMENT header provenance\n",
                1,
            )
            .replacen(
                "OBJECT_NAME = TEST SAT\n",
                "COMMENT metadata provenance\nOBJECT_NAME = TEST SAT\n",
                1,
            )
            .replacen(
                "2024-01-01T00:00:00 7000",
                "COMMENT before first state\n2024-01-01T00:00:00 7000",
                1,
            )
            .replacen(
                "2024-01-01T00:01:00 6990",
                "COMMENT between states\n2024-01-01T00:01:00 6990",
                1,
            )
    }

    fn chronology_signature(error: OemError) -> (usize, Epoch, usize, Epoch) {
        match error {
            OemError::NonIncreasingStateEpoch {
                previous_line,
                previous_epoch,
                current_line,
                current_epoch,
            } => (previous_line, previous_epoch, current_line, current_epoch),
            other => panic!("expected chronology error, received {other}"),
        }
    }

    #[test]
    fn parses_multiple_segments_into_typed_coordinates() {
        let message = parse_oem_kvn(SAMPLE).expect("valid CCSDS OEM KVN");

        assert_eq!(message.header().version(), "3.0");
        assert_eq!(message.segments().len(), 2);
        let first = &message.segments()[0];
        assert_eq!(first.metadata().frame(), ReferenceFrame::EME2000);
        assert_eq!(first.coordinates().len(), 2);
        assert_eq!(
            first.coordinates()[0]
                .coordinates()
                .position()
                .value()
                .to_metres(),
            [7_000_000.0, 0.0, 0.0]
        );
        assert_eq!(
            first.coordinates()[0]
                .coordinates()
                .velocity()
                .value()
                .to_metres_per_second(),
            [0.0, 7_500.0, 0.0]
        );
        assert_eq!(
            first.coordinates()[1]
                .coordinates()
                .acceleration()
                .expect("fixture has acceleration")
                .value()
                .to_metres_per_second_squared(),
            [1.0, 0.0, 0.0]
        );
        assert_eq!(
            message.segments()[1].metadata().frame(),
            ReferenceFrame::new(FrameOrigin::Body(Body::MARS), FrameOrientation::Icrf)
        );
    }

    #[test]
    fn collected_document_preserves_comment_and_sample_provenance() {
        let input = commented_input();
        let message = parse_oem_kvn(&input).expect("commented OEM is valid");

        let header_comment = message
            .header()
            .comments()
            .first()
            .expect("header comment is retained");
        assert_eq!(header_comment.segment_id(), None);
        assert_eq!(header_comment.section(), OemSection::Header);
        assert_eq!(header_comment.source_line(), 4);
        assert_eq!(header_comment.text(), "header provenance");

        let first = &message.segments()[0];
        let second = &message.segments()[1];
        assert_eq!(first.id().index(), 0);
        assert_eq!(second.id().index(), 1);

        let metadata_comment = first
            .metadata()
            .comments()
            .first()
            .expect("metadata comment is retained");
        assert_eq!(metadata_comment.segment_id(), Some(first.id()));
        assert_eq!(metadata_comment.section(), OemSection::Metadata);
        assert_eq!(metadata_comment.source_line(), 7);
        assert_eq!(metadata_comment.text(), "metadata provenance");

        assert_eq!(
            first
                .comments()
                .iter()
                .map(OemComment::source_line)
                .collect::<Vec<_>>(),
            [7, 18, 20]
        );
        assert_eq!(
            first
                .records()
                .map(|record| match record {
                    OemRecordRef::Comment(comment) => ("comment", comment.source_line()),
                    OemRecordRef::Coordinates(sample) => ("coordinates", sample.source_line()),
                    OemRecordRef::Covariance(covariance) => {
                        ("covariance", covariance.source_line())
                    }
                })
                .collect::<Vec<_>>(),
            [
                ("comment", 7),
                ("comment", 18),
                ("coordinates", 19),
                ("comment", 20),
                ("coordinates", 21),
            ]
        );

        let first_sample = &first.coordinates()[0];
        assert_eq!(first_sample.segment_id(), first.id());
        assert_eq!(first_sample.source_line(), 19);
        assert!(std::ptr::eq(first_sample.metadata(), first.metadata()));
        assert_eq!(first.coordinates()[1].source_line(), 21);
        assert_eq!(second.coordinates()[0].source_line(), 31);
    }

    #[test]
    fn oem_coordinates_require_explicit_properties_to_become_a_spacecraft_view() {
        let message = parse_oem_kvn(SAMPLE).expect("valid CCSDS OEM KVN");
        let coordinates = &message.segments()[0].coordinates()[0];
        let id = CustomFrameId::new(7);
        let body = ReferenceFrame::new(
            FrameOrigin::Custom(id),
            FrameOrientation::custom(id, FrameMotion::NonInertial),
        );
        let owned_body =
            SpacecraftBodyFrame::new("TEST-SC", body).expect("spacecraft-owned body axes");
        let orientation = Orientation::identity(body, coordinates.coordinates().position().frame());
        let attitude = AttitudeState::new(
            orientation,
            BodyAngularVelocity::new(
                AngularVelocityVector::from_radians_per_second(0.0, 0.0, 0.0),
                owned_body.clone(),
                coordinates.coordinates().position().frame(),
            )
            .expect("finite angular velocity"),
        )
        .expect("consistent attitude frames");
        let inertia = InertiaTensor::principal(
            owned_body.clone(),
            MomentOfInertia::new::<kilogram_square_meter>(1.0),
            MomentOfInertia::new::<kilogram_square_meter>(1.0),
            MomentOfInertia::new::<kilogram_square_meter>(1.0),
        )
        .expect("fixture inertia is physical");
        let state = CartesianState::try_from(*coordinates.coordinates())
            .expect("OEM position and velocity share one frame");
        let spacecraft = Spacecraft::new(owned_body, SpacecraftShape::Point);
        let view = SpacecraftView::new(
            &spacecraft,
            Orbit::new(coordinates.epoch(), state),
            Mass::new::<kilogram>(500.0),
            inertia,
            attitude,
        )
        .expect("fixture spacecraft view is physical");

        assert_eq!(view.spacecraft(), &spacecraft);
        assert_eq!(view.epoch(), coordinates.epoch());
        assert_eq!(view.mass(), Mass::new::<kilogram>(500.0));
    }

    #[test]
    fn streaming_events_do_not_require_document_collection() {
        let input = commented_input();
        let events = OemKvnReader::new(std::io::Cursor::new(input.as_bytes()))
            .collect::<Result<Vec<_>, _>>()
            .expect("stream is valid");

        let first_id = OemSegmentId(0);
        let second_id = OemSegmentId(1);
        assert!(matches!(
            &events[0],
            OemEvent::Header(header) if header.comments()[0].source_line() == 4
        ));
        assert!(matches!(
            &events[1],
            OemEvent::SegmentStart(context)
                if context.id() == first_id
                    && context.metadata().comments()[0].source_line() == 7
        ));
        assert!(matches!(
            &events[2],
            OemEvent::Comment(comment) if comment.source_line() == 18
        ));
        assert!(matches!(
            &events[3],
            OemEvent::Coordinates(sample)
                if sample.segment_id() == first_id && sample.source_line() == 19
        ));
        assert!(matches!(
            &events[4],
            OemEvent::Comment(comment) if comment.source_line() == 20
        ));
        assert!(matches!(
            &events[5],
            OemEvent::Coordinates(sample)
                if sample.segment_id() == first_id && sample.source_line() == 21
        ));
        assert!(matches!(&events[6], OemEvent::SegmentEnd(id) if *id == first_id));
        assert!(matches!(
            &events[7],
            OemEvent::SegmentStart(context) if context.id() == second_id
        ));
        assert!(matches!(
            &events[8],
            OemEvent::Coordinates(sample)
                if sample.segment_id() == second_id && sample.source_line() == 31
        ));
        assert!(matches!(&events[9], OemEvent::SegmentEnd(id) if *id == second_id));
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn tokio_reader_matches_blocking_event_order() {
        let input = commented_input();
        let expected = OemKvnReader::new(std::io::Cursor::new(input.as_bytes()))
            .collect::<Result<Vec<_>, _>>()
            .expect("blocking stream is valid");
        let mut reader = AsyncOemKvnReader::new(tokio::io::BufReader::new(input.as_bytes()));
        let mut actual = Vec::new();
        while let Some(event) = reader.next_event().await {
            actual.push(event.expect("async stream is valid"));
        }

        assert_eq!(actual, expected);
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn rayon_collection_matches_sequential_collection() {
        let input = commented_input();
        assert_eq!(
            parse_oem_kvn_parallel(&input).expect("parallel parse succeeds"),
            parse_oem_kvn(&input).expect("sequential parse succeeds")
        );
    }

    #[test]
    fn unsupported_time_system_is_explicit() {
        let input = SAMPLE.replacen("TIME_SYSTEM = UTC", "TIME_SYSTEM = MET", 1);
        assert!(matches!(
            parse_oem_kvn(&input),
            Err(OemError::UnsupportedTimeSystem { value, .. }) if value == "MET"
        ));
    }

    #[test]
    fn orekit_issue_839_covariances_are_constructed_with_units() {
        let message = parse_oem_kvn(ISSUE_839_COVARIANCE)
            .expect("Orekit Issue 839 covariance fixture parses");
        let segment = message.segments().first().expect("covariance segment");
        let covariances = segment.covariances();
        assert_eq!(covariances.len(), 2);
        assert_eq!(covariances[0].frame().identifier(), "RTN");
        assert_eq!(covariances[0].frame().reference_frame(), None);
        assert_eq!(
            covariances[1].frame().reference_frame(),
            Some(ReferenceFrame::new(
                segment.metadata().frame().origin(),
                FrameOrientation::Eme2000,
            ))
        );
        assert_eq!(
            covariances[0].matrix()[0][0],
            CartesianCovarianceEntry::Position(Area::new::<square_kilometer>(3.331_349_4e-4))
        );
        assert_eq!(
            covariances[0].matrix()[3][0],
            CartesianCovarianceEntry::PositionVelocity(
                PositionVelocityCovariance::from_square_metres_per_second(-0.334_936_5)
            )
        );
        assert_eq!(
            covariances[0].matrix()[4][4],
            CartesianCovarianceEntry::Velocity(
                VelocityVariance::from_square_metres_per_square_second(0.000_176_751_47)
            )
        );
        assert!(matches!(
            segment.records().last(),
            Some(OemRecordRef::Covariance(_))
        ));
    }

    #[test]
    fn application_declared_covariance_axes_are_preserved() {
        let input = ISSUE_839_COVARIANCE.replacen("COV_REF_FRAME = RTN", "COV_REF_FRAME = TNW", 1);
        let message = parse_oem_kvn(&input).expect("unknown axes declaration is retained");
        let covariance = &message.segments()[0].covariances()[0];

        assert_eq!(covariance.frame().identifier(), "TNW");
        assert_eq!(covariance.frame().reference_frame(), None);

        #[derive(Debug)]
        struct StationTopocentricAxes;
        impl OemCovarianceAxes for StationTopocentricAxes {
            fn identifier(&self) -> &str {
                "TOPOCENTRIC"
            }
        }

        assert_eq!(
            OemCovarianceFrame::new(StationTopocentricAxes).identifier(),
            "TOPOCENTRIC"
        );
    }

    #[test]
    fn malformed_state_reports_its_source_line() {
        let input = SAMPLE.replacen("7000 0 0 0 7.5 0", "7000 nope 0 0 7.5 0", 1);
        assert!(matches!(
            parse_oem_kvn(&input),
            Err(OemError::InvalidNumber {
                line: 16,
                field: "Y",
                ..
            })
        ));
    }

    #[test]
    fn decoder_limits_must_be_non_zero() {
        for (configured, expected) in [
            ((0, 1, 1, 1, 1), OemLimitKind::LineBytes),
            ((1, 0, 1, 1, 1), OemLimitKind::SectionBytes),
            ((1, 1, 0, 1, 1), OemLimitKind::SectionLines),
            ((1, 1, 1, 0, 1), OemLimitKind::DocumentBytes),
            ((1, 1, 1, 1, 0), OemLimitKind::DocumentLines),
        ] {
            assert_eq!(
                OemDecoderLimits::new(
                    configured.0,
                    configured.1,
                    configured.2,
                    configured.3,
                    configured.4,
                ),
                Err(OemDecoderLimitsError::Zero { kind: expected })
            );
        }
    }

    #[test]
    fn decoder_limits_reject_saturating_maxima() {
        for (configured, expected) in [
            ((usize::MAX, 1, 1, 1, 1), OemLimitKind::LineBytes),
            ((1, usize::MAX, 1, 1, 1), OemLimitKind::SectionBytes),
            ((1, 1, usize::MAX, 1, 1), OemLimitKind::SectionLines),
            ((1, 1, 1, usize::MAX, 1), OemLimitKind::DocumentBytes),
            ((1, 1, 1, 1, usize::MAX), OemLimitKind::DocumentLines),
        ] {
            assert_eq!(
                OemDecoderLimits::new(
                    configured.0,
                    configured.1,
                    configured.2,
                    configured.3,
                    configured.4,
                ),
                Err(OemDecoderLimitsError::Unbounded { kind: expected })
            );
        }
    }

    #[test]
    fn line_byte_limit_is_inclusive() {
        let longest = SAMPLE
            .lines()
            .map(str::len)
            .max()
            .expect("sample has lines");
        parse_oem_kvn_with_limits(
            SAMPLE,
            limits(
                longest,
                DEFAULT_MAX_SECTION_BYTES,
                DEFAULT_MAX_SECTION_LINES,
            ),
        )
        .expect("a line exactly at the configured boundary is valid");

        let error = parse_oem_kvn_with_limits(
            SAMPLE,
            limits(
                longest - 1,
                DEFAULT_MAX_SECTION_BYTES,
                DEFAULT_MAX_SECTION_LINES,
            ),
        )
        .expect_err("the longest source line exceeds the smaller boundary");

        assert!(matches!(
            error,
            OemError::ResourceLimitExceeded {
                kind: OemLimitKind::LineBytes,
                configured,
                observed,
                ..
            } if configured == longest - 1 && observed == longest
        ));
    }

    #[test]
    fn unterminated_oversized_line_fails_without_waiting_for_eof() {
        let mut reader = std::io::BufReader::with_capacity(4, std::io::Cursor::new(b"abcde"));
        let mut buffer = Vec::new();

        assert_eq!(
            read_bounded_line(&mut reader, &mut buffer, 3).expect("bounded read"),
            BoundedLine::TooLong { observed: 4 }
        );
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn async_unterminated_oversized_line_fails_without_waiting_for_eof() {
        let mut reader = tokio::io::BufReader::with_capacity(4, &b"abcde"[..]);
        let mut buffer = Vec::new();

        assert_eq!(
            read_bounded_line_async(&mut reader, &mut buffer, 3)
                .await
                .expect("bounded read"),
            BoundedLine::TooLong { observed: 4 }
        );
    }

    #[test]
    fn section_limits_are_inclusive_and_reset_at_boundaries() {
        let (max_bytes, max_lines) = maximum_section_totals(SAMPLE);
        parse_oem_kvn_with_limits(SAMPLE, limits(DEFAULT_MAX_LINE_BYTES, max_bytes, max_lines))
            .expect("each section independently fits its exact boundary");

        assert!(matches!(
            parse_oem_kvn_with_limits(
                SAMPLE,
                limits(DEFAULT_MAX_LINE_BYTES, max_bytes - 1, DEFAULT_MAX_SECTION_LINES),
            ),
            Err(OemError::ResourceLimitExceeded {
                kind: OemLimitKind::SectionBytes,
                configured,
                observed,
                ..
            }) if configured == max_bytes - 1 && observed > configured
        ));
        assert!(matches!(
            parse_oem_kvn_with_limits(
                SAMPLE,
                limits(DEFAULT_MAX_LINE_BYTES, DEFAULT_MAX_SECTION_BYTES, max_lines - 1),
            ),
            Err(OemError::ResourceLimitExceeded {
                kind: OemLimitKind::SectionLines,
                configured,
                observed,
                ..
            }) if configured == max_lines - 1 && observed == max_lines
        ));
    }

    #[test]
    fn repeated_short_comments_hit_a_finite_header_limit() {
        let input = format!("CCSDS_OEM_VERS = 3.0\n{}", "COMMENT filler\n".repeat(10));
        assert!(matches!(
            parse_oem_kvn_with_limits(&input, limits(64, 1_024, 3)),
            Err(OemError::ResourceLimitExceeded {
                line: 4,
                section: OemSection::Header,
                kind: OemLimitKind::SectionLines,
                configured: 3,
                observed: 4,
            })
        ));
    }

    #[test]
    fn document_limit_does_not_reset_between_segments_or_modes() {
        let max_lines = SAMPLE.lines().count() - 1;
        let limits = limits_with_document(DEFAULT_MAX_DOCUMENT_BYTES, max_lines);
        let assert_document_limit = |error: OemError| {
            assert!(matches!(
                error,
                OemError::ResourceLimitExceeded {
                    kind: OemLimitKind::DocumentLines,
                    configured,
                    observed,
                    ..
                } if configured == max_lines && observed == max_lines + 1
            ));
        };

        assert_document_limit(parse_oem_kvn_with_limits(SAMPLE, limits).expect_err("sequential"));
        #[cfg(feature = "parallel")]
        assert_document_limit(
            parse_oem_kvn_parallel_with_limits(SAMPLE, limits).expect_err("parallel"),
        );
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn async_document_limit_does_not_reset_between_segments() {
        let max_lines = SAMPLE.lines().count() - 1;
        let limits = limits_with_document(DEFAULT_MAX_DOCUMENT_BYTES, max_lines);
        let mut reader =
            AsyncOemKvnReader::with_limits(tokio::io::BufReader::new(SAMPLE.as_bytes()), limits);
        let error = loop {
            match reader.next_event().await {
                Some(Err(error)) => break error,
                Some(Ok(_)) => {}
                None => panic!("document limit must reject the source"),
            }
        };
        assert!(matches!(
            error,
            OemError::ResourceLimitExceeded {
                kind: OemLimitKind::DocumentLines,
                configured,
                observed,
                ..
            } if configured == max_lines && observed == max_lines + 1
        ));
    }

    #[test]
    fn lf_and_crlf_are_equivalent_in_blocking_and_parallel_modes() {
        let crlf = SAMPLE.replace('\n', "\r\n");
        let (max_section_bytes, max_section_lines) = maximum_section_totals(SAMPLE);
        let longest = SAMPLE
            .lines()
            .map(str::len)
            .max()
            .expect("sample has lines");
        let limits = limits(longest, max_section_bytes, max_section_lines);

        assert_eq!(
            parse_oem_kvn_with_limits(SAMPLE, limits).expect("LF document"),
            parse_oem_kvn_with_limits(&crlf, limits).expect("CRLF document")
        );
        let lf_events = OemKvnReader::with_limits(std::io::Cursor::new(SAMPLE.as_bytes()), limits)
            .collect::<Result<Vec<_>, _>>()
            .expect("LF stream");
        let crlf_events = OemKvnReader::with_limits(std::io::Cursor::new(crlf.as_bytes()), limits)
            .collect::<Result<Vec<_>, _>>()
            .expect("CRLF stream");
        assert_eq!(lf_events, crlf_events);

        #[cfg(feature = "parallel")]
        assert_eq!(
            parse_oem_kvn_parallel_with_limits(SAMPLE, limits).expect("parallel LF document"),
            parse_oem_kvn_parallel_with_limits(&crlf, limits).expect("parallel CRLF document")
        );
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn explicit_limits_and_line_endings_match_in_async_mode() {
        let crlf = SAMPLE.replace('\n', "\r\n");
        let (max_section_bytes, max_section_lines) = maximum_section_totals(SAMPLE);
        let longest = SAMPLE
            .lines()
            .map(str::len)
            .max()
            .expect("sample has lines");
        let limits = limits(longest, max_section_bytes, max_section_lines);
        let expected = OemKvnReader::with_limits(std::io::Cursor::new(SAMPLE.as_bytes()), limits)
            .collect::<Result<Vec<_>, _>>()
            .expect("blocking LF stream");
        let mut reader =
            AsyncOemKvnReader::with_limits(tokio::io::BufReader::new(crlf.as_bytes()), limits);
        let mut actual = Vec::new();
        while let Some(event) = reader.next_event().await {
            actual.push(event.expect("async CRLF stream"));
        }
        assert_eq!(actual, expected);
    }

    #[test]
    fn invalid_utf8_is_rejected_before_parsing() {
        let error = OemKvnReader::new(std::io::Cursor::new([0xff, b'\n']))
            .next()
            .expect("the source has a first line")
            .expect_err("OEM KVN is UTF-8 text");

        assert!(matches!(error, OemError::InvalidUtf8 { line: 1 }));
    }

    #[test]
    fn duplicate_state_epoch_is_rejected() {
        let (previous_line, previous_epoch, current_line, current_epoch) =
            chronology_signature(parse_oem_kvn(&duplicate_epoch_input()).expect_err("duplicate"));

        assert_eq!((previous_line, current_line), (16, 17));
        assert_eq!(previous_epoch, current_epoch);
    }

    #[test]
    fn reversed_state_epochs_are_rejected() {
        let input = SAMPLE.replacen("2024-01-01T00:00:00 7000", "2024-01-01T00:02:00 7000", 1);
        let (previous_line, previous_epoch, current_line, current_epoch) =
            chronology_signature(parse_oem_kvn(&input).expect_err("reversed chronology"));

        assert_eq!((previous_line, current_line), (16, 17));
        assert!(previous_epoch > current_epoch);
    }

    #[test]
    fn chronology_resets_between_segments() {
        let message = parse_oem_kvn(SAMPLE).expect("each segment is independently ordered");
        let first_segment_last = message.segments()[0]
            .coordinates()
            .last()
            .expect("first segment has states")
            .epoch();
        let second_segment_first = message.segments()[1]
            .coordinates()
            .first()
            .expect("second segment has states")
            .epoch();

        assert!(second_segment_first < first_segment_last);
    }

    #[test]
    fn chronology_error_matches_blocking_and_parallel_modes() {
        let input = duplicate_epoch_input();
        let expected = chronology_signature(parse_oem_kvn(&input).expect_err("sequential error"));
        let mut reader = OemKvnReader::new(std::io::Cursor::new(input.as_bytes()));
        let mut coordinates = 0usize;
        let streaming_error = loop {
            match reader.next().expect("stream reaches chronology error") {
                Ok(OemEvent::Coordinates(_)) => coordinates += 1,
                Ok(_) => {}
                Err(error) => break chronology_signature(error),
            }
        };
        assert_eq!(coordinates, 1, "invalid sample is not emitted");
        assert!(
            reader.next().is_none(),
            "reader terminates after chronology error"
        );
        assert_eq!(streaming_error, expected);

        #[cfg(feature = "parallel")]
        assert_eq!(
            chronology_signature(
                parse_oem_kvn_parallel(&input).expect_err("parallel chronology error")
            ),
            expected
        );
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn chronology_error_matches_async_mode() {
        let input = duplicate_epoch_input();
        let expected = chronology_signature(parse_oem_kvn(&input).expect_err("sequential error"));
        let mut reader = AsyncOemKvnReader::new(tokio::io::BufReader::new(input.as_bytes()));
        let mut coordinates = 0usize;
        let actual = loop {
            match reader
                .next_event()
                .await
                .expect("async stream reaches chronology error")
            {
                Ok(OemEvent::Coordinates(_)) => coordinates += 1,
                Ok(_) => {}
                Err(error) => break chronology_signature(error),
            }
        };
        assert_eq!(coordinates, 1, "invalid sample is not emitted");
        assert!(reader.next_event().await.is_none());
        assert_eq!(actual, expected);
    }

    #[test]
    fn state_epochs_must_fit_segment_bounds() {
        let input = SAMPLE.replacen("2024-01-01T00:00:00 7000", "2023-12-31T23:59:59 7000", 1);
        assert!(matches!(
            parse_oem_kvn(&input),
            Err(OemError::StateOutsideSegment { line: 16 })
        ));
    }

    #[test]
    fn earth_fixed_frame_rejects_non_earth_center() {
        let input = SAMPLE.replacen("REF_FRAME = ICRF", "REF_FRAME = ITRF2020", 1);
        assert!(matches!(
            parse_oem_kvn(&input),
            Err(OemError::IncompatibleFrameCenter { center, frame, .. })
                if center == "MARS" && frame == "ITRF2020"
        ));
    }
}
