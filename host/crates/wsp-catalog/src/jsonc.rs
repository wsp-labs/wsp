// SPDX-License-Identifier: AGPL-3.0-only
//! JSON with the comments and trailing commas Gemini CLI and OpenCode accept, read in one pass, and JSON written as
//! JavaScript writes it. Port of packages/catalog/src/jsonc.ts at a3b870a6e; the spec is spec/jsonc.md.
use serde_json::{Map, Number, Value};

use crate::json_parse::{lone_surrogate, parse_json};
use crate::plugins::js_entries;
use wsp_wire::types::js_number::JsNumber;
use wsp_words::exit::HostError;
use wsp_words::js::utf16;

/// A JSON object as the catalog reads one: keys in the order the text names them first.
pub type JsonObject = Map<String, Value>;

/// The value the text holds, comments and trailing commas taken out first; a real syntax error is an error as
/// `JSON.parse`'s throw is (jsonc.ts:12). A key named twice keeps its first place and its last value.
pub fn read_jsonc(text: &str) -> Result<Value, HostError> {
    parse_json(&strip_comments_and_commas(text))
}

/// The value alone (jsonc.ts:44).
pub fn parse_jsonc(text: &str) -> Result<Value, HostError> {
    read_jsonc(text)
}

/// The value as an object, or nothing for an array, a scalar or null (jsonc.ts:47).
pub fn json_object(value: &Value) -> Option<&JsonObject> {
    value.as_object()
}

/// `JSON.stringify(value)`: numbers as JavaScript writes them (rule 4.25: 1.0 is `1`, 1e21 is `1e+21`, -0 is `0`),
/// strings escaped as JavaScript escapes them, keys in the map's order.
pub fn js_stringify(value: &Value) -> String {
    let mut out = String::new();
    write_value(value, false, false, &mut out);
    out
}

/// `JSON.stringify(value, null, 2)`: the same, laid out with two-space indents; an empty object is `{}` and an empty
/// array `[]`.
pub fn js_stringify_pretty(value: &Value) -> String {
    let mut out = String::new();
    write_value(value, true, false, &mut out);
    out
}

pub(crate) use wsp_words::js::is_js_whitespace as is_js_space;

/// The index just past the string whose opening quote stands at `open`, or nothing where it never closes.
pub(crate) fn string_end(bytes: &[u8], open: usize) -> Option<usize> {
    let mut i = open + 1;
    while i < bytes.len() && bytes[i] != b'"' {
        if bytes[i] == b'\\' {
            i += 1;
        }
        i += 1;
    }
    (i < bytes.len()).then_some(i + 1)
}

/// A string as `JSON.stringify` writes it.
pub(crate) fn js_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => match lone_surrogate(c) {
                Some(unit) => out.push_str(&format!("\\u{unit:04x}")),
                None => out.push(c),
            },
        }
    }
    out.push('"');
    out
}

fn strip_comments_and_commas(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out: Vec<&str> = Vec::new();
    let mut from = 0;
    let mut i = 0;
    let mut comma: Option<usize> = None;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                comma = None;
                i = string_end(bytes, i).unwrap_or(bytes.len());
            }
            b'/' if matches!(bytes.get(i + 1), Some(b'/') | Some(b'*')) => {
                out.push(&text[from..i]);
                i = if bytes[i + 1] == b'/' {
                    text[i..].find('\n').map_or(bytes.len(), |k| i + k)
                } else {
                    text[i + 2..].find("*/").map_or(bytes.len(), |k| i + 2 + k + 2)
                };
                from = i;
            }
            b',' => {
                out.push(&text[from..i]);
                comma = Some(out.len());
                out.push(",");
                i += 1;
                from = i;
            }
            _ => {
                let ch = text.get(i..).and_then(|s| s.chars().next()).unwrap_or('\0');
                if (ch == '}' || ch == ']') && comma.is_some() {
                    if let Some(at) = comma {
                        out[at] = "";
                    }
                }
                if !is_js_space(ch) {
                    comma = None;
                }
                i += ch.len_utf8().max(1);
            }
        }
    }
    out.push(&text[from.min(text.len())..]);
    out.concat()
}

/// A container being written: what is left of its members, and whether none is written yet.
enum Open<'a> {
    Array(std::slice::Iter<'a, Value>, bool),
    Object(std::vec::IntoIter<(&'a String, &'a Value)>, bool),
}

/// `value` as `JSON.stringify` writes it, with a stack of the containers open instead of a call for each. With
/// `canonical` every object's keys stand in UTF-16 order instead of the order JavaScript keeps them in.
pub(crate) fn write_value(value: &Value, pretty: bool, canonical: bool, out: &mut String) {
    let mut stack: Vec<Open> = Vec::new();
    let mut next = Some(value);
    loop {
        if let Some(value) = next.take() {
            match value {
                Value::Null => out.push_str("null"),
                Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
                Value::Number(n) => out.push_str(&number_text(n)),
                Value::String(s) => out.push_str(&js_string(s)),
                Value::Array(items) if items.is_empty() => out.push_str("[]"),
                Value::Object(map) if map.is_empty() => out.push_str("{}"),
                Value::Array(items) => {
                    out.push('[');
                    stack.push(Open::Array(items.iter(), true));
                }
                Value::Object(map) => {
                    out.push('{');
                    let mut entries = js_entries(map);
                    if canonical {
                        entries.sort_by(|a, b| utf16::cmp(a.0, b.0));
                    }
                    stack.push(Open::Object(entries.into_iter(), true));
                }
            }
        }
        let depth = stack.len();
        let Some(top) = stack.last_mut() else {
            return;
        };
        match top {
            Open::Array(items, first) => match items.next() {
                Some(item) => {
                    if !std::mem::take(first) {
                        out.push(',');
                    }
                    newline(pretty, depth, out);
                    next = Some(item);
                }
                None => {
                    stack.pop();
                    newline(pretty, depth - 1, out);
                    out.push(']');
                }
            },
            Open::Object(members, first) => match members.next() {
                Some((key, item)) => {
                    if !std::mem::take(first) {
                        out.push(',');
                    }
                    newline(pretty, depth, out);
                    out.push_str(&js_string(key));
                    out.push_str(if pretty { ": " } else { ":" });
                    next = Some(item);
                }
                None => {
                    stack.pop();
                    newline(pretty, depth - 1, out);
                    out.push('}');
                }
            },
        }
    }
}

fn newline(pretty: bool, depth: usize, out: &mut String) {
    if pretty {
        out.push('\n');
        for _ in 0..depth {
            out.push_str("  ");
        }
    }
}

/// `Number.prototype.toString` of the number a JSON text read as a double.
fn number_text(n: &Number) -> String {
    JsNumber(n.as_f64().unwrap_or(f64::NAN)).js_string()
}
