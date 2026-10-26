// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/project-state/resolver.ts. Spec: spec/project_state_files.md.
//!
//! Moving a project's agent state to a new absolute path: the trait every agent module implements, the report it
//! returns, and the file operations the modules share.
use std::fs::{self, DirEntry, File, Permissions};
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use async_trait::async_trait;
use serde_json::{Map, Number, Value};

use wsp_wire::views::project_bundle::ProjectCarry;
use wsp_words::js::number_string;
use wsp_words::js::utf16::cmp as js_cmp;
use wsp_words::project_path::under_project;

use crate::errors::{EngineError, Result};

/// resolver.ts:15. One catalog projectState row the module moved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovedState {
    pub state: String,
    pub files: Vec<String>,
    pub changed: u64,
    /// Sessions the row named whose transcript was not under the home; `None`, never `Some(0)`.
    pub skipped: Option<u64>,
}

/// resolver.ts:26. A store one agent keeps for every project: its path under the home and the python3 lines that write
/// PATH's rows of STORE into COPY, counting them in FILTERED.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedStore {
    pub store: String,
    pub filter: Vec<String>,
}

/// A key rule (`keyOf`): the name a project's directory takes for a path.
pub type KeyOf = fn(&str) -> String;

/// resolver.ts:32. Moves one agent's project state inside that agent's home. A TS optional member is a method with a
/// default here: `key` `None`, `listing` `None`, `shared` `None`, and `merge` absent where `has_merge` is false.
#[async_trait(?Send)]
pub trait ProjectStateResolver: Send + Sync {
    /// The catalog id of the agent whose state this moves.
    fn agent(&self) -> &'static str;
    /// The catalog projectState rows this module moves, in order.
    fn states(&self) -> Vec<&'static str>;
    /// The paths under home this module reads, relative to it, in order.
    fn roots(&self) -> Vec<&'static str>;
    fn carry(&self) -> ProjectCarry;
    /// The name this agent keys a project's state under, where its store takes such a word.
    fn key(&self, _path: &str) -> Option<String> {
        None
    }
    /// Applies the move; empty where nothing in this home was keyed to `from`.
    async fn move_state(&self, home: &str, from: &str, to: &str) -> Result<Vec<MovedState>>;
    /// How many sessions in this home ran at `path` or in a folder under it.
    async fn sessions(&self, home: &str, path: &str) -> Result<u64>;
    /// The files under home holding state for `path` alone, absolute.
    async fn entries(&self, home: &str, path: &str) -> Result<Vec<String>>;
    /// The listing steps for the machine; `None` where the module's whole roots are the answer.
    fn listing(&self, _home: &str, _path: &str) -> Option<Vec<String>> {
        None
    }
    fn shared(&self) -> Option<SharedStore> {
        None
    }
    fn has_merge(&self) -> bool {
        false
    }
    /// The merge script for the machine; `None` where this home holds no row for `from`.
    async fn merge(&self, _home: &str, _from: &str, _to: &str, _guest_home: &str) -> Result<Option<String>> {
        Ok(None)
    }
}

/// resolver.ts:68. node's `path.resolve` against the process's cwd, then the real path where it is a directory that
/// exists; the resolved spelling where it does not or is not a directory. Never fails.
pub fn resolve_project_path(path: &str) -> String {
    let abs = resolve_posix(path);
    match fs::metadata(&abs) {
        Ok(meta) if meta.is_dir() => fs::canonicalize(&abs).map_or(abs.clone(), |real| real.to_string_lossy().into_owned()),
        _ => abs,
    }
}

/// resolver.ts:79. `to` plus the tail after `from` where `under_project(path, from)`, else `None`.
pub fn moved_path(path: &str, from: &str, to: &str) -> Option<String> {
    under_project(path, from).then(|| format!("{to}{}", &path[from.len()..]))
}

/// resolver.ts:86. Not empty, not absolute, not `..`, and not opening with `../`; `..cache` is inside.
pub fn inside_folder(rel: &str) -> bool {
    !rel.is_empty() && !rel.starts_with('/') && rel != ".." && !rel.starts_with("../")
}

/// resolver.ts:94. `join(home, parts...)` where its path relative to home stays inside it, else `None`.
pub fn under_home(home: &str, parts: &[&str]) -> Option<String> {
    let mut all = vec![home];
    all.extend_from_slice(parts);
    let at = join_posix(&all);
    inside_folder(&relative_posix(home, &at)).then_some(at)
}

