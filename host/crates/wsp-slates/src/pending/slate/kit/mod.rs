// SPDX-License-Identifier: AGPL-3.0-only
//! protocol/src/slate/kit.ts at 65f6b89c7: the pieces of kit wsp/2, one entry each, with its props, items, events, its
//! sketch line and its catalog text.
use std::collections::HashSet;
use std::sync::LazyLock;

use indexmap::IndexMap;
use regex::Regex;
use serde_json::{json, Value};
use wsp_wire::text::json_stringify;
use wsp_wire::types::attachments::{IMAGE_MAX_WORDS, IMAGE_TYPE_WORDS};
use wsp_words::js::{js_round, js_trim, js_trim_end, js_trim_start, locale_number, number_string, utf16, Grouping};
use wsp_words::js_date::{time_value, DateZone};
use wsp_words::words::turn::fmt_clock;
use wsp_words::words::units::{fmt_bytes, fmt_cost, fmt_duration, fmt_elapsed, fmt_inr, fmt_tokens};

use super::chart::{slate_axis_word, slate_chart_axis, slate_shares, slate_span_state, slate_time, SLATE_SPAN_STATES};
use super::expr::{slate_dependencies, slate_truthy};
use super::image::{slate_image_source, slate_list_too_long, SlateImageSource, SLATE_IMAGE_LAYOUTS};
use super::paths::slate_own_name;
use super::problems::slate_problem;
use super::shape::{sketch_slate_result, slate_result_shape};
use super::sources::slate_is_series;
use super::{is_slate_binding, SlateProblem, SLATE_LIMITS};

mod figures;
mod pieces;
mod sketch;
mod spec;
mod values;

pub use figures::*;
pub use pieces::*;
use sketch::*;
use spec::*;
use values::*;

pub const SLATE_TONES: &[&str] = &["default", "muted", "good", "warning", "bad", "info", "accent"];
const EMPHASIS: &[&str] = &["normal", "strong", "quiet"];
const SIZE: &[&str] = &["small", "normal", "large"];
const DENSITY: &[&str] = &["tight", "normal", "loose"];
const VARIANT: &[&str] = &["default", "primary", "quiet", "danger"];
const PAD: &[&str] = &["none", "tight", "normal", "loose"];
const SURFACE: &[&str] = &["plain", "inset"];
const PLACE: &[&str] = &["start", "center", "end"];
pub const SLATE_FIGURES: &[&str] = &["plain", "tokens", "bytes", "percent", "usd", "inr", "duration", "integer"];
pub const SLATE_EVENTS: &[&str] = &["press", "submit", "change"];
pub const SLATE_ITEM_KINDS: &[&str] = &["col", "action", "fact", "option", "series"];

pub const SLATE_RESERVED_PROPS: &[(&str, &str)] = &[
    ("color", "tone"),
    ("colour", "tone"),
    ("fill", "tone"),
    ("ink", "tone"),
    ("font", "emphasis, size or mono"),
    ("fontSize", "size"),
    ("fontWeight", "emphasis"),
    ("bold", "emphasis=\"strong\""),
    ("italic", "emphasis"),
    ("padding", "pad"),
    ("margin", "gap or pad"),
    ("spacing", "gap"),
    ("background", "surface=\"inset\" on a section, column or grid"),
    ("bg", "surface=\"inset\""),
    ("border", "nothing; wsp draws edges"),
    ("radius", "nothing; wsp draws edges"),
    ("shadow", "nothing; wsp draws edges"),
    ("outline", "nothing; wsp draws edges"),
    ("width", "nothing; the panel decides"),
    ("height", "nothing; the panel decides"),
    ("style", "nothing"),
    ("className", "nothing"),
    ("class", "nothing"),
    ("css", "nothing"),
    ("html", "nothing"),
    ("glyph", "nothing; say it in words"),
    ("emoji", "nothing; say it in words"),
    ("animation", "nothing"),
    ("transition", "nothing"),
    ("pulse", "nothing"),
    ("blink", "nothing"),
];

/// The reserved prop's meaning prop to use instead, or None when the name is not reserved.
pub fn slate_reserved_prop(name: &str) -> Option<&'static str> {
    SLATE_RESERVED_PROPS.iter().find(|(reserved, _)| *reserved == name).map(|(_, instead)| *instead)
}

/// A prop's type: text is a string or a number; path names a run or value as $name; an enum lists its words.
#[derive(Clone, Debug, PartialEq)]
pub enum SlatePropType {
    String,
    Number,
    Integer,
    Boolean,
    Text,
    List,
    Any,
    Id,
    Path,
    Enum(&'static [&'static str]),
}

/// How a prop may be bound: no for a literal only, yes for a formula or template, state for a bare $value written back,
/// item for a value read per row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlateBinds {
    No,
    Yes,
    State,
    Item,
}

