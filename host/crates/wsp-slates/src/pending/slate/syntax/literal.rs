// SPDX-License-Identifier: AGPL-3.0-only
//! Attribute literals: the JS-like literal reader and the seconds, numbers, strings and commands an attribute holds.
use super::*;

struct Literal {
    t: Vec<u16>,
    i: usize,
}

impl Literal {
    pub(super) fn at(&self, i: usize) -> Option<char> {
        self.t.get(i).and_then(|&u| char::from_u32(u as u32))
    }

    pub(super) fn text(&self, from: usize, n: usize) -> String {
        let start = from.min(self.t.len());
        let end = (from + n).min(self.t.len());
        String::from_utf16_lossy(&self.t[start..end])
    }

    pub(super) fn ws(&mut self) {
        while self.at(self.i).is_some_and(is_js_whitespace) {
            self.i += 1;
        }
    }

    pub(super) fn find(&self, q: char, from: usize) -> Option<usize> {
        self.t.get(from..)?.iter().position(|&u| u == q as u16).map(|p| p + from)
    }

    pub(super) fn value(&mut self) -> Result<Value, String> {
        self.ws();
        match self.at(self.i) {
            Some('{') => self.object(),
            Some('[') => self.list(),
            Some(q @ ('"' | '\'')) => self.string(q),
            _ => self.scalar(),
        }
    }

    pub(super) fn object(&mut self) -> Result<Value, String> {
        self.i += 1;
        let mut out: IndexMap<String, Value> = IndexMap::new();
        self.ws();
        if self.at(self.i) == Some('}') {
            self.i += 1;
            return Ok(record(out));
        }
        loop {
            self.ws();
            let key = match self.at(self.i) {
                Some(q @ ('"' | '\'')) => {
                    let Some(end) = self.find(q, self.i + 1) else {
                        return Err("a key never closes".into());
                    };
                    let key = self.text(self.i + 1, end - self.i - 1);
                    self.i = end + 1;
                    key
                }
                _ => {
                    let mut j = self.i;
                    if self.at(j).is_some_and(|c| c.is_ascii_alphabetic() || c == '_') {
                        j += 1;
                        while self.at(j).is_some_and(|c| c.is_ascii_alphanumeric() || c == '_') {
                            j += 1;
                        }
                    }
                    if j == self.i {
                        return Err(format!("a key at \"{}\"", self.text(self.i, 12)));
                    }
                    let key = self.text(self.i, j - self.i);
                    self.i = j;
                    key
                }
            };
            self.ws();
            if self.at(self.i) != Some(':') {
                return Err("expected :".into());
            }
            self.i += 1;
            let v = self.value()?;
            out.insert(key, v);
            self.ws();
            if self.at(self.i) == Some(',') {
                self.i += 1;
                self.ws();
                if self.at(self.i) == Some('}') {
                    self.i += 1;
                    return Ok(record(out));
                }
                continue;
            }
            if self.at(self.i) == Some('}') {
                self.i += 1;
                return Ok(record(out));
            }
            return Err("expected , or }".into());
        }
    }

    pub(super) fn list(&mut self) -> Result<Value, String> {
        self.i += 1;
        let mut out = Vec::new();
        self.ws();
        if self.at(self.i) == Some(']') {
            self.i += 1;
            return Ok(Value::Array(out));
        }
        loop {
            out.push(self.value()?);
            self.ws();
            if self.at(self.i) == Some(',') {
                self.i += 1;
                self.ws();
                if self.at(self.i) == Some(']') {
                    self.i += 1;
                    return Ok(Value::Array(out));
                }
                continue;
            }
            if self.at(self.i) == Some(']') {
                self.i += 1;
                return Ok(Value::Array(out));
            }
            return Err("expected , or ]".into());
        }
    }

    pub(super) fn string(&mut self, q: char) -> Result<Value, String> {
        let mut s: Vec<u16> = Vec::new();
        self.i += 1;
        while self.i < self.t.len() && self.t[self.i] != q as u16 {
            if self.t[self.i] == '\\' as u16 {
                match self.t.get(self.i + 1).copied() {
                    Some(u) if u == 'n' as u16 => s.push('\n' as u16),
                    Some(u) if u == 't' as u16 => s.push('\t' as u16),
                    Some(u) => s.push(u),
                    None => {}
                }
                self.i += 2;
                continue;
            }
            s.push(self.t[self.i]);
            self.i += 1;
        }
        if self.i >= self.t.len() {
            return Err("a string never closes".into());
        }
        self.i += 1;
        Ok(Value::String(String::from_utf16_lossy(&s)))
    }

