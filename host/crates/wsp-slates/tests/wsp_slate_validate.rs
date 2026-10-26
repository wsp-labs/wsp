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

#[test]
fn hostile_object_prototype_names_are_refused_as_pieces() {
    for name in [
        "__defineGetter__",
        "__defineSetter__",
        "__lookupGetter__",
        "__lookupSetter__",
        "__proto__",
        "constructor",
        "hasOwnProperty",
        "isPrototypeOf",
        "propertyIsEnumerable",
        "toLocaleString",
        "toString",
        "valueOf",
    ] {
        let doc = serde_json::json!({"version": 1, "root": {"id": "r", "type": name, "props": {}}});
        assert!(!validate_slate(&doc).errors.is_empty(), "{name}");
    }
}

#[test]
fn fixture_005_d206() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["column-2"]},"column-2":{"type":"column","children":["column-3"]},"column-3":{"type":"column","children":["column-4"]},"column-4":{"type":"column","children":["column-5"]},"column-5":{"type":"column","children":["column-6"]},"column-6":{"type":"column","children":["column-7"]},"column-7":{"type":"column","children":["column-8"]},"column-8":{"type":"column","children":["column-9"]},"column-9":{"type":"column","children":["column-10"]},"column-10":{"type":"column","children":["column-11"]},"column-11":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}}}"#,
        vec![row("D206", "the tree is deeper than 10", Some("column-11"), None, None, None)],
        vec![],
    );
}

#[test]
fn fixture_006_d207() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1","text-2","text-3","text-4","text-5","text-6","text-7","text-8","text-9","text-10","text-11","text-12","text-13","text-14","text-15","text-16","text-17","text-18","text-19","text-20","text-21","text-22","text-23","text-24","text-25","text-26","text-27","text-28","text-29","text-30","text-31","text-32","text-33","text-34","text-35","text-36","text-37","text-38","text-39","text-40","text-41","text-42","text-43","text-44","text-45","text-46","text-47","text-48","text-49","text-50","text-51","text-52","text-53","text-54","text-55","text-56","text-57","text-58","text-59","text-60","text-61","text-62","text-63","text-64","text-65","text-66","text-67","text-68","text-69","text-70","text-71","text-72","text-73","text-74","text-75","text-76","text-77","text-78","text-79","text-80","text-81","text-82","text-83","text-84","text-85","text-86","text-87","text-88","text-89","text-90","text-91","text-92","text-93","text-94","text-95","text-96","text-97","text-98","text-99","text-100","text-101"]},"text-1":{"type":"text","props":{"value":"x"}},"text-2":{"type":"text","props":{"value":"x"}},"text-3":{"type":"text","props":{"value":"x"}},"text-4":{"type":"text","props":{"value":"x"}},"text-5":{"type":"text","props":{"value":"x"}},"text-6":{"type":"text","props":{"value":"x"}},"text-7":{"type":"text","props":{"value":"x"}},"text-8":{"type":"text","props":{"value":"x"}},"text-9":{"type":"text","props":{"value":"x"}},"text-10":{"type":"text","props":{"value":"x"}},"text-11":{"type":"text","props":{"value":"x"}},"text-12":{"type":"text","props":{"value":"x"}},"text-13":{"type":"text","props":{"value":"x"}},"text-14":{"type":"text","props":{"value":"x"}},"text-15":{"type":"text","props":{"value":"x"}},"text-16":{"type":"text","props":{"value":"x"}},"text-17":{"type":"text","props":{"value":"x"}},"text-18":{"type":"text","props":{"value":"x"}},"text-19":{"type":"text","props":{"value":"x"}},"text-20":{"type":"text","props":{"value":"x"}},"text-21":{"type":"text","props":{"value":"x"}},"text-22":{"type":"text","props":{"value":"x"}},"text-23":{"type":"text","props":{"value":"x"}},"text-24":{"type":"text","props":{"value":"x"}},"text-25":{"type":"text","props":{"value":"x"}},"text-26":{"type":"text","props":{"value":"x"}},"text-27":{"type":"text","props":{"value":"x"}},"text-28":{"type":"text","props":{"value":"x"}},"text-29":{"type":"text","props":{"value":"x"}},"text-30":{"type":"text","props":{"value":"x"}},"text-31":{"type":"text","props":{"value":"x"}},"text-32":{"type":"text","props":{"value":"x"}},"text-33":{"type":"text","props":{"value":"x"}},"text-34":{"type":"text","props":{"value":"x"}},"text-35":{"type":"text","props":{"value":"x"}},"text-36":{"type":"text","props":{"value":"x"}},"text-37":{"type":"text","props":{"value":"x"}},"text-38":{"type":"text","props":{"value":"x"}},"text-39":{"type":"text","props":{"value":"x"}},"text-40":{"type":"text","props":{"value":"x"}},"text-41":{"type":"text","props":{"value":"x"}},"text-42":{"type":"text","props":{"value":"x"}},"text-43":{"type":"text","props":{"value":"x"}},"text-44":{"type":"text","props":{"value":"x"}},"text-45":{"type":"text","props":{"value":"x"}},"text-46":{"type":"text","props":{"value":"x"}},"text-47":{"type":"text","props":{"value":"x"}},"text-48":{"type":"text","props":{"value":"x"}},"text-49":{"type":"text","props":{"value":"x"}},"text-50":{"type":"text","props":{"value":"x"}},"text-51":{"type":"text","props":{"value":"x"}},"text-52":{"type":"text","props":{"value":"x"}},"text-53":{"type":"text","props":{"value":"x"}},"text-54":{"type":"text","props":{"value":"x"}},"text-55":{"type":"text","props":{"value":"x"}},"text-56":{"type":"text","props":{"value":"x"}},"text-57":{"type":"text","props":{"value":"x"}},"text-58":{"type":"text","props":{"value":"x"}},"text-59":{"type":"text","props":{"value":"x"}},"text-60":{"type":"text","props":{"value":"x"}},"text-61":{"type":"text","props":{"value":"x"}},"text-62":{"type":"text","props":{"value":"x"}},"text-63":{"type":"text","props":{"value":"x"}},"text-64":{"type":"text","props":{"value":"x"}},"text-65":{"type":"text","props":{"value":"x"}},"text-66":{"type":"text","props":{"value":"x"}},"text-67":{"type":"text","props":{"value":"x"}},"text-68":{"type":"text","props":{"value":"x"}},"text-69":{"type":"text","props":{"value":"x"}},"text-70":{"type":"text","props":{"value":"x"}},"text-71":{"type":"text","props":{"value":"x"}},"text-72":{"type":"text","props":{"value":"x"}},"text-73":{"type":"text","props":{"value":"x"}},"text-74":{"type":"text","props":{"value":"x"}},"text-75":{"type":"text","props":{"value":"x"}},"text-76":{"type":"text","props":{"value":"x"}},"text-77":{"type":"text","props":{"value":"x"}},"text-78":{"type":"text","props":{"value":"x"}},"text-79":{"type":"text","props":{"value":"x"}},"text-80":{"type":"text","props":{"value":"x"}},"text-81":{"type":"text","props":{"value":"x"}},"text-82":{"type":"text","props":{"value":"x"}},"text-83":{"type":"text","props":{"value":"x"}},"text-84":{"type":"text","props":{"value":"x"}},"text-85":{"type":"text","props":{"value":"x"}},"text-86":{"type":"text","props":{"value":"x"}},"text-87":{"type":"text","props":{"value":"x"}},"text-88":{"type":"text","props":{"value":"x"}},"text-89":{"type":"text","props":{"value":"x"}},"text-90":{"type":"text","props":{"value":"x"}},"text-91":{"type":"text","props":{"value":"x"}},"text-92":{"type":"text","props":{"value":"x"}},"text-93":{"type":"text","props":{"value":"x"}},"text-94":{"type":"text","props":{"value":"x"}},"text-95":{"type":"text","props":{"value":"x"}},"text-96":{"type":"text","props":{"value":"x"}},"text-97":{"type":"text","props":{"value":"x"}},"text-98":{"type":"text","props":{"value":"x"}},"text-99":{"type":"text","props":{"value":"x"}},"text-100":{"type":"text","props":{"value":"x"}},"text-101":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![row("D207", "column-1 has 101 children; the most is 100", Some("column-1"), None, Some("bind a list instead"), None)],
        vec![],
    );
}

#[test]
fn fixture_007_t300() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["metre-1"]},"metre-1":{"type":"metre","props":{"label":"x","value":1}}},"title":"T"}"#,
        vec![row(
            "T300",
            "\"metre\" is not a piece; did you mean meter? slate_catalog lists them",
            Some("metre-1"),
            None,
            Some("meter"),
            None,
        )],
        vec![],
    );
}

#[test]
fn fixture_008_t302() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["meter-1"]},"meter-1":{"type":"meter","props":{"label":"x","value":1,"maxx":3}}},"title":"T"}"#,
        vec![row("T302", "meter has no maxx; did you mean max?", Some("meter-1"), Some("maxx"), Some("max"), None)],
        vec![],
    );
}

#[test]
fn fixture_009_t303() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["meter-1"]},"meter-1":{"type":"meter","props":{"label":"x","value":"usage.week.percent"}}},"title":"T"}"#,
        vec![row(
            "T303",
            "value takes a number in braces, like value={5}, or a formula, like value={$count}; \"usage.week.percent\" is text",
            Some("meter-1"),
            Some("value"),
            None,
            None,
        )],
        vec![],
    );
}

#[test]
fn fixture_010_t304() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["meter-1"]},"meter-1":{"type":"meter","props":{"label":"x"}}},"title":"T"}"#,
        vec![row("T304", "meter \"meter-1\" needs value", Some("meter-1"), Some("value"), None, None)],
        vec![],
    );
}

#[test]
fn fixture_011_t305() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"size":{"bind":"pr.word"},"value":"x"}}},"title":"T"}"#,
        vec![row("T305", "size takes a literal, not a formula", Some("text-1"), Some("size"), None, None)],
        vec![],
    );
}

#[test]
fn fixture_012_t306() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"tone":"loud","value":"x"}}},"title":"T"}"#,
        vec![row(
            "T306",
            "tone is default, muted, good, warning, bad, info or accent; \"loud\" is not one",
            Some("text-1"),
            Some("tone"),
            Some("good"),
            None,
        )],
        vec![],
    );
}

#[test]
fn fixture_013_t307() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"on":{"start":false}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["toggle-1"]},"toggle-1":{"type":"toggle","props":{"value":{"bind":"$on"}}}},"title":"T"}"#,
        vec![row(
            "T307",
            "a toggle needs a label for people using a screen reader",
            Some("toggle-1"),
            Some("label"),
            Some("label=\"...\""),
            None,
        )],
        vec![],
    );
}

#[test]
fn fixture_014_t308() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["meter-1"]},"meter-1":{"type":"meter","props":{"label":"x","value":1},"children":["text-1"]},"text-1":{"type":"text","props":{"value":"y"}}},"title":"T"}"#,
        vec![row("T308", "meter holds no children", Some("meter-1"), None, None, None)],
        vec![],
    );
}

#[test]
fn fixture_015_t309() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["empty-1"]},"empty-1":{"type":"empty","props":{"title":"x"},"children":["button-1","button-2"]},"button-1":{"type":"button","props":{"label":"a"},"on":{"press":[{"do":"copy","text":"a"}]}},"button-2":{"type":"button","props":{"label":"b"},"on":{"press":[{"do":"copy","text":"b"}]}}},"title":"T"}"#,
        vec![row("T309", "empty takes at most 1 child", Some("empty-1"), None, None, None)],
        vec![],
    );
}

#[test]
fn fixture_016_t311() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["button-1","button-2"]},"button-1":{"type":"button","props":{"label":"a","variant":"primary"},"on":{"press":[{"do":"copy","text":"a"}]}},"button-2":{"type":"button","props":{"label":"b","variant":"primary"},"on":{"press":[{"do":"copy","text":"b"}]}}},"title":"T"}"#,
        vec![],
        vec![row(
            "T311",
            "primary on button-1, button-2; one per slate reads loud, the rest draw as default",
            Some("button-2"),
            None,
            None,
            None,
        )],
    );
}

#[test]
fn fixture_017_t312() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"color":"red","value":"x"}}},"title":"T"}"#,
        vec![row(
            "T312",
            "color is not a prop a slate can set; wsp draws the style. Use tone.",
            Some("text-1"),
            Some("color"),
            Some("tone"),
            None,
        )],
        vec![],
    );
}

#[test]
fn fixture_019_x400() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${1 +}"}}}},"title":"T"}"#,
        vec![row("X400", "the expression ended early", Some("text-1"), Some("value"), None, None)],
        vec![],
    );
}

#[test]
fn fixture_020_x401() {
    check(r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${pr.titel}"}}}},"title":"T"}"#, vec![row("X401", "pr.titel is not a path; pr has number, url, state, draft, base, branch, headOid, headSubject, mergeable, review, checks, additions, deletions, changedFiles, commits, behindBase, author, readAt, word, unread", Some("text-1"), Some("value"), None, None)], vec![]);
}

#[test]
fn fixture_021_x404() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${lenn(pr.checks)}"}}}},"title":"T"}"#,
        vec![row("X404", "lenn is not a function; did you mean len?", Some("text-1"), Some("value"), Some("len"), None)],
        vec![],
    );
}

#[test]
fn fixture_022_x405() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${short(pr.word)}"}}}},"title":"T"}"#,
        vec![row("X405", "short takes 2 arguments, got 1: short(s, n)", Some("text-1"), Some("value"), Some("short(s, n)"), None)],
        vec![],
    );
}

#[test]
fn fixture_023_x406() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${tokens(pr.word)}"}}}},"title":"T"}"#,
        vec![row("X406", "tokens takes a number; this gives text", Some("text-1"), Some("value"), Some("tokens(num(...))"), None)],
        vec![],
    );
}

#[test]
fn fixture_024_x407() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1}"}}}},"title":"T"}"#,
        vec![row("X407", "the expression is 521 characters; the most is 500", Some("text-1"), Some("value"), None, None)],
        vec![],
    );
}

#[test]
fn fixture_025_x408() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${pr.checks == 'fail' ? 'a' : 'b'}"}}}},"title":"T"}"#,
        vec![row(
            "X408",
            "a list never equals a string; use contains(), len() or a step",
            Some("text-1"),
            Some("value"),
            Some("contains(list, value), len(list) or list | where(item.field == value)"),
            None,
        )],
        vec![],
    );
}

#[test]
fn fixture_026_x409() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${item.name}"}}}},"title":"T"}"#,
        vec![row("X409", "item is only known inside a repeating piece or a step", Some("text-1"), Some("value"), None, None)],
        vec![],
    );
}

#[test]
fn fixture_027_x410() {
    check(r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["input-1"]},"input-1":{"type":"input","props":{"label":"x","value":{"bind":"pr.word"}}}},"title":"T"}"#, vec![row("X410", "value on input writes back, so it binds one plain value, not pr.word; bind a value of its own and keep a list with append in a <when> if you need one per item", Some("input-1"), Some("value"), Some("<value name=\"picked\" start={null} /> and value={$picked}"), None)], vec![]);
}

#[test]
fn fixture_028_x420() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${pr.checks.length}"}}}},"title":"T"}"#,
        vec![row("X420", "length is not a field; use len()", Some("text-1"), Some("value"), Some("len(x)"), None)],
        vec![],
    );
}

#[test]
fn fixture_029_q420() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${pr.checks | 3}"}}}},"title":"T"}"#,
        vec![row("Q420", "expected a step after | at column 13", Some("text-1"), Some("value"), None, None)],
        vec![],
    );
}

#[test]
fn fixture_030_q422() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${pr.checks | filter(item.done)}"}}}},"title":"T"}"#,
        vec![row("Q422", "filter is not a step; did you mean where?", Some("text-1"), Some("value"), Some("where"), None)],
        vec![],
    );
}

#[test]
fn fixture_031_q423() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${pr.checks | where()}"}}}},"title":"T"}"#,
        vec![row("Q423", "where takes 1 argument, got 0: where(cond)", Some("text-1"), Some("value"), Some("where(cond)"), None)],
        vec![],
    );
}

#[test]
fn fixture_032_q424() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${pr.number | count}"}}}},"title":"T"}"#,
        vec![row("Q424", "count takes a list; this is a number", Some("text-1"), Some("value"), None, None)],
        vec![],
    );
}

#[test]
fn fixture_033_q425() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${pr.checks | take(pr.number) | count}"}}}},"title":"T"}"#,
        vec![row("Q425", "take's count is a literal integer", Some("text-1"), Some("value"), Some("take(5)"), None)],
        vec![],
    );
}

#[test]
fn fixture_034_q426() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${pr.checks | skip(0) | skip(0) | skip(0) | skip(0) | skip(0) | skip(0) | skip(0) | skip(0) | skip(0) | skip(0) | skip(0) | skip(0) | skip(0) | count}"}}}},"title":"T"}"#,
        vec![row("Q426", "14 steps; the most is 12", Some("text-1"), Some("value"), None, None)],
        vec![],
    );
}

#[test]
fn fixture_036_s501() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${$nope}"}}}},"title":"T"}"#,
        vec![row("S501", "$nope is not declared; add <value name=\"nope\" start=... />", Some("text-1"), Some("value"), None, None)],
        vec![],
    );
}

#[test]
fn fixture_037_s502() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["checklist-1"]},"checklist-1":{"type":"checklist","props":{"items":{"bind":"pr.checks"},"title":{"bind":"item.name"},"done":{"bind":"item.state == 'pass'"},"editable":true}}},"title":"T"}"#,
        vec![row(
            "S502",
            "an editable checklist writes its ticks back, so items binds a value: items={$steps}",
            Some("checklist-1"),
            Some("items"),
            None,
            None,
        )],
        vec![],
    );
}

#[test]
fn fixture_039_s510() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{"a":"$b","b":"$a"},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![row(
            "S510",
            "derived values read each other in a cycle: $a reads $b reads $a",
            Some("$a"),
            Some("value"),
            Some("make one of them a value"),
            None,
        )],
        vec![],
    );
}

