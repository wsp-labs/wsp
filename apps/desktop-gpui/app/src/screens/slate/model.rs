// SPDX-License-Identifier: AGPL-3.0-only
//! One thread's slate as the window holds it: the host's record from `slates.get`, its document read into pieces, the
//! live values the host pushes, and the formula texts the panel sends `slates.resolve` for. The window never evaluates
//! a formula: a prop that holds a binding or a format is turned back into the formula language's own text and the host
//! answers its value.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Map, Value};

/// The document schema this build draws (`SLATE_SCHEMA` in `protocol:slate/types.ts`).
pub const SCHEMA: u64 = 2;

/// What a type this build does not know draws instead.
#[derive(Clone, Debug, PartialEq)]
pub enum Fallback {
    None,
    Drop,
    Piece(String),
    Text(String),
}

#[derive(Clone, Debug)]
pub struct Piece {
    pub id: String,
    pub kind: String,
    pub props: Map<String, Value>,
    pub children: Vec<String>,
    pub when: Option<String>,
    pub fallback: Fallback,
    /// The handlers by event name: `press`, `submit`, `change`.
    pub on: Map<String, Value>,
}

impl Piece {
    fn read(id: &str, v: &Value) -> Option<Piece> {
        let o = v.as_object()?;
        let kind = o.get("type")?.as_str()?.to_owned();
        let fallback = match o.get("fallback") {
            Some(Value::String(s)) if s == "drop" => Fallback::Drop,
            Some(Value::String(s)) => Fallback::Piece(s.clone()),
            Some(Value::Object(f)) => Fallback::Text(f.get("text").and_then(Value::as_str).unwrap_or_default().to_owned()),
            _ => Fallback::None,
        };
        Some(Piece {
            id: id.to_owned(),
            kind,
            props: o.get("props").and_then(Value::as_object).cloned().unwrap_or_default(),
            children: o
                .get("children")
                .and_then(Value::as_array)
                .map(|c| c.iter().filter_map(|c| c.as_str().map(str::to_owned)).collect())
                .unwrap_or_default(),
            when: o.get("when").and_then(Value::as_str).map(str::to_owned),
            fallback,
            on: o.get("on").and_then(Value::as_object).cloned().unwrap_or_default(),
        })
    }

    pub fn prop(&self, name: &str) -> Option<&Value> {
        self.props.get(name)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Doc {
    pub title: Option<String>,
    pub root: String,
    pub pieces: HashMap<String, Piece>,
    /// Each declared value by name: `start`, `secret`, `type`.
    pub values: Map<String, Value>,
    /// Each run by name: `kind`, `cmd`, `every`, `on`, `server`, `tool`.
    pub runs: Map<String, Value>,
    pub derived: Map<String, Value>,
    /// Each reaction: `on` (`change` names or a `done` run) and the steps it `do`es.
    pub reactions: Vec<Value>,
    parents: HashMap<String, String>,
    loud: HashMap<Loud, String>,
}

/// The three things a slate has one of at most (`loudOf` in `engine.ts`): the first primary button, the first large
/// text, and the first piece in the accent, a chart naming no tone among them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Loud {
    Primary,
    Large,
    Accent,
}

impl Doc {
    pub fn read(v: &Value) -> Option<Doc> {
        let o = v.as_object()?;
        if o.get("schema").and_then(Value::as_u64) != Some(SCHEMA) {
            return None;
        }
        let root = o.get("root")?.as_str()?.to_owned();
        let mut pieces = HashMap::new();
        for (id, p) in o.get("pieces")?.as_object()? {
            pieces.insert(id.clone(), Piece::read(id, p)?);
        }
        let mut parents = HashMap::new();
        for p in pieces.values() {
            for c in &p.children {
                parents.insert(c.clone(), p.id.clone());
            }
        }
        let map = |k: &str| o.get(k).and_then(Value::as_object).cloned().unwrap_or_default();
        let mut doc = Doc {
            title: o.get("title").and_then(Value::as_str).map(str::to_owned),
            root,
            pieces,
            values: map("values"),
            runs: map("runs"),
            derived: map("derived"),
            reactions: o.get("reactions").and_then(Value::as_array).cloned().unwrap_or_default(),
            parents,
            loud: HashMap::new(),
        };
        doc.loud = doc.loud_pieces();
        Some(doc)
    }

    fn loud_pieces(&self) -> HashMap<Loud, String> {
        let mut out = HashMap::new();
        let mut seen = HashSet::new();
        let mut stack = vec![self.root.as_str()];
        while let Some(id) = stack.pop() {
            if !seen.insert(id) {
                continue;
            }
            let Some(p) = self.pieces.get(id) else { continue };
            let prop = |k: &str| p.props.get(k).and_then(Value::as_str);
            if prop("variant") == Some("primary") {
                out.entry(Loud::Primary).or_insert_with(|| p.id.clone());
            }
            if prop("size") == Some("large") {
                out.entry(Loud::Large).or_insert_with(|| p.id.clone());
            }
            let accent = prop("tone") == Some("accent") || (super::pieces::flags(&p.kind).accent && !p.props.contains_key("tone"));
            if accent {
                out.entry(Loud::Accent).or_insert_with(|| p.id.clone());
            }
            stack.extend(p.children.iter().rev().map(String::as_str));
        }
        out
    }

    /// Whether `id` is the slate's one piece of this loud kind.
    pub fn is_loud(&self, kind: Loud, id: &str) -> bool {
        self.loud.get(&kind).is_some_and(|l| l == id)
    }

    pub fn piece(&self, id: &str) -> Option<&Piece> {
        self.pieces.get(id)
    }

    pub fn parent(&self, id: &str) -> Option<&Piece> {
        self.parents.get(id).and_then(|p| self.pieces.get(p))
    }

    pub fn parent_id(&self, id: &str) -> Option<&str> {
        self.parents.get(id).map(String::as_str)
    }

    pub fn is_secret(&self, name: &str) -> bool {
        self.values.get(name).and_then(|d| d.get("secret")).and_then(Value::as_bool) == Some(true)
    }

    /// The `$name`s of the values declared secret, which Forget secrets blanks.
    pub fn secrets(&self) -> Vec<String> {
        self.values.iter().filter(|(_, d)| d.get("secret").and_then(Value::as_bool) == Some(true)).map(|(n, _)| format!("${n}")).collect()
    }

    /// The pieces drawn from the root down, each once, in order.
    pub fn drawn(&self) -> Vec<&Piece> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        let mut stack = vec![self.root.as_str()];
        while let Some(id) = stack.pop() {
            if !seen.insert(id) {
                continue;
            }
            let Some(p) = self.pieces.get(id) else { continue };
            out.push(p);
            if let Fallback::Piece(f) = &p.fallback {
                stack.push(f);
            }
            stack.extend(p.children.iter().rev().map(String::as_str));
        }
        out
    }
}

/// A held run's ask, as `slates.get` and `slates.event` carry it: a command or a tool the person allows or refuses.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Ask {
    pub key: String,
    pub run: String,
    pub kind: String,
    #[serde(default)]
    pub cmd: Option<String>,
    #[serde(default)]
    pub env: Map<String, Value>,
    #[serde(default)]
    pub args: Value,
    #[serde(default)]
    pub stdin: Option<String>,
    #[serde(default)]
    pub computer: Option<String>,
    #[serde(default)]
    pub folder: Option<String>,
    #[serde(default)]
    pub timeout_s: Option<f64>,
    #[serde(default)]
    pub confirm: Option<String>,
    #[serde(default)]
    pub then: Option<String>,
    #[serde(default)]
    pub files: Map<String, Value>,
    #[serde(default)]
    pub no_always: Option<bool>,
    #[serde(default)]
    pub why: Option<String>,
    #[serde(default)]
    pub server: Option<String>,
    #[serde(default)]
    pub tool: Option<String>,
    #[serde(default)]
    pub resource: Option<bool>,
    #[serde(default)]
    pub description: Option<String>,
}

