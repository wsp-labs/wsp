// SPDX-License-Identifier: AGPL-3.0-only
//! JSON with the comments and trailing commas Gemini CLI and OpenCode accept, read in one pass, and JSON written as
//! JavaScript writes it. Port of packages/catalog/src/jsonc.ts at a3b870a6e; the spec is spec/jsonc.md.
use serde_json::{Map, Number, Value};

use wsp_words::exit::HostError;

/// A JSON object as the catalog reads one: keys in the order the text names them first.
pub type JsonObject = Map<String, Value>;

/// The value the text holds, comments and trailing commas taken out first; a real syntax error is an error as
/// `JSON.parse`'s throw is (jsonc.ts:12). A key named twice keeps its first place and its last value.
pub fn read_jsonc(text: &str) -> Result<Value, HostError> {
    serde_json::from_str(&strip_comments_and_commas(text)).map_err(|e| HostError::new(e.to_string()))
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
    write_value(value, false, 0, &mut out);
    out
}

/// `JSON.stringify(value, null, 2)`: the same, laid out with two-space indents; an empty object is `{}` and an empty
/// array `[]`.
pub fn js_stringify_pretty(value: &Value) -> String {
    let mut out = String::new();
    write_value(value, true, 0, &mut out);
    out
}

/// JavaScript's `/\s/`: spec rule 8.7.
pub(crate) fn is_js_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'
    )
}

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
            c => out.push(c),
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

fn write_value(value: &Value, pretty: bool, depth: usize, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(&number_text(n)),
        Value::String(s) => out.push_str(&js_string(s)),
        Value::Array(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                newline(pretty, depth + 1, out);
                write_value(item, pretty, depth + 1, out);
            }
            newline(pretty, depth, out);
            out.push(']');
        }
        Value::Object(map) => {
            if map.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push('{');
            for (i, (key, item)) in map.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                newline(pretty, depth + 1, out);
                out.push_str(&js_string(key));
                out.push_str(if pretty { ": " } else { ":" });
                write_value(item, pretty, depth + 1, out);
            }
            newline(pretty, depth, out);
            out.push('}');
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
    let x = n.as_f64().unwrap_or(f64::NAN);
    if x.is_nan() {
        return "null".to_string();
    }
    if x == 0.0 {
        return "0".to_string();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity".to_string() } else { "-Infinity".to_string() };
    }
    let sign = if x < 0.0 { "-" } else { "" };
    let sci = format!("{:e}", x.abs());
    let (mantissa, exponent) = sci.split_once('e').unwrap_or((sci.as_str(), "0"));
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let k = digits.len() as i64;
    let e: i64 = exponent.parse().unwrap_or(0);
    let point = e + 1;
    let body = if k <= point && point <= 21 {
        format!("{digits}{}", "0".repeat((point - k) as usize))
    } else if 0 < point && point <= 21 {
        format!("{}.{}", &digits[..point as usize], &digits[point as usize..])
    } else if -6 < point && point <= 0 {
        format!("0.{}{digits}", "0".repeat((-point) as usize))
    } else {
        let exp = point - 1;
        let fraction = if k == 1 { String::new() } else { format!(".{}", &digits[1..]) };
        format!("{}{fraction}e{}{}", &digits[..1], if exp < 0 { "-" } else { "+" }, exp.abs())
    };
    format!("{sign}{body}")
}
