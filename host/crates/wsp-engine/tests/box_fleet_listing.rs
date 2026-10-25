// SPDX-License-Identifier: AGPL-3.0-only
//! box-backend.test.ts "BoxBackend against a fake Box API", the fleet and its listings (lines 672-1420): get, list,
//! the snapshot listing and deletes, the templates that are named snapshots, the provider's state table read through
//! get and list, and the backstop timer. Every body in src/ is `todo!()`, so each test fails at the first call into it.
//!
//! golden's, skipped (they exercise golden.ts): box-backend.test.ts:1136 (snapshotUntilGone) and :1175 (killUntilGone,
//! GONE_READS). :1197's closing killUntilGone assertion is left out of its port.
mod common;

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_engine::box_backend::{BoxBudgetsAsk, BoxMachine, BACKSTOP_SLACK_MS, TRIAL_TTL_S};
use wsp_engine::errors::{EngineError, RoadError};
use wsp_engine::fetch::{Fetch, FetchRequest, FetchResponse};
use wsp_engine::machine::{
    Machine, MachineBackend, MachineKind, MachineListRow, MachineState, Seen as MachineSeen, SnapshotRow, TemplateRow,
};
use wsp_wire::types::JsNumber;
use wsp_wire::wire::capabilities::WorkspaceSize;
use wsp_wire::wire::machine_link::TemplateRowStatus;

use common::fake_box::{backend_on, backend_with, box_view, error, info, machine_on, named, offline, with, Answer, FakeBox, Reply};
use common::fake_clock::FakeClock;

const ID: &str = "bx_tumrjngm";

// labels.ts:6, :8, :14, :17
const WSP_LABEL: &str = "wsp";
const BUILDER_LABEL: &str = "wsp-builder";
const OWNER_LABEL: &str = "wsp-owner";
const WORKSPACE_LABEL: &str = "wsp-workspace";

fn reply(status: u16, body: Value) -> Answer {
    Answer::Reply(Reply { status, body })
}

fn idle(id: &str, state: &str) -> Answer {
    info(id, state, json!({}))
}

fn path(suffix: &str) -> String {
    format!("/sandboxes/{ID}{suffix}")
}

fn labels(pairs: &[(&str, &str)]) -> IndexMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn size(cpu: f64, mem_mb: f64) -> Option<WorkspaceSize> {
    Some(WorkspaceSize { cpu: JsNumber(cpu), mem_mb: JsNumber(mem_mb) })
}

fn mins(n: i64) -> i64 {
    n * 60_000
}

fn named_body(name: &str, status: &str, extra: Value) -> Answer {
    reply(200, json!({ "ok": true, "snapshot": named(name, status, extra) }))
}

fn patches(api: &FakeBox) -> usize {
    api.calls().iter().filter(|c| c.starts_with("PATCH")).count()
}

fn snapshot_ids(rows: &[SnapshotRow]) -> Vec<String> {
    rows.iter().map(|r| r.id.clone()).collect()
}

fn cursors(api: &FakeBox, at: &str) -> Vec<Option<String>> {
    api.seen().iter().filter(|s| s.path == at).map(|s| s.query_get("cursor").map(str::to_string)).collect()
}

/// box-backend.test.ts:717
#[tokio::test]
async fn a_stopped_boxs_handle_carries_its_stop_the_last_save_boat_completed_and_a_running_ones_none() {
    let api = FakeBox::new()
        .on("GET", "/sandboxes/bx_a1", vec![info("bx_a1", "archived", json!({ "snapshotCompletedAt": "2026-09-27T04:50:29.157Z" }))])
        .on("GET", "/sandboxes/bx_r1", vec![info("bx_r1", "ready", json!({ "snapshotCompletedAt": "2026-09-27T04:50:29.157Z" }))]);
    let on = backend_on(&api, "trial", Default::default());
    let stopped = on.backend.get("bx_a1").await.unwrap();
    let seen = stopped.seen().expect("a handle from get() carries what the provider saw");
    assert_eq!(seen.state, MachineState::Paused);
    assert_eq!(seen.paused_at.as_deref(), Some("2026-09-27T04:50:29.157Z"));
    let running = on.backend.get("bx_r1").await.unwrap();
    assert_eq!(running.seen().and_then(|s| s.paused_at.clone()), None);
    assert_eq!(on.backend.lifecycle().unwrap().disk_kept_ms(), Some(14 * 24 * 3_600_000));
}

