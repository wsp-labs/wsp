// SPDX-License-Identifier: AGPL-3.0-only
//! protocol/src/slate/expr.ts at 65f6b89c7.
use std::cell::Cell;
use std::cmp::Ordering;
use std::rc::Rc;

use indexmap::{IndexMap, IndexSet};
use serde_json::{Map, Value};
use wsp_wire::text::json_stringify;
use wsp_wire::types::pull_request::CHECK_STATE_WORDS;
use wsp_words::date_parse::date_parse;
use wsp_words::js::{collate, js_key_order, js_round, js_trim, locale_number, number_string, to_fixed, to_number, utf16, Grouping};
use wsp_words::js_date::{self, DateZone, Fields};
use wsp_words::v8_json::parse_json_lax;
use wsp_words::words::turn::fmt_clock;
use wsp_words::words::units::{fmt_bytes, fmt_cost, fmt_duration, fmt_tokens, DurationStyle};

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlateBinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

impl SlateBinOp {
    pub fn symbol(self) -> &'static str {
        match self {
            SlateBinOp::Add => "+",
            SlateBinOp::Sub => "-",
            SlateBinOp::Mul => "*",
            SlateBinOp::Div => "/",
            SlateBinOp::Rem => "%",
            SlateBinOp::Eq => "==",
            SlateBinOp::Ne => "!=",
            SlateBinOp::Lt => "<",
            SlateBinOp::Le => "<=",
            SlateBinOp::Gt => ">",
            SlateBinOp::Ge => ">=",
            SlateBinOp::And => "and",
            SlateBinOp::Or => "or",
        }
    }

    fn compare(self) -> bool {
        matches!(self, SlateBinOp::Eq | SlateBinOp::Ne | SlateBinOp::Lt | SlateBinOp::Le | SlateBinOp::Gt | SlateBinOp::Ge)
    }
}

/// A parsed expression. Every `at` is the 0-based UTF-16 offset of the node in its source.
#[derive(Clone, Debug, PartialEq)]
pub enum SlateExpr {
    Lit { v: Value, at: usize },
    Path { head: String, own: bool, segs: Vec<PathSeg>, at: usize },
    Field { of: Box<SlateExpr>, name: PathSeg, at: usize },
    Call { name: String, args: Vec<SlateExpr>, at: usize },
    Neg { arg: Box<SlateExpr>, at: usize },
    Not { arg: Box<SlateExpr>, at: usize },
    Bin { op: SlateBinOp, left: Box<SlateExpr>, right: Box<SlateExpr>, at: usize },
    Cond { test: Box<SlateExpr>, then: Box<SlateExpr>, other: Box<SlateExpr>, at: usize },
    Tpl { parts: Vec<SlateTemplatePart>, at: usize },
    List { items: Vec<SlateExpr>, at: usize },
    Index { of: Box<SlateExpr>, index: Box<SlateExpr>, at: usize },
    Rec { fields: Vec<(String, SlateExpr)>, at: usize },
    Pipe { head: Box<SlateExpr>, steps: Vec<SlatePipeStepNode>, at: usize },
}

impl SlateExpr {
    pub fn at(&self) -> usize {
        match self {
            SlateExpr::Lit { at, .. }
            | SlateExpr::Path { at, .. }
            | SlateExpr::Field { at, .. }
            | SlateExpr::Call { at, .. }
            | SlateExpr::Neg { at, .. }
            | SlateExpr::Not { at, .. }
            | SlateExpr::Bin { at, .. }
            | SlateExpr::Cond { at, .. }
            | SlateExpr::Tpl { at, .. }
            | SlateExpr::List { at, .. }
            | SlateExpr::Index { at, .. }
            | SlateExpr::Rec { at, .. }
            | SlateExpr::Pipe { at, .. } => *at,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SlatePipeStepNode {
    pub name: String,
    pub args: Vec<SlatePipeArg>,
    pub at: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SlatePipeArg {
    pub name: Option<String>,
    pub expr: SlateExpr,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SlateTemplatePart {
    Text(String),
    Hole { expr: SlateExpr, src: String, at: usize },
}

/// The parse of an expression: its tree, or the one error it stopped at.
#[derive(Clone, Debug, PartialEq)]
pub struct SlateExprParse {
    pub ast: Option<SlateExpr>,
    pub errors: Vec<SlateProblem>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SlateFormatPart {
    Text(String),
    Hole { expr: String, at: usize },
}

#[derive(Clone, Debug, PartialEq)]
pub struct SlateFormatParse {
    pub parts: Vec<SlateFormatPart>,
    pub errors: Vec<SlateProblem>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlateTypeName {
    Number,
    String,
    Boolean,
    Null,
    List,
    Record,
    Any,
}

impl SlateTypeName {
    pub fn name(self) -> &'static str {
        match self {
            SlateTypeName::Number => "number",
            SlateTypeName::String => "string",
            SlateTypeName::Boolean => "boolean",
            SlateTypeName::Null => "null",
            SlateTypeName::List => "list",
            SlateTypeName::Record => "record",
            SlateTypeName::Any => "any",
        }
    }
}

/// A type as the checker knows it: a name, and where known a list's element or a record's fields.
#[derive(Clone, Debug, PartialEq)]
pub struct SlateType {
    pub t: SlateTypeName,
    pub of: Option<Box<SlateType>>,
    pub fields: Option<IndexMap<String, SlateType>>,
}

fn ty(t: SlateTypeName) -> SlateType {
    SlateType { t, of: None, fields: None }
}

fn any_t() -> SlateType {
    ty(SlateTypeName::Any)
}

fn str_t() -> SlateType {
    ty(SlateTypeName::String)
}

fn num_t() -> SlateType {
    ty(SlateTypeName::Number)
}

fn bool_t() -> SlateType {
    ty(SlateTypeName::Boolean)
}

fn null_t() -> SlateType {
    ty(SlateTypeName::Null)
}

fn list_t() -> SlateType {
    ty(SlateTypeName::List)
}

fn rec_t() -> SlateType {
    ty(SlateTypeName::Record)
}

fn list_of(of: SlateType) -> SlateType {
    SlateType { t: SlateTypeName::List, of: Some(Box::new(of)), fields: None }
}

/// What the checker asks of a path's source: its type, or the problem with the path.
pub struct SlateScopeProblem {
    pub code: &'static str,
    pub message: String,
    pub fix: Option<String>,
}

/// What the checker knows about the world an expression is read in.
pub struct SlateCheckScope {
    /// The type of a path. item and index never reach this.
    pub path: Box<dyn Fn(&str, bool, &[PathSeg]) -> Result<SlateType, SlateScopeProblem>>,
    /// The row scope: item's type inside a repeating piece's row.
    pub row: Option<SlateType>,
}

/// The checker's answer: the expression's type and every problem with it.
#[derive(Clone, Debug, PartialEq)]
pub struct SlateExprCheck {
    pub ty: SlateType,
    pub problems: Vec<SlateProblem>,
}

/// A named function as the catalog and the checker read it.
pub struct SlateFunction {
    pub min: usize,
    pub max: usize,
    pub sig: &'static str,
    pub example: &'static str,
    returns: fn(&[SlateType]) -> SlateType,
    null_safe: bool,
    number_first: bool,
    list_first: bool,
    clock: bool,
    body: FnBody,
}

/// A pipeline step as the catalog and the parser read it.
pub struct SlatePipeStep {
    pub min: usize,
    pub max: usize,
    pub sig: &'static str,
    pub purpose: &'static str,
    pub example: &'static str,
    needs_list: bool,
    out: fn(&SlateType, &[SlateType]) -> SlateType,
    apply: PipeBody,
}

type Val = Option<Value>;

struct Over;

type Res<T> = Result<T, Over>;

struct Shared {
    resolve: Rc<dyn Fn(&str) -> Option<Value>>,
    now: i64,
    steps: Cell<usize>,
    visits: Cell<usize>,
}

/// One scope of evaluation: the budget it shares with every other scope of the same expression, and the row it reads.
#[derive(Clone)]
struct Env {
    sh: Rc<Shared>,
    item: Option<Value>,
    index: Option<i64>,
    row: bool,
}

impl Env {
    fn step(&self) -> Res<()> {
        if self.row {
            return self.charge(1);
        }
        let n = self.sh.steps.get() + 1;
        self.sh.steps.set(n);
        if n > SLATE_LIMITS.eval_steps {
            return Err(Over);
        }
        Ok(())
    }

    fn charge(&self, n: usize) -> Res<()> {
        let visits = self.sh.visits.get() + n;
        self.sh.visits.set(visits);
        if visits > SLATE_LIMITS.pipeline_visits {
            return Err(Over);
        }
        Ok(())
    }

    fn now(&self) -> f64 {
        as_num((self.sh.resolve)("time.now").as_ref()).unwrap_or(self.sh.now as f64)
    }

    fn per(&self, e: &SlateExpr, item: &Value, index: usize) -> Res<Val> {
        self.charge(1)?;
        let row = Env { sh: self.sh.clone(), item: Some(item.clone()), index: Some(index as i64), row: true };
        run(e, &row)
    }
}

type FnBody = fn(&[Val], &Env, &[SlateExpr]) -> Res<Val>;

struct PipeRun<'a> {
    list: Vec<Value>,
    cur: Val,
    args: &'a [SlatePipeArg],
    env: &'a Env,
}

impl PipeRun<'_> {
    fn arg(&self, n: usize) -> &SlateExpr {
        &self.args[n].expr
    }

    fn per(&self, e: &SlateExpr, item: &Value, index: usize) -> Res<Val> {
        self.env.per(e, item, index)
    }
}

type PipeBody = for<'a> fn(PipeRun<'a>) -> Res<Val>;

// ---- the tokenizer ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Num,
    Str,
    Tpl,
    Own,
    Name,
    Op,
    End,
}

#[derive(Clone, Debug)]
enum RawPart {
    Text(String),
    Hole { src: String, at: usize },
}

#[derive(Clone, Debug)]
struct Token {
    kind: Kind,
    v: String,
    parts: Vec<RawPart>,
    at: usize,
}

#[derive(Debug)]
struct Fault {
    code: &'static str,
    message: String,
    at: usize,
    fix: Option<String>,
    absolute: bool,
}

type PResult<T> = Result<T, Fault>;

fn fault(code: &'static str, message: String, at: usize, fix: Option<String>) -> Fault {
    Fault { code, message, at, fix, absolute: false }
}

/// JavaScript method names a model writes by reflex, with this language's spelling.
fn method_fix(name: &str) -> Option<&'static str> {
    Some(match name {
        "length" => "len(x)",
        "map" => "x | map(name: item.field) or pluck(x, 'field')",
        "filter" => "x | where(item.cond)",
        "toUpperCase" => "upper(x)",
        "toLowerCase" => "lower(x)",
        "includes" => "contains(x, v)",
        "join" => "join(x, sep)",
        "slice" => "x | take(n)",
        "find" => "x | where(...) | first",
        "some" => "len(x | where(...)) > 0",
        "trim" => "trim(x)",
        "split" => "split(x, sep)",
        "replace" => "replace(x, a, b)",
        "startsWith" => "startsWith(x, p)",
        "endsWith" => "endsWith(x, p)",
        "reduce" => "x | sum(item.field)",
        "sort" => "x | sortBy(item.field)",
        "toString" => "str(x)",
        "toFixed" => "number(x, places)",
        "test" => "contains, startsWith or endsWith; there are no regular expressions",
        "forEach" => "a piece that takes items",
        _ => return None,
    })
}

fn escape_of(c: u16) -> Option<&'static str> {
    Some(match c as u8 {
        b'n' if c < 128 => "\n",
        b't' if c < 128 => "\t",
        b'r' if c < 128 => "\r",
        b'b' if c < 128 => "\u{8}",
        b'f' if c < 128 => "\u{c}",
        _ => return None,
    })
}

