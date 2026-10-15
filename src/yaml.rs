//! YAML validation and four-space indentation.
//!
//! Validation runs through `saphyr`, a YAML 1.2 parser, so malformed documents and
//! duplicate mapping keys are reported instead of silently accepted. The document
//! is never re-serialised after that: every line keeps its own text and only its
//! leading indentation is recomputed. That is what lets comments, anchors, aliases,
//! tags, quoting style, block scalars and flow collections survive formatting
//! exactly as written, including scalar text that no printer would reproduce
//! byte-for-byte (for example `9007199254740993`).
//!
//! The indentation model matches what the Go version produced and what `libyaml`
//! does: one value block is indented one step (four columns) past its parent, and
//! the members of a mapping that starts on a `- ` line are aligned two columns
//! past the dash.

use saphyr::LoadableYamlNode;

use crate::INDENT;

/// Width of one indentation step.
const STEP: usize = INDENT.len();

/// Validates `input` and returns it re-indented with four spaces per level.
///
/// # Errors
///
/// Returns the parser's message for malformed YAML (unknown anchors included),
/// a duplicate-key error when a mapping key repeats — `duplicated key in mapping`
/// — and `no YAML document provided` for a document that holds nothing but
/// comments.
pub fn format(input: &str) -> Result<String, String> {
    let documents = saphyr::Yaml::load_from_str(input).map_err(|error| error.to_string())?;
    if documents.is_empty() {
        return Err("no YAML document provided".to_string());
    }
    reindent(input)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Map,
    Seq,
}

/// One nesting level of the document being re-indented.
#[derive(Clone, Copy)]
struct Frame {
    kind: Kind,
    /// Original indentation of this level's own lines (key column or dash column).
    orig: usize,
    /// Indentation this level's lines are written with.
    new: usize,
    /// Original indentation of the collection that owns this level. A block
    /// scalar's content is indented one step past it.
    block_orig: usize,
    /// Indentation of the collection that owns this level. Value blocks are
    /// indented one step past it, which is what keeps nested sequences inside a
    /// `- ` mapping aligned the way the Go version aligned them.
    block_new: usize,
    /// Set when this level's most recent line was `key:` with no value after it,
    /// which is how a `- ` at the same indentation is recognised as its sequence.
    pending_value: bool,
}

/// Everything the re-indenter needs to know about one line's structure.
#[derive(Default, Clone, Copy)]
struct LineScan {
    /// Number of leading spaces.
    indent: usize,
    /// The line is a comment (or blank, which reports `indent == len`).
    comment_only: bool,
    /// The line starts with a `%` directive.
    directive: bool,
    /// The line is a `---` or `...` document marker.
    document_marker: bool,
    /// The line starts a block sequence entry (`- ` or a bare `-`).
    seq_item: bool,
    /// Column where the text after `- ` starts.
    item_start: usize,
    /// The entry's content starts a mapping on this line (`- key: value`).
    item_mapping: bool,
    /// The line ends with `key:` and nothing else (optionally a comment).
    bare_key: bool,
    /// The line is a `|` or `>` block scalar header.
    block_scalar: bool,
    /// Explicit indentation indicator (`|2`), when the header has one.
    block_indent: Option<usize>,
    /// Flow brackets left open at the end of the line.
    flow_depth: usize,
    /// Net flow bracket change on the line; negative when it closes more than it
    /// opens, which is how a multi-line flow collection is followed to its end.
    flow_delta: isize,
    /// Quote left open at the end of the line.
    quote: Option<u8>,
}

/// State carried across the lines of a multi-line quoted scalar or flow collection.
#[derive(Clone, Copy)]
struct Continuation {
    /// Indentation of the line that opened the scalar or collection.
    indent: usize,
    depth: usize,
    quote: Option<u8>,
}