#[test]
fn fixture_040_s512() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{"d0":"1","d1":"1","d2":"1","d3":"1","d4":"1","d5":"1","d6":"1","d7":"1","d8":"1","d9":"1","d10":"1","d11":"1","d12":"1","d13":"1","d14":"1","d15":"1","d16":"1","d17":"1","d18":"1","d19":"1","d20":"1","d21":"1","d22":"1","d23":"1","d24":"1","d25":"1","d26":"1","d27":"1","d28":"1","d29":"1","d30":"1","d31":"1","d32":"1"},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![row("S512", "33 derived values; the most is 32", None, None, None, None)],
        vec![],
    );
}

#[test]
fn fixture_041_s513() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"v0":{"start":1},"v1":{"start":1},"v2":{"start":1},"v3":{"start":1},"v4":{"start":1},"v5":{"start":1},"v6":{"start":1},"v7":{"start":1},"v8":{"start":1},"v9":{"start":1},"v10":{"start":1},"v11":{"start":1},"v12":{"start":1},"v13":{"start":1},"v14":{"start":1},"v15":{"start":1},"v16":{"start":1},"v17":{"start":1},"v18":{"start":1},"v19":{"start":1},"v20":{"start":1},"v21":{"start":1},"v22":{"start":1},"v23":{"start":1},"v24":{"start":1},"v25":{"start":1},"v26":{"start":1},"v27":{"start":1},"v28":{"start":1},"v29":{"start":1},"v30":{"start":1},"v31":{"start":1},"v32":{"start":1},"v33":{"start":1},"v34":{"start":1},"v35":{"start":1},"v36":{"start":1},"v37":{"start":1},"v38":{"start":1},"v39":{"start":1},"v40":{"start":1},"v41":{"start":1},"v42":{"start":1},"v43":{"start":1},"v44":{"start":1},"v45":{"start":1},"v46":{"start":1},"v47":{"start":1},"v48":{"start":1},"v49":{"start":1},"v50":{"start":1},"v51":{"start":1},"v52":{"start":1},"v53":{"start":1},"v54":{"start":1},"v55":{"start":1},"v56":{"start":1},"v57":{"start":1},"v58":{"start":1},"v59":{"start":1},"v60":{"start":1},"v61":{"start":1},"v62":{"start":1},"v63":{"start":1},"v64":{"start":1}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![row("S513", "65 values; the most is 64", None, None, None, None)],
        vec![],
    );
}

#[test]
fn fixture_042_s520() {
    check(r#"{"schema":2,"root":"column-1","values":{"token":{"start":null,"secret":true}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${$token}"}}}},"title":"T"}"#, vec![row("S520", "$token is a secret; only a run's env or stdin, an input's value or a send's paths may name it, and a formula reads only its .set, .len and .at", Some("text-1"), Some("value"), Some("$token.set"), None)], vec![]);
}

#[test]
fn fixture_047_a606() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["button-1"]},"button-1":{"type":"button","props":{"label":"x"},"on":{"press":[{"do":"copy","text":"1"},{"do":"copy","text":"2"},{"do":"copy","text":"3"},{"do":"copy","text":"4"},{"do":"copy","text":"5"},{"do":"copy","text":"6"},{"do":"copy","text":"7"}]}}},"title":"T"}"#,
        vec![row(
            "A606",
            "7 steps in one handler; the most is 6",
            Some("button-1"),
            Some("onPress"),
            Some("split it: a second <when> on what the first one sets"),
            None,
        )],
        vec![],
    );
}

#[test]
fn fixture_048_a607() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{"ok":"1 == 1"},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["button-1"]},"button-1":{"type":"button","props":{"label":"x"},"on":{"press":[{"do":"set","path":"$ok","value":true}]}}},"title":"T"}"#,
        vec![row(
            "A607",
            "$ok is derived and cannot be written; write the value it reads, or make it a value",
            Some("button-1"),
            Some("onPress"),
            None,
            None,
        )],
        vec![],
    );
}

#[test]
fn fixture_049_a608() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"a":{"start":1}},"derived":{},"runs":{},"reactions":[{"on":{"change":["$a"]},"do":[{"do":"open","target":"https://x.example"}]}],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![row(
            "A608",
            "open runs in the window and goes on a press, not on a reaction",
            Some("reaction-1"),
            Some("do"),
            Some("onPress={open(...)}"),
            None,
        )],
        vec![],
    );
}

#[test]
fn fixture_050_a610() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"a":{"start":1},"b":{"start":1}},"derived":{},"runs":{},"reactions":[{"on":{"change":["$a"]},"do":[{"do":"set","path":"$b","value":{"bind":"$a"}}]},{"on":{"change":["$b"]},"do":[{"do":"set","path":"$a","value":{"bind":"$b"}}]}],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![row(
            "A610",
            "reactions and derived values form a cycle: when change of $a sets $b; when change of $b sets $a",
            None,
            None,
            Some("compute it as a <derived> value instead of setting it back"),
            None,
        )],
        vec![],
    );
}

#[test]
fn fixture_054_k702() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["button-1"]},"button-1":{"type":"button","props":{"label":"x"},"on":{"press":[{"do":"start","run":"nope"}]}}},"title":"T"}"#,
        vec![row("K702", "start names a run: $nope is not declared", Some("button-1"), Some("onPress"), None, None)],
        vec![],
    );
}

#[test]
fn fixture_055_k703() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{"r":{"kind":"cmd","cmd":"true","every":5}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![row("K703", "every is at least 10 s", Some("$r"), Some("every"), Some("every={10}"), None)],
        vec![],
    );
}

#[test]
fn fixture_056_k704() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{"r":{"kind":"cmd","cmd":"true","timeout":900}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![row("K704", "timeout is seconds up to 600", Some("$r"), Some("timeout"), Some("timeout={60}"), None)],
        vec![row(
            "W019",
            "nothing starts $r: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($r)} />"),
            None,
        )],
    );
}

#[test]
fn fixture_058_k707() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{"r0":{"kind":"cmd","cmd":"true"},"r1":{"kind":"cmd","cmd":"true"},"r2":{"kind":"cmd","cmd":"true"},"r3":{"kind":"cmd","cmd":"true"},"r4":{"kind":"cmd","cmd":"true"},"r5":{"kind":"cmd","cmd":"true"},"r6":{"kind":"cmd","cmd":"true"},"r7":{"kind":"cmd","cmd":"true"},"r8":{"kind":"cmd","cmd":"true"},"r9":{"kind":"cmd","cmd":"true"},"r10":{"kind":"cmd","cmd":"true"},"r11":{"kind":"cmd","cmd":"true"},"r12":{"kind":"cmd","cmd":"true"},"r13":{"kind":"cmd","cmd":"true"},"r14":{"kind":"cmd","cmd":"true"},"r15":{"kind":"cmd","cmd":"true"},"r16":{"kind":"cmd","cmd":"true"}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![row("K707", "17 runs; the most is 16", None, None, None, None)],
        vec![
            row(
                "W019",
                "nothing starts $r0: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r0)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r1: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r1)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r2: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r2)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r3: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r3)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r4: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r4)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r5: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r5)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r6: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r6)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r7: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r7)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r8: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r8)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r9: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r9)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r10: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r10)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r11: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r11)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r12: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r12)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r13: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r13)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r14: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r14)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r15: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r15)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r16: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r16)} />"),
                None,
            ),
        ],
    );
}

#[test]
fn fixture_059_w001() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"pr.checks"}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W001",
            "value is the literal text \"pr.checks\", which reads like a path",
            Some("text-1"),
            Some("value"),
            Some("value={pr.checks}"),
            None,
        )],
    );
}

#[test]
fn fixture_060_w003() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["bars-1"]},"bars-1":{"type":"bars","props":{"label":"x","items":{"bind":"machine.history.cpu"},"name":{"bind":"item.x"},"value":{"bind":"item.y"}}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W003",
            "machine.history.cpu is a series over time; draw it as a line",
            Some("bars-1"),
            Some("items"),
            Some("<chart>"),
            None,
        )],
    );
}

#[test]
fn fixture_061_w004() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"a \u2014 b"}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W004",
            "an em dash in the slate's words; use a comma, a colon or a full stop",
            Some("text-1"),
            Some("value"),
            None,
            None,
        )],
    );
}

#[test]
fn fixture_062_w011() {
    check(r#"{"schema":2,"root":"column-1","values":{"t":{"start":null,"secret":true}},"derived":{},"runs":{"r":{"kind":"cmd","cmd":"vercel ls --token \"$T\"","env":{"T":{"bind":"$t"}}}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#, vec![], vec![row("W011", "T carries the secret $t, and the command passes it after --token, as an argument ps can read; hand it on stdin={$t}, or let the program read T from its environment", Some("$r"), Some("cmd"), Some("drop --token \"$T\" and use stdin={$t} or the program's own T"), None), row("W019", "nothing starts $r: give it a button, a when, or every", None, None, Some("<button label=\"...\" onPress={start($r)} />"), None)]);
}

#[test]
fn fixture_063_w012() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"Market closed · Weekend close"}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W012",
            "value joins words with \"·\"; the kit separates things by layout, never a mark: give each its own piece, or join with a comma",
            Some("text-1"),
            Some("value"),
            Some("separate pieces (facts, chips, a row) or a comma"),
            None,
        )],
    );
}

#[test]
fn fixture_064_w013() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"hits":{"start":[{"n":1},{"n":2}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["chart-1"]},"chart-1":{"type":"chart","props":{"label":"Requests","items":{"bind":"$hits"},"x":{"bind":"index"},"value":{"bind":"item.n"}}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W013",
            "the chart's x is each row's index, so its axis reads 0 to the count; give each row its time, for example x={item.at}",
            Some("chart-1"),
            Some("x"),
            Some("x={item.at}"),
            None,
        )],
    );
}

#[test]
fn fixture_065_w014() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"mono":true,"value":"Kill the node process now"}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W014",
            "mono is for figures, ids, times and paths; this reads as a sentence, which reads better in the normal face",
            Some("text-1"),
            Some("mono"),
            Some("drop mono"),
            None,
        )],
    );
}

#[test]
fn stored_json_spec_example_is_clean() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"id":{"start":""},"repo":{"start":"wsp-labs/wsp"},"step":{"start":1}},"derived":{"ok":"$check.state == 'done' and $check.json.id != null"},"runs":{"check":{"kind":"cmd","cmd":"gh api \"repos/$REPO/issues/$ID\"","env":{"REPO":{"bind":"$repo"},"ID":{"bind":"$id"}},"timeout":20}},"reactions":[{"on":{"change":["$id"]},"do":[{"do":"start","run":"check"}]}],"pieces":{"column-1":{"type":"column","children":["issue","text-1","text-2","text-3","button-1"]},"issue":{"type":"input","props":{"label":"Issue number","value":{"bind":"$id"},"mono":true}},"text-1":{"type":"text","when":"$check.state == 'running'","props":{"tone":"muted","value":"Checking"}},"text-2":{"type":"text","when":"$check.state == 'failed'","props":{"tone":"bad","value":{"format":"No such issue: ${$check.err}"}}},"text-3":{"type":"text","when":"$ok","props":{"tone":"good","value":{"format":"${$check.json.title}"}}},"button-1":{"type":"button","when":"$ok","props":{"label":"Next","variant":"primary"},"on":{"press":[{"do":"set","path":"$step","value":2}]}}},"title":"Deploy setup"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn stored_json_ghost_child() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"id":{"start":""},"repo":{"start":"wsp-labs/wsp"},"step":{"start":1}},"derived":{"ok":"$check.state == 'done' and $check.json.id != null"},"runs":{"check":{"kind":"cmd","cmd":"gh api \"repos/$REPO/issues/$ID\"","env":{"REPO":{"bind":"$repo"},"ID":{"bind":"$id"}},"timeout":20}},"reactions":[{"on":{"change":["$id"]},"do":[{"do":"start","run":"check"}]}],"pieces":{"column-1":{"type":"column","children":["issue","text-1","text-2","text-3","button-1","ghost"]},"issue":{"type":"input","props":{"label":"Issue number","value":{"bind":"$id"},"mono":true}},"text-1":{"type":"text","when":"$check.state == 'running'","props":{"tone":"muted","value":"Checking"}},"text-2":{"type":"text","when":"$check.state == 'failed'","props":{"tone":"bad","value":{"format":"No such issue: ${$check.err}"}}},"text-3":{"type":"text","when":"$ok","props":{"tone":"good","value":{"format":"${$check.json.title}"}}},"button-1":{"type":"button","when":"$ok","props":{"label":"Next","variant":"primary"},"on":{"press":[{"do":"set","path":"$step","value":2}]}}},"title":"Deploy setup"}"#,
        vec![row("D203", "column-1 holds ghost, which is not a piece", Some("column-1"), None, None, None)],
        vec![],
    );
}

#[test]
fn stored_json_root_child_cycle() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"id":{"start":""},"repo":{"start":"wsp-labs/wsp"},"step":{"start":1}},"derived":{"ok":"$check.state == 'done' and $check.json.id != null"},"runs":{"check":{"kind":"cmd","cmd":"gh api \"repos/$REPO/issues/$ID\"","env":{"REPO":{"bind":"$repo"},"ID":{"bind":"$id"}},"timeout":20}},"reactions":[{"on":{"change":["$id"]},"do":[{"do":"start","run":"check"}]}],"pieces":{"column-1":{"type":"column","children":["issue","text-1","text-2","text-3","column-1"]},"issue":{"type":"input","props":{"label":"Issue number","value":{"bind":"$id"},"mono":true}},"text-1":{"type":"text","when":"$check.state == 'running'","props":{"tone":"muted","value":"Checking"}},"text-2":{"type":"text","when":"$check.state == 'failed'","props":{"tone":"bad","value":{"format":"No such issue: ${$check.err}"}}},"text-3":{"type":"text","when":"$ok","props":{"tone":"good","value":{"format":"${$check.json.title}"}}},"button-1":{"type":"button","when":"$ok","props":{"label":"Next","variant":"primary"},"on":{"press":[{"do":"set","path":"$step","value":2}]}}},"title":"Deploy setup"}"#,
        vec![row("D205", "column-1 is under itself: column-1 > column-1", Some("column-1"), None, None, None)],
        vec![],
    );
}

#[test]
fn stored_json_schema_1() {
    check(
        r#"{"schema":1,"root":"column-1","values":{"id":{"start":""},"repo":{"start":"wsp-labs/wsp"},"step":{"start":1}},"derived":{"ok":"$check.state == 'done' and $check.json.id != null"},"runs":{"check":{"kind":"cmd","cmd":"gh api \"repos/$REPO/issues/$ID\"","env":{"REPO":{"bind":"$repo"},"ID":{"bind":"$id"}},"timeout":20}},"reactions":[{"on":{"change":["$id"]},"do":[{"do":"start","run":"check"}]}],"pieces":{"column-1":{"type":"column","children":["issue","text-1","text-2","text-3","button-1"]},"issue":{"type":"input","props":{"label":"Issue number","value":{"bind":"$id"},"mono":true}},"text-1":{"type":"text","when":"$check.state == 'running'","props":{"tone":"muted","value":"Checking"}},"text-2":{"type":"text","when":"$check.state == 'failed'","props":{"tone":"bad","value":{"format":"No such issue: ${$check.err}"}}},"text-3":{"type":"text","when":"$ok","props":{"tone":"good","value":{"format":"${$check.json.title}"}}},"button-1":{"type":"button","when":"$ok","props":{"label":"Next","variant":"primary"},"on":{"press":[{"do":"set","path":"$step","value":2}]}}},"title":"Deploy setup"}"#,
        vec![row("D200", "schema 1 is not a version this host knows; this host reads schema 2", None, None, Some("update wsp"), None)],
        vec![],
    );
}

#[test]
fn shape_missing_root() {
    check(r#"{"schema":2,"pieces":{"top":{"type":"column"}}}"#, vec![row("D202", "root: Required", None, None, None, None)], vec![]);
}

#[test]
fn shape_extra_and_child_bad() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"bogus":1,"values":{"v":{"start":1,"extra":2}}}"#,
        vec![
            row("T302", "$v has no extra; it takes start, secret and keep", Some("$v"), Some("extra"), None, None),
            row(
                "D202",
                "the document has no bogus; it holds schema, kit, title, root, values, derived, runs, reactions, pieces, files",
                None,
                None,
                None,
                None,
            ),
        ],
        vec![],
    );
}

#[test]
fn shape_run_no_kind() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"runs":{"r":{"cmd":"x"}}}"#,
        vec![row("K701", "$r takes kind \"cmd\", \"tool\" or \"resource\"", Some("$r"), Some("kind"), None, None)],
        vec![],
    );
}

#[test]
fn shape_run_bad_kind() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"runs":{"r":{"kind":"nope","cmd":"x"}}}"#,
        vec![row("K701", "$r takes kind \"cmd\", \"tool\" or \"resource\"", Some("$r"), Some("kind"), None, None)],
        vec![],
    );
}

#[test]
fn shape_run_cmd_number() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"runs":{"r":{"kind":"cmd","cmd":5}}}"#,
        vec![row("K704", "$r.cmd: Expected string, received number", Some("$r"), Some("cmd"), None, None)],
        vec![],
    );
}

#[test]
fn shape_run_confirm_empty_bind() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"runs":{"r":{"kind":"cmd","cmd":"x","confirm":{"bind":""}}}}"#,
        vec![row("K704", "$r.confirm.bind: String must contain at least 1 character(s)", Some("$r"), Some("confirm"), None, None)],
        vec![],
    );
}

#[test]
fn shape_reaction_on_empty() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"reactions":[{"on":{},"do":[]}]}"#,
        vec![row(
            "A611",
            "reaction 1.on: Invalid input; a <when> is { on: { change: [paths] } or { done: name }, do: [steps] }",
            Some("reaction-1"),
            None,
            None,
            None,
        )],
        vec![],
    );
}

#[test]
fn shape_reaction_step_bad() {
    check(r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"reactions":[{"on":{"change":["$a"]},"do":[{"do":"nope"}]}]}"#, vec![row("A611", "reaction 1.do.0.do: Invalid discriminator value. Expected 'set' | 'toggle' | 'start' | 'cancel' | 'send' | 'steer' | 'queue' | 'fill' | 'open' | 'copy' | 'pane'; a <when> is { on: { change: [paths] } or { done: name }, do: [steps] }", Some("reaction-1"), None, None, None)], vec![]);
}

