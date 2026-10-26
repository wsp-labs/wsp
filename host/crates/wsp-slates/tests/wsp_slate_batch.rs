// SPDX-License-Identifier: AGPL-3.0-only
//! The batch and the sketch: packages/protocol/test/slate/batch.test.ts, sketch.test.ts, and the batch and sketch cases
//! of viz.test.ts and kit.test.ts, at 65f6b89c7. Each document is the TS parser's output for the test's slate text,
//! kept in fixtures/wsp_slate_batch_docs.json, and loads through validate_slate. The golden file holds the TS sketch of
//! every document and of each scenario the tests set values for, and is compared byte for byte.

use std::cell::Cell;
use std::rc::Rc;

use regex::Regex;
use serde_json::{json, Map, Value};
use wsp_slates::pending::slate::*;

const DOCS: &str = include_str!("fixtures/wsp_slate_batch_docs.json");
const GOLDEN: &str = include_str!("fixtures/wsp_slate_batch_golden.json");

/// 2026-10-04T12:00:00Z, the clock the sketch tests read.
const NOW: i64 = 1_791_115_200_000;
/// 2026-10-07T14:10:25 in the host's zone, the clock the viz tests read.
const VIEW_NOW: i64 = 1_791_382_225_000;

fn doc_json(name: &str) -> Value {
    let docs: Map<String, Value> = serde_json::from_str(DOCS).expect("the docs fixture is JSON");
    docs.get(name).cloned().unwrap_or_else(|| panic!("no fixture {name}"))
}

fn checked(json: &Value) -> SlateDoc {
    let checked = validate_slate(json);
    assert!(checked.errors.is_empty(), "{:?}", checked.errors);
    checked.document.expect("a valid document")
}

fn doc(name: &str) -> SlateDoc {
    checked(&doc_json(name))
}

fn golden(name: &str) -> Value {
    let all: Vec<Value> = serde_json::from_str(GOLDEN).expect("the golden fixture is JSON");
    all.into_iter().find(|g| g["name"] == name).unwrap_or_else(|| panic!("no golden {name}"))
}

fn no_source() -> SlateResolve {
    Rc::new(|_: &str| None)
}

fn eval(now: i64) -> SlateEvalContext {
    SlateEvalContext { now, item: None, index: None, resolve: no_source() }
}

fn ctx(by: &str, now: i64) -> SlateBatchContext {
    SlateBatchContext { eval: eval(now), by: by.to_string(), event: None, reaction_sends: false, react: None, start: None }
}

fn w(path: &str, value: Value) -> SlateWrite {
    SlateWrite { path: path.to_string(), value }
}

fn codes(problems: &[SlateProblem]) -> Vec<&str> {
    problems.iter().map(|p| p.code.as_str()).collect()
}

fn opts(now: i64, version: i64) -> SketchOptions {
    SketchOptions { eval: eval(now), version, warnings: Vec::new(), check: false, problems: Vec::new(), waiting: Vec::new() }
}

fn sketch(d: &SlateDoc, values: &SlateValues, o: SketchOptions) -> String {
    sketch_slate(Some(d), values, o)
}

fn start(d: &SlateDoc) -> SlateValues {
    slate_start_values(d)
}

fn held() -> Value {
    json!({"state": "held", "why": "needs your approval", "runs": 0})
}

fn js_utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

// batch.test.ts: the batch

#[test]
fn recomputes_derived_values_before_reactions_and_fires_them_in_document_order() {
    let d = doc("b_derived");
    let r = run_slate_batch(&d, &start(&d), vec![w("$n", json!(9))], ctx("agent", NOW));
    assert!(r.problems.is_empty(), "{:?}", r.problems);
    assert_eq!(r.values["log"], json!("big:true;n:9;"));
    assert_eq!(r.changed, ["$n", "$log"]);
}

#[test]
fn shows_a_set_in_one_round_to_the_reactions_of_the_next_never_the_same_round() {
    let d = doc("b_order");
    let r = run_slate_batch(&d, &start(&d), vec![w("$a", json!(1))], ctx("agent", NOW));
    assert_eq!((r.values["a"].clone(), r.values["b"].clone(), r.values["c"].clone()), (json!(1), json!(2), json!(3)));
    assert_eq!(r.values["seen"], json!(2));
}

#[test]
fn takes_no_write_that_changes_nothing_as_a_change() {
    let d = doc("b_noop");
    let r = run_slate_batch(&d, &start(&d), vec![w("$a", json!(1))], ctx("agent", NOW));
    assert_eq!(r.values["hits"], json!(0));
    assert!(r.changed.is_empty(), "{:?}", r.changed);
}

#[test]
fn refuses_a_static_cycle_at_write_time_so_the_batch_never_meets_one() {
    let json = json!({
        "schema": 2,
        "root": "column-1",
        "values": {"a": {"start": 0}},
        "derived": {"twice": "$a * 2"},
        "runs": {},
        "reactions": [{"on": {"change": ["$twice"]}, "do": [{"do": "set", "path": "$a", "value": 1}]}],
        "pieces": {"column-1": {"type": "column", "children": ["text-1"]}, "text-1": {"type": "text", "props": {"value": "x"}}}
    });
    let r = validate_slate(&json);
    assert_eq!(codes(&r.errors), ["A610"]);
    assert_eq!(r.errors[0].message, "reactions and derived values form a cycle: $twice reads $a; when change of $twice sets $a");
}

