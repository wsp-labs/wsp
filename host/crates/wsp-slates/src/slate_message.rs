// SPDX-License-Identifier: AGPL-3.0-only
//! Unit helpers: slate-message.ts at 65f6b89c7. Spec: spec/helpers.md, Behaviour 5-7.
use serde_json::{Map, Value};
use wsp_words::js::{is_js_whitespace, utf16};

pub use crate::pending::slate::PathSeg;

pub fn defanged(value: &Value) -> Value {
    match value {
        Value::String(text) => Value::String(defang_text(text)),
        Value::Array(items) => Value::Array(items.iter().map(defanged).collect()),
        Value::Object(map) => Value::Object(map.iter().map(|(k, v)| (k.clone(), defanged(v))).collect()),
        other => other.clone(),
    }
}

fn is_line_end(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

/// The `^(\s*slate)\s*:(?=\s*\{)` match at `start`: where `slate` ends and the colon sits.
fn slate_colon(chars: &[char], start: usize) -> Option<(usize, usize)> {
    let skip = |mut i: usize| {
        while i < chars.len() && is_js_whitespace(chars[i]) {
            i += 1;
        }
        i
    };
    let word = skip(start);
    let slate_end = word + 5;
    if chars.get(word..slate_end) != Some(&['s', 'l', 'a', 't', 'e'][..]) {
        return None;
    }
    let colon = skip(slate_end);
    if chars.get(colon) != Some(&':') {
        return None;
    }
    let brace = skip(colon + 1);
    (chars.get(brace) == Some(&'{')).then_some((slate_end, colon))
}

fn defang_text(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let line_start = i == 0 || is_line_end(chars[i - 1]);
        let hit = if line_start { slate_colon(&chars, i) } else { None };
        match hit {
            Some((slate_end, colon)) => {
                out.extend(&chars[i..slate_end]);
                out.push('\u{ff1a}');
                i = colon + 1;
            }
            None => {
                out.push(chars[i]);
                i += 1;
            }
        }
    }
    out
}

/// The longest string inside a value: its path and its length in UTF-16 units.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Longest {
    pub at: Vec<PathSeg>,
    pub length: usize,
}

pub fn longest(value: &Value, at: &[PathSeg]) -> Option<Longest> {
    let child = |seg: PathSeg, v: &Value| {
        let mut path = at.to_vec();
        path.push(seg);
        longest(v, &path)
    };
    let kids: Vec<Option<Longest>> = match value {
        Value::String(text) => return Some(Longest { at: at.to_vec(), length: utf16::len(text) }),
        Value::Array(items) => items.iter().enumerate().map(|(i, v)| child(PathSeg::Index(i as i64), v)).collect(),
        Value::Object(map) => map.iter().map(|(k, v)| child(PathSeg::Key(k.clone()), v)).collect(),
        _ => return None,
    };
    kids.into_iter().flatten().fold(None, |best: Option<Longest>, k| match best {
        Some(b) if b.length >= k.length => Some(b),
        _ => Some(k),
    })
}

/// `keep` in UTF-16 units.
pub fn cut_at(value: &Value, at: &[PathSeg], keep: usize) -> Value {
    let Some((head, rest)) = at.split_first() else {
        return match value {
            Value::String(text) => Value::String(format!("{} (cut)", utf16::cut(text, keep))),
            other => other.clone(),
        };
    };
    match value {
        Value::Array(items) => match head {
            PathSeg::Index(i) => Value::Array(
                items.iter().enumerate().map(|(j, v)| if j as i64 == *i { cut_at(v, rest, keep) } else { v.clone() }).collect(),
            ),
            PathSeg::Key(_) => value.clone(),
        },
        Value::Object(map) => {
            let key = match head {
                PathSeg::Key(k) => k.clone(),
                PathSeg::Index(i) => i.to_string(),
            };
            let mut out: Map<String, Value> = map.clone();
            if let Some(child) = map.get(&key) {
                out.insert(key, cut_at(child, rest, keep));
            }
            Value::Object(out)
        }
        _ => value.clone(),
    }
}
