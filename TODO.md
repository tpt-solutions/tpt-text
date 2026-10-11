# tpt-text TODO

Decisions: no_std+alloc core (`std` feature for fontdb), fixed-point `Fx` public units, rustybuzz only, spans over `&str`, trait-only hyphenation, caller-configured fallback, edition 2021 / MSRV 1.75.

## Phase 0: Setup
- [x] Cargo workspace: `tpt-text`, `tpt-text-fuzz`, `tpt-golden`
- [ ] Pin toolchain (stable, MSRV 1.75), rustfmt/clippy config (pinned and configured; MSRV 1.75 not yet verified, only rustc 1.97 installed)
- [x] Features: `std` (default), `fontdb` (`tpt_text::system::SystemFonts`), `serde` (golden output)
- [x] Verify no_std status of rustybuzz, ttf-parser, unicode-bidi, unicode-linebreak, unicode-script; record findings (`docs/dependency-audit.md`)
- [x] CI-style script: test, clippy, `wasm32-unknown-unknown` no_std build (`scripts/ci.ps1`)
- [x] Vendor open-license test fonts (Latin, Arabic, Devanagari, emoji) with license notes
- [x] Vendor Unicode BidiTest / BidiCharacterTest / LineBreakTest data

## Phase 1: Core substrate (Weeks 1-3)
- [x] `Fx` fixed-point newtype (arithmetic, rounding, `to_f32`, serde as integer)
- [x] `TextStyle` (family, weight, italic, size, letter-spacing, OpenType features, `ColorId(u32)`)
- [x] `FontFace` wrapper over rustybuzz/ttf-parser (metrics, glyph coverage, units-per-em)
- [x] `FontQuery` and `FontProvider` trait
- [x] `FontRegistry`: load from memory, `spin::RwLock` (no_std) / std RwLock, `Arc<FontFace>` cache
- [x] `fontdb` system font source behind `fontdb` feature (`tpt_text::system`)
- [x] Itemization by script, style and direction (direction from Bidi levels)
- [x] `ShapedGlyph` / `ShapedRun` with fixed-point values, `cluster_index` mapping to byte index
- [x] rustybuzz shaping for LTR runs, including OpenType features
- [x] Basic line breaking over shaped runs (no Bidi, no fallback)
- [x] Phase 1 golden tests

## Phase 2: The hard stuff (Weeks 4-6)
- [x] Full Unicode Bidi resolution (unicode-bidi), visual run reordering
- [x] RTL and mixed-direction shaping, cluster handling
- [x] `FallbackResolver` with caller-configured per-script chains and defaults
- [x] Missing-glyph handling (.notdef) and run splitting on fallback
- [x] UAX #14 line breaking (unicode-linebreak), soft hyphen
- [x] `Hyphenator` trait and hooks in the breaker
- [x] Explicit newlines and tab stops
- [x] Phase 2 golden tests (`mixed_bidi_fallback_justified`; behaviour also asserted in `tests/phase2_3.rs`)

## Phase 3: Layout and interaction (Weeks 7-9)
- [x] Line height / leading control, ascent/descent per line
- [x] Alignment: left, right, center
- [x] Full justification (word and letter spacing, deterministic rounding)
- [x] Ellipsis / max-lines truncation
- [x] `ParagraphLayout` / `LayoutLine` output types
- [x] `hit_test`: point to byte index + caret
- [x] `hit_test`: byte index to caret point
- [x] `hit_test`: selection rectangles (multi-line, Bidi)
- [x] Audit for accidental std usage; confirm `--no-default-features` build
- [x] WASM build check and example consumer (`examples/layout_demo.rs`)

## Testing and quality
- [x] `tpt-golden` helper: JSON golden files with update mode
- [ ] Golden scenarios: Arabic mixed with numbers (done); ZWJ/ZWNJ, emoji fallback, huge paragraphs asserted in `tests/scenarios.rs` but not yet golden files
- [ ] Unicode Bidi conformance tests (BidiCharacterTest levels and reorder run; 1 known level mismatch. BidiTest.txt not wired)
- [ ] Unicode LineBreak conformance tests (LineBreakTest runs; 1117 known failures because `unicode-linebreak` 0.1.5 implements Unicode 15, see `testdata/unicode/known-failures/`)
- [ ] Cross-platform determinism check (Windows vs WASM output identical)
- [ ] `tpt-text-fuzz`: random strings, font subsets, max_width; no panics or hangs (random strings and widths done; font subsets not yet)
- [ ] Benchmarks for shaping and layout of large paragraphs
- [ ] Docs and rustdoc examples

## Phase 4: Ecosystem integration (Weeks 10-12), blocked on external repos
- [ ] tpt-doc: PDF glyph positioning integration
- [ ] tpt-pwa: WASM text input and rendering
- [ ] tpt-vertex: hit-testing and title-block fit
- [ ] tpt-color: resolve `ColorId`
- [ ] Publish v1.0.0 to internal TPT registry
