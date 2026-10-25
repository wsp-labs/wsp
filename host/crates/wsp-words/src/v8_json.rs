// SPDX-License-Identifier: AGPL-3.0-only
//! V8's `JSON.parse`: a JSON text read by UTF-16 unit so its positions and its messages are V8's (node 22), and read
//! without recursing per bracket, so a reply nested a million deep is answered the way node answers it. See
//! spec/start.md.

use serde_json::{Map, Number, Value};

/// Past this many levels a container is read, checked and then dropped to an empty one of its kind: the shapes the host
/// checks hold nothing that deep, and a `Value` that deep would overflow the stack when it is dropped or read.
pub const KEPT_DEPTH: usize = 128;

/// The whole texts V8 names as not valid JSON in full, whatever token failed.
const JSON_SPECIAL_TEXTS: [&str; 4] = ["NaN", "Infinity", "undefined", "[object Object]"];

/// How a number past the double range reads. V8 reads `1e999` as Infinity, which a `Value` cannot hold.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Beyond {
    /// As null, which is what `JSON.stringify(Infinity)` writes.
    Null,
    /// As a fraction (plus or minus a half), which is what zod's `.int()` calls a float.
    Fraction,
}

/// `JSON.parse(text)`, with V8's message as the error. A number past the double range reads as a fraction (a half, with
/// its sign) where zod's checks read it as the float Infinity is; a container nested past `KEPT_DEPTH` is read whole and
/// kept empty.
pub fn parse_json(text: &str) -> Result<Value, String> {
    read(text, Beyond::Fraction)
}

/// `JSON.parse(text)` where only whether it parsed matters: a number past the double range reads as null, as
/// `JSON.stringify` writes Infinity.
pub fn parse_json_lax(text: &str) -> Option<Value> {
    read(text, Beyond::Null).ok()
}

fn read(text: &str, beyond: Beyond) -> Result<Value, String> {
    let mut reader = JsonReader { src: text.encode_utf16().collect(), at: 0, beyond };
    reader.skip_space();
    let value = reader.value()?;
    reader.skip_space();
    if reader.at < reader.src.len() {
        return Err(reader.located("Unexpected non-whitespace character after JSON", reader.at));
    }
    Ok(value)
}

struct JsonReader {
    src: Vec<u16>,
    at: usize,
    beyond: Beyond,
}

/// A container being read: what it holds so far, or nothing where it sits past `KEPT_DEPTH`.
enum Frame {
    Array(Option<Vec<Value>>),
    Object(Option<Map<String, Value>>, String),
}

impl JsonReader {
    fn unit(&self, at: usize) -> Option<u16> {
        self.src.get(at).copied()
    }

    fn is(&self, at: usize, c: char) -> bool {
        self.unit(at) == Some(c as u16)
    }

    fn is_digit(&self, at: usize) -> bool {
        matches!(self.unit(at), Some(0x30..=0x39))
    }

    fn text(&self, from: usize, to: usize) -> String {
        let end = to.min(self.src.len());
        String::from_utf16_lossy(&self.src[from.min(end)..end])
    }

    fn skip_space(&mut self) {
        while matches!(self.unit(self.at), Some(0x20 | 0x09 | 0x0A | 0x0D)) {
            self.at += 1;
        }
    }

    /// The prefix, then V8's position: the unit offset, and the line and column where `\n`, `\r\n` and a lone `\r`
    /// each end a line.
    fn located(&self, prefix: &str, at: usize) -> String {
        let end = at.min(self.src.len());
        let (mut line, mut start, mut i) = (1, 0, 0);
        while i < end {
            match self.src[i] {
                0x0A => {
                    line += 1;
                    start = i + 1;
                }
                0x0D => {
                    if self.src.get(i + 1) == Some(&0x0A) && i + 1 < end {
                        i += 1;
                    }
                    line += 1;
                    start = i + 1;
                }
                _ => {}
            }
            i += 1;
        }
        format!("{prefix} at position {at} (line {line} column {})", at - start + 1)
    }