/// One thread's record as the window draws it.
#[derive(Clone, Debug, Default)]
pub struct Record {
    pub thread_id: String,
    pub workspace_id: Option<String>,
    pub version: u64,
    pub revision: u64,
    pub doc: Option<Doc>,
    /// The schema of a document newer than this build, which draws the empty state instead.
    pub newer: Option<u64>,
    pub values: Map<String, Value>,
    /// `none`, `cleared` or `rewound-before`.
    pub empty: Option<String>,
    pub can_undo: bool,
    pub asks: Vec<Ask>,
    pub approvals: Map<String, Value>,
    pub shown_once: bool,
}

impl Record {
    /// The record of `slates.get`'s `slate`, None where the thread has none.
    pub fn read(slate: &Value) -> Option<Record> {
        let o = slate.as_object()?;
        let document = o.get("document").filter(|d| !d.is_null());
        let schema = document.and_then(|d| d.get("schema")).and_then(Value::as_u64);
        let newer = schema.filter(|s| *s > SCHEMA);
        Some(Record {
            thread_id: o.get("threadId").and_then(Value::as_str).unwrap_or_default().to_owned(),
            workspace_id: o.get("workspaceId").and_then(Value::as_str).map(str::to_owned),
            version: o.get("version").and_then(Value::as_u64).unwrap_or_default(),
            revision: o.get("revision").and_then(Value::as_u64).unwrap_or_default(),
            doc: if newer.is_some() { None } else { document.and_then(Doc::read) },
            newer,
            values: o.get("values").and_then(Value::as_object).cloned().unwrap_or_default(),
            empty: o.get("empty").and_then(Value::as_str).map(str::to_owned),
            can_undo: o.get("canUndo").and_then(Value::as_bool) == Some(true),
            asks: o
                .get("asks")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(|a| serde_json::from_value(a.clone()).ok()).collect())
                .unwrap_or_default(),
            approvals: o.get("approvals").and_then(Value::as_object).cloned().unwrap_or_default(),
            shown_once: o.get("shownOnce").and_then(Value::as_bool) == Some(true),
        })
    }

    /// A run's record, `{ state, runs, lines, refreshing, why, result, ... }`, as the values hold it.
    pub fn run(&self, name: &str) -> Option<&Map<String, Value>> {
        self.doc.as_ref()?.runs.get(name)?;
        self.values.get(name)?.as_object()
    }

    pub fn run_state(&self, name: &str) -> Option<&str> {
        self.run(name)?.get("state")?.as_str()
    }

    /// Started again, its last result still in its record until the new one lands.
    pub fn refreshing(&self, name: &str) -> bool {
        self.run(name).is_some_and(|r| {
            r.get("state").and_then(Value::as_str) == Some("running") && r.get("refreshing").and_then(Value::as_bool) == Some(true)
        })
    }

    /// Writes one pushed value by its own path: `$check`, `$steps[2].done`, `$hello.lines`.
    pub fn set_own(&mut self, path: &str, value: Value) -> bool {
        let Some((name, steps)) = own_path(path) else { return false };
        if steps.is_empty() {
            self.values.insert(name, value);
            return true;
        }
        let mut at = self.values.entry(name).or_insert(Value::Null);
        for step in &steps {
            at = match step {
                Step::Key(k) => {
                    if !at.is_object() {
                        *at = Value::Object(Map::new());
                    }
                    at.as_object_mut().expect("an object").entry(k.clone()).or_insert(Value::Null)
                }
                Step::Index(i) => {
                    if !at.is_array() {
                        *at = Value::Array(Vec::new());
                    }
                    let list = at.as_array_mut().expect("a list");
                    if list.len() <= *i {
                        list.resize(*i + 1, Value::Null);
                    }
                    &mut list[*i]
                }
            };
        }
        *at = value;
        true
    }

    /// The value at an own path, or None where it is not there.
    pub fn get_own(&self, path: &str) -> Option<&Value> {
        let (name, steps) = own_path(path)?;
        let mut at = self.values.get(&name)?;
        for step in steps {
            at = match step {
                Step::Key(k) => at.get(k.as_str())?,
                Step::Index(i) => at.get(i)?,
            };
        }
        Some(at)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Key(String),
    Index(usize),
}

/// `$name.key[2]` as its name and steps; None for anything not an own path.
pub fn own_path(path: &str) -> Option<(String, Vec<Step>)> {
    let rest = path.strip_prefix('$')?;
    let end = rest.find(['.', '[']).unwrap_or(rest.len());
    let name = &rest[..end];
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return None;
    }
    let mut steps = Vec::new();
    let mut s = &rest[end..];
    while !s.is_empty() {
        if let Some(r) = s.strip_prefix('.') {
            let e = r.find(['.', '[']).unwrap_or(r.len());
            steps.push(Step::Key(r[..e].to_owned()));
            s = &r[e..];
        } else {
            let r = s.strip_prefix('[')?;
            let e = r.find(']')?;
            steps.push(Step::Index(r[..e].trim().parse().ok()?));
            s = &r[e + 1..];
        }
    }
    Some((name.to_owned(), steps))
}

// ---- formula texts for slates.resolve ----

/// Whether a prop value holds a binding or a format anywhere in it, so its value comes from the host.
pub fn is_live(v: &Value) -> bool {
    match v {
        Value::Object(o) if is_binding(o) || is_format(o) => true,
        Value::Object(o) => o.iter().any(|(k, v)| k != "on" && is_live(v)),
        Value::Array(a) => a.iter().any(is_live),
        _ => false,
    }
}

/// Whether a row-scoped prop reads anything per row: a binding, a format, or an entry's own `when`.
fn row_live(v: &Value) -> bool {
    match v {
        Value::Object(o) if is_binding(o) || is_format(o) => true,
        Value::Object(o) => o.iter().any(|(k, v)| (k == "when" && v.is_string()) || (k != "on" && row_live(v))),
        Value::Array(a) => a.iter().any(row_live),
        _ => false,
    }
}

fn is_binding(o: &Map<String, Value>) -> bool {
    o.len() == 1 && o.get("bind").is_some_and(Value::is_string)
}

fn is_format(o: &Map<String, Value>) -> bool {
    o.len() == 1 && o.get("format").is_some_and(Value::is_string)
}

/// Each binding and format in a prop value, as the formula the host answers for it, a format past the cap as one per
/// hole; a handler (`on`) is the host's.
pub fn leaves(v: &Value) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::Object(o) if is_format(o) && long_format(&o["format"]).is_some() => {
                let parts = long_format(&o["format"]).unwrap_or_default();
                out.extend(parts.into_iter().filter_map(|p| match p {
                    Part::Hole(h) => Some(h),
                    Part::Words(_) => None,
                }));
            }
            Value::Object(o) if is_binding(o) || is_format(o) => out.push(expr_of(v)),
            Value::Object(o) => o.iter().filter(|(k, _)| *k != "on").for_each(|(_, v)| walk(v, out)),
            Value::Array(a) => a.iter().for_each(|v| walk(v, out)),
            _ => {}
        }
    }
    walk(v, &mut out);
    out
}

/// A prop value with each binding and format replaced by the host's answer to it; None until every one is answered.
/// A list entry that answered nothing reads null, as `resolveProp` writes it.
pub fn filled(v: &Value, resolved: &HashMap<String, Value>) -> Option<Value> {
    match v {
        Value::Object(o) if is_format(o) => match long_format(&o["format"]) {
            Some(parts) => joined(parts.iter().map(|p| match p {
                Part::Words(w) => Some(Ok(w.as_str())),
                Part::Hole(h) => resolved.get(h).cloned().map(Err),
            })),
            None => resolved.get(&expr_of(v)).cloned(),
        },
        Value::Object(o) if is_binding(o) => resolved.get(&expr_of(v)).cloned(),
        Value::Object(o) => o
            .iter()
            .map(|(k, x)| Some((k.clone(), if k == "on" { x.clone() } else { filled(x, resolved)? })))
            .collect::<Option<Map<_, _>>>()
            .map(Value::Object),
        Value::Array(a) => a.iter().map(|x| filled(x, resolved)).collect::<Option<Vec<_>>>().map(Value::Array),
        other => Some(other.clone()),
    }
}