/// box-backend.test.ts:1068
#[tokio::test]
async fn a_named_snapshot_is_the_template_promoting_answers_the_name_and_the_template_reads_the_snapshots_status() {
    let api = FakeBox::new()
        .on(
            "GET",
            "/named-snapshots/wsp-mac-default-v1",
            vec![reply(
                200,
                json!({ "ok": true, "type": "snapshot.named.info", "snapshot": named("wsp-mac-default-v1", "ready", json!({})) }),
            )],
        )
        .on("GET", "/named-snapshots/wsp-mac-default-v2", vec![named_body("wsp-mac-default-v2", "saving", json!({}))])
        .on(
            "GET",
            "/named-snapshots/wsp-mac-default-v3",
            vec![named_body("wsp-mac-default-v3", "failed", json!({ "error": "capture timed out" }))],
        )
        .on(
            "GET",
            "/named-snapshots",
            vec![reply(
                200,
                json!({ "ok": true, "type": "snapshot.named.list", "snapshots": [named("wsp-mac-default-v1", "ready", json!({}))] }),
            )],
        )
        .on(
            "DELETE",
            "/named-snapshots/wsp-mac-default-v1",
            vec![reply(200, json!({ "ok": true, "type": "snapshot.named.deleted", "name": "wsp-mac-default-v1" }))],
        );
    let on = backend_on(&api, "trial", Default::default());
    let b = &on.backend;
    assert_eq!(b.promote_snapshot("wsp-mac-default-v1", "wsp-mac-default-v1").await.unwrap(), "wsp-mac-default-v1");
    let ready = TemplateRow {
        id: "wsp-mac-default-v1".into(),
        name: "wsp-mac-default-v1".into(),
        status: TemplateRowStatus::Ready,
        error: None,
        created_at: Some("2026-09-11T06:47:23.959Z".into()),
    };
    assert_eq!(b.get_template("wsp-mac-default-v1").await.unwrap(), ready);
    assert_eq!(b.get_template("wsp-mac-default-v2").await.unwrap().status, TemplateRowStatus::Building);
    let failed = b.get_template("wsp-mac-default-v3").await.unwrap();
    assert_eq!(failed.status, TemplateRowStatus::Failed);
    assert_eq!(failed.error.as_deref(), Some("capture timed out"));
    assert_eq!(b.list_templates().await.unwrap(), vec![ready]);
    b.delete_template("wsp-mac-default-v1").await.unwrap();
    assert_eq!(api.calls().last().unwrap(), "DELETE /named-snapshots/wsp-mac-default-v1");
    // A template the provider no longer holds is deleted already.
    b.delete_template("wsp-mac-default-v9").await.unwrap();
    // The promote made no call: the name was durable the moment the save read ready.
    assert_eq!(api.calls().into_iter().filter(|c| c.starts_with("POST")).collect::<Vec<_>>(), Vec::<String>::new());
}

