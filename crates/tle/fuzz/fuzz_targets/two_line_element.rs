#![no_main]

use libfuzzer_sys::fuzz_target;
use tle::TwoLineElement;

const MAX_INPUT_BYTES: usize = 140;

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT_BYTES {
        return;
    }
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let _ = text.parse::<TwoLineElement>();
});
