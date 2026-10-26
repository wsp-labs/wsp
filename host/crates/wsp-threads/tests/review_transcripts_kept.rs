// SPDX-License-Identifier: AGPL-3.0-only
//! Review of wsp-threads (#2448): the session index record's thread records keep their key order. transcripts.ts:649
//! writes each record as `const { workspaceId: on, ...held }`, the record's own keys in order less `workspaceId`
//! (ruling 1 of 6402177374: key order on disk is the TS's). transcripts/kept.rs:404 takes `workspaceId` off a
//! `serde_json::Map` built with `preserve_order`, where `remove` is `swap_remove`: the record's last key moves first.
#[path = "support_b/mod.rs"]
mod support;

use serde_json::{json, Value};
use support::transcripts_ctx::{FakeTranscriptsCtx, WithState};
use support::{local, record, records_of};

const WS: &str = "ws1";

fn keys_of(v: &Value) -> Vec<String> {
    v.as_object().expect("an object").keys().cloned().collect()
}

#[test]
fn persist_sessions_keeps_each_thread_records_own_key_order() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        ctx.add_live(json!({ "id": WS }));
        let held = record(WS, json!({ "permissionMode": "default", "parentThreadId": "t0", "rootThreadId": "t0", "worked": true }));
        let mut own = keys_of(&serde_json::to_value(&held).expect("a record serializes"));
        own.retain(|k| k != "workspaceId");
        ctx.thread_records.with(|t| *t = records_of(vec![("t1", held.clone())]).take());
        ctx.area().persist_sessions(WS).await;
        let doc = ctx.store.get_now("sessions", WS).expect("the document is put");
        assert_eq!(keys_of(&doc["threads"]["t1"]), own, "the record's keys in its own order, workspaceId left out");
    });
}
