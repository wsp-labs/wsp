// SPDX-License-Identifier: AGPL-3.0-only
//! The probe's reader on edge lines against the TS (tests/fixtures/scenes/units.json; spec/context_probe.md rules
//! P-1..P-4).
use serde_json::{json, Value};
use wsp_engine::machine_context::*;

const UNIT: &str = "context_probe";
fn pins() -> serde_json::Value {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/scenes/units.json")).unwrap();
    serde_json::from_str::<serde_json::Value>(&text).unwrap()[UNIT].clone()
}

fn seen(stdout: &str) -> Value {
    match parse_probe(stdout) {
        None => Value::Null,
        Some(p) => json!({
            "kernel": p.kernel,
            "disk": p.disk.map(|d| json!({ "sizeBytes": d.size_bytes, "freeBytes": d.free_bytes })),
            "facts": p.facts.map(|f| serde_json::to_value(f).unwrap()),
            "aliases": p.aliases.iter().map(|a| json!({ "name": a.name, "word": a.word })).collect::<Vec<_>>(),
            "has": p.has.iter().collect::<Vec<_>>(),
        }),
    }
}

/// Numbers compared as f64: the TS writes 16384 where serde_json may write 16384.0.
fn normal(v: &Value) -> Value {
    match v {
        Value::Number(n) => json!(n.as_f64().unwrap()),
        Value::Array(a) => Value::Array(a.iter().map(normal).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, v)| (k.clone(), normal(v))).collect()),
        other => other.clone(),
    }
}

fn same(section: &str) {
    for case in pins()[section].as_array().unwrap() {
        let lines: Vec<String> = match &case["in"] {
            Value::String(s) if section == "facts" => vec![format!("FACTS {s}")],
            Value::String(s) => vec![s.clone()],
            Value::Array(a) => a.iter().map(|l| l.as_str().unwrap().to_string()).collect(),
            _ => unreachable!(),
        };
        let mut all = vec!["WSP_CTX".to_string()];
        all.extend(lines.iter().cloned());
        all.push("WSP_CTX_END".to_string());
        assert_eq!(normal(&seen(&all.join("\n"))), normal(&case["out"]), "{lines:?}");
    }
}

/// P-1: DISK reads its first two words with JS Number: hex and exponents count, more words are ignored, a negative
/// free is kept, the size must be above 0, and a non-finite figure is no disk.
#[test]
fn disk_words_are_read_with_js_number() {
    same("disk");
}

/// P-2: FACTS is base64 as Node reads it (no padding, white space skipped, stops at the first =), then JSON: null is
/// none, while a number, an array or a string is the empty facts.
#[test]
fn facts_decode_as_node_and_a_scalar_is_the_empty_facts() {
    same("facts");
}

/// P-3: the key ends at the first space only: a tab is part of the key, a leading space makes the key empty, and
/// ALIAS splits its rest at a space only.
#[test]
fn keys_split_at_a_space_only() {
    same("keys");
}

/// P-4: lines are trimmed at the end with JS white space: U+FEFF and U+00A0 go, U+0085 stays.
#[test]
fn the_markers_are_found_after_a_js_trim_end() {
    for case in pins()["trimEnd"].as_array().unwrap() {
        let lines: Vec<&str> = case["in"].as_array().unwrap().iter().map(|l| l.as_str().unwrap()).collect();
        let got = parse_probe(&lines.join("\n")).and_then(|p| p.kernel);
        assert_eq!(json!(got), case["kernel"], "{lines:?}");
    }
}
