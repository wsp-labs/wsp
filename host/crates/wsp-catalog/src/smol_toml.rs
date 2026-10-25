// SPDX-License-Identifier: AGPL-3.0-only
//! The TOML the TS host reads is read by smol-toml 1.9.0, which takes TOML 1.1 and fails in its own words; this is its
//! `parse`, ported, so a file reads as the same tree or fails with the same first line. A port of smol-toml's parse.js,
//! struct.js, primitive.js, extract.js, util.js and date.js. The spec is spec/mcp_toml_refer.md.
//!
//! smol-toml is Copyright (c) Squirrel Chat et al., All rights reserved, under the BSD-3-Clause license: redistribution
//! and use in source and binary forms, with or without modification, are permitted provided that the copyright notice,
//! this list of conditions and the following disclaimer are retained, that the same are reproduced in the documentation
//! with a binary form, and that neither the name of the copyright holder nor its contributors endorse products derived
//! from it without permission. THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY
//! EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS
//! FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY
//! DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES.
//!
//! What differs from the JavaScript: a number is `Integer` where JavaScript cannot tell it from one (`20.0` is 20), a
//! date is the `toml` crate's where it reads the text and a string where it does not, and a table lists its keys the
//! way a JavaScript object does (array indices first).
use std::str::FromStr;

use indexmap::IndexMap;

use crate::plugins::js_entries;
use wsp_words::js::js_trim;

type R<T> = Result<T, String>;

/// The tree smol-toml's `parse` returns; an error is the first line of its message.
pub fn parse_toml(text: &str) -> Result<toml::Table, String> {
    let mut ctx = Ctx { s: text.encode_utf16().collect(), p: 0, d: MAX_DEPTH };
    Ok(table_value(parse(&mut ctx)?))
}

/// smol-toml allows 1000 levels of arrays and inline tables; the recursion here stays within a test thread's stack at
/// this many, and the words past it are smol-toml's own.
const MAX_DEPTH: i32 = 256;

const BASIC: u16 = 0x22;
const LITERAL: u16 = 0x27;

#[derive(Debug)]
enum Node {
    Table(Tbl),
    Array(Vec<Node>),
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Date(String),
}

#[derive(Debug, Default)]
struct Tbl {
    map: IndexMap<String, Node>,
    /// An inline table: nothing is added to it by a later key.
    sealed: bool,
}

struct Ctx {
    s: Vec<u16>,
    p: usize,
    d: i32,
}

/// `TomlError`'s message: the words, then the lines around the pointer, with a caret under the place.
fn message_at(text: &[u16], ptr: usize, message: &str) -> String {
    let text = String::from_utf16_lossy(text);
    let before: String = String::from_utf16_lossy(&text.encode_utf16().take(ptr).collect::<Vec<u16>>());
    let split = |s: &str| -> Vec<String> { s.replace("\r\n", "\n").split('\n').map(str::to_string).collect() };
    let seen = split(&before);
    let line = seen.len();
    let column = seen.last().map_or(0, |l| l.encode_utf16().count()) + 1;
    let lines = split(&text);
    let width = ((line + 1) as f64).log10() as usize + 1;
    let mut block = String::new();
    for i in line.saturating_sub(1)..=line + 1 {
        let Some(l) = i.checked_sub(1).and_then(|at| lines.get(at)).filter(|l| !l.is_empty()) else { continue };
        block.push_str(&format!("{i:<width$}:  {l}\n"));
        if i == line {
            block.push_str(&" ".repeat(width + column + 2));
            block.push_str("^\n");
        }
    }
    format!("Invalid TOML document: {message}\n\n{block}")
}

fn is_digit(c: Option<u16>, base: u16) -> bool {
    c.is_some_and(|c| {
        if base == 16 {
            (0x30..0x3a).contains(&c) || (0x41..0x47).contains(&c) || (0x61..0x67).contains(&c)
        } else {
            c > 0x2f && c < 0x30 + base
        }
    })
}

fn is_end_of_value(c: Option<u16>, delim: Option<u16>) -> bool {
    let Some(c) = c else { return false };
    matches!(c, 0x20 | 0x9 | 0xa | 0xd | 0x23) || (delim.is_some() && (Some(c) == delim || c == 0x2c))
}

impl Ctx {
    fn fail<T>(&self, message: &str) -> R<T> {
        self.fail_at(message, self.p)
    }

    fn fail_at<T>(&self, message: &str, ptr: usize) -> R<T> {
        Err(message_at(&self.s, ptr, message))
    }

    fn at(&self, i: usize) -> Option<u16> {
        self.s.get(i).copied()
    }

