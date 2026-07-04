//! CCSDS 502.0-B-3 OEM KVN reader.

use std::{fmt, io::BufRead, str::FromStr};

use orskit_core::{
    CartesianCoordinates, CoordinateSample, Epoch, FramedAcceleration, FramedPosition,
    FramedVelocity, KinematicError,
};
use orskit_frames::{Body, FrameOrientation, FrameOrigin, ReferenceFrame};
use orskit_units::{AccelerationVector, Position, VelocityVector};
use thiserror::Error;

#[cfg(feature = "parallel")]
use rayon::prelude::*;
#[cfg(feature = "async")]
use tokio::io::{AsyncBufRead, AsyncBufReadExt};

const DEFAULT_MAX_LINE_LENGTH: usize = 64 * 1024;

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

/// OEM file header.
#[derive(Debug, Clone, PartialEq)]
pub struct OemHeader {
    version: String,
    creation_date: Epoch,
    originator: String,
    message_id: Option<String>,
    comments: Vec<String>,
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
    pub fn comments(&self) -> &[String] {
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
    comments: Vec<String>,
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
    pub fn comments(&self) -> &[String] {
        &self.comments
    }
}

/// One collected OEM segment.
#[derive(Debug, Clone, PartialEq)]
pub struct OemSegment {
    metadata: OemMetadata,
    coordinates: Vec<CoordinateSample<CartesianCoordinates>>,
}

impl OemSegment {
    /// Returns the segment metadata.
    #[must_use]
    pub const fn metadata(&self) -> &OemMetadata {
        &self.metadata
    }