#[test]
fn stops_a_loop_through_a_run_after_8_rounds_with_r910_keeping_what_was_written() {
    let d = doc("b_loop");
    let count = Rc::new(Cell::new(0));
    let counter = Rc::clone(&count);
    let mut c = ctx("run", NOW);
    c.start = Some(Rc::new(move |_run: &str, _by: &str| {
        let n = counter.get() + 1;
        counter.set(n);
        json!({"state": "done", "exit": 0, "runs": n})
    }));
    let r = run_slate_batch(&d, &start(&d), vec![w("$tick", json!({"state": "done", "exit": 0, "runs": 0}))], c);
    assert_eq!(codes(&r.problems), ["R910"]);
    assert_eq!(r.starts.len(), 8);
    assert_eq!(r.values["tick"], json!({"state": "done", "exit": 0, "runs": 8}));
}

#[test]
fn stops_a_reaction_at_a_refused_step_keeps_its_earlier_steps_and_fires_the_rest_of_the_round() {
    let d = doc("b_chain");
    let r = run_slate_batch(&d, &start(&d), vec![w("$a", json!(1))], ctx("agent", NOW));
    assert_eq!((r.values["x"].clone(), r.values["y"].clone(), r.values["z"].clone()), (json!(1), json!(0), json!(1)));
    assert_eq!(codes(&r.problems), ["R912"]);
    assert_eq!(r.problems[0].piece.as_deref(), Some("chain"));
    assert_eq!(
        r.problems[0].message,
        "reaction chain, step 2 (send): the person has not let this slate message the agent from a reaction; the steps after it did not run"
    );

    let mut allowed = ctx("agent", NOW);
    allowed.reaction_sends = true;
    let r = run_slate_batch(&d, &start(&d), vec![w("$a", json!(1))], allowed);
    assert_eq!((r.values["x"].clone(), r.values["y"].clone(), r.values["z"].clone()), (json!(1), json!(1), json!(1)));
    assert_eq!(
        r.sends,
        [SlateBatchSend {
            kind: "send".into(),
            text: "Moved.".into(),
            with: Some(vec!["$a".into()]),
            values: Some([("$a".to_string(), json!(1))].into_iter().collect()),
            piece: None,
            reaction: Some("chain".into()),
            by: Some("reaction".into()),
        }]
    );
}

#[test]
fn fires_done_once_a_run_ends_whatever_the_outcome() {
    let d = doc("b_done");
    let running = run_slate_batch(&d, &start(&d), vec![w("$check", json!({"state": "running", "runs": 1}))], ctx("run", NOW));
    assert_eq!(running.values["ok"], Value::Null);
    let failed = run_slate_batch(
        &d,
        &running.values,
        vec![w("$check", json!({"state": "failed", "exit": 1, "why": "the host restarted while it ran", "runs": 1}))],
        ctx("run", NOW),
    );
    assert_eq!(failed.values["ok"], json!(false));
}

#[test]
fn runs_a_press_as_round_0_with_the_row_in_scope_and_hands_window_steps_back() {
    let d = doc("b_press");
    let mut c = ctx("person", NOW);
    let resolve: SlateResolve = Rc::new(|path: &str| {
        (path == "pr.checks")
            .then(|| json!([{"name": "lint", "link": "https://x.example/1"}, {"name": "build", "link": "https://x.example/2"}]))
    });
    c.eval.resolve = resolve;
    c.event = Some(BatchEvent { piece: "table-1".into(), kind: "press".into(), item: None, index: Some(1), row_action: Some(0) });
    let r = run_slate_batch(&d, &start(&d), vec![], c);
    assert_eq!(r.values["pick"], json!("build"));
    assert_eq!(r.starts, [BatchStart { run: "finish".into(), by: "person".into(), why: "a press on table-1".into() }]);
    assert_eq!(r.other, ["open"]);
    assert_eq!(
        r.other_steps,
        [SlateBatchWindowStep {
            step: json!({"do": "open", "target": {"bind": "item.link"}}),
            piece: "table-1".into(),
            value: Some(json!("https://x.example/2")),
            values: None,
        }]
    );
}

#[test]
fn refuses_an_agents_write_to_a_derived_value_a_run_or_a_secret_and_takes_the_persons_handle() {
    let d = doc("b_secret");
    let writes = vec![w("$b", json!(1)), w("$r", json!({"state": "done", "runs": 1})), w("$t", json!("plaintext"))];
    let r = run_slate_batch(&d, &start(&d), writes, ctx("agent", NOW));
    assert_eq!(codes(&r.problems), ["A607", "A607", "S520"]);
    assert!(r.changed.is_empty(), "{:?}", r.changed);

    let handle = json!({"secret": true, "set": true, "len": 24, "at": 1});
    let r = run_slate_batch(&d, &start(&d), vec![w("$t", handle.clone())], ctx("person", NOW));
    assert_eq!(r.values["t"], handle);
}

#[test]
fn carries_the_setup_examples_chain_a_pasted_project_starts_the_check_held_until_approved() {
    let d = doc("setup");
    let mut v0 = start(&d);
    v0.insert("step".into(), json!(2));
    let mut c = ctx("person", NOW);
    c.start = Some(Rc::new(|_run: &str, _by: &str| held()));
    let b1 = run_slate_batch(&d, &v0, vec![w("$project", json!("wsp-landing"))], c);
    assert_eq!(b1.starts, [BatchStart { run: "check".into(), by: "reaction".into(), why: "reaction reaction-1".into() }]);
    assert_eq!(b1.values["check"], held());
    let b2 = run_slate_batch(
        &d,
        &b1.values,
        vec![w("$check", json!({"state": "done", "exit": 0, "out": "wsp-landing\n  Framework: Next.js", "runs": 1}))],
        ctx("run", NOW),
    );
    assert_eq!(b2.values["step"], json!(3));
}

