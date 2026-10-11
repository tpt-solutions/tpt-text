# Dependency audit: no_std and determinism

Checked with `cargo build -p tpt-text --no-default-features` for
`thumbv7em-none-eabihf` and `wasm32-unknown-unknown`, using rustc 1.97.0.

| Crate | Version | no_std + alloc build |
| --- | --- | --- |
| rustybuzz | 0.20.1 | Passes with `default-features = false` |
| ttf-parser (via rustybuzz) | re-exported | Passes |
| unicode-bidi | 0.3.18 | Passes |
| unicode-linebreak | 0.1.5 | Passes |
| unicode-script | 0.5.8 | Passes |
| spin | 0.12.3 | Passes (`rwlock` feature) |

Notes:

- The crate root uses `#![cfg_attr(not(feature = "std"), no_std)]`. No `std`
  imports exist in `src/` outside tests.
- The MSRV of 1.75 is not verified. Only rustc 1.97 is installed here.
- The Unicode crates may lag the Unicode 18.0.0 test data. Conformance tests
  will show any gaps.
- Determinism is achieved by integer-only layout math (`Fx`). The only float
  use is `Fx::from_f32` and `Fx::to_f32` at API boundaries.
