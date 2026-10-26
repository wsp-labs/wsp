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

fn spec(prop_type: SlatePropType, binds: SlateBinds) -> SlatePropSpec {
    SlatePropSpec {
        prop_type,
        binds,
        required: false,
        default: None,
        min: None,
        max: None,
        unseen: None,
        about: None,
        literal: false,
        of_number: false,
        most: None,
        time: false,
        names_run: false,
        marks: false,
        head: false,
    }
}

fn str_prop() -> SlatePropSpec {
    spec(SlatePropType::String, SlateBinds::Yes)
}

fn num_prop() -> SlatePropSpec {
    spec(SlatePropType::Number, SlateBinds::Yes)
}

fn flag() -> SlatePropSpec {
    spec(SlatePropType::Boolean, SlateBinds::No)
}

fn enm(values: &'static [&'static str], fallback: Option<&str>) -> SlatePropSpec {
    SlatePropSpec { default: fallback.map(|f| json!(f)), ..spec(SlatePropType::Enum(values), SlateBinds::No) }
}

fn tone(values: &'static [&'static str]) -> SlatePropSpec {
    spec(SlatePropType::Enum(values), SlateBinds::Yes)
}

fn req(prop: SlatePropSpec) -> SlatePropSpec {
    SlatePropSpec { required: true, ..prop }
}

fn row_key() -> SlatePropSpec {
    SlatePropSpec { unseen: Some("names a row so it keeps its place as rows move"), ..spec(SlatePropType::Any, SlateBinds::Item) }
}

