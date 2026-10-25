// SPDX-License-Identifier: AGPL-3.0-only
//! `JSON.parse` of a text, read the way node reads it: any nesting depth, a number past a double (`1e400`) is Infinity and
//! so `null` once written, and a lone surrogate escape (`"\ud800"`) is a string. serde_json refuses the first at depth 128 and
//! errors on the other two.

use serde_json::{Map, Number, Value};

/// Containers nested past this depth are read as `null`, after their syntax is checked.
const KEPT_DEPTH: usize = 100;

/// A Rust `String` cannot hold a lone surrogate, so one is kept as this character followed by `MARK_BASE` plus its offset
/// from U+D800, and a real occurrence of this character is kept doubled; `restore_surrogates` undoes both when a text is written.
const ESC: char = '\u{10FFFF}';
const MARK_BASE: u32 = 0xF0000;

/// What `JSON.parse(text)` reads, or None where it throws.
pub fn parse_json(text: &str) -> Option<Value> {
    let mut reader = Reader { b: text.as_bytes(), text, i: 0 };
    let value = reader.document()?;
    reader.white();
    (reader.i == reader.b.len()).then_some(value)
}

/// A JSON text written from strings `parse_json` read, with each lone surrogate back as the escape `JSON.stringify` writes.
pub fn restore_surrogates(text: &str) -> String {
    if !text.contains(ESC) {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, chars.peek().copied()) {
            (ESC, Some(ESC)) => {
                chars.next();
                out.push(ESC);
            }
            (ESC, Some(mark)) if (MARK_BASE..MARK_BASE + 0x800).contains(&(mark as u32)) => {
                chars.next();
                out.push_str(&format!("\\u{:04x}", 0xD800 + (mark as u32 - MARK_BASE)));
            }
            _ => out.push(c),
        }
    }
    out
}

enum Frame {
    Array(Vec<Value>),
    Object(Map<String, Value>, Option<String>),
}

struct Reader<'a> {
    b: &'a [u8],
    text: &'a str,
    i: usize,
}

