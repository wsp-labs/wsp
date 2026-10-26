// SPDX-License-Identifier: AGPL-3.0-only
//! The slate kit and sources: packages/protocol/src/slate/kit.ts, sources.ts and the kit.test.ts cases that need only the
//! formula language at 65f6b89c7. The registries, the piece sketches and checks, the source paths and the figures are
//! replayed from fixtures/slate_kit_*.json, which node wrote by running the TS modules (a fake view answers the piece
//! functions the way the TS sketch's view does). The TS source of each fixture is the TS module, not this file.

use std::rc::Rc;

use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use wsp_slates::pending::slate::*;
use wsp_words::js::js_trim;

const REGISTRY: &str = include_str!("fixtures/slate_kit_registry.json");
const SKETCH: &str = include_str!("fixtures/slate_kit_sketch.json");
const SOURCES: &str = include_str!("fixtures/slate_kit_sources.json");
const FIGURES: &str = include_str!("fixtures/slate_kit_figures.json");

fn fixture(text: &str) -> Value {
    serde_json::from_str(text).expect("a TS fixture is JSON")
}

fn obj(pairs: Vec<(&str, Value)>) -> Value {
    Value::Object(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect::<Map<String, Value>>())
}

fn num_json(x: f64) -> Value {
    if x.fract() == 0.0 {
        json!(x as i64)
    } else {
        json!(x)
    }
}

fn prop_type_json(t: &SlatePropType) -> Value {
    match t {
        SlatePropType::String => json!("string"),
        SlatePropType::Number => json!("number"),
        SlatePropType::Integer => json!("integer"),
        SlatePropType::Boolean => json!("boolean"),
        SlatePropType::Text => json!("text"),
        SlatePropType::List => json!("list"),
        SlatePropType::Any => json!("any"),
        SlatePropType::Id => json!("id"),
        SlatePropType::Path => json!("path"),
        SlatePropType::Enum(values) => json!(values),
    }
}

fn spec_json(s: &SlatePropSpec) -> Value {
    let binds = match s.binds {
        SlateBinds::No => "no",
        SlateBinds::Yes => "yes",
        SlateBinds::State => "state",
        SlateBinds::Item => "item",
    };
    let mut pairs = vec![("type", prop_type_json(&s.prop_type)), ("binds", json!(binds))];
    if s.required {
        pairs.push(("required", json!(true)));
    }
    if let Some(default) = &s.default {
        pairs.push(("default", default.clone()));
    }
    if let Some(min) = s.min {
        pairs.push(("min", num_json(min)));
    }
    if let Some(max) = s.max {
        pairs.push(("max", num_json(max)));
    }
    if let Some(unseen) = s.unseen {
        pairs.push(("unseen", json!(unseen)));
    }
    if let Some(about) = &s.about {
        pairs.push(("about", json!(about)));
    }
    if s.literal {
        pairs.push(("literal", json!(true)));
    }
    if s.of_number {
        pairs.push(("of", json!("number")));
    }
    if let Some(most) = &s.most {
        pairs.push(("most", obj(vec![("count", json!(most.count)), ("noun", json!(most.noun))])));
    }
    if s.time {
        pairs.push(("time", json!(true)));
    }
    if s.names_run {
        pairs.push(("names", json!("run")));
    }
    if s.marks {
        pairs.push(("marks", json!(true)));
    }
    if s.head {
        pairs.push(("head", json!(true)));
    }
    obj(pairs)
}

fn fields_json(fields: &IndexMap<&'static str, SlatePropSpec>) -> Value {
    Value::Object(fields.iter().map(|(k, s)| (k.to_string(), spec_json(s))).collect())
}

fn item_json(item: &SlateItemSpec) -> Value {
    let mut pairs = vec![
        ("prop", json!(item.prop)),
        ("fieldOrder", json!(item.fields.keys().collect::<Vec<_>>())),
        ("fields", fields_json(&item.fields)),
    ];
    if item.row {
        pairs.push(("row", json!(true)));
    }
    if item.row_when {
        pairs.push(("rowWhen", json!(true)));
    }
    if let Some(events) = item.events {
        pairs.push(("events", json!(events)));
    }
    if let Some(min) = item.min {
        pairs.push(("min", json!(min)));
    }
    if let Some(max) = item.max {
        pairs.push(("max", json!(max)));
    }
    obj(pairs)
}