    /// Returns timed ephemeris coordinates in source order.
    #[must_use]
    pub fn coordinates(&self) -> &[CoordinateSample<CartesianCoordinates>] {
        &self.coordinates
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
    /// The beginning of a segment and its validated metadata.
    SegmentStart(OemMetadata),
    /// One typed, timed Cartesian ephemeris point.
    Coordinates(CoordinateSample<CartesianCoordinates>),
    /// The end of the current segment.
    SegmentEnd,
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
    /// A line exceeded the configured allocation boundary.
    #[error("OEM line {line} exceeds the configured {max_bytes}-byte limit")]
    LineTooLong {
        /// Source line.
        line: usize,
        /// Configured byte limit.
        max_bytes: usize,
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
    /// Covariance needs a typed parameterization not implemented in this slice.
    #[error("OEM covariance block at line {line} is not supported yet")]
    UnsupportedCovariance {
        /// Source line.
        line: usize,
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
    TooLong,
}

fn read_bounded_line<R: BufRead>(
    reader: &mut R,
    buffer: &mut Vec<u8>,
    max_bytes: usize,
) -> std::io::Result<BoundedLine> {
    buffer.clear();
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return Ok(if buffer.is_empty() {
                BoundedLine::Eof
            } else {
                BoundedLine::Line
            });
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let take = newline.map_or(available.len(), |index| index + 1);
        let remaining = max_bytes.saturating_sub(buffer.len());
        if take > remaining {
            buffer.extend_from_slice(&available[..remaining]);
            reader.consume(remaining);
            return Ok(BoundedLine::TooLong);
        }
        buffer.extend_from_slice(&available[..take]);
        reader.consume(take);
        if newline.is_some() {
            return Ok(BoundedLine::Line);
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
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            return Ok(if buffer.is_empty() {
                BoundedLine::Eof
            } else {
                BoundedLine::Line
            });
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let take = newline.map_or(available.len(), |index| index + 1);
        let remaining = max_bytes.saturating_sub(buffer.len());
        if take > remaining {
            buffer.extend_from_slice(&available[..remaining]);
            reader.consume(remaining);
            return Ok(BoundedLine::TooLong);
        }
        buffer.extend_from_slice(&available[..take]);
        reader.consume(take);
        if newline.is_some() {
            return Ok(BoundedLine::Line);
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
    buffer: Vec<u8>,
    max_line_length: usize,
    finished: bool,
}

impl<R: BufRead> OemKvnReader<R> {
    /// Constructs a reader over any blocking buffered source.
    #[must_use]
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            decoder: Decoder::default(),
            buffer: Vec::new(),
            max_line_length: DEFAULT_MAX_LINE_LENGTH,
            finished: false,
        }
    }

    /// Constructs a reader with a caller-selected maximum source-line length.
    ///
    /// A zero-byte limit is promoted to one byte. Limiting individual lines
    /// prevents an untrusted source from forcing an unbounded line allocation.
    #[must_use]
    pub fn with_max_line_length(reader: R, max_bytes: usize) -> Self {
        Self {
            max_line_length: max_bytes.max(1),
            ..Self::new(reader)
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
            match read_bounded_line(&mut self.reader, &mut self.buffer, self.max_line_length) {
                Ok(BoundedLine::Eof) => {
                    self.finished = true;
                    return self.decoder.finish().transpose();
                }
                Ok(BoundedLine::TooLong) => {
                    self.finished = true;
                    return Some(Err(OemError::LineTooLong {
                        line: self.decoder.line + 1,
                        max_bytes: self.max_line_length,
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
                        Ok(Some(output)) => return Some(decode_output(output)),
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
    buffer: Vec<u8>,
    max_line_length: usize,
    finished: bool,
}

#[cfg(feature = "async")]
impl<R: AsyncBufRead + Unpin> AsyncOemKvnReader<R> {
    /// Constructs a reader over any Tokio buffered source.
    #[must_use]
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            decoder: Decoder::default(),
            buffer: Vec::new(),
            max_line_length: DEFAULT_MAX_LINE_LENGTH,
            finished: false,
        }
    }

    /// Constructs a reader with a caller-selected maximum source-line length.
    #[must_use]
    pub fn with_max_line_length(reader: R, max_bytes: usize) -> Self {
        Self {
            max_line_length: max_bytes.max(1),
            ..Self::new(reader)
        }
    }

    /// Reads and decodes the next event.
    pub async fn next_event(&mut self) -> Option<Result<OemEvent, OemError>> {
        if self.finished {
            return None;
        }

        loop {
            match read_bounded_line_async(&mut self.reader, &mut self.buffer, self.max_line_length)
                .await
            {
                Ok(BoundedLine::Eof) => {
                    self.finished = true;
                    return self.decoder.finish().transpose();
                }
                Ok(BoundedLine::TooLong) => {
                    self.finished = true;
                    return Some(Err(OemError::LineTooLong {
                        line: self.decoder.line + 1,
                        max_bytes: self.max_line_length,
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
                        Ok(Some(output)) => return Some(decode_output(output)),
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
    collect_document(OemKvnReader::new(std::io::Cursor::new(input.as_bytes())))
}

/// Parses and collects an in-memory OEM KVN document with ordered Rayon state
/// conversion.
///
/// Structural scanning remains sequential. Only independent state records are
/// converted in parallel, after their segment frame and time system are known.
#[cfg(feature = "parallel")]
pub fn parse_oem_kvn_parallel(input: &str) -> Result<Oem, OemError> {
    let mut decoder = Decoder::default();
    let mut layout = Vec::new();
    let mut states = Vec::new();

    for line in input.lines() {
        if line.len() > DEFAULT_MAX_LINE_LENGTH {
            return Err(OemError::LineTooLong {
                line: decoder.line + 1,
                max_bytes: DEFAULT_MAX_LINE_LENGTH,
            });
        }
        if let Some(output) = decoder.push_line(line)? {
            match output {
                DecoderOutput::Event(event) => layout.push(ParallelLayout::Event(Box::new(event))),
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

    let parsed: Vec<Result<CoordinateSample<CartesianCoordinates>, OemError>> = states
        .par_iter()
        .map(|raw| {
            parse_state_line(
                raw.text,
                raw.line,
                raw.frame,
                raw.time_system,
                raw.start_time,
                raw.stop_time,
            )
        })
        .collect();
    let mut parsed = parsed.into_iter().map(Some).collect::<Vec<_>>();
    let events = layout.into_iter().map(|item| match item {
        ParallelLayout::Event(event) => Ok(*event),
        ParallelLayout::State(index) => parsed[index]
            .take()
            .ok_or(OemError::InvalidEventOrder {
                message: "parallel state layout was consumed more than once",
            })?
            .map(OemEvent::Coordinates),
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
            OemEvent::SegmentStart(metadata) if header.is_some() && active.is_none() => {
                active = Some(OemSegment {
                    metadata,
                    coordinates: Vec::new(),
                });
            }
            OemEvent::Coordinates(coordinates) => active
                .as_mut()
                .ok_or(OemError::InvalidEventOrder {
                    message: "state outside a segment",
                })?
                .coordinates
                .push(coordinates),
            OemEvent::SegmentEnd => {
                segments.push(active.take().ok_or(OemError::InvalidEventOrder {
                    message: "segment end without segment start",
                })?)
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

fn decode_output(output: DecoderOutput<'_>) -> Result<OemEvent, OemError> {
    match output {
        DecoderOutput::Event(event) => Ok(event),
        DecoderOutput::State(raw) => parse_state_line(
            raw.text,
            raw.line,
            raw.frame,
            raw.time_system,
            raw.start_time,
            raw.stop_time,
        )
        .map(OemEvent::Coordinates),
    }
}

#[derive(Default)]
struct Decoder {
    line: usize,
    phase: Phase,
    header: HeaderBuilder,
    metadata: MetadataBuilder,
    current_metadata: Option<OemMetadata>,
    current_state_count: usize,
}

#[derive(Debug, Clone, Copy, Default)]
enum Phase {
    #[default]
    Header,
    Metadata,
    Data,
    Done,
}

enum DecoderOutput<'a> {
    Event(OemEvent),
    State(RawState<'a>),
}

#[derive(Clone, Copy)]
struct RawState<'a> {
    text: &'a str,
    line: usize,
    frame: ReferenceFrame,
    time_system: OemTimeSystem,
    start_time: Epoch,
    stop_time: Epoch,
}

impl Decoder {
    fn push_line<'a>(&mut self, source: &'a str) -> Result<Option<DecoderOutput<'a>>, OemError> {
        self.line += 1;
        let line = source.trim();
        if line.is_empty() {
            return Ok(None);
        }

        match self.phase {
            Phase::Header => {
                if line == "META_START" {
                    let header = std::mem::take(&mut self.header).finish(self.line)?;
                    self.phase = Phase::Metadata;
                    Ok(Some(DecoderOutput::Event(OemEvent::Header(header))))
                } else {
                    self.header.push(line, self.line)?;
                    Ok(None)
                }
            }
            Phase::Metadata => {
                if line == "META_STOP" {
                    let metadata = std::mem::take(&mut self.metadata).finish(self.line)?;
                    self.current_metadata = Some(metadata.clone());
                    self.current_state_count = 0;
                    self.phase = Phase::Data;
                    Ok(Some(DecoderOutput::Event(OemEvent::SegmentStart(metadata))))
                } else {
                    self.metadata.push(line, self.line)?;
                    Ok(None)
                }
            }
            Phase::Data => {
                if line == "META_START" {
                    if self.current_state_count == 0 {
                        return Err(OemError::EmptySegment { line: self.line });
                    }
                    self.current_metadata = None;
                    self.phase = Phase::Metadata;
                    return Ok(Some(DecoderOutput::Event(OemEvent::SegmentEnd)));
                }
                if line == "COVARIANCE_START" {
                    return Err(OemError::UnsupportedCovariance { line: self.line });
                }
                if comment_value(line).is_some() {
                    return Ok(None);
                }
                if line.contains('=') || line.ends_with("_STOP") {
                    return Err(OemError::UnexpectedContent {
                        line: self.line,
                        section: "data",
                        content: line.to_owned(),
                    });
                }
                let metadata =
                    self.current_metadata
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
                    frame: metadata.frame,
                    time_system: metadata.time_system,
                    start_time: metadata.start_time,
                    stop_time: metadata.stop_time,
                })))
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
                self.current_metadata = None;
                Ok(Some(OemEvent::SegmentEnd))
            }
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

#[derive(Default)]
struct HeaderBuilder {
    version: Option<String>,
    creation_date: Option<String>,
    originator: Option<String>,
    message_id: Option<String>,
    comments: Vec<String>,
}

impl HeaderBuilder {
    fn push(&mut self, line: &str, number: usize) -> Result<(), OemError> {
        if let Some(comment) = comment_value(line) {
            self.comments.push(comment.to_owned());
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
    comments: Vec<String>,
}

impl MetadataBuilder {
    fn push(&mut self, line: &str, number: usize) -> Result<(), OemError> {
        if let Some(comment) = comment_value(line) {
            self.comments.push(comment.to_owned());
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
    use orskit_core::{CartesianState, InertiaTensor, Orientation, SpacecraftProperties, State};
    use orskit_frames::{CustomFrameId, FrameOrientation};
    use orskit_units::uom::si::{mass::kilogram, moment_of_inertia::kilogram_square_meter};
    use orskit_units::{Mass, MomentOfInertia};

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
    fn oem_coordinates_require_explicit_properties_to_become_a_state() {
        let message = parse_oem_kvn(SAMPLE).expect("valid CCSDS OEM KVN");
        let coordinates = message.segments()[0].coordinates()[0];
        let id = CustomFrameId::new(7);
        let body = ReferenceFrame::new(FrameOrigin::Custom(id), FrameOrientation::Custom(id));
        let orientation = Orientation::identity(body, coordinates.coordinates().position().frame());
        let inertia = InertiaTensor::principal(
            body,
            MomentOfInertia::new::<kilogram_square_meter>(1.0),
            MomentOfInertia::new::<kilogram_square_meter>(1.0),
            MomentOfInertia::new::<kilogram_square_meter>(1.0),
        )
        .expect("fixture inertia is physical");
        let properties =
            SpacecraftProperties::new(Mass::new::<kilogram>(500.0), orientation, inertia)
                .expect("fixture properties are physical");
        let state = CartesianState::new(coordinates, properties);

        assert_eq!(state.epoch(), coordinates.epoch());
        assert_eq!(state.mass(), Mass::new::<kilogram>(500.0));
    }

    #[test]
    fn streaming_events_do_not_require_document_collection() {
        let events = OemKvnReader::new(std::io::Cursor::new(SAMPLE.as_bytes()))
            .collect::<Result<Vec<_>, _>>()
            .expect("stream is valid");

        assert!(matches!(events.first(), Some(OemEvent::Header(_))));
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, OemEvent::Coordinates(_)))
                .count(),
            3
        );
        assert!(matches!(events.last(), Some(OemEvent::SegmentEnd)));
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn tokio_reader_matches_blocking_event_order() {
        let expected = OemKvnReader::new(std::io::Cursor::new(SAMPLE.as_bytes()))
            .collect::<Result<Vec<_>, _>>()
            .expect("blocking stream is valid");
        let mut reader = AsyncOemKvnReader::new(tokio::io::BufReader::new(SAMPLE.as_bytes()));
        let mut actual = Vec::new();
        while let Some(event) = reader.next_event().await {
            actual.push(event.expect("async stream is valid"));
        }

        assert_eq!(actual, expected);
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn rayon_collection_matches_sequential_collection() {
        assert_eq!(
            parse_oem_kvn_parallel(SAMPLE).expect("parallel parse succeeds"),
            parse_oem_kvn(SAMPLE).expect("sequential parse succeeds")
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
    fn covariance_is_not_silently_discarded() {
        let input = SAMPLE.replacen(
            "META_START\nOBJECT_NAME = MARS TEST",
            "COVARIANCE_START\nMETA_START\nOBJECT_NAME = MARS TEST",
            1,
        );
        assert!(matches!(
            parse_oem_kvn(&input),
            Err(OemError::UnsupportedCovariance { .. })
        ));
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
    fn blocking_reader_enforces_line_allocation_limit() {
        let error = OemKvnReader::with_max_line_length(std::io::Cursor::new(SAMPLE.as_bytes()), 8)
            .next()
            .expect("the source has a first line")
            .expect_err("the first line exceeds eight bytes");

        assert!(matches!(
            error,
            OemError::LineTooLong {
                line: 1,
                max_bytes: 8
            }
        ));
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
