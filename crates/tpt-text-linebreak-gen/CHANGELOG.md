# Changelog

All notable changes to this crate are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this crate follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Command-line generator that reads `LineBreak.txt`, `UnicodeData.txt`,
  `EastAsianWidth.txt` and `emoji-data.txt` and writes `tpt-text-linebreak`'s tables.
- Stamps the output with `@generated` and the Unicode version.
