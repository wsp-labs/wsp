// SPDX-License-Identifier: AGPL-3.0-only
//! `JSON.parse` of a text nested deeper than serde_json reads: node has no depth limit and serde_json stops at 128.

use serde_json::Value;

/// Containers nested past this depth are read as `null`, after their syntax is checked.
const KEPT_DEPTH: usize = 100;

pub fn parse_json(text: &str) -> Option<Value> {
    match serde_json::from_str::<Value>(text) {
        Ok(value) => Some(value),
        Err(e) if e.to_string().starts_with("recursion limit exceeded") => serde_json::from_str(&without_depth(text)?).ok(),
        Err(_) => None,
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Want {
    Value,
    ValueOrClose,
    KeyOrClose,
    Key,
    Colon,
    CommaOrClose,
    End,
}

/// The text with every container below `KEPT_DEPTH` replaced by `null`, or None where the text is not JSON. The syntax
/// is walked with a stack of its own, so no depth is too deep for it.
fn without_depth(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = String::new();
    let mut stack: Vec<u8> = Vec::new();
    let mut want = Want::Value;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if matches!(c, b' ' | b'\t' | b'\n' | b'\r') {
            i += 1;
            continue;
        }
        let after_value = if stack.is_empty() { Want::End } else { Want::CommaOrClose };
        let start = i;
        let mut emit = stack.len() <= KEPT_DEPTH;
        match (want, c) {
            (Want::Value | Want::ValueOrClose, b'[' | b'{') => {
                emit = false;
                if stack.len() < KEPT_DEPTH {
                    out.push(c as char);
                } else if stack.len() == KEPT_DEPTH {
                    out.push_str("null");
                }
                want = if c == b'[' { Want::ValueOrClose } else { Want::KeyOrClose };
                stack.push(c);
            }
            (Want::ValueOrClose, b']') | (Want::KeyOrClose, b'}') | (Want::CommaOrClose, b']' | b'}') => {
                let open = if c == b']' { b'[' } else { b'{' };
                if stack.pop() != Some(open) {
                    return None;
                }
                emit = stack.len() < KEPT_DEPTH;
                want = if stack.is_empty() { Want::End } else { Want::CommaOrClose };
            }
            (Want::Value | Want::ValueOrClose, b'"') => {
                i = string_end(bytes, i)?;
                want = after_value;
            }
            (Want::KeyOrClose | Want::Key, b'"') => {
                i = string_end(bytes, i)?;
                want = Want::Colon;
            }
            (Want::Value | Want::ValueOrClose, b'-' | b'0'..=b'9') => {
                while i < bytes.len() && matches!(bytes[i], b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E') {
                    i += 1;
                }
                serde_json::from_str::<serde_json::Number>(&text[start..i]).ok()?;
                i -= 1;
                want = after_value;
            }
            (Want::Value | Want::ValueOrClose, b't' | b'f' | b'n') => {
                let word = ["true", "false", "null"].into_iter().find(|w| text[i..].starts_with(w))?;
                i += word.len() - 1;
                want = after_value;
            }
            (Want::Colon, b':') => want = Want::Value,
            (Want::CommaOrClose, b',') => want = if stack.last() == Some(&b'[') { Want::Value } else { Want::Key },
            _ => return None,
        }
        if emit {
            out.push_str(&text[start..=i]);
        }
        i += 1;
    }
    (want == Want::End).then_some(out)
}

/// The index of the quote that ends the string opened at `open`, or None where it is not a JSON string.
fn string_end(bytes: &[u8], open: usize) -> Option<usize> {
    let mut j = open + 1;
    loop {
        match *bytes.get(j)? {
            b'"' => return Some(j),
            b'\\' => match *bytes.get(j + 1)? {
                b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => j += 2,
                b'u' if bytes.get(j + 2..j + 6)?.iter().all(u8::is_ascii_hexdigit) => j += 6,
                _ => return None,
            },
            0..=0x1f => return None,
            _ => j += 1,
        }
    }
}
