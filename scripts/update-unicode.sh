#!/usr/bin/env bash
# Fetches the Unicode data that tpt-text-linebreak is generated from, then regenerates its tables.
#
# Usage: scripts/update-unicode.sh [unicode-version]
#
# Without a version, the highest version listed at unicode.org/Public is used.
# Run `cargo test --workspace` afterwards: a new Unicode version can change rules,
# not just data, and the conformance tests show when that happens.
set -euo pipefail
cd "$(dirname "$0")/.."

version="${1:-}"
if [ -z "$version" ]; then
    version=$(curl -sfL https://www.unicode.org/Public/ \
        | grep -oE 'href="[0-9]+\.[0-9]+\.[0-9]+/"' \
        | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' \
        | sort -t. -k1,1n -k2,2n -k3,3n \
        | tail -n 1)
fi
echo "Unicode version: $version"

base="https://www.unicode.org/Public/$version"
ucd="testdata/unicode/ucd/$version"
mkdir -p "$ucd"
for file in LineBreak.txt UnicodeData.txt EastAsianWidth.txt; do
    curl -sfL -o "$ucd/$file" "$base/ucd/$file"
done
curl -sfL -o "$ucd/emoji-data.txt" "$base/ucd/emoji/emoji-data.txt"
curl -sfL -o "testdata/unicode/LineBreakTest.txt" "$base/ucd/LineBreakTest.txt"

cargo run -q -p tpt-text-linebreak-gen -- "$ucd" "$version" crates/tpt-text-linebreak/src/tables.rs