fn props<const N: usize>(entries: [(&'static str, SlatePropSpec); N]) -> IndexMap<&'static str, SlatePropSpec> {
    entries.into_iter().collect()
}

/// How a group sits: inner space and the app's inset ground.
fn with_box(mut entries: IndexMap<&'static str, SlatePropSpec>) -> IndexMap<&'static str, SlatePropSpec> {
    entries.insert("pad", enm(PAD, Some("none")));
    entries.insert("surface", enm(SURFACE, Some("plain")));
    entries
}

fn row_action_item() -> SlateItemSpec {
    SlateItemSpec {
        prop: "rowActions",
        fields: props([("label", SlatePropSpec { binds: SlateBinds::Item, required: true, ..str_prop() })]),
        row: true,
        row_when: true,
        events: Some(&["press"]),
        min: None,
        max: Some(SLATE_LIMITS.row_actions),
    }
}

fn piece(
    kind: &'static str,
    purpose: &'static str,
    fallback: &'static str,
    example: &'static str,
    sketch: SlateSketchFn,
) -> SlatePieceModule {
    SlatePieceModule {
        kind,
        level: "core",
        purpose,
        holds_children: false,
        child_limit: None,
        text_prop: None,
        raw_text: false,
        repeating: false,
        interactive: false,
        needs_handler: None,
        props: IndexMap::new(),
        items: IndexMap::new(),
        events: &[],
        sketch,
        collapsed: None,
        check: None,
        fallback,
        example,
    }
}

/// A value as a sketch shows it: nothing for missing.
fn shown(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(text)) => text.clone(),
        Some(Value::Bool(flag)) => flag.to_string(),
        Some(Value::Number(n)) => number_string(n.as_f64().unwrap_or(f64::NAN)),
        Some(other) => json_stringify(other),
    }
}

/// `String(v)` for a value a template writes as it is.
fn js_string(v: &Value) -> String {
    match v {
        Value::Null => "null".to_string(),
        other => shown(Some(other)),
    }
}

fn word_of(v: Option<Value>, default: &str) -> String {
    match v {
        None | Some(Value::Null) => default.to_string(),
        Some(Value::String(text)) => text,
        Some(other) => js_string(&other),
    }
}

/// A held prop as the window reads it: a sentence holds the piece, and anything else (null, false, blank) holds nothing.
pub fn slate_held_text(v: Option<&Value>) -> Option<String> {
    match v {
        Some(Value::String(text)) if !js_trim(text).is_empty() => Some(text.clone()),
        _ => None,
    }
}

fn num_of(v: Option<&Value>) -> Option<f64> {
    match v {
        Some(Value::Number(n)) => n.as_f64().filter(|x| x.is_finite()),
        _ => None,
    }
}

fn numbers(v: Option<&Value>) -> Vec<f64> {
    match v {
        Some(Value::Array(items)) => items.iter().filter_map(|x| num_of(Some(x))).collect(),
        _ => Vec::new(),
    }
}

fn list_of(v: Option<&Value>) -> Vec<Value> {
    match v {
        Some(Value::Array(items)) => items.clone(),
        _ => Vec::new(),
    }
}

fn rec_of(v: Option<&Value>) -> Row {
    match v {
        Some(Value::Object(fields)) => fields.iter().map(|(k, x)| (k.clone(), x.clone())).collect(),
        _ => Row::new(),
    }
}

fn binding_of(v: Option<&Value>) -> Option<&str> {
    let v = v?;
    if is_slate_binding(v) {
        v.get("bind")?.as_str()
    } else {
        None
    }
}

/// A row record built from its entries; a missing entry is null, as the sketch reads it.
fn spec_of(entries: Vec<(&str, Option<Value>)>) -> Value {
    Value::Object(entries.into_iter().map(|(k, v)| (k.to_string(), v.unwrap_or(Value::Null))).collect())
}

fn pick(r: &Row, keys: &[&str]) -> Row {
    keys.iter().filter_map(|k| r.get(*k).map(|v| (k.to_string(), v.clone()))).collect()
}

/// A row's or an item's look as a mark after it: [bad strong mono], nothing when it has none.
fn marks(r: &Row) -> String {
    let text = |key: &str| r.get(key).and_then(Value::as_str);
    let mut words: Vec<&str> = [
        text("tone").filter(|t| *t != "default"),
        text("emphasis").filter(|e| *e != "normal"),
        (r.get("mono") == Some(&Value::Bool(true))).then_some("mono"),
        text("align").filter(|a| *a != "start"),
        text("width"),
    ]
    .into_iter()
    .flatten()
    .collect();
    words.retain(|w| !w.is_empty());
    if words.is_empty() {
        String::new()
    } else {
        format!(" [{}]", words.join(" "))
    }
}

fn bar(value: Option<&Value>, max: Option<&Value>) -> String {
    let Some(value) = num_of(value) else {
        return "[..........]".to_string();
    };
    let share = match num_of(max) {
        Some(max) if max > 0.0 => value / max,
        _ => value / 100.0,
    };
    let cells = js_round(share * 10.0).clamp(0.0, 10.0) as usize;
    format!("[{}{}]", "#".repeat(cells), ".".repeat(10 - cells))
}

fn plain_number(n: f64) -> String {
    locale_number(n, Grouping::EnUs, 0, 2)
}

/// A number in the words a piece's format names, the one rule the panel and the sketch both draw it by: a percent of
/// max where there is one, a fraction over max (the number alone with none), a duration from milliseconds, an integer
/// with its thousands grouped, none as nothing, and any other format the number itself to two places.
pub fn slate_figure(value: f64, format: &str, max: Option<f64>) -> String {
    match format {
        "percent" => {
            let share = match max {
                Some(max) if max > 0.0 => value / max * 100.0,
                _ => value,
            };
            format!("{}%", number_string(js_round(share)))
        }
        "fraction" => match max {
            None => plain_number(value),
            Some(max) => format!("{}/{}", plain_number(value), plain_number(max)),
        },
        "tokens" => fmt_tokens(value),
        "bytes" => fmt_bytes(value),
        "usd" => fmt_cost(value),
        "inr" => fmt_inr(value),
        "duration" => fmt_duration(value, None),
        "integer" => locale_number(js_round(value), Grouping::EnUs, 0, 0),
        "none" => String::new(),
        _ => plain_number(value),
    }
}

fn figure(format: &str, value: Option<&Value>, max: Option<f64>) -> String {
    match num_of(value) {
        Some(n) => slate_figure(n, format, max),
        None => shown(value),
    }
}

fn join_with(separator: &str, parts: &[String]) -> String {
    parts.iter().filter(|p| !p.is_empty()).map(String::as_str).collect::<Vec<_>>().join(separator)
}

fn join2(parts: &[String]) -> String {
    join_with("  ", parts)
}

fn js_min(vals: &[f64]) -> f64 {
    vals.iter().copied().fold(f64::INFINITY, f64::min)
}

fn js_max(vals: &[f64]) -> f64 {
    vals.iter().copied().fold(f64::NEG_INFINITY, f64::max)
}

fn trend(label: &str, vals: &[f64]) -> String {
    match vals.last() {
        None => format!("{label}  no points yet"),
        Some(last) => js_trim_start(&format!(
            "{label}  last {}, min {}, max {} over {} points",
            number_string(*last),
            number_string(js_min(vals)),
            number_string(js_max(vals)),
            vals.len()
        ))
        .to_string(),
    }
}

fn or_default(v: Option<Value>, default: Value) -> Value {
    match v {
        None | Some(Value::Null) => default,
        Some(x) => x,
    }
}

fn is_secret_handle(v: Option<&Value>) -> bool {
    let Some(Value::Object(fields)) = v else {
        return false;
    };
    fields.get("secret") == Some(&Value::Bool(true))
        && matches!(fields.get("set"), Some(Value::Bool(_)))
        && matches!(fields.get("len"), Some(Value::Number(_)))
        && fields.keys().all(|k| matches!(k.as_str(), "secret" | "set" | "len" | "at"))
}

fn clock_of(at: f64) -> String {
    let zone = DateZone::system();
    fmt_clock(Some(at), &|ms: f64| time_value(ms).map_or(0, |t| zone.offset_seconds(t)))
}

/// The items of a list prop whose when holds; a check with no thread keeps them all.
fn present(v: &dyn SlateSketchView, list: Vec<Value>) -> Vec<Value> {
    list.into_iter()
        .enumerate()
        .filter(|(i, x)| match rec_of(Some(x)).get("when") {
            Some(Value::String(when)) => v.test(when, &Value::Null, *i as i64),
            _ => true,
        })
        .map(|(_, x)| x)
        .collect()
}

/// Rows of a repeating piece, cut to the sketch's row cap, with the rest counted.
fn rows(v: &dyn SlateSketchView, mut line: impl FnMut(&Value, i64) -> String, empty: &str) -> Vec<String> {
    let items = v.prop("items");
    if v.unbound() {
        return vec![format!("rows of {}", shown(items.as_ref()))];
    }
    let items = match items {
        Some(Value::Array(items)) if !items.is_empty() => items,
        _ => {
            let text = shown(v.prop("empty").as_ref());
            return vec![if text.is_empty() { empty.to_string() } else { text }];
        }
    };
    let cut = items.len().min(SLATE_LIMITS.sketch_rows);
    let mut out: Vec<String> = items[..cut].iter().enumerate().map(|(i, item)| line(item, i as i64)).collect();
    if items.len() > cut {
        out.push(format!("and {} more rows; slate_read values with the list's path reads them all", items.len() - cut));
    }
    out
}

fn actions(v: &dyn SlateSketchView, item: &Value, index: i64) -> String {
    list_of(v.raw("rowActions").as_ref())
        .into_iter()
        .filter(|a| match rec_of(Some(a)).get("when") {
            Some(Value::String(when)) => v.test(when, item, index),
            _ => true,
        })
        .map(|a| format!(" [{}]", shown(v.row(&a, item, index).get("label"))))
        .collect()
}

fn reads_time(expr: &str) -> bool {
    static TIME_FNS: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?-u:\b)(?:date|time|ago|weekday)\s*\(").expect("time function pattern"));
    static ITEM_FIELD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?-u:\b)item\.([A-Za-z_]+)(?-u:\b)").expect("item field pattern"));
    const TIME_FIELDS: &[&str] = &["at", "time", "date", "ts", "timestamp", "when", "day", "hour", "minute"];
    if TIME_FNS.is_match(expr) || slate_dependencies(expr).iter().any(|dep| dep == "time.now") {
        return true;
    }
    ITEM_FIELD.captures(expr).is_some_and(|c| TIME_FIELDS.contains(&&c[1]))
}

fn sketch_empty(_: &dyn SlateSketchView) -> Vec<String> {
    vec![String::new()]
}

fn sketch_chart(v: &dyn SlateSketchView) -> Vec<String> {
    let label = shown(v.prop("label").as_ref());
    let items = v.prop("items");
    let series = present(v, list_of(v.raw("series").as_ref()));
    let stack = !series.is_empty() && v.prop("stack") == Some(Value::Bool(true));
    let lines =
        if series.is_empty() { "line".to_string() } else { format!("{}{} lines", series.len(), if stack { " stacked" } else { "" }) };
    if v.unbound() {
        let x = if v.raw("x").is_none() { "{index}".to_string() } else { shown(v.prop("x").as_ref()) };
        return vec![format!("{label}  {lines} of {}, x {x}", shown(items.as_ref()))];
    }
    let x = v.raw("x");
    let list = list_of(items.as_ref());
    let format = word_of(v.prop("format"), "plain");
    let unit = shown(v.prop("unit").as_ref());
    let fig = |n: f64| join_with(" ", &[slate_figure(n, &format, None), unit.clone()]);
    let xs: Vec<Option<Value>> = list
        .iter()
        .enumerate()
        .map(|(i, item)| match &x {
            None => Some(json!(i)),
            Some(x) => v.row(&spec_of(vec![("x", Some(x.clone()))]), item, i as i64).get("x").cloned(),
        })
        .collect();
    let axis_line = |all: &[f64], at: &[usize]| -> String {
        let pair;
        let axis_input: &[f64] = if all.len() == 1 {
            pair = [all[0], all[0]];
            &pair
        } else {
            all
        };
        let axis = slate_chart_axis(axis_input);
        format!(
            "  x {} to {}, y {} to {}",
            slate_axis_word(xs[at[0]].as_ref()),
            slate_axis_word(xs[at[at.len() - 1]].as_ref()),
            fig(axis.from),
            fig(axis.to)
        )
    };
    let value_spec = || spec_of(vec![("value", v.raw("value"))]);
    if series.is_empty() {
        if v.raw("value").is_none() {
            let span = if list.is_empty() {
                String::new()
            } else {
                format!(", x {} to {}", slate_axis_word(xs[0].as_ref()), slate_axis_word(xs.last().and_then(Option::as_ref)))
            };
            return vec![js_trim_start(&format!(
                "{label}  nothing plotted from {} rows{span}; give value or a <series> per line",
                list.len()
            ))
            .to_string()];
        }
        let at: Vec<usize> = list
            .iter()
            .enumerate()
            .filter(|(i, item)| num_of(v.row(&value_spec(), item, *i as i64).get("value")).is_some())
            .map(|(i, _)| i)
            .collect();
        if at.is_empty() {
            return vec![format!("{label}  no points yet")];
        }
        let vals: Vec<f64> = at.iter().map(|&i| num_of(v.row(&value_spec(), &list[i], i as i64).get("value")).unwrap_or(0.0)).collect();
        return vec![
            js_trim_start(&format!(
                "{label}  last {}, min {}, max {} over {} point{}",
                fig(vals[vals.len() - 1]),
                fig(js_min(&vals)),
                fig(js_max(&vals)),
                vals.len(),
                if vals.len() == 1 { "" } else { "s" }
            ))
            .to_string(),
            axis_line(&vals, &at),
        ];
    }
    let read: Vec<Vec<Row>> = series.iter().map(|s| list.iter().enumerate().map(|(i, item)| v.row(s, item, i as i64)).collect()).collect();
    let own: Vec<Vec<f64>> = read.iter().map(|rows| rows.iter().map(|r| num_of(r.get("value")).unwrap_or(0.0)).collect()).collect();
    let at: Vec<usize> = (0..list.len()).filter(|&i| read.iter().any(|rows| num_of(rows[i].get("value")).is_some())).collect();
    if at.is_empty() {
        return vec![format!("{label}  {lines}, no points yet")];
    }
    let drawn: Vec<Vec<f64>> = if stack {
        (0..own.len()).map(|k| (0..own[0].len()).map(|i| own[..=k].iter().fold(0.0, |sum, line| sum + line[i])).collect()).collect()
    } else {
        own.clone()
    };
    let all: Vec<f64> = at.iter().flat_map(|&i| drawn.iter().map(move |line| line[i])).collect();
    let mut out =
        vec![js_trim_start(&format!("{label}  {lines} over {} point{}", at.len(), if at.len() == 1 { "" } else { "s" })).to_string()];
    for (k, rows) in read.iter().enumerate() {
        let vals: Vec<f64> = at.iter().filter_map(|&i| num_of(rows[i].get("value"))).collect();
        let name = rows
            .first()
            .and_then(|r| r.get("label"))
            .filter(|l| !l.is_null())
            .cloned()
            .or_else(|| rec_of(Some(&series[k])).get("label").cloned());
        let said = if vals.is_empty() {
            "no points yet".to_string()
        } else {
            format!("last {}, min {}, max {}", fig(vals[vals.len() - 1]), fig(js_min(&vals)), fig(js_max(&vals)))
        };
        let tone_only = pick(&rec_of(Some(&series[k])), &["tone"]);
        out.push(format!("  {}  {said}{}", shown(name.as_ref()), marks(&tone_only)));
    }
    let axis_all = if stack {
        let mut with_zero = vec![0.0];
        with_zero.extend(all);
        with_zero
    } else {
        all
    };
    out.push(axis_line(&axis_all, &at));
    out
}

fn sketch_timeline(v: &dyn SlateSketchView) -> Vec<String> {
    let given = v.prop("now");
    let now = slate_time(given.as_ref()).unwrap_or_else(|| v.now());
    let not_time = |word: &str, value: Option<&Value>| -> String {
        match value {
            None | Some(Value::Null) => String::new(),
            Some(x) if slate_time(Some(x)).is_some() => String::new(),
            Some(x) => format!("{word} {} is not a time", json_stringify(x)),
        }
    };
    if v.unbound() {
        return vec![format!("{}  spans of {}", shown(v.prop("label").as_ref()), shown(v.prop("items").as_ref()))];
    }
    let mut group: Option<String> = None;
    let mut out = vec![join2(&[shown(v.prop("label").as_ref()), not_time("now", given.as_ref())])];
    let body = rows(
        v,
        |item, i| {
            let r = v.row(
                &spec_of(vec![
                    ("name", v.raw("name")),
                    ("start", v.raw("start")),
                    ("end", v.raw("end")),
                    ("state", v.raw("state")),
                    ("group", v.raw("group")),
                ]),
                item,
                i,
            );
            let start = slate_time(r.get("start"));
            let end = slate_time(r.get("end"));
            let state = slate_span_state(r.get("state"), start, end);
            let head = match r.get("group") {
                Some(Value::String(g)) if group.as_deref() != Some(g.as_str()) => {
                    group = Some(g.clone());
                    format!("  {g}\n")
                }
                _ => String::new(),
            };
            let clock = match start {
                None => String::new(),
                Some(start) => {
                    let until = if state == "running" {
                        "now".to_string()
                    } else {
                        match end {
                            None => "?".to_string(),
                            Some(end) => clock_of(end),
                        }
                    };
                    format!("{} to {until}", clock_of(start))
                }
            };
            let said = if state == "done" {
                match (start, end) {
                    (Some(start), Some(end)) => fmt_elapsed(end - start),
                    _ => "done".to_string(),
                }
            } else if state == "running" && start.is_some() {
                format!("running {}", fmt_elapsed(now - start.unwrap_or(now)))
            } else {
                state.to_string()
            };
            format!(
                "{head}  {}  {}",
                shown(r.get("name")),
                join2(&[clock, said, not_time("start", r.get("start")), not_time("end", r.get("end"))])
            )
        },
        "Nothing here",
    );
    out.extend(body.iter().flat_map(|line| line.split('\n').map(str::to_string)));
    out
}

fn sketch_treemap(v: &dyn SlateSketchView) -> Vec<String> {
    let label = shown(v.prop("label").as_ref());
    let items = v.prop("items");
    if v.unbound() {
        return vec![format!("{label}  boxes of {}", shown(items.as_ref()))];
    }
    let format = word_of(v.prop("format"), "plain");
    let parts: Vec<Row> = list_of(items.as_ref())
        .iter()
        .enumerate()
        .map(|(i, item)| {
            v.row(&spec_of(vec![("name", v.raw("name")), ("value", v.raw("value")), ("group", v.raw("group"))]), item, i as i64)
        })
        .collect();
    let whole: f64 = parts.iter().map(|p| num_of(p.get("value")).map_or(0.0, |x| x.max(0.0))).sum();
    let groups: HashSet<String> = parts.iter().map(|p| shown(p.get("group"))).collect();
    let summary = if parts.is_empty() {
        String::new()
    } else {
        format!(
            "{} in {} parts{}",
            slate_figure(whole, &format, None),
            parts.len(),
            if groups.len() > 1 { format!(", {} groups", groups.len()) } else { String::new() }
        )
    };
    let mut out = vec![join2(&[label, summary])];
    out.extend(rows(
        v,
        |_, i| {
            let p = &parts[i as usize];
            let share = match num_of(p.get("value")) {
                Some(value) if whole > 0.0 => format!(" {}%", js_round(value.max(0.0) / whole * 100.0)),
                _ => String::new(),
            };
            format!("  {}  {}{share}", join2(&[shown(p.get("group")), shown(p.get("name"))]), figure(&format, p.get("value"), None))
        },
        "Nothing here",
    ));
    out
}

fn sketch_donut(v: &dyn SlateSketchView) -> Vec<String> {
    let label = shown(v.prop("label").as_ref());
    let items = v.prop("items");
    if v.unbound() {
        return vec![format!("{label}  shares of {}", shown(items.as_ref()))];
    }
    let format = word_of(v.prop("format"), "plain");
    let unit = shown(v.prop("unit").as_ref());
    let given: Vec<(String, f64)> = list_of(items.as_ref())
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let r = v.row(&spec_of(vec![("name", v.raw("name")), ("value", v.raw("value"))]), item, i as i64);
            (shown(r.get("name")), num_of(r.get("value")).unwrap_or(0.0))
        })
        .collect();
    let refs: Vec<(&str, f64)> = given.iter().map(|(name, value)| (name.as_str(), *value)).collect();
    let parts = slate_shares(&refs);
    let whole: f64 = parts.iter().map(|p| p.value).sum();
    if whole == 0.0 {
        return vec![format!("{label}  no shares yet")];
    }
    let mut out = vec![join2(&[label, join_with(" ", &[slate_figure(whole, &format, None), unit])])];
    out.extend(
        parts.iter().map(|p| format!("  {}  {} {}%", p.name, slate_figure(p.value, &format, None), js_round(p.value / whole * 100.0))),
    );
    out
}

