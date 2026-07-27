use std::io::{BufRead, Cursor, Read};

use quick_xml::{
    events::{BytesStart, Event},
    Reader, XmlVersion,
};

use super::{
    collect_document, parse_state_line, validate_event_chronology, CovarianceBuilder,
    HeaderBuilder, MetadataBuilder, Oem, OemComment, OemDecoderLimits, OemError, OemEvent,
    OemLimitKind, OemSample, OemSection, OemSegmentContext, OemSegmentId, SegmentChronology,
};

const MAX_XML_DEPTH: usize = 32;

/// A bounded, streaming CCSDS OEM XML event reader.
///
/// The reader accepts the OEM 3.0 unqualified and namespace-qualified element
/// spellings defined by CCSDS 505.0-B-3. It validates the supported OEM subset
/// while emitting the same semantic events as [`super::OemKvnReader`].
///
/// ```
/// # use std::io::{BufReader, Cursor};
/// # use ccsds::{OemEvent, OemXmlReader};
/// # let source = Cursor::new(
/// # b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>
/// # <oem xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"
/// # xmlns:ndm=\"urn:ccsds:schema:ndmxml\" id=\"CCSDS_OEM_VERS\" version=\"3.0\">
/// # <header><CREATION_DATE>2024-01-01T00:00:00</CREATION_DATE><ORIGINATOR>TEST</ORIGINATOR></header>
/// # <body><segment><metadata><OBJECT_NAME>TEST</OBJECT_NAME><OBJECT_ID>2024-001A</OBJECT_ID>
/// # <CENTER_NAME>EARTH</CENTER_NAME><REF_FRAME>EME2000</REF_FRAME><TIME_SYSTEM>UTC</TIME_SYSTEM>
/// # <START_TIME>2024-01-01T00:00:00</START_TIME><STOP_TIME>2024-01-01T00:01:00</STOP_TIME></metadata>
/// # <data><stateVector><EPOCH>2024-01-01T00:00:00</EPOCH><X>7000</X><Y>0</Y><Z>0</Z>
/// # <X_DOT>0</X_DOT><Y_DOT>7.5</Y_DOT><Z_DOT>0</Z_DOT></stateVector></data>
/// # </segment></body></oem>");
/// let reader = OemXmlReader::new(BufReader::new(source));
/// for event in reader {
///     if let OemEvent::Coordinates(sample) = event? {
///         println!("{}", sample.epoch());
///     }
/// }
/// # Ok::<(), ccsds::OemError>(())
/// ```
pub struct OemXmlReader<R: BufRead> {
    reader: Reader<BoundedInput<R>>,
    buffer: Vec<u8>,
    decoder: XmlDecoder,
    chronology: SegmentChronology,
    finished: bool,
}

impl<R: BufRead> OemXmlReader<R> {
    /// Creates a reader with finite default resource limits.
    #[must_use]
    pub fn new(source: R) -> Self {
        Self::with_limits(source, OemDecoderLimits::default())
    }

    /// Creates a reader with explicit finite resource limits.
    #[must_use]
    pub fn with_limits(source: R, limits: OemDecoderLimits) -> Self {
        let mut reader = Reader::from_reader(BoundedInput::new(source, limits));
        reader.config_mut().trim_text(false);
        Self {
            reader,
            buffer: Vec::new(),
            decoder: XmlDecoder::new(limits),
            chronology: SegmentChronology::default(),
            finished: false,
        }
    }
}

impl<R: BufRead> Iterator for OemXmlReader<R> {
    type Item = Result<OemEvent, OemError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        loop {
            self.buffer.clear();
            let event = match self.reader.read_event_into(&mut self.buffer) {
                Ok(event) => event.into_owned(),
                Err(quick_xml::Error::Io(source)) => {
                    self.finished = true;
                    return Some(Err(OemError::Io {
                        line: self.reader.get_ref().line(),
                        source: std::io::Error::new(source.kind(), source.to_string()),
                    }));
                }
                Err(source) => {
                    self.finished = true;
                    if let Some(error) = self.reader.get_ref().limit_error(self.decoder.section()) {
                        return Some(Err(error));
                    }
                    return Some(Err(OemError::MalformedXml {
                        line: self.reader.get_ref().line(),
                        source,
                    }));
                }
            };
            let line = self.reader.get_ref().line();
            if let Some(error) = self.reader.get_ref().limit_error(self.decoder.section()) {
                self.finished = true;
                return Some(Err(error));
            }
            let bytes = self.reader.get_ref().bytes;
            let lines = self.reader.get_ref().lines;
            if let Err(error) = self.decoder.account_input(bytes, lines, line) {
                self.finished = true;
                return Some(Err(error));
            }
            match self.decoder.push(event, line, self.reader.decoder()) {
                Ok(Some(event)) => {
                    let result = validate_event_chronology(event, &mut self.chronology);
                    if result.is_err() {
                        self.finished = true;
                    }
                    return Some(result);
                }
                Ok(None) => {
                    if self.decoder.done {
                        self.finished = true;
                        return None;
                    }
                }
                Err(error) => {
                    self.finished = true;
                    return Some(Err(error));
                }
            }
        }
    }
}

