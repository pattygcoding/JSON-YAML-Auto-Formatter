# Stress Fixtures

These files are deterministic, synthetic data. Invalid files are intentional.

| File | Contents | Expected result |
| --- | --- | --- |
| `large-valid.json` | 3,000 nested records, compact JSON, large integers, escaped strings, arrays, nulls | JSON, four-space indentation |
| `large-invalid.json` | Same records with the final closing brace removed | Invalid JSON near end of input |
| `deep-valid.json` | 128 nested object/array pairs; formatting expands indentation substantially | JSON, four-space indentation |
| `large-valid.yaml` | 3,000 records with mappings, lists, multiline strings, and large integers | YAML, four-column indentation |
| `large-invalid.yaml` | Same records with an unterminated flow sequence at the end | Invalid YAML near end of input |
| `stream-valid.yaml` | 300 separate YAML documents | YAML, all documents retained |

Regenerate from the repository root:

```powershell
cargo run --release --bin generate-stress
```

Run native and compiled WASM stress tests:

```powershell
cargo test --test fixtures
.\scripts\build.ps1
node --test test/stress.test.cjs
```

Tests report input/output sizes and elapsed formatting time, validate both auto
and explicit modes, and check stable output and large-integer preservation.
Timings are observations, not performance thresholds. The browser's formatter is
synchronous, so large inputs can temporarily block the UI.

For a manual browser check, open a fixture in the editor and paste its contents
into the formatter at <http://127.0.0.1:8000>.
