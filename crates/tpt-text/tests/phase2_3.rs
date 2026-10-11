//! Tests for Phase 2 (fallback, hyphenation, tabs, Bidi) and Phase 3
//! (alignment, justification, spacing, truncation, hit-testing).

#![allow(clippy::unwrap_used)]

use tpt_text::hit_test::{caret_for_byte, hit_test, selection_rects};
use tpt_text::{
    layout_paragraph, layout_paragraph_with, Align, Direction, FallbackResolver, FontProvider,
    FontQuery, FontRegistry, Fx, Hyphenator, LayoutOptions, ParagraphLayout, TextStyle,
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

fn latin_glyph(reg: &FontRegistry, c: char) -> u32 {
    reg.load(&FontQuery::new("Noto Sans"))
        .unwrap()
        .glyph_index(c)
        .unwrap()
}

/// Hyphenates "international" after "inter" and nothing else.
struct Inter;

impl Hyphenator for Inter {
    fn break_points(&self, word: &str) -> Vec<usize> {
        if word == "international" {
            vec![5]
        } else {
            Vec::new()
        }
    }
}

/// (glyph id, cluster, advance) for every glyph, in visual order.
fn all_glyphs(layout: &ParagraphLayout) -> Vec<(u32, usize, Fx)> {
    layout
        .lines
        .iter()
        .flat_map(|l| l.runs.iter())
        .flat_map(|r| {
            r.glyphs
                .iter()
                .map(|g| (g.glyph_id, g.cluster_index, g.x_advance))
        })
        .collect()
}

fn line_width(layout: &ParagraphLayout, line: usize) -> Fx {
    layout.lines[line]
        .runs
        .iter()
        .fold(Fx::ZERO, |acc, r| acc + r.width())
}

/// Width of a line without its trailing space, which hangs past the line end.
fn visible_line_width(layout: &ParagraphLayout, line: usize) -> Fx {
    let end = layout.lines[line].text_range.end;
    let hanging = (end > 0 && layout.text.as_bytes()[end - 1] == b' ').then_some(end - 1);
    layout.lines[line]
        .runs
        .iter()
        .flat_map(|r| r.glyphs.iter())
        .filter(|g| Some(g.cluster_index) != hanging)
        .fold(Fx::ZERO, |acc, g| acc + g.x_advance)
}

#[test]
fn fallback_uses_second_font_for_missing_glyph() {
    let reg = registry();
    let resolver = FallbackResolver::new().with_defaults(&["Noto Sans Arabic"]);
    let opts = LayoutOptions {
        fallback: Some(&resolver),
        ..LayoutOptions::default()
    };
    let text = "ab\u{0628}";
    let layout =
        layout_paragraph_with(&reg, text, &style(), &[], Fx::from_int(1000), &opts).unwrap();

    let families: Vec<&str> = layout.lines[0]
        .runs
        .iter()
        .map(|r| r.font.family())
        .collect();
    assert_eq!(families, ["Noto Sans", "Noto Sans Arabic"]);
}

#[test]
fn missing_everywhere_falls_back_to_primary_notdef() {
    let reg = registry();
    let text = "\u{4E00}"; // CJK ideograph, not in either test font.
    let layout = layout_paragraph(&reg, text, &style(), &[], Fx::from_int(1000)).unwrap();
    let glyphs = all_glyphs(&layout);
    assert_eq!(glyphs.len(), 1);
    assert_eq!(glyphs[0].0, 0, "expected .notdef");
    assert_eq!(layout.lines[0].runs[0].font.family(), "Noto Sans");
}

#[test]
fn hyphenator_splits_word_and_draws_hyphen() {
    let reg = registry();
    let hyphen = latin_glyph(&reg, '-');
    let opts = LayoutOptions {
        hyphenator: Some(&Inter),
        ..LayoutOptions::default()
    };
    let text = "international";
    // Wide enough for "inter-" but not for "international".
    let layout = layout_paragraph_with(&reg, text, &style(), &[], Fx::from_int(60), &opts).unwrap();

    assert!(layout.lines.len() >= 2, "expected a hyphenated break");
    let first = &layout.lines[0];
    assert_eq!(first.text_range, 0..5);
    let last_glyph = first.runs.last().unwrap().glyphs.last().unwrap();
    assert_eq!(last_glyph.glyph_id, hyphen);
}

#[test]
fn soft_hyphen_is_invisible_unless_broken() {
    let reg = registry();
    let hyphen = latin_glyph(&reg, '-');
    let text = "inter\u{AD}national";

    // Plenty of room: nothing is drawn for the soft hyphen.
    let wide = layout_paragraph(&reg, text, &style(), &[], Fx::from_int(1000)).unwrap();
    assert_eq!(wide.lines.len(), 1);
    assert!(all_glyphs(&wide).iter().all(|g| g.0 != hyphen));

    // Narrow: the break is taken there and a hyphen is drawn.
    let narrow = layout_paragraph(&reg, text, &style(), &[], Fx::from_int(60)).unwrap();
    assert!(narrow.lines.len() >= 2);
    let last = narrow.lines[0].runs.last().unwrap().glyphs.last().unwrap();
    assert_eq!(last.glyph_id, hyphen);
}

#[test]
fn explicit_newline_produces_no_visible_glyph() {
    let reg = registry();
    let layout = layout_paragraph(&reg, "a\nb", &style(), &[], Fx::from_int(1000)).unwrap();
    assert_eq!(layout.lines.len(), 2);
    let newline_glyphs = all_glyphs(&layout)
        .iter()
        .filter(|g| layout.text.as_bytes()[g.1] == b'\n')
        .count();
    assert_eq!(newline_glyphs, 0);
}

#[test]
fn tab_snaps_to_next_stop() {
    let reg = registry();
    let opts = LayoutOptions {
        tab_stop: Some(Fx::from_int(50)),
        ..LayoutOptions::default()
    };
    let layout =
        layout_paragraph_with(&reg, "a\tb", &style(), &[], Fx::from_int(1000), &opts).unwrap();
    let glyphs = all_glyphs(&layout);
    let b_index = glyphs.iter().position(|g| g.1 == 2).unwrap();
    let x_b = glyphs[..b_index].iter().fold(Fx::ZERO, |acc, g| acc + g.2);
    assert_eq!(x_b.to_raw() % (50 << 16), 0, "b should start on a tab stop");
    assert!(x_b >= Fx::from_int(50));
}

#[test]
fn right_and_center_alignment_offsets() {
    let reg = registry();
    let text = "short";
    let width = Fx::from_int(300);
    for align in [Align::Right, Align::Center] {
        let opts = LayoutOptions {
            align,
            ..LayoutOptions::default()
        };
        let layout = layout_paragraph_with(&reg, text, &style(), &[], width, &opts).unwrap();
        let free = width - line_width(&layout, 0);
        let expected = match align {
            Align::Right => free,
            _ => Fx::from_raw(free.to_raw() / 2),
        };
        assert_eq!(layout.lines[0].x_offset, expected, "{align:?}");
    }
}

#[test]
fn justify_fills_non_final_lines() {
    let reg = registry();
    let text = "one two three four five six seven eight nine ten eleven twelve";
    let width = Fx::from_int(200);
    let opts = LayoutOptions {
        align: Align::Justify,
        ..LayoutOptions::default()
    };
    let layout = layout_paragraph_with(&reg, text, &style(), &[], width, &opts).unwrap();
    assert!(layout.lines.len() > 2);
    for i in 0..layout.lines.len() - 1 {
        assert_eq!(
            visible_line_width(&layout, i),
            width,
            "line {i} not justified"
        );
    }
    // The final line keeps its natural width.
    assert!(line_width(&layout, layout.lines.len() - 1) < width);
}

#[test]
fn line_spacing_scales_height() {
    let reg = registry();
    let plain = layout_paragraph(&reg, "x", &style(), &[], Fx::from_int(100)).unwrap();
    let opts = LayoutOptions {
        line_spacing: Some(Fx::from_int(2)),
        ..LayoutOptions::default()
    };
    let doubled =
        layout_paragraph_with(&reg, "x", &style(), &[], Fx::from_int(100), &opts).unwrap();
    assert_eq!(
        doubled.lines[0].height,
        plain.lines[0].height * Fx::from_int(2)
    );
}

#[test]
fn max_lines_truncates_with_ellipsis() {
    let reg = registry();
    let text = "word word word word word word word word word word word word";
    let opts = LayoutOptions {
        max_lines: Some(2),
        ellipsis: true,
        ..LayoutOptions::default()
    };
    let width = Fx::from_int(120);
    let layout = layout_paragraph_with(&reg, text, &style(), &[], width, &opts).unwrap();
    assert_eq!(layout.lines.len(), 2);
    let ellipsis = latin_glyph(&reg, '\u{2026}');
    let final_glyph = layout.lines[1].runs.last().unwrap().glyphs.last().unwrap();
    assert_eq!(final_glyph.glyph_id, ellipsis);
    assert!(line_width(&layout, 1) <= width);
}

#[test]
fn right_to_left_direction_option() {
    let reg = registry();
    let opts = LayoutOptions {
        direction: Some(Direction::RightToLeft),
        ..LayoutOptions::default()
    };
    // Latin inside an RTL paragraph is an embedded LTR run (UAX #9 level 2),
    // so "ab" keeps its order.
    let latin = layout_paragraph_with(&reg, "ab", &style(), &[], Fx::from_int(500), &opts).unwrap();
    let clusters: Vec<usize> = all_glyphs(&latin).iter().map(|g| g.1).collect();
    assert_eq!(clusters, [0, 1]);
    assert_eq!(latin.lines[0].runs[0].direction, Direction::LeftToRight);

    // Arabic in the same paragraph is genuinely right-to-left: the later
    // letter is drawn on the left.
    let arabic_text = "\u{0628}\u{062A}";
    let arabic_style = TextStyle::new("Noto Sans Arabic", Fx::from_int(16));
    let arabic = layout_paragraph_with(
        &reg,
        arabic_text,
        &arabic_style,
        &[],
        Fx::from_int(500),
        &opts,
    )
    .unwrap();
    assert_eq!(all_glyphs(&arabic)[0].1, 2);
    assert_eq!(arabic.lines[0].runs[0].direction, Direction::RightToLeft);
}

#[test]
fn hit_test_and_caret_round_trip() {
    let reg = registry();
    let text = "hello world";
    let layout = layout_paragraph(&reg, text, &style(), &[], Fx::from_int(1000)).unwrap();

    let at_start = hit_test(&layout, Fx::ZERO, Fx::ZERO).unwrap();
    assert_eq!(at_start.byte_index, 0);

    let end_x = line_width(&layout, 0);
    let at_end = hit_test(&layout, end_x + Fx::from_int(50), Fx::ZERO).unwrap();
    assert_eq!(at_end.byte_index, text.len());

    let caret = caret_for_byte(&layout, 6).unwrap();
    assert_eq!(caret.line, 0);
    let back = hit_test(&layout, caret.x, Fx::ZERO).unwrap();
    assert_eq!(back.byte_index, 6);
}

#[test]
fn selection_rects_cover_selected_text() {
    let reg = registry();
    let text = "hello world";
    let layout = layout_paragraph(&reg, text, &style(), &[], Fx::from_int(1000)).unwrap();
    let rects = selection_rects(&layout, 6..11);
    assert_eq!(rects.len(), 1);
    assert!(rects[0].x > Fx::ZERO);
    assert!(rects[0].x + rects[0].width <= line_width(&layout, 0) + Fx::ONE);
    assert!(selection_rects(&layout, 3..3).is_empty());
}

#[test]
fn bidi_selection_of_a_single_rtl_word_is_one_rect() {
    let reg = registry();
    let text = "\u{0628}\u{062A}\u{0628}";
    let arabic = TextStyle::new("Noto Sans Arabic", Fx::from_int(16));
    let layout = layout_paragraph(&reg, text, &arabic, &[], Fx::from_int(500)).unwrap();
    assert_eq!(selection_rects(&layout, 0..text.len()).len(), 1);
}