    fn text(&self, from: usize, to: usize) -> String {
        let to = to.min(self.s.len());
        String::from_utf16_lossy(&self.s[from.min(to)..to])
    }

    fn skip_comment(&mut self) -> R<()> {
        while self.p < self.s.len() {
            let c = self.s[self.p];
            if c == 0xa {
                break;
            }
            if c == 0xd && self.at(self.p + 1) == Some(0xa) {
                self.p += 1;
                break;
            }
            if (c < 0x20 && c != 0x9) || c == 0x7f {
                return self.fail("control characters are not allowed in comments");
            }
            self.p += 1;
        }
        Ok(())
    }

    fn skip_void(&mut self, ban_new_lines: bool, ban_comments: bool) -> R<()> {
        let mut c = 0;
        while self.p < self.s.len() {
            while self.p < self.s.len() {
                c = self.s[self.p];
                let blank = c == 0x20 || c == 0x9 || (!ban_new_lines && (c == 0xa || (c == 0xd && self.at(self.p + 1) == Some(0xa))));
                if !blank {
                    break;
                }
                self.p += 1;
            }
            if ban_comments || c != 0x23 {
                break;
            }
            self.skip_comment()?;
        }
        Ok(())
    }

    fn parse_string(&mut self) -> R<String> {
        let start_ptr = self.p;
        let first = self.s[self.p];
        self.p += 1;
        let is_literal = first == LITERAL;
        let is_multiline = Some(first) == self.at(self.p) && Some(first) == self.at(self.p + 1);
        if is_multiline {
            self.p += 2;
            if self.at(self.p) == Some(0xa) {
                self.p += 1;
            } else if self.at(self.p) == Some(0xd) && self.at(self.p + 1) == Some(0xa) {
                self.p += 2;
            }
        }
        let mut parsed: Vec<u16> = Vec::new();
        let mut slice_start = self.p;
        let mut state = 0;
        while self.p < self.s.len() {
            let c = self.s[self.p];
            if is_multiline && (c == 0xa || (c == 0xd && self.at(self.p + 1) == Some(0xa))) {
                state = if state != 0 { 3 } else { 0 };
            } else if (c < 0x20 && c != 0x9) || c == 0x7f {
                return self.fail("control characters are not allowed in strings");
            } else if (state == 0 || state == 3)
                && c == first
                && (!is_multiline || (self.at(self.p + 1) == Some(first) && self.at(self.p + 2) == Some(first)))
            {
                if is_multiline {
                    if self.at(self.p + 3) == Some(first) {
                        self.p += 1;
                    }
                    if self.at(self.p + 3) == Some(first) {
                        self.p += 1;
                    }
                }
                if state == 0 {
                    parsed.extend_from_slice(&self.s[slice_start..self.p]);
                }
                self.p += if is_multiline { 3 } else { 1 };
                return Ok(String::from_utf16_lossy(&parsed));
            } else if state == 0 {
                if !is_literal && c == 0x5c {
                    parsed.extend_from_slice(&self.s[slice_start..self.p]);
                    slice_start = self.p;
                    state = 1;
                }
            } else if state == 1 {
                if c == 0x78 || c == 0x75 || c == 0x55 {
                    let err_ptr = self.p - 1;
                    self.p += 1;
                    let digits = if c == 0x78 {
                        2
                    } else if c == 0x75 {
                        4
                    } else {
                        8
                    };
                    let mut value: i32 = 0;
                    for _ in 0..digits {
                        let Some(digit) = self.at(self.p).and_then(|h| char::from_u32(h as u32)).and_then(|h| h.to_digit(16)) else {
                            return self.fail("invalid non-hex character in unicode escape");
                        };
                        value = (value << 4) | digit as i32;
                        self.p += 1;
                    }
                    let Some(unit) = u32::try_from(value).ok().and_then(char::from_u32) else {
                        return self.fail_at("invalid unicode escape", err_ptr);
                    };
                    let mut buf = [0u16; 2];
                    parsed.extend_from_slice(unit.encode_utf16(&mut buf));
                    slice_start = self.p;
                    self.p -= 1;
                    state = 0;
                } else if is_multiline && (c == 0x20 || c == 0x9) {
                    state = 2;
                } else {
                    let plain = match c {
                        0x62 => 0x8,
                        0x74 => 0x9,
                        0x6e => 0xa,
                        0x66 => 0xc,
                        0x72 => 0xd,
                        0x65 => 0x1b,
                        0x22 => 0x22,
                        0x5c => 0x5c,
                        _ => return self.fail("unrecognised escape sequence"),
                    };
                    parsed.push(plain);
                    slice_start = self.p + 1;
                    state = 0;
                }
            } else if c != 0x20 && c != 0x9 {
                if state == 2 {
                    return self.fail_at("invalid escape: only line-ending whitespace may be escaped", slice_start);
                }
                state = if !is_literal && c == 0x5c { 1 } else { 0 };
                slice_start = self.p;
            }
            self.p += 1;
        }
        self.fail_at("unfinished string", start_ptr)
    }

