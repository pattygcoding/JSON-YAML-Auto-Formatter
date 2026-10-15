//! Generates the stress fixtures in `test/stress`.
//!
//! Run from the repository root:
//!
//! ```powershell
//! cargo run --bin generate-stress
//! ```
//!
//! The fixtures are deliberately large — a few thousand records, 300 documents and
//! 128 levels of nesting — so the formatter can be exercised on input far bigger
//! than a hand written example, and so its stability can be checked at that size.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

const RECORDS: usize = 3000;
const STREAM_DOCUMENTS: usize = 300;
const NESTING_DEPTH: usize = 128;
const BIG_INTEGER: i64 = 9_007_199_254_740_993;
const PATH_VALUE: &str = r"C:\data\imports\records.json";
const NOTES: &str = "First line with a colon: value and # hash.\nSecond line with \"quotes\", a tab\tand <html>.";

struct Record {
    id: i64,
    name: String,
    enabled: bool,
    region: usize,
    owner: String,
    url: String,
    fourth_sample: f64,
}

impl Record {
    fn new(index: usize) -> Self {
        Self {
            id: BIG_INTEGER + index as i64,
            name: format!("service-{index:04}"),
            enabled: !index.is_multiple_of(3),
            region: index % 12,
            owner: format!("team-{:02}", index % 40),
            url: format!("https://example.com/api/items/{index}?active=true&limit=100"),
            fourth_sample: index as f64 / 8.0,
        }
    }
}

fn main() {
    if !Path::new("Cargo.toml").exists() {
        panic!("run this generator from the repository root");
    }
    let directory = Path::new("test").join("stress");
    fs::create_dir_all(&directory).expect("create test/stress");

    let records: Vec<Record> = (0..RECORDS).map(Record::new).collect();

    let json = format!("{{\"name\":\"large-record-set\",\"records\":{}}}", records_json(&records));
    write(&directory, "large-valid.json", &json);
    write(&directory, "large-invalid.json", &json[..json.len() - 1]);

    let deep = deep_json();
    write(&directory, "deep-valid.json", &deep);

    let mut yaml = String::from("name: large-record-set\nrecords:\n");
    for record in &records {
        record_yaml_entry(&mut yaml, record, 4, 4);
    }
    write(&directory, "large-valid.yaml", &yaml);
    write(&directory, "large-invalid.yaml", &format!("{yaml}broken: [unterminated\n"));

    let mut stream = String::new();
    for (index, record) in records.iter().take(STREAM_DOCUMENTS).enumerate() {
        if index > 0 {
            stream.push_str("---\n");
        }
        record_yaml_body(&mut stream, record, 0, 0, 2, true);
    }
    write(&directory, "stream-valid.yaml", &stream);
}

fn write(directory: &Path, name: &str, content: &str) {
    fs::write(directory.join(name), content).expect("write fixture");
    println!("{name:<20} {:>10} bytes", content.len());
}

fn records_json(records: &[Record]) -> String {
    let mut out = String::from("[");
    for (index, record) in records.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        let _ = write!(
            out,
            "{{\"id\":{},\"name\":\"{}\",\"enabled\":{},\"tags\":[\"production\",\"stress-test\",\"region-{}\"],\"metadata\":{{\"owner\":\"{}\",\"path\":\"{}\",\"url\":\"{}\"}},\"samples\":[0,1.25,-3.5,{},1000000.125],\"notes\":\"{}\",\"optional\":null}}",
            record.id,
            record.name,
            record.enabled,
            record.region,
            record.owner,
            json_escape(PATH_VALUE),
            json_escape(&record.url),
            record.fourth_sample,
            json_escape(NOTES),
        );
    }
    out.push(']');
    out
}
/// Escapes a string the way Go's `encoding/json` does: short escapes for the
/// control characters that have one, and `\u00xx` for `<`, `>` and `&`, so the
/// fixtures also cover HTML-escape preservation.
fn json_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '<' => out.push_str("\\u003c"),
            '>' => out.push_str("\\u003e"),
            '&' => out.push_str("\\u0026"),
            control if (control as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", control as u32);
            }
            other => out.push(other),
        }
    }
    out
}

fn deep_json() -> String {
    let mut value = format!("{{\"leaf\":\"deepest\",\"number\":{BIG_INTEGER}}}");
    for level in 0..NESTING_DEPTH {
        value = format!("{{\"children\":[{value}],\"level\":{level},\"padding\":\"nested JSON stress fixture\"}}");
    }
    value
}

/// Writes one record as a mapping. Nested content sits `step` columns past the
/// mapping's own block indentation, which is the shape both `libyaml` and this
/// formatter use. `include_id` is false when the caller has already written the id
/// on a sequence entry's dash line.
fn record_yaml_body(
    out: &mut String,
    record: &Record,
    key_indent: usize,
    block: usize,
    step: usize,
    include_id: bool,
) {
    let keys = " ".repeat(key_indent);
    let nested = " ".repeat(block + step);
    if include_id {
        let _ = writeln!(out, "{keys}id: {}", record.id);
    }
    let _ = write!(
        out,
        "{keys}name: {}\n{keys}enabled: {}\n{keys}tags:\n{nested}- production\n{nested}- stress-test\n{nested}- region-{}\n{keys}metadata:\n{nested}owner: {}\n{nested}path: {}\n{nested}url: {}\n{keys}samples:\n{nested}- 0\n{nested}- 1.25\n{nested}- -3.5\n{nested}- {}\n{nested}- 1000000.125\n",
        record.name,
        record.enabled,
        record.region,
        record.owner,
        PATH_VALUE,
        record.url,
        record.fourth_sample,
    );
    let _ = writeln!(out, "{keys}notes: |");
    for line in NOTES.split('\n') {
        let _ = writeln!(out, "{nested}{line}");
    }
    let _ = writeln!(out, "{keys}optional: null");
}

/// Writes one record as a sequence entry: the id shares the `- ` line and the rest
/// of the mapping is aligned two columns past the dash.
fn record_yaml_entry(out: &mut String, record: &Record, item_indent: usize, step: usize) {
    let _ = writeln!(out, "{}- id: {}", " ".repeat(item_indent), record.id);
    record_yaml_body(out, record, item_indent + 2, item_indent, step, false);
}
