// SPDX-License-Identifier: AGPL-3.0-only
//! protocol/src/slate/sketch.ts at 65f6b89c7.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use indexmap::{IndexMap, IndexSet};
use serde_json::Value;
use wsp_wire::text::json_stringify;
use wsp_words::js::{js_trim, js_trim_start, number_string, to_number, utf16};
use wsp_words::words::base::plural;
use wsp_words::words::units::fmt_bytes;

use super::batch::{is_secret_handle, OwnReader};
use super::*;

pub type SlateSketchContext = SketchOptions;

const TAG_PROPS: [(&str, &str); 4] = [("tone", "default"), ("emphasis", "normal"), ("size", "normal"), ("variant", "default")];
const SECRET_DOTS: &str = "••••";
const RESULT_FIELDS: [&str; 9] = ["out", "err", "json", "exit", "lines", "ms", "endedAt", "cut", "text"];

pub fn sketch_slate(doc: Option<&SlateDoc>, values: &SlateValues, o: SketchOptions) -> String {
    let version = format!(" v{}", o.version);
    let Some(doc) = doc else {
        return format!("slate{version}, empty: write one with slate_write");
    };
    let doc = Rc::new(doc.clone());
    let values = Rc::new(values.clone());
    let reader = OwnReader::new(Rc::clone(&doc), Rc::clone(&values), Rc::clone(&o.eval.resolve), o.eval.now);
    let unbound = o.check;

    let pieces = doc.pieces.len();
    let bound: usize = doc.pieces.values().map(|p| p.props.iter().flatten().map(|(_, v)| bound_count(Some(v))).sum::<usize>()).sum();
    let plotted = if unbound { Vec::new() } else { plot_problems(&doc, &reader) };
    let (warn_plot, err_plot): (Vec<SlateProblem>, Vec<SlateProblem>) =
        plotted.into_iter().partition(|p| SLATE_WARNINGS.contains(&p.code.as_str()));
    let problems: Vec<SlateProblem> = o.problems.iter().cloned().chain(err_plot).collect();
    let warned: Vec<SlateProblem> = o.warnings.iter().cloned().chain(warn_plot).collect();

    let title = doc.title.as_ref().map(|t| format!(" {}", json_stringify(&Value::String(t.clone())))).unwrap_or_default();
    let warning_word = if warned.is_empty() { String::new() } else { format!(", {}", plural(warned.len() as f64, "warning")) };
    let head = format!(
        "slate{version}{title}, {}, {bound} bound, {}{warning_word}",
        plural(pieces as f64, "piece"),
        plural(problems.len() as f64, "problem")
    );

    let mut sketch = Sketch {
        doc: Rc::clone(&doc),
        values: Rc::clone(&values),
        reader: reader.clone(),
        unbound,
        lines: Vec::new(),
        piece_lines: 0,
        folded: 0,
        seen: HashSet::new(),
    };
    sketch.walk(&doc.root, 0);
    if sketch.folded > 0 {
        sketch.lines.push(format!("... and {} more pieces", sketch.folded));
    }

    let mut value_lines = Vec::new();
    for (name, decl) in &doc.values {
        let live = values.get(name);
        if decl.secret == Some(true) {
            if let Some(handle) = live.filter(|v| is_secret_handle(v) && v.get("set") == Some(&Value::Bool(true))) {
                value_lines.push(format!("  ${name} = {}", shown_value(Some(handle))));
            }
            continue;
        }
        if !slate_equal(live, Some(&decl.start)) {
            value_lines.push(format!("  ${name} = {}", shown_value(live)));
        }
    }
    if !unbound {
        for name in doc.derived.keys() {
            value_lines.push(format!("  ${name} = {}", shown_value(reader.resolve(&format!("${name}")).as_ref())));
        }
    }
    if !value_lines.is_empty() {
        sketch.lines.push("values:".to_string());
        sketch.lines.extend(value_lines);
    }

    let mut run_lines = Vec::new();
    for name in doc.runs.keys() {
        let Some(r) = values.get(name).and_then(Value::as_object) else { continue };
        let Some(state) = r.get("state") else { continue };
        if state.as_str() == Some("idle") {
            continue;
        }
        let mut facts = Vec::new();
        if let Some(exit) = r.get("exit").filter(|e| !e.is_null()) {
            facts.push(format!("exit {}", js_text(exit)));
        }
        if let Some(ms) = r.get("ms") {
            facts.push(format!("{} ms", js_text(ms)));
        }
        let said = if facts.is_empty() { String::new() } else { format!(" ({})", facts.join(", ")) };
        let text_only = if r.get("text") == Some(&Value::Bool(true)) { ", text only" } else { "" };
        let shown = if r.get("refreshing") == Some(&Value::Bool(true)) {
            let outcome = if r.get("exit").and_then(Value::as_f64) == Some(0.0) { "succeeded" } else { "failed" };
            format!("running again; the last run {outcome}{text_only}{said}")
        } else {
            format!("{}{text_only}{said}", js_text(state))
        };
        let why = r.get("why").map(|w| format!(" {}", js_text(w))).unwrap_or_default();
        let stale = if r.get("stale") == Some(&Value::Bool(true)) { ", stale: the command changed since it ran" } else { "" };
        run_lines.push(format!("  ${name}: {shown}{why}{stale}"));
    }
    if !run_lines.is_empty() {
        sketch.lines.push("runs:".to_string());
        sketch.lines.extend(run_lines);
    }
    if !o.waiting.is_empty() {
        sketch.lines.push(format!(
            "waiting for the person's approval: {}; each starts once they allow it on the slate. Until then nothing they feed has data: ask the person to allow them, and never call the panel live or ready",
            o.waiting.join(", ")
        ));
    }
    if let Some(files) = doc.files.as_ref().filter(|f| !f.is_empty()) {
        let listed: Vec<String> = files.iter().map(|(name, text)| format!("{name} ({})", fmt_bytes(text.len() as f64))).collect();
        sketch.lines.push(format!("files in $SLATE_DIR: {}", listed.join(", ")));
    }

    let mut notes = Vec::new();
    if !problems.is_empty() {
        notes.push("problems:".to_string());
        notes.extend(problems.iter().map(|p| format!("  {}", problem_line(p, false))));
    }
    if !warned.is_empty() {
        notes.push("warnings, stored all the same; fix them before the person reads the slate:".to_string());
        notes.extend(warned.iter().map(|p| format!("  {}", problem_line(p, true))));
    }

    let cap = SLATE_LIMITS.sketch_tokens * 4;
    let body = std::iter::once(head.clone()).chain(sketch.lines).collect::<Vec<_>>().join("\n");
    let tail = notes.join("\n");
    let whole = if tail.is_empty() { body.clone() } else { format!("{body}\n{tail}") };
    if utf16::len(&whole) <= cap {
        return whole;
    }
    let keep = (utf16::len(&head) as i64).max(cap as i64 - utf16::len(&tail) as i64 - 20);
    let mut kept_parts = vec![utf16::cut(&body, keep as usize), "... (cut)".to_string()];
    if !tail.is_empty() {
        kept_parts.push(tail);
    }
    let kept = kept_parts.join("\n");
    if utf16::len(&kept) <= cap {
        kept
    } else {
        format!("{}\n... (cut)", utf16::cut(&kept, cap - 20))
    }
}