#[test]
fn lists_s500_when_a_write_would_pass_the_values_cap_and_keeps_the_values_as_they_were() {
    let d = doc("b_size");
    let r = run_slate_batch(&d, &start(&d), vec![w("$notes", json!("x".repeat(300_000)))], ctx("person", NOW));
    assert_eq!(codes(&r.problems), ["S500"]);
    assert_eq!(r.values["notes"], json!(""));
}

#[test]
fn counts_the_document_and_the_values_in_utf8_bytes_not_characters() {
    let title = "\u{20ac}".repeat(30_000);
    assert!(js_utf16_len(&title) < SLATE_LIMITS.document_bytes);
    let mut json = doc_json("b_size");
    json["title"] = json!(title);
    let refused = validate_slate(&json);
    assert!(codes(&refused.errors).contains(&"D208"), "{:?}", codes(&refused.errors));

    let d = doc("b_size");
    let r = run_slate_batch(&d, &start(&d), vec![w("$notes", json!("\u{20ac}".repeat(100_000)))], ctx("person", NOW));
    assert_eq!(codes(&r.problems), ["S500"]);
}

// sketch.test.ts: the sketch

fn token_count(text: &str) -> usize {
    let tokens = Regex::new(r"[A-Za-z]+|\d+|[^\sA-Za-z\d]").expect("the token pattern compiles");
    tokens.find_iter(text).count()
}