    fn unexpected(&self, at: usize, expected: Option<&str>) -> String {
        if let Some(prefix) = expected {
            return self.located(prefix, at);
        }
        let Some(unit) = self.unit(at) else {
            return "Unexpected end of JSON input".to_string();
        };
        match unit {
            0x22 => return self.located("Unexpected string in JSON", at),
            0x2D | 0x30..=0x39 => return self.located("Unexpected number in JSON", at),
            _ => {}
        }
        let token = String::from_utf16_lossy(&[unit]);
        let source = self.text(0, self.src.len());
        let n = self.src.len();
        if JSON_SPECIAL_TEXTS.contains(&source.as_str()) {
            format!("\"{source}\" is not valid JSON")
        } else if n < 21 {
            format!("Unexpected token '{token}', \"{source}\" is not valid JSON")
        } else if at < 10 {
            format!("Unexpected token '{token}', \"{}\"... is not valid JSON", self.text(0, at + 10))
        } else if at < n - 10 {
            format!("Unexpected token '{token}', ...\"{}\"... is not valid JSON", self.text(at - 10, at + 10))
        } else {
            format!("Unexpected token '{token}', ...\"{}\" is not valid JSON", self.text(at - 10, n))
        }
    }

    /// One value, with every container in it read off an explicit stack.
    fn value(&mut self) -> Result<Value, String> {
        let mut stack: Vec<Frame> = Vec::new();
        loop {
            let mut done = match self.unit(self.at) {
                Some(0x7B) => {
                    self.at += 1;
                    self.skip_space();
                    if self.is(self.at, '}') {
                        self.at += 1;
                        Value::Object(Map::new())
                    } else {
                        let key = self.key("Expected property name or '}' in JSON")?;
                        let held = (stack.len() < KEPT_DEPTH).then(Map::new);
                        stack.push(Frame::Object(held, key));
                        continue;
                    }
                }
                Some(0x5B) => {
                    self.at += 1;
                    self.skip_space();
                    if self.is(self.at, ']') {
                        self.at += 1;
                        Value::Array(Vec::new())
                    } else {
                        let held = (stack.len() < KEPT_DEPTH).then(Vec::new);
                        stack.push(Frame::Array(held));
                        continue;
                    }
                }
                Some(0x22) => Value::String(self.string()?),
                Some(0x2D | 0x30..=0x39) => self.number()?,
                Some(0x74) => self.literal("true", Value::Bool(true))?,
                Some(0x66) => self.literal("false", Value::Bool(false))?,
                Some(0x6E) => self.literal("null", Value::Null)?,
                _ => return Err(self.unexpected(self.at, None)),
            };
            loop {
                match stack.last_mut() {
                    None => return Ok(done),
                    Some(Frame::Array(items)) => {
                        if let Some(items) = items {
                            items.push(done);
                        }
                        self.skip_space();
                        if self.is(self.at, ',') {
                            self.at += 1;
                            self.skip_space();
                            break;
                        } else if self.is(self.at, ']') {
                            self.at += 1;
                            done = match stack.pop() {
                                Some(Frame::Array(items)) => Value::Array(items.unwrap_or_default()),
                                _ => unreachable!("the frame on top was an array"),
                            };
                        } else {
                            return Err(self.unexpected(self.at, Some("Expected ',' or ']' after array element in JSON")));
                        }
                    }
                    Some(Frame::Object(map, key)) => {
                        if let Some(map) = map {
                            map.insert(std::mem::take(key), done);
                        }
                        self.skip_space();
                        if self.is(self.at, ',') {
                            self.at += 1;
                            self.skip_space();
                            *key = self.key("Expected double-quoted property name in JSON")?;
                            break;
                        } else if self.is(self.at, '}') {
                            self.at += 1;
                            done = match stack.pop() {
                                Some(Frame::Object(map, _)) => Value::Object(map.unwrap_or_default()),
                                _ => unreachable!("the frame on top was an object"),
                            };
                        } else {
                            return Err(self.unexpected(self.at, Some("Expected ',' or '}' after property value in JSON")));
                        }
                    }
                }
            }
        }
    }

