# tpt-text-wasm

WebAssembly bindings for [tpt-text](../tpt-text): SVG export, layout digests and
caret lookup, callable from JavaScript.

## Build

```sh
wasm-pack build crates/tpt-text-wasm --target web --release
```

This writes `pkg/` next to the crate. Then open the demo:

```sh
cd crates/tpt-text-wasm && npm run serve
```

and visit <http://localhost:8080>, choose a `.ttf` file, and type.

## API

| Function | Returns |
| --- | --- |
| `layout_svg(font, text, size, width)` | SVG markup for the paragraph |
| `layout_digest(font, text, size, width)` | 16 hex digits, equal for identical layouts |
| `caret_at(font, text, size, width, x, y)` | byte offset of the caret nearest to `(x, y)` |

`font` is a `Uint8Array` of the font file. Sizes and widths are whole pixels.