/// Parses and collects one in-memory CCSDS OEM XML document.
pub fn parse_oem_xml(input: &str) -> Result<Oem, OemError> {
    parse_oem_xml_with_limits(input, OemDecoderLimits::default())
}

/// Parses and collects one in-memory CCSDS OEM XML document with explicit
/// finite resource limits.
pub fn parse_oem_xml_with_limits(input: &str, limits: OemDecoderLimits) -> Result<Oem, OemError> {
    collect_document(OemXmlReader::with_limits(
        Cursor::new(input.as_bytes()),
        limits,
    ))
}

struct BoundedInput<R> {
    inner: R,
    limits: OemDecoderLimits,
    bytes: usize,
    lines: usize,
    line_bytes: usize,
    previous_was_cr: bool,
    at_line_start: bool,
    violation: Option<(OemLimitKind, usize, usize)>,
}

impl<R> BoundedInput<R> {
    const fn new(inner: R, limits: OemDecoderLimits) -> Self {
        Self {
            inner,
            limits,
            bytes: 0,
            lines: 0,
            line_bytes: 0,
            previous_was_cr: false,
            at_line_start: true,
            violation: None,
        }
    }

    const fn line(&self) -> usize {
        if self.lines == 0 {
            1
        } else {
            self.lines
        }
    }

    fn limit_error(&self, section: OemSection) -> Option<OemError> {
        self.violation.map(
            |(kind, configured, observed)| OemError::ResourceLimitExceeded {
                line: self.line(),
                section,
                kind,
                configured,
                observed,
            },
        )
    }
}

impl<R: BufRead> Read for BoundedInput<R> {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        let available = self.fill_buf()?;
        let count = output.len().min(available.len());
        output[..count].copy_from_slice(&available[..count]);
        self.consume(count);
        Ok(count)
    }
}

