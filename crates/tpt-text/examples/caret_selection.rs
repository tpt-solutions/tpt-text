//! Editor-style queries on a laid-out paragraph: where a click lands, where a
//! caret sits for a byte offset, and which rectangles a selection covers.
//!
//! Usage:
//!   cargo run -p tpt-text --example caret_selection

use tpt_text::hit_test::{caret_for_byte, hit_test, selection_rects};
use tpt_text::{layout_text, FontRegistry, Fx, TextStyle};

const LATIN: &[u8] = include_bytes!("../tests/fonts/NotoSans-Regular.ttf");

fn main() {
    let registry = FontRegistry::new();
    registry
        .load_font(LATIN.to_vec(), 0)
        .expect("vendored font");
    let style = TextStyle::new("Noto Sans", Fx::from_int(16));
    let text = "Click anywhere in this sentence to place the caret.";
    let layout =
        layout_text(&registry, text, &style, Fx::from_int(200)).expect("font is registered");

    // A click at (x, y) in layout units gives the nearest caret.
    let click = hit_test(&layout, Fx::from_int(40), Fx::from_int(20)).expect("non-empty layout");
    println!(
        "click at (40, 20): byte {} on line {} at x={}",
        click.byte_index, click.line, click.x
    );

    // The caret for a known byte offset, such as after a keystroke.
    let caret = caret_for_byte(&layout, 6).expect("offset is inside the text");
    println!(
        "caret at byte 6: line {} x={} height={}",
        caret.line, caret.x, caret.height
    );

    // Rectangles for a selection that may span several lines.
    let rects = selection_rects(&layout, 6..20);
    println!("selection 6..20 covers {} rectangle(s):", rects.len());
    for r in rects {
        println!(
            "  x={} y={} width={} height={}",
            r.x, r.y, r.width, r.height
        );
    }
}