    fn parse_key(&mut self, end: u16) -> R<Vec<String>> {
        let start_ptr = self.p;
        let mut state = 0;
        let mut parsed: Vec<String> = Vec::new();
        let mut slice_start = 0;
        let mut c = self.at(self.p);
        loop {
            if c == Some(end) {
                if state == 0 {
                    return self.fail("unexpected end of key");
                }
                if state == 1 {
                    parsed.push(self.text(slice_start, self.p));
                }
                self.p += 1;
                return Ok(parsed);
            } else if c == Some(0x2e) {
                if state == 0 {
                    return self.fail("illegal empty bare key");
                }
                if state == 1 {
                    parsed.push(self.text(slice_start, self.p));
                }
                state = 0;
            } else if state == 0 && (c == Some(BASIC) || c == Some(LITERAL)) {
                if c == self.at(self.p + 1) && c == self.at(self.p + 2) {
                    return self.fail("illegal quoted key: multiline strings are not allowed");
                }
                let key = self.parse_string()?;
                parsed.push(key);
                state = 2;
                self.p -= 1;
            } else if c == Some(0x20) || c == Some(0x9) {
                if state == 1 {
                    parsed.push(self.text(slice_start, self.p));
                    state = 2;
                }
            } else if state == 2
                || c.is_some_and(|c| (c < 0x30 && c != 0x2d) || (c > 0x39 && c < 0x41) || (c > 0x5a && c < 0x61 && c != 0x5f) || c > 0x7a)
            {
                return self.fail("illegal character in key");
            } else if state == 0 {
                state = 1;
                slice_start = self.p;
            }
            self.p += 1;
            c = self.at(self.p);
            if matches!(c, None | Some(0)) {
                break;
            }
        }
        self.fail_at("incomplete key-value: cannot find end of key", start_ptr)
    }

    fn parse_inline_table(&mut self) -> R<Node> {
        let start_ptr = self.p;
        self.p += 1;
        let mut res = Tbl { map: IndexMap::new(), sealed: true };
        while self.p < self.s.len() {
            self.skip_void(false, false)?;
            if self.at(self.p) == Some(0x7d) {
                self.p += 1;
                return Ok(Node::Table(res));
            }
            let err_ptr = self.p;
            let key = self.parse_key(0x3d)?;
            let mut t = &mut res;
            let mut has_own = false;
            for (i, k) in key.iter().enumerate() {
                if i > 0 {
                    let previous = &key[i - 1];
                    let Some(Node::Table(next)) = (if has_own {
                        t.map.get_mut(previous)
                    } else {
                        Some(t.map.entry(previous.clone()).or_insert_with(|| Node::Table(Tbl::default())))
                    }) else {
                        return self.fail_at("trying to redefine an already defined value", err_ptr);
                    };
                    t = next;
                }
                has_own = t.map.contains_key(k);
                if has_own && !matches!(t.map.get(k), Some(Node::Table(inner)) if !inner.sealed) {
                    return self.fail_at("trying to redefine an already defined value", err_ptr);
                }
            }
            if has_own {
                return self.fail_at("trying to redefine an already defined value", err_ptr);
            }
            self.skip_void(true, true)?;
            let value = self.extract_value(Some(0x7d))?;
            t.map.insert(key[key.len() - 1].clone(), value);
            self.skip_void(false, false)?;
            let c = self.at(self.p);
            self.p += 1;
            if c == Some(0x7d) {
                return Ok(Node::Table(res));
            }
            if c != Some(0x2c) {
                return self.fail_at("expected comma or end of structure", self.p - 1);
            }
        }
        self.fail_at("unfinished table", start_ptr)
    }

    fn parse_array(&mut self) -> R<Node> {
        let start_ptr = self.p;
        self.p += 1;
        let mut res = Vec::new();
        while self.p < self.s.len() {
            self.skip_void(false, false)?;
            if self.at(self.p) == Some(0x5d) {
                self.p += 1;
                return Ok(Node::Array(res));
            }
            res.push(self.extract_value(Some(0x5d))?);
            self.skip_void(false, false)?;
            let c = self.at(self.p);
            self.p += 1;
            if c == Some(0x5d) {
                return Ok(Node::Array(res));
            }
            if c != Some(0x2c) {
                return self.fail_at("expected comma or end of structure", self.p - 1);
            }
        }
        self.fail_at("unfinished array", start_ptr)
    }

