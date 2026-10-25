// SPDX-License-Identifier: AGPL-3.0-only
//! transcripts_kept (spec/transcripts_kept.md): sent images, the sessions document, daemon tokens and the reads other
//! areas make. Expected values read off transcripts.ts at 65f6b89c7 and the TS tests named above each test.
#[path = "support_b/mod.rs"]
mod support;

use std::rc::Rc;
use std::sync::Arc;

use futures::join;
use futures::FutureExt;
use indexmap::IndexMap;
use serde_json::{json, Value};
use support::transcripts_ctx::{FakeTranscriptsCtx, WithState};
use support::{err, event, json_of, local, record, records_of, row, sessions_of, settle, view, FakeMachine};
use wsp_core::console;
use wsp_core::types::internal::{Steered, TurnLive};
use wsp_engine::machine::Machine;
use wsp_threads::threads::threads::Row;
use wsp_threads::threads::transcripts::kept::{image_keys_of, ThreadsNamed, DAEMON_TOKEN_MISS_TTL_MS, SESSION_INDEX_CAP};
use wsp_threads::threads::transcripts::TranscriptsArea;
use wsp_wire::types::attachments::Attachment;
use wsp_wire::types::harness_picks::RanPicks;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::workspace_more::WorkspaceOrigin;

const WS: &str = "ws1";

fn att(media: &str, bytes: &str) -> Attachment {
    serde_json::from_value(json!({ "mediaType": media, "bytes": bytes })).expect("an attachment")
}

fn png(bytes: &str) -> Attachment {
    att("image/png", bytes)
}

fn pdf() -> Attachment {
    att("application/pdf", "cGRm")
}

fn entry_of(key: &str, media: &str) -> Value {
    json!({ "key": key, "mediaType": media })
}

fn names(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|s| s.to_string()).collect()
}

fn keys_of(v: &Value) -> Vec<String> {
    v.as_object().expect("an object").keys().cloned().collect()
}

fn sess(id: &str, ws: &str, thread: Option<&str>, status: &str, started_at: Option<i64>) -> Row {
    let mut v = json!({ "id": id, "workspaceId": ws, "status": status });
    if let Some(t) = thread {
        v["threadId"] = json!(t);
    }
    if let Some(s) = started_at {
        v["startedAt"] = json!(s);
    }
    row(view(v), &format!("turn-{id}"))
}

fn live_ctx() -> Rc<FakeTranscriptsCtx> {
    let ctx = FakeTranscriptsCtx::new();
    ctx.add_live(json!({ "id": WS }));
    ctx
}

fn session_ids(ctx: &FakeTranscriptsCtx) -> Vec<String> {
    ctx.sessions.with(|s| s.keys().cloned().collect())
}

fn ended_of(ctx: &FakeTranscriptsCtx, thread: &str) -> Value {
    ctx.thread_records.with(|t| json_of(&t.get(thread).expect("a record"))["ended"].clone())
}

// ---- kept images -------------------------------------------------------------------------------------------------

// TS: session-images.test.ts:195
// TS: steer-images.test.ts:67
#[test]
fn keep_sent_images_writes_the_entry_before_the_bytes() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        let area = ctx.area();
        area.keep_sent_images(WS, "t1", Some("req_a"), &[png("BwcHBw=="), pdf(), png("AwMD")]).await;
        assert_eq!(
            ctx.store.logged(),
            vec!["put attachment-keys/ws1", "putBlob attachments/t1.req_a.0", "putBlob attachments/t1.req_a.2"],
            "the document first, then each blob in order"
        );
        assert_eq!(
            ctx.store.get_now("attachment-keys", WS),
            Some(json!({ "threads": { "t1": [entry_of("t1.req_a.0", "image/png"), entry_of("t1.req_a.2", "image/png")] } })),
            "a file that is not an image keeps nothing, and the key counts its place in the whole list"
        );
        assert_eq!(ctx.store.blob_now("attachments", "t1.req_a.0"), Some(vec![7, 7, 7, 7]));
        assert_eq!(ctx.store.blob_now("attachments", "t1.req_a.2"), Some(vec![3, 3, 3]));
        assert!(ctx.store.blob_now("attachments", "t1.req_a.1").is_none());

        ctx.store.log.borrow_mut().clear();
        ctx.store.seed(
            "attachment-keys",
            WS,
            json!({ "threads": { "t1": [entry_of("t1.req_old.0", "image/png"), entry_of("t1.req_a.0", "image/gif")], "t2": [entry_of("t2.req_x.0", "image/png")] } }),
        );
        area.keep_sent_images(WS, "t1", Some("req_a"), &[png("BQU=")]).await;
        assert_eq!(
            ctx.store.get_now("attachment-keys", WS),
            Some(json!({ "threads": { "t1": [entry_of("t1.req_old.0", "image/png"), entry_of("t1.req_a.0", "image/png")], "t2": [entry_of("t2.req_x.0", "image/png")] } })),
            "the same send again replaces its own entries, the thread's others and every other thread stay"
        );
        assert_eq!(ctx.store.logged(), vec!["put attachment-keys/ws1", "putBlob attachments/t1.req_a.0"]);
        assert_eq!(ctx.store.blob_now("attachments", "t1.req_a.0"), Some(vec![5, 5]));
    });
}

// TS: session-images.test.ts:187
#[test]
fn keep_sent_images_keeps_none_without_a_plain_request() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        let area = ctx.area();
        let image = [png("BwcHBw==")];
        area.keep_sent_images(WS, "t1", None, &image).await;
        for request in ["", "bad id", "../x", ".dot", "a/b"] {
            area.keep_sent_images(WS, "t1", Some(request), &image).await;
        }
        area.keep_sent_images(WS, "bad thread", Some("req_a"), &image).await;
        area.keep_sent_images(WS, "", Some("req_a"), &image).await;
        area.keep_sent_images(WS, "t1", Some("req_a"), &[pdf(), att("image/svg+xml", "PHN2Zy8+")]).await;
        area.keep_sent_images(WS, "t1", Some("req_a"), &[]).await;
        assert!(ctx.store.logged().is_empty(), "nothing is written for a send no client could name");
        assert!(ctx.store.get_now("attachment-keys", WS).is_none());
        assert!(area.kept_writes.borrow().is_empty(), "and none of them took a turn on the chain");

        for media in ["image/jpeg", "image/gif", "image/webp"] {
            area.keep_sent_images(WS, "t1", Some("req_m"), &[att(media, "AwMD")]).await;
        }
        assert_eq!(ctx.store.logged().iter().filter(|l| l.starts_with("putBlob")).count(), 3, "the four image types are kept");
    });
}

