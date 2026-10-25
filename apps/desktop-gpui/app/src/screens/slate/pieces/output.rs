// SPDX-License-Identifier: AGPL-3.0-only
//! A run's output summary first, after `apps/web/src/slate/pieces/output.tsx`: a row with its label and its state as
//! one word, Cancel in the slot while it runs, then what passed and failed as the runner tallied it and the lines that
//! name a failure, or the last line where no tally reads; the raw lines fold under "All N lines", 12 px mono and
//! `lines` rows tall once open, following the end while it runs. Why under them when held or failed. A tool or
//! resource run's result draws by its shape (`shape.tsx`) through the kit's own text, facts and table pieces.

use std::sync::LazyLock;

use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, AnyElement, App, FontWeight, Hsla, Pixels, ScrollHandle, SharedString, Transformation, Window};
use regex::Regex;
use serde_json::{Map, Value};

use super::layout::{place_of, Place};
use super::lists::{table_view, tight_widths, Column, Table, TableRow};
use super::look;
use super::text::{facts_grid, Fact};
use super::{quiet, PieceCx};
use crate::ui::{motion, spinner, ActivePalette, Button, Crab, Glyph};

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
                .child(if refreshing {
                    spinner(cx).size(px(12.)).key(p.el("spinner")).into_any_element()
                } else {
                    div().child(run_word(state)).into_any_element()
                }),
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
    } else if !lines.is_empty() {
        parts.push(summary(&slate_run_summary(&lines), stale, inset, cx));
        let fold = format!("{}/lines", p.id());
        let open = slate.folds.get(&fold).copied().unwrap_or(false);
        let turn = motion::ease_px(p.el("lines-chevron"), "turn", px(if open { 0.25 } else { 0. }), motion::COLORS, window, cx);
        let screen = p.screen.clone();
        let toggle = div()
            .id(p.el("lines"))
            .debug_selector(move || format!("slate-fold-{run}{}", if open { "-open" } else { "" }))
            .flex()
            .items_center()
            .gap(px(4.))
            .h(px(28.))
            .cursor_pointer()
            .text_size(px(12.))
            .line_height(px(16.))
            .text_color(pal.muted_foreground)
            .hover(|el| el.text_color(pal.foreground))
            .on_click(move |_, _, cx| {
                let fold = fold.clone();
                _ = screen.update(cx, |s, cx| s.fold(fold, !open, cx));
            })
            .child(
                Icon::new(Glyph::ChevronRight)
                    .with_size(px(14.))
                    .flex_none()
                    .transform(Transformation::rotate(gpui_kit::percentage(f32::from(turn)))),
            )
            .child(format!(
                "All {} line{}",
                look::slate_figure(lines.len() as f64, "integer", None),
                if lines.len() == 1 { "" } else { "s" }
            ));
        let log = open.then(|| {
            let rows = p.num("lines").map_or(LINES.0, |n| n.floor().clamp(LINES.1, LINES.2));
            let follow = window.use_keyed_state(p.el("log"), cx, |_, _| (ScrollHandle::new(), 0usize));
            let (handle, seen) = follow.read(cx).clone();
            // `running` leaves a refresh out, and a refresh still streams lines to follow.
            if state == "running" && seen != lines.len() {
                handle.scroll_to_bottom();
                follow.update(cx, |f, _| f.1 = lines.len());
            }
            let wrap = p.bool("wrap");
            v_flex()
                .id(p.el("log-box"))
                .track_scroll(&handle)
                .when(wrap, |el| el.overflow_y_scroll())
                .when(!wrap, |el| el.overflow_scroll())
                .max_h(px((rows * 16. + 10.) as f32))
                .pt(px(4.))
                .pb(px(6.))
                .font_family(mono)
                .text_size(px(12.))
                .line_height(px(16.))
                .text_color(if stale { pal.muted_foreground } else { pal.foreground })
                .debug_selector(|| "slate-log".into())
                .children(lines.into_iter().map(move |line| {
                    let line = if line.is_empty() { " ".to_owned() } else { line };
                    div().when(!wrap, |el| el.whitespace_nowrap().flex_none()).child(SharedString::from(line))
                }))
        });
        parts.push(v_flex().min_w_0().py(px(6.)).px(inset).child(toggle).children(log).into_any_element());
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

/// `SlateRunSummary` in `protocol:slate/output.ts`: a run's lines as the runner tallied them, the lines that name a
/// failure, and the last line.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunSummary {
    pub passed: Option<u64>,
    pub failed: Option<u64>,
    pub skipped: Option<u64>,
    /// Failure and error lines, trimmed, in order, at most `SUMMARY_LINES`.
    pub matters: Vec<String>,
    /// The last line that is not blank, for output no runner's tally reads.
    pub last: Option<String>,
}

/// `SLATE_SUMMARY_LINES`.
pub const SUMMARY_LINES: usize = 5;

