// SPDX-License-Identifier: AGPL-3.0-only
//! Ports packages/engine/test/solari-backend.test.ts (lines 1-442, 527-750, 970-987) and the Solari cases of
//! packages/engine/test/provider-reads.test.ts.
#[path = "rest_common/mod.rs"]
mod common;

use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use base64::Engine as _;
use serde_json::{json, Value};

use common::{
    aggregate, fetch_failed, labels, no_road, reply, reply_text, reply_with, FakeFetch, FrozenClock, MovingClock, NoWaitClock, Play, Warned,
};
use wsp_engine::errors::{is_missing, EngineError, Result, RetryClock, RoadCause, RoadError, PROVIDER_READ_CAP_MS};
use wsp_engine::fetch::Fetch;
use wsp_engine::machine::{
    BackendPricing, ExecOptions, MachineBackend, MachineLife, MachineListRow, MachineShape, MachineSpec, MachineState, PreviewReach, Size,
    SnapshotOptions, SnapshotRow, TemplateRow, Waiting,
};
use wsp_engine::solari_backend::{
    preview_token_expiry, solari_lifecycle_budgets, MoveBudgetsGiven, SolariBackend, SolariBackendOptions, SolariPricing,
    IDLE_TIMEOUT_MAX_MS, PREVIEW_TTL_MS, REQUEST_ID_HEADER, RESUME_CAP_MS,
};
use wsp_engine::tool_sizes::BUILDER_DISK_GB;
use wsp_wire::types::JsNumber;
use wsp_wire::wire::capabilities::{Capabilities, MachineSizeOffer, PauseMode, WorkspaceSize};
use wsp_wire::wire::machine_link::TemplateRowStatus;
use wsp_words::words::thread::{image_served_wait_line, snapshot_listed_refused_line, snapshot_listed_wait_line};
use wsp_words::words::units::{offered_size, size_refusal};

fn build(f: &Arc<FakeFetch>, clock: Option<Arc<dyn RetryClock>>, warned: &Warned) -> Arc<SolariBackend> {
    let fetch: Arc<dyn Fetch> = f.clone();
    SolariBackend::new(SolariBackendOptions {
        api_key: "k".into(),
        fetch: Some(fetch),
        clock,
        warn: Some(warned.sink()),
        ..Default::default()
    })
}

fn backend(f: &Arc<FakeFetch>) -> Arc<SolariBackend> {
    build(f, None, &Warned::default())
}

fn with_clock(f: &Arc<FakeFetch>, clock: Arc<dyn RetryClock>) -> Arc<SolariBackend> {
    build(f, Some(clock), &Warned::default())
}

fn spec(v: Value) -> MachineSpec {
    serde_json::from_value(v).unwrap()
}

fn err_of<T>(r: Result<T>) -> EngineError {
    match r {
        Ok(_) => panic!("expected an error, got an answer"),
        Err(e) => e,
    }
}

fn norm(v: &Value) -> Value {
    match v {
        Value::Number(n) => match n.as_f64() {
            Some(x) if x.fract() == 0.0 && x.abs() < 9e15 => json!(x as i64),
            _ => v.clone(),
        },
        Value::Array(a) => Value::Array(a.iter().map(norm).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), norm(x))).collect()),
        _ => v.clone(),
    }
}

fn matches(actual: &Value, expected: &Value) -> bool {
    match (actual, expected) {
        (Value::Object(a), Value::Object(e)) => e.iter().all(|(k, ev)| a.get(k).is_some_and(|av| matches(av, ev))),
        _ => norm(actual) == norm(expected),
    }
}

fn assert_matches(actual: &Value, expected: Value) {
    assert!(matches(actual, &expected), "{actual} does not match {expected}");
}

fn body_of(s: &common::Seen) -> Value {
    s.body.clone().expect("the call carried a body")
}

fn bodies(f: &FakeFetch) -> Vec<Value> {
    f.calls().iter().map(body_of).collect()
}

fn has(v: &Value, key: &str) -> bool {
    v.get(key).is_some()
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64
}

fn approx(a: f64, b: f64, digits: i32) -> bool {
    (a - b).abs() < 10f64.powi(-digits) / 2.0
}

fn sandbox_created(id: &str) -> Play {
    reply(201, json!({ "sandboxId": id, "kind": "sandbox" }))
}

// ---- SolariBackend

// TS: packages/engine/test/solari-backend.test.ts:27
#[test]
fn declares_the_measured_solari_capability_truth() {
    let b = backend(&FakeFetch::new(vec![]));
    let got = b.capabilities();
    let rate = got.sizes[0].rate_usd_per_hour.0;
    assert!(approx(rate, 0.11, 10), "rate {rate}");
    let expected = Capabilities {
        live_clone_forks: true,
        pause_mode: Some(PauseMode::Memory),
        replaces_machine: true,
        preview_urls: true,
        signed_urls: true,
        callback_relay: true,
        disk_snapshots: true,
        images: true,
        snapshots_any_life: false,
        snapshot_listing: true,
        templates: true,
        kept: false,
        copies: true,
        own_network: true,
        sizes_last: true,
        sizes: vec![MachineSizeOffer { cpu: JsNumber(2.0), mem_mb: JsNumber(4096.0), rate_usd_per_hour: JsNumber(rate), name: None }],
    };
    assert_eq!(got, expected);
    let pricing = b.pricing();
    for s in &got.sizes {
        let own = pricing.rate_usd_per_hour(Size { cpu: s.cpu.0, mem_mb: s.mem_mb.0 });
        assert!(approx(s.rate_usd_per_hour.0, own, 10));
    }
    let default = pricing.default_size();
    assert_eq!((got.sizes[0].cpu.0, got.sizes[0].mem_mb.0), (default.cpu, default.mem_mb));
    assert_eq!(pricing.builder_disk_gb(), Some(20.0));
    assert_eq!(SolariPricing.builder_disk_gb(), Some(BUILDER_DISK_GB as f64));
}

