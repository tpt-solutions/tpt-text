#![allow(clippy::unwrap_used)]
//! Golden layouts: the full glyph output of representative paragraphs.
//!
//! Regenerate with `TPT_GOLDEN_UPDATE=1 cargo test -p tpt-text --test golden`.

use std::path::PathBuf;

use serde::Serialize;
use tpt_golden::check;
use tpt_text::{layout_paragraph, FontRegistry, Fx, ParagraphLayout, TextStyle};

#[derive(Serialize)]
struct GoldenLine {
    baseline_y: i32,
    height: i32,
    glyphs: Vec<(u32, i32, usize)>,
}

#[derive(Serialize)]
struct GoldenParagraph {
    total_height: i32,
    lines: Vec<GoldenLine>,
}

fn to_golden(layout: &ParagraphLayout) -> GoldenParagraph {
    GoldenParagraph {
        total_height: layout.total_height.to_raw(),
        lines: layout
            .lines
            .iter()
            .map(|line| GoldenLine {
                baseline_y: line.baseline_y.to_raw(),
                height: line.height.to_raw(),
                glyphs: line
                    .runs
                    .iter()
                    .flat_map(|r| r.glyphs.iter())
                    .map(|g| (g.glyph_id, g.x_advance.to_raw(), g.cluster_index))
                    .collect(),
            })
            .collect(),
    }
}

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(format!("{name}.json"))
}

fn registry() -> FontRegistry {
    let reg = FontRegistry::new();
    reg.load_from_memory(
        include_bytes!("fonts/NotoSans-Regular.ttf").to_vec(),
        0,
        "Noto Sans",
        400,
        false,
    )
    .unwrap();
    reg.load_from_memory(
        include_bytes!("fonts/NotoSansArabic-Regular.ttf").to_vec(),
        0,
        "Noto Sans Arabic",
        400,
        false,
    )
    .unwrap();
    reg
}

#[test]
fn latin_wrapping_paragraph() {
    let reg = registry();
    let style = TextStyle::new("Noto Sans", Fx::from_int(14));
    let layout = layout_paragraph(
        &reg,
        "Deterministic layout means the same glyphs and positions on every platform, \
         so PDFs and CAD drawings agree byte for byte.",
        &style,
        &[],
        Fx::from_int(240),
    )
    .unwrap();
    check(golden_path("latin_wrap"), &to_golden(&layout));
}

#[test]
fn arabic_with_numbers() {
    let reg = registry();
    let style = TextStyle::new("Noto Sans Arabic", Fx::from_int(16));
    let layout = layout_paragraph(
        &reg,
        "\u{0628}\u{062A}\u{0628} 2026 \u{062A}\u{0628}",
        &style,
        &[],
        Fx::from_int(400),
    )
    .unwrap();
    check(golden_path("arabic_numbers"), &to_golden(&layout));
}

#[test]
fn mixed_bidi_fallback_justified() {
    use tpt_text::{Align, FallbackResolver, LayoutOptions};

    let reg = registry();
    let resolver = FallbackResolver::new().with_defaults(&["Noto Sans Arabic"]);
    let options = LayoutOptions {
        align: Align::Justify,
        fallback: Some(&resolver),
        ..LayoutOptions::default()
    };
    // Latin text with an Arabic phrase that the Latin face cannot draw, so
    // the Arabic falls back to the second face and the lines are justified.
    let text = "Invoice \u{0628}\u{062A}\u{0628} total 2026 is due on receipt of this notice.";
    let style = TextStyle::new("Noto Sans", Fx::from_int(14));
    let layout =
        tpt_text::layout_paragraph_with(&reg, text, &style, &[], Fx::from_int(260), &options)
            .unwrap();
    check(golden_path("mixed_bidi_fallback"), &to_golden(&layout));
}

#[test]
fn emoji_fallback_with_joiners() {
    use tpt_text::{FallbackResolver, LayoutOptions};

    let reg = registry();
    reg.load_from_memory(
        include_bytes!("fonts/NotoEmoji-Regular.ttf").to_vec(),
        0,
        "Noto Emoji",
        400,
        false,
    )
    .unwrap();
    let resolver = FallbackResolver::new().with_defaults(&["Noto Emoji"]);
    let options = LayoutOptions {
        fallback: Some(&resolver),
        ..LayoutOptions::default()
    };
    // Emoji, a ZWJ sequence, and a ZWNJ inside Arabic.
    let text = "ok \u{1F600} family \u{1F468}\u{200D}\u{1F469} \u{0628}\u{200C}\u{062A} end";
    let style = TextStyle::new("Noto Sans", Fx::from_int(14));
    let layout =
        tpt_text::layout_paragraph_with(&reg, text, &style, &[], Fx::from_int(300), &options)
            .unwrap();
    check(golden_path("emoji_joiners"), &to_golden(&layout));
}

#[test]
fn long_paragraph_wrap() {
    let reg = registry();
    let style = TextStyle::new("Noto Sans", Fx::from_int(12));
    let text = "lorem ipsum dolor sit amet consectetur ".repeat(40);
    let layout = layout_paragraph(&reg, &text, &style, &[], Fx::from_int(360)).unwrap();
    check(golden_path("long_paragraph"), &to_golden(&layout));
}