#[test]
fn two_sends_at_once_keep_both_threads_entries() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        let area = ctx.area();
        let first = area.keep_sent_images(WS, "t1", Some("req_a"), &[png("BwcHBw==")]);
        let second = area.keep_sent_images(WS, "t2", Some("req_b"), &[png("AwMD")]);
        second.await;
        first.await;
        assert_eq!(
            ctx.store.get_now("attachment-keys", WS),
            Some(json!({ "threads": { "t1": [entry_of("t1.req_a.0", "image/png")], "t2": [entry_of("t2.req_b.0", "image/png")] } })),
            "two sends writing at once cannot lose each other's keys"
        );
        assert_eq!(
            ctx.store.logged(),
            vec!["put attachment-keys/ws1", "putBlob attachments/t1.req_a.0", "put attachment-keys/ws1", "putBlob attachments/t2.req_b.0"],
            "each takes its turn after the one before"
        );
        assert!(area.kept_writes.borrow().contains_key(WS));
    });
}

// TS: session-images.test.ts:195
#[test]
fn a_failed_image_write_warns_and_the_chain_goes_on() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        let area = ctx.area();
        let warned = console::capture();
        ctx.flaky.fail_blobs.set(true);
        area.keep_sent_images(WS, "t1", Some("req_a"), &[png("BwcHBw==")]).await;
        assert_eq!(warned.lines(), vec!["the images ws1's messages carried were not kept as asked: the host went down"]);
        assert_eq!(
            ctx.store.get_now("attachment-keys", WS),
            Some(json!({ "threads": { "t1": [entry_of("t1.req_a.0", "image/png")] } })),
            "the entry was named before its bytes, so a write cut short leaves an entry whose blob is missing"
        );
        assert!(ctx.store.blob_now("attachments", "t1.req_a.0").is_none());

        ctx.flaky.fail_blobs.set(false);
        area.keep_sent_images(WS, "t2", Some("req_b"), &[png("AwMD")]).await;
        assert_eq!(warned.lines().len(), 1, "the next send warns of nothing");
        assert_eq!(
            ctx.store.get_now("attachment-keys", WS),
            Some(json!({ "threads": { "t1": [entry_of("t1.req_a.0", "image/png")], "t2": [entry_of("t2.req_b.0", "image/png")] } })),
            "the chain goes on"
        );
        assert_eq!(ctx.store.blob_now("attachments", "t2.req_b.0"), Some(vec![3, 3, 3]));

        ctx.store.fail_put.set(true);
        area.keep_sent_images(WS, "t3", Some("req_c"), &[png("AwMD")]).await;
        assert_eq!(warned.lines()[1], "the images ws1's messages carried were not kept as asked: the disk is full");
        ctx.store.fail_put.set(false);
        area.keep_sent_images(WS, "t3", Some("req_c"), &[png("AwMD")]).await;
        assert!(ctx.store.get_now("attachment-keys", WS).expect("a document")["threads"].get("t3").is_some());
    });
}

fn held_two_threads(ctx: &FakeTranscriptsCtx) {
    ctx.store.seed(
        "attachment-keys",
        WS,
        json!({ "threads": { "t1": [entry_of("t1.req_a.0", "image/png"), entry_of("t1.req_a.1", "image/png")], "t2": [entry_of("t2.req_b.0", "image/png")] } }),
    );
    for key in ["t1.req_a.0", "t1.req_a.1", "t2.req_b.0"] {
        ctx.store.seed_blob("attachments", key, &[1]);
    }
}

// TS: session-images.test.ts:211
// TS: steer-images.test.ts:85
#[test]
fn drop_kept_keys_deletes_blobs_and_empties_the_document() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        held_two_threads(&ctx);
        let area = ctx.area();
        area.drop_kept_keys(WS, &names(&["t2.req_b.0", "t1.req_a.0", "ghost.req.0"])).await;
        assert_eq!(
            ctx.store.logged(),
            vec!["deleteBlob attachments/t2.req_b.0", "deleteBlob attachments/t1.req_a.0", "deleteBlob attachments/ghost.req.0", "put attachment-keys/ws1"],
            "every named key's blob goes, in the order given, then the document"
        );
        assert_eq!(ctx.store.get_now("attachment-keys", WS), Some(json!({ "threads": { "t1": [entry_of("t1.req_a.1", "image/png")] } })), "a thread left empty leaves the document");
        assert!(ctx.store.blob_now("attachments", "t1.req_a.1").is_some());

        ctx.store.log.borrow_mut().clear();
        area.drop_kept_keys(WS, &names(&["t1.req_a.1"])).await;
        assert_eq!(ctx.store.logged(), vec!["deleteBlob attachments/t1.req_a.1", "delete attachment-keys/ws1"], "a document left with no thread is deleted");
        assert!(ctx.store.get_now("attachment-keys", WS).is_none());
    });
}

#[test]
fn drop_kept_keys_naming_nothing_kept_writes_nothing() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        let area = ctx.area();
        area.drop_kept_keys(WS, &names(&["t1.req_a.0"])).await;
        assert!(ctx.store.logged().is_empty(), "no document, nothing to write");
        held_two_threads(&ctx);
        let held = ctx.store.get_now("attachment-keys", WS);
        area.drop_kept_keys(WS, &names(&["zzz.req.0", "t3.req_c.0"])).await;
        area.drop_kept_keys(WS, &[]).await;
        assert!(ctx.store.logged().is_empty(), "no thread holds these keys, so not even a blob is deleted");
        assert_eq!(ctx.store.get_now("attachment-keys", WS), held);
    });
}

