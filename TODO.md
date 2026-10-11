# tpt-text TODO

Decisions: no_std+alloc core (`std` feature for fontdb), fixed-point `Fx` public units, rustybuzz only, spans over `&str`, trait-only hyphenation, caller-configured fallback, edition 2021 / MSRV 1.75.

## Phase 0: Setup
- [x] Cargo workspace: `tpt-text`, `tpt-text-fuzz`, `tpt-text-golden`
- [x] Pin toolchain (stable, MSRV 1.75), rustfmt/clippy config (stable pinned; build and tests verified on 1.75.0)
- [x] Features: `std` (default), `fontdb` (`tpt_text::system::SystemFonts`), `serde` (golden output)
- [x] Verify no_std status of rustybuzz, ttf-parser, unicode-bidi, unicode-linebreak, unicode-script; record findings (`docs/dependency-audit.md`)
- [x] CI-style script: test, clippy, `wasm32-unknown-unknown` no_std build (first `scripts/ci.ps1`, now `cargo xtask ci`)
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
- [x] `tpt-text-golden` helper: JSON golden files with update mode
- [x] Golden scenarios: Arabic with numbers, emoji with ZWJ/ZWNJ, long paragraph (golden files; also asserted in `tests/scenarios.rs`)
- [x] Unicode Bidi conformance tests (BidiCharacterTest and BidiTest, levels and reorder: 0 failures; lines containing paragraph separators are skipped)
- [x] Unicode LineBreak conformance tests (LineBreakTest passes with zero known failures using the in-tree `tpt-text-linebreak` crate)
- [x] Cross-platform determinism check (native Windows digests equal digests from the WASM build under Node; `crates/tpt-text-wasm-check`, `scripts/wasm-digest.mjs`)
- [x] `tpt-text-fuzz`: random strings, font subsets, options, widths, hit-tests; no panics or hangs (`TPT_FUZZ_ITERATIONS` for soak runs)
- [x] Benchmarks for shaping and layout of large paragraphs (`cargo bench -p tpt-text`)
- [x] Docs and rustdoc examples (quick start and doctests in `lib.rs`)

