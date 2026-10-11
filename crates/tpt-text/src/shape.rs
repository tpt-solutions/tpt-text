//! Glyph shaping through rustybuzz.

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ops::Range;

use rustybuzz::ttf_parser::Tag;
use rustybuzz::{Feature, UnicodeBuffer, Variation};

use crate::font::FontFace;
use crate::fx::{scale_round, Fx};
use crate::style::TextStyle;

/// Text direction of a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum Direction {
    /// Left to right.
    LeftToRight,
    /// Right to left.
    RightToLeft,
}

/// One shaped glyph with positions already scaled to layout units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
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
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ShapedRun {
    /// Font the glyphs were shaped with.
    #[cfg_attr(
        feature = "serde",
        serde(serialize_with = "crate::font::serialize_font")
    )]
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

    let variations: Vec<Variation> = style
        .variations
        .iter()
        .map(|v| Variation {
            tag: Tag::from_bytes(&v.tag),
            value: v.value.to_f32(),
        })
        .collect();
    let variation_face;
    let face = if variations.is_empty() {
        font.hb_face()
    } else {
        variation_face = font.hb_face_with_variations(&variations);
        &variation_face
    };
    let output = rustybuzz::shape(face, &features, buffer);

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

/// Key for one shaping result: the font, the exact text, and every setting
/// that changes the glyphs.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ShapeKey {
    font: usize,
    text: alloc::string::String,
    right_to_left: bool,
    size: i32,
    letter_spacing: i32,
    features: Vec<([u8; 4], u32)>,
    variations: Vec<([u8; 4], i32)>,
}

/// Cache of shaped glyphs, shared by the paragraphs of a document.
///
/// Entries hold their font, so the pointer used in the key stays valid while
/// the entry exists. Glyph clusters are stored relative to the shaped text, so
/// a result can be reused at any position in a paragraph.
#[derive(Default)]
pub struct ShapeCache {
    entries: alloc::collections::BTreeMap<ShapeKey, (Arc<FontFace>, Vec<ShapedGlyph>)>,
}

impl ShapeCache {
    /// Number of distinct shaping results held.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True if nothing is cached.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl core::fmt::Debug for ShapeCache {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ShapeCache")
            .field("entries", &self.entries.len())
            .finish()
    }
}

/// Like [`shape_run`], but reuses a cached result for the same text and style.
pub(crate) fn shape_run_cached(
    cache: &mut ShapeCache,
    font: &Arc<FontFace>,
    text: &str,
    range: Range<usize>,
    style: &TextStyle,
    direction: Direction,
) -> ShapedRun {
    let key = ShapeKey {
        font: Arc::as_ptr(font) as usize,
        text: alloc::string::String::from(&text[range.clone()]),
        right_to_left: direction == Direction::RightToLeft,
        size: style.size.to_raw(),
        letter_spacing: style.letter_spacing.to_raw(),
        features: style.features.iter().map(|f| (f.tag, f.value)).collect(),
        variations: style
            .variations
            .iter()
            .map(|v| (v.tag, v.value.to_raw()))
            .collect(),
    };
    let start = range.start;
    let entry = cache.entries.entry(key).or_insert_with(|| {
        let run = shape_run(font, text, range.clone(), style, direction);
        let relative = run
            .glyphs
            .into_iter()
            .map(|mut g| {
                g.cluster_index -= start;
                g
            })
            .collect();
        (Arc::clone(font), relative)
    });
    ShapedRun {
        font: Arc::clone(font),
        style: style.clone(),
        glyphs: entry
            .1
            .iter()
            .map(|g| ShapedGlyph {
                cluster_index: g.cluster_index + start,
                ..*g
            })
            .collect(),
        direction,
    }
}