fn problem_line(p: &SlateProblem, with_fix: bool) -> String {
    let prop = p.prop.as_ref().map(|prop| format!(".{prop}")).unwrap_or_default();
    let line = p.line.map(|line| format!(" (line {line})")).unwrap_or_default();
    let fix = match (with_fix, &p.fix) {
        (true, Some(fix)) => format!(". Fix: {fix}"),
        _ => String::new(),
    };
    format!("{} {}{prop}{line}: {}{fix}", p.code, p.piece.as_deref().unwrap_or("slate"), p.message)
}

fn js_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => number_string(n.as_f64().unwrap_or(f64::NAN)),
        other => json_stringify(other),
    }
}

fn is_format(v: &Value) -> bool {
    v.as_object().is_some_and(|m| m.len() == 1 && m.get("format").is_some_and(Value::is_string))
}

fn is_bound(v: &Value) -> bool {
    is_slate_binding(v) || is_format(v)
}

fn bound_count(value: Option<&Value>) -> usize {
    let Some(v) = value else { return 0 };
    if is_bound(v) {
        return 1;
    }
    match v {
        Value::Array(items) => items.iter().map(|x| bound_count(Some(x))).sum(),
        Value::Object(fields) => fields.values().map(|x| bound_count(Some(x))).sum(),
        _ => 0,
    }
}