// TS: packages/engine/test/solari-backend.test.ts:58
#[test]
fn offers_no_size_its_forks_have_not_come_up_at_a_2x8_ask_is_refused_naming_2x4() {
    let sizes = backend(&FakeFetch::new(vec![])).capabilities().sizes;
    assert!(!offered_size(&sizes, &WorkspaceSize { cpu: JsNumber(2.0), mem_mb: JsNumber(8192.0) }));
    assert_eq!(size_refusal("2x8", &sizes), "2x8 is not a size this provider offers; the sizes are 2x4 ($0.11/hr)");
}

// TS: packages/engine/test/solari-backend.test.ts:64
#[test]
fn declares_its_lifecycle_and_its_resume_cap_shared_by_every_backend_built_here() {
    let f = FakeFetch::new(vec![]);
    let b = backend(&f);
    let other = backend(&f);
    let lifecycle = b.lifecycle().expect("a lifecycle");
    assert_eq!(lifecycle.budgets(), solari_lifecycle_budgets());
    assert_eq!(other.lifecycle().expect("a lifecycle").budgets(), solari_lifecycle_budgets());
    assert_eq!(RESUME_CAP_MS, 30_000);
    assert_eq!(b.capabilities().pause_mode, Some(PauseMode::Memory));
}

// TS: packages/engine/test/solari-backend.test.ts:75
#[tokio::test]
async fn idle_timeout_above_six_hours_is_sent_as_six_hours() {
    let f = FakeFetch::fixed(vec![("POST /sandboxes", sandbox_created("s1"))]);
    let b = backend(&f);
    b.create(spec(json!({ "kind": "sandbox", "onIdle": "pause", "idleTimeoutMs": 12 * 60 * 60_000 })), None, None)
        .await
        .map_err(|e| e.to_string())
        .unwrap();
    assert_matches(&body_of(&f.calls()[0]), json!({ "timeoutMs": IDLE_TIMEOUT_MAX_MS }));
    b.create(spec(json!({ "kind": "sandbox", "onIdle": "pause", "idleTimeoutMs": 40 * 60_000 })), None, None)
        .await
        .map_err(|e| e.to_string())
        .unwrap();
    assert_matches(&body_of(&f.calls()[1]), json!({ "timeoutMs": 40 * 60_000 }));
    assert_eq!(IDLE_TIMEOUT_MAX_MS, 6 * 60 * 60_000);
}

// TS: packages/engine/test/solari-backend.test.ts:85
#[tokio::test]
async fn lists_every_snapshot_on_the_account_with_billed_size_and_the_name() {
    let f = FakeFetch::fixed(vec![(
        "GET /snapshots",
        reply(
            200,
            json!({ "snapshots": [
                { "id": "snap_a", "parent": null, "name": "golden", "sizeBytes": 3839352763u64, "createdAt": "2026-08-31T22:39:02.170Z", "kind": "sandbox", "template": "base" },
                { "id": "snap_b", "parent": null, "name": null, "sizeBytes": 8_500_000_000u64, "createdAt": "2026-09-04T10:00:00Z", "kind": "sandbox", "template": "base" }
            ] }),
        ),
    )]);
    let b = backend(&f);
    assert_eq!(
        b.list_snapshots(None).await.unwrap(),
        vec![
            SnapshotRow {
                id: "snap_a".into(),
                name: Some("golden".into()),
                size_bytes: JsNumber(3839352763.0),
                restored_bytes: Some(JsNumber(3839352763.0)),
                created_at: Some("2026-08-31T22:39:02.170Z".into()),
                parent: Some(None),
            },
            SnapshotRow {
                id: "snap_b".into(),
                name: None,
                size_bytes: JsNumber(8_500_000_000.0),
                restored_bytes: Some(JsNumber(8_500_000_000.0)),
                created_at: Some("2026-09-04T10:00:00Z".into()),
                parent: Some(None),
            },
        ]
    );
    assert_eq!(f.keys(), vec!["GET /snapshots"]);
}

// TS: packages/engine/test/solari-backend.test.ts:101
#[tokio::test]
async fn promotes_a_snapshot_to_a_template_and_reads_lists_and_deletes_templates() {
    let f = FakeFetch::fixed(vec![
        (
            "POST /snapshots/snap_dl8pcs2yj1fu/promote",
            reply(200, json!({ "templateId": "tpl_e6f26b64338f4eba", "name": "wsp-default-v1" })),
        ),
        (
            "GET /templates/tpl_e6f26b64338f4eba",
            reply(
                200,
                json!({ "templateId": "tpl_e6f26b64338f4eba", "name": "wsp-default-v1", "kind": "sandbox", "status": "ready", "builtin": false, "cpu": 2, "memMb": 4096, "createdAt": "2026-09-07T17:31:00Z" }),
            ),
        ),
        (
            "GET /templates/tpl_bad",
            reply(
                200,
                json!({ "templateId": "tpl_bad", "name": "x", "kind": "sandbox", "status": "failed", "builtin": false, "error": "restore copy failed" }),
            ),
        ),
        (
            "GET /templates",
            reply(
                200,
                json!({ "templates": [
                    { "templateId": "base", "name": "base", "kind": "sandbox", "status": "ready", "builtin": true, "description": "Ubuntu base" },
                    { "templateId": "tpl_e6f26b64338f4eba", "name": "wsp-default-v1", "kind": "sandbox", "status": "ready", "builtin": false, "cpu": 2, "memMb": 4096, "error": null, "createdAt": "2026-09-07T17:31:00Z" }
                ] }),
            ),
        ),
        ("DELETE /templates/tpl_e6f26b64338f4eba", reply(200, json!({ "ok": true }))),
    ]);
    let b = backend(&f);
    assert_eq!(b.promote_snapshot("snap_dl8pcs2yj1fu", "wsp-default-v1").await.unwrap(), "tpl_e6f26b64338f4eba");
    assert_eq!(body_of(&f.calls()[0]), json!({ "name": "wsp-default-v1" }));
    assert_eq!(
        b.get_template("tpl_e6f26b64338f4eba").await.unwrap(),
        TemplateRow {
            id: "tpl_e6f26b64338f4eba".into(),
            name: "wsp-default-v1".into(),
            status: TemplateRowStatus::Ready,
            error: None,
            created_at: Some("2026-09-07T17:31:00Z".into()),
        }
    );
    assert_eq!(
        b.get_template("tpl_bad").await.unwrap(),
        TemplateRow {
            id: "tpl_bad".into(),
            name: "x".into(),
            status: TemplateRowStatus::Failed,
            error: Some("restore copy failed".into()),
            created_at: None
        }
    );
    assert_eq!(
        b.list_templates().await.unwrap(),
        vec![
            TemplateRow { id: "base".into(), name: "base".into(), status: TemplateRowStatus::Ready, error: None, created_at: None },
            TemplateRow {
                id: "tpl_e6f26b64338f4eba".into(),
                name: "wsp-default-v1".into(),
                status: TemplateRowStatus::Ready,
                error: None,
                created_at: Some("2026-09-07T17:31:00Z".into()),
            },
        ]
    );
    b.delete_template("tpl_e6f26b64338f4eba").await.unwrap();
    assert_eq!(
        f.keys(),
        vec![
            "POST /snapshots/snap_dl8pcs2yj1fu/promote",
            "GET /templates/tpl_e6f26b64338f4eba",
            "GET /templates/tpl_bad",
            "GET /templates",
            "DELETE /templates/tpl_e6f26b64338f4eba",
        ]
    );
}

