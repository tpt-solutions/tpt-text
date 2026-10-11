# tpt-text-golden

JSON golden-file testing helper for [`tpt-text`](../tpt-text).

A golden test serializes a value, such as a `ParagraphLayout`, and compares it
with a JSON file checked into the repository. Output is pretty-printed JSON with
a trailing newline, so a change to the expected output shows up as a readable git
diff.

## Usage

```rust,ignore
use tpt_text_golden::check;

#[test]
fn layout_matches_golden() {
    let value = compute_layout();
    check("tests/golden/latin_wrap.json", &value);
}
```

To create or accept changes, run the tests in update mode:

```sh
TPT_GOLDEN_UPDATE=1 cargo test -p tpt-text
```

Update mode writes the file and skips the comparison. Review the resulting diff
before committing.

## Behaviour

- **Missing file**: `check` panics and names the file and the update variable.
- **Mismatch**: `check` panics with the path and the update variable.
- **Line endings**: CRLF in the expected file is normalized to LF, so golden files
  checked out with `core.autocrlf` on Windows still compare correctly.
- **Directories**: update mode creates missing parent directories.

## Panics

`check` panics on purpose. It is test support code, and a failing golden
comparison should fail the test.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

Copyright (c) 2026 TPT Solutions.

## Contributing

Issues only. Report bugs and request features at <https://github.com/tpt-solutions/tpt-text/issues>. Please do not open pull requests.
