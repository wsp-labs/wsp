// SPDX-License-Identifier: AGPL-3.0-only
//! The TS engine's own answers, printed by oracle/machines.oracle.ts into tests/fixtures/oracle/*.json.
#![allow(dead_code)]
use serde_json::Value;

pub fn load(name: &str) -> Value {
    let path = format!("{}/tests/fixtures/oracle/{name}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).expect("oracle JSON")
}

pub fn s(v: &Value) -> &str {
    v.as_str().unwrap_or_else(|| panic!("not a string: {v}"))
}

/// A JSON null as `None`.
pub fn opt_s(v: &Value) -> Option<&str> {
    v.as_str()
}
