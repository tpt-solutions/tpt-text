//! Glyph outlines, scaled to layout units and flipped to a y-down screen.
//!
//! Coordinates are converted with integer arithmetic (after truncating the
//! font's own `f32` coordinates to 1/256 of a unit), so the same glyph gives
//! the same path on every platform.

use alloc::vec::Vec;

use rustybuzz::ttf_parser::{GlyphId, OutlineBuilder};

use crate::font::FontFace;
use crate::fx::Fx;

/// One drawing command of a glyph outline, in layout units, y down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathSegment {
    /// Starts a new contour at `(x, y)`.
    MoveTo(Fx, Fx),
    /// Straight line to `(x, y)`.
    LineTo(Fx, Fx),
    /// Quadratic curve through control point `(x1, y1)` to `(x, y)`.
    QuadTo(Fx, Fx, Fx, Fx),
    /// Cubic curve through control points `(x1, y1)` and `(x2, y2)` to `(x, y)`.
    CubicTo(Fx, Fx, Fx, Fx, Fx, Fx),
    /// Closes the current contour.
    Close,
}

/// Converts font units to layout units: `units * size / units_per_em`.
struct Scale {
    size_raw: i64,
    upem: i64,
}

impl Scale {
    /// `value` is in font units with 1/256 fractional precision.
    fn convert(&self, value: f32) -> Fx {
        let fixed = (value * 256.0) as i64;
        let raw = fixed * self.size_raw / (self.upem * 256);
        Fx::from_raw(raw.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32)
    }
}

struct Collector {
    scale: Scale,
    out: Vec<PathSegment>,
}

impl Collector {
    fn point(&self, x: f32, y: f32) -> (Fx, Fx) {
        // Font y grows upwards; layout y grows downwards.
        (self.scale.convert(x), self.scale.convert(-y))
    }
}

impl OutlineBuilder for Collector {
    fn move_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.point(x, y);
        self.out.push(PathSegment::MoveTo(x, y));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.point(x, y);
        self.out.push(PathSegment::LineTo(x, y));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (x1, y1) = self.point(x1, y1);
        let (x, y) = self.point(x, y);
        self.out.push(PathSegment::QuadTo(x1, y1, x, y));
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (x1, y1) = self.point(x1, y1);
        let (x2, y2) = self.point(x2, y2);
        let (x, y) = self.point(x, y);
        self.out.push(PathSegment::CubicTo(x1, y1, x2, y2, x, y));
    }

    fn close(&mut self) {
        self.out.push(PathSegment::Close);
    }
}

impl FontFace {
    /// The outline of `glyph_id` at `size`, with its origin on the baseline.
    ///
    /// Returns an empty path for glyphs without an outline, such as spaces.
    pub fn glyph_outline(&self, glyph_id: u32, size: Fx) -> Vec<PathSegment> {
        let mut collector = Collector {
            scale: Scale {
                size_raw: i64::from(size.to_raw()),
                upem: i64::from(self.units_per_em()),
            },
            out: Vec::new(),
        };
        let glyph = u16::try_from(glyph_id).ok().map(GlyphId);
        if let Some(glyph) = glyph {
            // Glyphs without outlines simply produce no segments.
            let _ = self.parsed().outline_glyph(glyph, &mut collector);
        }
        collector.out
    }
}
