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

/// A list or a record being copied: what is left of the original, and the copy so far.
enum Copy<'a> {
    List(std::slice::Iter<'a, Value>, Vec<Value>),
    Record(serde_json::map::Iter<'a>, Map<String, Value>, Option<&'a String>),
}

impl<'a> Copy<'a> {
    fn of(value: &'a Value) -> Option<Copy<'a>> {
        match value {
            Value::Array(items) => Some(Copy::List(items.iter(), Vec::with_capacity(items.len()))),
            Value::Object(fields) => Some(Copy::Record(fields.iter(), Map::new(), None)),
            _ => None,
        }
    }

    fn finish(self) -> Value {
        match self {
            Copy::List(_, items) => Value::Array(items),
            Copy::Record(_, fields, _) => Value::Object(fields),
        }
    }

    fn put(&mut self, value: Value) {
        match self {
            Copy::List(_, items) => items.push(value),
            Copy::Record(_, fields, key) => {
                if let Some(key) = key.take() {
                    fields.insert(key.clone(), value);
                }
            }
        }
    }
}

/// `value.clone()` with the open lists and records on a stack of its own. A slate value may nest 500 levels, where the
/// derived `clone` is a call per level and dies on a 1 MiB stack at 628 with nothing else on it.
pub fn clone_value(value: &Value) -> Value {
    let Some(root) = Copy::of(value) else { return value.clone() };
    let mut open = vec![root];
    loop {
        let Some(top) = open.last_mut() else { unreachable!("the root is open until it is returned") };
        let next = match top {
            Copy::List(rest, _) => rest.next(),
            Copy::Record(rest, _, key) => rest.next().map(|(name, member)| {
                *key = Some(name);
                member
            }),
        };
        match next {
            Some(member) => match Copy::of(member) {
                Some(inner) => open.push(inner),
                None => top.put(member.clone()),
            },
            None => {
                let Some(done) = open.pop() else { unreachable!("a value is open") };
                match open.last_mut() {
                    Some(parent) => parent.put(done.finish()),
                    None => return done.finish(),
                }
            }
        }
    }
}

/// The levels a tree may hold before `from_deep_value` reads it on a stack of its own.
const SHALLOW: usize = 100;

/// The stack a deep tree is read on; only the pages it touches are ever mapped.
const DEEP_STACK: usize = 64 * 1024 * 1024;

/// `serde_json::from_value`, which is a call per level of every value in the type it reads and dies on a 1 MiB stack at about
/// 326 levels. A tree past 100 levels is read on a thread with a stack that holds the 500 a slate may nest; the error is
/// serde_json's text.
pub fn from_deep_value<T: serde::de::DeserializeOwned + Send>(value: Value) -> Result<T, String> {
    if slate_depth(&value) <= SHALLOW {
        return serde_json::from_value(value).map_err(|e| e.to_string());
    }
    std::thread::scope(|scope| {
        let reading = std::thread::Builder::new().stack_size(DEEP_STACK).spawn_scoped(scope, move || serde_json::from_value(value));
        match reading.map_err(|e| e.to_string())?.join() {
            Ok(read) => read.map_err(|e| e.to_string()),
            Err(_) => Err("a deep tree could not be read".to_string()),
        }
    })
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
    at.as_ref().and_then(|value| step_ref(value, seg)).map(clone_value)
}

/// The value at an own path, or None when the name is not in the values or a step finds nothing.
pub fn get_slate_value(values: &SlateValues, path: &str) -> Option<Value> {
    let path = parse_slate_own_path(path)?;
    let head = values.get(&path.name)?;
    path.segs.iter().try_fold(head, |at, seg| step_ref(at, seg)).map(clone_value)
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