fn braces(value: &Value) -> Value {
    if is_slate_binding(value) {
        return Value::String(format!("{{{}}}", value["bind"].as_str().unwrap_or_default()));
    }
    if is_format(value) {
        let text: String = parse_slate_format(value["format"].as_str().unwrap_or_default())
            .parts
            .iter()
            .map(|part| match part {
                SlateFormatPart::Text(text) => text.clone(),
                SlateFormatPart::Hole { expr, .. } => format!("{{{expr}}}"),
            })
            .collect();
        return Value::String(text);
    }
    match value {
        Value::Array(items) => Value::Array(items.iter().map(braces).collect()),
        Value::Object(fields) => Value::Object(fields.iter().map(|(k, v)| (k.clone(), braces(v))).collect()),
        other => other.clone(),
    }
}

fn cut(text: &str, room: i64) -> String {
    if utf16::len(text) as i64 <= room {
        return text.to_string();
    }
    if room <= 1 {
        "…".to_string()
    } else {
        format!("{}…", utf16::cut(text, (room - 1) as usize))
    }
}

fn shown_value(v: Option<&Value>) -> String {
    if let Some(handle) = v.filter(|h| is_secret_handle(h)) {
        return if handle.get("set") == Some(&Value::Bool(true)) {
            format!("{SECRET_DOTS} ({} characters)", js_text(handle.get("len").unwrap_or(&Value::Null)))
        } else {
            "(empty)".to_string()
        };
    }
    cut(&json_stringify(v.unwrap_or(&Value::Null)), SLATE_LIMITS.sketch_value_chars as i64)
}

fn kind_of(v: &Value) -> &'static str {
    match v {
        Value::Array(_) => "a list",
        Value::Object(_) | Value::Null => "an object",
        Value::String(_) => "a string",
        _ => "a boolean",
    }
}

fn wrong(v: &Value) -> bool {
    !v.is_null() && !v.is_number()
}

fn numeric(v: &Value) -> bool {
    v.as_str().is_some_and(|s| !js_trim(s).is_empty() && to_number(s).is_finite())
}

fn number_field(v: &Value) -> Option<String> {
    v.as_object().and_then(|fields| fields.iter().find(|(_, x)| x.is_number()).map(|(k, _)| k.clone()))
}

fn binding_text(raw: &Value) -> &str {
    js_trim(raw["bind"].as_str().unwrap_or_default())
}