/// box-backend.test.ts:1089
#[tokio::test]
async fn lists_every_snapshot_with_the_size_storage_holds_and_the_size_it_restores_to_only_where_the_provider_says_it() {
    let api = FakeBox::new()
        .on("GET", "/named-snapshots", vec![reply(200, json!({ "ok": true, "snapshots": [named("wsp-mac-default-v1", "ready", json!({}))] }))])
        .on(
            "GET",
            "/snapshots",
            vec![with(|seen| {
                let first = seen.query_get("cursor").is_none();
                let snapshots = if first {
                    json!([{ "id": "866f04de-8e8a-4401-9e29-cf58e1738213", "sandboxId": ID, "status": "completed", "kind": "incremental", "generation": 4, "chainId": "d9365279-5b40-4860-85b4-4a168a878da1", "createdAt": "2026-09-11T06:32:52.039Z", "completedAt": "2026-09-11T06:32:53.272Z", "sizeBytes": 343, "contentSizeBytes": 7838315315u64, "fileCount": 2 }])
                } else {
                    json!([{ "id": "1826c6c1-6f77-4beb-8406-93f9e27281a2", "sandboxId": ID, "status": "completed", "kind": "noop", "generation": null, "chainId": null, "createdAt": "2026-09-11T06:55:43.354Z", "completedAt": "2026-09-11T06:55:43.547Z", "sizeBytes": null }])
                };
                reply(
                    200,
                    json!({ "ok": true, "type": "snapshot.list", "snapshots": snapshots, "pageInfo": { "nextCursor": if first { json!("c2") } else { Value::Null }, "hasMore": first, "limit": 200 } }),
                )
            })],
        );
    let on = backend_on(&api, "trial", Default::default());
    // A history row says what it restores to beside the change it stores; a named one says no such size, so none is put
    // on it and no image reads its stored figure as its size.
    assert_eq!(
        on.backend.list_snapshots(None).await.unwrap(),
        vec![
            SnapshotRow {
                id: "wsp-mac-default-v1".into(),
                name: Some("wsp-mac-default-v1".into()),
                size_bytes: JsNumber(1255755776.0),
                restored_bytes: None,
                created_at: Some("2026-09-11T06:47:23.959Z".into()),
                parent: None,
            },
            SnapshotRow {
                id: "866f04de-8e8a-4401-9e29-cf58e1738213".into(),
                name: None,
                size_bytes: JsNumber(343.0),
                restored_bytes: Some(JsNumber(7838315315.0)),
                created_at: Some("2026-09-11T06:32:53.272Z".into()),
                parent: Some(Some("d9365279-5b40-4860-85b4-4a168a878da1".into())),
            },
            SnapshotRow {
                id: "1826c6c1-6f77-4beb-8406-93f9e27281a2".into(),
                name: None,
                size_bytes: JsNumber(0.0),
                restored_bytes: None,
                created_at: Some("2026-09-11T06:55:43.547Z".into()),
                parent: Some(None),
            },
        ]
    );
    assert_eq!(cursors(&api, "/snapshots"), vec![None, Some("c2".to_string())]);
    let other = FakeBox::new().on("GET", "/named-snapshots", vec![reply(200, json!({ "ok": true, "items": [] }))]);
    let e = backend_on(&other, "trial", Default::default()).backend.list_snapshots(None).await.err().expect("a refusal");
    assert!(e.to_string().contains("GET /named-snapshots answered without a snapshots array"), "{e}");
}

/// box-backend.test.ts:1119
#[tokio::test]
async fn asked_for_named_snapshots_alone_reads_no_history_and_for_a_history_id_reads_only_as_far_as_its_page() {
    let history = |cursor: Option<&str>| -> Answer {
        let first = cursor.is_none();
        reply(
            200,
            json!({
                "ok": true,
                "type": "snapshot.list",
                "snapshots": [{
                    "id": if first { "866f04de-8e8a-4401-9e29-cf58e1738213" } else { "1826c6c1-6f77-4beb-8406-93f9e27281a2" },
                    "sandboxId": ID,
                    "status": "completed",
                    "createdAt": "2026-09-11T06:32:52.039Z",
                    "sizeBytes": 343,
                    "contentSizeBytes": 7838315315u64,
                }],
                "pageInfo": { "nextCursor": if first { json!("c2") } else { Value::Null }, "hasMore": first, "limit": 200 },
            }),
        )
    };
    let api = FakeBox::new()
        .on(
            "GET",
            "/named-snapshots",
            vec![reply(200, json!({ "ok": true, "snapshots": [named("wsp-mac-default-v1", "ready", json!({}))] }))],
        )
        .on("GET", "/snapshots", vec![with(move |seen| history(seen.query_get("cursor")))]);
    let on = backend_on(&api, "trial", Default::default());
    let rows = on.backend.list_snapshots(Some(&["wsp-mac-default-v1".to_string()])).await.unwrap();
    assert_eq!(snapshot_ids(&rows), vec!["wsp-mac-default-v1"]);
    assert!(api.seen().iter().all(|s| s.path != "/snapshots"));
    let rows = on.backend.list_snapshots(Some(&["866f04de-8e8a-4401-9e29-cf58e1738213".to_string()])).await.unwrap();
    assert_eq!(snapshot_ids(&rows), vec!["wsp-mac-default-v1", "866f04de-8e8a-4401-9e29-cf58e1738213"]);
    assert_eq!(cursors(&api, "/snapshots"), vec![None]);
}