    fn extract_value(&mut self, end: Option<u16>) -> R<Node> {
        let err_ptr = self.p;
        let c = self.at(self.p);
        if c == Some(0x5b) || c == Some(0x7b) {
            let deep = self.d == 0;
            self.d -= 1;
            if deep {
                return self.fail("document contains excessively nested structures. aborting.");
            }
            let value = if c == Some(0x5b) { self.parse_array()? } else { self.parse_inline_table()? };
            self.d += 1;
            return Ok(value);
        }
        if c == Some(BASIC) || c == Some(LITERAL) {
            return self.parse_string().map(Node::Str);
        }
        if c == Some(0x74) {
            for want in [0x72, 0x75, 0x65] {
                self.p += 1;
                if self.at(self.p) != Some(want) {
                    return self.fail_at("invalid value", err_ptr);
                }
            }
            self.p += 1;
            return Ok(Node::Bool(true));
        }
        if c == Some(0x66) {
            for want in [0x61, 0x6c, 0x73, 0x65] {
                self.p += 1;
                if self.at(self.p) != Some(want) {
                    return self.fail_at("invalid value", err_ptr);
                }
            }
            self.p += 1;
            return Ok(Node::Bool(false));
        }
        if c == Some(0x2b) || c == Some(0x2d) {
            let start = self.p;
            self.p += 1;
            let next = self.at(self.p);
            return self.parse_number(start, next, if c == Some(0x2b) { 1 } else { -1 }, end);
        }
        if self.at(self.p + 4) == Some(0x2d) && self.at(self.p + 7) == Some(0x2d) {
            return self.parse_date(c, end);
        }
        if self.at(self.p + 2) == Some(0x3a) {
            return self.parse_time(c, end);
        }
        self.parse_number(self.p, c, 0, end)
    }

    fn parse_number(&mut self, start: usize, start_chr: Option<u16>, sign: i64, end_chr: Option<u16>) -> R<Node> {
        let mut c = start_chr;
        let mut state = 0;
        let mut has_underscores = false;
        if c == Some(0x69) {
            for want in [0x6e, 0x66] {
                self.p += 1;
                if self.at(self.p) != Some(want) {
                    return self.fail_at("invalid value", start);
                }
            }
            self.p += 1;
            return Ok(Node::Float(if sign < 0 { f64::NEG_INFINITY } else { f64::INFINITY }));
        }
        if c == Some(0x6e) {
            for want in [0x61, 0x6e] {
                self.p += 1;
                if self.at(self.p) != Some(want) {
                    return self.fail_at("invalid value", start);
                }
            }
            self.p += 1;
            return Ok(Node::Float(f64::NAN));
        }
        if c == Some(0x30) {
            self.p += 1;
            if self.p >= self.s.len() {
                return Ok(Node::Int(0));
            }
            c = self.at(self.p);
            if is_end_of_value(c, end_chr) {
                return Ok(Node::Int(0));
            }
            if sign == 0 {
                if c == Some(0x78) {
                    return self.parse_integer_base_n(start, 16, end_chr);
                } else if c == Some(0x62) {
                    return self.parse_integer_base_n(start, 2, end_chr);
                } else if c == Some(0x6f) {
                    return self.parse_integer_base_n(start, 8, end_chr);
                }
            }
            if c == Some(0x2e) {
                state = 2;
            } else if c == Some(0x65) || c == Some(0x45) {
                state = 4;
            } else {
                return self.fail_at("illegal leading zero", start);
            }
        } else if !is_digit(c, 10) {
            return self.fail_at("invalid value", start);
        }
        loop {
            self.p += 1;
            if self.p >= self.s.len() {
                break;
            }
            c = self.at(self.p);
            if is_end_of_value(c, end_chr) {
                break;
            }
            if state == 0 {
                state = 1;
            }
            if c == Some(0x5f) {
                if state & 1 == 0 {
                    return self.fail("illegal underscore");
                }
                state += 11;
                has_underscores = true;
            } else if state == 1 && c == Some(0x2e) {
                state = 2;
            } else if (state == 1 || state == 3) && (c == Some(0x65) || c == Some(0x45)) {
                state = 4;
            } else if state == 4 && (c == Some(0x2b) || c == Some(0x2d)) {
            } else if !is_digit(c, 10) {
                return self.fail("illegal character in numeric literal");
            } else if state > 9 {
                state -= 11;
            } else if state & 1 == 0 {
                state += 1;
            }
        }
        if state == 0 {
            let digit = i64::from(start_chr.unwrap_or(0x30)) - 0x30;
            return Ok(Node::Int(digit * if sign == 0 { 1 } else { sign }));
        }
        if state & 1 == 0 {
            return self.fail_at("unfinished numeric value", start);
        }
        let mut text = self.text(start, self.p);
        if has_underscores {
            text = text.replace('_', "");
        }
        if state > 1 {
            Ok(float_node(text.parse::<f64>().unwrap_or(f64::NAN)))
        } else {
            parse_integer(&text, 10).ok_or(()).or_else(|()| self.fail_at("integer value cannot be represented losslessly", start))
        }
    }

