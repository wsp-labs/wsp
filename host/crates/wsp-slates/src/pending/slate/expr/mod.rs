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

mod check;
mod deps;
mod eval;
mod functions;
mod parse;
mod pipes;
mod tokenize;
mod values;

pub use check::*;
pub use deps::*;
pub use eval::*;
pub use functions::*;
pub use parse::*;
pub use pipes::*;
pub use tokenize::*;
use values::*;

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
#[derive(Debug, PartialEq)]
pub struct SlateType {
    pub t: SlateTypeName,
    pub of: Option<Box<SlateType>>,
    pub fields: Option<IndexMap<String, SlateType>>,
}

/// A type being copied: the copy so far, the members of the original still to copy, and the key it goes under in its parent.
struct Copying<'a> {
    copy: SlateType,
    todo: std::vec::IntoIter<(Option<&'a String>, &'a SlateType)>,
    key: Option<&'a String>,
}

impl<'a> Copying<'a> {
    fn of(src: &'a SlateType, key: Option<&'a String>) -> Copying<'a> {
        let members: Vec<(Option<&String>, &SlateType)> = src
            .of
            .iter()
            .map(|of| (None, &**of))
            .chain(src.fields.iter().flat_map(|fields| fields.iter().map(|(name, field)| (Some(name), field))))
            .collect();
        let copy = SlateType { t: src.t, of: None, fields: src.fields.as_ref().map(|_| IndexMap::new()) };
        Copying { copy, todo: members.into_iter(), key }
    }
}

/// Chained derived values nest a record's type as deep as their values nest, 500 levels, where a derived `clone` is a call
/// per level and dies on a 1 MiB stack at about 380: the copy keeps its open types on a stack of its own.
impl Clone for SlateType {
    fn clone(&self) -> SlateType {
        let mut open = vec![Copying::of(self, None)];
        loop {
            let Some(top) = open.last_mut() else { unreachable!("the root is open until it is returned") };
            if let Some((key, member)) = top.todo.next() {
                open.push(Copying::of(member, key));
                continue;
            }
            let Some(done) = open.pop() else { unreachable!("a type is open") };
            match (open.last_mut(), done.key) {
                (None, _) => return done.copy,
                (Some(parent), None) => parent.copy.of = Some(Box::new(done.copy)),
                (Some(parent), Some(name)) => {
                    parent.copy.fields.get_or_insert_with(IndexMap::new).insert(name.clone(), done.copy);
                }
            }
        }
    }
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

/// How a path is typed: its source, whether it is a `$` path and its steps.
pub type SlatePathType = Box<dyn Fn(&str, bool, &[PathSeg]) -> Result<SlateType, SlateScopeProblem>>;

/// What the checker knows about the world an expression is read in.
pub struct SlateCheckScope {
    /// The type of a path. item and index never reach this.
    pub path: SlatePathType,
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
    resolve: crate::pending::slate::SlateResolve,
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
