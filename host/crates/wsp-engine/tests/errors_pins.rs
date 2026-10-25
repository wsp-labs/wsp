// SPDX-License-Identifier: AGPL-3.0-only
//! errors' edge inputs against the TS (tests/fixtures/scenes/units.json, oracle/unit_pins.oracle.ts; spec/errors.md
//! rules E-1, E-2).
use wsp_engine::errors::*;

const UNIT: &str = "errors";
fn pins() -> serde_json::Value {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/scenes/units.json")).unwrap();
    serde_json::from_str::<serde_json::Value>(&text).unwrap()[UNIT].clone()
}

/// E-1: past attempt 4 the wait is 8,000 and never overflows.
#[test]
fn backoff_holds_at_eight_seconds_for_any_attempt() {
    for case in pins()["backoff"].as_array().unwrap() {
        let attempt = case["attempt"].as_u64().unwrap() as u32;
        assert_eq!(backoff_ms(attempt, 0.0), case["ms"].as_u64().unwrap(), "attempt {attempt}");
    }
}

/// E-2: a provider refusal is thrown as an Error carrying its words and code, so "fetch failed" or a network code on
/// it reads as the network, and an unsent code as never sent.
#[test]
fn a_thrown_refusal_with_network_words_or_codes_reads_as_the_network() {
    for case in pins()["network"].as_array().unwrap() {
        let c = &case["in"];
        let w = classify(c["status"].as_u64().unwrap() as u16, c["code"].as_str(), c["error"].as_str(), None);
        assert_eq!(is_network_error(&EngineError::Provider(w)), case["thrown"].as_bool().unwrap(), "{c}");
    }
    assert!(never_sent(&EngineError::Provider(classify(500, Some("ECONNREFUSED"), None, None))));
    assert!(!never_sent(&EngineError::Provider(classify(502, Some("ECONNRESET"), None, None))));
}
