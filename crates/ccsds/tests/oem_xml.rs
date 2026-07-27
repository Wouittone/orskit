use std::io::{BufReader, Cursor};

use ccsds::{
    parse_oem_xml, parse_oem_xml_with_limits, OemDecoderLimits, OemError, OemEvent, OemLimitKind,
    OemXmlReader,
};

const FIXTURE: &str = include_str!("../testdata/project_oem_3_0.xml");

#[test]
fn standard_shaped_fixture_preserves_events_and_semantics() {
    let events = OemXmlReader::new(BufReader::new(Cursor::new(FIXTURE)))
        .collect::<Result<Vec<_>, _>>()
        .expect("valid OEM XML events");
    assert!(matches!(events.first(), Some(OemEvent::Header(_))));
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, OemEvent::Coordinates(_)))
            .count(),
        2
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, OemEvent::Covariance(_)))
            .count(),
        1
    );
    assert!(events.iter().any(
        |event| matches!(event, OemEvent::Comment(comment) if comment.text().contains("Data comment"))
    ));

    let document = parse_oem_xml(FIXTURE).expect("valid collected OEM XML");
    assert_eq!(document.header().version(), "3.0");
    assert_eq!(document.segments().len(), 1);
    assert_eq!(document.segments()[0].coordinates().len(), 2);
    assert_eq!(document.segments()[0].covariances().len(), 1);
}

#[test]
fn rejects_malformed_xml_and_external_markup() {
    let malformed = FIXTURE.replacen("</stateVector>", "</wrong>", 1);
    assert!(matches!(
        parse_oem_xml(&malformed),
        Err(OemError::MalformedXml { .. }) | Err(OemError::UnexpectedContent { .. })
    ));
    let with_doctype = FIXTURE.replacen(
        "<oem ",
        "<!DOCTYPE oem [<!ENTITY x SYSTEM \"file:///etc/passwd\">]><oem ",
        1,
    );
    assert!(parse_oem_xml(&with_doctype).is_err());
}

#[test]
fn accepts_consistently_qualified_ccsds_elements() {
    let document = parse_oem_xml(&qualified_fixture()).expect("qualified OEM XML");
    assert_eq!(document.segments().len(), 1);
    assert_eq!(document.segments()[0].coordinates().len(), 2);
}

#[test]
fn enforces_xml_declaration_and_namespace_contract() {
    let missing_declaration = FIXTURE
        .strip_prefix("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n")
        .expect("fixture declaration");
    assert!(matches!(
        parse_oem_xml(missing_declaration),
        Err(OemError::UnexpectedContent { .. })
    ));

    let wrong_version = FIXTURE.replacen("version=\"1.0\"", "version=\"1.1\"", 1);
    assert!(matches!(
        parse_oem_xml(&wrong_version),
        Err(OemError::InvalidXmlElement { .. })
    ));

    let missing_encoding = FIXTURE.replacen(" encoding=\"UTF-8\"", "", 1);
    assert!(matches!(
        parse_oem_xml(&missing_encoding),
        Err(OemError::InvalidXmlElement { .. })
    ));

    let standalone = FIXTURE.replacen("?>", " standalone=\"yes\"?>", 1);
    assert!(matches!(
        parse_oem_xml(&standalone),
        Err(OemError::InvalidXmlElement { .. })
    ));

    let duplicate_declaration =
        FIXTURE.replacen("?>", "?>\n<?xml version=\"1.0\" encoding=\"UTF-8\"?>", 1);
    assert!(matches!(
        parse_oem_xml(&duplicate_declaration),
        Err(OemError::UnexpectedContent { .. })
    ));

    let wrong_namespace = FIXTURE.replacen("urn:ccsds:schema:ndmxml", "urn:example:unrelated", 1);
    assert!(matches!(
        parse_oem_xml(&wrong_namespace),
        Err(OemError::InvalidXmlElement { .. })
    ));

    let mixed = qualified_fixture().replacen("<ndm:header>", "<header>", 1);
    assert!(matches!(
        parse_oem_xml(&mixed),
        Err(OemError::UnexpectedContent { .. })
    ));

    let wrong_id = FIXTURE.replacen("CCSDS_OEM_VERS", "CCSDS_OPM_VERS", 1);
    assert!(matches!(
        parse_oem_xml(&wrong_id),
        Err(OemError::InvalidXmlElement {
            element,
            message,
            ..
        }) if element == "oem"
            && message.contains("id must be CCSDS_OEM_VERS")
            && message.contains("CCSDS_OPM_VERS")
    ));
}

