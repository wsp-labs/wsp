// SPDX-License-Identifier: AGPL-3.0-only
//! protocol/src/slate/paths.ts at 65f6b89c7.
use serde_json::{Map, Value};
use wsp_words::js::{js_key_order, js_trim};

use super::*;

/// Names an object property read through a path never reaches (paths.ts UNSAFE).
const UNSAFE: &[&str] = &["__proto__", "constructor", "prototype"];

/// The Object.prototype members a value name can read as a function, so a path through one names nothing.
const PROTOTYPE_NAMES: &[&str] = &[
    "__defineGetter__",
    "__defineSetter__",
    "__lookupGetter__",
    "__lookupSetter__",
    "hasOwnProperty",
    "isPrototypeOf",
    "propertyIsEnumerable",
    "toLocaleString",
    "toString",
    "valueOf",
];

fn is_name_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

fn is_name_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// The ".field" and "[index]" steps of what follows a path's head, or None when the text is not steps.
pub fn slate_segments(rest: &str) -> Option<Vec<PathSeg>> {
    let bytes = rest.as_bytes();
    let mut segs = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'.' => {
                let start = at + 1;
                if start >= bytes.len() || !is_name_start(bytes[start]) {
                    return None;
                }
                let mut end = start + 1;
                while end < bytes.len() && is_name_char(bytes[end]) {
                    end += 1;
                }
                segs.push(PathSeg::Key(rest[start..end].to_string()));
                at = end;
            }
            b'[' => {
                let mut end = at + 1;
                let negative = bytes.get(end) == Some(&b'-');
                if negative {
                    end += 1;
                }
                let digits = end;
                while end < bytes.len() && bytes[end].is_ascii_digit() {
                    end += 1;
                }
                if end == digits || bytes.get(end) != Some(&b']') {
                    return None;
                }
                let magnitude = rest[digits..end].parse::<i64>().unwrap_or(i64::MAX);
                segs.push(PathSeg::Index(if negative { -magnitude } else { magnitude }));
                at = end + 1;
            }
            _ => return None,
        }
    }
    Some(segs)
}

/// The name and the segments after it, or None when the text is not an own path.
pub fn parse_slate_own_path(path: &str) -> Option<SlateOwnPath> {
    let bytes = path.as_bytes();
    if bytes.first() != Some(&b'$') || !bytes.get(1).is_some_and(|b| is_name_start(*b)) {
        return None;
    }
    let mut end = 2;
    while end < bytes.len() && is_name_char(bytes[end]) {
        end += 1;
    }
    let segs = slate_segments(&path[end..])?;
    Some(SlateOwnPath { name: path[1..end].to_string(), segs })
}

/// The name of a bare own path, "$name" with no fields, or None.
pub fn slate_own_name(text: &str) -> Option<String> {
    let path = parse_slate_own_path(js_trim(text))?;
    path.segs.is_empty().then_some(path.name)
}

fn step_ref<'a>(value: &'a Value, seg: &PathSeg) -> Option<&'a Value> {
    match (value, seg) {
        (Value::Array(items), PathSeg::Index(index)) => {
            let at = if *index < 0 { items.len() as i64 + index } else { *index };
            usize::try_from(at).ok().and_then(|at| items.get(at))
        }
        (Value::Object(record), PathSeg::Key(key)) if !UNSAFE.contains(&key.as_str()) => record.get(key),
        _ => None,
    }
}

/// One step into a value: a record's own field, or a list's item counting back from the end for a negative index.
pub fn slate_step(at: Option<Value>, seg: &PathSeg) -> Option<Value> {
    at.as_ref().and_then(|value| step_ref(value, seg)).cloned()
}

/// The value at an own path, or None when the name is not in the values or a step finds nothing.
pub fn get_slate_value(values: &SlateValues, path: &str) -> Option<Value> {
    let path = parse_slate_own_path(path)?;
    let head = values.get(&path.name)?;
    path.segs.iter().try_fold(head, |at, seg| step_ref(at, seg)).cloned()
}

/// How many lists and records a value, or a path, may nest; node's stack gives out at a depth that moves with its stack.
pub const SLATE_NEST_LIMIT: usize = 500;

/// How many lists and records deep a value goes: a scalar is 0.
pub fn slate_depth(value: &Value) -> usize {
    let mut deepest = 0;
    let mut stack = vec![(value, 0usize)];
    while let Some((v, at)) = stack.pop() {
        match v {
            Value::Array(items) => stack.extend(items.iter().map(|x| (x, at + 1))),
            Value::Object(fields) => stack.extend(fields.values().map(|x| (x, at + 1))),
            _ => {}
        }
        if v.is_array() || v.is_object() {
            deepest = deepest.max(at + 1);
        }
    }
    deepest
}

fn write_at(at: Option<&Value>, segs: &[PathSeg], value: &Value) -> Option<Value> {
    let Some((seg, rest)) = segs.split_first() else {
        return Some(value.clone());
    };
    match seg {
        PathSeg::Index(index) => {
            let mut list = match at {
                Some(Value::Array(items)) => items.clone(),
                None | Some(Value::Null) => Vec::new(),
                _ => return None,
            };
            let position = if *index < 0 { list.len() as i64 + index } else { *index };
            if position < 0 || position >= SLATE_LIMITS.list_items as i64 {
                return None;
            }
            let position = position as usize;
            let next = write_at(list.get(position), rest, value)?;
            list.resize(list.len().max(position), Value::Null);
            if position == list.len() {
                list.push(next);
            } else {
                list[position] = next;
            }
            Some(Value::Array(list))
        }
        PathSeg::Key(key) => {
            let mut record: Map<String, Value> = match at {
                Some(Value::Object(fields)) => js_key_order(fields.iter()).into_iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
                None | Some(Value::Null) => Map::new(),
                _ => return None,
            };
            let next = write_at(record.get(key), rest, value)?;
            record.insert(key.clone(), next);
            Some(Value::Object(record))
        }
    }
}

/// A copy of values with value at path, or None when the path is not an own path or steps through a scalar.
/// Missing records on the way are made; a list index past the end pads with null, up to the list cap.
pub fn set_slate_value(values: &SlateValues, path: &str, value: Value) -> Option<SlateValues> {
    let path = parse_slate_own_path(path)?;
    if UNSAFE.contains(&path.name.as_str()) || path.segs.iter().any(|s| matches!(s, PathSeg::Key(k) if UNSAFE.contains(&k.as_str()))) {
        return None;
    }
    if path.segs.len() + slate_depth(&value) > SLATE_NEST_LIMIT {
        return None;
    }
    let head = match values.get(&path.name) {
        Some(head) => Some(head),
        None if !path.segs.is_empty() && PROTOTYPE_NAMES.contains(&path.name.as_str()) => return None,
        None => None,
    };
    let next = write_at(head, &path.segs, &value)?;
    let mut out = values.clone();
    out.insert(path.name, next);
    Some(out)
}

fn value_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        (Value::String(x), Value::String(y)) => x == y,
        (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| value_equal(p, q)),
        (Value::Object(x), Value::Object(y)) => x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| value_equal(v, w))),
        _ => false,
    }
}

/// Deep equality over JSON, the one rule for "changed" (02, "Equality"). An absent value equals a null.
pub fn slate_equal(a: Option<&Value>, b: Option<&Value>) -> bool {
    match (a, b) {
        (None, None) => true,
        (None, Some(v)) | (Some(v), None) => v.is_null(),
        (Some(a), Some(b)) => value_equal(a, b),
    }
}
