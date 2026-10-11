# Changelog

All notable changes to this crate are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this crate follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/tpt-solutions/tpt-text/releases/tag/tpt-text-linebreak-v0.1.0) - 2026-10-11

### Other

- Restructure tpt-text workspace and add CI, tooling and docs
- Restructure tpt-text into split crates and add Unicode data tooling

### Added

- `BreakClass` enum covering all UAX #14 classes, including the Unicode 16+
  Brahmic classes (`AK`, `AP`, `AS`, `VF`, `VI`) and `HH`.
- `class_of` for per-code-point class lookup.
- `linebreaks` iterator over `(byte offset, BreakOpportunity)`.
- `UNICODE_VERSION` constant (Unicode 18.0.0).
- Generated tables from `tpt-text-linebreak-gen`.
- Conformance tests against `LineBreakTest.txt`.
