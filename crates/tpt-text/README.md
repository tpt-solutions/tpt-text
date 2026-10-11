# tpt-text

Deterministic, pure-Rust text shaping and layout for the TPT ecosystem.

`tpt-text` takes styled Unicode text and a maximum width and returns a fully
laid-out paragraph: positioned glyphs, line boxes, alignment, justification,
truncation, and cursor hit-testing. It orchestrates existing pure-Rust
primitives behind a fixed-point, integer-only pipeline, so the same input
produces byte-identical output on every platform. That matters for reproducible
PDFs and CAD drawings.

## Features

- **Shaping** through [rustybuzz](https://crates.io/crates/rustybuzz), with
  OpenType features, ligatures, contextual alternates, and complex-script
  positioning (for example Arabic joining and Devanagari reordering).
- **Bidirectional text** through `unicode-bidi`: visual run reordering for
  mixed LTR and RTL paragraphs.
- **Itemization** by script, direction, and style.
- **Line breaking** under UAX #14, using the in-tree `tpt-text-linebreak`
  crate, with soft hyphens, a pluggable `Hyphenator` trait, explicit newlines,
  and tab stops.
- **Fallback chains** configured by the caller per script, with `.notdef`
  detection and run splitting.
- **Layout**: left, right, center, and full justification (word and letter
  spacing, deterministic rounding), line height and leading control,
  ellipsis and max-lines truncation.
- **Hit testing**: point to byte index and caret, byte index to caret point,
  and selection rectangles that handle multi-line and Bidi text.
- **Fonts**: an in-memory `FontRegistry` (thread-safe, `Arc<FontFace>` cache)
  and a `FontProvider` trait for custom sources. System font discovery is
  available behind the `fontdb` feature.
- **Fixed-point metrics**: the public `Fx` type avoids float drift across
  platforms.
- **`no_std` + `alloc`** core. The `std` feature is only needed for OS font
  discovery.

## Quick start

```rust,no_run
use tpt_text::{layout_paragraph, FontRegistry, Fx, TextStyle};

let font = std::fs::read("NotoSans-Regular.ttf").expect("font file");
let registry = FontRegistry::new();
registry
    .load_from_memory(font, 0, "Noto Sans", 400, false)
    .expect("valid font");

let style = TextStyle::new("Noto Sans", Fx::from_int(12));
let layout = layout_paragraph(&registry, "Hello, world", &style, &[], Fx::from_int(200))
    .expect("font is registered");
for line in &layout.lines {
    println!("line at y={} holds {} runs", line.top, line.runs.len());
}
```

Invalid font bytes are rejected with `FontError::InvalidFontData` rather than
panicking.

For alignment, fallback, tab stops, and truncation, pass a `LayoutOptions` to
`layout_paragraph_with`. Per-range styling goes in as `StyleSpan`s.

## Feature flags

| Feature   | Default | Enables                                                       |
|-----------|---------|---------------------------------------------------------------|
| `std`     | yes     | `std` types and the `std` build of the crate root             |
| `fontdb`  | no      | `tpt_text::system::SystemFonts` (OS font discovery via `fontdb`) |
| `serde`   | no      | Serialization of `Fx` (as an integer). Layout types do not implement it yet |

Build without default features for `no_std` targets:

```sh
cargo build -p tpt-text --no-default-features --target thumbv7em-none-eabihf
```

## Determinism

Layout uses fixed-point `Fx` values and integer arithmetic throughout. Given the
same font bytes, text, and options, the output is the same on every platform.
The repository checks this in two ways:

- `tpt-text-wasm-check` lays out a fixed set of paragraphs and hashes every
  position. The native digests are compared with digests recorded from the
  WebAssembly build under Node (`scripts/wasm-digest.mjs`).
- Golden tests in `tests/` compare the full `ParagraphLayout` tree against
  checked-in JSON files, using `tpt-text-golden`.

## Testing

- Unicode Bidi conformance (BidiCharacterTest and BidiTest) for levels and
  reordering.
- Unicode LineBreakTest conformance through `tpt-text-linebreak`.
- Golden scenarios: Arabic with numbers, emoji with ZWJ and ZWNJ, long
  paragraphs, mixed Bidi with fallback.
- `tpt-text-fuzz` feeds random strings, font subsets, options, widths, and
  hit-test queries through the engine and checks for panics and hangs.

```sh
cargo test -p tpt-text
cargo test -p tpt-text --features fontdb
cargo bench -p tpt-text
```

Vendored test fonts (Noto Sans, Noto Sans Arabic, Noto Sans Devanagari,
Noto Emoji) are licensed under the SIL Open Font License. See
`tests/fonts/NOTICE.md`.

## Scope

In scope: font loading and parsing, itemization, Bidi resolution, shaping,
line breaking, paragraph layout, and hit testing.

Out of scope: glyph rasterization (use a downstream rasterizer), GPU or
platform-specific rendering, and rich-text document storage.

## Status

Phases 1 to 3 of the design are implemented. Integration with other TPT
projects is not planned. See [`CHANGELOG.md`](CHANGELOG.md) for release notes.

## Minimum supported Rust version

1.75. It is verified locally with `cargo +1.75.0 test`. The CI MSRV job has not
run yet.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

Copyright (c) 2026 TPT Solutions.

## Contributing

Report bugs and request features at <https://github.com/tpt-solutions/tpt-text/issues>. Pull requests from automation (for example Dependabot) are welcome.