/// box-backend.test.ts:1146
#[tokio::test]
async fn deletes_a_snapshot_by_what_its_id_is_history_with_the_confirm_header_a_named_one_by_name_one_gone_is_done() {
    let api = FakeBox::new().on("DELETE", "/snapshots/866f04de-8e8a-4401-9e29-cf58e1738213", vec![reply(202, json!({ "ok": true }))]).on(
        "DELETE",
        "/named-snapshots/wsp-mac-default-v1",
        vec![reply(200, json!({ "ok": true }))],
    );
    let on = backend_on(&api, "trial", Default::default());
    on.backend.delete_snapshot("866f04de-8e8a-4401-9e29-cf58e1738213").await.unwrap();
    assert_eq!(
        api.seen().last().unwrap().headers.get("x-ascii-confirm-delete").map(String::as_str),
        Some("866f04de-8e8a-4401-9e29-cf58e1738213")
    );
    on.backend.delete_snapshot("wsp-mac-default-v1").await.unwrap();
    on.backend.delete_snapshot("wsp-mac-default-v7").await.unwrap();
    assert_eq!(
        api.calls(),
        vec![
            "DELETE /snapshots/866f04de-8e8a-4401-9e29-cf58e1738213",
            "DELETE /named-snapshots/wsp-mac-default-v1",
            "DELETE /named-snapshots/wsp-mac-default-v7",
        ]
    );
    let refused = FakeBox::new().on(
        "DELETE",
        "/named-snapshots/wsp-mac-default-v1",
        vec![error(409, "save_in_progress", "a save under this name is still running").into()],
    );
    let e = backend_on(&refused, "trial", Default::default()).backend.delete_snapshot("wsp-mac-default-v1").await.unwrap_err();
    let EngineError::Provider(w) = &e else { panic!("not a Provider refusal: {e:?}") };
    assert_eq!(w.code.as_deref(), Some("save_in_progress"));
}

/// box-backend.test.ts:1197
#[tokio::test]
async fn a_state_the_table_never_learned_reads_running_on_the_machine_its_handle_and_the_listing() {
    let api = FakeBox::new()
        .on("GET", &path(""), vec![idle(ID, "deleting")])
        .on(
            "GET",
            "/sandboxes",
            vec![with(|seen| {
                let rows = if seen.query_get("state") == Some("archived") { json!([]) } else { json!([box_view(ID, "deleting", json!({ "name": "wsp" }))]) };
                reply(200, json!({ "ok": true, "type": "sandbox.list", "sandboxes": rows, "pageInfo": { "nextCursor": null, "hasMore": false, "limit": 200 } }))
            })],
        )
        .on("DELETE", &path(""), vec![reply(202, json!({ "ok": true, "operation": { "id": "bdop_1" } }))]);
    let (on, machine) = machine_on(&api, ID);
    assert_eq!(machine.state().await.unwrap(), MachineState::Running);
    assert_eq!(on.backend.get(ID).await.unwrap().seen().map(|s| s.state), Some(MachineState::Running));
    let listed: Vec<MachineState> = on.backend.list(None).await.unwrap().iter().map(|m| m.state).collect();
    assert_eq!(listed, vec![MachineState::Running]);
    // The TS ends on `killUntilGone(...)` rejecting machineAlive/running: golden.ts's, skipped.
}

/// box-backend.test.ts:1215
#[tokio::test]
async fn get_reads_the_labels_off_the_name_and_what_the_provider_saw() {
    let api = FakeBox::new().on("GET", &path(""), vec![info(ID, "archived", json!({ "name": "wsp;o=h_1a2b3c4d;w=ws_1" }))]);
    let on = backend_on(&api, "trial", Default::default());
    let machine = on.backend.get(ID).await.unwrap();
    assert_eq!(machine.labels(), Some(&labels(&[(WSP_LABEL, "1"), (OWNER_LABEL, "h_1a2b3c4d"), (WORKSPACE_LABEL, "ws_1")])));
    assert_eq!(
        machine.seen(),
        Some(&MachineSeen { state: MachineState::Paused, created_at: Some("2026-09-11T06:28:06.053Z".into()), paused_at: None })
    );
}

