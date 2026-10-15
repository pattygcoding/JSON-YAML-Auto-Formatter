//! Format selection: detect the input, dispatch to the right formatter, and wrap
//! failures in the same messages the JavaScript layer reports to users.

use crate::{json, yaml};

/// The format a document was treated as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Json,
    Yaml,
}

impl Format {
    /// Name used by the bridge and by the regression tests.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Yaml => "yaml",
        }
    }

    /// Upper-case name used in error messages.
    fn to_upper(self) -> &'static str {
        match self {
            Self::Json => "JSON",
            Self::Yaml => "YAML",
        }
    }
}

/// A successfully formatted document.
#[derive(Debug, PartialEq, Eq)]
pub struct Formatted {
    pub text: String,
    pub format: Format,
}

/// Formats `input` using `mode`, which is `"auto"`, `"json"` or `"yaml"`.
///
/// # Errors
///
/// Returns `no input provided` for blank input, `unknown format "x"` for an
/// unrecognised mode, and `invalid JSON: …` / `invalid YAML: …` when the
/// document does not parse.
pub fn format(input: &str, mode: &str) -> Result<Formatted, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("no input provided".to_string());
    }

    let mode = if mode.is_empty() || mode == "auto" {
        if json::is_valid(input) || looks_like_json(input) {
            Format::Json
        } else {
            Format::Yaml
        }
    } else {
        match mode {
            "json" => Format::Json,
            "yaml" => Format::Yaml,
            other => return Err(format!("unknown format \"{other}\"")),
        }
    };

    let text = match mode {
        Format::Json => json::indent(input),
        Format::Yaml => yaml::format(input),
    };
    text.map(|text| Formatted { text, format: mode })
        .map_err(|error| format!("invalid {}: {error}", mode.to_upper()))
}

/// Mirrors the historical detection heuristic used when input is not valid JSON.
///
/// Some JSON-like collections are already valid YAML, so they would otherwise be
/// interpreted as YAML and errors would be silently accepted. In auto-detect, an
/// input that starts like a JSON collection but does not validate as JSON keeps
/// going down the JSON path and reports a JSON error, which is what users expect
/// from "format this JSON". Selecting YAML explicitly still interprets it as YAML.
pub(crate) fn looks_like_json(input: &str) -> bool {
    let mut remaining = input;
    let mut stripped_brackets = false;
    while let Some(rest) = remaining.strip_prefix('[') {
        remaining = rest.trim_start();
        stripped_brackets = true;
    }

    if let Some(rest) = remaining.strip_prefix('{') {
        let rest = rest.trim_start();
        return rest.is_empty() || rest.starts_with('"') || rest.starts_with('}');
    }

    if !stripped_brackets {
        return false;
    }
    if remaining.is_empty() || remaining.starts_with(['"', '-', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', ']']) {
        return true;
    }
    ["true", "false", "null"].iter().any(|literal| {
        remaining == *literal
            || remaining.starts_with(&format!("{literal},"))
            || remaining.starts_with(&format!("{literal}]"))
            || remaining.starts_with(&format!("{literal} "))
    })
}

#[cfg(test)]
mod tests {
    use super::{format, Format};

    #[test]
    fn detects_json_and_yaml() {
        let cases = [
            (r#""https://example.com""#, Format::Json),
            (r#"[{"name":"test"}]"#, Format::Json),
            ("[1,2,3]", Format::Json),
            ("true", Format::Json),
            ("null", Format::Json),
            ("42", Format::Json),
            ("hello", Format::Yaml),
            (r#""name": test"#, Format::Yaml),
            ("{name: test, items: [one, two]}", Format::Yaml),
            ("[one, two]", Format::Yaml),
            ("- one\n- two", Format::Yaml),
            ("---\nname: test", Format::Yaml),
        ];
        for (input, expected) in cases {
            let result = format(input, "auto").unwrap_or_else(|error| panic!("{input}: {error}"));
            assert_eq!(result.format, expected, "input {input}");
        }
    }

    #[test]
    fn reports_blank_input_and_unknown_modes() {
        assert_eq!(format("", "auto").unwrap_err(), "no input provided");
        assert_eq!(format("  \n", "yaml").unwrap_err(), "no input provided");
        assert_eq!(format("name: test", "xml").unwrap_err(), "unknown format \"xml\"");
    }

    #[test]
    fn forced_modes_do_not_fall_back() {
        assert!(format("name: test", "json").unwrap_err().starts_with("invalid JSON:"));
        assert_eq!(format(r#"{"name":"test"}"#, "yaml").unwrap().format, Format::Yaml);
        assert_eq!(
            format("name: test", "json").unwrap_err(),
            "invalid JSON: invalid character 'a' in literal null (expecting 'u') at offset 1"
        );
    }

    #[test]
    fn malformed_json_reports_a_json_error_when_detecting() {
        for input in [r#"{"name":}"#, r#"[{"name":}]"#, "[1,2,]", "[true,false,]"] {
            let error = format(input, "auto").unwrap_err();
            assert!(error.starts_with("invalid JSON:"), "input {input} gave {error}");
        }
    }
}
