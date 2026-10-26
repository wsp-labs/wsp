// SPDX-License-Identifier: AGPL-3.0-only
//! JSON read and dropped without recursion: node's `JSON.parse` takes a frame nested tens of thousands deep and the
//! schema then refuses it, where serde's own read and drop of a `Value` would overflow the stack first.
use serde_json::{Map, Value};

enum Open {
    Array(Vec<Value>),
    Object(Map<String, Value>, Option<String>),
}

/// `JSON.parse(text)`: the one value, or nothing where the text is not JSON.
pub fn parse(text: &str) -> Option<Value> {
    let bytes = text.as_bytes();
    let mut at = 0;
    let mut stack: Vec<Open> = Vec::new();
    let mut done: Option<Value> = None;
    loop {
        skip_space(bytes, &mut at);
        let byte = *bytes.get(at)?;
        let mut value = match byte {
            b'[' => {
                at += 1;
                skip_space(bytes, &mut at);
                if bytes.get(at) == Some(&b']') {
                    at += 1;
                    Value::Array(Vec::new())
                } else {
                    stack.push(Open::Array(Vec::new()));
                    continue;
                }
            }
            b'{' => {
                at += 1;
                skip_space(bytes, &mut at);
                if bytes.get(at) == Some(&b'}') {
                    at += 1;
                    Value::Object(Map::new())
                } else {
                    stack.push(Open::Object(Map::new(), None));
                    at = key(text, at, &mut stack)?;
                    continue;
                }
            }
            _ => {
                let end = leaf_end(bytes, at)?;
                let leaf = serde_json::from_str::<Value>(&text[at..end]).ok()?;
                at = end;
                leaf
            }
        };
        loop {
            match stack.last_mut() {
                None => {
                    done = Some(value);
                    break;
                }
                Some(Open::Array(items)) => {
                    items.push(value);
                    skip_space(bytes, &mut at);
                    match bytes.get(at)? {
                        b',' => {
                            at += 1;
                            break;
                        }
                        b']' => {
                            at += 1;
                            let Some(Open::Array(items)) = stack.pop() else { return None };
                            value = Value::Array(items);
                        }
                        _ => return None,
                    }
                }
                Some(Open::Object(map, name)) => {
                    map.insert(name.take()?, value);
                    skip_space(bytes, &mut at);
                    match bytes.get(at)? {
                        b',' => {
                            at += 1;
                            skip_space(bytes, &mut at);
                            at = key(text, at, &mut stack)?;
                            break;
                        }
                        b'}' => {
                            at += 1;
                            let Some(Open::Object(map, _)) = stack.pop() else { return None };
                            value = Value::Object(map);
                        }
                        _ => return None,
                    }
                }
            }
        }
        if let Some(value) = done.take() {
            skip_space(bytes, &mut at);
            return (at == bytes.len()).then_some(value);
        }
    }
}

/// Reads `"name" :` at `at` into the object on top of the stack; answers where the value begins.
fn key(text: &str, at: usize, stack: &mut [Open]) -> Option<usize> {
    let bytes = text.as_bytes();
    if bytes.get(at) != Some(&b'"') {
        return None;
    }
    let end = string_end(bytes, at)?;
    let Value::String(name) = serde_json::from_str::<Value>(&text[at..end]).ok()? else { return None };
    let mut at = end;
    skip_space(bytes, &mut at);
    if bytes.get(at) != Some(&b':') {
        return None;
    }
    if let Some(Open::Object(_, slot)) = stack.last_mut() {
        *slot = Some(name);
    }
    Some(at + 1)
}

fn skip_space(bytes: &[u8], at: &mut usize) {
    while matches!(bytes.get(*at), Some(b' ' | b'\t' | b'\n' | b'\r')) {
        *at += 1;
    }
}

/// One past the closing quote of the string that opens at `at`.
fn string_end(bytes: &[u8], at: usize) -> Option<usize> {
    let mut i = at + 1;
    loop {
        match bytes.get(i)? {
            b'"' => return Some(i + 1),
            b'\\' => i += 2,
            _ => i += 1,
        }
    }
}

/// One past a string, number, `true`, `false` or `null` that begins at `at`.
fn leaf_end(bytes: &[u8], at: usize) -> Option<usize> {
    if bytes[at] == b'"' {
        return string_end(bytes, at);
    }
    let mut i = at;
    while i < bytes.len() && !matches!(bytes[i], b',' | b']' | b'}' | b' ' | b'\t' | b'\n' | b'\r') {
        i += 1;
    }
    (i > at).then_some(i)
}

/// Drops a value without recursing into it.
pub fn drop_flat(value: Value) {
    let mut pending = vec![value];
    while let Some(value) = pending.pop() {
        match value {
            Value::Array(items) => pending.extend(items),
            Value::Object(map) => pending.extend(map.into_iter().map(|(_, value)| value)),
            _ => {}
        }
    }
}

/// A frame that drops flat.
pub struct Flat(pub Option<Value>);

impl Flat {
    pub fn new(value: Value) -> Flat {
        Flat(Some(value))
    }
    pub fn take(mut self) -> Value {
        self.0.take().unwrap_or(Value::Null)
    }
}

impl std::ops::Deref for Flat {
    type Target = Value;
    fn deref(&self) -> &Value {
        self.0.as_ref().unwrap_or(&Value::Null)
    }
}

impl Drop for Flat {
    fn drop(&mut self) {
        if let Some(value) = self.0.take() {
            drop_flat(value);
        }
    }
}
