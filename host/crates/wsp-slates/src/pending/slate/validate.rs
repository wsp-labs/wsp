// SPDX-License-Identifier: AGPL-3.0-only
//! protocol/src/slate/validate.ts at 65f6b89c7.

use std::collections::HashSet;
use std::rc::Rc;
use std::sync::LazyLock;

use indexmap::IndexMap;
use regex::Regex;
use serde_json::{json, Map, Value};
use wsp_wire::text::json_stringify;
use wsp_words::js::{is_js_whitespace, js_trim, js_trim_start, number_string, JS_WS};
use wsp_words::words::units::fmt_bytes;

use super::expr::{check_slate_expression, parse_slate_expression, parse_slate_format, slate_dependencies, SlateCheckScope, SlateExpr, SlateFormatPart, SlateType, SlateTypeName};
use super::kit::{SlateItemSpec, SlatePieceCheck, SlatePieceModule, SlatePropSpec, SlatePropType, SlateBinds};
use super::paths::{parse_slate_own_path, slate_own_name};
use super::*;

pub type SlateLines = IndexMap<String, i64>;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SlateValidated {
    pub errors: Vec<SlateProblem>,
    pub warnings: Vec<SlateProblem>,
}

const RUN_FIELDS: [&str; 15] = ["state", "why", "exit", "out", "err", "json", "lines", "startedAt", "endedAt", "ms", "runs", "cut", "stale", "refreshing", "text"];
const SECRET_FIELDS: [&str; 3] = ["set", "len", "at"];
const PANE_KINDS: [&str; 9] = ["preview", "terminal", "diff", "pr", "files", "machine", "processes", "agents", "slate"];
const STEP_KINDS: [&str; 11] = ["set", "toggle", "start", "cancel", "send", "steer", "queue", "fill", "open", "copy", "pane"];
const RUN_ATTRS: [&str; 7] = ["every", "cmd", "timeout", "always", "once", "interval", "refresh"];
const DOC_KEYS: [&str; 10] = ["schema", "kit", "title", "root", "values", "derived", "runs", "reactions", "pieces", "files"];
const PIECE_KEYS: &str = "type, props, children, when, on and fallback";
const MONO_WARNING: &str = "mono is for figures, ids, times and paths; this reads as a sentence, which reads better in the normal face";
const DOCUMENT_SHAPE: &str = "a document is an object with schema, root, values, derived, runs, reactions and pieces";
const ENV_NAME_FIX: &str = "capitals, digits and _";

fn re(pattern: &str) -> Regex {
    Regex::new(pattern).expect("a slate validator pattern")
}