    fn parse_integer_base_n(&mut self, start: usize, base: u16, end_chr: Option<u16>) -> R<Node> {
        let mut underscore = 1;
        loop {
            self.p += 1;
            if self.p >= self.s.len() {
                break;
            }
            let c = self.at(self.p);
            if is_end_of_value(c, end_chr) {
                break;
            }
            if c == Some(0x5f) {
                if underscore & 1 == 1 {
                    return self.fail("illegal underscore");
                }
                underscore = 3;
            } else if !is_digit(c, base) {
                return self.fail("illegal character in numeric literal");
            } else if underscore & 1 == 1 {
                underscore -= 1;
            }
        }
        if underscore & 1 == 1 {
            return self.fail("unfinished numeric value");
        }
        let mut text = self.text(start + 2, self.p);
        if underscore != 0 {
            text = text.replace('_', "");
        }
        parse_integer(&text, base).ok_or(()).or_else(|()| self.fail_at("integer value cannot be represented losslessly", start))
    }

    fn parse_date(&mut self, c: Option<u16>, end_chr: Option<u16>) -> R<Node> {
        let start = self.p;
        self.p += 1;
        let mut digits = is_digit(c, 10);
        for _ in 0..3 {
            if !digits {
                break;
            }
            digits = is_digit(self.at(self.p), 10);
            self.p += 1;
        }
        if !digits {
            self.p = start;
            return self.parse_number(start, c, 0, end_chr);
        }
        self.p += 5;
        let day_digit = is_digit(self.at(self.p), 10);
        self.p += 1;
        if !day_digit {
            return self.fail_at("invalid date-time: date part is malformed", start);
        }
        let mut unsafe_separator = false;
        let c = self.at(self.p);
        let date_only = self.p >= self.s.len() || {
            let space_ok = c == Some(0x20) && {
                unsafe_separator = true;
                is_digit(self.at(self.p + 1), 10)
            };
            !space_ok && c != Some(0x54) && c != Some(0x74)
        };
        if date_only {
            let t = self.text(start, self.p);
            return self.read_date(&t, 3, false, start);
        }
        self.p += 3;
        if self.at(self.p) != Some(0x3a) {
            return self.fail_at("invalid date-time: time part is malformed", start);
        }
        self.p += 3;
        if self.at(self.p) == Some(0x3a) {
            self.p += 3;
        }
        if self.at(self.p) == Some(0x2e) {
            loop {
                self.p += 1;
                if !is_digit(self.at(self.p), 10) {
                    break;
                }
            }
        }
        match self.at(self.p) {
            Some(0x5a | 0x7a) => {
                self.p += 1;
                let t = self.text(start, self.p);
                self.read_date(&t, 1, unsafe_separator, start)
            }
            Some(0x2b | 0x2d) => {
                self.p += 6;
                let t = self.text(start, self.p);
                self.read_date(&t, 1, unsafe_separator, start)
            }
            _ => {
                let t = self.text(start, self.p);
                self.read_date(&t, 2, unsafe_separator, start)
            }
        }
    }

    fn parse_time(&mut self, c: Option<u16>, end_chr: Option<u16>) -> R<Node> {
        let start = self.p;
        let second = if is_digit(c, 10) {
            self.p += 1;
            is_digit(self.at(self.p), 10)
        } else {
            false
        };
        if !second {
            self.p = self.p.wrapping_sub(1);
            return self.parse_number(self.p, c, 0, end_chr);
        }
        self.p += 4;
        if self.at(self.p) == Some(0x3a) {
            self.p += 3;
        }
        if self.at(self.p) == Some(0x2e) {
            loop {
                self.p += 1;
                if !is_digit(self.at(self.p), 10) {
                    break;
                }
            }
        }
        let t = self.text(start, self.p);
        self.read_date(&t, 4, false, start)
    }
}

fn float_node(f: f64) -> Node {
    if f.is_finite() && f.fract() == 0.0 && f.abs() < 9.0e15 {
        Node::Int(f as i64)
    } else {
        Node::Float(f)
    }
}

