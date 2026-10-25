// SPDX-License-Identifier: AGPL-3.0-only
//! Hard tests of box_backend::plan against the TS's own answers (spec/box_plan.md).
#[path = "oracle/mod.rs"]
mod oracle;

use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_engine::box_backend::*;
use wsp_engine::machine::{BackendPricing, MachineState, Size};

#[test]
fn the_class_table_and_root_disks_are_the_ts_tables_in_order() {
    let o = oracle::load("box.json");
    let classes = o["classes"].as_array().unwrap();
    assert_eq!(classes.len(), BOX_CLASSES.len());
    for (c, want) in BOX_CLASSES.iter().zip(classes) {
        assert_eq!(c.kind.as_str(), oracle::s(&want["type"]));
        assert_eq!(c.name, oracle::s(&want["name"]));
        assert_eq!(c.cpu, want["cpu"].as_f64().unwrap());
        assert_eq!(c.mem_mb, want["memMb"].as_f64().unwrap());
        assert_eq!(c.disk_gb, want["diskGb"].as_f64().unwrap());
        assert_eq!(c.rate_usd_per_hour, want["rateUsdPerHour"].as_f64().unwrap());
        assert_eq!(box_root_disk_gb(c.kind), o["rootDisk"][c.kind.as_str()].as_f64());
        assert_eq!(BoxClassType::parse(c.kind.as_str()), Some(c.kind));
    }
    assert_eq!(BoxClassType::parse("huge"), None);
    assert_eq!(box_root_disk_gb(BoxClassType::Xlarge), None);
}

#[test]
fn every_constant_is_the_ts_value() {
    let c = &oracle::load("box.json")["constants"];
    let pairs: [(&str, u64); 22] = [
        ("BOX_PAUSE_MS", BOX_PAUSE_MS),
        ("BOX_RESUME_MS", BOX_RESUME_MS),
        ("BOX_CREATE_MS", BOX_CREATE_MS),
        ("BOX_SNAPSHOT_MS", BOX_SNAPSHOT_MS),
        ("BOX_POLL_MS", BOX_POLL_MS),
        ("BOX_RESTORE_MS", BOX_RESTORE_MS),
        ("BOX_KEY_KEPT_MS", BOX_KEY_KEPT_MS),
        ("LOST_ANSWER_MS", LOST_ANSWER_MS),
        ("LOST_ANSWER_EVERY_MS", LOST_ANSWER_EVERY_MS),
        ("IN_PROGRESS_WAIT_MS", IN_PROGRESS_WAIT_MS),
        ("BACKSTOP_SLACK_MS", BACKSTOP_SLACK_MS),
        ("BACKSTOP_RETRY_MS", BACKSTOP_RETRY_MS),
        ("BACKSTOP_RETRY_MAX_MS", BACKSTOP_RETRY_MAX_MS),
        ("BACKSTOP_CAP_MS", BACKSTOP_CAP_MS),
        ("LIMITS_KEPT_MS", LIMITS_KEPT_MS),
        ("REFUSAL_LOG_QUIET_MS", REFUSAL_LOG_QUIET_MS),
        ("FIREWALL_HOLD_MS", FIREWALL_HOLD_MS),
        ("FIREWALL_WATCH_MS", FIREWALL_WATCH_MS),
        ("ENV_LANDING_MS", ENV_LANDING_MS),
        ("FILE_PUT_MAX", FILE_PUT_MAX),
        ("FILE_GET_MAX", FILE_GET_MAX),
        ("BOX_INLINE_MAX_MS", BOX_INLINE_MAX_MS),
    ];
    for (name, value) in pairs {
        assert_eq!(value, c[name].as_u64().unwrap(), "{name}");
    }
    assert_eq!(TRIAL_TTL_S, c["TRIAL_TTL_S"].as_u64().unwrap());
    assert_eq!(BOX_API_URL, oracle::s(&c["BOX_API_URL"]));
    assert_eq!(BOX_BASE_TEMPLATE, oracle::s(&c["BOX_BASE_TEMPLATE"]));
    assert_eq!(BOX_NAME_MAX as u64, c["BOX_NAME_MAX"].as_u64().unwrap());
    assert_eq!(SNAPSHOT_NAME_MAX as u64, c["SNAPSHOT_NAME_MAX"].as_u64().unwrap());
    assert_eq!(BOX_DISK_KEPT_MS, 14 * 24 * 3_600_000);
}

