// SPDX-License-Identifier: AGPL-3.0-only
//! Unit tools_disk's hard tests: every value here is what golden-tools.ts printed under oracle/golden.oracle.ts
//! (tests/fixtures/oracle/golden_tools_disk.json). spec/golden-tools_disk.md names the rule each pins.
mod golden_common;

use golden_common::{assert_calls, cases, exec_result, fixture, s, Scripted};
use serde_json::{json, Value};
use wsp_catalog::roads::RoadName;
use wsp_engine::golden_seam::{
    closing, df_kb_cmd, disk_use, disk_use_size, free_bytes, free_kb_cmd, free_note, guard_deadline_ms, guarded, guarded_road,
    missing_commands, owned_floor_bytes, prefix_volume, reason_of, road_limit_s, road_of, sweep_caches, used_kb_cmd, DfColumn, FreeDisk,
    ALREADY_ON_MACHINE, GUARD_SLACK_S, TOOLS_DISK_FLOOR,
};
use wsp_words::words::image::DiskUse;

fn column(v: &Value) -> DfColumn {
    match v.as_str().unwrap() {
        "size" => DfColumn::Size,
        "used" => DfColumn::Used,
        "free" => DfColumn::Free,
        other => panic!("{other}"),
    }
}

fn road(name: &str) -> RoadName {
    serde_json::from_value(json!(name)).unwrap()
}

fn disk_json(d: &DiskUse) -> Value {
    match d {
        DiskUse::Use { used_bytes, size_bytes } => json!({ "kind": "use", "usedBytes": used_bytes, "sizeBytes": size_bytes }),
        DiskUse::Unknown { reason } => json!({ "kind": "unknown", "reason": reason }),
    }
}

fn same_number(got: &Value, want: &Value) -> bool {
    match (got.as_f64(), want.as_f64()) {
        (Some(a), Some(b)) => a == b,
        _ => got == want,
    }
}

fn same_json(got: &Value, want: &Value) -> bool {
    match (got, want) {
        (Value::Object(a), Value::Object(b)) => a.len() == b.len() && a.iter().all(|(k, v)| b.get(k).is_some_and(|w| same_json(v, w))),
        (Value::Number(_), Value::Number(_)) => same_number(got, want),
        _ => got == want,
    }
}

#[test]
fn the_df_lines_and_the_constants_are_the_ts_byte_for_byte() {
    let f = fixture("tools_disk");
    assert_eq!(free_kb_cmd(), s(&f["FREE_KB_CMD"]));
    assert_eq!(used_kb_cmd(), s(&f["USED_KB_CMD"]));
    assert_eq!(free_kb_cmd(), "df -Pk /root | awk 'NR==2{print $4}'");
    assert_eq!(TOOLS_DISK_FLOOR, f["TOOLS_DISK_FLOOR"].as_u64().unwrap());
    assert_eq!(GUARD_SLACK_S, f["GUARD_SLACK_S"].as_u64().unwrap());
    assert_eq!(ALREADY_ON_MACHINE, s(&f["ALREADY_ON_MACHINE"]));
}

#[test]
fn df_kb_cmd_writes_the_columns_in_the_order_asked_and_the_path_unquoted() {
    for c in cases(&fixture("tools_disk"), "dfKbCmd") {
        let columns: Vec<DfColumn> = c["columns"].as_array().unwrap().iter().map(column).collect();
        assert_eq!(df_kb_cmd(&columns, c["path"].as_str()), s(&c["out"]), "{c}");
    }
}

#[test]
fn prefix_volume_quotes_the_prefix_and_walks_up_to_a_folder_that_exists() {
    for c in cases(&fixture("tools_disk"), "prefixVolume") {
        assert_eq!(prefix_volume(&s(&c["prefix"])), s(&c["out"]), "{c}");
    }
}

#[test]
fn guarded_is_the_ts_script_byte_for_byte_with_a_quote_and_a_zero_timeout() {
    for c in cases(&fixture("tools_disk"), "guarded") {
        assert_eq!(guarded(&s(&c["script"]), c["timeoutS"].as_u64().unwrap()), s(&c["out"]), "{}", c["script"]);
    }
}

#[test]
fn every_road_has_the_ts_limit_deadline_and_guarded_script() {
    let f = fixture("tools_disk");
    let roads = cases(&f, "roads");
    assert_eq!(roads.len(), 12);
    for c in roads {
        let r = road(&s(&c["road"]));
        assert_eq!(road_limit_s(r), c["limitS"].as_u64().unwrap(), "{}", c["road"]);
        assert_eq!(guard_deadline_ms(road_limit_s(r)), c["deadlineMs"].as_u64().unwrap(), "{}", c["road"]);
        assert_eq!(guarded_road(r, "install it"), s(&c["guarded"]), "{}", c["road"]);
    }
    for c in cases(&f, "guardDeadlineMs") {
        assert_eq!(guard_deadline_ms(c["s"].as_u64().unwrap()), c["out"].as_u64().unwrap());
    }
}

#[test]
fn reason_of_names_a_failure_as_the_ts_does_for_every_result_and_timeout() {
    let f = fixture("tools_disk");
    let all = cases(&f, "reasonOf");
    assert_eq!(all.len(), 24);
    for c in all {
        assert_eq!(reason_of(&exec_result(&c["res"]), c["timeoutS"].as_f64().unwrap()), s(&c["out"]), "{c}");
    }
}

#[test]
fn road_of_reads_the_last_road_line_and_only_the_fields_that_match() {
    for c in cases(&fixture("tools_disk"), "roadOf") {
        let got = road_of(&s(&c["stdout"])).map(|r| serde_json::to_value(r).unwrap()).unwrap_or(Value::Null);
        assert_eq!(got, c["out"], "{}", c["stdout"]);
    }
}

