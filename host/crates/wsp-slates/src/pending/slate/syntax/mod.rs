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

mod build;
mod compiler;
mod literal;
mod patch_ops;
mod piece;
mod print;
mod reader;
mod run;
mod text;

use build::*;
use literal::*;
pub use print::print_slate;
use reader::*;
use text::*;

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
static OBJECT_PART: LazyLock<Regex> =
    LazyLock::new(|| js_re(r#"^[@]*(?:([A-Za-z_][A-Za-z0-9_]*)|"([^"]*)"|'([^']*)')[@]*:[@]*([\s\S]+)$"#));
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

struct Compiler {
    errors: Vec<SlateProblem>,
    lines: SlateLines,
    doc: SlateDoc,
    minted: HashMap<String, usize>,
    taken: HashSet<String>,
    names: IndexMap<String, String>,
    patch: bool,
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
