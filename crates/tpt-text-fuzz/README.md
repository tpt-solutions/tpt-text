# tpt-text-fuzz

Randomised robustness tests for [`tpt-text`](../tpt-text). The tests feed
pseudo-random text, font subsets, layout options, widths, and hit-test queries
through the engine. They check that nothing panics and nothing loops forever.

## Running

```sh
cargo test -p tpt-text-fuzz
```

For a longer soak run, raise the iteration count:

```sh
TPT_FUZZ_ITERATIONS=100000 cargo test -p tpt-text-fuzz --release
```

## Reproducibility

The generator is a fixed-seed 64-bit LCG (`Lcg`), so every run covers the same
inputs and any failure reproduces exactly. The LCG is not cryptographic and is
not meant to be.

The character pool (`random_text`) covers ASCII, Latin-1, Arabic, Devanagari,
ZWJ and ZWNJ, emoji, newlines, soft hyphens, line separators, and combining marks.

## Corpus

Fuzzing corpora and artifacts are git-ignored under this crate. The current
harness is seed-based and does not use a coverage-guided fuzzer.

## Publishing

This crate is a test harness and is not published to crates.io.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

Copyright (c) 2026 TPT Solutions.

## Contributing

Issues only. Report bugs and request features at <https://github.com/tpt-solutions/tpt-text/issues>. Please do not open pull requests.
