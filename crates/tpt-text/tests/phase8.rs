//! Tests for the Phase 8 features: outlines and SVG, metadata and collections,
//! builders, multi-paragraph documents, the layout digest and line-break
//! explanations.

#![allow(clippy::unwrap_used)]

use tpt_text::svg::layout_to_svg;
use tpt_text::{
    explain_line_breaks, layout_paragraph_with, layout_text, Align, Document, FontFace,
    FontRegistry, Fx, LayoutOptions, PathSegment, StyleSpan, TextStyle,
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

fn style() -> TextStyle {
    TextStyle::new("Noto Sans", Fx::from_int(16))
}

#[test]
fn glyph_outline_starts_on_the_baseline_and_closes() {
    let face = FontFace::from_bytes(LATIN.to_vec(), 0, "Noto Sans", 400, false).unwrap();
    let a = face.glyph_index('A').unwrap();
    let path = face.glyph_outline(a, Fx::from_int(100));
    assert!(matches!(path.first(), Some(PathSegment::MoveTo(..))));
    assert_eq!(path.last(), Some(&PathSegment::Close));
    // Glyph 'A' sits on the baseline, so no point lies below it (y grows down).
    for seg in &path {
        if let PathSegment::MoveTo(_, y) | PathSegment::LineTo(_, y) = seg {
            assert!(y.to_raw() <= 0, "outline point below the baseline");
        }
    }
    assert!(face
        .glyph_outline(face.glyph_index(' ').unwrap(), Fx::from_int(100))
        .is_empty());
}

#[test]
fn svg_export_is_deterministic_and_contains_paths() {
    let reg = registry();
    let layout = layout_text(&reg, "Hello, world", &style(), Fx::from_int(200)).unwrap();
    let svg = layout_to_svg(&layout);
    assert!(svg.starts_with("<svg"));
    assert_eq!(
        svg.matches("<path").count(),
        11,
        "one path per non-space glyph"
    );
    assert_eq!(svg, layout_to_svg(&layout));
}

#[test]
fn font_metadata_is_read_from_the_file() {
    let reg = FontRegistry::new();
    let face = reg.load_font(LATIN.to_vec(), 0).unwrap();
    assert_eq!(face.family(), "Noto Sans");
    assert_eq!(face.weight(), 400);
    assert!(!face.is_italic());
}

#[test]
fn plain_font_file_is_a_one_face_collection() {
    let reg = FontRegistry::new();
    let faces = reg.load_collection(LATIN.to_vec()).unwrap();
    assert_eq!(faces.len(), 1);
    assert_eq!(faces[0].index(), 0);
}

#[test]
fn builders_set_the_same_fields_as_struct_literals() {
    let built = TextStyle::new("Noto Sans", Fx::from_int(12))
        .with_weight(700)
        .with_italic(true)
        .with_letter_spacing(Fx::ONE)
        .with_feature(*b"liga", 0)
        .with_variation(*b"wght", Fx::from_int(600))
        .with_color(tpt_text::ColorId(7));
    assert_eq!(built.weight, 700);
    assert!(built.italic);
    assert_eq!(built.features[0].tag, *b"liga");
    assert_eq!(built.variations[0].value, Fx::from_int(600));
    assert_eq!(built.color, tpt_text::ColorId(7));

    let options = LayoutOptions::default()
        .align(Align::Center)
        .max_lines(2)
        .ellipsis(true);
    assert_eq!(options.align, Align::Center);
    assert_eq!(options.max_lines, Some(2));
    assert!(format!("{options:?}").contains("Center"));
    let copy = options.clone();
    assert!(copy.ellipsis);
}

#[test]
fn static_font_ignores_variation_axes() {
    let reg = registry();
    let plain = layout_text(&reg, "Variable", &style(), Fx::from_int(300)).unwrap();
    let varied = layout_text(
        &reg,
        "Variable",
        &style().with_variation(*b"wght", Fx::from_int(700)),
        Fx::from_int(300),
    )
    .unwrap();
    assert_eq!(plain.digest(), varied.digest());
}

#[test]
fn digest_matches_equal_layouts_and_differs_otherwise() {
    let reg = registry();
    let text = "The quick brown fox jumps over the lazy dog";
    let a = layout_text(&reg, text, &style(), Fx::from_int(150)).unwrap();
    let b = layout_text(&reg, text, &style(), Fx::from_int(150)).unwrap();
    let c = layout_text(&reg, text, &style(), Fx::from_int(300)).unwrap();
    assert_eq!(a.digest(), b.digest());
    assert_ne!(a.digest(), c.digest());
}

#[test]
fn explanations_name_the_rule_for_each_line_end() {
    let reg = registry();
    let layout = layout_text(&reg, "hello brave new world", &style(), Fx::from_int(80)).unwrap();
    let reasons = explain_line_breaks(&layout);
    assert_eq!(reasons.len(), layout.lines.len() - 1);
    // Breaks after spaces are UAX #14 rule LB18.
    assert!(reasons.iter().all(|r| r.rule == "LB18"), "{reasons:?}");
}

#[test]
fn document_relays_out_only_changed_paragraphs() {
    let reg = registry();
    let mut doc = Document::new(Fx::from_int(8));
    doc.push("First paragraph of the document.", vec![]);
    doc.push("Second one.", vec![]);
    doc.push("Third and last.", vec![]);
    let width = Fx::from_int(200);
    let options = LayoutOptions::default();

    let first = doc.layout(&reg, &style(), width, &options).unwrap();
    assert_eq!(doc.last_relayout_count(), 3);
    assert_eq!(first.paragraphs.len(), 3);
    assert!(first.paragraphs[1].top > first.paragraphs[0].top);

    let again = doc.layout(&reg, &style(), width, &options).unwrap();
    assert_eq!(doc.last_relayout_count(), 0);
    assert_eq!(again.total_height, first.total_height);

    doc.set(1, "Second one, now longer than before.", vec![]);
    doc.layout(&reg, &style(), width, &options).unwrap();
    assert_eq!(doc.last_relayout_count(), 1);

    doc.layout(&reg, &style(), Fx::from_int(400), &options)
        .unwrap();
    assert_eq!(
        doc.last_relayout_count(),
        3,
        "a new width relays out everything"
    );
}

#[test]
fn document_keeps_span_styles() {
    let reg = registry();
    let mut doc = Document::new(Fx::ZERO);
    let bold = TextStyle::new("Noto Sans", Fx::from_int(16)).with_weight(700);
    doc.push(
        "plain bold",
        vec![StyleSpan {
            range: 6..10,
            style: bold,
        }],
    );
    let layout = doc
        .layout(&reg, &style(), Fx::from_int(500), &LayoutOptions::default())
        .unwrap();
    assert_eq!(layout.paragraphs[0].layout.lines[0].runs.len(), 2);
}

#[test]
fn paragraph_direction_is_reported() {
    let reg = registry();
    let arabic = TextStyle::new("Noto Sans Arabic", Fx::from_int(16));
    let layout = layout_text(&reg, "\u{0628}\u{062A}", &arabic, Fx::from_int(200)).unwrap();
    assert_eq!(layout.direction, tpt_text::Direction::RightToLeft);
    let ltr = layout_paragraph_with(
        &reg,
        "abc",
        &style(),
        &[],
        Fx::from_int(200),
        &LayoutOptions::default(),
    )
    .unwrap();
    assert_eq!(ltr.direction, tpt_text::Direction::LeftToRight);
}

#[test]
fn shape_cache_reuses_identical_runs_without_changing_output() {
    let reg = registry();
    let width = Fx::from_int(180);
    let mut doc = Document::new(Fx::ZERO);
    doc.push("the same words", vec![]);
    doc.push("the same words", vec![]);
    let cached = doc
        .layout(&reg, &style(), width, &LayoutOptions::default())
        .unwrap();
    assert_eq!(
        doc.shape_cache_len(),
        1,
        "identical runs should share one entry"
    );

    let fresh = layout_text(&reg, "the same words", &style(), width).unwrap();
    assert_eq!(cached.paragraphs[0].layout.digest(), fresh.digest());
    assert_eq!(cached.paragraphs[1].layout.digest(), fresh.digest());
}
