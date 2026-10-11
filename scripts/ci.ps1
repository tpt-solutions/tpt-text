# CI-style check: formatting, lints, tests, and no_std builds for embedded and wasm targets.
# Run from the repository root. Stops at the first failing step.
$ErrorActionPreference = 'Stop'

function Invoke-Step([string]$Name, [scriptblock]$Command) {
    Write-Host "==> $Name" -ForegroundColor Cyan
    & $Command
    if ($LASTEXITCODE -ne 0) { throw "step failed: $Name" }
}

Invoke-Step 'rustfmt' { cargo fmt --all -- --check }
Invoke-Step 'clippy' { cargo clippy --workspace --all-targets -- -D warnings }
Invoke-Step 'test' { cargo test --workspace }
Invoke-Step 'test (serde feature)' { cargo test -p tpt-text --features serde }
Invoke-Step 'test (fontdb feature)' { cargo test -p tpt-text --features fontdb }
Invoke-Step 'example consumer' { cargo build -p tpt-text --example layout_demo }
Invoke-Step 'no_std build (thumbv7em-none-eabihf)' { cargo build -p tpt-text --no-default-features --target thumbv7em-none-eabihf }
Invoke-Step 'no_std build (wasm32-unknown-unknown)' { cargo build -p tpt-text --no-default-features --target wasm32-unknown-unknown }
# The native digest test compares against digests recorded from this module under Node.
Invoke-Step 'determinism module (wasm32-unknown-unknown)' { cargo build -p tpt-wasm-check --target wasm32-unknown-unknown --release }

Write-Host 'All checks passed.' -ForegroundColor Green
