// SPDX-License-Identifier: AGPL-3.0-only
//! Box's refusal words on edge views against the TS (tests/fixtures/scenes/units.json; spec/box_request.md rule
//! B-R3).
use wsp_engine::box_backend::*;

const UNIT: &str = "box_request";
fn pins() -> serde_json::Value {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/scenes/units.json")).unwrap();
    serde_json::from_str::<serde_json::Value>(&text).unwrap()[UNIT].clone()
}

/// B-R3: only an empty error is skipped; an empty status, attempt or completion is written as it is, and an error of
/// white space alone is the words.
#[test]
fn stop_refusal_writes_empty_strings_as_they_are() {
    for case in pins()["stopRefusal"].as_array().unwrap() {
        let mut v = case["in"].clone();
        v["id"] = "bx_1".into();
        v["state"] = "running".into();
        let view: BoxView = serde_json::from_value(v).unwrap();
        assert_eq!(stop_refusal_of(&view), case["out"].as_str().unwrap(), "{}", case["in"]);
    }
}
