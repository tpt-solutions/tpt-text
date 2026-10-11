#![no_main]

//! UAX #14 break positions must be sorted, on character boundaries, and end at
//! the text length, for any UTF-8 input.

use libfuzzer_sys::fuzz_target;
use tpt_text_linebreak::linebreaks;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let mut last = 0;
    let mut end = 0;
    for (position, _kind) in linebreaks(text) {
        assert!(position >= last, "break positions must not decrease");
        assert!(text.is_char_boundary(position), "break inside a character");
        last = position;
        end = position;
    }
    assert_eq!(end, text.len(), "the end of text is always a break");
});