#[test]
fn shape_step_set_no_value() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"reactions":[{"on":{"change":["$a"]},"do":[{"do":"set","path":"$a"}]}]}"#,
        vec![row(
            "A611",
            "reaction 1.do.0.value: Invalid input; a <when> is { on: { change: [paths] } or { done: name }, do: [steps] }",
            Some("reaction-1"),
            None,
            None,
            None,
        )],
        vec![],
    );
}

#[test]
fn shape_piece_type_number() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":5}}}"#,
        vec![row("T300", "top's type is the piece's name as a string", Some("top"), Some("type"), None, None)],
        vec![],
    );
}

#[test]
fn shape_piece_props_array() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","props":[1]}}}"#,
        vec![row("T303", "top.props: Expected object, received array", Some("top"), None, None, None)],
        vec![],
    );
}

#[test]
fn shape_kit_number() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"kit":5}"#,
        vec![row("D202", "kit: Expected string, received number", None, None, None, None)],
        vec![],
    );
}

#[test]
fn shape_value_no_start() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"values":{"v":{}}}"#,
        vec![row("T303", "$v.start: Invalid input", Some("$v"), Some("start"), None, None)],
        vec![],
    );
}

#[test]
fn shape_value_secret_false() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"values":{"v":{"start":1,"secret":false}}}"#,
        vec![row("T303", "$v.secret: Invalid literal value, expected true", Some("$v"), Some("secret"), None, None)],
        vec![],
    );
}

#[test]
fn shape_pane_bad_kind() {
    check(r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"reactions":[{"on":{"done":"x"},"do":[{"do":"pane","kind":"zzz"}]}]}"#, vec![row("A611", "reaction 1.do.0.kind: Invalid enum value. Expected 'preview' | 'terminal' | 'diff' | 'pr' | 'files' | 'machine' | 'processes' | 'agents' | 'slate', received 'zzz'; a <when> is { on: { change: [paths] } or { done: name }, do: [steps] }", Some("reaction-1"), None, None, None)], vec![]);
}

#[test]
fn shape_fallback_number() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","fallback":5}}}"#,
        vec![row("T303", "top.fallback: Invalid input", Some("top"), Some("fallback"), None, None)],
        vec![],
    );
}

#[test]
fn shape_derived_number() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"derived":{"a":5}}"#,
        vec![row("T303", "$a.decl: Expected string, received number", Some("$a"), None, None, None)],
        vec![],
    );
}

#[test]
fn shape_array_document() {
    check(
        r#"[1]"#,
        vec![row("D202", "a document is an object with schema, root, values, derived, runs, reactions and pieces", None, None, None, None)],
        vec![],
    );
}

#[test]
fn shape_top_unknown_and_root_number() {
    check(
        r#"{"schema":2,"bogus":1,"root":3,"pieces":{}}"#,
        vec![
            row("D202", "root: Expected string, received number", None, None, None, None),
            row(
                "D202",
                "the document has no bogus; it holds schema, kit, title, root, values, derived, runs, reactions, pieces, files",
                None,
                None,
                None,
                None,
            ),
        ],
        vec![],
    );
}

#[test]
fn shape_piece_on_bad_events() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","on":{"press":"x","zap":[]}}}}"#,
        vec![
            row(
                "A600",
                "top.on.press: a handler is a list of steps, Expected array, received string",
                Some("top"),
                Some("on.press"),
                None,
                None,
            ),
            row("A602", "top has no on.zap; events are press, submit and change", Some("top"), Some("on"), None, None),
        ],
        vec![],
    );
}

#[test]
fn shape_schema_string() {
    check(
        r#"{"schema":"x","root":"top"}"#,
        vec![row("D200", "schema \"x\" is not a version this host knows; this host reads schema 2", None, None, Some("update wsp"), None)],
        vec![],
    );
}

#[test]
fn shape_no_schema() {
    check(
        r#"{"root":"top"}"#,
        vec![row(
            "D200",
            "this JSON is not a slate: a slate is the JSX-like text slate_catalog shows, sent as text, never JSON of your own",
            None,
            None,
            Some("<slate title=\"Gold\"><number label=\"Spot\" value={$spot.json.usd} unit=\"USD\" /></slate>"),
            None,
        )],
        vec![],
    );
}

#[test]
fn shape_no_schema_markup_value() {
    check(
        r#"{"title":"x","body":"<slate><text>x</text></slate>"}"#,
        vec![row(
            "D200",
            "this JSON wraps slate markup; send the markup itself as text, not inside document",
            None,
            None,
            Some("text: \"<slate title=\"Gold\">...</slate>\""),
            None,
        )],
        vec![],
    );
}

#[test]
fn shape_markup_key_only() {
    check(
        r#"{"slate":"<slate><text>x</text></slate>"}"#,
        vec![row(
            "D200",
            "this JSON wraps slate markup; send the markup itself as text, not inside document",
            None,
            None,
            Some("text: \"<slate title=\"Gold\">...</slate>\""),
            None,
        )],
        vec![],
    );
}

#[test]
fn shape_schema_3() {
    check(
        r#"{"schema":3}"#,
        vec![row("D200", "schema 3 is not a version this host knows; this host reads schema 2", None, None, Some("update wsp"), None)],
        vec![],
    );
}

#[test]
fn shape_props_on_document_top() {
    check(r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"className":"x"}"#, vec![row("D202", "the document has no className; it holds schema, kit, title, root, values, derived, runs, reactions, pieces, files; a piece's props go under pieces.<id>.props", None, None, None, None)], vec![]);
}

#[test]
fn shape_run_tool_missing_server() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"runs":{"t":{"kind":"tool","tool":"x"}}}"#,
        vec![row("K704", "$t.server: Required", Some("$t"), Some("server"), None, None)],
        vec![],
    );
}

#[test]
fn shape_run_resource_missing_uri() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"runs":{"s":{"kind":"resource","server":"srv"}}}"#,
        vec![row("K704", "$s.uri: Required", Some("$s"), Some("uri"), None, None)],
        vec![],
    );
}

#[test]
fn shape_run_extra_key_alway() {
    check(r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"runs":{"r":{"kind":"cmd","cmd":"true","every":60,"alway":true}}}"#, vec![row("K704", "$r has no alway; it takes kind, cmd, env, args, stdin, on, cwd, timeout, stream, confirm, every, once, always, then and description", Some("$r"), Some("alway"), None, None)], vec![]);
}

#[test]
fn shape_piece_prop_beside_props() {
    check(
        r#"{"schema":2,"root":"col","pieces":{"col":{"type":"column","children":["t"]},"t":{"type":"text","tone":"bad","props":{"value":"x"}}}}"#,
        vec![row(
            "T302",
            "tone sits beside props on t; a piece holds type, props, children, when, on and fallback, and tone goes inside props",
            Some("t"),
            Some("tone"),
            Some("\"props\": { \"tone\": ... }"),
            None,
        )],
        vec![],
    );
}

#[test]
fn shape_reaction_both_on_keys() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"reactions":[{"on":{"change":["$a"],"done":"r"},"do":[]}]}"#,
        vec![row(
            "A611",
            "reaction 1.on: Unrecognized key(s) in object: 'done'; a <when> is { on: { change: [paths] } or { done: name }, do: [steps] }",
            Some("reaction-1"),
            None,
            None,
            None,
        )],
        vec![],
    );
}

#[test]
fn shape_reaction_id_number() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"reactions":[{"id":4,"on":{"change":["$a"]},"do":[]}]}"#,
        vec![row(
            "A611",
            "reaction 1.id: Expected string, received number; a <when> is { on: { change: [paths] } or { done: name }, do: [steps] }",
            Some("reaction-1"),
            None,
            None,
            None,
        )],
        vec![],
    );
}

#[test]
fn shape_run_env_not_record() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"runs":{"r":{"kind":"cmd","cmd":"x","env":[1]}}}"#,
        vec![row("K704", "$r.env: Expected object, received array", Some("$r"), Some("env"), None, None)],
        vec![],
    );
}

#[test]
fn shape_run_on_bad() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"runs":{"r":{"kind":"cmd","cmd":"x","on":"laptop"}}}"#,
        vec![row("K704", "$r.on: Invalid enum value. Expected 'thread' | 'host', received 'laptop'", Some("$r"), Some("on"), None, None)],
        vec![],
    );
}

#[test]
fn shape_run_secret_arg_refused() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"values":{"tok":{"start":null,"secret":true}},"runs":{"r":{"kind":"cmd","cmd":"echo","args":[{"bind":"$tok"}]}}}"#,
        vec![row(
            "S520",
            "args[0] is the secret $tok; a secret reaches a command through env or stdin, never as an argument, which ps can read",
            Some("$r"),
            Some("args[0]"),
            Some("env={{ TOK: $tok }}, or stdin={$tok}"),
            None,
        )],
        vec![row(
            "W019",
            "nothing starts $r: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($r)} />"),
            None,
        )],
    );
}

#[test]
fn shape_run_secret_env_ok() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"values":{"tok":{"start":null,"secret":true}},"runs":{"r":{"kind":"cmd","cmd":"echo","env":{"TOK":{"bind":"$tok"}}}}}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $r: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($r)} />"),
            None,
        )],
    );
}

#[test]
fn shape_run_flag_carries_secret() {
    check(r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"values":{"t":{"start":null,"secret":true}},"runs":{"r":{"kind":"cmd","cmd":"vercel ls --token \"$T\"","env":{"T":{"bind":"$t"}}}}}"#, vec![], vec![row("W011", "T carries the secret $t, and the command passes it after --token, as an argument ps can read; hand it on stdin={$t}, or let the program read T from its environment", Some("$r"), Some("cmd"), Some("drop --token \"$T\" and use stdin={$t} or the program's own T"), None), row("W019", "nothing starts $r: give it a button, a when, or every", None, None, Some("<button label=\"...\" onPress={start($r)} />"), None)]);
}

#[test]
fn shape_run_token_in_cmd() {
    check(r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"runs":{"a":{"kind":"cmd","cmd":"curl -H 'Authorization: Bearer xapt-dd233a3a-85a9-45d2-9a1c-a4608f2b9eeb' https://x"}}}"#, vec![row("S520", "cmd holds a token written out (xapt-d...), which the slate keeps and shows the person; let the command read it from the file it lives in, or take it from a <secret> the person types", Some("$a"), Some("cmd"), None, None)], vec![row("W019", "nothing starts $a: give it a button, a when, or every", None, None, Some("<button label=\"...\" onPress={start($a)} />"), None)]);
}

#[test]
fn shape_file_token() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"files":{"q.py":"KEY = \"sk-ant-api03-abcdefghijklmnopqrst\""}}"#,
        vec![row(
            "S520",
            "q.py holds a token written out (sk-ant...); let the code read it from the file it lives in, or from an env the run hands it",
            Some("q.py"),
            None,
            None,
            None,
        )],
        vec![],
    );
}

#[test]
fn shape_file_bad_name() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"files":{"../x.py":"print(1)"}}"#,
        vec![row(
            "K708",
            "\"../x.py\" is not a file name: letters, digits, dots, dashes and _, no slashes, not starting with a dot",
            Some("../x.py"),
            None,
            Some("x.py"),
            None,
        )],
        vec![],
    );
}

#[test]
fn shape_file_slate_dir_missing() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"runs":{"r":{"kind":"cmd","cmd":"python3 $SLATE_DIR/shape.py","every":60}}}"#,
        vec![],
        vec![row("W015", "$r runs $SLATE_DIR/shape.py, and no <file name=\"shape.py\"> is declared", Some("$r"), Some("cmd"), None, None)],
    );
}

#[test]
fn shape_derived_cycle() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"values":{"x":{"start":1}},"derived":{"a":"$b","b":"$a"}}"#,
        vec![row(
            "S510",
            "derived values read each other in a cycle: $a reads $b reads $a",
            Some("$a"),
            Some("value"),
            Some("make one of them a value"),
            None,
        )],
        vec![],
    );
}

#[test]
fn shape_reaction_cycle() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"values":{"a":{"start":1},"b":{"start":1}},"reactions":[{"on":{"change":["$a"]},"do":[{"do":"set","path":"$b","value":1}]},{"on":{"change":["$b"]},"do":[{"do":"set","path":"$a","value":1}]}]}"#,
        vec![row(
            "A610",
            "reactions and derived values form a cycle: when change of $a sets $b; when change of $b sets $a",
            None,
            None,
            Some("compute it as a <derived> value instead of setting it back"),
            None,
        )],
        vec![],
    );
}

#[test]
fn shape_done_names_non_run() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"values":{"v":{"start":1}},"reactions":[{"on":{"done":"v"},"do":[]}]}"#,
        vec![row("K702", "done names a run: $v is value", Some("reaction-1"), Some("done"), None, None)],
        vec![],
    );
}

#[test]
fn shape_when_per_row_literal() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":["c"]},"c":{"type":"chart","props":{"label":"x","items":{"bind":"$h"},"x":"t","value":"v"}}},"values":{"h":{"start":[]}}}"#,
        vec![
            row(
                "T303",
                "x is read per row, a formula over item: x={item.t}; \"t\" would be the same on every row",
                Some("c"),
                Some("x"),
                Some("x={item.t}"),
                None,
            ),
            row(
                "T303",
                "value is read per row, a formula over item: value={item.v}; \"v\" would be the same on every row",
                Some("c"),
                Some("value"),
                Some("value={item.v}"),
                None,
            ),
        ],
        vec![],
    );
}

#[test]
fn shape_title_case() {
    check(
        r#"{"schema":2,"title":"Live Gold Prices","root":"top","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"top":{"type":"column","children":[]}}}"#,
        vec![],
        vec![row(
            "W018",
            "the slate's title \"Live Gold Prices\" is in Title Case; heads take sentence case",
            None,
            None,
            Some("title=\"Live gold prices\""),
            None,
        )],
    );
}

#[test]
fn shape_reserved_prop_top() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","props":{"tone":"bad"}}}}"#,
        vec![row("T302", "column has no tone; it takes gap, align, pad, surface", Some("top"), Some("tone"), None, None)],
        vec![],
    );
}

#[test]
fn shape_depth_and_children() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":["a"]},"a":{"type":"column","children":["top"]}}}"#,
        vec![row("D205", "top is under itself: top > a > top", Some("top"), None, None, None)],
        vec![],
    );
}

#[test]
fn shape_fallback_drop() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":["a"]},"a":{"type":"sparkle","fallback":"drop"}}}"#,
        vec![],
        vec![],
    );
}

#[test]
fn shape_fallback_empty_text() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":["a"]},"a":{"type":"sparkle","fallback":{"text":"hi"}}}}"#,
        vec![],
        vec![],
    );
}

#[test]
fn shape_when_not_string() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","when":5}}}"#,
        vec![row("T303", "top.when: Expected string, received number", Some("top"), Some("when"), None, None)],
        vec![],
    );
}

#[test]
fn shape_on_handler_not_list() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","on":{"press":"x"}}}}"#,
        vec![row(
            "A600",
            "top.on.press: a handler is a list of steps, Expected array, received string",
            Some("top"),
            Some("on.press"),
            None,
            None,
        )],
        vec![],
    );
}

#[test]
fn shape_props_null_value() {
    check(r#"{"schema":2,"root":"top","pieces":{"top":{"type":"text","props":{"value":null}}}}"#, vec![], vec![]);
}

#[test]
fn shape_value_keep_on_value() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"values":{"v":{"start":1,"keep":true}}}"#,
        vec![row("T302", "keep is for a <secret>; $v is a value", Some("$v"), Some("start"), None, None)],
        vec![],
    );
}

#[test]
fn shape_secret_with_start() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"values":{"s":{"start":"typed","secret":true}}}"#,
        vec![row("S503", "$s is a secret and starts empty; the person types it", Some("$s"), Some("start"), None, None)],
        vec![],
    );
}

#[test]
fn shape_derived_limit() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"derived":{"d0":"1","d1":"1","d2":"1","d3":"1","d4":"1","d5":"1","d6":"1","d7":"1","d8":"1","d9":"1","d10":"1","d11":"1","d12":"1","d13":"1","d14":"1","d15":"1","d16":"1","d17":"1","d18":"1","d19":"1","d20":"1","d21":"1","d22":"1","d23":"1","d24":"1","d25":"1","d26":"1","d27":"1","d28":"1","d29":"1","d30":"1","d31":"1","d32":"1"}}"#,
        vec![row("S512", "33 derived values; the most is 32", None, None, None, None)],
        vec![],
    );
}

#[test]
fn shape_values_limit() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"values":{"v0":{"start":1},"v1":{"start":1},"v2":{"start":1},"v3":{"start":1},"v4":{"start":1},"v5":{"start":1},"v6":{"start":1},"v7":{"start":1},"v8":{"start":1},"v9":{"start":1},"v10":{"start":1},"v11":{"start":1},"v12":{"start":1},"v13":{"start":1},"v14":{"start":1},"v15":{"start":1},"v16":{"start":1},"v17":{"start":1},"v18":{"start":1},"v19":{"start":1},"v20":{"start":1},"v21":{"start":1},"v22":{"start":1},"v23":{"start":1},"v24":{"start":1},"v25":{"start":1},"v26":{"start":1},"v27":{"start":1},"v28":{"start":1},"v29":{"start":1},"v30":{"start":1},"v31":{"start":1},"v32":{"start":1},"v33":{"start":1},"v34":{"start":1},"v35":{"start":1},"v36":{"start":1},"v37":{"start":1},"v38":{"start":1},"v39":{"start":1},"v40":{"start":1},"v41":{"start":1},"v42":{"start":1},"v43":{"start":1},"v44":{"start":1},"v45":{"start":1},"v46":{"start":1},"v47":{"start":1},"v48":{"start":1},"v49":{"start":1},"v50":{"start":1},"v51":{"start":1},"v52":{"start":1},"v53":{"start":1},"v54":{"start":1},"v55":{"start":1},"v56":{"start":1},"v57":{"start":1},"v58":{"start":1},"v59":{"start":1},"v60":{"start":1},"v61":{"start":1},"v62":{"start":1},"v63":{"start":1},"v64":{"start":1}}}"#,
        vec![row("S513", "65 values; the most is 64", None, None, None, None)],
        vec![],
    );
}