    /// A property name and the colon after it, with the blanks that follow the colon.
    fn key(&mut self, expected: &str) -> Result<String, String> {
        if !self.is(self.at, '"') {
            return Err(self.unexpected(self.at, Some(expected)));
        }
        let key = self.string()?;
        self.skip_space();
        if !self.is(self.at, ':') {
            return Err(self.unexpected(self.at, Some("Expected ':' after property name in JSON")));
        }
        self.at += 1;
        self.skip_space();
        Ok(key)
    }

    fn literal(&mut self, word: &str, value: Value) -> Result<Value, String> {
        for (k, c) in word.bytes().enumerate() {
            if self.unit(self.at + k) != Some(c as u16) {
                return Err(self.unexpected(self.at + k, None));
            }
        }
        self.at += word.len();
        Ok(value)
    }

    fn string(&mut self) -> Result<String, String> {
        self.at += 1;
        let mut out: Vec<u16> = Vec::new();
        loop {
            let Some(unit) = self.unit(self.at) else {
                return Err(self.located("Unterminated string in JSON", self.at));
            };
            match unit {
                0x22 => {
                    self.at += 1;
                    return Ok(String::from_utf16_lossy(&out));
                }
                0x5C => {
                    let escape_at = self.at + 1;
                    let Some(escape) = self.unit(escape_at) else {
                        return Err(self.unexpected(escape_at, None));
                    };
                    if escape == 0x75 {
                        let mut code = 0u16;
                        for k in 1..=4 {
                            let digit = self.unit(escape_at + k).and_then(|u| char::from_u32(u as u32)).and_then(|c| c.to_digit(16));
                            match digit {
                                Some(d) => code = code * 16 + d as u16,
                                None => return Err(self.located("Bad Unicode escape in JSON", escape_at + k)),
                            }
                        }
                        out.push(code);
                        self.at = escape_at + 5;
                    } else {
                        let decoded = match escape {
                            0x22 | 0x5C | 0x2F => escape,
                            0x62 => 0x08,
                            0x66 => 0x0C,
                            0x6E => 0x0A,
                            0x72 => 0x0D,
                            0x74 => 0x09,
                            0x00..=0xFF => return Err(self.located("Bad escaped character in JSON", escape_at)),
                            _ => return Err(self.unexpected(escape_at, None)),
                        };
                        out.push(decoded);
                        self.at = escape_at + 1;
                    }
                }
                0x00..=0x1F => return Err(self.located("Bad control character in string literal in JSON", self.at)),
                _ => {
                    out.push(unit);
                    self.at += 1;
                }
            }
        }
    }

    fn number(&mut self) -> Result<Value, String> {
        let start = self.at;
        if self.is(self.at, '-') {
            self.at += 1;
            if !self.is_digit(self.at) {
                return Err(self.located("No number after minus sign in JSON", self.at));
            }
        }
        if self.is(self.at, '0') {
            self.at += 1;
            if self.is_digit(self.at) {
                return Err(self.located("Unexpected number in JSON", self.at));
            }
        } else {
            while self.is_digit(self.at) {
                self.at += 1;
            }
        }
        if self.is(self.at, '.') {
            self.at += 1;
            if !self.is_digit(self.at) {
                return Err(self.located("Unterminated fractional number in JSON", self.at));
            }
            while self.is_digit(self.at) {
                self.at += 1;
            }
        }
        if self.is(self.at, 'e') || self.is(self.at, 'E') {
            self.at += 1;
            if self.is(self.at, '+') || self.is(self.at, '-') {
                self.at += 1;
            }
            if !self.is_digit(self.at) {
                return Err(self.located("Exponent part is missing a number in JSON", self.at));
            }
            while self.is_digit(self.at) {
                self.at += 1;
            }
        }
        let f: f64 = self.text(start, self.at).parse().expect("the text was read as a JSON number");
        if f.is_infinite() {
            return Ok(match self.beyond {
                Beyond::Null => Value::Null,
                Beyond::Fraction => Value::from(f.signum() * 0.5),
            });
        }
        Ok(if f.fract() == 0.0 && f.abs() <= 9_007_199_254_740_991.0 {
            Value::from(f as i64)
        } else {
            Value::Number(Number::from_f64(f).expect("a finite number"))
        })
    }
}
