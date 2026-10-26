// SPDX-License-Identifier: AGPL-3.0-only
//! protocol/src/slate/syntax.ts at 65f6b89c7.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use indexmap::{IndexMap, IndexSet};
use regex::Regex;
use serde_json::{json, Map, Value};
use wsp_wire::text::json_stringify;
use wsp_wire::types::js_number::JsNumber;
use wsp_words::js::{is_js_whitespace, js_key_order, js_trim, number_string, upper_first, utf16, JS_WS};

use super::*;

const SLATE_PANE_KINDS: &[&str] = &["preview", "terminal", "diff", "pr", "files", "machine", "processes", "agents", "slate"];
const DECLARATIONS: &[&str] = &["value", "secret", "derived", "run", "when"];
const READ_DEPTH: usize = SLATE_LIMITS.depth + 10;

fn js_re(pattern: &str) -> Regex {
    Regex::new(&pattern.replace('@', JS_WS)).expect("a syntax pattern compiles")
}

static HTML_ENTITY: LazyLock<Regex> = LazyLock::new(|| js_re(r"&(?:[A-Za-z]+|#[0-9]+|#x[0-9A-Fa-f]+);"));
static ENTITY_QUOTE: LazyLock<Regex> = LazyLock::new(|| js_re(r"^&(?:quot|apos|#34|#39|#x22|#x27);$"));
static SLATE_NAME: LazyLock<Regex> = LazyLock::new(|| js_re(r"^[a-z_][a-zA-Z0-9_]{0,47}$"));
static SLATE_ID: LazyLock<Regex> = LazyLock::new(|| js_re(r"^[A-Za-z][A-Za-z0-9_-]{0,47}$"));
static IDENT: LazyLock<Regex> = LazyLock::new(|| js_re(r"^[A-Za-z_][A-Za-z0-9_]*$"));
static FILE_BRACED: LazyLock<Regex> = LazyLock::new(|| js_re(r"^\{[@]*`([\s\S]*)`[@]*\}$"));
static COMMAND_HOLE: LazyLock<Regex> = LazyLock::new(|| js_re(r"\$\{[@]*\$"));
static HANDLER_FN: LazyLock<Regex> = LazyLock::new(|| js_re(r"^\(?[^()]*\)?[@]*=>"));
static HANDLER_BODY: LazyLock<Regex> = LazyLock::new(|| js_re(r"^\(?[^()]*\)?[@]*=>[@]*"));
static STEP_CALL: LazyLock<Regex> = LazyLock::new(|| js_re(r"^[@]*([A-Za-z_][A-Za-z0-9_]*)[@]*\(([\s\S]*)\)[@]*$"));
static AGENT_TOOL: LazyLock<Regex> = LazyLock::new(|| js_re(r"^mcp__([^\n\r\x{2028}\x{2029}]+?)__([^\n\r\x{2028}\x{2029}]+)$"));
static SECONDS_BRACED: LazyLock<Regex> = LazyLock::new(|| js_re(r"^[@]*[0-9]+(\.[0-9]+)?[@]*$"));
static SECONDS_TEXT: LazyLock<Regex> = LazyLock::new(|| js_re(r"^([0-9]+)[@]*(s|m|h)?$"));
static LITERAL_SCALAR: LazyLock<Regex> = LazyLock::new(|| js_re(r"^(-?[0-9]+(?:\.[0-9]+)?(?:[eE]-?[0-9]+)?|true|false|null)"));
static OBJECT_PART: LazyLock<Regex> = LazyLock::new(|| js_re(r#"^[@]*(?:([A-Za-z_][A-Za-z0-9_]*)|"([^"]*)"|'([^']*)')[@]*:[@]*([\s\S]+)$"#));
static RUN_SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[ \t]+").expect("a pattern"));
static RUN_NEWLINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r" ?\n ?").expect("a pattern"));
static RUN_BLANKS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n{3,}").expect("a pattern"));

struct Fatal {
    code: &'static str,
    message: String,
    line: i64,
    fix: Option<String>,
}

fn fatal(code: &'static str, message: String, line: i64) -> Fatal {
    Fatal { code, message, line, fix: None }
}

#[derive(Clone, Copy, Default)]
struct Ext<'a> {
    piece: Option<&'a str>,
    prop: Option<&'a str>,
    fix: Option<&'a str>,
}

fn problem(code: &str, message: &str, line: i64, ext: &Ext) -> SlateProblem {
    let mut p = slate_problem(code, message, ext.fix);
    p.piece = ext.piece.map(str::to_string);
    p.prop = ext.prop.map(str::to_string);
    p.line = Some(line);
    p
}

fn fatal_problem(f: &Fatal) -> SlateProblem {
    problem(f.code, &f.message, f.line, &Ext { fix: f.fix.as_deref(), ..Ext::default() })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Bare,
    Str,
    Braced,
}

#[derive(Clone)]
struct Attr {
    name: String,
    kind: Kind,
    value: String,
    line: i64,
}

#[derive(Clone)]
enum Part {
    Text(String),
    Hole(String),
}

#[derive(Clone)]
struct El {
    tag: String,
    attrs: Vec<Attr>,
    children: Vec<El>,
    text: Option<Vec<Part>>,
    raw: Option<String>,
    line: i64,
}

fn attr_of<'a>(el: &'a El, name: &str) -> Option<&'a Attr> {
    el.attrs.iter().find(|a| a.name == name)
}

fn is_decl_tag(tag: &str) -> bool {
    DECLARATIONS.contains(&tag)
}

fn is_file_decl(tag: &str, attrs: &[Attr]) -> bool {
    tag == "file" && attrs.iter().any(|a| a.name == "name")
}

fn is_file_decl_el(el: &El) -> bool {
    is_file_decl(&el.tag, &el.attrs)
}

fn is_decl_el(el: &El) -> bool {
    is_decl_tag(&el.tag) || is_file_decl_el(el)
}

fn is_on_attr(name: &str) -> bool {
    name.len() > 2 && name.starts_with("on") && name.as_bytes()[2].is_ascii_uppercase()
}

fn tag_len(s: &str) -> usize {
    let b = s.as_bytes();
    if !b.first().is_some_and(u8::is_ascii_alphabetic) {
        return 0;
    }
    b.iter().take_while(|c| c.is_ascii_alphanumeric() || **c == b'-').count()
}

fn attr_len(s: &str) -> usize {
    let b = s.as_bytes();
    if !b.first().is_some_and(u8::is_ascii_alphabetic) {
        return 0;
    }
    b.iter().take_while(|c| c.is_ascii_alphanumeric() || **c == b'_' || **c == b'-').count()
}

fn closing_tag(s: &str) -> Option<(&str, usize)> {
    let body = s.strip_prefix("</")?;
    let n = tag_len(body);
    if n == 0 {
        return None;
    }
    let after = body[n..].trim_start_matches(is_js_whitespace).strip_prefix('>')?;
    Some((&body[..n], s.len() - after.len()))
}

fn byte_at_utf16(src: &str, target: usize) -> usize {
    let mut units = 0;
    for (at, c) in src.char_indices() {
        if units >= target {
            return at;
        }
        units += c.len_utf16();
    }
    src.len()
}

struct Reader<'a> {
    src: &'a str,
    i: usize,
    newlines: Vec<usize>,
}

impl<'a> Reader<'a> {
    fn new(src: &'a str) -> Self {
        Reader { src, i: 0, newlines: src.match_indices('\n').map(|(at, _)| at).collect() }
    }

    fn line(&self, at: usize) -> i64 {
        (self.newlines.partition_point(|&p| p < at) + 1) as i64
    }

    fn here(&self) -> i64 {
        self.line(self.i)
    }

    fn rest(&self) -> &'a str {
        &self.src[self.i..]
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn starts(&self, s: &str) -> bool {
        self.rest().starts_with(s)
    }

    fn fail<T>(&self, message: String, fix: Option<String>) -> Result<T, Fatal> {
        Err(Fatal { code: "P100", message, line: self.here(), fix })
    }

    fn ws(&mut self) {
        while let Some(c) = self.peek() {
            if !is_js_whitespace(c) {
                break;
            }
            self.i += c.len_utf8();
        }
    }