/// A JSON value as the formula language writes it: strings quoted with JSON's escapes, which the language reads.
pub fn literal(v: &Value) -> String {
    match v {
        Value::Array(a) => format!("[{}]", a.iter().map(literal).collect::<Vec<_>>().join(", ")),
        Value::Object(o) => {
            format!("{{{}}}", o.iter().map(|(k, v)| format!("{}: {}", Value::String(k.clone()), literal(v))).collect::<Vec<_>>().join(", "))
        }
        // The language reads an exponent without its plus sign: `1e21`, not serde's `1e+21`.
        Value::Number(n) => n.to_string().replace("e+", "e"),
        other => other.to_string(),
    }
}

/// A format string as a template: its `${...}` holes as written, cut where `parseSlateFormat` cuts them, a backquote in
/// its words escaped, and a backslash ending it written as a hole, since the closing backquote would read it as an escape.
fn template(src: &str) -> String {
    let b = src.as_bytes();
    let mut out = String::from("`");
    let (mut i, mut words) = (0, 0);
    while i < b.len() {
        if b[i..].starts_with(b"$${") {
            i += 3;
        } else if let Some(end) = b[i..].starts_with(b"${").then(|| closing_brace(b, i + 2)).flatten() {
            out.push_str(&src[words..i].replace('`', "\\`"));
            out.push_str(&src[i..=end]);
            i = end + 1;
            words = i;
        } else {
            i += 1;
        }
    }
    let rest = src[words..].replace('`', "\\`");
    match rest.strip_suffix('\\') {
        Some(head) => out.push_str(&format!("{head}${{'\\\\'}}")),
        None => out.push_str(&rest),
    }
    out.push('`');
    out
}

/// One piece of a format too long to ask whole: words the window draws as written, or one hole asked alone.
#[derive(Clone, Debug, PartialEq)]
enum Part {
    Words(String),
    Hole(String),
}

/// A format whose template passes the host's cap, cut where `parseSlateFormat` cuts it, each hole a template of its own
/// that the host answers as the hole's text, or null where it reads nothing; None for a format that fits.
fn long_format(format: &Value) -> Option<Vec<Part>> {
    let src = format.as_str()?;
    if template(src).len() <= EXPR_CHARS {
        return None;
    }
    let b = src.as_bytes();
    let (mut parts, mut words, mut i, mut from) = (Vec::new(), String::new(), 0, 0);
    while i < b.len() {
        if b[i..].starts_with(b"$${") {
            words.push_str(&src[from..i]);
            words.push_str("${");
            i += 3;
            from = i;
        } else if let Some(end) = b[i..].starts_with(b"${").then(|| closing_brace(b, i + 2)).flatten() {
            words.push_str(&src[from..i]);
            if !words.is_empty() {
                parts.push(Part::Words(std::mem::take(&mut words)));
            }
            parts.push(Part::Hole(format!("`{}`", &src[i..=end])));
            i = end + 1;
            from = i;
        } else {
            i += 1;
        }
    }
    words.push_str(&src[from..]);
    if !words.is_empty() {
        parts.push(Part::Words(words));
    }
    Some(parts)
}

/// Words and the host's answers to holes as `joined` in `protocol:slate/expr.ts` joins them: null where every hole read
/// nothing, an unanswered hole leaving the whole unanswered (an `Err` is a hole's answer).
fn joined<'a>(parts: impl Iterator<Item = Option<Result<&'a str, Value>>>) -> Option<Value> {
    let (mut out, mut holes, mut read) = (String::new(), false, false);
    for part in parts {
        match part? {
            Ok(words) => out.push_str(words),
            Err(answer) => {
                holes = true;
                match answer {
                    Value::Null => {}
                    Value::String(s) => {
                        read = true;
                        out.push_str(&s);
                    }
                    other => {
                        read = true;
                        out.push_str(&other.to_string());
                    }
                }
            }
        }
    }
    Some(if holes && !read { Value::Null } else { Value::String(out) })
}

/// A binding as a formula of its own, in parentheses unless they would take it past the host's cap: every place the host
/// parses a formula takes a whole pipeline, so they only keep the text apart for the eye.
pub fn paren(text: &str) -> String {
    if text.len() + 2 > EXPR_CHARS {
        text.to_owned()
    } else {
        format!("({text})")
    }
}

/// A prop value written as one formula: a literal as itself, a binding in parentheses, a format as a template, lists
/// and records through.
pub fn expr_of(v: &Value) -> String {
    match v {
        Value::Object(o) if is_binding(o) => paren(o["bind"].as_str().unwrap_or_default()),
        Value::Object(o) if is_format(o) => template(o["format"].as_str().unwrap_or_default()),
        Value::Array(a) => format!("[{}]", a.iter().map(expr_of).collect::<Vec<_>>().join(", ")),
        Value::Object(o) => format!(
            "{{{}}}",
            o.iter()
                .map(|(k, v)| format!("{}: {}", Value::String(k.clone()), if k == "on" { literal(v) } else { expr_of(v) }))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        other => literal(other),
    }
}

/// The field a row's own formula is answered under in a `map` step; no slate value is spelled like it.
fn row_field(k: usize) -> String {
    format!("_slate{k}")
}

/// The props a piece type reads per row of its items rather than whole (`rowScoped` in the web's piece views).
pub fn row_scoped(kind: &str) -> &'static [&'static str] {
    match kind {
        "timeline" => &["name", "start", "end", "state", "group"],
        "donut" => &["name", "value"],
        "bars" => &["name", "value", "tone", "key"],
        "images" => &["src", "caption"],
        "treemap" => &["name", "value", "group"],
        "chart" => &["x", "value", "series"],
        "checklist" => &["key", "title", "done", "state", "note"],
        "output" => &["run"],
        "table" => &["columns", "rowActions", "key"],
        _ => &[],
    }
}

/// One piece's per-row props as formulas over its items, each under the host's 500-character cap (`exprChars`): the
/// items themselves, and `(items) | map(_slate0: ..., _slate1: ...)` with each binding or format of the row-scoped
/// props one named field, as many fields to a formula as fit. Items the document wrote out are asked a run of elements
/// at a time, `[e0, e1, ...] | map(...)`, each run's fields reading `index` from where the run starts ([`shifted`]):
/// the host takes no row scope from outside a formula.
pub struct Rows {
    /// The items formula, or one per written element.
    items: Items,
    /// Each field formula and the field numbers it answers, per items formula.
    fields: Vec<Vec<(String, Vec<usize>)>>,
    /// The row-scoped props with each live leaf replaced by its field's number.
    shapes: Vec<(String, Shape)>,
}

enum Items {
    Whole(String),
    Each(Vec<Run>),
}

/// A run of written elements asked as one list, how many it holds, and whether it is one element too long for the list's
/// brackets, asked bare.
struct Run {
    formula: String,
    count: usize,
    bare: bool,
}

/// The host's cap on one formula's length (`SLATE_LIMITS.exprChars`).
pub const EXPR_CHARS: usize = 500;

#[derive(Clone, Debug)]
enum Shape {
    Lit(Value),
    Leaf(usize),
    /// A format past the cap: its words, and each hole's field.
    Joined(Vec<Result<String, usize>>),
    List(Vec<Shape>),
    Rec(Vec<(String, Shape)>),
}

