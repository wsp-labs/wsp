// SPDX-License-Identifier: AGPL-3.0-only
//! A run's output as it streams, after `apps/web/src/slate/pieces/output.tsx`: a row with its label and its state as
//! one word, Cancel in the slot while it runs, then the tail of its lines in 12 px mono, `lines` rows tall, following
//! the end while it runs; why under them when held or failed. A tool or resource run's result draws by its shape
//! (`shape.tsx`) through the kit's own text, facts and table pieces.

use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _};
use gpui_kit::{div, prelude::*, px, AnyElement, App, FontWeight, Hsla, Pixels, ScrollHandle, SharedString, Window};
use serde_json::{Map, Value};

use super::layout::{place_of, Place};
use super::lists::{table_view, tight_widths, Column, Table, TableRow};
use super::text::{facts_grid, Fact};
use super::{quiet, PieceCx};
use crate::ui::{spinner, ActivePalette, Button, Crab};

/// A run state as the status mark's grammar says it (`RUN_WORDS`).
pub fn run_word(state: &str) -> &'static str {
    match state {
        "held" => "Held",
        "running" => "Running",
        "done" => "Done",
        "failed" => "Failed",
        "cancelled" => "Cancelled",
        _ => "Not run yet",
    }
}

/// `RUN_INK`.
pub fn run_ink(state: &str, cx: &App) -> Hsla {
    let p = cx.palette();
    match state {
        "held" => p.status_input,
        "running" => p.status_working,
        "failed" => p.status_failed,
        _ => p.muted_foreground,
    }
}

/// The slate's `--settings-inset`.
const INSET: Pixels = px(16.);
/// The rows a log is tall without `lines`, and the bounds `lines` is held to.
const LINES: (f64, f64, f64) = (8., 3., 40.);
/// `SLATE_RESULT_ROWS` and the columns a result table takes at most.
pub const RESULT_ROWS: usize = 50;
const RESULT_COLUMNS: usize = 6;

/// The run an output names: `$quick` written as `{$quick}`.
pub fn run_name(v: Option<&Value>) -> Option<String> {
    let text = match v? {
        Value::Object(o) if o.len() == 1 => o.get("bind")?.as_str()?,
        Value::String(s) => s.as_str(),
        _ => return None,
    };
    let name = text.trim().strip_prefix('$')?;
    (!name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')).then(|| name.to_owned())
}

fn text_of(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => serde_json::to_string_pretty(other).unwrap_or_default(),
    }
}

/// The record's own lines, else the tail of out and err split at line ends.
pub fn record_lines(record: &Map<String, Value>) -> Vec<String> {
    if let Some(lines) = record.get("lines").and_then(Value::as_array) {
        return lines.iter().map(|l| l.as_str().map_or_else(|| l.to_string(), str::to_owned)).collect();
    }
    let parts: Vec<String> = [text_of(record.get("out")), text_of(record.get("err"))].into_iter().filter(|p| !p.is_empty()).collect();
    let text = parts.join("\n");
    if text.is_empty() {
        return Vec::new();
    }
    text.strip_suffix('\n').unwrap_or(&text).split('\n').map(str::to_owned).collect()
}