    fn scalar(&mut self) -> Result<Value, String> {
        let rest = String::from_utf16_lossy(&self.t[self.i.min(self.t.len())..]);
        let Some(m) = LITERAL_SCALAR.find(&rest) else {
            return Err(format!("not a literal: \"{}\"", self.text(self.i, 16)));
        };
        let word = m.as_str();
        self.i += word.len();
        Ok(match word {
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),
            "null" => Value::Null,
            number => jnum(number.parse::<f64>().unwrap_or(f64::NAN)),
        })
    }
}

pub(super) fn parse_literal(text: &str) -> Result<Value, String> {
    let mut p = Literal { t: text.encode_utf16().collect(), i: 0 };
    let v = p.value()?;
    p.ws();
    if p.i < p.t.len() {
        return Err(format!("unexpected \"{}\"", p.text(p.i, 16)));
    }
    Ok(v)
}

pub(super) fn seconds(a: &Attr) -> f64 {
    if a.kind == Kind::Braced {
        return if SECONDS_BRACED.is_match(&a.value) { to_number_text(&a.value) } else { f64::NAN };
    }
    match SECONDS_TEXT.captures(js_trim(&a.value)) {
        None => f64::NAN,
        Some(c) => {
            let factor = match c.get(2).map_or("s", |m| m.as_str()) {
                "m" => 60.0,
                "h" => 3600.0,
                _ => 1.0,
            };
            to_number_text(&c[1]) * factor
        }
    }
}

pub(super) fn to_number_text(text: &str) -> f64 {
    wsp_words::js::to_number(text)
}

pub(super) fn value_text(text: &str) -> Value {
    let t = js_trim(text);
    let ast = parse_slate_expression(t).ast;
    if let Some(SlateExpr::Lit { v, .. }) = &ast {
        return v.clone();
    }
    if let Some(SlateExpr::Neg { arg, .. }) = &ast {
        if let SlateExpr::Lit { v, .. } = arg.as_ref() {
            if let Some(n) = v.as_f64() {
                return jnum(-n);
            }
        }
    }
    if let Some(SlateExpr::Tpl { .. }) = &ast {
        let inner = t.get(1..t.len().saturating_sub(1)).unwrap_or("");
        return json!({"format": inner.replace("\\`", "`")});
    }
    json!({"bind": t})
}

pub(super) fn string_literal(text: &str) -> Option<String> {
    match parse_slate_expression(js_trim(text)).ast {
        Some(SlateExpr::Lit { v: Value::String(s), .. }) => Some(s),
        _ => None,
    }
}

pub(super) fn path_of(text: &str) -> Option<String> {
    match parse_slate_expression(js_trim(text)).ast {
        Some(SlateExpr::Path { head, own, segs, .. }) => Some(slate_path_text(&head, own, &segs)),
        _ => None,
    }
}

pub(super) fn command_text(braced: &str) -> Option<String> {
    let t = js_trim(braced);
    if t.len() >= 2 && t.starts_with('`') && t.ends_with('`') && !t[1..t.len() - 1].contains('`') {
        return if COMMAND_HOLE.is_match(t) { None } else { Some(dedent(&t[1..t.len() - 1])) };
    }
    string_literal(t)
}

pub(super) fn command_body(text: &[Part]) -> Option<String> {
    let holes: Vec<&String> = text
        .iter()
        .filter_map(|p| match p {
            Part::Hole(e) => Some(e),
            Part::Text(_) => None,
        })
        .collect();
    let words: String = text
        .iter()
        .filter_map(|p| match p {
            Part::Text(s) => Some(s.as_str()),
            Part::Hole(_) => None,
        })
        .collect();
    if holes.len() == 1 && js_trim(&words).is_empty() {
        return command_text(holes[0]);
    }
    if holes.is_empty() && !js_trim(&words).is_empty() {
        return Some(dedent(&words));
    }
    None
}