/// A block scalar whose content lines are being re-indented.
#[derive(Clone, Copy)]
struct BlockScalar {
    /// Original indentation of the content; discovered from the first content
    /// line unless the header carried an explicit indicator.
    base_orig: Option<usize>,
    /// Indentation the content is written with.
    base_new: usize,
}
/// Scans one line's structure without interpreting its text.
///
/// `initial_quote` continues a quoted scalar that started on an earlier line, in
/// which case the structural fields stay empty: everything on such a line is
/// scalar content.
fn scan_line(line: &str, initial_quote: Option<u8>) -> LineScan {
    let bytes = line.as_bytes();
    let mut scan = LineScan { quote: initial_quote, ..LineScan::default() };
    let mut start = 0;
    while start < bytes.len() && bytes[start] == b' ' {
        start += 1;
    }
    scan.indent = start;
    if start >= bytes.len() {
        scan.comment_only = true;
        return scan;
    }

    if initial_quote.is_none() {
        let text = &line[start..];
        if text.starts_with('#') {
            scan.comment_only = true;
            return scan;
        }
        if text.starts_with('%') {
            scan.directive = true;
            return scan;
        }
        if is_document_marker(text) {
            scan.document_marker = true;
            return scan;
        }
        if text.starts_with('-') && (text.len() == 1 || text.as_bytes()[1] == b' ') {
            scan.seq_item = true;
            let mut content = start + 1;
            while content < bytes.len() && bytes[content] == b' ' {
                content += 1;
            }
            scan.item_start = content;
        }
    }

    // Walk the line once, tracking quotes, flow depth and key separators. Only a
    // `:` outside quotes and flow collections separates a key from its value.
    let mut depth: isize = 0;
    let mut quote = initial_quote;
    let mut position = start;
    let mut key_colon: Option<usize> = None;
    while position < bytes.len() {
        let byte = bytes[position];
        if let Some(active) = quote {
            if active == b'"' {
                if byte == b'\\' {
                    position += 2;
                    continue;
                }
                if byte == b'"' {
                    quote = None;
                }
            } else if byte == b'\'' {
                if bytes.get(position + 1) == Some(&b'\'') {
                    position += 2;
                    continue;
                }
                quote = None;
            }
            position += 1;
            continue;
        }
        match byte {
            b'#' if position == start || bytes[position - 1] == b' ' || bytes[position - 1] == b'\t' => break,
            b'"' | b'\'' => {
                quote = Some(byte);
                position += 1;
            }
            b'[' | b'{' => {
                depth += 1;
                position += 1;
            }
            b']' | b'}' => {
                depth -= 1;
                position += 1;
            }
            b':' if depth == 0 => {
                let next = bytes.get(position + 1).copied();
                let separator = next.is_none() || next == Some(b' ') || next == Some(b'\t') || next == Some(b'#');
                if separator && key_colon.is_none() {
                    key_colon = Some(position);
                }
                position += 1;
            }
            _ => position += 1,
        }
    }
    scan.flow_depth = depth.max(0) as usize;
    scan.flow_delta = depth;
    scan.quote = quote;
    if initial_quote.is_some() {
        return scan;
    }

    scan.item_mapping = scan.seq_item && key_colon.is_some_and(|colon| colon >= scan.item_start);
    scan.bare_key = key_colon.is_some_and(|colon| {
        let remainder = line[colon + 1..].trim_start();
        remainder.is_empty() || remainder.starts_with('#')
    });

    // A block scalar is introduced either by a key on this line (`key: |`) or by
    // the entry itself (`- |`).
    let header = match key_colon {
        Some(colon) => Some(&line[colon + 1..]),
        None if scan.seq_item => Some(&line[scan.item_start..]),
        None => None,
    };
    if let Some(header) = header {
        if let Some(indicator) = parse_block_header(header) {
            scan.block_scalar = true;
            scan.block_indent = indicator;
        }
    }
    scan
}
/// Recognises a `|`, `>`, `|2`, `|-`, `>2+` style header. The outer `Option`
/// reports whether this is a block scalar at all; the inner value is the explicit
/// indentation indicator when the header carries one.
fn parse_block_header(header: &str) -> Option<Option<usize>> {
    let mut rest = header.trim_start();
    let kind = rest.chars().next()?;
    if kind != '|' && kind != '>' {
        return None;
    }
    rest = &rest[1..];

    let mut indicator = None;
    if let Some(digit) = rest.chars().next().and_then(|character| character.to_digit(10)) {
        if (1..=9).contains(&digit) {
            indicator = Some(digit as usize);
            rest = &rest[1..];
        }
    }
    for _ in 0..2 {
        if matches!(rest.chars().next(), Some('+' | '-')) {
            rest = &rest[1..];
        }
    }

    let tail = rest.trim_start();
    if tail.is_empty() || tail.starts_with('#') {
        Some(indicator)
    } else {
        None
    }
}