fn parse_integer(text: &str, base: u16) -> Option<Node> {
    match i128::from_str_radix(text.strip_prefix('+').unwrap_or(text), base as u32) {
        Ok(n) if n.abs() <= 9_007_199_254_740_991 => Some(Node::Int(n as i64)),
        _ => None,
    }
}

/// `new TomlDate(text, kind, unsafe_separator).isValid()` and the date it holds: kind 1 an offset date-time, 2 a local
/// one, 3 a local date, 4 a local time.
/// The date a text holds, or smol-toml's refusal of it, the pointer at `start`.
impl Ctx {
    fn read_date(&self, text: &str, kind: u8, unsafe_separator: bool, start: usize) -> R<Node> {
        match date_node(text, kind, unsafe_separator) {
            Some(date) => Ok(date),
            None => self.fail_at("invalid date", start),
        }
    }
}

fn date_node(text: &str, kind: u8, unsafe_separator: bool) -> Option<Node> {
    let chars: Vec<char> = text.chars().collect();
    let number = |from: usize, to: usize| -> f64 {
        let digits: String = chars.get(from.min(chars.len())..to.min(chars.len())).unwrap_or_default().iter().collect();
        let digits = js_trim(&digits);
        if digits.is_empty() {
            0.0
        } else {
            digits.parse().unwrap_or(f64::NAN)
        }
    };
    let mut date: Vec<char> = chars.clone();
    if kind < 3 {
        if number(11, 13) > 23.0 {
            date.clear();
        } else {
            if kind == 2 {
                date.push('Z');
            }
            if unsafe_separator && date.len() > 10 {
                date[10] = 'T';
            }
        }
    } else if kind == 4 {
        date = if number(0, 2) > 23.0 { Vec::new() } else { format!("0000-01-01T{text}Z").chars().collect() };
    }
    v8_date_valid(&date.iter().collect::<String>()).then(|| Node::Date(text.to_string()))
}

/// Whether V8's `new Date(text)` reads the text: its ISO format, with a month 1-12, a day 1-31, an hour 0-23, minutes
/// and seconds 0-59, and an offset.
fn v8_date_valid(text: &str) -> bool {
    let b = text.as_bytes();
    let num = |from: usize, len: usize| -> Option<u32> {
        let digits = b.get(from..from + len)?;
        digits.iter().all(u8::is_ascii_digit).then(|| digits.iter().fold(0, |n, d| n * 10 + u32::from(d - b'0')))
    };
    if num(0, 4).is_none() {
        return false;
    }
    let mut at = 4;
    if b.get(at) == Some(&b'-') {
        if !num(5, 2).is_some_and(|m| (1..=12).contains(&m)) {
            return false;
        }
        at = 7;
        if b.get(at) == Some(&b'-') {
            if !num(8, 2).is_some_and(|d| (1..=31).contains(&d)) {
                return false;
            }
            at = 10;
        }
    }
    if at == b.len() {
        return true;
    }
    if !matches!(b.get(at), Some(b'T' | b't')) || at != 10 {
        return false;
    }
    if !num(11, 2).is_some_and(|h| h <= 23) || b.get(13) != Some(&b':') || !num(14, 2).is_some_and(|m| m <= 59) {
        return false;
    }
    at = 16;
    if b.get(at) == Some(&b':') {
        if !num(17, 2).is_some_and(|s| s <= 59) {
            return false;
        }
        at = 19;
        if b.get(at) == Some(&b'.') {
            let digits = b[at + 1..].iter().take_while(|d| d.is_ascii_digit()).count();
            if digits == 0 {
                return false;
            }
            at += 1 + digits;
        }
    }
    match b.get(at) {
        None => true,
        Some(b'Z' | b'z') => at + 1 == b.len(),
        Some(b'+' | b'-') => {
            b.get(at + 3) == Some(&b':')
                && num(at + 1, 2).is_some_and(|h| h <= 23)
                && num(at + 4, 2).is_some_and(|m| m <= 59)
                && at + 6 == b.len()
        }
        _ => false,
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Dotted,
    Explicit,
    Array,
    ArrayDotted,
}

#[derive(Debug)]
struct Meta {
    t: Kind,
    d: bool,
    i: usize,
    c: IndexMap<String, Meta>,
}

impl Meta {
    fn new(t: Kind) -> Meta {
        Meta { t, d: false, i: 0, c: IndexMap::new() }
    }
}

/// A step from the root to the current table: the key, and whether the key holds an array of tables whose last table
/// is the one meant.
#[derive(Clone, Debug)]
struct Seg {
    key: String,
    last: bool,
}

type Metas = IndexMap<String, Meta>;

/// The table and the metas a path leads to.
fn nav<'a>(mut t: &'a mut Tbl, mut m: &'a mut Metas, path: &[Seg]) -> (&'a mut Tbl, &'a mut Metas) {
    for seg in path {
        let node = t.map.get_mut(&seg.key).expect("a path names keys that are there");
        let meta = m.get_mut(&seg.key).expect("a key that is there has a meta");
        if seg.last {
            let Node::Array(items) = node else { unreachable!("a last segment names an array") };
            let at = items.len() - 1;
            let Some(Node::Table(table)) = items.get_mut(at) else { unreachable!("an array of tables holds tables") };
            t = table;
            m = &mut meta.c.get_mut(&at.to_string()).expect("an array element has a meta").c;
        } else {
            let Node::Table(table) = node else { unreachable!("a path names tables") };
            t = table;
            m = &mut meta.c;
        }
    }
    (t, m)
}

