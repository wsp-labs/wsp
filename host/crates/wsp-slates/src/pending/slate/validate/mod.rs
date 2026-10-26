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

use super::expr::{
    check_slate_expression, parse_slate_expression, parse_slate_format, slate_dependencies, SlateCheckScope, SlateExpr, SlateFormatPart,
    SlateType, SlateTypeName,
};
use super::kit::{SlateBinds, SlateItemSpec, SlatePieceCheck, SlatePieceModule, SlatePropSpec, SlatePropType};
use super::paths::{parse_slate_own_path, slate_own_name};
use super::*;

use super::problems::slate_nearest;
use super::sources::slate_source_type;
use super::steps::slate_step_module;

mod driver;
mod pieces;
mod reactions;
mod runs;
mod scope;
mod text;
mod zod;

use scope::*;
use text::*;
use zod::*;

pub type SlateLines = IndexMap<String, i64>;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SlateValidated {
    pub errors: Vec<SlateProblem>,
    pub warnings: Vec<SlateProblem>,
}

const RUN_FIELDS: [&str; 15] =
    ["state", "why", "exit", "out", "err", "json", "lines", "startedAt", "endedAt", "ms", "runs", "cut", "stale", "refreshing", "text"];
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

struct Ctx {
    doc: Value,
    kinds: IndexMap<String, Kind>,
    derived_types: std::cell::RefCell<IndexMap<String, SlateType>>,
    derived_depth: std::cell::Cell<usize>,
    infinite: HashSet<String>,
}

/// Types read through this many chained derived values at most; a document with that many is over the limit of 32 and refused.
const DERIVED_TYPE_DEPTH: usize = 64;

struct Validator {
    ctx: Rc<Ctx>,
    lines: Option<SlateLines>,
    errors: Vec<SlateProblem>,
    warnings: Vec<SlateProblem>,
    seen: HashSet<String>,
    started: HashSet<String>,
}

pub fn validate_document(doc: &SlateDoc, lines: Option<&SlateLines>) -> SlateValidated {
    validate_document_with(doc, lines, &[])
}

/// `validate_document` for a document read from text, whose values in `infinite` start at a number past the double range.
pub fn validate_document_with(doc: &SlateDoc, lines: Option<&SlateLines>, infinite: &[String]) -> SlateValidated {
    check_json(&serde_json::to_value(doc).unwrap_or(Value::Null), lines, infinite)
}

pub(crate) fn validate_json(doc: &Value, lines: Option<&SlateLines>) -> SlateValidated {
    check_json(doc, lines, &[])
}

fn check_json(doc: &Value, lines: Option<&SlateLines>, infinite: &[String]) -> SlateValidated {
    let mut v = Validator::new(doc.clone(), lines.cloned(), infinite);
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
        return refused(
            "D200",
            &format!(
                "schema {} is not a version this host knows; this host reads schema 2",
                json_stringify(schema.unwrap_or(&Value::Null))
            ),
            Some("update wsp"),
        );
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
