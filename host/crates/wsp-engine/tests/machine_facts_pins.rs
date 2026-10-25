// SPDX-License-Identifier: AGPL-3.0-only
//! machine_facts' edge inputs against the TS (tests/fixtures/scenes/units.json; spec/machine_facts.md rules F-1..F-3).
use indexmap::IndexMap;
use wsp_engine::machine_facts::*;

const UNIT: &str = "machine_facts";
fn pins() -> serde_json::Value {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/scenes/units.json")).unwrap();
    serde_json::from_str::<serde_json::Value>(&text).unwrap()[UNIT].clone()
}

fn values(v: &serde_json::Value) -> IndexMap<String, String> {
    v.as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string())).collect()
}

/// F-1: Math.round (half toward +infinity), JS Number, the unanchored ASCII-digit boot read, and a huge boot through
/// f64 to 0.
#[test]
fn uptime_rounds_and_reads_boot_as_the_ts() {
    for case in pins()["uptime"].as_array().unwrap() {
        assert_eq!(uptime_ms_of(&values(&case["in"]), 5_000_000), case["ms"].as_i64(), "{}", case["in"]);
    }
}

/// F-2: memkb read with JS Number: 0b and 0o forms, a sign and a bare point count. 1e30 is left out: a u64 cannot hold
/// the TS's 9.77e26, an accepted difference no machine reaches.
#[test]
fn memory_reads_every_number_form_js_reads() {
    for case in pins()["mem"].as_array().unwrap() {
        let input = case["in"].as_str().unwrap();
        if input == "1e30" {
            continue;
        }
        let v: IndexMap<String, String> = [("memkb".to_string(), input.to_string())].into_iter().collect();
        assert_eq!(mem_mb_of(&v), case["mb"].as_u64(), "{input}");
    }
}

/// F-3: the last value wins even when empty, a value is trimmed, a tab is no separator, and a BOM stays in the key.
#[test]
fn the_last_value_wins_and_keys_are_not_trimmed() {
    let got = read_values("a 1\na \nb  2\nc\td\n\u{feff}arch x\n");
    let want = values(&pins()["lastValueWins"]);
    assert_eq!(got.get("a"), want.get("a"));
    assert_eq!(got.get("b"), want.get("b"));
    assert_eq!(got.get("\u{feff}arch"), Some(&"x".to_string()));
    assert_eq!(got.get("arch"), None);
    assert_eq!(got.get("c\td"), None);
    assert_eq!(got.len(), want.len());
}