/// resolver.ts:100. A string moved when it is a path at or under `from`; any other value as it was.
pub fn moved_or(value: &Value, from: &str, to: &str) -> Value {
    value.as_str().and_then(|p| moved_path(p, from, to)).map_or_else(|| value.clone(), Value::String)
}

/// resolver.ts:103. The `cwd` string of the first JSON line carrying one, across the `.jsonl` files under dir in path
/// order; a line that is not JSON, or whose cwd is not a string, is passed over.
pub async fn recorded_cwd(dir: &str) -> Result<Option<String>> {
    for file in files_under(dir, ".jsonl")? {
        for line in read_text(&file)?.split('\n') {
            if let Some(cwd) = cwd_of(line) {
                return Ok(Some(cwd));
            }
        }
    }
    Ok(None)
}

/// resolver.ts:135 answer row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyedDirectory {
    pub dir: String,
    pub cwd: String,
}

/// resolver.ts:135. Rules in spec/project_state_files.md; nothing where `parent` does not exist.
pub async fn keyed_directories(parent: &str, key_of: KeyOf, from: &str) -> Result<Vec<KeyedDirectory>> {
    if !Path::new(parent).exists() {
        return Ok(vec![]);
    }
    let own = key_of(from);
    let stem = common_prefix(&own, &key_of(&format!("{from}/x")));
    let mut names: Vec<String> = dir_entries(parent)?.iter().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.retain(|n| n.starts_with(&stem));
    names.sort_by(|a, b| js_cmp(a, b));
    let mut out = Vec::new();
    for name in names {
        let dir = join_posix(&[parent, &name]);
        if !fs::metadata(&dir).map_err(|e| node_io(&e, "stat", &dir))?.is_dir() {
            continue;
        }
        let Some(cwd) = recorded_cwd(&dir).await?.or_else(|| (name == own).then(|| from.to_string())) else {
            continue;
        };
        if under_project(&cwd, from) {
            out.push(KeyedDirectory { dir, cwd });
        }
    }
    Ok(out)
}

/// resolver.ts:152 answer.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyedMove {
    pub files: Vec<String>,
    pub changed: u64,
}

/// resolver.ts:152. A destination that exists fails with `<moved> already exists`, directories renamed before it
/// staying renamed.
pub async fn move_keyed_directories(parent: &str, key_of: KeyOf, from: &str, to: &str) -> Result<KeyedMove> {
    let mut out = KeyedMove::default();
    for KeyedDirectory { dir, cwd } in keyed_directories(parent, key_of, from).await? {
        let Some(target) = moved_path(&cwd, from, to) else {
            continue;
        };
        let moved = join_posix(&[parent, &key_of(&target)]);
        if Path::new(&moved).exists() {
            return Err(EngineError::Other { message: format!("{moved} already exists"), code: None });
        }
        fs::rename(&dir, &moved).map_err(|e| node_io(&e, "rename", &dir))?;
        out.files.push(moved.clone());
        out.changed += 1;
        for f in files_under(&moved, ".jsonl")? {
            let n = rewrite_cwd(&f, from, to, None).await?;
            if n > 0 {
                out.files.push(f);
                out.changed += n;
            }
        }
    }
    Ok(out)
}

/// resolver.ts:176. The `.jsonl` regular files directly in each keyed directory.
pub async fn keyed_sessions(parent: &str, key_of: KeyOf, path: &str) -> Result<u64> {
    let mut n = 0;
    for KeyedDirectory { dir, .. } in keyed_directories(parent, key_of, path).await? {
        for entry in dir_entries(&dir)? {
            let is_file = entry.file_type().map_err(|e| node_io(&e, "scandir", &dir))?.is_file();
            n += u64::from(is_file && entry.file_name().to_string_lossy().ends_with(".jsonl"));
        }
    }
    Ok(n)
}

/// resolver.ts:183. Every regular file under each keyed directory, directory by directory.
pub async fn keyed_entries(parent: &str, key_of: KeyOf, path: &str) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for KeyedDirectory { dir, .. } in keyed_directories(parent, key_of, path).await? {
        out.extend(files_under(&dir, "")?);
    }
    Ok(out)
}

