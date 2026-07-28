use tle::TwoLineElement;

#[test]
fn retained_parser_inputs_do_not_panic() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("testdata")
        .join("parser-regressions");

    for entry in std::fs::read_dir(directory).expect("regression directory must exist") {
        let path = entry.expect("regression entry must be readable").path();
        if !path.is_file() || path.extension().is_some_and(|extension| extension == "md") {
            continue;
        }
        let bytes = std::fs::read(path).expect("regression input must be readable");
        if let Ok(text) = std::str::from_utf8(&bytes) {
            let _ = text.parse::<TwoLineElement>();
        }
    }
}