#[test]
fn enforces_document_text_depth_and_record_budgets() {
    let limits = OemDecoderLimits::new(1_000, 1_000_000, 10_000, 64, 10_000).unwrap();
    let byte_error = parse_oem_xml_with_limits(FIXTURE, limits).expect_err("byte limit");
    assert!(
        matches!(
            byte_error,
            OemError::ResourceLimitExceeded {
                kind: OemLimitKind::DocumentBytes,
                ..
            }
        ),
        "{byte_error:?}"
    );

    let long_text = FIXTURE.replace("ORSKIT", &"X".repeat(80));
    let limits = OemDecoderLimits::new(64, 1_000_000, 10_000, 1_000_000, 10_000).unwrap();
    assert!(matches!(
        parse_oem_xml_with_limits(&long_text, limits),
        Err(OemError::ResourceLimitExceeded {
            kind: OemLimitKind::LineBytes,
            ..
        })
    ));

    let nested = format!("{}{}{}", "<a>".repeat(33), FIXTURE, "</a>".repeat(33));
    assert!(matches!(
        parse_oem_xml(&nested),
        Err(OemError::UnexpectedContent { .. }) | Err(OemError::XmlDepthLimitExceeded { .. })
    ));

    let limits = OemDecoderLimits::new(65_536, 1_000_000, 10_000, 1_000_000, 5).unwrap();
    assert!(matches!(
        parse_oem_xml_with_limits(FIXTURE, limits),
        Err(OemError::ResourceLimitExceeded {
            kind: OemLimitKind::DocumentLines,
            ..
        }) | Err(OemError::XmlRecordLimitExceeded { .. })
    ));
}

#[test]
fn physical_line_limit_is_inclusive_at_eof_and_for_crlf() {
    let single_line = FIXTURE.lines().collect::<String>();
    assert!(!single_line.ends_with(['\r', '\n']));
    let exact = single_line.len();
    parse_oem_xml_with_limits(&single_line, generous_limits(exact, &single_line))
        .expect("an EOF-terminated line exactly at the bound is valid");

    let error = parse_oem_xml_with_limits(&single_line, generous_limits(exact - 1, &single_line))
        .expect_err("one byte beyond the EOF-terminated line bound");
    assert!(matches!(
        error,
        OemError::ResourceLimitExceeded {
            kind: OemLimitKind::LineBytes,
            configured,
            observed,
            ..
        } if configured == exact - 1 && observed == exact
    ));

    let crlf = FIXTURE.replace('\n', "\r\n");
    let longest_content_line = FIXTURE.lines().map(str::len).max().expect("fixture lines");
    parse_oem_xml_with_limits(&crlf, generous_limits(longest_content_line, &crlf))
        .expect("CRLF bytes are excluded from the inclusive content bound");
}

#[test]
fn segment_cardinality_rejects_duplicate_or_missing_sections_coherently() {
    let body_start = FIXTURE.find("<body>").expect("body start");
    let body_end = FIXTURE.find("</body>").expect("body end") + "</body>".len();
    let body = &FIXTURE[body_start..body_end];
    let duplicate_body = FIXTURE.replacen("</oem>", &format!("{body}</oem>"), 1);
    assert!(matches!(
        parse_oem_xml(&duplicate_body),
        Err(OemError::UnexpectedContent { content, .. }) if content.contains("duplicate <body>")
    ));
    let without_body = format!("{}{}", &FIXTURE[..body_start], &FIXTURE[body_end..]);
    assert!(matches!(
        parse_oem_xml(&without_body),
        Err(OemError::UnexpectedContent { content, .. })
            if content.contains("</oem> before <body>")
    ));

    let duplicate_data = FIXTURE.replacen("</data>", "</data><data></data>", 1);
    assert!(matches!(
        parse_oem_xml(&duplicate_data),
        Err(OemError::UnexpectedContent { content, .. }) if content.contains("duplicate <data>")
    ));

    let metadata_start = FIXTURE.find("<metadata>").expect("metadata start");
    let metadata_end = FIXTURE.find("</metadata>").expect("metadata end") + "</metadata>".len();
    let metadata = &FIXTURE[metadata_start..metadata_end];
    let duplicate_metadata = FIXTURE.replacen("<data>", &format!("{metadata}<data>"), 1);
    let mut reader = OemXmlReader::new(BufReader::new(Cursor::new(duplicate_metadata)));
    let mut segment_starts = 0;
    let error = loop {
        match reader.next().expect("duplicate metadata must be rejected") {
            Ok(OemEvent::SegmentStart(_)) => segment_starts += 1,
            Ok(_) => {}
            Err(error) => break error,
        }
    };
    assert_eq!(segment_starts, 1, "must not emit a second segment start");
    assert!(matches!(
        error,
        OemError::UnexpectedContent { content, .. } if content.contains("duplicate <metadata>")
    ));
    assert!(reader.next().is_none(), "structural errors are terminal");

    let data_start = FIXTURE.find("<data>").expect("data start");
    let data_end = FIXTURE.find("</data>").expect("data end") + "</data>".len();
    let without_data = format!("{}{}", &FIXTURE[..data_start], &FIXTURE[data_end..]);
    assert!(matches!(
        parse_oem_xml(&without_data),
        Err(OemError::UnexpectedContent { content, .. })
            if content.contains("segment ended before data")
    ));
}