fn units(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

fn text(u: &[u16]) -> String {
    String::from_utf16_lossy(u)
}

fn at_is(src: &[u16], i: usize, c: u8) -> bool {
    src.get(i) == Some(&u16::from(c))
}

fn starts(src: &[u16], i: usize, pat: &str) -> bool {
    pat.bytes().enumerate().all(|(k, b)| src.get(i + k) == Some(&u16::from(b)))
}

fn is_digit(src: &[u16], i: usize) -> bool {
    src.get(i).is_some_and(|c| (u16::from(b'0')..=u16::from(b'9')).contains(c))
}

fn is_name_start(c: u16) -> bool {
    c < 128 && ((c as u8).is_ascii_alphabetic() || c == u16::from(b'_'))
}

fn is_name_char(c: u16) -> bool {
    c < 128 && ((c as u8).is_ascii_alphanumeric() || c == u16::from(b'_'))
}

fn tokenize(src: &[u16]) -> PResult<Vec<Token>> {
    let mut out = Vec::new();
    let n = src.len();
    let mut i = 0;
    while i < n {
        let c = src[i];
        if matches!(c, 0x20 | 0x09 | 0x0a | 0x0d) {
            i += 1;
            continue;
        }
        let at = i;
        if is_digit(src, i) {
            let mut j = i;
            while is_digit(src, j) {
                j += 1;
            }
            if at_is(src, j, b'.') && is_digit(src, j + 1) {
                j += 1;
                while is_digit(src, j) {
                    j += 1;
                }
            }
            if at_is(src, j, b'e') || at_is(src, j, b'E') {
                let mut k = j + 1;
                if at_is(src, k, b'-') {
                    k += 1;
                }
                if is_digit(src, k) {
                    while is_digit(src, k) {
                        k += 1;
                    }
                    j = k;
                }
            }
            out.push(Token { kind: Kind::Num, v: text(&src[i..j]), parts: vec![], at });
            i = j;
            continue;
        }
        if c == u16::from(b'\'') || c == u16::from(b'"') {
            let mut s: Vec<u16> = Vec::new();
            i += 1;
            while i < n && src[i] != c {
                if src[i] == u16::from(b'\\') && i + 1 < n {
                    let nc = src[i + 1];
                    if nc == u16::from(b'u') {
                        let hex: Option<u16> = (i + 2..(i + 6).min(n))
                            .map(|k| char::from_u32(u32::from(src[k])).and_then(|ch| ch.to_digit(16)))
                            .collect::<Option<Vec<u32>>>()
                            .filter(|digits| digits.len() == 4)
                            .map(|digits| digits.iter().fold(0u32, |acc, d| acc * 16 + d) as u16);
                        if let Some(unit) = hex {
                            s.push(unit);
                            i += 6;
                            continue;
                        }
                    }
                    match escape_of(nc) {
                        Some(rep) => s.extend(rep.encode_utf16()),
                        None => s.push(nc),
                    }
                    i += 2;
                    continue;
                }
                s.push(src[i]);
                i += 1;
            }
            if i >= n {
                return Err(fault("X400", format!("a string opened at column {} never closes", at + 1), at, None));
            }
            i += 1;
            out.push(Token { kind: Kind::Str, v: text(&s), parts: vec![], at });
            continue;
        }
        if c == u16::from(b'`') {
            let mut parts: Vec<RawPart> = Vec::new();
            let mut lit: Vec<u16> = Vec::new();
            i += 1;
            while i < n && src[i] != u16::from(b'`') {
                if starts(src, i, "$${") {
                    lit.extend("${".encode_utf16());
                    i += 3;
                    continue;
                }
                if starts(src, i, "\\`") {
                    lit.push(u16::from(b'`'));
                    i += 2;
                    continue;
                }
                if starts(src, i, "${") {
                    let start = i + 2;
                    let Some(end) = closing_brace_units(src, start) else {
                        return Err(fault("X400", format!("a ${{ at column {} never closes", i + 1), i, None));
                    };
                    if !lit.is_empty() {
                        parts.push(RawPart::Text(text(&lit)));
                        lit.clear();
                    }
                    parts.push(RawPart::Hole { src: text(&src[start..end]), at: start });
                    i = end + 1;
                    continue;
                }
                lit.push(src[i]);
                i += 1;
            }
            if i >= n {
                return Err(fault("X400", format!("a template opened at column {} never closes", at + 1), at, None));
            }
            if !lit.is_empty() {
                parts.push(RawPart::Text(text(&lit)));
            }
            i += 1;
            out.push(Token { kind: Kind::Tpl, v: "`".into(), parts, at });
            continue;
        }
        if c == u16::from(b'$') {
            if !src.get(i + 1).is_some_and(|&d| is_name_start(d)) {
                return Err(fault("X400", format!("$ at column {} must be followed by a name", at + 1), at, None));
            }
            let mut j = i + 1;
            while j < n && is_name_char(src[j]) {
                j += 1;
            }
            out.push(Token { kind: Kind::Own, v: text(&src[i + 1..j]), parts: vec![], at });
            i = j;
            continue;
        }
        if is_name_start(c) {
            let mut j = i;
            while j < n && is_name_char(src[j]) {
                j += 1;
            }
            out.push(Token { kind: Kind::Name, v: text(&src[i..j]), parts: vec![], at });
            i = j;
            continue;
        }
        if starts(src, i, "...") {
            return Err(fault("X420", "no spreads here; a list is reshaped with steps".into(), at, None));
        }
        if starts(src, i, "===") || starts(src, i, "!==") {
            out.push(Token { kind: Kind::Op, v: text(&src[i..i + 2]), parts: vec![], at });
            i += 3;
            continue;
        }
        if starts(src, i, "=>") {
            return Err(fault(
                "X420",
                "no functions here; a list is reshaped with steps".into(),
                at,
                Some("items | map(name: item.name) or pluck(items, 'name')".into()),
            ));
        }
        if starts(src, i, "??") {
            return Err(fault("X420", "?? is not an operator here".into(), at, Some("orElse(a, b)".into())));
        }
        if starts(src, i, "?.") {
            return Err(fault("X420", "?. is not needed: a missing field reads as null".into(), at, Some("a.b".into())));
        }
        let two: Option<&str> = ["==", "!=", "<=", ">="].into_iter().chain(["&&", "||"]).find(|p| starts(src, i, p));
        if let Some(p) = two {
            let v = match p {
                "&&" => "and",
                "||" => "or",
                other => other,
            };
            out.push(Token { kind: Kind::Op, v: v.into(), parts: vec![], at });
            i += 2;
            continue;
        }
        if c == u16::from(b'!') {
            out.push(Token { kind: Kind::Op, v: "not".into(), parts: vec![], at });
            i += 1;
            continue;
        }
        if c < 128 && b"()[],.?:+-*/%<>|{}".contains(&(c as u8)) {
            out.push(Token { kind: Kind::Op, v: text(&[c]), parts: vec![], at });
            i += 1;
            continue;
        }
        if c == u16::from(b'=') {
            return Err(fault(
                "X420",
                "compare with ==; to write a value use set($x, 1) in a handler".into(),
                at,
                Some("==".into()),
            ));
        }
        return Err(fault("X400", format!("\"{}\" is not part of an expression (column {})", text(&[c]), at + 1), at, None));
    }
    out.push(Token { kind: Kind::End, v: String::new(), parts: vec![], at: src.len() });
    Ok(out)
}

/// The index of the } closing a ${ whose body starts at start, quotes and nested braces skipped; None when none.
fn closing_brace_units(src: &[u16], start: usize) -> Option<usize> {
    let mut depth = 1;
    let mut quote: Option<u16> = None;
    let mut j = start;
    while j < src.len() {
        let ch = src[j];
        if let Some(q) = quote {
            if ch == u16::from(b'\\') {
                j += 1;
            } else if ch == q {
                quote = None;
            }
        } else if ch == u16::from(b'\'') || ch == u16::from(b'"') || ch == u16::from(b'`') {
            quote = Some(ch);
        } else if ch == u16::from(b'{') {
            depth += 1;
        } else if ch == u16::from(b'}') {
            depth -= 1;
            if depth == 0 {
                return Some(j);
            }
        }
        j += 1;
    }
    None
}

/// The index of the } closing a ${ whose body starts at start (a UTF-16 offset), or None when none closes.
pub fn closing_brace(src: &str, start: usize) -> Option<usize> {
    closing_brace_units(&units(src), start)
}

// ---- the parser ----

struct Parser {
    toks: Vec<Token>,
    i: usize,
    depth: usize,
    base: usize,
}

/// Each `column N` in a message moved by base, as the TS's replace does on every inner message.
fn shift_columns(message: &str, base: usize) -> String {
    let mut out = String::new();
    let mut rest = message;
    while let Some(pos) = rest.find("column ") {
        let after = &rest[pos + 7..];
        let digits = after.bytes().take_while(u8::is_ascii_digit).count();
        out.push_str(&rest[..pos + 7]);
        if digits == 0 {
            rest = after;
            continue;
        }
        let n: u64 = after[..digits].parse().unwrap_or(u64::MAX);
        out.push_str(&n.saturating_add(base as u64).to_string());
        rest = &after[digits..];
    }
    out.push_str(rest);
    out
}

fn seg_index(x: f64) -> i64 {
    x as i64
}

impl Parser {
    fn peek_n(&self, n: usize) -> &Token {
        &self.toks[(self.i + n).min(self.toks.len() - 1)]
    }

    fn peek(&self) -> &Token {
        self.peek_n(0)
    }

    fn next(&mut self) -> Token {
        let t = self.toks[self.i.min(self.toks.len() - 1)].clone();
        self.i += 1;
        t
    }

    fn is_op(&self, v: &str) -> bool {
        let t = self.peek();
        t.kind == Kind::Op && t.v == v
    }

    fn is_word(&self, v: &str) -> bool {
        let t = self.peek();
        t.kind == Kind::Name && t.v == v
    }

    fn expect(&mut self, v: &str) -> PResult<()> {
        let t = self.peek().clone();
        if t.kind != Kind::Op || t.v != v {
            let ended = if t.kind == Kind::End { ", the expression ended" } else { "" };
            return Err(fault("X400", format!("expected {v} at column {}{ended}", t.at + 1), t.at, None));
        }
        self.i += 1;
        Ok(())
    }

    fn enter(&mut self, at: usize) -> PResult<()> {
        self.depth += 1;
        if self.depth > SLATE_LIMITS.expr_depth {
            return Err(fault("X407", format!("nesting is deeper than {}", SLATE_LIMITS.expr_depth), at, None));
        }
        Ok(())
    }

    fn parse_all(&mut self) -> PResult<SlateExpr> {
        let e = self.pipeline()?;
        let t = self.peek().clone();
        if t.kind != Kind::End {
            if t.kind == Kind::Op && t.v == "=" {
                return Err(fault("X420", "compare with ==".into(), t.at, Some("==".into())));
            }
            let shown = if t.v.is_empty() { "text" } else { t.v.as_str() };
            return Err(fault("X400", format!("unexpected {shown} at column {}", t.at + 1), t.at, None));
        }
        Ok(e)
    }

    fn pipeline(&mut self) -> PResult<SlateExpr> {
        let at = self.peek().at;
        let head = self.expression()?;
        let mut steps: Vec<SlatePipeStepNode> = Vec::new();
        while self.is_op("|") {
            self.next();
            let name = self.next();
            if name.kind != Kind::Name {
                return Err(fault("Q420", format!("expected a step after | at column {}", name.at + 1), name.at, None));
            }
            let Some(spec) = pipe_step(&name.v) else {
                let names: Vec<String> = SLATE_PIPE_STEPS.iter().map(|(n, _)| n.to_string()).collect();
                let fix = match name.v.as_str() {
                    "filter" => Some("where".to_string()),
                    "sort" | "orderBy" => Some("sortBy".to_string()),
                    "limit" => Some("take".to_string()),
                    other => slate_nearest(other, &names),
                };
                let fn_note = if function_spec(&name.v).is_some() {
                    let how = if name.v == "pluck" { "pick(name) or pluck(list, 'name')".to_string() } else { format!("{}(list, ...)", name.v) };
                    format!("; {} is a function, not a step: {how}", name.v)
                } else {
                    String::new()
                };
                let tail = match &fix {
                    Some(f) => format!("; did you mean {f}?"),
                    None => fn_note,
                };
                let fix_out = fix.or_else(|| (name.v == "pluck").then(|| "pick(name) or pluck(list, 'name')".to_string()));
                return Err(fault("Q422", format!("{} is not a step{tail}", name.v), name.at, fix_out));
            };
            let mut args: Vec<SlatePipeArg> = Vec::new();
            if self.is_op("(") {
                self.next();
                if !self.is_op(")") {
                    loop {
                        let mut arg_name: Option<String> = None;
                        if self.peek().kind == Kind::Name && self.peek_n(1).kind == Kind::Op && self.peek_n(1).v == ":" {
                            arg_name = Some(self.next().v);
                            self.next();
                        }
                        let expr = self.pipeline()?;
                        args.push(SlatePipeArg { name: arg_name, expr });
                        if self.is_op(",") {
                            self.next();
                            continue;
                        }
                        break;
                    }
                }
                self.expect(")")?;
            }
            if args.len() < spec.min || args.len() > spec.max {
                let takes = if spec.min == spec.max {
                    spec.min.to_string()
                } else if spec.max >= 99 {
                    format!("at least {}", spec.min)
                } else {
                    format!("{} to {}", spec.min, spec.max)
                };
                let plural = if spec.max == 1 { "" } else { "s" };
                return Err(fault(
                    "Q423",
                    format!("{} takes {takes} argument{plural}, got {}: {}", name.v, args.len(), spec.sig),
                    name.at,
                    Some(spec.sig.to_string()),
                ));
            }
            if (name.v == "take" || name.v == "skip")
                && !matches!(&args[0].expr, SlateExpr::Lit { v, .. } if v.as_f64().is_some_and(|x| x.is_finite() && x.fract() == 0.0))
            {
                return Err(fault(
                    "Q425",
                    format!("{}'s count is a literal integer", name.v),
                    name.at,
                    Some(format!("{}(5)", name.v)),
                ));
            }
            if name.v == "pick" {
                for a in &args {
                    if !matches!(&a.expr, SlateExpr::Path { own: false, segs, .. } if segs.is_empty()) {
                        return Err(fault("Q423", "pick takes bare field names: pick(name, state)".into(), name.at, None));
                    }
                }
            }
            if name.v == "map" && args.iter().any(|a| a.name.is_none()) {
                return Err(fault("Q423", "map takes named arguments: map(name: item.title)".into(), name.at, None));
            }
            if name.v != "map" && args.iter().any(|a| a.name.is_some()) {
                return Err(fault("Q423", format!("{} takes no named arguments; only map does", name.v), name.at, None));
            }
            steps.push(SlatePipeStepNode { name: name.v, args, at: name.at + self.base });
        }
        if steps.len() > SLATE_LIMITS.pipeline_steps {
            return Err(fault("Q426", format!("{} steps; the most is {}", steps.len(), SLATE_LIMITS.pipeline_steps), at, None));
        }
        if steps.is_empty() {
            return Ok(head);
        }
        Ok(SlateExpr::Pipe { head: Box::new(head), steps, at: at + self.base })
    }

    fn expression(&mut self) -> PResult<SlateExpr> {
        let at = self.peek().at;
        self.enter(at)?;
        let test = self.or()?;
        let out = if self.is_op("?") {
            self.next();
            let then = self.expression()?;
            self.expect(":")?;
            let other = self.expression()?;
            SlateExpr::Cond { test: Box::new(test), then: Box::new(then), other: Box::new(other), at: at + self.base }
        } else {
            test
        };
        self.depth -= 1;
        Ok(out)
    }

    fn or(&mut self) -> PResult<SlateExpr> {
        let mut left = self.and()?;
        while self.is_word("or") || self.is_op("or") {
            let at = self.next().at;
            let right = self.and()?;
            left = SlateExpr::Bin { op: SlateBinOp::Or, left: Box::new(left), right: Box::new(right), at: at + self.base };
        }
        Ok(left)
    }

    fn and(&mut self) -> PResult<SlateExpr> {
        let mut left = self.not()?;
        while self.is_word("and") || self.is_op("and") {
            let at = self.next().at;
            let right = self.not()?;
            left = SlateExpr::Bin { op: SlateBinOp::And, left: Box::new(left), right: Box::new(right), at: at + self.base };
        }
        Ok(left)
    }

    fn not(&mut self) -> PResult<SlateExpr> {
        if self.is_word("not") || self.is_op("not") {
            let at = self.next().at;
            self.enter(at)?;
            let arg = self.not()?;
            self.depth -= 1;
            return Ok(SlateExpr::Not { arg: Box::new(arg), at: at + self.base });
        }
        self.compare()
    }

    fn compare(&mut self) -> PResult<SlateExpr> {
        let left = self.sum()?;
        let t = self.peek().clone();
        if let Some(op) = (t.kind == Kind::Op).then(|| bin_op(&t.v)).flatten().filter(|op| op.compare()) {
            self.next();
            let right = self.sum()?;
            let after = self.peek().clone();
            if after.kind == Kind::Op && bin_op(&after.v).is_some_and(SlateBinOp::compare) {
                return Err(fault(
                    "X400",
                    "comparisons do not chain; write a < b and b < c".into(),
                    after.at,
                    Some("a < b and b < c".into()),
                ));
            }
            return Ok(SlateExpr::Bin { op, left: Box::new(left), right: Box::new(right), at: t.at + self.base });
        }
        Ok(left)
    }

    fn sum(&mut self) -> PResult<SlateExpr> {
        let mut left = self.product()?;
        while self.is_op("+") || self.is_op("-") {
            let t = self.next();
            let right = self.product()?;
            left = SlateExpr::Bin { op: bin_op(&t.v).unwrap_or(SlateBinOp::Add), left: Box::new(left), right: Box::new(right), at: t.at + self.base };
        }
        Ok(left)
    }

    fn product(&mut self) -> PResult<SlateExpr> {
        let mut left = self.unary()?;
        while self.is_op("*") || self.is_op("/") || self.is_op("%") {
            let t = self.next();
            let right = self.unary()?;
            left = SlateExpr::Bin { op: bin_op(&t.v).unwrap_or(SlateBinOp::Mul), left: Box::new(left), right: Box::new(right), at: t.at + self.base };
        }
        Ok(left)
    }

    fn unary(&mut self) -> PResult<SlateExpr> {
        if self.is_op("-") {
            let at = self.next().at;
            self.enter(at)?;
            let arg = self.unary()?;
            self.depth -= 1;
            return Ok(SlateExpr::Neg { arg: Box::new(arg), at: at + self.base });
        }
        self.postfix()
    }

    fn postfix(&mut self) -> PResult<SlateExpr> {
        let mut node = self.primary()?;
        loop {
            if self.is_op(".") {
                self.next();
                let n = self.next();
                if n.kind != Kind::Name {
                    return Err(fault("X400", format!("expected a name after . at column {}", n.at + 1), n.at, None));
                }
                if self.is_op("(") {
                    let fix = method_fix(&n.v).map(str::to_string);
                    let message = if n.v == "test" {
                        "no regular expressions; use contains, startsWith or endsWith".to_string()
                    } else {
                        let how = fix.as_ref().map_or("use a function or a step".to_string(), |f| format!("use {f}"));
                        format!("no method calls; {how}")
                    };
                    return Err(fault("X420", message, n.at, fix));
                }
                if n.v == "length" {
                    return Err(fault("X420", "length is not a field; use len()".into(), n.at, Some("len(x)".into())));
                }
                node = match node {
                    SlateExpr::Path { head, own, mut segs, at } => {
                        segs.push(PathSeg::Key(n.v));
                        SlateExpr::Path { head, own, segs, at }
                    }
                    other => SlateExpr::Field { of: Box::new(other), name: PathSeg::Key(n.v), at: n.at + self.base },
                };
                continue;
            }
            if self.is_op("[") {
                let open = self.next();
                let neg = usize::from(self.is_op("-"));
                let n = self.peek_n(neg).clone();
                let closes = {
                    let t = self.peek_n(neg + 1);
                    t.kind == Kind::Op && t.v == "]"
                };
                if n.kind == Kind::Num && !n.v.is_empty() && n.v.bytes().all(|b| b.is_ascii_digit()) && closes {
                    self.i += neg + 2;
                    let magnitude: f64 = n.v.parse().unwrap_or(f64::INFINITY);
                    let seg = PathSeg::Index(seg_index(if neg == 1 { -magnitude } else { magnitude }));
                    node = match node {
                        SlateExpr::Path { head, own, mut segs, at } => {
                            segs.push(seg);
                            SlateExpr::Path { head, own, segs, at }
                        }
                        other => SlateExpr::Field { of: Box::new(other), name: seg, at: n.at + self.base },
                    };
                    continue;
                }
                let index = self.pipeline()?;
                self.expect("]")?;
                node = SlateExpr::Index { of: Box::new(node), index: Box::new(index), at: open.at + self.base };
                continue;
            }
            break;
        }
        Ok(node)
    }

    fn primary(&mut self) -> PResult<SlateExpr> {
        let t = self.next();
        let at = t.at + self.base;
        match t.kind {
            Kind::Num => {
                let v: f64 = t.v.parse().unwrap_or(f64::NAN);
                if !v.is_finite() {
                    return Err(fault("X400", format!("{} is past the largest number, at column {}", t.v, t.at + 1), t.at, None));
                }
                Ok(SlateExpr::Lit { v: jnum(v), at })
            }
            Kind::Str => Ok(SlateExpr::Lit { v: Value::String(t.v), at }),
            Kind::Tpl => {
                let mut parts: Vec<SlateTemplatePart> = Vec::new();
                for p in &t.parts {
                    match p {
                        RawPart::Text(s) => parts.push(SlateTemplatePart::Text(s.clone())),
                        RawPart::Hole { src, at: pos } => {
                            let inner = parse_at(&units(src), self.base + pos)?;
                            parts.push(SlateTemplatePart::Hole { expr: inner, src: js_trim(src).to_string(), at: self.base + pos });
                        }
                    }
                }
                let holes = parts.iter().filter(|p| matches!(p, SlateTemplatePart::Hole { .. })).count();
                if holes > SLATE_LIMITS.holes {
                    return Err(fault("X407", format!("{holes} holes in one template; the most is {}", SLATE_LIMITS.holes), t.at, None));
                }
                Ok(SlateExpr::Tpl { parts, at })
            }
            Kind::Own => Ok(SlateExpr::Path { head: t.v, own: true, segs: vec![], at }),
            Kind::Op if t.v == "(" => {
                let e = self.pipeline()?;
                self.expect(")")?;
                Ok(e)
            }
            Kind::Op if t.v == "[" => {
                let mut items: Vec<SlateExpr> = Vec::new();
                while !self.is_op("]") {
                    items.push(self.pipeline()?);
                    if !self.is_op(",") {
                        break;
                    }
                    self.next();
                }
                self.expect("]")?;
                Ok(SlateExpr::List { items, at })
            }
            Kind::Op if t.v == "{" => {
                let mut fields: Vec<(String, SlateExpr)> = Vec::new();
                while !self.is_op("}") {
                    let key = self.next();
                    if key.kind != Kind::Name && key.kind != Kind::Str {
                        return Err(fault(
                            "X400",
                            format!("a record's field is a name, at column {}: {{ at: time.now, v: 1 }}", key.at + 1),
                            key.at,
                            None,
                        ));
                    }
                    self.expect(":")?;
                    fields.push((key.v, self.pipeline()?));
                    if !self.is_op(",") {
                        break;
                    }
                    self.next();
                }
                self.expect("}")?;
                Ok(SlateExpr::Rec { fields, at })
            }
            Kind::Op if t.v == "/" => {
                Err(fault("X420", "no regular expressions; use contains, startsWith or endsWith".into(), t.at, None))
            }
            Kind::Name => {
                match t.v.as_str() {
                    "true" => return Ok(SlateExpr::Lit { v: Value::Bool(true), at }),
                    "false" => return Ok(SlateExpr::Lit { v: Value::Bool(false), at }),
                    "null" | "undefined" => return Ok(SlateExpr::Lit { v: Value::Null, at }),
                    _ => {}
                }
                if is_reserved(&t.v) {
                    return Err(fault("X400", format!("{} cannot start a value (column {})", t.v, t.at + 1), t.at, None));
                }
                if self.is_op("(") {
                    self.next();
                    let mut args: Vec<SlateExpr> = Vec::new();
                    if !self.is_op(")") {
                        loop {
                            args.push(self.pipeline()?);
                            if self.is_op(",") {
                                self.next();
                                continue;
                            }
                            break;
                        }
                    }
                    self.expect(")")?;
                    return Ok(SlateExpr::Call { name: t.v, args, at });
                }
                Ok(SlateExpr::Path { head: t.v, own: false, segs: vec![], at })
            }
            _ => {
                let message = if t.kind == Kind::End {
                    "the expression ended early".to_string()
                } else {
                    format!("unexpected {} at column {}", t.v, t.at + 1)
                };
                Err(fault("X400", message, t.at, None))
            }
        }
    }
}

fn is_reserved(word: &str) -> bool {
    matches!(word, "and" | "or" | "not" | "true" | "false" | "null")
}

fn bin_op(v: &str) -> Option<SlateBinOp> {
    Some(match v {
        "+" => SlateBinOp::Add,
        "-" => SlateBinOp::Sub,
        "*" => SlateBinOp::Mul,
        "/" => SlateBinOp::Div,
        "%" => SlateBinOp::Rem,
        "==" => SlateBinOp::Eq,
        "!=" => SlateBinOp::Ne,
        "<" => SlateBinOp::Lt,
        "<=" => SlateBinOp::Le,
        ">" => SlateBinOp::Gt,
        ">=" => SlateBinOp::Ge,
        "and" => SlateBinOp::And,
        "or" => SlateBinOp::Or,
        _ => return None,
    })
}

fn parse_at(src: &[u16], base: usize) -> PResult<SlateExpr> {
    let result = if js_trim(&text(src)).is_empty() {
        Err(fault("X400", "a hole is empty".into(), 0, None))
    } else {
        tokenize(src).and_then(|toks| Parser { toks, i: 0, depth: 0, base }.parse_all())
    };
    result.map_err(|e| {
        if e.absolute {
            e
        } else {
            Fault { code: e.code, message: shift_columns(&e.message, base), at: e.at + base, fix: e.fix, absolute: true }
        }
    })
}

fn problem(code: &str, message: &str, at: Option<usize>, fix: Option<&str>) -> SlateProblem {
    let mut out = slate_problem(code, message, fix);
    out.at = at.map(|a| a as i64);
    out
}

/// The syntax tree of an expression, or the first error it stops at. Positions are UTF-16 offsets into src.
pub fn parse_slate_expression(src: &str) -> SlateExprParse {
    let len = utf16::len(src);
    if len > SLATE_LIMITS.expr_chars {
        let message = format!("the expression is {len} characters; the most is {}", SLATE_LIMITS.expr_chars);
        return SlateExprParse { ast: None, errors: vec![problem("X407", &message, None, None)] };
    }
    if js_trim(src).is_empty() {
        return SlateExprParse { ast: None, errors: vec![problem("X400", "the expression is empty", Some(0), None)] };
    }
    match parse_at(&units(src), 0) {
        Ok(ast) => SlateExprParse { ast: Some(ast), errors: vec![] },
        Err(e) => SlateExprParse { ast: None, errors: vec![problem(e.code, &e.message, Some(e.at), e.fix.as_deref())] },
    }
}

/// The text of a path as dependency sets and resolvers name it: "pr.checks[0].name", "$check.json".
pub fn slate_path_text(head: &str, own: bool, segs: &[PathSeg]) -> String {
    let mut out = if own { format!("${head}") } else { head.to_string() };
    for seg in segs {
        match seg {
            PathSeg::Key(k) => {
                out.push('.');
                out.push_str(k);
            }
            PathSeg::Index(i) => out.push_str(&format!("[{i}]")),
        }
    }
    out
}

// ---- format strings ----

/// A format string's literal runs and its ${...} holes; "$${" is a literal "${".
pub fn parse_slate_format(src: &str) -> SlateFormatParse {
    let u = units(src);
    let mut parts: Vec<SlateFormatPart> = Vec::new();
    let mut lit: Vec<u16> = Vec::new();
    let mut i = 0;
    while i < u.len() {
        if starts(&u, i, "$${") {
            lit.extend("${".encode_utf16());
            i += 3;
            continue;
        }
        if starts(&u, i, "${") {
            let start = i + 2;
            let Some(end) = closing_brace_units(&u, start) else {
                let message = format!("a ${{ at column {} never closes", i + 1);
                return SlateFormatParse { parts, errors: vec![problem("X400", &message, Some(i), None)] };
            };
            if !lit.is_empty() {
                parts.push(SlateFormatPart::Text(text(&lit)));
                lit.clear();
            }
            parts.push(SlateFormatPart::Hole { expr: text(&u[start..end]), at: start });
            i = end + 1;
            continue;
        }
        lit.push(u[i]);
        i += 1;
    }
    if !lit.is_empty() {
        parts.push(SlateFormatPart::Text(text(&lit)));
    }
    let holes = parts.iter().filter(|p| matches!(p, SlateFormatPart::Hole { .. })).count();
    let errors = if holes > SLATE_LIMITS.holes {
        vec![problem("X407", &format!("{holes} holes in one text; the most is {}", SLATE_LIMITS.holes), None, None)]
    } else {
        vec![]
    };
    SlateFormatParse { parts, errors }
}

// ---- values ----

fn jnum(x: f64) -> Value {
    if x.fract() == 0.0 && x.abs() < 9.0e15 {
        Value::from(x as i64)
    } else {
        Value::from(x)
    }
}

fn fin(x: f64) -> Val {
    Some(if x.is_finite() { jnum(x) } else { Value::Null })
}

fn nul() -> Val {
    Some(Value::Null)
}

fn sval(s: String) -> Val {
    Some(Value::String(s))
}

fn as_num(v: Option<&Value>) -> Option<f64> {
    v.and_then(Value::as_f64).filter(|x| x.is_finite())
}

fn is_list(v: Option<&Value>) -> bool {
    matches!(v, Some(Value::Array(_)))
}

fn list_ref(v: Option<&Value>) -> Option<&Vec<Value>> {
    match v {
        Some(Value::Array(items)) => Some(items),
        _ => None,
    }
}

fn missing(v: Option<&Value>) -> bool {
    matches!(v, None | Some(Value::Null))
}

fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|x| x != 0.0),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(items)) => !items.is_empty(),
        Some(Value::Object(_)) => true,
    }
}