#[test]
fn shape_runs_limit() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"runs":{"r0":{"kind":"cmd","cmd":"true"},"r1":{"kind":"cmd","cmd":"true"},"r2":{"kind":"cmd","cmd":"true"},"r3":{"kind":"cmd","cmd":"true"},"r4":{"kind":"cmd","cmd":"true"},"r5":{"kind":"cmd","cmd":"true"},"r6":{"kind":"cmd","cmd":"true"},"r7":{"kind":"cmd","cmd":"true"},"r8":{"kind":"cmd","cmd":"true"},"r9":{"kind":"cmd","cmd":"true"},"r10":{"kind":"cmd","cmd":"true"},"r11":{"kind":"cmd","cmd":"true"},"r12":{"kind":"cmd","cmd":"true"},"r13":{"kind":"cmd","cmd":"true"},"r14":{"kind":"cmd","cmd":"true"},"r15":{"kind":"cmd","cmd":"true"},"r16":{"kind":"cmd","cmd":"true"}}}"#,
        vec![row("K707", "17 runs; the most is 16", None, None, None, None)],
        vec![
            row(
                "W019",
                "nothing starts $r0: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r0)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r1: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r1)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r2: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r2)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r3: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r3)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r4: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r4)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r5: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r5)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r6: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r6)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r7: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r7)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r8: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r8)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r9: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r9)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r10: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r10)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r11: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r11)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r12: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r12)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r13: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r13)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r14: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r14)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r15: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r15)} />"),
                None,
            ),
            row(
                "W019",
                "nothing starts $r16: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r16)} />"),
                None,
            ),
        ],
    );
}

#[test]
fn shape_pieces_limit() {
    check(
        r#"{"schema":2,"root":"top","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"top":{"type":"column","children":["t0","t1","t2","t3","t4","t5","t6","t7","t8","t9","t10","t11","t12","t13","t14","t15","t16","t17","t18","t19","t20","t21","t22","t23","t24","t25","t26","t27","t28","t29","t30","t31","t32","t33","t34","t35","t36","t37","t38","t39","t40","t41","t42","t43","t44","t45","t46","t47","t48","t49","t50","t51","t52","t53","t54","t55","t56","t57","t58","t59","t60","t61","t62","t63","t64","t65","t66","t67","t68","t69","t70","t71","t72","t73","t74","t75","t76","t77","t78","t79","t80","t81","t82","t83","t84","t85","t86","t87","t88","t89","t90","t91","t92","t93","t94","t95","t96","t97","t98","t99","t100"]},"t0":{"type":"text","props":{"value":"x"}},"t1":{"type":"text","props":{"value":"x"}},"t2":{"type":"text","props":{"value":"x"}},"t3":{"type":"text","props":{"value":"x"}},"t4":{"type":"text","props":{"value":"x"}},"t5":{"type":"text","props":{"value":"x"}},"t6":{"type":"text","props":{"value":"x"}},"t7":{"type":"text","props":{"value":"x"}},"t8":{"type":"text","props":{"value":"x"}},"t9":{"type":"text","props":{"value":"x"}},"t10":{"type":"text","props":{"value":"x"}},"t11":{"type":"text","props":{"value":"x"}},"t12":{"type":"text","props":{"value":"x"}},"t13":{"type":"text","props":{"value":"x"}},"t14":{"type":"text","props":{"value":"x"}},"t15":{"type":"text","props":{"value":"x"}},"t16":{"type":"text","props":{"value":"x"}},"t17":{"type":"text","props":{"value":"x"}},"t18":{"type":"text","props":{"value":"x"}},"t19":{"type":"text","props":{"value":"x"}},"t20":{"type":"text","props":{"value":"x"}},"t21":{"type":"text","props":{"value":"x"}},"t22":{"type":"text","props":{"value":"x"}},"t23":{"type":"text","props":{"value":"x"}},"t24":{"type":"text","props":{"value":"x"}},"t25":{"type":"text","props":{"value":"x"}},"t26":{"type":"text","props":{"value":"x"}},"t27":{"type":"text","props":{"value":"x"}},"t28":{"type":"text","props":{"value":"x"}},"t29":{"type":"text","props":{"value":"x"}},"t30":{"type":"text","props":{"value":"x"}},"t31":{"type":"text","props":{"value":"x"}},"t32":{"type":"text","props":{"value":"x"}},"t33":{"type":"text","props":{"value":"x"}},"t34":{"type":"text","props":{"value":"x"}},"t35":{"type":"text","props":{"value":"x"}},"t36":{"type":"text","props":{"value":"x"}},"t37":{"type":"text","props":{"value":"x"}},"t38":{"type":"text","props":{"value":"x"}},"t39":{"type":"text","props":{"value":"x"}},"t40":{"type":"text","props":{"value":"x"}},"t41":{"type":"text","props":{"value":"x"}},"t42":{"type":"text","props":{"value":"x"}},"t43":{"type":"text","props":{"value":"x"}},"t44":{"type":"text","props":{"value":"x"}},"t45":{"type":"text","props":{"value":"x"}},"t46":{"type":"text","props":{"value":"x"}},"t47":{"type":"text","props":{"value":"x"}},"t48":{"type":"text","props":{"value":"x"}},"t49":{"type":"text","props":{"value":"x"}},"t50":{"type":"text","props":{"value":"x"}},"t51":{"type":"text","props":{"value":"x"}},"t52":{"type":"text","props":{"value":"x"}},"t53":{"type":"text","props":{"value":"x"}},"t54":{"type":"text","props":{"value":"x"}},"t55":{"type":"text","props":{"value":"x"}},"t56":{"type":"text","props":{"value":"x"}},"t57":{"type":"text","props":{"value":"x"}},"t58":{"type":"text","props":{"value":"x"}},"t59":{"type":"text","props":{"value":"x"}},"t60":{"type":"text","props":{"value":"x"}},"t61":{"type":"text","props":{"value":"x"}},"t62":{"type":"text","props":{"value":"x"}},"t63":{"type":"text","props":{"value":"x"}},"t64":{"type":"text","props":{"value":"x"}},"t65":{"type":"text","props":{"value":"x"}},"t66":{"type":"text","props":{"value":"x"}},"t67":{"type":"text","props":{"value":"x"}},"t68":{"type":"text","props":{"value":"x"}},"t69":{"type":"text","props":{"value":"x"}},"t70":{"type":"text","props":{"value":"x"}},"t71":{"type":"text","props":{"value":"x"}},"t72":{"type":"text","props":{"value":"x"}},"t73":{"type":"text","props":{"value":"x"}},"t74":{"type":"text","props":{"value":"x"}},"t75":{"type":"text","props":{"value":"x"}},"t76":{"type":"text","props":{"value":"x"}},"t77":{"type":"text","props":{"value":"x"}},"t78":{"type":"text","props":{"value":"x"}},"t79":{"type":"text","props":{"value":"x"}},"t80":{"type":"text","props":{"value":"x"}},"t81":{"type":"text","props":{"value":"x"}},"t82":{"type":"text","props":{"value":"x"}},"t83":{"type":"text","props":{"value":"x"}},"t84":{"type":"text","props":{"value":"x"}},"t85":{"type":"text","props":{"value":"x"}},"t86":{"type":"text","props":{"value":"x"}},"t87":{"type":"text","props":{"value":"x"}},"t88":{"type":"text","props":{"value":"x"}},"t89":{"type":"text","props":{"value":"x"}},"t90":{"type":"text","props":{"value":"x"}},"t91":{"type":"text","props":{"value":"x"}},"t92":{"type":"text","props":{"value":"x"}},"t93":{"type":"text","props":{"value":"x"}},"t94":{"type":"text","props":{"value":"x"}},"t95":{"type":"text","props":{"value":"x"}},"t96":{"type":"text","props":{"value":"x"}},"t97":{"type":"text","props":{"value":"x"}},"t98":{"type":"text","props":{"value":"x"}},"t99":{"type":"text","props":{"value":"x"}},"t100":{"type":"text","props":{"value":"x"}}}}"#,
        vec![row("D207", "top has 101 children; the most is 100", Some("top"), None, Some("bind a list instead"), None)],
        vec![],
    );
}

#[test]
fn shape_html_habit_p() {
    check(
        r#"{"schema":2,"root":"c","pieces":{"c":{"type":"column","children":["p1"]},"p1":{"type":"p","props":{"value":"Waiting"}}}}"#,
        vec![row("T300", "\"p\" is not a piece; words go in <text>...</text>", Some("p1"), None, None, None)],
        vec![],
    );
}

#[test]
fn shape_html_habit_a() {
    check(
        r#"{"schema":2,"root":"c","pieces":{"c":{"type":"column","children":["a1"]},"a1":{"type":"a","props":{"value":"x"}}}}"#,
        vec![row("T300", "\"a\" is not a piece; a link is a <button> whose onPress opens it", Some("a1"), None, None, None)],
        vec![],
    );
}

#[test]
fn shape_piece_id_digit_start() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]},"9lives":{"type":"text","props":{"value":"x"}}}}"#,
        vec![row(
            "P103",
            "\"9lives\" is not a piece id: letters, digits, dashes and _, starting with a letter",
            Some("9lives"),
            None,
            None,
            None,
        )],
        vec![],
    );
}

#[test]
fn shape_step_set_path_not_own() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"values":{"x":{"start":1}},"reactions":[{"on":{"change":["$x"]},"do":[{"do":"set","path":"x","value":1}]}]}"#,
        vec![row("A601", "set writes the slate's own values: a $name path, not x", Some("reaction-1"), Some("do"), None, None)],
        vec![],
    );
}

#[test]
fn shape_step_send_formula_with() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"values":{"x":{"start":1}},"runs":{"r":{"kind":"cmd","cmd":"true","every":60}},"reactions":[{"on":{"done":"r"},"do":[{"do":"send","text":"hi","with":["1 + 2"]}]}]}"#,
        vec![row("A601", "send carries paths after its text; 1 + 2 is a formula", Some("reaction-1"), Some("do"), None, None)],
        vec![],
    );
}

#[test]
fn shape_run_cmd_empty() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"runs":{"r":{"kind":"cmd","cmd":""}}}"#,
        vec![row(
            "K700",
            "the command is literal text; hand values to it with env={{ NAME: $value }}",
            Some("$r"),
            Some("cmd"),
            None,
            None,
        )],
        vec![row(
            "W019",
            "nothing starts $r: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($r)} />"),
            None,
        )],
    );
}

#[test]
fn shape_run_tool_no_server() {
    check(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"runs":{"r":{"kind":"tool","server":"","tool":"nodot"}}}"#,
        vec![row("K705", "tool names a server and a tool as \"server.tool\"", Some("$r"), Some("tool"), None, None)],
        vec![row(
            "W019",
            "nothing starts $r: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($r)} />"),
            None,
        )],
    );
}

#[test]
fn kit_run_alway_key() {
    check(r#"{"schema":2,"root":"c","runs":{"r":{"kind":"cmd","cmd":"true","every":60,"alway":true}},"pieces":{"c":{"type":"column"}}}"#, vec![row("K704", "$r has no alway; it takes kind, cmd, env, args, stdin, on, cwd, timeout, stream, confirm, every, once, always, then and description", Some("$r"), Some("alway"), None, None)], vec![]);
}

#[test]
fn kit_run_timer_not_set() {
    check(
        r#"{"schema":2,"root":"c","runs":{"r":{"kind":"cmd","cmd":"true","every":60}},"pieces":{"c":{"type":"column"}}}"#,
        vec![],
        vec![],
    );
}

#[test]
fn extension_fallback_self() {
    check(
        r#"{"schema":2,"root":"top","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"top":{"type":"column","children":["a"]},"a":{"type":"sparkle","fallback":"a"}}}"#,
        vec![row("D205", "a is under itself: top > a > a", Some("a"), None, None, None)],
        vec![],
    );
}

#[test]
fn extension_fallback_gone() {
    check(
        r#"{"schema":2,"root":"top","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"top":{"type":"column","children":["a"]},"a":{"type":"sparkle","fallback":"gone"}}}"#,
        vec![row("D203", "fallback names gone, which is not a piece", Some("a"), Some("fallback"), None, None)],
        vec![],
    );
}

#[test]
fn extension_fallback_cycle() {
    check(
        r#"{"schema":2,"root":"top","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"top":{"type":"column","children":["a"]},"a":{"type":"sparkle","fallback":"b"},"b":{"type":"sparkle","fallback":"a"}}}"#,
        vec![row("D205", "a is under itself: top > a > b > a", Some("a"), None, None, None)],
        vec![],
    );
}

#[test]
fn extension_fallback_text_no_value() {
    check(
        r#"{"schema":2,"root":"top","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"top":{"type":"column","children":["a"]},"a":{"type":"sparkle","fallback":"b"},"b":{"type":"text","props":{}}}}"#,
        vec![row("T304", "text \"b\" needs value", Some("b"), Some("value"), None, None)],
        vec![],
    );
}

#[test]
fn extension_fallback_ok() {
    check(
        r#"{"schema":2,"root":"top","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"top":{"type":"column","children":["a"]},"a":{"type":"sparkle","fallback":"b"},"b":{"type":"text","props":{"value":"Hi"}}}}"#,
        vec![],
        vec![],
    );
}

#[test]
fn shape_document_over_cap() {
    check_big_start(
        r#"{"schema":2,"root":"top","pieces":{"top":{"type":"column","children":[]}},"values":{"big":{"start":""}}}"#,
        262200,
        vec![
            row("D208", "the document is over 64 KB", None, None, None, None),
            row("S500", "the values' starts are over 256 KB", None, None, None, None),
        ],
        vec![],
    );
}

#[test]
fn block_w019_dead() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{"tail":{"kind":"cmd","cmd":"tail -n 100 -F logs/error.log","once":true,"stream":true}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["output-1"]},"output-1":{"type":"output","props":{"run":"$tail","lines":40}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $tail: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($tail)} />"),
            None,
        )],
    );
}

#[test]
fn block_w019_started_button() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{"tail":{"kind":"cmd","cmd":"tail logs/error.log"}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["output-1","button-1"]},"output-1":{"type":"output","props":{"run":"$tail"}},"button-1":{"type":"button","props":{"label":"Tail"},"on":{"press":[{"do":"start","run":"tail"}]}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_w019_started_when() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"go":{"start":0}},"derived":{},"runs":{"tail":{"kind":"cmd","cmd":"tail logs/error.log"}},"reactions":[{"on":{"change":["$go"]},"do":[{"do":"start","run":"tail"}]}],"pieces":{"column-1":{"type":"column","children":["output-1","text-1"]},"output-1":{"type":"output","props":{"run":"$tail"}},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_w019_started_row_action() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{"tail":{"kind":"cmd","cmd":"tail logs/error.log"}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["output-1","table-1"]},"output-1":{"type":"output","props":{"run":"$tail"}},"table-1":{"type":"table","props":{"items":[{"bind":"{ n: 1 }"}],"columns":[{"title":"N","value":{"bind":"item.n"}}],"rowActions":[{"label":"Tail","on":{"press":[{"do":"start","run":"tail"}]}}]}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_w019_started_input_submit() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"p":{"start":""}},"derived":{},"runs":{"tail":{"kind":"cmd","cmd":"tail logs/error.log"}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["output-1","input-1"]},"output-1":{"type":"output","props":{"run":"$tail"}},"input-1":{"type":"input","props":{"label":"Path","value":{"bind":"$p"}},"on":{"submit":[{"do":"start","run":"tail"}]}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_w019_every() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{"tail":{"kind":"cmd","cmd":"tail logs/error.log","every":60}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["output-1"]},"output-1":{"type":"output","props":{"run":"$tail"}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_w011_corrected() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"step":{"start":1},"repo":{"start":"acme/landing"},"project":{"start":""},"vercelToken":{"start":null,"secret":true}},"derived":{"checked":"$check.state == 'done' and contains($check.out, $project)","written":"$env.state == 'done' and $ci.state == 'done'"},"runs":{"check":{"kind":"cmd","cmd":"vercel project inspect \"$PROJECT\" 2>&1 | head -c 4000","env":{"PROJECT":{"bind":"$project"},"VERCEL_TOKEN":{"bind":"$vercelToken"}},"on":"host","timeout":30},"env":{"kind":"cmd","cmd":"grep -v \"^VERCEL_TOKEN=\" .env > .env.tmp 2>/dev/null; printf \"VERCEL_TOKEN=%s\\n\" \"$VERCEL_TOKEN\" >> .env.tmp; mv .env.tmp .env","env":{"VERCEL_TOKEN":{"bind":"$vercelToken"}},"on":"host","timeout":10},"ci":{"kind":"cmd","cmd":"gh secret set VERCEL_TOKEN --repo \"$REPO\"","env":{"REPO":{"bind":"$repo"}},"stdin":{"bind":"$vercelToken"},"on":"host","timeout":60}},"reactions":[{"on":{"change":["$project"]},"do":[{"do":"start","run":"check"}]},{"on":{"change":["$checked"]},"do":[{"do":"set","path":"$step","value":{"bind":"$checked ? 3 : 2"}}]},{"on":{"change":["$written"]},"do":[{"do":"set","path":"$step","value":{"bind":"$written ? 4 : 3"}}]}],"pieces":{"column-1":{"type":"column","children":["text-1","section-1","section-2","section-3","section-4"]},"text-1":{"type":"text","props":{"emphasis":"strong","value":{"format":"Step ${$step} of 4"}}},"section-1":{"type":"section","when":"$step == 1","props":{"title":"1. The Vercel token"},"children":["markdown-1","button-1","input-1","button-2"]},"markdown-1":{"type":"markdown","props":{"value":"Open the Vercel dashboard, make a token with the project's scope, and paste it here. It never reaches the agent."}},"button-1":{"type":"button","props":{"label":"Open Vercel tokens"},"on":{"press":[{"do":"open","target":"https://vercel.com/account/tokens"}]}},"input-1":{"type":"input","props":{"label":"Vercel token","value":{"bind":"$vercelToken"},"kind":"password"}},"button-2":{"type":"button","when":"$vercelToken.set","props":{"label":"Next","variant":"primary"},"on":{"press":[{"do":"set","path":"$step","value":2}]}},"section-2":{"type":"section","when":"$step == 2 or $step == 3","props":{"title":"2. The project"},"children":["project-name","text-2","text-3","text-4","text-5"]},"project-name":{"type":"input","props":{"label":"Project name on Vercel","value":{"bind":"$project"},"mono":true}},"text-2":{"type":"text","when":"$check.state == 'held'","props":{"tone":"muted","value":"Approve the check to go on"}},"text-3":{"type":"text","when":"$check.state == 'running'","props":{"tone":"muted","value":"Checking"}},"text-4":{"type":"text","when":"$check.state == 'failed'","props":{"tone":"bad","value":{"format":"Vercel did not find it: ${short($check.err, 160)}"}}},"text-5":{"type":"text","when":"$checked","props":{"tone":"good","value":{"format":"Found ${$project}"}}},"section-3":{"type":"section","when":"$step == 3","props":{"title":"3. Write it down"},"children":["facts-1","row-1","text-6","text-7","text-8"]},"facts-1":{"type":"facts","props":{"facts":[{"label":"Repo","value":{"bind":"$repo"},"mono":true},{"label":"Project","value":{"bind":"$project"},"mono":true}]}},"row-1":{"type":"row","children":["button-3","button-4"]},"button-3":{"type":"button","when":"$env.state != 'done'","props":{"label":"Write .env"},"on":{"press":[{"do":"start","run":"env"}]}},"button-4":{"type":"button","when":"$ci.state != 'done'","props":{"label":"Set the CI secret"},"on":{"press":[{"do":"start","run":"ci"}]}},"text-6":{"type":"text","when":"$env.state == 'done'","props":{"tone":"good","value":".env written"}},"text-7":{"type":"text","when":"$ci.state == 'done'","props":{"tone":"good","value":"CI secret set"}},"text-8":{"type":"text","when":"$env.state == 'failed' or $ci.state == 'failed'","props":{"tone":"bad","value":{"format":"${short(concat($env.err, $ci.err), 200)}"}}},"section-4":{"type":"section","when":"$step == 4","props":{"title":"4. Done"},"children":["text-9","write"]},"text-9":{"type":"text","props":{"tone":"good","value":"The token is in .env and in the repo's secrets."}},"write":{"type":"button","props":{"label":"Tell the agent","variant":"primary"},"on":{"press":[{"do":"send","text":"The Vercel setup is done on the slate; wire the deploy hook next.","with":["$repo","$project","$vercelToken"]}]}}},"title":"Deploy setup"}"#,
        vec![],
        vec![row(
            "T311",
            "primary on button-2, write; one per slate reads loud, the rest draw as default",
            Some("write"),
            None,
            None,
            None,
        )],
    );
}