    fn elements(&mut self) -> Result<Vec<El>, Fatal> {
        let most = SLATE_LIMITS.document_bytes + SLATE_LIMITS.files_bytes;
        if utf16::len(self.src) > most || slate_bytes(self.src) > most {
            return Err(fatal("D208", format!("the text is over {} KB, the most a slate and its files hold; write less, or patch it in parts", most / 1024), 1));
        }
        let mut out = Vec::new();
        loop {
            self.ws();
            self.comments()?;
            self.ws();
            if self.i >= self.src.len() {
                break;
            }
            if self.peek() != Some('<') && out.is_empty() && matches!(self.peek(), Some('{') | Some('[')) {
                return self.fail(
                    "this is JSON, and a slate is JSX-like text that starts with <slate>; slate_catalog shows it".into(),
                    Some(r#"<slate title="Gold"><number label="Spot" value={$spot.json.usd} unit="USD" /></slate>"#.into()),
                );
            }
            if self.peek() != Some('<') {
                return self.fail(format!("expected an element at line {}, found text", self.here()), None);
            }
            out.push(self.element(1)?);
        }
        Ok(out)
    }

    fn comments(&mut self) -> Result<(), Fatal> {
        loop {
            if self.starts("<!--") {
                let Some(end) = self.rest().find("-->") else {
                    return self.fail("a comment never closes".into(), None);
                };
                self.i += end + 3;
            } else if self.starts("{/*") {
                let Some(end) = self.rest().find("*/}") else {
                    return self.fail("a comment never closes".into(), None);
                };
                self.i += end + 3;
            } else {
                break;
            }
            self.ws();
        }
        Ok(())
    }

    fn element(&mut self, depth: usize) -> Result<El, Fatal> {
        let line = self.here();
        if depth > READ_DEPTH {
            return Err(fatal("D206", format!("elements nest more than {} deep at line {line}; flatten the layout", SLATE_LIMITS.depth), line));
        }
        self.i += 1;
        let len = tag_len(self.rest());
        if len == 0 {
            return self.fail(format!("cannot read a tag name at line {line}"), None);
        }
        let name = self.rest()[..len].to_string();
        self.i += len;
        let mut attrs: Vec<Attr> = Vec::new();
        loop {
            self.ws();
            if self.starts("/>") {
                self.i += 2;
                return Ok(El { tag: name, attrs, children: Vec::new(), text: None, raw: None, line });
            }
            if self.peek() == Some('>') {
                self.i += 1;
                break;
            }
            if self.i >= self.src.len() {
                return self.fail(format!("<{name}> at line {line} is not closed; write <{name} ... /> or </{name}>"), None);
            }
            let alen = attr_len(self.rest());
            if alen == 0 {
                return self.fail(format!("cannot read an attribute of <{name}> at line {}; write <{name} prop=\"literal\" prop={{formula}} />", self.here()), None);
            }
            let aname = self.rest()[..alen].to_string();
            let aline = self.here();
            self.i += alen;
            self.ws();
            if self.peek() != Some('=') {
                attrs.push(Attr { name: aname, kind: Kind::Bare, value: String::new(), line: aline });
                continue;
            }
            self.i += 1;
            self.ws();
            match self.peek() {
                Some(q @ ('"' | '\'')) => {
                    let body = &self.src[self.i + 1..];
                    let Some(end_rel) = body.find(q) else {
                        return self.fail(format!("the string for {aname} at line {aline} never closes"), None);
                    };
                    let raw = &body[..end_rel];
                    if raw.ends_with('\\') {
                        let fix = if q == '"' {
                            format!(r#"{aname}='echo "hi"', single quotes outside the double ones; with both quotes inside, the block form <run name="x">{{`...`}}</run>"#)
                        } else {
                            format!(r#"{aname}="echo 'hi'", double quotes outside the single ones; with both quotes inside, the block form <run name="x">{{`...`}}</run>"#)
                        };
                        return self.fail(format!("attribute strings take no escapes ({aname} at line {aline}): a backslash does not hide a {q} inside {q}...{q}"), Some(fix));
                    }
                    if let Some(entity) = HTML_ENTITY.find(raw) {
                        let quote = ENTITY_QUOTE.is_match(entity.as_str());
                        let fix = if quote {
                            format!("{aname}='python3 \"$SLATE_DIR/x.py\"', single quotes outside the double ones")
                        } else {
                            format!("write the character itself: {aname}=\"... & ...\"")
                        };
                        return self.fail(
                            format!("attribute strings take no HTML entities ({aname} at line {aline}): {} stays as written, nothing decodes it", entity.as_str()),
                            Some(fix),
                        );
                    }
                    let value = raw.to_string();
                    self.i += end_rel + 2;
                    attrs.push(Attr { name: aname, kind: Kind::Str, value, line: aline });
                    continue;
                }
                Some('{') => {
                    let end = self.balanced(self.i)?;
                    let value = self.src[self.i + 1..end].to_string();
                    attrs.push(Attr { name: aname, kind: Kind::Braced, value, line: aline });
                    self.i = end + 1;
                    continue;
                }
                Some('`') => {
                    return self.fail(format!("a template for {aname} goes inside braces: {aname}={{`...`}}"), None);
                }
                _ => {
                    return self.fail(format!("cannot read the value of {aname} at line {aline}"), None);
                }
            }
        }
        let raw_piece = slate_piece_module(&name).is_some_and(|m| m.raw_text);
        if raw_piece || is_file_decl(&name, &attrs) {
            let close = format!("</{name}>");
            let Some(end) = self.rest().find(&close).map(|p| self.i + p) else {
                return Err(fatal("P100", format!("<{name}> at line {line} is not closed; write </{name}>"), line));
            };
            let raw = dedent(&self.src[self.i..end]);
            self.i = end + close.len();
            return Ok(El { tag: name, attrs, children: Vec::new(), text: None, raw: (!raw.is_empty()).then_some(raw), line });
        }
        let mut children = Vec::new();
        let mut text: Vec<Part> = Vec::new();
        loop {
            if self.i >= self.src.len() {
                return self.fail(format!("<{name}> at line {line} is not closed; write <{name} ... /> or </{name}>"), None);
            }
            if self.starts("</") {
                let Some((closing, consumed)) = closing_tag(self.rest()) else {
                    return self.fail(format!("cannot read the closing tag at line {}", self.here()), None);
                };
                if closing != name {
                    return self.fail(
                        format!("</{closing}> at line {} closes <{name}> opened at line {line}; expected </{name}>", self.here()),
                        Some(format!("</{name}>")),
                    );
                }
                self.i += consumed;
                break;
            }
            if self.starts("<!--") || self.starts("{/*") {
                self.comments()?;
                continue;
            }
            if self.peek() == Some('<') {
                children.push(self.element(depth + 1)?);
                continue;
            }
            if self.peek() == Some('{') {
                let end = self.balanced(self.i)?;
                text.push(Part::Hole(self.src[self.i + 1..end].to_string()));
                self.i = end + 1;
                continue;
            }
            let rest = self.rest();
            let j = rest.find(['<', '{']).unwrap_or(rest.len());
            text.push(Part::Text(rest[..j].to_string()));
            self.i += j;
        }
        let meaningful = text.iter().any(|t| match t {
            Part::Hole(_) => true,
            Part::Text(s) => !js_trim(s).is_empty(),
        });
        Ok(El { tag: name, attrs, children, text: meaningful.then_some(text), raw: None, line })
    }

    fn balanced(&self, i: usize) -> Result<usize, Fatal> {
        let start = utf16::len(&self.src[..i + 1]);
        match closing_brace(self.src, start) {
            Some(end) => Ok(byte_at_utf16(self.src, end)),
            None => Err(fatal("P100", format!("a {{ at line {} never closes", self.line(i)), self.line(i))),
        }
    }
}

fn dedent(text: &str) -> String {
    let text = text.replace("\r\n", "\n");
    let mut lines: Vec<&str> = text.split('\n').collect();
    while lines.first().is_some_and(|l| js_trim(l).is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|l| js_trim(l).is_empty()) {
        lines.pop();
    }
    let indent = lines
        .iter()
        .filter(|l| !js_trim(l).is_empty())
        .map(|l| l.chars().take_while(|c| is_js_whitespace(*c)).count())
        .min()
        .unwrap_or(0);
    lines.iter().map(|l| l.chars().skip(indent).collect::<String>()).collect::<Vec<_>>().join("\n")
}

fn normalize_run(s: &str) -> String {
    let s = RUN_SPACES.replace_all(s, " ");
    let s = RUN_NEWLINE.replace_all(&s, "\n");
    RUN_BLANKS.replace_all(&s, "\n\n").into_owned()
}

fn slate_text_child_form(s: &str) -> String {
    js_trim(&normalize_run(s)).to_string()
}

fn js_replace_units(s: &str, keep: impl Fn(char) -> bool, with: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if keep(c) {
            out.push(c);
        } else {
            for _ in 0..c.len_utf16() {
                out.push_str(with);
            }
        }
    }
    out
}

fn split_top(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut depth: i64 = 0;
    let mut quote: Option<char> = None;
    let mut cur = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if let Some(q) = quote {
            cur.push(c);
            if c == '\\' {
                i += 1;
                if let Some(next) = chars.get(i) {
                    cur.push(*next);
                }
            } else if c == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        if c == '\'' || c == '"' || c == '`' {
            quote = Some(c);
            cur.push(c);
            i += 1;
            continue;
        }
        if "{[(".contains(c) {
            depth += 1;
        }
        if "}])".contains(c) {
            depth -= 1;
        }
        if c == ',' && depth == 0 {
            out.push(std::mem::take(&mut cur));
            i += 1;
            continue;
        }
        cur.push(c);
        i += 1;
    }
    if !js_trim(&cur).is_empty() {
        out.push(cur);
    }
    out
}

fn js_index(map: IndexMap<String, Value>) -> IndexMap<String, Value> {
    js_key_order(map.iter()).into_iter().map(|(k, v)| (k.clone(), v.clone())).collect()
}

fn record(map: IndexMap<String, Value>) -> Value {
    Value::Object(js_index(map).into_iter().collect())
}

fn record_value(map: &IndexMap<String, Value>) -> Value {
    Value::Object(map.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
}

fn jnum(x: f64) -> Value {
    if x.fract() == 0.0 && x.abs() < 9.0e15 {
        Value::from(x as i64)
    } else {
        Value::from(x)
    }
}

struct Literal {
    t: Vec<u16>,
    i: usize,
}

impl Literal {
    fn at(&self, i: usize) -> Option<char> {
        self.t.get(i).and_then(|&u| char::from_u32(u as u32))
    }

    fn text(&self, from: usize, n: usize) -> String {
        let start = from.min(self.t.len());
        let end = (from + n).min(self.t.len());
        String::from_utf16_lossy(&self.t[start..end])
    }

    fn ws(&mut self) {
        while self.at(self.i).is_some_and(is_js_whitespace) {
            self.i += 1;
        }
    }

    fn find(&self, q: char, from: usize) -> Option<usize> {
        self.t.get(from..)?.iter().position(|&u| u == q as u16).map(|p| p + from)
    }

    fn value(&mut self) -> Result<Value, String> {
        self.ws();
        match self.at(self.i) {
            Some('{') => self.object(),
            Some('[') => self.list(),
            Some(q @ ('"' | '\'')) => self.string(q),
            _ => self.scalar(),
        }
    }

    fn object(&mut self) -> Result<Value, String> {
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

    fn list(&mut self) -> Result<Value, String> {
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

    fn string(&mut self, q: char) -> Result<Value, String> {
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

fn parse_literal(text: &str) -> Result<Value, String> {
    let mut p = Literal { t: text.encode_utf16().collect(), i: 0 };
    let v = p.value()?;
    p.ws();
    if p.i < p.t.len() {
        return Err(format!("unexpected \"{}\"", p.text(p.i, 16)));
    }
    Ok(v)
}

fn seconds(a: &Attr) -> f64 {
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

fn to_number_text(text: &str) -> f64 {
    wsp_words::js::to_number(text)
}

fn value_text(text: &str) -> Value {
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

fn string_literal(text: &str) -> Option<String> {
    match parse_slate_expression(js_trim(text)).ast {
        Some(SlateExpr::Lit { v: Value::String(s), .. }) => Some(s),
        _ => None,
    }
}

fn path_of(text: &str) -> Option<String> {
    match parse_slate_expression(js_trim(text)).ast {
        Some(SlateExpr::Path { head, own, segs, .. }) => Some(slate_path_text(&head, own, &segs)),
        _ => None,
    }
}

fn command_text(braced: &str) -> Option<String> {
    let t = js_trim(braced);
    if t.len() >= 2 && t.starts_with('`') && t.ends_with('`') && !t[1..t.len() - 1].contains('`') {
        return if COMMAND_HOLE.is_match(t) { None } else { Some(dedent(&t[1..t.len() - 1])) };
    }
    string_literal(t)
}

fn command_body(text: &[Part]) -> Option<String> {
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

fn explicit_ids(el: &El) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(id) = attr_of(el, "id") {
        if id.kind == Kind::Str {
            out.push(id.value.clone());
        }
    }
    for ch in &el.children {
        out.extend(explicit_ids(ch));
    }
    out
}

fn nested_declarations(el: &mut El) -> Vec<El> {
    let mut found = Vec::new();
    let mut keep = Vec::new();
    for mut ch in std::mem::take(&mut el.children) {
        if is_decl_el(&ch) {
            found.push(ch);
        } else {
            found.extend(nested_declarations(&mut ch));
            keep.push(ch);
        }
    }
    el.children = keep;
    found
}

fn lift_declarations(children: Vec<El>) -> Vec<El> {
    let mut rest = Vec::new();
    let mut found = Vec::new();
    for mut ch in children {
        if is_decl_el(&ch) {
            found.push(ch);
        } else {
            found.extend(nested_declarations(&mut ch));
            rest.push(ch);
        }
    }
    rest.extend(found);
    rest
}

fn owner_of(spec: &SlatePieceModule, name: &str) -> Option<String> {
    spec.props
        .iter()
        .find(|(_, p)| matches!(p.prop_type, SlatePropType::Enum(list) if list.contains(&name)))
        .map(|(key, _)| key.to_string())
}

fn strs(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| w.to_string()).collect()
}

fn step_names() -> Vec<String> {
    SLATE_STEPS.iter().map(|s| s.kind.to_string()).collect()
}

fn piece_names() -> Vec<String> {
    SLATE_PIECES.keys().map(|k| k.to_string()).collect()
}

fn format_text(v: &Value) -> Option<&str> {
    let m = v.as_object()?;
    if m.len() != 1 {
        return None;
    }
    m.get("format")?.as_str()
}

fn is_piece_base(ty: &str) -> bool {
    let mut chars = ty.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase()) && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn json_text(v: Option<&Value>) -> String {
    match v {
        None => "\u{0}undefined".to_string(),
        Some(v) => json_stringify(v),
    }
}

fn sub_map<'a>(op: &'a mut Map<String, Value>, key: &str) -> &'a mut Map<String, Value> {
    op.entry(key.to_string()).or_insert_with(|| Value::Object(Map::new())).as_object_mut().expect("a sub object")
}

fn piece_value(p: &SlatePiece) -> Value {
    let mut m = Map::new();
    m.insert("type".into(), Value::String(p.kind.clone()));
    if let Some(when) = &p.when {
        m.insert("when".into(), Value::String(when.clone()));
    }
    if let Some(props) = &p.props {
        m.insert("props".into(), record_value(props));
    }
    if let Some(children) = &p.children {
        m.insert("children".into(), json!(children));
    }
    if let Some(on) = &p.on {
        m.insert("on".into(), on.clone());
    }
    if let Some(fallback) = &p.fallback {
        m.insert("fallback".into(), fallback.clone());
    }
    Value::Object(m)
}

fn json_key(k: &str) -> String {
    if IDENT.is_match(k) {
        k.to_string()
    } else {
        json_stringify(&Value::String(k.to_string()))
    }
}

#[derive(Default)]
struct Common {
    every: Option<JsNumber>,
    once: Option<bool>,
    always: Option<bool>,
    confirm: Option<Value>,
    then: Option<String>,
}

#[derive(Default)]
struct PieceState {
    when: Option<String>,
    fallback: Option<Value>,
    on: Option<Map<String, Value>>,
    props: IndexMap<String, Value>,
}

pub struct SlateCompiled {
    pub document: Option<SlateDoc>,
    pub lines: SlateLines,
    pub errors: Vec<SlateProblem>,
}

fn empty_doc() -> SlateDoc {
    SlateDoc {
        schema: SLATE_SCHEMA,
        kit: None,
        title: None,
        root: String::new(),
        values: IndexMap::new(),
        derived: IndexMap::new(),
        runs: IndexMap::new(),
        reactions: Vec::new(),
        pieces: IndexMap::new(),
        files: None,
    }
}

struct Compiler {
    errors: Vec<SlateProblem>,
    lines: SlateLines,
    doc: SlateDoc,
    minted: HashMap<String, usize>,
    taken: HashSet<String>,
    names: IndexMap<String, String>,
    patch: bool,
}

impl Compiler {
    fn new(current: Option<&SlateDoc>) -> Compiler {
        let mut c = Compiler {
            errors: Vec::new(),
            lines: IndexMap::new(),
            doc: empty_doc(),
            minted: HashMap::new(),
            taken: HashSet::new(),
            names: IndexMap::new(),
            patch: false,
        };
        if let Some(cur) = current {
            for id in cur.pieces.keys() {
                c.taken.insert(id.clone());
            }
            for (n, v) in &cur.values {
                c.names.insert(n.clone(), if v.secret == Some(true) { "secret" } else { "value" }.to_string());
            }
            for n in cur.derived.keys() {
                c.names.insert(n.clone(), "derived".to_string());
            }
            for n in cur.runs.keys() {
                c.names.insert(n.clone(), "run".to_string());
            }
        }
        c
    }

    fn error(&mut self, code: &str, message: &str, line: i64, ext: Ext) {
        if self.errors.len() < SLATE_LIMITS.errors_per_pass {
            self.errors.push(problem(code, message, line, &ext));
        }
    }

    fn report(&mut self, f: Fatal) {
        self.error(f.code, &f.message, f.line, Ext { fix: f.fix.as_deref(), ..Ext::default() });
    }

    fn document(&mut self, el: El) {
        for a in &el.attrs {
            if a.name == "title" && a.kind == Kind::Str {
                self.doc.title = Some(a.value.clone());
            } else if a.name == "title" && a.kind == Kind::Braced && string_literal(&a.value).is_some() {
                self.doc.title = string_literal(&a.value);
            } else if a.name == "title" {
                self.error("T303", "title is a literal string: <slate title=\"...\">", a.line, Ext::default());
            } else {
                self.error("T302", &format!("<slate> takes title alone, not {}", a.name), a.line, Ext { fix: Some("title"), ..Ext::default() });
            }
        }
        if el.text.is_some() {
            self.error("P100", "<slate> takes elements, not text; put the words in a <text>", el.line, Ext::default());
        }
        let mut decls = Vec::new();
        let mut files = Vec::new();
        let mut pieces = Vec::new();
        for ch in lift_declarations(el.children) {
            if is_decl_tag(&ch.tag) {
                decls.push(ch);
            } else if is_file_decl_el(&ch) {
                files.push(ch);
            } else {
                pieces.push(ch);
            }
        }
        for d in &decls {
            if let Err(f) = self.declare_name(d) {
                self.report(f);
            }
        }
        for f in &files {
            self.file(f);
        }
        for p in &pieces {
            for id in explicit_ids(p) {
                self.taken.insert(id);
            }
        }
        for d in &decls {
            self.declaration(d);
        }
        if pieces.len() != 1 {
            let names: Vec<String> = pieces.iter().map(|p| attr_of(p, "id").map_or(p.tag.clone(), |a| a.value.clone())).collect();
            let message = if pieces.is_empty() {
                "there is no piece; write one, like <column>".to_string()
            } else {
                format!("{} pieces at the top ({}); wrap them in a <column>", pieces.len(), names.join(", "))
            };
            let line = pieces.get(1).map_or(el.line, |p| p.line);
            self.error("D202", &message, line, Ext { fix: Some("<column>...</column>"), ..Ext::default() });
        }
        let mut pieces = pieces.into_iter();
        if let Some(first) = pieces.next() {
            let root = self.piece(first).unwrap_or_default();
            self.doc.root = root;
        }
        for extra in pieces {
            self.piece(extra);
        }
    }

    fn declare_name(&mut self, el: &El) -> Result<(), Fatal> {
        if el.tag == "when" {
            return Ok(());
        }
        let name = match attr_of(el, "name") {
            Some(a) if a.kind == Kind::Str => a,
            _ => return Err(fatal("P103", format!("<{}> needs name=\"...\"", el.tag), el.line)),
        };
        if !SLATE_NAME.is_match(&name.value) {
            let stripped = name.value.trim_start_matches(|c: char| !(c.is_ascii_lowercase() || c == '_'));
            let fix = js_replace_units(stripped, |c| c.is_ascii_alphanumeric() || c == '_', "_");
            return Err(Fatal {
                code: "P103",
                message: format!("\"{}\" is not a name: a lowercase letter or _, then letters, digits and _", name.value),
                line: name.line,
                fix: (!fix.is_empty()).then_some(fix),
            });
        }
        if self.names.contains_key(&name.value) && !self.patch {
            return Err(fatal("P104", format!("${} is declared twice", name.value), el.line));
        }
        self.names.insert(name.value.clone(), el.tag.clone());
        Ok(())
    }

    fn declaration(&mut self, el: &El) {
        let allowed: &[&str] = match el.tag.as_str() {
            "value" => &["name", "start"],
            "secret" => &["name", "keep"],
            "derived" => &["name", "value"],
            "run" => &[
                "name", "cmd", "tool", "resource", "env", "args", "stdin", "on", "cwd", "timeout", "stream", "confirm", "every", "once",
                "always", "then", "description",
            ],
            "when" => &["id", "change", "done", "do"],
            _ => &[],
        };
        for a in &el.attrs {
            if !allowed.contains(&a.name.as_str()) {
                let fix = slate_nearest(&a.name, &strs(allowed));
                self.error(
                    "T302",
                    &format!("<{}> has no attribute {}; it takes {}", el.tag, a.name, allowed.join(", ")),
                    a.line,
                    Ext { fix: fix.as_deref(), ..Ext::default() },
                );
            }
        }
        let is_when = el.tag == "when";
        let has_content = el.text.is_some() || !el.children.is_empty();
        let when_ok = is_when && el.attrs.iter().all(|a| a.name != "do");
        let run_ok = el.tag == "run" && el.children.is_empty();
        if has_content && !when_ok && !run_ok {
            let hint = if is_when { "when takes do={steps}".to_string() } else { format!("<{}> is self-closing and takes no children", el.tag) };
            let fix = is_when.then_some("<when change={$id} do={start($check)} />");
            self.error("A601", &hint, el.line, Ext { fix, ..Ext::default() });
        }
        let name = attr_of(el, "name").map_or(String::new(), |a| a.value.clone());
        let key = if is_when { String::new() } else { format!("${name}") };
        if !is_when {
            self.lines.insert(key.clone(), el.line);
        }
        match el.tag.as_str() {
            "value" => {
                let start = match attr_of(el, "start") {
                    Some(a) => self.literal(a, &key),
                    None => Value::Null,
                };
                self.doc.values.insert(name, SlateValueDecl { start, secret: None, keep: None });
            }
            "secret" => {
                let keep = attr_of(el, "keep").is_some();
                self.doc.values.insert(name, SlateValueDecl { start: Value::Null, secret: Some(true), keep: keep.then_some(true) });
            }
            "derived" => match attr_of(el, "value") {
                Some(v) if v.kind == Kind::Braced => {
                    self.doc.derived.insert(name, js_trim(&v.value).to_string());
                    self.lines.insert(format!("{key}.value"), v.line);
                }
                _ => self.error(
                    "T304",
                    &format!("<derived name=\"{name}\"> needs value={{formula}}"),
                    el.line,
                    Ext { piece: Some(&key), prop: Some("value"), ..Ext::default() },
                ),
            },
            "run" => self.run(el, &name),
            "when" => self.reaction(el),
            _ => {}
        }
    }

    fn file(&mut self, el: &El) -> Option<String> {
        for a in &el.attrs {
            if a.name != "name" {
                self.error("T302", &format!("<file name=\"...\"> takes name alone, not {}", a.name), a.line, Ext::default());
            }
        }
        let name = match attr_of(el, "name") {
            Some(a) if a.kind == Kind::Str => a.value.clone(),
            _ => {
                self.error("K708", "<file> needs name=\"...\", a plain file name like name=\"shape.py\"", el.line, Ext::default());
                return None;
            }
        };
        if self.doc.files.as_ref().is_some_and(|f| f.contains_key(&name)) {
            self.error("P104", &format!("the file {name} is declared twice"), el.line, Ext { piece: Some(&name), ..Ext::default() });
        }
        let raw = el.raw.clone().unwrap_or_default();
        let text = match FILE_BRACED.captures(&raw) {
            None => raw,
            Some(c) => dedent(&c[1]),
        };
        if js_trim(&text).is_empty() {
            self.error(
                "K700",
                &format!("<file name=\"{name}\"> is empty; its text goes inside: <file name=\"{name}\">{{`...`}}</file>"),
                el.line,
                Ext { piece: Some(&name), ..Ext::default() },
            );
        }
        self.doc.files.get_or_insert_with(IndexMap::new).insert(name.clone(), text);
        self.lines.insert(name.clone(), el.line);
        Some(name)
    }

    fn run(&mut self, el: &El, name: &str) {
        let key = format!("${name}");
        for a in &el.attrs {
            self.lines.insert(format!("{key}.{}", a.name), a.line);
        }
        let attr = |n: &str| attr_of(el, n);
        let kinds: Vec<&str> = ["cmd", "tool", "resource"].into_iter().filter(|k| attr(k).is_some() || (*k == "cmd" && el.text.is_some())).collect();
        if attr("cmd").is_some() && el.text.is_some() {
            self.error(
                "K701",
                &format!("<run name=\"{name}\"> has cmd= and a command inside; keep one"),
                el.line,
                Ext { piece: Some(&key), ..Ext::default() },
            );
            return;
        }
        if kinds.len() != 1 {
            self.error(
                "K701",
                &format!("<run name=\"{name}\"> takes exactly one of cmd, tool or resource"),
                el.line,
                Ext { piece: Some(&key), ..Ext::default() },
            );
            return;
        }
        let mut common = Common::default();
        if let Some(every) = attr("every") {
            let s = seconds(every);
            if s.is_finite() {
                common.every = Some(JsNumber(s));
            } else {
                self.error(
                    "K703",
                    "every takes seconds: every=\"30s\" or every={30}",
                    every.line,
                    Ext { piece: Some(&key), prop: Some("every"), ..Ext::default() },
                );
            }
        }
        if attr("once").is_some() {
            common.once = Some(true);
        }
        if attr("always").is_some() {
            common.always = Some(true);
        }
        if let Some(c) = attr("confirm") {
            let v = if c.kind == Kind::Braced { value_text(&c.value) } else { Value::String(c.value.clone()) };
            if v.is_string() || is_slate_binding(&v) || format_text(&v).is_some() {
                common.confirm = Some(v);
            } else {
                self.error(
                    "T303",
                    "confirm is a sentence: confirm=\"Stop it?\" or confirm={`Kill ${$name}?`}",
                    c.line,
                    Ext { piece: Some(&key), prop: Some("confirm"), ..Ext::default() },
                );
            }
        }
        if let Some(t) = attr("then") {
            let text = match t.kind {
                Kind::Str => Some(t.value.clone()),
                Kind::Braced => command_text(&t.value),
                Kind::Bare => None,
            };
            match text {
                Some(s) if !js_trim(&s).is_empty() => common.then = Some(s),
                _ => self.error(
                    "K700",
                    "then is a literal command that reads the result on stdin: then='python3 shape.py'",
                    t.line,
                    Ext { piece: Some(&key), prop: Some("then"), ..Ext::default() },
                ),
            }
        }
        let kind = kinds[0];
        let mut description: Option<String> = None;
        if let Some(d) = attr("description") {
            if kind == "cmd" {
                self.error(
                    "K701",
                    "a cmd run takes no description: its sheet shows the command itself",
                    d.line,
                    Ext { piece: Some(&key), prop: Some("description"), ..Ext::default() },
                );
            } else if d.kind != Kind::Str || js_trim(&d.value).is_empty() {
                self.error(
                    "T303",
                    "description is one literal line: description=\"Reads the last hour of traffic\"",
                    d.line,
                    Ext { piece: Some(&key), prop: Some("description"), ..Ext::default() },
                );
            } else {
                description = Some(js_trim(&d.value).to_string());
            }
        }
        let decl = match kind {
            "cmd" => {
                let cmd = attr("cmd");
                let literal = match cmd {
                    None => command_body(el.text.as_deref().unwrap_or(&[])),
                    Some(c) if c.kind == Kind::Str => Some(c.value.clone()),
                    Some(c) if c.kind == Kind::Braced => command_text(&c.value),
                    Some(_) => None,
                };
                if literal.is_none() {
                    self.error(
                        "K700",
                        "the command is literal text: cmd=\"...\", cmd={\"...\"} with \\\" escapes, or <run name=\"x\">{`any text`}</run>; values reach it as env, args or stdin",
                        cmd.map_or(el.line, |c| c.line),
                        Ext {
                            piece: Some(&key),
                            prop: Some("cmd"),
                            fix: Some("cmd='gh api \"$URL\"' env={{ URL: $url }}"),
                        },
                    );
                }
                let mut r = SlateCmdDecl {
                    cmd: literal.unwrap_or_default(),
                    env: None,
                    args: None,
                    stdin: None,
                    on: None,
                    cwd: None,
                    timeout: None,
                    stream: None,
                    confirm: common.confirm.take(),
                    every: common.every.take(),
                    once: common.once.take(),
                    always: common.always.take(),
                    then: common.then.take(),
                };
                if let Some(env) = attr("env") {
                    r.env = self.object(env, &key);
                }
                if let Some(args) = attr("args") {
                    r.args = self.array(args, &key);
                }
                if let Some(stdin) = attr("stdin") {
                    r.stdin = Some(self.value(stdin));
                }
                if let Some(on) = attr("on") {
                    if on.kind != Kind::Str || (on.value != "thread" && on.value != "host") {
                        self.error("K704", "on is \"thread\" or \"host\"", on.line, Ext { piece: Some(&key), prop: Some("on"), ..Ext::default() });
                    } else {
                        r.on = Some(on.value.clone());
                    }
                }
                if let Some(cwd) = attr("cwd") {
                    if cwd.kind != Kind::Str {
                        self.error("T303", "cwd is a literal path", cwd.line, Ext { piece: Some(&key), prop: Some("cwd"), ..Ext::default() });
                    } else {
                        r.cwd = Some(cwd.value.clone());
                    }
                }
                if let Some(timeout) = attr("timeout") {
                    let s = seconds(timeout);
                    if s.is_finite() {
                        r.timeout = Some(JsNumber(s));
                    } else {
                        self.error("K704", "timeout is seconds: timeout={20}", timeout.line, Ext { piece: Some(&key), prop: Some("timeout"), ..Ext::default() });
                    }
                }
                if attr("stream").is_some() {
                    r.stream = Some(true);
                }
                for n in ["tool", "resource"] {
                    if attr(n).is_some() {
                        self.error("K701", &format!("a cmd run takes no {n}"), el.line, Ext { piece: Some(&key), ..Ext::default() });
                    }
                }
                Some(SlateRunDecl::Cmd(r))
            }
            "tool" => {
                let Some(named) = attr("tool") else { return };
                let tool_value = match AGENT_TOOL.captures(&named.value) {
                    Some(c) if named.kind == Kind::Str => format!("{}.{}", &c[1], &c[2]),
                    _ => named.value.clone(),
                };
                let dot = tool_value.find('.');
                let bad = named.kind != Kind::Str || dot.map_or(true, |d| d == 0 || d == tool_value.len() - 1);
                if bad {
                    self.error(
                        "K705",
                        "tool names a server and a tool as \"server.tool\"",
                        named.line,
                        Ext { piece: Some(&key), prop: Some("tool"), ..Ext::default() },
                    );
                }
                let (server, tool) = match dot {
                    Some(d) => (tool_value[..d].to_string(), tool_value[d + 1..].to_string()),
                    None => (String::new(), tool_value.clone()),
                };
                let mut r = SlateToolDecl {
                    server,
                    tool,
                    args: None,
                    description: description.take(),
                    confirm: common.confirm.take(),
                    every: common.every.take(),
                    once: common.once.take(),
                    always: common.always.take(),
                    then: common.then.take(),
                };
                if let Some(args) = attr("args") {
                    r.args = self.object(args, &key);
                }
                for n in ["env", "stdin", "on", "cwd", "timeout", "stream"] {
                    if attr(n).is_some() {
                        self.error("K701", &format!("a tool run takes no {n}"), el.line, Ext { piece: Some(&key), ..Ext::default() });
                    }
                }
                Some(SlateRunDecl::Tool(r))
            }
            _ => {
                let Some(res) = attr("resource") else { return };
                let colon = res.value.find(':');
                if res.kind != Kind::Str || colon.map_or(true, |c| c == 0) {
                    self.error(
                        "K705",
                        "resource names a server and a uri as \"server:uri\"",
                        res.line,
                        Ext { piece: Some(&key), prop: Some("resource"), ..Ext::default() },
                    );
                }
                let (server, uri) = match colon {
                    Some(c) => (res.value[..c].to_string(), res.value[c + 1..].to_string()),
                    None => (String::new(), res.value.clone()),
                };
                let r = SlateResourceDecl {
                    server,
                    uri,
                    every: common.every.take(),
                    always: common.always.take(),
                    then: common.then.take(),
                    description: description.take(),
                };
                for n in ["env", "args", "stdin", "on", "cwd", "timeout", "stream", "confirm", "once"] {
                    if attr(n).is_some() {
                        self.error("K701", &format!("a resource run takes no {n}"), el.line, Ext { piece: Some(&key), ..Ext::default() });
                    }
                }
                Some(SlateRunDecl::Resource(r))
            }
        };
        if let Some(decl) = decl {
            self.doc.runs.insert(name.to_string(), decl);
        }
    }

    fn reaction(&mut self, el: &El) {
        let change = attr_of(el, "change");
        let done = attr_of(el, "done");
        let id_str = attr_of(el, "id").filter(|a| a.kind == Kind::Str).map(|a| a.value.clone());
        let key = id_str.clone().unwrap_or_else(|| format!("reaction-{}", self.doc.reactions.len() + 1));
        self.lines.insert(key.clone(), el.line);
        if self.patch && id_str.is_none() {
            self.error(
                "P107",
                "a reaction in a patch carries an id, so a later patch can name it",
                el.line,
                Ext { fix: Some("<when id=\"check-on-id\" ... />"), ..Ext::default() },
            );
        }
        if change.is_none() == done.is_none() {
            self.error("A611", "<when> takes exactly one of change={...} or done={$run}", el.line, Ext { piece: Some(&key), ..Ext::default() });
            return;
        }
        let do_attr = match attr_of(el, "do") {
            Some(a) if a.kind == Kind::Braced => a,
            _ => {
                self.error(
                    "A601",
                    "when takes do={steps}",
                    el.line,
                    Ext { piece: Some(&key), fix: Some("<when change={$id} do={start($check)} />"), ..Ext::default() },
                );
                return;
            }
        };
        let on: Value = match (change, done) {
            (Some(c), _) => {
                if c.kind != Kind::Braced {
                    self.error(
                        "A601",
                        "change takes a path or a list of paths in braces: change={$id}",
                        c.line,
                        Ext { piece: Some(&key), ..Ext::default() },
                    );
                    return;
                }
                let text = js_trim(&c.value);
                let items = if text.starts_with('[') && text.ends_with(']') { split_top(&text[1..text.len() - 1]) } else { vec![text.to_string()] };
                let mut paths: Vec<String> = Vec::new();
                for t in items {
                    match path_of(&t) {
                        Some(p) => paths.push(p),
                        None => self.error(
                            "A601",
                            &format!("change lists paths, not formulas: {}", js_trim(&t)),
                            c.line,
                            Ext { piece: Some(&key), ..Ext::default() },
                        ),
                    }
                }
                json!({"change": paths})
            }
            (None, Some(d)) => match slate_own_name_braced(d) {
                Some(run) => json!({"done": run}),
                None => {
                    self.error("K702", "done names a run: done={$check}", d.line, Ext { piece: Some(&key), ..Ext::default() });
                    return;
                }
            },
            (None, None) => return,
        };
        self.lines.insert(format!("{key}.do"), do_attr.line);
        let steps = self.steps(do_attr, &key);
        let mut r = Map::new();
        if let Some(id) = id_str {
            r.insert("id".into(), Value::String(id));
        }
        r.insert("on".into(), on);
        r.insert("do".into(), Value::Array(steps));
        self.doc.reactions.push(Value::Object(r));
    }

    fn step_bad(&mut self, a: &Attr, piece: &str, kind: &str, message: &str, fix: Option<&str>) {
        self.error("A601", &format!("{kind}: {message}"), a.line, Ext { piece: Some(piece), prop: Some(a.name.as_str()), fix });
    }

    fn steps(&mut self, a: &Attr, piece: &str) -> Vec<Value> {
        let at = Ext { piece: Some(piece), prop: Some(a.name.as_str()), fix: None };
        if a.kind != Kind::Braced {
            self.error("A600", "a handler is a step or a list of steps in braces: onPress={set($x, 1)}", a.line, at);
            return Vec::new();
        }
        let mut text = js_trim(&a.value).to_string();
        if HANDLER_FN.is_match(&text) || text.starts_with("function") {
            let body = HANDLER_BODY.replace(&text, "").into_owned();
            let fix = format!("{}={{{}}}", a.name, body);
            self.error("A600", "a handler is a step or a list of steps, not a function", a.line, Ext { fix: Some(&fix), ..at });
            return Vec::new();
        }
        let list = text.starts_with('[') && text.ends_with(']');
        if list {
            text = text[1..text.len() - 1].to_string();
        }
        let calls = if list { split_top(&text) } else { vec![text.clone()] };
        let names = step_names();
        let mut out = Vec::new();
        for call in calls {
            let Some(m) = STEP_CALL.captures(&call) else {
                self.error("A600", &format!("\"{}\" is not a step; steps are {}", js_trim(&call), names.join(", ")), a.line, at);
                continue;
            };
            let kind = m[1].to_string();
            if slate_step_module(&kind).is_none() {
                let fix = if kind == "setState" {
                    Some("set".to_string())
                } else if kind == "navigate" || kind == "openUrl" {
                    Some("open".to_string())
                } else {
                    slate_nearest(&kind, &names)
                };
                self.error(
                    "A600",
                    &format!("{kind} is not a step; steps are {}", names.join(", ")),
                    a.line,
                    Ext { fix: fix.as_deref(), ..at },
                );
                continue;
            }
            let args: Vec<String> = split_top(&m[2]).iter().map(|s| js_trim(s).to_string()).collect();
            let own = |s: Option<&String>| s.filter(|s| parse_slate_own_path(s).is_some()).cloned();
            match kind.as_str() {
                "set" => match own(args.first()) {
                    Some(path) if args.len() == 2 => out.push(json!({"do": "set", "path": path, "value": value_text(&args[1])})),
                    _ => self.step_bad(a, piece, &kind, "set writes one of the slate's values: set($name, value)", Some("set($step, 2)")),
                },
                "toggle" => match own(args.first()) {
                    Some(path) if args.len() == 1 => out.push(json!({"do": "toggle", "path": path})),
                    _ => self.step_bad(a, piece, &kind, "toggle flips one of the slate's values: toggle($name)", None),
                },
                "start" | "cancel" => match slate_own_name(args.first().map_or("", String::as_str)) {
                    Some(run) if args.len() == 1 => out.push(json!({"do": kind, "run": run})),
                    _ => self.step_bad(a, piece, &kind, &format!("{kind} names a run: {kind}($check)"), None),
                },
                "send" | "steer" | "queue" | "fill" => {
                    let Some(text) = args.first().and_then(|s| string_literal(s)) else {
                        let fix = format!("{kind}(\"What to do.\", $path)");
                        self.error(
                            "A603",
                            &format!("{kind}'s text is a literal string; put the changing parts after it as paths"),
                            a.line,
                            Ext { piece: Some(piece), prop: Some(a.name.as_str()), fix: Some(&fix) },
                        );
                        continue;
                    };
                    let mut paths: Vec<String> = Vec::new();
                    for p in args.iter().skip(1) {
                        match path_of(p) {
                            Some(path) => paths.push(path),
                            None => self.step_bad(a, piece, &kind, &format!("{p} is not a path; {kind} carries paths after its text, as data"), None),
                        }
                    }
                    let mut step = json!({"do": kind, "text": text});
                    if !paths.is_empty() {
                        step["with"] = json!(paths);
                    }
                    out.push(step);
                }
                "open" | "copy" => {
                    if args.len() != 1 {
                        self.step_bad(a, piece, &kind, &format!("{kind} takes one value"), None);
                        continue;
                    }
                    let v = value_text(&args[0]);
                    out.push(if kind == "open" { json!({"do": "open", "target": v}) } else { json!({"do": "copy", "text": v}) });
                }
                "pane" => {
                    let p = args.first().and_then(|s| string_literal(s));
                    match p {
                        Some(p) if SLATE_PANE_KINDS.contains(&p.as_str()) => out.push(json!({"do": "pane", "kind": p})),
                        other => {
                            let fix = slate_nearest(other.as_deref().unwrap_or(""), &strs(SLATE_PANE_KINDS));
                            self.step_bad(
                                a,
                                piece,
                                &kind,
                                &format!("pane names one of {}", SLATE_PANE_KINDS.join(", ")),
                                fix.as_deref(),
                            );
                        }
                    }
                }
                _ => {}
            }
        }
        out
    }

    fn piece(&mut self, mut el: El) -> Option<String> {
        if is_decl_el(&el) {
            self.error("P100", &format!("<{}> is a declaration and goes directly under <slate>", el.tag), el.line, Ext::default());
            return None;
        }
        if SLATE_ITEM_KINDS.contains(&el.tag.as_str()) {
            self.error("T314", &format!("<{}> is an item and goes inside the piece that takes it", el.tag), el.line, Ext::default());
            return None;
        }
        let spec = slate_piece_module(&el.tag);
        if spec.is_some_and(|s| s.holds_children && s.text_prop.is_none()) && el.text.is_some() {
            let text = el.text.take();
            let mut kids = vec![El { tag: "text".into(), attrs: Vec::new(), children: Vec::new(), text, raw: None, line: el.line }];
            kids.append(&mut el.children);
            el.children = kids;
        }
        let id_attr = attr_of(&el, "id").cloned();
        let id = match &id_attr {
            Some(a) if a.kind == Kind::Str && SLATE_ID.is_match(&a.value) => {
                if self.doc.pieces.contains_key(&a.value) {
                    self.error("P104", &format!("{} is used twice", a.value), a.line, Ext { piece: Some(&a.value), ..Ext::default() });
                }
                a.value.clone()
            }
            other => {
                if let Some(a) = other {
                    let replaced = js_replace_units(&a.value, |c| c.is_ascii_alphanumeric() || c == '_' || c == '-', "-");
                    let stripped = replaced.trim_start_matches(|c: char| !c.is_ascii_alphabetic());
                    let fix = (!stripped.is_empty()).then(|| stripped.to_string());
                    self.error(
                        "P103",
                        &format!("\"{}\" is not a piece id: letters, digits, dashes and _, starting with a letter", a.value),
                        a.line,
                        Ext { fix: fix.as_deref(), ..Ext::default() },
                    );
                }
                self.mint(&el.tag)
            }
        };
        self.doc.pieces.insert(id.clone(), SlatePiece { kind: el.tag.clone(), props: None, children: None, when: None, on: None, fallback: None });
        self.lines.insert(id.clone(), el.line);
        let mut st = PieceState::default();
        for a in &el.attrs {
            if a.name == "id" {
                continue;
            }
            self.lines.insert(format!("{id}.{}", a.name), a.line);
            if let Err(f) = self.piece_attr(a, &id, &el.tag, spec, &mut st) {
                self.report(f);
            }
        }
        if let (Some(s), Some(raw)) = (spec, &el.raw) {
            if let Some(tp) = s.text_prop {
                st.props.insert(tp.to_string(), Value::String(raw.clone()));
            }
        }
        if let Some(text) = &el.text {
            match spec {
                None => {
                    let names = piece_names();
                    let unknown = slate_unknown_piece(&el.tag, &names);
                    self.error("T300", &unknown.message, el.line, Ext { piece: Some(&id), fix: unknown.fix.as_deref(), ..Ext::default() });
                }
                Some(s) => match s.text_prop {
                    None => self.error(
                        "P100",
                        &format!("{} takes elements, not text; put the words in a <text>", el.tag),
                        el.line,
                        Ext { piece: Some(&id), ..Ext::default() },
                    ),
                    Some(tp) if st.props.contains_key(tp) => self.error(
                        "T303",
                        &format!("{} has both {tp}= and a text child; use one", el.tag),
                        el.line,
                        Ext { piece: Some(&id), prop: Some(tp), ..Ext::default() },
                    ),
                    Some(tp) => {
                        let v = text_child(text);
                        st.props.insert(tp.to_string(), v);
                        self.lines.insert(format!("{id}.{tp}"), el.line);
                    }
                },
            }
        }
        let mut children: Vec<String> = Vec::new();
        for ch in el.children {
            if SLATE_ITEM_KINDS.contains(&ch.tag.as_str()) {
                let Some(item_spec) = spec.and_then(|s| s.items.get(ch.tag.as_str())) else {
                    let kinds: Vec<&str> = spec.map(|s| s.items.keys().copied().collect()).unwrap_or_default();
                    let message = if kinds.is_empty() {
                        format!("{} takes no items, not <{}>", el.tag, ch.tag)
                    } else {
                        format!("{} takes {}, not <{}>", el.tag, kinds.iter().map(|k| format!("<{k}>")).collect::<Vec<_>>().join(" and "), ch.tag)
                    };
                    self.error("T314", &message, ch.line, Ext { piece: Some(&id), ..Ext::default() });
                    continue;
                };
                if !matches!(st.props.get(item_spec.prop), Some(Value::Array(_))) {
                    st.props.insert(item_spec.prop.to_string(), Value::Array(Vec::new()));
                }
                let n = match st.props.get(item_spec.prop) {
                    Some(Value::Array(list)) => list.len(),
                    _ => 0,
                };
                self.lines.insert(format!("{id}.{}[{n}]", item_spec.prop), ch.line);
                let v = self.item(&ch, item_spec, &id, &format!("{}[{n}]", item_spec.prop));
                if let Some(Value::Array(list)) = st.props.get_mut(item_spec.prop) {
                    list.push(v);
                }
                continue;
            }
            if let Some(child) = self.piece(ch) {
                children.push(child);
            }
        }
        let piece = SlatePiece {
            kind: el.tag,
            props: (!st.props.is_empty()).then_some(st.props),
            children: (!children.is_empty()).then_some(children),
            when: st.when,
            on: st.on.map(Value::Object),
            fallback: st.fallback,
        };
        self.doc.pieces.insert(id.clone(), piece);
        Some(id)
    }

    fn piece_attr(&mut self, a: &Attr, id: &str, tag: &str, spec: Option<&'static SlatePieceModule>, st: &mut PieceState) -> Result<(), Fatal> {
        if a.name == "when" {
            if a.kind != Kind::Braced {
                let shown = if a.kind == Kind::Str { a.value.as_str() } else { "..." };
                return Err(fatal("T303", format!("when takes a formula in braces: when={{{shown}}}"), a.line));
            }
            st.when = Some(js_trim(&a.value).to_string());
            return Ok(());
        }
        if a.name == "fallback" {
            if a.kind == Kind::Str {
                st.fallback = Some(Value::String(a.value.clone()));
                return Ok(());
            }
            let text = if a.kind == Kind::Braced { string_literal(&a.value) } else { None };
            let Some(text) = text else {
                return Err(fatal("T303", "fallback is drop, a piece id or a sentence in braces".into(), a.line));
            };
            st.fallback = Some(json!({"text": text}));
            return Ok(());
        }
        if is_on_attr(&a.name) {
            let event = a.name[2..].to_ascii_lowercase();
            if !matches!(event.as_str(), "press" | "submit" | "change") {
                let fix = if a.name == "onInput" { "onChange" } else { "onPress" };
                self.error(
                    "A602",
                    &format!("{} is not an event; events are onPress, onSubmit and onChange", a.name),
                    a.line,
                    Ext { piece: Some(id), prop: Some(a.name.as_str()), fix: Some(fix) },
                );
                return Ok(());
            }
            let steps = self.steps(a, id);
            st.on.get_or_insert_with(Map::new).insert(event, Value::Array(steps));
            return Ok(());
        }
        if a.kind == Kind::Bare {
            let declared = spec.and_then(|s| s.props.get(a.name.as_str()));
            let non_bool = declared.is_some_and(|p| p.prop_type != SlatePropType::Boolean);
            let owner = spec.and_then(|s| owner_of(s, &a.name));
            if non_bool || (declared.is_none() && owner.is_some()) {
                let fix = match &owner {
                    Some(o) => format!("{o}=\"{}\"", a.name),
                    None => format!("{}=\"...\"", a.name),
                };
                self.error(
                    "P102",
                    &format!("{} is not a boolean prop of {tag}", a.name),
                    a.line,
                    Ext { piece: Some(id), prop: Some(a.name.as_str()), fix: Some(&fix) },
                );
                return Ok(());
            }
            st.props.insert(a.name.clone(), Value::Bool(true));
            return Ok(());
        }
        let is_path = spec.and_then(|s| s.props.get(a.name.as_str())).is_some_and(|p| p.prop_type == SlatePropType::Path);
        if is_path && a.kind == Kind::Braced && slate_own_name(&a.value).is_some() {
            st.props.insert(a.name.clone(), Value::String(js_trim(&a.value).to_string()));
            return Ok(());
        }
        let v = self.value(a);
        st.props.insert(a.name.clone(), v);
        Ok(())
    }

    fn item(&mut self, el: &El, spec: &SlateItemSpec, piece_id: &str, at: &str) -> Value {
        let mut out = Map::new();
        for a in &el.attrs {
            self.lines.insert(format!("{piece_id}.{at}.{}", a.name), a.line);
            if is_on_attr(&a.name) {
                let event = a.name[2..].to_ascii_lowercase();
                if !spec.events.is_some_and(|list| list.contains(&event.as_str())) {
                    let extra = match spec.events {
                        Some(list) => format!("; it has {}", list.iter().map(|e| format!("on{}", upper_first(e))).collect::<Vec<_>>().join(", ")),
                        None => String::new(),
                    };
                    let prop = format!("{at}.{}", a.name);
                    self.error(
                        "A602",
                        &format!("<{}> has no {}{extra}", el.tag, a.name),
                        a.line,
                        Ext { piece: Some(piece_id), prop: Some(&prop), ..Ext::default() },
                    );
                    continue;
                }
                let steps = self.steps(a, piece_id);
                let mut on = match out.get("on") {
                    Some(Value::Object(m)) => m.clone(),
                    _ => Map::new(),
                };
                on.insert(event, Value::Array(steps));
                out.insert("on".into(), Value::Object(on));
                continue;
            }
            if a.name == "when" {
                if a.kind != Kind::Braced {
                    let prop = format!("{at}.when");
                    self.error("T303", "when takes a formula in braces", a.line, Ext { piece: Some(piece_id), prop: Some(&prop), ..Ext::default() });
                    continue;
                }
                out.insert("when".into(), Value::String(js_trim(&a.value).to_string()));
                continue;
            }
            let v = if a.kind == Kind::Bare { Value::Bool(true) } else { self.value(a) };
            out.insert(a.name.clone(), v);
        }
        if el.text.is_some() || !el.children.is_empty() || el.raw.is_some() {
            self.error(
                "P100",
                &format!("<{}> is self-closing: <{} ... />", el.tag, el.tag),
                el.line,
                Ext { piece: Some(piece_id), ..Ext::default() },
            );
        }
        Value::Object(out)
    }

    fn value(&mut self, a: &Attr) -> Value {
        match a.kind {
            Kind::Bare => Value::Bool(true),
            Kind::Str => Value::String(a.value.clone()),
            Kind::Braced => {
                let text = js_trim(&a.value);
                if text.starts_with('[') && text.ends_with(']') {
                    Value::Array(self.array(a, "").unwrap_or_default())
                } else if text.starts_with('{') && text.ends_with('}') {
                    Value::Object(self.object(a, "").unwrap_or_default().into_iter().collect())
                } else {
                    value_text(text)
                }
            }
        }
    }

    fn literal(&mut self, a: &Attr, piece: &str) -> Value {
        match a.kind {
            Kind::Str => Value::String(a.value.clone()),
            Kind::Bare => Value::Bool(true),
            Kind::Braced => match parse_literal(js_trim(&a.value)) {
                Ok(v) => v,
                Err(message) => {
                    self.error(
                        "S503",
                        &format!("start takes a literal (a string, a number, true, false, null, a list or a record of literals): {message}"),
                        a.line,
                        Ext { piece: Some(piece), prop: Some("start"), ..Ext::default() },
                    );
                    Value::Null
                }
            },
        }
    }

    fn object(&mut self, a: &Attr, piece: &str) -> Option<IndexMap<String, Value>> {
        let text = js_trim(&a.value);
        if a.kind != Kind::Braced || !text.starts_with('{') || !text.ends_with('}') {
            self.error(
                "A601",
                &format!("{} takes an object: {}={{{{ NAME: $value }}}}", a.name, a.name),
                a.line,
                Ext { piece: Some(piece), prop: Some(a.name.as_str()), ..Ext::default() },
            );
            return None;
        }
        let mut out: IndexMap<String, Value> = IndexMap::new();
        for part in split_top(&text[1..text.len() - 1]) {
            match OBJECT_PART.captures(&part) {
                None => self.error(
                    "A601",
                    &format!("cannot read \"{}\" as key: value in {}", js_trim(&part), a.name),
                    a.line,
                    Ext { piece: Some(piece), prop: Some(a.name.as_str()), ..Ext::default() },
                ),
                Some(c) => {
                    let key = c.get(1).or(c.get(2)).or(c.get(3)).map_or("", |m| m.as_str()).to_string();
                    out.insert(key, value_text(c.get(4).map_or("", |m| m.as_str())));
                }
            }
        }
        Some(js_index(out))
    }

    fn array(&mut self, a: &Attr, piece: &str) -> Option<Vec<Value>> {
        let text = js_trim(&a.value);
        if a.kind != Kind::Braced || !text.starts_with('[') || !text.ends_with(']') {
            self.error(
                "A601",
                &format!("{} takes a list: {}={{[$a, 'x']}}", a.name, a.name),
                a.line,
                Ext { piece: Some(piece), prop: Some(a.name.as_str()), ..Ext::default() },
            );
            return None;
        }
        Some(split_top(&text[1..text.len() - 1]).iter().map(|p| value_text(p)).collect())
    }

    fn mint(&mut self, ty: &str) -> String {
        let base = if is_piece_base(ty) { ty.to_string() } else { "piece".to_string() };
        let mut n = self.minted.get(&base).copied().unwrap_or(0);
        let id = loop {
            n += 1;
            let id = format!("{base}-{n}");
            if !(self.taken.contains(&id) || self.doc.pieces.contains_key(&id)) {
                break id;
            }
        };
        self.minted.insert(base, n);
        id
    }

    fn patch_ops(&mut self, tops: Vec<El>, current: Option<&SlateDoc>) -> Vec<Value> {
        let mut ops: Vec<Value> = Vec::new();
        for el in &tops {
            for id in explicit_ids(el) {
                if current.map_or(true, |c| !c.pieces.contains_key(&id)) {
                    self.taken.insert(id);
                }
            }
        }
        for el in tops {
            match el.tag.as_str() {
                "clear" | "undo" => {
                    ops.push(json!({"op": el.tag}));
                    continue;
                }
                "remove" => {
                    let id = lit(&el, "id");
                    let name = lit(&el, "name");
                    if id.is_none() == name.is_none() {
                        self.error(
                            "P105",
                            "<remove> takes id=\"...\" for a piece or a reaction, or name=\"...\" for a declaration",
                            el.line,
                            Ext::default(),
                        );
                        continue;
                    }
                    if let Some(name) = name {
                        if current.is_some_and(|c| c.derived.contains_key(&name)) {
                            ops.push(json!({"op": "derived", "name": name, "expr": null}));
                        } else if current.is_some_and(|c| c.runs.contains_key(&name)) {
                            ops.push(json!({"op": "run", "name": name, "decl": null}));
                        } else if current.is_some_and(|c| c.values.contains_key(&name)) {
                            ops.push(json!({"op": "value", "name": name, "decl": null}));
                        } else if current.is_some_and(|c| c.files.as_ref().is_some_and(|f| f.contains_key(&name))) {
                            ops.push(json!({"op": "file", "name": name, "text": null}));
                        } else {
                            let mut names: Vec<String> = self.names.keys().cloned().collect();
                            names.extend(current.and_then(|c| c.files.as_ref()).map(|f| f.keys().cloned().collect::<Vec<_>>()).unwrap_or_default());
                            let fix = slate_nearest(&name, &names);
                            self.error(
                                "D203",
                                &format!("nothing is declared as {name}"),
                                el.line,
                                Ext { fix: fix.as_deref(), ..Ext::default() },
                            );
                        }
                        continue;
                    }
                    let id = id.unwrap_or_default();
                    let is_reaction = current.is_some_and(|c| {
                        !c.pieces.contains_key(&id) && c.reactions.iter().any(|r| r.get("id").and_then(Value::as_str) == Some(id.as_str()))
                    });
                    if is_reaction {
                        ops.push(json!({"op": "reaction", "id": id, "reaction": null}));
                    } else {
                        ops.push(json!({"op": "remove", "id": id}));
                    }
                    continue;
                }
                "move" => {
                    let id = lit(&el, "id");
                    if id.is_none() || attr_of(&el, "under").is_none() {
                        self.error("P105", "<move> takes id and under", el.line, Ext::default());
                        continue;
                    }
                    let n = self.at_of(&el);
                    let mut op = json!({"op": "move", "id": id, "under": under_of(&el, current)});
                    if let Some(n) = n {
                        op["at"] = n;
                    }
                    ops.push(op);
                    continue;
                }
                "add" => {
                    if attr_of(&el, "under").is_none() {
                        self.error("P105", "<add> takes under=\"<id>\"", el.line, Ext::default());
                        continue;
                    }
                    let before: HashSet<String> = self.doc.pieces.keys().cloned().collect();
                    let mut order: Vec<String> = Vec::new();
                    for ch in el.children.clone() {
                        if let Some(id) = self.piece(ch) {
                            order.push(id);
                        }
                    }
                    let mut pieces = Map::new();
                    for (k, p) in &self.doc.pieces {
                        if !before.contains(k) {
                            pieces.insert(k.clone(), piece_value(p));
                        }
                    }
                    let n = self.at_of(&el);
                    let mut op = json!({"op": "add", "under": under_of(&el, current)});
                    if let Some(n) = n {
                        op["at"] = n;
                    }
                    op["pieces"] = Value::Object(pieces);
                    op["order"] = json!(order);
                    ops.push(op);
                    continue;
                }
                "props" => {
                    let Some(id) = lit(&el, "id") else {
                        self.error("P105", "<props> takes id=\"...\"", el.line, Ext::default());
                        continue;
                    };
                    let op = self.props_op(&el, &id, current);
                    ops.push(op);
                    continue;
                }
                _ => {}
            }
            if is_file_decl_el(&el) {
                if let Some(name) = self.file(&el) {
                    let text = self.doc.files.as_ref().and_then(|f| f.get(&name)).cloned().unwrap_or_default();
                    ops.push(json!({"op": "file", "name": name, "text": text}));
                }
                continue;
            }
            if is_decl_tag(&el.tag) {
                if let Err(f) = self.declare_name(&el) {
                    self.report(f);
                }
                let before = self.doc.reactions.len();
                self.declaration(&el);
                let name = attr_of(&el, "name").map_or(String::new(), |a| a.value.clone());
                match el.tag.as_str() {
                    "value" | "secret" => {
                        if let Some(decl) = self.doc.values.get(&name) {
                            ops.push(json!({"op": "value", "name": name, "decl": decl}));
                        }
                    }
                    "derived" => {
                        if let Some(expr) = self.doc.derived.get(&name) {
                            ops.push(json!({"op": "derived", "name": name, "expr": expr}));
                        }
                    }
                    "run" => {
                        if let Some(decl) = self.doc.runs.get(&name) {
                            ops.push(json!({"op": "run", "name": name, "decl": decl}));
                        }
                    }
                    _ => {
                        if self.doc.reactions.len() > before {
                            let r = self.doc.reactions.last().cloned().unwrap_or(Value::Null);
                            let rid = r.get("id").and_then(Value::as_str).unwrap_or("").to_string();
                            ops.push(json!({"op": "reaction", "id": rid, "reaction": r}));
                        }
                    }
                }
                continue;
            }
            if slate_piece_module(&el.tag).is_some() || !SLATE_ITEM_KINDS.contains(&el.tag.as_str()) {
                let Some(id) = lit(&el, "id") else {
                    let fix = format!("<add under=\"{}\"><{} ... /></add>", current.map_or("root", |c| c.root.as_str()), el.tag);
                    self.error(
                        "P105",
                        &format!("a piece at the top of a patch replaces the piece with its id, so <{}> needs id=\"...\"", el.tag),
                        el.line,
                        Ext { fix: Some(&fix), ..Ext::default() },
                    );
                    continue;
                };
                if let Some(c) = current {
                    if !c.pieces.contains_key(&id) {
                        let names: Vec<String> = c.pieces.keys().cloned().collect();
                        let fix = slate_nearest(&id, &names);
                        self.error(
                            "D203",
                            &format!("there is no piece {id} to replace; add it with <add under=\"...\">"),
                            el.line,
                            Ext { fix: fix.as_deref(), ..Ext::default() },
                        );
                        continue;
                    }
                }
                self.taken.remove(&id);
                let before: HashSet<String> = self.doc.pieces.keys().cloned().collect();
                self.piece(el);
                let mut children = Map::new();
                for (k, p) in &self.doc.pieces {
                    if !before.contains(k) && *k != id {
                        children.insert(k.clone(), piece_value(p));
                    }
                }
                let piece_v = self.doc.pieces.get(&id).map_or(Value::Null, piece_value);
                let mut op = json!({"op": "replace", "id": id, "piece": piece_v});
                if !children.is_empty() {
                    op["children"] = Value::Object(children);
                }
                ops.push(op);
                continue;
            }
            self.error(
                "P105",
                &format!(
                    "a patch is pieces with an id, <props>, <add>, <remove>, <move>, declarations, <clear> or <undo>; not <{}>, an item: change it inside its piece with <props id=\"...\"><{} ... /></props>",
                    el.tag, el.tag
                ),
                el.line,
                Ext::default(),
            );
        }
        ops
    }

    fn props_op(&mut self, el: &El, id: &str, current: Option<&SlateDoc>) -> Value {
        let mut op = Map::new();
        op.insert("op".into(), json!("props"));
        op.insert("id".into(), json!(id));
        for a in &el.attrs {
            if a.name == "id" {
                continue;
            }
            self.lines.insert(format!("{id}.{}", a.name), a.line);
            let is_null = a.kind == Kind::Braced && js_trim(&a.value) == "null";
            if a.name == "when" {
                let v = if is_null {
                    Value::Null
                } else if a.kind == Kind::Braced {
                    Value::String(js_trim(&a.value).to_string())
                } else {
                    self.error("T303", "when takes a formula in braces", a.line, Ext { piece: Some(id), prop: Some("when"), ..Ext::default() });
                    Value::Null
                };
                op.insert("when".into(), v);
                continue;
            }
            if a.name == "fallback" {
                let v = if is_null {
                    Value::Null
                } else if a.kind == Kind::Str {
                    Value::String(a.value.clone())
                } else {
                    json!({"text": string_literal(&a.value).unwrap_or_default()})
                };
                op.insert("fallback".into(), v);
                continue;
            }
            if is_on_attr(&a.name) {
                let event = a.name[2..].to_ascii_lowercase();
                let steps = self.steps(a, id);
                sub_map(&mut op, "on").insert(event, Value::Array(steps));
                continue;
            }
            let v = if is_null { Value::Null } else { self.value(a) };
            sub_map(&mut op, "props").insert(a.name.clone(), v);
        }
        let target = current.and_then(|c| c.pieces.get(id)).cloned();
        let target_module = target.as_ref().and_then(|t| slate_piece_module(&t.kind));
        if let Some(text) = &el.text {
            let text_prop = target_module.and_then(|m| m.text_prop);
            let has_prop = op.get("props").and_then(Value::as_object).is_some_and(|p| text_prop.is_some_and(|tp| p.contains_key(tp)));
            match (&target, text_prop) {
                (Some(t), None) => self.error(
                    "P105",
                    &format!("{} takes no text child; set its props as attributes: <props id=\"{id}\" ... />", t.kind),
                    el.line,
                    Ext { piece: Some(id), ..Ext::default() },
                ),
                (_, Some(tp)) if has_prop => self.error(
                    "T303",
                    &format!("<props> has both {tp}= and a text child; use one"),
                    el.line,
                    Ext { piece: Some(id), prop: Some(tp), ..Ext::default() },
                ),
                (_, Some(tp)) => {
                    let v = text_child(text);
                    sub_map(&mut op, "props").insert(tp.to_string(), v);
                    self.lines.insert(format!("{id}.{tp}"), el.line);
                }
                (None, None) => {}
            }
        }
        for ch in &el.children {
            let item_spec = target_module.and_then(|m| m.items.get(ch.tag.as_str()));
            let Some(spec) = item_spec else {
                if let Some(t) = &target {
                    self.error(
                        "P105",
                        &format!(
                            "<props> merges attributes and the items {} takes; to change <{}> under {id}, write {id} again with its id or use <add under=\"{id}\">",
                            t.kind, ch.tag
                        ),
                        ch.line,
                        Ext { piece: Some(id), ..Ext::default() },
                    );
                }
                continue;
            };
            let key = spec.fields.keys().next().copied().unwrap_or("");
            let existing = op.get("props").and_then(|p| p.get(spec.prop)).filter(|v| !v.is_null()).cloned();
            let mut list: Vec<Value> = match existing {
                Some(Value::Array(xs)) => xs,
                Some(_) => Vec::new(),
                None => target
                    .as_ref()
                    .and_then(|t| t.props.as_ref())
                    .and_then(|p| p.get(spec.prop))
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default(),
            };
            let n = list.len();
            let item = self.item(ch, spec, id, &format!("{}[{n}]", spec.prop));
            let item_map = item.as_object().cloned().unwrap_or_default();
            let wanted = json_text(item_map.get(key));
            let pos = list.iter().position(|x| json_text(x.get(key)) == wanted);
            let mut merged: Map<String, Value> = match pos {
                Some(i) => list[i].as_object().cloned().unwrap_or_default(),
                None => Map::new(),
            };
            for (k, v) in item_map {
                merged.insert(k, v);
            }
            merged.retain(|_, v| !v.is_null());
            match pos {
                Some(i) => list[i] = Value::Object(merged),
                None => list.push(Value::Object(merged)),
            }
            sub_map(&mut op, "props").insert(spec.prop.to_string(), Value::Array(list));
        }
        Value::Object(op)
    }
}

fn lit(el: &El, name: &str) -> Option<String> {
    let a = attr_of(el, name)?;
    match a.kind {
        Kind::Str => Some(a.value.clone()),
        Kind::Braced => string_literal(&a.value),
        Kind::Bare => None,
    }
}

impl Compiler {
    fn at_of(&mut self, el: &El) -> Option<Value> {
        let a = attr_of(el, "at")?;
        let n = to_number_text(&a.value);
        if !(n.is_finite() && n.fract() == 0.0) {
            self.error("P105", "at is a whole number: at={0}", a.line, Ext::default());
        }
        Some(jnum(n))
    }
}

fn under_of(el: &El, current: Option<&SlateDoc>) -> String {
    let u = lit(el, "under").unwrap_or_default();
    if u == "root" && current.map_or(true, |c| !c.pieces.contains_key("root")) {
        current.map_or(u.clone(), |c| c.root.clone())
    } else {
        u
    }
}

fn slate_own_name_braced(a: &Attr) -> Option<String> {
    if a.kind == Kind::Braced {
        slate_own_name(&a.value)
    } else {
        None
    }
}

fn text_child(parts: &[Part]) -> Value {
    let mut format = String::new();
    let mut holes = 0;
    for p in parts {
        match p {
            Part::Text(s) => format.push_str(&normalize_run(s).replace("${", "$${")),
            Part::Hole(expr) => {
                let e = js_trim(expr);
                if e.starts_with("/*") && e.ends_with("*/") {
                    continue;
                }
                if let Some(lit) = string_literal(e) {
                    format.push_str(&lit.replace("${", "$${"));
                    continue;
                }
                if format.ends_with('$') {
                    format.pop();
                    format.push_str("${'$'}");
                }
                format.push_str(&format!("${{{e}}}"));
                holes += 1;
            }
        }
    }
    let format = js_trim(&format).to_string();
    if holes == 0 {
        Value::String(format.replace("$${", "${"))
    } else {
        json!({"format": format})
    }
}

/// Compiles the JSX-like text to a document and its lines, without validating it.
pub fn compile_slate_text(text: &str) -> SlateCompiled {
    let mut c = Compiler::new(None);
    let tops = match Reader::new(text).elements() {
        Ok(tops) => tops,
        Err(f) => return SlateCompiled { document: None, lines: c.lines, errors: vec![fatal_problem(&f)] },
    };
    if tops.len() != 1 || tops[0].tag != "slate" {
        let first = tops.iter().find(|t| t.tag != "slate");
        let message = if tops.is_empty() {
            "there is no <slate> element".to_string()
        } else {
            format!(
                "a document is one <slate> element with the pieces inside it; found {}",
                tops.iter().map(|t| format!("<{}>", t.tag)).collect::<Vec<_>>().join(", ")
            )
        };
        let line = first.map_or(1, |t| t.line);
        let errors = vec![problem("D202", &message, line, &Ext { fix: Some("<slate title=\"...\">...</slate>"), ..Ext::default() })];
        return SlateCompiled { document: None, lines: c.lines, errors };
    }
    if let Some(top) = tops.into_iter().next() {
        c.document(top);
    }
    let document = if c.errors.is_empty() { Some(c.doc) } else { None };
    SlateCompiled { document, lines: c.lines, errors: c.errors }
}

/// The JSX-like form compiled and validated: the document when there are no errors.
pub fn parse_slate(text: &str) -> SlateChecked {
    let compiled = compile_slate_text(text);
    let Some(doc) = compiled.document else {
        return SlateChecked { document: None, errors: compiled.errors, warnings: Vec::new() };
    };
    let v = validate_document(&doc, Some(&compiled.lines));
    let document = if v.errors.is_empty() { Some(doc) } else { None };
    SlateChecked { document, errors: v.errors, warnings: v.warnings }
}

/// A patch's top-level elements compiled against the document they apply to; the shape only, apply_slate_patch validates.
pub fn parse_slate_patch(text: &str, doc: Option<&SlateDoc>) -> SlatePatchParsed {
    let mut c = Compiler::new(doc);
    c.patch = true;
    let tops = match Reader::new(text).elements() {
        Ok(tops) => tops,
        Err(f) => {
            return SlatePatchParsed { ops: None, lines: lines_value(&c.lines), errors: vec![fatal_problem(&f)] };
        }
    };
    if tops.iter().any(|t| t.tag == "slate") {
        let errors = vec![problem(
            "P105",
            "a patch is elements without <slate>; to rewrite the whole slate, write the <slate> alone",
            1,
            &Ext::default(),
        )];
        return SlatePatchParsed { ops: None, lines: lines_value(&c.lines), errors };
    }
    if tops.is_empty() {
        let errors = vec![problem("P105", "the patch is empty", 1, &Ext::default())];
        return SlatePatchParsed { ops: None, lines: lines_value(&c.lines), errors };
    }
    let ops = c.patch_ops(tops, doc);
    let alone = ops.iter().find_map(|o| match o.get("op").and_then(Value::as_str) {
        Some(op @ ("clear" | "undo")) => Some(op.to_string()),
        _ => None,
    });
    if let Some(alone) = alone {
        if ops.len() > 1 {
            c.error("P105", &format!("<{alone} /> stands alone in its write"), 1, Ext::default());
        }
    }
    let lines = lines_value(&c.lines);
    SlatePatchParsed { ops: c.errors.is_empty().then_some(ops), lines, errors: c.errors }
}

fn lines_value(lines: &SlateLines) -> Value {
    Value::Object(lines.iter().map(|(k, v)| (k.clone(), Value::from(*v))).collect())
}

// ---- printing ----

fn quote_attr(s: &str) -> String {
    let quotable = !s.ends_with('\\') && !HTML_ENTITY.is_match(s);
    if quotable && !s.contains('"') {
        return format!("\"{s}\"");
    }
    if quotable && !s.contains('\'') {
        return format!("'{s}'");
    }
    format!("{{{}}}", expr_string(s))
}

fn expr_string(s: &str) -> String {
    let body = s.replace('\\', "\\\\").replace('\'', "\\'").replace('\n', "\\n").replace('\r', "\\r").replace('\t', "\\t");
    format!("'{body}'")
}

fn command_attr(name: &str, cmd: &str) -> String {
    let raw = (cmd.contains('\n') || (cmd.contains('"') && cmd.contains('\''))) && !cmd.contains('`') && dedent(cmd) == cmd;
    if raw {
        format!("{name}={{`{cmd}`}}")
    } else {
        format!("{name}={}", quote_attr(cmd))
    }
}

fn literal_text(v: &Value) -> String {
    match v {
        Value::String(s) => expr_string(s),
        Value::Array(xs) => format!("[{}]", xs.iter().map(literal_text).collect::<Vec<_>>().join(", ")),
        Value::Object(m) => format!("{{ {} }}", object_fields(m, literal_text)),
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => number_string(n.as_f64().unwrap_or(f64::NAN)),
    }
}

fn object_fields(m: &Map<String, Value>, each: fn(&Value) -> String) -> String {
    m.iter().map(|(k, x)| format!("{}: {}", json_key(k), each(x))).collect::<Vec<_>>().join(", ")
}

fn value_expr(v: &Value) -> String {
    if is_slate_binding(v) {
        return v.get("bind").and_then(Value::as_str).unwrap_or("").to_string();
    }
    if let Some(format) = format_text(v) {
        let mut out = String::from("`");
        for part in parse_slate_format(format).parts {
            match part {
                SlateFormatPart::Text(t) => out.push_str(&t.replace("${", "$${").replace('`', "\\`")),
                SlateFormatPart::Hole { expr, .. } => out.push_str(&format!("${{{expr}}}")),
            }
        }
        out.push('`');
        return out;
    }
    match v {
        Value::Array(xs) => format!("[{}]", xs.iter().map(value_expr).collect::<Vec<_>>().join(", ")),
        Value::Object(m) => format!("{{ {} }}", object_fields(m, value_expr)),
        _ => literal_text(v),
    }
}

fn attr_text(name: &str, v: &Value) -> String {
    match v {
        Value::Bool(true) => name.to_string(),
        Value::String(s) => format!("{name}={}", quote_attr(s)),
        Value::Array(xs) => format!("{name}={{[{}]}}", xs.iter().map(value_expr).collect::<Vec<_>>().join(", ")),
        _ => format!("{name}={{{}}}", value_expr(v)),
    }
}

fn step_text(s: &Value) -> String {
    let kind = s.get("do").and_then(Value::as_str).unwrap_or("");
    let text = |k: &str| s.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    match kind {
        "set" => format!("set({}, {})", text("path"), value_expr(s.get("value").unwrap_or(&Value::Null))),
        "toggle" => format!("toggle({})", text("path")),
        "start" | "cancel" => format!("{kind}(${})", text("run")),
        "send" | "steer" | "queue" | "fill" => {
            let mut args = vec![json_stringify(&Value::String(text("text")))];
            if let Some(with) = s.get("with").and_then(Value::as_array) {
                args.extend(with.iter().filter_map(Value::as_str).map(str::to_string));
            }
            format!("{kind}({})", args.join(", "))
        }
        "open" => format!("open({})", value_expr(s.get("target").unwrap_or(&Value::Null))),
        "copy" => format!("copy({})", value_expr(s.get("text").unwrap_or(&Value::Null))),
        "pane" => format!("pane({})", json_stringify(&Value::String(text("kind")))),
        _ => String::new(),
    }
}

fn steps_attr(name: &str, steps: &[Value]) -> String {
    let body = if steps.len() == 1 {
        step_text(&steps[0])
    } else {
        format!("[{}]", steps.iter().map(step_text).collect::<Vec<_>>().join(", "))
    };
    format!("{name}={{{body}}}")
}

fn as_text_child(v: &Value) -> Option<String> {
    if let Value::String(s) = v {
        if s.is_empty() || s.contains(['<', '>', '{', '}', '\n']) || slate_text_child_form(s) != *s || s.contains("${") {
            return None;
        }
        return Some(s.clone());
    }
    let f = format_text(v)?;
    if f.contains('\n') || slate_text_child_form(f) != f || f.contains("$${") {
        return None;
    }
    let src: Vec<char> = f.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < src.len() {
        if src[i] == '$' && src.get(i + 1) == Some(&'{') {
            let mut depth = 1;
            let mut j = i + 2;
            let mut quote: Option<char> = None;
            while j < src.len() {
                let c = src[j];
                if let Some(q) = quote {
                    if c == '\\' {
                        j += 1;
                    } else if c == q {
                        quote = None;
                    }
                } else if c == '\'' || c == '"' || c == '`' {
                    quote = Some(c);
                } else if c == '{' {
                    depth += 1;
                } else if c == '}' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                j += 1;
            }
            let end = j.min(src.len());
            out.push('{');
            out.extend(src[i + 2..end].iter());
            out.push('}');
            i = j + 1;
            continue;
        }
        if matches!(src[i], '<' | '>' | '{' | '}') {
            return None;
        }
        out.push(src[i]);
        i += 1;
    }
    Some(out)
}

struct Printer<'a> {
    doc: &'a SlateDoc,
    out: Vec<String>,
    seen: HashSet<String>,
}

impl Printer<'_> {
    fn line(&mut self, depth: usize, parts: &[String], tail: &str) {
        let indent = "  ".repeat(depth);
        let mut cur = format!("{indent}{}", parts[0]);
        for p in &parts[1..] {
            if utf16::len(&cur) + 1 + utf16::len(p) + utf16::len(tail) > SLATE_LIMITS.print_columns && js_trim(&cur) != parts[0] {
                self.out.push(cur);
                cur = format!("{indent}    {p}");
            } else {
                cur.push(' ');
                cur.push_str(p);
            }
        }
        self.out.push(format!("{cur}{tail}"));
    }

    fn declarations(&mut self) {
        let doc = self.doc;
        for (name, v) in doc.values.iter().filter(|(_, v)| v.secret != Some(true)) {
            let mut parts = vec!["<value".to_string(), format!("name=\"{name}\"")];
            if !v.start.is_null() {
                parts.push(match &v.start {
                    Value::String(s) => format!("start={}", quote_attr(s)),
                    other => format!("start={{{}}}", literal_text(other)),
                });
            }
            self.line(1, &parts, " />");
        }
        for (name, v) in doc.values.iter().filter(|(_, v)| v.secret == Some(true)) {
            let mut parts = vec!["<secret".to_string(), format!("name=\"{name}\"")];
            if v.keep == Some(true) {
                parts.push("keep".to_string());
            }
            self.line(1, &parts, " />");
        }
        for (name, expr) in &doc.derived {
            let parts = vec!["<derived".to_string(), format!("name=\"{name}\""), format!("value={{{expr}}}")];
            self.line(1, &parts, " />");
        }
        for (name, r) in &doc.runs {
            let mut parts = vec!["<run".to_string(), format!("name=\"{name}\"")];
            match r {
                SlateRunDecl::Cmd(c) => {
                    parts.push(command_attr("cmd", &c.cmd));
                    if let Some(env) = &c.env {
                        parts.push(format!("env={{{}}}", value_expr(&record_value(env))));
                    }
                    if let Some(args) = &c.args {
                        parts.push(format!("args={{[{}]}}", args.iter().map(value_expr).collect::<Vec<_>>().join(", ")));
                    }
                    if let Some(stdin) = &c.stdin {
                        parts.push(attr_text("stdin", stdin));
                    }
                    if let Some(on) = &c.on {
                        parts.push(format!("on=\"{on}\""));
                    }
                    if let Some(cwd) = &c.cwd {
                        parts.push(format!("cwd={}", quote_attr(cwd)));
                    }
                    if let Some(timeout) = &c.timeout {
                        parts.push(format!("timeout={{{}}}", timeout.js_string()));
                    }
                    if c.stream == Some(true) {
                        parts.push("stream".to_string());
                    }
                    push_tail(&mut parts, c.confirm.as_ref(), c.every.as_ref(), c.once, c.always, c.then.as_deref());
                }
                SlateRunDecl::Tool(t) => {
                    parts.push(format!("tool=\"{}.{}\"", t.server, t.tool));
                    if let Some(args) = &t.args {
                        parts.push(format!("args={{{}}}", value_expr(&record_value(args))));
                    }
                    push_tail(&mut parts, t.confirm.as_ref(), t.every.as_ref(), t.once, t.always, t.then.as_deref());
                    if let Some(d) = &t.description {
                        parts.push(format!("description={}", quote_attr(d)));
                    }
                }
                SlateRunDecl::Resource(res) => {
                    parts.push(format!("resource={}", quote_attr(&format!("{}:{}", res.server, res.uri))));
                    push_tail(&mut parts, None, res.every.as_ref(), None, res.always, res.then.as_deref());
                    if let Some(d) = &res.description {
                        parts.push(format!("description={}", quote_attr(d)));
                    }
                }
            }
            self.line(1, &parts, " />");
        }
        for r in &doc.reactions {
            let mut parts = vec!["<when".to_string()];
            if let Some(id) = r.get("id").and_then(Value::as_str) {
                parts.push(format!("id=\"{id}\""));
            }
            let on = r.get("on");
            if let Some(change) = on.and_then(|o| o.get("change")).and_then(Value::as_array) {
                let paths: Vec<&str> = change.iter().filter_map(Value::as_str).collect();
                let body = if paths.len() == 1 { paths[0].to_string() } else { format!("[{}]", paths.join(", ")) };
                parts.push(format!("change={{{body}}}"));
            } else {
                let done = on.and_then(|o| o.get("done")).and_then(Value::as_str).unwrap_or("");
                parts.push(format!("done={{${done}}}"));
            }
            let steps = r.get("do").and_then(Value::as_array).cloned().unwrap_or_default();
            parts.push(steps_attr("do", &steps));
            self.line(1, &parts, " />");
        }
    }

    fn piece(&mut self, id: &str, depth: usize) {
        let doc = self.doc;
        let Some(p) = doc.pieces.get(id) else { return };
        if !self.seen.insert(id.to_string()) {
            return;
        }
        let spec = slate_piece_module(&p.kind);
        let mut props: IndexMap<String, Value> = p.props.clone().unwrap_or_default();
        let is_item_list = |v: Option<&Value>| {
            matches!(v, Some(Value::Array(xs)) if xs.iter().all(|x| x.is_object() && !is_slate_binding(x) && format_text(x).is_none()))
        };
        let mut item_props: IndexMap<String, &'static str> = IndexMap::new();
        if let Some(s) = spec {
            for (tag, item) in &s.items {
                if is_item_list(p.props.as_ref().and_then(|m| m.get(item.prop))) {
                    item_props.insert(item.prop.to_string(), tag);
                }
            }
        }
        let mut text: Option<String> = None;
        let mut raw: Option<String> = None;
        if let Some(tp) = spec.and_then(|s| s.text_prop) {
            if let Some(v) = props.get(tp).cloned() {
                let raw_text = spec.is_some_and(|s| s.raw_text);
                if raw_text {
                    if let Value::String(s) = &v {
                        if !s.contains(&format!("</{}>", p.kind)) {
                            raw = Some(s.clone());
                            props.shift_remove(tp);
                        }
                    }
                } else if let Some(t) = as_text_child(&v) {
                    text = Some(t);
                    props.shift_remove(tp);
                }
            }
        }
        let mut names: IndexSet<String> = IndexSet::new();
        if let Some(s) = spec {
            names.extend(s.props.keys().map(|k| k.to_string()));
        }
        names.extend(props.keys().cloned());
        let mut parts = vec![format!("<{}", p.kind), format!("id=\"{id}\"")];
        for name in &names {
            if !props.contains_key(name) || item_props.contains_key(name) || name == "when" {
                continue;
            }
            let v = &props[name];
            let is_path = spec.and_then(|s| s.props.get(name.as_str())).is_some_and(|x| x.prop_type == SlatePropType::Path);
            match v {
                Value::String(s) if is_path && s.starts_with('$') => parts.push(format!("{name}={{{s}}}")),
                _ => parts.push(attr_text(name, v)),
            }
        }
        if let Some(on) = p.on.as_ref().and_then(Value::as_object) {
            for (ev, steps) in on {
                if let Some(steps) = steps.as_array() {
                    parts.push(steps_attr(&format!("on{}", upper_first(ev)), steps));
                }
            }
        }
        if let Some(fallback) = &p.fallback {
            match fallback {
                Value::String(s) => parts.push(format!("fallback={}", quote_attr(s))),
                other => {
                    let body = other.get("text").and_then(Value::as_str).unwrap_or("");
                    parts.push(format!("fallback={{{}}}", expr_string(body)));
                }
            }
        }
        if let Some(when) = &p.when {
            parts.push(format!("when={{{when}}}"));
        }
        let mut items: Vec<Vec<String>> = Vec::new();
        for (prop, tag) in &item_props {
            let Some(Value::Array(list)) = props.get(prop) else { continue };
            for it in list {
                let empty = Map::new();
                let r = it.as_object().unwrap_or(&empty);
                let mut ip = vec![format!("<{tag}")];
                let fields: Vec<String> = spec.and_then(|s| s.items.get(*tag)).map_or(Vec::new(), |i| i.fields.keys().map(|k| k.to_string()).collect());
                let mut keys: IndexSet<String> = fields.into_iter().collect();
                keys.extend(r.keys().cloned());
                for k in &keys {
                    if !r.contains_key(k) || k == "on" || k == "when" {
                        continue;
                    }
                    ip.push(attr_text(k, &r[k]));
                }
                if let Some(on) = r.get("on").and_then(Value::as_object) {
                    for (ev, steps) in on {
                        if let Some(steps) = steps.as_array() {
                            ip.push(steps_attr(&format!("on{}", upper_first(ev)), steps));
                        }
                    }
                }
                if let Some(when) = r.get("when").and_then(Value::as_str) {
                    ip.push(format!("when={{{when}}}"));
                }
                items.push(ip);
            }
        }
        let children = p.children.clone().unwrap_or_default();
        if text.is_none() && raw.is_none() && items.is_empty() && children.is_empty() {
            self.line(depth, &parts, " />");
            return;
        }
        if let Some(r) = raw {
            self.line(depth, &parts, &format!(">{r}</{}>", p.kind));
            return;
        }
        if let Some(t) = &text {
            if items.is_empty() && children.is_empty() {
                self.line(depth, &parts, &format!(">{t}</{}>", p.kind));
                return;
            }
        }
        self.line(depth, &parts, ">");
        if let Some(t) = &text {
            self.out.push(format!("{}{t}", "  ".repeat(depth + 1)));
        }
        for it in &items {
            self.line(depth + 1, it, " />");
        }
        for c in &children {
            self.piece(c, depth + 1);
        }
        self.out.push(format!("{}</{}>", "  ".repeat(depth), p.kind));
    }

    fn files(&mut self) {
        let doc = self.doc;
        let Some(files) = &doc.files else { return };
        for (name, text) in files {
            let edge_space = text.chars().next().is_some_and(is_js_whitespace) || text.chars().last().is_some_and(is_js_whitespace);
            let braced = !text.contains('`') && !edge_space;
            let body = if braced {
                let inner: Vec<String> = text.split('\n').map(|l| if l.is_empty() { String::new() } else { format!("    {l}") }).collect();
                format!("{{`\n{}\n  `}}", inner.join("\n"))
            } else {
                text.clone()
            };
            self.out.push(format!("  <file name=\"{name}\">{body}</file>"));
        }
    }
}

fn push_tail(parts: &mut Vec<String>, confirm: Option<&Value>, every: Option<&JsNumber>, once: Option<bool>, always: Option<bool>, then: Option<&str>) {
    if let Some(c) = confirm {
        parts.push(match c {
            Value::String(s) => format!("confirm={}", quote_attr(s)),
            other => attr_text("confirm", other),
        });
    }
    if let Some(e) = every {
        parts.push(format!("every={{{}}}", e.js_string()));
    }
    if once == Some(true) {
        parts.push("once".to_string());
    }
    if always == Some(true) {
        parts.push("always".to_string());
    }
    if let Some(t) = then {
        parts.push(command_attr("then", t));
    }
}

/// The document in the JSX-like form: declarations first, then the root piece and its subtree.
pub fn print_slate(doc: &SlateDoc) -> String {
    let head = match &doc.title {
        Some(t) => format!("<slate title={}>", quote_attr(t)),
        None => "<slate>".to_string(),
    };
    let mut p = Printer { doc, out: vec![head], seen: HashSet::new() };
    p.declarations();
    p.piece(&doc.root, 1);
    p.files();
    p.out.push("</slate>".to_string());
    p.out.join("\n")
}
