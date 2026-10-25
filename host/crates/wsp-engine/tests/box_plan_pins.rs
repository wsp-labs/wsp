// SPDX-License-Identifier: AGPL-3.0-only
//! Box's tables and names on edge inputs against the TS (tests/fixtures/scenes/units.json; spec/box_plan.md rules
//! B-P1..B-P3).
use serde_json::json;
use wsp_engine::box_backend::*;

const UNIT: &str = "box_plan";
fn pins() -> serde_json::Value {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/scenes/units.json")).unwrap();
    serde_json::from_str::<serde_json::Value>(&text).unwrap()[UNIT].clone()
}

/// B-P1: a %XX in a label is one UTF-16 unit of that value (String.fromCharCode), never a UTF-8 decode; a % with no
/// two hex digits after it stays as written.
#[test]
fn labels_decode_each_escape_as_one_latin_1_unit() {
    for case in pins()["labels"].as_array().unwrap() {
        assert_eq!(json!(box_labels(case["in"].as_str())), case["out"], "{}", case["in"]);
    }
}

/// B-P2: a tier with no key matches a plan with no key, so its dollars decide the sizes.
#[test]
fn a_keyless_tier_matches_a_keyless_plan() {
    let got = serde_json::to_value(limits_of(&json!({ "sandboxPlanTiers": [{ "dollars": 100 }] }))).unwrap();
    let want = &pins()["keylessTier"];
    assert_eq!(got["planUsd"].as_f64(), want["planUsd"].as_f64());
    assert_eq!(got["sizes"], want["sizes"]);
}

/// B-P3: a NaN size fits no class, so it is the last one.
#[test]
fn a_nan_size_is_the_largest_class() {
    let got = box_class_for(SizeAsk { cpu: Some(f64::NAN), mem_mb: Some(f64::NAN) });
    assert_eq!(got.kind.as_str(), pins()["classForNaN"].as_str().unwrap());
}
