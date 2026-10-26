// SPDX-License-Identifier: AGPL-3.0-only
//! Hard tests of BoxBackend's fleet roads (spec/box_fleet.md).
mod common;

use std::sync::{Arc, Mutex};

use common::fake_box::*;
use indexmap::IndexMap;
use serde_json::json;
use wsp_engine::box_backend::*;
use wsp_engine::errors::EngineError;
use wsp_engine::machine::{MachineBackend, MachineKind, MachineSpec, MachineState, ProviderBoxState, Waiting};

fn spec() -> MachineSpec {
    serde_json::from_value(json!({ "kind": "sandbox" })).unwrap()
}

fn labels(pairs: &[(&str, &str)]) -> IndexMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

#[tokio::test]
async fn a_size_the_plan_does_not_sell_is_refused_before_any_create_in_the_runtimes_words() {
    let api = FakeBox::new();
    let on = backend_on(&api, "trial", Default::default());
    let mut s = spec();
    s.cpu = Some(8.0.into());
    s.mem_mb = Some(16384.0.into());
    let e = on.backend.box_create(s, None, None).await.err().expect("refused");
    assert!(matches!(e, EngineError::Invalid { .. }), "{e:?}");
    // The sizes named are the classes the trial sells, read off /limits before the create.
    let sizes = on.backend.capabilities().sizes;
    let want = wsp_words::words::thread::refusal_line(
        &wsp_words::words::units::size_refusal("8x16", &sizes),
        wsp_words::words::units::SIZE_PICK_FIX,
    );
    assert_eq!(e.to_string(), want);
    assert_eq!(sizes.len(), 2);
    assert_eq!(api.calls(), vec!["GET /limits".to_string()]);
}

#[tokio::test]
async fn the_capabilities_list_every_class_before_the_first_read_and_the_plans_after() {
    let api = FakeBox::new();
    let on = backend_on(&api, "trial", Default::default());
    assert_eq!(on.backend.capabilities().sizes.len(), 4);
    on.backend.limits().await.unwrap();
    let names: Vec<Option<String>> = on.backend.capabilities().sizes.into_iter().map(|s| s.name).collect();
    assert_eq!(names, vec![Some("small".to_string()), Some("default".to_string())]);
}

#[tokio::test]
async fn the_person_hears_each_state_the_box_passes_through_once_in_order() {
    let api = FakeBox::new()
        .on(
            "POST",
            "/sandboxes",
            vec![Answer::Reply(Reply {
                status: 202,
                body: json!({ "status": "provisioning", "sandbox": box_view("bx_1", "provisioning", json!({ "name": "wsp" })) }),
            })],
        )
        .on(
            "GET",
            "/sandboxes/bx_1",
            vec![
                info("bx_1", "provisioning", json!({ "name": "wsp" })),
                info("bx_1", "provisioning", json!({ "name": "wsp" })),
                info("bx_1", "cloning", json!({ "name": "wsp" })),
                info("bx_1", "ready", json!({ "name": "wsp" })),
            ],
        )
        .on("POST", "/sandboxes/bx_1/commands", vec![command("", Some(0), json!({}))]);
    let on = backend_on(&api, "trial", Default::default());
    #[allow(clippy::type_complexity)]
    let heard: Arc<Mutex<Vec<(String, Option<ProviderBoxState>)>>> = Arc::default();
    let h = heard.clone();
    let waiting: Waiting = Arc::new(move |line: &str, b: Option<&ProviderBoxState>| h.lock().unwrap().push((line.to_string(), b.cloned())));
    let m = on.backend.box_create(spec(), Some(waiting), None).await.unwrap();
    assert_eq!(m.id, "bx_1");
    let states: Vec<String> = heard.lock().unwrap().iter().map(|(_, b)| b.as_ref().unwrap().state.clone()).collect();
    assert_eq!(states, vec!["provisioning", "cloning", "ready"]);
    let lines: Vec<String> = heard.lock().unwrap().iter().map(|(l, _)| l.clone()).collect();
    assert_eq!(lines[1], wsp_words::words::units::boat_box_state_line("bx_1", "cloning"));
    // A fresh create counts one start; the name already reads right, so no PATCH.
    assert!(!api.calls().iter().any(|c| c.starts_with("PATCH")));
}

#[tokio::test]
async fn a_create_with_no_key_sends_a_fresh_uuid_and_one_with_no_envs_sends_no_landing() {
    let api = FakeBox::new()
        .on(
            "POST",
            "/sandboxes",
            vec![Answer::Reply(Reply {
                status: 202,
                body: json!({ "status": "provisioning", "sandbox": box_view("bx_1", "provisioning", json!({ "name": "wsp" })) }),
            })],
        )
        .on("GET", "/sandboxes/bx_1", vec![info("bx_1", "ready", json!({ "name": "wsp" }))])
        .on("POST", "/sandboxes/bx_1/commands", vec![command("", Some(0), json!({}))]);
    let on = backend_on(&api, "trial", Default::default());
    let mut s = spec();
    s.envs = Some(IndexMap::new());
    on.backend.box_create(s, None, None).await.unwrap();
    let key = api.took("POST", "/sandboxes").unwrap().headers["idempotency-key"].clone();
    assert!(is_uuid(&key), "{key}");
    // One command: the road's proof. An empty envs map lands nothing.
    assert_eq!(api.commands_sent().len(), 1);
}