/// A list prop's cap: a list longer than count draws none of it, refused in the noun's words.
#[derive(Clone, Debug, PartialEq)]
pub struct SlateMost {
    pub count: usize,
    pub noun: &'static str,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SlatePropSpec {
    pub prop_type: SlatePropType,
    pub binds: SlateBinds,
    pub required: bool,
    pub default: Option<Value>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    /// Why the person never sees this prop on the panel; a prop without it reaches the sketch.
    pub unseen: Option<&'static str>,
    /// What the catalog's entry for the piece says beside the prop.
    pub about: Option<String>,
    /// A two-way prop that also takes a literal: the state it starts in, which the person then changes.
    pub literal: bool,
    /// A list the piece plots, whose every entry is a number.
    pub of_number: bool,
    pub most: Option<SlateMost>,
    /// A value the piece places on a clock, a time in ms or ISO.
    pub time: bool,
    /// A path prop that names a run, never a value.
    pub names_run: bool,
    /// Free text, whose words may sit beside a mark such as a middle dot, as markdown's do.
    pub marks: bool,
    /// A head, in sentence case, never Title Case.
    pub head: bool,
}

/// What a piece's own check reads and says through: its props, the kind of a name the slate declares, and a problem.
pub struct SlatePieceCheck<'a> {
    pub props: &'a IndexMap<String, Value>,
    pub kind: &'a dyn Fn(&str) -> Option<String>,
    pub problems: Vec<SlateProblem>,
}

impl SlatePieceCheck<'_> {
    pub fn add(&mut self, code: &str, message: &str, prop: Option<&str>, fix: Option<&str>) {
        let mut problem = slate_problem(code, message, fix);
        problem.prop = prop.map(str::to_string);
        self.problems.push(problem);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SlateItemSpec {
    /// The list prop an item of this kind appends to.
    pub prop: &'static str,
    pub fields: IndexMap<&'static str, SlatePropSpec>,
    /// Read in the row scope of the piece's items: a column, a row action.
    pub row: bool,
    /// Its when reads the row, hiding the item on that row alone; every other item's when hides it whole.
    pub row_when: bool,
    pub events: Option<&'static [&'static str]>,
    pub min: Option<usize>,
    pub max: Option<usize>,
}

/// What a piece's sketch line reads its props through.
pub trait SlateSketchView {
    fn id(&self) -> &str;
    /// A prop as the person would see it now; None where its data has not arrived.
    fn prop(&self, name: &str) -> Option<Value>;
    /// The prop as the document holds it.
    fn raw(&self, name: &str) -> Option<Value>;
    fn bound(&self, name: &str) -> bool;
    /// A record's fields resolved against one row.
    fn row(&self, spec: &Value, item: &Value, index: i64) -> IndexMap<String, Value>;
    /// An expression read for truth in a row scope: a row action's when.
    fn test(&self, expr: &str, item: &Value, index: i64) -> bool;
    /// An own path's live value: a run's record for output.
    fn read(&self, path: &str) -> Option<Value>;
    /// What a declared run calls: a command, a tool or a resource.
    fn run_kind(&self, path: &str) -> Option<&'static str>;
    /// A check with no thread: bound props read as their formula in braces.
    fn unbound(&self) -> bool;
    /// The clock the sketch reads time.now by, in ms.
    fn now(&self) -> f64;
}

pub type SlateSketchFn = fn(&dyn SlateSketchView) -> Vec<String>;

#[derive(Clone, Debug, PartialEq)]
pub struct SlateChildLimit {
    pub max: usize,
    pub types: &'static [&'static str],
}

pub struct SlatePieceModule {
    pub kind: &'static str,
    pub level: &'static str,
    pub purpose: &'static str,
    pub holds_children: bool,
    /// At most this many children, of these types.
    pub child_limit: Option<SlateChildLimit>,
    /// The prop a text child fills.
    pub text_prop: Option<&'static str>,
    /// The text child is literal: no holes, braces are characters.
    pub raw_text: bool,
    /// Has items with a row scope.
    pub repeating: bool,
    pub interactive: bool,
    /// The event a piece must handle (A602).
    pub needs_handler: Option<&'static str>,
    pub props: IndexMap<&'static str, SlatePropSpec>,
    pub items: IndexMap<&'static str, SlateItemSpec>,
    pub events: &'static [&'static str],
    pub sketch: SlateSketchFn,
    /// Whether the person sees its children now; a collapsed piece's children are not sketched.
    pub collapsed: Option<fn(&dyn SlateSketchView) -> bool>,
    /// The rules only this piece is held to, beyond its props' types.
    pub check: Option<fn(&mut SlatePieceCheck)>,
    pub fallback: &'static str,
    pub example: &'static str,
}

type Row = IndexMap<String, Value>;