/// resolver.ts:195. Every regular file (not a link) under dir, at any depth, whose name ends with `ext` (empty: all),
/// sorted by path in UTF-16 code unit order; nothing where dir does not exist.
pub fn files_under(dir: &str, ext: &str) -> Result<Vec<String>> {
    if !Path::new(dir).exists() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    walk_files(dir, ext, &mut out)?;
    out.sort_by(|a, b| js_cmp(a, b));
    Ok(out)
}

/// resolver.ts:208. Rewrites the JSON lines `edit` answers true for, leaving every other byte alone; answers how many.
/// An edit that fails fails the rewrite with that failure, the file as it was. Rules (temp file, mode, a file with none
/// untouched, a failure leaving no temp file, the missing-file words) in spec/project_state_files.md.
pub async fn rewrite_jsonl(file: &str, edit: &mut dyn FnMut(&mut Map<String, Value>) -> Result<bool>) -> Result<u64> {
    let text = read_text(file)?;
    let tmp = format!("{file}.wsp-move");
    let parts: Vec<&str> = text.split('\n').collect();
    let last = parts.len() - 1;
    let mut out = String::with_capacity(text.len());
    let mut changed = 0;
    for (i, line) in parts.iter().enumerate() {
        if i == last && line.is_empty() {
            break;
        }
        let mut written = line.to_string();
        if let Some((Value::Object(mut map), truncated)) = js_read_kept(line) {
            if edit(&mut map)? {
                if truncated {
                    return Err(EngineError::Other { message: "Maximum call stack size exceeded".to_string(), code: None });
                }
                changed += 1;
                written = js_write(&Value::Object(map), None);
            }
        }
        out.push_str(&written);
        if i < last {
            out.push('\n');
        }
    }
    if changed == 0 {
        let _ = fs::remove_file(&tmp);
        return Ok(0);
    }
    let mode = fs::metadata(file).map_err(|e| node_io(&e, "stat", file))?.permissions().mode() & 0o7777;
    let landed = write_file(&tmp, out.as_bytes(), mode).and_then(|()| fs::rename(&tmp, file).map_err(|e| node_io(&e, "rename", file)));
    if landed.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    landed.map(|()| changed)
}

/// resolver.ts:258 `at`: picks the object holding `cwd` on a line.
pub type CwdHolder = fn(&mut Map<String, Value>) -> Option<&mut Map<String, Value>>;

/// resolver.ts:258. Rewrites `cwd` where it is `from` or under it, in the object `at` picks (the line itself when
/// `None`); a holder that is not an object, or a cwd that is not a string, is no change.
pub async fn rewrite_cwd(file: &str, from: &str, to: &str, at: Option<CwdHolder>) -> Result<u64> {
    rewrite_jsonl(file, &mut |line: &mut Map<String, Value>| {
        let holder = match at {
            Some(pick) => pick(line),
            None => Some(line),
        };
        let Some(holder) = holder else {
            return Ok(false);
        };
        let Some(moved) = holder.get("cwd").and_then(Value::as_str).and_then(|cwd| moved_path(cwd, from, to)) else {
            return Ok(false);
        };
        holder.insert("cwd".into(), Value::String(moved));
        Ok(true)
    })
    .await
}

/// node's `path.join` over the parts: empty parts drop, the rest joined with `/` and normalised; all empty is `.`.
pub(crate) fn join_posix(parts: &[&str]) -> String {
    let joined: Vec<&str> = parts.iter().copied().filter(|p| !p.is_empty()).collect();
    if joined.is_empty() {
        ".".into()
    } else {
        normalize(&joined.join("/"))
    }
}

/// How deep a value is kept: the JSON.parse port's cap, since serde_json clones past 1,180 levels and drops past 6,500; node reads deeper.
const KEPT_DEPTH: usize = 1_000;

/// `JSON.parse`: None where JS would throw, or where the value is nested past `KEPT_DEPTH`. A number overflow reads as
/// null, as JSON.stringify writes Infinity.
pub(crate) fn js_read(text: &str) -> Option<Value> {
    match js_read_kept(text)? {
        (value, false) => Some(value),
        (_, true) => None,
    }
}

