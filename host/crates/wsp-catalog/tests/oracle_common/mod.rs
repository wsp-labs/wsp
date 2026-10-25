// SPDX-License-Identifier: AGPL-3.0-only
// Reads the fixtures oracle/dump.oracle.ts wrote from the TS catalog, and compares Rust answers to them.
#![allow(dead_code)]
use serde::Serialize;
use serde_json::Value;

pub fn fixture(name: &str) -> Value {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap()
}

pub fn json<T: Serialize>(v: T) -> Value {
    serde_json::to_value(v).unwrap()
}

/// `null` for `None`, the value for `Some`.
pub fn opt<T: Serialize>(v: Option<T>) -> Value {
    v.map(json).unwrap_or(Value::Null)
}

pub fn s(v: &Value) -> &str {
    v.as_str().unwrap_or_else(|| panic!("not a string: {v}"))
}

/// Compares and names the case in the failure, since one test walks many cases.
#[track_caller]
pub fn same(case: &str, got: Value, want: &Value) {
    assert!(
        &got == want,
        "{case}\n got: {}\nwant: {}",
        serde_json::to_string_pretty(&got).unwrap(),
        serde_json::to_string_pretty(want).unwrap()
    );
}