fn shape_of(v: &Value, leaves: &mut Vec<String>) -> Shape {
    match v {
        Value::Object(o) if is_format(o) && long_format(&o["format"]).is_some() => Shape::Joined(
            long_format(&o["format"])
                .unwrap_or_default()
                .into_iter()
                .map(|part| match part {
                    Part::Words(w) => Ok(w),
                    Part::Hole(h) => {
                        leaves.push(h);
                        Err(leaves.len() - 1)
                    }
                })
                .collect(),
        ),
        Value::Object(o) if is_binding(o) || is_format(o) => {
            leaves.push(expr_of(v));
            Shape::Leaf(leaves.len() - 1)
        }
        Value::Array(a) if row_live(v) => Shape::List(a.iter().map(|x| shape_of(x, leaves)).collect()),
        Value::Object(o) if row_live(v) => Shape::Rec(
            o.iter()
                .map(|(k, x)| match (k.as_str(), x) {
                    ("on", _) => (k.clone(), Shape::Lit(x.clone())),
                    ("when", Value::String(when)) => {
                        leaves.push(format!("({when})"));
                        (k.clone(), Shape::Leaf(leaves.len() - 1))
                    }
                    _ => (k.clone(), shape_of(x, leaves)),
                })
                .collect(),
        ),
        other => Shape::Lit(other.clone()),
    }
}

fn fill(shape: &Shape, row: &Map<String, Value>) -> Value {
    match shape {
        Shape::Lit(v) => v.clone(),
        Shape::Leaf(k) => row.get(&row_field(*k)).cloned().unwrap_or(Value::Null),
        Shape::Joined(parts) => joined(parts.iter().map(|part| {
            Some(match part {
                Ok(words) => Ok(words.as_str()),
                Err(k) => Err(row.get(&row_field(*k)).cloned().unwrap_or(Value::Null)),
            })
        }))
        .unwrap_or(Value::Null),
        Shape::List(l) => Value::Array(l.iter().map(|s| fill(s, row)).collect()),
        Shape::Rec(r) => Value::Object(r.iter().map(|(k, s)| (k.clone(), fill(s, row))).collect()),
    }
}

impl Rows {
    pub fn of(piece: &Piece) -> Option<Rows> {
        let scoped = row_scoped(&piece.kind);
        if scoped.is_empty() {
            return None;
        }
        let items = piece.prop("items")?;
        let mut leaves = Vec::new();
        let shapes: Vec<(String, Shape)> =
            scoped.iter().filter_map(|name| piece.prop(name).map(|v| (name.to_string(), shape_of(v, &mut leaves)))).collect();
        let (items, starts) = match items {
            Value::Array(list) => {
                // Room for the longest field beside a chunk, so each field fits with every chunk.
                let widest = leaves
                    .iter()
                    .enumerate()
                    .map(|(k, e)| format!(" | map({}: {})", row_field(k), shifted(e, list.len())).len())
                    .max()
                    .unwrap_or(0);
                let runs = chunks(list.iter().map(expr_of), EXPR_CHARS.saturating_sub(widest).max(EXPR_CHARS / 5));
                let starts = runs.iter().map(|(_, from)| *from).collect();
                (Items::Each(runs.into_iter().map(|(run, _)| run).collect()), starts)
            }
            other => (Items::Whole(expr_of(other)), vec![0]),
        };
        let heads: Vec<Option<&String>> = match &items {
            Items::Whole(e) => vec![Some(e)],
            Items::Each(each) => each.iter().map(|run| (!run.bare).then_some(&run.formula)).collect(),
        };
        let fields = heads
            .iter()
            .zip(starts)
            .map(|(head, from)| match head {
                Some(head) => packed(head, &leaves.iter().map(|e| shifted(e, from)).collect::<Vec<_>>()),
                None => Vec::new(),
            })
            .collect();
        Some(Rows { items, fields, shapes })
    }

    /// Every formula the rows need answered.
    pub fn formulas(&self) -> Vec<String> {
        let heads: Vec<String> = match &self.items {
            Items::Whole(e) => vec![e.clone()],
            Items::Each(each) => each.iter().map(|run| run.formula.clone()).collect(),
        };
        heads.into_iter().chain(self.fields.iter().flatten().map(|(f, _)| f.clone())).collect()
    }

    /// Each row as the piece reads it, once the host has answered every formula: `item` the element as the items
    /// hold it, `props` its row-scoped props. A field formula the host left unanswered reads null.
    pub fn read(&self, resolved: &HashMap<String, Value>) -> Option<Vec<Row>> {
        let empty = Map::new();
        let mut rows: Vec<(Value, Map<String, Value>)> = Vec::new();
        let groups: Vec<(&String, Option<&Run>)> = match &self.items {
            Items::Whole(e) => vec![(e, None)],
            Items::Each(each) => each.iter().map(|run| (&run.formula, Some(run))).collect(),
        };
        for (g, (head, run)) in groups.iter().enumerate() {
            let answer = resolved.get(*head)?;
            // A run the host could not answer keeps its rows, so every later row keeps its index and its scope.
            let list = match (answer, run) {
                (answer, Some(run)) if run.bare => vec![answer.clone()],
                (Value::Array(list), _) => list.clone(),
                (_, Some(run)) => vec![Value::Null; run.count],
                (_, None) => Vec::new(),
            };
            let first = rows.len();
            rows.extend(list.into_iter().map(|item| (item, Map::new())));
            for (formula, _) in &self.fields[g] {
                let Some(Value::Array(answers)) = resolved.get(formula) else { continue };
                for (at, answer) in answers.iter().enumerate() {
                    if let Some((_, fields)) = rows.get_mut(first + at) {
                        for (k, v) in answer.as_object().unwrap_or(&empty) {
                            if k.starts_with("_slate") {
                                fields.insert(k.clone(), v.clone());
                            }
                        }
                    }
                }
            }
        }
        Some(
            rows.into_iter()
                .enumerate()
                .map(|(index, (item, fields))| Row {
                    index,
                    item,
                    props: self.shapes.iter().map(|(name, s)| (name.clone(), fill(s, &fields))).collect(),
                })
                .collect(),
        )
    }
}

/// Written-out elements as list formulas of at most `room` characters each, in order, each with the element it starts
/// at; an element longer than that stands alone, bare where the brackets would take it past the host's cap.
fn chunks(elements: impl Iterator<Item = String>, room: usize) -> Vec<(Run, usize)> {
    let mut out = Vec::new();
    let mut chunk: Vec<String> = Vec::new();
    let (mut len, mut from) = (2, 0);
    let run = |chunk: &mut Vec<String>| {
        let count = chunk.len();
        let formula = format!("[{}]", chunk.join(", "));
        if count == 1 && formula.len() > EXPR_CHARS {
            Run { formula: chunk.pop().unwrap_or_default(), count, bare: true }
        } else {
            chunk.clear();
            Run { formula, count, bare: false }
        }
    };
    for (at, e) in elements.enumerate() {
        if !chunk.is_empty() && len + e.len() + 2 > room {
            out.push((run(&mut chunk), from));
            chunk.clear();
            len = 2;
            from = at;
        }
        len += e.len() + 2;
        chunk.push(e);
    }
    if !chunk.is_empty() {
        out.push((run(&mut chunk), from));
    }
    out
}

/// A row formula as a run starting at element `from` reads it: each `index` the row scope binds becomes `(index + from)`,
/// found by the host's own tokens and scopes (`protocol:slate/expr.ts`); text the host refuses stays as written.
pub fn shifted(formula: &str, from: usize) -> String {
    let mut reads = Vec::new();
    if from == 0 || !formula.contains("index") || index_reads(formula, 0, true, &mut reads).is_none() {
        return formula.to_owned();
    }
    let mut out = String::with_capacity(formula.len() + reads.len() * 12);
    let mut last = 0;
    for at in reads {
        out.push_str(&formula[last..at]);
        out.push_str(&format!("(index + {from})"));
        last = at + "index".len();
    }
    out.push_str(&formula[last..]);
    out
}

