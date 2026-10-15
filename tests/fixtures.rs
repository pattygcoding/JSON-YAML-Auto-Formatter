//! Regression tests over the committed fixtures, mirroring the Go test suite that
//! this project used before the move to Rust.

use std::path::Path;

use formatter::{format, Format};

const BIG_INTEGER: &str = "9007199254740993";

/// (path, expected format, expected to be rejected)
const FIXTURES: &[(&str, Format, bool)] = &[
    ("test/json/valid.json", Format::Json, false),
    ("test/json/invalid.json", Format::Json, true),
    ("test/yaml/valid.yaml", Format::Yaml, false),
    ("test/yaml/invalid.yaml", Format::Yaml, true),
    ("test/yaml/duplicate-keys.yaml", Format::Yaml, true),
    ("test/yaml/multiple-documents.yaml", Format::Yaml, false),
    ("test/yaml/list.yaml", Format::Yaml, false),
];

#[test]
fn fixtures_format_or_report_errors() {
    for (path, expected, invalid) in FIXTURES {
        let input = std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{path}: {error}"));
        for mode in ["auto", expected.as_str()] {
            let outcome = format(&input, mode);
            match outcome {
                Ok(result) => {
                    assert!(!invalid, "{path} [{mode}] was expected to be rejected");
                    assert_eq!(result.format, *expected, "{path} [{mode}]");
                    let repeated = format(&result.text, mode)
                        .unwrap_or_else(|error| panic!("{path} [{mode}]: {error}"));
                    assert_eq!(repeated, result, "{path} [{mode}] is not stable");
                }
                Err(error) => {
                    assert!(invalid, "{path} [{mode}] failed unexpectedly: {error}");
                    let prefix = format!("invalid {}:", expected.as_str().to_uppercase());
                    assert!(error.starts_with(&prefix), "{path} [{mode}] reported {error}");
                }
            }
        }
    }
}

#[test]
fn stress_fixtures_are_formatted_stably() {
    let files = stress_fixtures();
    assert_eq!(
        files.len(),
        6,
        "expected six generated stress fixtures; run cargo run --bin generate-stress"
    );

    for path in files {
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let expected = if path.extension().is_some_and(|extension| extension == "json") {
            Format::Json
        } else {
            Format::Yaml
        };
        let invalid = name.contains("-invalid.");
        let input = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{name}: {error}"));

        for mode in ["auto", expected.as_str()] {
            let outcome = format(&input, mode);
            match outcome {
                Ok(result) => {
                    assert!(!invalid, "{name} [{mode}] was expected to be rejected");
                    assert_eq!(result.format, expected, "{name} [{mode}] was misdetected");
                    assert!(
                        result.text.contains(BIG_INTEGER),
                        "{name} [{mode}] lost the large integer"
                    );
                    let repeated = format(&result.text, mode)
                        .unwrap_or_else(|error| panic!("{name} [{mode}]: {error}"));
                    assert_eq!(repeated.text, result.text, "{name} [{mode}] is not stable");
                }
                Err(error) => {
                    assert!(invalid, "{name} [{mode}] failed unexpectedly: {error}");
                    let prefix = format!("invalid {}:", expected.as_str().to_uppercase());
                    assert!(error.starts_with(&prefix), "{name} [{mode}] reported {error}");
                }
            }
        }
    }
}

fn stress_fixtures() -> Vec<std::path::PathBuf> {
    let directory = Path::new("test/stress");
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("test/stress: {error}"))
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            let extension = path.extension()?.to_str()?;
            if extension == "json" || extension == "yaml" {
                Some(path)
            } else {
                None
            }
        })
        .collect();
    files.sort();
    files
}
