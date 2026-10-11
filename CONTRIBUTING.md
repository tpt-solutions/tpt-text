# Contributing to tpt-text

Thanks for helping. Bug reports and feature requests are welcome as issues, and
pull requests are welcome too.

## Before you start

- Check `TODO.md` for planned work. Items marked with a phase are tracked there.
- For a larger change, open an issue first so the design can be agreed.

## Checks

Run the same checks as CI before opening a pull request:

```sh
cargo xtask ci
```

That runs formatting, clippy with warnings as errors, the workspace tests, the
feature-gated tests, and the `no_std` builds for embedded and WebAssembly targets.
On Windows, the same command works from PowerShell.

Other useful commands:

- `cargo xtask bench-check` compares benchmark timings with `crates/tpt-text/benches/baseline.txt`.
- `TPT_FUZZ_ITERATIONS=5000 cargo test -p tpt-text-fuzz` runs a longer randomised soak.
- `TPT_GOLDEN_UPDATE=1` rewrites golden files. Check the diff by hand before committing.
- `TPT_SNAPSHOT_UPDATE=1` rewrites snapshot files in the same way.

## Conventions

- Layout output must stay integer-only so it is identical on every platform.
  Do not introduce floating point into positions or widths.
- Every bug fix starts with a failing regression test.
- Public items need rustdoc. The crate denies missing docs for the `no_std` core.
- Keep `unsafe` out of the workspace. Dependencies may use it.

## Unicode data

Line break tables are generated from the Unicode Character Database. Use
`scripts/update-unicode.sh` on Linux and macOS, or `scripts/update-unicode.ps1`
on Windows, then run the conformance tests.

## Licensing

By contributing you agree that your work is licensed under the same terms as the
project: MIT or Apache-2.0, at your option.