fn piece_json(key: &str, m: &SlatePieceModule) -> Value {
    let mut pairs = vec![
        ("key", json!(key)),
        ("type", json!(m.kind)),
        ("level", json!(m.level)),
        ("purpose", json!(m.purpose)),
        ("holdsChildren", json!(m.holds_children)),
        ("propOrder", json!(m.props.keys().collect::<Vec<_>>())),
        ("props", fields_json(&m.props)),
        ("itemOrder", json!(m.items.keys().collect::<Vec<_>>())),
        ("items", Value::Object(m.items.iter().map(|(tag, item)| (tag.to_string(), item_json(item))).collect())),
        ("events", json!(m.events)),
        ("fallback", json!(m.fallback)),
        ("example", json!(m.example)),
    ];
    if let Some(limit) = &m.child_limit {
        pairs.push(("childLimit", obj(vec![("max", json!(limit.max)), ("types", json!(limit.types))])));
    }
    if let Some(text) = m.text_prop {
        pairs.push(("textProp", json!(text)));
    }
    if m.raw_text {
        pairs.push(("rawText", json!(true)));
    }
    if m.repeating {
        pairs.push(("repeating", json!(true)));
    }
    if m.interactive {
        pairs.push(("interactive", json!(true)));
    }
    if let Some(handler) = m.needs_handler {
        pairs.push(("needsHandler", json!(handler)));
    }
    obj(pairs)
}

fn shape_json(shape: &SlateShape) -> Value {
    match shape {
        SlateShape::Number => json!("number"),
        SlateShape::String => json!("string"),
        SlateShape::Boolean => json!("boolean"),
        SlateShape::Time => json!("time"),
        SlateShape::Any => json!("any"),
        SlateShape::Enum(values) => obj(vec![("enum", json!(values))]),
        SlateShape::List(of) => obj(vec![("list", shape_json(of))]),
        SlateShape::Keyed(of) => obj(vec![("keyed", shape_json(of))]),
        SlateShape::Fields(fields) => obj(vec![
            ("fields", Value::Object(fields.iter().map(|(k, s)| (k.to_string(), shape_json(s))).collect())),
            ("fieldOrder", json!(fields.keys().collect::<Vec<_>>())),
        ]),
    }
}

fn source_json(key: &str, s: &SlateSourceModule) -> Value {
    obj(vec![
        ("key", json!(key)),
        ("name", json!(s.name)),
        ("level", json!(s.level)),
        ("purpose", json!(s.purpose)),
        ("update", json!(s.update)),
        ("cost", json!(s.cost)),
        ("scope", json!(s.scope)),
        ("shape", shape_json(&s.shape)),
        ("series", json!(s.series)),
        ("indexed", s.indexed.as_ref().map_or(Value::Null, |names| json!(names))),
        ("notes", json!(s.notes.iter().map(|(k, v)| json!([k, v])).collect::<Vec<_>>())),
        ("example", json!(s.example)),
    ])
}

#[test]
fn the_piece_registry_is_the_ts_registry_key_for_key() {
    let theirs = fixture(REGISTRY);
    let pieces = theirs["pieces"].as_array().expect("pieces");
    let ours: Vec<(&str, Value)> = SLATE_PIECES.iter().map(|(key, m)| (*key, piece_json(key, m))).collect();
    assert_eq!(ours.len(), pieces.len(), "the piece count");
    for ((key, value), want) in ours.iter().zip(pieces) {
        assert_eq!(value, want, "piece {key}");
    }
}

#[test]
fn the_source_registry_is_the_ts_registry_key_for_key() {
    let theirs = fixture(REGISTRY);
    let sources = theirs["sources"].as_array().expect("sources");
    let ours: Vec<Value> = SLATE_SOURCES.iter().map(|(key, s)| source_json(key, s)).collect();
    assert_eq!(ours.len(), sources.len(), "the source count");
    for (value, want) in ours.iter().zip(sources) {
        assert_eq!(value, want, "source {}", want["key"]);
    }
}

#[test]
fn the_source_names_are_the_registry_keys_in_order() {
    let names: Vec<&str> = SLATE_SOURCES.keys().copied().collect();
    assert_eq!(SLATE_SOURCE_NAMES.iter().copied().collect::<Vec<_>>(), names);
    assert_eq!(names, ["thread", "tree", "usage", "cost", "time", "git", "pr", "machine", "processes", "ports", "notifications", "mcp"]);
    assert!(is_slate_source("pr"));
    assert!(!is_slate_source("toString"));
}

