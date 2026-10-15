//! Strict JSON validation and four-space indentation.
//!
//! This mirrors the implementation the project used before the move to Rust
//! (`encoding/json.Valid` plus `encoding/json.Indent` with a four-space indent):
//! insignificant whitespace is dropped, every member of an object or array starts
//! on its own line, empty objects and arrays stay inline, and every other byte is
//! copied through untouched. Key order, numeric precision and existing string
//! escapes therefore survive formatting.

use crate::INDENT;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Container {
    Object,
    Array,
}

/// What the scanner expects to read next.
#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    /// A value: at the start of the document, an array element, or after `:`.
    Value,
    /// A value or `]`, immediately after `[`.
    ValueOrClose,
    /// An object key string.
    Key,
    /// An object key string or `}`, immediately after `{`.
    KeyOrClose,
    /// The `:` separating an object key from its value.
    Colon,
    /// A `,` or the closing bracket of the current container.
    AfterValue,
    /// The single top-level value is complete; only whitespace may follow.
    Top,
}

enum Token<'a> {
    Open(char),
    Close(char),
    Comma,
    Colon,
    Scalar(&'a str),
}

/// Reports whether `input` is exactly one complete JSON value.
#[must_use]
pub fn is_valid(input: &str) -> bool {
    scan(input, None).is_ok()
}

/// Re-indents `input` with four spaces per level, or reports why it is not JSON.
pub fn indent(input: &str) -> Result<String, String> {
    let mut output = String::new();
    scan(input, Some(&mut output))?;
    Ok(output)
}

fn scan(input: &str, mut output: Option<&mut String>) -> Result<(), String> {
    let bytes = input.as_bytes();
    let mut emitter = Emitter::new(output.is_some());
    let mut stack: Vec<Container> = Vec::new();
    let mut state = State::Value;
    let mut position = 0usize;

    loop {
        if state == State::Top {
            // Whitespace after the top-level value is copied through verbatim.
            let start = position;
            while position < bytes.len() && is_space(bytes[position]) {
                position += 1;
            }
            if position < bytes.len() {
                return Err(invalid_character(bytes[position], "after top-level value", position));
            }
            emitter.raw(&input[start..position]);
            emitter.flush(output.as_deref_mut());
            return Ok(());
        }

        while position < bytes.len() && is_space(bytes[position]) {
            position += 1;
        }
        if position >= bytes.len() {
            return Err("unexpected end of JSON input".to_string());
        }

        match next_token(input, &mut position, &mut state, &mut stack)? {
            Token::Open(character) => emitter.open(character),
            Token::Close(character) => emitter.close(character),
            Token::Comma => emitter.comma(),
            Token::Colon => emitter.colon(),
            Token::Scalar(raw) => emitter.scalar(raw),
        }
    }
}