/// A value as a format hole and str() write it: nothing for null, JSON for a list or record.
fn text_of(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Number(n)) => number_string(n.as_f64().unwrap_or(f64::NAN)),
        Some(other) => json_stringify(other),
    }
}

/// Text joined from literal words and the values it reads: null when everything it reads is missing, so a label
/// whose data has not come draws its placeholder rather than half a sentence.
fn joined(parts: &[(bool, Val)]) -> Option<String> {
    if parts.iter().any(|(read, _)| *read) && parts.iter().all(|(read, v)| !read || missing(v.as_ref())) {
        return None;
    }
    Some(parts.iter().map(|(_, v)| text_of(v.as_ref())).collect())
}

/// JSON's key order for an object: array-index keys ascending, then the rest as given.
fn js_object(entries: impl IntoIterator<Item = (String, Value)>) -> Value {
    let mut map: Map<String, Value> = Map::new();
    for (k, v) in entries {
        map.insert(k, v);
    }
    let ordered: Map<String, Value> = js_key_order(map.iter()).into_iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    Value::Object(ordered)
}

/// A time as ms: a number is ms epoch, an ISO string is parsed.
fn ms(v: Option<&Value>) -> Option<f64> {
    if let Some(x) = as_num(v) {
        return Some(x);
    }
    match v {
        Some(Value::String(s)) => {
            let t = date_parse(s);
            (!t.is_nan()).then_some(t)
        }
        _ => None,
    }
}