struct Peeked {
    /// The path to the table holding the key, then the key.
    parent: Vec<Seg>,
    key: String,
    /// The path to the table a header opens (the key's own table, or the new table of an array).
    opened: Vec<Seg>,
}

/// `peekTable`: what a key's declaration claims in the document; nothing where the document already holds it another way.
fn peek_table(root: &mut Tbl, metas: &mut Metas, base: &[Seg], key: &[String], kind: Kind) -> Option<Peeked> {
    let mut path: Vec<Seg> = base.to_vec();
    let (mut t, mut m) = nav(root, metas, base);
    let mut has_own = false;
    let mut k = String::new();
    for (i, next) in key.iter().enumerate() {
        if i > 0 {
            let state_t = m.get(&k).expect("the key before has a meta").t;
            if kind == Kind::Dotted && (state_t == Kind::Explicit || state_t == Kind::Array) {
                return None;
            }
            let last = state_t == Kind::Array;
            if !has_own {
                t.map.insert(k.clone(), Node::Table(Tbl::default()));
            }
            path.push(Seg { key: k.clone(), last });
            let (nt, nm) = step(t, m, &k, last);
            t = nt;
            m = nm;
        }
        k = next.clone();
        has_own = t.map.contains_key(&k);
        if has_own && m.get(&k).is_some_and(|s| s.t == Kind::Dotted && s.d) {
            return None;
        }
        if !has_own {
            let own = if i < key.len() - 1 && kind == Kind::Array { Kind::ArrayDotted } else { kind };
            m.insert(k.clone(), Meta::new(own));
        }
    }
    let state_t = m.get(&k).expect("the last key has a meta").t;
    if state_t != kind && !(kind == Kind::Explicit && state_t == Kind::ArrayDotted) {
        return None;
    }
    let mut opened = path.clone();
    if kind == Kind::Array {
        let state = m.get_mut(&k).expect("the last key has a meta");
        if !state.d {
            state.d = true;
            t.map.insert(k.clone(), Node::Array(Vec::new()));
        }
        let Some(Node::Array(items)) = t.map.get_mut(&k) else { return None };
        items.push(Node::Table(Tbl::default()));
        let index = state.i;
        state.i += 1;
        let mut element = Meta::new(Kind::Explicit);
        element.d = true;
        state.c.insert(index.to_string(), element);
        opened.push(Seg { key: k.clone(), last: true });
        return Some(Peeked { parent: path, key: k, opened });
    }
    let state = m.get_mut(&k).expect("the last key has a meta");
    if state.d {
        return None;
    }
    state.d = true;
    if kind == Kind::Explicit {
        if !has_own {
            t.map.insert(k.clone(), Node::Table(Tbl::default()));
        }
        opened.push(Seg { key: k.clone(), last: false });
    } else if kind == Kind::Dotted && has_own {
        return None;
    }
    Some(Peeked { parent: path, key: k, opened })
}

fn step<'a>(t: &'a mut Tbl, m: &'a mut Metas, key: &str, last: bool) -> (&'a mut Tbl, &'a mut Metas) {
    let seg = [Seg { key: key.to_string(), last }];
    nav(t, m, &seg)
}

