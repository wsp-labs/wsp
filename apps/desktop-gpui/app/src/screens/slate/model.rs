// SPDX-License-Identifier: AGPL-3.0-only
//! One thread's slate as the window holds it: the host's record from `slates.get`, its document read into pieces, the
//! live values the host pushes, and the formula texts the panel sends `slates.resolve` for. The window never evaluates
//! a formula: a prop that holds a binding or a format is turned back into the formula language's own text and the host
//! answers its value.

use std::collections::{HashMap, HashSet};

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

/// Each binding and format in a prop value, as the formula the host answers for it; a handler (`on`) is the host's.
pub fn leaves(v: &Value) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(v: &Value, out: &mut Vec<String>) {
        match v {
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
        Value::Object(o) if is_binding(o) || is_format(o) => resolved.get(&expr_of(v)).cloned(),
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
        other => other.to_string(),
    }
}

/// A format string as a template: its `${...}` holes are the template's, and a backquote in its words is escaped.
fn template(src: &str) -> String {
    let mut out = String::from("`");
    for c in src.chars() {
        if c == '`' {
            out.push_str("\\`");
        } else {
            out.push(c);
        }
    }
    out.push('`');
    out
}

/// A prop value written as one formula: a literal as itself, a binding in parentheses, a format as a template, lists
/// and records through.
pub fn expr_of(v: &Value) -> String {
    match v {
        Value::Object(o) if is_binding(o) => format!("({})", o["bind"].as_str().unwrap_or_default()),
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
/// at a time, `[e0, e1, ...] | map(...)`, where `index` counts from each run's start: the host takes no row scope from
/// outside a formula.
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
    Each(Vec<String>),
}

/// The host's cap on one formula's length (`SLATE_LIMITS.exprChars`).
pub const EXPR_CHARS: usize = 500;

#[derive(Clone, Debug)]
enum Shape {
    Lit(Value),
    Leaf(usize),
    List(Vec<Shape>),
    Rec(Vec<(String, Shape)>),
}

fn shape_of(v: &Value, leaves: &mut Vec<String>) -> Shape {
    match v {
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
        let items = match items {
            Value::Array(list) => {
                // Room for the longest field beside a chunk, so each field fits with every chunk.
                let widest = leaves.iter().enumerate().map(|(k, e)| format!(" | map({}: {e})", row_field(k)).len()).max().unwrap_or(0);
                Items::Each(chunks(list.iter().map(expr_of), EXPR_CHARS.saturating_sub(widest).max(EXPR_CHARS / 5)))
            }
            other => Items::Whole(expr_of(other)),
        };
        let heads: Vec<&String> = match &items {
            Items::Whole(e) => vec![e],
            Items::Each(each) => each.iter().collect(),
        };
        let fields = heads.iter().map(|head| packed(head, &leaves)).collect();
        Some(Rows { items, fields, shapes })
    }

    /// Every formula the rows need answered.
    pub fn formulas(&self) -> Vec<String> {
        let heads: Vec<String> = match &self.items {
            Items::Whole(e) => vec![e.clone()],
            Items::Each(each) => each.clone(),
        };
        heads.into_iter().chain(self.fields.iter().flatten().map(|(f, _)| f.clone())).collect()
    }

    /// Each row as the piece reads it, once the host has answered every formula: `item` the element as the items
    /// hold it, `props` its row-scoped props. A field formula the host left unanswered reads null.
    pub fn read(&self, resolved: &HashMap<String, Value>) -> Option<Vec<Row>> {
        let empty = Map::new();
        let mut rows: Vec<(Value, Map<String, Value>)> = Vec::new();
        let groups: Vec<(&String, usize)> = match &self.items {
            Items::Whole(e) => vec![(e, 0)],
            Items::Each(each) => each.iter().enumerate().map(|(i, e)| (e, i)).collect(),
        };
        for (g, (head, _)) in groups.iter().enumerate() {
            let list = resolved.get(*head)?.as_array().cloned().unwrap_or_default();
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

/// Written-out elements as list formulas of at most `room` characters each, in order; an element longer than that
/// stands alone.
fn chunks(elements: impl Iterator<Item = String>, room: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut chunk: Vec<String> = Vec::new();
    let mut len = 2;
    for e in elements {
        if !chunk.is_empty() && len + e.len() + 2 > room {
            out.push(format!("[{}]", chunk.join(", ")));
            chunk.clear();
            len = 2;
        }
        len += e.len() + 2;
        chunk.push(e);
    }
    if !chunk.is_empty() {
        out.push(format!("[{}]", chunk.join(", ")));
    }
    out
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

/// Whether a formula text reads the clock, so the panel asks again each second while it is drawn. A word scan, not a
/// parse: a name inside a string reads as a clock read too, which only costs a tick.
pub fn reads_clock(expr: &str) -> bool {
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
    fn own_paths_write_where_they_point() {
        let mut r = Record::default();
        assert!(r.set_own("$steps[1].done", json!(true)));
        assert_eq!(r.values["steps"], json!([null, {"done": true}]));
        assert_eq!(r.get_own("$steps[1].done"), Some(&json!(true)));
        assert!(!r.set_own("steps", json!(1)));
    }

    #[test]
    fn clock_reads_are_seen_and_names_like_them_are_not() {
        assert!(reads_clock("ago(item.at)"));
        assert!(reads_clock("time.now - 5"));
        assert!(!reads_clock("$agony + 1"));
        assert!(!reads_clock("$x.ago"));
    }
}