/// box-backend.test.ts:1223
#[tokio::test]
async fn lists_wsps_boxes_alone_off_both_listings_a_box_on_both_once_by_the_labels_asked_for_following_the_cursor() {
    // The plain listing as the spike read it at 20f: an archived box beside an idle one. The archived listing as it
    // read it at 26d, with the boxes the plain one may leave out on another day.
    let api = FakeBox::new().on(
        "GET",
        "/sandboxes",
        vec![with(|seen| {
            if seen.query_get("state") == Some("archived") {
                return reply(
                    200,
                    json!({
                        "ok": true,
                        "type": "sandbox.list",
                        "sandboxes": [
                            box_view("bx_s24x3q6e", "archived", json!({ "name": "wsp;o=h_1a2b3c4d;w=ws_1" })),
                            box_view("bx_napping", "archived", json!({ "name": "wsp;o=h_1a2b3c4d;w=ws_2" })),
                        ],
                        "pageInfo": { "nextCursor": null, "hasMore": false, "limit": 200 },
                    }),
                );
            }
            let first = seen.query_get("cursor").is_none();
            let rows = if first {
                json!([
                    box_view("bx_s24x3q6e", "archived", json!({ "name": "wsp;o=h_1a2b3c4d;w=ws_1" })),
                    box_view("bx_other", "idle", json!({ "name": "Box 2026-09-11 06:28" })),
                ])
            } else {
                json!([
                    box_view("bx_tumrjngm", "idle", json!({ "name": "wsp;o=h_1a2b3c4d;b=1", "type": "default", "vcpu": 4, "memoryGB": 8 })),
                    box_view("bx_theirs", "idle", json!({ "name": "wsp;o=h_other" })),
                ])
            };
            reply(
                200,
                json!({ "ok": true, "type": "sandbox.list", "sandboxes": rows, "pageInfo": { "nextCursor": if first { json!("c2") } else { Value::Null }, "hasMore": first, "limit": 200 } }),
            )
        })],
    );
    let on = backend_on(&api, "trial", Default::default());
    let rows = on.backend.list(Some(&labels(&[(OWNER_LABEL, "h_1a2b3c4d")]))).await.unwrap();
    assert_eq!(
        rows,
        vec![
            MachineListRow {
                id: "bx_s24x3q6e".into(),
                state: MachineState::Paused,
                labels: labels(&[(WSP_LABEL, "1"), (OWNER_LABEL, "h_1a2b3c4d"), (WORKSPACE_LABEL, "ws_1")]),
                size: size(2.0, 4096.0),
            },
            MachineListRow {
                id: "bx_tumrjngm".into(),
                state: MachineState::Running,
                labels: labels(&[(WSP_LABEL, "1"), (OWNER_LABEL, "h_1a2b3c4d"), (BUILDER_LABEL, "1")]),
                size: size(4.0, 8192.0),
            },
            MachineListRow {
                id: "bx_napping".into(),
                state: MachineState::Paused,
                labels: labels(&[(WSP_LABEL, "1"), (OWNER_LABEL, "h_1a2b3c4d"), (WORKSPACE_LABEL, "ws_2")]),
                size: size(2.0, 4096.0),
            },
        ]
    );
    let asked: Vec<(Option<String>, Option<String>, Option<String>)> = api
        .seen()
        .iter()
        .map(|s| {
            (s.query_get("limit").map(str::to_string), s.query_get("cursor").map(str::to_string), s.query_get("state").map(str::to_string))
        })
        .collect();
    let some = |s: &str| Some(s.to_string());
    assert_eq!(asked, vec![(some("200"), None, None), (some("200"), some("c2"), None), (some("200"), None, some("archived"))]);
    let ids: Vec<String> = on.backend.list(None).await.unwrap().into_iter().map(|r| r.id).collect();
    assert_eq!(ids, vec!["bx_s24x3q6e", "bx_tumrjngm", "bx_theirs", "bx_napping"]);
}

/// The PATCH answer that echoes the timer it was sent as the box's `archiveAfter`.
fn echoing_timer() -> Answer {
    with(|seen| {
        let ttl = seen.body["ttlSeconds"].as_i64().unwrap_or_default();
        let at = jiff::Timestamp::from_second(ttl).unwrap().to_string();
        info(ID, "idle", json!({ "archiveAfter": at }))
    })
}