#[test]
fn closing_joins_the_parts_and_leaves_out_the_empty_ones() {
    for c in cases(&fixture("tools_disk"), "closing") {
        let parts: Vec<Option<String>> = c["parts"].as_array().unwrap().iter().map(|p| p.as_str().map(str::to_string)).collect();
        let parts: Vec<Option<&str>> = parts.iter().map(|p| p.as_deref()).collect();
        assert_eq!(closing(&parts), s(&c["out"]), "{c}");
    }
}

#[test]
fn owned_floor_bytes_keeps_the_larger_of_the_floor_and_a_tenth_of_the_disk_floored() {
    for c in cases(&fixture("tools_disk"), "ownedFloorBytes") {
        assert_eq!(owned_floor_bytes(c["size"].as_u64()), c["out"].as_u64().unwrap(), "{c}");
    }
}

#[tokio::test]
async fn disk_use_reads_df_as_the_ts_and_never_fails_even_when_the_exec_rejects() {
    for c in cases(&fixture("tools_disk"), "diskUse") {
        let m = Scripted::new(&c["rules"], false);
        let got = disk_json(&disk_use(&m, c["prefix"].as_str()).await);
        assert!(same_json(&got, &c["out"]["ok"]), "{}: {got} against {}", c["name"], c["out"]["ok"]);
        assert_calls(&s(&c["name"]), &m.calls(), &c["calls"]);
    }
}

#[tokio::test]
async fn disk_use_size_is_the_size_of_a_reading_and_none_for_an_unknown_one() {
    for c in cases(&fixture("tools_disk"), "diskUse") {
        let m = Scripted::new(&c["rules"], false);
        let want = (c["out"]["ok"]["kind"] == "use").then(|| c["out"]["ok"]["sizeBytes"].as_u64().unwrap());
        assert_eq!(disk_use_size(&m, c["prefix"].as_str()).await, want, "{}", c["name"]);
    }
}

#[tokio::test]
async fn free_bytes_and_free_note_read_df_as_the_ts_and_a_rejected_exec_is_an_error() {
    for c in cases(&fixture("tools_disk"), "freeBytes") {
        let m = Scripted::new(&c["rules"], false);
        let got = free_bytes(&m, c["prefix"].as_str()).await;
        match (&got, c["out"].get("error")) {
            (Err(e), Some(words)) => assert_eq!(e.to_string(), s(words), "{}", c["name"]),
            (Ok(FreeDisk::Free { bytes }), None) => {
                assert!(same_json(&json!({ "kind": "free", "bytes": bytes }), &c["out"]["ok"]), "{}: {bytes}", c["name"])
            }
            (Ok(FreeDisk::Unknown { reason }), None) => {
                assert_eq!(json!({ "kind": "unknown", "reason": reason }), c["out"]["ok"], "{}", c["name"])
            }
            _ => panic!("{}: {got:?} against {}", c["name"], c["out"]),
        }
        assert_calls(&s(&c["name"]), &m.calls(), &c["calls"]);
        let note = free_note(&Scripted::new(&c["rules"], false), c["prefix"].as_str()).await;
        match (note, c["note"].get("error")) {
            (Err(e), Some(words)) => assert_eq!(e.to_string(), s(words), "{}", c["name"]),
            (Ok(n), None) => assert_eq!(n.as_deref(), c["note"]["ok"].as_str(), "{}", c["name"]),
            (n, _) => panic!("{}: {n:?} against {}", c["name"], c["note"]),
        }
    }
}

#[tokio::test]
async fn missing_commands_sends_one_line_on_the_path_and_reads_missing_off_stdout_even_when_it_failed() {
    for c in cases(&fixture("tools_disk"), "missingCommands") {
        let m = Scripted::new(&c["rules"], false);
        let bins: Vec<String> = c["bins"].as_array().unwrap().iter().map(s).collect();
        let path = c["path"].as_str().unwrap_or(wsp_engine::golden_seam::TOOLS_PATH);
        let got = missing_commands(&m, &bins, path, c["prefix"].as_str()).await.unwrap();
        assert_eq!(got.missing.iter().cloned().collect::<Vec<_>>(), golden_common::strings(&c["out"]["missing"]), "{c}");
        assert_eq!(got.failed.as_deref(), c["out"]["failed"].as_str(), "{c}");
        assert_calls("missingCommands", &m.calls(), &c["calls"]);
    }
}

#[tokio::test]
async fn sweep_caches_runs_the_sweep_under_the_guard_and_says_what_came_back() {
    for c in cases(&fixture("tools_disk"), "sweepCaches") {
        let m = Scripted::new(&c["rules"], false);
        let path = c["path"].as_str().unwrap_or(wsp_engine::golden_seam::TOOLS_PATH);
        assert_eq!(sweep_caches(&m, path).await.unwrap(), s(&c["out"]), "{}", c["name"]);
        assert_calls(&s(&c["name"]), &m.calls(), &c["calls"]);
    }
}

// The TS keeps the lone half of a pair its 160-unit cut splits; a Rust String cannot, so the half is U+FFFD (the
// coordinator's ruling on #2446 for every UTF-16 cut). No fixture holds a lone surrogate, so this value is written by hand.
#[test]
fn a_cut_inside_an_emoji_writes_the_replacement_character_for_the_half() {
    let stderr = format!("a{}", "😀".repeat(100));
    assert_eq!(reason_of(&golden_common::res(1, "", &stderr), 30.0), format!("a{}\u{FFFD}", "😀".repeat(79)));
    let stdout = format!("{}\n", "😀".repeat(100));
    assert_eq!(reason_of(&golden_common::res(1, &stdout, ""), 30.0), "😀".repeat(80), "a cut on a pair boundary keeps whole pairs");
}
