# JSON/YAML Auto Formatter

A browser-based JSON and YAML formatter powered by Rust WebAssembly. Formatting
runs locally in the browser, with four-space indentation for JSON and YAML.

The Go/WebAssembly implementation this replaced is kept at
[JSON-YAML-Auto-Formatter-Legacy-Go](https://github.com/pattygcoding/JSON-YAML-Auto-Formatter-Legacy-Go).

## Build

Requires Rust 1.87 or newer with the WebAssembly target, plus PowerShell. From the
repository root:

```powershell
rustup target add wasm32-unknown-unknown
.\scripts\build.ps1
```

The script runs `cargo build --release --target wasm32-unknown-unknown` and copies
the result to `formatter.wasm`. `formatter.js` is the small hand-written bridge
between the module and the page, so there is no `wasm-bindgen` or `wasm-pack` step
and no generated JavaScript to keep in sync with a tool version.

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
  numeric precision, and existing string escapes are preserved: the JSON path
  validates and re-indents the original text rather than re-encoding it.
- Other input is parsed as YAML. Comments, anchors, aliases, tags, quoting style,
  block scalars and flow collections are retained, because YAML is validated with
  `saphyr` and then re-indented line by line, so only leading indentation changes.
  One value block is indented four columns past its parent, and the members of a
  mapping that starts on a `- ` line stay aligned two columns past the dash.
- JSON and YAML overlap. In Auto-detect, JSON-looking collections (such as objects
  with double-quoted keys or arrays starting with JSON values) are validated as
  strict JSON, so malformed JSON is not silently accepted as YAML. Select YAML
  explicitly to interpret these ambiguous inputs as YAML.
- Empty input, malformed documents, duplicate YAML mapping keys and unknown
  anchors report errors. All YAML documents are validated; later documents are not
  silently discarded.

Failures come back as `invalid JSON: …` or `invalid YAML: …` messages that name the
offset or line the parser stopped at.

## Bridge

`formatter.js` loads the module and exposes:

- `FormatterWasm.init(source)` — `source` may be a URL, `ArrayBuffer`, typed array
  or compiled `WebAssembly.Module`; resolves once the module is ready.
- `FormatterWasm.format(input, mode)` — returns the formatted text, or the error
  message when the input was rejected.
- `formatInput(input, mode)` and the legacy alias `formatJSON` — installed on the
  global object by `init`, which is what `index.html` and the tests call. A leading
  `///force:json///` or `///force:yaml///` marker overrides `mode`, which is how the
  site's embedded formatter forces a format.

## Tests

```powershell
cargo test
.\scripts\build.ps1
node --test test/wasm.test.cjs
node --test test/stress.test.cjs
```

Node.js 18 or newer is needed only for the WASM integration tests. `cargo test`
runs the unit tests — detection, JSON cases captured from Go's `encoding/json`,
YAML re-indentation and idempotence — plus fixture tests over everything under
`test/`. The stress fixtures are generated rather than hand written:

```powershell
cargo run --release --bin generate-stress
```

## Differences from the Go version

The Go implementation re-serialised YAML through `yaml.v3`, which moved sequence
items, reflowed multi-line scalars, dropped blank lines and rewrote block scalar
chomping indicators. This version never rewrites the text of a document: it
validates and re-indents, so a document keeps its own quoting, comments, blank
lines and `---` markers. YAML 1.2 directives such as `%YAML 1.2` are accepted, and
a tab immediately after `:` is rejected the way the specification requires.

## Layout

```text
Cargo.toml                      Crate metadata, release profile, fixture generator
src/lib.rs                      Crate root and the four-space indent constant
src/format.rs                   Detection, dispatch and error messages
src/json.rs                     Strict JSON validation and indentation
src/yaml.rs                     YAML validation and indentation
src/wasm.rs                     WebAssembly exports: alloc, dealloc, format_input
src/bin/generate_stress.rs      Generates the fixtures in test/stress
formatter.js                    JavaScript half of the bridge
index.html                      Browser interface
test/json/, test/yaml/          Valid and invalid JSON and YAML fixtures
test/stress/                    Generated stress fixtures
test/wasm.test.cjs              Tests against the compiled WASM bridge
test/stress.test.cjs            Stress tests against the compiled WASM bridge
scripts/build.ps1               Builds formatter.wasm
formatter.wasm                  Generated WebAssembly module
```