#[derive(Clone, Copy, PartialEq)]
enum Lex<'a> {
    Name(&'a str),
    /// One of `()[]{},.:|`.
    Mark(u8),
    Other,
}

struct Token<'a> {
    at: usize,
    lex: Lex<'a>,
    /// A template's `${...}` holes, as byte ranges of the text.
    holes: Vec<(usize, usize)>,
}

/// `src` as the host's `tokenize` cuts it, or None where it throws.
fn tokens(src: &str) -> Option<Vec<Token<'_>>> {
    let b = src.as_bytes();
    let word = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let digits = |mut j: usize| {
        while b.get(j).is_some_and(u8::is_ascii_digit) {
            j += 1;
        }
        j
    };
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let (c, at) = (b[i], i);
        let mut holes = Vec::new();
        let lex = match c {
            b' ' | b'\t' | b'\n' | b'\r' => {
                i += 1;
                continue;
            }
            b'0'..=b'9' => {
                i = digits(i);
                if b.get(i) == Some(&b'.') && b.get(i + 1).is_some_and(u8::is_ascii_digit) {
                    i = digits(i + 1);
                }
                let exp = if b.get(i + 1) == Some(&b'-') { i + 2 } else { i + 1 };
                if matches!(b.get(i), Some(b'e' | b'E')) && b.get(exp).is_some_and(u8::is_ascii_digit) {
                    i = digits(exp);
                }
                Lex::Other
            }
            b'\'' | b'"' => {
                i += 1;
                while i < b.len() && b[i] != c {
                    let hex = b.get(i + 1) == Some(&b'u') && b.get(i + 2..i + 6).is_some_and(|h| h.iter().all(u8::is_ascii_hexdigit));
                    i += match (b[i] == b'\\' && i + 1 < b.len(), hex) {
                        (true, true) => 6,
                        (true, false) => 2,
                        _ => 1,
                    };
                }
                i = i.checked_add(1).filter(|&end| end <= b.len())?;
                Lex::Other
            }
            b'`' => {
                i += 1;
                while i < b.len() && b[i] != b'`' {
                    if b[i..].starts_with(b"$${") {
                        i += 3;
                    } else if b[i..].starts_with(b"\\`") {
                        i += 2;
                    } else if b[i..].starts_with(b"${") {
                        let end = closing_brace(b, i + 2)?;
                        holes.push((i + 2, end));
                        i = end + 1;
                    } else {
                        i += 1;
                    }
                }
                i = i.checked_add(1).filter(|&end| end <= b.len())?;
                Lex::Other
            }
            b'$' => {
                if !b.get(i + 1).is_some_and(|&n| n.is_ascii_alphabetic() || n == b'_') {
                    return None;
                }
                i += 1;
                while b.get(i).is_some_and(|&n| word(n)) {
                    i += 1;
                }
                Lex::Other
            }
            c if c.is_ascii_alphabetic() || c == b'_' => {
                while b.get(i).is_some_and(|&n| word(n)) {
                    i += 1;
                }
                Lex::Name(&src[at..i])
            }
            _ => {
                let rest = &b[i..];
                if [&b"..."[..], b"=>", b"??", b"?."].iter().any(|refused| rest.starts_with(refused)) {
                    return None;
                }
                let long = [&b"==="[..], b"!==", b"==", b"!=", b"<=", b">=", b"&&", b"||"].into_iter().find(|op| rest.starts_with(op));
                if let Some(op) = long {
                    i += op.len();
                    Lex::Other
                } else if b"()[]{},.:|".contains(&c) {
                    i += 1;
                    Lex::Mark(c)
                } else if b"!?+-*/%<>".contains(&c) {
                    i += 1;
                    Lex::Other
                } else {
                    return None;
                }
            }
        };
        out.push(Token { at, lex, holes });
    }
    Some(out)
}

/// `closingBrace`: the `}` closing a `${` whose body starts at `start`, quotes and nested braces skipped.
fn closing_brace(b: &[u8], start: usize) -> Option<usize> {
    let (mut depth, mut quote, mut j) = (1, None, start);
    while j < b.len() {
        match (quote, b[j]) {
            (Some(_), b'\\') => j += 1,
            (Some(q), ch) if ch == q => quote = None,
            (Some(_), _) => {}
            (None, ch @ (b'\'' | b'"' | b'`')) => quote = Some(ch),
            (None, b'{') => depth += 1,
            (None, b'}') => {
                depth -= 1;
                if depth == 0 {
                    return Some(j);
                }
            }
            _ => {}
        }
        j += 1;
    }
    None
}

/// The step arguments the host reads where the step stands rather than per element (`run(x.arg(n), x.env)` in the
/// `PIPE` table): `sortBy`'s order, `join`'s other list, and the counts of `take` and `skip`.
fn read_outside(step: &str, arg: usize) -> bool {
    matches!((step, arg), ("sortBy", 1) | ("join", 0) | ("take", 0) | ("skip", 0))
}

/// Where `src`, which starts at `base` in the whole formula, reads the row scope's `index`.
fn index_reads(src: &str, base: usize, row: bool, out: &mut Vec<usize>) -> Option<()> {
    struct Open<'a> {
        around: bool,
        step: Option<&'a str>,
        arg: usize,
        row: bool,
    }
    let toks = tokens(src)?;
    let mut open: Vec<Open> = Vec::new();
    for (k, t) in toks.iter().enumerate() {
        let here = open.last().map_or(row, |o| o.row);
        let lex = |back: usize| k.checked_sub(back).map(|p| toks[p].lex);
        match t.lex {
            Lex::Mark(b'(') => {
                let step = match (lex(2), lex(1)) {
                    (Some(Lex::Mark(b'|')), Some(Lex::Name(name))) => Some(name),
                    _ => None,
                };
                open.push(Open { around: here, step, arg: 0, row: here && step.is_none_or(|s| read_outside(s, 0)) });
            }
            Lex::Mark(b'[' | b'{') => open.push(Open { around: here, step: None, arg: 0, row: here }),
            Lex::Mark(b')' | b']' | b'}') => {
                open.pop();
            }
            Lex::Mark(b',') => {
                if let Some(o) = open.last_mut() {
                    if let Some(step) = o.step {
                        o.arg += 1;
                        o.row = o.around && read_outside(step, o.arg);
                    }
                }
            }
            Lex::Name("index") if here => {
                let next = toks.get(k + 1).map(|t| t.lex);
                let field_or_step = matches!(lex(1), Some(Lex::Mark(b'.' | b'|')));
                let key = next == Some(Lex::Mark(b':')) && matches!(lex(1), Some(Lex::Mark(b'{' | b',' | b'(')));
                if !field_or_step && !key && next != Some(Lex::Mark(b'(')) {
                    out.push(base + t.at);
                }
            }
            _ => {}
        }
        for &(start, end) in &t.holes {
            index_reads(&src[start..end], base + start, here, out)?;
        }
    }
    Some(())
}

/// `head | map(...)` formulas carrying every leaf, as many to a formula as stay under the cap; a leaf that alone does
/// not fit is left out and reads null.
fn packed(head: &str, leaves: &[String]) -> Vec<(String, Vec<usize>)> {
    let formula = |fields: &[usize]| {
        let named = fields.iter().map(|k| format!("{}: {}", row_field(*k), leaves[*k])).collect::<Vec<_>>().join(", ");
        format!("{head} | map({named})")
    };
    let mut out = Vec::new();
    let mut group: Vec<usize> = Vec::new();
    for k in 0..leaves.len() {
        group.push(k);
        if formula(&group).len() > EXPR_CHARS {
            group.pop();
            if !group.is_empty() {
                out.push((formula(&group), std::mem::take(&mut group)));
            }
            if formula(&[k]).len() <= EXPR_CHARS {
                group.push(k);
            }
        }
    }
    if !group.is_empty() {
        out.push((formula(&group), group));
    }
    out
}