## Phase 5: Correctness fixes (add a failing regression test first for each)
Regression tests: `crates/tpt-text/tests/phase5.rs`.
- [x] Exclude trailing spaces from line width (breaking, right/center alignment, justify)
- [x] Account for tabs when breaking lines; fix tab stops in RTL (tab stops are measured from the right edge on RTL lines)
- [x] Hide U+2028, U+2029, U+0085, VT and FF glyphs (`is_hidden_at`)
- [x] Fallback per grapheme cluster, not per char, so ZWJ/VS16/combining sequences stay in one font (extender rules: marks, selectors, ZWJ; not full UAX #29)
- [x] `hit_test` `edge_byte` returns the cluster end, not the char end
- [x] Hyphen and ellipsis glyphs must not break carets/selection (marks are skipped in hit-testing and selection)
- [x] RTL hyphen placement and bidi-aware ellipsis (marks drawn from the font that has the glyph, whole clusters removed, `text_range` updated)
- [x] Apply bidi rule L1 per line, not per paragraph; garbled doc comment fixed
- [x] Trailing `\n` produces an empty last line (kept on purpose: a caret target after a trailing newline)
- [x] `SystemFonts::load` must match weight/italic and cache misses; resolve system fallback families
- [x] `FontRegistry::load_from_memory`: re-check for duplicates under the write lock
- [x] Stop re-parsing in `FontFace::parsed()` / `hb_face()` (parsers are cached with `self_cell`; variations build a face per call)
- [x] Implement `std::error::Error` for `FontError` and `LayoutError` (behind `std`)
- [x] Decide `max_lines: Some(0)` behaviour (kept: treated as 1, documented on `LayoutOptions::max_lines`)
- [x] Ellipsise a single overlong word even without `max_lines` (with `ellipsis` set, a paragraph with no break opportunity is one truncated line)

## Phase 6: Repo hygiene
- [x] Reconcile MSRV claims (`TODO.md`, tpt-text README, `docs/dependency-audit.md`) and verify on 1.75 (`cargo +1.75.0 build` and `test` for `tpt-text`)
- [x] Update stale docs: `testdata/unicode/README.md`, `docs/dependency-audit.md` (linebreak is no_std), "Phase 4 pending" in `lib.rs`
- [x] README says serde covers layout output: derive it for layout types or fix the claim
- [x] `scripts/update-unicode.sh`: run tests as documented, refresh Bidi test files, add a Windows equivalent (`scripts/update-unicode.ps1`)
- [x] Bounds checks and tests for `tpt-text-linebreak-gen` (code points past U+10FFFF and reversed ranges are rejected)
- [x] Set `publish = false` on `tpt-text-golden`; add `include`/`exclude` so `tests/fonts` does not ship in the crate
- [x] Reword "Issues only, no pull requests" to allow automation PRs
- [x] Add CONTRIBUTING and `cargo-deny` / `cargo-audit` config (`CONTRIBUTING.md`, `deny.toml`, `.cargo/audit.toml`)

## Phase 7: Automation
- [x] Per-push GitHub Actions CI on Linux, macOS and Windows with an MSRV job (`.github/workflows/ci.yml`)
- [ ] Confirm the first CI run passes (fix MSRV job if dependencies need a newer rustc). Needs a push to GitHub, so not done here
- [x] Record wasm digests on Linux and macOS to prove cross-platform determinism (the CI check matrix compares native digests with the recorded WebAssembly digests on each OS; it runs on the first push)
- [x] Replace `scripts/ci.ps1` with a cross-platform `cargo xtask ci` (`xtask/`)
- [x] Real `cargo-fuzz` targets: linebreak alone, corrupt font bytes, spans/direction/features (`crates/tpt-text-fuzz/fuzz/`; type-checked, not run on this machine)
- [x] Fuzz invariants: lines cover the text, widths fit, output deterministic; generate `\r` and U+2028
- [x] Goldens record the full `ParagraphLayout` (`crates/tpt-text/tests/golden_full.rs`, run with `--features serde`)
- [x] Benchmark baseline with a regression threshold (`cargo xtask bench-check`, 25%, baseline in `crates/tpt-text/benches/baseline.txt`)
- [x] Dependabot and release automation (release-plz opens release PRs only; publishing is left out) and a docs.rs-style doc build in `cargo xtask ci`

## Phase 8: Features
- [x] Glyph outline / path extraction (ttf-parser) and SVG export (`outline.rs`, `svg.rs`)
- [x] `Start` / `End` alignment; RTL-aware default alignment (the default is `Start`)
- [x] Variable-font axes in `TextStyle` (applied during shaping; no variable font is vendored to test against)
- [x] Report missing (.notdef) glyphs to the caller (`ParagraphLayout::missing_glyphs`)
- [x] Read family/weight/italic from the font name table; `.ttc` collection loading (`load_font`, `load_collection`)
- [x] Builders for `TextStyle` and `LayoutOptions`; shorter layout entry point (`layout_text`); `Debug`/`Clone` on `LayoutOptions`
- [x] Multi-paragraph layout (`Document`), shape cache (`ShapeCache`), incremental relayout (only changed paragraphs)
- [x] Layout digest API for verifying identical layouts across machines (`ParagraphLayout::digest`)
- [x] Layout "explain" mode (which UAX #14 rule caused each break: `explain_line_breaks`)
- [x] Snapshot-testing helper crate for downstream users (`crates/tpt-text-snapshot`)

## Phase 9: Adoption
- [x] Root README with a copy-paste hello example, feature table and determinism claim
- [x] Examples: layout to SVG, editor caret and selection, bidi and emoji showcase (`crates/tpt-text/examples/`), browser demo (`crates/tpt-text-wasm/www/`)
- [x] Starter template (`templates/tiny-skia`) showing renderer integration (tiny-skia; egui and wgpu not done)
- [x] `tpt-text-wasm` crate (wasm-bindgen) and npm package manifest (`private`; not published). Optional C header and Python bindings not done
- [x] Small CLI: `tpt-text-cli layout --font x.ttf "text"` printing JSON, SVG, digest or break explanations
- [x] Cookbook in `docs/cookbook.md`
- [x] Migration guide from cosmic-text / parley (`docs/migration.md`)
- [ ] Publish 0.1.0 with crates.io / docs.rs / CI badges. Left out on purpose; publishing is a separate decision

## Phase 4: Ecosystem integration — dropped
Decision: other TPT repos will not depend on tpt-text. These items are not planned.
- ~~tpt-doc: PDF glyph positioning integration~~
- ~~tpt-pwa: WASM text input and rendering~~
- ~~tpt-vertex: hit-testing and title-block fit~~
- ~~tpt-color: resolve `ColorId`~~ (the repo is now `tpt-colour`; `ColorId` stays abstract in tpt-text)
- ~~Publish v1.0.0 to internal TPT registry~~