// TS: packages/engine/test/solari-backend.test.ts:132
#[tokio::test]
async fn a_templates_reply_of_another_shape_is_a_failure_never_an_empty_registry() {
    let b = backend(&FakeFetch::fixed(vec![("GET /templates", reply(200, json!({ "items": [] })))]));
    let e = err_of(b.list_templates().await);
    assert!(e.to_string().contains("GET /templates answered without a templates array"), "{e}");
}

// TS: packages/engine/test/solari-backend.test.ts:137
#[tokio::test]
async fn a_listing_reply_of_another_shape_is_a_failure_never_an_empty_account() {
    let b = backend(&FakeFetch::fixed(vec![("GET /snapshots", reply(200, json!({ "items": [] })))]));
    let e = err_of(b.list_snapshots(None).await);
    assert!(e.to_string().contains("GET /snapshots answered without a snapshots array"), "{e}");
}

// TS: packages/engine/test/solari-backend.test.ts:142
#[tokio::test]
async fn creates_a_sandbox_and_url_encodes_ids_on_follow_up_calls() {
    let id = "pool:vm_1:org.SIG";
    let f = FakeFetch::fixed(vec![
        ("POST /sandboxes", sandbox_created(id)),
        ("POST /sandboxes/pool%3Avm_1%3Aorg.SIG/exec", reply(200, json!({ "exitCode": 0, "stdout": "hi", "stderr": "" }))),
    ]);
    let b = backend(&f);
    let m = b.create(spec(json!({ "kind": "sandbox", "template": "base" })), None, None).await.map_err(|e| e.to_string()).unwrap();
    let r = m.exec("echo hi", ExecOptions::default()).await.unwrap();
    assert_eq!(r.stdout, "hi");
}

// TS: packages/engine/test/solari-backend.test.ts:153
#[tokio::test]
async fn maps_on_idle_and_idle_timeout_to_lifecycle_and_timeout_omitting_both_otherwise() {
    let f = FakeFetch::fixed(vec![("POST /sandboxes", reply(201, json!({ "sandboxId": "x", "kind": "desktop" })))]);
    let b = backend(&f);
    b.create(spec(json!({ "kind": "desktop", "template": "default", "onIdle": "kill", "idleTimeoutMs": 7_200_000 })), None, None)
        .await
        .map_err(|e| e.to_string())
        .unwrap();
    b.create(spec(json!({ "kind": "sandbox", "fromSnapshot": "snap_1" })), None, None).await.map_err(|e| e.to_string()).unwrap();
    let sent = bodies(&f);
    assert_matches(&sent[0], json!({ "kind": "desktop", "lifecycle": { "onTimeout": "kill" }, "timeoutMs": 7_200_000 }));
    assert!(!has(&sent[1], "lifecycle"));
    assert!(!has(&sent[1], "timeoutMs"));
}

// TS: packages/engine/test/solari-backend.test.ts:163
#[tokio::test]
async fn sends_the_disk_as_camel_case_disk_gb_when_asked_for_and_leaves_it_otherwise() {
    let f = FakeFetch::fixed(vec![("POST /sandboxes", sandbox_created("x"))]);
    let b = backend(&f);
    b.create(spec(json!({ "kind": "sandbox", "template": "base", "diskGb": 20 })), None, None).await.map_err(|e| e.to_string()).unwrap();
    b.create(spec(json!({ "kind": "sandbox", "fromSnapshot": "snap_1" })), None, None).await.map_err(|e| e.to_string()).unwrap();
    let sent = bodies(&f);
    assert_matches(&sent[0], json!({ "kind": "sandbox", "template": "base", "diskGb": 20 }));
    assert!(!has(&sent[0], "disk_gb"));
    assert!(!has(&sent[1], "diskGb"));
}

// TS: packages/engine/test/solari-backend.test.ts:228
#[tokio::test]
async fn passes_the_specs_envs_as_the_create_bodys_envs_and_sends_none_when_none() {
    let f = FakeFetch::fixed(vec![("POST /sandboxes", sandbox_created("x"))]);
    let b = backend(&f);
    b.create(
        spec(json!({ "kind": "sandbox", "fromSnapshot": "snap_1", "envs": { "HOME": "/root", "USER": "root", "PATH": "/usr/bin:/bin" } })),
        None,
        None,
    )
    .await
    .ok()
    .unwrap();
    b.create(spec(json!({ "kind": "sandbox", "fromSnapshot": "snap_1" })), None, None).await.map_err(|e| e.to_string()).unwrap();
    let sent = bodies(&f);
    assert_matches(&sent[0], json!({ "fromSnapshot": "snap_1", "envs": { "HOME": "/root", "USER": "root", "PATH": "/usr/bin:/bin" } }));
    assert!(!has(&sent[1], "envs"));
}