fn plot_problems(doc: &SlateDoc, reader: &OwnReader) -> Vec<SlateProblem> {
    let read = reader.context(None, None);
    let mut out = Vec::new();
    for (id, piece) in &doc.pieces {
        let Some(module) = slate_piece_module(&piece.kind) else { continue };
        let prop_value = |name: &str| piece.props.as_ref().and_then(|p| p.get(name));
        let items_or_null = prop_value("items").cloned().unwrap_or(Value::Null);
        let push = |out: &mut Vec<SlateProblem>, code: &str, message: String, prop: &str, fix: Option<String>| {
            let mut p = slate_problem(code, &message, fix.as_deref());
            p.piece = Some(id.clone());
            p.prop = Some(prop.to_string());
            out.push(p);
        };

        for (prop, spec) in &module.props {
            let Some(raw) = prop_value(prop).filter(|r| is_slate_binding(r)) else { continue };
            let Some(most) = &spec.most else { continue };
            if let Some(list) = resolve_slate_prop(raw, &read).as_ref().and_then(Value::as_array) {
                if list.len() > most.count {
                    let mut p = slate_list_too_long(list.len(), most.count, most.noun, Some(binding_text(raw)));
                    p.piece = Some(id.clone());
                    p.prop = Some(prop.to_string());
                    out.push(p);
                }
            }
        }

        for (kind_tag, item) in module.items.iter().filter(|(_, item)| item.row) {
            let Some(entries) = prop_value(item.prop).and_then(Value::as_array) else { continue };
            for entry in entries {
                for (field, _) in item.fields.iter().filter(|(_, f)| f.prop_type == SlatePropType::Number && f.binds == SlateBinds::Item) {
                    let Some(raw) = entry.as_object().and_then(|m| m.get(*field)).filter(|r| is_slate_binding(r)) else { continue };
                    let items = resolve_slate_prop(&items_or_null, &read);
                    let Some(rows) = items.as_ref().and_then(Value::as_array) else { continue };
                    let bad: Vec<Value> = rows
                        .iter()
                        .enumerate()
                        .filter_map(|(index, row)| resolve_slate_prop(raw, &reader.context(Some(row.clone()), Some(index as i64))))
                        .filter(wrong)
                        .collect();
                    if let Some(first) = bad.first() {
                        let message = format!(
                            "<{kind_tag} {field}={{{}}}> read {} on {} of {} rows, like {}, so it draws no point for them; a plotted value is a number",
                            binding_text(raw),
                            kind_of(first),
                            bad.len(),
                            rows.len(),
                            shown_value(Some(first))
                        );
                        push(&mut out, "R905", message, item.prop, None);
                    }
                }
            }
        }

        for (prop, spec) in &module.props {
            let Some(raw) = prop_value(prop).filter(|r| is_slate_binding(r)) else { continue };
            let bind = binding_text(raw);
            let meant = |got: &Value, list: bool| -> Option<String> {
                if let Some(key) = number_field(got) {
                    return Some(if list { format!("pluck({bind}, '{key}')") } else { format!("{bind}.{key}") });
                }
                (!list && numeric(got)).then(|| format!("num({bind})"))
            };
            let add = |out: &mut Vec<SlateProblem>, said: String, rule: &str, got: &Value, list: bool| {
                let fix = meant(got, list);
                let like = fix.as_ref().map(|f| format!(", like {prop}={{{f}}}")).unwrap_or_default();
                let message = format!("{prop}={{{bind}}} {said}; {rule}{like}");
                push(out, "R905", message, prop, fix.map(|f| format!("{prop}={{{f}}}")));
            };
            let where_ = |n: usize, rows: Option<&Vec<Value>>| match rows {
                Some(rows) => format!(" on {n} of {} rows, like", rows.len()),
                None => ",".to_string(),
            };

            if spec.of_number {
                if let Some(list) = resolve_slate_prop(raw, &read).as_ref().and_then(Value::as_array) {
                    let bad: Vec<Value> = list.iter().filter(|v| wrong(v)).cloned().collect();
                    if let Some(first) = bad.first() {
                        let said = format!(
                            "holds {} in {} of {} places, like {}, so it draws no point for them",
                            kind_of(first),
                            bad.len(),
                            list.len(),
                            shown_value(Some(first))
                        );
                        add(&mut out, said, "a plotted list holds numbers", first, true);
                    }
                }
            } else if spec.time {
                let items = if spec.binds == SlateBinds::Item {
                    Some(resolve_slate_prop(&items_or_null, &read).unwrap_or(Value::Null))
                } else {
                    None
                };
                let rows = items.as_ref().and_then(Value::as_array);
                let got: Vec<Option<Value>> = match rows {
                    Some(rows) => rows
                        .iter()
                        .enumerate()
                        .map(|(index, row)| resolve_slate_prop(raw, &reader.context(Some(row.clone()), Some(index as i64))))
                        .collect(),
                    None => vec![resolve_slate_prop(raw, &read)],
                };
                let bad: Vec<Value> = got.iter().flatten().filter(|v| !v.is_null() && slate_time(Some(*v)).is_none()).cloned().collect();
                if let Some(first) = bad.first() {
                    let message = format!(
                        "{prop}={{{bind}}} read {}{} {}, which is not a time, so it draws no span; a time is ISO or ms",
                        kind_of(first),
                        where_(bad.len(), rows),
                        shown_value(Some(first))
                    );
                    push(&mut out, "R905", message, prop, None);
                }
                let early: Vec<f64> = got
                    .iter()
                    .flatten()
                    .filter(|v| v.is_number() && slate_time(Some(*v)).is_some())
                    .filter_map(|v| v.as_f64().filter(|t| *t < SLATE_TIME_FLOOR))
                    .collect();
                if let Some(first) = early.first() {
                    let message = format!(
                        "{prop}={{{bind}}} read{} {}, a time before 2000 that is most likely seconds where a slate reads milliseconds, like {prop}={{{bind} * 1000}}",
                        where_(early.len(), rows),
                        number_string(*first)
                    );
                    push(&mut out, "W021", message, prop, Some(format!("{prop}={{{bind} * 1000}}")));
                }
            } else if matches!(spec.prop_type, SlatePropType::Number | SlatePropType::Integer) && spec.binds == SlateBinds::Item {
                let items = resolve_slate_prop(&items_or_null, &read).unwrap_or(Value::Null);
                if let Some(rows) = items.as_array() {
                    let bad: Vec<Value> = rows
                        .iter()
                        .enumerate()
                        .filter_map(|(index, row)| resolve_slate_prop(raw, &reader.context(Some(row.clone()), Some(index as i64))))
                        .filter(wrong)
                        .collect();
                    if let Some(first) = bad.first() {
                        let said = format!(
                            "read {} on {} of {} rows, like {}, so it draws no point for them",
                            kind_of(first),
                            bad.len(),
                            rows.len(),
                            shown_value(Some(first))
                        );
                        add(&mut out, said, "a plotted value is a number", first, false);
                    }
                }
            } else if matches!(spec.prop_type, SlatePropType::Number | SlatePropType::Integer) && spec.binds == SlateBinds::Yes {
                if let Some(value) = resolve_slate_prop(raw, &read).filter(wrong) {
                    let said = format!("read {}, {}, so it draws nothing", kind_of(&value), shown_value(Some(&value)));
                    add(&mut out, said, "a plotted value is a number", &value, false);
                }
            }
        }
    }
    out
}

