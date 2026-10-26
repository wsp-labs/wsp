// SPDX-License-Identifier: AGPL-3.0-only
//! Review driver: runs the slate parser, validator and patch over a JSON list of cases and writes one JSON line per
//! case, in the shape /root/rv2448/diff/ts-driver.ts writes for the TS. REVIEW_IN and REVIEW_OUT name the files.

use serde_json::{json, Map, Value};
use wsp_slates::pending::slate::*;

fn v<T: serde::Serialize>(x: &T) -> Value {
    serde_json::to_value(x).expect("serializes")
}

fn one(c: &Value) -> Value {
    let kind = c["kind"].as_str().unwrap_or("");
    match kind {
        "parse" => {
            let text = c["text"].as_str().unwrap_or("");
            let r = parse_slate(text);
            let comp = compile_slate_text(text);
            let mut row = Map::new();
            row.insert("errors".into(), v(&r.errors));
            row.insert("warnings".into(), v(&r.warnings));
            row.insert("document".into(), r.document.as_ref().map_or(Value::Null, v));
            row.insert("lines".into(), v(&comp.lines));
            row.insert("compileErrors".into(), v(&comp.errors));
            if let Some(doc) = &r.document {
                let printed = print_slate(doc);
                let again = parse_slate(&printed);
                row.insert("printed".into(), Value::String(printed));
                row.insert("reparsedSame".into(), Value::Bool(again.document.as_ref().map(v) == Some(v(doc))));
                row.insert("startValues".into(), v(&slate_start_values(doc)));
                let sources: Value = serde_json::from_str(&std::fs::read_to_string("/root/rv2448/diff/sources.json").unwrap()).unwrap();
                let resolve: SlateResolve = std::rc::Rc::new(move |path: &str| {
                    let mut cur = &sources;
                    for k in path.split(['.', '[', ']']).filter(|k| !k.is_empty()) {
                        cur = match cur {
                            Value::Object(m) => m.get(k)?,
                            Value::Array(a) => a.get(k.parse::<usize>().ok()?)?,
                            _ => return None,
                        };
                    }
                    Some(cur.clone())
                });
                let o = SketchOptions {
                    eval: SlateEvalContext { now: 1761480000000, item: None, index: None, resolve },
                    version: 1,
                    warnings: r.warnings.clone(),
                    check: false,
                    problems: vec![],
                    waiting: vec![],
                };
                row.insert("sketch".into(), Value::String(sketch_slate(Some(doc), &slate_start_values(doc), o)));
            }
            Value::Object(row)
        }
        "validate" => {
            let r = validate_slate(&c["json"]);
            json!({ "errors": v(&r.errors), "warnings": v(&r.warnings), "document": r.document.as_ref().map_or(Value::Null, v) })
        }
        "patch" => {
            let base = c["base"].as_str().and_then(|b| parse_slate(b).document);
            let p = parse_slate_patch(c["text"].as_str().unwrap_or(""), base.as_ref());
            let mut row = Map::new();
            row.insert("errors".into(), v(&p.errors));
            row.insert("patch".into(), p.ops.as_ref().map_or(Value::Null, |ops| json!({ "ops": ops })));
            row.insert("lines".into(), p.lines.clone());
            if let Some(ops) = &p.ops {
                let values = base.as_ref().map(slate_start_values).unwrap_or_default();
                let a = apply_slate_patch(base.as_ref(), &values, ops, &p.lines);
                let mut ap = Map::new();
                if let Some(d) = &a.document {
                    ap.insert("document".into(), d.as_ref().map_or(Value::Null, v));
                }
                if let Some(vals) = &a.values {
                    ap.insert("values".into(), v(vals));
                }
                ap.insert("errors".into(), v(&a.errors));
                ap.insert("warnings".into(), v(&a.warnings));
                row.insert("applied".into(), Value::Object(ap));
            }
            Value::Object(row)
        }
        other => json!({ "unknownKind": other }),
    }
}

#[test]
#[ignore = "review driver; set REVIEW_IN and REVIEW_OUT"]
fn review_differential_driver() {
    let input = std::env::var("REVIEW_IN").expect("REVIEW_IN");
    let output = std::env::var("REVIEW_OUT").expect("REVIEW_OUT");
    let cases: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(input).unwrap()).unwrap();
    use std::io::Write;
    let mut out = std::fs::File::create(output).unwrap();
    for c in &cases {
        let row = std::panic::catch_unwind(|| one(c)).unwrap_or_else(|_| json!({ "panicked": true }));
        writeln!(out, "{}", serde_json::to_string(&row).unwrap()).unwrap();
        out.flush().unwrap();
    }
}

// ---- findings, each pinned to the TS's own output under node at 65f6b89c7 ----

// syntax.ts:281 makes {schema, root, values, derived, runs, reactions, pieces} and :309 sets title after it;
// syntax.ts:671-679 writes a piece "type, when, props, children, on, fallback"; :434 writes a cmd run as kind, cmd,
// the common attributes (every first), then env, args, stdin, on, cwd, timeout, stream. slates.ts stores and sends
// that object (SlateView.document, wire.ts:109); ruling 1 keeps the TS's key order on disk and the wire.
#[test]
fn a_text_written_document_keeps_the_ts_key_order() {
    let text = r#"<slate title="T"><run name="r" cmd="ls" every={60} timeout={20} /><column><text id="a" when={$r.exit == 0}>x</text></column></slate>"#;
    let doc = parse_slate(text).document.expect("a document");
    assert_eq!(
        serde_json::to_string(&doc).unwrap(),
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{"r":{"kind":"cmd","cmd":"ls","every":60,"timeout":20}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["a"]},"a":{"type":"text","when":"$r.exit == 0","props":{"value":"x"}}},"title":"T"}"#
    );
}

