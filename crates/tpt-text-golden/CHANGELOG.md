# Changelog

All notable changes to this crate are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this crate follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `check` compares a `serde::Serialize` value with a pretty-printed JSON file.
- Update mode, enabled by `TPT_GOLDEN_UPDATE=1`, rewrites the file instead of failing.
- CRLF normalization so golden files compare correctly on Windows checkouts.