#[test]
fn rejects_wrong_units_and_non_increasing_epochs() {
    let units = FIXTURE.replacen("units=\"km/s\"", "units=\"m/s\"", 1);
    assert!(matches!(
        parse_oem_xml(&units),
        Err(OemError::InvalidXmlElement { .. })
    ));
    let duplicate = FIXTURE.replacen(
        "2024-01-01T00:01:00</EPOCH>",
        "2024-01-01T00:00:00</EPOCH>",
        1,
    );
    assert!(matches!(
        parse_oem_xml(&duplicate),
        Err(OemError::NonIncreasingStateEpoch { .. })
    ));
}

#[test]
fn chronology_errors_are_terminal_for_streaming_iteration() {
    let duplicate = FIXTURE.replacen(
        "2024-01-01T00:01:00</EPOCH>",
        "2024-01-01T00:00:00</EPOCH>",
        1,
    );
    let mut reader = OemXmlReader::new(BufReader::new(Cursor::new(duplicate)));
    let error = loop {
        match reader.next().expect("duplicate epoch must be rejected") {
            Ok(_) => {}
            Err(error) => break error,
        }
    };
    assert!(matches!(error, OemError::NonIncreasingStateEpoch { .. }));
    assert!(reader.next().is_none(), "chronology errors are terminal");
}

#[test]
fn future_xml_fuzz_regressions_do_not_panic_the_bounded_reader() {
    let regressions = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("testdata")
        .join("fuzz-regressions")
        .join("xml");
    let limits = OemDecoderLimits::new(4_096, 131_072, 4_096, 262_144, 8_192)
        .expect("fixed limits are finite and non-zero");

    for entry in std::fs::read_dir(regressions).expect("regression directory must exist") {
        let path = entry.expect("regression entry must be readable").path();
        if !path.is_file() || path.extension().is_some_and(|extension| extension == "md") {
            continue;
        }
        let input = std::fs::read(&path).expect("regression input must be readable");
        for event in OemXmlReader::with_limits(BufReader::new(Cursor::new(input)), limits) {
            drop(event);
        }
    }
}

fn qualified_fixture() -> String {
    const TAGS: &[&str] = &[
        "oem",
        "header",
        "COMMENT",
        "CREATION_DATE",
        "ORIGINATOR",
        "MESSAGE_ID",
        "body",
        "segment",
        "metadata",
        "OBJECT_NAME",
        "OBJECT_ID",
        "CENTER_NAME",
        "REF_FRAME",
        "TIME_SYSTEM",
        "START_TIME",
        "STOP_TIME",
        "INTERPOLATION",
        "INTERPOLATION_DEGREE",
        "data",
        "stateVector",
        "EPOCH",
        "X",
        "Y",
        "Z",
        "X_DOT",
        "Y_DOT",
        "Z_DOT",
        "X_DDOT",
        "Y_DDOT",
        "Z_DDOT",
        "covarianceMatrix",
        "COV_REF_FRAME",
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

    let mut qualified = FIXTURE.to_owned();
    for tag in TAGS {
        qualified = qualified
            .replace(&format!("<{tag}>"), &format!("<ndm:{tag}>"))
            .replace(&format!("<{tag} "), &format!("<ndm:{tag} "))
            .replace(&format!("</{tag}>"), &format!("</ndm:{tag}>"));
    }
    qualified
}

fn generous_limits(max_line_bytes: usize, input: &str) -> OemDecoderLimits {
    OemDecoderLimits::new(
        max_line_bytes,
        input.len() + 1,
        input.len() + 1,
        input.len() + 1,
        input.len() + 1,
    )
    .expect("test limits are finite and non-zero")
}
