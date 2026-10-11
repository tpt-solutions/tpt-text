# tpt-text-wasm-check

A cross-platform determinism check for [`tpt-text`](../tpt-text). The same
layouts must produce the same digests natively and under WebAssembly.

It holds a fixed set of paragraphs (`CASES`) covering Latin wrapping, justification,
Arabic with numbers, mixed Bidi with fallback, and a long paragraph. `digest_case`
lays each one out and folds every line and glyph position into an FNV-1a hash,
always writing little-endian integers.

## How the check works

1. `tests/wasm_digests.rs` computes the digests natively and compares them
   with `golden/digests.json`.
2. The crate is built for `wasm32-unknown-unknown` as a `cdylib`.
3. `scripts/wasm-digest.mjs` runs the module under Node and writes the digests
   the WebAssembly build produces.

```sh
cargo build -p tpt-text-wasm-check --target wasm32-unknown-unknown --release
node scripts/wasm-digest.mjs target/wasm32-unknown-unknown/release/tpt_text_wasm_check.wasm      crates/tpt-text-wasm-check/golden/digests.json
cargo test -p tpt-text-wasm-check
```

If native and WebAssembly digests differ, the layout depends on something that is
not deterministic across targets. Investigate before updating the golden file.

## Adding a case

Add an entry to `CASES` in `src/lib.rs` and keep the order in step with
`scripts/wasm-digest.mjs`. Then regenerate the digests.

## Publishing

This crate is a verification tool and is not published to crates.io.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

Copyright (c) 2026 TPT Solutions.

## Contributing

Issues only. Report bugs and request features at <https://github.com/tpt-solutions/tpt-text/issues>. Please do not open pull requests.