#[test]
fn every_spec_example_sketches_under_its_budget() {
    let docs: Map<String, Value> = serde_json::from_str(DOCS).expect("the docs fixture is JSON");
    let first = Regex::new(r#"^slate v1 ".+", \d+ pieces, \d+ bound, 0 problems$"#).expect("the header pattern compiles");
    let mut seen = 0;
    for name in docs.keys().filter(|n| n.starts_with("ex_")) {
        seen += 1;
        let d = doc(name);
        let s = sketch(&d, &start(&d), opts(NOW, 1));
        let head = s.split('\n').next().unwrap_or_default();
        assert!(first.is_match(head), "{name}: {head}");
        assert!(token_count(&s) < 500, "{name}: {} tokens", token_count(&s));
        assert!(s.split('\n').all(|l| js_utf16_len(l) <= 100), "{name}: a line passes 100 columns");
    }
    assert_eq!(seen, 9);
}

#[test]
fn says_a_run_started_again_after_a_failure_is_running_again_with_a_failed_last_run() {
    let d = doc("s_run_again");
    let again =
        json!({"spot": {"state": "running", "refreshing": true, "exit": 1, "ms": 40, "runs": 2, "startedAt": NOW, "endedAt": NOW - 1000}});
    let values: SlateValues = serde_json::from_value(again).unwrap();
    assert!(sketch(&d, &values, opts(NOW, 1)).contains("$spot: running again; the last run failed (exit 1, 40 ms)"));
}

#[test]
fn says_which_facts_the_panel_leaves_out_for_having_no_value() {
    let d = doc("s_facts");
    let mut values = start(&d);
    values.insert("v".into(), Value::Null);
    assert!(sketch(&d, &values, opts(NOW, 1)).contains("Health: ok  (no value yet, not shown: Version)  [f facts]"));
}

#[test]
fn draws_bars_to_the_panels_scale_the_longest_row_full_with_no_max_and_says_how_many_rows_it_left_out() {
    let d = doc("s_bars");
    let many: Vec<Value> = [("CA", 284), ("CN", 102), ("US", 71), ("DE", 40), ("FR", 30), ("IN", 20), ("JP", 10), ("BR", 5), ("MX", 2)]
        .iter()
        .map(|(n, v)| json!({"n": n, "v": v}))
        .collect();
    let mut values = start(&d);
    values.insert("rows".into(), Value::Array(many));
    let s = sketch(&d, &values, opts(NOW, 1));
    assert!(s.contains("  CA  [##########] 284"), "{s}");
    assert!(s.contains("  CN  [####......] 102"), "{s}");
    assert!(s.contains("and 1 more rows; slate_read values with the list's path reads them all"), "{s}");
}

#[test]
fn marks_a_piece_that_reads_a_runs_result_before_the_run_has_one_and_not_one_that_reads_its_state() {
    let d = doc("s_reads");
    let before = sketch(&d, &start(&d), opts(NOW, 1));
    assert!(before.contains("Market closed (reads $market, not run yet)  [closed text]"), "{before}");
    assert!(before.contains("Spot  not read yet  [spot number]"), "{before}");
    assert!(before.contains("Waiting  [waiting text]"), "{before}");

    let mut ran = start(&d);
    ran.insert("market".into(), json!({"state": "done", "exit": 0, "out": "{\"open\":false}", "json": {"open": false}, "runs": 1, "startedAt": NOW - 1000, "endedAt": NOW}));
    assert!(sketch(&d, &ran, opts(NOW, 2)).contains("Market closed  [closed text]"));

    let mut failed = start(&d);
    failed.insert(
        "market".into(),
        json!({"state": "failed", "why": "exited with 1", "exit": 1, "out": "", "runs": 1, "startedAt": NOW - 1000, "endedAt": NOW}),
    );
    assert!(sketch(&d, &failed, opts(NOW, 3)).contains("Market closed (reads $market, which failed)  [closed text]"));
}

#[test]
fn draws_the_setup_slate_as_the_person_sees_it_the_secret_as_dots_and_the_runs_that_moved() {
    let d = doc("setup");
    let handle = json!({"secret": true, "set": true, "len": 24, "at": NOW});
    let mut v = r_batch(&d, &start(&d), vec![w("$vercelToken", handle)], "person", None).values;
    v = r_batch(&d, &v, vec![], "person", Some(("button-2", "press"))).values;
    let mut c = ctx("person", NOW);
    c.start = Some(Rc::new(|_run: &str, _by: &str| held()));
    v = run_slate_batch(&d, &v, vec![w("$project", json!("wsp-landing"))], c).values;
    let s = sketch(&d, &v, opts(NOW, 4));
    let expected = [
        "slate v4 \"Deploy setup\", 24 pieces, 8 bound, 0 problems",
        "Step 2 of 4  [text-1 text strong]",
        "(hidden) [section-1 section]",
        "2. The project  [section-2 section]",
        "  Project name on Vercel: \"wsp-landing\"  [project-name input mono]",
        "  Approve the check to go on  [text-2 text muted]",
        "  (hidden) [text-3 text muted]",
        "  (hidden) [text-4 text bad]",
        "  (hidden) [text-5 text good]",
        "(hidden) [section-3 section]",
        "(hidden) [section-4 section]",
        "values:",
        "  $step = 2",
        "  $project = \"wsp-landing\"",
        "  $vercelToken = \u{2022}\u{2022}\u{2022}\u{2022} (24 characters)",
        "  $checked = false",
        "  $written = false",
        "runs:",
        "  $check: held needs your approval",
    ]
    .join("\n");
    assert_eq!(s, expected);
    assert!(!s.contains("plaintext"));
}

fn r_batch(d: &SlateDoc, values: &SlateValues, writes: Vec<SlateWrite>, by: &str, event: Option<(&str, &str)>) -> SlateBatchResult {
    let mut c = ctx(by, NOW);
    c.event = event.map(|(piece, kind)| BatchEvent { piece: piece.into(), kind: kind.into(), item: None, index: None, row_action: None });
    run_slate_batch(d, values, writes, c)
}

#[test]
fn shows_formulas_in_braces_for_a_check_with_no_thread() {
    let d = doc("s_check");
    let s = sketch(&d, &SlateValues::new(), SketchOptions { check: true, ..opts(0, 0) });
    assert_eq!(s, "slate v0, 2 pieces, 1 bound, 0 problems\nWeekly  [{usage.week.percent} of 100]  [week meter]");
}

#[test]
fn says_a_charts_axes_as_they_are_labelled_an_index_from_0_and_a_time_by_the_clock() {
    let by_index = sketch(&doc("s_chart_index"), &start(&doc("s_chart_index")), opts(NOW, 1));
    assert!(by_index.contains("Requests  last 159, min 100, max 159 over 60 points"), "{by_index}");
    assert!(by_index.split('\n').any(|l| l == "  x 0 to 59, y 100 to 180"), "{by_index}");
    let by_time = sketch(&doc("s_chart_time"), &start(&doc("s_chart_time")), opts(NOW, 1));
    assert!(by_time.split('\n').any(|l| l == "  x 16:48 to 17:47, y 100 to 180"), "{by_time}");
}

fn sessions_values(d: &SlateDoc, all: bool) -> SlateValues {
    let mut v = start(d);
    v.insert("all".into(), json!(all));
    v
}

#[test]
fn leaves_out_a_fact_a_column_and_an_option_whose_when_does_not_hold_and_keeps_them_when_it_does() {
    let d = doc("w_when");
    let off = sketch(&d, &sessions_values(&d, false), opts(NOW, 0));
    assert!(off.contains("Name: node  [facts-1 facts]"), "{off}");
    assert!(!off.contains("PID: 19271"), "{off}");
    assert!(off.contains("| Name |  [table-1 table]"), "{off}");
    assert!(off.contains("Pick: (none) (of a)"), "{off}");
    let on = sketch(&d, &sessions_values(&d, true), opts(NOW, 0));
    assert!(on.contains("Name: node  PID: 19271"), "{on}");
    assert!(on.contains("| Name | PID |"), "{on}");
    assert!(on.contains("(of a, b)"), "{on}");
}

#[test]
fn draws_a_section_a_literal_open_false_starts_shut_as_collapsed() {
    let d = doc("w_collapsed");
    assert!(sketch(&d, &start(&d), opts(NOW, 0)).contains("More (collapsed)"));
}

fn plot_lines(s: &str) -> Vec<String> {
    s.split('\n')
        .filter(|l| Regex::new(r"^  [A-Z]\d{3} ").expect("the code pattern compiles").is_match(l))
        .map(|l| l.trim().to_string())
        .collect()
}

#[test]
fn names_a_plotted_value_that_is_not_a_number_as_a_problem_with_the_binding_what_it_read_and_a_fix() {
    let d = doc("w_plot");
    let blank = sketch(&d, &start(&d), opts(NOW, 0));
    assert!(blank.contains("0 problems"), "{blank}");

    let mut bad = start(&d);
    bad.insert("rows".into(), json!([{"at": "2026-10-04T19:46:00Z", "n": 690}, {"at": "2026-10-04T19:47:00Z", "n": 747}]));
    bad.insert("m".into(), json!("12"));
    let s = sketch(&d, &bad, opts(NOW, 0));
    assert!(s.contains("5 problems"), "{s}");
    assert!(s.contains("R905 chart-1.value: value={item} read an object on 2 of 2 rows, like {\"at\":\"2026-10-04T19:46:00Z\",\"n\":690}, so it draws no point for them; a plotted value is a number, like value={item.n}"), "{s}");
    assert!(s.contains("R905 number-1.trend: trend={$rows} holds an object in 2 of 2 places, like {\"at\":\"2026-10-04T19:46:00Z\",\"n\":690}, so it draws no point for them; a plotted list holds numbers, like trend={pluck($rows, 'n')}"), "{s}");
    assert!(s.contains("R905 bars-1.value: value={item.at} read a string on 2 of 2 rows, like \"2026-10-04T19:46:00Z\", so it draws no point for them; a plotted value is a number"), "{s}");
    assert!(
        s.contains(
            "R905 meter-1.value: value={$m} read a string, \"12\", so it draws nothing; a plotted value is a number, like value={num($m)}"
        ),
        "{s}"
    );
    assert!(s.contains("R905 sparkline-1.values: values={pluck($rows, 'at')} holds a string in 2 of 2 places"), "{s}");

    let mut short = start(&d);
    short.insert("rows".into(), json!([{"at": "a", "n": 1}]));
    short.insert("m".into(), json!(3));
    assert!(sketch(&d, &short, opts(NOW, 0)).contains("4 problems"));

    let fine = doc("w_plot_fine");
    let mut ok = start(&fine);
    ok.insert("rows".into(), json!([{"at": "2026-10-04T19:46:00Z", "n": 690}, {"at": "2026-10-04T19:47:00Z", "n": 747}, {"at": "2026-10-04T19:48:00Z", "n": null}]));
    ok.insert("m".into(), json!(3));
    assert!(sketch(&fine, &ok, opts(NOW, 0)).contains("0 problems"));
}

#[test]
fn holds_the_pieces_to_the_line_cap_and_cuts_above_the_problems_and_warnings_the_header_counts() {
    let d = doc("w_size");
    let values: SlateValues = (0..60).map(|i| (format!("v{i}"), json!("x".repeat(200)))).collect();
    let warnings: Vec<SlateProblem> = serde_json::from_value(golden("size")["opts"]["warnings"].clone()).unwrap();
    let s = sketch(&d, &values, SketchOptions { warnings, ..opts(NOW, 0) });
    let all: Vec<&str> = s.split('\n').collect();
    let values_at = all.iter().position(|l| *l == "values:").expect("a values section");
    let piece_lines = all[1..values_at].iter().filter(|l| !l.starts_with("... and")).count();
    assert!(piece_lines <= SLATE_LIMITS.sketch_piece_lines, "{piece_lines} piece lines");
    assert!(js_utf16_len(&s) <= SLATE_LIMITS.sketch_tokens * 4, "{} units", js_utf16_len(&s));
    assert!(s.contains("W004"), "{s}");
}

// viz.test.ts: the chart kinds and what they refuse

fn drawn(name: &str, rows: &[(&str, Value)]) -> (String, Vec<String>) {
    let d = doc(name);
    let mut values = start(&d);
    for (key, value) in rows {
        values.insert((*key).into(), value.clone());
    }
    let s = sketch(&d, &values, opts(VIEW_NOW, 0));
    let problems = plot_lines(&s);
    (s, problems)
}

fn lat() -> Value {
    json!([
        {"at": "2026-10-07T14:00:00", "p50": 19, "p99": 210},
        {"at": "2026-10-07T14:10:00", "p50": 22, "p99": 340},
        {"at": "2026-10-07T14:20:00", "p50": 18, "p99": 190}
    ])
}

fn steps() -> Value {
    json!([
        {"job": "Build", "name": "Web app", "start": "2026-10-07T14:02:10", "end": "2026-10-07T14:04:03"},
        {"job": "Test", "name": "Shard 1", "start": "2026-10-07T14:04:03"},
        {"job": "Test", "name": "Shard 2", "start": "2026-10-07T14:04:03", "end": "2026-10-07T14:05:00", "state": "failed"},
        {"job": "Deploy", "name": "Preview"}
    ])
}

fn modules() -> Value {
    json!([
        {"pkg": "mermaid", "name": "core", "bytes": 231424},
        {"pkg": "mermaid", "name": "dagre", "bytes": 59392},
        {"pkg": "app", "name": "chat", "bytes": 94208}
    ])
}

fn countries(n: usize) -> Value {
    let names = ["India", "Brazil", "Germany", "Japan", "Kenya", "Peru", "Chile", "Norway"];
    Value::Array(names.iter().take(n).enumerate().map(|(i, name)| json!({"name": name, "n": 800 - i * 100})).collect())
}

#[test]
fn draws_two_to_four_series_over_one_x_each_line_with_its_own_figures() {
    let (s, _) = drawn("v_lines", &[("lat", lat())]);
    assert!(s.contains("Latency  2 lines over 3 points"), "{s}");
    assert!(s.contains("  p50  last 18 ms, min 18 ms, max 22 ms"), "{s}");
    assert!(s.contains("  p99  last 190 ms, min 190 ms, max 340 ms [bad]"), "{s}");
    assert!(s.contains("  x 14:00 to 14:20, y 0 ms to 400 ms"), "{s}");
}

#[test]
fn piles_a_stacks_series_into_bands_of_one_total_the_axis_reaching_the_total() {
    let (s, _) = drawn("v_stack", &[("lat", lat())]);
    assert!(s.contains("Latency  2 stacked lines over 3 points"), "{s}");
    assert!(s.contains("  p99  last 190, min 190, max 340"), "{s}");
    assert!(s.contains("y 0 to 400"), "{s}");
}

#[test]
fn draws_a_timelines_spans_by_clock_a_running_one_to_now_a_failed_and_a_waiting_one_in_words_under_their_group() {
    let (s, _) = drawn("v_timeline", &[("steps", steps())]);
    let lines: Vec<&str> = s.split('\n').collect();
    for line in [
        "  Build",
        "  Web app  14:02:10 to 14:04:03  1m 53s",
        "  Shard 1  14:04:03 to now  running 6m 22s",
        "  Shard 2  14:04:03 to 14:05:00  failed",
        "  Preview  waiting",
    ] {
        assert!(lines.contains(&line), "{line} missing from {s}");
    }
}

#[test]
fn draws_a_treemaps_parts_with_their_share_of_the_whole_and_a_donuts_biggest_five_with_the_rest_as_other() {
    let (tree, _) = drawn("v_treemap", &[("mods", modules())]);
    assert!(tree.contains("Bundle  376 KB in 3 parts, 2 groups"), "{tree}");
    assert!(tree.contains("  mermaid  core  226 KB 60%"), "{tree}");

    let (donut, _) = drawn("v_donut", &[("c", countries(8))]);
    assert!(donut.contains("Clicks  3,600 clicks"), "{donut}");
    assert!(donut.contains("  India  800 22%"), "{donut}");
    assert!(donut.contains("  Other  600 17%"), "{donut}");
    assert!(!donut.contains("Peru"), "{donut}");

    let (six, _) = drawn("v_donut_plain", &[("c", countries(6))]);
    let names: Vec<String> = six
        .split('\n')
        .filter(|l| Regex::new(r"^  [A-Z][a-z]+  \d").expect("the part pattern compiles").is_match(l))
        .map(|l| l.trim().split("  ").next().unwrap_or_default().to_string())
        .collect();
    assert_eq!(names, ["India", "Brazil", "Germany", "Japan", "Kenya", "Other"]);
}

#[test]
fn a_timeline_whose_start_is_not_a_time_is_named_as_a_problem() {
    let (_, problems) =
        drawn("v_timeline_start_bad", &[("steps", json!([{"name": "Build", "start": "soon", "end": "2026-10-07T14:04:03"}]))]);
    assert_eq!(problems, ["R905 ci.start: start={item.start} read a string on 1 of 1 rows, like \"soon\", which is not a time, so it draws no span; a time is ISO or ms"]);
}

#[test]
fn a_timeline_whose_end_is_not_a_time_is_named_as_a_problem() {
    let (_, problems) =
        drawn("v_timeline_start_bad", &[("steps", json!([{"name": "Build", "start": "2026-10-07T14:02:10", "end": {"at": 1}}]))]);
    assert_eq!(problems, ["R905 ci.end: end={item.end} read an object on 1 of 1 rows, like {\"at\":1}, which is not a time, so it draws no span; a time is ISO or ms"]);
}

#[test]
fn a_treemap_value_that_is_not_a_number_is_named_as_a_problem() {
    let (_, problems) = drawn("v_treemap_bad", &[("mods", json!([{"name": "core", "size": "226 KB"}]))]);
    assert_eq!(problems, ["R905 b.value: value={item.size} read a string on 1 of 1 rows, like \"226 KB\", so it draws no point for them; a plotted value is a number"]);
}

#[test]
fn a_donut_value_that_is_not_a_number_is_named_as_a_problem() {
    let (_, problems) = drawn("v_donut_bad", &[("c", json!([{"name": "India", "n": 41}]))]);
    assert_eq!(problems, ["R905 c.value: value={item} read an object on 1 of 1 rows, like {\"name\":\"India\",\"n\":41}, so it draws no point for them; a plotted value is a number, like value={item.n}"]);
}

#[test]
fn a_series_value_that_is_not_a_number_is_named_as_a_problem() {
    let (_, problems) = drawn("v_series_bad", &[("lat", lat())]);
    assert_eq!(problems, ["R905 lat.series: <series value={item.at}> read a string on 3 of 3 rows, like \"2026-10-07T14:00:00\", so it draws no point for them; a plotted value is a number"]);
}

#[test]
fn a_timeline_start_written_in_seconds_is_told_it_reads_as_1970_with_the_fix_and_the_panel_still_draws() {
    let (s, problems) =
        drawn("v_timeline_fixable", &[("steps", json!([{"name": "Build", "start": 1_759_900_000, "end": "2026-10-07T14:04:03"}]))]);
    assert_eq!(problems, ["W021 ci.start: start={item.start} read on 1 of 1 rows, like 1759900000, a time before 2000 that is most likely seconds where a slate reads milliseconds, like start={item.start * 1000}. Fix: start={item.start * 1000}"]);
    assert!(s.contains("0 problems, 1 warning"), "{s}");
    let (_, fixed) =
        drawn("v_timeline_fixed", &[("steps", json!([{"name": "Build", "start": 1_759_900_000, "end": "2026-10-07T14:04:03"}]))]);
    assert!(fixed.is_empty(), "{fixed:?}");
}

// kit.test.ts: the batch and sketch cases

#[test]
fn validates_sketches_and_prints_back_every_new_piece_and_prop() {
    let d = doc("k_richer");
    let mut values = start(&d);
    values.insert("pick".into(), json!(2));
    let s = sketch(&d, &values, opts(NOW, 0));
    for line in [
        "# Gold  [heading-1 heading]",
        "FEED",
        "grid of 3",
        "A  1  trend 1 to 3",
        "(good) Live",
        "[main]",
        "Done  (3/4) [########..]  almost",
        "Line  last $3.00, min $1.00, max $3.00 over 3 points",
        "(x) 2 right",
    ] {
        assert!(s.contains(line), "{line} missing from {s}");
    }
    assert!(Regex::new(r"\n {2}x \d\d:\d\d to \d\d:\d\d, y \$0\.00 to \$4\.00\n").unwrap().is_match(&s), "{s}");
}

#[test]
fn draws_the_example_a_heading_numbers_an_inset_section_a_status_a_chip_a_ring_and_a_chart_fed_by_a_history() {
    let d = doc("k_gold");
    let mut values = start(&d);
    for (at, price) in [(1000i64, 2400.5f64), (61000, 2401.25), (121000, 2399.0)] {
        values = r_batch_at(&d, &values, "$spot", json!({"state": "done", "exit": 0, "out": "", "json": {"price": price}, "runs": 1}), at);
    }
    assert_eq!(values["hist"], json!([{"at": 1000, "v": 2400.5}, {"at": 61000, "v": 2401.25}, {"at": 121000, "v": 2399.0}]));
    let s = sketch(&d, &values, opts(121_000, 0));
    for line in [
        "# Gold",
        "grid of 3",
        "Per ounce  $2,399.00  trend 2400.5 to 2399",
        "Low  $2,399.00",
        "High  $2,401.25",
        "Feed",
        "(good) Live",
        "[every minute]",
        "Last hour  (3/60) [#.........]",
        "Per ounce, last hour  last $2,399.00, min $2,399.00, max $2,401.25 over 3 points",
        "  x 1000 to 121000, y $2,399.00 to $2,401.50",
    ] {
        assert!(s.contains(line), "{line} missing from {s}");
    }

    let mut capped = values.clone();
    capped.insert("hist".into(), Value::Array((0..60).map(|i| json!({"at": i, "v": i})).collect()));
    let capped = r_batch_at(&d, &capped, "$spot", json!({"state": "done", "exit": 0, "json": {"price": 99}, "runs": 2}), 9);
    assert_eq!(capped["hist"].as_array().map(Vec::len), Some(60));
    assert_eq!(capped["hist"].as_array().and_then(|h| h.last()), Some(&json!({"at": 9, "v": 99})));
}

fn r_batch_at(d: &SlateDoc, values: &SlateValues, path: &str, value: Value, now: i64) -> SlateValues {
    run_slate_batch(d, values, vec![w(path, value)], ctx("run", now)).values
}

#[test]
fn logs_a_quizs_answers_with_append_under_a_computed_index() {
    let d = doc("k_quiz");
    let picked = r_batch(&d, &start(&d), vec![w("$pick", json!("Network"))], "person", Some(("answer", "change")));
    assert_eq!(picked.values["log"], json!([{"q": "Which layer routes packets between networks?", "pick": "Network", "right": true}]));
    assert!(sketch(&d, &picked.values, opts(NOW, 0)).contains("(x) Network right"));
}

#[test]
fn marks_a_run_whose_command_changed_since_it_ran() {
    let d = doc("k_stale");
    let values: SlateValues =
        serde_json::from_value(json!({"weather": {"state": "done", "exit": 0, "ms": 812, "runs": 1, "stale": true}})).unwrap();
    let s = sketch(&d, &values, opts(NOW, 0));
    assert!(s.contains("$weather: done (exit 0, 812 ms), stale: the command changed since it ran"), "{s}");
    assert!(s.contains("(hidden)"), "{s}");
}

#[test]
fn takes_a_mermaid_source_as_a_formula_that_reads_the_live_step() {
    let d = doc("k_diagram");
    let values: SlateValues = serde_json::from_value(json!({"step": "test"})).unwrap();
    assert!(sketch(&d, &values, opts(NOW, 0)).contains("Deploy  flowchart LR (+3 lines)"));
}

#[test]
fn draws_rupees_with_lakh_grouping_in_the_sketch() {
    let d = doc("k_inr");
    assert!(sketch(&d, &SlateValues::new(), opts(NOW, 1)).contains("24K, per gram  \u{20b9}14,918"));
}

#[test]
fn sketches_a_figure_as_the_panel_draws_it_through_the_one_formatter() {
    let d = doc("k_figures");
    let s = sketch(&d, &SlateValues::new(), opts(NOW, 0));
    assert!(s.contains("Up  60m"), "{s}");
    assert!(s.contains("1,234,567"), "{s}");
}

// Every document and scenario against the TS sketch.

#[test]
fn every_document_and_scenario_sketches_byte_for_byte_as_the_ts_sketch_does() {
    let all: Vec<Value> = serde_json::from_str(GOLDEN).expect("the golden fixture is JSON");
    let mut failures = Vec::new();
    for g in &all {
        let name = g["name"].as_str().unwrap_or_default();
        let d = doc(g["doc"].as_str().unwrap_or_default());
        let values: SlateValues = serde_json::from_value(g["values"].clone()).expect("golden values");
        let o = &g["opts"];
        let options = SketchOptions {
            eval: eval(o["now"].as_i64().unwrap_or(0)),
            version: o["version"].as_i64().unwrap_or(0),
            warnings: serde_json::from_value(o["warnings"].clone()).expect("golden warnings"),
            check: o["check"].as_bool().unwrap_or(false),
            problems: Vec::new(),
            waiting: Vec::new(),
        };
        let got = sketch(&d, &values, options);
        let want = g["expect"].as_str().unwrap_or_default();
        if got != want {
            failures.push(format!("{name}\n--- rust\n{got}\n--- ts\n{want}"));
        }
    }
    assert!(failures.is_empty(), "{} of {} differ:\n{}", failures.len(), all.len(), failures.join("\n\n"));
}

// sketch.test.ts: every prop a piece declares that the person sees changes its sketch

fn rows() -> Value {
    json!([
        {"name": "a", "v": 1, "at": 1_759_000_000_000i64, "title": "A", "done": false},
        {"name": "b", "v": 2, "at": 1_759_000_060_000i64, "title": "B", "done": true}
    ])
}

fn js_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Array(items) => items.iter().map(js_string).collect::<Vec<_>>().join(","),
        _ => "[object Object]".to_string(),
    }
}