#[test]
fn block_w011_uncorrected() {
    check(r#"{"schema":2,"root":"column-1","values":{"step":{"start":1},"repo":{"start":"acme/landing"},"project":{"start":""},"vercelToken":{"start":null,"secret":true}},"derived":{"checked":"$check.state == 'done' and contains($check.out, $project)","written":"$env.state == 'done' and $ci.state == 'done'"},"runs":{"check":{"kind":"cmd","cmd":"vercel project inspect \"$PROJECT\" --token \"$VERCEL_TOKEN\" 2>&1 | head -c 4000","env":{"PROJECT":{"bind":"$project"},"VERCEL_TOKEN":{"bind":"$vercelToken"}},"on":"host","timeout":30},"env":{"kind":"cmd","cmd":"grep -v \"^VERCEL_TOKEN=\" .env > .env.tmp 2>/dev/null; printf \"VERCEL_TOKEN=%s\\n\" \"$VERCEL_TOKEN\" >> .env.tmp; mv .env.tmp .env","env":{"VERCEL_TOKEN":{"bind":"$vercelToken"}},"on":"host","timeout":10},"ci":{"kind":"cmd","cmd":"gh secret set VERCEL_TOKEN --repo \"$REPO\" --body \"$VERCEL_TOKEN\"","env":{"REPO":{"bind":"$repo"},"VERCEL_TOKEN":{"bind":"$vercelToken"}},"on":"host","timeout":60}},"reactions":[{"on":{"change":["$project"]},"do":[{"do":"start","run":"check"}]},{"on":{"change":["$checked"]},"do":[{"do":"set","path":"$step","value":{"bind":"$checked ? 3 : 2"}}]},{"on":{"change":["$written"]},"do":[{"do":"set","path":"$step","value":{"bind":"$written ? 4 : 3"}}]}],"pieces":{"column-1":{"type":"column","children":["text-1","section-1","section-2","section-3","section-4"]},"text-1":{"type":"text","props":{"emphasis":"strong","value":{"format":"Step ${$step} of 4"}}},"section-1":{"type":"section","when":"$step == 1","props":{"title":"1. The Vercel token"},"children":["markdown-1","button-1","input-1","button-2"]},"markdown-1":{"type":"markdown","props":{"value":"Open the Vercel dashboard, make a token with the project's scope, and paste it here. It never reaches the agent."}},"button-1":{"type":"button","props":{"label":"Open Vercel tokens"},"on":{"press":[{"do":"open","target":"https://vercel.com/account/tokens"}]}},"input-1":{"type":"input","props":{"label":"Vercel token","value":{"bind":"$vercelToken"},"kind":"password"}},"button-2":{"type":"button","when":"$vercelToken.set","props":{"label":"Next","variant":"primary"},"on":{"press":[{"do":"set","path":"$step","value":2}]}},"section-2":{"type":"section","when":"$step == 2 or $step == 3","props":{"title":"2. The project"},"children":["project-name","text-2","text-3","text-4","text-5"]},"project-name":{"type":"input","props":{"label":"Project name on Vercel","value":{"bind":"$project"},"mono":true}},"text-2":{"type":"text","when":"$check.state == 'held'","props":{"tone":"muted","value":"Approve the check to go on"}},"text-3":{"type":"text","when":"$check.state == 'running'","props":{"tone":"muted","value":"Checking"}},"text-4":{"type":"text","when":"$check.state == 'failed'","props":{"tone":"bad","value":{"format":"Vercel did not find it: ${short($check.err, 160)}"}}},"text-5":{"type":"text","when":"$checked","props":{"tone":"good","value":{"format":"Found ${$project}"}}},"section-3":{"type":"section","when":"$step == 3","props":{"title":"3. Write it down"},"children":["facts-1","row-1","text-6","text-7","text-8"]},"facts-1":{"type":"facts","props":{"facts":[{"label":"Repo","value":{"bind":"$repo"},"mono":true},{"label":"Project","value":{"bind":"$project"},"mono":true}]}},"row-1":{"type":"row","children":["button-3","button-4"]},"button-3":{"type":"button","when":"$env.state != 'done'","props":{"label":"Write .env"},"on":{"press":[{"do":"start","run":"env"}]}},"button-4":{"type":"button","when":"$ci.state != 'done'","props":{"label":"Set the CI secret"},"on":{"press":[{"do":"start","run":"ci"}]}},"text-6":{"type":"text","when":"$env.state == 'done'","props":{"tone":"good","value":".env written"}},"text-7":{"type":"text","when":"$ci.state == 'done'","props":{"tone":"good","value":"CI secret set"}},"text-8":{"type":"text","when":"$env.state == 'failed' or $ci.state == 'failed'","props":{"tone":"bad","value":{"format":"${short(concat($env.err, $ci.err), 200)}"}}},"section-4":{"type":"section","when":"$step == 4","props":{"title":"4. Done"},"children":["text-9","write"]},"text-9":{"type":"text","props":{"tone":"good","value":"The token is in .env and in the repo's secrets."}},"write":{"type":"button","props":{"label":"Tell the agent","variant":"primary"},"on":{"press":[{"do":"send","text":"The Vercel setup is done on the slate; wire the deploy hook next.","with":["$repo","$project","$vercelToken"]}]}}},"title":"Deploy setup"}"#, vec![], vec![row("W011", "VERCEL_TOKEN carries the secret $vercelToken, and the command passes it after --token, as an argument ps can read; hand it on stdin={$vercelToken}, or let the program read VERCEL_TOKEN from its environment", Some("$check"), Some("cmd"), Some("drop --token \"$VERCEL_TOKEN\" and use stdin={$vercelToken} or the program's own VERCEL_TOKEN"), None), row("W011", "VERCEL_TOKEN carries the secret $vercelToken, and the command passes it after --body, as an argument ps can read; hand it on stdin={$vercelToken}, or let the program read VERCEL_TOKEN from its environment", Some("$ci"), Some("cmd"), Some("drop --body \"$VERCEL_TOKEN\" and use stdin={$vercelToken} or the program's own VERCEL_TOKEN"), None), row("T311", "primary on button-2, write; one per slate reads loud, the rest draw as default", Some("write"), None, None, None)]);
}

#[test]
fn block_w011_body_flag() {
    check(r#"{"schema":2,"root":"column-1","values":{"t":{"start":null,"secret":true}},"derived":{},"runs":{"r":{"kind":"cmd","cmd":"gh x --body=$T","env":{"T":{"bind":"$t"}}}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#, vec![], vec![row("W011", "T carries the secret $t, and the command passes it after --body, as an argument ps can read; hand it on stdin={$t}, or let the program read T from its environment", Some("$r"), Some("cmd"), Some("drop --body \"$T\" and use stdin={$t} or the program's own T"), None), row("W019", "nothing starts $r: give it a button, a when, or every", None, None, Some("<button label=\"...\" onPress={start($r)} />"), None)]);
}

#[test]
fn block_w011_vercel_argument() {
    check(r#"{"schema":2,"root":"column-1","values":{"t":{"start":null,"secret":true}},"derived":{},"runs":{"r":{"kind":"cmd","cmd":"vercel ls -p $T","env":{"T":{"bind":"$t"}}}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#, vec![], vec![row("W011", "T carries the secret $t, and the command passes it after -p, as an argument ps can read; hand it on stdin={$t}, or let the program read T from its environment", Some("$r"), Some("cmd"), Some("drop -p \"$T\" and use stdin={$t} or the program's own T"), None), row("W019", "nothing starts $r: give it a button, a when, or every", None, None, Some("<button label=\"...\" onPress={start($r)} />"), None)]);
}

#[test]
fn block_w011_printf_ok() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"t":{"start":null,"secret":true}},"derived":{},"runs":{"r":{"kind":"cmd","cmd":"printf \"%s\" \"$T\" > f","env":{"T":{"bind":"$t"}}}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $r: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($r)} />"),
            None,
        )],
    );
}

#[test]
fn block_w012_0() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"a":{"start":"x"},"xs":{"start":["a","b"]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"Market closed · Weekend close"}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W012",
            "value joins words with \"·\"; the kit separates things by layout, never a mark: give each its own piece, or join with a comma",
            Some("text-1"),
            Some("value"),
            Some("separate pieces (facts, chips, a row) or a comma"),
            None,
        )],
    );
}

#[test]
fn block_w012_1() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"a":{"start":"x"},"xs":{"start":["a","b"]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["facts-1"]},"facts-1":{"type":"facts","props":{"facts":[{"label":"Purity","value":"24K · 99.9% pure"}]}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W012",
            "value joins words with \"·\"; the kit separates things by layout, never a mark: give each its own piece, or join with a comma",
            Some("facts-1"),
            Some("facts[0].value"),
            Some("separate pieces (facts, chips, a row) or a comma"),
            None,
        )],
    );
}

#[test]
fn block_w012_2() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"a":{"start":"x"},"xs":{"start":["a","b"]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"Open • Closed"}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W012",
            "value joins words with \"•\"; the kit separates things by layout, never a mark: give each its own piece, or join with a comma",
            Some("text-1"),
            Some("value"),
            Some("separate pieces (facts, chips, a row) or a comma"),
            None,
        )],
    );
}

#[test]
fn block_w012_3() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"a":{"start":"x"},"xs":{"start":["a","b"]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"Spot | Ask"}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W012",
            "value joins words with \"|\"; the kit separates things by layout, never a mark: give each its own piece, or join with a comma",
            Some("text-1"),
            Some("value"),
            Some("separate pieces (facts, chips, a row) or a comma"),
            None,
        )],
    );
}

#[test]
fn block_w012_4() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"a":{"start":"x"},"xs":{"start":["a","b"]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${`${$a} · ${$a}`}"}}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W012",
            "value joins words with \"·\"; the kit separates things by layout, never a mark: give each its own piece, or join with a comma",
            Some("text-1"),
            Some("value"),
            Some("separate pieces (facts, chips, a row) or a comma"),
            None,
        )],
    );
}

#[test]
fn block_w012_5() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"a":{"start":"x"},"xs":{"start":["a","b"]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"Closed · ${$a}"}}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W012",
            "value joins words with \"·\"; the kit separates things by layout, never a mark: give each its own piece, or join with a comma",
            Some("text-1"),
            Some("value"),
            Some("separate pieces (facts, chips, a row) or a comma"),
            None,
        )],
    );
}

#[test]
fn block_w012_6() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"a":{"start":"x"},"xs":{"start":["a","b"]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${join($xs, \" · \")}"}}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W012",
            "value joins words with \"·\"; the kit separates things by layout, never a mark: give each its own piece, or join with a comma",
            Some("text-1"),
            Some("value"),
            Some("separate pieces (facts, chips, a row) or a comma"),
            None,
        )],
    );
}

#[test]
fn block_w012_7() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"a":{"start":"x"},"xs":{"start":["a","b"]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${join($xs, \" | \")}"}}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W012",
            "value joins words with \"|\"; the kit separates things by layout, never a mark: give each its own piece, or join with a comma",
            Some("text-1"),
            Some("value"),
            Some("separate pieces (facts, chips, a row) or a comma"),
            None,
        )],
    );
}

#[test]
fn block_w012_8() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"a":{"start":"x"},"xs":{"start":["a","b"]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"Market closed, weekend close"}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_w012_9() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"a":{"start":"x"},"xs":{"start":["a","b"]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${$xs | take(1) | join(\", \")}"}}}},"title":"T"}"#,
        vec![row(
            "Q423",
            "join takes 2 to 3 arguments, got 1: join(other, key, otherKey?)",
            Some("text-1"),
            Some("value"),
            Some("join(other, key, otherKey?)"),
            None,
        )],
        vec![],
    );
}

#[test]
fn block_w012_10() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"a":{"start":"x"},"xs":{"start":["a","b"]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["markdown-1"]},"markdown-1":{"type":"markdown","props":{"value":"| a | b |\n| - | - |\n| 1 | 2 |"}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_w012_11() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"a":{"start":"x"},"xs":{"start":["a","b"]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"• first"}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_w013_index() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"hits":{"start":[{"n":1,"at":1}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["chart-1"]},"chart-1":{"type":"chart","props":{"label":"Requests","items":{"bind":"$hits"},"x":{"bind":"index"},"value":{"bind":"item.n"}}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W013",
            "the chart's x is each row's index, so its axis reads 0 to the count; give each row its time, for example x={item.at}",
            Some("chart-1"),
            Some("x"),
            Some("x={item.at}"),
            None,
        )],
    );
}

#[test]
fn block_w013_none() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"hits":{"start":[{"n":1,"at":1}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["chart-1"]},"chart-1":{"type":"chart","props":{"label":"Requests","items":{"bind":"$hits"},"value":{"bind":"item.n"}}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W013",
            "the chart's x is each row's index, so its axis reads 0 to the count; give each row its time, for example x={item.at}",
            Some("chart-1"),
            Some("x"),
            Some("x={item.at}"),
            None,
        )],
    );
}

#[test]
fn block_w013_at_clean() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"hits":{"start":[{"n":1,"at":1}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["chart-1"]},"chart-1":{"type":"chart","props":{"label":"Requests","items":{"bind":"$hits"},"x":{"bind":"item.at"},"value":{"bind":"item.n"}}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_always_without_every() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{"r":{"kind":"cmd","cmd":"true","always":true}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![row(
            "K703",
            "always keeps a timer running while the slate is not shown, so it needs every",
            Some("$r"),
            Some("always"),
            Some("every={60} always"),
            None,
        )],
        vec![row(
            "W019",
            "nothing starts $r: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($r)} />"),
            None,
        )],
    );
}

#[test]
fn block_always_with_every() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{"r":{"kind":"cmd","cmd":"true","every":60,"always":true}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_confirm_formula() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"pid":{"start":19271},"name":{"start":"node"}},"derived":{},"runs":{"kill":{"kind":"cmd","cmd":"kill \"$PID\"","confirm":{"format":"Kill ${$name} (PID ${$pid})?"},"env":{"PID":{"bind":"$pid"}}}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["button-1"]},"button-1":{"type":"button","props":{"label":"Kill"},"on":{"press":[{"do":"start","run":"kill"}]}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_confirm_ternary() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"ok":{"start":true}},"derived":{},"runs":{"r":{"kind":"cmd","cmd":"true","confirm":{"bind":"$ok ? 'Again?' : 'Run it?'"}}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $r: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($r)} />"),
            None,
        )],
    );
}

#[test]
fn block_confirm_literal() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{"r":{"kind":"cmd","cmd":"true","confirm":"Run it?"}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $r: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($r)} />"),
            None,
        )],
    );
}

#[test]
fn block_confirm_unknown_value() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{"r":{"kind":"cmd","cmd":"true","confirm":{"bind":"$nope"}}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![row("S501", "$nope is not declared; add <value name=\"nope\" start=... />", Some("$r"), Some("confirm"), None, None)],
        vec![row(
            "W019",
            "nothing starts $r: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($r)} />"),
            None,
        )],
    );
}

