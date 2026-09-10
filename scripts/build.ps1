$ErrorActionPreference = "Stop"
$previousGOOS = $env:GOOS
$previousGOARCH = $env:GOARCH
Push-Location (Join-Path $PSScriptRoot "..")

try {
    $env:GOOS = "js"
    $env:GOARCH = "wasm"
    go build -o formatter.wasm .
    if ($LASTEXITCODE -ne 0) {
        throw "WASM build failed."
    }

    $goRoot = go env GOROOT
    if ($LASTEXITCODE -ne 0) {
        throw "Unable to locate the Go runtime."
    }
    $runtime = Join-Path $goRoot "lib\wasm\wasm_exec.js"
    if (-not (Test-Path $runtime)) {
        $runtime = Join-Path $goRoot "misc\wasm\wasm_exec.js"
    }
    Copy-Item $runtime "wasm_exec.js" -Force
    Write-Output "Built formatter.wasm and copied the matching wasm_exec.js."
} finally {
    $env:GOOS = $previousGOOS
    $env:GOARCH = $previousGOARCH
    Pop-Location
}