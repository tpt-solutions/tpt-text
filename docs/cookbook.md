# Cookbook

Short recipes for common tasks. Each uses the public API of `tpt-text`; the
examples in `crates/tpt-text/examples/` run as written.

## Load a font

```rust
use tpt_text::FontRegistry;

let registry = FontRegistry::new();
let face = registry.load_font(std::fs::read("Font.ttf")?, 0)?;
println!("{} weight {}", face.family(), face.weight());
```

`load_font` reads the family, weight and slant from the file. Use
`load_from_memory` to name the family yourself. A `.ttc` collection gives one
face per index: `registry.load_collection(bytes)?`.

Loading the same bytes twice returns the same `Arc<FontFace>`.

## Set up fallback for emoji and scripts

Fallback has two parts. The `FontRegistry` holds per-script chains that every
caller shares. A `FallbackResolver` is a per-call choice that overrides them.

```rust
use tpt_text::{FallbackResolver, FontRegistry, Script};

let registry = FontRegistry::new();
// Shared for the whole registry: Arabic text falls back to this family.
registry.set_fallback_chain(Script::Arabic, &["Noto Sans Arabic"]);

// Per call: emoji fallback for this paragraph only.
let resolver = FallbackResolver::new().with_defaults(&["Noto Color Emoji"]);
let options = tpt_text::LayoutOptions::default().fallback(&resolver);
```

Emoji fonts must be registered (`load_font`) before layout, or the fallback
finds nothing. A character that no face covers is drawn as `.notdef`, and
`layout.missing_glyphs` lists it so you can warn the user.

## Wrap, justify and truncate

```rust
use tpt_text::{layout_paragraph_with, Align, Fx, LayoutOptions, TextStyle};

let options = LayoutOptions::default()
    .align(Align::Justify)
    .max_lines(3)
    .ellipsis(true);
let layout = layout_paragraph_with(&registry, text, &style, &[], Fx::from_int(300), &options)?;
```

- `Align::Start` and `Align::End` follow the paragraph direction, so Arabic text
  is right-aligned by default.
- Trailing spaces hang past the line end, so justified lines look right.
- With `ellipsis(true)`, a line cut by `max_lines` ends with `…`. A single word
  wider than the paragraph also gets one truncated line.

## Hyphenate words

Implement `Hyphenator` with your dictionary, then pass it in:

```rust
use tpt_text::Hyphenator;

struct Dictionary;

impl Hyphenator for Dictionary {
    /// Byte offsets inside `word` where a hyphen may be drawn.
    fn break_points(&self, word: &str) -> Vec<usize> {
        if word == "international" { vec![5] } else { Vec::new() }
    }
}

let options = LayoutOptions::default().hyphenator(&Dictionary);
```

The hyphen is drawn at the line end and is not a caret target. Its width counts
when deciding whether the break fits.

## Find the caret for a click

```rust
use tpt_text::hit_test::{caret_for_byte, hit_test, selection_rects};

// A click at pixel (x, y) in the paragraph's own coordinates.
let caret = hit_test(&layout, Fx::from_int(x), Fx::from_int(y)).unwrap();
store_cursor(caret.byte_index);

// Where the caret sits for a byte offset.
let caret = caret_for_byte(&layout, byte).unwrap();

// Highlight a selection. One rectangle per visual stretch per line.
let rects = selection_rects(&layout, start..end);
```

Byte indices always fall on character boundaries and cluster starts. Marks
(hyphen, ellipsis) never receive a caret.

Right-to-left and mixed text use visual order. A click returns the logical
byte index regardless of direction.

## Lay out several paragraphs

```rust
use tpt_text::{Document, Fx, LayoutOptions};

let mut doc = Document::new(Fx::from_int(8)); // 8 layout units between paragraphs
doc.push("First paragraph.", vec![]);
doc.push("Second paragraph.", vec![]);

let placed = doc.layout(&registry, &style, width, &LayoutOptions::default())?;
for p in &placed.paragraphs {
    draw(&p.layout, p.top);
}

// Editing one paragraph relays out only that paragraph next time.
doc.set(1, "Second paragraph, edited.", vec![]);
doc.layout(&registry, &style, width, &LayoutOptions::default())?;
assert_eq!(doc.last_relayout_count(), 1);
```

Changing the width, base style or a layout option relays out everything. If you
change the fallback resolver or hyphenator, call `doc.invalidate()`.

## Variable fonts

```rust
use tpt_text::{Fx, TextStyle};

let style = TextStyle::new("Recursive", Fx::from_int(18))
    .with_variation(*b"wght", Fx::from_int(650))
    .with_variation(*b"CASL", Fx::from_raw(32768)); // 0.5
```

Axis values are in design units. A static font ignores them.

## Export SVG

```rust
let svg = tpt_text::svg::layout_to_svg(&layout);
std::fs::write("out.svg", svg)?;
```

Glyph outlines are available on their own through
`FontFace::glyph_outline(glyph_id, size)`.

## Compare layouts across machines

```rust
let digest = layout.digest(); // 64-bit FNV-1a over positions and glyph ids
assert_eq!(digest, expected);
```

Store the digest next to a test, or print it from the CLI:

```sh
cargo run -p tpt-text-cli -- layout --font Font.ttf --format digest "text"
```

## Explain a line break

```rust
for reason in tpt_text::explain_line_breaks(&layout) {
    println!("line {} ended by {}", reason.line, reason.rule);
}
```

`LB18` means a break after a space. `hyphenation` marks a hyphen drawn at the
break. `overflow` means a word wider than the line was split.

## Snapshot a layout in a test

```rust
tpt_text_snapshot::check("tests/snapshots/heading.txt", &layout);
```

Run the test with `TPT_SNAPSHOT_UPDATE=1` the first time, then review the file.

## Use system fonts

With the `fontdb` feature:

```rust
use tpt_text::system::SystemFonts;
use tpt_text::FontProvider;

let fonts = SystemFonts::load();
let style = TextStyle::new("Helvetica", Fx::from_int(14));
let layout = layout_text(&fonts, text, &style, width)?;
```

System fonts are not part of the default build, because discovery depends on
the machine. Layouts that must match everywhere should use bundled fonts.

## Keep a layout cache small

Shaping is the expensive step. `Document` keeps a shape cache, so identical
runs across paragraphs are shaped once. `doc.shape_cache_len()` reports how
many distinct runs are held.