fn pair(name: &str, spec: &SlatePropSpec) -> (Value, Value) {
    match &spec.prop_type {
        SlatePropType::Enum(words) => match &spec.default {
            None => (json!(words[0]), json!(words[1])),
            Some(default) => (default.clone(), words.iter().map(|w| json!(w)).find(|v| v != default).expect("a second word")),
        },
        SlatePropType::Boolean => (json!(true), json!(false)),
        SlatePropType::Number => (json!(40), json!(70)),
        SlatePropType::Text => (json!(12345), json!(23456)),
        SlatePropType::Integer => {
            let min = spec.min.unwrap_or(1.0) as i64;
            (json!(min), json!(min + 1))
        }
        SlatePropType::List if name == "items" => {
            let mut more = rows().as_array().cloned().unwrap_or_default();
            more.push(json!({"name": "c", "v": 3, "at": 1_759_000_120_000i64, "title": "B", "done": true}));
            (rows(), Value::Array(more))
        }
        SlatePropType::List => (json!([1, 2, 3]), json!([4, 5, 6, 7])),
        SlatePropType::Path => (json!("$r"), json!("$s")),
        SlatePropType::Id => (json!("a"), json!("b")),
        _ => (json!("Alpha"), json!("Beta")),
    }
}

fn fresh(m: &SlatePieceModule) -> Map<String, Value> {
    let mut props = Map::new();
    for (name, spec) in &m.props {
        if spec.required {
            props.insert((*name).to_string(), pair(name, spec).0);
        }
    }
    for item in m.items.values() {
        let min = item.min.unwrap_or(0);
        if min == 0 {
            continue;
        }
        let entries: Vec<Value> = (0..min)
            .map(|i| {
                let mut entry = Map::new();
                for (k, f) in item.fields.iter().filter(|(_, f)| f.required) {
                    let text = if f.prop_type == SlatePropType::Id { format!("c{i}") } else { format!("{}{i}", js_string(&pair(k, f).0)) };
                    entry.insert((*k).to_string(), json!(text));
                }
                Value::Object(entry)
            })
            .collect();
        props.insert(item.prop.to_string(), Value::Array(entries));
    }
    props
}