#[test]
fn the_kit_constants_are_the_ts_lists() {
    assert_eq!(SLATE_TONES, ["default", "muted", "good", "warning", "bad", "info", "accent"]);
    assert_eq!(SLATE_FIGURES, ["plain", "tokens", "bytes", "percent", "usd", "inr", "duration", "integer"]);
    assert_eq!(SLATE_ITEM_KINDS, ["col", "action", "fact", "option", "series"]);
    assert_eq!(SLATE_EVENTS, ["press", "submit", "change"]);
    assert_eq!(SLATE_RESERVED_PROPS.len(), 31);
    assert_eq!(slate_reserved_prop("color"), Some("tone"));
    assert_eq!(slate_reserved_prop("padding"), Some("pad"));
    assert_eq!(slate_reserved_prop("bold"), Some("emphasis=\"strong\""));
    assert_eq!(slate_reserved_prop("title"), None);
}

#[test]
fn a_piece_is_found_by_its_type_name() {
    assert_eq!(slate_piece_module("grid").map(|m| m.kind), Some("grid"));
    assert!(slate_piece_module("icon").is_none());
    assert!(slate_piece_module("toString").is_none());
}

fn ty_json(t: &SlateType) -> Value {
    let mut pairs = vec![("t", json!(t.t.name()))];
    if let Some(of) = &t.of {
        pairs.push(("of", ty_json(of)));
    }
    if let Some(fields) = &t.fields {
        pairs.push(("fields", Value::Object(fields.iter().map(|(k, v)| (k.clone(), ty_json(v))).collect())));
    }
    obj(pairs)
}

fn path_segs(segs: &Value) -> Vec<PathSeg> {
    segs.as_array()
        .expect("segs")
        .iter()
        .map(|seg| match seg {
            Value::Number(n) => PathSeg::Index(n.as_i64().expect("an index")),
            Value::String(key) => PathSeg::Key(key.clone()),
            other => panic!("a path segment is a key or an index: {other}"),
        })
        .collect()
}

#[test]
fn a_source_path_has_the_type_or_the_problem_the_ts_checker_gives() {
    let theirs = fixture(SOURCES);
    for case in theirs["out"].as_array().expect("out") {
        let head = case["head"].as_str().expect("head");
        let segs = path_segs(&case["segs"]);
        let got = match slate_source_type(head, &segs) {
            Ok(t) => ty_json(&t),
            Err(problem) => {
                let mut pairs = vec![("code", json!(problem.code)), ("message", json!(problem.message))];
                if let Some(fix) = problem.fix {
                    pairs.push(("fix", json!(fix)));
                }
                obj(pairs)
            }
        };
        assert_eq!(got, case["result"], "{head} {}", case["segs"]);
    }
}

#[test]
fn a_shape_reads_as_the_ts_phrase_and_checks_as_the_ts_type() {
    let theirs = fixture(SOURCES);
    for (key, text) in theirs["texts"].as_object().expect("texts") {
        assert_eq!(slate_shape_text(&SLATE_SOURCES[key.as_str()].shape), text.as_str().expect("text"), "{key}");
    }
    for (key, want) in theirs["shapes"].as_object().expect("shapes") {
        assert_eq!(ty_json(&slate_shape_type(&SLATE_SOURCES[key.as_str()].shape)), *want, "{key}");
    }
}

#[test]
fn a_series_is_a_registered_machine_history_path() {
    let theirs = fixture(SOURCES);
    for pair in theirs["series"].as_array().expect("series") {
        let path = pair[0].as_str().expect("path");
        assert_eq!(slate_is_series(path), pair[1].as_bool().expect("bool"), "{path}");
    }
}

#[test]
fn a_figure_reads_as_the_ts_formatter_reads_it() {
    let theirs = fixture(FIGURES);
    for case in theirs["figs"].as_array().expect("figs") {
        let value = case["v"].as_f64().expect("value");
        let format = case["f"].as_str().expect("format");
        let max = case["m"].as_f64();
        assert_eq!(slate_figure(value, format, max), case["out"].as_str().expect("out"), "{value} {format} {max:?}");
    }
}