/// `JSON.parse` of a line, with whether anything below `KEPT_DEPTH` was read for its syntax and left out (as null):
/// such a value reads as far as a caller looks at its top and cannot be written back.
pub(crate) fn js_read_kept(text: &str) -> Option<(Value, bool)> {
    let mut reader = JsReader { chars: text.chars().collect(), at: 0 };
    let read = reader.value()?;
    reader.skip_space();
    (reader.at == reader.chars.len()).then_some(read)
}

/// `JSON.stringify(value)`, or `JSON.stringify(value, null, indent)` with `Some(indent)`.
pub(crate) fn js_write(value: &Value, indent: Option<usize>) -> String {
    let mut out = String::new();
    write_value(value, indent, &mut out);
    out
}

// A string is held as its text with two escapes, so that a lone surrogate, which has no char, and every char there is
// stay told apart: a lone surrogate is `ESCAPE` and then the private-use char standing for it, and a real `ESCAPE` char is
// written twice. `write_string` reads them back.
const ESCAPE: char = '\u{10F7FF}';
const LONE_BASE: u32 = 0x10F800;

fn lone(unit: u16) -> char {
    char::from_u32(LONE_BASE + u32::from(unit - 0xD800)).unwrap_or(char::REPLACEMENT_CHARACTER)
}

struct JsReader {
    chars: Vec<char>,
    at: usize,
}

/// A container being read: what it holds so far and, for an object, the key its next value goes under. A `Skip` is one
/// below `KEPT_DEPTH`, which keeps nothing.
enum Frame {
    Array(Vec<Value>),
    Object(Map<String, Value>, String),
    SkipArray,
    SkipObject,
}