fn next_token<'a>(
    input: &'a str,
    position: &mut usize,
    state: &mut State,
    stack: &mut Vec<Container>,
) -> Result<Token<'a>, String> {
    let byte = input.as_bytes()[*position];
    match *state {
        State::Value | State::ValueOrClose => {
            if byte == b']' && *state == State::ValueOrClose {
                return close_container(input, position, state, stack, ']');
            }
            parse_value(input, position, state, stack)
        }
        State::Key | State::KeyOrClose => {
            if byte == b'}' && *state == State::KeyOrClose {
                return close_container(input, position, state, stack, '}');
            }
            if byte != b'"' {
                return Err(invalid_character(byte, "looking for beginning of object key string", *position));
            }
            let raw = parse_string(input, position)?;
            *state = State::Colon;
            Ok(Token::Scalar(raw))
        }
        State::Colon => {
            if byte != b':' {
                return Err(invalid_character(byte, "after object key", *position));
            }
            *position += 1;
            *state = State::Value;
            Ok(Token::Colon)
        }
        State::AfterValue => {
            let container = stack.last().copied();
            match byte {
                b',' => {
                    *position += 1;
                    *state = if container == Some(Container::Object) { State::Key } else { State::Value };
                    Ok(Token::Comma)
                }
                b'}' if container == Some(Container::Object) => {
                    close_container(input, position, state, stack, '}')
                }
                b']' if container == Some(Container::Array) => {
                    close_container(input, position, state, stack, ']')
                }
                other => Err(invalid_character(
                    other,
                    if container == Some(Container::Object) {
                        "after object key:value pair"
                    } else {
                        "after array element"
                    },
                    *position,
                )),
            }
        }
        State::Top => Err("unexpected end of JSON input".to_string()),
    }
}
fn parse_value<'a>(
    input: &'a str,
    position: &mut usize,
    state: &mut State,
    stack: &mut Vec<Container>,
) -> Result<Token<'a>, String> {
    let byte = input.as_bytes()[*position];
    let token = match byte {
        b'{' => {
            stack.push(Container::Object);
            *position += 1;
            *state = State::KeyOrClose;
            return Ok(Token::Open('{'));
        }
        b'[' => {
            stack.push(Container::Array);
            *position += 1;
            *state = State::ValueOrClose;
            return Ok(Token::Open('['));
        }
        b'"' => Token::Scalar(parse_string(input, position)?),
        b't' => Token::Scalar(parse_literal(input, position, "true")?),
        b'f' => Token::Scalar(parse_literal(input, position, "false")?),
        b'n' => Token::Scalar(parse_literal(input, position, "null")?),
        b'-' | b'0'..=b'9' => Token::Scalar(parse_number(input, position)?),
        other => return Err(invalid_character(other, "looking for beginning of value", *position)),
    };
    *state = if stack.is_empty() { State::Top } else { State::AfterValue };
    Ok(token)
}

fn close_container<'a>(
    input: &'a str,
    position: &mut usize,
    state: &mut State,
    stack: &mut Vec<Container>,
    closing: char,
) -> Result<Token<'a>, String> {
    let expected = if closing == '}' { Container::Object } else { Container::Array };
    if stack.pop() != Some(expected) {
        return Err(invalid_character(input.as_bytes()[*position], "after top-level value", *position));
    }
    *position += 1;
    *state = if stack.is_empty() { State::Top } else { State::AfterValue };
    Ok(Token::Close(closing))
}

fn parse_string<'a>(input: &'a str, position: &mut usize) -> Result<&'a str, String> {
    let bytes = input.as_bytes();
    let start = *position;
    *position += 1;
    while *position < bytes.len() {
        match bytes[*position] {
            b'"' => {
                *position += 1;
                return Ok(&input[start..*position]);
            }
            b'\\' => {
                *position += 1;
                let escape = match bytes.get(*position).copied() {
                    Some(escape) => escape,
                    None => return Err("unexpected end of JSON input".to_string()),
                };
                match escape {
                    b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => *position += 1,
                    b'u' => {
                        *position += 1;
                        for _ in 0..4 {
                            let digit = match bytes.get(*position).copied() {
                                Some(digit) => digit,
                                None => return Err("unexpected end of JSON input".to_string()),
                            };
                            if !digit.is_ascii_hexdigit() {
                                return Err(invalid_character(
                                    digit,
                                    "in \\u hexadecimal character escape",
                                    *position,
                                ));
                            }
                            *position += 1;
                        }
                    }
                    other => return Err(invalid_character(other, "in string escape code", *position)),
                }
            }
            control if control < 0x20 => {
                return Err(invalid_character(control, "in string literal", *position));
            }
            _ => *position += 1,
        }
    }
    Err("unexpected end of JSON input".to_string())
}
fn parse_literal<'a>(input: &'a str, position: &mut usize, literal: &str) -> Result<&'a str, String> {
    let bytes = input.as_bytes();
    for (offset, expected) in literal.bytes().enumerate() {
        let actual = match bytes.get(*position + offset).copied() {
            Some(actual) => actual,
            None => return Err("unexpected end of JSON input".to_string()),
        };
        if actual != expected {
            return Err(format!(
                "invalid character {} in literal {literal} (expecting {}) at offset {}",
                quoted_character(actual),
                quoted_character(expected),
                *position + offset
            ));
        }
    }
    let start = *position;
    *position += literal.len();
    Ok(&input[start..*position])
}