fn sketch_table(v: &dyn SlateSketchView) -> Vec<String> {
    let cols = present(v, list_of(v.raw("columns").as_ref()));
    let heads: Vec<String> = cols
        .iter()
        .map(|c| {
            let r = rec_of(Some(c));
            format!("{}{}", shown(r.get("title")), marks(&pick(&r, &["mono", "align", "width"])))
        })
        .collect();
    let mut out = vec![format!("| {} |", heads.join(" | "))];
    out.extend(rows(
        v,
        |item, i| {
            let cells: Vec<String> = cols
                .iter()
                .map(|c| {
                    let r = v.row(c, item, i);
                    format!("{}{}", shown(r.get("value")), marks(&pick(&r, &["tone", "emphasis"])))
                })
                .collect();
            format!("| {} |{}", cells.join(" | "), actions(v, item, i))
        },
        "Nothing here",
    ));
    out
}

fn sketch_rows_text(v: &dyn SlateSketchView) -> Vec<String> {
    rows(
        v,
        |item, i| {
            let r = v.row(&spec_of(vec![("title", v.raw("title")), ("done", v.raw("done"))]), item, i);
            let note = shown(v.row(&spec_of(vec![("note", v.raw("note"))]), item, i).get("note"));
            let tick = if r.get("done") == Some(&Value::Bool(true)) { "x" } else { " " };
            let tail = if note.is_empty() { String::new() } else { format!("  {note}") };
            format!("[{tick}] {}{tail}", shown(r.get("title")))
        },
        "Nothing here",
    )
}

fn sketch_facts(v: &dyn SlateSketchView) -> Vec<String> {
    let all: Vec<Row> =
        present(v, list_of(v.raw("facts").as_ref())).iter().enumerate().map(|(i, f)| v.row(f, &Value::Null, i as i64)).collect();
    let empty = |f: &Row| matches!(f.get("value"), None | Some(Value::Null)) || f.get("value") == Some(&json!(""));
    let left: Vec<String> = all.iter().filter(|f| empty(f)).map(|f| shown(f.get("label"))).collect();
    let mut parts: Vec<String> =
        all.iter().filter(|f| !empty(f)).map(|f| format!("{}: {}{}", shown(f.get("label")), shown(f.get("value")), marks(f))).collect();
    if !left.is_empty() && !v.unbound() {
        parts.push(format!("(no value yet, not shown: {})", left.join(", ")));
    }
    vec![parts.join("  ")]
}

