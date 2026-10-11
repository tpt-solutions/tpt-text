//! SVG export of a laid-out paragraph.
//!
//! Every glyph becomes one `<path>` in layout units. Numbers are written with
//! two decimals using integer arithmetic, so the same layout always produces
//! the same bytes.

use alloc::string::String;
use core::fmt::Write;

use crate::fx::Fx;
use crate::layout::ParagraphLayout;
use crate::outline::PathSegment;

/// Renders `layout` as an SVG document of size `max_width` by `total_height`.
///
/// Glyphs are filled black. Colour references are abstract in `tpt-text`, so
/// the caller recolours the output if needed.
pub fn layout_to_svg(layout: &ParagraphLayout) -> String {
    let mut out = String::new();
    let width = fmt_fx(layout.max_width);
    let height = fmt_fx(layout.total_height);
    let _ = write!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">"
    );
    out.push('\n');
    for line in &layout.lines {
        let mut pen = line.x_offset;
        for run in &line.runs {
            for g in &run.glyphs {
                let x = pen + g.x_offset;
                let y = line.baseline_y + g.y_offset;
                let path = run.font.glyph_outline(g.glyph_id, run.style.size);
                if !path.is_empty() {
                    out.push_str("<path d=\"");
                    write_path(&mut out, &path, x, y);
                    out.push_str("\" fill=\"#000\"/>\n");
                }
                pen += g.x_advance;
            }
        }
    }
    out.push_str("</svg>\n");
    out
}

/// Writes `path` translated by `(dx, dy)` as SVG path data.
fn write_path(out: &mut String, path: &[PathSegment], dx: Fx, dy: Fx) {
    let point = |out: &mut String, x: Fx, y: Fx| {
        out.push(' ');
        out.push_str(&fmt_fx(x + dx));
        out.push(' ');
        out.push_str(&fmt_fx(y + dy));
    };
    for segment in path {
        match *segment {
            PathSegment::MoveTo(x, y) => {
                out.push('M');
                point(out, x, y);
            }
            PathSegment::LineTo(x, y) => {
                out.push('L');
                point(out, x, y);
            }
            PathSegment::QuadTo(x1, y1, x, y) => {
                out.push('Q');
                point(out, x1, y1);
                point(out, x, y);
            }
            PathSegment::CubicTo(x1, y1, x2, y2, x, y) => {
                out.push('C');
                point(out, x1, y1);
                point(out, x2, y2);
                point(out, x, y);
            }
            PathSegment::Close => out.push('Z'),
        }
    }
}

/// Formats a fixed-point value with two decimals, rounding half away from zero.
pub(crate) fn fmt_fx(value: Fx) -> String {
    let raw = i64::from(value.to_raw());
    let magnitude = (raw.abs() * 100 + 32_768) / 65_536;
    let sign = if raw < 0 && magnitude != 0 { "-" } else { "" };
    let whole = magnitude / 100;
    let frac = magnitude % 100;
    alloc::format!("{sign}{whole}.{frac:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_two_decimals() {
        assert_eq!(fmt_fx(Fx::from_int(3)), "3.00");
        assert_eq!(fmt_fx(Fx::from_raw(32_768)), "0.50");
        assert_eq!(fmt_fx(Fx::from_raw(-65_536 * 2 - 1)), "-2.00");
        assert_eq!(fmt_fx(Fx::from_raw(-6554)), "-0.10");
    }
}