fn is_document_marker(text: &str) -> bool {
    text == "---" || text.starts_with("--- ") || text == "..." || text.starts_with("... ")
}

/// Validates anchor usage and rewrites the indentation of every line.
///
/// Output is produced line by line: the text of a line (minus its leading spaces)
/// is copied verbatim, so nothing except indentation and insignificant trailing
/// whitespace can change.
fn reindent(input: &str) -> Result<String, String> {
    let mut output = String::with_capacity(input.len() + input.len() / 8);
    let mut frames: Vec<Frame> = Vec::new();
    let mut continuation: Option<Continuation> = None;
    let mut block: Option<BlockScalar> = None;
    let mut first_line = true;

    for raw_line in input.split('\n') {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        if first_line {
            first_line = false;
        } else {
            output.push('\n');
        }

        let incoming_quote = continuation.and_then(|state| state.quote);
        let scan = scan_line(line, incoming_quote);

        // 1. Content of a block scalar: only the leading indentation moves, and
        //    trailing whitespace is kept because it belongs to the value.
        if let Some(active) = block {
            if scan.indent >= line.len() {
                continue;
            }
            let base_orig = match active.base_orig {
                Some(base) => base,
                None => {
                    block = Some(BlockScalar { base_orig: Some(scan.indent), base_new: active.base_new });
                    scan.indent
                }
            };
            if scan.indent >= base_orig {
                push_indent(&mut output, active.base_new + (scan.indent - base_orig));
                output.push_str(&line[scan.indent..]);
                continue;
            }
            block = None;
        }

        // 2. Continuation of a multi-line quoted scalar or flow collection: the
        //    text is kept as written, indented one step past the opening line.
        if let Some(active) = continuation {
            if scan.indent < line.len() {
                push_indent(&mut output, active.indent + STEP);
                output.push_str(&line[scan.indent..]);
            }
            let depth = (active.depth as isize + scan.flow_delta).max(0) as usize;
            continuation = if depth == 0 && scan.quote.is_none() {
                None
            } else {
                Some(Continuation { indent: active.indent, depth, quote: scan.quote })
            };
            continue;
        }

        // 3. Blank, comment, directive and document marker lines.
        if scan.indent >= line.len() {
            continue;
        }
        if scan.comment_only {
            let indent = frames.last().map_or(0, |frame| frame.new);
            push_indent(&mut output, indent);
            output.push_str(line[scan.indent..].trim_end());
            continue;
        }
        if scan.directive {
            output.push_str(line[scan.indent..].trim_end());
            continue;
        }
        if scan.document_marker {
            frames.clear();
            block = None;
            output.push_str(line[scan.indent..].trim_end());
            continue;
        }
        // 4. A structural line: first drop the levels it is no longer inside.
        while let Some(top) = frames.last() {
            let closes = scan.indent < top.orig
                || (!scan.seq_item && top.kind == Kind::Seq && scan.indent <= top.orig);
            if !closes {
                break;
            }
            frames.pop();
        }

        match frames.last().copied() {
            None => frames.push(Frame {
                kind: if scan.seq_item { Kind::Seq } else { Kind::Map },
                orig: scan.indent,
                new: 0,
                block_orig: scan.indent,
                block_new: 0,
                pending_value: scan.bare_key,
            }),
            Some(top)
                if scan.indent > top.orig
                    || (scan.seq_item && top.kind != Kind::Seq && top.pending_value) =>
            {
                // A value block one step in, or a `- ` belonging to the bare
                // `key:` above it even though it sits at the same indentation.
                let new = top.block_new + STEP;
                frames.push(Frame {
                    kind: if scan.seq_item { Kind::Seq } else { Kind::Map },
                    orig: scan.indent,
                    new,
                    block_orig: scan.indent,
                    block_new: new,
                    pending_value: scan.bare_key,
                });
            }
            Some(top) => {
                if let Some(frame) = frames.last_mut() {
                    *frame = Frame { pending_value: scan.bare_key, ..top };
                }
            }
        }

        let level = frames.last().map_or(0, |frame| frame.new);

        // A `- key: value` line continues as a mapping aligned two columns past the
        // dash, which is the shape the Go version produced and the shape `libyaml`
        // uses.
        if scan.seq_item && scan.item_mapping {
            let offset = scan.item_start - scan.indent;
            frames.push(Frame {
                kind: Kind::Map,
                orig: scan.item_start,
                new: level + offset,
                block_orig: scan.indent,
                block_new: level,
                pending_value: scan.bare_key,
            });
        }

        push_indent(&mut output, level);
        let text = &line[scan.indent..];
        if scan.quote.is_some() || scan.flow_depth > 0 {
            // The line continues inside a scalar: keep it exactly as written.
            output.push_str(text);
        } else {
            output.push_str(text.trim_end());
        }

        if scan.block_scalar {
            let (block_orig, block_new) = frames
                .last()
                .map_or((scan.indent, level), |frame| (frame.block_orig, frame.block_new));
            block = Some(BlockScalar {
                base_orig: scan.block_indent.map(|indicator| block_orig + indicator),
                base_new: scan.block_indent.map_or(block_new + STEP, |indicator| block_new + indicator),
            });
        } else if scan.flow_depth > 0 || scan.quote.is_some() {
            continuation = Some(Continuation { indent: level, depth: scan.flow_depth, quote: scan.quote });
        }
    }

    Ok(output)
}