impl Reader<'_> {
    fn white(&mut self) {
        while matches!(self.b.get(self.i), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.i += 1;
        }
    }

    fn take(&mut self, byte: u8) -> Option<()> {
        (self.b.get(self.i) == Some(&byte)).then(|| self.i += 1)
    }

    /// One value, read with a stack of its own so no depth is too deep; a container past `KEPT_DEPTH` is checked and read as null.
    fn document(&mut self) -> Option<Value> {
        let mut frames: Vec<Frame> = Vec::new();
        let mut kinds: Vec<u8> = Vec::new();
        loop {
            self.white();
            let mut done = match *self.b.get(self.i)? {
                open @ (b'[' | b'{') => {
                    self.i += 1;
                    let built = kinds.len() < KEPT_DEPTH;
                    kinds.push(open);
                    if built {
                        frames.push(if open == b'[' { Frame::Array(Vec::new()) } else { Frame::Object(Map::new(), None) });
                    }
                    self.white();
                    if self.take(if open == b'[' { b']' } else { b'}' }).is_some() {
                        kinds.pop();
                        Some(close(built, &mut frames))
                    } else {
                        if open == b'{' {
                            self.member_key(built.then(|| frames.last_mut()).flatten())?;
                        }
                        None
                    }
                }
                b'"' => Some(Value::String(self.string()?)),
                b'-' | b'0'..=b'9' => Some(self.number()?),
                b't' => Some(self.word("true", Value::Bool(true))?),
                b'f' => Some(self.word("false", Value::Bool(false))?),
                b'n' => Some(self.word("null", Value::Null)?),
                _ => return None,
            };
            while let Some(value) = done.take() {
                let Some(&kind) = kinds.last() else { return Some(value) };
                let built = kinds.len() <= KEPT_DEPTH;
                if built {
                    match frames.last_mut()? {
                        Frame::Array(items) => items.push(value),
                        Frame::Object(map, key) => {
                            map.insert(key.take()?, value);
                        }
                    }
                }
                self.white();
                match *self.b.get(self.i)? {
                    b',' => {
                        self.i += 1;
                        if kind == b'{' {
                            self.white();
                            self.member_key(built.then(|| frames.last_mut()).flatten())?;
                        }
                    }
                    c if c == if kind == b'[' { b']' } else { b'}' } => {
                        self.i += 1;
                        kinds.pop();
                        done = Some(close(built, &mut frames));
                    }
                    _ => return None,
                }
            }
        }
    }

    /// The `"key":` that opens an object member.
    fn member_key(&mut self, frame: Option<&mut Frame>) -> Option<()> {
        self.white();
        if self.b.get(self.i) != Some(&b'"') {
            return None;
        }
        let key = self.string()?;
        self.white();
        self.take(b':')?;
        if let Some(Frame::Object(_, slot)) = frame {
            *slot = Some(key);
        }
        Some(())
    }

    fn word(&mut self, word: &str, value: Value) -> Option<Value> {
        self.text[self.i..].starts_with(word).then(|| {
            self.i += word.len();
            value
        })
    }

    fn digits(&mut self) -> Option<()> {
        let from = self.i;
        while matches!(self.b.get(self.i), Some(b'0'..=b'9')) {
            self.i += 1;
        }
        (self.i > from).then_some(())
    }

    fn number(&mut self) -> Option<Value> {
        let start = self.i;
        self.take(b'-');
        match *self.b.get(self.i)? {
            b'0' => self.i += 1,
            b'1'..=b'9' => self.digits()?,
            _ => return None,
        }
        if self.take(b'.').is_some() {
            self.digits()?;
        }
        if matches!(self.b.get(self.i), Some(b'e' | b'E')) {
            self.i += 1;
            if matches!(self.b.get(self.i), Some(b'+' | b'-')) {
                self.i += 1;
            }
            self.digits()?;
        }
        let text = &self.text[start..self.i];
        if let Ok(n) = serde_json::from_str::<Number>(text) {
            return Some(Value::Number(n));
        }
        Some(text.parse::<f64>().ok().and_then(Number::from_f64).map_or(Value::Null, Value::Number))
    }

    fn hex4(&mut self) -> Option<u32> {
        let digits = self.text.get(self.i..self.i + 4)?;
        if !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        self.i += 4;
        u32::from_str_radix(digits, 16).ok()
    }

    /// The string whose opening quote is next.
    fn string(&mut self) -> Option<String> {
        self.i += 1;
        let mut out = String::new();
        let mut run = self.i;
        loop {
            match *self.b.get(self.i)? {
                b'"' => {
                    push_plain(&mut out, &self.text[run..self.i]);
                    self.i += 1;
                    return Some(out);
                }
                b'\\' => {
                    push_plain(&mut out, &self.text[run..self.i]);
                    self.i += 1;
                    let escape = *self.b.get(self.i)?;
                    self.i += 1;
                    match escape {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let unit = self.hex4()?;
                            self.unit(&mut out, unit)?;
                        }
                        _ => return None,
                    }
                    run = self.i;
                }
                0..=0x1f => return None,
                _ => self.i += 1,
            }
        }
    }

    /// A `\uXXXX` read: a surrogate pair is one character, a surrogate on its own is kept as `ESC` and its mark.
    fn unit(&mut self, out: &mut String, unit: u32) -> Option<()> {
        let mut low = None;
        if (0xD800..0xDC00).contains(&unit) && self.text[self.i..].starts_with("\\u") {
            let at = self.i;
            self.i += 2;
            match self.hex4() {
                Some(next) if (0xDC00..0xE000).contains(&next) => low = Some(next),
                _ => self.i = at,
            }
        }
        match (low, char::from_u32(unit)) {
            (Some(low), _) => out.push(char::from_u32(0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00))?),
            (None, Some(c)) => out.push(c),
            (None, None) => {
                out.push(ESC);
                out.push(char::from_u32(MARK_BASE + (unit - 0xD800))?);
            }
        }
        Some(())
    }
}

fn push_plain(out: &mut String, text: &str) {
    if text.contains(ESC) {
        out.push_str(&text.replace(ESC, "\u{10FFFF}\u{10FFFF}"));
    } else {
        out.push_str(text);
    }
}

/// The value a closed container is: what was built, or null for one past `KEPT_DEPTH`.
fn close(built: bool, frames: &mut Vec<Frame>) -> Value {
    if !built {
        return Value::Null;
    }
    match frames.pop() {
        Some(Frame::Array(items)) => Value::Array(items),
        Some(Frame::Object(map, _)) => Value::Object(map),
        None => Value::Null,
    }
}