#[test]
fn block_confirm_secret() {
    check(r#"{"schema":2,"root":"column-1","values":{"t":{"start":null,"secret":true}},"derived":{},"runs":{"r":{"kind":"cmd","cmd":"true","confirm":{"bind":"$t"}}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#, vec![row("S520", "$t is a secret; only a run's env or stdin, an input's value or a send's paths may name it, and a formula reads only its .set, .len and .at", Some("$r"), Some("confirm"), Some("$t.set"), None)], vec![row("W019", "nothing starts $r: give it a button, a when, or every", None, None, Some("<button label=\"...\" onPress={start($r)} />"), None)]);
}

#[test]
fn block_when_on_fact() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"all":{"start":false},"pick":{"start":null},"rows":{"start":[{"name":"a","pid":1}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["facts-1"]},"facts-1":{"type":"facts","props":{"facts":[{"label":"PID","value":"1","when":"$all"}]}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_when_on_cols_and_action() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"all":{"start":false},"pick":{"start":null},"rows":{"start":[{"name":"a","pid":1}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["table-1"]},"table-1":{"type":"table","props":{"items":{"bind":"$rows"},"columns":[{"title":"Name","value":{"bind":"item.name"}},{"title":"PID","value":{"bind":"item.pid"},"when":"$all"}],"rowActions":[{"label":"Kill","when":"item.pid > 0","on":{"press":[{"do":"send","text":"Kill it.","with":["item.pid"]}]}}]}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_when_on_option() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"all":{"start":false},"pick":{"start":null},"rows":{"start":[{"name":"a","pid":1}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["select-1"]},"select-1":{"type":"select","props":{"label":"Pick","value":{"bind":"$pick"},"options":[{"value":"a"},{"value":"b","when":"$all"}]}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_when_on_choice() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"all":{"start":false},"pick":{"start":null},"rows":{"start":[{"name":"a","pid":1}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["choices-1"]},"choices-1":{"type":"choices","props":{"label":"Pick","value":{"bind":"$pick"},"options":[{"value":"a"},{"value":"b","when":"$all"}]}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_when_row_col_per_row() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"all":{"start":false},"pick":{"start":null},"rows":{"start":[{"name":"a","pid":1}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["table-1"]},"table-1":{"type":"table","props":{"items":{"bind":"$rows"},"columns":[{"title":"PID","value":{"bind":"item.pid"},"when":"item.pid > 0"}]}}},"title":"T"}"#,
        vec![row("X409", "item is only known inside a repeating piece or a step", Some("table-1"), Some("columns[0].when"), None, None)],
        vec![],
    );
}

#[test]
fn block_section_literal_open() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["section-1"]},"section-1":{"type":"section","props":{"title":"More","collapsible":true,"open":false},"children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_section_state_open() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"open":{"start":true}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["section-1"]},"section-1":{"type":"section","props":{"title":"More","collapsible":true,"open":{"bind":"$open"}},"children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_section_open_string() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["section-1"]},"section-1":{"type":"section","props":{"title":"More","open":"no"},"children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![row(
            "X410",
            "open on section is two-way and binds a value: open={$name}",
            Some("section-1"),
            Some("open"),
            Some("open={$no}"),
            None,
        )],
        vec![],
    );
}

#[test]
fn block_toggle_literal_value() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["toggle-1"]},"toggle-1":{"type":"toggle","props":{"label":"On","value":true}}},"title":"T"}"#,
        vec![row(
            "X410",
            "value on toggle is two-way and binds a value: value={$name}",
            Some("toggle-1"),
            Some("value"),
            Some("value={$name}"),
            None,
        )],
        vec![],
    );
}

#[test]
fn block_camel_ids() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{"spot":{"kind":"cmd","cmd":"echo '{}'"}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["goldPrice","spot","spot_at"]},"goldPrice":{"type":"number","props":{"label":"Gold","value":{"bind":"$spot.json.v"}}},"spot":{"type":"text","props":{"value":"x"}},"spot_at":{"type":"text","props":{"value":"y"}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $spot: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($spot)} />"),
            None,
        )],
    );
}

#[test]
fn block_w014_sentence_text() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"pid":{"start":1},"rows":{"start":[{"n":"a"}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"mono":true,"value":"Kill the node process now"}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W014",
            "mono is for figures, ids, times and paths; this reads as a sentence, which reads better in the normal face",
            Some("text-1"),
            Some("mono"),
            Some("drop mono"),
            None,
        )],
    );
}

#[test]
fn block_w014_sentence_formula() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"pid":{"start":1},"rows":{"start":[{"n":"a"}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"mono":true,"value":{"format":"${$pid} processes are running here"}}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W014",
            "mono is for figures, ids, times and paths; this reads as a sentence, which reads better in the normal face",
            Some("text-1"),
            Some("mono"),
            Some("drop mono"),
            None,
        )],
    );
}

#[test]
fn block_w014_sentence_fact() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"pid":{"start":1},"rows":{"start":[{"n":"a"}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["facts-1"]},"facts-1":{"type":"facts","props":{"facts":[{"label":"Note","value":"the market is closed today","mono":true}]}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W014",
            "mono is for figures, ids, times and paths; this reads as a sentence, which reads better in the normal face",
            Some("facts-1"),
            Some("facts[0].mono"),
            Some("drop mono"),
            None,
        )],
    );
}

#[test]
fn block_w014_sentence_row() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"pid":{"start":1},"rows":{"start":[{"n":"a"}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["table-1"]},"table-1":{"type":"table","props":{"items":{"bind":"$rows"},"columns":[{"title":"Why","value":{"format":"waiting on the ${item.n} lock"},"mono":true}]}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W014",
            "mono is for figures, ids, times and paths; this reads as a sentence, which reads better in the normal face",
            Some("table-1"),
            Some("columns[0].mono"),
            Some("drop mono"),
            None,
        )],
    );
}

#[test]
fn block_w014_figure() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"pid":{"start":1},"rows":{"start":[{"n":"a"}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"mono":true,"value":{"format":"${$pid}"}}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_w014_time() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"pid":{"start":1},"rows":{"start":[{"n":"a"}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"mono":true,"value":"2026-10-04 17:23 UTC"}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_w014_path() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"pid":{"start":1},"rows":{"start":[{"n":"a"}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"mono":true,"value":"src/slate/kit.ts"}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_w014_pid_text() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"pid":{"start":1},"rows":{"start":[{"n":"a"}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"mono":true,"value":"PID 19271"}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_w014_id_fact() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"pid":{"start":1},"rows":{"start":[{"n":"a"}]}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["facts-1"]},"facts-1":{"type":"facts","props":{"facts":[{"label":"Head","value":"b5ebeeff4","mono":true}]}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_run_field_state() {
    check(
        r#"{"schema":2,"root":"text-1","values":{},"derived":{},"runs":{"p":{"kind":"cmd","cmd":"echo 1"}},"reactions":[],"pieces":{"text-1":{"type":"text","props":{"value":{"bind":"if($p.state, \"a\", \"b\")"}}}}}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $p: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($p)} />"),
            None,
        )],
    );
}

#[test]
fn block_run_field_why() {
    check(
        r#"{"schema":2,"root":"text-1","values":{},"derived":{},"runs":{"p":{"kind":"cmd","cmd":"echo 1"}},"reactions":[],"pieces":{"text-1":{"type":"text","props":{"value":{"bind":"if($p.why, \"a\", \"b\")"}}}}}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $p: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($p)} />"),
            None,
        )],
    );
}

#[test]
fn block_run_field_exit() {
    check(
        r#"{"schema":2,"root":"text-1","values":{},"derived":{},"runs":{"p":{"kind":"cmd","cmd":"echo 1"}},"reactions":[],"pieces":{"text-1":{"type":"text","props":{"value":{"bind":"if($p.exit, \"a\", \"b\")"}}}}}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $p: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($p)} />"),
            None,
        )],
    );
}

#[test]
fn block_run_field_out() {
    check(
        r#"{"schema":2,"root":"text-1","values":{},"derived":{},"runs":{"p":{"kind":"cmd","cmd":"echo 1"}},"reactions":[],"pieces":{"text-1":{"type":"text","props":{"value":{"bind":"if($p.out, \"a\", \"b\")"}}}}}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $p: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($p)} />"),
            None,
        )],
    );
}

#[test]
fn block_run_field_err() {
    check(
        r#"{"schema":2,"root":"text-1","values":{},"derived":{},"runs":{"p":{"kind":"cmd","cmd":"echo 1"}},"reactions":[],"pieces":{"text-1":{"type":"text","props":{"value":{"bind":"if($p.err, \"a\", \"b\")"}}}}}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $p: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($p)} />"),
            None,
        )],
    );
}

#[test]
fn block_run_field_json() {
    check(
        r#"{"schema":2,"root":"text-1","values":{},"derived":{},"runs":{"p":{"kind":"cmd","cmd":"echo 1"}},"reactions":[],"pieces":{"text-1":{"type":"text","props":{"value":{"bind":"if($p.json, \"a\", \"b\")"}}}}}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $p: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($p)} />"),
            None,
        )],
    );
}

#[test]
fn block_run_field_lines() {
    check(
        r#"{"schema":2,"root":"text-1","values":{},"derived":{},"runs":{"p":{"kind":"cmd","cmd":"echo 1"}},"reactions":[],"pieces":{"text-1":{"type":"text","props":{"value":{"bind":"if($p.lines, \"a\", \"b\")"}}}}}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $p: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($p)} />"),
            None,
        )],
    );
}

#[test]
fn block_run_field_startedat() {
    check(
        r#"{"schema":2,"root":"text-1","values":{},"derived":{},"runs":{"p":{"kind":"cmd","cmd":"echo 1"}},"reactions":[],"pieces":{"text-1":{"type":"text","props":{"value":{"bind":"if($p.startedAt, \"a\", \"b\")"}}}}}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $p: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($p)} />"),
            None,
        )],
    );
}

#[test]
fn block_run_field_endedat() {
    check(
        r#"{"schema":2,"root":"text-1","values":{},"derived":{},"runs":{"p":{"kind":"cmd","cmd":"echo 1"}},"reactions":[],"pieces":{"text-1":{"type":"text","props":{"value":{"bind":"if($p.endedAt, \"a\", \"b\")"}}}}}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $p: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($p)} />"),
            None,
        )],
    );
}

#[test]
fn block_run_field_ms() {
    check(
        r#"{"schema":2,"root":"text-1","values":{},"derived":{},"runs":{"p":{"kind":"cmd","cmd":"echo 1"}},"reactions":[],"pieces":{"text-1":{"type":"text","props":{"value":{"bind":"if($p.ms, \"a\", \"b\")"}}}}}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $p: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($p)} />"),
            None,
        )],
    );
}

#[test]
fn block_run_field_runs() {
    check(
        r#"{"schema":2,"root":"text-1","values":{},"derived":{},"runs":{"p":{"kind":"cmd","cmd":"echo 1"}},"reactions":[],"pieces":{"text-1":{"type":"text","props":{"value":{"bind":"if($p.runs, \"a\", \"b\")"}}}}}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $p: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($p)} />"),
            None,
        )],
    );
}

#[test]
fn block_run_field_cut() {
    check(
        r#"{"schema":2,"root":"text-1","values":{},"derived":{},"runs":{"p":{"kind":"cmd","cmd":"echo 1"}},"reactions":[],"pieces":{"text-1":{"type":"text","props":{"value":{"bind":"if($p.cut, \"a\", \"b\")"}}}}}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $p: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($p)} />"),
            None,
        )],
    );
}

#[test]
fn block_run_field_stale() {
    check(
        r#"{"schema":2,"root":"text-1","values":{},"derived":{},"runs":{"p":{"kind":"cmd","cmd":"echo 1"}},"reactions":[],"pieces":{"text-1":{"type":"text","props":{"value":{"bind":"if($p.stale, \"a\", \"b\")"}}}}}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $p: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($p)} />"),
            None,
        )],
    );
}

#[test]
fn block_run_field_refreshing() {
    check(
        r#"{"schema":2,"root":"text-1","values":{},"derived":{},"runs":{"p":{"kind":"cmd","cmd":"echo 1"}},"reactions":[],"pieces":{"text-1":{"type":"text","props":{"value":{"bind":"if($p.refreshing, \"a\", \"b\")"}}}}}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $p: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($p)} />"),
            None,
        )],
    );
}

#[test]
fn block_run_field_text() {
    check(
        r#"{"schema":2,"root":"text-1","values":{},"derived":{},"runs":{"p":{"kind":"cmd","cmd":"echo 1"}},"reactions":[],"pieces":{"text-1":{"type":"text","props":{"value":{"bind":"if($p.text, \"a\", \"b\")"}}}}}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $p: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($p)} />"),
            None,
        )],
    );
}

#[test]
fn block_per_row_chart_literal() {
    check(
        r#"{"schema":2,"root":"chart-1","values":{"h":{"start":[]}},"derived":{},"runs":{},"reactions":[],"pieces":{"chart-1":{"type":"chart","props":{"label":"x","items":{"bind":"$h"},"x":"t","value":"v"}}}}"#,
        vec![
            row(
                "T303",
                "x is read per row, a formula over item: x={item.t}; \"t\" would be the same on every row",
                Some("chart-1"),
                Some("x"),
                Some("x={item.t}"),
                None,
            ),
            row(
                "T303",
                "value is read per row, a formula over item: value={item.v}; \"v\" would be the same on every row",
                Some("chart-1"),
                Some("value"),
                Some("value={item.v}"),
                None,
            ),
        ],
        vec![],
    );
}

#[test]
fn block_per_row_bars_name() {
    check(
        r#"{"schema":2,"root":"bars-1","values":{"h":{"start":[]}},"derived":{},"runs":{},"reactions":[],"pieces":{"bars-1":{"type":"bars","props":{"label":"x","items":{"bind":"$h"},"name":"n","value":{"bind":"item.v"}}}}}"#,
        vec![row(
            "T303",
            "name is read per row, a formula over item: name={item.n}; \"n\" would be the same on every row",
            Some("bars-1"),
            Some("name"),
            Some("name={item.n}"),
            None,
        )],
        vec![],
    );
}

#[test]
fn block_meter_max_name() {
    check(
        r#"{"schema":2,"root":"meter-1","values":{"n":{"start":1}},"derived":{},"runs":{},"reactions":[],"pieces":{"meter-1":{"type":"meter","props":{"label":"x","value":1,"max":"n"}}}}"#,
        vec![row(
            "T303",
            "max takes a number in braces, like max={5}, or a formula, like max={$count}; \"n\" is text",
            Some("meter-1"),
            Some("max"),
            Some("max={$n}"),
            None,
        )],
        vec![],
    );
}

#[test]
fn block_meter_max_text() {
    check(
        r#"{"schema":2,"root":"meter-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"meter-1":{"type":"meter","props":{"label":"x","value":1,"max":"5"}}}}"#,
        vec![row(
            "T303",
            "max takes a number in braces, like max={5}, or a formula, like max={$count}; \"5\" is text",
            Some("meter-1"),
            Some("max"),
            Some("max={5}"),
            None,
        )],
        vec![],
    );
}

#[test]
fn block_html_header() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["header-1"]},"header-1":{"type":"header","children":["text-1"]},"text-1":{"type":"text","props":{"value":"PR"}}}}"#,
        vec![row(
            "T300",
            "\"header\" is not a piece; a title is a <heading>, or a <section title=\"...\">",
            Some("header-1"),
            None,
            None,
            None,
        )],
        vec![],
    );
}

#[test]
fn block_html_div() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["div-1"]},"div-1":{"type":"div","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}}}"#,
        vec![row("T300", "\"div\" is not a piece; a group is a <column>, a <row> or a <section>", Some("div-1"), None, None, None)],
        vec![],
    );
}

#[test]
fn block_html_text_every() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"every":60,"value":"x"}}}}"#,
        vec![row(
            "T302",
            "every is a <run>'s, not a piece's: <run name=\"x\" cmd='...' every={60} />, and the piece reads $x",
            Some("text-1"),
            Some("every"),
            None,
            None,
        )],
        vec![],
    );
}

#[test]
fn block_html_img() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["img-1"]},"img-1":{"type":"img","props":{"src":"a.png"}}}}"#,
        vec![row("T300", "\"img\" is not a piece; a slate draws no images", Some("img-1"), None, None, None)],
        vec![],
    );
}

#[test]
fn block_html_list() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["list-1"]},"list-1":{"type":"list","props":{"items":[]}}}}"#,
        vec![row("T300", "\"list\" is not a piece; a list is a <checklist> or a <table items={...}>", Some("list-1"), None, None, None)],
        vec![],
    );
}

#[test]
fn block_render_judge() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"x":{"start":1}},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["section-1","text-2","text-3","heading-1"]},"section-1":{"type":"section","props":{"title":"Bengaluru Retail Prices"},"children":["text-1"]},"text-1":{"type":"text","props":{"value":"📈 Up"}},"text-2":{"type":"text","props":{"value":"$x.out"}},"text-3":{"type":"text","props":{"value":"{$x}"}},"heading-1":{"type":"heading","props":{"value":"Open PRs"}}},"title":"Live Gold Prices"}"#,
        vec![],
        vec![
            row(
                "W018",
                "the slate's title \"Live Gold Prices\" is in Title Case; heads take sentence case",
                None,
                None,
                Some("title=\"Live gold prices\""),
                None,
            ),
            row(
                "W018",
                "title \"Bengaluru Retail Prices\" is in Title Case; heads take sentence case",
                Some("section-1"),
                Some("title"),
                Some("title=\"Bengaluru retail prices\""),
                None,
            ),
            row("W017", "value has an emoji; the slate's words carry none, so say it in words", Some("text-1"), Some("value"), None, None),
            row(
                "W001",
                "value is the literal text \"$x.out\", which reads like a formula; quotes show it as written",
                Some("text-2"),
                Some("value"),
                Some("value={$x.out}"),
                None,
            ),
            row(
                "W001",
                "value is the literal text \"{$x}\", which reads like a formula; quotes show it as written",
                Some("text-3"),
                Some("value"),
                Some("value={$x}"),
                None,
            ),
        ],
    );
}

#[test]
fn block_nested_run() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{"rates":{"kind":"cmd","cmd":"echo 1","every":60}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"${$rates.out}"}}}}}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_words_in_section() {
    check(
        r#"{"schema":2,"root":"section-1","values":{"n":{"start":[]}},"derived":{},"runs":{},"reactions":[],"pieces":{"section-1":{"type":"section","props":{"title":"Requests"},"children":["text-1"]},"text-1":{"type":"text","props":{"value":{"format":"Requests: ${len($n)}"}}}}}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_tool_mcp_name() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{"q":{"kind":"tool","server":"axiom","tool":"queryDataset","args":{"apl":"count()"}}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}}}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $q: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($q)} />"),
            None,
        )],
    );
}

#[test]
fn block_token_in_cmd() {
    check(r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{"a":{"kind":"cmd","cmd":"curl -H 'Authorization: Bearer xapt-dd233a3a-85a9-45d2-9a1c-a4608f2b9eeb' https://x"}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}}}"#, vec![row("S520", "cmd holds a token written out (xapt-d...), which the slate keeps and shows the person; let the command read it from the file it lives in, or take it from a <secret> the person types", Some("$a"), Some("cmd"), None, None)], vec![row("W019", "nothing starts $a: give it a button, a when, or every", None, None, Some("<button label=\"...\" onPress={start($a)} />"), None)]);
}