// JavaScript reads `\d`, `\w` and `\b` as ASCII and `$` before a `\r` too, so these spell them out to read alike.
static TALLY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)([0-9][0-9,]*) (passed|passing|failed|failing|failures|errors?|skipped|pending|ignored|todo)(?-u:\b)").unwrap()
});
/// cargo prints one per test binary, so a run of several crates sums them.
static CARGO: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*test result: [0-9A-Za-z_]+\. ").unwrap());
/// `RUNNER_FAILURE`, then `NAMED_FAILURE`: the lines a runner or a language prints on its own, then the ones that only
/// name a failure inside a log; the summary reads both.
static MATTERS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?mR)^\s*FAIL +(?:\|[^|]+\| +)?\S+\.[0-9A-Za-z_]+|--- FAIL|\.\.\. FAILED$|^\s*error(?:\[[0-9A-Za-z_]+\])?:",
        r"|^\s*[0-9A-Za-z_.]*(?:Error|Exception)(?:\[[0-9A-Za-z_]+\])?:|(?-u:\b)panicked at(?-u:\b)",
        r"|^Traceback \(most recent call last\)|(?-u:\b)npm ERR!|(?-u:\b)error TS[0-9]+:",
        r"|^\s*(?:FAIL(?-u:\b)|FAILED(?-u:\b)|ERROR(?-u:\b)|[×✗✖❌]|[0-9]+\) )|(?-u:\b)error(?:\[[0-9A-Za-z_]+\])?:",
        r"|(?-u:\b)AssertionError(?-u:\b)",
    ))
    .unwrap()
});
/// `FAIL_NAMES` and `MARK_NAMES`: vitest names a failed test twice, by its mark in the tree and on its FAIL line after;
/// the FAIL line, with the file, is the one kept.
static FAIL_NAMES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?R)^\s*FAIL\s+(?:\|[^|]+\|\s+)?\S+ > (.+)$").unwrap());
static MARK_NAMES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?R)^\s*[×✗✖]\s+(.+?)(?:\s+[0-9]+(?:\.[0-9]+)?m?s)?$").unwrap());
static ANSI: LazyLock<Regex> = LazyLock::new(|| Regex::new("\x1b\\[[0-9;]*m").unwrap());
/// `LINE_CHARS`: a failure line or the last line is cut to one line's worth.
const LINE_CHARS: usize = 160;

fn cut(line: &str) -> String {
    if line.chars().count() > LINE_CHARS {
        format!("{}…", line.chars().take(LINE_CHARS - 1).collect::<String>())
    } else {
        line.to_owned()
    }
}

/// One line's counts, by passed, failed and skipped.
fn tally(line: &str) -> [Option<u64>; 3] {
    let mut out = [None; 3];
    for m in TALLY.captures_iter(line) {
        let at = match m[2].to_lowercase().as_str() {
            "passed" | "passing" => 0,
            "failed" | "failing" | "failures" | "error" | "errors" => 1,
            _ => 2,
        };
        let n: u64 = m[1].replace(',', "").parse().unwrap_or(0);
        out[at] = Some(out[at].unwrap_or(0) + n);
    }
    out
}

fn tallies(line: &str) -> bool {
    tally(line).iter().any(Option::is_some)
}

/// `slateRunSummary`: adjacent tally lines merge, the later winning a count both say; every cargo test result line
/// sums.
pub fn slate_run_summary(lines: &[String]) -> RunSummary {
    let clean: Vec<String> = lines.iter().map(|l| ANSI.replace_all(l, "").into_owned()).collect();
    let mut counts = [None; 3];
    let cargo: Vec<&String> = clean.iter().filter(|l| CARGO.is_match(l)).collect();
    if !cargo.is_empty() {
        for line in cargo {
            for (at, n) in tally(line).into_iter().enumerate() {
                if let Some(n) = n {
                    counts[at] = Some(counts[at].unwrap_or(0) + n);
                }
            }
        }
    } else if let Some(end) = clean.iter().rposition(|l| tallies(l)) {
        let mut start = end;
        while start > 0 && tallies(&clean[start - 1]) {
            start -= 1;
        }
        for line in &clean[start..=end] {
            for (at, n) in tally(line).into_iter().enumerate() {
                if n.is_some() {
                    counts[at] = n;
                }
            }
        }
    }
    let fail_named: Vec<&str> = clean.iter().filter_map(|l| FAIL_NAMES.captures(l)).filter_map(|c| c.get(1)).map(|m| m.as_str()).collect();
    let mut matters: Vec<String> = Vec::new();
    for line in &clean {
        let said = cut(line.trim());
        if said.is_empty() || !MATTERS.is_match(line) || matters.contains(&said) || tallies(line) {
            continue;
        }
        if MARK_NAMES.captures(line).and_then(|c| c.get(1)).is_some_and(|m| fail_named.contains(&m.as_str())) {
            continue;
        }
        matters.push(said);
        if matters.len() == SUMMARY_LINES {
            break;
        }
    }
    let last = clean.iter().rev().map(|l| l.trim()).find(|l| !l.is_empty()).map(cut);
    RunSummary { passed: counts[0], failed: counts[1], skipped: counts[2], matters, last }
}

