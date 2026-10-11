#![allow(clippy::unwrap_used)]
//! Integration tests for shaping and line breaking, using vendored OFL fonts.

use std::sync::Arc;

use tpt_text::font::FontQuery;
use tpt_text::{
    layout_paragraph, FontProvider, FontRegistry, Fx, LayoutError, ParagraphLayout, ShapedRun,
    StyleSpan, TextStyle,
};

const LATIN: &[u8] = include_bytes!("fonts/NotoSans-Regular.ttf");
const ARABIC: &[u8] = include_bytes!("fonts/NotoSansArabic-Regular.ttf");

fn registry() -> FontRegistry {
    let reg = FontRegistry::new();
    reg.load_from_memory(LATIN.to_vec(), 0, "Noto Sans", 400, false)
        .unwrap();
    reg.load_from_memory(ARABIC.to_vec(), 0, "Noto Sans Arabic", 400, false)
        .unwrap();
    reg
}

fn style(family: &str) -> TextStyle {
    TextStyle::new(family, Fx::from_int(16))
}

/// Concatenates the glyph clusters of every run on every line, in order.
fn glyph_clusters(layout: &ParagraphLayout) -> Vec<usize> {
    layout
        .lines
        .iter()
        .flat_map(|line| line.runs.iter())
        .flat_map(|run| run.glyphs.iter().map(|g| g.cluster_index))
        .collect()
}

fn line_width(runs: &[ShapedRun]) -> Fx {
    runs.iter().fold(Fx::ZERO, |acc, r| acc + r.width())
}

#[test]
fn wraps_latin_text_within_max_width() {
    let reg = registry();
    let text = "The quick brown fox jumps over the lazy dog and keeps running far away.";
    let max = Fx::from_int(160);

    let layout = layout_paragraph(&reg, text, &style("Noto Sans"), &[], max).unwrap();

    assert!(layout.lines.len() > 1, "expected wrapping");
    for line in &layout.lines {
        // Every line fits, unless it is a single over-wide word.
        let glyph_count: usize = line.runs.iter().map(|r| r.glyphs.len()).sum();
        if glyph_count > 1 {
            assert!(line_width(&line.runs) <= max, "line overflows max width");
        }
    }
}

#[test]
fn every_byte_position_appears_in_order() {
    let reg = registry();
    let text = "alpha beta gamma delta epsilon zeta eta theta";
    let layout = layout_paragraph(&reg, text, &style("Noto Sans"), &[], Fx::from_int(90)).unwrap();

    let clusters = glyph_clusters(&layout);
    assert_eq!(clusters.first(), Some(&0));
    assert!(
        clusters.windows(2).all(|w| w[0] <= w[1]),
        "clusters not monotonic"
    );
    assert!(clusters.iter().all(|&c| c < text.len()));
}

#[test]
fn layout_is_deterministic() {
    let reg = registry();
    let text = "Determinism matters for reproducible PDFs and CAD drawings.";
    let max = Fx::from_int(120);

    let a = layout_paragraph(&reg, text, &style("Noto Sans"), &[], max).unwrap();
    let b = layout_paragraph(&reg, text, &style("Noto Sans"), &[], max).unwrap();

    assert_eq!(a.total_height, b.total_height);
    assert_eq!(glyph_clusters(&a), glyph_clusters(&b));
    let ids = |l: &ParagraphLayout| -> Vec<u32> {
        l.lines
            .iter()
            .flat_map(|x| x.runs.iter())
            .flat_map(|r| r.glyphs.iter().map(|g| g.glyph_id))
            .collect()
    };
    assert_eq!(ids(&a), ids(&b));
}

#[test]
fn mandatory_newline_forces_a_break() {
    let reg = registry();
    let layout = layout_paragraph(
        &reg,
        "first\nsecond",
        &style("Noto Sans"),
        &[],
        Fx::from_int(10_000),
    )
    .unwrap();
    assert_eq!(layout.lines.len(), 2);
}

#[test]
fn empty_text_yields_one_line_with_height() {
    let reg = registry();
    let layout = layout_paragraph(&reg, "", &style("Noto Sans"), &[], Fx::from_int(100)).unwrap();
    assert_eq!(layout.lines.len(), 1);
    assert!(layout.lines[0].runs.is_empty());
    assert!(layout.total_height > Fx::ZERO);
}

#[test]
fn zero_width_limit_terminates_one_glyph_per_line() {
    let reg = registry();
    let layout = layout_paragraph(&reg, "abc", &style("Noto Sans"), &[], Fx::ZERO).unwrap();
    assert_eq!(layout.lines.len(), 3);
}