/// Writes `columns` spaces. Levels are tracked as columns rather than counts, so
/// that the two-column alignment of mappings inside `- ` entries is exact.
fn push_indent(output: &mut String, columns: usize) {
    for _ in 0..columns {
        output.push(' ');
    }
}
#[cfg(test)]
mod tests {
    use super::format;

    /// Formats `input` twice and checks the second pass changes nothing, then
    /// returns the result: every case below doubles as an idempotence test.
    fn stable(input: &str) -> String {
        let once = format(input).unwrap_or_else(|error| panic!("{input}\n{error}"));
        let twice = format(&once).unwrap_or_else(|error| panic!("{once}\n{error}"));
        assert_eq!(once, twice, "formatting is not stable");
        once
    }

    #[test]
    fn converts_fixture_shaped_documents_to_four_spaces() {
        let input = "# Formatter settings\nname: formatter\nnested:\n  enabled: true\n  items:\n    - one\n    - two\nmessage: |\n  Keep this text.\n  And this line.";
        let expected = "# Formatter settings\nname: formatter\nnested:\n    enabled: true\n    items:\n        - one\n        - two\nmessage: |\n    Keep this text.\n    And this line.";
        assert_eq!(stable(input), expected);
    }

    #[test]
    fn keeps_comments_anchors_and_documents() {
        let input = "# settings\ndefaults: &defaults\n  enabled: true\ncopy: *defaults\nmessage: |\n  first\n  second\n---\nname: second";
        let expected = "# settings\ndefaults: &defaults\n    enabled: true\ncopy: *defaults\nmessage: |\n    first\n    second\n---\nname: second";
        assert_eq!(stable(input), expected);
    }

    #[test]
    fn indents_sequences_and_compact_sequences() {
        assert_eq!(stable("- one\n- two"), "- one\n- two");
        assert_eq!(stable("items:\n- one\n- two"), "items:\n    - one\n    - two");
        assert_eq!(stable("value:\n  items:\n    - one\n    - two"), "value:\n    items:\n        - one\n        - two");
        assert_eq!(stable("- tags:\n  - one"), "- tags:\n    - one");
    }

    #[test]
    fn aligns_mappings_inside_sequence_entries_without_moving_them() {
        let input = "records:\n    - id: 9007199254740993\n      tags:\n        - production\n      metadata:\n        owner: team-00\n      notes: |\n        First line.\n        Second line.\n      optional: null";
        assert_eq!(stable(input), input);
    }

    #[test]
    fn reindents_two_space_sequence_entries() {
        let input = "records:\n  - id: 1\n    tags:\n      - production\n    metadata:\n      owner: team-00\n    notes: |\n      First line.";
        let expected = "records:\n    - id: 1\n      tags:\n        - production\n      metadata:\n        owner: team-00\n      notes: |\n        First line.";
        assert_eq!(stable(input), expected);
    }

    #[test]
    fn keeps_indented_comments_with_their_level() {
        let input = "outer:\n  inner: 1\n  # indented comment\n  other: 2";
        let expected = "outer:\n    inner: 1\n    # indented comment\n    other: 2";
        assert_eq!(stable(input), expected);
    }