pub fn draw(p: &PieceCx, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    let Some(run) = run_name(p.piece.prop("run")) else { return Some(quiet(run_word("idle"), cx)) };
    let slate = p.slate;
    let record = slate.record.run(&run);
    let state = record
        .and_then(|r| r.get("state"))
        .and_then(Value::as_str)
        .filter(|s| matches!(*s, "held" | "running" | "done" | "failed" | "cancelled"))
        .unwrap_or("idle");
    let lines: Vec<String> = slate.lines.get(&run).cloned().unwrap_or_else(|| record.map(record_lines).unwrap_or_default());
    // A refresh draws the last result with a quiet mark, never the running row's crab and Cancel.
    let refreshing = slate.record.refreshing(&run);
    let running = state == "running" && !refreshing;
    let stale = record.and_then(|r| r.get("stale")).and_then(Value::as_bool) == Some(true) && !running;
    let why = record.filter(|_| matches!(state, "held" | "failed")).and_then(|r| r.get("why")).and_then(Value::as_str).map(str::to_owned);
    let kind = slate.doc().and_then(|d| d.runs.get(&run)).and_then(|d| d.get("kind")).and_then(Value::as_str).unwrap_or("cmd");
    let result = record
        .filter(|_| kind != "cmd" && state != "failed")
        .and_then(|r| r.get("json").filter(|v| !v.is_null()).or_else(|| r.get("out")))
        .filter(|v| !v.is_null())
        .cloned();
    let place = place_of(slate, p.id());
    let inset = if place == Place::Inside { px(0.) } else { INSET };
    let pal = cx.palette().clone();
    let ink = if refreshing { pal.muted_foreground } else { run_ink(state, cx) };
    let mono = cx.theme().mono_font_family.clone();

    let screen = p.screen.clone();
    let cancel_run = run.clone();
    let head = h_flex()
        .min_h(px(44.))
        .min_w_0()
        .gap(px(12.))
        .py(px(12.))
        .px(inset)
        .children(p.str("label").map(|l| div().min_w_0().text_size(px(14.)).line_height(px(20.)).text_color(pal.foreground).child(l)))
        .child(div().flex_1())
        .child(
            h_flex()
                .flex_none()
                .gap(px(6.))
                .text_size(px(13.))
                .line_height(px(20.))
                .whitespace_nowrap()
                .text_color(ink)
                .when(matches!(state, "failed" | "held"), |el| el.font_weight(FontWeight::MEDIUM))
                .debug_selector({
                    let (run, state) = (run.clone(), if refreshing { "refreshing" } else { state });
                    move || format!("slate-run-{run}-{state}")
                })
                .when(running, |el| el.child(Crab::new(pal.status_working).key(p.el("crab"))))
                .child(if refreshing { spinner(cx).into_any_element() } else { div().child(run_word(state)).into_any_element() }),
        )
        .when(stale, |el| {
            el.child(div().min_w_0().text_size(px(13.)).text_color(pal.muted_foreground).child("from before the command changed"))
        })
        .when(running, |el| {
            let selector = format!("slate-cancel-{run}");
            el.child(div().debug_selector(move || selector.clone()).child(Button::outline(p.el("cancel"), "Cancel").xs().on_click(
                move |_, _, cx| {
                    let run = cancel_run.clone();
                    _ = screen.update(cx, |s, cx| s.cancel(run, cx));
                },
            )))
        });

    let mut parts: Vec<AnyElement> = vec![head.into_any_element()];
    if let Some(result) = result {
        let body = shape(p, &shape_of(&result, 0), "result", window, cx);
        parts.push(
            div().min_w_0().py(px(12.)).px(inset).debug_selector(move || format!("slate-result-{run}")).children(body).into_any_element(),
        );
    } else if !(lines.is_empty() && state == "idle") {
        let rows = p.num("lines").map_or(LINES.0, |n| n.floor().clamp(LINES.1, LINES.2));
        let follow = window.use_keyed_state(p.el("log"), cx, |_, _| (ScrollHandle::new(), 0usize));
        let (handle, seen) = follow.read(cx).clone();
        if running && seen != lines.len() {
            handle.scroll_to_bottom();
            follow.update(cx, |f, _| f.1 = lines.len());
        }
        let wrap = p.bool("wrap");
        let log = v_flex()
            .id(p.el("log-box"))
            .track_scroll(&handle)
            .when(wrap, |el| el.overflow_y_scroll())
            .when(!wrap, |el| el.overflow_scroll())
            .max_h(px((rows * 16. + 20.) as f32))
            .py(px(10.))
            .px(inset)
            .font_family(mono)
            .text_size(px(12.))
            .line_height(px(16.))
            .text_color(if stale { pal.muted_foreground } else { pal.foreground })
            .debug_selector(|| "slate-log".into())
            .children(lines.into_iter().map(|line| {
                let line = if line.is_empty() { " ".to_owned() } else { line };
                div().when(!wrap, |el| el.whitespace_nowrap().flex_none()).child(SharedString::from(line))
            }));
        parts.push(log.into_any_element());
    }
    if let Some(why) = why {
        parts.push(div().py(px(10.)).px(inset).debug_selector(|| "slate-why".into()).child(quiet(why, cx)).into_any_element());
    }
    if place == Place::Page {
        return super::layout::card(p.el("card"), parts.into_iter().map(|part| (true, part)).collect());
    }
    let rule = pal.border.opacity(0.5);
    let count = parts.len();
    Some(
        v_flex()
            .min_w_0()
            .children(
                parts
                    .into_iter()
                    .enumerate()
                    .map(|(ix, part)| div().when(ix > 0 && count > 1, |el| el.border_t_1().border_color(rule)).child(part)),
            )
            .into_any_element(),
    )
}

