//! Example consumer: lay out a paragraph from a font file and print the lines.
//!
//! Usage:
//!   cargo run -p tpt-text --example layout_demo -- <font.ttf> "<text>" [width]

use std::env;
use std::fs;
use std::process::ExitCode;

use tpt_text::hit_test::caret_for_byte;
use tpt_text::{layout_paragraph_with, Align, FontRegistry, Fx, LayoutOptions, TextStyle};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: layout_demo <font.ttf> <text> [width]");
        return ExitCode::FAILURE;
    }
    let font_bytes = match fs::read(&args[1]) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("cannot read {}: {e}", args[1]);
            return ExitCode::FAILURE;
        }
    };
    let width = args
        .get(3)
        .and_then(|w| w.parse::<i32>().ok())
        .map_or(Fx::from_int(240), Fx::from_int);

    let registry = FontRegistry::new();
    if let Err(e) = registry.load_from_memory(font_bytes, 0, "Demo", 400, false) {
        eprintln!("cannot parse font: {e}");
        return ExitCode::FAILURE;
    }

    let style = TextStyle::new("Demo", Fx::from_int(14));
    let options = LayoutOptions {
        align: Align::Justify,
        ..LayoutOptions::default()
    };
    let layout = match layout_paragraph_with(&registry, &args[2], &style, &[], width, &options) {
        Ok(layout) => layout,
        Err(e) => {
            eprintln!("layout failed: {e}");
            return ExitCode::FAILURE;
        }
    };

    for (i, line) in layout.lines.iter().enumerate() {
        let glyphs: usize = line.runs.iter().map(|r| r.glyphs.len()).sum();
        println!(
            "line {i:>2}: top={:>6.2} glyphs={glyphs:>3} bytes={:?}",
            line.top.to_f32(),
            line.text_range
        );
    }
    if let Some(caret) = caret_for_byte(&layout, args[2].len()) {
        println!(
            "end caret: line {} at x={:.2}",
            caret.line,
            caret.x.to_f32()
        );
    }
    println!("total height: {:.2}", layout.total_height.to_f32());
    ExitCode::SUCCESS
}