fn parse_number<'a>(input: &'a str, position: &mut usize) -> Result<&'a str, String> {
    let bytes = input.as_bytes();
    let start = *position;
    if bytes[*position] == b'-' {
        *position += 1;
    }
    match bytes.get(*position).copied() {
        Some(b'0') => *position += 1,
        Some(b'1'..=b'9') => {
            while matches!(bytes.get(*position), Some(b'0'..=b'9')) {
                *position += 1;
            }
        }
        Some(other) => return Err(invalid_character(other, "in numeric literal", *position)),
        None => return Err("unexpected end of JSON input".to_string()),
    }
    if bytes.get(*position) == Some(&b'.') {
        *position += 1;
        let first_digit = *position;
        while matches!(bytes.get(*position), Some(b'0'..=b'9')) {
            *position += 1;
        }
        if *position == first_digit {
            return Err(match bytes.get(*position).copied() {
                Some(other) => invalid_character(other, "after decimal point in numeric literal", *position),
                None => "unexpected end of JSON input".to_string(),
            });
        }
    }
    if matches!(bytes.get(*position), Some(b'e' | b'E')) {
        *position += 1;
        if matches!(bytes.get(*position), Some(b'+' | b'-')) {
            *position += 1;
        }
        let first_digit = *position;
        while matches!(bytes.get(*position), Some(b'0'..=b'9')) {
            *position += 1;
        }
        if *position == first_digit {
            return Err(match bytes.get(*position).copied() {
                Some(other) => invalid_character(other, "in exponent of numeric literal", *position),
                None => "unexpected end of JSON input".to_string(),
            });
        }
    }
    Ok(&input[start..*position])
}
/// Mirrors `encoding/json.Indent`: whitespace between tokens is dropped, and a
/// newline plus one indent per nesting level is inserted before each member. The
/// indent is deferred until a token really arrives, which is what keeps `{}` and
/// `[]` on a single line.
struct Emitter {
    enabled: bool,
    text: String,
    pending_indent: bool,
    depth: usize,
}

impl Emitter {
    fn new(enabled: bool) -> Self {
        Self { enabled, text: String::new(), pending_indent: false, depth: 0 }
    }

    fn newline(&mut self, depth: usize) {
        if !self.enabled {
            return;
        }
        self.text.push('\n');
        for _ in 0..depth {
            self.text.push_str(INDENT);
        }
    }

    /// Runs before every token that opens or fills a container.
    fn before_token(&mut self) {
        if self.pending_indent {
            self.pending_indent = false;
            self.depth += 1;
            let depth = self.depth;
            self.newline(depth);
        }
    }

    fn open(&mut self, character: char) {
        self.before_token();
        if self.enabled {
            self.text.push(character);
        }
        self.pending_indent = true;
    }

    fn close(&mut self, character: char) {
        if self.pending_indent {
            self.pending_indent = false;
        } else {
            self.depth = self.depth.saturating_sub(1);
            let depth = self.depth;
            self.newline(depth);
        }
        if self.enabled {
            self.text.push(character);
        }
    }

    fn comma(&mut self) {
        if self.enabled {
            self.text.push(',');
        }
        let depth = self.depth;
        self.newline(depth);
    }

    fn colon(&mut self) {
        if self.enabled {
            self.text.push_str(": ");
        }
    }

    fn scalar(&mut self, raw: &str) {
        self.before_token();
        if self.enabled {
            self.text.push_str(raw);
        }
    }

    fn raw(&mut self, raw: &str) {
        if self.enabled {
            self.text.push_str(raw);
        }
    }

    fn flush(self, output: Option<&mut String>) {
        if let Some(output) = output {
            output.push_str(&self.text);
        }
    }
}

fn is_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r')
}

fn invalid_character(byte: u8, context: &str, offset: usize) -> String {
    format!("invalid character {} {context} at offset {offset}", quoted_character(byte))
}

/// Renders a byte the way `encoding/json` reports it in scanner errors.
fn quoted_character(byte: u8) -> String {
    match byte {
        b'\'' => "'\\''".to_string(),
        b'"' => "'\"'".to_string(),
        control if control < b' ' => format!("'\\x{control:02x}'"),
        other => format!("'{}'", other as char),
    }
}
#[cfg(test)]
mod tests {
    use super::{indent, is_valid};

