# Changelog

All notable changes to this crate are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this crate follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `Lcg`: a deterministic 64-bit LCG for reproducible inputs.
- `random_text`: random strings from a pool of Unicode edge cases.
- Robustness tests asserting no panics and no hangs, with `TPT_FUZZ_ITERATIONS`
  for longer runs.