fn sketch_piece(kind: &str, props: Map<String, Value>) -> String {
    let run = json!({"state": "done", "exit": 0, "out": "hi", "err": "", "runs": 1});
    let mut other = run.clone();
    other["out"] = json!("other");
    let doc_json = json!({
        "schema": 2,
        "root": "p",
        "values": {},
        "derived": {},
        "reactions": [],
        "runs": {"r": {"kind": "cmd", "cmd": "true"}, "s": {"kind": "cmd", "cmd": "true"}},
        "pieces": {"p": {"type": kind, "props": props}}
    });
    let d: SlateDoc = serde_json::from_value(doc_json).expect("a piece document");
    let mut values = SlateValues::new();
    values.insert("r".into(), run);
    values.insert("s".into(), other);
    sketch(&d, &values, opts(NOW, 0))
}

#[test]
fn every_piece_prop_the_person_sees_changes_its_sketch() {
    let draws = |kind: &str| -> Vec<(String, Value)> {
        match kind {
            "chart" => vec![("value".into(), json!(40))],
            "treemap" | "donut" => vec![("value".into(), json!(12345))],
            "timeline" => vec![("start".into(), json!(1_759_000_000_000i64))],
            _ => Vec::new(),
        }
    };
    let shown_when = |name: &str| -> Vec<(String, Value)> {
        match name {
            "placeholder" => vec![("value".into(), json!(""))],
            "answer" => vec![("options".into(), json!(["Alpha", "Beta"])), ("value".into(), json!("Alpha"))],
            _ => Vec::new(),
        }
    };
    let mut failures = Vec::new();
    for (kind, m) in SLATE_PIECES.iter() {
        for (name, spec) in &m.props {
            if spec.unseen.is_some() {
                continue;
            }
            let (a, b) = pair(name, spec);
            let mut base = fresh(m);
            base.extend(draws(kind));
            base.extend(shown_when(name));
            let mut with_a = base.clone();
            with_a.insert((*name).to_string(), a);
            let mut with_b = base;
            with_b.insert((*name).to_string(), b);
            if sketch_piece(kind, with_a) == sketch_piece(kind, with_b) {
                failures.push(format!("{kind}.{name}"));
            }
        }
        for (kind_item, item) in &m.items {
            for (field, spec) in &item.fields {
                if spec.unseen.is_some() {
                    continue;
                }
                let (a, b) = pair(field, spec);
                let base = fresh(m);
                let list: Vec<Value> = base.get(item.prop).and_then(Value::as_array).cloned().unwrap_or_else(|| {
                    let entry: Map<String, Value> =
                        item.fields.iter().filter(|(_, f)| f.required).map(|(k, f)| ((*k).to_string(), pair(k, f).0)).collect();
                    vec![Value::Object(entry)]
                });
                let with_field = |v: Value| {
                    let mut props = base.clone();
                    if item.row {
                        props.insert("items".into(), rows());
                    }
                    let edited: Vec<Value> = list
                        .iter()
                        .enumerate()
                        .map(|(i, x)| {
                            if i == 0 {
                                let mut x = x.clone();
                                x[*field] = v.clone();
                                x
                            } else {
                                x.clone()
                            }
                        })
                        .collect();
                    props.insert(item.prop.to_string(), Value::Array(edited));
                    props
                };
                if sketch_piece(kind, with_field(a)) == sketch_piece(kind, with_field(b)) {
                    failures.push(format!("{kind} <{kind_item} {field}>"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "no change in the sketch for: {failures:?}");
}