#[test]
fn image_keys_of_reads_starts_and_steers_with_a_thread() {
    let carried = json!([{ "mediaType": "image/png", "bytes": 4 }, { "mediaType": "application/pdf", "bytes": 3 }, { "mediaType": "image/webp", "bytes": 5 }]);
    let start = event(json!({ "type": "session.start", "workspaceId": WS, "sessionId": "s1", "threadId": "t1", "prompt": "look", "requestId": "req_a", "attachments": carried }));
    assert_eq!(image_keys_of(&start), names(&["t1.req_a.0", "t1.req_a.2"]), "the keys of the image attachments, by their place in the message");
    let steer = event(json!({ "type": "session.steer", "workspaceId": WS, "sessionId": "s1", "threadId": "t1", "prompt": "and this", "requestId": "req_s", "attachments": carried }));
    assert_eq!(image_keys_of(&steer), names(&["t1.req_s.0", "t1.req_s.2"]));
    let no_thread = event(json!({ "type": "session.start", "workspaceId": WS, "sessionId": "s1", "prompt": "look", "requestId": "req_a", "attachments": carried }));
    assert!(image_keys_of(&no_thread).is_empty());
    let no_request = event(json!({ "type": "session.start", "workspaceId": WS, "sessionId": "s1", "threadId": "t1", "prompt": "look", "attachments": carried }));
    assert!(image_keys_of(&no_request).is_empty(), "a send with no request id kept none");
    let none = event(json!({ "type": "session.start", "workspaceId": WS, "sessionId": "s1", "threadId": "t1", "prompt": "look", "requestId": "req_a" }));
    assert!(image_keys_of(&none).is_empty());
    let delta = event(json!({ "type": "session.delta", "workspaceId": WS, "sessionId": "s1", "threadId": "t1", "kind": "text", "text": "x", "attachments": carried }));
    assert!(image_keys_of(&delta).is_empty(), "any other event names none");
    let end = event(json!({ "type": "session.end", "workspaceId": WS, "sessionId": "s1", "threadId": "t1", "exitCode": 0, "sawResult": true }));
    assert!(image_keys_of(&end).is_empty());
}

// TS: session-images.test.ts:245
// TS: folder-threads.test.ts:673
#[test]
fn drop_sent_images_takes_named_threads_or_all() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        let area = ctx.area();
        area.drop_sent_images(WS, ThreadsNamed::All).await;
        assert!(ctx.store.logged().is_empty(), "no document, nothing to write");

        ctx.store.seed(
            "attachment-keys",
            WS,
            json!({ "threads": { "t1": [entry_of("t1.req_a.0", "image/png"), entry_of("t1.req_a.1", "image/png")], "t2": [entry_of("t2.req_b.0", "image/png")], "t3": [entry_of("t3.req_c.0", "image/png")] } }),
        );
        area.drop_sent_images(WS, ThreadsNamed::These(names(&["zzz"]))).await;
        area.drop_sent_images(WS, ThreadsNamed::These(vec![])).await;
        assert!(ctx.store.logged().is_empty(), "a thread that holds no entry writes nothing");

        area.drop_sent_images(WS, ThreadsNamed::These(names(&["t1", "zzz"]))).await;
        assert_eq!(ctx.store.logged(), vec!["deleteBlob attachments/t1.req_a.0", "deleteBlob attachments/t1.req_a.1", "put attachment-keys/ws1"]);
        assert_eq!(
            ctx.store.get_now("attachment-keys", WS),
            Some(json!({ "threads": { "t2": [entry_of("t2.req_b.0", "image/png")], "t3": [entry_of("t3.req_c.0", "image/png")] } })),
            "the thread leaves the document and another thread's images stay"
        );

        ctx.store.log.borrow_mut().clear();
        area.drop_sent_images(WS, ThreadsNamed::All).await;
        assert_eq!(ctx.store.logged(), vec!["deleteBlob attachments/t2.req_b.0", "deleteBlob attachments/t3.req_c.0", "delete attachment-keys/ws1"], "every blob of every thread, then the document");
    });
}

// ---- the session index cap ---------------------------------------------------------------------------------------

// TS: runtime-index.test.ts:685
#[test]
fn cap_sessions_drops_the_oldest_finished_rows_past_two_hundred() {
    let ctx = FakeTranscriptsCtx::new();
    let mut rows = vec![sess("stuck", WS, Some("tS"), "running", Some(1))];
    for i in 0..202 {
        let status = match i {
            1 => "failed",
            2 => "interrupted",
            _ => "completed",
        };
        rows.push(sess(&format!("r{i}"), WS, Some("t0"), status, Some(10 + i)));
    }
    for i in 0..30 {
        rows.push(sess(&format!("o{i}"), "ws2", Some(&format!("o{i}")), "completed", Some(i)));
    }
    ctx.sessions.with(|s| *s = sessions_of(rows).take());
    ctx.thread_records.with(|t| *t = records_of(vec![("t0", record(WS, json!({ "readAt": 5 }))), ("tS", record(WS, json!({})))]).take());
    let area = ctx.area();
    area.cap_sessions(WS);
    let mut want = vec!["stuck".to_string()];
    want.extend((3..202).map(|i| format!("r{i}")));
    want.extend((0..30).map(|i| format!("o{i}")));
    assert_eq!(session_ids(&ctx), want, "the oldest three finished rows went, a running row stayed, the other workspace's rows do not count, and the map keeps its order");
    assert_eq!(session_ids(&ctx).iter().filter(|id| !id.starts_with('o')).count(), SESSION_INDEX_CAP);
    assert_eq!(ended_of(&ctx, "t0"), json!("interrupted"), "the thread's record names how its last dropped row ended");
    ctx.thread_records.with(|t| {
        assert_eq!(num_of(&json_of(&t["t0"])["readAt"]), 5.0);
        assert!(json_of(&t["tS"]).get("ended").is_none());
    });

    let boundary = FakeTranscriptsCtx::new();
    let mut rows = vec![sess("stuck", WS, Some("tS"), "running", Some(1))];
    rows.extend((0..199).map(|i| sess(&format!("r{i}"), WS, Some("t0"), "completed", Some(10 + i))));
    boundary.sessions.with(|s| *s = sessions_of(rows).take());
    boundary.area().cap_sessions(WS);
    assert_eq!(session_ids(&boundary).len(), 200, "exactly the cap drops nothing");
}

fn num_of(v: &Value) -> f64 {
    v.as_f64().expect("a number")
}

