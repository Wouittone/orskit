#![no_main]

use std::io::{BufReader, Cursor};

use ccsds::{OemDecoderLimits, OemXmlReader, parse_oem_xml_with_limits};
use libfuzzer_sys::fuzz_target;

const MAX_INPUT_BYTES: usize = 256 * 1024;

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT_BYTES {
        return;
    }

    let limits = OemDecoderLimits::new(4_096, 128 * 1_024, 4_096, MAX_INPUT_BYTES, 8_192)
        .expect("fixed fuzz limits are finite and non-zero");

    for event in OemXmlReader::with_limits(BufReader::new(Cursor::new(data)), limits) {
        drop(event);
    }

    if let Ok(input) = std::str::from_utf8(data) {
        drop(parse_oem_xml_with_limits(input, limits));
    }
});
