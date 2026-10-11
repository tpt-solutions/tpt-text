//! Glyph shaping through rustybuzz.

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ops::Range;

use rustybuzz::ttf_parser::Tag;
use rustybuzz::{Feature, UnicodeBuffer};

use crate::font::FontFace;
use crate::fx::{scale_round, Fx};
use crate::style::TextStyle;

/// Text direction of a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// Left to right.
    LeftToRight,
    /// Right to left.
    RightToLeft,
}

/// One shaped glyph with positions already scaled to layout units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShapedGlyph {
    /// Glyph index within the font.
    pub glyph_id: u32,
    /// Horizontal advance, including letter spacing.
    pub x_advance: Fx,
    /// Vertical advance (zero for horizontal text).
    pub y_advance: Fx,
    /// Horizontal offset from the pen position.
    pub x_offset: Fx,
    /// Vertical offset from the pen position.
    pub y_offset: Fx,
    /// Byte index in the paragraph text where this glyph's cluster starts.
    pub cluster_index: usize,
}

/// A sequence of glyphs from one font and one style.
#[derive(Clone, Debug)]
pub struct ShapedRun {
    /// Font the glyphs were shaped with.
    pub font: Arc<FontFace>,
    /// Style the run was shaped with.
    pub style: TextStyle,
    /// Glyphs in visual order.
    pub glyphs: Vec<ShapedGlyph>,
    /// Direction the run was shaped in.
    pub direction: Direction,
}

impl ShapedRun {
    /// Sum of the horizontal advances of all glyphs in the run.
    pub fn width(&self) -> Fx {
        self.glyphs
            .iter()
            .fold(Fx::ZERO, |acc, g| acc + g.x_advance)
    }
}

/// Shapes `text[range]` with `font` and returns the positioned run.
///
/// `cluster_index` values are absolute byte indices into `text`.
pub(crate) fn shape_run(
    font: &Arc<FontFace>,
    text: &str,
    range: Range<usize>,
    style: &TextStyle,
    direction: Direction,
) -> ShapedRun {
    let mut buffer = UnicodeBuffer::new();
    buffer.push_str(&text[range.clone()]);
    buffer.set_direction(match direction {
        Direction::LeftToRight => rustybuzz::Direction::LeftToRight,
        Direction::RightToLeft => rustybuzz::Direction::RightToLeft,
    });

    let features: Vec<Feature> = style
        .features
        .iter()
        .map(|f| Feature::new(Tag::from_bytes(&f.tag), f.value, ..))
        .collect();

    let face = font.hb_face();
    let output = rustybuzz::shape(&face, &features, buffer);

    let upem = i64::from(font.units_per_em());
    let size = i64::from(style.size.to_raw());
    let to_fx = |units: i32| Fx::from_raw(clamp_i32(scale_round(i64::from(units), size, upem)));

    let glyphs = output
        .glyph_infos()
        .iter()
        .zip(output.glyph_positions())
        .map(|(info, pos)| ShapedGlyph {
            glyph_id: info.glyph_id,
            x_advance: to_fx(pos.x_advance) + style.letter_spacing,
            y_advance: to_fx(pos.y_advance),
            x_offset: to_fx(pos.x_offset),
            y_offset: to_fx(pos.y_offset),
            cluster_index: range.start + info.cluster as usize,
        })
        .collect();

    ShapedRun {
        font: Arc::clone(font),
        style: style.clone(),
        glyphs,
        direction,
    }
}

fn clamp_i32(value: i64) -> i32 {
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}
