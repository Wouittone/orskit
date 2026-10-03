#![no_main]

use libfuzzer_sys::fuzz_target;
use tle::TwoLineElement;

fuzz_target!(|input: &[u8]| {
    if let Ok(text) = std::str::from_utf8(input) {
        let _ = text.parse::<TwoLineElement>();
    }
});
