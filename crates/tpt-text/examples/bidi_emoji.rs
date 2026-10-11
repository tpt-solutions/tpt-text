//! A mixed-direction paragraph with emoji and a missing glyph: shows Bidi
//! ordering, fallback fonts, reported missing glyphs, and why each line ends
//! where it does.
//!
//! Usage:
//!   cargo run -p tpt-text --example bidi_emoji

use tpt_text::{
    explain_line_breaks, layout_paragraph_with, FallbackResolver, FontRegistry, Fx, LayoutOptions,
    TextStyle,
};

const LATIN: &[u8] = include_bytes!("../tests/fonts/NotoSans-Regular.ttf");
const ARABIC: &[u8] = include_bytes!("../tests/fonts/NotoSansArabic-Regular.ttf");
const EMOJI: &[u8] = include_bytes!("../tests/fonts/NotoEmoji-Regular.ttf");

fn main() {
    let registry = FontRegistry::new();
    registry
        .load_from_memory(LATIN.to_vec(), 0, "Noto Sans", 400, false)
        .expect("font");
    registry
        .load_from_memory(ARABIC.to_vec(), 0, "Noto Sans Arabic", 400, false)
        .expect("font");
    registry
        .load_from_memory(EMOJI.to_vec(), 0, "Noto Emoji", 400, false)
        .expect("font");

    // Emoji and Arabic come from fallback faces; the CJK character has no face.
    let resolver = FallbackResolver::new().with_defaults(&["Noto Emoji", "Noto Sans Arabic"]);
    let options = LayoutOptions::default().fallback(&resolver);
    let style = TextStyle::new("Noto Sans", Fx::from_int(16));
    let text = "Order: \u{0628}\u{062A}\u{0628} 2026 ships \u{1F680} today \u{4E00}";

    let layout = layout_paragraph_with(&registry, text, &style, &[], Fx::from_int(150), &options)
        .expect("font is registered");

    println!("paragraph direction: {:?}", layout.direction);
    for (i, line) in layout.lines.iter().enumerate() {
        let glyphs: usize = line.runs.iter().map(|r| r.glyphs.len()).sum();
        println!(
            "line {i}: bytes {:?}, {glyphs} glyphs, {} run(s), x offset {}",
            line.text_range,
            line.runs.len(),
            line.x_offset
        );
    }
    for reason in explain_line_breaks(&layout) {
        println!(
            "line {} ended at byte {} by {}",
            reason.line, reason.byte, reason.rule
        );
    }
    for missing in &layout.missing_glyphs {
        println!("no glyph for {:?} at byte {}", missing.ch, missing.byte);
    }
    println!("digest: {:016x}", layout.digest());
}
