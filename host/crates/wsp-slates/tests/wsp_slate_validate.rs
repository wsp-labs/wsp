// SPDX-License-Identifier: AGPL-3.0-only
//! The validator and the patch of the slate module (packages/protocol/src/slate/validate.ts and patch.ts at 65f6b89c7),
//! tested where they need no parser: a document as JSON in, the checked answer out. Each expectation is what the TS
//! answers for the same document. The fixtures are validate.test.ts's FIXTURES, with the documents the TS parser
//! writes for them; the fixtures that raise their code only while parsing are not here.

use indexmap::IndexMap;
use serde_json::Value;
use wsp_slates::pending::slate::{apply_slate_patch, validate_slate, SlateProblem, SlateValues, SLATE_CODES};

type Row = (String, String, Option<String>, Option<String>, Option<String>, Option<i64>);

fn row(code: &str, message: &str, piece: Option<&str>, prop: Option<&str>, fix: Option<&str>, line: Option<i64>) -> Row {
    (code.to_string(), message.to_string(), piece.map(str::to_string), prop.map(str::to_string), fix.map(str::to_string), line)
}

fn shown(list: &[SlateProblem]) -> Vec<Row> {
    list.iter().map(|p| (p.code.clone(), p.message.clone(), p.piece.clone(), p.prop.clone(), p.fix.clone(), p.line)).collect()
}

fn with_defaults(doc: &Value) -> Value {
    let mut out = doc.clone();
    for key in ["values", "derived", "runs"] {
        if out.get(key).is_none() {
            out[key] = serde_json::json!({});
        }
    }
    if out.get("reactions").is_none() {
        out["reactions"] = serde_json::json!([]);
    }
    out
}

fn check(doc: &str, errors: Vec<Row>, warnings: Vec<Row>) {
    let doc: Value = serde_json::from_str(doc).expect("a case's document is JSON");
    let got = validate_slate(&doc);
    assert_eq!(shown(&got.errors), errors, "errors");
    assert_eq!(shown(&got.warnings), warnings, "warnings");
    assert_eq!(got.document.is_some(), errors.is_empty(), "a document comes back exactly when nothing is an error");
    if let Some(document) = got.document {
        assert_eq!(
            serde_json::to_value(&document).expect("the document writes"),
            with_defaults(&doc),
            "the document reads back as written"
        );
    }
}

/// A document whose one value holds a start of `len` characters: the case writes the long text itself.
fn check_big_start(doc: &str, len: usize, errors: Vec<Row>, warnings: Vec<Row>) {
    let mut doc: Value = serde_json::from_str(doc).expect("a case's document is JSON");
    doc["values"]["big"]["start"] = Value::String("x".repeat(len));
    let got = validate_slate(&doc);
    assert_eq!(shown(&got.errors), errors, "errors");
    assert_eq!(shown(&got.warnings), warnings, "warnings");
    assert_eq!(got.document.is_some(), errors.is_empty(), "a document comes back exactly when nothing is an error");
    if let Some(document) = got.document {
        assert_eq!(
            serde_json::to_value(&document).expect("the document writes"),
            with_defaults(&doc),
            "the document reads back as written"
        );
    }
}

fn json_of(text: &str) -> Value {
    serde_json::from_str(text).expect("an expectation is JSON")
}

enum Want {
    Refused(Vec<Row>, Vec<Row>),
    Cleared(&'static str),
    Applied(&'static str, &'static str, Vec<Row>),
}

fn run_patch(base: Option<&str>, live: &str, ops: &str, lines: &str, want: Want) {
    let doc = base.map(|text| {
        let parsed: Value = serde_json::from_str(text).expect("the base is JSON");
        validate_slate(&parsed).document.expect("the base document is valid")
    });
    let values: SlateValues = serde_json::from_str(live).expect("the live values are a JSON object");
    let ops: Vec<Value> = serde_json::from_str(ops).expect("the ops are a JSON array");
    let lines = json_of(lines);
    let got = apply_slate_patch(doc.as_ref(), &values, &ops, &lines);
    match want {
        Want::Refused(errors, warnings) => {
            assert!(got.document.is_none(), "a refused patch answers no document");
            assert!(got.values.is_none(), "a refused patch answers no values");
            assert_eq!(shown(&got.errors), errors, "errors");
            assert_eq!(shown(&got.warnings), warnings, "warnings");
        }
        Want::Cleared(values_json) => {
            assert_eq!(got.document, Some(None), "a clear answers no document");
            let kept: IndexMap<String, Value> = got.values.expect("a clear keeps the values");
            assert_eq!(serde_json::to_value(kept).expect("values write"), json_of(values_json));
        }
        Want::Applied(doc_json, values_json, warnings) => {
            let document = got.document.clone().flatten().expect("the patch answers a document");
            assert_eq!(serde_json::to_value(&document).expect("the document writes"), json_of(doc_json), "document");
            let kept: IndexMap<String, Value> = got.values.clone().expect("the patch keeps values");
            assert_eq!(serde_json::to_value(kept).expect("values write"), json_of(values_json), "values");
            assert!(got.errors.is_empty(), "an applied patch has no errors");
            assert_eq!(shown(&got.warnings), warnings, "warnings");
        }
    }
}

#[path = "wsp_slate_validate/fixtures_a.rs"]
mod fixtures_a;
#[path = "wsp_slate_validate/fixtures_b.rs"]
mod fixtures_b;
#[path = "wsp_slate_validate/patches.rs"]
mod patches;
