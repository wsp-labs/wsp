// SPDX-License-Identifier: AGPL-3.0-only
//! `JSON.parse` has no depth limit and reads `1e999` as Infinity; serde_json refuses both, and wsp-words' V8 reader
//! keeps a container past `KEPT_DEPTH` as an empty one. A text that reaches either limit is read again through the
//! catalog's `JSON.parse` port, whole down to 500 levels and with a number past the double range as null. A text the
//! port refuses for its depth is not read at all, never as a shallower tree.
use serde_json::Value;
use wsp_catalog::json_parse::parse_json as parse_whole;
use wsp_wire::types::js_number::JsNumber;
use wsp_words::v8_json::{parse_json, parse_json_lax, KEPT_DEPTH};

fn kept_to_the_limit(value: &Value, level: usize) -> bool {
    let mut work = vec![(value, level)];
    while let Some((value, level)) = work.pop() {
        if level >= KEPT_DEPTH {
            return true;
        }
        match value {
            Value::Array(items) => work.extend(items.iter().map(|item| (item, level + 1))),
            Value::Object(fields) => work.extend(fields.values().map(|field| (field, level + 1))),
            _ => {}
        }
    }
    false
}

/// The port's refusal of a tree nested past its cap.
const TOO_DEEP: &str = "Maximum call stack size exceeded";

fn whole(text: &str, kept: Value) -> Result<Value, String> {
    if !kept_to_the_limit(&kept, 0) {
        return Ok(kept);
    }
    match parse_whole(text) {
        Err(refused) if refused.message == TOO_DEEP => Err(refused.message),
        read => Ok(read.unwrap_or(kept)),
    }
}

/// A frame the host sent, read as `JSON.parse` reads it; none where it is not JSON or nests past the port's cap.
pub fn parse_frame(text: &str) -> Option<Value> {
    if let Ok(read) = serde_json::from_str(text) {
        return Some(read);
    }
    match parse_whole(text) {
        Ok(read) => Some(read),
        Err(refused) if refused.message == TOO_DEEP => None,
        Err(_) => parse_json_lax(text),
    }
}

/// `JSON.parse(text)` with V8's message, a container nested past 128 levels kept whole; past the port's cap it is the
/// port's refusal.
pub fn parse_document(text: &str) -> Result<Value, String> {
    parse_json(text).and_then(|kept| whole(text, kept))
}

/// `JSON.parse(text)` where only whether it parsed matters.
pub fn parse_value(text: &str) -> Option<Value> {
    parse_json_lax(text).and_then(|kept| whole(text, kept).ok())
}

/// `JSON.stringify(value)`: serde_json's text, with every number as JavaScript's `String(n)` writes it (`1e+21`, and a
/// whole number past 2^53 rounded to the double it is).
pub fn stringify(value: &Value) -> String {
    let mut text = String::new();
    write_value(value, &mut text);
    text
}

/// A container being written: what is left of its members, and whether none is written yet.
enum Open<'a> {
    Array(std::slice::Iter<'a, Value>, bool),
    Object(serde_json::map::Iter<'a>, bool),
}

/// The text of `value`, with the open containers on a stack so no depth of value overflows a thread's.
fn write_value(value: &Value, text: &mut String) {
    let mut stack: Vec<Open> = Vec::new();
    let mut next = Some(value);
    loop {
        match next.take() {
            Some(Value::Number(n)) => match n.as_f64() {
                Some(f) if n.is_f64() || f.abs() > 9_007_199_254_740_992.0 => text.push_str(&JsNumber(f).js_string()),
                _ => text.push_str(&n.to_string()),
            },
            Some(Value::Array(items)) => {
                text.push('[');
                stack.push(Open::Array(items.iter(), true));
            }
            Some(Value::Object(fields)) => {
                text.push('{');
                stack.push(Open::Object(fields.iter(), true));
            }
            Some(other) => text.push_str(&other.to_string()),
            None => {}
        }
        match stack.last_mut() {
            None => return,
            Some(Open::Array(items, first)) => match items.next() {
                Some(item) => {
                    if !std::mem::take(first) {
                        text.push(',');
                    }
                    next = Some(item);
                }
                None => {
                    stack.pop();
                    text.push(']');
                }
            },
            Some(Open::Object(fields, first)) => match fields.next() {
                Some((key, field)) => {
                    if !std::mem::take(first) {
                        text.push(',');
                    }
                    text.push_str(&Value::String(key.clone()).to_string());
                    text.push(':');
                    next = Some(field);
                }
                None => {
                    stack.pop();
                    text.push('}');
                }
            },
        }
    }
}