    /// (input, expected output) pairs captured from Go's `encoding/json`, so the
    /// Rust port keeps byte-for-byte parity for valid documents.
    const VALID: &[(&str, &str)] = &[
        (r"{}", "{}"),
        (r"[]", "[]"),
        (r#"{"a":{}}"#, "{\n    \"a\": {}\n}"),
        (r#"{"a":[]}"#, "{\n    \"a\": []\n}"),
        ("{}   ", "{}   "),
        ("[ ]", "[]"),
        ("{ }", "{}"),
        ("[\n]", "[]"),
        ("{\n}", "{}"),
        ("[ 1 , 2 ]", "[\n    1,\n    2\n]"),
        (r#""\uD800""#, r#""\uD800""#),
        (r#""\u0041""#, r#""\u0041""#),
        ("-0", "-0"),
        ("0.5e10", "0.5e10"),
        ("1E5", "1E5"),
        ("9007199254740993", "9007199254740993"),
        ("null", "null"),
        ("[true,false,null]", "[\n    true,\n    false,\n    null\n]"),
        (r#"{"":1}"#, "{\n    \"\": 1\n}"),
        ("{\n\"a\":1\n}", "{\n    \"a\": 1\n}"),
        ("{\"a\":1}\n\n", "{\n    \"a\": 1\n}\n\n"),
        ("{\"a\":1} ", "{\n    \"a\": 1\n} "),
        (r#"{"a":1,"a":2}"#, "{\n    \"a\": 1,\n    \"a\": 2\n}"),
        (
            r#"{"nested":{"number":9007199254740993},"html":"<b>"}"#,
            "{\n    \"nested\": {\n        \"number\": 9007199254740993\n    },\n    \"html\": \"<b>\"\n}",
        ),
        (
            r#"{ "a" : {"b" : [ 1 , 2 ] } }"#,
            "{\n    \"a\": {\n        \"b\": [\n            1,\n            2\n        ]\n    }\n}",
        ),
        (
            r#"[{"a":1},{"b":2}]"#,
            "[\n    {\n        \"a\": 1\n    },\n    {\n        \"b\": 2\n    }\n]",
        ),
        (
            "[[[[1]]]]",
            "[\n    [\n        [\n            [\n                1\n            ]\n        ]\n    ]\n]",
        ),
        (
            r#"{"a":[{"b":[]}]}"#,
            "{\n    \"a\": [\n        {\n            \"b\": []\n        }\n    ]\n}",
        ),
    ];

    /// Inputs Go's `encoding/json` rejects; the Rust port must reject them too so
    /// auto-detection keeps sending malformed JSON down the JSON error path.
    const INVALID: &[&str] = &[
        "1.", "1.x", "01", "-", "1e", "1e+", "[1,]", "{\"a\":1,}", "tru", "{\"a\" 1}", "{}extra",
        "[1 2]", r#""a\qb""#, r#"{"a":"\u12"}"#, "[1,2]x", "  ", "", "{\"a\":1}{\"b\":2}", "[1,,2]",
        "[,1]", r#"{"a"::1}"#, "\"a\tb\"", "NaN", "Infinity", "'a'", "{a:1}", "// comment\n{}",
        "0x10", "+1", "1_000",
    ];

    #[test]
    fn valid_documents_match_go_indentation() {
        for (input, expected) in VALID {
            assert!(is_valid(input), "expected {input} to be valid");
            assert_eq!(indent(input).unwrap(), *expected, "input {input}");
        }
    }

    #[test]
    fn invalid_documents_are_rejected() {
        for input in INVALID {
            assert!(!is_valid(input), "expected {input} to be invalid");
            assert!(indent(input).is_err(), "input {input}");
        }
    }

    #[test]
    fn errors_name_the_offending_character() {
        assert_eq!(
            indent("{\"a\":1,}").unwrap_err(),
            "invalid character '}' looking for beginning of object key string at offset 7"
        );
        assert_eq!(
            indent("{}extra").unwrap_err(),
            "invalid character 'e' after top-level value at offset 2"
        );
        assert_eq!(
            indent("[1 2]").unwrap_err(),
            "invalid character '2' after array element at offset 3"
        );
        assert_eq!(
            indent("\"a\tb\"").unwrap_err(),
            "invalid character '\\x09' in string literal at offset 2"
        );
    }
}

