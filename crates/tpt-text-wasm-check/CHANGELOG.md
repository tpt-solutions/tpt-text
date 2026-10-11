# Changelog

All notable changes to this crate are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this crate follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `CASES`: five fixed paragraphs covering wrapping, justification, Arabic numerals,
  Bidi with fallback, and long text.
- `digest_case` and `digest`: FNV-1a digests of layout positions, little-endian.
- Native digest test against `golden/digests.json`, with the WebAssembly build
  checked by `scripts/wasm-digest.mjs`.
