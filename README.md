# JSON/YAML Auto Formatter

A browser-based JSON and YAML formatter powered by Go WebAssembly. Formatting
runs locally in the browser, with four-space indentation for JSON and YAML.

## Build

Requires Go 1.22.3 or newer and PowerShell. From the repository root:

```powershell
go mod download
.\scripts\build.ps1
```

The build produces `formatter.wasm` and copies `wasm_exec.js` from the same Go
installation. Rebuild both together after changing Go versions. The script
restores your previous `GOOS` and `GOARCH` environment variables.

## Run

Serve the repository over HTTP; opening `index.html` directly cannot load WASM.
For example, with Python installed:

```powershell
python -m http.server 8000 --bind 127.0.0.1
```

Open <http://127.0.0.1:8000>, paste input, choose Auto-detect, JSON, or YAML,
and select Format.

## Detection and Formatting

- Valid JSON is detected first, including arrays, strings, numbers, and literals.
- JSON objects and arrays are indented with four spaces per level. Key order,
	numeric precision, and existing string escapes are preserved.
- Other input is parsed as YAML, including block lists, flow collections, scalar
	values, and multiple documents. Comments, anchors, and block strings are retained.
	Existing YAML flow collections remain in flow style.
- JSON and YAML overlap. In Auto-detect, JSON-looking collections (such as objects
	with double-quoted keys or arrays starting with JSON values) are validated as
	strict JSON, so malformed JSON is not silently accepted as YAML. Select YAML
	explicitly to interpret these ambiguous inputs as YAML.
- Empty input, malformed documents, and duplicate YAML mapping keys report errors.
	All YAML documents are validated; later documents are not silently discarded.

## Tests

```powershell
go test ./...
.\scripts\build.ps1
node --test test/wasm.test.cjs
```

Node.js 18 or newer is needed only for the WASM integration tests. The fixture
folders contain valid and intentionally invalid JSON and YAML for both automated
tests and manual browser checks.

## Layout

```text
main.go                         JavaScript/WASM bridge
internal/formatter/             Detection, formatting, and Go regression tests
scripts/build.ps1               WASM and matching JavaScript runtime build
test/json/                      Valid and invalid JSON fixtures
test/yaml/                      Valid and invalid YAML fixtures
test/wasm.test.cjs               Tests against the compiled WASM bridge
index.html                      Browser interface
formatter.wasm                  Generated Go WebAssembly binary
wasm_exec.js                    Generated matching Go runtime support
```