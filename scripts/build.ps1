$ErrorActionPreference = "Stop"
Push-Location (Join-Path $PSScriptRoot "..")

try {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        throw "cargo was not found. Install Rust from https://rustup.rs/ first."
    }
    if (Get-Command rustup -ErrorAction SilentlyContinue) {
        if ((rustup target list --installed) -notcontains "wasm32-unknown-unknown") {
            throw "The wasm32-unknown-unknown target is missing. Run: rustup target add wasm32-unknown-unknown"
        }
    }

    cargo build --release --target wasm32-unknown-unknown
    if ($LASTEXITCODE -ne 0) {
        throw "WASM build failed."
    }

    Copy-Item "target\wasm32-unknown-unknown\release\formatter.wasm" "formatter.wasm" -Force
    $size = (Get-Item "formatter.wasm").Length
    Write-Output "Built formatter.wasm ($size bytes). formatter.js is committed, so nothing else is generated."
} finally {
    Pop-Location
}