fn places(v: Option<&Value>, fallback: u32) -> u32 {
    as_num(v).map_or(fallback, |x| js_round(x).clamp(0.0, 6.0) as u32)
}

/// "3d 7h", "3h 24m", "12m 4s", "40s": the two largest units.
fn span(ms_left: f64, two: bool) -> String {
    let s = js_round(ms_left / 1000.0).max(0.0);
    let d = (s / 86_400.0).floor();
    let h = ((s % 86_400.0) / 3_600.0).floor();
    let m = ((s % 3_600.0) / 60.0).floor();
    let sec = s % 60.0;
    if d > 0.0 {
        return if two && h > 0.0 { format!("{}d {}h", number_string(d), number_string(h)) } else { format!("{}d", number_string(d)) };
    }
    if h > 0.0 {
        return if two && m > 0.0 { format!("{}h {}m", number_string(h), number_string(m)) } else { format!("{}h", number_string(h)) };
    }
    if m > 0.0 {
        return if two && sec > 0.0 { format!("{}m {}s", number_string(m), number_string(sec)) } else { format!("{}m", number_string(m)) };
    }
    format!("{}s", number_string(sec))
}

fn plain_number(n: f64, places: Option<u32>) -> String {
    match places {
        None => locale_number(n, Grouping::EnUs, 0, 2),
        Some(p) => locale_number(n, Grouping::EnUs, p, p),
    }
}

const WEEKDAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const MONTHS: [&str; 12] = [
    "January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December",
];
const MONTHS_SHORT: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sept", "Oct", "Nov", "Dec"];

fn local_fields(at: f64) -> Option<Fields> {
    let t = js_date::time_value(at)?;
    let zone = DateZone::system();
    Some(js_date::fields(t, zone.offset_seconds(t)))
}

/// `toLocaleDateString("en-GB", ...)` with the comma removed, as the TS's `.replace(",", "")` leaves it.
fn en_gb_date(at: f64, long: bool) -> String {
    let Some(f) = local_fields(at) else {
        return "Invalid Date".to_string();
    };
    let weekday = WEEKDAYS[f.weekday as usize];
    if long {
        format!("{weekday} {} {} {}", f.day, MONTHS[f.month as usize - 1], js_date::intl_year(f.year))
    } else {
        format!("{} {} {}", &weekday[..3], f.day, MONTHS_SHORT[f.month as usize - 1])
    }
}

fn clock_text(at: f64) -> String {
    let zone = DateZone::system();
    fmt_clock(Some(at), &|t: f64| zone.offset_seconds(t.trunc() as i64))
}

/// The app's own word for a known state (04, word()): check states, thread statuses, review and mergeable enums,
/// run states; any other string capitalised once.
fn known_word(x: &str) -> Option<&'static str> {
    if let Some((_, word)) = CHECK_STATE_WORDS.iter().find(|(k, _)| *k == x) {
        return Some(*word);
    }
    Some(match x {
        "starting" => "Starting",
        "working" => "Working",
        "needs-you" => "Needs you",
        "resting" => "Resting",
        "done" => "Done",
        "failed" => "Failed",
        "none" => "None",
        "approved" => "Approved",
        "changes_asked" => "Changes asked",
        "required" => "Review required",
        "mergeable" => "Mergeable",
        "conflicting" => "Conflicting",
        "unknown" => "Unknown",
        "idle" => "Not run yet",
        "held" => "Held",
        "running" => "Running",
        "open" => "Open",
        "merged" => "Merged",
        "closed" => "Closed",
        _ => return None,
    })
}

fn slate_word(x: Option<&Value>) -> Val {
    let Some(Value::String(s)) = x else {
        return if missing(x) { nul() } else { sval(text_of(x)) };
    };
    if let Some(known) = known_word(s) {
        return sval(known.to_string());
    }
    let collapsed: String = {
        let mut out = String::new();
        let mut in_run = false;
        for ch in s.chars() {
            if ch == '_' || ch == '-' {
                if !in_run {
                    out.push(' ');
                }
                in_run = true;
            } else {
                out.push(ch);
                in_run = false;
            }
        }
        out
    };
    let spaced = js_trim(&collapsed);
    let mut chars = spaced.chars();
    let Some(first) = chars.next() else {
        return sval(String::new());
    };
    let head: String = if first.len_utf16() == 1 { first.to_uppercase().collect() } else { first.to_string() };
    sval(format!("{head}{}", chars.as_str()))
}

fn value_text_for_split(sep: &str, s: &str) -> Vec<String> {
    if sep.is_empty() {
        return s.encode_utf16().map(|u| String::from_utf16_lossy(&[u])).collect();
    }
    s.split(sep).map(str::to_string).collect()
}

fn normalize(v: Value) -> Value {
    match v {
        Value::Number(n) => jnum(n.as_f64().unwrap_or(f64::NAN)),
        Value::Array(items) => Value::Array(items.into_iter().map(normalize).collect()),
        Value::Object(m) => js_object(m.into_iter().map(|(k, x)| (k, normalize(x)))),
        other => other,
    }
}

/// A list's first `cap` items, JavaScript's `slice(0, listItems)`.
fn capped(mut items: Vec<Value>) -> Vec<Value> {
    items.truncate(SLATE_LIMITS.list_items);
    items
}

// ---- functions ----

fn arg(a: &[Val], i: usize) -> Option<&Value> {
    a.get(i).and_then(Option::as_ref)
}

fn undef(a: &[Val], i: usize) -> bool {
    a.get(i).map_or(true, Option::is_none)
}

fn ret_str(_: &[SlateType]) -> SlateType {
    str_t()
}

fn ret_num(_: &[SlateType]) -> SlateType {
    num_t()
}

fn ret_bool(_: &[SlateType]) -> SlateType {
    bool_t()
}

fn ret_any(_: &[SlateType]) -> SlateType {
    any_t()
}

fn ret_list(_: &[SlateType]) -> SlateType {
    list_t()
}

fn elem_of(a: &[SlateType]) -> SlateType {
    a.first().and_then(|t| t.of.as_deref().cloned()).unwrap_or_else(any_t)
}

fn merge(a: SlateType, b: SlateType) -> SlateType {
    if a.t == b.t {
        return a;
    }
    if a.t == SlateTypeName::Null {
        return b;
    }
    if b.t == SlateTypeName::Null {
        return a;
    }
    any_t()
}

/// list[i] or record[key] for a computed index: a whole number counts from the end when negative.
fn pick_at(of: Option<&Value>, at: Option<&Value>) -> Val {
    if let (Some(items), Some(i)) = (list_ref(of), as_num(at)) {
        if i.fract() != 0.0 {
            return nul();
        }
        let idx = if i < 0.0 { items.len() as f64 + i } else { i };
        if idx >= 0.0 && (idx as usize) < items.len() {
            return Some(items[idx as usize].clone());
        }
        return nul();
    }
    if let (Some(Value::Object(fields)), Some(Value::String(key))) = (of, at) {
        return Some(fields.get(key).cloned().unwrap_or(Value::Null));
    }
    nul()
}

/// A list's element at a path step, read through a name when one is given (JS `item[name]`, else the item).
fn field_of(item: &Value, name: Option<&Value>) -> Val {
    match name {
        Some(Value::String(key)) => slate_step(Some(item.clone()), &PathSeg::Key(key.clone())),
        _ => Some(item.clone()),
    }
}

fn numbers_of(env: &Env, list: Option<&Value>, name: Option<&Value>) -> Res<Vec<f64>> {
    let Some(items) = list_ref(list) else {
        return Ok(vec![]);
    };
    let mut out = Vec::new();
    for item in items {
        env.charge(1)?;
        let v = if name.is_none() { Some(item.clone()) } else { field_of(item, name) };
        if let Some(x) = as_num(v.as_ref()) {
            out.push(x);
        }
    }
    Ok(out)
}

fn list_or_args(a: &[Val]) -> Vec<Val> {
    if let Some(Value::Array(items)) = arg(a, 0).filter(|_| a.len() == 1) {
        return items.iter().map(|v| Some(v.clone())).collect();
    }
    a.iter().map(|v| Some(v.clone().unwrap_or(Value::Null))).collect()
}

fn heldtext(v: Val) -> Res<Val> {
    if let Some(Value::String(s)) = &v {
        if utf16::len(s) > SLATE_LIMITS.values_bytes {
            return Err(Over);
        }
    }
    Ok(v)
}

const fn func(min: usize, max: usize, sig: &'static str, example: &'static str, returns: fn(&[SlateType]) -> SlateType, body: FnBody) -> SlateFunction {
    SlateFunction { min, max, sig, example, returns, null_safe: false, number_first: false, list_first: false, clock: false, body }
}

