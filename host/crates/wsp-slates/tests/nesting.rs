// SPDX-License-Identifier: AGPL-3.0-only
//! Input nested or chained as deep as the size caps let it go must be answered, not abort the host: a Rust stack overflow
//! kills the whole process where node turns it into a refused or failed call. Each case runs in a child copy of this binary
//! on a thread with half of tokio's worker stack (1 MiB), which leaves the real 2 MiB a margin of two and fails when the child dies.

use std::rc::Rc;

use serde_json::{json, Value};
use wsp_slates::pending::slate::*;

const CHILD: &str = "NESTING_CHILD";
const WORKER_STACK: usize = 1024 * 1024;
const N: usize = 30_000;
/// serde_json stops reading JSON text at 128 levels, so no document or tool argument arrives deeper than this.
const DEEPEST_JSON: usize = 120;

fn eval_context() -> SlateEvalContext {
    SlateEvalContext { now: 0, item: None, index: None, resolve: Rc::new(|_: &str| None) }
}

/// Everything a pasted slate text goes through: parse, validate, print, start values, sketch and a batch.
fn through_everything(text: &str) {
    let compiled = compile_slate_text(text);
    let _ = compiled.errors.len();
    let parsed = parse_slate(text);
    let Some(doc) = parsed.document else { return };
    let printed = print_slate(&doc);
    let _ = parse_slate(&printed);
    let values = slate_start_values(&doc);
    let options = SketchOptions { eval: eval_context(), version: 1, warnings: vec![], check: true, problems: vec![], waiting: vec![] };
    let _ = sketch_slate(Some(&doc), &values, options);
    let ctx = SlateBatchContext { eval: eval_context(), by: "person".into(), event: None, reaction_sends: true, react: None, start: None };
    let _ = run_slate_batch(&doc, &values, vec![], ctx);
}

