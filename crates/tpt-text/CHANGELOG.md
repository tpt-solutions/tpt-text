# Changelog

All notable changes to `tpt-text` are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this crate follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Deterministic fixed-point layout pipeline with `Fx` values and integer arithmetic.
- `TextStyle`, `StyleSpan`, `FontFeature`, and `ColorId` for styling.
- `FontFace`, `FontQuery`, `FontProvider`, and a thread-safe `FontRegistry`
  with an `Arc<FontFace>` cache. Fonts load from memory.
- `fontdb`-backed `SystemFonts` behind the `fontdb` feature.
- Bidi-aware itemization by script, style, and direction, with visual run reordering.
- rustybuzz shaping for LTR and RTL runs, including OpenType features and
  cluster-to-byte mapping in `ShapedGlyph` and `ShapedRun`.
- `FallbackResolver` with caller-configured per-script chains and defaults,
  `.notdef` handling, and run splitting on fallback.
- UAX #14 line breaking through `tpt-text-linebreak`, soft hyphens, and the
  `Hyphenator` trait.
- Explicit newlines and tab stops.
- Line height and leading control, left, right, center, and full justification,
  ellipsis and max-lines truncation.
- `ParagraphLayout` and `LayoutLine` output types, and `layout_paragraph` and
  `layout_paragraph_with` entry points.
- Hit testing: point to byte index and caret, byte index to caret point, and
  selection rectangles across lines and Bidi runs.
- `serde` feature for `Fx` and layout output.
- `no_std` + `alloc` core, verified for `thumbv7em-none-eabihf` and `wasm32-unknown-unknown`.
- Golden, Unicode Bidi conformance, and benchmark tests.

[Unreleased]: https://github.com/tpt-solutions/tpt-text/commits/master/crates/tpt-text
