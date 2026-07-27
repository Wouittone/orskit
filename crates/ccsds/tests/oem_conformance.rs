use std::io::Cursor;

use ccsds::{parse_oem_kvn, OemDecoderLimits, OemKvnReader, OemTimeSystem};
use frames::{Body, FrameOrientation, FrameOrigin, ReferenceFrame};

const PROJECT_MULTISEGMENT: &str = include_str!("../testdata/project_multisegment.oem");
const OREKIT_ISSUE_839: &str = include_str!("../testdata/orekit_oem_issue839_covariance.oem");

#[test]
fn project_fixture_preserves_segments_frames_times_and_acceleration() {
    let document = parse_oem_kvn(PROJECT_MULTISEGMENT).expect("project fixture must remain valid");

    assert_eq!(document.header().message_id(), Some("ORSKIT-CONFORMANCE-1"));
    assert_eq!(document.segments().len(), 2);

    let earth = &document.segments()[0];
    assert_eq!(earth.metadata().frame(), ReferenceFrame::EME2000);
    assert_eq!(earth.metadata().time_system(), OemTimeSystem::Utc);
    assert_eq!(earth.coordinates().len(), 2);
    assert!(earth.coordinates()[1]
        .coordinates()
        .acceleration()
        .is_some());

    let mars = &document.segments()[1];
    assert_eq!(
        mars.metadata().frame(),
        ReferenceFrame::new(FrameOrigin::Body(Body::MARS), FrameOrientation::Icrf)
    );
    assert_eq!(mars.metadata().time_system(), OemTimeSystem::Tai);
}

#[test]
fn attributed_interoperability_fixture_preserves_both_covariance_axes() {
    let document = parse_oem_kvn(OREKIT_ISSUE_839).expect("attributed fixture must remain valid");
    let covariances = document.segments()[0].covariances();

    assert_eq!(covariances.len(), 2);
    assert_eq!(covariances[0].frame().identifier(), "RTN");
    assert_eq!(covariances[1].frame().identifier(), "EME2000");
}

#[test]
fn future_fuzz_regressions_do_not_panic_the_bounded_streaming_reader() {
    let regressions = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("testdata")
        .join("fuzz-regressions");
    let limits = OemDecoderLimits::new(4_096, 131_072, 4_096, 262_144, 8_192)
        .expect("fixed limits are finite and non-zero");

    for entry in std::fs::read_dir(regressions).expect("regression directory must exist") {
        let path = entry.expect("regression entry must be readable").path();
        if !path.is_file() || path.extension().is_some_and(|extension| extension == "md") {
            continue;
        }
        let input = std::fs::read(&path).expect("regression input must be readable");
        for event in OemKvnReader::with_limits(Cursor::new(input), limits) {
            drop(event);
        }
    }
}
