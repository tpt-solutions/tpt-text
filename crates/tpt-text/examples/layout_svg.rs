//! Writes a laid-out paragraph as SVG.
//!
//! Usage:
//!   cargo run -p tpt-text --example layout_svg -- <font.ttf> "<text>" > out.svg

use std::env;
use std::fs;
use std::process::ExitCode;

use tpt_text::svg::layout_to_svg;
use tpt_text::{layout_text, FontRegistry, Fx, TextStyle};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let (Some(font_path), Some(text)) = (args.get(1), args.get(2)) else {
        eprintln!("usage: layout_svg <font.ttf> <text>");
        return ExitCode::FAILURE;
    };
    let font = match fs::read(font_path) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("cannot read {font_path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    // The family name comes from the font file itself.
    let registry = FontRegistry::new();
    let face = match registry.load_font(font, 0) {
        Ok(face) => face,
        Err(e) => {
            eprintln!("not a usable font: {e}");
            return ExitCode::FAILURE;
        }
    };
    let style = TextStyle::new(face.family(), Fx::from_int(24));
    match layout_text(&registry, text, &style, Fx::from_int(480)) {
        Ok(layout) => {
            print!("{}", layout_to_svg(&layout));
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("layout failed: {e}");
            ExitCode::FAILURE
        }
    }
}
