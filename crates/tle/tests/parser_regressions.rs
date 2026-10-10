use tle::{TleError, TleLine, TwoLineElement};

#[test]
fn retained_checksum_regression_is_rejected_with_line_context() {
    let text = "1 23455U 94089A   97320.90946019  .00000140  00000-0  10191-3 0  2620\n\
                2 23455  99.0090 272.6745 0008546 223.1686 136.8816 14.11711747148495";
    assert!(matches!(
        text.parse::<TwoLineElement>(),
        Err(TleError::ChecksumMismatch {
            line: TleLine::One,
            ..
        })
    ));
}
