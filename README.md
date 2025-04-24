# JSON-YAML-Lint-Formatter
 
## To compile:
```powershell
$env:GOOS = "js"; $env:GOARCH = "wasm"; go build -o formatter-go.wasm
```

## Generate wasm_exec.js
```powershell
Copy-Item -Path "$(go env GOROOT)\misc\wasm\wasm_exec.js" -Destination ".\wasm_exec.js"
```

## Run locally:
```powershell
python -m http.server 8000 
``` 