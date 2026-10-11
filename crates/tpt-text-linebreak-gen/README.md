# tpt-text-linebreak-gen

Code generator for the character tables in [`tpt-text-linebreak`](../tpt-text-linebreak).

It reads four Unicode Character Database files and writes a Rust source file
with two sorted tables of `(first code point, value)` pairs: one for line
break classes and one for property flags.

## Usage

```sh
cargo run -p tpt-text-linebreak-gen -- <ucd-dir> <unicode-version> <output.rs>
```

For example:

```sh
cargo run -q -p tpt-text-linebreak-gen -- testdata/unicode/ucd/18.0.0 18.0.0 crates/tpt-text-linebreak/src/tables.rs
```

`<ucd-dir>` must contain:

- `LineBreak.txt`
- `UnicodeData.txt`
- `EastAsianWidth.txt`
- `emoji-data.txt`

The usual way to run it is `scripts/update-unicode.sh`, which downloads the
inputs, regenerates the tables, and runs the workspace tests.

## Output

The generated file starts with `// @generated` and records the Unicode version.
Its property flags match the constants in `tpt-text-linebreak`. Change both sides
together, or the tables will disagree with the rules.

## Errors

The generator exits with status 1 and a message on stderr for an unknown line
break class, a malformed code point, or a missing input file.

## Publishing

This crate is a development tool and is not published to crates.io.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

Copyright (c) 2026 TPT Solutions.

## Contributing

Issues only. Report bugs and request features at <https://github.com/tpt-solutions/tpt-text/issues>. Please do not open pull requests.
