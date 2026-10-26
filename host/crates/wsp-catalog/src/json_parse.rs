// SPDX-License-Identifier: AGPL-3.0-only
//! `JSON.parse`: the values it reads and the words it fails with, V8's. serde_json refuses what JavaScript reads (a
//! lone surrogate escape, a number past f64, nesting past 128), so every text the catalog reads as JSON comes
//! through here. The spec is spec/jsonc.md.
use serde_json::{Map, Number, Value};

use wsp_words::exit::HostError;

/// A lone surrogate escape has no `char`, so it is read as the char at this base plus its offset from U+D800 (in the
/// supplementary private use area B) and written back as its escape (`js_string`): one a file holds survives a read
/// and a write. A file that holds one of these chars itself reads as the same escape.
const LONE_BASE: u32 = 0x10F800;

/// Nesting past this refuses as V8's parser does when its stack is gone; a tree kept deeper would abort a clone or a drop.
const MAX_DEPTH: usize = 1000;

/// The lone surrogate unit a char stands for, if it stands for one.
pub(crate) fn lone_surrogate(c: char) -> Option<u16> {
    let n = c as u32;
    (LONE_BASE..LONE_BASE + 0x800).contains(&n).then(|| (0xD800 + (n - LONE_BASE)) as u16)
}

/// A JavaScript string's UTF-16 units as text: a lone surrogate is the char it is held as.
pub(crate) fn string_of_units(units: &[u16]) -> String {
    char::decode_utf16(units.iter().copied()).map(|c| c.unwrap_or_else(|e| lone_char(e.unpaired_surrogate()))).collect()
}

fn lone_char(unit: u16) -> char {
    char::from_u32(LONE_BASE + (unit as u32 - 0xD800)).expect("a char in the private use area")
}

/// The value the text holds as `JSON.parse` reads it. A number past f64 is `null`, as `JSON.stringify` writes
/// infinity.
pub fn parse_json(text: &str) -> Result<Value, HostError> {
    let mut p = Parser { text, bytes: text.as_bytes(), pos: 0 };
    let value = p.value().map_err(HostError::new)?;
    p.skip_space();
    if p.pos < p.bytes.len() {
        return Err(HostError::new(p.at(p.pos, "Unexpected non-whitespace character after JSON")));
    }
    Ok(value)
}

/// The string a JSON string literal (quotes included) holds, nothing where it is not one.
pub fn parse_json_string(literal: &str) -> Option<String> {
    let mut p = Parser { text: literal, bytes: literal.as_bytes(), pos: 0 };
    if p.bytes.first() != Some(&b'"') {
        return None;
    }
    let s = p.string().ok()?;
    (p.pos == p.bytes.len()).then_some(s)
}

struct Parser<'a> {
    text: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

enum Frame {
    Object(Map<String, Value>, String),
    Array(Vec<Value>),
}

#[derive(PartialEq)]
enum Token {
    Eos,
    Number,
    String,
    Other,
}

