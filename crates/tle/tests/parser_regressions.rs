use tle::{TleError, TleLine, TwoLineElement};

#[test]
fn retained_checksum_regression_is_rejected_with_line_context() {
    let regression = include_bytes!("../fuzz/corpus/two_line_element/checksum.tle");
    let text = std::str::from_utf8(regression).expect("regression corpus is UTF-8");
    assert!(matches!(
        text.parse::<TwoLineElement>(),
        Err(TleError::ChecksumMismatch {
            line: TleLine::One,
            ..
        })
    ));
}
