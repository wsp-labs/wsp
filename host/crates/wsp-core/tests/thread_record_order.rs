// SPDX-License-Identifier: AGPL-3.0-only
//! A thread's record writes its keys in the order the TS inserts them (a JS object keeps insertion order), not in the
//! order the struct declares them. The expected bytes are node's, made by fixtures/thread_record_order/gen.mjs over
//! the TS expressions of every road that writes the record (boot.ts:932, sessions.ts:831, transcripts.ts:607 and the
//! rest, named in that file).
use serde_json::{Map, Value};
use wsp_core::types::internal::ThreadRecord;

const CASES: &str = include_str!("fixtures/thread_record_order/cases.json");

fn cases() -> Vec<Value> {
    serde_json::from_str(CASES).expect("the cases are JSON")
}

fn object(text: &str) -> Map<String, Value> {
    serde_json::from_str(text).expect("an object")
}

fn bytes(record: &ThreadRecord) -> String {
    serde_json::to_string(record).expect("serializes")
}

#[test]
fn a_record_read_from_json_writes_the_struct_order() {
    let text = r#"{"workspaceId":"w1","harness":"claude","readAt":1,"worked":true}"#;
    let record: ThreadRecord = serde_json::from_str(text).expect("reads");
    assert_eq!(bytes(&record), text);
}

#[test]
fn every_key_the_struct_holds_survives_a_round_trip_in_struct_order() {
    let text = concat!(
        r#"{"workspaceId":"w1","harness":"claude","permissionMode":"plan","parentThreadId":"t0","rootThreadId":"t00","#,
        r#""readAt":1,"settledAt":2,"settleNamedAt":3,"pinnedAt":4,"order":2.5,"foldedAt":5,"replaces":"t-old","#,
        r#""snoozedUntil":6,"section":{"name":"working","whileState":"x"},"cutOwed":"a1","#,
        r#""forkedFrom":{"threadId":"t9","turnId":"u9","title":"Old"},"limitResume":{"at":7,"turnId":"u1"},"#,
        r#""rewound":{"before":"c1","at":8},"filesIn":["/a","/b"],"worked":false,"ended":"failed","clouds":["solari"]}"#
    );
    let record: ThreadRecord = serde_json::from_str(text).expect("reads");
    assert_eq!(bytes(&record), text);
    assert_eq!(record.to_map().len(), 22);
}

#[test]
fn a_record_made_from_a_map_writes_the_maps_order() {
    let text = r#"{"workspaceId":"w1","harness":"claude","worked":true,"readAt":1700,"permissionMode":"plan"}"#;
    let record = ThreadRecord::from_map(object(text)).expect("reads");
    assert_eq!(bytes(&record), text);
    assert_eq!(
        record.to_map().keys().map(String::as_str).collect::<Vec<_>>(),
        ["workspaceId", "harness", "worked", "readAt", "permissionMode"]
    );
}

#[test]
fn every_record_node_writes_on_a_road_can_be_held_and_written_back_byte_for_byte() {
    let all = cases();
    assert!(all.len() >= 50, "the cases were made");
    for case in all {
        let after = case["after"].as_str().expect("the bytes");
        let held = ThreadRecord::from_map(object(after)).expect("reads");
        assert_eq!(bytes(&held), after, "{} / {}", case["road"], case["name"]);
    }
}

#[test]
fn a_key_set_after_the_order_is_pinned_is_written_after_the_keys_held() {
    let mut record: ThreadRecord = serde_json::from_str(r#"{"workspaceId":"w1","harness":"claude","worked":true}"#).expect("reads");
    record.pin_order();
    record.read_at = Some(wsp_wire::types::js_number::JsNumber(1700.0));
    assert_eq!(bytes(&record), r#"{"workspaceId":"w1","harness":"claude","worked":true,"readAt":1700}"#);
}

#[test]
fn two_keys_set_one_after_the_other_keep_the_order_they_were_set_in() {
    let mut record: ThreadRecord = serde_json::from_str(r#"{"workspaceId":"w1","harness":"claude"}"#).expect("reads");
    record.pin_order();
    record.worked = Some(true);
    record.pin_order();
    record.read_at = Some(wsp_wire::types::js_number::JsNumber(5.0));
    record.pin_order();
    record.permission_mode = Some("plan".into());
    assert_eq!(bytes(&record), r#"{"workspaceId":"w1","harness":"claude","worked":true,"readAt":5,"permissionMode":"plan"}"#);
}

#[test]
fn a_key_taken_off_and_set_again_is_written_last() {
    let mut record: ThreadRecord =
        ThreadRecord::from_map(object(r#"{"workspaceId":"w1","harness":"claude","readAt":1,"settledAt":2,"worked":true}"#)).expect("reads");
    record.settled_at = None;
    record.pin_order();
    record.settled_at = Some(wsp_wire::types::js_number::JsNumber(3.0));
    assert_eq!(bytes(&record), r#"{"workspaceId":"w1","harness":"claude","readAt":1,"worked":true,"settledAt":3}"#);
}

#[test]
fn a_held_key_set_again_keeps_its_place() {
    let mut record: ThreadRecord =
        ThreadRecord::from_map(object(r#"{"workspaceId":"w1","harness":"claude","readAt":1,"settledAt":2,"worked":true}"#)).expect("reads");
    record.pin_order();
    record.settled_at = Some(wsp_wire::types::js_number::JsNumber(9.0));
    assert_eq!(bytes(&record), r#"{"workspaceId":"w1","harness":"claude","readAt":1,"settledAt":9,"worked":true}"#);
}

#[test]
fn two_records_with_the_same_values_are_equal_whatever_order_they_were_made_in() {
    let a = ThreadRecord::from_map(object(r#"{"workspaceId":"w1","harness":"claude","readAt":1,"worked":true}"#)).expect("reads");
    let b = ThreadRecord::from_map(object(r#"{"workspaceId":"w1","harness":"claude","worked":true,"readAt":1}"#)).expect("reads");
    assert_eq!(a, b);
    assert_ne!(bytes(&a), bytes(&b));
}

#[test]
fn a_clone_keeps_the_order_and_the_workspace_leaves_the_index_copy_unwritten() {
    let record = ThreadRecord::from_map(object(r#"{"workspaceId":"w1","harness":"claude","worked":true,"readAt":1}"#)).expect("reads");
    let mut held = record.clone();
    assert_eq!(bytes(&held), bytes(&record));
    held.workspace_id = String::new();
    assert_eq!(bytes(&held), r#"{"harness":"claude","worked":true,"readAt":1}"#);
}