// TS: runtime-index.test.ts:652
// TS: box-lead.test.ts:446
#[test]
fn cap_sessions_keeps_each_threads_newest_and_running_rows() {
    let ctx = FakeTranscriptsCtx::new();
    let mut rows: Vec<Row> = (0..200).map(|i| sess(&format!("s{i:03}"), WS, Some(&format!("t{i:03}")), "completed", Some(100 + i))).collect();
    rows.push(sess("x1", WS, Some("tX"), "completed", Some(5)));
    rows.push(sess("x2", WS, Some("tX"), "completed", Some(5)));
    rows.push(sess("r1", WS, Some("tR"), "running", Some(1)));
    rows.push(sess("r2", WS, Some("tR"), "completed", Some(2)));
    rows.push(sess("solo", WS, None, "completed", Some(3)));
    ctx.sessions.with(|s| *s = sessions_of(rows).take());
    ctx.area().cap_sessions(WS);
    let ids = session_ids(&ctx);
    assert_eq!(ids.len(), 204, "a row that is its thread's newest or running never goes, so the count can stay over the cap");
    assert!(!ids.contains(&"x1".to_string()), "of two rows started together the earlier in the map goes: the later is the newest");
    for kept in ["x2", "r1", "r2", "solo", "s000", "s199"] {
        assert!(ids.contains(&kept.to_string()), "{kept}");
    }
    let mut want: Vec<String> = (0..200).map(|i| format!("s{i:03}")).collect();
    want.extend(names(&["x2", "r1", "r2", "solo"]));
    assert_eq!(ids, want);
}

// TS: runtime-index.test.ts:685
#[test]
fn a_capped_row_marks_its_thread_ended() {
    let ctx = FakeTranscriptsCtx::new();
    let mut rows: Vec<Row> = (0..200).map(|i| sess(&format!("f{i:03}"), WS, Some(&format!("tf{i:03}")), "completed", Some(100 + i))).collect();
    rows.push(sess("a1", WS, Some("tA"), "failed", Some(1)));
    rows.push(sess("a2", WS, Some("tA"), "completed", Some(2)));
    rows.push(sess("m1", WS, Some("tM"), "interrupted", Some(1)));
    rows.push(sess("m2", WS, Some("tM"), "completed", Some(2)));
    ctx.sessions.with(|s| *s = sessions_of(rows).take());
    ctx.thread_records.with(|t| {
        *t = records_of(vec![("tZ", record(WS, json!({}))), ("tA", record(WS, json!({ "readAt": 5 }))), ("tY", record(WS, json!({})))]).take();
    });
    ctx.area().cap_sessions(WS);
    let ids = session_ids(&ctx);
    assert!(!ids.contains(&"a1".to_string()) && !ids.contains(&"m1".to_string()));
    assert_eq!(ids.len(), 202, "excess four, two rows to give");
    assert_eq!(ended_of(&ctx, "tA"), json!("failed"), "ended is the dropped row's status");
    ctx.thread_records.with(|t| {
        assert_eq!(t.keys().cloned().collect::<Vec<_>>(), vec!["tZ", "tA", "tY"], "the record keeps its place in the map");
        assert_eq!(num_of(&json_of(&t["tA"])["readAt"]), 5.0, "and its other fields");
        assert!(json_of(&t["tZ"]).get("ended").is_none());
        assert!(!t.contains_key("tM"), "a thread with no record gets none made");
    });
}

// ---- persisting the sessions document ----------------------------------------------------------------------------

// TS: runtime-index.test.ts:49
#[test]
fn persist_sessions_writes_the_document_in_the_ts_key_order() {
    local(async {
        let ctx = live_ctx();
        let r1 = sess("r1", WS, Some("t1"), "completed", Some(10));
        r1.borrow_mut().view.claude_session_id = Some("c1".into());
        r1.borrow_mut().notify = Some(names(&["t0"]));
        r1.borrow_mut().snapshot = Some("snap".into());
        let r2 = sess("r2", "ws2", Some("t9"), "completed", None);
        ctx.sessions.with(|s| *s = sessions_of(vec![r1, r2]).take());
        ctx.thread_records.with(|t| *t = records_of(vec![("t9", record("ws2", json!({}))), ("t1", record(WS, json!({ "readAt": 5 })))]).take());
        let area = ctx.area();
        area.viewed_marks.borrow_mut().insert(WS.into(), IndexMap::from([("a.txt".to_string(), "blob1".to_string())]));
        area.persist_sessions(WS).await;
        let doc = ctx.store.get_now("sessions", WS).expect("the document is put");
        assert_eq!(keys_of(&doc), vec!["workspaceId", "sessions", "threads", "viewed"], "the TS key order, which the wsp-core type has the other way round");
        assert_eq!(doc["workspaceId"], json!(WS));
        assert_eq!(doc["viewed"], json!({ "a.txt": "blob1" }));
        assert_eq!(keys_of(&doc["threads"]), vec!["t1"], "the records of this workspace only");
        assert!(doc["threads"]["t1"].get("workspaceId").is_none(), "with workspaceId left out");
        assert_eq!(num_of(&doc["threads"]["t1"]["readAt"]), 5.0);
        assert_eq!(doc["threads"]["t1"]["harness"], json!("claude"));
        let sessions = doc["sessions"].as_array().expect("an array");
        assert_eq!(sessions.len(), 1);
        let keys = keys_of(&sessions[0]);
        let at = keys.iter().position(|k| k == "turnId").expect("a turnId");
        assert_eq!(&keys[at..], &["turnId", "notify", "snapshot"], "the view's own keys first, then the row's");
        assert!(["id", "workspaceId", "harness", "status", "claudeSessionId"].iter().all(|k| keys[..at].iter().any(|x| x == k)));
        assert_eq!(sessions[0]["turnId"], json!("turn-r1"));
        assert_eq!(sessions[0]["notify"], json!(["t0"]));
        assert_eq!(sessions[0]["snapshot"], json!("snap"));

        area.viewed_marks.borrow_mut().insert(WS.into(), IndexMap::new());
        area.persist_sessions(WS).await;
        let doc = ctx.store.get_now("sessions", WS).expect("the document is put");
        assert_eq!(keys_of(&doc), vec!["workspaceId", "sessions", "threads"], "viewed only when the workspace's marks are not empty");
        area.viewed_marks.borrow_mut().shift_remove(WS);
        ctx.thread_records.with(|t| t.clear());
        area.persist_sessions(WS).await;
        let doc = ctx.store.get_now("sessions", WS).expect("the document is put");
        assert_eq!(keys_of(&doc), vec!["workspaceId", "sessions", "threads"]);
        assert_eq!(doc["threads"], json!({}), "threads is always there");
    });
}