/// `SlateResultShape`: the protocol's one decision for how a result draws.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    None,
    Text(String),
    Table { rows: Vec<Map<String, Value>>, columns: Vec<(String, String)> },
    Record { facts: Vec<(String, String)>, nested: Vec<(String, Shape)> },
}

/// `slateFieldWords`: `receivedTime` and `received_time` read "Received time".
pub fn field_words(key: &str) -> String {
    let mut spaced = String::new();
    let mut prev: Option<char> = None;
    for c in key.chars() {
        if c.is_ascii_uppercase() && prev.is_some_and(|p| p.is_ascii_lowercase() || p.is_ascii_digit()) {
            spaced.push(' ');
        }
        spaced.push(c);
        prev = Some(c);
    }
    let mut out = String::new();
    let mut gap = false;
    for c in spaced.chars() {
        if c == '_' || c == '-' {
            gap = true;
            continue;
        }
        if gap {
            out.push(' ');
            gap = false;
        }
        out.push(c);
    }
    let words = out.trim().to_lowercase();
    let mut chars = words.chars();
    match chars.next() {
        None => key.to_owned(),
        Some(first) => first.to_uppercase().chain(chars).collect(),
    }
}

fn is_scalar(v: &Value) -> bool {
    matches!(v, Value::Null | Value::String(_) | Value::Number(_) | Value::Bool(_))
}

fn scalar_text(v: &Value) -> String {
    match v {
        Value::Null => "null".to_owned(),
        Value::String(s) => s.clone(),
        Value::Number(n) => super::look::num_text(n.as_f64().unwrap_or_default()),
        other => other.to_string(),
    }
}

fn is_path_key(key: &str) -> bool {
    let mut chars = key.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub fn shape_of(value: &Value, depth: usize) -> Shape {
    match value {
        Value::Null => Shape::None,
        Value::String(s) if s.is_empty() => Shape::None,
        v if is_scalar(v) => Shape::Text(scalar_text(v)),
        Value::Array(list) => {
            let rows: Vec<Map<String, Value>> = list.iter().filter_map(|v| v.as_object().cloned()).collect();
            if !list.is_empty() && rows.len() == list.len() {
                let mut keys: Vec<String> = Vec::new();
                'rows: for row in rows.iter().take(RESULT_ROWS) {
                    for (key, v) in row {
                        if keys.len() >= RESULT_COLUMNS {
                            break 'rows;
                        }
                        let plain = is_scalar(v) && !v.is_null() && v.as_str() != Some("");
                        if !keys.contains(key) && is_path_key(key) && plain {
                            keys.push(key.clone());
                        }
                    }
                }
                let columns = keys.into_iter().map(|k| (field_words(&k), k)).collect();
                return Shape::Table { rows, columns };
            }
            if list.iter().all(is_scalar) {
                return if list.is_empty() {
                    Shape::None
                } else {
                    Shape::Text(list.iter().map(scalar_text).collect::<Vec<_>>().join("\n"))
                };
            }
            Shape::Text(value.to_string())
        }
        Value::Object(o) => {
            let (mut facts, mut nested) = (Vec::new(), Vec::new());
            for (key, v) in o {
                let plain_list = v.as_array().is_some_and(|l| l.iter().all(is_scalar));
                if is_scalar(v) || plain_list {
                    let shown = match v {
                        Value::Array(l) => l.iter().map(scalar_text).collect::<Vec<_>>().join(", "),
                        Value::Null => String::new(),
                        other => scalar_text(other),
                    };
                    if !shown.is_empty() {
                        facts.push((field_words(key), shown));
                    }
                } else if depth + 1 >= 2 {
                    facts.push((field_words(key), v.to_string()));
                } else {
                    nested.push((field_words(key), shape_of(v, depth + 1)));
                }
            }
            Shape::Record { facts, nested }
        }
        _ => Shape::None,
    }
}

