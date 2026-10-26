// SPDX-License-Identifier: AGPL-3.0-only
//! A thread's record in the runtime's map is `ThreadRecord & { workspaceId }` (context.ts:213), and what it writes is
//! the record's own bytes: `workspaceId` once, first, then the keys in the order the TS spreads them. The expected
//! strings are node 22's `JSON.stringify` over `{ ...base, ...stamps }` (boot.ts:933) with the record literals of
//! boot.ts:403, run for this file.
mod common;

#[path = "support/boot.rs"]
mod boot_support;
#[path = "support/machines.rs"]
mod machines_support;

use std::cell::RefCell;
use std::rc::Rc;

use indexmap::IndexMap;
use serde_json::{json, Map, Value};

use boot_support::{Opts as BootOpts, World as BootWorld};
use machines_support::{EntryOpts, Opts as MachinesOpts, World as MachinesWorld};
use wsp_core::types::internal::ThreadRecord;
use wsp_machines::machines::boot::rows::Stamped;
use wsp_machines::machines::machines::ThreadOn;
use wsp_wire::types::js_number::JsNumber;

const RECORD: &str = r#"{"workspaceId":"ws_1","harness":"claude","permissionMode":"plan","rootThreadId":"t0","readAt":1700000000000}"#;
const MARKED: &str = r#"{"workspaceId":"ws_1","harness":"claude","permissionMode":"plan","rootThreadId":"t0","readAt":1700000000000,"pinnedAt":1700000005000,"order":2}"#;
const FRESH: &str = r#"{"workspaceId":"ws_2","harness":"codex","settledAt":1700000009000}"#;

fn bytes(records: &Rc<RefCell<IndexMap<String, ThreadRecord>>>, thread: &str) -> String {
    serde_json::to_string(records.borrow().get(thread).expect("the record is held"))
        .expect("serializes")
}

fn stamps(v: Value) -> Map<String, Value> {
    v.as_object().expect("stamps").clone()
}

fn record(ws: &str, harness: &str) -> ThreadRecord {
    ThreadRecord {
        workspace_id: ws.into(),
        harness: harness.into(),
        ..Default::default()
    }
}

#[test]
fn a_record_read_back_from_its_json_writes_workspace_id_once() {
    common::run_local(async {
        let w = BootWorld::rows(BootOpts {
            live: json!({"ws_1": {}}),
            ..BootOpts::default()
        });
        w.started().await;
        w.rt.add_record("t1", serde_json::from_str(RECORD).expect("the literal"));
        assert_eq!(bytes(&w.rt.thread_records, "t1"), RECORD);
    });
}

#[test]
fn the_boot_road_marks_a_record_and_it_writes_the_bytes_the_ts_writes() {
    common::run_local(async {
        let w = BootWorld::rows(BootOpts {
            live: json!({"ws_1": {}}),
            ..BootOpts::default()
        });
        w.started().await;
        w.rt.add_record("t1", serde_json::from_str(RECORD).expect("the literal"));
        w.area
            .mark(
                &["t1".into()],
                Stamped::Same(stamps(
                    json!({"pinnedAt": 1_700_000_005_000_i64, "order": 2}),
                )),
                None,
            )
            .await
            .expect("mark");
        assert_eq!(bytes(&w.rt.thread_records, "t1"), MARKED);
    });
}

#[test]
fn a_thread_the_boot_road_marks_with_no_record_starts_from_workspace_id_and_harness() {
    common::run_local(async {
        let w = BootWorld::rows(BootOpts {
            live: json!({"ws_2": {}}),
            rows: json!([{"view": {"id": "r2", "workspaceId": "ws_2", "threadId": "t2", "harness": "codex"}}]),
            ..BootOpts::default()
        });
        w.started().await;
        w.area
            .mark(
                &["t2".into()],
                Stamped::Same(stamps(json!({"settledAt": 1_700_000_009_000_i64}))),
                None,
            )
            .await
            .expect("mark");
        assert_eq!(bytes(&w.rt.thread_records, "t2"), FRESH);
    });
}

#[test]
fn the_machines_road_drops_a_workspace_and_the_record_left_keeps_its_bytes() {
    common::run_local(async {
        let fx = common::fixture("machines_phases");
        let scene = common::scene(
            &fx,
            "drop: everything the workspace left goes, in order, and the event last",
        );
        let w = MachinesWorld::new(MachinesOpts {
            store: machines_support::store_of(scene),
            ..Default::default()
        });
        w.entry_in(EntryOpts {
            sub: true,
            ..Default::default()
        });
        let mut kept = record("ws_9", "codex");
        kept.settled_at = Some(JsNumber(1_700_000_009_000.0));
        w.rt.thread_records
            .borrow_mut()
            .insert("t1".into(), record("ws_1", "claude"));
        w.rt.thread_records.borrow_mut().insert("t9".into(), kept);

        w.area.drop("ws_1").await.expect("the drop lands");
        common::flush().await;

        assert_eq!(
            w.rt.thread_records
                .borrow()
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["t9".to_string()]
        );
        assert_eq!(
            bytes(&w.rt.thread_records, "t9"),
            r#"{"workspaceId":"ws_9","harness":"codex","settledAt":1700000009000}"#
        );
    });
}

#[test]
fn the_view_a_record_reads_as_names_its_workspace_and_is_never_written() {
    let r = record("ws_1", "claude");
    let on = ThreadOn::of(&r);
    assert_eq!(on.workspace_id, "ws_1");
    assert_eq!(on.record.harness, "claude");
}