/// box-backend.test.ts:1253
#[tokio::test]
async fn pushes_the_providers_stop_timer_to_the_runtimes_backstop_instant_capped_by_the_trial_once_a_minute_at_most() {
    let api = FakeBox::new().on("PATCH", &path(""), vec![echoing_timer()]);
    let (on, machine) = machine_on(&api, ID);
    let lifecycle = on.backend.lifecycle().unwrap();
    lifecycle.backstop(&machine, on.clock.now_ms() + mins(40), false).await.unwrap();
    assert_eq!(api.took("PATCH", &path("")).unwrap().body, json!({ "ttlSeconds": 40 * 60 }));
    // Armed again a few seconds later on a streamed chunk: not worth a call.
    on.clock.advance(5_000);
    lifecycle.backstop(&machine, on.clock.now_ms() + mins(40), false).await.unwrap();
    assert_eq!(patches(&api), 1);
    // Past the slack it is.
    on.clock.advance(BACKSTOP_SLACK_MS as i64);
    lifecycle.backstop(&machine, on.clock.now_ms() + mins(40), false).await.unwrap();
    assert_eq!(patches(&api), 2);
    // Auto-nap off hands over six hours; the trial takes two at most.
    on.clock.advance(BACKSTOP_SLACK_MS as i64);
    lifecycle.backstop(&machine, on.clock.now_ms() + mins(360), false).await.unwrap();
    assert_eq!(api.seen().last().unwrap().body, json!({ "ttlSeconds": TRIAL_TTL_S }));
}

/// box-backend.test.ts:1272
#[tokio::test]
async fn an_exact_backstop_goes_as_it_is_earlier_than_the_last_one_sent() {
    let api = FakeBox::new().on("PATCH", &path(""), vec![idle(ID, "idle")]);
    let (on, machine) = machine_on(&api, ID);
    let lifecycle = on.backend.lifecycle().unwrap();
    lifecycle.backstop(&machine, on.clock.now_ms() + mins(40), false).await.unwrap();
    lifecycle.backstop(&machine, on.clock.now_ms() + mins(12), false).await.unwrap();
    assert_eq!(patches(&api), 1);
    lifecycle.backstop(&machine, on.clock.now_ms() + mins(12), true).await.unwrap();
    assert_eq!(api.seen().last().unwrap().body, json!({ "ttlSeconds": 12 * 60 }));
}

/// box-backend.test.ts:1282
#[tokio::test]
async fn off_the_trial_the_backstop_is_the_runtimes_instant_whole_and_a_killed_boxs_instant_is_forgotten() {
    let api = FakeBox::new().on("PATCH", &path(""), vec![idle(ID, "idle")]).on(
        "DELETE",
        &path(""),
        vec![reply(202, json!({ "ok": true, "operation": { "id": "bdop_1" } }))],
    );
    let on = backend_on(&api, "standard", Default::default());
    let machine = BoxMachine::new(on.backend.clone(), ID, MachineKind::Sandbox, None, None, false);
    on.backend.box_backstop(ID, on.clock.now_ms() + mins(360), false).await.unwrap();
    assert_eq!(api.took("PATCH", &path("")).unwrap().body, json!({ "ttlSeconds": 6 * 60 * 60 }));
    machine.kill().await.unwrap();
    on.backend.box_backstop(ID, on.clock.now_ms() + mins(360), false).await.unwrap();
    assert_eq!(patches(&api), 2);
}

/// box-backend.test.ts:1296
#[tokio::test]
async fn a_failed_backstop_waits_its_backoff_30s_doubling_to_five_minutes_and_every_delta_inside_it_is_dropped() {
    let tries = Arc::new(Mutex::new(Vec::<i64>::new()));
    let clock_of = Arc::new(OnceLock::<Arc<FakeClock>>::new());
    let api = FakeBox::new().on(
        "PATCH",
        &path(""),
        vec![with({
            let (tries, clock_of) = (tries.clone(), clock_of.clone());
            move |_| {
                tries.lock().unwrap().push(clock_of.get().expect("the clock is set before any call").now_ms());
                Answer::Road(offline())
            }
        })],
    );
    let (on, _machine) = machine_on(&api, ID);
    clock_of.set(on.clock.clone()).ok().unwrap();
    // A delta every 10 s for 2000 s: every backoff is a multiple of 10 s, so each wait is exactly its window.
    for _ in 0..200 {
        let _ = on.backend.box_backstop(ID, on.clock.now_ms() + mins(40), false).await;
        on.clock.advance(10_000);
    }
    let tries = tries.lock().unwrap().clone();
    let waits: Vec<i64> = tries.windows(2).map(|w| w[1] - w[0]).collect();
    assert!(waits.len() >= 6, "{waits:?}");
    let mut expected = vec![30_000, 60_000, 120_000, 240_000];
    expected.extend(std::iter::repeat(300_000).take(waits.len() - 4));
    assert_eq!(waits, expected);
}

/// Hands every call to the fake, holding each PATCH until `open` is set.
struct Gated {
    inner: Arc<FakeBox>,
    sent: AtomicUsize,
    gate: tokio::sync::watch::Receiver<bool>,
}