/// `slateSummaryCounts`: the counts as words, failed first and only those above nought.
pub fn summary_counts(s: &RunSummary) -> Vec<(&'static str, String)> {
    [("failed", s.failed), ("passed", s.passed), ("skipped", s.skipped)]
        .into_iter()
        .filter_map(|(word, n)| n.filter(|n| *n > 0).map(|n| (word, format!("{} {word}", look::slate_figure(n as f64, "integer", None)))))
        .collect()
}

/// What the lines come to, ahead of them: the runner's counts, failed first, then the lines that name a failure; the
/// last line alone where neither reads.
fn summary(s: &RunSummary, stale: bool, inset: Pixels, cx: &App) -> AnyElement {
    let pal = cx.palette().clone();
    let counts = summary_counts(s);
    let told = !counts.is_empty() || !s.matters.is_empty();
    let lead: Vec<String> = if told { s.matters.clone() } else { s.last.clone().into_iter().collect() };
    v_flex()
        .debug_selector(|| "slate-summary".into())
        .min_w_0()
        .gap(px(6.))
        .py(px(12.))
        .px(inset)
        .when(!counts.is_empty(), |el| {
            el.child(h_flex().min_w_0().flex_wrap().gap_x(px(16.)).gap_y(px(4.)).text_size(px(14.)).line_height(px(20.)).children(
                counts.into_iter().map(|(count, words)| {
                    let ink = match (stale, count) {
                        (true, _) | (false, "skipped") => pal.muted_foreground,
                        (false, "failed") => pal.error_foreground,
                        _ => pal.foreground,
                    };
                    div()
                        .debug_selector(move || format!("slate-count-{count}"))
                        .text_color(ink)
                        .when(count == "failed" && !stale, |el| el.font_weight(FontWeight::MEDIUM))
                        .child(words)
                }),
            ))
        })
        .when(!lead.is_empty(), |el| {
            el.child(
                v_flex()
                    .debug_selector(|| "slate-matters".into())
                    .min_w_0()
                    .gap(px(2.))
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .text_color(if told || stale { pal.muted_foreground } else { pal.foreground })
                    .children(lead.into_iter().map(|line| div().min_w_0().child(line))),
            )
        })
        .into_any_element()
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
                steps: Vec::new(),
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

    /// The protocol's own readings of real and written runner output, which `protocol:test/slate/output.test.ts` holds
    /// `slateRunSummary` to, so the two apps never read one output two ways.
    const READINGS: &str = include_str!("../../../../../../../packages/protocol/test/slate/output/readings.json");
    fn printed(name: &str) -> String {
        match name {
            "vitest.txt" => include_str!("../../../../../../../packages/protocol/test/slate/output/vitest.txt"),
            "vitest-color.txt" => include_str!("../../../../../../../packages/protocol/test/slate/output/vitest-color.txt"),
            "cargo.txt" => include_str!("../../../../../../../packages/protocol/test/slate/output/cargo.txt"),
            "tsc.txt" => include_str!("../../../../../../../packages/protocol/test/slate/output/tsc.txt"),
            "tsc-pretty.txt" => include_str!("../../../../../../../packages/protocol/test/slate/output/tsc-pretty.txt"),
            other => panic!("readings.json names {other}: include it here too"),
        }
        .to_owned()
    }

    #[test]
    fn a_summary_reads_every_output_as_the_protocols_readings_say() {
        let cases: Vec<Value> = serde_json::from_str(READINGS).unwrap();
        assert!(cases.len() > 20);
        for case in cases {
            let name = case["name"].as_str().unwrap();
            let lines: Vec<String> = match case["file"].as_str() {
                Some(file) => printed(file).split('\n').map(str::to_owned).collect(),
                None => case["lines"].as_array().unwrap().iter().map(|l| l.as_str().unwrap().to_owned()).collect(),
            };
            let s = slate_run_summary(&lines);
            let want = &case["summary"];
            let count = |k: &str| want.get(k).and_then(Value::as_u64);
            assert_eq!((s.passed, s.failed, s.skipped), (count("passed"), count("failed"), count("skipped")), "{name}: the counts");
            assert_eq!(json!(s.matters), want["matters"], "{name}: the lines that name a failure");
            assert_eq!(s.last.as_deref(), want.get("last").and_then(Value::as_str), "{name}: the last line");
            let words: Vec<String> = summary_counts(&s).into_iter().map(|c| c.1).collect();
            assert_eq!(json!(words), case["counts"], "{name}: the counts as words");
        }
    }

    #[test]
    fn r3_a_long_last_line_is_kept_to_one_lines_worth_as_a_failure_line_is() {
        let rows: Vec<String> = (0..60).map(|i| format!("{{\"number\":{i},\"title\":\"Pull request {i}\",\"state\":\"open\"}}")).collect();
        let last = slate_run_summary(&[format!("[{}]", rows.join(","))]).last.unwrap();
        assert!(last.chars().count() <= 160, "{} characters", last.chars().count());
    }

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