#[test]
fn the_budgets_and_the_pricing_are_the_ts_objects() {
    let o = oracle::load("box.json");
    assert_eq!(serde_json::to_value(box_budgets()).unwrap(), o["budgets"]);
    let p = BoxPricing;
    assert_eq!(p.default_size(), Size { cpu: 2.0, mem_mb: 4096.0 });
    let st = p.snapshot_storage();
    assert_eq!(serde_json::to_value(&st.wire).unwrap(), json!({ "freeGb": 0, "usdPerGbMonth": 0, "billedFrom": "" }));
    assert_eq!(st.free_named, Some(10));
    assert_eq!(st.usd_per_named_month, Some(1.7));
    assert_eq!(p.builder_disk_gb(), None);
    // A rate is the class the size reads onto, a size between classes included.
    assert_eq!(p.rate_usd_per_hour(Size { cpu: 3.0, mem_mb: 4096.0 }), 0.036);
    assert_eq!(p.rate_usd_per_hour(Size { cpu: 64.0, mem_mb: 1.0 }), 0.2);
}

#[test]
fn a_size_reads_onto_the_class_it_matches_else_the_smallest_that_holds_it_else_the_largest() {
    for case in oracle::load("box.json")["boxClassFor"].as_array().unwrap() {
        let ask = SizeAsk { cpu: case["in"]["cpu"].as_f64(), mem_mb: case["in"]["memMb"].as_f64() };
        assert_eq!(box_class_for(ask).kind.as_str(), oracle::s(&case["out"]), "{case}");
    }
    assert_eq!(class_of_type(Some("large")).map(|c| c.kind), Some(BoxClassType::Large));
    assert_eq!(class_of_type(Some("extra large")), None);
    assert_eq!(class_of_type(None), None);
}

#[test]
fn plan_sizes_sell_two_on_the_trial_and_xlarge_from_100_dollars() {
    for case in oracle::load("box.json")["planSizes"].as_array().unwrap() {
        let got: Vec<&str> = plan_sizes(case["trial"].as_bool().unwrap(), case["planUsd"].as_f64()).iter().map(|k| k.as_str()).collect();
        let want: Vec<&str> = case["out"].as_array().unwrap().iter().map(oracle::s).collect();
        assert_eq!(got, want, "{case}");
    }
}

#[test]
fn limits_of_reads_each_raw_answer_into_the_ts_object_with_its_key_order() {
    for case in oracle::load("box.json")["limitsOf"].as_array().unwrap() {
        let limits = limits_of(&case["in"]);
        let got = serde_json::to_value(&limits).unwrap();
        assert_eq!(got, case["out"], "limits_of({})", case["in"]);
        let keys: Vec<&str> = got.as_object().unwrap().keys().map(String::as_str).collect();
        let want_keys: Vec<&str> = case["keys"].as_array().unwrap().iter().map(oracle::s).collect();
        assert_eq!(keys, want_keys, "key order of limits_of({})", case["in"]);
        let place = place_limits_of(&limits).map(|p| serde_json::to_value(p).unwrap()).unwrap_or(Value::Null);
        assert_eq!(place, case["place"], "place_limits_of({})", case["in"]);
    }
}

