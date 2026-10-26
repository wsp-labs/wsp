// SPDX-License-Identifier: AGPL-3.0-only
//! `JSON.parse` has no depth limit and reads `1e999` as Infinity; serde_json refuses both, and wsp-words' V8 reader
//! keeps a container past `KEPT_DEPTH` as an empty one. A text that reaches either limit is read again through the
//! catalog's `JSON.parse` port, whole down to 1,000 levels and with a number past the double range as null.
use serde_json::Value;
use wsp_catalog::json_parse::parse_json as parse_whole;
use wsp_wire::types::js_number::JsNumber;
use wsp_words::v8_json::{parse_json, parse_json_lax, KEPT_DEPTH};

fn kept_to_the_limit(value: &Value, level: usize) -> bool {
    level >= KEPT_DEPTH
        || match value {
            Value::Array(items) => items.iter().any(|item| kept_to_the_limit(item, level + 1)),
            Value::Object(fields) => fields.values().any(|field| kept_to_the_limit(field, level + 1)),
            _ => false,
        }
}

fn whole(text: &str, kept: Value) -> Value {
    if kept_to_the_limit(&kept, 0) {
        parse_whole(text).unwrap_or(kept)
    } else {
        kept
    }
}

/// A frame the host sent, read as `JSON.parse` reads it; none where it is not JSON.
pub fn parse_frame(text: &str) -> Option<Value> {
    serde_json::from_str(text).ok().or_else(|| parse_whole(text).ok()).or_else(|| parse_json_lax(text))
}

/// `JSON.parse(text)` with V8's message, a container nested past 128 levels kept whole.
pub fn parse_document(text: &str) -> Result<Value, String> {
    parse_json(text).map(|kept| whole(text, kept))
}

/// `JSON.parse(text)` where only whether it parsed matters.
pub fn parse_value(text: &str) -> Option<Value> {
    parse_json_lax(text).map(|kept| whole(text, kept))
}

/// `JSON.stringify(value)`: serde_json's text, with every number as JavaScript's `String(n)` writes it (`1e+21`, and a
/// whole number past 2^53 rounded to the double it is).
pub fn stringify(value: &Value) -> String {
    let mut text = String::new();
    write_value(value, &mut text);
    text
}

fn write_value(value: &Value, text: &mut String) {
    match value {
        Value::Number(n) => match n.as_f64() {
            Some(f) if n.is_f64() || f.abs() > 9_007_199_254_740_992.0 => text.push_str(&JsNumber(f).js_string()),
            _ => text.push_str(&n.to_string()),
        },
        Value::Array(items) => {
            text.push('[');
            for (at, item) in items.iter().enumerate() {
                if at > 0 {
                    text.push(',');
                }
                write_value(item, text);
            }
            text.push(']');
        }
        Value::Object(fields) => {
            text.push('{');
            for (at, (key, field)) in fields.iter().enumerate() {
                if at > 0 {
                    text.push(',');
                }
                text.push_str(&Value::String(key.clone()).to_string());
                text.push(':');
                write_value(field, text);
            }
            text.push('}');
        }
        other => text.push_str(&other.to_string()),
    }
}
