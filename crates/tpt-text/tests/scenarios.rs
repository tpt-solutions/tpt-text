//! Scenario tests: emoji fallback, joiners (ZWJ/ZWNJ), and a large paragraph.

#![allow(clippy::unwrap_used)]

use tpt_text::{
    layout_paragraph, layout_paragraph_with, FallbackResolver, FontRegistry, Fx, LayoutOptions,
    ParagraphLayout, TextStyle,
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

fn style(family: &str) -> TextStyle {
    TextStyle::new(family, Fx::from_int(16))
}

fn clusters(layout: &ParagraphLayout) -> Vec<usize> {
    layout
        .lines
        .iter()
        .flat_map(|l| l.runs.iter())
        .flat_map(|r| r.glyphs.iter().map(|g| g.cluster_index))
        .collect()
}

fn families(layout: &ParagraphLayout) -> Vec<String> {
    layout.lines[0]
        .runs
        .iter()
        .map(|r| r.font.family().to_string())
        .collect()
}

#[test]
fn emoji_falls_back_to_emoji_font() {
    let reg = registry();
    let resolver = FallbackResolver::new().with_defaults(&["Noto Emoji"]);
    let opts = LayoutOptions {
        fallback: Some(&resolver),
        ..LayoutOptions::default()
    };
    let text = "a\u{1F600}b";
    let layout = layout_paragraph_with(
        &reg,
        text,
        &style("Noto Sans"),
        &[],
        Fx::from_int(1000),
        &opts,
    )
    .unwrap();
    assert_eq!(families(&layout), ["Noto Sans", "Noto Emoji", "Noto Sans"]);
    assert!(clusters(&layout).windows(2).all(|w| w[0] <= w[1]));
}

#[test]
fn zero_width_joiner_sequence_lays_out_in_order() {
    let reg = registry();
    let resolver = FallbackResolver::new().with_defaults(&["Noto Emoji"]);
    let opts = LayoutOptions {
        fallback: Some(&resolver),
        ..LayoutOptions::default()
    };
    // Man, ZWJ, woman: the joiner has no glyph of its own and must not panic.
    let text = "\u{1F468}\u{200D}\u{1F469}x";
    let layout = layout_paragraph_with(
        &reg,
        text,
        &style("Noto Sans"),
        &[],
        Fx::from_int(1000),
        &opts,
    )
    .unwrap();
    let c = clusters(&layout);
    assert!(!c.is_empty());
    assert!(c.windows(2).all(|w| w[0] <= w[1]));
    assert!(c.iter().all(|&b| b < text.len()));
}

#[test]
fn zero_width_non_joiner_in_arabic_does_not_break_shaping() {
    let reg = registry();
    let text = "\u{0628}\u{200C}\u{062A}";
    let layout = layout_paragraph(
        &reg,
        text,
        &style("Noto Sans Arabic"),
        &[],
        Fx::from_int(1000),
    )
    .unwrap();
    let c = clusters(&layout);
    assert!(
        c.windows(2).all(|w| w[0] >= w[1]),
        "RTL clusters should decrease visually"
    );
    assert!(c.iter().all(|&b| b < text.len()));
}

#[test]
fn huge_paragraph_lays_out_within_width() {
    let reg = registry();
    let word = "lorem ipsum dolor sit amet consectetur ";
    let text = word.repeat(2000); // About 80 KB of text.
    let width = Fx::from_int(480);
    let layout = layout_paragraph(&reg, &text, &style("Noto Sans"), &[], width).unwrap();

    assert!(layout.lines.len() > 1000);
    for (i, line) in layout.lines.iter().enumerate() {
        let w = line.runs.iter().fold(Fx::ZERO, |a, r| a + r.width());
        assert!(
            w <= width || line.runs.iter().map(|r| r.glyphs.len()).sum::<usize>() == 1,
            "line {i} overflows"
        );
    }
    // Lines cover the text in order with no gaps.
    let mut expect = 0;
    for line in &layout.lines {
        assert_eq!(line.text_range.start, expect, "gap before line");
        expect = line.text_range.end;
    }
    assert_eq!(expect, text.len());
}