#[test]
fn block_token_in_file() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"files":{"q.py":"KEY = \"sk-ant-api03-abcdefghijklmnopqrst\""}}"#,
        vec![row(
            "S520",
            "q.py holds a token written out (sk-ant...); let the code read it from the file it lives in, or from an env the run hands it",
            Some("q.py"),
            None,
            None,
            None,
        )],
        vec![],
    );
}

#[test]
fn block_bearer_var_ok() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{"a":{"kind":"cmd","cmd":"curl -H \"Authorization: Bearer $TOKEN\" https://x"}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}}}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $a: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($a)} />"),
            None,
        )],
    );
}

#[test]
fn block_token_in_env() {
    check(r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{"a":{"kind":"cmd","cmd":"curl -H \"Authorization: Bearer $T\" https://x","env":{"T":"xapt-dd233a3a-85a9-45d2-9a1c-a4608f2b9eeb"}}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}}}"#, vec![row("S520", "the slate holds a token written out (xapt-d...), which it keeps and shows the person; let a run's command read it from the file it lives in, or take it from a <secret> the person types", None, None, None, None)], vec![row("W019", "nothing starts $a: give it a button, a when, or every", None, None, Some("<button label=\"...\" onPress={start($a)} />"), None)]);
}

#[test]
fn block_piece_tag() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["rates"]},"rates":{"type":"piece","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}}}"#,
        vec![row(
            "T300",
            "\"piece\" is not a piece; the tag is the piece's own type, like <number ...> or <text>",
            Some("rates"),
            None,
            None,
            None,
        )],
        vec![],
    );
}

#[test]
fn block_secret_run_0() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"tok":{"start":null,"secret":true}},"derived":{},"runs":{"r":{"kind":"cmd","cmd":"echo","args":[{"bind":"$tok"}]}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![row(
            "S520",
            "args[0] is the secret $tok; a secret reaches a command through env or stdin, never as an argument, which ps can read",
            Some("$r"),
            Some("args[0]"),
            Some("env={{ TOK: $tok }}, or stdin={$tok}"),
            None,
        )],
        vec![row(
            "W019",
            "nothing starts $r: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($r)} />"),
            None,
        )],
    );
}

#[test]
fn block_secret_run_1() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"tok":{"start":null,"secret":true}},"derived":{},"runs":{"r":{"kind":"cmd","cmd":"echo","env":{"TOK":{"bind":"$tok"}}}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $r: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($r)} />"),
            None,
        )],
    );
}

#[test]
fn block_secret_run_2() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"tok":{"start":null,"secret":true}},"derived":{},"runs":{"r":{"kind":"cmd","cmd":"echo","stdin":{"bind":"$tok"}}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $r: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($r)} />"),
            None,
        )],
    );
}

#[test]
fn block_secret_run_3() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"tok":{"start":null,"secret":true}},"derived":{},"runs":{"r":{"kind":"cmd","cmd":"echo","args":[{"bind":"$tok.len"}]}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![],
        vec![row(
            "W019",
            "nothing starts $r: give it a button, a when, or every",
            None,
            None,
            Some("<button label=\"...\" onPress={start($r)} />"),
            None,
        )],
    );
}

#[test]
fn block_reaction_path_0() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"x":{"start":{"a":1,"b":0}}},"derived":{},"runs":{},"reactions":[{"on":{"change":["$x.a"]},"do":[{"do":"set","path":"$x.b","value":1}]}],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_reaction_path_1() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"x":{"start":{"a":1,"b":0}}},"derived":{},"runs":{},"reactions":[{"on":{"change":["$x.a"]},"do":[{"do":"set","path":"$x.a","value":1}]}],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![row(
            "A610",
            "reactions and derived values form a cycle: when change of $x.a sets $x.a",
            None,
            None,
            Some("compute it as a <derived> value instead of setting it back"),
            None,
        )],
        vec![],
    );
}

#[test]
fn block_reaction_path_2() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"x":{"start":{"a":1,"b":0}}},"derived":{},"runs":{},"reactions":[{"on":{"change":["$x"]},"do":[{"do":"set","path":"$x.b","value":1}]}],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![row(
            "A610",
            "reactions and derived values form a cycle: when change of $x sets $x.b",
            None,
            None,
            Some("compute it as a <derived> value instead of setting it back"),
            None,
        )],
        vec![],
    );
}

#[test]
fn block_reaction_path_3() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"x":{"start":{"a":1,"b":0}}},"derived":{},"runs":{},"reactions":[{"on":{"change":["$x.b"]},"do":[{"do":"set","path":"$x","value":{"bind":"{ a: 1, b: 2 }"}}]}],"pieces":{"column-1":{"type":"column","children":["text-1"]},"text-1":{"type":"text","props":{"value":"x"}}},"title":"T"}"#,
        vec![row(
            "A610",
            "reactions and derived values form a cycle: when change of $x.b sets $x",
            None,
            None,
            Some("compute it as a <derived> value instead of setting it back"),
            None,
        )],
        vec![],
    );
}

#[test]
fn block_spec_02_model_1() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"id":{"start":""},"repo":{"start":"wsp-labs/wsp"},"step":{"start":1}},"derived":{"ok":"$check.state == 'done' and $check.json.id != null"},"runs":{"check":{"kind":"cmd","cmd":"gh api \"repos/$REPO/issues/$ID\"","env":{"REPO":{"bind":"$repo"},"ID":{"bind":"$id"}},"timeout":20}},"reactions":[{"on":{"change":["$id"]},"do":[{"do":"start","run":"check"}]}],"pieces":{"column-1":{"type":"column","children":["issue","text-1","text-2","text-3","button-1"]},"issue":{"type":"input","props":{"label":"Issue number","value":{"bind":"$id"},"mono":true}},"text-1":{"type":"text","when":"$check.state == 'running'","props":{"tone":"muted","value":"Checking"}},"text-2":{"type":"text","when":"$check.state == 'failed'","props":{"tone":"bad","value":{"format":"No such issue: ${$check.err}"}}},"text-3":{"type":"text","when":"$ok","props":{"tone":"good","value":{"format":"${$check.json.title}"}}},"button-1":{"type":"button","when":"$ok","props":{"label":"Next","variant":"primary"},"on":{"press":[{"do":"set","path":"$step","value":2}]}}},"title":"Deploy setup"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_spec_03_syntax_1() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{"failing":"pr.checks | where(item.state == 'fail')"},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["empty-1","facts-1","checks","text-1","button-1"]},"empty-1":{"type":"empty","when":"pr.number == null","props":{"title":"No pull request yet","body":"This thread's branch has no pull request."}},"facts-1":{"type":"facts","when":"pr.number != null","props":{"facts":[{"label":"State","value":{"bind":"pr.word"}},{"label":"Review","value":{"bind":"word(pr.review)"}},{"label":"Mergeable","value":{"bind":"word(pr.mergeable)"},"tone":{"bind":"pr.mergeable == 'mergeable' ? 'good' : 'muted'"}}]}},"checks":{"type":"table","when":"pr.number != null","props":{"items":{"bind":"pr.checks"},"key":{"bind":"item.name"},"empty":"No checks yet","columns":[{"title":"Check","value":{"bind":"item.name"}},{"title":"State","value":{"bind":"word(item.state)"},"tone":{"bind":"item.state == 'fail' ? 'bad' : item.state == 'pass' ? 'good' : 'muted'"}},{"title":"Took","value":{"bind":"item.completedAt ? duration(num(item.completedAt) - num(item.startedAt)) : 'running'"},"mono":true,"align":"end"}],"rowActions":[{"label":"Send to agent","when":"item.state == 'fail'","on":{"press":[{"do":"send","text":"A check failed. Read its log and fix it.","with":["item.name","item.link"]}]}}]}},"text-1":{"type":"text","when":"len($failing) > 0","props":{"tone":"muted","value":{"format":"${len($failing)} failing of ${len(pr.checks)}"}}},"button-1":{"type":"button","when":"pr.number != null","props":{"label":"Open on GitHub"},"on":{"press":[{"do":"open","target":{"bind":"pr.url"}}]}}},"title":"Pull request"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_spec_07_runs_1() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"id":{"start":""},"body":{"start":""}},"derived":{},"runs":{"check":{"kind":"cmd","cmd":"gh api \"repos/{owner}/{repo}/issues/$ID\"","env":{"ID":{"bind":"$id"}},"timeout":20},"tests":{"kind":"cmd","cmd":"pnpm exec vitest run --minWorkers=1 --maxWorkers=2 $1","once":true,"args":["packages/host/test/parity.test.ts"],"timeout":300,"stream":true},"note":{"kind":"cmd","cmd":"cat > notes/latest.md","stdin":{"bind":"$body"},"on":"host","cwd":"notes"},"issues":{"kind":"tool","server":"linear","tool":"list_issues","every":60,"args":{"first":50}},"board":{"kind":"resource","server":"linear","uri":"linear://team/WSP/board","every":60}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["input-1","input-2","button-1","button-2","output-1","button-3"]},"input-1":{"type":"input","props":{"label":"Issue","value":{"bind":"$id"}}},"input-2":{"type":"input","props":{"label":"Note","value":{"bind":"$body"},"lines":4}},"button-1":{"type":"button","props":{"label":"Check"},"on":{"press":[{"do":"start","run":"check"}]}},"button-2":{"type":"button","props":{"label":"Run the parity test"},"on":{"press":[{"do":"start","run":"tests"}]}},"output-1":{"type":"output","props":{"run":"$tests","lines":12}},"button-3":{"type":"button","props":{"label":"Save the note"},"on":{"press":[{"do":"start","run":"note"}]}}},"title":"Runs"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_spec_08_secrets_1() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"token":{"start":null,"secret":true},"saved":{"start":false}},"derived":{},"runs":{"save":{"kind":"cmd","cmd":"printf \"VERCEL_TOKEN=%s\\n\" \"$TOKEN\" >> .env","env":{"TOKEN":{"bind":"$token"}},"on":"host"}},"reactions":[{"on":{"done":"save"},"do":[{"do":"set","path":"$saved","value":{"bind":"$save.exit == 0"}}]}],"pieces":{"column-1":{"type":"column","children":["input-1","text-1","button-1","text-2"]},"input-1":{"type":"input","props":{"label":"Vercel token","value":{"bind":"$token"},"kind":"password"}},"text-1":{"type":"text","when":"$token.set and not $saved","props":{"tone":"muted","value":{"format":"Filled, ${$token.len} characters"}}},"button-1":{"type":"button","when":"$token.set","props":{"label":"Write it to .env"},"on":{"press":[{"do":"start","run":"save"}]}},"text-2":{"type":"text","when":"$saved","props":{"tone":"good","value":"Written to .env"}}},"title":"Deploy key"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_spec_appendix_examples_1() {
    check(
        r#"{"schema":2,"root":"column-1","values":{},"derived":{"failing":"pr.checks | where(item.state == 'fail')"},"runs":{"merge":{"kind":"cmd","cmd":"gh pr merge --squash --auto \"$PR\"","confirm":"Enable auto-merge on this pull request? It merges when the checks pass.","env":{"PR":{"bind":"pr.number"}},"timeout":30}},"reactions":[],"pieces":{"column-1":{"type":"column","children":["empty-1","head","checks","summary","row-1","text-1","text-2"]},"empty-1":{"type":"empty","when":"pr.number == null","props":{"title":"No pull request yet","body":"This thread's branch has no pull request."}},"head":{"type":"facts","when":"pr.number != null","props":{"facts":[{"label":"State","value":{"bind":"pr.word"}},{"label":"Review","value":{"bind":"word(pr.review)"}},{"label":"Mergeable","value":{"bind":"word(pr.mergeable)"},"tone":{"bind":"pr.mergeable == 'mergeable' ? 'good' : pr.mergeable == 'conflicting' ? 'warning' : 'muted'"}}]}},"checks":{"type":"table","when":"pr.number != null","props":{"items":{"bind":"pr.checks"},"key":{"bind":"item.name"},"empty":"No checks yet","columns":[{"title":"Check","value":{"bind":"item.name"}},{"title":"State","value":{"bind":"word(item.state)"},"tone":{"bind":"item.state == 'fail' ? 'bad' : item.state == 'pass' ? 'good' : 'muted'"}},{"title":"Took","value":{"bind":"item.completedAt ? duration(num(item.completedAt) - num(item.startedAt)) : 'running'"},"mono":true,"align":"end"}],"rowActions":[{"label":"Send to agent","when":"item.state == 'fail'","on":{"press":[{"do":"send","text":"A check failed. Read its log and fix it.","with":["item.name","item.link"]}]}}]}},"summary":{"type":"text","when":"len($failing) > 0","props":{"tone":"muted","value":{"format":"${len($failing)} failing of ${len(pr.checks)}"}}},"row-1":{"type":"row","when":"pr.number != null","children":["button-1","button-2","automerge"]},"button-1":{"type":"button","props":{"label":"Open on GitHub"},"on":{"press":[{"do":"open","target":{"bind":"pr.url"}}]}},"button-2":{"type":"button","props":{"label":"Checks and comments","variant":"quiet"},"on":{"press":[{"do":"pane","kind":"pr"}]}},"automerge":{"type":"button","when":"pr.mergeable == 'mergeable' and pr.review == 'approved' and $merge.state != 'done'","props":{"label":"Auto-merge"},"on":{"press":[{"do":"start","run":"merge"}]}},"text-1":{"type":"text","when":"$merge.state == 'done'","props":{"tone":"good","value":"Auto-merge is on"}},"text-2":{"type":"text","when":"$merge.state == 'failed'","props":{"tone":"bad","value":{"format":"gh refused: ${short($merge.err, 120)}"}}}},"title":"Pull request"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_spec_appendix_examples_2() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"steps":{"start":[{"title":"Find where the cart rounds totals","done":true},{"title":"Round once at the end","done":true},{"title":"Fix the two failing tests","done":false},{"title":"Open a pull request","done":false}]}},"derived":{"done":"$steps | where(item.done) | count"},"runs":{"tests":{"kind":"cmd","cmd":"pnpm exec vitest run --minWorkers=1 --maxWorkers=2 test/cart.test.ts","once":true,"timeout":300,"stream":true}},"reactions":[{"on":{"done":"tests"},"do":[{"do":"set","path":"$steps[2].done","value":{"bind":"$tests.exit == 0"}}]}],"pieces":{"column-1":{"type":"column","children":["progress","todo","section-1","row-1","output-1"]},"progress":{"type":"meter","props":{"label":"Steps","value":{"bind":"$done"},"max":{"bind":"len($steps)"},"format":"fraction"}},"todo":{"type":"checklist","props":{"items":{"bind":"$steps"},"key":{"bind":"item.title"},"title":{"bind":"item.title"},"done":{"bind":"item.done"},"editable":true}},"section-1":{"type":"section","props":{"title":"Files changed","collapsible":true},"children":["table-1"]},"table-1":{"type":"table","props":{"items":{"bind":"thread.changes.files"},"key":{"bind":"item.path"},"empty":"Nothing changed yet","columns":[{"title":"File","value":{"bind":"item.path"},"mono":true},{"title":"Added","value":{"bind":"item.additions"},"tone":"good","align":"end"},{"title":"Removed","value":{"bind":"item.deletions"},"tone":"bad","align":"end"}]}},"row-1":{"type":"row","children":["button-1","next"]},"button-1":{"type":"button","when":"$tests.state != 'running'","props":{"label":"Run the cart tests"},"on":{"press":[{"do":"start","run":"tests"}]}},"next":{"type":"button","when":"$done < len($steps) and thread.status != 'working'","props":{"label":"Go on to the next step","variant":"primary"},"on":{"press":[{"do":"send","text":"Go on to the next unchecked step on the slate.","with":["$steps"]}]}},"output-1":{"type":"output","props":{"run":"$tests","lines":10}}},"title":"Cart rounding fix"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_spec_appendix_examples_3() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"step":{"start":1},"repo":{"start":"acme/landing"},"project":{"start":""},"vercelToken":{"start":null,"secret":true}},"derived":{"checked":"$check.state == 'done' and contains($check.out, $project)","written":"$env.state == 'done' and $ci.state == 'done'"},"runs":{"check":{"kind":"cmd","cmd":"vercel project inspect \"$PROJECT\" 2>&1 | head -c 4000","env":{"PROJECT":{"bind":"$project"},"VERCEL_TOKEN":{"bind":"$vercelToken"}},"on":"host","timeout":30},"env":{"kind":"cmd","cmd":"grep -v \"^VERCEL_TOKEN=\" .env > .env.tmp 2>/dev/null; printf \"VERCEL_TOKEN=%s\\n\" \"$VERCEL_TOKEN\" >> .env.tmp; mv .env.tmp .env","env":{"VERCEL_TOKEN":{"bind":"$vercelToken"}},"on":"host","timeout":10},"ci":{"kind":"cmd","cmd":"gh secret set VERCEL_TOKEN --repo \"$REPO\"","env":{"REPO":{"bind":"$repo"}},"stdin":{"bind":"$vercelToken"},"on":"host","timeout":60}},"reactions":[{"on":{"change":["$project"]},"do":[{"do":"start","run":"check"}]},{"on":{"change":["$checked"]},"do":[{"do":"set","path":"$step","value":{"bind":"$checked ? 3 : 2"}}]},{"on":{"change":["$written"]},"do":[{"do":"set","path":"$step","value":{"bind":"$written ? 4 : 3"}}]}],"pieces":{"column-1":{"type":"column","children":["text-1","section-1","section-2","section-3","section-4"]},"text-1":{"type":"text","props":{"emphasis":"strong","value":{"format":"Step ${$step} of 4"}}},"section-1":{"type":"section","when":"$step == 1","props":{"title":"1. The Vercel token"},"children":["markdown-1","button-1","input-1","button-2"]},"markdown-1":{"type":"markdown","props":{"value":"Open the Vercel dashboard, make a token with the project's scope, and paste it here. It never reaches the agent."}},"button-1":{"type":"button","props":{"label":"Open Vercel tokens"},"on":{"press":[{"do":"open","target":"https://vercel.com/account/tokens"}]}},"input-1":{"type":"input","props":{"label":"Vercel token","value":{"bind":"$vercelToken"},"kind":"password"}},"button-2":{"type":"button","when":"$vercelToken.set","props":{"label":"Next","variant":"primary"},"on":{"press":[{"do":"set","path":"$step","value":2}]}},"section-2":{"type":"section","when":"$step == 2 or $step == 3","props":{"title":"2. The project"},"children":["project-name","text-2","text-3","text-4","text-5"]},"project-name":{"type":"input","props":{"label":"Project name on Vercel","value":{"bind":"$project"},"mono":true}},"text-2":{"type":"text","when":"$check.state == 'held'","props":{"tone":"muted","value":"Approve the check to go on"}},"text-3":{"type":"text","when":"$check.state == 'running'","props":{"tone":"muted","value":"Checking"}},"text-4":{"type":"text","when":"$check.state == 'failed'","props":{"tone":"bad","value":{"format":"Vercel did not find it: ${short($check.err, 160)}"}}},"text-5":{"type":"text","when":"$checked","props":{"tone":"good","value":{"format":"Found ${$project}"}}},"section-3":{"type":"section","when":"$step == 3","props":{"title":"3. Write it down"},"children":["facts-1","row-1","text-6","text-7","text-8"]},"facts-1":{"type":"facts","props":{"facts":[{"label":"Repo","value":{"bind":"$repo"},"mono":true},{"label":"Project","value":{"bind":"$project"},"mono":true}]}},"row-1":{"type":"row","children":["button-3","button-4"]},"button-3":{"type":"button","when":"$env.state != 'done'","props":{"label":"Write .env"},"on":{"press":[{"do":"start","run":"env"}]}},"button-4":{"type":"button","when":"$ci.state != 'done'","props":{"label":"Set the CI secret"},"on":{"press":[{"do":"start","run":"ci"}]}},"text-6":{"type":"text","when":"$env.state == 'done'","props":{"tone":"good","value":".env written"}},"text-7":{"type":"text","when":"$ci.state == 'done'","props":{"tone":"good","value":"CI secret set"}},"text-8":{"type":"text","when":"$env.state == 'failed' or $ci.state == 'failed'","props":{"tone":"bad","value":{"format":"${short(concat($env.err, $ci.err), 200)}"}}},"section-4":{"type":"section","when":"$step == 4","props":{"title":"4. Done"},"children":["text-9","write"]},"text-9":{"type":"text","props":{"tone":"good","value":"The token is in .env and in the repo's secrets."}},"write":{"type":"button","props":{"label":"Tell the agent","variant":"primary"},"on":{"press":[{"do":"send","text":"The Vercel setup is done on the slate; wire the deploy hook next.","with":["$repo","$project","$vercelToken"]}]}}},"title":"Deploy setup"}"#,
        vec![],
        vec![row(
            "T311",
            "primary on button-2, write; one per slate reads loud, the rest draw as default",
            Some("write"),
            None,
            None,
            None,
        )],
    );
}

