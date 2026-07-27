#![no_main]

use std::io::Cursor;

use ccsds::{
    parse_oem_kvn_parallel_with_limits, parse_oem_kvn_with_limits, OemDecoderLimits,
    OemKvnReader,
};
use libfuzzer_sys::fuzz_target;

const MAX_INPUT_BYTES: usize = 256 * 1024;

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT_BYTES {
        return;
    }

    let limits = OemDecoderLimits::new(4_096, 128 * 1_024, 4_096, MAX_INPUT_BYTES, 8_192)
        .expect("fixed fuzz limits are finite and non-zero");

    for event in OemKvnReader::with_limits(Cursor::new(data), limits) {
        drop(event);
    }

    let Ok(input) = std::str::from_utf8(data) else {
        return;
    };
    let sequential = parse_oem_kvn_with_limits(input, limits);
    let parallel = parse_oem_kvn_parallel_with_limits(input, limits);
    assert_eq!(sequential.is_ok(), parallel.is_ok());
    if let (Ok(sequential), Ok(parallel)) = (sequential, parallel) {
        assert_eq!(sequential, parallel);
    }
});