#[test]
fn the_state_table_reads_every_word_and_an_unknown_one_reads_running() {
    let table = [
        ("init", MachineState::Starting),
        ("provisioning", MachineState::Starting),
        ("provisioned", MachineState::Starting),
        ("cloning", MachineState::Starting),
        ("ready", MachineState::Running),
        ("idle", MachineState::Running),
        ("running", MachineState::Running),
        ("archiving", MachineState::Paused),
        ("archived", MachineState::Paused),
        ("error", MachineState::Gone),
        ("cancelled", MachineState::Gone),
    ];
    for (word, state) in table {
        assert_eq!(state_map(word), Some(state), "{word}");
        assert_eq!(state_of(word), state, "{word}");
    }
    assert_eq!(state_map("hibernating"), None);
    assert_eq!(state_of("hibernating"), MachineState::Running);
    assert_eq!(state_of(""), MachineState::Running);
    // An inherited property name is no state either.
    assert_eq!(state_map("constructor"), None);
}

#[test]
fn box_name_ranks_wsps_keys_then_the_rest_by_locale_order_and_stops_at_the_first_pair_that_does_not_fit() {
    for case in oracle::load("box.json")["boxName"].as_array().unwrap() {
        let labels: IndexMap<String, String> =
            case["in"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), oracle::s(v).to_string())).collect();
        assert_eq!(box_name(&labels), oracle::s(&case["out"]), "{}", case["in"]);
    }
}

#[test]
fn box_labels_reads_back_only_names_wsp_made_with_their_keys_in_order() {
    for case in oracle::load("box.json")["boxLabels"].as_array().unwrap() {
        let input = case["in"].as_str();
        let got = box_labels(input);
        match case["out"].as_object() {
            None => assert_eq!(got, None, "{case}"),
            Some(want) => {
                let got = got.unwrap_or_else(|| panic!("no labels for {case}"));
                let pairs: Vec<(&str, &str)> = got.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
                let want_pairs: Vec<(&str, &str)> = case["keys"].as_array().unwrap().iter().map(|k| (oracle::s(k), oracle::s(&want[oracle::s(k)]))).collect();
                assert_eq!(pairs, want_pairs, "{case}");
            }
        }
    }
}

#[test]
fn a_name_and_its_labels_round_trip_through_the_encoding() {
    let labels: IndexMap<String, String> =
        [("wsp", "1"), ("wsp-owner", "a;b=c%d"), ("wsp-workspace", "w1")].iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    let name = box_name(&labels);
    assert_eq!(name, "wsp;o=a%3bb%3dc%25d;w=w1");
    assert_eq!(box_labels(Some(&name)), Some(labels));
}

#[test]
fn a_snapshot_name_is_lowered_cleaned_and_a_long_one_ends_in_the_hash_of_the_whole() {
    for case in oracle::load("box.json")["boxSnapshotName"].as_array().unwrap() {
        let input = oracle::s(&case["in"]);
        match (box_snapshot_name(input), case["out"].get("ok"), case["out"].get("threw")) {
            (Ok(got), Some(want), _) => assert_eq!(got, oracle::s(want), "{input:?}"),
            (Err(e), _, Some(want)) => assert_eq!(e.to_string(), oracle::s(want), "{input:?}"),
            (got, _, _) => panic!("{input:?}: {got:?} against {}", case["out"]),
        }
    }
}

#[test]
fn a_character_outside_utf16s_basic_plane_is_two_dashes() {
    // JavaScript's replace walks UTF-16 units: one emoji is two units, so two dashes.
    assert_eq!(box_snapshot_name("a\u{1F600}b").unwrap(), "a--b");
}

#[test]
fn a_uuid_is_a_history_id_in_any_case() {
    assert!(is_uuid("dc8cd70e-d16b-472a-b5d7-070bc84db7a7"));
    assert!(is_uuid("DC8CD70E-D16B-472A-B5D7-070BC84DB7A7"));
    assert!(!is_uuid("wsp-golden-v3"));
    assert!(!is_uuid("dc8cd70e-d16b-472a-b5d7-070bc84db7a"));
    assert!(!is_uuid(" dc8cd70e-d16b-472a-b5d7-070bc84db7a7"));
}