/// Every function with its signature and an example, for the catalog and the checker.
pub static SLATE_FUNCTIONS: &[(&str, SlateFunction)] = &[
    ("percent", SlateFunction { number_first: true, ..func(1, 2, "percent(fraction, places?)", "percent(thread.context.used / thread.context.window)", ret_str, |a, _, _| {
        let Some(v) = as_num(arg(a, 0)).map(|x| x * 100.0).filter(|v| v.is_finite()) else {
            return Ok(nul());
        };
        let fallback = if v.abs() < 10.0 && v != 0.0 { 1 } else { 0 };
        Ok(sval(format!("{}%", to_fixed(v, places(arg(a, 1), fallback)))))
    }) }),
    ("pct", SlateFunction { number_first: true, ..func(1, 2, "pct(points, places?)", "pct(usage.week.percent)", ret_str, |a, _, _| {
        Ok(as_num(arg(a, 0)).map_or(nul(), |x| sval(format!("{}%", to_fixed(x, places(arg(a, 1), 0))))))
    }) }),
    ("tokens", SlateFunction { number_first: true, ..func(1, 1, "tokens(n)", "tokens(thread.context.free)", ret_str, |a, _, _| {
        Ok(as_num(arg(a, 0)).map_or(nul(), |x| sval(fmt_tokens(x))))
    }) }),
    ("bytes", SlateFunction { number_first: true, ..func(1, 1, "bytes(n)", "bytes(machine.mem.used)", ret_str, |a, _, _| {
        Ok(as_num(arg(a, 0)).map_or(nul(), |x| sval(fmt_bytes(x))))
    }) }),
    ("usd", SlateFunction { number_first: true, ..func(1, 2, "usd(n, places?)", "usd(thread.cost.usd)", ret_str, |a, _, _| {
        let Some(x) = as_num(arg(a, 0)) else {
            return Ok(nul());
        };
        Ok(sval(if undef(a, 1) { fmt_cost(x) } else { format!("${}", plain_number(x, Some(places(arg(a, 1), 2)))) }))
    }) }),
    ("number", SlateFunction { number_first: true, ..func(1, 2, "number(x, places?)", "number(machine.load1, 2)", ret_str, |a, _, _| {
        let Some(x) = as_num(arg(a, 0)) else {
            return Ok(nul());
        };
        Ok(sval(plain_number(x, if undef(a, 1) { None } else { Some(places(arg(a, 1), 0)) })))
    }) }),
    ("duration", SlateFunction { number_first: true, ..func(1, 2, "duration(ms, style?)", "duration(thread.lastTurn.durationMs)", ret_str, |a, _, _| {
        let Some(x) = as_num(arg(a, 0)) else {
            return Ok(nul());
        };
        let style = if matches!(arg(a, 1), Some(Value::String(s)) if s == "clock") { DurationStyle::Clock } else { DurationStyle::Short };
        Ok(sval(fmt_duration(x, Some(style))))
    }) }),
    ("ago", SlateFunction { clock: true, ..func(1, 1, "ago(t)", "ago(pr.readAt)", ret_str, |a, env, _| {
        let Some(at) = ms(arg(a, 0)) else {
            return Ok(nul());
        };
        Ok(sval(format!("{} ago", span(env.now() - at, false))))
    }) }),
    ("until", SlateFunction { clock: true, ..func(1, 1, "until(t)", "until(usage.week.resetsAt)", ret_str, |a, env, _| {
        let Some(at) = ms(arg(a, 0)) else {
            return Ok(nul());
        };
        let now = env.now();
        Ok(sval(if at <= now { "now".to_string() } else { format!("in {}", span(at - now, true)) }))
    }) }),
    ("date", SlateFunction { ..func(1, 2, "date(t, style?)", "date(usage.week.resetsAt)", ret_str, |a, _, _| {
        let Some(at) = ms(arg(a, 0)) else {
            return Ok(nul());
        };
        Ok(sval(en_gb_date(at, matches!(arg(a, 1), Some(Value::String(s)) if s == "long"))))
    }) }),
    ("time", SlateFunction { ..func(1, 1, "time(t)", "time(usage.session.resetsAt)", ret_str, |a, _, _| {
        let Some(at) = ms(arg(a, 0)) else {
            return Ok(nul());
        };
        Ok(sval(utf16::slice(&clock_text(at), 0, Some(5))))
    }) }),
    ("weekday", SlateFunction { ..func(1, 1, "weekday(t)", "weekday(usage.week.resetsAt)", ret_str, |a, _, _| {
        let Some(at) = ms(arg(a, 0)) else {
            return Ok(nul());
        };
        Ok(sval(match local_fields(at) {
            Some(f) => WEEKDAYS[f.weekday as usize].to_string(),
            None => "Invalid Date".to_string(),
        }))
    }) }),
    ("plural", SlateFunction { number_first: true, ..func(2, 3, "plural(n, one, many?)", "plural(len(pr.checks), 'check')", ret_str, |a, _, _| {
        let (Some(n), Some(Value::String(one))) = (as_num(arg(a, 0)), arg(a, 1).filter(|v| v.is_string())) else {
            return Ok(nul());
        };
        let noun = if n == 1.0 {
            one.clone()
        } else if let Some(Value::String(many)) = arg(a, 2) {
            many.clone()
        } else {
            format!("{one}s")
        };
        Ok(sval(format!("{} {noun}", plain_number(n, None))))
    }) }),
    ("word", SlateFunction { ..func(1, 1, "word(x)", "word(item.state)", ret_str, |a, _, _| Ok(slate_word(arg(a, 0)))) }),
    ("upper", SlateFunction { ..func(1, 1, "upper(s)", "upper(git.branch)", ret_str, |a, _, _| {
        Ok(match arg(a, 0) {
            Some(Value::String(s)) => sval(s.to_uppercase()),
            _ => nul(),
        })
    }) }),
    ("lower", SlateFunction { ..func(1, 1, "lower(s)", "lower(pr.word)", ret_str, |a, _, _| {
        Ok(match arg(a, 0) {
            Some(Value::String(s)) => sval(s.to_lowercase()),
            _ => nul(),
        })
    }) }),
    ("trim", SlateFunction { ..func(1, 1, "trim(s)", "trim($note)", ret_str, |a, _, _| {
        Ok(match arg(a, 0) {
            Some(Value::String(s)) => sval(js_trim(s).to_string()),
            _ => nul(),
        })
    }) }),
    ("short", SlateFunction { ..func(2, 2, "short(s, n)", "short(pr.headSubject, 40)", ret_str, |a, _, _| {
        let (Some(Value::String(s)), Some(n)) = (arg(a, 0), as_num(arg(a, 1))) else {
            return Ok(nul());
        };
        if (utf16::len(s) as f64) > n {
            let end = (n - 1.0).max(0.0).trunc() as i64;
            return Ok(sval(format!("{}…", utf16::slice(s, 0, Some(end)))));
        }
        Ok(sval(s.clone()))
    }) }),
    ("round", SlateFunction { number_first: true, ..func(1, 2, "round(x, places?)", "round(usage.week.percent)", ret_num, |a, _, _| {
        let Some(x) = as_num(arg(a, 0)) else {
            return Ok(nul());
        };
        let f = 10f64.powi(places(arg(a, 1), 0) as i32);
        Ok(fin(js_round(x * f) / f))
    }) }),
    ("floor", SlateFunction { number_first: true, ..func(1, 1, "floor(x)", "floor(thread.context.percent)", ret_num, |a, _, _| {
        Ok(as_num(arg(a, 0)).map_or(nul(), |x| fin(x.floor())))
    }) }),
    ("ceil", SlateFunction { number_first: true, ..func(1, 1, "ceil(x)", "ceil(thread.context.percent)", ret_num, |a, _, _| {
        Ok(as_num(arg(a, 0)).map_or(nul(), |x| fin(x.ceil())))
    }) }),
    ("abs", SlateFunction { number_first: true, ..func(1, 1, "abs(x)", "abs(git.behind)", ret_num, |a, _, _| {
        Ok(as_num(arg(a, 0)).map_or(nul(), |x| fin(x.abs())))
    }) }),
    ("min", SlateFunction { null_safe: true, ..func(1, 99, "min(a, b, ...) or min(list)", "min(usage.week.percent, 100)", ret_num, |a, env, _| {
        let ns = numbers_of_values(env, &list_or_args(a))?;
        Ok(ns.iter().copied().reduce(f64::min).map_or(nul(), fin))
    }) }),
    ("max", SlateFunction { null_safe: true, ..func(1, 99, "max(a, b, ...) or max(list)", "max(0, thread.context.free)", ret_num, |a, env, _| {
        let ns = numbers_of_values(env, &list_or_args(a))?;
        Ok(ns.iter().copied().reduce(f64::max).map_or(nul(), fin))
    }) }),
    ("clamp", SlateFunction { number_first: true, ..func(3, 3, "clamp(x, lo, hi)", "clamp(usage.week.percent, 0, 100)", ret_num, |a, _, _| {
        Ok(match (as_num(arg(a, 0)), as_num(arg(a, 1)), as_num(arg(a, 2))) {
            (Some(x), Some(lo), Some(hi)) => fin(hi.min(lo.max(x))),
            _ => nul(),
        })
    }) }),
    ("str", SlateFunction { ..func(1, 1, "str(x)", "str(pr.number)", ret_str, |a, _, _| {
        Ok(if missing(arg(a, 0)) { nul() } else { sval(text_of(arg(a, 0))) })
    }) }),
    ("num", SlateFunction { ..func(1, 1, "num(x)", "num(item.completedAt)", ret_num, |a, _, _| {
        let x = arg(a, 0);
        if let Some(n) = as_num(x) {
            return Ok(fin(n));
        }
        match x {
            Some(Value::Bool(b)) => Ok(Some(jnum(if *b { 1.0 } else { 0.0 }))),
            Some(Value::String(s)) if !js_trim(s).is_empty() => {
                let n = to_number(s);
                if n.is_finite() {
                    return Ok(fin(n));
                }
                let b = s.as_bytes();
                let iso_date = b.len() >= 10
                    && b[..4].iter().all(u8::is_ascii_digit)
                    && b[4] == b'-'
                    && b[5..7].iter().all(u8::is_ascii_digit)
                    && b[7] == b'-'
                    && b[8..10].iter().all(u8::is_ascii_digit);
                Ok(if iso_date { ms(x).map_or(nul(), fin) } else { nul() })
            }
            _ => Ok(nul()),
        }
    }) }),
    ("bool", SlateFunction { null_safe: true, ..func(1, 1, "bool(x)", "bool($done)", ret_bool, |a, _, _| Ok(Some(Value::Bool(truthy(arg(a, 0)))))) }),
    ("concat", SlateFunction { null_safe: true, ..func(1, 99, "concat(a, b, ...)", "concat(git.branch, ' at ', git.head)", ret_str, |a, _, nodes| {
        let parts: Vec<(bool, Val)> = a.iter().enumerate().map(|(i, v)| (!matches!(nodes.get(i), Some(SlateExpr::Lit { .. })), v.clone())).collect();
        Ok(joined(&parts).map_or(nul(), |s| sval(s)))
    }) }),
    ("json", SlateFunction { ..func(1, 1, "json(s)", "json($check.out).title", ret_any, |a, _, _| {
        Ok(match arg(a, 0) {
            Some(Value::String(s)) => parse_json_lax(s).map(normalize).or(nul()),
            _ => nul(),
        })
    }) }),
    ("lines", SlateFunction { ..func(1, 1, "lines(s)", "lines($tests.out)", ret_list, |a, _, _| {
        let Some(Value::String(s)) = arg(a, 0) else {
            return Ok(nul());
        };
        let pieces: Vec<&str> = s.split('\n').collect();
        let last = pieces.len() - 1;
        let mut out: Vec<String> = pieces
            .iter()
            .enumerate()
            .map(|(i, p)| if i < last { p.strip_suffix('\r').unwrap_or(p).to_string() } else { p.to_string() })
            .collect();
        if out.last().is_some_and(String::is_empty) {
            out.pop();
        }
        out.truncate(SLATE_LIMITS.list_items);
        Ok(Some(Value::Array(out.into_iter().map(Value::String).collect())))
    }) }),
    ("split", SlateFunction { ..func(2, 2, "split(s, sep)", "split($tags, ',')", |_| list_of(str_t()), |a, _, _| {
        Ok(match (arg(a, 0), arg(a, 1)) {
            (Some(Value::String(s)), Some(Value::String(sep))) => {
                let mut pieces = value_text_for_split(sep, s);
                pieces.truncate(SLATE_LIMITS.list_items);
                Some(Value::Array(pieces.into_iter().map(Value::String).collect()))
            }
            _ => nul(),
        })
    }) }),
    ("replace", SlateFunction { ..func(3, 3, "replace(s, from, to)", "replace(git.branch, '/', ' ')", ret_str, |a, _, _| {
        Ok(match (arg(a, 0), arg(a, 1)) {
            (Some(Value::String(s)), Some(Value::String(from))) if !from.is_empty() => sval(s.replace(from.as_str(), &text_of(arg(a, 2)))),
            (Some(Value::String(s)), _) => sval(s.clone()),
            _ => nul(),
        })
    }) }),
    ("contains", SlateFunction { null_safe: true, ..func(2, 2, "contains(list or text, value)", "contains(pluck(pr.checks, 'state'), 'fail')", ret_bool, |a, env, _| {
        match arg(a, 0) {
            Some(Value::String(x)) => Ok(Some(Value::Bool(match arg(a, 1) {
                Some(Value::String(v)) => x.contains(v.as_str()),
                _ => false,
            }))),
            Some(Value::Array(items)) => {
                env.charge(items.len())?;
                let wanted = a.get(1).and_then(Option::as_ref).cloned().unwrap_or(Value::Null);
                Ok(Some(Value::Bool(items.iter().any(|item| slate_equal(Some(item), Some(&wanted))))))
            }
            _ => Ok(Some(Value::Bool(false))),
        }
    }) }),
    ("startsWith", SlateFunction { null_safe: true, ..func(2, 2, "startsWith(s, p)", "startsWith(git.branch, 'poc/')", ret_bool, |a, _, _| {
        Ok(Some(Value::Bool(match (arg(a, 0), arg(a, 1)) {
            (Some(Value::String(s)), Some(Value::String(p))) => s.starts_with(p.as_str()),
            _ => false,
        })))
    }) }),
    ("endsWith", SlateFunction { null_safe: true, ..func(2, 2, "endsWith(s, p)", "endsWith(item.path, '.ts')", ret_bool, |a, _, _| {
        Ok(Some(Value::Bool(match (arg(a, 0), arg(a, 1)) {
            (Some(Value::String(s)), Some(Value::String(p))) => s.ends_with(p.as_str()),
            _ => false,
        })))
    }) }),
    ("orElse", SlateFunction { null_safe: true, ..func(2, 2, "orElse(x, fallback)", "orElse(pr.word, 'no pull request')", |a| {
        if matches!(a.first().map(|t| t.t), Some(SlateTypeName::Any | SlateTypeName::Null)) {
            a.get(1).cloned().unwrap_or_else(any_t)
        } else {
            a.first().cloned().unwrap_or_else(any_t)
        }
    }, |a, _, _| {
        Ok(if missing(arg(a, 0)) { Some(arg(a, 1).cloned().unwrap_or(Value::Null)) } else { arg(a, 0).cloned() })
    }) }),
    ("exists", SlateFunction { null_safe: true, ..func(1, 1, "exists(x)", "exists(usage.note)", ret_bool, |a, _, _| Ok(Some(Value::Bool(!missing(arg(a, 0)))))) }),
    ("isNull", SlateFunction { null_safe: true, ..func(1, 1, "isNull(x)", "isNull(pr.number)", ret_bool, |a, _, _| Ok(Some(Value::Bool(missing(arg(a, 0)))))) }),
    ("coalesce", SlateFunction { null_safe: true, ..func(1, 99, "coalesce(a, b, ...)", "coalesce(pr.headSubject, git.branch)", ret_any, |a, _, _| {
        Ok(Some(a.iter().find(|v| !missing(v.as_ref())).and_then(Clone::clone).unwrap_or(Value::Null)))
    }) }),
    ("if", SlateFunction { null_safe: true, ..func(3, 3, "if(cond, a, b)", "if(pr.draft, 'Draft', 'Ready')", |a| {
        merge(a.get(1).cloned().unwrap_or_else(any_t), a.get(2).cloned().unwrap_or_else(any_t))
    }, |a, _, _| {
        let chosen = if truthy(arg(a, 0)) { a.get(1) } else { a.get(2) };
        Ok(Some(chosen.and_then(Clone::clone).unwrap_or(Value::Null)))
    }) }),
    ("len", SlateFunction { null_safe: true, ..func(1, 1, "len(x)", "len(pr.checks)", ret_num, |a, _, _| {
        Ok(Some(Value::from(match arg(a, 0) {
            Some(Value::Array(items)) => items.len() as u64,
            Some(Value::String(s)) => utf16::len(s) as u64,
            Some(Value::Object(fields)) => fields.len() as u64,
            _ => 0,
        })))
    }) }),
    ("count", SlateFunction { null_safe: true, list_first: true, ..func(1, 1, "count(list)", "count(pr.checks)", ret_num, |a, _, _| {
        Ok(Some(Value::from(list_ref(arg(a, 0)).map_or(0, Vec::len) as u64)))
    }) }),
    ("sum", SlateFunction { list_first: true, ..func(1, 2, "sum(list, field?)", "sum(thread.changes.files, 'additions')", ret_num, |a, env, _| {
        if !is_list(arg(a, 0)) {
            return Ok(nul());
        }
        let ns = numbers_of(env, arg(a, 0), arg(a, 1))?;
        Ok(fin(ns.iter().fold(0.0, |acc, x| acc + x)))
    }) }),
    ("avg", SlateFunction { list_first: true, ..func(1, 2, "avg(list, field?)", "avg(thread.changes.files, 'additions')", ret_num, |a, env, _| {
        let ns = numbers_of(env, arg(a, 0), arg(a, 1))?;
        if ns.is_empty() {
            return Ok(nul());
        }
        Ok(fin(ns.iter().fold(0.0, |acc, x| acc + x) / ns.len() as f64))
    }) }),
    ("first", SlateFunction { list_first: true, ..func(1, 2, "first(list, n?)", "first(pr.checks).name", |a| {
        if a.len() > 1 {
            a.first().cloned().unwrap_or_else(list_t)
        } else {
            elem_of(a)
        }
    }, |a, _, _| {
        let Some(items) = list_ref(arg(a, 0)) else {
            return Ok(nul());
        };
        if undef(a, 1) {
            return Ok(Some(items.first().cloned().unwrap_or(Value::Null)));
        }
        Ok(match as_num(arg(a, 1)) {
            Some(n) => {
                let k = n.floor().max(0.0) as usize;
                Some(Value::Array(items.iter().take(k).cloned().collect()))
            }
            None => nul(),
        })
    }) }),
    ("last", SlateFunction { list_first: true, ..func(1, 2, "last(list, n?)", "last(append($hist, $spot.json.v), 60)", |a| {
        if a.len() > 1 {
            a.first().cloned().unwrap_or_else(list_t)
        } else {
            elem_of(a)
        }
    }, |a, _, _| {
        let Some(items) = list_ref(arg(a, 0)) else {
            return Ok(nul());
        };
        if undef(a, 1) {
            return Ok(Some(items.last().cloned().unwrap_or(Value::Null)));
        }
        Ok(match as_num(arg(a, 1)) {
            Some(n) if n >= 1.0 => {
                let k = n.floor() as usize;
                Some(Value::Array(items[items.len().saturating_sub(k)..].to_vec()))
            }
            Some(_) => Some(Value::Array(vec![])),
            None => nul(),
        })
    }) }),
    ("at", SlateFunction { list_first: true, ..func(2, 2, "at(list, i)", "at($questions, $i).text", elem_of_first, |a, _, _| {
        Ok(pick_at(arg(a, 0), arg(a, 1)))
    }) }),
    ("append", SlateFunction { null_safe: true, ..func(2, 2, "append(list, x)", "append($hist, { at: time.now, v: $spot.json.v })", |a| {
        let of = a.first().and_then(|t| t.of.as_deref().cloned()).or_else(|| a.get(1).cloned());
        SlateType { t: SlateTypeName::List, of: of.map(Box::new), fields: None }
    }, |a, env, _| {
        let items: Vec<Value> = list_ref(arg(a, 0)).cloned().unwrap_or_default();
        env.charge(items.len())?;
        let mut out = items;
        out.push(arg(a, 1).cloned().unwrap_or(Value::Null));
        Ok(Some(Value::Array(out)))
    }) }),
    ("pluck", SlateFunction { list_first: true, ..func(2, 2, "pluck(list, field)", "pluck(pr.checks, 'name')", ret_list, |a, env, _| {
        let (Some(items), Some(Value::String(f))) = (list_ref(arg(a, 0)), arg(a, 1)) else {
            return Ok(nul());
        };
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            env.charge(1)?;
            out.push(field_of(item, Some(&Value::String(f.clone()))).unwrap_or(Value::Null));
        }
        Ok(Some(Value::Array(out)))
    }) }),
    ("join", SlateFunction { list_first: true, ..func(2, 2, "join(list, sep)", "join(pluck(pr.checks, 'name'), ', ')", ret_str, |a, env, _| {
        let Some(items) = list_ref(arg(a, 0)) else {
            return Ok(nul());
        };
        let sep = match arg(a, 1) {
            Some(Value::String(s)) => s.clone(),
            _ => ", ".to_string(),
        };
        let mut pieces = Vec::with_capacity(items.len());
        for item in items {
            env.charge(1)?;
            pieces.push(text_of(Some(item)));
        }
        Ok(sval(pieces.join(&sep)))
    }) }),
];