#[test]
fn a_held_sentence_is_text_that_is_not_blank() {
    let theirs = fixture(FIGURES);
    for case in theirs["held"].as_array().expect("held") {
        let got = slate_held_text(Some(&case["v"])).map_or(Value::Null, Value::String);
        assert_eq!(got, case["out"], "{}", case["v"]);
    }
}

#[test]
fn an_undefined_held_sentence_is_none() {
    assert_eq!(slate_held_text(None), None);
}

struct FakeView {
    props: IndexMap<String, Value>,
    values: IndexMap<String, Value>,
    runs: IndexMap<String, String>,
    unbound: bool,
}

fn object_of(v: &Value) -> IndexMap<String, Value> {
    v.as_object().map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default()
}

fn resolve(v: &Value, item: Option<&Value>, index: Option<i64>, values: &IndexMap<String, Value>) -> Option<Value> {
    if is_slate_binding(v) {
        let bind = js_trim(v["bind"].as_str().unwrap_or_default());
        if bind == "index" {
            return index.map(|i| json!(i));
        }
        if let Some(field) = bind.strip_prefix("item.") {
            return item.and_then(|item| item.get(field)).cloned();
        }
        if let Some(name) = bind.strip_prefix('$') {
            return values.get(name).cloned();
        }
        return None;
    }
    match v {
        Value::Array(items) => Some(Value::Array(items.iter().map(|x| resolve(x, item, index, values).unwrap_or(Value::Null)).collect())),
        other => Some(other.clone()),
    }
}

fn braces(v: &Value) -> Value {
    if is_slate_binding(v) {
        return json!(format!("{{{}}}", v["bind"].as_str().unwrap_or_default()));
    }
    match v {
        Value::Array(items) => Value::Array(items.iter().map(braces).collect()),
        other => other.clone(),
    }
}

impl FakeView {
    fn from_case(case: &Value) -> FakeView {
        FakeView {
            props: object_of(&case["props"]),
            values: object_of(&case["values"]),
            runs: object_of(&case["runs"]).into_iter().map(|(k, v)| (k, v.as_str().unwrap_or_default().to_string())).collect(),
            unbound: case["unbound"].as_bool() == Some(true),
        }
    }

    fn context(&self, item: Option<Value>, index: Option<i64>) -> SlateEvalContext {
        let values = self.values.clone();
        SlateEvalContext {
            now: 0,
            item,
            index,
            resolve: Rc::new(move |path: &str| path.strip_prefix('$').and_then(|name| values.get(name).cloned())),
        }
    }
}

impl SlateSketchView for FakeView {
    fn id(&self) -> &str {
        "p"
    }

    fn prop(&self, name: &str) -> Option<Value> {
        let v = self.props.get(name)?;
        if self.unbound {
            Some(braces(v))
        } else {
            resolve(v, None, None, &self.values)
        }
    }

    fn raw(&self, name: &str) -> Option<Value> {
        self.props.get(name).cloned()
    }

    fn bound(&self, name: &str) -> bool {
        self.props.get(name).is_some_and(|v| is_slate_binding(v))
    }

    fn row(&self, spec: &Value, item: &Value, index: i64) -> IndexMap<String, Value> {
        let Value::Object(fields) = spec else {
            return IndexMap::new();
        };
        if is_slate_binding(spec) {
            return IndexMap::new();
        }
        fields
            .iter()
            .filter(|(k, _)| k.as_str() != "on" && k.as_str() != "when")
            .filter_map(|(k, v)| {
                let resolved = if self.unbound { Some(braces(v)) } else { resolve(v, Some(item), Some(index), &self.values) };
                resolved.map(|r| (k.clone(), r))
            })
            .collect()
    }

    fn test(&self, expr: &str, item: &Value, index: i64) -> bool {
        if self.unbound {
            return true;
        }
        slate_truthy(evaluate_slate_expression(expr, &self.context(Some(item.clone()), Some(index))).as_ref())
    }

    fn read(&self, path: &str) -> Option<Value> {
        self.values.get(path.strip_prefix('$').unwrap_or(path)).cloned()
    }

