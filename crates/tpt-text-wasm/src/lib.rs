//! JavaScript bindings for tpt-text, built with `wasm-pack build crates/tpt-text-wasm`.
//!
//! Every function takes the font as bytes, so the page decides where fonts come
//! from. Sizes and widths are whole pixels; positions are returned in pixels.

use tpt_text::hit_test::hit_test;
use tpt_text::svg::layout_to_svg;
use tpt_text::{layout_text, FontRegistry, Fx, ParagraphLayout, TextStyle};
use wasm_bindgen::prelude::*;

/// Lays out `text` in the font given as bytes, with the family name read from the font.
fn lay_out(
    font: &[u8],
    text: &str,
    size_px: i32,
    width_px: i32,
) -> Result<ParagraphLayout, JsValue> {
    let registry = FontRegistry::new();
    let face = registry
        .load_font(font.to_vec(), 0)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let style = TextStyle::new(face.family(), Fx::from_int(size_px));
    layout_text(&registry, text, &style, Fx::from_int(width_px))
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Renders the paragraph as an SVG document string.
#[wasm_bindgen]
pub fn layout_svg(font: &[u8], text: &str, size_px: i32, width_px: i32) -> Result<String, JsValue> {
    Ok(layout_to_svg(&lay_out(font, text, size_px, width_px)?))
}

/// The layout digest as 16 hex digits. Equal digests mean identical positions.
#[wasm_bindgen]
pub fn layout_digest(
    font: &[u8],
    text: &str,
    size_px: i32,
    width_px: i32,
) -> Result<String, JsValue> {
    Ok(format!(
        "{:016x}",
        lay_out(font, text, size_px, width_px)?.digest()
    ))
}

/// The byte offset of the caret nearest to the point `(x, y)` in pixels.
#[wasm_bindgen]
pub fn caret_at(
    font: &[u8],
    text: &str,
    size_px: i32,
    width_px: i32,
    x: i32,
    y: i32,
) -> Result<usize, JsValue> {
    let layout = lay_out(font, text, size_px, width_px)?;
    hit_test(&layout, Fx::from_int(x), Fx::from_int(y))
        .map(|caret| caret.byte_index)
        .ok_or_else(|| JsValue::from_str("empty layout"))
}
