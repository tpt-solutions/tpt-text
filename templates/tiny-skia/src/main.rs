//! Starter renderer: lays out text with tpt-text and fills the glyph outlines
//! with tiny-skia, writing a PNG.
//!
//! Usage:
//!   cargo run -- <font.ttf> "<text>" [out.png]

use std::env;
use std::fs;
use std::process::ExitCode;

use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Transform};
use tpt_text::{layout_text, FontRegistry, Fx, ParagraphLayout, PathSegment, TextStyle};

const WIDTH: u32 = 480;

/// Adds every glyph of `layout` to one path, in layout units (y down).
fn glyph_path(layout: &ParagraphLayout) -> Option<tiny_skia::Path> {
    let mut builder = PathBuilder::new();
    for line in &layout.lines {
        let mut pen = line.x_offset;
        for run in &line.runs {
            for glyph in &run.glyphs {
                let dx = (pen + glyph.x_offset).to_f32();
                let dy = (line.baseline_y + glyph.y_offset).to_f32();
                for segment in run.font.glyph_outline(glyph.glyph_id, run.style.size) {
                    match segment {
                        PathSegment::MoveTo(x, y) => builder.move_to(x.to_f32() + dx, y.to_f32() + dy),
                        PathSegment::LineTo(x, y) => builder.line_to(x.to_f32() + dx, y.to_f32() + dy),
                        PathSegment::QuadTo(x1, y1, x, y) => builder.quad_to(
                            x1.to_f32() + dx,
                            y1.to_f32() + dy,
                            x.to_f32() + dx,
                            y.to_f32() + dy,
                        ),
                        PathSegment::CubicTo(x1, y1, x2, y2, x, y) => builder.cubic_to(
                            x1.to_f32() + dx,
                            y1.to_f32() + dy,
                            x2.to_f32() + dx,
                            y2.to_f32() + dy,
                            x.to_f32() + dx,
                            y.to_f32() + dy,
                        ),
                        PathSegment::Close => builder.close(),
                    }
                }
                pen += glyph.x_advance;
            }
        }
    }
    builder.finish()
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let (Some(font_path), Some(text)) = (args.get(1), args.get(2)) else {
        eprintln!("usage: tpt-text-tiny-skia <font.ttf> <text> [out.png]");
        return ExitCode::FAILURE;
    };
    let out_path = args.get(3).map_or("layout.png", String::as_str);

    let bytes = match fs::read(font_path) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("cannot read {font_path}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let registry = FontRegistry::new();
    let face = match registry.load_font(bytes, 0) {
        Ok(face) => face,
        Err(e) => {
            eprintln!("not a usable font: {e}");
            return ExitCode::FAILURE;
        }
    };

    let style = TextStyle::new(face.family(), Fx::from_int(28));
    let layout = match layout_text(&registry, text, &style, Fx::from_int(WIDTH as i32 - 32)) {
        Ok(layout) => layout,
        Err(e) => {
            eprintln!("layout failed: {e}");
            return ExitCode::FAILURE;
        }
    };

    let height = (layout.total_height.to_f32().ceil() as u32 + 32).max(64);
    let Some(mut pixmap) = Pixmap::new(WIDTH, height) else {
        eprintln!("cannot allocate a {WIDTH}x{height} pixmap");
        return ExitCode::FAILURE;
    };
    pixmap.fill(Color::WHITE);

    if let Some(path) = glyph_path(&layout) {
        let mut paint = Paint::default();
        paint.set_color(Color::BLACK);
        paint.anti_alias = true;
        // Offset by the margin so the text does not touch the edge.
        let transform = Transform::from_translate(16.0, 16.0);
        pixmap.fill_path(&path, &paint, FillRule::Winding, transform, None);
    }

    match pixmap.save_png(out_path) {
        Ok(()) => {
            println!("wrote {out_path} ({WIDTH}x{height}), layout digest {:016x}", layout.digest());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("cannot write {out_path}: {e}");
            ExitCode::FAILURE
        }
    }
}