static SLATE_NAME_RE: LazyLock<Regex> = LazyLock::new(|| re(r"^[a-z_][a-zA-Z0-9_]{0,47}$"));
static SLATE_ID_RE: LazyLock<Regex> = LazyLock::new(|| re(r"^[A-Za-z][A-Za-z0-9_-]{0,47}$"));
static SLATE_FILE_RE: LazyLock<Regex> = LazyLock::new(|| re(r"^[A-Za-z0-9_][A-Za-z0-9._-]{0,63}$"));
static ENV_NAME_RE: LazyLock<Regex> = LazyLock::new(|| re(r"^[A-Z_][A-Z0-9_]*$"));
static KEPT_CAPS_RE: LazyLock<Regex> = LazyLock::new(|| re(r"^[A-Z0-9]{2,}s?$|^.+[A-Z].*$"));
static ACRONYM_RE: LazyLock<Regex> = LazyLock::new(|| re(r"^[A-Z0-9]{2,}s?$"));
static LONG_WORD_RE: LazyLock<Regex> = LazyLock::new(|| re(r"^[A-Za-z]{4,}"));
static CAP_START_RE: LazyLock<Regex> = LazyLock::new(|| re(r"^[A-Z]"));
static SENTENCE_WORD_RE: LazyLock<Regex> = LazyLock::new(|| re(r"^[A-Za-z][a-z]*[,.:;!?]?$"));
static WS_RUN_RE: LazyLock<Regex> = LazyLock::new(|| re(&format!("[{JS_WS}]+")));
static ALL_WS_RE: LazyLock<Regex> = LazyLock::new(|| re(&format!("^[{JS_WS}]+$")));
static TOKEN_RE: LazyLock<Regex> = LazyLock::new(|| {
    re(&format!(
        r"(?-u:\b)Bearer[{JS_WS}]+([A-Za-z0-9._~+/-]{{20,}})|(?-u:\b)((?:xapt|xaat|sk|sk-ant|ghp|gho|ghs|github_pat|glpat|xox[abp]|AKIA)[-_][A-Za-z0-9_-]{{16,}})"
    ))
});
static JOINED_RE: LazyLock<Regex> = LazyLock::new(|| {
    re(&format!(
        r"[^{JS_WS}][{JS_WS}]*([\x{{B7}}\x{{2022}}])[{JS_WS}]*[^{JS_WS}]|[^{JS_WS}] (\|) [^{JS_WS}]|^[{JS_WS}]*([\x{{B7}}\x{{2022}}])[{JS_WS}]*$|^ +(\|) +$"
    ))
});
static QUOTED_RE: LazyLock<Regex> = LazyLock::new(|| {
    re(r#""((?:[^"\\]|\\[^\n\r\x{2028}\x{2029}])*)"|'((?:[^'\\]|\\[^\n\r\x{2028}\x{2029}])*)'|`((?:[^`\\]|\\[^\n\r\x{2028}\x{2029}])*)`"#)
});
static HOLE_RE: LazyLock<Regex> = LazyLock::new(|| re(r"\$\{[^}]*\}"));
static PATH_LIKE_RE: LazyLock<Regex> = LazyLock::new(|| re(r"^([a-z]+)\.[a-zA-Z_.\[\]0-9]+$"));
static MUSTACHE_RE: LazyLock<Regex> = LazyLock::new(|| re(r"\{\$[^}]*\}"));
static EMOJI_RE: LazyLock<Regex> = LazyLock::new(|| re(r"\p{Extended_Pictographic}"));
static NUMBER_TEXT_RE: LazyLock<Regex> = LazyLock::new(|| re(r"^-?[0-9]+(\.[0-9]+)?$"));
static LOCAL_NAME_RE: LazyLock<Regex> = LazyLock::new(|| re(r"^[a-z_][A-Za-z0-9_]*$"));
static LOCAL_ITEM_RE: LazyLock<Regex> = LazyLock::new(|| re(r"^[A-Za-z_][A-Za-z0-9_]*$"));
static SLATE_DIR_RE: LazyLock<Regex> = LazyLock::new(|| re(r"\$\{?SLATE_DIR\}?/([A-Za-z0-9._-]+)"));

fn flag_re(env: &str) -> Regex {
    let pattern = String::from(r#"(?:^|["#)
        + JS_WS
        + r#"])(-{1,2}[A-Za-z][A-Za-z0-9_-]*)(?:=|["#
        + JS_WS
        + r#"]+)["']?\$\{?"#
        + &regex::escape(env)
        + r"(?-u:\b)";
    re(&pattern)
}

fn secret_ref_re(secret: &str) -> Regex {
    re(&(String::from(r"\$\{?") + &regex::escape(secret) + r"(?-u:\b)"))
}

fn split_ws(s: &str) -> Vec<&str> {
    WS_RUN_RE.split(s).collect()
}

fn split_keep_ws(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut pos = 0;
    for m in WS_RUN_RE.find_iter(s) {
        out.push(&s[pos..m.start()]);
        out.push(m.as_str());
        pos = m.end();
    }
    out.push(&s[pos..]);
    out
}

fn tail(s: &str) -> &str {
    let mut chars = s.chars();
    chars.next();
    chars.as_str()
}

fn js_str(v: &Value) -> String {
    match v {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => number_string(n.as_f64().unwrap_or(f64::NAN)),
        Value::String(s) => s.clone(),
        Value::Array(items) => items.iter().map(|x| if x.is_null() { String::new() } else { js_str(x) }).collect::<Vec<_>>().join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

fn opt_text(o: &Option<String>) -> &str {
    o.as_deref().unwrap_or("undefined")
}

fn js_opt(v: Option<&Value>) -> String {
    v.map_or_else(|| "undefined".to_string(), js_str)
}

fn nn(v: Option<&Value>) -> Option<&Value> {
    v.filter(|x| !x.is_null())
}

fn is_true(v: Option<&Value>) -> bool {
    v == Some(&Value::Bool(true))
}

fn parsed_type(v: Option<&Value>) -> &'static str {
    match v {
        None => "undefined",
        Some(Value::Null) => "null",
        Some(Value::Bool(_)) => "boolean",
        Some(Value::Number(_)) => "number",
        Some(Value::String(_)) => "string",
        Some(Value::Array(_)) => "array",
        Some(Value::Object(_)) => "object",
    }
}

fn is_slate_format(v: &Value) -> bool {
    v.as_object().is_some_and(|m| m.len() == 1 && m.get("format").is_some_and(Value::is_string))
}

fn is_bound(v: &Value) -> bool {
    is_slate_binding(v) || is_slate_format(v)
}

fn bind_text(v: &Value) -> &str {
    v.get("bind").and_then(Value::as_str).unwrap_or("")
}

fn is_record_value(v: &Value) -> bool {
    v.is_object() && !is_bound(v)
}

fn js_entries(v: &Value) -> Vec<(String, &Value)> {
    match v {
        Value::Object(m) => m.iter().map(|(k, x)| (k.clone(), x)).collect(),
        Value::Array(items) => items.iter().enumerate().map(|(i, x)| (i.to_string(), x)).collect(),
        _ => Vec::new(),
    }
}

fn kind_word_of(kind: Option<Kind>) -> Option<&'static str> {
    kind.map(Kind::word)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Value,
    Secret,
    Derived,
    Run,
}

impl Kind {
    fn word(self) -> &'static str {
        match self {
            Kind::Value => "value",
            Kind::Secret => "secret",
            Kind::Derived => "derived",
            Kind::Run => "run",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
struct Where {
    piece: Option<String>,
    prop: Option<String>,
}

impl Where {
    fn piece(id: &str) -> Where {
        Where { piece: Some(id.to_string()), prop: None }
    }
    fn at(id: &str, prop: &str) -> Where {
        Where { piece: Some(id.to_string()), prop: Some(prop.to_string()) }
    }
}

#[derive(Clone, Debug, Default)]
struct ExprOpts {
    row: Option<SlateType>,
    secret_ok: bool,
    enum_words: Option<Vec<String>>,
}

fn any_t() -> SlateType {
    SlateType { t: SlateTypeName::Any, of: None, fields: None }
}

fn ty_of(t: SlateTypeName) -> SlateType {
    SlateType { t, of: None, fields: None }
}

fn literal_type(v: &Value) -> SlateType {
    match v {
        Value::Null => any_t(),
        Value::Number(_) => ty_of(SlateTypeName::Number),
        Value::String(_) => ty_of(SlateTypeName::String),
        Value::Bool(_) => ty_of(SlateTypeName::Boolean),
        Value::Array(items) => match items.first() {
            None => ty_of(SlateTypeName::List),
            Some(first) => SlateType { t: SlateTypeName::List, of: Some(Box::new(literal_type(first))), fields: None },
        },
        Value::Object(m) => {
            if m.is_empty() {
                ty_of(SlateTypeName::Record)
            } else {
                let fields = m.iter().map(|(k, x)| (k.clone(), literal_type(x))).collect();
                SlateType { t: SlateTypeName::Record, of: None, fields: Some(fields) }
            }
        }
    }
}

fn run_field_types() -> IndexMap<String, SlateType> {
    let list_of_strings = SlateType { t: SlateTypeName::List, of: Some(Box::new(ty_of(SlateTypeName::String))), fields: None };
    [
        ("state", ty_of(SlateTypeName::String)),
        ("why", ty_of(SlateTypeName::String)),
        ("exit", ty_of(SlateTypeName::Number)),
        ("out", any_t()),
        ("err", ty_of(SlateTypeName::String)),
        ("json", any_t()),
        ("lines", list_of_strings),
        ("startedAt", ty_of(SlateTypeName::Number)),
        ("endedAt", ty_of(SlateTypeName::Number)),
        ("ms", ty_of(SlateTypeName::Number)),
        ("runs", ty_of(SlateTypeName::Number)),
        ("cut", ty_of(SlateTypeName::Boolean)),
        ("stale", ty_of(SlateTypeName::Boolean)),
        ("refreshing", ty_of(SlateTypeName::Boolean)),
        ("text", ty_of(SlateTypeName::Boolean)),
    ]
    .into_iter()
    .map(|(k, t)| (k.to_string(), t))
    .collect()
}

fn secret_field_type(field: &str) -> SlateType {
    match field {
        "set" => ty_of(SlateTypeName::Boolean),
        _ => ty_of(SlateTypeName::Number),
    }
}

struct Ctx {
    doc: Value,
    kinds: IndexMap<String, Kind>,
    derived_types: std::cell::RefCell<IndexMap<String, SlateType>>,
}

impl Ctx {
    fn derived_expr(&self, name: &str) -> Option<&str> {
        self.doc.get("derived").and_then(|d| d.get(name)).and_then(Value::as_str)
    }

    fn scope(self: &Rc<Self>, row: Option<SlateType>, secret_ok: bool, enum_words: Option<Vec<String>>) -> SlateCheckScope {
        let ctx = Rc::clone(self);
        SlateCheckScope {
            row,
            path: Box::new(move |head: &str, own: bool, segs: &[PathSeg]| {
                if own {
                    return ctx.own_type(head, segs, secret_ok);
                }
                if ctx.kinds.contains_key(head) {
                    return Err(SlateScopeProblem {
                        code: "X401",
                        message: format!("{head} is not a path; the slate's own values are ${head}"),
                        fix: Some(format!("${head}")),
                    });
                }
                if enum_words.as_ref().is_some_and(|words| words.iter().any(|w| w == head)) {
                    return Err(SlateScopeProblem {
                        code: "X401",
                        message: format!("{head} is not a path; an enum word is a string"),
                        fix: Some(format!("\"{head}\"")),
                    });
                }
                slate_source_type(head, segs)
            }),
        }
    }

    fn own_type(self: &Rc<Self>, name: &str, segs: &[PathSeg], secret_ok: bool) -> Result<SlateType, SlateScopeProblem> {
        let Some(kind) = self.kinds.get(name).copied() else {
            let names: Vec<String> = self.kinds.keys().cloned().collect();
            let fix = slate_nearest(name, &names);
            let hint = fix.as_ref().map_or(String::new(), |f| format!(" or did you mean ${f}?"));
            return Err(SlateScopeProblem {
                code: "S501",
                message: format!("${name} is not declared; add <value name=\"{name}\" start=... />{hint}"),
                fix: fix.map(|f| format!("${f}")),
            });
        };
        let first = segs.first();
        match kind {
            Kind::Secret => {
                if let Some(PathSeg::Key(field)) = first {
                    if SECRET_FIELDS.contains(&field.as_str()) {
                        return Ok(secret_field_type(field));
                    }
                }
                if secret_ok && segs.is_empty() {
                    return Ok(any_t());
                }
                Err(SlateScopeProblem {
                    code: "S520",
                    message: format!("${name} is a secret; only a run's env or stdin, an input's value or a send's paths may name it, and a formula reads only its .set, .len and .at"),
                    fix: Some(format!("${name}.set")),
                })
            }
            Kind::Run => {
                let Some(first) = first else {
                    let fields = run_field_types();
                    return Ok(SlateType { t: SlateTypeName::Record, of: None, fields: Some(fields) });
                };
                let field = match first {
                    PathSeg::Key(k) if RUN_FIELDS.contains(&k.as_str()) => k.clone(),
                    other => {
                        let shown = match other {
                            PathSeg::Key(k) => k.clone(),
                            PathSeg::Index(i) => i.to_string(),
                        };
                        let fix = slate_nearest(&shown, &RUN_FIELDS.iter().map(|s| s.to_string()).collect::<Vec<_>>());
                        return Err(SlateScopeProblem {
                            code: "X401",
                            message: format!("${name}.{shown} is not a field of a run's result; the fields are {}", RUN_FIELDS.join(", ")),
                            fix: fix.map(|f| format!("${name}.{f}")),
                        });
                    }
                };
                if segs.len() == 1 {
                    Ok(run_field_types().get(&field).cloned().unwrap_or_else(any_t))
                } else {
                    Ok(any_t())
                }
            }
            Kind::Value | Kind::Derived => {
                let mut t = if kind == Kind::Derived {
                    self.derived_type(name)
                } else {
                    literal_type(self.doc.get("values").and_then(|v| v.get(name)).and_then(|v| v.get("start")).unwrap_or(&Value::Null))
                };
                for seg in segs {
                    t = match (seg, &t) {
                        (PathSeg::Index(_), SlateType { t: SlateTypeName::List, of, .. }) => of.as_deref().cloned().unwrap_or_else(any_t),
                        (PathSeg::Key(key), SlateType { t: SlateTypeName::Record, fields: Some(fields), .. }) => {
                            fields.get(key).cloned().unwrap_or_else(any_t)
                        }
                        _ => any_t(),
                    };
                }
                Ok(t)
            }
        }
    }

    fn derived_type(self: &Rc<Self>, name: &str) -> SlateType {
        if let Some(hit) = self.derived_types.borrow().get(name) {
            return hit.clone();
        }
        self.derived_types.borrow_mut().insert(name.to_string(), any_t());
        let t = match self.derived_expr(name) {
            None => any_t(),
            Some(expr) => check_slate_expression(expr, &self.scope(None, false, None), 0).ty,
        };
        self.derived_types.borrow_mut().insert(name.to_string(), t.clone());
        t
    }
}

use super::problems::slate_nearest;
use super::sources::slate_source_type;
use super::steps::slate_step_module;

fn wrote(doc: &Value, w: &Where, fix: &str) -> bool {
    let (Some(piece), Some(prop)) = (&w.piece, &w.prop) else {
        return false;
    };
    let Some(v) = doc.get("pieces").and_then(|p| p.get(piece)).and_then(|p| p.get("props")).and_then(|p| p.get(prop)) else {
        return false;
    };
    let texts: Vec<String> = match v {
        Value::String(s) => vec![s.clone(), format!("{prop}=\"{s}\"")],
        _ if is_slate_binding(v) => {
            let b = bind_text(v);
            vec![b.to_string(), format!("{{{b}}}"), format!("{prop}={{{b}}}")]
        }
        _ => Vec::new(),
    };
    let flat = |x: &str| -> String { x.chars().filter(|c| !is_js_whitespace(*c)).collect() };
    texts.iter().any(|t| flat(t) == flat(fix))
}

fn title_cased(v: &str) -> bool {
    let later: Vec<&str> = split_ws(js_trim(v))
        .into_iter()
        .skip(1)
        .filter(|w| LONG_WORD_RE.is_match(w) && !KEPT_CAPS_RE.is_match(tail(w)) && !ACRONYM_RE.is_match(w))
        .collect();
    later.len() >= 2 && later.iter().all(|w| CAP_START_RE.is_match(w))
}

fn sentence_case(v: &str) -> String {
    split_keep_ws(js_trim(v))
        .into_iter()
        .enumerate()
        .map(|(i, w)| {
            if i == 0 || ALL_WS_RE.is_match(w) || ACRONYM_RE.is_match(w) || KEPT_CAPS_RE.is_match(tail(w)) {
                w.to_string()
            } else {
                w.to_lowercase()
            }
        })
        .collect()
}

fn token_in(text: &str) -> Option<String> {
    let m = TOKEN_RE.captures(text)?;
    let token = m.get(1).or_else(|| m.get(2))?.as_str();
    if token.starts_with('$') {
        return None;
    }
    Some(token.chars().take(6).collect())
}

fn quoted(expr: &str) -> Vec<String> {
    QUOTED_RE
        .captures_iter(expr)
        .map(|m| {
            let text = m.get(1).or_else(|| m.get(2)).or_else(|| m.get(3)).map_or("", |g| g.as_str());
            HOLE_RE.replace_all(text, "x").into_owned()
        })
        .collect()
}

fn joining_mark(v: &Value) -> Option<String> {
    let parts = if is_slate_format(v) { parse_slate_format(v.get("format").and_then(Value::as_str).unwrap_or("")).parts } else { Vec::new() };
    let texts: Vec<String> = if let Some(s) = v.as_str() {
        vec![s.to_string()]
    } else if is_slate_binding(v) {
        quoted(bind_text(v))
    } else {
        let mut out = vec![parts
            .iter()
            .map(|p| match p {
                SlateFormatPart::Text(t) => t.clone(),
                SlateFormatPart::Hole { .. } => "x".to_string(),
            })
            .collect::<String>()];
        for p in &parts {
            if let SlateFormatPart::Hole { expr, .. } = p {
                out.extend(quoted(expr));
            }
        }
        out
    };
    for t in &texts {
        if let Some(m) = JOINED_RE.captures(t) {
            return (1..m.len()).find_map(|i| m.get(i)).map(|g| g.as_str().to_string());
        }
    }
    None
}

fn sentence(v: Option<&Value>) -> bool {
    let Some(v) = nn(v) else {
        return false;
    };
    let texts: Vec<String> = if let Some(s) = v.as_str() {
        vec![s.to_string()]
    } else if is_slate_binding(v) {
        quoted(bind_text(v))
    } else if is_slate_format(v) {
        let parts = parse_slate_format(v.get("format").and_then(Value::as_str).unwrap_or("")).parts;
        vec![parts
            .iter()
            .map(|p| match p {
                SlateFormatPart::Text(t) => t.clone(),
                SlateFormatPart::Hole { .. } => " 0 ".to_string(),
            })
            .collect()]
    } else {
        Vec::new()
    };
    texts.iter().any(|t| split_ws(t).into_iter().filter(|w| SENTENCE_WORD_RE.is_match(w)).count() >= 3)
}

fn piece_spec(ty: Option<&str>) -> Option<&'static SlatePieceModule> {
    ty.and_then(|t| SLATE_PIECES.get(t))
}

fn cap(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn length_of(v: Option<&Value>) -> Option<usize> {
    match nn(v) {
        None => Some(0),
        Some(Value::Array(items)) => Some(items.len()),
        Some(Value::String(s)) => Some(s.encode_utf16().count()),
        Some(_) => None,
    }
}

struct Validator {
    ctx: Rc<Ctx>,
    lines: Option<SlateLines>,
    errors: Vec<SlateProblem>,
    warnings: Vec<SlateProblem>,
    seen: HashSet<String>,
    started: HashSet<String>,
}

impl Validator {
    fn new(doc: Value, lines: Option<SlateLines>) -> Validator {
        let mut kinds: IndexMap<String, Kind> = IndexMap::new();
        for (n, v) in doc.get("values").and_then(Value::as_object).into_iter().flatten() {
            kinds.insert(n.clone(), if is_true(v.get("secret")) { Kind::Secret } else { Kind::Value });
        }
        for n in doc.get("derived").and_then(Value::as_object).into_iter().flatten().map(|(n, _)| n) {
            kinds.insert(n.clone(), Kind::Derived);
        }
        for n in doc.get("runs").and_then(Value::as_object).into_iter().flatten().map(|(n, _)| n) {
            kinds.insert(n.clone(), Kind::Run);
        }
        Validator {
            ctx: Rc::new(Ctx { doc, kinds, derived_types: std::cell::RefCell::new(IndexMap::new()) }),
            lines,
            errors: Vec::new(),
            warnings: Vec::new(),
            seen: HashSet::new(),
            started: HashSet::new(),
        }
    }

    fn line_of(&self, w: &Where) -> Option<i64> {
        let lines = self.lines.as_ref()?;
        let piece = w.piece.as_ref()?;
        let mut key = match &w.prop {
            Some(p) => format!("{piece}.{p}"),
            None => piece.clone(),
        };
        loop {
            if let Some(hit) = lines.get(key.as_str()) {
                return Some(*hit);
            }
            let cut = key.rfind('.').into_iter().chain(key.rfind('[')).max().map_or(-1, |c| c as i64);
            if cut <= 0 {
                return None;
            }
            key.truncate(cut as usize);
        }
    }

    fn add(&mut self, code: &str, message: &str, w: &Where, fix: Option<&str>) {
        let fix = fix.filter(|f| !wrote(&self.ctx.doc, w, f));
        let line = self.line_of(w);
        let mut p = slate_problem(code, message, fix);
        p.piece = w.piece.clone();
        p.prop = w.prop.clone();
        p.line = line;
        if SLATE_WARNINGS.contains(&code) {
            self.warnings.push(p);
            return;
        }
        let key = format!("{code}|{}|{}|{message}", opt_text(&w.piece), opt_text(&w.prop));
        if self.seen.contains(&key) || self.errors.len() >= SLATE_LIMITS.errors_per_pass {
            return;
        }
        self.seen.insert(key);
        self.errors.push(p);
    }

    fn run(&mut self) {
        let ctx = Rc::clone(&self.ctx);
        let d = &ctx.doc;
        let schema = d.get("schema");
        if schema.and_then(Value::as_f64) != Some(2.0) {
            self.add("D200", &format!("schema {} is not a version this host knows; this host reads schema 2", js_opt(schema)), &Where::default(), Some("update wsp"));
        }
        if let Some(kit) = d.get("kit") {
            if kit.as_str() != Some("wsp/2") {
                self.add("D201", &format!("kit {} is not one this host has; it has wsp/2", js_str(kit)), &Where::default(), None);
            }
        }
        if let Some(title) = d.get("title").and_then(Value::as_str) {
            if title_cased(title) {
                self.add(
                    "W018",
                    &format!("the slate's title \"{title}\" is in Title Case; heads take sentence case"),
                    &Where::default(),
                    Some(format!("title=\"{}\"", sentence_case(title)).as_str()),
                );
            }
        }
        let runs: Vec<&Value> = d.get("runs").and_then(Value::as_object).into_iter().flatten().map(|(_, r)| r).collect();
        let mut run_texts: Vec<String> = Vec::new();
        for r in &runs {
            if r.get("kind").and_then(Value::as_str) == Some("cmd") {
                run_texts.push(js_opt(r.get("cmd")));
                run_texts.push(nn(r.get("then")).map_or_else(String::new, js_str));
            }
        }
        let token_doc = {
            let mut t = d.clone();
            if let Some(m) = t.as_object_mut() {
                m.remove("files");
                if let Some(Value::Object(rs)) = m.get_mut("runs") {
                    for r in rs.values_mut() {
                        if r.get("kind").and_then(Value::as_str) == Some("cmd") {
                            if let Some(rm) = r.as_object_mut() {
                                rm.insert("cmd".to_string(), Value::String(String::new()));
                                rm.remove("then");
                            }
                        }
                    }
                }
            }
            t
        };
        if let Some(elsewhere) = token_in(&json_stringify(&token_doc)) {
            if !run_texts.iter().any(|t| token_in(t).is_some()) {
                self.add(
                    "S520",
                    &format!("the slate holds a token written out ({elsewhere}...), which it keeps and shows the person; let a run's command read it from the file it lives in, or take it from a <secret> the person types"),
                    &Where::default(),
                    None,
                );
            }
        }
        let files: Vec<(String, String)> = d.get("files").and_then(Value::as_object).into_iter().flatten().map(|(n, t)| (n.clone(), js_str(t))).collect();
        for (file, text) in &files {
            if let Some(token) = token_in(text) {
                self.add(
                    "S520",
                    &format!("{file} holds a token written out ({token}...); let the code read it from the file it lives in, or from an env the run hands it"),
                    &Where::piece(file),
                    None,
                );
            }
        }
        let size_doc = {
            let mut t = d.clone();
            if let Some(m) = t.as_object_mut() {
                m.remove("files");
            }
            t
        };
        if json_stringify(&size_doc).len() > SLATE_LIMITS.document_bytes {
            self.add("D208", &format!("the document is over {}", fmt_bytes(SLATE_LIMITS.document_bytes as f64)), &Where::default(), None);
        }
        self.names();
        self.limits();
        for (name, v) in d.get("values").and_then(Value::as_object).into_iter().flatten() {
            self.value(name, v.get("start"), is_true(v.get("secret")), is_true(v.get("keep")));
        }
        for (name, expr) in d.get("derived").and_then(Value::as_object).into_iter().flatten() {
            self.expr(js_str(expr).as_str(), &Where::at(&format!("${name}"), "value"), ExprOpts::default());
        }
        self.derived_cycles();
        for (name, r) in d.get("runs").and_then(Value::as_object).into_iter().flatten() {
            self.run_decl(name, r);
        }
        self.files();
        for (i, r) in d.get("reactions").and_then(Value::as_array).into_iter().flatten().enumerate() {
            self.reaction(r, i);
        }
        self.reaction_cycles();
        self.tree();
        self.loud();
        for (name, r) in d.get("runs").and_then(Value::as_object).into_iter().flatten() {
            if r.get("every").is_none() && !self.started.contains(name) {
                self.add(
                    "W019",
                    &format!("nothing starts ${name}: give it a button, a when, or every"),
                    &Where::default(),
                    Some(format!("<button label=\"...\" onPress={{start(${name})}} />").as_str()),
                );
            }
        }
    }

    fn names(&mut self) {
        let ctx = Rc::clone(&self.ctx);
        for name in ctx.kinds.keys() {
            if !SLATE_NAME_RE.is_match(name) {
                self.add("P103", &format!("\"{name}\" is not a name: a lowercase letter or _, then letters, digits and _"), &Where::piece(&format!("${name}")), None);
            }
        }
        let d = &ctx.doc;
        let mut counts: IndexMap<String, usize> = IndexMap::new();
        for section in ["values", "derived", "runs"] {
            for (n, _) in d.get(section).and_then(Value::as_object).into_iter().flatten() {
                *counts.entry(n.clone()).or_insert(0) += 1;
            }
        }
        for (n, c) in &counts {
            if *c > 1 {
                self.add("P104", &format!("${n} is declared twice"), &Where::piece(&format!("${n}")), None);
            }
        }
        for (id, _) in d.get("pieces").and_then(Value::as_object).into_iter().flatten() {
            if !SLATE_ID_RE.is_match(id) {
                self.add("P103", &format!("\"{id}\" is not a piece id: letters, digits, dashes and _, starting with a letter"), &Where::piece(id), None);
            }
        }
        let reaction_ids: Vec<String> = d.get("reactions").and_then(Value::as_array).into_iter().flatten().filter_map(|r| r.get("id").and_then(Value::as_str).map(str::to_string)).collect();
        let mut unique: IndexMap<&str, ()> = IndexMap::new();
        for id in &reaction_ids {
            unique.insert(id.as_str(), ());
        }
        for id in unique.keys() {
            if reaction_ids.iter().filter(|x| x.as_str() == *id).count() > 1 {
                self.add("P104", &format!("reaction {id} is declared twice"), &Where::piece(id), None);
            }
        }
    }

    fn limits(&mut self) {
        let ctx = Rc::clone(&self.ctx);
        let d = &ctx.doc;
        let count = |section: &str| d.get(section).and_then(Value::as_object).map_or(0, Map::len);
        let over = |this: &mut Validator, n: usize, max: usize, code: &str, what: &str, fix: Option<&str>| {
            if n > max {
                this.add(code, &format!("{n} {what}; the most is {max}"), &Where::default(), fix);
            }
        };
        over(self, count("values"), SLATE_LIMITS.values, "S513", "values", None);
        over(self, count("derived"), SLATE_LIMITS.derived, "S512", "derived values", None);
        over(self, count("runs"), SLATE_LIMITS.runs, "K707", "runs", None);
        let reactions = d.get("reactions").and_then(Value::as_array).map_or(0, Vec::len);
        over(self, reactions, SLATE_LIMITS.reactions, "A611", "reactions", None);
        over(self, count("pieces"), SLATE_LIMITS.pieces, "D207", "pieces", Some("bind a list to a table or a list instead of writing rows"));
        let starts: Vec<Value> = d.get("values").and_then(Value::as_object).into_iter().flatten().map(|(_, v)| v.get("start").cloned().unwrap_or(Value::Null)).collect();
        if json_stringify(&Value::Array(starts)).len() > SLATE_LIMITS.values_bytes {
            self.add("S500", &format!("the values' starts are over {}", fmt_bytes(SLATE_LIMITS.values_bytes as f64)), &Where::default(), None);
        }
    }

    fn value(&mut self, name: &str, start: Option<&Value>, secret: bool, keep: bool) {
        let w = Where::at(&format!("${name}"), "start");
        if secret && !start.is_some_and(Value::is_null) {
            self.add("S503", &format!("${name} is a secret and starts empty; the person types it"), &w, None);
        }
        if !secret && keep {
            self.add("T302", &format!("keep is for a <secret>; ${name} is a value"), &w, None);
        }
    }

    fn expr(&mut self, src: &str, w: &Where, opts: ExprOpts) -> SlateType {
        let ctx = Rc::clone(&self.ctx);
        let checked = check_slate_expression(src, &ctx.scope(opts.row, opts.secret_ok, opts.enum_words), 0);
        for p in checked.problems {
            self.add(&p.code, &p.message, w, p.fix.as_deref());
        }
        checked.ty
    }

    fn prop_value(&mut self, v: &Value, w: &Where, opts: &ExprOpts) -> Option<SlateType> {
        if !v.is_object() {
            return None;
        }
        if is_slate_binding(v) {
            return Some(self.expr(bind_text(v), w, opts.clone()));
        }
        if is_slate_format(v) {
            let parsed = parse_slate_format(v.get("format").and_then(Value::as_str).unwrap_or(""));
            for e in &parsed.errors {
                self.add(&e.code, &e.message, w, None);
            }
            for p in &parsed.parts {
                if let SlateFormatPart::Hole { expr, .. } = p {
                    self.expr(expr, w, opts.clone());
                }
            }
            return Some(ty_of(SlateTypeName::String));
        }
        if let Some(items) = v.as_array() {
            for (i, x) in items.iter().enumerate() {
                let sub = Where { piece: w.piece.clone(), prop: Some(format!("{}[{i}]", opt_text(&w.prop))) };
                self.prop_value(x, &sub, opts);
            }
            return Some(ty_of(SlateTypeName::List));
        }
        if let Some(m) = v.as_object() {
            for (k, x) in m {
                let sub = Where { piece: w.piece.clone(), prop: Some(format!("{}.{k}", opt_text(&w.prop))) };
                self.prop_value(x, &sub, opts);
            }
            return Some(ty_of(SlateTypeName::Record));
        }
        None
    }

    fn derived_cycles(&mut self) {
        let ctx = Rc::clone(&self.ctx);
        let derived: Vec<(String, String)> = ctx.doc.get("derived").and_then(Value::as_object).into_iter().flatten().map(|(n, e)| (n.clone(), js_str(e))).collect();
        let mut deps: IndexMap<String, Vec<String>> = IndexMap::new();
        for (name, expr) in &derived {
            let roots_of: Vec<String> = roots(&slate_dependencies(expr)).into_iter().filter(|n| ctx.derived_expr(n).is_some()).collect();
            deps.insert(name.clone(), roots_of);
        }
        let mut state: IndexMap<String, bool> = IndexMap::new();
        let mut reported: HashSet<String> = HashSet::new();
        for n in deps.keys().cloned().collect::<Vec<_>>() {
            self.derived_visit(&n, &mut Vec::new(), &deps, &mut state, &mut reported);
        }
    }

    fn derived_visit(&mut self, n: &str, stack: &mut Vec<String>, deps: &IndexMap<String, Vec<String>>, state: &mut IndexMap<String, bool>, reported: &mut HashSet<String>) {
        match state.get(n) {
            Some(&true) => return,
            Some(&false) => {
                let start = stack.iter().position(|s| s == n).unwrap_or(0);
                let mut cycle: Vec<String> = stack[start..].to_vec();
                cycle.push(n.to_string());
                let mut sorted = cycle.clone();
                sorted.sort();
                let key = sorted.join(",");
                if !reported.contains(&key) {
                    reported.insert(key);
                    let shown: Vec<String> = cycle.iter().map(|x| format!("${x}")).collect();
                    self.add(
                        "S510",
                        &format!("derived values read each other in a cycle: {}", shown.join(" reads ")),
                        &Where::at(&format!("${n}"), "value"),
                        Some("make one of them a value"),
                    );
                }
                return;
            }
            None => {}
        }
        state.insert(n.to_string(), false);
        stack.push(n.to_string());
        for m in deps.get(n).cloned().unwrap_or_default() {
            self.derived_visit(&m, stack, deps, state, reported);
        }
        stack.pop();
        state.insert(n.to_string(), true);
    }

    fn run_decl(&mut self, name: &str, r: &Value) {
        let piece = format!("${name}");
        let w = |prop: &str| Where::at(&piece, prop);
        if let Some(every) = r.get("every") {
            if !every.as_f64().is_some_and(|x| x.is_finite() && x >= SLATE_LIMITS.timer_floor_s as f64) {
                self.add("K703", &format!("every is at least {} s", SLATE_LIMITS.timer_floor_s), &w("every"), Some(format!("every={{{}}}", SLATE_LIMITS.timer_floor_s).as_str()));
            }
        }
        if let Some(confirm) = nn(r.get("confirm")).or_else(|| r.get("confirm").filter(|c| !c.is_null())) {
            self.prop_value(confirm, &w("confirm"), &ExprOpts::default());
        }
        if let Some(then) = r.get("then") {
            if then.as_str().map_or(true, |t| js_trim(t).is_empty()) {
                self.add("K700", "then is a literal command that reads the result on stdin", &w("then"), Some("then='python3 shape.py'"));
            }
        }
        if is_true(r.get("always")) && r.get("every").is_none() {
            self.add("K703", "always keeps a timer running while the slate is not shown, so it needs every", &w("always"), Some("every={60} always"));
        }
        let kind = r.get("kind").and_then(Value::as_str);
        if kind == Some("cmd") {
            let cmd = r.get("cmd");
            if cmd.and_then(Value::as_str).map_or(true, |c| js_trim(c).is_empty()) {
                self.add("K700", "the command is literal text; hand values to it with env={{ NAME: $value }}", &w("cmd"), None);
            }
            for (field, text) in [("cmd", r.get("cmd")), ("then", r.get("then"))] {
                if let Some(token) = text.and_then(Value::as_str).and_then(token_in) {
                    self.add(
                        "S520",
                        &format!("{field} holds a token written out ({token}...), which the slate keeps and shows the person; let the command read it from the file it lives in, or take it from a <secret> the person types"),
                        &w(field),
                        None,
                    );
                }
            }
            if let Some(timeout) = r.get("timeout") {
                if !timeout.as_f64().is_some_and(|t| t.is_finite() && t > 0.0 && t <= SLATE_LIMITS.timeout_max_s as f64) {
                    self.add("K704", &format!("timeout is seconds up to {}", SLATE_LIMITS.timeout_max_s), &w("timeout"), Some(format!("timeout={{{}}}", SLATE_LIMITS.timeout_default_s).as_str()));
                }
            }
            if let Some(on) = r.get("on") {
                if on.as_str() != Some("thread") && on.as_str() != Some("host") {
                    self.add("K704", "on is \"thread\" or \"host\"", &w("on"), None);
                }
            }
            let mut secret_env: Vec<(String, String)> = Vec::new();
            for (k, v) in r.get("env").and_then(Value::as_object).into_iter().flatten() {
                if !ENV_NAME_RE.is_match(k) {
                    self.add("K702", &format!("{k} is not an environment variable name: {ENV_NAME_FIX}"), &w("env"), Some(k.to_uppercase().chars().map(|c| if c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_' { c } else { '_' }).collect::<String>().as_str()));
                }
                self.prop_value(v, &w(&format!("env.{k}")), &ExprOpts { secret_ok: true, ..ExprOpts::default() });
                for s in self.secrets_in(v) {
                    secret_env.push((k.clone(), s));
                }
            }
            for (i, v) in r.get("args").and_then(Value::as_array).into_iter().flatten().enumerate() {
                let secret = if is_slate_binding(v) { slate_own_name(bind_text(v)) } else { None };
                let secret_kind = secret.as_ref().and_then(|s| self.ctx.kinds.get(s).copied());
                match secret {
                    Some(s) if secret_kind == Some(Kind::Secret) => {
                        self.add(
                            "S520",
                            &format!("args[{i}] is the secret ${s}; {SLATE_SECRET_IN_ARGS}"),
                            &w(&format!("args[{i}]")),
                            Some(format!("env={{{{ {}: ${s} }}}}, or stdin={{${s}}}", s.to_uppercase()).as_str()),
                        );
                    }
                    _ => {
                        self.prop_value(v, &w(&format!("args[{i}]")), &ExprOpts::default());
                    }
                }
            }
            if let Some(stdin) = r.get("stdin") {
                self.prop_value(stdin, &w("stdin"), &ExprOpts { secret_ok: true, ..ExprOpts::default() });
            }
            let cmd_text = js_opt(r.get("cmd"));
            for (env_name, secret) in &secret_env {
                if let Some(m) = flag_re(env_name).captures(&cmd_text) {
                    let flag = m.get(1).map_or("", |g| g.as_str());
                    self.add(
                        "W011",
                        &format!("{env_name} carries the secret ${secret}, and the command passes it after {flag}, as an argument ps can read; hand it on stdin={{${secret}}}, or let the program read {env_name} from its environment"),
                        &w("cmd"),
                        Some(format!("drop {flag} \"${env_name}\" and use stdin={{${secret}}} or the program's own {env_name}").as_str()),
                    );
                }
            }
        } else if kind == Some("tool") {
            let server = r.get("server").and_then(Value::as_str).unwrap_or("");
            let tool = r.get("tool").and_then(Value::as_str).unwrap_or("");
            if server.is_empty() || tool.is_empty() {
                self.add("K705", "tool names a server and a tool as \"server.tool\"", &w("tool"), None);
            }
            for (k, v) in r.get("args").and_then(Value::as_object).into_iter().flatten() {
                self.prop_value(v, &w(&format!("args.{k}")), &ExprOpts { secret_ok: true, ..ExprOpts::default() });
            }
        } else if kind == Some("resource") {
            let server = r.get("server").and_then(Value::as_str).unwrap_or("");
            let uri = r.get("uri").and_then(Value::as_str).unwrap_or("");
            if server.is_empty() || uri.is_empty() {
                self.add("K705", "resource names a server and a uri as \"server:uri\"", &w("resource"), None);
            }
        } else {
            self.add("K701", &format!("{piece} takes exactly one of cmd, tool or resource"), &w("kind"), None);
        }
    }

    fn files(&mut self) {
        let ctx = Rc::clone(&self.ctx);
        let files: Vec<(String, String)> = ctx.doc.get("files").and_then(Value::as_object).into_iter().flatten().map(|(n, t)| (n.clone(), js_str(t))).collect();
        let names: Vec<String> = files.iter().map(|(n, _)| n.clone()).collect();
        if files.len() > SLATE_LIMITS.files {
            self.add("K709", &format!("{} files; the most is {}", files.len(), SLATE_LIMITS.files), &Where::default(), None);
        }
        let bytes: usize = files.iter().map(|(_, t)| t.len()).sum();
        if bytes > SLATE_LIMITS.files_bytes {
            self.add(
                "K709",
                &format!("the files hold {} KB; the most is {} KB in all", bytes.div_ceil(1024), SLATE_LIMITS.files_bytes / 1024),
                &Where::default(),
                Some("keep code that belongs to the project in the project and call it where it is"),
            );
        }
        let secrets: Vec<String> = ctx.kinds.iter().filter(|(_, k)| **k == Kind::Secret).map(|(n, _)| n.clone()).collect();
        for (name, text) in &files {
            if !SLATE_FILE_RE.is_match(name) {
                let fix: String = name
                    .rsplit('/')
                    .next()
                    .unwrap_or("")
                    .trim_start_matches('.')
                    .chars()
                    .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' { c } else { '_' })
                    .collect();
                self.add(
                    "K708",
                    &format!("\"{name}\" is not a file name: letters, digits, dots, dashes and _, no slashes, not starting with a dot"),
                    &Where::piece(name),
                    if fix.is_empty() { None } else { Some(fix.as_str()) },
                );
            }
            for secret in &secrets {
                if secret_ref_re(secret).is_match(text) {
                    self.add(
                        "S520",
                        &format!("{name} names the secret ${secret}; a file is stored as written, so a run hands the secret to it on stdin or in env"),
                        &Where::piece(name),
                        Some(format!("env={{{{ {}: ${secret} }}}} on the run, read from the environment in {name}", secret.to_uppercase()).as_str()),
                    );
                }
            }
        }
        for (run, r) in ctx.doc.get("runs").and_then(Value::as_object).into_iter().flatten() {
            for prop in ["cmd", "then"] {
                let Some(text) = r.get(prop).and_then(Value::as_str) else {
                    continue;
                };
                for m in SLATE_DIR_RE.captures_iter(text) {
                    let file = m.get(1).map_or("", |g| g.as_str());
                    if !files.iter().any(|(n, _)| n == file) {
                        let fix = slate_nearest(file, &names);
                        self.add(
                            "W015",
                            &format!("${run} runs $SLATE_DIR/{file}, and no <file name=\"{file}\"> is declared"),
                            &Where::at(&format!("${run}"), prop),
                            fix.as_deref(),
                        );
                    }
                }
            }
        }
    }

    fn secrets_in(&self, v: &Value) -> Vec<String> {
        let mut out = Vec::new();
        self.secrets_visit(v, &mut out);
        out
    }

    fn secrets_visit(&self, x: &Value, out: &mut Vec<String>) {
        if !x.is_object() && !x.is_array() {
            return;
        }
        if is_slate_binding(x) {
            for n in roots(&slate_dependencies(bind_text(x))) {
                if self.ctx.kinds.get(&n) == Some(&Kind::Secret) {
                    out.push(n);
                }
            }
            return;
        }
        if is_slate_format(x) {
            for p in parse_slate_format(x.get("format").and_then(Value::as_str).unwrap_or("")).parts {
                if let SlateFormatPart::Hole { expr, .. } = p {
                    for n in roots(&slate_dependencies(&expr)) {
                        if self.ctx.kinds.get(&n) == Some(&Kind::Secret) {
                            out.push(n);
                        }
                    }
                }
            }
            return;
        }
        match x {
            Value::Array(items) => items.iter().for_each(|y| self.secrets_visit(y, out)),
            Value::Object(m) => m.values().for_each(|y| self.secrets_visit(y, out)),
            _ => {}
        }
    }

    fn reaction(&mut self, r: &Value, i: usize) {
        let key = match nn(r.get("id")).and_then(Value::as_str) {
            Some(id) => id.to_string(),
            None => format!("reaction-{}", i + 1),
        };
        let on = r.get("on");
        let change = on.and_then(|o| o.get("change"));
        let done = on.and_then(|o| o.get("done"));
        if change.is_none() == done.is_none() {
            self.add("A611", "<when> takes exactly one of change={...} or done={$run}", &Where::piece(&key), None);
            return;
        }
        if let Some(paths) = change {
            let paths: Vec<String> = paths.as_array().map_or_else(Vec::new, |items| items.iter().map(js_str).collect());
            if paths.is_empty() {
                self.add("A611", "change lists at least one path", &Where::at(&key, "change"), None);
            }
            for p in &paths {
                if !matches!(parse_slate_expression(p).ast, Some(SlateExpr::Path { .. })) {
                    self.add("A601", &format!("change lists paths, not formulas: {p}"), &Where::at(&key, "change"), None);
                    continue;
                }
                self.expr(p, &Where::at(&key, "change"), ExprOpts { secret_ok: true, ..ExprOpts::default() });
            }
        } else if let Some(name) = done.and_then(Value::as_str) {
            if self.ctx.kinds.get(name) != Some(&Kind::Run) {
                let word = kind_word_of(self.ctx.kinds.get(name).copied()).unwrap_or("not declared");
                let runs: Vec<String> = self.run_names().into_iter().map(|n| format!("${n}")).collect();
                self.add(
                    "K702",
                    &format!("done names a run: ${name} is {word}"),
                    &Where::at(&key, "done"),
                    slate_nearest(name, &runs).as_deref(),
                );
            }
        }
        self.steps(r.get("do"), &Where::at(&key, "do"), "reaction", None);
    }

    fn run_names(&self) -> Vec<String> {
        self.ctx.kinds.iter().filter(|(_, k)| **k == Kind::Run).map(|(n, _)| n.clone()).collect()
    }

    fn steps(&mut self, steps: Option<&Value>, w: &Where, trigger: &str, row: Option<SlateType>) {
        let Some(steps) = steps.and_then(Value::as_array) else {
            self.add("A600", "a handler is a list of steps", w, None);
            return;
        };
        if steps.len() > SLATE_LIMITS.steps_per_reaction {
            self.add(
                "A606",
                &format!("{} steps in one handler; the most is {}", steps.len(), SLATE_LIMITS.steps_per_reaction),
                w,
                Some("split it: a second <when> on what the first one sets"),
            );
        }
        for s in steps {
            let do_text = js_opt(s.get("do"));
            let Some(module) = s.get("do").and_then(Value::as_str).and_then(slate_step_module) else {
                let fix = slate_nearest(&do_text, &STEP_KINDS.iter().map(|k| k.to_string()).collect::<Vec<_>>());
                self.add("A600", &format!("{do_text} is not a step; steps are {}", STEP_KINDS.join(", ")), w, fix.as_deref());
                continue;
            };
            if module.runs == "window" && trigger != "press" {
                let on = if trigger == "reaction" { "reaction".to_string() } else { format!("{trigger} handler") };
                self.add("A608", &format!("{do_text} runs in the window and goes on a press, not on a {on}"), w, Some(format!("onPress={{{do_text}(...)}}").as_str()));
            }
            match do_text.as_str() {
                "set" | "toggle" => {
                    let path = js_opt(s.get("path"));
                    let Some(p) = parse_slate_own_path(&path) else {
                        self.add("A601", &format!("{do_text} writes the slate's own values: a $name path, not {path}"), w, None);
                        continue;
                    };
                    match self.ctx.kinds.get(&p.name).copied() {
                        None => {
                            let names: Vec<String> = self.ctx.kinds.keys().cloned().collect();
                            self.add("S501", &format!("${} is not declared; add <value name=\"{}\" start=... />", p.name, p.name), w, slate_nearest(&p.name, &names).as_deref());
                        }
                        Some(Kind::Derived) => {
                            self.add("A607", &format!("${} is derived and cannot be written; write the value it reads, or make it a value", p.name), w, None);
                        }
                        Some(Kind::Run) => {
                            self.add("A607", &format!("${} is a run; its result is the host's to write", p.name), w, None);
                        }
                        Some(Kind::Secret) => {
                            self.add("S520", &format!("${} is a secret; only the person's typing fills it", p.name), w, None);
                        }
                        Some(Kind::Value) => {}
                    }
                    if do_text == "set" {
                        self.prop_value(s.get("value").unwrap_or(&Value::Null), w, &ExprOpts { row: row.clone(), ..ExprOpts::default() });
                    }
                }
                "start" | "cancel" => {
                    let run = js_opt(s.get("run"));
                    if do_text == "start" {
                        self.started.insert(run.clone());
                    }
                    if self.ctx.kinds.get(&run) != Some(&Kind::Run) {
                        let word = kind_word_of(self.ctx.kinds.get(&run).copied()).unwrap_or("not declared");
                        let runs = self.run_names();
                        self.add(
                            "K702",
                            &format!("{do_text} names a run: ${run} is {word}"),
                            w,
                            slate_nearest(&run, &runs).as_deref(),
                        );
                    }
                }
                "send" | "steer" | "queue" | "fill" => {
                    if s.get("text").and_then(Value::as_str).is_none() {
                        self.add("A603", &format!("{do_text}'s text is a literal string"), w, None);
                    }
                    for p in s.get("with").and_then(Value::as_array).into_iter().flatten().map(js_str) {
                        if !matches!(parse_slate_expression(&p).ast, Some(SlateExpr::Path { .. })) {
                            self.add("A601", &format!("{do_text} carries paths after its text; {p} is a formula"), w, None);
                            continue;
                        }
                        self.expr(&p, w, ExprOpts { secret_ok: true, row: row.clone(), ..ExprOpts::default() });
                    }
                }
                "open" => {
                    if let Some(target) = s.get("target") {
                        self.prop_value(target, w, &ExprOpts { row: row.clone(), ..ExprOpts::default() });
                    }
                }
                "copy" => {
                    if let Some(text) = s.get("text") {
                        self.prop_value(text, w, &ExprOpts { row: row.clone(), ..ExprOpts::default() });
                    }
                }
                "pane" => {
                    let kind = js_opt(s.get("kind"));
                    if !PANE_KINDS.contains(&kind.as_str()) {
                        self.add(
                            "A601",
                            &format!("pane names one of {}", PANE_KINDS.join(", ")),
                            w,
                            slate_nearest(&kind, &PANE_KINDS.iter().map(|k| k.to_string()).collect::<Vec<_>>()).as_deref(),
                        );
                    }
                }
                _ => {}
            }
        }
    }

    fn reaction_cycles(&mut self) {
        let ctx = Rc::clone(&self.ctx);
        let mut edges: Vec<(String, String, String)> = Vec::new();
        for (name, expr) in ctx.doc.get("derived").and_then(Value::as_object).into_iter().flatten() {
            for dep in slate_dependencies(&js_str(expr)) {
                edges.push((dep.clone(), format!("${name}"), format!("${name} reads {dep}")));
            }
        }
        for r in ctx.doc.get("reactions").and_then(Value::as_array).into_iter().flatten() {
            let Some(change) = r.get("on").and_then(|o| o.get("change")).and_then(Value::as_array) else {
                continue;
            };
            let targets: Vec<String> = r
                .get("do")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|s| matches!(s.get("do").and_then(Value::as_str), Some("set" | "toggle")))
                .map(|s| js_opt(s.get("path")))
                .collect();
            for t in change.iter().map(js_str) {
                for g in &targets {
                    edges.push((t.clone(), g.clone(), format!("when change of {t} sets {g}")));
                }
            }
        }
        let mut state: IndexMap<String, bool> = IndexMap::new();
        let mut reported = false;
        let froms: Vec<String> = edges.iter().map(|e| e.0.clone()).collect();
        for from in froms {
            self.cycle_visit(&from, &mut Vec::new(), &edges, &mut state, &mut reported);
        }
    }

    fn cycle_visit(&mut self, n: &str, stack: &mut Vec<(String, String)>, edges: &[(String, String, String)], state: &mut IndexMap<String, bool>, reported: &mut bool) {
        if *reported || state.get(n) == Some(&true) {
            return;
        }
        if state.get(n) == Some(&false) {
            let idx = stack.iter().position(|(node, _)| node == n).map_or(0, |i| i + 1);
            let said: Vec<String> = stack[idx..].iter().map(|(_, why)| why.clone()).collect();
            if said.iter().any(|s| s.starts_with("when")) {
                *reported = true;
                self.add("A610", &format!("reactions and derived values form a cycle: {}", said.join("; ")), &Where::default(), Some("compute it as a <derived> value instead of setting it back"));
            }
            return;
        }
        state.insert(n.to_string(), false);
        let next: Vec<(String, String)> = edges.iter().filter(|e| paths_overlap(&e.0, n)).map(|e| (e.1.clone(), e.2.clone())).collect();
        for (to, why) in next {
            let mut sub = stack.clone();
            if stack.is_empty() {
                sub.push((n.to_string(), String::new()));
            }
            sub.push((to.clone(), why));
            self.cycle_visit(&to, &mut sub, edges, state, reported);
        }
        state.insert(n.to_string(), true);
    }

    fn tree(&mut self) {
        let ctx = Rc::clone(&self.ctx);
        let d = &ctx.doc;
        let pieces = d.get("pieces").and_then(Value::as_object);
        let root = d.get("root").and_then(Value::as_str).unwrap_or("");
        let piece_ids: Vec<String> = pieces.into_iter().flatten().map(|(k, _)| k.clone()).collect();
        if pieces.and_then(|p| p.get(root)).is_none() {
            let shown = if root.is_empty() { "(none)" } else { root };
            self.add("D203", &format!("the root {shown} is not a piece"), &Where::default(), slate_nearest(root, &piece_ids).as_deref());
            return;
        }
        let mut placed: HashSet<String> = HashSet::new();
        self.walk(root, 1, &[], None, &mut placed);
    }

    fn walk(&mut self, id: &str, depth: usize, path: &[String], row: Option<SlateType>, placed: &mut HashSet<String>) {
        let ctx = Rc::clone(&self.ctx);
        let pieces = ctx.doc.get("pieces").and_then(Value::as_object);
        if path.iter().any(|p| p == id) {
            let mut chain: Vec<&str> = path.iter().map(String::as_str).collect();
            chain.push(id);
            self.add("D205", &format!("{id} is under itself: {}", chain.join(" > ")), &Where::piece(id), None);
            return;
        }
        if placed.contains(id) {
            self.add("D205", &format!("{id} sits in two places"), &Where::piece(id), None);
            return;
        }
        placed.insert(id.to_string());
        if depth > SLATE_LIMITS.depth {
            self.add("D206", &format!("the tree is deeper than {}", SLATE_LIMITS.depth), &Where::piece(id), None);
            return;
        }
        let Some(p) = pieces.and_then(|m| m.get(id)) else {
            return;
        };
        let child_row = self.piece(id, p, row.clone());
        let piece_ids: Vec<String> = pieces.into_iter().flatten().map(|(k, _)| k.clone()).collect();
        let mut sub_path = path.to_vec();
        sub_path.push(id.to_string());
        if let Some(fallback) = p.get("fallback").and_then(Value::as_str) {
            if fallback != "drop" {
                if pieces.and_then(|m| m.get(fallback)).is_none() {
                    self.add("D203", &format!("fallback names {fallback}, which is not a piece"), &Where::at(id, "fallback"), slate_nearest(fallback, &piece_ids).as_deref());
                } else {
                    self.walk(fallback, depth + 1, &sub_path, row.clone(), placed);
                }
            }
        }
        let children: Vec<String> = p.get("children").and_then(Value::as_array).into_iter().flatten().map(js_str).collect();
        if children.len() > SLATE_LIMITS.children {
            self.add("D207", &format!("{id} has {} children; the most is {}", children.len(), SLATE_LIMITS.children), &Where::piece(id), Some("bind a list instead"));
        }
        for c in &children {
            if pieces.and_then(|m| m.get(c)).is_none() {
                self.add("D203", &format!("{id} holds {c}, which is not a piece"), &Where::piece(id), slate_nearest(c, &piece_ids).as_deref());
                continue;
            }
            self.walk(c, depth + 1, &sub_path, child_row.clone(), placed);
        }
    }

    fn piece(&mut self, id: &str, p: &Value, row: Option<SlateType>) -> Option<SlateType> {
        let ctx = Rc::clone(&self.ctx);
        let ty = p.get("type").and_then(Value::as_str);
        let Some(spec) = piece_spec(ty) else {
            if p.get("fallback").is_none() {
                let tag = ty.unwrap_or("undefined");
                let names: Vec<String> = SLATE_PIECES.keys().map(|k| k.to_string()).collect();
                let unknown = slate_unknown_piece(tag, &names);
                self.add("T300", &unknown.message, &Where::piece(id), unknown.fix.as_deref());
            }
            return row;
        };
        if let Some(when) = p.get("when").and_then(Value::as_str) {
            self.expr(when, &Where::at(id, "when"), ExprOpts { row: row.clone(), ..ExprOpts::default() });
        }
        let empty = Map::new();
        let props = p.get("props").and_then(Value::as_object).unwrap_or(&empty);
        let row_type = if spec.repeating { Some(self.items_row(props.get("items"), row.as_ref())) } else { None };
        let ty_name = ty.unwrap_or("undefined");
        for (name, v) in props {
            let item = spec.items.iter().filter(|(_, s)| s.prop == name.as_str()).last();
            let scalars = v.as_array().is_some_and(|items| items.iter().all(|x| x.is_string() || x.is_number()));
            if let Some((tag, item_spec)) = item {
                if !(spec.props.get(name.as_str()).is_some() && (scalars || is_bound(v))) {
                    let item_row = row_type.clone().or_else(|| row.clone());
                    self.items(id, name, v, tag, item_spec, item_row, row.clone());
                    continue;
                }
            }
            let Some(ps) = spec.props.get(name.as_str()) else {
                if let Some(reserved) = slate_reserved_prop(name) {
                    self.add("T312", &format!("{name} is not a prop a slate can set; wsp draws the style. Use {reserved}."), &Where::at(id, name), Some(reserved));
                } else if RUN_ATTRS.contains(&name.as_str()) {
                    let example = if name == "every" { "every={60}".to_string() } else { format!("{name}=...") };
                    self.add(
                        "T302",
                        &format!("{name} is a <run>'s, not a piece's: <run name=\"x\" cmd='...' {example} />, and the piece reads $x"),
                        &Where::at(id, name),
                        None,
                    );
                } else {
                    let keys: Vec<String> = spec.props.keys().map(|k| k.to_string()).collect();
                    let fix = slate_nearest(name, &keys);
                    let hint = match &fix {
                        Some(f) => format!("; did you mean {f}?"),
                        None => format!("; it takes {}", keys.join(", ")),
                    };
                    self.add("T302", &format!("{ty_name} has no {name}{hint}"), &Where::at(id, name), fix.as_deref());
                }
                continue;
            };
            let ps: &SlatePropSpec = ps;
            let prop_row = if ps.binds == SlateBinds::Item { row_type.clone() } else { row.clone() };
            self.prop(ty_name, ps, v, &Where::at(id, name), prop_row);
        }
        for (name, ps) in &spec.props {
            if !ps.required || props.get(*name).is_some() {
                continue;
            }
            if spec.interactive && *name == "label" {
                self.add("T307", &format!("a {ty_name} needs a label for people using a screen reader"), &Where::at(id, name), Some("label=\"...\""));
            } else {
                self.add("T304", &format!("{ty_name} \"{id}\" needs {name}"), &Where::at(id, name), None);
            }
        }
        for (tag, is) in &spec.items {
            let n = props.get(is.prop).and_then(Value::as_array).map_or(0, Vec::len);
            if let Some(min) = is.min {
                if n < min && !(is.prop == "options" && props.get("options").is_some()) {
                    self.add("T304", &format!("{ty_name} needs at least {min} <{tag}>"), &Where::at(id, is.prop), None);
                }
            }
            if let Some(max) = is.max {
                if n > max {
                    self.add("T309", &format!("{ty_name} takes at most {max} <{tag}>"), &Where::at(id, is.prop), None);
                }
            }
        }
        for (event, steps) in p.get("on").and_then(Value::as_object).into_iter().flatten() {
            let prop = format!("on{}", cap(event));
            if !spec.events.contains(&event.as_str()) {
                let has = if spec.events.is_empty() {
                    String::new()
                } else {
                    format!("; it has {}", spec.events.iter().map(|e| format!("on{}", cap(e))).collect::<Vec<_>>().join(", "))
                };
                self.add("A602", &format!("{ty_name} has no {prop}{has}"), &Where::at(id, &prop), None);
                continue;
            }
            self.steps(Some(steps), &Where::at(id, &prop), event, row.clone());
        }
        if let Some(needs) = spec.needs_handler {
            let handler_len = length_of(p.get("on").and_then(|o| o.get(needs)));
            if handler_len == Some(0) {
                self.add("A602", &format!("a {ty_name} needs on{}", cap(needs)), &Where::piece(id), Some("onPress={send(\"...\")}"));
            }
        }
        let children: Vec<String> = p.get("children").and_then(Value::as_array).into_iter().flatten().map(js_str).collect();
        if !spec.holds_children && !children.is_empty() {
            self.add("T308", &format!("{ty_name} holds no children"), &Where::piece(id), None);
        }
        if let Some(limit) = &spec.child_limit {
            if children.len() > limit.max {
                let word = if limit.max == 1 { "child" } else { "children" };
                self.add("T309", &format!("{ty_name} takes at most {} {word}", limit.max), &Where::piece(id), None);
            }
            if !limit.types.is_empty() {
                for c in &children {
                    let t = ctx.doc.get("pieces").and_then(|m| m.get(c)).and_then(|x| x.get("type")).and_then(Value::as_str);
                    if let Some(t) = t {
                        if !limit.types.contains(&t) {
                            self.add("T309", &format!("{ty_name} takes {} children, not {t}", or_list(limit.types)), &Where::piece(id), None);
                        }
                    }
                }
            }
        }
        if is_true(props.get("mono")) {
            if let Some(text_prop) = spec.text_prop {
                if sentence(props.get(text_prop)) {
                    self.add("W014", MONO_WARNING, &Where::at(id, "mono"), Some("drop mono"));
                }
            }
        }
        if let Some(check) = spec.check {
            let props_map: IndexMap<String, Value> = props.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            let kinds = Rc::clone(&ctx);
            let kind_of = move |n: &str| kind_word_of(kinds.kinds.get(n).copied()).map(str::to_string);
            let mut checked = SlatePieceCheck { props: &props_map, kind: &kind_of, problems: Vec::new() };
            check(&mut checked);
            let problems = checked.problems;
            for p in problems {
                let w = Where { piece: Some(id.to_string()), prop: p.prop.clone() };
                self.add(&p.code, &p.message, &w, p.fix.as_deref());
            }
        }
        row
    }

    fn items_row(&self, items: Option<&Value>, row: Option<&SlateType>) -> SlateType {
        let Some(items) = items.filter(|v| is_slate_binding(v)) else {
            return any_t();
        };
        let checked = check_slate_expression(bind_text(items), &self.ctx.scope(row.cloned(), false, None), 0);
        match checked.ty {
            SlateType { t: SlateTypeName::List, of, .. } => of.map_or_else(any_t, |b| *b),
            _ => any_t(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn items(&mut self, id: &str, prop: &str, v: &Value, tag: &str, spec: &SlateItemSpec, row: Option<SlateType>, outer: Option<SlateType>) {
        let Some(list) = v.as_array() else {
            self.add("T303", &format!("{prop} is a list of <{tag}> items"), &Where::at(id, prop), None);
            return;
        };
        for (i, it) in list.iter().enumerate() {
            let wh = format!("{prop}[{i}]");
            if !is_record_value(it) {
                self.add("T303", &format!("each <{tag}> is a record"), &Where::at(id, &wh), None);
                continue;
            }
            let Some(fields) = it.as_object() else {
                continue;
            };
            let item_row = if spec.row { row.clone() } else { None };
            for (k, x) in fields {
                if k == "on" {
                    if let Some(events) = spec.events {
                        for (ev, steps) in nn(Some(x)).map_or_else(Vec::new, js_entries) {
                            if !events.contains(&ev.as_str()) {
                                self.add("A602", &format!("<{tag}> has no on{ev}"), &Where::at(id, &format!("{wh}.on")), None);
                                continue;
                            }
                            self.steps(Some(steps), &Where::at(id, &format!("{wh}.onPress")), "press", item_row.clone());
                        }
                        continue;
                    }
                }
                if k == "when" {
                    let when_row = if spec.row_when { item_row.clone() } else { outer.clone() };
                    match x.as_str() {
                        Some(text) => {
                            self.expr(text, &Where::at(id, &format!("{wh}.when")), ExprOpts { row: when_row, ..ExprOpts::default() });
                        }
                        None => self.add("T303", "when is a formula: when={...}", &Where::at(id, &format!("{wh}.when")), None),
                    }
                    continue;
                }
                let Some(fs) = spec.fields.get(k.as_str()) else {
                    if let Some(reserved) = slate_reserved_prop(k) {
                        self.add("T312", &format!("{k} is not a prop a slate can set; use {reserved}"), &Where::at(id, &format!("{wh}.{k}")), Some(reserved));
                    } else {
                        let keys: Vec<String> = spec.fields.keys().map(|f| f.to_string()).collect();
                        let fix = slate_nearest(k, &keys);
                        self.add("T302", &format!("<{tag}> has no {k}; it takes {}", keys.join(", ")), &Where::at(id, &format!("{wh}.{k}")), fix.as_deref());
                    }
                    continue;
                };
                let field_row = if fs.binds == SlateBinds::Item || spec.row { item_row.clone() } else { row.clone() };
                self.prop(tag, fs, x, &Where::at(id, &format!("{wh}.{k}")), field_row);
            }
            for (k, fs) in &spec.fields {
                if fs.required && it.get(*k).is_none() {
                    self.add("T304", &format!("<{tag}> needs {k}"), &Where::at(id, &wh), None);
                }
            }
            if is_true(it.get("mono")) && sentence(it.get("value")) {
                self.add("W014", MONO_WARNING, &Where::at(id, &format!("{wh}.mono")), Some("drop mono"));
            }
        }
    }

    fn prop(&mut self, ty: &str, ps: &SlatePropSpec, v: &Value, w: &Where, row: Option<SlateType>) {
        let name = w.prop.as_deref().unwrap_or("").rsplit('.').next().unwrap_or("").to_string();
        let enum_words: Option<Vec<String>> = match &ps.prop_type {
            SlatePropType::Enum(words) => Some(words.iter().map(|s| s.to_string()).collect()),
            _ => None,
        };
        if v.is_null() {
            return;
        }
        let mark = if ps.marks || ps.binds == SlateBinds::State { None } else { joining_mark(v) };
        if let Some(mark) = mark {
            self.add(
                "W012",
                &format!("{name} joins words with \"{mark}\"; the kit separates things by layout, never a mark: give each its own piece, or join with a comma"),
                w,
                Some("separate pieces (facts, chips, a row) or a comma"),
            );
        }
        let opts = ExprOpts { row: row.clone(), secret_ok: false, enum_words: enum_words.clone() };
        if is_bound(v) {
            if ps.binds == SlateBinds::No {
                self.add("T305", &format!("{name} takes a literal, not a formula"), w, None);
                return;
            }
            if ps.binds == SlateBinds::State {
                let bind = if is_slate_binding(v) { js_trim(bind_text(v)).to_string() } else { String::new() };
                let Some(m) = slate_own_name(&bind) else {
                    let into = if name == "value" { "picked" } else { name.as_str() };
                    self.add(
                        "X410",
                        &format!("{name} on {ty} writes back, so it binds one plain value, not {bind}; bind a value of its own and keep a list with append in a <when> if you need one per item"),
                        w,
                        Some(format!("<value name=\"{into}\" start={{null}} /> and {name}={{${into}}}").as_str()),
                    );
                    return;
                };
                let kind = self.ctx.kinds.get(&m).copied();
                let Some(kind) = kind else {
                    let names: Vec<String> = self.ctx.kinds.keys().cloned().collect();
                    self.add("S501", &format!("${m} is not declared; add <value name=\"{m}\" start=... />"), w, slate_nearest(&m, &names).as_deref());
                    return;
                };
                if kind == Kind::Secret && ty != "input" {
                    self.add("S520", "a secret is typed into an input and nowhere else", w, None);
                    return;
                }
                if kind != Kind::Value && kind != Kind::Secret {
                    let what = if kind == Kind::Derived { "derived" } else { "a run" };
                    self.add("X410", &format!("{name} on {ty} writes a value; ${m} is {what}"), w, None);
                }
                return;
            }
            let t = self.prop_value(v, w, &opts);
            if let Some(t) = t {
                let wants_number = matches!(ps.prop_type, SlatePropType::Number | SlatePropType::Integer);
                if wants_number && matches!(t.t, SlateTypeName::String | SlateTypeName::List | SlateTypeName::Record | SlateTypeName::Boolean) {
                    let (text, fix) = match t.t {
                        SlateTypeName::String => ("text".to_string(), Some("num(...)")),
                        SlateTypeName::List => ("a list".to_string(), Some("len(...)")),
                        SlateTypeName::Record => ("a record".to_string(), None),
                        _ => ("a boolean".to_string(), None),
                    };
                    self.add("X408", &format!("{name} takes a number; this formula gives {text}"), w, fix);
                }
                if matches!(ps.prop_type, SlatePropType::List) && matches!(t.t, SlateTypeName::String | SlateTypeName::Number | SlateTypeName::Boolean) {
                    let text = match t.t {
                        SlateTypeName::String => "text".to_string(),
                        SlateTypeName::Number => "a number".to_string(),
                        _ => "a boolean".to_string(),
                    };
                    self.add("X408", &format!("{name} takes a list; this formula gives {text}"), w, None);
                }
            }
            return;
        }
        if ps.binds == SlateBinds::State && ps.literal && ps.prop_type == SlatePropType::Boolean && v.is_boolean() {
            return;
        }
        if ps.binds == SlateBinds::State {
            let fix_name = match v.as_str() {
                Some(s) if LOCAL_NAME_RE.is_match(s) => s.to_string(),
                _ => "name".to_string(),
            };
            self.add("X410", &format!("{name} on {ty} is two-way and binds a value: {name}={{$name}}"), w, Some(format!("{name}={{${fix_name}}}").as_str()));
            return;
        }
        if ps.prop_type == SlatePropType::Path {
            let Some(m) = v.as_str().and_then(slate_own_name) else {
                self.add("T303", &format!("{name} names a run or a value as $name"), w, None);
                return;
            };
            let kind = self.ctx.kinds.get(&m).copied();
            if ps.names_run && kind != Some(Kind::Run) {
                let word = kind_word_of(kind).unwrap_or("not declared");
                let runs: Vec<String> = self.run_names().into_iter().map(|n| format!("${n}")).collect();
                self.add("K702", &format!("run names a run: ${m} is {word}"), w, slate_nearest(&m, &runs).as_deref());
            }
            return;
        }
        if let Some(words) = &enum_words {
            let ok = v.as_str().is_some_and(|s| words.iter().any(|x| x == s));
            if !ok {
                let shown = js_str(v);
                let quoted = serde_json::to_string(v).unwrap_or_default();
                let fix = slate_nearest(&shown, words);
                self.add("T306", &format!("{name} is {}; {quoted} is not one", or_list(words)), w, fix.as_deref());
            }
            return;
        }
        let row_path = w.prop.as_deref().unwrap_or("");
        if ps.binds == SlateBinds::Item && name != "key" && !row_path.contains(['.', '[']) && (v.is_string() || v.is_number()) {
            let field = match v.as_str() {
                Some(s) if LOCAL_ITEM_RE.is_match(s) => s.to_string(),
                _ => "field".to_string(),
            };
            self.add(
                "T303",
                &format!("{name} is read per row, a formula over item: {name}={{item.{field}}}; {} would be the same on every row", json_stringify(v)),
                w,
                Some(format!("{name}={{item.{field}}}").as_str()),
            );
            return;
        }
        match ps.prop_type {
            SlatePropType::Number | SlatePropType::Integer => {
                if let Some(text) = v.as_str() {
                    let trimmed = js_trim(text);
                    let fix = if NUMBER_TEXT_RE.is_match(trimmed) {
                        Some(format!("{name}={{{trimmed}}}"))
                    } else if self.ctx.kinds.contains_key(text) {
                        Some(format!("{name}={{${text}}}"))
                    } else {
                        None
                    };
                    self.add(
                        "T303",
                        &format!("{name} takes a number in braces, like {name}={{5}}, or a formula, like {name}={{$count}}; \"{text}\" is text"),
                        w,
                        fix.as_deref(),
                    );
                    return;
                }
                let Some(n) = v.as_f64() else {
                    self.add("T303", &format!("{name} takes a number"), w, None);
                    return;
                };
                if ps.prop_type == SlatePropType::Integer && n.fract() != 0.0 {
                    self.add("T303", &format!("{name} is a whole number"), w, None);
                }
                let below = ps.min.is_some_and(|min| n < min);
                let above = ps.max.is_some_and(|max| n > max);
                if below || above {
                    let lo = ps.min.map_or("undefined".to_string(), number_string);
                    let hi = ps.max.map_or("undefined".to_string(), number_string);
                    self.add("T303", &format!("{name} is {lo} to {hi}"), w, None);
                }
            }
            SlatePropType::Boolean => {
                if !v.is_boolean() {
                    self.add("T303", &format!("{name} takes true or false: bare {name}, or {name}={{false}}"), w, None);
                }
            }
            SlatePropType::List => {
                if !v.is_array() {
                    self.add("T303", &format!("{name} takes a list: {name}={{path}} or {name}={{[...]}}"), w, None);
                }
            }
            SlatePropType::String | SlatePropType::Text => {
                if let Some(s) = v.as_str() {
                    if looks_like_path(s) {
                        self.add("W001", &format!("{name} is the literal text \"{s}\", which reads like a path"), w, Some(format!("{name}={{{s}}}").as_str()));
                    } else if parse_slate_own_path(s).is_some() || MUSTACHE_RE.is_match(s) {
                        let inner = unbrace(s);
                        self.add(
                            "W001",
                            &format!("{name} is the literal text \"{s}\", which reads like a formula; quotes show it as written"),
                            w,
                            Some(format!("{name}={{{inner}}}").as_str()),
                        );
                    }
                    if s.contains('\u{2014}') {
                        self.add("W004", "an em dash in the slate's words; use a comma, a colon or a full stop", w, None);
                    }
                    if EMOJI_RE.is_match(s) {
                        self.add("W017", &format!("{name} has an emoji; the slate's words carry none, so say it in words"), w, None);
                    }
                    if (name == "title" || ps.head) && title_cased(s) {
                        self.add("W018", &format!("{name} \"{s}\" is in Title Case; heads take sentence case"), w, Some(format!("{name}=\"{}\"", sentence_case(s)).as_str()));
                    }
                    return;
                }
                if v.is_number() && ps.prop_type == SlatePropType::Text {
                    return;
                }
                self.add("T303", &format!("{name} takes text"), w, None);
            }
            _ => {}
        }
    }

    fn loud(&mut self) {
        let ctx = Rc::clone(&self.ctx);
        let pieces: Vec<(String, Value)> = ctx.doc.get("pieces").and_then(Value::as_object).into_iter().flatten().map(|(k, v)| (k.clone(), v.clone())).collect();
        let tally = |pred: &dyn Fn(&Value) -> bool| -> Vec<String> { pieces.iter().filter(|(_, p)| pred(p)).map(|(id, _)| id.clone()).collect() };
        let check = |this: &mut Validator, ids: Vec<String>, what: &str| {
            if ids.len() > 1 {
                this.add(
                    "T311",
                    &format!("{what} on {}; one per slate reads loud, the rest draw as default", ids.join(", ")),
                    &Where::piece(&ids[1]),
                    None,
                );
            }
        };
        let prop_is = |p: &Value, name: &str, value: &str| p.get("props").and_then(|m| m.get(name)).and_then(Value::as_str) == Some(value);
        check(self, tally(&|p| prop_is(p, "tone", "accent")), "accent");
        check(self, tally(&|p| prop_is(p, "variant", "primary")), "primary");
        check(self, tally(&|p| prop_is(p, "size", "large")), "large");
    }
}

fn looks_like_path(s: &str) -> bool {
    PATH_LIKE_RE.captures(s).is_some_and(|m| SLATE_SOURCES.contains_key(m.get(1).map_or("", |g| g.as_str())))
}

fn unbrace(s: &str) -> &str {
    if s.len() >= 2 && s.starts_with('{') && s.ends_with('}') && !s[1..s.len() - 1].contains(['\n', '\r', '\u{2028}', '\u{2029}']) {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

fn paths_overlap(a: &str, b: &str) -> bool {
    let under = |x: &str, y: &str| x.starts_with(y) && matches!(x.as_bytes().get(y.len()), Some(b'.') | Some(b'['));
    a == b || under(a, b) || under(b, a)
}

fn roots(paths: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for p in paths.iter().filter(|p| p.starts_with('$')) {
        let head = p[1..].split(['.', '[']).next().unwrap_or("").to_string();
        if !out.contains(&head) {
            out.push(head);
        }
    }
    out
}

pub fn validate_document(doc: &SlateDoc, lines: Option<&SlateLines>) -> SlateValidated {
    validate_json(&serde_json::to_value(doc).unwrap_or(Value::Null), lines)
}

pub(crate) fn validate_json(doc: &Value, lines: Option<&SlateLines>) -> SlateValidated {
    let mut v = Validator::new(doc.clone(), lines.cloned());
    v.run();
    SlateValidated { errors: v.errors, warnings: v.warnings }
}

fn refused(code: &str, message: &str, fix: Option<&str>) -> SlateChecked {
    SlateChecked { document: None, errors: vec![slate_problem(code, message, fix)], warnings: Vec::new() }
}

pub fn validate_slate(document: &Value) -> SlateChecked {
    let Some(input) = document.as_object() else {
        return refused("D202", DOCUMENT_SHAPE, None);
    };
    let schema = input.get("schema");
    if schema.is_none() && input.values().any(|v| v.as_str().is_some_and(|s| js_trim_start(s).starts_with('<'))) {
        return refused(
            "D200",
            "this JSON wraps slate markup; send the markup itself as text, not inside document",
            Some("text: \"<slate title=\"Gold\">...</slate>\""),
        );
    }
    if schema.is_none() {
        return refused(
            "D200",
            "this JSON is not a slate: a slate is the JSX-like text slate_catalog shows, sent as text, never JSON of your own",
            Some("<slate title=\"Gold\"><number label=\"Spot\" value={$spot.json.usd} unit=\"USD\" /></slate>"),
        );
    }
    if schema.and_then(Value::as_f64) != Some(2.0) {
        return refused("D200", &format!("schema {} is not a version this host knows; this host reads schema 2", json_stringify(schema.unwrap_or(&Value::Null))), Some("update wsp"));
    }
    let mut filled = Map::new();
    for key in ["values", "derived", "runs"] {
        filled.insert(key.to_string(), json!({}));
    }
    filled.insert("reactions".to_string(), json!([]));
    for (k, v) in input {
        filled.insert(k.clone(), v.clone());
    }
    let filled = Value::Object(filled);
    let mut issues = Vec::new();
    zod_parse(&doc_schema(), Some(&filled), &[], &mut issues);
    if !issues.is_empty() {
        let errors = issues.iter().take(SLATE_LIMITS.errors_per_pass).flat_map(|i| shape_problems(i, &filled)).collect();
        return SlateChecked { document: None, errors, warnings: Vec::new() };
    }
    let typed: SlateDoc = match serde_json::from_value(filled) {
        Ok(doc) => doc,
        Err(_) => return refused("D202", DOCUMENT_SHAPE, None),
    };
    let checked = validate_document(&typed, None);
    let ok = checked.errors.is_empty();
    SlateChecked { document: if ok { Some(typed) } else { None }, errors: checked.errors, warnings: checked.warnings }
}

#[derive(Clone, Debug)]
struct Iss {
    code: &'static str,
    path: Vec<String>,
    message: String,
    keys: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum St {
    Ok,
    Dirty,
    Bad,
}

enum S {
    Str,
    StrMin1,
    Num,
    Lit(Value),
    Enum(&'static [&'static str]),
    Json,
    Arr(Box<S>),
    Rec(Box<S>),
    Obj { fields: Vec<(&'static str, S, bool)>, strict: bool },
    Union(Vec<S>),
    Disc { key: &'static str, opts: Vec<(&'static str, S)> },
}

fn child_path(path: &[String], seg: &str) -> Vec<String> {
    let mut p = path.to_vec();
    p.push(seg.to_string());
    p
}

fn issue(code: &'static str, path: &[String], message: String) -> Iss {
    Iss { code, path: path.to_vec(), message, keys: Vec::new() }
}

fn expected_type(expected: &str, v: Option<&Value>, path: &[String], out: &mut Vec<Iss>) -> St {
    let message = if v.is_none() { "Required".to_string() } else { format!("Expected {expected}, received {}", parsed_type(v)) };
    out.push(issue("invalid_type", path, message));
    St::Bad
}

fn zod_parse(s: &S, v: Option<&Value>, path: &[String], out: &mut Vec<Iss>) -> St {
    match s {
        S::Str | S::StrMin1 => match v {
            Some(Value::String(text)) => {
                if matches!(s, S::StrMin1) && text.is_empty() {
                    out.push(issue("too_small", path, "String must contain at least 1 character(s)".to_string()));
                    St::Dirty
                } else {
                    St::Ok
                }
            }
            _ => expected_type("string", v, path, out),
        },
        S::Num => match v {
            Some(Value::Number(_)) => St::Ok,
            _ => expected_type("number", v, path, out),
        },
        S::Lit(expected) => {
            if v == Some(expected) {
                St::Ok
            } else {
                out.push(issue("invalid_literal", path, format!("Invalid literal value, expected {}", json_stringify(expected))));
                St::Bad
            }
        }
        S::Enum(words) => match v.and_then(Value::as_str) {
            Some(text) => {
                if words.contains(&text) {
                    St::Ok
                } else {
                    let options: Vec<String> = words.iter().map(|w| format!("'{w}'")).collect();
                    out.push(issue("invalid_enum_value", path, format!("Invalid enum value. Expected {}, received '{text}'", options.join(" | "))));
                    St::Bad
                }
            }
            None => {
                let options: Vec<String> = words.iter().map(|w| format!("'{w}'")).collect();
                expected_type(&options.join(" | "), v, path, out)
            }
        },
        S::Json => match v {
            Some(_) => St::Ok,
            None => {
                out.push(issue("invalid_union", path, "Invalid input".to_string()));
                St::Bad
            }
        },
        S::Arr(child) => match v.and_then(Value::as_array) {
            None => expected_type("array", v, path, out),
            Some(items) => {
                let mut status = St::Ok;
                let mut bad = false;
                for (i, x) in items.iter().enumerate() {
                    match zod_parse(child, Some(x), &child_path(path, &i.to_string()), out) {
                        St::Bad => bad = true,
                        St::Dirty => status = St::Dirty,
                        St::Ok => {}
                    }
                }
                if bad {
                    St::Bad
                } else {
                    status
                }
            }
        },
        S::Rec(child) => match v.and_then(Value::as_object) {
            None => expected_type("object", v, path, out),
            Some(m) => {
                let mut status = St::Ok;
                let mut bad = false;
                for (k, x) in m {
                    match zod_parse(child, Some(x), &child_path(path, k), out) {
                        St::Bad => bad = true,
                        St::Dirty => status = St::Dirty,
                        St::Ok => {}
                    }
                }
                if bad {
                    St::Bad
                } else {
                    status
                }
            }
        },
        S::Obj { fields, strict } => {
            let Some(m) = v.and_then(Value::as_object) else {
                return expected_type("object", v, path, out);
            };
            let mut status = St::Ok;
            let mut bad = false;
            for (key, child, optional) in fields {
                let cv = m.get(*key);
                if cv.is_none() && *optional {
                    continue;
                }
                match zod_parse(child, cv, &child_path(path, key), out) {
                    St::Bad => bad = true,
                    St::Dirty => status = St::Dirty,
                    St::Ok => {}
                }
            }
            let extra: Vec<String> = m.keys().filter(|k| !fields.iter().any(|(f, _, _)| *f == k.as_str())).cloned().collect();
            if *strict && !extra.is_empty() {
                let shown: Vec<String> = extra.iter().map(|k| format!("'{k}'")).collect();
                out.push(Iss { code: "unrecognized_keys", path: path.to_vec(), message: format!("Unrecognized key(s) in object: {}", shown.join(", ")), keys: extra });
                status = St::Dirty;
            }
            if bad {
                St::Bad
            } else {
                status
            }
        }
        S::Union(options) => {
            let mut dirty: Option<Vec<Iss>> = None;
            for option in options {
                let mut local = Vec::new();
                match zod_parse(option, v, path, &mut local) {
                    St::Ok => return St::Ok,
                    St::Dirty => {
                        if dirty.is_none() {
                            dirty = Some(local);
                        }
                    }
                    St::Bad => {}
                }
            }
            match dirty {
                Some(issues) => {
                    out.extend(issues);
                    St::Dirty
                }
                None => {
                    out.push(issue("invalid_union", path, "Invalid input".to_string()));
                    St::Bad
                }
            }
        }
        S::Disc { key, opts } => {
            let Some(m) = v.and_then(Value::as_object) else {
                return expected_type("object", v, path, out);
            };
            let tag = m.get(*key).and_then(Value::as_str);
            match opts.iter().find(|(t, _)| Some(*t) == tag) {
                None => {
                    let tags: Vec<String> = opts.iter().map(|(t, _)| format!("'{t}'")).collect();
                    out.push(issue("invalid_union_discriminator", &child_path(path, key), format!("Invalid discriminator value. Expected {}", tags.join(" | "))));
                    St::Bad
                }
                Some((_, option)) => zod_parse(option, v, path, out),
            }
        }
    }
}

fn opt(child: S) -> (S, bool) {
    (child, true)
}

fn req(child: S) -> (S, bool) {
    (child, false)
}

fn obj(fields: Vec<(&'static str, (S, bool))>, strict: bool) -> S {
    S::Obj { fields: fields.into_iter().map(|(k, (s, o))| (k, s, o)).collect(), strict }
}

fn binding() -> S {
    obj(vec![("bind", req(S::StrMin1))], true)
}

fn format_obj() -> S {
    obj(vec![("format", req(S::Str))], true)
}

fn confirm_schema() -> S {
    S::Union(vec![S::Str, binding(), format_obj()])
}

fn steps_schema() -> S {
    S::Arr(Box::new(step_schema()))
}

fn step_schema() -> S {
    S::Disc {
        key: "do",
        opts: vec![
            ("set", obj(vec![("do", req(S::Lit(json!("set")))), ("path", req(S::Str)), ("value", req(S::Json))], true)),
            ("toggle", obj(vec![("do", req(S::Lit(json!("toggle")))), ("path", req(S::Str))], true)),
            ("start", obj(vec![("do", req(S::Lit(json!("start")))), ("run", req(S::Str))], true)),
            ("cancel", obj(vec![("do", req(S::Lit(json!("cancel")))), ("run", req(S::Str))], true)),
            ("send", obj(vec![("do", req(S::Lit(json!("send")))), ("text", req(S::Str)), ("with", opt(S::Arr(Box::new(S::Str))))], true)),
            ("steer", obj(vec![("do", req(S::Lit(json!("steer")))), ("text", req(S::Str)), ("with", opt(S::Arr(Box::new(S::Str))))], true)),
            ("queue", obj(vec![("do", req(S::Lit(json!("queue")))), ("text", req(S::Str)), ("with", opt(S::Arr(Box::new(S::Str))))], true)),
            ("fill", obj(vec![("do", req(S::Lit(json!("fill")))), ("text", req(S::Str)), ("with", opt(S::Arr(Box::new(S::Str))))], true)),
            ("open", obj(vec![("do", req(S::Lit(json!("open")))), ("target", req(S::Json))], true)),
            ("copy", obj(vec![("do", req(S::Lit(json!("copy")))), ("text", req(S::Json))], true)),
            ("pane", obj(vec![("do", req(S::Lit(json!("pane")))), ("kind", req(S::Enum(&PANE_KINDS)))], true)),
        ],
    }
}

fn run_schema() -> S {
    let common = || {
        vec![
            ("confirm", opt(confirm_schema())),
            ("every", opt(S::Num)),
            ("once", opt(S::Lit(json!(true)))),
            ("always", opt(S::Lit(json!(true)))),
            ("then", opt(S::Str)),
        ]
    };
    let mut cmd = vec![
        ("kind", req(S::Lit(json!("cmd")))),
        ("cmd", req(S::Str)),
        ("env", opt(S::Rec(Box::new(S::Json)))),
        ("args", opt(S::Arr(Box::new(S::Json)))),
        ("stdin", opt(S::Json)),
        ("on", opt(S::Enum(&["thread", "host"]))),
        ("cwd", opt(S::Str)),
        ("timeout", opt(S::Num)),
        ("stream", opt(S::Lit(json!(true)))),
    ];
    cmd.extend(common());
    let mut tool = vec![("kind", req(S::Lit(json!("tool")))), ("server", req(S::Str)), ("tool", req(S::Str)), ("args", opt(S::Rec(Box::new(S::Json))))];
    tool.push(("description", opt(S::Str)));
    tool.extend(common());
    let resource = vec![
        ("kind", req(S::Lit(json!("resource")))),
        ("server", req(S::Str)),
        ("uri", req(S::Str)),
        ("every", opt(S::Num)),
        ("always", opt(S::Lit(json!(true)))),
        ("then", opt(S::Str)),
        ("description", opt(S::Str)),
    ];
    S::Disc { key: "kind", opts: vec![("cmd", obj(cmd, true)), ("tool", obj(tool, true)), ("resource", obj(resource, true))] }
}

fn piece_schema() -> S {
    let on = obj(vec![("press", opt(steps_schema())), ("submit", opt(steps_schema())), ("change", opt(steps_schema()))], true);
    let fallback = S::Union(vec![S::Str, obj(vec![("text", req(S::Str))], true)]);
    obj(
        vec![
            ("type", req(S::Str)),
            ("props", opt(S::Rec(Box::new(S::Json)))),
            ("children", opt(S::Arr(Box::new(S::Str)))),
            ("when", opt(S::Str)),
            ("on", opt(on)),
            ("fallback", opt(fallback)),
        ],
        true,
    )
}

fn reaction_schema() -> S {
    let on = S::Union(vec![obj(vec![("change", req(S::Arr(Box::new(S::Str))))], true), obj(vec![("done", req(S::Str))], true)]);
    obj(vec![("id", opt(S::Str)), ("on", req(on)), ("do", req(steps_schema()))], true)
}

fn doc_schema() -> S {
    let value = obj(vec![("start", req(S::Json)), ("secret", opt(S::Lit(json!(true)))), ("keep", opt(S::Lit(json!(true))))], true);
    obj(
        vec![
            ("schema", req(S::Lit(json!(2)))),
            ("kit", opt(S::Str)),
            ("title", opt(S::Str)),
            ("root", req(S::Str)),
            ("values", req(S::Rec(Box::new(value)))),
            ("derived", req(S::Rec(Box::new(S::Str)))),
            ("runs", req(S::Rec(Box::new(run_schema())))),
            ("reactions", req(S::Arr(Box::new(reaction_schema())))),
            ("pieces", req(S::Rec(Box::new(piece_schema())))),
            ("files", opt(S::Rec(Box::new(S::Str)))),
        ],
        true,
    )
}

fn shape_problems(issue: &Iss, doc: &Value) -> Vec<SlateProblem> {
    let path = &issue.path;
    let area = path.first().map(String::as_str);
    let name = path.get(1);
    let field = path.get(2);
    let sub = path.get(3);
    let keys = if issue.code == "unrecognized_keys" { issue.keys.clone() } else { Vec::new() };
    if area == Some("pieces") {
        if let Some(name) = name {
            let piece = doc.get("pieces").and_then(|p| p.get(name));
            let ty = piece.and_then(|p| p.get("type")).and_then(Value::as_str);
            let spec = ty.and_then(|t| SLATE_PIECES.get(t));
            if field.is_none() && !keys.is_empty() {
                return keys
                    .iter()
                    .map(|k| {
                        let known = spec.is_some_and(|s| s.props.get(k.as_str()).is_some()) || spec.is_some_and(|s| s.items.values().any(|i| i.prop == k.as_str()));
                        let tail_text = if known {
                            format!(", and {k} goes inside props")
                        } else if spec.is_some() {
                            format!(", and {} has no {k}", ty.unwrap_or("undefined"))
                        } else {
                            String::new()
                        };
                        let fix = if known {
                            Some(format!("\"props\": {{ \"{k}\": ... }}"))
                        } else {
                            let keys: Vec<String> = spec.map_or_else(Vec::new, |s| s.props.keys().map(|x| x.to_string()).collect());
                            slate_nearest(k, &keys)
                        };
                        let mut p = slate_problem(
                            "T302",
                            &format!("{k} sits beside props on {name}; a piece holds {PIECE_KEYS}{tail_text}"),
                            fix.as_deref(),
                        );
                        p.piece = Some(name.clone());
                        p.prop = Some(k.clone());
                        p
                    })
                    .collect();
            }
            if field.map(String::as_str) == Some("type") {
                let mut p = slate_problem("T300", &format!("{name}'s type is the piece's name as a string"), None);
                p.piece = Some(name.clone());
                p.prop = Some("type".to_string());
                return vec![p];
            }
            if field.map(String::as_str) == Some("on") {
                let (code, message) = if !keys.is_empty() {
                    let shown: Vec<String> = keys.clone();
                    ("A602", format!("{name} has no on.{}; events are press, submit and change", shown.join(", on.")))
                } else {
                    let rest = path[2..].join(".");
                    ("A600", format!("{name}.{rest}: a handler is a list of steps, {}", issue.message))
                };
                let prop = match sub {
                    Some(s) => format!("on.{s}"),
                    None => "on".to_string(),
                };
                let mut p = slate_problem(code, &message, None);
                p.piece = Some(name.clone());
                p.prop = Some(prop);
                return vec![p];
            }
            let prop = if field.map(String::as_str) == Some("props") { sub.cloned() } else { field.cloned() };
            let rest = if path.len() > 2 { path[2..].join(".") } else { String::new() };
            let mut p = slate_problem("T303", &format!("{name}.{rest}: {}", issue.message), None);
            p.piece = Some(name.clone());
            p.prop = prop;
            return vec![p];
        }
    }
    if (area == Some("values") || area == Some("derived") || area == Some("runs")) && name.is_some() {
        let name = name.map_or("", String::as_str);
        let piece = format!("${name}");
        if !keys.is_empty() {
            let takes = match area {
                Some("values") => "start, secret and keep",
                Some("runs") => "kind, cmd, env, args, stdin, on, cwd, timeout, stream, confirm, every, once, always, then and description",
                _ => "an expression",
            };
            let code = if area == Some("runs") { "K704" } else { "T302" };
            return keys
                .iter()
                .map(|k| {
                    let mut p = slate_problem(code, &format!("{piece} has no {k}; it takes {takes}"), None);
                    p.piece = Some(piece.clone());
                    p.prop = Some(k.clone());
                    p
                })
                .collect();
        }
        if area == Some("runs") && issue.code == "invalid_union_discriminator" {
            let mut p = slate_problem("K701", &format!("{piece} takes kind \"cmd\", \"tool\" or \"resource\""), None);
            p.piece = Some(piece);
            p.prop = Some("kind".to_string());
            return vec![p];
        }
        let rest = if path.len() > 2 { path[2..].join(".") } else { String::new() };
        let rest = if rest.is_empty() { "decl".to_string() } else { rest };
        let mut p = slate_problem(if area == Some("runs") { "K704" } else { "T303" }, &format!("{piece}.{rest}: {}", issue.message), None);
        p.piece = Some(piece);
        p.prop = field.cloned();
        return vec![p];
    }
    if area == Some("reactions") {
        let index: f64 = name.and_then(|n| n.parse().ok()).unwrap_or(f64::NAN);
        let number = number_string(index + 1.0);
        let at = if path.len() > 2 { format!(".{}", path[2..].join(".")) } else { String::new() };
        let mut p = slate_problem(
            "A611",
            &format!(
                "reaction {number}{at}: {}; a <when> is {{ on: {{ change: [paths] }} or {{ done: name }}, do: [steps] }}",
                issue.message
            ),
            None,
        );
        p.piece = Some(format!("reaction-{number}"));
        return vec![p];
    }
    if !keys.is_empty() {
        return keys
            .iter()
            .map(|k| {
                let fix = slate_nearest(k, &DOC_KEYS.iter().map(|s| s.to_string()).collect::<Vec<_>>());
                let extra = if area.is_none() && slate_reserved_prop(k).is_some() {
                    "; a piece's props go under pieces.<id>.props"
                } else {
                    ""
                };
                slate_problem("D202", &format!("the document has no {k}; it holds {}{extra}", DOC_KEYS.join(", ")), fix.as_deref())
            })
            .collect();
    }
    let at = if path.is_empty() { "the document".to_string() } else { path.join(".") };
    vec![slate_problem("D202", &format!("{at}: {}", issue.message), None)]
}