    fn run_kind(&self, path: &str) -> Option<&'static str> {
        match self.runs.get(path.strip_prefix('$').unwrap_or(path))?.as_str() {
            "cmd" => Some("cmd"),
            "tool" => Some("tool"),
            "resource" => Some("resource"),
            _ => None,
        }
    }

    fn unbound(&self) -> bool {
        self.unbound
    }

    fn now(&self) -> f64 {
        121_000.0
    }
}

#[test]
fn every_piece_sketch_collapse_and_check_is_the_ts_answer() {
    let cases = fixture(SKETCH);
    let cases = cases.as_array().expect("cases");
    assert!(cases.len() >= 60, "the fixture holds the cases");
    for case in cases {
        let name = case["name"].as_str().expect("name");
        let piece = SLATE_PIECES.get(case["piece"].as_str().expect("piece")).expect(name);
        let view = FakeView::from_case(case);
        let expect = &case["expect"];
        let lines: Vec<String> = (piece.sketch)(&view);
        let want: Vec<String> = expect["lines"].as_array().expect("lines").iter().map(|l| l.as_str().expect("line").to_string()).collect();
        assert_eq!(lines, want, "{name}: sketch");
        assert_eq!(piece.collapsed.map(|collapsed| collapsed(&view)), expect["collapsed"].as_bool(), "{name}: collapsed");
        if expect["check"].as_bool() == Some(true) {
            let props = object_of(&case["props"]);
            let kinds = object_of(&case["kinds"]);
            let kind = |n: &str| kinds.get(n).and_then(Value::as_str).map(str::to_string);
            let mut check = SlatePieceCheck { props: &props, kind: &kind, problems: Vec::new() };
            (piece.check.expect("a check"))(&mut check);
            let got: Vec<Value> = check.problems.iter().map(|p| json!([p.code, p.message, p.prop, p.fix])).collect();
            assert_eq!(Value::Array(got), expect["problems"], "{name}: check");
        }
    }
}

fn ev(expr: &str, values: &[(&str, Value)], now: i64) -> Option<Value> {
    let map: IndexMap<String, Value> = values.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
    let ctx = SlateEvalContext {
        now,
        item: None,
        index: None,
        resolve: Rc::new(move |path: &str| path.strip_prefix('$').and_then(|name| map.get(name).cloned())),
    };
    evaluate_slate_expression(expr, &ctx)
}

#[test]
fn a_rolling_history_keeps_its_last_points_with_append_and_last() {
    assert_eq!(
        ev("append($h, { at: time.now, v: 2 })", &[("h", json!([{ "at": 1, "v": 1 }]))], 5),
        Some(json!([{ "at": 1, "v": 1 }, { "at": 5, "v": 2 }]))
    );
    assert_eq!(ev("append(null, 1)", &[], 0), Some(json!([1])));
    assert_eq!(ev("last(append($h, 4), 3)", &[("h", json!([1, 2, 3]))], 0), Some(json!([2, 3, 4])));
    assert_eq!(ev("last($h)", &[("h", json!([1, 2, 3]))], 0), Some(json!(3)));
    assert_eq!(ev("[1, $a + 1, 'x']", &[("a", json!(1))], 0), Some(json!([1, 2, "x"])));
}

#[test]
fn a_computed_index_reads_a_list_item_and_counts_back_from_the_end() {
    let qs = json!([{ "q": "a" }, { "q": "b" }]);
    let values = [("qs", qs), ("i", json!(1))];
    assert_eq!(ev("$qs[$i].q", &values, 0), Some(json!("b")));
    assert_eq!(ev("$qs[$i - 1].q", &values, 0), Some(json!("a")));
    assert_eq!(ev("$qs[-1].q", &values, 0), Some(json!("b")));
    assert_eq!(ev("at($qs, $i).q", &values, 0), Some(json!("b")));
    assert_eq!(ev("$qs[$i + 4]", &values, 0), Some(Value::Null));
}

#[test]
fn no_other_file_of_the_slate_module_switches_on_a_piece_name() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src/pending/slate");
    for entry in std::fs::read_dir(dir).expect("the slate module") {
        let path = entry.expect("a directory entry").path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") || name == "kit.rs" {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("a source file");
        for piece in SLATE_PIECES.keys() {
            for needle in [format!("kind == \"{piece}\""), format!("kind != \"{piece}\""), format!("\"{piece}\" =>")] {
                assert!(!text.contains(&needle), "{name} switches on a piece: {needle}");
            }
        }
    }
}
