// SPDX-License-Identifier: AGPL-3.0-only
//! A thread's record keeps the order the TS inserts its keys in (boot.ts:932-937 spreads the stamps over the record), so
//! a stamp for a key the struct declares before a key already held writes the bytes node writes. The expected strings
//! are node's `JSON.stringify` over the TS expression, made by `wsp-core/tests/fixtures/thread_record_order/gen.mjs`.
mod common;

#[path = "support/boot.rs"]
mod boot_support;

use serde_json::{json, Map, Value};

use boot_support::{Opts as BootOpts, World as BootWorld};
use wsp_core::types::internal::ThreadRecord;
use wsp_machines::machines::boot::rows::Stamped;

const CASES: &str = include_str!("../../wsp-core/tests/fixtures/thread_record_order/cases.json");

fn cases(road: &str) -> Vec<Value> {
    let all: Vec<Value> = serde_json::from_str(CASES).expect("the cases are JSON");
    all.into_iter().filter(|c| c["road"] == road).collect()
}

fn object(v: &Value) -> Map<String, Value> {
    v.as_object().expect("an object").clone()
}

#[test]
fn every_stamp_the_boot_road_marks_writes_the_bytes_node_writes() {
    let marks = cases("mark");
    assert!(marks.len() >= 20, "the cases were made");
    for case in marks {
        let name = case["name"].as_str().unwrap_or_default().to_string();
        common::run_local(async move {
            let w = BootWorld::rows(BootOpts { live: json!({"w1": {}}), ..BootOpts::default() });
            w.started().await;
            w.rt.add_record("t1", ThreadRecord::from_map(object(&case["before"])).expect("the record literal"));
            w.area
                .mark(&["t1".into()], Stamped::Same(object(&case["input"])), None)
                .await
                .expect("mark");
            let held = w.rt.thread_records.borrow().get("t1").cloned().expect("the record is held");
            let wrote = serde_json::to_string(&held).expect("serializes");
            assert_eq!(wrote, case["after"].as_str().expect("the bytes"), "{name}");
        });
    }
}

#[test]
fn a_thread_with_no_record_marked_twice_keeps_the_order_the_stamps_came_in() {
    common::run_local(async {
        let w = BootWorld::rows(BootOpts {
            live: json!({"w1": {}}),
            rows: json!([{"view": {"id": "r1", "workspaceId": "w1", "threadId": "t1", "harness": "claude"}}]),
            ..BootOpts::default()
        });
        w.started().await;
        for stamp in [json!({"settledAt": 4}), json!({"snoozedUntil": 9, "readAt": 5}), json!({"order": 1})] {
            w.area.mark(&["t1".into()], Stamped::Same(object(&stamp)), None).await.expect("mark");
        }
        let held = w.rt.thread_records.borrow().get("t1").cloned().expect("held");
        assert_eq!(
            serde_json::to_string(&held).expect("serializes"),
            r#"{"workspaceId":"w1","harness":"claude","settledAt":4,"snoozedUntil":9,"readAt":5,"order":1}"#
        );
    });
}