fn elem_of_first(a: &[SlateType]) -> SlateType {
    elem_of(a)
}

/// The numbers among the values, each read as a number (JS `Math.min(...ns)` input after numbersOf with no field).
fn numbers_of_values(env: &Env, items: &[Val]) -> Res<Vec<f64>> {
    let mut out = Vec::new();
    for item in items {
        env.charge(1)?;
        if let Some(x) = as_num(item.as_ref()) {
            out.push(x);
        }
    }
    Ok(out)
}

fn function_spec(name: &str) -> Option<&'static SlateFunction> {
    SLATE_FUNCTIONS.iter().find(|(n, _)| *n == name).map(|(_, spec)| spec)
}

// ---- pipeline steps ----

fn pipe_step(name: &str) -> Option<&'static SlatePipeStep> {
    SLATE_PIPE_STEPS.iter().find(|(n, _)| *n == name).map(|(_, spec)| spec)
}

fn same_t(input: &SlateType, _: &[SlateType]) -> SlateType {
    input.clone()
}

fn num_out(_: &SlateType, _: &[SlateType]) -> SlateType {
    num_t()
}

fn list_rec_out(_: &SlateType, _: &[SlateType]) -> SlateType {
    list_of(rec_t())
}

fn list_out(_: &SlateType, _: &[SlateType]) -> SlateType {
    list_t()
}

fn elem_out(input: &SlateType, _: &[SlateType]) -> SlateType {
    input.of.as_deref().cloned().unwrap_or_else(any_t)
}

/// The keys of a step's group: how they compare, for sortBy. Numbers, then text, then everything else, nulls last.
fn rank(v: &Val) -> u8 {
    if as_num(v.as_ref()).is_some() {
        0
    } else if matches!(v, Some(Value::String(_))) {
        1
    } else if missing(v.as_ref()) {
        3
    } else {
        2
    }
}

fn compare_keys(a: &Val, b: &Val, desc: bool) -> Ordering {
    let (ra, rb) = (rank(a), rank(b));
    if ra == 3 || rb == 3 {
        return ra.cmp(&rb);
    }
    let base = if ra != rb {
        ra.cmp(&rb)
    } else if ra == 0 {
        as_num(a.as_ref()).unwrap_or(0.0).partial_cmp(&as_num(b.as_ref()).unwrap_or(0.0)).unwrap_or(Ordering::Equal)
    } else if ra == 1 {
        let (Some(Value::String(x)), Some(Value::String(y))) = (a, b) else {
            return Ordering::Equal;
        };
        collate(x, y)
    } else {
        Ordering::Equal
    };
    if desc {
        base.reverse()
    } else {
        base
    }
}

/// JSON.stringify of a key: the identity of a value for grouping and distinct.
fn key_text(v: &Val) -> String {
    json_stringify(v.as_ref().unwrap_or(&Value::Null))
}

fn field_name(e: &SlateExpr) -> Option<String> {
    match e {
        SlateExpr::Path { head, own: false, segs, .. } if segs.is_empty() => Some(head.clone()),
        SlateExpr::Path { own: false, head, segs, .. } if head == "item" && segs.len() == 1 => match &segs[0] {
            PathSeg::Key(k) => Some(k.clone()),
            PathSeg::Index(_) => None,
        },
        SlateExpr::Lit { v: Value::String(s), .. } => Some(s.clone()),
        _ => None,
    }
}

fn pipe_sort(x: PipeRun) -> Res<Val> {
    let desc = x.args.get(1).is_some() && matches!(run(x.arg(1), x.env)?, Some(Value::String(ref s)) if s == "desc");
    let mut keyed: Vec<(Val, usize, Value)> = Vec::with_capacity(x.list.len());
    for (i, item) in x.list.iter().enumerate() {
        keyed.push((x.per(x.arg(0), item, i)?, i, item.clone()));
    }
    keyed.sort_by(|a, b| compare_keys(&a.0, &b.0, desc).then(a.1.cmp(&b.1)));
    Ok(Some(Value::Array(keyed.into_iter().map(|k| k.2).collect())))
}

fn pipe_group(x: PipeRun) -> Res<Val> {
    let mut groups: IndexMap<String, (Value, Vec<Value>)> = IndexMap::new();
    for (i, item) in x.list.iter().enumerate() {
        let key = x.per(x.arg(0), item, i)?.unwrap_or(Value::Null);
        let k = key_text(&Some(key.clone()));
        groups.entry(k).or_insert_with(|| (key, Vec::new())).1.push(item.clone());
    }
    let out = groups
        .into_values()
        .map(|(key, items)| {
            let count = items.len() as u64;
            js_object([("key".to_string(), key), ("items".to_string(), Value::Array(items)), ("count".to_string(), Value::from(count))])
        })
        .collect();
    Ok(Some(Value::Array(out)))
}

fn pipe_take(x: PipeRun) -> Res<Val> {
    let n = run(x.arg(0), x.env)?;
    x.env.charge(x.list.len())?;
    Ok(match as_num(n.as_ref()) {
        Some(n) => {
            let k = n.max(0.0).trunc() as usize;
            Some(Value::Array(x.list.iter().take(k).cloned().collect()))
        }
        None => nul(),
    })
}

fn pipe_skip(x: PipeRun) -> Res<Val> {
    let n = run(x.arg(0), x.env)?;
    x.env.charge(x.list.len())?;
    Ok(match as_num(n.as_ref()) {
        Some(n) => {
            let k = n.max(0.0).trunc() as usize;
            Some(Value::Array(x.list.iter().skip(k).cloned().collect()))
        }
        None => nul(),
    })
}

fn numbers_per(x: &PipeRun) -> Res<Vec<f64>> {
    let mut out = Vec::new();
    for (i, item) in x.list.iter().enumerate() {
        if let Some(v) = as_num(x.per(x.arg(0), item, i)?.as_ref()) {
            out.push(v);
        }
    }
    Ok(out)
}

fn pipe_pick(x: PipeRun) -> Res<Val> {
    let names: Vec<String> = x.args.iter().filter_map(|a| field_name(&a.expr)).collect();
    let mut out = Vec::with_capacity(x.list.len());
    for item in &x.list {
        x.env.charge(1)?;
        out.push(match item {
            Value::Object(fields) => js_object(
                names.iter().filter(|n| fields.contains_key(n.as_str())).map(|n| (n.clone(), fields.get(n.as_str()).cloned().unwrap_or(Value::Null))),
            ),
            _ => Value::Null,
        });
    }
    Ok(Some(Value::Array(out)))
}

fn pipe_map(x: PipeRun) -> Res<Val> {
    let mut out = Vec::with_capacity(x.list.len());
    for (i, item) in x.list.iter().enumerate() {
        let mut base: Map<String, Value> = match item {
            Value::Object(fields) => fields.clone(),
            other => {
                let mut m = Map::new();
                m.insert("value".to_string(), other.clone());
                m
            }
        };
        for a in x.args {
            let name = a.name.clone().unwrap_or_default();
            let v = x.per(&a.expr, item, i)?.unwrap_or(Value::Null);
            if name == "__proto__" && !base.contains_key(&name) {
                continue;
            }
            base.insert(name, v);
        }
        out.push(js_object(base.into_iter()));
    }
    Ok(Some(Value::Array(out)))
}

fn pipe_distinct(x: PipeRun) -> Res<Val> {
    let mut seen: IndexSet<String> = IndexSet::new();
    let mut out = Vec::new();
    for (i, item) in x.list.iter().enumerate() {
        let key = if x.args.is_empty() { Some(item.clone()) } else { x.per(x.arg(0), item, i)? };
        if seen.insert(key_text(&key)) {
            out.push(item.clone());
        }
    }
    Ok(Some(Value::Array(out)))
}

fn pipe_flatten(x: PipeRun) -> Res<Val> {
    let name = x.args.first().and_then(|a| field_name(&a.expr));
    let mut out: Vec<Value> = Vec::new();
    for item in &x.list {
        let inner = match &name {
            Some(n) => slate_step(Some(item.clone()), &PathSeg::Key(n.clone())),
            None => Some(item.clone()),
        };
        if let Some(Value::Array(items)) = inner {
            x.env.charge(items.len())?;
            out.extend(items);
        }
    }
    Ok(Some(Value::Array(capped(out))))
}

fn pipe_join(x: PipeRun) -> Res<Val> {
    let other_expr = x.arg(0);
    let other = run(other_expr, x.env)?;
    let key = field_name(x.arg(1));
    let other_key = if x.args.len() > 2 { field_name(x.arg(2)) } else { key.clone() };
    let under = match other_expr {
        SlateExpr::Path { head, segs, .. } => segs
            .iter()
            .rev()
            .find_map(|s| match s {
                PathSeg::Key(k) => Some(k.clone()),
                PathSeg::Index(_) => None,
            })
            .unwrap_or_else(|| head.clone()),
        _ => "other".to_string(),
    };
    let pool: Vec<Value> = list_ref(other.as_ref()).cloned().unwrap_or_default();
    x.env.charge(pool.len())?;
    let mut by_key: IndexMap<String, Value> = IndexMap::new();
    if let Some(ok) = &other_key {
        for o in &pool {
            let k = key_text(&Some(slate_step(Some(o.clone()), &PathSeg::Key(ok.clone())).unwrap_or(Value::Null)));
            by_key.entry(k).or_insert_with(|| o.clone());
        }
    }
    let mut out = Vec::with_capacity(x.list.len());
    for item in &x.list {
        x.env.charge(1)?;
        let mine = key.as_ref().and_then(|k| slate_step(Some(item.clone()), &PathSeg::Key(k.clone())));
        let hit = by_key.get(&key_text(&Some(mine.unwrap_or(Value::Null)))).cloned().unwrap_or(Value::Null);
        out.push(match item {
            Value::Object(fields) => {
                let mut entries: Vec<(String, Value)> = fields.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                entries.push((under.clone(), hit));
                js_object(entries)
            }
            other => js_object([("value".to_string(), other.clone()), (under.clone(), hit)]),
        });
    }
    Ok(Some(Value::Array(out)))
}

fn pipe_format(x: PipeRun) -> Res<Val> {
    if let Some(Value::Array(_)) = x.cur {
        let mut out = Vec::with_capacity(x.list.len());
        for (i, item) in x.list.iter().enumerate() {
            out.push(Value::String(text_of(x.per(x.arg(0), item, i)?.as_ref())));
        }
        return Ok(Some(Value::Array(out)));
    }
    let cur = x.cur.clone().unwrap_or(Value::Null);
    let v = x.per(x.arg(0), &cur, 0)?;
    Ok(sval(text_of(v.as_ref())))
}