impl JsReader {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }

    fn eat(&mut self, want: char) -> bool {
        let hit = self.peek() == Some(want);
        if hit {
            self.at += 1;
        }
        hit
    }

    fn skip_space(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t' | '\n' | '\r')) {
            self.at += 1;
        }
    }

    /// One value, read without recursion so no depth can end the thread; the flag says whether a part below `KEPT_DEPTH`
    /// was left out.
    fn value(&mut self) -> Option<(Value, bool)> {
        let mut stack: Vec<Frame> = Vec::new();
        let mut truncated = false;
        loop {
            self.skip_space();
            let mut done = match self.peek()? {
                '{' => {
                    self.at += 1;
                    self.skip_space();
                    if self.eat('}') {
                        if stack.len() >= KEPT_DEPTH {
                            truncated = true;
                            Value::Null
                        } else {
                            Value::Object(Map::new())
                        }
                    } else if stack.len() >= KEPT_DEPTH || matches!(stack.last(), Some(Frame::SkipArray | Frame::SkipObject)) {
                        self.key()?;
                        stack.push(Frame::SkipObject);
                        continue;
                    } else {
                        let key = self.key()?;
                        stack.push(Frame::Object(Map::new(), key));
                        continue;
                    }
                }
                '[' => {
                    self.at += 1;
                    self.skip_space();
                    if self.eat(']') {
                        if stack.len() >= KEPT_DEPTH {
                            truncated = true;
                            Value::Null
                        } else {
                            Value::Array(Vec::new())
                        }
                    } else {
                        let kept = stack.len() < KEPT_DEPTH && !matches!(stack.last(), Some(Frame::SkipArray | Frame::SkipObject));
                        stack.push(if kept { Frame::Array(Vec::new()) } else { Frame::SkipArray });
                        continue;
                    }
                }
                '"' => Value::String(self.string()?),
                't' => self.word("true", Value::Bool(true))?,
                'f' => self.word("false", Value::Bool(false))?,
                'n' => self.word("null", Value::Null)?,
                '-' | '0'..='9' => self.number()?,
                _ => return None,
            };
            loop {
                match stack.last_mut() {
                    None => return Some((done, truncated)),
                    Some(Frame::Array(items)) => {
                        items.push(done);
                        self.skip_space();
                        if self.eat(',') {
                            break;
                        }
                        if !self.eat(']') {
                            return None;
                        }
                        let Some(Frame::Array(items)) = stack.pop() else { return None };
                        done = Value::Array(items);
                    }
                    Some(Frame::Object(map, key)) => {
                        map.insert(std::mem::take(key), done);
                        self.skip_space();
                        if self.eat(',') {
                            self.skip_space();
                            *key = self.key()?;
                            break;
                        }
                        if !self.eat('}') {
                            return None;
                        }
                        let Some(Frame::Object(map, _)) = stack.pop() else { return None };
                        done = Value::Object(map);
                    }
                    Some(skip @ (Frame::SkipArray | Frame::SkipObject)) => {
                        let object = matches!(skip, Frame::SkipObject);
                        self.skip_space();
                        if self.eat(',') {
                            if object {
                                self.skip_space();
                                self.key()?;
                            }
                            break;
                        }
                        if !self.eat(if object { '}' } else { ']' }) {
                            return None;
                        }
                        stack.pop();
                        if !matches!(stack.last(), Some(Frame::SkipArray | Frame::SkipObject)) {
                            truncated = true;
                            done = Value::Null;
                        } else {
                            done = Value::Null;
                        }
                    }
                }
            }
        }
    }

    /// An object's key and the colon after it.
    fn key(&mut self) -> Option<String> {
        self.skip_space();
        let key = self.string()?;
        self.skip_space();
        self.eat(':').then_some(key)
    }

    fn word(&mut self, text: &str, value: Value) -> Option<Value> {
        for want in text.chars() {
            if !self.eat(want) {
                return None;
            }
        }
        Some(value)
    }

    fn digits(&mut self) -> bool {
        let start = self.at;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.at += 1;
        }
        self.at > start
    }

    fn number(&mut self) -> Option<Value> {
        let start = self.at;
        self.eat('-');
        if !self.eat('0') && !self.digits() {
            return None;
        }
        if self.eat('.') && !self.digits() {
            return None;
        }
        if self.eat('e') || self.eat('E') {
            if !self.eat('+') {
                self.eat('-');
            }
            if !self.digits() {
                return None;
            }
        }
        let text: String = self.chars[start..self.at].iter().collect();
        let n: f64 = text.parse().ok()?;
        Some(Number::from_f64(n).map_or(Value::Null, Value::Number))
    }

    fn hex4(&mut self) -> Option<u16> {
        let mut unit = 0u32;
        for _ in 0..4 {
            unit = unit * 16 + self.peek()?.to_digit(16)?;
            self.at += 1;
        }
        u16::try_from(unit).ok()
    }

    /// What a `\u` unit adds to a string: the char it is, or a lone surrogate as its escape.
    fn push_unit(&mut self, unit: u16, out: &mut String) -> Option<()> {
        if (0xDC00..0xE000).contains(&unit) {
            out.extend([ESCAPE, lone(unit)]);
            return Some(());
        }
        if !(0xD800..0xDC00).contains(&unit) {
            push_char(char::from_u32(u32::from(unit))?, out);
            return Some(());
        }
        let mark = self.at;
        if self.eat('\\') && self.eat('u') {
            let low = self.hex4()?;
            if (0xDC00..0xE000).contains(&low) {
                let code = 0x10000 + ((u32::from(unit) - 0xD800) << 10) + (u32::from(low) - 0xDC00);
                push_char(char::from_u32(code)?, out);
                return Some(());
            }
        }
        self.at = mark;
        out.extend([ESCAPE, lone(unit)]);
        Some(())
    }

    fn string(&mut self) -> Option<String> {
        if !self.eat('"') {
            return None;
        }
        let mut out = String::new();
        loop {
            let c = self.peek()?;
            self.at += 1;
            match c {
                '"' => return Some(out),
                '\\' => {
                    let e = self.peek()?;
                    self.at += 1;
                    match e {
                        '"' | '\\' | '/' => out.push(e),
                        'b' => out.push('\u{8}'),
                        'f' => out.push('\u{c}'),
                        'n' => out.push('\n'),
                        'r' => out.push('\r'),
                        't' => out.push('\t'),
                        'u' => {
                            let unit = self.hex4()?;
                            self.push_unit(unit, &mut out)?;
                        }
                        _ => return None,
                    }
                }
                c if (c as u32) < 0x20 => return None,
                c => push_char(c, &mut out),
            }
        }
    }
}

/// A char into a held string: the `ESCAPE` char is told apart from the escape by being written twice.
fn push_char(c: char, out: &mut String) {
    out.push(c);
    if c == ESCAPE {
        out.push(c);
    }
}