fn parse(ctx: &mut Ctx) -> R<Tbl> {
    let mut res = Tbl::default();
    let mut meta: Metas = IndexMap::new();
    let mut current: Vec<Seg> = Vec::new();
    if ctx.at(0) == Some(0xfeff) {
        ctx.p += 1;
    }
    ctx.skip_void(false, false)?;
    while ctx.p < ctx.s.len() {
        if ctx.at(ctx.p) == Some(0x5b) {
            ctx.p += 1;
            let is_table_array = ctx.at(ctx.p) == Some(0x5b);
            if is_table_array {
                ctx.p += 1;
            }
            let key_start = ctx.p;
            let key = ctx.parse_key(0x5d)?;
            if is_table_array {
                if ctx.at(ctx.p) != Some(0x5d) {
                    return ctx.fail("expected end of table array declaration");
                }
                ctx.p += 1;
            }
            let Some(peeked) = peek_table(&mut res, &mut meta, &[], &key, if is_table_array { Kind::Array } else { Kind::Explicit }) else {
                return ctx.fail_at("trying to redefine an already defined table or value", key_start);
            };
            current = peeked.opened;
        } else {
            let key_start = ctx.p;
            let key = ctx.parse_key(0x3d)?;
            let Some(peeked) = peek_table(&mut res, &mut meta, &current, &key, Kind::Dotted) else {
                return ctx.fail_at("trying to redefine an already defined table or value", key_start);
            };
            ctx.skip_void(true, true)?;
            let value = ctx.extract_value(None)?;
            let (t, _) = nav(&mut res, &mut meta, &peeked.parent);
            t.map.insert(peeked.key, value);
        }
        ctx.skip_void(true, false)?;
        if ctx.p < ctx.s.len() {
            let c = ctx.at(ctx.p);
            if c != Some(0xa) && (c != Some(0xd) || ctx.at(ctx.p + 1) != Some(0xa)) {
                return ctx.fail("each key-value declaration must be followed by an end-of-line");
            }
        }
        ctx.skip_void(false, false)?;
    }
    Ok(res)
}

fn table_value(t: Tbl) -> toml::Table {
    let mut out = toml::Table::new();
    let ordered: Vec<(&String, &Node)> = js_order(&t.map);
    for (k, v) in ordered {
        out.insert(k.clone(), value_of(v));
    }
    out
}

/// A table's entries as JavaScript lists them: array-index keys first, ascending, then the rest in order.
fn js_order(map: &IndexMap<String, Node>) -> Vec<(&String, &Node)> {
    let as_json: serde_json::Map<String, serde_json::Value> = map.keys().map(|k| (k.clone(), serde_json::Value::Null)).collect();
    let order: Vec<String> = js_entries(&as_json).into_iter().map(|(k, _)| k.clone()).collect();
    order.iter().filter_map(|k| map.get_key_value(k)).collect()
}

fn value_of(n: &Node) -> toml::Value {
    match n {
        Node::Table(t) => {
            let mut table = toml::Table::new();
            for (k, v) in js_order(&t.map) {
                table.insert(k.clone(), value_of(v));
            }
            toml::Value::Table(table)
        }
        Node::Array(items) => toml::Value::Array(items.iter().map(value_of).collect()),
        Node::Str(s) => toml::Value::String(s.clone()),
        Node::Int(i) => toml::Value::Integer(*i),
        Node::Float(f) => toml::Value::Float(*f),
        Node::Bool(b) => toml::Value::Boolean(*b),
        Node::Date(text) => date_value(text),
    }
}

/// The date the `toml` crate reads from the text, the text as a string where it does not read it.
fn date_value(text: &str) -> toml::Value {
    let text = &roll_day(text);
    let normal = text.replace(' ', "T");
    let with_seconds = match normal.find(':') {
        Some(at) if normal[at + 1..].find(':').is_none_or(|n| n > 2) && normal.len() >= at + 3 => {
            let end = at + 3;
            format!("{}:00{}", &normal[..end], &normal[end..])
        }
        _ => normal.clone(),
    };
    let tries = [text.to_string(), normal, with_seconds];
    tries
        .iter()
        .find_map(|t| toml::value::Datetime::from_str(t).ok())
        .map_or_else(|| toml::Value::String(text.to_string()), toml::Value::Datetime)
}

/// A day past the end of its month as JavaScript's `Date` takes it: the first days of the next month (`02-30` of a
/// common year is `03-02`).
fn roll_day(text: &str) -> String {
    let digits = |from: usize, len: usize| {
        text.get(from..from + len).filter(|d| d.bytes().all(|b| b.is_ascii_digit())).and_then(|d| d.parse::<u32>().ok())
    };
    let (Some(mut year), Some(mut month), Some(mut day)) = (digits(0, 4), digits(5, 2), digits(8, 2)) else {
        return text.to_string();
    };
    let long = |year: u32, month: u32| match month {
        2 if (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    while day > long(year, month) {
        day -= long(year, month);
        month += 1;
        if month > 12 {
            month = 1;
            year += 1;
        }
    }
    format!("{year:04}-{month:02}-{day:02}{}", text.get(10..).unwrap_or(""))
}