fn live_row(running: bool) -> Row {
    let status = if running { "running" } else { "completed" };
    let id = if running { "r1" } else { "r2" };
    let r = sess(id, WS, Some("t1"), status, Some(1));
    r.borrow_mut().notify = Some(names(&["lead"]));
    r.borrow_mut().notify_by = Some(serde_json::from_value(json!({ "kind": "thread", "threadId": "lead", "workspaceId": WS, "rootThreadId": "lead" })).expect("a scope"));
    r.borrow_mut().notify_road = Some(WorkspaceOrigin::Here);
    r.borrow_mut().turn_live = Some(TurnLive {
        reply: Some(serde_json::from_value(json!("completed")).expect("a status")),
        told: Some(2),
        steered: Some(serde_json::from_value::<IndexMap<String, Steered>>(json!({ "req_1": { "prompt": "also this", "startedBy": "person" } })).expect("steered")),
        waited: Some(JsNumber(5.0)),
        waiting_since: Some(JsNumber(9.0)),
        ..TurnLive::default()
    });
    r.borrow_mut().run = Some("run-1".into());
    r.borrow_mut().from = Some(7);
    r.borrow_mut().asked = Some(serde_json::from_value(json!({ "prompt": "do it" })).expect("asked"));
    r.borrow_mut().turn_token = Some("tok".into());
    r.borrow_mut().scope_device_id = Some("dev".into());
    r.borrow_mut().snapshot = Some("snap".into());
    r
}

fn tail(keys: Vec<String>) -> Vec<String> {
    let at = keys.iter().position(|k| k == "turnId").expect("a turnId");
    keys[at + 1..].to_vec()
}

#[test]
fn persist_sessions_keeps_live_fields_only_while_running() {
    local(async {
        let ctx = live_ctx();
        ctx.sessions.with(|s| *s = sessions_of(vec![live_row(true), live_row(false)]).take());
        let area = ctx.area();
        area.persist_sessions(WS).await;
        let doc = ctx.store.get_now("sessions", WS).expect("the document is put");
        let rows = doc["sessions"].as_array().expect("an array");
        assert_eq!(
            tail(keys_of(&rows[0])),
            names(&["notify", "notifyBy", "notifyRoad", "reply", "told", "steered", "waited", "waitingSince", "run", "from", "asked", "turnToken", "scopeDeviceId", "snapshot"]),
            "a running row carries its live fields between the notify keys and the snapshot, in this order"
        );
        assert_eq!(rows[0]["reply"], json!("completed"));
        assert_eq!(num_of(&rows[0]["told"]), 2.0);
        assert_eq!(rows[0]["steered"], json!({ "req_1": { "prompt": "also this", "startedBy": "person" } }));
        assert_eq!((num_of(&rows[0]["waited"]), num_of(&rows[0]["waitingSince"])), (5.0, 9.0));
        assert_eq!((rows[0]["run"].clone(), num_of(&rows[0]["from"])), (json!("run-1"), 7.0));
        assert_eq!(rows[0]["asked"], json!({ "prompt": "do it" }));
        assert_eq!((rows[0]["turnToken"].clone(), rows[0]["scopeDeviceId"].clone()), (json!("tok"), json!("dev")));
        assert_eq!(rows[0]["notifyBy"], json!({ "kind": "thread", "threadId": "lead", "workspaceId": WS, "rootThreadId": "lead" }));
        assert_eq!(rows[0]["notifyRoad"], json!("here"));
        assert_eq!(
            tail(keys_of(&rows[1])),
            names(&["notify", "notifyBy", "notifyRoad", "snapshot"]),
            "a settled row keeps none of them, though its turn still holds them"
        );
    });
}

// TS: runtime-index.test.ts:392
#[test]
fn persist_sessions_leaves_out_unlaunched_rows_and_other_workspaces() {
    local(async {
        let ctx = live_ctx();
        let launching = sess("r2", WS, Some("t2"), "running", Some(2));
        launching.borrow_mut().launch = Some(async {}.boxed_local().shared());
        ctx.sessions.with(|s| *s = sessions_of(vec![sess("r1", WS, Some("t1"), "completed", Some(1)), launching, sess("r3", "ws2", Some("t3"), "completed", Some(3))]).take());
        ctx.thread_records.with(|t| *t = records_of(vec![("t1", record(WS, json!({}))), ("t2", record(WS, json!({}))), ("t3", record("ws2", json!({})))]).take());
        let area = ctx.area();
        area.persist_sessions(WS).await;
        let doc = ctx.store.get_now("sessions", WS).expect("the document is put");
        let ids: Vec<Value> = doc["sessions"].as_array().expect("an array").iter().map(|r| r["id"].clone()).collect();
        assert_eq!(ids, vec![json!("r1")], "a row for a turn still reaching the machine never reaches the file, nor does another workspace's");
        assert_eq!(keys_of(&doc["threads"]), vec!["t1", "t2"], "the records of the workspace are all there");
        assert_eq!(session_ids(&ctx), vec!["r1", "r2", "r3"], "the runtime's own rows are untouched");
    });
}

// TS: runtime-index.test.ts:611
#[test]
fn persist_sessions_of_a_workspace_not_live_writes_nothing() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        let mut rows = vec![sess("w1", WS, Some("t1"), "completed", Some(1))];
        rows[0].borrow_mut().view.claude_session_id = Some("c1".into());
        rows.extend((0..SESSION_INDEX_CAP as i64 + 2).map(|i| sess(&format!("r{i}"), WS, Some(&format!("t{}", i + 10)), "completed", Some(10 + i))));
        let count = rows.len();
        ctx.sessions.with(|s| *s = sessions_of(rows).take());
        ctx.thread_records.with(|t| *t = records_of(vec![("t1", record(WS, json!({ "worked": false })))]).take());
        let area = ctx.area();
        area.persist_sessions(WS).await;
        assert!(ctx.store.logged().is_empty(), "a workspace deleted while a turn runs gets no document written back");
        assert!(ctx.store.get_now("sessions", WS).is_none());
        assert!(ctx.index_flushes.with(|m| m.is_empty()), "no chain turn was taken");
        assert_eq!(session_ids(&ctx).len(), count, "the cap did not run");
        assert_eq!(ctx.thread_records.with(|t| t["t1"].worked), Some(false), "nor did the worked mark");
    });
}

