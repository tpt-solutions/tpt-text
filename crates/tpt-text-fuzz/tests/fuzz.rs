#![allow(clippy::unwrap_used)]
//! Random text and width inputs must never panic or hang the layout engine.

use tpt_text::{layout_paragraph, FontRegistry, Fx, TextStyle};
use tpt_text_fuzz::{random_text, Lcg};

const ITERATIONS: u32 = 300;

#[test]
fn random_paragraphs_do_not_panic() {
    let reg = FontRegistry::new();
    for (family, bytes) in [
        (
            "Noto Sans",
            &include_bytes!("../../tpt-text/tests/fonts/NotoSans-Regular.ttf")[..],
        ),
        (
            "Noto Sans Arabic",
            &include_bytes!("../../tpt-text/tests/fonts/NotoSansArabic-Regular.ttf")[..],
        ),
    ] {
        reg.load_from_memory(bytes.to_vec(), 0, family, 400, false)
            .unwrap();
    }

    let mut rng = Lcg::new(0x5EED_2026);
    for i in 0..ITERATIONS {
        let text = random_text(&mut rng, 120);
        // Include zero and very small widths, which force one glyph per line.
        let width = Fx::from_raw(rng.below(400 * 65536) as i32 - 1000);
        let family = if i % 2 == 0 {
            "Noto Sans"
        } else {
            "Noto Sans Arabic"
        };
        let style = TextStyle::new(family, Fx::from_int(12));

        let layout = layout_paragraph(&reg, &text, &style, &[], width)
            .unwrap_or_else(|e| panic!("iteration {i}: {e} for {text:?}"));

        // Every line keeps at least one glyph unless the text is empty.
        if !text.is_empty() {
            for line in &layout.lines {
                assert!(
                    !line.runs.is_empty() || text.contains('\n'),
                    "empty line for {text:?}"
                );
            }
        }
    }
}