fn sketch_output(v: &dyn SlateSketchView) -> Vec<String> {
    let name = shown(v.raw("run").as_ref());
    let r = rec_of(v.read(&name).as_ref());
    let Some(state) = r.get("state").map(js_string) else {
        return vec![format!("output {name}: not run yet")];
    };
    if v.unbound() || state == "idle" {
        return vec![format!("output {name}: not run yet")];
    }
    let lines: Vec<String> = match r.get("lines") {
        Some(Value::Array(lines)) if !lines.is_empty() => lines.iter().map(|l| js_string(l)).collect(),
        _ => [r.get("out"), r.get("err")]
            .into_iter()
            .filter_map(|s| s.and_then(Value::as_str))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
            .split('\n')
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect(),
    };
    if let Some(kind) = v.run_kind(&name) {
        if kind != "cmd" && state != "failed" {
            let result = match r.get("json") {
                None | Some(Value::Null) => r.get("out"),
                value => value,
            };
            let shape = sketch_slate_result(&slate_result_shape(result));
            return match shape.first() {
                None => vec![format!("output {name}: {state}")],
                Some(first) => {
                    let mut out = vec![format!("output {name}: {state}, {first}")];
                    out.extend(shape.iter().skip(1).take(3).map(|l| format!("  {l}")));
                    out
                }
            };
        }
    }
    let why = r.get("why").map(|w| format!(" ({})", js_string(w))).unwrap_or_default();
    let mut out = vec![format!("output {name}: {state}, {} line{}{why}", lines.len(), if lines.len() == 1 { "" } else { "s" })];
    out.extend(lines[lines.len().saturating_sub(3)..].iter().map(|l| format!("  {l}")));
    out
}

fn sketch_input(v: &dyn SlateSketchView) -> Vec<String> {
    let raw = v.prop("value");
    let label = shown(v.prop("label").as_ref());
    if is_secret_handle(raw.as_ref()) {
        let set = raw.as_ref().and_then(|r| r.get("set")).is_some_and(|set| slate_truthy(Some(set)));
        return vec![if set { format!("{label}: ••••") } else { format!("{label}: (empty)") }];
    }
    let value = shown(raw.as_ref());
    if value.is_empty() {
        let placeholder = v.prop("placeholder");
        let hint = if slate_truthy(placeholder.as_ref()) { format!(", \"{}\"", shown(placeholder.as_ref())) } else { String::new() };
        return vec![format!("{label}: (empty{hint})")];
    }
    let clipped = if utf16::len(&value) > 60 { format!("{}…", utf16::cut(&value, 59)) } else { value };
    vec![format!("{label}: \"{clipped}\"")]
}

fn sketch_select(v: &dyn SlateSketchView) -> Vec<String> {
    let options = v.prop("options");
    let list: Vec<String> = present(v, list_of(options.as_ref()))
        .iter()
        .map(|o| match o {
            Value::Object(_) | Value::Array(_) => {
                let r = rec_of(Some(o));
                let chosen = r.get("label").filter(|x| !x.is_null()).or_else(|| r.get("value").filter(|x| !x.is_null()));
                shown(chosen)
            }
            other => shown(Some(other)),
        })
        .collect();
    let placeholder = shown(v.prop("placeholder").as_ref());
    let empty = if placeholder.is_empty() { "(none)".to_string() } else { format!("(none, \"{placeholder}\")") };
    let value = shown(v.prop("value").as_ref());
    let value = if value.is_empty() { empty } else { value };
    let of = if list.len() <= 6 {
        let joined = list.join(", ");
        if joined.is_empty() {
            "0".to_string()
        } else {
            joined
        }
    } else {
        list.len().to_string()
    };
    vec![format!("{}: {value} (of {of})", shown(v.prop("label").as_ref()))]
}

fn sketch_choices(v: &dyn SlateSketchView) -> Vec<String> {
    let picked = v.prop("value");
    let answer = v.prop("answer");
    let options = v.prop("options");
    let json_eq = |a: Option<&Value>, b: &Value| a.is_some_and(|a| json_stringify(a) == json_stringify(b));
    let present_picked = picked.as_ref().filter(|p| !p.is_null());
    let present_answer = answer.as_ref().filter(|a| !a.is_null());
    let list: Vec<Row> = present(v, list_of(options.as_ref()))
        .into_iter()
        .map(|o| match o {
            Value::Object(_) => rec_of(Some(&o)),
            other => Row::from([("value".to_string(), other)]),
        })
        .collect();
    let lines: Vec<String> = list
        .iter()
        .map(|o| {
            let chosen = present_picked.is_some_and(|p| json_eq(o.get("value"), p));
            let right = match (present_picked, present_answer) {
                (Some(_), Some(answer)) => json_eq(o.get("value"), answer),
                _ => false,
            };
            let mark = if right {
                " right"
            } else if chosen && present_answer.is_some() {
                " wrong"
            } else {
                ""
            };
            let note = o.get("note").filter(|n| !n.is_null()).map(|n| format!("  {}", shown(Some(n)))).unwrap_or_default();
            let label = o.get("label").filter(|l| !l.is_null()).or_else(|| o.get("value"));
            format!("({}) {}{mark}{note}", if chosen { "x" } else { " " }, shown(label))
        })
        .collect();
    let lost = present_picked.is_some_and(|p| !list.iter().any(|o| json_eq(o.get("value"), p)));
    let head = format!("{}:{}", shown(v.prop("label").as_ref()), if lost { format!(" {}", shown(picked.as_ref())) } else { String::new() });
    let mut out = vec![head];
    out.extend(lines.iter().map(|m| format!("  {m}")));
    out
}

fn sketch_text(v: &dyn SlateSketchView) -> Vec<String> {
    let text = shown(v.prop("value").as_ref());
    vec![if text.is_empty() { shown(v.prop("placeholder").as_ref()) } else { text }]
}