// TS: packages/engine/test/solari-backend.test.ts:237
#[tokio::test]
async fn reads_the_metrics_route_for_the_hosts_answer_and_a_404_there_is_the_machine_missing() {
    let f = FakeFetch::fixed(vec![
        ("POST /sandboxes", sandbox_created("x")),
        (
            "GET /sandboxes/x/metrics",
            reply(200, json!({ "cpuPct": 12.5, "memBytes": 734003200u64, "memTotalBytes": 8589934592u64, "diskBytes": 2147483648u64 })),
        ),
        ("GET /sandboxes/lost", reply(200, json!({ "sandboxId": "lost", "kind": "sandbox", "state": "running" }))),
        ("GET /sandboxes/lost/metrics", reply(404, json!({ "error": "Sandbox not found" }))),
    ]);
    let b = backend(&f);
    let m = b.create(spec(json!({ "kind": "sandbox" })), None, None).await.map_err(|e| e.to_string()).unwrap();
    assert!(m.offers().metrics);
    m.metrics().await.unwrap();
    let lost = b.get("lost").await.map_err(|e| e.to_string()).unwrap();
    assert_eq!(lost.state().await.unwrap(), MachineState::Running);
    let refused = err_of(lost.metrics().await);
    assert!(is_missing(&refused));
    assert_eq!(
        f.keys(),
        vec!["POST /sandboxes", "GET /sandboxes/x/metrics", "GET /sandboxes/lost", "GET /sandboxes/lost", "GET /sandboxes/lost/metrics"]
    );
}

// TS: packages/engine/test/solari-backend.test.ts:253
#[tokio::test]
async fn a_refused_calls_error_carries_the_request_id_the_replys_header_named() {
    let f = FakeFetch::fixed(vec![
        ("POST /sandboxes", sandbox_created("vm_1")),
        (
            "POST /sandboxes/vm_1/snapshots",
            reply_with(502, json!({ "error": "Failed to snapshot sandbox" }), &[(REQUEST_ID_HEADER, "req_42")]),
        ),
    ]);
    let b = backend(&f);
    let m = b.create(spec(json!({ "kind": "sandbox", "template": "base" })), None, None).await.map_err(|e| e.to_string()).unwrap();
    let e = err_of(m.snapshot("g", MachineLife { first_life: true }, SnapshotOptions::default()).await);
    assert_eq!(e.kind(), Some(wsp_engine::errors::ErrorKind::SnapshotUnavailable));
    assert_eq!(e.status(), Some(502));
    assert_eq!(e.request_id(), Some("req_42"));
}

// TS: packages/engine/test/solari-backend.test.ts:264
#[tokio::test]
async fn a_snapshot_of_a_machine_not_in_its_first_life_is_refused_before_any_call() {
    let f = FakeFetch::fixed(vec![
        ("GET /sandboxes/x", reply(200, json!({ "sandboxId": "x", "kind": "sandbox", "state": "running" }))),
        ("POST /sandboxes/x/snapshots", reply(200, json!({ "snapshotId": "snap_1" }))),
    ]);
    let b = backend(&f);
    let m = b.get("x").await.map_err(|e| e.to_string()).unwrap();
    let sent = f.calls().len();
    let err = err_of(m.snapshot("v2", MachineLife { first_life: false }, SnapshotOptions::default()).await);
    match &err {
        EngineError::NotFirstLife { machine_id, .. } => assert_eq!(machine_id, "x"),
        other => panic!("expected NotFirstLife, got {other:?}"),
    }
    assert!(err.to_string().starts_with("snapshot v2 refused: machine x is not first-life"), "{err}");
    assert_eq!(f.calls().len(), sent);
    assert_eq!(m.snapshot("v2", MachineLife { first_life: true }, SnapshotOptions::default()).await.unwrap(), "snap_1");
    assert_eq!(f.calls().len(), sent + 1);
}

// TS: packages/engine/test/solari-backend.test.ts:283
#[tokio::test]
async fn surfaces_snapshot_unavailable_without_retrying() {
    let f = FakeFetch::fixed(vec![
        ("POST /sandboxes", sandbox_created("x")),
        ("POST /sandboxes/x/snapshots", reply(502, json!({ "error": "Failed to snapshot sandbox" }))),
    ]);
    let b = backend(&f);
    let m = b.create(spec(json!({ "kind": "sandbox" })), None, None).await.map_err(|e| e.to_string()).unwrap();
    let e = err_of(m.snapshot("g", MachineLife { first_life: true }, SnapshotOptions::default()).await);
    assert_eq!(e.kind(), Some(wsp_engine::errors::ErrorKind::SnapshotUnavailable));
    assert_eq!(f.calls().iter().filter(|c| c.url.contains("/snapshots")).count(), 1);
}

// ---- previewTokenExpiry

/// Base64url claims, a dot, a signature: not a 3-part JWT, the sandboxId claim embeds literal dots, exp is epoch ms.
fn mint_token(exp: i64) -> String {
    let claims = json!({
        "sandboxId": "desktop-pool-i-0fd9ed7dc03a79db2:vm_001130:cmthqj8lg.TIblxI9qSig",
        "port": 7070,
        "orgId": "cmthqj8lg00sso001svwbxyxb",
        "exp": exp,
    });
    format!("{}.WO_khRk6fakeSignature", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(claims.to_string()))
}

// TS: packages/engine/test/solari-backend.test.ts:309
#[test]
fn reads_the_ms_exp_claim_from_the_real_token_shape() {
    let now = now_ms();
    let exp = now + 60 * 60_000;
    assert_eq!(preview_token_expiry(&mint_token(exp), now), exp);
}

// TS: packages/engine/test/solari-backend.test.ts:314
#[test]
fn falls_back_to_the_measured_60_min_ttl_when_the_token_is_opaque() {
    let now = now_ms();
    assert_eq!(preview_token_expiry("not-a-token-at-all", now), now + PREVIEW_TTL_MS);
    assert_eq!(PREVIEW_TTL_MS, 60 * 60_000);
}

// ---- SolariMachine.previewUrl