impl Parser<'_> {
    fn skip_space(&mut self) {
        while matches!(self.bytes.get(self.pos), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.skip_space();
        self.bytes.get(self.pos).copied()
    }

    /// `message in JSON at position P (line L column C)`, `at` a byte offset.
    fn at(&self, at: usize, message: &str) -> String {
        let before = &self.text[..at];
        let (mut line, mut line_start, mut units) = (1, 0, 0);
        let mut chars = before.chars().peekable();
        while let Some(c) = chars.next() {
            units += c.len_utf16();
            if c == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
                units += 1;
            }
            if c == '\r' || c == '\n' {
                line += 1;
                line_start = units;
            }
        }
        let column = 1 + units - line_start;
        let word = if message.ends_with("JSON") { "" } else { " in JSON" };
        format!("{message}{word} at position {units} (line {line} column {column})")
    }

    /// A token no rule of the grammar takes at `at`, in V8's words for what stands there.
    fn unexpected(&self, at: usize) -> String {
        match self.token(at) {
            Token::Eos => "Unexpected end of JSON input".to_string(),
            Token::Number => self.at(at, "Unexpected number"),
            Token::String => self.at(at, "Unexpected string"),
            Token::Other => {
                if ["[object Object]", "NaN", "Infinity", "undefined"].contains(&self.text) {
                    return format!("\"{}\" is not valid JSON", self.text);
                }
                let units: Vec<u16> = self.text.encode_utf16().collect();
                let at16 = self.text[..at].encode_utf16().count();
                let unit = String::from_utf16_lossy(&units[at16..at16 + 1]);
                let context = |from: usize, to: usize| String::from_utf16_lossy(&units[from..to]);
                let (head, shown, tail) = if units.len() < 21 {
                    ("", context(0, units.len()), "")
                } else if at16 < 10 {
                    ("", context(0, at16 + 10), "...")
                } else if at16 < units.len() - 10 {
                    ("...", context(at16 - 10, at16 + 10), "...")
                } else {
                    ("...", context(at16 - 10, units.len()), "")
                };
                format!("Unexpected token '{unit}', {head}\"{shown}\"{tail} is not valid JSON")
            }
        }
    }

    fn token(&self, at: usize) -> Token {
        match self.bytes.get(at) {
            None => Token::Eos,
            Some(b'-' | b'0'..=b'9') => Token::Number,
            Some(b'"') => Token::String,
            Some(_) => Token::Other,
        }
    }

    /// The value at `pos`, read with a stack of its own: a container is a frame, not a call, so the depth cap fires
    /// before any thread's stack could.
    fn value(&mut self) -> Result<Value, String> {
        let mut stack: Vec<Frame> = Vec::new();
        loop {
            let mut done = match self.peek() {
                Some(b'"') => self.string().map(Value::String)?,
                Some(b'-' | b'0'..=b'9') => self.number()?,
                Some(b'{') => {
                    self.enter(stack.len())?;
                    if self.peek() == Some(b'}') {
                        self.pos += 1;
                        Value::Object(Map::new())
                    } else {
                        let key = self.property_name(true)?;
                        stack.push(Frame::Object(Map::new(), key));
                        continue;
                    }
                }
                Some(b'[') => {
                    self.enter(stack.len())?;
                    if self.peek() == Some(b']') {
                        self.pos += 1;
                        Value::Array(Vec::new())
                    } else {
                        stack.push(Frame::Array(Vec::new()));
                        continue;
                    }
                }
                Some(b't') => self.literal("true", Value::Bool(true))?,
                Some(b'f') => self.literal("false", Value::Bool(false))?,
                Some(b'n') => self.literal("null", Value::Null)?,
                _ => return Err(self.unexpected(self.pos)),
            };
            loop {
                match stack.last_mut() {
                    None => return Ok(done),
                    Some(Frame::Object(map, key)) => {
                        map.insert(std::mem::take(key), done);
                        match self.peek() {
                            Some(b',') => {
                                self.pos += 1;
                                *key = self.property_name(false)?;
                                break;
                            }
                            Some(b'}') => self.pos += 1,
                            _ => return Err(self.at(self.pos, "Expected ',' or '}' after property value")),
                        }
                    }
                    Some(Frame::Array(items)) => {
                        items.push(done);
                        match self.peek() {
                            Some(b',') => {
                                self.pos += 1;
                                break;
                            }
                            Some(b']') => self.pos += 1,
                            _ => return Err(self.at(self.pos, "Expected ',' or ']' after array element")),
                        }
                    }
                }
                done = match stack.pop() {
                    Some(Frame::Object(map, _)) => Value::Object(map),
                    Some(Frame::Array(items)) => Value::Array(items),
                    None => unreachable!("a frame was just read"),
                };
            }
        }
    }

    /// The opening bracket at `pos` of a container `depth` deep; it refuses past the cap and `pos` ends past it.
    fn enter(&mut self, depth: usize) -> Result<(), String> {
        if depth >= MAX_DEPTH {
            return Err("Maximum call stack size exceeded".to_string());
        }
        self.pos += 1;
        Ok(())
    }

    /// An object's next key and its colon; `pos` ends at the value.
    fn property_name(&mut self, first: bool) -> Result<String, String> {
        if self.peek() != Some(b'"') {
            let what = if first { "Expected property name or '}'" } else { "Expected double-quoted property name" };
            return Err(self.at(self.pos, what));
        }
        let key = self.string()?;
        if self.peek() != Some(b':') {
            return Err(self.at(self.pos, "Expected ':' after property name"));
        }
        self.pos += 1;
        Ok(key)
    }

    fn literal(&mut self, word: &str, value: Value) -> Result<Value, String> {
        let rest = &self.bytes[self.pos..];
        if rest.starts_with(word.as_bytes()) {
            self.pos += word.len();
            return Ok(value);
        }
        let mut at = self.pos + 1;
        for expected in word.bytes().skip(1).take(rest.len().saturating_sub(1)) {
            if self.bytes[at] != expected {
                return Err(self.unexpected(at));
            }
            at += 1;
        }
        Err(self.unexpected(at))
    }

    fn digits(&mut self) -> usize {
        let from = self.pos;
        while matches!(self.bytes.get(self.pos), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
        self.pos - from
    }

    fn number(&mut self) -> Result<Value, String> {
        let start = self.pos;
        if self.bytes[self.pos] == b'-' {
            self.pos += 1;
        }
        if self.bytes.get(self.pos) == Some(&b'0') {
            self.pos += 1;
            if matches!(self.bytes.get(self.pos), Some(b'0'..=b'9')) {
                return Err(self.at(self.pos, "Unexpected number"));
            }
        } else if self.digits() == 0 {
            return Err(self.at(self.pos, "No number after minus sign"));
        }
        let mut whole = true;
        if self.bytes.get(self.pos) == Some(&b'.') {
            whole = false;
            self.pos += 1;
            if self.digits() == 0 {
                return Err(self.at(self.pos, "Unterminated fractional number"));
            }
        }
        if matches!(self.bytes.get(self.pos), Some(b'e' | b'E')) {
            whole = false;
            self.pos += 1;
            if matches!(self.bytes.get(self.pos), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            if self.digits() == 0 {
                return Err(self.at(self.pos, "Exponent part is missing a number"));
            }
        }
        let literal = &self.text[start..self.pos];
        if whole && literal != "-0" {
            if let Ok(n) = literal.parse::<u64>() {
                return Ok(Value::Number(n.into()));
            }
            if let Ok(n) = literal.parse::<i64>() {
                return Ok(Value::Number(n.into()));
            }
        }
        let n: f64 = literal.parse().expect("a JSON number reads as a double");
        Ok(Number::from_f64(n).map_or(Value::Null, Value::Number))
    }

    /// The string whose opening quote stands at `pos`; `pos` ends just past its closing quote.
    fn string(&mut self) -> Result<String, String> {
        self.pos += 1;
        let mut out = String::new();
        let mut high: Option<u16> = None;
        let mut from = self.pos;
        loop {
            let Some(i) = self.bytes[self.pos..].iter().position(|&b| b == b'"' || b == b'\\' || b < 0x20) else {
                return Err(self.at(self.bytes.len(), "Unterminated string"));
            };
            self.pos += i;
            if i > 0 {
                flush(&mut high, &mut out);
            }
            out.push_str(&self.text[from..self.pos]);
            match self.bytes[self.pos] {
                b'"' => {
                    flush(&mut high, &mut out);
                    self.pos += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.pos += 1;
                    let Some(&escape) = self.bytes.get(self.pos) else {
                        return Err(self.unexpected(self.pos));
                    };
                    if self.text[self.pos..].chars().next().is_some_and(|c| c as u32 > 0xFF) {
                        return Err(self.unexpected(self.pos));
                    }
                    let plain = match escape {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let unit = self.hex4()?;
                            match (unit, high) {
                                (0xDC00..=0xDFFF, Some(h)) => {
                                    high = None;
                                    let c = 0x10000 + (((h as u32) - 0xD800) << 10) + (unit as u32 - 0xDC00);
                                    out.push(char::from_u32(c).expect("a surrogate pair is a char"));
                                }
                                (0xDC00..=0xDFFF, None) => out.push(lone_char(unit)),
                                (0xD800..=0xDBFF, _) => {
                                    flush(&mut high, &mut out);
                                    high = Some(unit);
                                }
                                _ => {
                                    flush(&mut high, &mut out);
                                    out.push(char::from_u32(unit as u32).expect("a unit outside the surrogates is a char"));
                                }
                            }
                            from = self.pos;
                            continue;
                        }
                        _ => return Err(self.at(self.pos, "Bad escaped character")),
                    };
                    flush(&mut high, &mut out);
                    out.push(plain);
                    self.pos += 1;
                    from = self.pos;
                }
                _ => return Err(self.at(self.pos, "Bad control character in string literal")),
            }
        }
    }

    /// The four hex digits after the `u` at `pos`; `pos` ends past them.
    fn hex4(&mut self) -> Result<u16, String> {
        let mut unit = 0u16;
        for _ in 0..4 {
            self.pos += 1;
            let digit = self.bytes.get(self.pos).and_then(|&b| (b as char).to_digit(16));
            let Some(digit) = digit else {
                return Err(self.at(self.pos.min(self.bytes.len()), "Bad Unicode escape"));
            };
            unit = unit * 16 + digit as u16;
        }
        self.pos += 1;
        Ok(unit)
    }
}

fn flush(high: &mut Option<u16>, out: &mut String) {
    if let Some(unit) = high.take() {
        out.push(lone_char(unit));
    }
}