/// Every piece's entry, in the order the catalog lists them.
pub static SLATE_PIECES: LazyLock<IndexMap<&'static str, SlatePieceModule>> = LazyLock::new(|| {
    let mut pieces = IndexMap::new();

    pieces.insert(
        "column",
        SlatePieceModule {
            holds_children: true,
            props: with_box(props([
                ("gap", enm(DENSITY, Some("normal"))),
                ("align", enm(&["start", "center", "end", "stretch"], Some("stretch"))),
            ])),
            ..piece(
                "column",
                "Stacks children top to bottom.",
                "children in order",
                r#"<column gap="tight"><text>One</text><text>Two</text></column>"#,
                sketch_empty,
            )
        },
    );
    pieces.insert(
        "grid",
        SlatePieceModule {
            holds_children: true,
            props: with_box(props([
                ("columns", SlatePropSpec { min: Some(2.0), max: Some(4.0), required: true, ..spec(SlatePropType::Integer, SlateBinds::No) }),
                ("gap", enm(DENSITY, Some("normal"))),
                ("align", enm(PLACE, None)),
            ])),
            ..piece(
                "grid",
                "Children in 2 to 4 equal columns, one column under 360 px.",
                "column",
                r#"<grid columns={3}><number label="Open" value={3} /><number label="Done" value={9} /><number label="Failed" value={0} /></grid>"#,
                |v| vec![format!("grid of {}", shown(v.prop("columns").as_ref()))],
            )
        },
    );
    pieces.insert(
        "row",
        SlatePieceModule {
            holds_children: true,
            props: props([
                ("gap", enm(DENSITY, Some("normal"))),
                ("align", enm(&["start", "center", "end", "between"], Some("start"))),
                ("wrap", flag()),
            ]),
            ..piece(
                "row",
                "Places children side by side.",
                "as column",
                r#"<row align="between"><text>Left</text><text>Right</text></row>"#,
                sketch_empty,
            )
        },
    );
    pieces.insert(
        "section",
        SlatePieceModule {
            holds_children: true,
            props: with_box(props([
                ("title", req(str_prop())),
                ("note", str_prop()),
                ("collapsible", flag()),
                (
                    "open",
                    SlatePropSpec {
                        literal: true,
                        about: Some("open={false} starts it shut; open={$open} also keeps it".to_string()),
                        ..spec(SlatePropType::Boolean, SlateBinds::State)
                    },
                ),
                ("align", enm(PLACE, None)),
            ])),
            events: &["change"],
            collapsed: Some(|v: &dyn SlateSketchView| v.prop("open") == Some(Value::Bool(false))),
            ..piece(
                "section",
                "A titled group, optionally collapsible.",
                "children under a text with the title",
                r#"<section title="Files changed" collapsible><text>None yet</text></section>"#,
                |v| {
                    let title = shown(v.prop("title").as_ref());
                    if v.prop("open") == Some(Value::Bool(false)) {
                        vec![format!("{title} (collapsed)")]
                    } else {
                        vec![title]
                    }
                },
            )
        },
    );
    pieces.insert(
        "text",
        SlatePieceModule {
            text_prop: Some("value"),
            props: props([
                ("value", req(spec(SlatePropType::Text, SlateBinds::Yes))),
                ("tone", tone(SLATE_TONES)),
                ("emphasis", spec(SlatePropType::Enum(EMPHASIS), SlateBinds::Yes)),
                ("mono", flag()),
                ("size", enm(SIZE, None)),
                ("lines", SlatePropSpec { min: Some(1.0), max: Some(20.0), ..spec(SlatePropType::Integer, SlateBinds::No) }),
                ("placeholder", str_prop()),
            ]),
            ..piece(
                "text",
                "A line or a paragraph; a text child with {holes} fills a sentence.",
                "none needed",
                r#"<text tone="muted">{tokens(thread.context.free)} free</text>"#,
                sketch_text,
            )
        },
    );
    pieces.insert(
        "heading",
        SlatePieceModule {
            text_prop: Some("value"),
            props: props([
                ("value", SlatePropSpec { required: true, head: true, ..spec(SlatePropType::Text, SlateBinds::Yes) }),
                ("level", enm(&["title", "section", "label"], None)),
            ]),
            ..piece(
                "heading",
                "A heading on the app's type ladder: title, section, or label in small caps.",
                "text",
                r#"<heading level="title">Gold price</heading>"#,
                |v| {
                    let s = shown(v.prop("value").as_ref());
                    match v.prop("level").as_ref().and_then(Value::as_str) {
                        Some("title") => vec![format!("# {s}")],
                        Some("label") => vec![s.to_uppercase()],
                        _ => vec![format!("## {s}")],
                    }
                },
            )
        },
    );
    pieces.insert(
        "markdown",
        SlatePieceModule {
            text_prop: Some("value"),
            props: props([("value", SlatePropSpec { required: true, marks: true, ..req(str_prop()) })]),
            ..piece(
                "markdown",
                "Rich text, sanitised; images are not fetched.",
                "text",
                "<markdown>**Why.** The host keeps the last 5,000 events.</markdown>",
                |v| {
                    let text = shown(v.prop("value").as_ref());
                    let lines: Vec<&str> = text.split('\n').collect();
                    vec![if lines.len() > 1 { format!("{} (+{} lines)", lines[0], lines.len() - 1) } else { lines[0].to_string() }]
                },
            )
        },
    );
    pieces.insert(
        "number",
        SlatePieceModule {
            props: props([
                ("label", req(str_prop())),
                ("value", req(spec(SlatePropType::Text, SlateBinds::Yes))),
                ("format", enm(SLATE_FIGURES, Some("plain"))),
                ("unit", str_prop()),
                ("tone", tone(SLATE_TONES)),
                ("note", str_prop()),
                ("size", enm(&["normal", "large"], None)),
                (
                    "trend",
                    SlatePropSpec {
                        about: Some("read in the sketch alone; the panel's stat cell draws its figure alone, so a trend the person sees is a sparkline beside it".to_string()),
                        of_number: true,
                        ..spec(SlatePropType::List, SlateBinds::Yes)
                    },
                ),
            ]),
            ..piece(
                "number",
                "A figure with a label.",
                "text",
                r#"<number label="Spent" value={thread.cost.usd} format="usd" />"#,
                |v| {
                    let value = v.prop("value");
                    let fig = if value.as_ref().is_none_or(Value::is_null) {
                        "not read yet".to_string()
                    } else {
                        figure(&word_of(v.prop("format"), "plain"), value.as_ref(), Some(100.0))
                    };
                    let line = numbers(v.prop("trend").as_ref());
                    let trend_text = if line.len() > 1 {
                        format!("trend {} to {}", number_string(line[0]), number_string(line[line.len() - 1]))
                    } else {
                        String::new()
                    };
                    let unit = shown(v.prop("unit").as_ref());
                    vec![join2(&[
                        shown(v.prop("label").as_ref()),
                        join_with(" ", &[fig, unit]),
                        shown(v.prop("note").as_ref()),
                        trend_text,
                    ])]
                },
            )
        },
    );
    pieces.insert(
        "meter",
        SlatePieceModule {
            props: props([
                ("label", req(str_prop())),
                ("value", req(num_prop())),
                ("max", num_prop()),
                ("note", str_prop()),
                ("tone", tone(&["default", "good", "warning", "bad"])),
                ("format", enm(&["percent", "value", "fraction", "tokens", "bytes", "none"], Some("percent"))),
            ]),
            ..piece(
                "meter",
                "One value against a maximum.",
                "number",
                r#"<meter id="week" label="Weekly" value={usage.week.percent} note={`resets ${until(usage.week.resetsAt)}`} />"#,
                |v| {
                    let value = v.prop("value");
                    let max = or_default(v.prop("max"), json!(100));
                    let label = shown(v.prop("label").as_ref());
                    let note = shown(v.prop("note").as_ref());
                    if v.unbound() {
                        return vec![join2(&[label, format!("[{} of {}]", shown(value.as_ref()), shown(Some(&max))), note])];
                    }
                    if num_of(value.as_ref()).is_none() {
                        return vec![join2(&[label, "[..........] not read yet".to_string()])];
                    }
                    let format = word_of(v.prop("format"), "percent");
                    let drawn = js_trim_end(&format!(
                        "{} {}",
                        bar(value.as_ref(), Some(&max)),
                        figure(&format, value.as_ref(), num_of(Some(&max)))
                    ))
                    .to_string();
                    vec![join2(&[label, drawn, note])]
                },
            )
        },
    );
    pieces.insert(
        "bars",
        SlatePieceModule {
            repeating: true,
            props: props([
                ("label", req(str_prop())),
                ("items", SlatePropSpec { required: true, ..spec(SlatePropType::List, SlateBinds::Yes) }),
                ("key", row_key()),
                ("name", SlatePropSpec { required: true, ..spec(SlatePropType::String, SlateBinds::Item) }),
                ("value", SlatePropSpec { required: true, ..spec(SlatePropType::Number, SlateBinds::Item) }),
                ("tone", spec(SlatePropType::Enum(SLATE_TONES), SlateBinds::Item)),
                ("max", num_prop()),
                ("format", enm(&["percent", "value", "none"], Some("value"))),
            ]),
            check: Some(|c: &mut SlatePieceCheck| {
                let props = c.props;
                if let Some(bind) = binding_of(props.get("items")).filter(|b| slate_is_series(js_trim(b))) {
                    c.add("W003", &format!("{bind} is a series over time; draw it as a line"), Some("items"), Some("<chart>"));
                } else if let Some(bind) = binding_of(props.get("name")).filter(|b| reads_time(b)) {
                    c.add(
                        "W003",
                        &format!("bars name each row by {bind}, a time: a series over time is a line"),
                        Some("name"),
                        Some("<chart>"),
                    );
                }
            }),
            ..piece(
                "bars",
                "Categories compared as rows of name, track and figure; never a series over time.",
                "table",
                r#"<bars label="Busiest" items={processes.list | take(5)} name={item.name} value={item.cpu} />"#,
                |v| {
                    let items = v.prop("items");
                    let values: Vec<f64> = list_of(items.as_ref())
                        .iter()
                        .enumerate()
                        .filter_map(|(i, item)| num_of(v.row(&spec_of(vec![("value", v.raw("value"))]), item, i as i64).get("value")))
                        .collect();
                    let max = or_default(v.prop("max"), json!(js_max(&values).max(0.0)));
                    let format = word_of(v.prop("format"), "value");
                    let mut out = vec![shown(v.prop("label").as_ref())];
                    out.extend(rows(
                        v,
                        |item, i| {
                            let r = v.row(&spec_of(vec![("name", v.raw("name")), ("value", v.raw("value"))]), item, i);
                            let t = v.row(&spec_of(vec![("tone", v.raw("tone"))]), item, i);
                            format!(
                                "  {}  {} {}{}",
                                shown(r.get("name")),
                                bar(r.get("value"), Some(&max)),
                                figure(&format, r.get("value"), num_of(Some(&max))),
                                marks(&t)
                            )
                        },
                        "Nothing here",
                    ));
                    out
                },
            )
        },
    );
    pieces.insert(
        "chart",
        SlatePieceModule {
            repeating: true,
            props: props([
                ("label", req(str_prop())),
                ("items", SlatePropSpec { required: true, ..spec(SlatePropType::List, SlateBinds::Yes) }),
                (
                    "x",
                    SlatePropSpec {
                        about: Some("a time on x (ISO or ms) labels the axis by clock, in this computer's time zone; a number by its value".to_string()),
                        ..spec(SlatePropType::Any, SlateBinds::Item)
                    },
                ),
                ("value", spec(SlatePropType::Number, SlateBinds::Item)),
                ("format", enm(SLATE_FIGURES, Some("plain"))),
                ("unit", str_prop()),
                ("tone", tone(SLATE_TONES)),
                ("height", enm(SIZE, Some("normal"))),
                ("stack", flag()),
            ]),
            items: IndexMap::from([(
                "series",
                SlateItemSpec {
                    prop: "series",
                    fields: props([
                        ("label", SlatePropSpec { required: true, ..spec(SlatePropType::String, SlateBinds::No) }),
                        ("value", SlatePropSpec { required: true, ..spec(SlatePropType::Number, SlateBinds::Item) }),
                        ("tone", spec(SlatePropType::Enum(SLATE_TONES), SlateBinds::No)),
                    ]),
                    row: true,
                    row_when: false,
                    events: None,
                    min: None,
                    max: Some(4),
                },
            )]),
            check: Some(|c: &mut SlatePieceCheck| {
                let props = c.props;
                let x = props.get("x");
                if x.is_none() || binding_of(x).is_some_and(|b| js_trim(b) == "index") {
                    c.add(
                        "W013",
                        "the chart's x is each row's index, so its axis reads 0 to the count; give each row its time, for example x={item.at}",
                        Some("x"),
                        Some("x={item.at}"),
                    );
                }
                let has_series = !list_of(props.get("series")).is_empty();
                if props.get("value").is_none() && !has_series {
                    c.add(
                        "T304",
                        "a chart draws value={...} or a <series> per line, and it has neither",
                        Some("value"),
                        Some("value={item.v}"),
                    );
                } else if props.get("value").is_some() && has_series {
                    c.add(
                        "W020",
                        "the chart draws its <series> and leaves value out; drop value, or give it a <series> of its own",
                        Some("value"),
                        None,
                    );
                }
            }),
            ..piece(
                "chart",
                "Lines over a list, oldest first; stack piles the <series>.",
                "text",
                r#"<chart label="Gold" items={$hist} x={item.at} value={item.v} />"#,
                sketch_chart,
            )
        },
    );
    pieces.insert(
        "diagram",
        SlatePieceModule {
            raw_text: true,
            text_prop: Some("value"),
            props: props([
                (
                    "value",
                    SlatePropSpec {
                        required: true,
                        about: Some("Mermaid source; a text child is taken as written, braces and all. Labels a few words, decisions short questions; detail goes in a text beside it. Under about 15 nodes; past that, split it or summarize".to_string()),
                        ..spec(SlatePropType::String, SlateBinds::Yes)
                    },
                ),
                ("label", str_prop()),
            ]),
            ..piece(
                "diagram",
                "A Mermaid diagram: a flow, a sequence or a state machine; value={`...${$step}`} shows the live step.",
                "its source as code",
                "<diagram label=\"Deploy\" value={`flowchart LR\n  build --> test --> ship\n  classDef now stroke-width:3px\n  class ${$step} now`} />",
                |v| {
                    let text = shown(v.prop("value").as_ref());
                    let lines: Vec<&str> = text.split('\n').collect();
                    let label = shown(v.prop("label").as_ref());
                    let prefix = if label.is_empty() { String::new() } else { format!("{label}  ") };
                    let more = if lines.len() > 1 {
                        format!(" (+{} line{})", lines.len() - 1, if lines.len() == 2 { "" } else { "s" })
                    } else {
                        String::new()
                    };
                    vec![format!("{prefix}{}{more}", lines[0])]
                },
            )
        },
    );
    pieces.insert(
        "timeline",
        SlatePieceModule {
            repeating: true,
            props: props([
                ("label", req(str_prop())),
                ("items", SlatePropSpec { required: true, ..spec(SlatePropType::List, SlateBinds::Yes) }),
                ("name", SlatePropSpec { required: true, ..spec(SlatePropType::String, SlateBinds::Item) }),
                (
                    "start",
                    SlatePropSpec {
                        time: true,
                        about: Some("a time, ISO or ms; none waits".to_string()),
                        ..spec(SlatePropType::Any, SlateBinds::Item)
                    },
                ),
                (
                    "end",
                    SlatePropSpec {
                        time: true,
                        about: Some("none while it runs".to_string()),
                        ..spec(SlatePropType::Any, SlateBinds::Item)
                    },
                ),
                ("state", spec(SlatePropType::Enum(SLATE_SPAN_STATES), SlateBinds::Item)),
                (
                    "group",
                    SlatePropSpec {
                        about: Some("rows of one group sit under its name".to_string()),
                        ..spec(SlatePropType::String, SlateBinds::Item)
                    },
                ),
                (
                    "now",
                    SlatePropSpec {
                        time: true,
                        about: Some("where a running span ends; the clock if unset".to_string()),
                        ..spec(SlatePropType::Any, SlateBinds::Yes)
                    },
                ),
            ]),
            ..piece(
                "timeline",
                "Spans on a clock, Gantt style, such as a run's steps or a deploy's phases.",
                "table",
                r#"<timeline label="CI run" items={$steps} name={item.name} start={item.started} end={item.ended} group={item.job} />"#,
                sketch_timeline,
            )
        },
    );
    pieces.insert(
        "treemap",
        SlatePieceModule {
            repeating: true,
            props: props([
                ("label", req(str_prop())),
                ("items", SlatePropSpec { required: true, ..spec(SlatePropType::List, SlateBinds::Yes) }),
                ("name", SlatePropSpec { required: true, ..spec(SlatePropType::String, SlateBinds::Item) }),
                ("value", SlatePropSpec { required: true, ..spec(SlatePropType::Number, SlateBinds::Item) }),
                ("group", spec(SlatePropType::String, SlateBinds::Item)),
                ("format", enm(SLATE_FIGURES, Some("plain"))),
            ]),
            ..piece(
                "treemap",
                "Sizes of one whole's parts as nested boxes, such as a bundle by module within package.",
                "bars",
                r#"<treemap label="Bundle" items={$stats.json.modules} name={item.name} value={item.bytes} group={item.package} format="bytes" />"#,
                sketch_treemap,
            )
        },
    );
    pieces.insert(
        "donut",
        SlatePieceModule {
            repeating: true,
            props: props([
                ("label", req(str_prop())),
                ("items", SlatePropSpec { required: true, ..spec(SlatePropType::List, SlateBinds::Yes) }),
                ("name", SlatePropSpec { required: true, ..spec(SlatePropType::String, SlateBinds::Item) }),
                ("value", SlatePropSpec { required: true, ..spec(SlatePropType::Number, SlateBinds::Item) }),
                ("format", enm(SLATE_FIGURES, Some("plain"))),
                ("unit", str_prop()),
            ]),
            ..piece(
                "donut",
                "Shares of one whole, five parts at most with the rest as Other; never a series over time.",
                "bars",
                r#"<donut label="By country" items={$countries} name={item.name} value={item.n} format="integer" />"#,
                sketch_donut,
            )
        },
    );
    pieces.insert(
        "image",
        SlatePieceModule {
            props: props([
                (
                    "src",
                    SlatePropSpec {
                        required: true,
                        about: Some(format!(
                            "a path on the thread's computer, relative to its folder or whole, or an http(s) address; {IMAGE_TYPE_WORDS} up to {IMAGE_MAX_WORDS}"
                        )),
                        ..str_prop()
                    },
                ),
                ("caption", str_prop()),
            ]),
            check: Some(|c: &mut SlatePieceCheck| {
                if let Some(Value::String(src)) = c.props.get("src") {
                    if let SlateImageSource::Problem(problem) = slate_image_source(src) {
                        c.add(&problem.code, &problem.message, Some("src"), Some("src=\"/tmp/home.png\""));
                    }
                }
            }),
            ..piece(
                "image",
                "A picture from a file on the thread's computer, such as a screenshot or a saved plot, or from an https address once the person allows its domain; opens large to zoom.",
                "its path as text",
                r#"<image src="/tmp/login-failed.png" caption="Login, after the redirect" />"#,
                |v| vec![join2(&[format!("[image {}]", shown(v.prop("src").as_ref())), shown(v.prop("caption").as_ref())])],
            )
        },
    );
    pieces.insert(
        "images",
        SlatePieceModule {
            repeating: true,
            props: props([
                (
                    "items",
                    SlatePropSpec {
                        required: true,
                        most: Some(SlateMost { count: SLATE_LIMITS.images_per_list, noun: "images" }),
                        ..spec(SlatePropType::List, SlateBinds::Yes)
                    },
                ),
                ("src", SlatePropSpec { required: true, ..spec(SlatePropType::String, SlateBinds::Item) }),
                ("caption", spec(SlatePropType::String, SlateBinds::Item)),
                ("layout", enm(SLATE_IMAGE_LAYOUTS, Some("gallery"))),
            ]),
            check: Some(|c: &mut SlatePieceCheck| {
                let props = c.props;
                let items = props.get("items");
                if props.get("layout") == Some(&json!("compare")) {
                    if let Some(Value::Array(list)) = items {
                        if list.len() != 2 {
                            c.add(
                                "T303",
                                &format!("a compare shows exactly two images, before and after, and this list has {}", list.len()),
                                Some("items"),
                                None,
                            );
                        }
                    }
                }
                if let Some(Value::Array(list)) = items {
                    if list.len() > SLATE_LIMITS.images_per_list {
                        let problem = slate_list_too_long(list.len(), SLATE_LIMITS.images_per_list, "images", None);
                        c.add("R905", &problem.message, Some("items"), None);
                    }
                }
            }),
            ..piece(
                "images",
                "Several pictures, as image takes them: a row that scrolls, a grid, or a before and after under a slider.",
                "a list of paths",
                r#"<images layout="compare" items={[{ p: "shots/before.png", c: "Before" }, { p: "shots/after.png", c: "After" }]} src={item.p} caption={item.c} />"#,
                |v| {
                    let layout = word_of(v.prop("layout"), "gallery");
                    let mut out = vec![format!("{layout} of images")];
                    out.extend(rows(
                        v,
                        |item, i| {
                            let r = v.row(
                                &spec_of(vec![("src", v.raw("src")), ("caption", v.raw("caption"))]),
                                item,
                                i,
                            );
                            format!("  {}", join2(&[format!("[image {}]", shown(r.get("src"))), shown(r.get("caption"))]))
                        },
                        "No images",
                    ));
                    out
                },
            )
        },
    );
    pieces.insert(
        "sparkline",
        SlatePieceModule {
            props: props([
                ("label", req(str_prop())),
                ("values", SlatePropSpec { required: true, of_number: true, ..spec(SlatePropType::List, SlateBinds::Yes) }),
                ("tone", tone(SLATE_TONES)),
            ]),
            ..piece(
                "sparkline",
                "A small line beside text, the way to show a figure's trend.",
                "text",
                r#"<sparkline label="Load" values={pluck($hist, 'v')} />"#,
                |v| vec![trend(&shown(v.prop("label").as_ref()), &numbers(v.prop("values").as_ref()))],
            )
        },
    );
    pieces.insert(
        "table",
        SlatePieceModule {
            repeating: true,
            props: props([
                ("items", SlatePropSpec { required: true, ..spec(SlatePropType::List, SlateBinds::Yes) }),
                ("key", row_key()),
                ("empty", str_prop()),
                (
                    "rows",
                    SlatePropSpec { min: Some(1.0), max: Some(5000.0), ..spec(SlatePropType::Integer, SlateBinds::No) },
                ),
            ]),
            items: IndexMap::from([
                (
                    "col",
                    SlateItemSpec {
                        prop: "columns",
                        fields: props([
                            ("title", SlatePropSpec { required: true, ..spec(SlatePropType::String, SlateBinds::No) }),
                            ("value", SlatePropSpec { required: true, ..spec(SlatePropType::Text, SlateBinds::Item) }),
                            ("tone", spec(SlatePropType::Enum(SLATE_TONES), SlateBinds::Item)),
                            ("emphasis", spec(SlatePropType::Enum(EMPHASIS), SlateBinds::Item)),
                            ("mono", flag()),
                            ("align", enm(&["start", "end"], Some("start"))),
                            ("width", enm(&["fit", "fill"], None)),
                        ]),
                        row: true,
                        row_when: false,
                        events: None,
                        min: Some(1),
                        max: Some(SLATE_LIMITS.columns),
                    },
                ),
                ("action", row_action_item()),
            ]),
            ..piece(
                "table",
                "Rows with columns from a bound list: <col> per column, <action> per row button.",
                "a list of text rows",
                r#"<table id="checks" items={pr.checks} key={item.name}><col title="Check" value={item.name} /><action label="Fix" when={item.state == 'fail'} onPress={send("Fix this check.", item.name)} /></table>"#,
                sketch_table,
            )
        },
    );
    pieces.insert(
        "checklist",
        SlatePieceModule {
            repeating: true,
            events: &["change"],
            props: props([
                ("items", SlatePropSpec { required: true, ..spec(SlatePropType::List, SlateBinds::Yes) }),
                ("key", row_key()),
                ("title", SlatePropSpec { required: true, ..spec(SlatePropType::String, SlateBinds::Item) }),
                ("done", SlatePropSpec { required: true, ..spec(SlatePropType::Boolean, SlateBinds::Item) }),
                (
                    "state",
                    SlatePropSpec {
                        unseen: Some("this build's checklist draws done alone"),
                        ..spec(SlatePropType::Enum(&["pending", "working", "done"]), SlateBinds::Item)
                    },
                ),
                ("note", spec(SlatePropType::String, SlateBinds::Item)),
                ("editable", flag()),
                ("empty", str_prop()),
            ]),
            check: Some(|c: &mut SlatePieceCheck| {
                let props = c.props;
                if props.get("editable") == Some(&Value::Bool(true)) {
                    let bound_value = binding_of(props.get("items"))
                        .is_some_and(|bind| (c.kind)(&slate_own_name(bind).unwrap_or_default()).as_deref() == Some("value"));
                    if !bound_value {
                        c.add(
                            "S502",
                            "an editable checklist writes its ticks back, so items binds a value: items={$steps}",
                            Some("items"),
                            None,
                        );
                    }
                }
            }),
            ..piece(
                "checklist",
                "Rows with a tick; editable writes the tick back to the value.",
                "text rows",
                r#"<checklist items={$steps} title={item.title} done={item.done} editable />"#,
                sketch_rows_text,
            )
        },
    );
    pieces.insert(
        "facts",
        SlatePieceModule {
            props: props([("layout", enm(&["line", "grid"], Some("line")))]),
            items: IndexMap::from([(
                "fact",
                SlateItemSpec {
                    prop: "facts",
                    fields: props([
                        ("label", req(str_prop())),
                        ("value", SlatePropSpec { required: true, ..spec(SlatePropType::Text, SlateBinds::Yes) }),
                        ("tone", tone(SLATE_TONES)),
                        ("emphasis", spec(SlatePropType::Enum(EMPHASIS), SlateBinds::Yes)),
                        ("mono", flag()),
                    ]),
                    row: false,
                    row_when: false,
                    events: None,
                    min: Some(1),
                    max: Some(SLATE_LIMITS.facts),
                },
            )]),
            ..piece(
                "facts",
                "Label and value pairs: <fact> per pair.",
                "text lines",
                r#"<facts><fact label="State" value={pr.word} /><fact label="Review" value={word(pr.review)} /></facts>"#,
                sketch_facts,
            )
        },
    );
    pieces.insert(
        "status",
        SlatePieceModule {
            text_prop: Some("value"),
            props: props([
                ("value", SlatePropSpec { required: true, ..spec(SlatePropType::Text, SlateBinds::Yes) }),
                ("tone", tone(&["default", "muted", "good", "warning", "bad", "info"])),
            ]),
            ..piece(
                "status",
                "A dot in a tone with a word: Live, Down, Waiting.",
                "text",
                "<status tone={$up ? 'good' : 'bad'}>{$up ? 'Live' : 'Down'}</status>",
                |v| {
                    let tone_word = shown(v.prop("tone").as_ref());
                    let tone_word = if tone_word.is_empty() { "default".to_string() } else { tone_word };
                    vec![format!("({tone_word}) {}", shown(v.prop("value").as_ref()))]
                },
            )
        },
    );
    pieces.insert(
        "chip",
        SlatePieceModule {
            text_prop: Some("value"),
            props: props([("value", SlatePropSpec { required: true, ..spec(SlatePropType::Text, SlateBinds::Yes) })]),
            ..piece("chip", "A short fact in a small framed chip.", "text", "<chip>{git.branch}</chip>", |v| {
                vec![format!("[{}]", shown(v.prop("value").as_ref()))]
            })
        },
    );
    pieces.insert(
        "ring",
        SlatePieceModule {
            props: props([
                ("label", req(str_prop())),
                ("value", req(num_prop())),
                ("max", num_prop()),
                ("format", enm(&["percent", "value", "fraction"], Some("percent"))),
                ("tone", tone(&["default", "good", "warning", "bad"])),
                ("note", str_prop()),
            ]),
            ..piece(
                "ring",
                "Progress as a ring with its figure inside.",
                "meter",
                r#"<ring label="Done" value={$done} max={len($steps)} format="fraction" />"#,
                |v| {
                    let value = v.prop("value");
                    let max = or_default(v.prop("max"), json!(100));
                    let label = shown(v.prop("label").as_ref());
                    if v.unbound() || num_of(value.as_ref()).is_none() {
                        let shown_value = if v.unbound() { shown(value.as_ref()) } else { "not read yet".to_string() };
                        return vec![join2(&[label, format!("({shown_value})")])];
                    }
                    let format = word_of(v.prop("format"), "percent");
                    let drawn = format!("({}) {}", figure(&format, value.as_ref(), num_of(Some(&max))), bar(value.as_ref(), Some(&max)));
                    vec![join2(&[label, drawn, shown(v.prop("note").as_ref())])]
                },
            )
        },
    );
    pieces.insert(
        "output",
        SlatePieceModule {
            props: props([
                ("run", SlatePropSpec { required: true, names_run: true, ..spec(SlatePropType::Path, SlateBinds::No) }),
                ("label", str_prop()),
                (
                    "lines",
                    SlatePropSpec {
                        min: Some(3.0),
                        max: Some(40.0),
                        about: Some("how many of the newest lines show, out then err, 8 by default".to_string()),
                        ..spec(SlatePropType::Integer, SlateBinds::No)
                    },
                ),
                ("wrap", flag()),
            ]),
            ..piece(
                "output",
                "A run's output as it streams, with its state and a Cancel.",
                "text with the run's state",
                r#"<output run={$tests} lines={12} />"#,
                sketch_output,
            )
        },
    );
    pieces.insert(
        "button",
        SlatePieceModule {
            interactive: true,
            text_prop: Some("label"),
            needs_handler: Some("press"),
            events: &["press"],
            props: props([
                ("label", req(str_prop())),
                ("variant", enm(VARIANT, None)),
                ("held", str_prop()),
                ("note", str_prop()),
                ("size", enm(&["normal", "small"], None)),
            ]),
            ..piece(
                "button",
                "An action the person presses. held is a sentence that disables it: bind it to a condition.",
                "none needed",
                r#"<button label="Next" variant="primary" held={$ok ? null : 'Check the id first'} onPress={set($step, 2)} />"#,
                |v| {
                    let held = slate_held_text(v.prop("held").as_ref());
                    let tail = held.map(|h| format!(" (held: {h})")).unwrap_or_default();
                    vec![format!("[ {} ]{tail}", shown(v.prop("label").as_ref()))]
                },
            )
        },
    );
    pieces.insert(
        "input",
        SlatePieceModule {
            interactive: true,
            events: &["change", "submit"],
            props: props([
                ("label", req(str_prop())),
                ("value", SlatePropSpec { required: true, ..spec(SlatePropType::Text, SlateBinds::State) }),
                ("lines", SlatePropSpec { min: Some(1.0), max: Some(20.0), ..spec(SlatePropType::Integer, SlateBinds::No) }),
                ("kind", enm(&["text", "number", "password"], Some("text"))),
                ("placeholder", str_prop()),
                ("held", str_prop()),
                ("mono", flag()),
                ("submit", str_prop()),
            ]),
            ..piece(
                "input",
                "Text in, written to its $value as the person types; a secret's input is a password field.",
                "none needed",
                r#"<input label="Issue number" value={$id} mono />"#,
                sketch_input,
            )
        },
    );
    pieces.insert(
        "select",
        SlatePieceModule {
            interactive: true,
            events: &["change"],
            props: props([
                ("label", req(str_prop())),
                ("value", SlatePropSpec { required: true, ..spec(SlatePropType::String, SlateBinds::State) }),
                ("options", spec(SlatePropType::List, SlateBinds::Yes)),
                ("placeholder", str_prop()),
                ("held", str_prop()),
            ]),
            items: IndexMap::from([(
                "option",
                SlateItemSpec {
                    prop: "options",
                    fields: props([
                        ("value", SlatePropSpec { required: true, ..spec(SlatePropType::String, SlateBinds::No) }),
                        ("label", spec(SlatePropType::String, SlateBinds::No)),
                    ]),
                    row: false,
                    row_when: false,
                    events: None,
                    min: None,
                    max: None,
                },
            )]),
            ..piece(
                "select",
                "One choice from options, written to its $value.",
                "none needed",
                r#"<select label="Hide" value={$hide}><option value="Done" /><option value="Canceled" /></select>"#,
                sketch_select,
            )
        },
    );
    pieces.insert(
        "choices",
        SlatePieceModule {
            interactive: true,
            events: &["change"],
            props: props([
                ("label", req(str_prop())),
                ("value", SlatePropSpec { required: true, ..spec(SlatePropType::Any, SlateBinds::State) }),
                ("options", spec(SlatePropType::List, SlateBinds::Yes)),
                ("answer", spec(SlatePropType::Any, SlateBinds::Yes)),
                ("held", str_prop()),
            ]),
            items: IndexMap::from([(
                "option",
                SlateItemSpec {
                    prop: "options",
                    fields: props([
                        ("value", SlatePropSpec { required: true, ..spec(SlatePropType::Any, SlateBinds::Yes) }),
                        ("label", str_prop()),
                        ("note", str_prop()),
                    ]),
                    row: false,
                    row_when: false,
                    events: None,
                    min: None,
                    max: None,
                },
            )]),
            ..piece(
                "choices",
                "Large options to pick one, written to its $value; answer marks the right one once picked.",
                "select",
                r#"<choices label="Which layer routes packets?" value={$pick} options={$q.options} answer={$q.answer} />"#,
                sketch_choices,
            )
        },
    );
    pieces.insert(
        "toggle",
        SlatePieceModule {
            interactive: true,
            events: &["change"],
            props: props([
                ("label", req(str_prop())),
                ("value", SlatePropSpec { required: true, ..spec(SlatePropType::Boolean, SlateBinds::State) }),
                ("note", str_prop()),
                ("held", str_prop()),
            ]),
            ..piece(
                "toggle",
                "An on/off switch written to its boolean $value.",
                "none needed",
                r#"<toggle label="Show done" value={$showDone} />"#,
                |v| {
                    vec![format!(
                        "{}: {}",
                        shown(v.prop("label").as_ref()),
                        if v.prop("value") == Some(Value::Bool(true)) { "on" } else { "off" }
                    )]
                },
            )
        },
    );
    pieces.insert(
        "empty",
        SlatePieceModule {
            holds_children: true,
            child_limit: Some(SlateChildLimit { max: 1, types: &["button"] }),
            text_prop: Some("body"),
            props: props([("title", req(str_prop())), ("body", str_prop())]),
            ..piece(
                "empty",
                "An empty state, with at most one button under it.",
                "text",
                r#"<empty when={pr.number == null} title="No pull request yet">This branch has none.</empty>"#,
                |v| {
                    let parts = [shown(v.prop("title").as_ref()), shown(v.prop("body").as_ref())];
                    let joined: Vec<&str> = parts.iter().map(String::as_str).filter(|p| !p.is_empty()).collect();
                    vec![joined.join(". ").replace("..", ".")]
                },
            )
        },
    );

    pieces
});

/// The entry of a piece by its type name, or None when no piece has it.
pub fn slate_piece_module(kind: &str) -> Option<&'static SlatePieceModule> {
    SLATE_PIECES.get(kind)
}