    #[test]
    fn keeps_trailing_comments_blank_lines_and_tabs() {
        assert_eq!(stable("name: value # trailing comment"), "name: value # trailing comment");
        assert_eq!(stable("name: value\n\n\nother: 2"), "name: value\n\n\nother: 2");
        assert_eq!(stable("body: |\n  column\tvalue\nnext: 1"), "body: |\n    column\tvalue\nnext: 1");
    }

    #[test]
    fn keeps_scalars_that_span_lines() {
        assert_eq!(
            stable("items: [one,\n  two,\n  three]\nnext: value"),
            "items: [one,\n    two,\n    three]\nnext: value"
        );
        assert_eq!(
            stable("key: this is a long\n  plain scalar continued\nnext: value"),
            "key: this is a long\n    plain scalar continued\nnext: value"
        );
        assert_eq!(
            stable("key: \"first part\n  second part\"\nnext: value"),
            "key: \"first part\n    second part\"\nnext: value"
        );
    }

    #[test]
    fn keeps_block_scalar_headers_trailing_whitespace_and_content() {
        assert_eq!(stable("message: >\n  folded text\nnext: value"), "message: >\n    folded text\nnext: value");
        assert_eq!(stable("message: |2\n   indented body\nnext: value"), "message: |2\n   indented body\nnext: value");
        assert_eq!(stable("message: |-\n  keep\n  trailing\n"), "message: |-\n    keep\n    trailing\n");
        assert_eq!(stable("body: |\n  text   \nnext: 1"), "body: |\n    text   \nnext: 1");
        assert_eq!(stable("key: value   \nother: 2\t"), "key: value\nother: 2");
    }

    #[test]
    fn keeps_directives_markers_and_scalars() {
        assert_eq!(stable("---\nname: test"), "---\nname: test");
        assert_eq!(stable("---"), "---");
        assert_eq!(stable("hello"), "hello");
        assert_eq!(stable("%YAML 1.2\n---\nname: test"), "%YAML 1.2\n---\nname: test");
        assert_eq!(stable("{name: test, items: [one, two]}"), "{name: test, items: [one, two]}");
    }

    #[test]
    fn rejects_malformed_yaml_duplicate_keys_and_unknown_aliases() {
        for input in [
            "name: formatter\nitems: [one, two",
            "name: first\n---\nitems: [broken",
            "name: first\nname: second",
            "name: *missing",
        ] {
            assert!(format(input).is_err(), "expected {input} to be rejected");
        }
        let alias_error = format("name: *missing").unwrap_err();
        assert!(alias_error.contains("anchor"), "unexpected message: {alias_error}");
        assert!(format("name: first\nname: second").unwrap_err().contains("duplicated key"));
        assert_eq!(format("# comment only").unwrap_err(), "no YAML document provided");
    }

    #[test]
    fn resolves_anchors_defined_in_the_same_document() {
        assert_eq!(stable("base: &b\n    x: 1\ncopy: *b"), "base: &b\n    x: 1\ncopy: *b");
        // Anchors are document scoped, so the alias below has nothing to resolve to.
        assert!(format("a: &b 1\n---\nc: *b").unwrap_err().contains("anchor"));
    }

    #[test]
    fn ignores_asterisks_that_are_not_aliases() {
        let input = "glob: '*.txt'\nmath: 2 * 3\n# *not an alias\nend: 1";
        assert_eq!(stable(input), input);
    }

    #[test]
    fn generated_stress_shaped_input_stays_stable() {
        let mut input = String::new();
        for index in 0..40 {
            if index > 0 {
                input.push_str("\n---\n");
            }
            input.push_str(&format!(
                "id: 9007199254740993\nname: service-{index:04}\nenabled: false\ntags:\n  - production\n  - region-{}\nmetadata:\n  owner: team-00\n  url: https://example.com/items/{index}\nnotes: |\n  First line with a colon: value and # hash.\n  Second line with \"quotes\" and <html>.",
                index % 12
            ));
        }
        let formatted = stable(&input);
        assert!(formatted.contains("9007199254740993"));
        assert!(formatted.contains("\ntags:\n    - production\n"));
        assert!(formatted.contains("\nnotes: |\n    First line with a colon: value and # hash."));
        assert!(formatted.contains("\n---\n"));
    }
}