fn unrun_reads(doc: &SlateDoc, values: &SlateValues, piece: &SlatePiece) -> Vec<String> {
    let mut paths: Vec<String> = piece.props.iter().flatten().flat_map(|(_, v)| slate_prop_dependencies(Some(v))).collect();
    if let Some(when) = &piece.when {
        paths.extend(slate_dependencies(when));
    }
    let mut runs = IndexSet::new();
    for path in paths {
        let Some(own) = parse_slate_own_path(&path) else { continue };
        if !doc.runs.contains_key(&own.name) {
            continue;
        }
        if let Some(first) = own.segs.first() {
            if !matches!(first, PathSeg::Key(key) if RESULT_FIELDS.contains(&key.as_str())) {
                continue;
            }
        }
        let rec = values.get(&own.name);
        if rec.and_then(|r| r.get("endedAt")).is_none() {
            runs.insert(format!("${}, not run yet", own.name));
        } else if rec.and_then(|r| r.get("state")).and_then(Value::as_str) == Some("failed") {
            runs.insert(format!("${}, which failed", own.name));
        }
    }
    runs.into_iter().collect()
}

struct PieceView {
    id: String,
    piece: SlatePiece,
    doc: Rc<SlateDoc>,
    values: Rc<SlateValues>,
    reader: OwnReader,
    unbound: bool,
    reads: RefCell<IndexSet<String>>,
}

impl PieceView {
    fn note(&self, name: &str) {
        self.reads.borrow_mut().insert(name.to_string());
    }