// TS: forget.test.ts:387
// TS: forget.test.ts:412
#[test]
fn persist_sessions_marks_a_settled_working_thread_worked() {
    local(async {
        let ctx = live_ctx();
        let worked = sess("r1", WS, Some("t1"), "completed", Some(1));
        worked.borrow_mut().view.claude_session_id = Some("c1".into());
        let unannounced = sess("r2", WS, Some("t2"), "completed", Some(2));
        let unrecorded = sess("r3", WS, Some("t3"), "completed", Some(3));
        unrecorded.borrow_mut().view.claude_session_id = Some("c3".into());
        let running = sess("r5", WS, Some("t5"), "running", Some(5));
        running.borrow_mut().view.claude_session_id = Some("c5".into());
        let elsewhere = sess("r7", "ws2", Some("t7"), "completed", Some(7));
        elsewhere.borrow_mut().view.claude_session_id = Some("c7".into());
        ctx.sessions.with(|s| *s = sessions_of(vec![worked, unannounced, unrecorded, running, elsewhere]).take());
        ctx.thread_records.with(|t| {
            *t = records_of(vec![
                ("t1", record(WS, json!({ "worked": false }))),
                ("t2", record(WS, json!({ "worked": false }))),
                ("t3", record(WS, json!({}))),
                ("t4", record(WS, json!({ "worked": true }))),
                ("t5", record(WS, json!({ "worked": false }))),
                ("t7", record("ws2", json!({ "worked": false }))),
            ]).take();
        });
        let area = ctx.area();
        area.persist_sessions(WS).await;
        let worked_of = |thread: &str| ctx.thread_records.with(|t| t[thread].worked);
        assert_eq!(worked_of("t1"), Some(true), "a settled row that announced its agent marks the thread");
        assert_eq!(worked_of("t2"), Some(false), "a turn that never announced marks nothing");
        assert_eq!(worked_of("t3"), None, "an absent mark stays absent");
        assert_eq!(worked_of("t4"), Some(true));
        assert_eq!(worked_of("t5"), Some(false), "a running row marks nothing yet");
        assert_eq!(worked_of("t7"), Some(false), "another workspace's row is not this document's to settle");
        let doc = ctx.store.get_now("sessions", WS).expect("the document is put");
        assert_eq!(doc["threads"]["t1"]["worked"], json!(true));
        assert_eq!(doc["threads"]["t2"]["worked"], json!(false));
        assert!(doc["threads"]["t3"].get("worked").is_none());
        assert_eq!(keys_of(&doc["threads"]), vec!["t1", "t2", "t3", "t4", "t5"]);
    });
}

// TS: forget.test.ts:412
#[test]
fn persist_sessions_marks_worked_before_the_cap_drops_the_row_that_says_so() {
    local(async {
        let ctx = live_ctx();
        let said = sess("r1", WS, Some("t1"), "completed", Some(1));
        said.borrow_mut().view.claude_session_id = Some("c1".into());
        let mut rows = vec![said, sess("r1b", WS, Some("t1"), "failed", Some(2))];
        rows.extend((0..199).map(|i| sess(&format!("f{i:03}"), WS, Some(&format!("tf{i:03}")), "completed", Some(100 + i))));
        ctx.sessions.with(|s| *s = sessions_of(rows).take());
        ctx.thread_records.with(|t| *t = records_of(vec![("t1", record(WS, json!({ "worked": false })))]).take());
        let area = ctx.area();
        area.persist_sessions(WS).await;
        assert!(!session_ids(&ctx).contains(&"r1".to_string()), "the cap dropped the row that said the thread worked");
        assert_eq!(session_ids(&ctx).len(), SESSION_INDEX_CAP);
        let doc = ctx.store.get_now("sessions", WS).expect("the document is put");
        assert_eq!(doc["threads"]["t1"]["worked"], json!(true), "but its record already says so");
        assert_eq!(doc["threads"]["t1"]["ended"], json!("completed"));
        assert_eq!(doc["sessions"].as_array().expect("an array").len(), SESSION_INDEX_CAP);
    });
}

#[test]
fn persist_sessions_lands_snapshots_in_call_order() {
    local(async {
        let ctx = live_ctx();
        ctx.sessions.with(|s| *s = sessions_of(vec![sess("r1", WS, Some("t1"), "completed", Some(1))]).take());
        let area = ctx.area();
        let first = area.persist_sessions(WS);
        ctx.sessions.with(|s| {
            s.insert("r2".to_string(), sess("r2", WS, Some("t2"), "completed", Some(2)));
        });
        let second = area.persist_sessions(WS);
        second.await;
        first.await;
        let puts: Vec<usize> = ctx.flaky.puts.borrow().iter().filter(|(c, _, _)| c == "sessions").map(|(_, _, v)| v["sessions"].as_array().expect("an array").len()).collect();
        assert_eq!(puts, vec![1, 2], "each snapshot is the rows as they were when it was asked for, and an earlier one never lands after a later one");
        assert_eq!(ctx.store.get_now("sessions", WS).expect("the document is put")["sessions"].as_array().expect("an array").len(), 2);
        assert_eq!(ctx.store.logged(), vec!["put sessions/ws1", "put sessions/ws1"]);
        assert!(ctx.index_flushes.with(|m| m.contains_key(WS)), "the puts go on the workspace's chain");
    });
}

#[test]
fn a_failed_sessions_put_is_swallowed_with_no_log() {
    local(async {
        let ctx = live_ctx();
        ctx.sessions.with(|s| *s = sessions_of(vec![sess("r1", WS, Some("t1"), "completed", Some(1))]).take());
        let area = ctx.area();
        let warned = console::capture();
        ctx.store.fail_put.set(true);
        area.persist_sessions(WS).await;
        assert!(warned.lines().is_empty());
        assert!(ctx.store.get_now("sessions", WS).is_none());
        ctx.store.fail_put.set(false);
        area.persist_sessions(WS).await;
        assert!(ctx.store.get_now("sessions", WS).is_some(), "the chain goes on after a failed put");
        settle().await;
    });
}

// ---- daemon tokens -----------------------------------------------------------------------------------------------

const SEED: &str = "abc123";
const M1_FROM_SEED: &str = "3259f64b46fa7e471e5486385184d6c0591c88a066817e09fe726d701845f3b5";
const M2_FROM_SEED: &str = "83eabdbd018d15e2a0afadaac574555a134e5908042df12f1ddf61d79719ce7a";

