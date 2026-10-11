# Migrating from cosmic-text or parley

This guide maps the concepts of two popular Rust text libraries onto `tpt-text`.
It describes concepts rather than exact signatures, because those libraries
change between releases. Check their current documentation for the details.

## What is different

`tpt-text` makes three choices that the others do not:

- **Integer layout.** All positions and widths are `Fx`, a 16.16 fixed-point
  number. Layout is identical on every platform, so you can compare digests.
- **Caller-supplied fonts.** The core loads fonts from bytes you provide. System
  font discovery is an opt-in feature (`fontdb`), not the default.
- **Abstract colour.** A `ColorId` is carried through layout and never resolved
  to a colour. Your renderer maps it.

It is also narrower: it lays out and measures text and answers caret queries.
It does not own an editor, a scroll model, a glyph cache or a rasteriser. The
tiny-skia template shows the rendering part.

## Concept map

### cosmic-text

| cosmic-text | tpt-text |
| --- | --- |
| `FontSystem` | `FontRegistry` (fonts from bytes) or `SystemFonts` with `fontdb` |
| `Attrs` (family, weight, style) | `TextStyle` with `family`, `with_weight`, `with_italic` |
| `Metrics` (font size, line height) | `TextStyle::size` and `LayoutOptions::line_spacing` |
| `Buffer` with a set width | the `max_width` argument to a layout call |
| `Buffer::set_text` with spans | `layout_paragraph_with(..., spans, ...)` with `StyleSpan` |
| `Buffer::layout_runs` | `ParagraphLayout::lines`, each with `runs` and positions |
| `Shaping::Advanced` | always on; rustybuzz shapes every run |
| `SwashCache` glyph rasterising | not included; use the tiny-skia template |
| `Editor` cursor and selection | `hit_test`, `caret_for_byte`, `selection_rects` |

Things to check when you port:

- cosmic-text rasterises; `tpt-text` returns positions and outlines. Draw the
  outlines with `FontFace::glyph_outline`, or use `layout_to_svg`.
- Line height in cosmic-text is an absolute value. In `tpt-text`,
  `line_spacing` is a multiple of the font's natural height.
- Positions are fixed-point. Convert with `Fx::to_f32()` where your renderer
  needs floats.

### parley

| parley | tpt-text |
| --- | --- |
| `FontContext` | `FontRegistry` or `SystemFonts` |
| `LayoutContext` | not needed; the shaping cache lives in `Document` |
| `RangedBuilder` with style properties | `StyleSpan` list with `TextStyle` per range |
| `Layout` | `ParagraphLayout` |
| `Layout::break_all_lines(max_advance)` | the `max_width` argument |
| `Alignment::Start` / `End` | `Align::Start` / `Align::End` |
| `Cursor` and `Selection` | `hit_test` and `selection_rects` |
| `BreakReason` | `explain_line_breaks` |
| Brush or colour in styles | `ColorId` (abstract, resolved by your renderer) |

Things to check when you port:

- parley's layout is a tree of styled spans with one builder. `tpt-text` takes
  the paragraph text plus a list of spans, which is closer to a flat run list.
- Mixed-direction text is supported in both. `tpt-text` reports the paragraph
  direction on `ParagraphLayout::direction` and applies rule L1 per line.
- For several paragraphs, use `Document` rather than building one large text.

## A short port

Before, with a typical cosmic-text setup:

```rust
// Pseudocode for the shape of the calls, not exact signatures.
let mut buffer = Buffer::new(&mut font_system, Metrics::new(16.0, 20.0));
buffer.set_size(&mut font_system, 300.0, 400.0);
buffer.set_text(&mut font_system, text, Attrs::new().family(Family::Name("Noto Sans")), Shaping::Advanced);
buffer.shape_until_scroll(&mut font_system, false);
```

After, with `tpt-text`:

```rust
use tpt_text::{layout_text, FontRegistry, Fx, TextStyle};

let registry = FontRegistry::new();
let face = registry.load_font(std::fs::read("NotoSans-Regular.ttf")?, 0)?;
let style = TextStyle::new(face.family(), Fx::from_int(16));
let layout = layout_text(&registry, text, &style, Fx::from_int(300))?;
```

Then draw `layout.lines` yourself, or call `tpt_text::svg::layout_to_svg`.

## Behaviour to expect

- A paragraph that ends with a newline gets an empty last line. This gives
  editors a caret position after the newline.
- A single word wider than the width wraps mid-word, unless `ellipsis` is set,
  in which case it is shown as one truncated line.
- Trailing spaces hang. They do not count towards alignment or justification.
- Unsupported characters are reported in `missing_glyphs`. They are drawn as the
  font's `.notdef` glyph, not skipped silently.

## Tests

Keep your existing visual tests. Add a digest check next to them, so a change
in layout shows up even when the rendered image looks the same:

```rust
assert_eq!(layout.digest(), 0x0123_4567_89ab_cdef);
```

Use `tpt_text_snapshot::check` for a readable diff when the digest changes.