// TS: packages/engine/test/solari-backend.test.ts:322
#[tokio::test]
async fn mints_via_the_ports_route_and_derives_expires_at_from_the_token() {
    let id = "pool:vm_1:org.SIG";
    let exp = now_ms() + 60 * 60_000;
    let token = mint_token(exp);
    let url = format!("https://3fe6a8b705a72d14c7cc-7070.preview.getsolari.com?pt_token={token}");
    let f = FakeFetch::fixed(vec![
        ("POST /sandboxes", sandbox_created(id)),
        ("GET /sandboxes/pool%3Avm_1%3Aorg.SIG/ports/7070", reply(200, json!({ "url": url, "token": token }))),
    ]);
    let b = backend(&f);
    let m = b.create(spec(json!({ "kind": "sandbox" })), None, None).await.map_err(|e| e.to_string()).unwrap();
    assert!(m.offers().preview_url, "SolariMachine must support previewUrl");
    assert_eq!(m.preview_url(7070).await.unwrap(), PreviewReach { url, token, expires_at: JsNumber(exp as f64) });
}

// ---- describe

// TS: packages/engine/test/solari-backend.test.ts:341
#[tokio::test]
async fn reads_the_providers_size_and_creation_time_off_the_machine_route() {
    let f = FakeFetch::fixed(vec![
        ("POST /sandboxes", sandbox_created("x")),
        (
            "GET /sandboxes/x",
            reply(
                200,
                json!({ "sandboxId": "x", "kind": "sandbox", "state": "running", "cpu": 2, "memMb": 2048, "diskGb": 20, "createdAt": "2026-09-02T19:03:35Z" }),
            ),
        ),
    ]);
    let b = backend(&f);
    let m = b.create(spec(json!({ "kind": "sandbox", "memMb": 4096, "diskGb": 20 })), None, None).await.map_err(|e| e.to_string()).unwrap();
    assert_eq!(
        m.describe().await.unwrap(),
        MachineShape {
            cpu: Some(JsNumber(2.0)),
            mem_mb: Some(JsNumber(2048.0)),
            disk_gb: Some(JsNumber(20.0)),
            created_at: Some("2026-09-02T19:03:35Z".into()),
            used_bytes: None,
        }
    );
}

// ---- list

// TS: packages/engine/test/solari-backend.test.ts:353
#[tokio::test]
async fn list_reads_labels_and_size_per_row_follows_the_cursor_and_filters_by_label() {
    let f = FakeFetch::handled(|seen| {
        let cursor = url::form_urlencoded::parse(seen.query.as_bytes())
            .find(|(k, _)| k == "cursor")
            .map(|(_, v)| v.into_owned())
            .unwrap_or_default();
        let page = match cursor.as_str() {
            "" => json!({
                "sandboxes": [
                    { "sandboxId": "a", "kind": "sandbox", "state": "running", "metadata": { "wsp": "1" }, "cpu": 2, "memMb": 4096 },
                    { "sandboxId": "b", "kind": "sandbox", "state": "paused" }
                ],
                "nextCursor": "c2"
            }),
            _ => {
                json!({ "sandboxes": [{ "sandboxId": "c", "kind": "desktop", "state": "archived", "metadata": { "poc": "p1" }, "cpu": 4 }] })
            }
        };
        Some(reply(200, page))
    });
    let b = backend(&f);
    let rows = b.list(Some(&labels(&[("wsp", "1")]))).await.unwrap();
    assert_eq!(
        rows,
        vec![
            MachineListRow {
                id: "a".into(),
                state: MachineState::Running,
                labels: labels(&[("wsp", "1")]),
                size: Some(WorkspaceSize { cpu: JsNumber(2.0), mem_mb: JsNumber(4096.0) }),
            },
            MachineListRow { id: "b".into(), state: MachineState::Paused, labels: labels(&[]), size: None },
            MachineListRow { id: "c".into(), state: MachineState::Gone, labels: labels(&[("poc", "p1")]), size: None },
        ]
    );
    let calls = f.calls();
    assert_eq!(calls.len(), 2);
    for c in &calls {
        assert_eq!(c.path, "/sandboxes");
        let wsp = url::form_urlencoded::parse(c.query.as_bytes()).find(|(k, _)| k == "metadata.wsp").map(|(_, v)| v.into_owned());
        assert_eq!(wsp.as_deref(), Some("1"));
    }
}

// ---- create from a snapshot the provider answers not found

/// POST /sandboxes refuses `refusals` times, then boots; the listing holds `listed`. Returns the fetch and the
/// Idempotency-Key of each create call.
fn forking(refusals: usize, listed: &[&str]) -> (Arc<FakeFetch>, Arc<Mutex<Vec<String>>>) {
    let keys: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let keys_in = keys.clone();
    let listed: Vec<String> = listed.iter().map(|s| s.to_string()).collect();
    let f = FakeFetch::handled(move |seen| {
        if seen.path == "/snapshots" {
            let rows: Vec<Value> = listed.iter().map(|id| json!({ "id": id, "sizeBytes": 1 })).collect();
            return Some(reply(200, json!({ "snapshots": rows })));
        }
        let mut k = keys_in.lock().unwrap();
        k.push(seen.header("Idempotency-Key").unwrap_or_default());
        if k.len() <= refusals {
            Some(reply(404, json!({ "error": "Snapshot not found" })))
        } else {
            Some(sandbox_created("sbx_1"))
        }
    });
    (f, keys)
}

fn fork_spec() -> MachineSpec {
    spec(json!({ "kind": "sandbox", "fromSnapshot": "snap_1", "idempotencyKey": "ws/1:k" }))
}

