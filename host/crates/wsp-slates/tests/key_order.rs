// SPDX-License-Identifier: AGPL-3.0-only
//! The key order a document is written in follows the road it came by (syntax.ts:281-309, 434, 671-679 for text, the zod
//! schema for JSON) and survives a trip through the store, which reads a row's keys back as they were written.
use serde_json::{json, Value};
use wsp_slates::pending::slate::*;
use wsp_slates::slates::SlateSnap;

const TEXT: &str = r#"<slate title="T"><run name="r" cmd="ls" every={60} confirm="Sure?" env={{ A: 1 }} timeout={20} /><run name="t" tool="s.t" args={{ a: 1 }} description="d" every={30} /><column><text id="a" when={$r.exit == 0}>x</text></column></slate>"#;

fn text_doc() -> SlateDoc {
    let r = parse_slate(TEXT);
    r.document.unwrap_or_else(|| panic!("{:?}", r.errors))
}

fn keys(v: &Value) -> Vec<&str> {
    v.as_object().expect("an object").keys().map(String::as_str).collect()
}

#[test]
fn a_text_document_writes_runs_and_pieces_in_the_text_order() {
    let doc = text_doc();
    let v = serde_json::to_value(&doc).unwrap();
    assert_eq!(keys(&v), ["schema", "root", "values", "derived", "runs", "reactions", "pieces", "title"]);
    assert_eq!(keys(&v["runs"]["r"]), ["kind", "cmd", "every", "confirm", "env", "timeout"]);
    assert_eq!(keys(&v["runs"]["t"]), ["kind", "server", "tool", "every", "description", "args"]);
    assert_eq!(keys(&v["pieces"]["a"]), ["type", "when", "props"]);
}

#[test]
fn a_json_document_writes_the_schema_order_whatever_order_it_came_in() {
    let doc = json!({
        "pieces": { "a": { "props": { "value": "x" }, "when": "$r.exit == 0", "type": "text" } },
        "title": "T", "root": "a", "schema": 2,
        "runs": { "r": { "every": 60, "cmd": "ls", "kind": "cmd", "timeout": 20 } },
    });
    let checked = validate_slate(&doc);
    let got = checked.document.unwrap_or_else(|| panic!("{:?}", checked.errors));
    let v = serde_json::to_value(&got).unwrap();
    assert_eq!(keys(&v), ["schema", "title", "root", "values", "derived", "runs", "reactions", "pieces"]);
    assert_eq!(keys(&v["runs"]["r"]), ["kind", "cmd", "timeout", "every"]);
    assert_eq!(keys(&v["pieces"]["a"]), ["type", "props", "when"]);
}

#[test]
fn a_row_read_back_is_written_as_it_was_read() {
    let text_doc = text_doc();
    let schema_doc: SlateDoc = serde_json::from_value(json!({
        "schema": 2, "title": "T", "root": "a", "values": {}, "derived": {}, "runs": {}, "reactions": [],
        "pieces": { "a": { "type": "text", "props": { "value": "x" }, "when": "1 == 1" } },
    }))
    .unwrap();
    for doc in [text_doc, schema_doc] {
        let snap = SlateSnap { document: Some(doc), values: Default::default(), empty: None };
        let written = serde_json::to_string(&snap).unwrap();
        let read: SlateSnap = serde_json::from_str(&written).unwrap();
        assert_eq!(serde_json::to_string(&read).unwrap(), written);
    }
}

#[test]
fn a_patched_document_keeps_the_order_of_the_one_it_patched() {
    let base = text_doc();
    let p = parse_slate_patch(r#"<add under="root" at={1}><text id="b" when={$r.exit == 1}>y</text></add>"#, Some(&base));
    let applied = apply_slate_patch(Some(&base), &slate_start_values(&base), &p.ops.expect("ops"), &p.lines);
    let doc = applied.document.and_then(|d| d).unwrap_or_else(|| panic!("{:?}", applied.errors));
    let v = serde_json::to_value(&doc).unwrap();
    assert_eq!(keys(&v)[7], "title");
}

#[test]
fn a_syntax_error_writes_line_first_and_a_validator_error_after_the_prop() {
    let syntax = parse_slate(r#"<slate><value name="v" start={1} /><button label="a" onPress={X($v, 1)} /></slate>"#);
    let v = serde_json::to_value(&syntax.errors[0]).unwrap();
    assert_eq!(keys(&v), ["code", "name", "message", "line", "piece", "prop"]);
    let checked = parse_slate(
        r#"<slate><value name="rows" start={1} /><column><bars id="h" label="H" items={$rows} name={item.n} value={item.v} /></column></slate>"#,
    );
    let v = serde_json::to_value(&checked.errors[0]).unwrap();
    assert_eq!(keys(&v), ["code", "name", "message", "piece", "prop", "line"]);
}
