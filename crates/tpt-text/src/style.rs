//! Text style description.

use alloc::string::String;
use alloc::vec::Vec;
use core::ops::Range;

use crate::fx::Fx;

/// An abstract colour reference. `tpt-color` resolves it during rendering;
/// this crate never interprets it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ColorId(pub u32);

/// An OpenType feature setting such as `liga` or `smcp=1`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FontFeature {
    /// Four-byte feature tag.
    pub tag: [u8; 4],
    /// Feature value (`1` enables, `0` disables, other values select alternates).
    pub value: u32,
}

/// A variable-font axis setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FontVariation {
    /// Four-byte axis tag, such as `wght`.
    pub tag: [u8; 4],
    /// Axis value in design units, such as `700` for `wght`.
    pub value: Fx,
}

/// Visual style applied to a range of text.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TextStyle {
    /// Preferred family name, matched case-insensitively.
    pub family: String,
    /// CSS-style weight, 100 to 900.
    pub weight: u16,
    /// Whether an italic face is requested.
    pub italic: bool,
    /// Font size in layout units.
    pub size: Fx,
    /// Extra advance added after every glyph.
    pub letter_spacing: Fx,
    /// OpenType features applied during shaping.
    pub features: Vec<FontFeature>,
    /// Abstract colour reference.
    pub color: ColorId,
    /// Variable-font axis settings, such as `wght` or `wdth`.
    pub variations: Vec<FontVariation>,
}

impl TextStyle {
    /// Creates a regular-weight, upright style with no spacing or features.
    pub fn new(family: impl Into<String>, size: Fx) -> TextStyle {
        TextStyle {
            family: family.into(),
            weight: 400,
            italic: false,
            size,
            letter_spacing: Fx::ZERO,
            features: Vec::new(),
            color: ColorId::default(),
            variations: Vec::new(),
        }
    }

    /// Sets the CSS-style weight.
    pub fn with_weight(mut self, weight: u16) -> TextStyle {
        self.weight = weight;
        self
    }

    /// Requests an italic face.
    pub fn with_italic(mut self, italic: bool) -> TextStyle {
        self.italic = italic;
        self
    }

    /// Sets the extra advance added after every glyph.
    pub fn with_letter_spacing(mut self, spacing: Fx) -> TextStyle {
        self.letter_spacing = spacing;
        self
    }

    /// Adds an OpenType feature, such as `*b"liga"` with value `0` to disable it.
    pub fn with_feature(mut self, tag: [u8; 4], value: u32) -> TextStyle {
        self.features.push(FontFeature { tag, value });
        self
    }

    /// Sets a variable-font axis, such as `*b"wght"` with value `700`.
    pub fn with_variation(mut self, tag: [u8; 4], value: Fx) -> TextStyle {
        self.variations.push(FontVariation { tag, value });
        self
    }

    /// Sets the abstract colour reference.
    pub fn with_color(mut self, color: ColorId) -> TextStyle {
        self.color = color;
        self
    }
}

/// A style applied to a byte range of the paragraph text.
///
/// Bytes not covered by any span use the base style passed to the layout
/// call. Ranges must lie on UTF-8 character boundaries.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StyleSpan {
    /// Byte range within the paragraph text.
    pub range: Range<usize>,
    /// Style for that range.
    pub style: TextStyle,
}