// TS: packages/engine/test/solari-backend.test.ts:401
#[tokio::test]
async fn asks_again_under_a_fresh_key_while_the_listing_holds_the_snapshot_and_says_each_wait() {
    let (f, keys) = forking(2, &["snap_1"]);
    let clock = MovingClock::new();
    let warned = Warned::default();
    let b = build(&f, Some(clock.clone()), &warned);
    let said: Arc<Mutex<Vec<(String, bool)>>> = Arc::new(Mutex::new(Vec::new()));
    let said_in = said.clone();
    let waiting: Waiting = Arc::new(move |line, provider_box| said_in.lock().unwrap().push((line.to_string(), provider_box.is_some())));
    let m = b.create(fork_spec(), Some(waiting), None).await.map_err(|e| e.to_string()).unwrap();
    assert_eq!(m.id(), "sbx_1");
    let keys = keys.lock().unwrap().clone();
    assert_eq!(keys[0], "ws/1:k");
    assert_eq!(keys.iter().collect::<HashSet<_>>().len(), 3);
    let every = clock.waits()[0] as f64;
    assert_eq!(warned.lines(), vec![snapshot_listed_wait_line("snap_1", every, 0.0), snapshot_listed_wait_line("snap_1", every, every)]);
    assert_eq!(
        said.lock().unwrap().clone(),
        vec![(image_served_wait_line("Solari", every), false), (image_served_wait_line("Solari", every), false)]
    );
}

// TS: packages/engine/test/solari-backend.test.ts:420
#[tokio::test]
async fn is_missing_at_once_where_the_listing_does_not_hold_the_snapshot() {
    let (f, keys) = forking(1, &["snap_other"]);
    let b = with_clock(&f, MovingClock::new());
    let e = err_of(b.create(fork_spec(), None, None).await);
    assert!(is_missing(&e));
    assert_eq!(e.to_string(), "Snapshot not found");
    assert_eq!(e.status(), Some(404));
    assert_eq!(keys.lock().unwrap().len(), 1);
}

// TS: packages/engine/test/solari-backend.test.ts:428
#[tokio::test]
async fn is_not_missing_where_the_listing_still_holds_the_snapshot_once_the_wait_runs_out() {
    let (f, keys) = forking(usize::MAX, &["snap_1"]);
    let clock = MovingClock::new();
    let b = build(&f, Some(clock.clone()), &Warned::default());
    let e = err_of(b.create(fork_spec(), None, None).await);
    assert!(!is_missing(&e));
    assert_eq!(e.to_string(), snapshot_listed_refused_line("snap_1", clock.now() as f64));
    assert_eq!(e.status(), Some(404));
    let sent = keys.lock().unwrap().len();
    assert_eq!(sent, clock.waits().len() + 1);
    assert!(sent > 2);
}

// ---- idempotency

fn header_of(s: &common::Seen) -> Option<String> {
    s.header("Idempotency-Key")
}

// TS: packages/engine/test/solari-backend.test.ts:530
#[tokio::test]
async fn sends_the_specs_key_on_create_and_nothing_on_the_posts_the_provider_ignores_it_for() {
    let f = FakeFetch::fixed(vec![
        ("POST /sandboxes", sandbox_created("x")),
        ("POST /sandboxes/x/exec", reply(200, json!({ "exitCode": 0, "stdout": "", "stderr": "" }))),
        ("POST /sandboxes/x/pause", reply(200, json!({}))),
    ]);
    let b = backend(&f);
    let m = b
        .create(spec(json!({ "kind": "sandbox", "template": "base", "idempotencyKey": "workspace/ws_1:a1" })), None, None)
        .await
        .map_err(|e| e.to_string())
        .unwrap();
    m.exec("true", ExecOptions::default()).await.unwrap();
    m.pause(None).await.unwrap();
    let headers: Vec<Option<String>> = f.calls().iter().map(header_of).collect();
    assert_eq!(headers, vec![Some("workspace/ws_1:a1".to_string()), None, None]);
}

// TS: packages/engine/test/solari-backend.test.ts:543
#[tokio::test]
async fn holds_one_key_across_the_retries_of_a_create_and_mints_one_when_the_caller_sent_none() {
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_in = calls.clone();
    let f = FakeFetch::handled(move |_| {
        if calls_in.fetch_add(1, Ordering::SeqCst) == 0 {
            Some(reply(503, json!({ "error": "upstream" })))
        } else {
            Some(sandbox_created("x"))
        }
    });
    let b = backend(&f);
    b.create(spec(json!({ "kind": "sandbox", "template": "base", "idempotencyKey": "workspace/ws_1:a1" })), None, None)
        .await
        .map_err(|e| e.to_string())
        .unwrap();
    let held: Vec<Option<String>> = f.calls().iter().map(header_of).collect();
    assert_eq!(held, vec![Some("workspace/ws_1:a1".to_string()), Some("workspace/ws_1:a1".to_string())]);
    f.calls.lock().unwrap().clear();
    b.create(spec(json!({ "kind": "sandbox", "template": "base" })), None, None).await.map_err(|e| e.to_string()).unwrap();
    b.create(spec(json!({ "kind": "sandbox", "template": "base" })), None, None).await.map_err(|e| e.to_string()).unwrap();
    let minted: Vec<Option<String>> = f.calls().iter().map(header_of).collect();
    assert_eq!(minted.len(), 2);
    assert!(minted.iter().all(|k| k.as_ref().is_some_and(|k| !k.is_empty())));
    assert_ne!(minted[0], minted[1]);
}

// TS: packages/engine/test/solari-backend.test.ts:562
#[tokio::test]
async fn retries_a_create_once_under_the_same_key_when_the_fetch_itself_throws() {
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_in = calls.clone();
    let f = FakeFetch::handled(move |_| {
        if calls_in.fetch_add(1, Ordering::SeqCst) == 0 {
            Some(Play::Fail(fetch_failed()))
        } else {
            Some(reply_with(201, json!({ "sandboxId": "x", "kind": "sandbox" }), &[("Idempotent-Replayed", "true")]))
        }
    });
    let b = backend(&f);
    let m = b
        .create(spec(json!({ "kind": "sandbox", "template": "base", "idempotencyKey": "workspace/ws_1:a1" })), None, None)
        .await
        .map_err(|e| e.to_string())
        .unwrap();
    let headers: Vec<Option<String>> = f.calls().iter().map(header_of).collect();
    assert_eq!(headers, vec![Some("workspace/ws_1:a1".to_string()), Some("workspace/ws_1:a1".to_string())]);
    assert!(m.replayed());
    assert_eq!(m.id(), "x");
}

