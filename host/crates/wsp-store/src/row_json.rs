// SPDX-License-Identifier: AGPL-3.0-only
//! A stored row read back whole to the host's cap. serde_json refuses a text past 128 levels, but the store writes a row of
//! any depth (`put` is a loop), and a slate's record holds a value 500 levels deep inside a record that wraps it.

use serde_json::{Map, Value};
use wsp_words::exit::HostError;

/// The levels a stored row may nest: the host's 500 for a tree read from outside (ruling 1 of the round 2 brief) and the 6 a
/// slate's record adds around a value (record, kept, snapshot, document, values and the declaration holding a start value).
/// A row this deep clones and drops on a 1 MiB debug stack, where a `Value` of objects dies in clone at 628.
pub const ROW_DEPTH_MAX: usize = 506;

/// A stored row parsed afresh; a row that is not JSON fails the call (the TS throws `SyntaxError`), one nested past
/// `ROW_DEPTH_MAX` too. A row serde_json reads is the value it reads; only one past its limit takes the loop below.
pub fn parse_row(json: &str) -> Result<Value, HostError> {
    match serde_json::from_str(json) {
        Err(e) if e.to_string().starts_with("recursion limit exceeded") => deep_row(json),
        read => read.map_err(|e| HostError::new(e.to_string())),
    }
}

/// A container being read: its members so far, and for an object the key its next member is under.
enum Open {
    Array(Vec<Value>),
    Object(Map<String, Value>, String),
}

/// The row read with its open containers on a stack of its own, each scalar read by serde_json from its own token, so
/// no depth of row is a depth of the thread's stack.
fn deep_row(json: &str) -> Result<Value, HostError> {
    let bad = |at: usize| HostError::new(format!("a stored row is not JSON at column {}", at + 1));
    let b = json.as_bytes();
    let white = |at: &mut usize| {
        while matches!(b.get(*at), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            *at += 1;
        }
    };
    let key = |at: &mut usize| -> Result<String, HostError> {
        white(at);
        let end = string_end(b, *at).ok_or_else(|| bad(*at))?;
        let key = serde_json::from_str::<String>(&json[*at..end]).map_err(|_| bad(*at))?;
        *at = end;
        white(at);
        if b.get(*at) != Some(&b':') {
            return Err(bad(*at));
        }
        *at += 1;
        Ok(key)
    };
    let mut stack: Vec<Open> = Vec::new();
    let mut at = 0;
    loop {
        white(&mut at);
        let mut done = match b.get(at) {
            Some(b'[') | Some(b'{') => {
                if stack.len() >= ROW_DEPTH_MAX {
                    return Err(HostError::new(format!("a stored row nests deeper than {ROW_DEPTH_MAX} levels")));
                }
                let array = b[at] == b'[';
                at += 1;
                white(&mut at);
                let close = if array { b']' } else { b'}' };
                if b.get(at) == Some(&close) {
                    at += 1;
                    Some(if array { Value::Array(Vec::new()) } else { Value::Object(Map::new()) })
                } else {
                    stack.push(if array { Open::Array(Vec::new()) } else { Open::Object(Map::new(), key(&mut at)?) });
                    None
                }
            }
            Some(b'"') => {
                let end = string_end(b, at).ok_or_else(|| bad(at))?;
                let value = serde_json::from_str(&json[at..end]).map_err(|_| bad(at))?;
                at = end;
                Some(value)
            }
            Some(_) => {
                let end = at + b[at..].iter().take_while(|c| !matches!(c, b',' | b']' | b'}' | b' ' | b'\t' | b'\n' | b'\r')).count();
                let value = serde_json::from_str(&json[at..end]).map_err(|_| bad(at))?;
                at = end;
                Some(value)
            }
            None => return Err(bad(at)),
        };
        while let Some(value) = done.take() {
            white(&mut at);
            match stack.last_mut() {
                None if at == b.len() => return Ok(value),
                None => return Err(bad(at)),
                Some(Open::Array(items)) => {
                    items.push(value);
                    match b.get(at) {
                        Some(b',') => at += 1,
                        Some(b']') => {
                            at += 1;
                            if let Some(Open::Array(items)) = stack.pop() {
                                done = Some(Value::Array(items));
                            }
                        }
                        _ => return Err(bad(at)),
                    }
                }
                Some(Open::Object(fields, name)) => {
                    fields.insert(std::mem::take(name), value);
                    match b.get(at) {
                        Some(b',') => {
                            at += 1;
                            *name = key(&mut at)?;
                        }
                        Some(b'}') => {
                            at += 1;
                            if let Some(Open::Object(fields, _)) = stack.pop() {
                                done = Some(Value::Object(fields));
                            }
                        }
                        _ => return Err(bad(at)),
                    }
                }
            }
        }
    }
}

/// Where the string that starts at `at` ends, one past its closing quote.
fn string_end(b: &[u8], at: usize) -> Option<usize> {
    if b.get(at) != Some(&b'"') {
        return None;
    }
    let mut i = at + 1;
    while let Some(c) = b.get(i) {
        match c {
            b'"' => return Some(i + 1),
            b'\\' => i += 2,
            _ => i += 1,
        }
    }
    None
}