/// The result by its shape through the kit's own table and facts views, standing inside the output's row.
fn shape(p: &PieceCx, shape: &Shape, part: &str, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    let pal = cx.palette().clone();
    match shape {
        Shape::None => None,
        Shape::Text(text) => {
            Some(div().min_w_0().text_size(px(14.)).line_height(px(20.)).text_color(pal.foreground).child(text.clone()).into_any_element())
        }
        Shape::Table { rows, columns } => {
            let heads: Vec<Column> =
                columns.iter().map(|(title, _)| Column { title: Some(title.clone()), mono: false, align: None }).collect();
            let shown: Vec<TableRow> = rows
                .iter()
                .take(RESULT_ROWS)
                .enumerate()
                .map(|(index, row)| TableRow {
                    key: index.to_string(),
                    item: Value::Object(row.clone()),
                    index,
                    cells: columns.iter().map(|(_, key)| row.get(key).cloned()).collect(),
                    tones: vec![None; columns.len()],
                    actions: Vec::new(),
                })
                .collect();
            let cells: Vec<Vec<Option<Value>>> = shown.iter().map(|r| r.cells.clone()).collect();
            let widths = tight_widths(&heads, &cells);
            let table = Table {
                columns: heads,
                rows: shown,
                total: rows.len(),
                empty: None,
                place: Place::Inside,
                widths,
                opens: false,
                actions: 0,
                in_section: false,
                hovers: Vec::new(),
            };
            Some(table_view(p, part, table, window, cx))
        }
        Shape::Record { facts, nested } => {
            let list: Vec<Fact> = facts
                .iter()
                .map(|(label, value)| Fact {
                    label: label.clone(),
                    value: value.clone(),
                    tone: None,
                    quiet: false,
                    strong: false,
                    mono: false,
                })
                .collect();
            let mut parts: Vec<AnyElement> = Vec::new();
            if !list.is_empty() {
                parts.push(facts_grid(p.el(&format!("{part}-facts")), &list, Place::Inside, window, cx));
            }
            for (at, (title, inner)) in nested.iter().enumerate() {
                let body = self::shape(p, inner, &format!("{part}-{at}"), window, cx);
                parts.push(
                    v_flex()
                        .min_w_0()
                        .gap(px(4.))
                        .child(div().text_size(px(12.)).line_height(px(16.)).text_color(pal.muted_foreground).child(title.clone()))
                        .children(body)
                        .into_any_element(),
                );
            }
            Some(v_flex().min_w_0().gap(px(12.)).children(parts).into_any_element())
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_result_draws_by_its_shape() {
        assert_eq!(shape_of(&json!(null), 0), Shape::None);
        assert_eq!(shape_of(&json!(["a", 2]), 0), Shape::Text("a\n2".into()));
        let Shape::Table { rows, columns } = shape_of(&json!([{"receivedTime": 1, "name": "x", "deep": {}}]), 0) else { panic!() };
        assert_eq!(rows.len(), 1);
        assert_eq!(columns, vec![("Received time".to_owned(), "receivedTime".to_owned()), ("Name".to_owned(), "name".to_owned())]);
        let Shape::Record { facts, nested } = shape_of(&json!({"a_b": "1", "list": [1, 2], "inner": {"x": {"y": 1}}}), 0) else { panic!() };
        assert_eq!(facts, vec![("A b".to_owned(), "1".to_owned()), ("List".to_owned(), "1, 2".to_owned())]);
        assert_eq!(nested[0].1, Shape::Record { facts: vec![("X".into(), r#"{"y":1}"#.into())], nested: Vec::new() });
    }

    #[test]
    fn an_output_names_its_run() {
        assert_eq!(run_name(Some(&json!({"bind": "$quick"}))), Some("quick".into()));
        assert_eq!(run_name(Some(&json!({"bind": "$quick.out"}))), None);
        let rec = json!({"out": "a\nb\n", "err": "c"});
        assert_eq!(
            record_lines(rec.as_object().unwrap()),
            vec!["a", "b", "", "c"],
            "out and err joined by a line end, as the web joins them"
        );
    }
}