// TS: packages/engine/test/solari-backend.test.ts:575
#[tokio::test]
async fn lets_a_second_thrown_fetch_propagate_and_never_retries_a_thrown_exec() {
    let f = FakeFetch::handled(|_| Some(Play::Fail(fetch_failed())));
    let b = backend(&f);
    let e =
        err_of(b.create(spec(json!({ "kind": "sandbox", "template": "base", "idempotencyKey": "workspace/ws_1:a1" })), None, None).await);
    assert!(e.to_string().contains("fetch failed"), "{e}");
    assert_eq!(f.calls().len(), 2);
    f.calls.lock().unwrap().clear();
    let e = err_of(b.request::<Value>("POST", "/sandboxes/x/exec", Some(json!({ "cmd": "true" })), None, None).await);
    assert!(e.to_string().contains("fetch failed"), "{e}");
    assert_eq!(f.calls().len(), 1);
}

// TS: packages/engine/test/solari-backend.test.ts:587
#[tokio::test]
async fn reads_idempotent_replayed_off_the_create_reply_onto_the_handle() {
    let replays = Arc::new(AtomicUsize::new(0));
    let replays_in = replays.clone();
    let f = FakeFetch::handled(move |_| {
        if replays_in.fetch_add(1, Ordering::SeqCst) == 0 {
            Some(sandbox_created("x"))
        } else {
            Some(reply_with(201, json!({ "sandboxId": "x", "kind": "sandbox" }), &[("Idempotent-Replayed", "true")]))
        }
    });
    let b = backend(&f);
    let make = || spec(json!({ "kind": "sandbox", "template": "base", "idempotencyKey": "workspace/ws_1:a1" }));
    let first = b.create(make(), None, None).await.map_err(|e| e.to_string()).unwrap();
    let second = b.create(make(), None, None).await.map_err(|e| e.to_string()).unwrap();
    assert!(!first.replayed());
    assert!(second.replayed());
    assert_eq!(second.id(), first.id());
}

// ---- road retries

fn road_failing(code: &str) -> RoadError {
    RoadError {
        name: "TypeError".into(),
        message: "fetch failed".into(),
        code: None,
        cause: Some(RoadCause { message: format!("connect {code}"), code: Some(code.into()), errors: None }),
    }
}

fn sandbox_view() -> Value {
    json!({ "sandboxId": "x", "kind": "sandbox", "state": "running" })
}

// TS: packages/engine/test/solari-backend.test.ts:615
#[tokio::test]
async fn tries_a_call_that_never_left_this_computer_three_times_and_logs_one_line_per_retry() {
    let clock = NoWaitClock::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_in = calls.clone();
    let f = FakeFetch::handled(move |_| {
        if calls_in.fetch_add(1, Ordering::SeqCst) < 2 {
            Some(Play::Fail(no_road("ENOTFOUND")))
        } else {
            Some(reply(200, json!({ "snapshotId": "snap_1" })))
        }
    });
    let warned = Warned::default();
    let b = build(&f, Some(clock.clone()), &warned);
    let got: Value = b.request("POST", "/sandboxes/x/snapshots", Some(json!({ "name": "g" })), None, None).await.unwrap();
    assert_eq!(got, json!({ "snapshotId": "snap_1" }));
    assert_eq!(f.calls().len(), 3);
    assert_eq!(
        warned.lines(),
        vec![
            "POST /sandboxes/x/snapshots did not leave this computer (ENOTFOUND); try 2 of 3",
            "POST /sandboxes/x/snapshots did not leave this computer (ENOTFOUND); try 3 of 3",
        ]
    );
    let waits = clock.waits();
    assert_eq!(waits.len(), 2);
    let span: u64 = waits.iter().sum();
    assert!(span >= 9_000, "span {span}");
    assert!(span < 11_000, "span {span}");
}

// TS: packages/engine/test/solari-backend.test.ts:640
#[tokio::test]
async fn gives_up_after_the_third_try_and_throws_what_the_road_threw() {
    let f = FakeFetch::handled(|_| Some(Play::Fail(no_road("EAI_AGAIN"))));
    let b = with_clock(&f, NoWaitClock::new());
    let e = err_of(b.request::<Value>("GET", "/sandboxes/x", None, None, None).await);
    assert!(e.to_string().contains("fetch failed"), "{e}");
    assert_eq!(f.calls().len(), 3);
}

// TS: packages/engine/test/solari-backend.test.ts:653
#[tokio::test]
async fn reads_a_road_out_of_an_aggregate_error_one_error_per_address_tried() {
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_in = calls.clone();
    let f = FakeFetch::handled(move |_| {
        if calls_in.fetch_add(1, Ordering::SeqCst) == 0 {
            Some(Play::Fail(aggregate(&["ENETUNREACH", "ENETUNREACH"])))
        } else {
            Some(reply(200, sandbox_view()))
        }
    });
    let warned = Warned::default();
    let b = build(&f, Some(NoWaitClock::new()), &warned);
    let m = b.get("x").await.map_err(|e| e.to_string()).unwrap();
    assert_eq!(m.id(), "x");
    assert_eq!(warned.lines(), vec!["GET /sandboxes/x did not leave this computer (ENETUNREACH); try 2 of 3"]);
}

// TS: packages/engine/test/solari-backend.test.ts:677
#[tokio::test]
async fn a_road_that_flapped_before_the_first_answer_leaves_the_gateway_retries_whole() {
    let answers = [502u16, 502, 200];
    let calls = Arc::new(AtomicUsize::new(0));
    let calls_in = calls.clone();
    let f = FakeFetch::handled(move |_| {
        let n = calls_in.fetch_add(1, Ordering::SeqCst) + 1;
        if n == 1 {
            return Some(Play::Fail(no_road("ENOTFOUND")));
        }
        let status = answers[n - 2];
        let body = if status == 200 { sandbox_view() } else { json!({ "error": "upstream sad" }) };
        Some(reply(status, body))
    });
    let b = with_clock(&f, NoWaitClock::new());
    let m = b.get("x").await.map_err(|e| e.to_string()).unwrap();
    assert_eq!(m.id(), "x");
    assert_eq!(f.calls().len(), 4);
}