impl<R: BufRead> BufRead for BoundedInput<R> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        if self.violation.is_some() {
            return Ok(&[]);
        }
        let remaining = self
            .limits
            .max_document_bytes()
            .saturating_add(1)
            .saturating_sub(self.bytes);
        let available = self.inner.fill_buf()?;
        Ok(&available[..available.len().min(remaining)])
    }

    fn consume(&mut self, amount: usize) {
        let available = self.inner.fill_buf().unwrap_or(&[]);
        let amount = amount.min(available.len());
        for &byte in &available[..amount] {
            self.bytes = self.bytes.saturating_add(1);
            let crlf_continuation = byte == b'\n' && self.previous_was_cr;
            if crlf_continuation {
                self.previous_was_cr = false;
            } else {
                if self.at_line_start {
                    self.lines = self.lines.saturating_add(1);
                    self.at_line_start = false;
                }
                if matches!(byte, b'\r' | b'\n') {
                    self.line_bytes = 0;
                    self.at_line_start = true;
                    self.previous_was_cr = byte == b'\r';
                } else {
                    self.previous_was_cr = false;
                    self.line_bytes = self.line_bytes.saturating_add(1);
                    if self.line_bytes > self.limits.max_line_bytes() && self.violation.is_none() {
                        self.violation = Some((
                            OemLimitKind::LineBytes,
                            self.limits.max_line_bytes(),
                            self.line_bytes,
                        ));
                    }
                }
            }
            if self.bytes > self.limits.max_document_bytes() && self.violation.is_none() {
                self.violation = Some((
                    OemLimitKind::DocumentBytes,
                    self.limits.max_document_bytes(),
                    self.bytes,
                ));
            }
            if self.lines > self.limits.max_document_lines() && self.violation.is_none() {
                self.violation = Some((
                    OemLimitKind::DocumentLines,
                    self.limits.max_document_lines(),
                    self.lines,
                ));
            }
        }
        self.inner.consume(amount);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    BeforeRoot,
    Root,
    Header,
    Body,
    Segment,
    Metadata,
    Data,
    State,
    Covariance,
    AfterRoot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NamespaceMode {
    Unqualified,
    Qualified,
}

impl NamespaceMode {
    fn accepts(self, name: &[u8]) -> bool {
        match self {
            Self::Unqualified => !name.contains(&b':'),
            Self::Qualified => name.starts_with(b"ndm:") && !name[4..].contains(&b':'),
        }
    }
}

struct Leaf {
    name: String,
    text: String,
    unit: Option<String>,
    line: usize,
}

struct XmlDecoder {
    limits: OemDecoderLimits,
    phase: Phase,
    depth: usize,
    records: usize,
    accounted_bytes: usize,
    section_bytes: usize,
    section_start_line: usize,
    next_segment: usize,
    current_context: Option<OemSegmentContext>,
    header: HeaderBuilder,
    metadata: MetadataBuilder,
    leaf: Option<Leaf>,
    state: Vec<(String, String)>,
    covariance: Vec<(String, String)>,
    covariance_line: usize,
    state_line: usize,
    header_complete: bool,
    body_seen: bool,
    declaration_seen: bool,
    namespace_mode: Option<NamespaceMode>,
    segment_count: usize,
    current_state_count: usize,
    data_seen: bool,
    done: bool,
}

impl XmlDecoder {
    fn new(limits: OemDecoderLimits) -> Self {
        Self {
            limits,
            phase: Phase::BeforeRoot,
            depth: 0,
            records: 0,
            accounted_bytes: 0,
            section_bytes: 0,
            section_start_line: 1,
            next_segment: 0,
            current_context: None,
            header: HeaderBuilder::default(),
            metadata: MetadataBuilder::default(),
            leaf: None,
            state: Vec::new(),
            covariance: Vec::new(),
            covariance_line: 1,
            state_line: 1,
            header_complete: false,
            body_seen: false,
            declaration_seen: false,
            namespace_mode: None,
            segment_count: 0,
            current_state_count: 0,
            data_seen: false,
            done: false,
        }
    }

    const fn section(&self) -> OemSection {
        match self.phase {
            Phase::BeforeRoot | Phase::Root | Phase::Header | Phase::Body | Phase::AfterRoot => {
                OemSection::Header
            }
            Phase::Segment | Phase::Metadata => OemSection::Metadata,
            Phase::Data | Phase::State | Phase::Covariance => OemSection::Data,
        }
    }

    fn push(
        &mut self,
        event: Event<'static>,
        line: usize,
        decoder: quick_xml::encoding::Decoder,
    ) -> Result<Option<OemEvent>, OemError> {
        match event {
            Event::Start(start) => self.start(&start, line, decoder),
            Event::Empty(start) => {
                self.start(&start, line, decoder)?;
                self.end(start.name().as_ref(), line)
            }
            Event::End(end) => self.end(end.name().as_ref(), line),
            Event::Text(text) => {
                let value = text
                    .decode()
                    .map_err(|source| OemError::MalformedXml {
                        line,
                        source: source.into(),
                    })?
                    .into_owned();
                self.text(&value, line)?;
                Ok(None)
            }
            Event::CData(text) => {
                let value = text
                    .decode()
                    .map_err(|source| OemError::MalformedXml {
                        line,
                        source: source.into(),
                    })?
                    .into_owned();
                self.text(&value, line)?;
                Ok(None)
            }
            Event::Decl(declaration) => {
                if self.phase != Phase::BeforeRoot || self.declaration_seen {
                    return Err(self.unexpected(line, "XML declaration"));
                }
                let version =
                    declaration
                        .version()
                        .map_err(|source| OemError::InvalidXmlElement {
                            line,
                            element: "xml".to_owned(),
                            message: source.to_string(),
                        })?;
                if version.as_ref() != b"1.0" {
                    return Err(OemError::InvalidXmlElement {
                        line,
                        element: "xml".to_owned(),
                        message: "XML version must be 1.0".to_owned(),
                    });
                }
                let encoding = declaration
                    .encoding()
                    .ok_or_else(|| OemError::InvalidXmlElement {
                        line,
                        element: "xml".to_owned(),
                        message: "XML declaration must specify UTF-8".to_owned(),
                    })?
                    .map_err(|source| OemError::InvalidXmlElement {
                        line,
                        element: "xml".to_owned(),
                        message: source.to_string(),
                    })?;
                if !encoding.as_ref().eq_ignore_ascii_case(b"UTF-8") {
                    return Err(OemError::InvalidXmlElement {
                        line,
                        element: "xml".to_owned(),
                        message: "only UTF-8 XML declarations are supported".to_owned(),
                    });
                }
                if declaration.standalone().is_some() {
                    return Err(OemError::InvalidXmlElement {
                        line,
                        element: "xml".to_owned(),
                        message: "standalone is not part of the CCSDS XML declaration".to_owned(),
                    });
                }
                self.declaration_seen = true;
                Ok(None)
            }
            Event::Comment(_) => Ok(None),
            Event::GeneralRef(reference) => {
                let value = if let Some(value) = reference
                    .resolve_char_ref()
                    .map_err(|source| OemError::MalformedXml { line, source })?
                {
                    value.to_string()
                } else {
                    match reference.as_ref() {
                        b"amp" => "&".to_owned(),
                        b"lt" => "<".to_owned(),
                        b"gt" => ">".to_owned(),
                        b"apos" => "'".to_owned(),
                        b"quot" => "\"".to_owned(),
                        _ => return Err(self.unexpected(line, "undeclared entity reference")),
                    }
                };
                self.text(&value, line)?;
                Ok(None)
            }
            Event::PI(_) | Event::DocType(_) => {
                Err(self.unexpected(line, "processing instruction or DTD"))
            }
            Event::Eof => {
                if self.phase != Phase::AfterRoot || self.depth != 0 {
                    return Err(self.unexpected(line, "end of input before </oem>"));
                }
                self.done = true;
                Ok(None)
            }
        }
    }

    fn start(
        &mut self,
        start: &BytesStart<'_>,
        line: usize,
        decoder: quick_xml::encoding::Decoder,
    ) -> Result<Option<OemEvent>, OemError> {
        self.depth = self.depth.saturating_add(1);
        if self.depth > MAX_XML_DEPTH {
            return Err(OemError::XmlDepthLimitExceeded {
                line,
                configured: MAX_XML_DEPTH,
                observed: self.depth,
            });
        }
        if self.leaf.is_some() {
            return Err(self.unexpected(line, "nested element inside scalar value"));
        }
        let raw_name = start.name();
        let raw_name = raw_name.as_ref();
        let name = local_name(raw_name).to_owned();
        match (self.phase, name.as_str()) {
            (Phase::BeforeRoot, "oem") => {
                if !self.declaration_seen {
                    return Err(self.unexpected(line, "missing XML declaration"));
                }
                let (version, namespace_mode) = validate_root(start, line, decoder)?;
                self.header
                    .push(&format!("CCSDS_OEM_VERS = {version}"), line)?;
                self.namespace_mode = Some(namespace_mode);
                self.phase = Phase::Root;
            }
            _ if !self
                .namespace_mode
                .is_some_and(|mode| mode.accepts(raw_name)) =>
            {
                return Err(self.unexpected(line, &format!("<{name}> with inconsistent namespace")));
            }
            (Phase::Root, "header") => {
                if self.header_complete {
                    return Err(self.unexpected(line, "duplicate <header>"));
                }
                no_attributes(start, line)?;
                self.phase = Phase::Header;
                self.reset_section(line);
            }
            (Phase::Root, "body") => {
                if !self.header_complete {
                    return Err(self.unexpected(line, "<body> before </header>"));
                }
                if self.body_seen {
                    return Err(self.unexpected(line, "duplicate <body>"));
                }
                no_attributes(start, line)?;
                self.body_seen = true;
                self.phase = Phase::Body;
            }
            (Phase::Body, "segment") => {
                no_attributes(start, line)?;
                self.phase = Phase::Segment;
                self.current_state_count = 0;
                self.data_seen = false;
            }
            (Phase::Segment, "metadata") => {
                if self.current_context.is_some() {
                    return Err(self.unexpected(line, "duplicate <metadata>"));
                }
                no_attributes(start, line)?;
                self.phase = Phase::Metadata;
                self.metadata = MetadataBuilder::default();
                self.reset_section(line);
            }
            (Phase::Segment, "data") => {
                if self.current_context.is_none() {
                    return Err(self.unexpected(line, "<data> before metadata"));
                }
                if self.data_seen {
                    return Err(self.unexpected(line, "duplicate <data>"));
                }
                no_attributes(start, line)?;
                self.data_seen = true;
                self.phase = Phase::Data;
                self.reset_section(line);
            }
            (Phase::Data, "stateVector") => {
                no_attributes(start, line)?;
                self.phase = Phase::State;
                self.state.clear();
                self.state_line = line;
            }
            (Phase::Data, "covarianceMatrix") => {
                no_attributes(start, line)?;
                self.phase = Phase::Covariance;
                self.covariance.clear();
                self.covariance_line = line;
            }
            (phase, leaf) if allowed_leaf(phase, leaf) => {
                let unit = scalar_attributes(start, line, decoder)?;
                self.leaf = Some(Leaf {
                    name,
                    text: String::new(),
                    unit,
                    line,
                });
            }
            _ => return Err(self.unexpected(line, &format!("<{name}>"))),
        }
        Ok(None)
    }

    fn text(&mut self, value: &str, line: usize) -> Result<(), OemError> {
        if let Some(leaf) = &mut self.leaf {
            let observed = leaf.text.len().saturating_add(value.len());
            if observed > self.limits.max_line_bytes() {
                return Err(OemError::ResourceLimitExceeded {
                    line,
                    section: self.section(),
                    kind: OemLimitKind::LineBytes,
                    configured: self.limits.max_line_bytes(),
                    observed,
                });
            }
            leaf.text.push_str(value);
        } else if !value.trim().is_empty() {
            return Err(self.unexpected(line, "non-whitespace mixed content"));
        }
        Ok(())
    }

    fn end(&mut self, raw_name: &[u8], line: usize) -> Result<Option<OemEvent>, OemError> {
        if !self
            .namespace_mode
            .is_some_and(|mode| mode.accepts(raw_name))
        {
            return Err(self.unexpected(
                line,
                &format!("</{}> with inconsistent namespace", local_name(raw_name)),
            ));
        }
        let name = local_name(raw_name);
        let result = if self.leaf.as_ref().is_some_and(|leaf| leaf.name == name) {
            let leaf = self.leaf.take().expect("leaf existence checked");
            self.finish_leaf(leaf)?
        } else {
            match (self.phase, name) {
                (Phase::Header, "header") => {
                    self.phase = Phase::Root;
                    self.header_complete = true;
                    Some(OemEvent::Header(
                        std::mem::take(&mut self.header).finish(line)?,
                    ))
                }
                (Phase::Metadata, "metadata") => {
                    let metadata = std::mem::take(&mut self.metadata).finish(line)?;
                    let context = OemSegmentContext {
                        id: OemSegmentId(self.next_segment),
                        metadata: std::sync::Arc::new(metadata),
                    };
                    self.next_segment = self.next_segment.saturating_add(1);
                    self.current_context = Some(context.clone());
                    self.phase = Phase::Segment;
                    Some(OemEvent::SegmentStart(context))
                }
                (Phase::State, "stateVector") => {
                    let event = self.finish_state(line)?;
                    self.current_state_count = self.current_state_count.saturating_add(1);
                    self.phase = Phase::Data;
                    Some(event)
                }
                (Phase::Covariance, "covarianceMatrix") => {
                    let event = self.finish_covariance(line)?;
                    self.phase = Phase::Data;
                    Some(event)
                }
                (Phase::Data, "data") => {
                    self.phase = Phase::Segment;
                    None
                }
                (Phase::Segment, "segment") => {
                    if !self.data_seen {
                        return Err(self.unexpected(line, "segment ended before data"));
                    }
                    if self.current_state_count == 0 {
                        return Err(OemError::EmptySegment { line });
                    }
                    let context = self
                        .current_context
                        .take()
                        .ok_or_else(|| self.unexpected(line, "segment ended before metadata"))?;
                    self.phase = Phase::Body;
                    self.segment_count = self.segment_count.saturating_add(1);
                    Some(OemEvent::SegmentEnd(context.id()))
                }
                (Phase::Body, "body") => {
                    if self.segment_count == 0 {
                        return Err(self.unexpected(line, "body contains no segment"));
                    }
                    self.phase = Phase::Root;
                    None
                }
                (Phase::Root, "oem") => {
                    if !self.body_seen {
                        return Err(self.unexpected(line, "</oem> before <body>"));
                    }
                    self.phase = Phase::AfterRoot;
                    None
                }
                _ => return Err(self.unexpected(line, &format!("</{name}>"))),
            }
        };
        self.depth = self.depth.saturating_sub(1);
        Ok(result)
    }

    fn finish_leaf(&mut self, leaf: Leaf) -> Result<Option<OemEvent>, OemError> {
        let value = leaf.text.trim();
        if value.is_empty() && leaf.name != "COMMENT" {
            return Err(OemError::InvalidXmlElement {
                line: leaf.line,
                element: leaf.name,
                message: "value must not be empty".to_owned(),
            });
        }
        validate_unit(&leaf.name, leaf.unit.as_deref(), leaf.line)?;
        self.count_record(leaf.line)?;
        match self.phase {
            Phase::Header => {
                if leaf.name == "CLASSIFICATION" {
                    return Err(unsupported(
                        &leaf,
                        "CLASSIFICATION is not represented by OemHeader",
                    ));
                }
                self.header
                    .push(&format!("{} = {value}", leaf.name), leaf.line)?;
                Ok(None)
            }
            Phase::Metadata => {
                if leaf.name == "REF_FRAME_EPOCH" {
                    return Err(unsupported(
                        &leaf,
                        "REF_FRAME_EPOCH is not represented by OemMetadata",
                    ));
                }
                self.metadata.push(
                    &format!("{} = {value}", leaf.name),
                    leaf.line,
                    OemSegmentId(self.next_segment),
                )?;
                Ok(None)
            }
            Phase::Data if leaf.name == "COMMENT" => {
                let context = self
                    .current_context
                    .as_ref()
                    .expect("data requires segment context");
                Ok(Some(OemEvent::Comment(OemComment {
                    segment_id: Some(context.id()),
                    section: OemSection::Data,
                    source_line: leaf.line,
                    text: value.to_owned(),
                })))
            }
            Phase::State => {
                self.state.push((leaf.name, value.to_owned()));
                Ok(None)
            }
            Phase::Covariance => {
                if leaf.name == "COMMENT" {
                    return Err(unsupported(
                        &leaf,
                        "covariance comments are not represented by OemCartesianCovariance",
                    ));
                }
                self.covariance.push((leaf.name, value.to_owned()));
                Ok(None)
            }
            _ => Err(self.unexpected(leaf.line, &leaf.name)),
        }
    }

    fn finish_state(&mut self, line: usize) -> Result<OemEvent, OemError> {
        const REQUIRED: [&str; 7] = ["EPOCH", "X", "Y", "Z", "X_DOT", "Y_DOT", "Z_DOT"];
        const ACCELERATION: [&str; 3] = ["X_DDOT", "Y_DDOT", "Z_DDOT"];
        let names: Vec<&str> = self.state.iter().map(|(name, _)| name.as_str()).collect();
        if names != REQUIRED && names != [REQUIRED.as_slice(), ACCELERATION.as_slice()].concat() {
            return Err(self.unexpected(line, "invalid stateVector field order or completeness"));
        }
        let context = self
            .current_context
            .as_ref()
            .expect("state requires context")
            .clone();
        let wire = self
            .state
            .iter()
            .map(|(_, value)| value.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let metadata = context.metadata();
        let sample = parse_state_line(
            &wire,
            self.state_line,
            metadata.frame,
            metadata.time_system,
            metadata.start_time,
            metadata.stop_time,
        )?;
        Ok(OemEvent::Coordinates(OemSample {
            context,
            source_line: self.state_line,
            sample,
        }))
    }

    fn finish_covariance(&mut self, line: usize) -> Result<OemEvent, OemError> {
        const FIELDS: [&str; 22] = [
            "EPOCH",
            "CX_X",
            "CY_X",
            "CY_Y",
            "CZ_X",
            "CZ_Y",
            "CZ_Z",
            "CX_DOT_X",
            "CX_DOT_Y",
            "CX_DOT_Z",
            "CX_DOT_X_DOT",
            "CY_DOT_X",
            "CY_DOT_Y",
            "CY_DOT_Z",
            "CY_DOT_X_DOT",
            "CY_DOT_Y_DOT",
            "CZ_DOT_X",
            "CZ_DOT_Y",
            "CZ_DOT_Z",
            "CZ_DOT_X_DOT",
            "CZ_DOT_Y_DOT",
            "CZ_DOT_Z_DOT",
        ];
        let mut offset = 0;
        let frame = if self
            .covariance
            .get(1)
            .is_some_and(|(name, _)| name == "COV_REF_FRAME")
        {
            offset = 1;
            Some(self.covariance[1].1.clone())
        } else {
            None
        };
        let names = self
            .covariance
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != 1 || offset == 0)
            .map(|(_, (name, _))| name.as_str())
            .collect::<Vec<_>>();
        if names != FIELDS {
            return Err(
                self.unexpected(line, "invalid covarianceMatrix field order or completeness")
            );
        }
        let context = self
            .current_context
            .as_ref()
            .expect("covariance requires context")
            .clone();
        let epoch_text = &self.covariance[0].1;
        let epoch = super::parse_epoch(
            epoch_text,
            context.metadata().time_system,
            self.covariance_line,
        )?;
        let mut builder = CovarianceBuilder::new(context, self.covariance_line, epoch);
        let frame =
            frame.unwrap_or_else(|| builder.context.metadata().frame.orientation().to_string());
        builder.set_frame(&frame, self.covariance_line)?;
        let values = &self.covariance[(1 + offset)..];
        let mut start = 0;
        for width in 1..=6 {
            let row = values[start..start + width]
                .iter()
                .map(|(_, value)| value.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            builder.push_row(&row, self.covariance_line)?;
            start += width;
        }
        Ok(OemEvent::Covariance(builder.finish()?))
    }

    fn count_record(&mut self, line: usize) -> Result<(), OemError> {
        self.records = self.records.saturating_add(1);
        if self.records > self.limits.max_document_lines() {
            return Err(OemError::XmlRecordLimitExceeded {
                line,
                configured: self.limits.max_document_lines(),
                observed: self.records,
            });
        }
        Ok(())
    }

    fn account_input(
        &mut self,
        document_bytes: usize,
        document_lines: usize,
        line: usize,
    ) -> Result<(), OemError> {
        self.section_bytes = self
            .section_bytes
            .saturating_add(document_bytes.saturating_sub(self.accounted_bytes));
        self.accounted_bytes = document_bytes;
        if self.section_bytes > self.limits.max_section_bytes() {
            return Err(OemError::ResourceLimitExceeded {
                line,
                section: self.section(),
                kind: OemLimitKind::SectionBytes,
                configured: self.limits.max_section_bytes(),
                observed: self.section_bytes,
            });
        }
        let section_lines = document_lines
            .saturating_sub(self.section_start_line)
            .saturating_add(1);
        if section_lines > self.limits.max_section_lines() {
            return Err(OemError::ResourceLimitExceeded {
                line,
                section: self.section(),
                kind: OemLimitKind::SectionLines,
                configured: self.limits.max_section_lines(),
                observed: section_lines,
            });
        }
        Ok(())
    }

    fn reset_section(&mut self, line: usize) {
        self.section_bytes = 0;
        self.section_start_line = line;
    }

    fn unexpected(&self, line: usize, content: &str) -> OemError {
        OemError::UnexpectedContent {
            line,
            section: match self.phase {
                Phase::BeforeRoot
                | Phase::Root
                | Phase::Header
                | Phase::Body
                | Phase::AfterRoot => "XML header",
                Phase::Segment | Phase::Metadata => "XML metadata",
                Phase::Data | Phase::State | Phase::Covariance => "XML data",
            },
            content: content.to_owned(),
        }
    }
}

fn allowed_leaf(phase: Phase, name: &str) -> bool {
    match phase {
        Phase::Header => matches!(
            name,
            "COMMENT" | "CLASSIFICATION" | "CREATION_DATE" | "ORIGINATOR" | "MESSAGE_ID"
        ),
        Phase::Metadata => matches!(
            name,
            "COMMENT"
                | "OBJECT_NAME"
                | "OBJECT_ID"
                | "CENTER_NAME"
                | "REF_FRAME"
                | "REF_FRAME_EPOCH"
                | "TIME_SYSTEM"
                | "START_TIME"
                | "USEABLE_START_TIME"
                | "USEABLE_STOP_TIME"
                | "STOP_TIME"
                | "INTERPOLATION"
                | "INTERPOLATION_DEGREE"
        ),
        Phase::Data => name == "COMMENT",
        Phase::State => matches!(
            name,
            "EPOCH"
                | "X"
                | "Y"
                | "Z"
                | "X_DOT"
                | "Y_DOT"
                | "Z_DOT"
                | "X_DDOT"
                | "Y_DDOT"
                | "Z_DDOT"
        ),
        Phase::Covariance => matches!(
            name,
            "COMMENT"
                | "EPOCH"
                | "COV_REF_FRAME"
                | "CX_X"
                | "CY_X"
                | "CY_Y"
                | "CZ_X"
                | "CZ_Y"
                | "CZ_Z"
                | "CX_DOT_X"
                | "CX_DOT_Y"
                | "CX_DOT_Z"
                | "CX_DOT_X_DOT"
                | "CY_DOT_X"
                | "CY_DOT_Y"
                | "CY_DOT_Z"
                | "CY_DOT_X_DOT"
                | "CY_DOT_Y_DOT"
                | "CZ_DOT_X"
                | "CZ_DOT_Y"
                | "CZ_DOT_Z"
                | "CZ_DOT_X_DOT"
                | "CZ_DOT_Y_DOT"
                | "CZ_DOT_Z_DOT"
        ),
        _ => false,
    }
}

fn local_name(name: &[u8]) -> &str {
    let local = name.rsplit(|byte| *byte == b':').next().unwrap_or(name);
    std::str::from_utf8(local).unwrap_or("")
}

fn validate_root(
    start: &BytesStart<'_>,
    line: usize,
    decoder: quick_xml::encoding::Decoder,
) -> Result<(String, NamespaceMode), OemError> {
    let namespace_mode = match start.name().as_ref() {
        b"oem" => NamespaceMode::Unqualified,
        b"ndm:oem" => NamespaceMode::Qualified,
        _ => {
            return Err(OemError::InvalidXmlElement {
                line,
                element: "oem".to_owned(),
                message: "root must use either oem or ndm:oem".to_owned(),
            });
        }
    };
    let mut id = None;
    let mut version = None;
    let mut xsi_namespace = false;
    let mut ndm_namespace = false;
    for attribute in start.attributes() {
        let attribute = attribute.map_err(|source| OemError::InvalidXmlElement {
            line,
            element: "oem".to_owned(),
            message: source.to_string(),
        })?;
        let name = std::str::from_utf8(attribute.key.as_ref()).unwrap_or("");
        let value = attribute
            .decoded_and_normalized_value(XmlVersion::Implicit1_0, decoder)
            .map_err(|source| OemError::MalformedXml { line, source })?
            .into_owned();
        match name {
            "id" => id = Some(value),
            "version" => version = Some(value),
            "xmlns:xsi" if value == "http://www.w3.org/2001/XMLSchema-instance" => {
                xsi_namespace = true;
            }
            "xmlns:ndm" if value == "urn:ccsds:schema:ndmxml" => {
                ndm_namespace = true;
            }
            "xsi:noNamespaceSchemaLocation" => {}
            _ => {
                return Err(OemError::InvalidXmlElement {
                    line,
                    element: "oem".to_owned(),
                    message: format!("unexpected attribute {name}"),
                });
            }
        }
    }
    if !xsi_namespace || !ndm_namespace {
        return Err(OemError::InvalidXmlElement {
            line,
            element: "oem".to_owned(),
            message: "root must declare the CCSDS ndm and XML Schema Instance namespaces"
                .to_owned(),
        });
    }
    if id.as_deref() != Some("CCSDS_OEM_VERS") {
        return Err(OemError::InvalidXmlElement {
            line,
            element: "oem".to_owned(),
            message: format!(
                "id must be CCSDS_OEM_VERS, found {}",
                id.as_deref().unwrap_or("<missing>")
            ),
        });
    }
    if version.as_deref() != Some("3.0") {
        return Err(OemError::UnsupportedVersion {
            line,
            value: version.unwrap_or_else(|| "<missing>".to_owned()),
        });
    }
    Ok((version.expect("version checked above"), namespace_mode))
}

fn no_attributes(start: &BytesStart<'_>, line: usize) -> Result<(), OemError> {
    if let Some(attribute) = start.attributes().next() {
        let name = attribute
            .map(|attribute| String::from_utf8_lossy(attribute.key.as_ref()).into_owned())
            .unwrap_or_else(|error| error.to_string());
        return Err(OemError::InvalidXmlElement {
            line,
            element: local_name(start.name().as_ref()).to_owned(),
            message: format!("unexpected attribute {name}"),
        });
    }
    Ok(())
}

fn scalar_attributes(
    start: &BytesStart<'_>,
    line: usize,
    decoder: quick_xml::encoding::Decoder,
) -> Result<Option<String>, OemError> {
    let mut unit = None;
    for attribute in start.attributes() {
        let attribute = attribute.map_err(|source| OemError::InvalidXmlElement {
            line,
            element: local_name(start.name().as_ref()).to_owned(),
            message: source.to_string(),
        })?;
        let name = local_name(attribute.key.as_ref());
        if name != "units" {
            return Err(OemError::InvalidXmlElement {
                line,
                element: local_name(start.name().as_ref()).to_owned(),
                message: format!("unexpected attribute {name}"),
            });
        }
        if unit.is_some() {
            return Err(OemError::InvalidXmlElement {
                line,
                element: local_name(start.name().as_ref()).to_owned(),
                message: "duplicate units attribute".to_owned(),
            });
        }
        unit = Some(
            attribute
                .decoded_and_normalized_value(XmlVersion::Implicit1_0, decoder)
                .map_err(|source| OemError::MalformedXml { line, source })?
                .into_owned(),
        );
    }
    Ok(unit)
}

fn validate_unit(name: &str, unit: Option<&str>, line: usize) -> Result<(), OemError> {
    let expected = match name {
        "X" | "Y" | "Z" => Some("km"),
        "X_DOT" | "Y_DOT" | "Z_DOT" => Some("km/s"),
        "X_DDOT" | "Y_DDOT" | "Z_DDOT" => Some("km/s**2"),
        "CX_X" | "CY_X" | "CY_Y" | "CZ_X" | "CZ_Y" | "CZ_Z" => Some("km**2"),
        name if name.ends_with("_DOT") => Some("km**2/s**2"),
        name if name.contains("_DOT_") => Some("km**2/s"),
        _ => None,
    };
    if let Some(unit) = unit {
        if expected != Some(unit) {
            return Err(OemError::InvalidXmlElement {
                line,
                element: name.to_owned(),
                message: match expected {
                    Some(expected) => format!("units must be {expected}, found {unit}"),
                    None => format!("units attribute is not allowed (found {unit})"),
                },
            });
        }
    }
    Ok(())
}

fn unsupported(leaf: &Leaf, message: &str) -> OemError {
    OemError::InvalidXmlElement {
        line: leaf.line,
        element: leaf.name.clone(),
        message: message.to_owned(),
    }
}
