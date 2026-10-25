// SPDX-License-Identifier: AGPL-3.0-only
//! What every core-types test reads: the TS oracle's answers (oracle/core_types.oracle.ts) and wire values built off
//! JSON, so a test names its input as the TS test does.
#![allow(dead_code)]

use serde::de::DeserializeOwned;
use serde_json::Value;

/// One unit's TS answers, tests/fixtures/core_types/<name>.json.
pub fn fixture(name: &str) -> Value {
    let path = format!("{}/tests/fixtures/core_types/{name}.json", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// A wire value read off JSON, failing the test with serde's words where it does not read.
pub fn wire<T: DeserializeOwned>(v: &Value) -> T {
    serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("{v} does not read: {e}"))
}

/// Every element of a JSON array read as wire values.
pub fn wires<T: DeserializeOwned>(v: &Value) -> Vec<T> {
    v.as_array().expect("an array").iter().map(wire).collect()
}

/// A value as JSON, keys in the order the value writes them: what a test compares when key order is the point.
pub fn json_of<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string(v).expect("serializes")
}

/// A session event for workspace w1 and session s1 unless the fields name others, as the oracle's `ev` builds one.
pub fn ev(kind: &str, fields: Value) -> Value {
    let mut out = serde_json::json!({ "type": kind, "workspaceId": "w1", "sessionId": "s1" });
    for (k, v) in fields.as_object().expect("an object") {
        out[k] = v.clone();
    }
    out
}