/// Every pipeline step with its signature, purpose and an example.
pub static SLATE_PIPE_STEPS: &[(&str, SlatePipeStep)] = &[
    ("where", SlatePipeStep { min: 1, max: 1, sig: "where(cond)", purpose: "keeps the elements where cond is true", example: "pr.checks | where(item.state == 'fail')", needs_list: true, out: same_t, apply: |x| {
        let mut out = Vec::new();
        for (i, item) in x.list.iter().enumerate() {
            if truthy(x.per(x.arg(0), item, i)?.as_ref()) {
                out.push(item.clone());
            }
        }
        Ok(Some(Value::Array(out)))
    } }),
    ("sortBy", SlatePipeStep { min: 1, max: 2, sig: "sortBy(expr, 'asc'|'desc')", purpose: "stable sort; nulls last", example: "processes.list | sortBy(item.cpu, 'desc')", needs_list: true, out: same_t, apply: pipe_sort }),
    ("groupBy", SlatePipeStep { min: 1, max: 1, sig: "groupBy(expr)", purpose: "{ key, items, count } per group in first-seen order", example: "pr.checks | groupBy(item.state)", needs_list: true, out: |i, _| list_of(SlateType { t: SlateTypeName::Record, of: None, fields: Some(IndexMap::from([("key".to_string(), any_t()), ("items".to_string(), i.clone()), ("count".to_string(), num_t())])) }), apply: pipe_group }),
    ("take", SlatePipeStep { min: 1, max: 1, sig: "take(n)", purpose: "the first n; n is a literal", example: "pr.checks | take(5)", needs_list: true, out: same_t, apply: pipe_take }),
    ("skip", SlatePipeStep { min: 1, max: 1, sig: "skip(n)", purpose: "all but the first n", example: "pr.checks | skip(1)", needs_list: true, out: same_t, apply: pipe_skip }),
    ("count", SlatePipeStep { min: 0, max: 0, sig: "count", purpose: "the length", example: "$steps | where(item.done) | count", needs_list: true, out: num_out, apply: |x| Ok(Some(Value::from(x.list.len() as u64))) }),
    ("sum", SlatePipeStep { min: 1, max: 1, sig: "sum(expr)", purpose: "the total of expr; nulls skipped", example: "thread.changes.files | sum(item.additions)", needs_list: true, out: num_out, apply: |x| {
        Ok(fin(numbers_per(&x)?.iter().fold(0.0, |acc, v| acc + v)))
    } }),
    ("min", SlatePipeStep { min: 1, max: 1, sig: "min(expr)", purpose: "the least expr", example: "pr.checks | min(num(item.startedAt))", needs_list: true, out: num_out, apply: |x| {
        Ok(numbers_per(&x)?.iter().copied().reduce(f64::min).map_or(nul(), fin))
    } }),
    ("max", SlatePipeStep { min: 1, max: 1, sig: "max(expr)", purpose: "the greatest expr", example: "processes.list | max(item.cpu)", needs_list: true, out: num_out, apply: |x| {
        Ok(numbers_per(&x)?.iter().copied().reduce(f64::max).map_or(nul(), fin))
    } }),
    ("avg", SlatePipeStep { min: 1, max: 1, sig: "avg(expr)", purpose: "the mean of expr", example: "processes.list | avg(item.mem)", needs_list: true, out: num_out, apply: |x| {
        let ns = numbers_per(&x)?;
        if ns.is_empty() {
            return Ok(nul());
        }
        Ok(fin(ns.iter().fold(0.0, |acc, v| acc + v) / ns.len() as f64))
    } }),
    ("first", SlatePipeStep { min: 0, max: 0, sig: "first", purpose: "the first element or null", example: "thread.plan.steps | where(item.state == 'working') | first", needs_list: true, out: elem_out, apply: |x| Ok(Some(x.list.first().cloned().unwrap_or(Value::Null))) }),
    ("last", SlatePipeStep { min: 0, max: 0, sig: "last", purpose: "the last element or null", example: "thread.plan.steps | last", needs_list: true, out: elem_out, apply: |x| Ok(Some(x.list.last().cloned().unwrap_or(Value::Null))) }),
    ("pick", SlatePipeStep { min: 1, max: 99, sig: "pick(field, ...)", purpose: "keeps the named fields", example: "processes.list | pick(name, cpu)", needs_list: true, out: list_rec_out, apply: pipe_pick }),
    ("map", SlatePipeStep { min: 1, max: 99, sig: "map(name: expr, ...)", purpose: "adds or replaces fields", example: "pr.checks | map(took: num(item.completedAt) - num(item.startedAt))", needs_list: true, out: list_rec_out, apply: pipe_map }),
    ("distinct", SlatePipeStep { min: 0, max: 1, sig: "distinct(expr?)", purpose: "drops later repeats", example: "pr.checks | distinct(item.workflow)", needs_list: true, out: same_t, apply: pipe_distinct }),
    ("flatten", SlatePipeStep { min: 0, max: 1, sig: "flatten(field?)", purpose: "joins inner lists", example: "tree.all | flatten(children)", needs_list: true, out: list_out, apply: pipe_flatten }),
    ("join", SlatePipeStep { min: 2, max: 3, sig: "join(other, key, otherKey?)", purpose: "attaches the matching element of other", example: "thread.subagents | join(tree.children, id)", needs_list: true, out: same_t, apply: pipe_join }),
    ("format", SlatePipeStep { min: 1, max: 1, sig: "format(expr)", purpose: "expr as text per element", example: "pr.checks | format(`${item.name}: ${word(item.state)}`)", needs_list: false, out: |i, _| if i.t == SlateTypeName::List { list_of(str_t()) } else { str_t() }, apply: pipe_format }),
];

// ---- evaluation ----

fn root_env(ctx: &SlateEvalContext) -> Env {
    Env {
        sh: Rc::new(Shared { resolve: ctx.resolve.clone(), now: ctx.now, steps: Cell::new(0), visits: Cell::new(0) }),
        item: ctx.item.clone(),
        index: ctx.index,
        row: false,
    }
}

fn in_row(env: &Env) -> bool {
    env.item.is_some() || env.index.is_some()
}

fn read_path(head: &str, own: bool, segs: &[PathSeg], env: &Env) -> Val {
    if !own && (head == "item" || head == "index") {
        if !in_row(env) {
            return None;
        }
        let mut v: Val = if head == "item" {
            env.item.clone()
        } else {
            Some(env.index.map_or(Value::Null, Value::from))
        };
        for s in segs {
            v = slate_step(v, s);
        }
        return v;
    }
    let first_index = segs.iter().position(|s| matches!(s, PathSeg::Index(_))).unwrap_or(segs.len());
    for names in (0..=first_index).rev() {
        if let Some(found) = (env.sh.resolve)(&slate_path_text(head, own, &segs[..names])) {
            let mut out: Val = Some(found);
            for s in &segs[names..] {
                out = slate_step(out, s);
            }
            return out;
        }
    }
    if !own && head == "time" && matches!(segs, [PathSeg::Key(k)] if k == "now") {
        return Some(jnum(env.now()));
    }
    None
}

fn arith(op: SlateBinOp, a: &Val, b: &Val) -> Val {
    let (Some(x), Some(y)) = (as_num(a.as_ref()), as_num(b.as_ref())) else {
        return nul();
    };
    match op {
        SlateBinOp::Add => fin(x + y),
        SlateBinOp::Sub => fin(x - y),
        SlateBinOp::Mul => fin(x * y),
        SlateBinOp::Div if y == 0.0 => nul(),
        SlateBinOp::Div => fin(x / y),
        _ if y == 0.0 => nul(),
        _ => fin(x % y),
    }
}

fn order(op: SlateBinOp, a: &Val, b: &Val) -> Val {
    if missing(a.as_ref()) || missing(b.as_ref()) {
        return nul();
    }
    let ord = match (a, b) {
        (Some(Value::String(x)), Some(Value::String(y))) => utf16::cmp(x, y),
        _ => match (as_num(a.as_ref()), as_num(b.as_ref())) {
            (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
            _ => return nul(),
        },
    };
    Some(Value::Bool(match op {
        SlateBinOp::Lt => ord == Ordering::Less,
        SlateBinOp::Le => ord != Ordering::Greater,
        SlateBinOp::Gt => ord == Ordering::Greater,
        _ => ord != Ordering::Less,
    }))
}

fn run_pipe(head: &SlateExpr, steps: &[SlatePipeStepNode], env: &Env) -> Res<Val> {
    let mut cur = run(head, env)?;
    for step in steps {
        let Some(spec) = pipe_step(&step.name) else {
            continue;
        };
        if spec.needs_list && !is_list(cur.as_ref()) {
            return Ok(nul());
        }
        let list = capped(list_ref(cur.as_ref()).cloned().unwrap_or_default());
        let input = cur.take();
        cur = (spec.apply)(PipeRun { list, cur: input, args: &step.args, env })?;
    }
    Ok(cur)
}

fn run(node: &SlateExpr, env: &Env) -> Res<Val> {
    env.step()?;
    match node {
        SlateExpr::Lit { v, .. } => Ok(Some(v.clone())),
        SlateExpr::Path { head, own, segs, .. } => Ok(read_path(head, *own, segs, env)),
        SlateExpr::Field { of, name, .. } => Ok(slate_step(run(of, env)?, name)),
        SlateExpr::Neg { arg, .. } => {
            let v = run(arg, env)?;
            Ok(as_num(v.as_ref()).map_or(nul(), |x| fin(-x)))
        }
        SlateExpr::Not { arg, .. } => Ok(Some(Value::Bool(!truthy(run(arg, env)?.as_ref())))),
        SlateExpr::Cond { test, then, other, .. } => {
            if truthy(run(test, env)?.as_ref()) {
                run(then, env)
            } else {
                run(other, env)
            }
        }
        SlateExpr::Tpl { parts, .. } => {
            let mut pieces: Vec<(bool, Val)> = Vec::with_capacity(parts.len());
            for p in parts {
                match p {
                    SlateTemplatePart::Text(s) => pieces.push((false, sval(s.clone()))),
                    SlateTemplatePart::Hole { expr, .. } => {
                        let v = run(expr, env)?;
                        pieces.push((!matches!(expr, SlateExpr::Lit { .. }), v));
                    }
                }
            }
            heldtext(Some(joined(&pieces).map_or(Value::Null, Value::String)))
        }
        SlateExpr::Pipe { head, steps, .. } => run_pipe(head, steps, env),
        SlateExpr::List { items, .. } => {
            let mut out = Vec::with_capacity(items.len());
            for e in items {
                out.push(run(e, env)?.unwrap_or(Value::Null));
            }
            Ok(Some(Value::Array(out)))
        }
        SlateExpr::Index { of, index, .. } => {
            let of_v = run(of, env)?;
            let at_v = run(index, env)?;
            Ok(pick_at(of_v.as_ref(), at_v.as_ref()))
        }
        SlateExpr::Rec { fields, .. } => {
            let mut entries = Vec::with_capacity(fields.len());
            for (name, e) in fields {
                entries.push((name.clone(), run(e, env)?.unwrap_or(Value::Null)));
            }
            Ok(Some(js_object(entries)))
        }
        SlateExpr::Bin { op, left, right, .. } => match op {
            SlateBinOp::And => {
                let l = run(left, env)?;
                if truthy(l.as_ref()) {
                    run(right, env)
                } else {
                    Ok(l)
                }
            }
            SlateBinOp::Or => {
                let l = run(left, env)?;
                if truthy(l.as_ref()) {
                    Ok(l)
                } else {
                    run(right, env)
                }
            }
            _ => {
                let l = run(left, env)?;
                let r = run(right, env)?;
                Ok(match op {
                    SlateBinOp::Eq => Some(Value::Bool(slate_equal(
                        Some(l.as_ref().unwrap_or(&Value::Null)),
                        Some(r.as_ref().unwrap_or(&Value::Null)),
                    ))),
                    SlateBinOp::Ne => Some(Value::Bool(!slate_equal(
                        Some(l.as_ref().unwrap_or(&Value::Null)),
                        Some(r.as_ref().unwrap_or(&Value::Null)),
                    ))),
                    SlateBinOp::Lt | SlateBinOp::Le | SlateBinOp::Gt | SlateBinOp::Ge => order(*op, &l, &r),
                    _ => arith(*op, &l, &r),
                })
            }
        },
        SlateExpr::Call { name, args, .. } => {
            let Some(spec) = function_spec(name) else {
                return Ok(nul());
            };
            if args.len() < spec.min || args.len() > spec.max {
                return Ok(nul());
            }
            let mut values: Vec<Val> = Vec::with_capacity(args.len());
            for a in args {
                values.push(run(a, env)?);
            }
            if !spec.null_safe && missing(values.first().and_then(Option::as_ref)) {
                return Ok(nul());
            }
            let out = (spec.body)(&values, env, args)?;
            heldtext(out)
        }
    }
}

/// The value of an expression; null where it fails, undefined where a bare path's data has not arrived. Never throws.
pub fn evaluate_slate_expression(expr: &str, ctx: &SlateEvalContext) -> Option<Value> {
    match parse_slate_expression(expr).ast {
        Some(ast) => evaluate_slate_ast(&ast, ctx),
        None => nul(),
    }
}

/// evaluate_slate_expression for a tree already parsed.
pub fn evaluate_slate_ast(ast: &SlateExpr, ctx: &SlateEvalContext) -> Option<Value> {
    match run(ast, &root_env(ctx)) {
        Ok(v) => v,
        Err(Over) => nul(),
    }
}

fn evaluate_slate_format(src: &str, ctx: &SlateEvalContext) -> Val {
    let pieces: Vec<(bool, Val)> = parse_slate_format(src)
        .parts
        .into_iter()
        .map(|p| match p {
            SlateFormatPart::Text(t) => (false, sval(t)),
            SlateFormatPart::Hole { expr, .. } => (true, evaluate_slate_expression(&expr, ctx)),
        })
        .collect();
    Some(joined(&pieces).map_or(Value::Null, Value::String))
}

fn is_slate_format(v: &Value) -> bool {
    v.as_object().is_some_and(|m| m.len() == 1 && m.get("format").is_some_and(Value::is_string))
}

/// A prop's value: a literal as written, a binding evaluated, a format filled, lists and records through.
pub fn resolve_slate_prop(v: &Value, ctx: &SlateEvalContext) -> Option<Value> {
    match v {
        Value::Object(fields) => {
            if is_slate_binding(v) {
                let bind = fields.get("bind").and_then(Value::as_str).unwrap_or_default();
                return evaluate_slate_expression(bind, ctx);
            }
            if is_slate_format(v) {
                let format = fields.get("format").and_then(Value::as_str).unwrap_or_default();
                return evaluate_slate_format(format, ctx);
            }
            Some(js_object(fields.iter().map(|(k, x)| (k.clone(), resolve_slate_prop(x, ctx).unwrap_or(Value::Null)))))
        }
        Value::Array(items) => Some(Value::Array(items.iter().map(|x| resolve_slate_prop(x, ctx).unwrap_or(Value::Null)).collect())),
        other => Some(other.clone()),
    }
}

// ---- dependencies ----

fn walk(node: &SlateExpr, out: &mut IndexSet<String>) {
    match node {
        SlateExpr::Path { head, own, segs, .. } => {
            if *own || (head != "item" && head != "index") {
                out.insert(slate_path_text(head, *own, segs));
            }
        }
        SlateExpr::Call { name, args, .. } => {
            if function_spec(name).is_some_and(|s| s.clock) {
                out.insert("time.now".to_string());
            }
            for a in args {
                walk(a, out);
            }
        }
        SlateExpr::Lit { .. } => {}
        SlateExpr::Field { of, .. } => walk(of, out),
        SlateExpr::Neg { arg, .. } | SlateExpr::Not { arg, .. } => walk(arg, out),
        SlateExpr::Bin { left, right, .. } => {
            walk(left, out);
            walk(right, out);
        }
        SlateExpr::Cond { test, then, other, .. } => {
            walk(test, out);
            walk(then, out);
            walk(other, out);
        }
        SlateExpr::Tpl { parts, .. } => {
            for p in parts {
                if let SlateTemplatePart::Hole { expr, .. } = p {
                    walk(expr, out);
                }
            }
        }
        SlateExpr::List { items, .. } => {
            for e in items {
                walk(e, out);
            }
        }
        SlateExpr::Index { of, index, .. } => {
            walk(of, out);
            walk(index, out);
        }
        SlateExpr::Rec { fields, .. } => {
            for (_, e) in fields {
                walk(e, out);
            }
        }
        SlateExpr::Pipe { head, steps, .. } => {
            walk(head, out);
            for s in steps {
                for (i, a) in s.args.iter().enumerate() {
                    let bare = s.name == "pick" || (s.name == "join" && i > 0) || s.name == "flatten";
                    if bare && field_name(&a.expr).is_some() {
                        continue;
                    }
                    walk(&a.expr, out);
                }
            }
        }
    }
}

/// Every own and source path a parsed expression reads, in first-read order, without item and index; a call to a
/// function that reads the clock adds time.now.
pub fn slate_ast_dependencies(ast: &SlateExpr) -> Vec<String> {
    let mut out = IndexSet::new();
    walk(ast, &mut out);
    out.into_iter().collect()
}

/// Every own and source path an expression reads (see slate_ast_dependencies); empty when it does not parse.
pub fn slate_dependencies(expr: &str) -> Vec<String> {
    parse_slate_expression(expr).ast.map_or_else(Vec::new, |ast| slate_ast_dependencies(&ast))
}

fn visit_prop(v: &Value, out: &mut IndexSet<String>) {
    match v {
        Value::Object(fields) => {
            if is_slate_binding(v) {
                if let Some(bind) = fields.get("bind").and_then(Value::as_str) {
                    out.extend(slate_dependencies(bind));
                }
            } else if is_slate_format(v) {
                let format = fields.get("format").and_then(Value::as_str).unwrap_or_default();
                for part in parse_slate_format(format).parts {
                    if let SlateFormatPart::Hole { expr, .. } = part {
                        out.extend(slate_dependencies(&expr));
                    }
                }
            } else {
                for x in fields.values() {
                    visit_prop(x, out);
                }
            }
        }
        Value::Array(items) => {
            for x in items {
                visit_prop(x, out);
            }
        }
        _ => {}
    }
}

/// Every path a prop value reads, through its bindings, format holes and nested literals.
pub fn slate_prop_dependencies(value: Option<&Value>) -> Vec<String> {
    let mut out = IndexSet::new();
    if let Some(v) = value {
        visit_prop(v, &mut out);
    }
    out.into_iter().collect()
}

// ---- static checks ----

/// A field of a known type: a record's declared field, or any. None is a field the record does not declare.
fn field_type(of: &SlateType, seg: &PathSeg) -> Option<SlateType> {
    match seg {
        PathSeg::Index(_) => Some(if of.t == SlateTypeName::List { of.of.as_deref().cloned().unwrap_or_else(any_t) } else { any_t() }),
        PathSeg::Key(k) => {
            if of.t == SlateTypeName::Record {
                if let Some(fields) = &of.fields {
                    return fields.get(k).cloned();
                }
            }
            Some(any_t())
        }
    }
}

fn seg_text(seg: &PathSeg) -> String {
    match seg {
        PathSeg::Key(k) => k.clone(),
        PathSeg::Index(i) => i.to_string(),
    }
}

fn field_keys(t: &SlateType) -> Vec<String> {
    t.fields.as_ref().map_or_else(Vec::new, |f| f.keys().cloned().collect())
}

struct Checker<'s> {
    scope: &'s SlateCheckScope,
    base: usize,
    problems: Vec<SlateProblem>,
}