#[test]
fn block_spec_appendix_examples_4() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"hide":{"start":"Done"},"pick":{"start":""}},"derived":{"open":"$issues.json | where(item.state != $hide) | sortBy(item.priority)"},"runs":{"issues":{"kind":"tool","server":"linear","tool":"list_issues","every":60,"always":true,"args":{"first":50}},"finish":{"kind":"tool","server":"linear","tool":"update_issue","args":{"id":{"bind":"$pick"},"state":"Done"}}},"reactions":[{"on":{"done":"finish"},"do":[{"do":"start","run":"issues"}]}],"pieces":{"column-1":{"type":"column","children":["text-1","row-1","board"]},"text-1":{"type":"text","when":"mcp.linear.state != 'allowed'","props":{"tone":"muted","value":"Allow Linear in this thread's slate to see the board."}},"row-1":{"type":"row","props":{"align":"between"},"children":["text-2","select-1"]},"text-2":{"type":"text","props":{"emphasis":"strong","value":{"format":"${len($open)} open"}}},"select-1":{"type":"select","props":{"label":"Hide","value":{"bind":"$hide"},"options":[{"value":"Done"},{"value":"Canceled"},{"value":"In Review"}]}},"board":{"type":"table","props":{"items":{"bind":"$open"},"key":{"bind":"item.id"},"empty":"Nothing open","columns":[{"title":"Issue","value":{"bind":"item.identifier"},"mono":true},{"title":"Title","value":{"bind":"item.title"}},{"title":"Who","value":{"bind":"item.assignee"},"tone":"muted"}],"rowActions":[{"label":"Work on this","on":{"press":[{"do":"send","text":"Work on this Linear issue. Read it on Linear first.","with":["item.identifier","item.url"]}]}},{"label":"Done","on":{"press":[{"do":"set","path":"$pick","value":{"bind":"item.id"}},{"do":"start","run":"finish"}]}}]}}},"title":"Linear"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn block_spec_appendix_examples_5() {
    check(
        r#"{"schema":2,"root":"column-1","values":{"startedAt":{"start":null},"minutes":{"start":"25"}},"derived":{"length":"num($minutes) * 60000","left":"exists($startedAt) ? max(0, $startedAt + $length - time.now) : null","over":"exists($startedAt) and time.now >= $startedAt + $length","working":"thread.plan.steps | where(item.state == 'working') | first"},"runs":{},"reactions":[],"pieces":{"column-1":{"type":"column","children":["text-1","text-2","meter-1","text-3","row-1","text-4"]},"text-1":{"type":"text","props":{"emphasis":"strong","value":"Focus timer"}},"text-2":{"type":"text","when":"exists($startedAt)","props":{"size":"large","mono":true,"value":{"format":"${duration($left)} left"}}},"meter-1":{"type":"meter","when":"exists($startedAt)","props":{"label":"Elapsed","value":{"bind":"clamp(time.now - $startedAt, 0, $length)"},"max":{"bind":"$length"},"format":"none"}},"text-3":{"type":"text","when":"$over","props":{"tone":"good","emphasis":"strong","value":"Time. Stretch, then press Reset."}},"row-1":{"type":"row","children":["button-1","button-2","select-1"]},"button-1":{"type":"button","when":"not exists($startedAt)","props":{"label":"Start","variant":"primary"},"on":{"press":[{"do":"set","path":"$startedAt","value":{"bind":"time.now"}}]}},"button-2":{"type":"button","when":"exists($startedAt)","props":{"label":"Reset","variant":"quiet"},"on":{"press":[{"do":"set","path":"$startedAt","value":null}]}},"select-1":{"type":"select","props":{"label":"Minutes","value":{"bind":"$minutes"},"options":["15","25","45"]}},"text-4":{"type":"text","props":{"tone":"muted","size":"small","value":{"format":"Meanwhile the agent is on ${orElse($working.text, 'nothing in particular')}"}}}},"title":"Focus"}"#,
        vec![],
        vec![],
    );
}

#[test]
fn every_code_the_validator_raises_is_in_the_closed_table() {
    for code in [
        "A600", "A601", "A602", "A606", "A607", "A608", "A610", "A611", "D200", "D202", "D203", "D205", "D206", "D207", "K700", "K701",
        "K702", "K703", "K704", "K705", "K707", "K708", "P103", "Q420", "Q422", "Q423", "Q424", "Q425", "Q426", "S501", "S502", "S503",
        "S510", "S512", "S513", "S520", "T300", "T302", "T303", "T304", "T305", "T306", "T307", "T308", "T309", "T311", "T312", "W001",
        "W003", "W004", "W011", "W012", "W013", "W014", "W015", "W017", "W018", "W019", "X400", "X401", "X404", "X405", "X406", "X407",
        "X408", "X409", "X410", "X420",
    ] {
        assert!(SLATE_CODES.iter().any(|(known, _)| *known == code), "{code} is not in the table");
    }
}

#[test]
fn patch_remove_piece() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"remove","id":"a"}]"#,
        r#"null"#,
        Want::Applied(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["b"]},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
            r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
            vec![],
        ),
    );
}

#[test]
fn patch_add_under_at_zero() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"add","under":"top","at":0,"pieces":{"c":{"type":"text","props":{"value":"C"}}},"order":["c"]}]"#,
        r#"null"#,
        Want::Applied(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["c","a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}},"c":{"type":"text","props":{"value":"C"}}},"files":{"keep.py":"print(0)"}}"#,
            r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
            vec![],
        ),
    );
}

#[test]
fn patch_move_under_itself() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"move","id":"a","under":"a"}]"#,
        r#"null"#,
        Want::Refused(vec![row("D205", "a cannot move under itself", Some("a"), None, None, None)], vec![]),
    );
}

#[test]
fn patch_props_set_and_add() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"props","id":"a","props":{"value":"Z","tone":"good"}}]"#,
        r#"null"#,
        Want::Applied(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"Z","tone":"good"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
            r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
            vec![],
        ),
    );
}

#[test]
fn patch_props_null_removes_value() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"props","id":"a","props":{"value":null}}]"#,
        r#"null"#,
        Want::Refused(vec![row("T304", "text \"a\" needs value", Some("a"), Some("value"), None, None)], vec![]),
    );
}

#[test]
fn patch_replace_with_children() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"replace","id":"a","piece":{"type":"text","props":{"value":"new"}},"children":{}}]"#,
        r#"null"#,
        Want::Applied(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"new"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
            r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
            vec![],
        ),
    );
}

#[test]
fn patch_value_added() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"value","name":"n2","decl":{"start":5}}]"#,
        r#"null"#,
        Want::Applied(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true},"n2":{"start":5}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
            r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"n2":5,"r":{"state":"idle","runs":0}}"#,
            vec![],
        ),
    );
}

#[test]
fn patch_value_removed_leaves_reference() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"value","name":"n","decl":null}]"#,
        r#"null"#,
        Want::Refused(
            vec![
                row(
                    "S501",
                    "$n is not declared; add <value name=\"n\" start=... /> or did you mean $s?",
                    Some("$d"),
                    Some("value"),
                    Some("$s"),
                    None,
                ),
                row(
                    "S501",
                    "$n is not declared; add <value name=\"n\" start=... /> or did you mean $s?",
                    Some("b"),
                    Some("value"),
                    Some("$s"),
                    None,
                ),
            ],
            vec![],
        ),
    );
}

#[test]
fn patch_derived_added() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"derived","name":"d2","expr":"$n * 2"}]"#,
        r#"null"#,
        Want::Applied(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1","d2":"$n * 2"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
            r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
            vec![],
        ),
    );
}

#[test]
fn patch_run_removed() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"run","name":"r","decl":null}]"#,
        r#"null"#,
        Want::Applied(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
            r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null}}"#,
            vec![],
        ),
    );
}

#[test]
fn patch_reaction_added() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"reaction","id":"x1","reaction":{"id":"x1","on":{"change":["$n"]},"do":[{"do":"set","path":"$s","value":"y"}]}}]"#,
        r#"null"#,
        Want::Applied(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[{"id":"x1","on":{"change":["$n"]},"do":[{"do":"set","path":"$s","value":"y"}]}],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
            r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
            vec![],
        ),
    );
}

#[test]
fn patch_reaction_missing_removed() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"reaction","id":"nope","reaction":null}]"#,
        r#"null"#,
        Want::Refused(vec![row("D203", "there is no reaction nope", Some("nope"), None, None, None)], vec![]),
    );
}

#[test]
fn patch_file_added() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"file","name":"a.py","text":"print(1)"}]"#,
        r#"null"#,
        Want::Applied(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)","a.py":"print(1)"}}"#,
            r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
            vec![],
        ),
    );
}

#[test]
fn patch_file_missing_removed() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"file","name":"gone.py","text":null}]"#,
        r#"null"#,
        Want::Refused(vec![row("D203", "there is no file gone.py", None, None, None, None)], vec![]),
    );
}

#[test]
fn patch_file_added_then_removed() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"file","name":"a.py","text":"x"},{"op":"file","name":"a.py","text":null}]"#,
        r#"null"#,
        Want::Applied(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
            r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
            vec![],
        ),
    );
}

#[test]
fn patch_clear() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"clear"}]"#,
        r#"null"#,
        Want::Cleared(r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#),
    );
}

#[test]
fn patch_undo_refused() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"undo"}]"#,
        r#"null"#,
        Want::Refused(
            vec![row(
                "V752",
                "undo goes back to the document before the agent's last write, which the host holds; send <undo /> alone to the host",
                None,
                None,
                None,
                None,
            )],
            vec![],
        ),
    );
}

#[test]
fn patch_remove_root_refused() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"remove","id":"top"}]"#,
        r#"null"#,
        Want::Refused(
            vec![row("D202", "the root piece cannot be removed; write <clear /> or replace it", Some("top"), None, None, None)],
            vec![],
        ),
    );
}

#[test]
fn patch_props_missing_piece() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"props","id":"ghost","props":{}}]"#,
        r#"null"#,
        Want::Refused(vec![row("D203", "there is no piece ghost", Some("ghost"), None, None, None)], vec![]),
    );
}

#[test]
fn patch_add_existing_piece() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"add","under":"top","pieces":{"a":{"type":"text","props":{"value":"dup"}}},"order":["a"]}]"#,
        r#"null"#,
        Want::Refused(vec![row("P104", "a is already a piece; replace it by writing it with its id", Some("a"), None, None, None)], vec![]),
    );
}

#[test]
fn patch_move_missing_piece() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"move","id":"ghost","under":"top"}]"#,
        r#"null"#,
        Want::Refused(vec![row("D203", "move names ghost, which is not a piece", Some("ghost"), None, None, None)], vec![]),
    );
}

#[test]
fn patch_clear_with_other_op() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"clear"},{"op":"remove","id":"a"}]"#,
        r#"null"#,
        Want::Refused(vec![row("P105", "<clear /> stands alone in its write", None, None, None, None)], vec![]),
    );
}

#[test]
fn patch_remove_then_validate_refuses() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"remove","id":"b"},{"op":"props","id":"a","props":{"value":null}}]"#,
        r#"null"#,
        Want::Refused(vec![row("T304", "text \"a\" needs value", Some("a"), Some("value"), None, None)], vec![]),
    );
}

#[test]
fn patch_set_run_bad_kind() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"run","name":"r","decl":{"kind":"nope"}}]"#,
        r#"null"#,
        Want::Refused(
            vec![row("K701", "$r takes exactly one of cmd, tool or resource", Some("$r"), Some("kind"), None, None)],
            vec![row(
                "W019",
                "nothing starts $r: give it a button, a when, or every",
                None,
                None,
                Some("<button label=\"...\" onPress={start($r)} />"),
                None,
            )],
        ),
    );
}

#[test]
fn patch_move_child_up() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"move","id":"b","under":"top","at":0}]"#,
        r#"null"#,
        Want::Applied(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["b","a"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
            r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
            vec![],
        ),
    );
}

#[test]
fn patch_add_at_end_clamped() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"add","under":"top","at":99,"pieces":{"c":{"type":"text","props":{"value":"C"}}},"order":["c"]}]"#,
        r#"null"#,
        Want::Applied(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b","c"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}},"c":{"type":"text","props":{"value":"C"}}},"files":{"keep.py":"print(0)"}}"#,
            r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
            vec![],
        ),
    );
}

#[test]
fn patch_derived_cycle_refused() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"derived","name":"n","expr":"$d"}]"#,
        r#"null"#,
        Want::Refused(
            vec![
                row("P104", "$n is declared twice", Some("$n"), None, None, None),
                row(
                    "S510",
                    "derived values read each other in a cycle: $d reads $n reads $d",
                    Some("$d"),
                    Some("value"),
                    Some("make one of them a value"),
                    None,
                ),
            ],
            vec![],
        ),
    );
}

#[test]
fn patch_lines_on_missing_piece() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"remove","id":"zz"}]"#,
        r#"{"zz":7}"#,
        Want::Refused(vec![row("D203", "there is no piece zz to remove", Some("zz"), None, None, Some(7))], vec![]),
    );
}

#[test]
fn patch_lines_on_validation_prefix() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"props","id":"a","props":{"value":null}}]"#,
        r#"{"a":4}"#,
        Want::Refused(vec![row("T304", "text \"a\" needs value", Some("a"), Some("value"), None, Some(4))], vec![]),
    );
}

#[test]
fn patch_lines_on_duplicate_piece() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null},"r":{"state":"idle","runs":0}}"#,
        r#"[{"op":"add","under":"top","pieces":{"a":{"type":"text","props":{"value":"dup"}}},"order":["a"]}]"#,
        r#"{"a":9}"#,
        Want::Refused(
            vec![row("P104", "a is already a piece; replace it by writing it with its id", Some("a"), None, None, Some(9))],
            vec![],
        ),
    );
}

#[test]
fn patch_keeps_live_values_drops_removed() {
    run_patch(
        Some(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{"r":{"kind":"cmd","cmd":"echo","every":60}},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
        ),
        r#"{"n":7,"s":"hi","r":{"state":"done","runs":1}}"#,
        r#"[{"op":"value","name":"n","decl":{"start":1}},{"op":"run","name":"r","decl":null}]"#,
        r#"null"#,
        Want::Applied(
            r#"{"schema":2,"title":"T","root":"top","values":{"n":{"start":1},"s":{"start":"x"},"sec":{"start":null,"secret":true}},"derived":{"d":"$n + 1"},"runs":{},"reactions":[],"pieces":{"top":{"type":"column","children":["a","b"]},"a":{"type":"text","props":{"value":"A"}},"b":{"type":"text","props":{"value":{"bind":"$n"}}}},"files":{"keep.py":"print(0)"}}"#,
            r#"{"n":7,"s":"hi","sec":{"secret":true,"set":false,"len":0,"at":null}}"#,
            vec![],
        ),
    );
}

#[test]
fn patch_without_slate_refused() {
    run_patch(
        None,
        r#"{}"#,
        r#"[{"op":"props","id":"a","props":{}}]"#,
        r#"null"#,
        Want::Refused(
            vec![row(
                "Z802",
                "there is no slate to patch; write one with <slate>",
                None,
                None,
                Some("<slate title=\"...\">...</slate>"),
                None,
            )],
            vec![],
        ),
    );
}

#[test]
fn patch_clear_without_slate() {
    run_patch(None, r#"{}"#, r#"[{"op":"clear"}]"#, r#"null"#, Want::Cleared(r#"{}"#));
}