    fn prop_value(&self, name: &str) -> Option<&Value> {
        self.piece.props.as_ref()?.get(name)
    }
}

impl SlateSketchView for PieceView {
    fn id(&self) -> &str {
        &self.id
    }

    fn prop(&self, name: &str) -> Option<Value> {
        self.note(name);
        let v = self.prop_value(name)?;
        if self.unbound {
            Some(braces(v))
        } else {
            resolve_slate_prop(v, &self.reader.context(None, None))
        }
    }

    fn raw(&self, name: &str) -> Option<Value> {
        self.note(name);
        self.prop_value(name).cloned()
    }

    fn bound(&self, name: &str) -> bool {
        self.note(name);
        self.prop_value(name).is_some_and(is_bound)
    }

    fn row(&self, spec: &Value, item: &Value, index: i64) -> IndexMap<String, Value> {
        let Some(fields) = spec.as_object().filter(|_| !is_bound(spec)) else { return IndexMap::new() };
        let ctx = self.reader.context(Some(item.clone()), Some(index));
        fields
            .iter()
            .filter(|(k, _)| k.as_str() != "on" && k.as_str() != "when")
            .filter_map(|(k, v)| {
                let value = if self.unbound { Some(braces(v)) } else { resolve_slate_prop(v, &ctx) };
                value.map(|x| (k.clone(), x))
            })
            .collect()
    }

    fn test(&self, expr: &str, item: &Value, index: i64) -> bool {
        self.unbound || slate_truthy(evaluate_slate_expression(expr, &self.reader.context(Some(item.clone()), Some(index))).as_ref())
    }

    fn read(&self, path: &str) -> Option<Value> {
        let own = parse_slate_own_path(path)?;
        own.segs.iter().fold(self.values.get(&own.name).cloned(), slate_step)
    }

    fn run_kind(&self, path: &str) -> Option<&'static str> {
        let own = parse_slate_own_path(path)?;
        if !own.segs.is_empty() {
            return None;
        }
        Some(match self.doc.runs.get(&own.name)? {
            SlateRunDecl::Cmd(_) => "cmd",
            SlateRunDecl::Tool(_) => "tool",
            SlateRunDecl::Resource(_) => "resource",
        })
    }

    fn unbound(&self) -> bool {
        self.unbound
    }

    fn now(&self) -> f64 {
        self.reader.now() as f64
    }
}

fn tag(id: &str, piece: &SlatePiece, view: &PieceView, look: &[String]) -> String {
    let mut parts = vec![id.to_string(), piece.kind.clone()];
    for (name, default) in TAG_PROPS {
        let bound = view.bound(name);
        let word = if bound && view.unbound { None } else { view.prop(name) };
        if let Some(text) = word.as_ref().and_then(Value::as_str) {
            if text != default {
                parts.push(text.to_string());
            }
        }
    }
    parts.extend(look.iter().cloned());
    format!("[{}]", parts.join(" "))
}

struct Sketch {
    doc: Rc<SlateDoc>,
    values: Rc<SlateValues>,
    reader: OwnReader,
    unbound: bool,
    lines: Vec<String>,
    piece_lines: usize,
    folded: usize,
    seen: HashSet<String>,
}

impl Sketch {
    fn emit(&mut self, indent: &str, body: &str, label: &str) {
        if self.piece_lines >= SLATE_LIMITS.sketch_piece_lines {
            self.folded += 1;
            return;
        }
        self.piece_lines += 1;
        let room = SLATE_LIMITS.sketch_columns as i64 - utf16::len(indent) as i64 - utf16::len(label) as i64 - 2;
        let line = if body.is_empty() {
            format!("{indent}{label}")
        } else {
            let gap = if body == "(hidden)" { " " } else { "  " };
            format!("{indent}{}{gap}{label}", cut(body, room.max(8)))
        };
        self.lines.push(line);
    }

