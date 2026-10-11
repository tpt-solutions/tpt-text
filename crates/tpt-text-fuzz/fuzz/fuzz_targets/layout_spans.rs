#![no_main]

//! Random text, style spans, options and widths must lay out without panicking,
//! and the same input must give the same digest twice.

use libfuzzer_sys::fuzz_target;
use tpt_text::{
    layout_paragraph_with, Align, FontRegistry, Fx, LayoutOptions, StyleSpan, TextStyle,
};

const LATIN: &[u8] = include_bytes!("../../../tpt-text/tests/fonts/NotoSans-Regular.ttf");
const ARABIC: &[u8] = include_bytes!("../../../tpt-text/tests/fonts/NotoSansArabic-Regular.ttf");

fuzz_target!(|data: &[u8]| {
    // The first bytes choose the options; the rest is the text.
    if data.len() < 4 {
        return;
    }
    let (header, body) = data.split_at(4);
    let Ok(text) = std::str::from_utf8(body) else {
        return;
    };

    let registry = FontRegistry::new();
    registry
        .load_from_memory(LATIN.to_vec(), 0, "Noto Sans", 400, false)
        .expect("vendored font");
    registry
        .load_from_memory(ARABIC.to_vec(), 0, "Noto Sans Arabic", 400, false)
        .expect("vendored font");

    let align = match header[0] % 6 {
        0 => Align::Start,
        1 => Align::End,
        2 => Align::Left,
        3 => Align::Right,
        4 => Align::Center,
        _ => Align::Justify,
    };
    let base = TextStyle::new("Noto Sans", Fx::from_int(1 + i32::from(header[1] % 24)));
    let mid = text.len() / 2;
    let spans = if text.is_char_boundary(mid) && header[2] % 2 == 0 {
        vec![StyleSpan {
            range: mid..text.len(),
            style: TextStyle::new("Noto Sans Arabic", Fx::from_int(12)),
        }]
    } else {
        Vec::new()
    };
    let options = LayoutOptions {
        align,
        max_lines: (header[3] % 3 == 0).then_some(usize::from(header[3] % 5) + 1),
        ellipsis: header[3] % 2 == 0,
        ..LayoutOptions::default()
    };
    let width = Fx::from_int(1 + i32::from(header[2]) * 2);

    if let Ok(layout) = layout_paragraph_with(&registry, text, &base, &spans, width, &options) {
        let again = layout_paragraph_with(&registry, text, &base, &spans, width, &options)
            .expect("same input lays out again");
        assert_eq!(layout.digest(), again.digest(), "layout must be deterministic");
    }
});
