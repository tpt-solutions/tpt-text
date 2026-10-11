# tpt-text-linebreak

UAX #14 (Unicode Line Breaking Algorithm) for [`tpt-text`](../tpt-text).

Given a string, `linebreaks` returns every break opportunity as a byte offset
and a `BreakOpportunity` (`Allowed` or `Mandatory`). `class_of` exposes the
UAX #14 line break class of a single code point.

## Usage

```rust
use tpt_text_linebreak::{linebreaks, BreakOpportunity::{Allowed, Mandatory}};

let text = "one two\nthree";
let breaks: Vec<_> = linebreaks(text).collect();
assert_eq!(breaks[0], (4, Allowed));   // after "one "
assert_eq!(breaks[1], (8, Mandatory)); // after the newline
```

## Data and generation

The character data is generated, not hand-written:

- `src/tables.rs` is produced by [`tpt-text-linebreak-gen`](../tpt-text-linebreak-gen)
  from the Unicode Character Database. Do not edit it by hand.
- `UNICODE_VERSION` reports the Unicode release the tables were built from.
- `src/rules.rs` holds the rules LB1 to LB31 as written in UAX #14 revision 57. It
  is the only hand-written part that must change when a new Unicode release
  alters the algorithm.

The conformance test in `tests/conformance.rs` runs the Unicode
`LineBreakTest.txt` cases. A pass means the rules match the spec for that data.

Updating to a new Unicode version is described in
[`docs/unicode-data.md`](../../docs/unicode-data.md).

## Properties

- `#![no_std]`, using `alloc` for the output iterator.
- No `unsafe` code (the workspace forbids it).
- No dependencies.
- Deterministic: the same text always yields the same breaks.

## Scope

This crate follows the default (non-tailored) UAX #14 behaviour. It does not
perform dictionary-based word breaking, hyphenation, or line fitting. Those
belong to the caller or to `tpt-text`.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

Copyright (c) 2026 TPT Solutions.

## Contributing

Issues only. Report bugs and request features at <https://github.com/tpt-solutions/tpt-text/issues>. Please do not open pull requests.