#[test]
fn arabic_is_contextually_shaped() {
    let reg = registry();
    let text = "\u{0628}\u{062A}\u{0628}"; // beh teh beh: joins into connected forms.
    let layout = layout_paragraph(
        &reg,
        text,
        &style("Noto Sans Arabic"),
        &[],
        Fx::from_int(1000),
    )
    .unwrap();

    let font = reg.load(&FontQuery::new("Noto Sans Arabic")).unwrap();
    let nominal: Vec<u32> = text.chars().map(|c| font.glyph_index(c).unwrap()).collect();
    let shaped: Vec<(u32, usize)> = layout
        .lines
        .iter()
        .flat_map(|l| l.runs.iter())
        .flat_map(|r| r.glyphs.iter().map(|g| (g.glyph_id, g.cluster_index)))
        .collect();

    // The paragraph is right-to-left, so glyphs come out in reverse logical
    // order. Noto Sans Arabic draws each dot as its own mark glyph, so every
    // letter yields a base glyph plus marks, all on that letter's cluster.
    let clusters: Vec<usize> = shaped.iter().map(|&(_, c)| c).collect();
    assert_eq!(clusters, [4, 4, 2, 2, 0, 0]);
    // The first letter (cluster 0) must be in a contextual form, not isolated.
    let first_letter: Vec<u32> = shaped
        .iter()
        .filter(|&&(_, c)| c == 0)
        .map(|&(g, _)| g)
        .collect();
    assert!(
        !first_letter.contains(&nominal[0]),
        "expected contextual forms"
    );
}

#[test]
fn style_span_changes_font_size() {
    let reg = registry();
    let text = "small BIG small";
    let spans = [StyleSpan {
        range: 6..9,
        style: TextStyle::new("Noto Sans", Fx::from_int(40)),
    }];
    let layout = layout_paragraph(
        &reg,
        text,
        &style("Noto Sans"),
        &spans,
        Fx::from_int(10_000),
    )
    .unwrap();

    let sizes: Vec<Fx> = layout.lines[0].runs.iter().map(|r| r.style.size).collect();
    assert_eq!(
        sizes,
        [Fx::from_int(16), Fx::from_int(40), Fx::from_int(16)]
    );
    assert!(layout.lines[0].height > Fx::from_int(40));
}

#[test]
fn missing_family_is_an_error() {
    let reg = registry();
    let err =
        layout_paragraph(&reg, "x", &style("No Such Font"), &[], Fx::from_int(100)).unwrap_err();
    assert_eq!(err, LayoutError::MissingFont("No Such Font".into()));
}

#[test]
fn loading_identical_bytes_returns_the_same_handle() {
    let reg = FontRegistry::new();
    let a = reg
        .load_from_memory(LATIN.to_vec(), 0, "Noto Sans", 400, false)
        .unwrap();
    let b = reg
        .load_from_memory(LATIN.to_vec(), 0, "Noto Sans", 400, false)
        .unwrap();
    assert!(Arc::ptr_eq(&a, &b));
}

#[test]
fn load_prefers_matching_weight_and_slant() {
    let reg = FontRegistry::new();
    let regular = reg
        .load_from_memory(LATIN.to_vec(), 0, "Noto Sans", 400, false)
        .unwrap();
    let bold = reg
        .load_from_memory(ARABIC.to_vec(), 0, "Noto Sans", 700, false)
        .unwrap();

    let found = reg
        .load(&FontQuery {
            family: "noto sans".into(),
            weight: 700,
            italic: false,
        })
        .unwrap();
    assert!(Arc::ptr_eq(&found, &bold));
    let found = reg.load(&FontQuery::new("Noto Sans")).unwrap();
    assert!(Arc::ptr_eq(&found, &regular));
}

#[test]
fn fallback_chain_uses_configured_families() {
    let reg = registry();
    reg.set_fallback_chain(tpt_text::Script::Arabic, &["Noto Sans Arabic"]);
    let chain = reg.fallback_chain(tpt_text::Script::Arabic, "Noto Sans");
    assert_eq!(chain.len(), 1);
    assert_eq!(chain[0].family(), "Noto Sans Arabic");
    assert!(reg
        .fallback_chain(tpt_text::Script::Latin, "Noto Sans")
        .is_empty());
}

#[test]
fn opentype_feature_changes_glyphs() {
    use tpt_text::FontFeature;

    let reg = registry();
    let text = "0123456789";
    let plain = style("Noto Sans");
    let mut oldstyle = style("Noto Sans");
    oldstyle.features.push(FontFeature {
        tag: *b"onum",
        value: 1,
    });

    let ids = |s: &TextStyle| -> Vec<u32> {
        let layout = layout_paragraph(&reg, text, s, &[], Fx::from_int(10_000)).unwrap();
        layout
            .lines
            .iter()
            .flat_map(|l| l.runs.iter())
            .flat_map(|r| r.glyphs.iter().map(|g| g.glyph_id))
            .collect()
    };

    assert_ne!(ids(&plain), ids(&oldstyle), "onum should swap digit glyphs");
}