// TS: runtime-reach.test.ts:40
// TS: runtime-reach.test.ts:62
// TS: runtime-reach.test.ts:88
#[test]
fn token_for_machine_is_made_once_per_machine_from_the_seed_or_random() {
    let ctx = FakeTranscriptsCtx::new();
    let area = ctx.area();
    let first = area.token_for_machine("m1");
    assert_eq!(first, "000102030405060708090a0b0c0d0e0f1011121314151617", "24 random bytes as 48 lowercase hex");
    assert_eq!(area.token_for_machine("m1"), first, "kept for the host's life");
    assert_eq!(ctx.ids.drawn.get(), 24, "no second draw");
    let second = area.token_for_machine("m2");
    assert_eq!(second, "18191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f");
    assert_ne!(first, second, "a token per machine");

    let seeded = FakeTranscriptsCtx::new();
    *seeded.seed.borrow_mut() = Some(SEED.to_string());
    let area = seeded.area();
    assert_eq!(area.token_for_machine("m1"), M1_FROM_SEED, "derived from the seed a caller pinned");
    assert_eq!(area.token_for_machine("m2"), M2_FROM_SEED);
    assert_eq!(area.token_for_machine("m1"), M1_FROM_SEED);
    assert_eq!(seeded.ids.drawn.get(), 0, "a seeded host draws nothing");

    for bad in ["it's not hex", "ABCDEF", "", "12 34"] {
        let ctx = FakeTranscriptsCtx::new();
        *ctx.seed.borrow_mut() = Some(bad.to_string());
        let refused = TranscriptsArea::new(ctx.clone()).err().expect("the build fails");
        assert_eq!(refused.message, "a daemon token is lowercase hex", "{bad:?}");
    }
}

fn machine_of(id: &str) -> Arc<dyn Machine> {
    let machine: Arc<FakeMachine> = FakeMachine::new(id);
    machine
}

// TS: runtime-reach.test.ts:22
// TS: runtime-reach.test.ts:103
// TS: serve-daemon.test.ts:103
#[test]
fn daemon_token_of_shares_one_rotation_and_caches_a_miss_for_a_minute() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        *ctx.seed.borrow_mut() = Some(SEED.to_string());
        let area = ctx.area();
        let machine = machine_of("m1");
        ctx.hold_rotations.set(true);
        let one = area.daemon_token_of(machine.clone(), Some("/guest/.wsp-daemon-token".to_string()));
        let two = area.daemon_token_of(machine.clone(), None);
        let (a, b, _) = join!(one, two, async {
            settle().await;
            ctx.release_rotations();
        });
        assert_eq!(a.expect("the first answers"), Some(M1_FROM_SEED.to_string()));
        assert_eq!(b.expect("the second answers"), Some(M1_FROM_SEED.to_string()));
        assert_eq!(
            ctx.rotations_asked(),
            vec![("m1".to_string(), M1_FROM_SEED.to_string(), Some("/guest/.wsp-daemon-token".to_string()))],
            "one rotation, shared by every caller, at the path the first asked for"
        );
        assert!(area.writing.borrow().is_empty(), "the rotation in flight is forgotten once it settles");
        assert_eq!(area.daemon_tokens.borrow().get("m1").map(|d| d.has_daemon), Some(true));

        ctx.hold_rotations.set(false);
        assert_eq!(area.daemon_token_of(machine.clone(), None).await.expect("answers"), Some(M1_FROM_SEED.to_string()));
        ctx.clock.advance(DAEMON_TOKEN_MISS_TTL_MS as i64 * 5);
        assert_eq!(area.daemon_token_of(machine.clone(), None).await.expect("answers"), Some(M1_FROM_SEED.to_string()));
        assert_eq!(ctx.rotations_asked().len(), 1, "a machine that had a daemon is never asked again");
    });
}

#[test]
fn daemon_token_of_a_machine_without_a_daemon_is_asked_again_after_a_minute() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        *ctx.seed.borrow_mut() = Some(SEED.to_string());
        ctx.rotate_answers.borrow_mut().push_back(Ok(false));
        let area = ctx.area();
        let machine = machine_of("m1");
        assert_eq!(area.daemon_token_of(machine.clone(), None).await.expect("answers"), None, "a guest with no daemon has no token");
        assert_eq!(ctx.rotations_asked().len(), 1);
        ctx.clock.advance(DAEMON_TOKEN_MISS_TTL_MS as i64 - 1);
        assert_eq!(area.daemon_token_of(machine.clone(), None).await.expect("answers"), None);
        assert_eq!(ctx.rotations_asked().len(), 1, "a miss under a minute old stands");
        ctx.clock.advance(1);
        assert_eq!(area.daemon_token_of(machine.clone(), None).await.expect("answers"), Some(M1_FROM_SEED.to_string()), "at a minute it is asked again");
        assert_eq!(ctx.rotations_asked().len(), 2);
        assert_eq!(area.daemon_tokens.borrow().get("m1").map(|d| (d.has_daemon, d.at)), Some((true, 1_000_000 + DAEMON_TOKEN_MISS_TTL_MS as i64)));
    });
}

#[test]
fn a_failed_rotation_fails_every_caller_and_caches_nothing() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        *ctx.seed.borrow_mut() = Some(SEED.to_string());
        ctx.rotate_answers.borrow_mut().push_back(Err(err("the guest did not answer")));
        ctx.hold_rotations.set(true);
        let area = ctx.area();
        let machine = machine_of("m2");
        let one = area.daemon_token_of(machine.clone(), None);
        let two = area.daemon_token_of(machine.clone(), None);
        let (a, b, _) = join!(one, two, async {
            settle().await;
            ctx.release_rotations();
        });
        assert_eq!(a.expect_err("the first fails").message, "the guest did not answer");
        assert_eq!(b.expect_err("the second fails").message, "the guest did not answer");
        assert_eq!(ctx.rotations_asked().len(), 1);
        assert!(!area.daemon_tokens.borrow().contains_key("m2"), "nothing is cached");
        assert!(area.writing.borrow().is_empty());

        ctx.hold_rotations.set(false);
        assert_eq!(area.daemon_token_of(machine, None).await.expect("answers"), Some(M2_FROM_SEED.to_string()));
        assert_eq!(ctx.rotations_asked().len(), 2, "the next ask rotates again");
    });
}

// ---- reads for other areas ---------------------------------------------------------------------------------------