// syntax.ts:297-298 writes slateProblem(code, message, { line, ...extra }) and problems.ts:54-57 keeps that order,
// so a syntax refusal's errors read line before piece and prop; the Rust struct writes line after them.
#[test]
fn a_syntax_problem_writes_line_before_piece_as_the_ts_does() {
    let r = parse_slate(r#"<slate><value name="v" start={1} /><button label="a" onPress={X($v, 1)} /></slate>"#);
    assert_eq!(
        serde_json::to_string(&r.errors).unwrap(),
        r#"[{"code":"A600","name":"step-unknown","message":"X is not a step; steps are set, toggle, start, cancel, send, steer, queue, fill, open, copy, pane","line":1,"piece":"button-1","prop":"onPress"}]"#
    );
}

// syntax.ts:261 reads 1e400 with Number() as Infinity, so the validator types $rows as a number and refuses it as items
// (validate.ts X408); the port reads it as null and takes the slate.
#[test]
fn a_start_past_the_double_range_is_a_number_and_items_refuses_it() {
    let r = parse_slate(
        r#"<slate><value name="rows" start={1e400} /><column><bars id="h" label="H" items={$rows} name={item.n} value={item.v} /></column></slate>"#,
    );
    assert_eq!(
        serde_json::to_string(&r.errors).unwrap(),
        r#"[{"code":"X408","name":"expr-type","message":"items takes a list; this formula gives a number","piece":"h","prop":"items","line":1}]"#
    );
}

// validate.ts:315's env-name fix replaces with a regex without the u flag, so each UTF-16 unit of an astral character
// becomes one _: "A😀(" is fixed as "A___".
#[test]
fn an_env_name_fix_replaces_each_utf16_unit() {
    let r = parse_slate(
        r#"<slate><secret name="tok" /><run name="r" cmd="echo" env={{ "A😀(": $tok }} /><column><text>x</text></column></slate>"#,
    );
    assert_eq!(r.errors.first().and_then(|e| e.fix.clone()).as_deref(), Some("A___"));
}

/// Runs one ignored test of this file in a child process, so a stack overflow that aborts it fails only the caller.
fn in_child(name: &str) -> std::process::Output {
    std::process::Command::new(std::env::current_exe().unwrap()).args(["--exact", name, "--ignored", "--test-threads=1"]).output().unwrap()
}

/// Parses on a thread with tokio's default worker stack (2 MiB), as the host would.
fn parse_on_worker_stack(text: String) -> SlateChecked {
    std::thread::Builder::new().stack_size(2 << 20).spawn(move || parse_slate(&text)).unwrap().join().unwrap()
}

// syntax.ts:236-262 reads a start literal by recursion and :748 turns its throw, node's RangeError included, into
// S503 ("start takes a literal ...: Maximum call stack size exceeded"); 5,000 brackets is 10 KB, under the 64 KB cap.
// The port's literal reader (syntax/literal.rs) recurses with no depth cap, and on a 2 MiB stack the overflow aborts
// the whole host, in a release build as in a debug one.
#[test]
fn a_start_nested_5000_deep_is_refused_s503_and_the_host_lives() {
    let out = in_child("child_a_start_nested_5000_deep");
    assert!(out.status.success(), "the parse aborted the process: {:?}\n{}", out.status, String::from_utf8_lossy(&out.stderr));
}

#[test]
#[ignore = "run by a_start_nested_5000_deep_is_refused_s503_and_the_host_lives"]
fn child_a_start_nested_5000_deep() {
    let n = 5000;
    let text = format!(
        r#"<slate title="W"><value name="a" start={{{}{}}} /><column><text>x</text></column></slate>"#,
        "[".repeat(n),
        "]".repeat(n)
    );
    let r = parse_on_worker_stack(text);
    assert_eq!(r.errors.first().map(|e| e.code.as_str()), Some("S503"));
}

// The same reader over records: node throws RangeError out of parseSlate at 2,500 deep, which slates.ts's write
// turns into a failed call; the host goes on. The port must not abort.
#[test]
fn a_start_record_nested_2500_deep_does_not_take_the_host_down() {
    let out = in_child("child_a_start_record_nested_2500_deep");
    assert!(out.status.success(), "the parse aborted the process: {:?}\n{}", out.status, String::from_utf8_lossy(&out.stderr));
}

#[test]
#[ignore = "run by a_start_record_nested_2500_deep_does_not_take_the_host_down"]
fn child_a_start_record_nested_2500_deep() {
    let n = 2500;
    let text = format!(
        r#"<slate title="W"><value name="a" start={{{}1{}}} /><column><text>x</text></column></slate>"#,
        "{a:".repeat(n),
        "}".repeat(n)
    );
    let _ = parse_on_worker_stack(text);
}