    fn look_words(&self, piece: &SlatePiece, props: &IndexMap<&str, SlatePropSpec>, reads: &IndexSet<String>) -> Vec<String> {
        let ctx = self.reader.context(None, None);
        let mut words = Vec::new();
        for (name, spec) in props {
            if spec.unseen.is_some() || spec.binds == SlateBinds::Item || reads.contains(*name) || TAG_PROPS.iter().any(|(t, _)| t == name)
            {
                continue;
            }
            let Some(raw) = piece.props.as_ref().and_then(|p| p.get(*name)) else { continue };
            if raw.is_null() || (self.unbound && is_bound(raw)) {
                continue;
            }
            let value = resolve_slate_prop(raw, &ctx);
            if *name == "held" && slate_held_text(value.as_ref()).is_none() {
                continue;
            }
            if let Some(default) = &spec.default {
                if slate_equal(value.as_ref(), Some(default)) {
                    continue;
                }
            }
            match value {
                Some(Value::Bool(true)) => words.push(name.to_string()),
                None | Some(Value::Null) | Some(Value::Bool(false)) => {}
                Some(v) => {
                    let text = v.as_str().map_or_else(|| json_stringify(&v), str::to_string);
                    words.push(format!("{name}={}", cut(&text, 24)));
                }
            }
        }
        words
    }

    fn view(&self, id: &str, piece: &SlatePiece) -> PieceView {
        PieceView {
            id: id.to_string(),
            piece: piece.clone(),
            doc: Rc::clone(&self.doc),
            values: Rc::clone(&self.values),
            reader: self.reader.clone(),
            unbound: self.unbound,
            reads: RefCell::new(IndexSet::new()),
        }
    }

    fn walk(&mut self, id: &str, depth: usize) {
        let doc = Rc::clone(&self.doc);
        let Some(piece) = doc.pieces.get(id) else { return };
        if !self.seen.insert(id.to_string()) {
            return;
        }
        let indent = "  ".repeat(depth);
        let view = self.view(id, piece);
        if let Some(when) = &piece.when {
            if !self.unbound && !slate_truthy(evaluate_slate_expression(when, &self.reader.context(None, None)).as_ref()) {
                let label = tag(id, piece, &view, &[]);
                self.emit(&indent, "(hidden)", &label);
                return;
            }
        }
        let Some(module) = slate_piece_module(&piece.kind) else {
            let fallback = piece.fallback.as_ref();
            if fallback.and_then(Value::as_str) != Some("drop") {
                let text = match fallback {
                    Some(obj @ Value::Object(_)) => obj.get("text").and_then(Value::as_str).unwrap_or_default().to_string(),
                    _ => "This part needs a newer wsp".to_string(),
                };
                self.emit(&indent, &text, &format!("[{id} {}]", piece.kind));
            }
            return;
        };

        let mut out = (module.sketch)(&view);
        let unrun = if self.unbound { Vec::new() } else { unrun_reads(&doc, &self.values, piece) };
        if !unrun.is_empty() {
            let first = out.first().filter(|f| !f.contains("not read yet") && !f.contains("not run yet"));
            let replaced = first.map(|f| js_trim_start(&format!("{f} (reads {})", unrun.join("; "))).to_string());
            if let Some(text) = replaced {
                out[0] = text;
            }
        }
        let look = self.look_words(piece, &module.props, &view.reads.borrow());
        let transparent = module.holds_children && out.iter().all(|l| l.is_empty()) && look.is_empty();
        if !transparent {
            let first = out.first().cloned().unwrap_or_default();
            let label = tag(id, piece, &view, &look);
            self.emit(&indent, &first, &label);
            let room = SLATE_LIMITS.sketch_columns as i64 - utf16::len(&indent) as i64;
            for more in out.iter().skip(1) {
                if self.piece_lines >= SLATE_LIMITS.sketch_piece_lines {
                    break;
                }
                self.piece_lines += 1;
                self.lines.push(format!("{indent}{}", cut(more, room)));
            }
        }
        if module.collapsed.is_some_and(|collapsed| collapsed(&view)) {
            return;
        }
        let child_depth = if transparent { depth } else { depth + 1 };
        for child in piece.children.iter().flatten() {
            self.walk(child, child_depth);
        }
    }
}