/// What a container being written still has to write: its items, or an object's entries in key order.
enum Open<'a> {
    Array(std::slice::Iter<'a, Value>),
    Object(std::vec::IntoIter<(&'a String, &'a Value)>),
}

/// `JSON.stringify` from a stack of its own, so the depth of a value is no depth of the thread's stack.
fn write_value(value: &Value, indent: Option<usize>, out: &mut String) {
    let mut open: Vec<(Open, bool)> = Vec::new();
    let mut next = Some(value);
    loop {
        if let Some(value) = next.take() {
            match value {
                Value::Null => out.push_str("null"),
                Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
                Value::Number(n) => out.push_str(&number_string(n.as_f64().unwrap_or(f64::NAN))),
                Value::String(s) => write_string(s, out),
                Value::Array(items) if items.is_empty() => out.push_str("[]"),
                Value::Array(items) => {
                    out.push('[');
                    open.push((Open::Array(items.iter()), false));
                }
                Value::Object(map) if map.is_empty() => out.push_str("{}"),
                Value::Object(map) => {
                    out.push('{');
                    open.push((Open::Object(ordered(map).into_iter()), false));
                }
            }
        }
        let depth = open.len();
        let Some((frame, started)) = open.last_mut() else { return };
        let item = match frame {
            Open::Array(items) => items.next().map(|item| (None, item)),
            Open::Object(entries) => entries.next().map(|(key, item)| (Some(key), item)),
        };
        match item {
            Some((key, item)) => {
                if std::mem::replace(started, true) {
                    out.push(',');
                }
                newline(indent, depth, out);
                if let Some(key) = key {
                    write_string(key, out);
                    out.push(':');
                    if indent.is_some() {
                        out.push(' ');
                    }
                }
                next = Some(item);
            }
            None => {
                newline(indent, depth - 1, out);
                out.push(if matches!(frame, Open::Array(_)) { ']' } else { '}' });
                open.pop();
            }
        }
    }
}

fn newline(indent: Option<usize>, depth: usize, out: &mut String) {
    if let Some(width) = indent {
        out.push('\n');
        out.extend(std::iter::repeat_n(' ', width * depth));
    }
}

fn write_string(s: &str, out: &mut String) {
    out.push('"');
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            ESCAPE => match chars.next() {
                Some(next) if (LONE_BASE..LONE_BASE + 0x800).contains(&(next as u32)) => {
                    out.push_str(&format!("\\u{:04x}", next as u32 - LONE_BASE + 0xD800));
                }
                _ => out.push(ESCAPE),
            },
            c => out.push(c),
        }
    }
    out.push('"');
}

fn ordered(map: &Map<String, Value>) -> Vec<(&String, &Value)> {
    let mut indexed: Vec<(u64, &String, &Value)> = Vec::new();
    let mut rest = Vec::new();
    for (key, value) in map {
        match array_index(key) {
            Some(i) => indexed.push((i, key, value)),
            None => rest.push((key, value)),
        }
    }
    indexed.sort_by_key(|(i, _, _)| *i);
    indexed.into_iter().map(|(_, key, value)| (key, value)).chain(rest).collect()
}

fn array_index(key: &str) -> Option<u64> {
    let digits = !key.is_empty() && key.bytes().all(|b| b.is_ascii_digit()) && (key == "0" || !key.starts_with('0'));
    let i: u64 = if digits { key.parse().ok()? } else { return None };
    (i <= 4_294_967_294).then_some(i)
}

fn cwd_of(line: &str) -> Option<String> {
    match js_read_kept(line)?.0 {
        Value::Object(map) => map.get("cwd")?.as_str().map(str::to_string),
        _ => None,
    }
}

fn walk_files(dir: &str, ext: &str, out: &mut Vec<String>) -> Result<()> {
    let mut open = vec![(dir.to_string(), dir_entries(dir)?.into_iter())];
    while let Some((here, entries)) = open.last_mut() {
        let Some(entry) = entries.next() else {
            open.pop();
            continue;
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = join_posix(&[here, &name]);
        let kind = entry.file_type().map_err(|e| node_io(&e, "scandir", here))?;
        if kind.is_dir() || (kind.is_symlink() && fs::metadata(&path).is_ok_and(|m| m.is_dir())) {
            let inside = dir_entries(&path)?.into_iter();
            open.push((path, inside));
        } else if kind.is_file() && name.ends_with(ext) {
            out.push(path);
        }
    }
    Ok(())
}

fn dir_entries(dir: &str) -> Result<Vec<DirEntry>> {
    let entries = fs::read_dir(dir).map_err(|e| node_io(&e, "scandir", dir))?;
    entries.collect::<std::io::Result<Vec<_>>>().map_err(|e| node_io(&e, "scandir", dir))
}

pub(crate) fn read_bytes(file: &str) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(file).map_err(|e| node_io(&e, "open", file))?.read_to_end(&mut bytes).map_err(|e| node_io(&e, "read", file))?;
    Ok(bytes)
}

