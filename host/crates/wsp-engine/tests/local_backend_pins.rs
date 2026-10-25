// SPDX-License-Identifier: AGPL-3.0-only
//! This computer's memory readers on edge inputs against the TS (tests/fixtures/scenes/units.json;
//! spec/local_backend.md rules L-1, L-2).
use serde_json::json;
use wsp_engine::local_backend::*;

const UNIT: &str = "local_backend";
fn pins() -> serde_json::Value {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/scenes/units.json")).unwrap();
    serde_json::from_str::<serde_json::Value>(&text).unwrap()[UNIT].clone()
}

fn as_json(m: Option<MemoryHere>) -> serde_json::Value {
    m.map(|m| json!({ "freeMb": m.free_mb, "totalMb": m.total_mb })).unwrap_or(serde_json::Value::Null)
}

/// L-1: the percentage is JS Number of the trimmed reading: 0b, 0o, 0x, a sign, a bare point and an exponent all count;
/// Infinity and `1_0` do not; -0 is 0.
#[test]
fn the_pressure_reading_takes_every_js_number_form() {
    for case in pins()["pressure"].as_array().unwrap() {
        assert_eq!(as_json(memory_of_pressure(case["in"].as_str().unwrap(), 16 << 30)), case["out"], "{}", case["in"]);
    }
}

/// L-2: `^KEY:\s*(\d+)\s*kB` under JS `m`: the white space may cross a newline, `^` also follows a `\r`, `kB` is
/// case-sensitive, and the first match counts.
#[test]
fn meminfo_is_read_with_javascript_line_rules() {
    for case in pins()["meminfo"].as_array().unwrap() {
        assert_eq!(as_json(memory_of_meminfo(case["in"].as_str().unwrap())), case["out"], "{:?}", case["in"]);
    }
}
