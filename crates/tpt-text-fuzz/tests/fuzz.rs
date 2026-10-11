#![allow(clippy::unwrap_used)]
//! Randomised robustness run: random text, font subsets, layout options,
//! widths, hit-tests and selections must never panic or hang.
//!
//! The generator is seeded, so a failure reproduces with the same iteration
//! number.

use tpt_text::hit_test::{caret_for_byte, hit_test, selection_rects};
use tpt_text::{
    layout_paragraph, layout_paragraph_with, Align, FallbackResolver, FontRegistry, Fx, Hyphenator,
    LayoutError, LayoutOptions, ParagraphLayout, TextStyle,
};
use tpt_text_fuzz::{random_text, Lcg};

/// Iterations per run. Set `TPT_FUZZ_ITERATIONS` for a longer soak test.
fn iterations() -> u32 {
    std::env::var("TPT_FUZZ_ITERATIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300)
}

const FONTS: &[(&str, &[u8])] = &[
    (
        "Noto Sans",
        include_bytes!("../../tpt-text/tests/fonts/NotoSans-Regular.ttf"),
    ),
    (
        "Noto Sans Arabic",
        include_bytes!("../../tpt-text/tests/fonts/NotoSansArabic-Regular.ttf"),
    ),
    (
        "Noto Emoji",
        include_bytes!("../../tpt-text/tests/fonts/NotoEmoji-Regular.ttf"),
    ),
];

/// Splits every word after the first letter, as a stand-in hyphenator.
struct EveryThird;

impl Hyphenator for EveryThird {
    fn break_points(&self, word: &str) -> Vec<usize> {
        word.char_indices()
            .map(|(i, _)| i)
            .filter(|&i| i > 0 && i % 3 == 0)
            .collect()
    }
}

/// Registers a random non-empty subset of the fonts. Returns the registry and
/// the families that were registered.
fn random_font_subset(rng: &mut Lcg) -> (FontRegistry, Vec<&'static str>) {
    let reg = FontRegistry::new();
    let mut families = Vec::new();
    loop {
        for (family, bytes) in FONTS {
            if rng.below(2) == 0 {
                reg.load_from_memory(bytes.to_vec(), 0, *family, 400, false)
                    .unwrap();
                families.push(*family);
            }
        }
        if !families.is_empty() {
            return (reg, families);
        }
    }
}

fn check_layout(layout: &ParagraphLayout, text: &str, rng: &mut Lcg) {
    // Line byte ranges must be ordered, and every position must be a boundary.
    for w in layout.lines.windows(2) {
        assert!(w[0].text_range.start <= w[1].text_range.start);
    }
    for line in &layout.lines {
        assert!(line.text_range.end <= text.len());
        assert!(text.is_char_boundary(line.text_range.start));
        assert!(text.is_char_boundary(line.text_range.end));
    }

    // Hit-testing and selection must not panic on arbitrary input.
    let x = Fx::from_raw(rng.below(600 * 65536) as i32 - 100 * 65536);
    let y = Fx::from_raw(rng.below(300 * 65536) as i32 - 50 * 65536);
    let _ = hit_test(layout, x, y);
    let byte = if text.is_empty() {
        0
    } else {
        rng.below(text.len() as u32 + 1) as usize
    };
    let _ = caret_for_byte(layout, byte);
    let start = byte.min(text.len());
    let end = (start + rng.below(40) as usize).min(text.len());
    let _ = selection_rects(layout, start..end);
}

#[test]
fn random_layouts_do_not_panic() {
    let mut rng = Lcg::new(0x5EED_2026);
    for i in 0..iterations() {
        let (reg, families) = random_font_subset(&mut rng);
        let text = random_text(&mut rng, 120);
        let width = Fx::from_raw(rng.below(400 * 65536) as i32 - 1000);

        // Options: each one is switched on at random.
        let resolver = FallbackResolver::new().with_defaults(&["Noto Emoji", "Noto Sans Arabic"]);
        let align = match rng.below(4) {
            0 => Align::Left,
            1 => Align::Right,
            2 => Align::Center,
            _ => Align::Justify,
        };
        let options = LayoutOptions {
            align,
            max_lines: (rng.below(2) == 0).then(|| rng.below(5) as usize),
            ellipsis: rng.below(2) == 0,
            tab_stop: (rng.below(2) == 0).then(|| Fx::from_int(rng.below(80) as i32)),
            line_spacing: (rng.below(2) == 0)
                .then(|| Fx::from_f32(0.5 + rng.below(20) as f32 / 10.0)),
            fallback: (rng.below(2) == 0).then_some(&resolver),
            hyphenator: (rng.below(2) == 0).then_some(&EveryThird as &dyn Hyphenator),
            ..LayoutOptions::default()
        };

        let family = families[rng.below(families.len() as u32) as usize];
        let style = TextStyle::new(family, Fx::from_int(12));

        let outcome = layout_paragraph_with(&reg, &text, &style, &[], width, &options);
        match outcome {
            Ok(layout) => check_layout(&layout, &text, &mut rng),
            Err(LayoutError::MissingFont(_)) => {
                // Only possible if the chosen family was not registered.
                assert!(
                    !families.contains(&family),
                    "iteration {i}: unexpected missing font"
                );
            }
        }
    }
}

#[test]
fn default_options_still_lay_out_every_registered_family() {
    let reg = FontRegistry::new();
    for (family, bytes) in FONTS {
        reg.load_from_memory(bytes.to_vec(), 0, *family, 400, false)
            .unwrap();
    }
    let mut rng = Lcg::new(7);
    for _ in 0..50 {
        let text = random_text(&mut rng, 60);
        let style = TextStyle::new("Noto Sans", Fx::from_int(12));
        let width = Fx::from_int(1 + rng.below(300) as i32);
        let layout = layout_paragraph(&reg, &text, &style, &[], width).unwrap();
        check_layout(&layout, &text, &mut rng);
    }
}
