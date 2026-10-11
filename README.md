# tpt-text

Deterministic text shaping and layout in pure Rust. The same input produces the
same glyph positions on every platform, including WebAssembly, so PDFs, CAD
drawings and browser previews agree byte for byte.

## Hello

```rust
use tpt_text::{layout_text, FontRegistry, Fx, TextStyle};

let registry = FontRegistry::new();
let font = std::fs::read("NotoSans-Regular.ttf")?;
let face = registry.load_font(font, 0)?;

let style = TextStyle::new(face.family(), Fx::from_int(16));
let layout = layout_text(&registry, "Hello, world", &style, Fx::from_int(200))?;

for line in &layout.lines {
    println!("line at y={} holds {} runs", line.top, line.runs.len());
}
println!("digest {:016x}", layout.digest());
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Features

| Area | What you get |
| --- | --- |
| Shaping | rustybuzz, OpenType features, letter spacing, variable-font axes |
| Bidi | UAX #9 resolution, visual reordering, per-line rule L1 |
| Line breaking | UAX #14 (Unicode 18), soft hyphens, caller-supplied hyphenation |
| Fallback | caller-configured per-script chains, per-cluster font choice |
| Layout | alignment (start, end, left, right, centre, justify), tab stops, line height, truncation with ellipsis |
| Editing | caret from a point, point from a byte, selection rectangles |
| Output | positions, glyph outlines, SVG, JSON with the `serde` feature, layout digests |
| Documents | several paragraphs with incremental relayout and a shape cache |
| Fonts | in-memory registry, collections (`.ttc`), names read from the file, system fonts with `fontdb` |
| Targets | `std`, `no_std` + `alloc`, `wasm32-unknown-unknown` |

Cargo features:

| Feature | Default | Adds |
| --- | --- | --- |
| `std` | yes | `std::error::Error` impls |
| `fontdb` | no | `tpt_text::system::SystemFonts`, OS font discovery |
| `serde` | no | `Serialize` for layouts and styles; `Deserialize` for styles |

## Determinism

Layout arithmetic is fixed-point (`Fx`, 16.16) and integer-only. Floating point
appears only where values enter or leave the crate, such as font variation
values and conversions for display. CI runs a digest check that compares native
output with output recorded from the WebAssembly build.

## Crates in this repository

| Crate | Purpose |
| --- | --- |
| [`tpt-text`](crates/tpt-text) | The library |
| [`tpt-text-linebreak`](crates/tpt-text-linebreak) | UAX #14 line breaking, `no_std` |
| [`tpt-text-wasm`](crates/tpt-text-wasm) | WebAssembly bindings and a browser demo |
| [`tpt-text-cli`](crates/tpt-text-cli) | Command-line layout: JSON, SVG, digest, break explanations |
| [`tpt-text-snapshot`](crates/tpt-text-snapshot) | Text snapshots of layouts for tests |
| [`tpt-text-golden`](crates/tpt-text-golden) | JSON golden files for tests |

Examples: [`layout_svg`](crates/tpt-text/examples/layout_svg.rs),
[`caret_selection`](crates/tpt-text/examples/caret_selection.rs),
[`bidi_emoji`](crates/tpt-text/examples/bidi_emoji.rs). A renderer starter using
tiny-skia is in [`templates/tiny-skia`](templates/tiny-skia).

Guides: [cookbook](docs/cookbook.md) and [migration](docs/migration.md).

## Working on the repository

```sh
cargo xtask ci          # formatting, lints, tests, no_std and wasm builds, docs, digests
cargo xtask bench-check # benchmarks against crates/tpt-text/benches/baseline.txt
```

See [CONTRIBUTING.md](CONTRIBUTING.md). Dual-licensed under MIT or Apache-2.0.