fn read_text(file: &str) -> Result<String> {
    Ok(String::from_utf8_lossy(&read_bytes(file)?).into_owned())
}

fn write_file(path: &str, bytes: &[u8], mode: u32) -> Result<()> {
    let mut file = File::create(path).map_err(|e| node_io(&e, "open", path))?;
    file.write_all(bytes).map_err(|e| node_io(&e, "write", path))?;
    fs::set_permissions(path, Permissions::from_mode(mode)).map_err(|e| node_io(&e, "chmod", path))
}

/// node's words for a failed file call: `<CODE>: <text>, <syscall> '<path>'`, a read naming no path.
pub(crate) fn node_io(e: &std::io::Error, syscall: &str, path: &str) -> EngineError {
    let (code, text) = match e.raw_os_error() {
        Some(1) => ("EPERM", "operation not permitted"),
        Some(2) => ("ENOENT", "no such file or directory"),
        Some(13) => ("EACCES", "permission denied"),
        Some(17) => ("EEXIST", "file already exists"),
        Some(18) => ("EXDEV", "cross-device link not permitted"),
        Some(20) => ("ENOTDIR", "not a directory"),
        Some(21) => ("EISDIR", "illegal operation on a directory"),
        Some(28) => ("ENOSPC", "no space left on device"),
        Some(30) => ("EROFS", "read-only file system"),
        Some(40) => ("ELOOP", "too many symbolic links encountered"),
        _ => return EngineError::Other { message: e.to_string(), code: None },
    };
    let at = if syscall == "read" { String::new() } else { format!(" '{path}'") };
    EngineError::Other { message: format!("{code}: {text}, {syscall}{at}"), code: Some(code.to_string()) }
}

fn cwd() -> String {
    std::env::current_dir().map_or_else(|_| "/".to_string(), |p| p.to_string_lossy().into_owned())
}

fn resolve_posix(path: &str) -> String {
    let full = if path.starts_with('/') { path.to_string() } else { format!("{}/{path}", cwd()) };
    let resolved = normalize(&full);
    if resolved.len() > 1 {
        resolved.trim_end_matches('/').to_string()
    } else {
        resolved
    }
}

fn relative_posix(from: &str, to: &str) -> String {
    let (from, to) = (resolve_posix(from), resolve_posix(to));
    if from == to {
        return String::new();
    }
    let from_parts: Vec<&str> = from.split('/').filter(|s| !s.is_empty()).collect();
    let to_parts: Vec<&str> = to.split('/').filter(|s| !s.is_empty()).collect();
    let common = from_parts.iter().zip(&to_parts).take_while(|(a, b)| a == b).count();
    let mut out: Vec<&str> = vec![".."; from_parts.len() - common];
    out.extend(&to_parts[common..]);
    out.join("/")
}

/// node's `path.normalize`: `.` and empty segments drop, `..` climbs where it can, a trailing slash is kept.
fn normalize(path: &str) -> String {
    if path.is_empty() {
        return ".".into();
    }
    let absolute = path.starts_with('/');
    let mut stack: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                if stack.last().is_some_and(|top| *top != "..") {
                    stack.pop();
                } else if !absolute {
                    stack.push("..");
                }
            }
            seg => stack.push(seg),
        }
    }
    let mut out = stack.join("/");
    if out.is_empty() && !absolute {
        out = ".".into();
    }
    if !out.is_empty() && path.ends_with('/') {
        out.push('/');
    }
    if absolute {
        format!("/{out}")
    } else {
        out
    }
}

fn common_prefix(a: &str, b: &str) -> String {
    a.chars().zip(b.chars()).take_while(|(x, y)| x == y).map(|(x, _)| x).collect()
}