// TS: packages/engine/test/solari-backend.test.ts:697
#[tokio::test]
async fn never_retries_an_answer_a_409_is_the_callers_on_the_first_reply() {
    let f = FakeFetch::fixed(vec![("POST /sandboxes/x/pause", reply(409, json!({ "error": "sandbox is not running" })))]);
    let b = with_clock(&f, NoWaitClock::new());
    let e = err_of(b.request::<Value>("POST", "/sandboxes/x/pause", Some(json!({})), None, None).await);
    assert_eq!(e.kind(), Some(wsp_engine::errors::ErrorKind::Conflict));
    assert_eq!(e.status(), Some(409));
    assert_eq!(f.calls().len(), 1);
}

// TS: packages/engine/test/solari-backend.test.ts:705
#[tokio::test]
async fn never_retries_a_refused_reset_or_timed_out_connection() {
    for code in ["ECONNREFUSED", "ECONNRESET", "ETIMEDOUT"] {
        let f = FakeFetch::handled(move |_| Some(Play::Fail(road_failing(code))));
        let b = with_clock(&f, NoWaitClock::new());
        let e = err_of(b.request::<Value>("GET", "/sandboxes/x", None, None, None).await);
        assert!(e.to_string().contains("fetch failed"), "{code}: {e}");
        assert_eq!(f.calls().len(), 1, "{code}");
    }
}

// TS: packages/engine/test/solari-backend.test.ts:717
#[tokio::test]
async fn every_road_takes_it_create_snapshot_pause_poll_and_promote_survive_one_dropped_lookup() {
    let routes: Vec<(&'static str, Value)> = vec![
        ("POST /sandboxes", json!({ "sandboxId": "x", "kind": "sandbox" })),
        ("POST /sandboxes/x/snapshots", json!({ "snapshotId": "snap_1" })),
        ("POST /sandboxes/x/pause", json!({})),
        ("GET /sandboxes/x", sandbox_view()),
        ("POST /snapshots/snap_1/promote", json!({ "templateId": "tpl_1" })),
    ];
    let dropped: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    let dropped_in = dropped.clone();
    let f = FakeFetch::handled(move |seen| {
        let key = seen.key();
        if dropped_in.lock().unwrap().insert(key.clone()) {
            return Some(Play::Fail(no_road("ENOTFOUND")));
        }
        let body = routes.iter().find(|(k, _)| *k == key).map(|(_, v)| v.clone()).unwrap_or(Value::Null);
        Some(reply(200, body))
    });
    let warned = Warned::default();
    let b = build(&f, Some(NoWaitClock::new()), &warned);
    let m = b.create(spec(json!({ "kind": "sandbox", "template": "base" })), None, None).await.map_err(|e| e.to_string()).unwrap();
    assert_eq!(m.snapshot("g", MachineLife { first_life: true }, SnapshotOptions::default()).await.unwrap(), "snap_1");
    m.pause(None).await.unwrap();
    assert_eq!(m.state().await.unwrap(), MachineState::Running);
    assert_eq!(b.promote_snapshot("snap_1", "wsp-default-v1").await.unwrap(), "tpl_1");
    assert_eq!(dropped.lock().unwrap().len(), 5);
    assert_eq!(f.calls().len(), 10);
    assert_eq!(warned.lines().len(), 5);
}

// ---- error bodies

// TS: packages/engine/test/solari-backend.test.ts:971
#[tokio::test]
async fn keeps_a_plain_text_error_body_as_the_refusals_words_instead_of_an_empty_one() {
    let f = FakeFetch::fixed(vec![(
        "POST /sandboxes",
        Play::Reply(500, "Internal Server Error".into(), vec![("content-type".into(), "text/plain".into())]),
    )]);
    let b = with_clock(&f, Arc::new(FrozenClock));
    let refused = err_of(b.create(spec(json!({ "kind": "sandbox" })), None, None).await);
    assert_eq!(refused.status(), Some(500));
    assert_eq!(refused.to_string(), "Internal Server Error");
}

// TS: packages/engine/test/solari-backend.test.ts:980
#[tokio::test]
async fn names_the_class_and_status_where_the_provider_said_nothing_at_all() {
    let f = FakeFetch::fixed(vec![("POST /sandboxes", reply_text(500, ""))]);
    let b = with_clock(&f, Arc::new(FrozenClock));
    let refused = err_of(b.create(spec(json!({ "kind": "sandbox" })), None, None).await);
    let message = refused.to_string();
    assert_ne!(message, "");
    assert!(message.contains("500"), "{message}");
}

// ---- provider-reads.test.ts: a provider that never answers a read

// TS: packages/engine/test/provider-reads.test.ts:23
#[test]
fn reads_at_most_the_measured_default_by_default() {
    let b = SolariBackend::new(SolariBackendOptions { api_key: "k".into(), ..Default::default() });
    assert_eq!(b.budgets().read_ms, PROVIDER_READ_CAP_MS as f64);
}

// TS: packages/engine/test/provider-reads.test.ts:36
#[tokio::test]
async fn ends_each_of_solaris_reads_of_a_machine_the_fleet_and_the_snapshots_at_the_reads_cap() {
    let f =
        FakeFetch::fixed(vec![("GET /sandboxes/sb_acme", Play::Hangs), ("GET /sandboxes", Play::Hangs), ("GET /snapshots", Play::Hangs)]);
    let fetch: Arc<dyn Fetch> = f.clone();
    let solari = SolariBackend::new(SolariBackendOptions {
        api_key: "k".into(),
        fetch: Some(fetch),
        budgets: MoveBudgetsGiven { read_ms: Some(150.0), ..Default::default() },
        ..Default::default()
    });
    assert!(solari.get("sb_acme").await.is_err());
    assert!(solari.list(None).await.is_err());
    assert!(solari.list_snapshots(None).await.is_err());
    let asked: Vec<(String, String, bool)> = f.calls().iter().map(|c| (c.method.clone(), c.path.clone(), c.had_abort)).collect();
    assert_eq!(
        asked,
        vec![
            ("GET".to_string(), "/sandboxes/sb_acme".to_string(), true),
            ("GET".to_string(), "/sandboxes".to_string(), true),
            ("GET".to_string(), "/snapshots".to_string(), true),
        ]
    );
}
