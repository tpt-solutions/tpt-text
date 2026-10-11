//! Golden files that record the whole `ParagraphLayout`, not a summary.
//! Runs with `--features serde`; update with `TPT_GOLDEN_UPDATE=1`.

#![cfg(feature = "serde")]
#![allow(clippy::unwrap_used)]

use std::path::PathBuf;

use tpt_text::{
    layout_paragraph_with, Align, FallbackResolver, FontRegistry, Fx, LayoutOptions, TextStyle,
};

const LATIN: &[u8] = include_bytes!("fonts/NotoSans-Regular.ttf");
const ARABIC: &[u8] = include_bytes!("fonts/NotoSansArabic-Regular.ttf");
const EMOJI: &[u8] = include_bytes!("fonts/NotoEmoji-Regular.ttf");

fn registry() -> FontRegistry {
    let reg = FontRegistry::new();
    reg.load_from_memory(LATIN.to_vec(), 0, "Noto Sans", 400, false)
        .unwrap();
    reg.load_from_memory(ARABIC.to_vec(), 0, "Noto Sans Arabic", 400, false)
        .unwrap();
    reg.load_from_memory(EMOJI.to_vec(), 0, "Noto Emoji", 400, false)
        .unwrap();
    reg
}

fn golden(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/full")
        .join(format!("{name}.json"))
}

#[test]
fn full_latin_justify() {
    let reg = registry();
    let style = TextStyle::new("Noto Sans", Fx::from_int(14));
    let options = LayoutOptions::default().align(Align::Justify);
    let text = "Deterministic layout means the same glyphs and positions on every platform.";
    let layout =
        layout_paragraph_with(&reg, text, &style, &[], Fx::from_int(220), &options).unwrap();
    tpt_text_golden::check(golden("latin_justify"), &layout);
}

#[test]
fn full_arabic_numbers() {
    let reg = registry();
    let style = TextStyle::new("Noto Sans Arabic", Fx::from_int(14));
    let options = LayoutOptions::default().align(Align::Right);
    let text = "\u{0628}\u{062A}\u{0628} 2026 \u{062A}\u{0628}";
    let layout =
        layout_paragraph_with(&reg, text, &style, &[], Fx::from_int(300), &options).unwrap();
    tpt_text_golden::check(golden("arabic_numbers"), &layout);
}

#[test]
fn full_emoji_joiners_with_fallback() {
    let reg = registry();
    let resolver = FallbackResolver::new().with_defaults(&["Noto Emoji"]);
    let style = TextStyle::new("Noto Sans", Fx::from_int(14));
    let options = LayoutOptions::default().fallback(&resolver);
    let text = "family \u{1F469}\u{200D}\u{1F680} launch";
    let layout =
        layout_paragraph_with(&reg, text, &style, &[], Fx::from_int(260), &options).unwrap();
    tpt_text_golden::check(golden("emoji_joiners"), &layout);
}

#[test]
fn full_mixed_bidi_with_missing_glyph() {
    let reg = registry();
    let style = TextStyle::new("Noto Sans", Fx::from_int(14));
    let text = "Invoice \u{0628}\u{062A} total \u{4E00} due.";
    let layout = layout_paragraph_with(
        &reg,
        text,
        &style,
        &[],
        Fx::from_int(240),
        &LayoutOptions::default(),
    )
    .unwrap();
    tpt_text_golden::check(golden("mixed_bidi_missing"), &layout);
}