/// One row of a repeating piece.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub index: usize,
    pub item: Value,
    pub props: Map<String, Value>,
}

/// The clock reads a formula can make: `time.now`, and the functions `protocol:slate/expr.ts` marks `clock: true`.
pub const CLOCK_FUNCTIONS: [&str; 2] = ["ago", "until"];

/// Whether a formula text reads the clock, so the panel asks again each second while it is drawn. A word scan over
/// the formula's code, its strings and template words left out.
pub fn reads_clock(expr: &str) -> bool {
    let expr = &code_of(expr);
    if expr.contains("time.now") {
        return true;
    }
    CLOCK_FUNCTIONS.iter().any(|f| {
        expr.match_indices(f).any(|(at, _)| {
            let before = expr[..at].chars().next_back();
            let after = expr[at + f.len()..].trim_start().starts_with('(');
            after && !before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '$')
        })
    })
}

/// The sources the host answers off what moves outside the slate (`SLATE_SOURCES` besides `time`): a push that moves
/// one asks its formulas again.
pub const SOURCES: [&str; 5] = ["git", "pr", "thread", "usage", "cost"];

/// The sources the host reads more often while a window holds them (`held: true` in the web's sources).
pub const HELD_SOURCES: [&str; 2] = ["git", "pr"];

/// A formula text with every string literal and every template's words outside its `${}` holes blanked, as the host's
/// tokenizer reads them as text, so a word scan sees only names.
pub fn code_of(expr: &str) -> String {
    enum At {
        Code(usize),
        Template,
    }
    let mut stack = vec![At::Code(0)];
    let mut out = String::with_capacity(expr.len());
    let mut chars = expr.chars().peekable();
    while let Some(c) = chars.next() {
        let nested = stack.len() > 1;
        match stack.last_mut() {
            Some(At::Template) => match c {
                '\\' => {
                    chars.next();
                    out.push(' ');
                }
                '`' => {
                    stack.pop();
                    out.push(' ');
                }
                '$' if chars.peek() == Some(&'{') => {
                    chars.next();
                    stack.push(At::Code(0));
                    out.push(' ');
                }
                _ => out.push(' '),
            },
            Some(At::Code(braces)) => match c {
                '\'' | '"' => {
                    while let Some(d) = chars.next() {
                        if d == '\\' {
                            chars.next();
                        } else if d == c {
                            break;
                        }
                    }
                    out.push(' ');
                }
                '`' => {
                    stack.push(At::Template);
                    out.push(' ');
                }
                '{' => {
                    *braces += 1;
                    out.push(c);
                }
                '}' if *braces == 0 && nested => {
                    stack.pop();
                    out.push(' ');
                }
                '}' => {
                    *braces = braces.saturating_sub(1);
                    out.push(c);
                }
                _ => out.push(c),
            },
            None => break,
        }
    }
    out
}

/// Whether a formula text names `source` as its root, as `git.branch` or `pr[0]` do. A word scan, like [`reads_clock`].
pub fn reads_source(expr: &str, source: &str) -> bool {
    let expr = &code_of(expr);
    let joins = |c: Option<char>, also: &[char]| c.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || also.contains(&c));
    expr.match_indices(source)
        .any(|(at, _)| !(joins(expr[..at].chars().next_back(), &['.', '$']) || joins(expr[at + source.len()..].chars().next(), &['('])))
}

/// The [`SOURCES`] any of `formulas` reads, itself or through the derived values it names.
pub fn sources(formulas: &[String], derived: &Map<String, Value>) -> Vec<&'static str> {
    SOURCES.into_iter().filter(|s| reads_through(formulas, derived, |e| reads_source(e, s))).collect()
}

/// How often the panel asks `formulas` again for the clock: each second while one reads the time, each 30 s while one
/// reads only the day or the zone (`time.ts`), never otherwise.
pub fn tick(formulas: &[String], derived: &Map<String, Value>) -> Option<Duration> {
    if clocked(formulas, derived) {
        Some(Duration::from_secs(1))
    } else if reads_through(formulas, derived, |e| reads_source(e, "time")) {
        Some(Duration::from_secs(30))
    } else {
        None
    }
}

/// Whether any of `formulas` reads the clock, itself or through the derived values it names as `$name`, as the web's
/// engine reads a piece through the derived values it names.
pub fn clocked(formulas: &[String], derived: &Map<String, Value>) -> bool {
    reads_through(formulas, derived, reads_clock)
}