#[async_trait]
impl Fetch for Gated {
    async fn fetch(&self, req: FetchRequest) -> Result<FetchResponse, RoadError> {
        if req.method == "PATCH" {
            self.sent.fetch_add(1, Ordering::SeqCst);
            let mut gate = self.gate.clone();
            gate.wait_for(|open| *open).await.unwrap();
        }
        self.inner.fetch(req).await
    }
}

/// box-backend.test.ts:1313
#[tokio::test]
async fn a_second_backstop_never_starts_while_one_is_in_flight() {
    let api = FakeBox::new().on("PATCH", &path(""), vec![idle(ID, "idle")]).on("GET", "/limits", vec![common::fake_box::limits("trial")]);
    let (open, gate) = tokio::sync::watch::channel(false);
    let fetch = Arc::new(Gated { inner: api.clone(), sent: AtomicUsize::new(0), gate });
    let on = backend_with(fetch.clone() as Arc<dyn Fetch>, BoxBudgetsAsk::default());
    let mut calls = Vec::new();
    for _ in 0..50 {
        let (backend, until) = (on.backend.clone(), on.clock.now_ms() + mins(40));
        calls.push(tokio::spawn(async move { backend.box_backstop(ID, until, false).await }));
        // Past the slack every time, so only the call in flight can hold the next one back.
        on.clock.advance(BACKSTOP_SLACK_MS as i64 + 1_000);
        for _ in 0..20 {
            tokio::task::yield_now().await;
        }
    }
    assert_eq!(fetch.sent.load(Ordering::SeqCst), 1);
    open.send(true).unwrap();
    for call in calls {
        call.await.unwrap().unwrap();
    }
    on.backend.box_backstop(ID, on.clock.now_ms() + mins(40), false).await.unwrap();
    assert_eq!(fetch.sent.load(Ordering::SeqCst), 2);
}

/// box-backend.test.ts:1346
#[tokio::test]
async fn a_backstop_the_provider_never_answers_ends_at_its_cap_frees_the_box_and_waits_its_backoff() {
    let api = FakeBox::new().on("PATCH", &path(""), vec![Answer::Hang]);
    let on = backend_on(&api, "trial", BoxBudgetsAsk { backstop_ms: Some(100), ..Default::default() });
    let e = on.backend.box_backstop(ID, on.clock.now_ms() + mins(40), false).await.unwrap_err();
    assert_eq!(e.name(), "TimeoutError");
    on.clock.advance(10_000);
    on.backend.box_backstop(ID, on.clock.now_ms() + mins(40), false).await.unwrap();
    assert_eq!(patches(&api), 1);
    on.clock.advance(20_000);
    let e = on.backend.box_backstop(ID, on.clock.now_ms() + mins(40), false).await.unwrap_err();
    assert_eq!(e.name(), "TimeoutError");
    assert_eq!(patches(&api), 2);
}

/// box-backend.test.ts:1370
#[tokio::test]
async fn a_backstop_that_lands_resets_the_backoff_and_the_minutes_slack_still_holds_after_it() {
    let down = Arc::new(AtomicBool::new(true));
    let api = FakeBox::new().on(
        "PATCH",
        &path(""),
        vec![with({
            let down = down.clone();
            move |_| if down.load(Ordering::SeqCst) { Answer::Road(offline()) } else { idle(ID, "idle") }
        })],
    );
    let (on, _machine) = machine_on(&api, ID);
    let push = || async {
        let _ = on.backend.box_backstop(ID, on.clock.now_ms() + mins(40), false).await;
    };
    push().await;
    on.clock.advance(30_000);
    push().await;
    assert_eq!(patches(&api), 2);
    // The second failure doubled the wait.
    on.clock.advance(30_000);
    push().await;
    assert_eq!(patches(&api), 2);
    down.store(false, Ordering::SeqCst);
    on.clock.advance(30_000);
    push().await;
    assert_eq!(patches(&api), 3);
    on.clock.advance(5_000);
    push().await;
    assert_eq!(patches(&api), 3);
    // Down again past the slack: the first failure waits 30 s, not the doubled minute.
    down.store(true, Ordering::SeqCst);
    on.clock.advance(BACKSTOP_SLACK_MS as i64);
    push().await;
    on.clock.advance(30_000);
    push().await;
    assert_eq!(patches(&api), 5);
}