#[test]
fn accrued_follows_the_cost_events() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        let area = ctx.area();
        assert_eq!(ctx.bus.listening("workspace.cost"), 1, "the listener is added when the area is built");
        let cost = |ws: &str, usd: f64| json!({ "type": "workspace.cost", "workspaceId": ws, "phase": "running", "rateUsdPerHour": 0.5, "awakeMs": 3_600_000, "accruedUsd": usd, "at": "2026-01-01T00:00:00.000Z" });
        ctx.bus.emit_json(cost(WS, 1.25));
        ctx.bus.emit_json(cost("ws2", 9.5));
        assert_eq!(area.accrued.borrow().get(WS).copied(), Some(1.25));
        ctx.bus.emit_json(cost(WS, 2.5));
        assert_eq!(area.accrued.borrow().get(WS).copied(), Some(2.5), "the newest figure the meter pushed");
        assert_eq!(area.accrued.borrow().get("ws2").copied(), Some(9.5));
        assert_eq!(area.accrued.borrow().keys().cloned().collect::<Vec<_>>(), vec![WS, "ws2"]);
    });
}

fn view_in(thread: &str, ws: &str, fields: Value) -> wsp_wire::views::session::SessionView {
    let mut v = json!({ "id": format!("v-{thread}-{}", fields["startedAt"]), "workspaceId": ws, "threadId": thread });
    for (k, x) in fields.as_object().expect("an object").iter() {
        v[k] = x.clone();
    }
    view(v)
}

fn picks(model: Option<&str>, window: Option<&str>, effort: Option<&str>) -> RanPicks {
    RanPicks { model: model.map(String::from), context_window: window.map(String::from), effort: effort.map(String::from), permission_mode: None }
}

// TS: thread-keeps-picks.test.ts:82
// TS: thread-keeps-picks.test.ts:103
// TS: thread-keeps-picks.test.ts:118
#[test]
fn ran_on_reads_the_rows_then_the_sessions_facts() {
    let ctx = FakeTranscriptsCtx::new();
    let area = ctx.area();
    ctx.rows_on.borrow_mut().insert(
        "t1".into(),
        vec![
            view_in("t1", WS, json!({ "startedAt": 20, "effort": "max" })),
            view_in("t1", "ws2", json!({ "startedAt": 30, "model": "other-model", "effort": "low" })),
            view_in("t1", WS, json!({ "startedAt": 10, "model": "claude-opus-5", "contextWindow": "1m" })),
        ],
    );
    assert_eq!(area.ran_on(WS, "t1", None), picks(Some("claude-opus-5"), Some("1m"), Some("max")), "the thread's rows on this workspace by start, each pick off the newest row that named it");
    assert_eq!(area.ran_on(WS, "t1", Some("sess-1")), picks(Some("claude-opus-5"), Some("1m"), Some("max")));
    assert!(ctx.fact_asks.borrow().is_empty(), "a pick the rows say is not asked of the session");

    ctx.rows_on.borrow_mut().insert("t2".into(), vec![view_in("t2", WS, json!({ "startedAt": 1, "effort": "low" }))]);
    ctx.facts.borrow_mut().insert((WS.into(), "sess-2".into(), "model".into()), "claude-opus-5[1m]".into());
    ctx.facts.borrow_mut().insert((WS.into(), "sess-2".into(), "effort".into()), "high".into());
    assert_eq!(area.ran_on(WS, "t2", Some("sess-2")), picks(Some("claude-opus-5"), Some("1m"), Some("low")), "a model the rows do not say is read off the session; the rows' effort wins");
    assert_eq!(*ctx.fact_asks.borrow(), vec![(WS.to_string(), "sess-2".to_string(), "model".to_string())]);
    assert_eq!(area.ran_on(WS, "t2", None), picks(None, None, Some("low")));

    ctx.fact_asks.borrow_mut().clear();
    ctx.facts.borrow_mut().insert((WS.into(), "sess-3".into(), "model".into()), "claude-sonnet-5".into());
    ctx.facts.borrow_mut().insert((WS.into(), "sess-3".into(), "effort".into()), "max".into());
    assert_eq!(area.ran_on(WS, "t3", Some("sess-3")), picks(Some("claude-sonnet-5"), None, Some("max")), "a thread whose rows fell off the index is read off its session's newest start");
    assert_eq!(
        *ctx.fact_asks.borrow(),
        vec![(WS.to_string(), "sess-3".to_string(), "model".to_string()), (WS.to_string(), "sess-3".to_string(), "effort".to_string())],
        "the model is asked, then the effort"
    );
    assert_eq!(area.ran_on(WS, "t3", Some("sess-unknown")), RanPicks::default(), "a fact the session holds none of leaves the pick out");
}

// TS: box-lead.test.ts:446
#[test]
fn box_of_is_none_for_a_local_workspace() {
    let ctx = FakeTranscriptsCtx::new();
    ctx.add_live(json!({ "id": "local1", "kind": "local" }));
    ctx.add_live(json!({ "id": "cloud1", "kind": "cloud" }));
    ctx.add_live(json!({ "id": "place1", "kind": "place" }));
    let area = ctx.area();
    let latest = |thread: &str, ws: &str| {
        ctx.latest_on.borrow_mut().insert(thread.into(), view_in(thread, ws, json!({ "startedAt": 1 })));
    };
    latest("t1", "local1");
    latest("t2", "cloud1");
    latest("t3", "gone1");
    latest("t5", "place1");
    assert!(area.box_of("t1").is_none(), "this computer is not a box");
    assert_eq!(area.box_of("t2").map(|e| e.borrow().record.borrow().id.clone()), Some("cloud1".to_string()), "the workspace of the thread's latest row");
    assert!(area.box_of("t3").is_none(), "a workspace that is not live");
    assert_eq!(area.box_of("t5").map(|e| e.borrow().record.borrow().id.clone()), Some("place1".to_string()), "only a local workspace is not a box");

    ctx.thread_records.with(|t| *t = records_of(vec![("t4", record("cloud1", json!({}))), ("t6", record("local1", json!({})))]).take());
    assert_eq!(area.box_of("t4").map(|e| e.borrow().record.borrow().id.clone()), Some("cloud1".to_string()), "no row: the workspace of the thread's record");
    assert!(area.box_of("t6").is_none());
    assert!(area.box_of("nobody").is_none());
    ctx.latest_on.borrow_mut().insert("t4".into(), view_in("t4", "local1", json!({ "startedAt": 2 })));
    assert!(area.box_of("t4").is_none(), "the latest row wins over the record");
}
