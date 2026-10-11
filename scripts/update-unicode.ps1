# Fetches the Unicode data that tpt-text-linebreak is generated from, then regenerates its tables.
# Windows counterpart of scripts/update-unicode.sh.
#
# Usage: scripts/update-unicode.ps1 [-Version <unicode-version>]
#
# Without a version, the highest version listed at unicode.org/Public is used.
# Run `cargo test --workspace` afterwards: a new Unicode version can change rules,
# not just data, and the conformance tests show when that happens.
param([string]$Version = '')
$ErrorActionPreference = 'Stop'
Set-Location (Join-Path $PSScriptRoot '..')

if (-not $Version) {
    $index = Invoke-WebRequest -UseBasicParsing -Uri 'https://www.unicode.org/Public/'
    $Version = ([regex]::Matches($index.Content, 'href="(\d+\.\d+\.\d+)/"') |
        ForEach-Object { $_.Groups[1].Value } |
        Sort-Object { [version]$_ } |
        Select-Object -Last 1)
}
Write-Host "Unicode version: $Version"

$base = "https://www.unicode.org/Public/$Version"
$ucd = "testdata/unicode/ucd/$Version"
New-Item -ItemType Directory -Force -Path $ucd | Out-Null
foreach ($file in 'LineBreak.txt', 'UnicodeData.txt', 'EastAsianWidth.txt') {
    Invoke-WebRequest -UseBasicParsing -Uri "$base/ucd/$file" -OutFile "$ucd/$file"
}
Invoke-WebRequest -UseBasicParsing -Uri "$base/ucd/emoji/emoji-data.txt" -OutFile "$ucd/emoji-data.txt"
Invoke-WebRequest -UseBasicParsing -Uri "$base/ucd/LineBreakTest.txt" -OutFile 'testdata/unicode/LineBreakTest.txt'

cargo run -q -p tpt-text-linebreak-gen -- $ucd $Version 'crates/tpt-text-linebreak/src/tables.rs'
if ($LASTEXITCODE -ne 0) { throw 'table generation failed' }