fn hole(inner: String) -> String {
    format!(r#"<slate><value name="v" start={{1}} /><column><text>{{{inner}}}</text></column></slate>"#)
}

fn derived(inner: String) -> String {
    format!(r#"<slate><value name="v" start={{1}} /><derived name="d" value={{{inner}}} /><column><text>{{$d}}</text></column></slate>"#)
}

fn prop(name: &str, inner: String) -> String {
    format!(
        r#"<slate><value name="v" start={{1}} /><column><text tone={{{inner}}}>x</text><meter label="m" {name}={{{inner}}} /></column></slate>"#
    )
}

fn press(steps: String) -> String {
    format!(r#"<slate><value name="v" start={{1}} /><column><button label="b" onPress={{{steps}}} /></column></slate>"#)
}

type TextCase = (&'static str, fn() -> String);
type JsonCase = (&'static str, fn() -> Value);

fn cases() -> Vec<TextCase> {
    vec![
        ("parens", || hole(format!("{}1{}", "(".repeat(N), ")".repeat(N)))),
        ("nots", || hole(format!("{}true", "!".repeat(N)))),
        ("negs", || hole(format!("{}1", "-".repeat(N)))),
        ("ternary_right", || hole("$v ? 1 : ".repeat(N / 4) + "0")),
        ("ternary_left", || hole("$v ? 1 : 2 ".to_string() + &"? 1 : 2 ".repeat(N / 4))),
        ("calls", || hole(format!("{}1{}", "len(".repeat(N / 4), ")".repeat(N / 4)))),
        ("lists", || hole(format!("{}1{}", "[".repeat(N), "]".repeat(N)))),
        ("records", || hole(format!("{}1{}", "{a: ".repeat(N / 4), "}".repeat(N / 4)))),
        ("plus_chain", || hole("1+".repeat(N) + "1")),
        ("and_chain", || hole("$v and ".repeat(N / 2) + "$v")),
        ("compare_chain", || hole("1 < ".repeat(N / 2) + "1")),
        ("or_chain", || hole("$v or ".repeat(N / 2) + "$v")),
        ("pipes", || hole("$v".to_string() + &" | round".repeat(N / 4))),
        ("fields", || hole("pr.a".to_string() + &".a".repeat(N))),
        ("indexes", || hole("pr.a".to_string() + &"[0]".repeat(N / 2))),
        ("templates", || hole(format!("{}x{}", "`${".repeat(N / 8), "}`".repeat(N / 8)))),
        ("derived_parens", || derived(format!("{}1{}", "(".repeat(N), ")".repeat(N)))),
        ("derived_plus", || derived("1+".repeat(N) + "1")),
        ("prop_parens", || prop("value", format!("{}1{}", "(".repeat(N), ")".repeat(N)))),
        ("prop_plus", || prop("value", "1+".repeat(N) + "1")),
        ("prop_lists", || prop("max", format!("{}1{}", "[".repeat(N), "]".repeat(N)))),
        ("press_lists", || press(format!("{}set($v, 1){}", "[".repeat(N), "]".repeat(N)))),
        ("press_nested_value", || press(format!("set($v, {}1{})", "[".repeat(N), "]".repeat(N)))),
        ("press_parens", || press(format!("set($v, {}1{})", "(".repeat(N), ")".repeat(N)))),
        ("elements", || format!("<slate>{}<text>x</text>{}</slate>", "<column>".repeat(N / 20), "</column>".repeat(N / 20))),
        ("sections", || {
            format!(
                "<slate><column>{}<text>x</text>{}</column></slate>",
                "<section title=\"s\">".repeat(N / 20),
                "</section>".repeat(N / 20)
            )
        }),
        ("records_in_env", || {
            format!(
                r#"<slate><run name="r" cmd="ls" env={{{{ A: {}1{} }}}} /><column><text>x</text></column></slate>"#,
                "{a: ".repeat(N / 4),
                "}".repeat(N / 4)
            )
        }),
        ("lists_in_args", || {
            format!(
                r#"<slate><run name="r" cmd="ls" args={{{}1{}}} /><column><text>x</text></column></slate>"#,
                "[".repeat(N),
                "]".repeat(N)
            )
        }),
        ("lists_in_stdin", || {
            format!(
                r#"<slate><run name="r" cmd="ls" stdin={{{}1{}}} /><column><text>x</text></column></slate>"#,
                "[".repeat(N),
                "]".repeat(N)
            )
        }),
        ("reaction_lists", || {
            format!(
                r#"<slate><value name="v" start={{1}} /><when change={{$v}} do={{{}set($v, 2){}}} /><column><text>x</text></column></slate>"#,
                "[".repeat(N),
                "]".repeat(N)
            )
        }),
    ]
}

fn json_cases() -> Vec<JsonCase> {
    fn nest(depth: usize, leaf: Value) -> Value {
        let mut v = leaf;
        for _ in 0..depth {
            v = Value::Array(vec![v]);
        }
        v
    }
    fn record(depth: usize, leaf: Value) -> Value {
        let mut v = leaf;
        for _ in 0..depth {
            let mut m = serde_json::Map::new();
            m.insert("a".into(), v);
            v = Value::Object(m);
        }
        v
    }
    vec![
        (
            "json_start_lists",
            || json!({"schema": 2, "root": "t", "values": {"v": {"start": nest(DEEPEST_JSON, json!(1))}}, "pieces": {"t": {"type": "text", "props": {"value": "x"}}}}),
        ),
        (
            "json_start_records",
            || json!({"schema": 2, "root": "t", "values": {"v": {"start": record(DEEPEST_JSON, json!(1))}}, "pieces": {"t": {"type": "text", "props": {"value": "x"}}}}),
        ),
        (
            "json_prop_lists",
            || json!({"schema": 2, "root": "t", "pieces": {"t": {"type": "text", "props": {"value": nest(DEEPEST_JSON, json!("x"))}}}}),
        ),
        (
            "json_prop_records",
            || json!({"schema": 2, "root": "t", "pieces": {"t": {"type": "text", "props": {"value": record(DEEPEST_JSON, json!("x"))}}}}),
        ),
        (
            "json_env_records",
            || json!({"schema": 2, "root": "t", "runs": {"r": {"kind": "cmd", "cmd": "ls", "env": {"A": record(DEEPEST_JSON, json!("x"))}}}, "pieces": {"t": {"type": "text", "props": {"value": "x"}}}}),
        ),
    ]
}

#[test]
fn json_text_past_the_reader_s_depth_is_an_error_not_an_abort() {
    let text = format!("{}1{}", "[".repeat(100_000), "]".repeat(100_000));
    assert!(serde_json::from_str::<Value>(&text).is_err());
}

fn write_nested(depth: usize) -> Vec<String> {
    let doc = parse_slate(r#"<slate><value name="v" start={null} /><column><text>{len($v)}</text></column></slate>"#)
        .document
        .expect("a document");
    let values = slate_start_values(&doc);
    let mut value = json!(1);
    for _ in 0..depth {
        value = Value::Array(vec![value]);
    }
    let ctx = SlateBatchContext { eval: eval_context(), by: "person".into(), event: None, reaction_sends: true, react: None, start: None };
    let out = run_slate_batch(&doc, &values, vec![SlateWrite { path: "$v".into(), value }], ctx);
    let said = out.problems.iter().map(|p| p.code.clone()).collect();
    let text = serde_json::to_string(&out.values).map(|t| t.len());
    assert!(text.is_ok());
    said
}

#[test]
fn a_value_at_the_depth_limit_is_stored_on_a_small_stack_and_one_more_level_is_refused() {
    let (kept, refused) = std::thread::Builder::new()
        .stack_size(WORKER_STACK)
        .spawn(|| (write_nested(SLATE_NEST_LIMIT), write_nested(SLATE_NEST_LIMIT + 1)))
        .unwrap()
        .join()
        .unwrap();
    assert!(kept.is_empty(), "{kept:?}");
    assert_eq!(refused, ["S500"]);
}

fn with_stack(body: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new().stack_size(WORKER_STACK).spawn(body).expect("a thread").join().expect("the body held");
}

fn run_child(name: &str) {
    if let Some((_, make)) = cases().into_iter().find(|(n, _)| *n == name) {
        with_stack(move || through_everything(&make()));
    } else if let Some((_, make)) = json_cases().into_iter().find(|(n, _)| *n == name) {
        with_stack(move || {
            let doc = make();
            let r = validate_slate(&doc);
            if let Some(d) = r.document {
                let _ = print_slate(&d);
                let _ = serde_json::to_string(&d);
                let _ = slate_start_values(&d);
            }
            std::mem::forget(doc);
        });
    } else {
        panic!("no case {name}");
    }
}

#[test]
fn no_case_takes_the_process_down() {
    if let Ok(name) = std::env::var(CHILD) {
        run_child(&name);
        return;
    }
    let names: Vec<&str> = cases().iter().map(|(n, _)| *n).chain(json_cases().iter().map(|(n, _)| *n)).collect();
    let mut died = Vec::new();
    for name in names {
        let out = std::process::Command::new(std::env::current_exe().expect("this binary"))
            .args(["--exact", "no_case_takes_the_process_down", "--nocapture", "--test-threads=1"])
            .env(CHILD, name)
            .output()
            .expect("the child starts");
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            let tail = err.lines().rfind(|l| !l.trim().is_empty()).unwrap_or("").to_string();
            died.push(format!("{name}: {} {tail}", out.status));
        }
    }
    assert!(died.is_empty(), "these inputs took the process down:\n{}", died.join("\n"));
}