/// Whether any of `formulas` reads what `reads` sees in a text, itself or through the derived values it names.
fn reads_through(formulas: &[String], derived: &Map<String, Value>, reads: impl Fn(&str) -> bool) -> bool {
    let names = |expr: &str, name: &str| {
        let own = format!("${name}");
        expr.match_indices(&own).any(|(at, _)| !expr[at + own.len()..].starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_'))
    };
    let mut through: Vec<&str> = Vec::new();
    loop {
        let more: Vec<&str> = derived
            .iter()
            .filter(|(name, expr)| {
                !through.contains(&name.as_str()) && expr.as_str().is_some_and(|e| reads(e) || through.iter().any(|t| names(e, t)))
            })
            .map(|(name, _)| name.as_str())
            .collect();
        if more.is_empty() {
            break;
        }
        through.extend(more);
    }
    formulas.iter().any(|f| reads(f) || through.iter().any(|t| names(f, t)))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_prop_turns_back_into_the_formula_language() {
        assert_eq!(expr_of(&json!({"bind": "$presses + 1"})), "($presses + 1)");
        assert_eq!(expr_of(&json!({"format": "v${$n} `x`"})), "`v${$n} \\`x\\``");
        assert_eq!(expr_of(&json!([1, "a\"b", null, {"k": true}])), r#"[1, "a\"b", null, {"k": true}]"#);
        assert_eq!(expr_of(&json!({"bind": "item.n"})), "(item.n)");
        assert!(!is_live(&json!({"bind": 3})));
        let facts = json!([{"label": "A", "value": {"bind": "$a"}}, {"label": "B", "value": {"format": "${$b} s"}}]);
        assert_eq!(leaves(&facts), ["($a)", "`${$b} s`"], "each binding is its own formula, so a long list stays under the cap");
        let answered: HashMap<String, Value> =
            [("($a)".to_owned(), json!(1)), ("`${$b} s`".to_owned(), json!("2 s"))].into_iter().collect();
        assert_eq!(filled(&facts, &answered), Some(json!([{"label": "A", "value": 1}, {"label": "B", "value": "2 s"}])));
        assert_eq!(filled(&facts, &HashMap::new()), None);
        assert!(!is_live(&json!({"label": "x", "on": {"press": [{"do": "set", "path": "$n", "value": {"bind": "$n + 1"}}]}})));
        assert!(is_live(&json!([{"title": "x", "value": {"bind": "item.name"}}])));
    }

    #[test]
    fn rows_ask_each_written_element_and_read_each_row_back() {
        let piece = Piece::read(
            "table-1",
            &json!({"type": "table", "props": {"items": [{"bind": "{ n: 1 }"}], "key": {"bind": "item.n"},
                "columns": [{"title": "N", "value": {"bind": "item.n"}, "mono": true}]}}),
        )
        .unwrap();
        let rows = Rows::of(&piece).unwrap();
        assert_eq!(rows.formulas(), ["[({ n: 1 })]", "[({ n: 1 })] | map(_slate0: (item.n), _slate1: (item.n))"]);
        let answered: HashMap<String, Value> = [
            ("[({ n: 1 })]".to_owned(), json!([{"n": 1}])),
            ("[({ n: 1 })] | map(_slate0: (item.n), _slate1: (item.n))".to_owned(), json!([{"n": 1, "_slate0": 1, "_slate1": 1}])),
        ]
        .into_iter()
        .collect();
        let read = rows.read(&answered).unwrap();
        assert_eq!(read[0].item, json!({"n": 1}));
        assert_eq!(read[0].props["columns"], json!([{"title": "N", "value": 1, "mono": true}]));
        assert_eq!(read[0].props["key"], json!(1));
        assert!(rows.read(&HashMap::new()).is_none(), "rows wait for the host's answer");
        let acted = Piece::read(
            "table-2",
            &json!({"type": "table", "props": {"items": {"bind": "$rows"},
                "rowActions": [{"label": "Count", "when": "item.bad", "on": {"press": [{"do": "set", "path": "$n", "value": {"bind": "$n + 1"}}]}}]}}),
        )
        .unwrap();
        let rows = Rows::of(&acted).unwrap();
        assert_eq!(
            rows.formulas(),
            ["($rows)", "($rows) | map(_slate0: (item.bad))"],
            "a row action's own when is read per row, its handler is not"
        );
        let answered: HashMap<String, Value> = [
            ("($rows)".to_owned(), json!([{"bad": true}])),
            ("($rows) | map(_slate0: (item.bad))".to_owned(), json!([{"bad": true, "_slate0": true}])),
        ]
        .into_iter()
        .collect();
        let read = rows.read(&answered).unwrap();
        assert_eq!(read[0].props["rowActions"][0]["when"], json!(true));
        assert_eq!(read[0].props["rowActions"][0]["on"]["press"][0]["value"], json!({"bind": "$n + 1"}));
    }

    #[test]
    fn no_formula_passes_the_hosts_cap() {
        let long = "x".repeat(300);
        let columns: Vec<Value> = (0..6).map(|i| json!({"title": format!("c{i}"), "value": {"bind": format!("item.{long}{i}")}})).collect();
        let piece = Piece::read("t", &json!({"type": "table", "props": {"items": {"bind": "$rows"}, "columns": columns}})).unwrap();
        let rows = Rows::of(&piece).unwrap();
        let formulas = rows.formulas();
        assert_eq!(formulas.len(), 7, "the items, then one formula per field once two no longer fit: {formulas:?}");
        assert!(formulas.iter().all(|f| f.len() <= EXPR_CHARS));
        let points: Vec<Value> =
            (0..400).map(|i| json!({"bind": format!("{{ at: {}, v: {} }}", 1_792_281_600_000u64 + i * 60_000, 2400 + i % 7)})).collect();
        let chart =
            Piece::read("c", &json!({"type": "chart", "props": {"items": points, "x": {"bind": "item.at"}, "value": {"bind": "item.v"}}}))
                .unwrap();
        let formulas = Rows::of(&chart).unwrap().formulas();
        assert!(formulas.iter().all(|f| f.len() <= EXPR_CHARS), "a written list is cut into runs under the cap");
        assert!(formulas.len() < 150, "runs of elements, not one formula per element: {}", formulas.len());
    }

    #[test]
    fn a_run_reads_index_from_where_it_starts_and_nothing_else_moves() {
        let at7 = |e: &str| shifted(e, 7);
        assert_eq!(at7("(index)"), "((index + 7))");
        assert_eq!(at7("(index * 2 + item.index)"), "((index + 7) * 2 + item.index)");
        assert_eq!(at7(r#"("index" + str(index))"#), r#"("index" + str((index + 7)))"#);
        assert_eq!(at7("`index ${index} $${index} \\` ${ `${index}` }`"), "`index ${(index + 7)} $${index} \\` ${ `${(index + 7)}` }`");
        assert_eq!(at7("($index + index)"), "($index + (index + 7))");
        assert_eq!(at7(r#"({ index: index, "index": 1 }).index"#), r#"({ index: (index + 7), "index": 1 }).index"#);
        assert_eq!(at7("($c ? index : index + 1)"), "($c ? (index + 7) : (index + 7) + 1)");
        assert_eq!(at7("(index.x == null and index[0] == null)"), "((index + 7).x == null and (index + 7)[0] == null)");
        assert_eq!(at7("(index||index&&index)"), "((index + 7)||(index + 7)&&(index + 7))");
        assert_eq!(at7("(1.5e3+index)"), "(1.5e3+(index + 7))");
        assert_eq!(at7("(at($l, index) | first)"), "(at($l, (index + 7)) | first)");
        // A nested step reads its own index per element, but sortBy's order and join's list where the step stands.
        let nested = "(item.kids | where(index > 0) | map(index: index) | pick(index) | flatten(index) | count)";
        assert_eq!(at7(nested), nested);
        assert_eq!(
            at7(r#"(item.kids | sortBy(item.v, index > 7 ? "desc" : "asc") | join($l[index], id) | first)"#),
            r#"(item.kids | sortBy(item.v, (index + 7) > 7 ? "desc" : "asc") | join($l[(index + 7)], id) | first)"#
        );
        assert_eq!(
            at7("([index, index] | map(n: item + index) | format(`${item.n}:${index}`))"),
            "([(index + 7), (index + 7)] | map(n: item + index) | format(`${item.n}:${index}`))"
        );
        for same in ["(orElse(index(), 3))", "(index_ + 1) == null", "(item . index)", "('index unclosed)", "(indexes)"] {
            assert_eq!(at7(same), same);
        }
        assert_eq!(shifted("(index)", 0), "(index)");
    }

    #[test]
    fn a_written_list_past_one_run_counts_its_rows_from_the_start() {
        let items: Vec<Value> = (0..60).map(|i| json!({"name": format!("row-{i:04}")})).collect();
        let piece =
            Piece::read("t", &json!({"type": "table", "props": {"items": items, "columns": [{"title": "N", "value": {"bind": "index"}}]}}))
                .unwrap();
        let rows = Rows::of(&piece).unwrap();
        let Items::Each(runs) = &rows.items else { panic!("a written list goes in runs") };
        assert!(runs.len() >= 2, "long enough for two runs: {}", runs.len());
        let mut answered = HashMap::new();
        let mut from = 0;
        for (run, fields) in runs.iter().map(|r| &r.formula).zip(&rows.fields) {
            let len = run.matches("row-").count();
            let want = if from == 0 { "(index)".to_owned() } else { format!("((index + {from}))") };
            assert_eq!(fields[0].0, format!("{run} | map(_slate0: {want})"));
            answered.insert(run.clone(), Value::Array((from..from + len).map(|i| json!({"name": format!("row-{i:04}")})).collect()));
            answered.insert(fields[0].0.clone(), Value::Array((from..from + len).map(|i| json!({"_slate0": i})).collect()));
            from += len;
        }
        let read = rows.read(&answered).unwrap();
        assert_eq!(read.len(), 60);
        assert!(read.iter().all(|r| r.props["columns"][0]["value"] == json!(r.index)));
    }

    #[test]
    fn a_format_past_the_cap_asks_each_hole_and_joins_the_words_here() {
        let words = "w".repeat(520);
        let format = json!({ "format": format!("{words} ${{$n}} and $${{not}} ${{$m}}!") });
        assert_eq!(leaves(&format), ["`${$n}`", "`${$m}`"], "each hole is its own formula, under the cap");
        let answered =
            |n: Value, m: Value| -> HashMap<String, Value> { [("`${$n}`".to_owned(), n), ("`${$m}`".to_owned(), m)].into_iter().collect() };
        assert_eq!(filled(&format, &answered(json!("5"), json!("x"))), Some(json!(format!("{words} 5 and ${{not}} x!"))));
        assert_eq!(filled(&format, &answered(json!("5"), Value::Null)), Some(json!(format!("{words} 5 and ${{not}} !"))));
        assert_eq!(filled(&format, &answered(Value::Null, Value::Null)), Some(Value::Null), "every hole read nothing: null, as joined");
        assert_eq!(filled(&format, &HashMap::new()), None, "unanswered until each hole is");
        let short = json!({ "format": "a ${$n}" });
        assert_eq!(leaves(&short), ["`a ${$n}`"], "a format that fits is still one formula");
        let table = Piece::read(
            "t",
            &json!({"type": "table", "props": {"items": {"bind": "$rows"}, "columns": [{"title": "N", "value": format}]}}),
        )
        .unwrap();
        let rows = Rows::of(&table).unwrap();
        assert_eq!(rows.formulas(), ["($rows)", "($rows) | map(_slate0: `${$n}`, _slate1: `${$m}`)"]);
        let answered: HashMap<String, Value> = [
            ("($rows)".to_owned(), json!([{}])),
            ("($rows) | map(_slate0: `${$n}`, _slate1: `${$m}`)".to_owned(), json!([{"_slate0": "1", "_slate1": "2"}])),
        ]
        .into_iter()
        .collect();
        assert_eq!(rows.read(&answered).unwrap()[0].props["columns"][0]["value"], json!(format!("{words} 1 and ${{not}} 2!")));
    }

    #[test]
    fn a_written_element_too_long_for_its_brackets_keeps_its_row_and_the_rows_after() {
        let long = format!("{{ name: \"{}\" }}", "x".repeat(485));
        let mut items: Vec<Value> = (0..3).map(|i| json!({"bind": format!("{{ name: \"row-{i}\" }}")})).collect();
        items.insert(1, json!({ "bind": long }));
        let piece =
            Piece::read("t", &json!({"type": "table", "props": {"items": items, "columns": [{"title": "N", "value": {"bind": "index"}}]}}))
                .unwrap();
        let rows = Rows::of(&piece).unwrap();
        let formulas = rows.formulas();
        assert!(formulas.iter().all(|f| f.len() <= EXPR_CHARS), "{formulas:?}");
        assert!(formulas.iter().any(|f| f.starts_with("({ name") && f.len() == 499), "the long element is asked bare: {formulas:?}");
        let mut answered: HashMap<String, Value> = HashMap::new();
        for f in &formulas {
            let count = f.matches("row-").count();
            let answer = if f.contains("| map(") {
                let from = f.split("index + ").nth(1).and_then(|r| r.split(')').next()).and_then(|n| n.parse().ok()).unwrap_or(0);
                Value::Array((from..from + count).map(|i| json!({ "_slate0": i })).collect())
            } else if f.contains("xxxx") {
                json!({ "name": "long" })
            } else {
                Value::Array((0..count).map(|_| json!({ "name": "short" })).collect())
            };
            answered.insert(f.clone(), answer);
        }
        let read = rows.read(&answered).unwrap();
        assert_eq!(read.iter().map(|r| r.index).collect::<Vec<_>>(), [0, 1, 2, 3], "the long element keeps its row");
        assert_eq!(read[1].item, json!({ "name": "long" }));
        assert_eq!(read[2].props["columns"][0]["value"], json!(2), "and the next row reads its own index");
        assert_eq!(read[1].props["columns"][0]["value"], Value::Null, "its own field does not fit beside it and reads null");
        let first = formulas.iter().find(|f| f.starts_with('[') && !f.contains("| map(")).unwrap();
        answered.insert(first.clone(), Value::Null);
        let read = rows.read(&answered).unwrap();
        assert_eq!(read.iter().map(|r| r.index).collect::<Vec<_>>(), [0, 1, 2, 3], "a run the host refused keeps its rows too");
    }

    #[test]
    fn a_source_is_read_by_its_root_name_alone() {
        assert!(reads_source("git.branch", "git") && reads_source("(pr[0].state)", "pr") && reads_source("$x + thread.turns", "thread"));
        assert!(!reads_source("$git.branch", "git") && !reads_source("item.pr", "pr") && !reads_source("github", "git"));
        let derived: Map<String, Value> = serde_json::from_value(json!({"b": "git.branch", "c": "$b + 1"})).unwrap();
        assert_eq!(sources(&["($c)".to_owned(), "(usage.week)".to_owned()], &derived), ["git", "usage"]);
    }

    #[test]
    fn the_day_and_the_zone_tick_each_thirty_seconds_and_the_time_each_second() {
        let none = Map::new();
        let tick = |f: &str| super::tick(&[f.to_owned()], &none);
        assert_eq!(tick("(time.now - 5)"), Some(Duration::from_secs(1)));
        assert_eq!(tick("(ago($at))"), Some(Duration::from_secs(1)));
        assert_eq!(tick("(time.today)"), Some(Duration::from_secs(30)));
        assert_eq!(tick("(time.zone)"), Some(Duration::from_secs(30)));
        assert_eq!(tick("($time.today)"), None);
    }

    #[test]
    fn a_source_named_in_words_or_strings_is_not_read() {
        let none = Map::new();
        let read = |f: &str| super::sources(&[f.to_owned()], &none);
        assert_eq!(read("`This thread has ${$n} files`"), Vec::<&str>::new());
        assert_eq!(read("(\"run git status\" + 'open a pr')"), Vec::<&str>::new());
        assert_eq!(super::tick(&["`at time.now, ago(1)`".to_owned()], &none), None);
        assert_eq!(read("`on ${git.branch} in ${`pr ${pr.number}`}`"), ["git", "pr"]);
        assert_eq!(read("(thread.title + \"}\")"), ["thread"]);
    }

    #[test]
    fn a_binding_too_long_for_its_parentheses_goes_bare() {
        let long = format!("$n + {}", "1 + ".repeat(123) + "11");
        assert_eq!(long.len(), 499);
        assert_eq!(expr_of(&json!({ "bind": long })), long);
        assert_eq!(expr_of(&json!({ "bind": "$n" })), "($n)");
    }

    #[test]
    fn own_paths_write_where_they_point() {
        let mut r = Record::default();
        assert!(r.set_own("$steps[1].done", json!(true)));
        assert_eq!(r.values["steps"], json!([null, {"done": true}]));
        assert_eq!(r.get_own("$steps[1].done"), Some(&json!(true)));
        assert!(!r.set_own("steps", json!(1)));
    }

    #[test]
    fn a_big_number_reads_back_as_the_same_number() {
        assert_eq!(literal(&json!(1e21)), "1e21");
        assert_eq!(literal(&json!(123_456_789_012_345_680_000.0)), "1.2345678901234568e20");
        assert_eq!(literal(&json!([1e-7, 42])), "[1e-7, 42]");
    }

    #[test]
    fn a_format_keeps_its_holes_as_written() {
        let sent = |format: &str| expr_of(&json!({ "format": format }));
        assert_eq!(sent("${`${$n} items`}"), "`${`${$n} items`}`");
        assert_eq!(sent(r"a `b` ${$ok ? `x${$n}` : 'y'} c`"), r"`a \`b\` ${$ok ? `x${$n}` : 'y'} c\``");
        assert_eq!(sent(r"Saved in ${$dir}\"), r"`Saved in ${$dir}${'\\'}`");
        assert_eq!(sent(r"${$n}\\"), r"`${$n}\${'\\'}`");
        assert_eq!(sent("$${not} ${$n}"), "`$${not} ${$n}`");
    }

    #[test]
    fn clock_reads_are_seen_and_names_like_them_are_not() {
        assert!(reads_clock("ago(item.at)"));
        assert!(reads_clock("time.now - 5"));
        assert!(!reads_clock("$agony + 1"));
        assert!(!reads_clock("$x.ago"));
    }

    #[test]
    fn a_clock_read_through_a_derived_value_is_seen() {
        let derived: Map<String, Value> =
            serde_json::from_value(json!({"now": "time.now", "since": "ago($start)", "later": "$since", "plain": "$n + 1"})).unwrap();
        let asks = |f: &str| clocked(&[f.to_owned()], &derived);
        assert!(asks("($now)") && asks("($later * 2)") && asks("(time.now)"));
        assert!(!asks("($plain)") && !asks("($nowhere)"));
    }
}