#[tokio::test]
async fn the_listing_keeps_wsps_boxes_matching_every_label_asked_with_the_size_in_mb() {
    let page = |rows: serde_json::Value| {
        Answer::Reply(Reply { status: 200, body: json!({ "sandboxes": rows, "pageInfo": { "nextCursor": null } }) })
    };
    let api = FakeBox::new().on(
        "GET",
        "/sandboxes",
        vec![
            page(json!([
                box_view("bx_a", "ready", json!({ "name": "wsp;o=h_1;w=ws_1", "vcpu": 4, "memoryGB": 8 })),
                box_view("bx_b", "ready", json!({ "name": "wsp;o=h_2", "vcpu": 2 })),
                box_view("bx_c", "ready", json!({ "name": "Box 2026" })),
            ])),
            page(json!([box_view("bx_a", "archived", json!({ "name": "wsp;o=h_1;w=ws_1" }))])),
        ],
    );
    let on = backend_on(&api, "trial", Default::default());
    let rows = on.backend.box_list(Some(&labels(&[("wsp-owner", "h_1")]))).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "bx_a");
    assert_eq!(rows[0].state, MachineState::Running, "the first listing's row of a box on both wins");
    assert_eq!(rows[0].labels, labels(&[("wsp", "1"), ("wsp-owner", "h_1"), ("wsp-workspace", "ws_1")]));
    let size = rows[0].size.as_ref().unwrap();
    assert_eq!((size.cpu.0, size.mem_mb.0), (4.0, 8192.0));
    // Every page is asked 200 at a time; the archived listing names its state.
    let gets: Vec<_> = api.seen().into_iter().filter(|s| s.path == "/sandboxes").collect();
    assert_eq!(gets.len(), 2);
    assert_eq!(gets[0].query_get("limit"), Some("200"));
    assert_eq!(gets[0].query_get("state"), None);
    assert_eq!(gets[1].query_get("state"), Some("archived"));
    assert_eq!(gets[1].query_get("limit"), Some("200"));
}

#[tokio::test]
async fn the_unfiltered_listing_keeps_every_box_wsp_named_in_listing_order() {
    let page = |rows: serde_json::Value| Answer::Reply(Reply { status: 200, body: json!({ "sandboxes": rows }) });
    let api = FakeBox::new().on(
        "GET",
        "/sandboxes",
        vec![
            page(json!([
                box_view("bx_b", "ready", json!({ "name": "wsp;o=h_2" })),
                box_view("bx_c", "ready", json!({ "name": "Box 2026" }))
            ])),
            page(json!([
                box_view("bx_a", "archived", json!({ "name": "wsp;o=h_1" })),
                box_view("bx_b", "archived", json!({ "name": "wsp;o=h_2" }))
            ])),
        ],
    );
    let on = backend_on(&api, "trial", Default::default());
    let all = on.backend.box_list(None).await.unwrap();
    let rows: Vec<(&str, MachineState)> = all.iter().map(|r| (r.id.as_str(), r.state)).collect();
    assert_eq!(rows, vec![("bx_b", MachineState::Running), ("bx_a", MachineState::Paused)]);
    let size = all[0].size.as_ref().unwrap();
    assert_eq!((size.cpu.0, size.mem_mb.0), (2.0, 4096.0));
}

#[tokio::test]
async fn asked_for_ids_that_are_no_history_id_the_snapshot_listing_reads_no_history_at_all() {
    let api = FakeBox::new().on(
        "GET",
        "/named-snapshots",
        vec![Answer::Reply(Reply { status: 200, body: json!({ "snapshots": [named("wsp-g-v1", "ready", json!({}))] }) })],
    );
    let on = backend_on(&api, "trial", Default::default());
    let rows = on.backend.box_list_snapshots(Some(&["wsp-g-v1".to_string()])).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(
        serde_json::to_value(&rows[0]).unwrap(),
        json!({ "id": "wsp-g-v1", "name": "wsp-g-v1", "sizeBytes": 1255755776u64, "createdAt": "2026-09-11T06:47:23.959Z" })
    );
    assert!(!api.calls().iter().any(|c| c.starts_with("GET /snapshots")));
}

#[tokio::test]
async fn a_named_listing_without_its_array_is_a_failure_not_an_empty_list() {
    let api = FakeBox::new().on("GET", "/named-snapshots", vec![Answer::Reply(Reply { status: 200, body: json!({ "items": [] }) })]);
    let on = backend_on(&api, "trial", Default::default());
    let e = on.backend.box_list_templates().await.unwrap_err();
    assert_eq!(e.to_string(), "GET /named-snapshots answered without a snapshots array");
}

#[tokio::test]
async fn get_reads_a_box_as_a_sandbox_and_a_paused_ones_last_save_as_its_pause() {
    let api = FakeBox::new().on(
        "GET",
        "/sandboxes/bx_1",
        vec![info("bx_1", "archived", json!({ "name": "Box 2026", "snapshotCompletedAt": "2026-09-11T07:00:00Z", "createdAt": null }))],
    );
    let on = backend_on(&api, "trial", Default::default());
    let m = on.backend.box_get("bx_1").await.unwrap();
    assert_eq!(m.kind, MachineKind::Sandbox);
    assert_eq!(m.labels, None, "a name wsp did not make carries no labels");
    let seen = m.seen.unwrap();
    assert_eq!((seen.state, seen.created_at, seen.paused_at.as_deref()), (MachineState::Paused, None, Some("2026-09-11T07:00:00Z")));
}