impl Checker<'_> {
    fn add(&mut self, code: &str, message: String, at: usize, fix: Option<String>) {
        self.problems.push(problem(code, &message, Some(at + self.base), fix.as_deref()));
    }

    fn ty(&mut self, node: &SlateExpr, row: Option<&SlateType>) -> SlateType {
        match node {
            SlateExpr::Lit { v, .. } => match v {
                Value::Null => null_t(),
                Value::Number(_) => num_t(),
                Value::String(_) => str_t(),
                Value::Bool(_) => bool_t(),
                _ => any_t(),
            },
            SlateExpr::Tpl { parts, .. } => {
                for p in parts {
                    if let SlateTemplatePart::Hole { expr, .. } = p {
                        self.ty(expr, row);
                    }
                }
                str_t()
            }
            SlateExpr::Index { of, index, .. } => {
                let of_t = self.ty(of, row);
                let at_t = self.ty(index, row);
                if matches!(at_t.t, SlateTypeName::Boolean | SlateTypeName::List | SlateTypeName::Record) {
                    let message = format!("an index is a number or a field name; this gives a {}", at_t.t.name());
                    self.add("X408", message, index.at(), None);
                }
                if of_t.t == SlateTypeName::List {
                    of_t.of.as_deref().cloned().unwrap_or_else(any_t)
                } else {
                    any_t()
                }
            }
            SlateExpr::List { items, .. } => {
                let types: Vec<SlateType> = items.iter().map(|e| self.ty(e, row)).collect();
                if types.is_empty() {
                    return list_t();
                }
                let mut it = types.into_iter();
                let first = it.next().unwrap_or_else(any_t);
                list_of(it.fold(first, merge))
            }
            SlateExpr::Rec { fields, .. } => {
                let mut map: IndexMap<String, SlateType> = IndexMap::new();
                for (name, e) in fields {
                    let t = self.ty(e, row);
                    map.insert(name.clone(), t);
                }
                let ordered: IndexMap<String, SlateType> = js_key_order(map.iter()).into_iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                SlateType { t: SlateTypeName::Record, of: None, fields: Some(ordered) }
            }
            SlateExpr::Path { head, own, segs, at } => {
                if !*own && (head == "item" || head == "index") {
                    let Some(row) = row else {
                        self.add("X409", format!("{head} is only known inside a repeating piece or a step"), *at, None);
                        return any_t();
                    };
                    if head == "index" {
                        return if segs.is_empty() { num_t() } else { any_t() };
                    }
                    let mut t = row.clone();
                    for (i, seg) in segs.iter().enumerate() {
                        match field_type(&t, seg) {
                            None => {
                                let known = field_keys(&t);
                                let fix = slate_nearest(&seg_text(seg), &known);
                                let path: Vec<String> = segs[..=i].iter().map(seg_text).collect();
                                let tail = match &fix {
                                    Some(f) => format!("; did you mean item.{f}?"),
                                    None => format!("; they have {}", known.join(", ")),
                                };
                                let fix_out = fix.as_ref().map(|f| format!("item.{f}"));
                                self.add("X401", format!("item.{} is not a field of this list's rows{tail}", path.join(".")), *at, fix_out);
                                return any_t();
                            }
                            Some(next) => t = next,
                        }
                    }
                    return t;
                }
                match (self.scope.path)(head, *own, segs) {
                    Ok(t) => t,
                    Err(p) => {
                        self.add(p.code, p.message, *at, p.fix);
                        any_t()
                    }
                }
            }
            SlateExpr::Field { of, name, at } => {
                let of_t = self.ty(of, row);
                match field_type(&of_t, name) {
                    Some(t) => t,
                    None => {
                        let fix = slate_nearest(&seg_text(name), &field_keys(&of_t));
                        let tail = fix.as_ref().map_or(String::new(), |f| format!("; did you mean {f}?"));
                        self.add("X401", format!("{} is not a field here{tail}", seg_text(name)), *at, fix);
                        any_t()
                    }
                }
            }
            SlateExpr::Neg { arg, .. } => {
                self.ty(arg, row);
                num_t()
            }
            SlateExpr::Not { arg, .. } => {
                self.ty(arg, row);
                bool_t()
            }
            SlateExpr::Cond { test, then, other, .. } => {
                self.ty(test, row);
                let a = self.ty(then, row);
                let b = self.ty(other, row);
                merge(a, b)
            }
            SlateExpr::Bin { op, left, right, at } => {
                let l = self.ty(left, row);
                let r = self.ty(right, row);
                match op {
                    SlateBinOp::And | SlateBinOp::Or => {
                        if l.t == r.t {
                            l
                        } else {
                            any_t()
                        }
                    }
                    SlateBinOp::Eq | SlateBinOp::Ne => {
                        let mixed = (l.t == SlateTypeName::List && r.t == SlateTypeName::String)
                            || (l.t == SlateTypeName::String && r.t == SlateTypeName::List);
                        if mixed {
                            self.add(
                                "X408",
                                "a list never equals a string; use contains(), len() or a step".into(),
                                *at,
                                Some("contains(list, value), len(list) or list | where(item.field == value)".into()),
                            );
                        }
                        bool_t()
                    }
                    SlateBinOp::Lt | SlateBinOp::Le | SlateBinOp::Gt | SlateBinOp::Ge => bool_t(),
                    _ => {
                        if l.t == SlateTypeName::String || r.t == SlateTypeName::String {
                            self.add(
                                "X408",
                                format!("{} works on numbers; join text with concat() or a template", op.symbol()),
                                *at,
                                Some("concat(a, b) or `${a}${b}`".into()),
                            );
                        }
                        num_t()
                    }
                }
            }
            SlateExpr::Call { name, args, at } => {
                if HANDLER_STEPS.contains(&name.as_str()) && function_spec(name).is_none() {
                    for a in args {
                        self.ty(a, row);
                    }
                    self.add(
                        "X400",
                        format!("{name}(...) is a handler step, which goes in onPress, onSubmit, onChange or do"),
                        *at,
                        None,
                    );
                    return any_t();
                }
                let arg_types: Vec<SlateType> = args.iter().map(|a| self.ty(a, row)).collect();
                let Some(spec) = function_spec(name) else {
                    let fix = match name.as_str() {
                        "length" => Some("len".to_string()),
                        "toUpperCase" => Some("upper".to_string()),
                        other => {
                            let names: Vec<String> = SLATE_FUNCTIONS.iter().map(|(n, _)| n.to_string()).collect();
                            slate_nearest(other, &names)
                        }
                    };
                    let tail = fix.as_ref().map_or(String::new(), |f| format!("; did you mean {f}?"));
                    self.add("X404", format!("{name} is not a function{tail}"), *at, fix);
                    return any_t();
                };
                if args.len() < spec.min || args.len() > spec.max {
                    let want = if spec.min == spec.max {
                        spec.min.to_string()
                    } else if spec.max >= 99 {
                        format!("at least {}", spec.min)
                    } else {
                        format!("{} or {}", spec.min, spec.max)
                    };
                    let plural = if want == "1" { "" } else { "s" };
                    let message = format!("{name} takes {want} argument{plural}, got {}: {}", args.len(), spec.sig);
                    self.add("X405", message, *at, Some(spec.sig.to_string()));
                }
                if let Some(first) = arg_types.first() {
                    let first_at = args[0].at();
                    if spec.number_first && matches!(first.t, SlateTypeName::String | SlateTypeName::List | SlateTypeName::Record | SlateTypeName::Boolean) {
                        let what = if first.t == SlateTypeName::String { "text".to_string() } else { format!("a {}", first.t.name()) };
                        let fix = (first.t == SlateTypeName::String).then(|| format!("{name}(num(...))"));
                        self.add("X406", format!("{name} takes a number; this gives {what}"), first_at, fix);
                    }
                    if spec.list_first && matches!(first.t, SlateTypeName::String | SlateTypeName::Number | SlateTypeName::Boolean) {
                        let what = if first.t == SlateTypeName::String { "text".to_string() } else { format!("a {}", first.t.name()) };
                        self.add("X406", format!("{name} takes a list; this gives {what}"), first_at, None);
                    }
                }
                (spec.returns)(&arg_types)
            }
            SlateExpr::Pipe { head, steps, .. } => {
                let mut cur = self.ty(head, row);
                for step in steps {
                    let Some(spec) = pipe_step(&step.name) else {
                        continue;
                    };
                    if spec.needs_list && matches!(cur.t, SlateTypeName::Number | SlateTypeName::String | SlateTypeName::Boolean | SlateTypeName::Record) {
                        let what = if cur.t == SlateTypeName::String { "text".to_string() } else { format!("a {}", cur.t.name()) };
                        self.add("Q424", format!("{} takes a list; this is {what}", step.name), step.at, None);
                    }
                    let elem = if cur.t == SlateTypeName::List { cur.of.as_deref().cloned().unwrap_or_else(any_t) } else { any_t() };
                    let mut arg_types: Vec<SlateType> = Vec::new();
                    for (i, a) in step.args.iter().enumerate() {
                        let join = step.name == "join";
                        let bare = step.name == "pick" || (join && i > 0) || step.name == "flatten";
                        if bare && field_name(&a.expr).is_some() {
                            arg_types.push(str_t());
                            continue;
                        }
                        if join && i > 0 {
                            self.add(
                                "Q425",
                                "join's keys are field names: join($other, id) or join($other, id, ownerId)".into(),
                                a.expr.at(),
                                None,
                            );
                        }
                        let scope_row = if join && i == 0 { row } else { Some(&elem) };
                        arg_types.push(self.ty(&a.expr, scope_row));
                    }
                    cur = (spec.out)(&cur, &arg_types);
                }
                cur
            }
        }
    }
}

const HANDLER_STEPS: [&str; 11] = ["set", "toggle", "start", "cancel", "send", "steer", "queue", "fill", "open", "copy", "pane"];

/// Problems with an expression at write time: unknown paths, functions, arity, argument and step input types, item
/// outside a row, string arithmetic, a list compared with a string. Positions are offsets into src plus base.
pub fn check_slate_expression(src: &str, scope: &SlateCheckScope, base: usize) -> SlateExprCheck {
    let parsed = parse_slate_expression(src);
    match parsed.ast {
        None => SlateExprCheck {
            ty: any_t(),
            problems: parsed
                .errors
                .into_iter()
                .map(|mut e| {
                    e.at = Some(e.at.unwrap_or(0) + base as i64);
                    e
                })
                .collect(),
        },
        Some(ast) => {
            let mut checker = Checker { scope, base, problems: Vec::new() };
            let ty = checker.ty(&ast, scope.row.as_ref());
            SlateExprCheck { ty, problems: checker.problems }
        }
    }
}

/// A value as a format hole and str() write it.
pub fn slate_text(v: &Value) -> String {
    text_of(Some(v))
}

/// Whether a value is missing, truthy or the rest, as the expression language reads it.
pub fn slate_truthy(v: Option<&Value>) -> bool {
    truthy(v)
}
