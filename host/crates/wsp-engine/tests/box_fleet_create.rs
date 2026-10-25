// SPDX-License-Identifier: AGPL-3.0-only
//! The create half of "BoxBackend against a fake Box API" in packages/engine/test/box-backend.test.ts: create, its
//! request and timer, the replay flag, the environment landing during a create, a create that is cancelled or ends
//! in error, the refusals a create reads, the key check, and the healthy create that proves the road. Bodies are the
//! ones the live spike recorded on 2026-09-11; nothing here dials the real API.
mod common;

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_engine::box_backend::{
    env_landing_script, sudo_command, BoxBackend, BoxBackendOptions, BoxBudgetsAsk, BOX_BASE_TEMPLATE, TRIAL_TTL_S,
};
use wsp_engine::errors::{EngineError, ErrorKind, WspError};
use wsp_engine::fetch::Fetch;
use wsp_engine::machine::{MachineBackend, MachineKind, MachineSpec, ProviderBoxState, Waiting};
use wsp_wire::wire::machine_link::MachineSpecOnIdle;
use wsp_wire::types::JsNumber;
use wsp_wire::wire::daemon::EXEC_DEADLINE_EXIT;

use common::fake_box::{backend_on, box_view, command, error, info, inner, FakeBox, Answer, Reply};

// exec-detached.ts:21 DEADLINE_EXIT is protocol's EXEC_DEADLINE_EXIT.
const DEADLINE_EXIT: i64 = EXEC_DEADLINE_EXIT;
// exec-detached.ts:18
const INLINE_EXEC_MS: u64 = 20_000;
// exec-detached.ts:50
const DAEMON_ENV_FILE: &str = "/etc/wsp/daemon.env";
// labels.ts
const WSP_LABEL: &str = "wsp";
const OWNER_LABEL: &str = "wsp-owner";

fn n(v: f64) -> JsNumber {
    JsNumber(v)
}

fn map(pairs: &[(&str, &str)]) -> IndexMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn spec() -> MachineSpec {
    MachineSpec {
        kind: MachineKind::Sandbox,
        template: None,
        from_snapshot: None,
        cpu: None,
        mem_mb: None,
        disk_gb: None,
        envs: None,
        labels: None,
        on_idle: None,
        idle_timeout_ms: None,
        idempotency_key: None,
        engine: None,
        copy: None,
        shares: None,
        binds: None,
    }
}

fn reply(status: u16, body: Value) -> Answer {
    Answer::Reply(Reply { status, body })
}

fn refuse(status: u16, code: &str, message: &str) -> Answer {
    error(status, code, message).into()
}

fn cmd(stdout: &str) -> Answer {
    command(stdout, Some(0), json!({}))
}

/// box-backend.test.ts:143 `detachedGuest`: the commands endpoint answering as a guest that ran a detached script:
/// the launch confirms, the first poll carries the exit code and the streams, and the cleanup says nothing.
fn detached_guest(exit_code: i64, out: &str, err: &str) -> Answer {
    let (out, err) = (STANDARD.encode(out), STANDARD.encode(err));
    common::fake_box::with(move |seen| {
        let command_line = seen.command();
        if command_line.contains("WSP_LAUNCHED") {
            return cmd("WSP_LAUNCHED\n");
        }
        if command_line.contains("WSP_POLL") {
            return cmd(&format!("WSP_POLL\n{exit_code}\n{out}\n{err}\ndown\nWSP_POLL_END\n"));
        }
        cmd("")
    })
}

fn detached_ok() -> Answer {
    detached_guest(0, "", "")
}

/// box-backend.test.ts:139 `launchedText`: the text of the file one detached launch puts on the guest, the base64
/// its own exec carries.
fn launched_text(command_line: &str) -> String {
    let script = inner(command_line);
    let start = script.find("printf %s '").expect("a launch carries its file as printf %s '<base64>'") + "printf %s '".len();
    let end = start + script[start..].find('\'').expect("the base64 ends at a quote");
    String::from_utf8(STANDARD.decode(&script[start..end]).expect("base64")).expect("UTF-8")
}

fn backend(api: &Arc<FakeBox>, tier: &str) -> BoxBackend {
    backend_on(api, tier, BoxBudgetsAsk::default()).backend
}

fn failed<T>(r: Result<T, EngineError>) -> EngineError {
    match r {
        Ok(_) => panic!("expected the call to fail"),
        Err(e) => e,
    }
}

fn provider(e: &EngineError) -> &WspError {
    match e {
        EngineError::Provider(w) => w,
        other => panic!("expected the provider's refusal, got {other:?}"),
    }
}

fn posts(api: &FakeBox, path: &str) -> Vec<common::fake_box::Seen> {
    api.seen().into_iter().filter(|s| s.method == "POST" && s.path == path).collect()
}

fn count(api: &FakeBox, call: &str) -> usize {
    api.calls().iter().filter(|c| *c == call).count()
}

fn deletes(api: &FakeBox) -> Vec<String> {
    api.calls().into_iter().filter(|c| c.starts_with("DELETE")).collect()
}

/// box-backend.test.ts:294
#[tokio::test]
async fn creates_from_the_stock_image_on_the_trial_class_two_hour_stop_no_secrets_labels_in_the_name_key_on_the_header_ready_before_the_handle() {
    let api = FakeBox::new()
        .on("POST", "/sandboxes", vec![reply(202, json!({ "ok": true, "type": "sandbox.created", "status": "provisioning", "ttlSeconds": 7200, "sandbox": box_view("bx_tumrjngm", "provisioning", json!({})) }))])
        .on(
            "GET",
            "/sandboxes/bx_tumrjngm",
            vec![info("bx_tumrjngm", "provisioning", json!({ "name": "wsp;o=h_1a2b3c4d" })), info("bx_tumrjngm", "ready", json!({ "name": "wsp;o=h_1a2b3c4d" }))],
        )
        .on("POST", "/sandboxes/bx_tumrjngm/commands", vec![detached_ok()]);
    let backend = backend(&api, "trial");
    let machine = backend
        .create(
            MachineSpec {
                template: Some(BOX_BASE_TEMPLATE.to_string()),
                cpu: Some(n(2.0)),
                mem_mb: Some(n(4096.0)),
                labels: Some(map(&[(WSP_LABEL, "1"), (OWNER_LABEL, "h_1a2b3c4d")])),
                envs: Some(map(&[("IS_SANDBOX", "1"), ("ANTHROPIC_API_KEY", "sk-ant-x")])),
                idempotency_key: Some("wsp-golden-default-1".to_string()),
                ..spec()
            },
            None,
            None,
        )
        .await
        .unwrap();
    assert_eq!(machine.id(), "bx_tumrjngm");
    assert!(!machine.replayed());
    assert_eq!(machine.stream_url(), None);
    let create = api.took("POST", "/sandboxes").unwrap();
    assert_eq!(create.body, json!({ "type": "small", "ttlSeconds": TRIAL_TTL_S, "noEnv": true, "name": "wsp;o=h_1a2b3c4d" }));
    assert_eq!(create.headers.get("idempotency-key").map(String::as_str), Some("wsp-golden-default-1"));
    // The environment lands where root's services read it, carried onto the guest as the launched script's own file.
    let landed = launched_text(&api.commands_sent().into_iter().find(|c| c.contains("WSP_LAUNCHED")).unwrap());
    assert_eq!(landed, env_landing_script(&map(&[("IS_SANDBOX", "1"), ("ANTHROPIC_API_KEY", "sk-ant-x")])));
    assert!(landed.contains(&format!("cat > '{DAEMON_ENV_FILE}'")));
    assert!(landed.contains("ANTHROPIC_API_KEY=\"sk-ant-x\""));
    let mut expected = vec!["GET /limits", "POST /sandboxes", "GET /sandboxes/bx_tumrjngm", "GET /sandboxes/bx_tumrjngm"];
    expected.extend(std::iter::repeat("POST /sandboxes/bx_tumrjngm/commands").take(4));
    assert_eq!(api.calls(), expected);
}

/// box-backend.test.ts:323
#[tokio::test]
async fn creates_from_a_named_snapshot_renames_to_the_labels_and_off_the_trial_asks_no_auto_stop() {
    let api = FakeBox::new()
        .on(
            "POST",
            "/sandboxes",
            vec![reply(202, json!({ "ok": true, "type": "sandbox.created", "status": "provisioning", "ttlSeconds": null, "from": "wsp-mac-default-v1", "sandbox": box_view("bx_uwepudz7", "provisioned", json!({ "name": "wsp-mac-default-v1" })) }))],
        )
        .on(
            "GET",
            "/sandboxes/bx_uwepudz7",
            vec![info("bx_uwepudz7", "cloning", json!({ "name": "wsp-mac-default-v1" })), info("bx_uwepudz7", "idle", json!({ "name": "wsp-mac-default-v1" }))],
        )
        .on("PATCH", "/sandboxes/bx_uwepudz7", vec![info("bx_uwepudz7", "idle", json!({ "name": "wsp;o=h_1" }))])
        .on("POST", "/sandboxes/bx_uwepudz7/commands", vec![cmd("")]);
    let backend = backend(&api, "standard");
    let heard: Arc<Mutex<Vec<(String, Option<(String, String)>)>>> = Arc::default();
    let sink = heard.clone();
    let waiting: Waiting = Arc::new(move |line: &str, boat: Option<&ProviderBoxState>| {
        sink.lock().unwrap().push((line.to_string(), boat.map(|b| (b.id.clone(), b.state.clone()))));
    });
    let machine = backend
        .create(
            MachineSpec {
                template: Some("wsp-mac-default-v1".to_string()),
                cpu: Some(n(4.0)),
                mem_mb: Some(n(8192.0)),
                labels: Some(map(&[(OWNER_LABEL, "h_1")])),
                ..spec()
            },
            Some(waiting),
            None,
        )
        .await
        .unwrap();
    assert_eq!(machine.id(), "bx_uwepudz7");
    // Each of Boat's own states the box passed through, once, for the person following the create.
    let expected: Vec<(String, Option<(String, String)>)> = ["provisioned", "cloning", "idle"]
        .iter()
        .map(|state| (format!("Boat's machine bx_uwepudz7 is {state}"), Some(("bx_uwepudz7".to_string(), state.to_string()))))
        .collect();
    assert_eq!(*heard.lock().unwrap(), expected);
    // No window named and no cap on the account: no timer, the provider's own meaning of null.
    assert_eq!(
        api.took("POST", "/sandboxes").unwrap().body,
        json!({ "type": "default", "ttlSeconds": null, "noEnv": true, "name": "wsp;o=h_1", "from": "wsp-mac-default-v1" })
    );
    // The deployed box came back named after its snapshot, so the labels went back on with one update.
    assert_eq!(api.took("PATCH", "/sandboxes/bx_uwepudz7").unwrap().body, json!({ "name": "wsp;o=h_1" }));
    // fromSnapshot names the image the same way.
    api.on(
        "POST",
        "/sandboxes",
        vec![reply(202, json!({ "ok": true, "type": "sandbox.created", "status": "provisioning", "sandbox": box_view("bx_uwepudz7", "provisioned", json!({})) }))],
    );
    backend.create(MachineSpec { from_snapshot: Some("wsp-mac-default-v2".to_string()), ..spec() }, None, None).await.unwrap();
    let last = posts(&api, "/sandboxes").pop().unwrap().body;
    assert_eq!(last["from"], "wsp-mac-default-v2");
    assert_eq!(last["type"], "small");
    assert_eq!(last["name"], "wsp");
    let patch = api.seen().into_iter().filter(|s| s.method == "PATCH").last().unwrap();
    assert_eq!(patch.body, json!({ "name": "wsp" }));
    // One read of the tier serves every create.
    assert_eq!(count(&api, "GET /limits"), 1);
}

/// box-backend.test.ts:348
#[tokio::test]
async fn the_create_carries_the_idle_window_as_the_stop_timer_off_the_trial_and_the_cap_when_shorter() {
    let off = FakeBox::new()
        .on("POST", "/sandboxes", vec![reply(202, json!({ "ok": true, "status": "provisioning", "sandbox": box_view("bx_t1", "provisioning", json!({ "name": "wsp" })) }))])
        .on("GET", "/sandboxes/bx_t1", vec![info("bx_t1", "ready", json!({ "name": "wsp" }))])
        .on("POST", "/sandboxes/bx_t1/commands", vec![cmd("")]);
    backend(&off, "standard")
        .create(MachineSpec { on_idle: Some(MachineSpecOnIdle::Pause), idle_timeout_ms: Some(n(40.0 * 60_000.0)), ..spec() }, None, None)
        .await
        .unwrap();
    assert_eq!(off.took("POST", "/sandboxes").unwrap().body["ttlSeconds"], 40 * 60);
    let trial = FakeBox::new()
        .on("POST", "/sandboxes", vec![reply(202, json!({ "ok": true, "status": "provisioning", "sandbox": box_view("bx_t2", "provisioning", json!({ "name": "wsp" })) }))])
        .on("GET", "/sandboxes/bx_t2", vec![info("bx_t2", "ready", json!({ "name": "wsp" }))])
        .on("POST", "/sandboxes/bx_t2/commands", vec![cmd("")]);
    let backend = backend(&trial, "trial");
    backend
        .create(MachineSpec { on_idle: Some(MachineSpecOnIdle::Pause), idle_timeout_ms: Some(n(6.0 * 60.0 * 60_000.0)), ..spec() }, None, None)
        .await
        .unwrap();
    assert_eq!(trial.took("POST", "/sandboxes").unwrap().body["ttlSeconds"], TRIAL_TTL_S);
    backend
        .create(MachineSpec { on_idle: Some(MachineSpecOnIdle::Pause), idle_timeout_ms: Some(n(40.0 * 60_000.0)), ..spec() }, None, None)
        .await
        .unwrap();
    assert_eq!(posts(&trial, "/sandboxes").pop().unwrap().body["ttlSeconds"], 40 * 60);
}

/// box-backend.test.ts:366
#[tokio::test]
async fn a_create_answered_from_an_earlier_key_reads_replayed_and_the_envs_still_land() {
    let api = FakeBox::new()
        .on("POST", "/sandboxes", vec![reply(202, json!({ "ok": true, "type": "sandbox.created", "status": "ready", "ttlSeconds": 7200, "sandbox": box_view("bx_tumrjngm", "idle", json!({ "name": "wsp" })) }))])
        .on("GET", "/sandboxes/bx_tumrjngm", vec![info("bx_tumrjngm", "idle", json!({ "name": "wsp" }))])
        .on("POST", "/sandboxes/bx_tumrjngm/commands", vec![cmd("")]);
    let backend = backend(&api, "trial");
    let machine = backend.create(spec(), None, None).await.unwrap();
    assert!(machine.replayed());
    assert_eq!(api.calls().into_iter().filter(|c| c.starts_with("PATCH")).collect::<Vec<_>>(), Vec::<String>::new());
}

/// box-backend.test.ts:377
#[tokio::test]
async fn a_create_left_in_error_or_never_ready_fails_naming_the_state_and_the_box_it_made_is_deleted() {
    let api = FakeBox::new()
        .on("POST", "/sandboxes", vec![reply(202, json!({ "ok": true, "type": "sandbox.created", "status": "provisioning", "sandbox": box_view("bx_e1", "provisioning", json!({})) }))])
        .on("GET", "/sandboxes/bx_e1", vec![info("bx_e1", "error", json!({ "error": "no capacity" }))])
        .on("DELETE", "/sandboxes/bx_e1", vec![reply(202, json!({ "ok": true, "operation": { "id": "bdop_1" } }))]);
    let backend = backend(&api, "trial");
    let e = failed(backend.create(spec(), None, None).await);
    assert!(e.to_string().contains("bx_e1 reads error during create: no capacity"), "{e}");
    assert_eq!(deletes(&api), vec!["DELETE /sandboxes/bx_e1"]);
    api.on("GET", "/sandboxes/bx_e1", vec![info("bx_e1", "provisioning", json!({}))]);
    let e = failed(backend.create(spec(), None, None).await);
    assert!(e.to_string().contains("still reads provisioning"), "{e}");
    assert_eq!(deletes(&api).len(), 2);
    // The environment failing to land ends the create the same way: nothing the caller cannot name is left running.
    api.on("GET", "/sandboxes/bx_e1", vec![info("bx_e1", "ready", json!({ "name": "wsp" }))]);
    api.on("POST", "/sandboxes/bx_e1/commands", vec![detached_guest(1, "", "systemctl: not found")]);
    let e = failed(backend.create(MachineSpec { envs: Some(map(&[("A", "1")])), ..spec() }, None, None).await);
    assert!(e.to_string().contains("environment did not land on bx_e1"), "{e}");
    assert_eq!(deletes(&api).len(), 3);
}

/// box-backend.test.ts:395
#[tokio::test]
async fn a_create_boat_cancels_ends_on_the_first_read_with_its_reason_typed_and_the_box_is_deleted() {
    let api = FakeBox::new()
        .on("POST", "/sandboxes", vec![reply(202, json!({ "ok": true, "type": "sandbox.created", "status": "provisioning", "sandbox": box_view("bx_c1", "provisioning", json!({})) }))])
        .on("GET", "/sandboxes/bx_c1", vec![info("bx_c1", "cancelled", json!({ "error": "no capacity" }))])
        .on("DELETE", "/sandboxes/bx_c1", vec![reply(202, json!({ "ok": true, "operation": { "id": "bdop_c" } }))]);
    let backend = backend(&api, "trial");
    let e = failed(backend.create(spec(), None, None).await);
    let EngineError::CreateCancelled { machine_id, why, .. } = &e else { panic!("expected CreateCancelled, got {e:?}") };
    assert_eq!(machine_id, "bx_c1");
    assert_eq!(why, "no capacity");
    assert_eq!(count(&api, "GET /sandboxes/bx_c1"), 1);
    assert_eq!(deletes(&api), vec!["DELETE /sandboxes/bx_c1"]);
}

/// box-backend.test.ts:408
#[tokio::test]
async fn an_environment_not_landed_at_the_restore_budget_is_typed_with_its_box_and_its_exit() {
    let api = FakeBox::new()
        .on("POST", "/sandboxes", vec![reply(202, json!({ "ok": true, "status": "provisioning", "sandbox": box_view("bx_t1", "provisioning", json!({ "name": "wsp" })) }))])
        .on("GET", "/sandboxes/bx_t1", vec![info("bx_t1", "ready", json!({ "name": "wsp" }))])
        .on("POST", "/sandboxes/bx_t1/commands", vec![cmd(""), detached_guest(DEADLINE_EXIT, "", "")])
        .on("DELETE", "/sandboxes/bx_t1", vec![reply(202, json!({ "ok": true, "operation": { "id": "bdop_t" } }))]);
    let backend = backend_on(&api, "trial", BoxBudgetsAsk { restore_ms: Some(0), ..Default::default() }).backend;
    let e = failed(backend.create(MachineSpec { envs: Some(map(&[("A", "1")])), ..spec() }, None, None).await);
    let EngineError::EnvNotLanded { machine_id, exit_code, .. } = &e else { panic!("expected EnvNotLanded, got {e:?}") };
    assert_eq!(machine_id, "bx_t1");
    assert_eq!(*exit_code, DEADLINE_EXIT);
    assert!(e.to_string().starts_with("environment did not land on bx_t1 (exit 124)"), "{e}");
}

/// box-backend.test.ts:421
#[tokio::test]
async fn a_landing_of_the_environment_runs_detached_and_one_that_times_out_is_launched_again_inside_the_budget() {
    let api = FakeBox::new()
        .on("POST", "/sandboxes", vec![reply(202, json!({ "ok": true, "status": "provisioning", "sandbox": box_view("bx_l1", "provisioning", json!({ "name": "wsp" })) }))])
        .on("GET", "/sandboxes/bx_l1", vec![info("bx_l1", "ready", json!({ "name": "wsp" }))])
        .on(
            "POST",
            "/sandboxes/bx_l1/commands",
            vec![cmd(""), detached_guest(DEADLINE_EXIT, "", ""), detached_guest(DEADLINE_EXIT, "", ""), detached_guest(DEADLINE_EXIT, "", ""), detached_ok()],
        );
    let backend = backend(&api, "trial");
    let machine = backend.create(MachineSpec { envs: Some(map(&[("A", "1")])), ..spec() }, None, None).await.unwrap();
    assert_eq!(machine.id(), "bx_l1");
    // Every command the road sends is an inline exec of its own, none of them near the edge's cut.
    let asked: BTreeSet<u64> = api
        .seen()
        .iter()
        .filter(|s| s.path == "/sandboxes/bx_l1/commands")
        .map(|s| s.body["timeoutSeconds"].as_u64().expect("every command asks a timeout in seconds"))
        .collect();
    assert_eq!(asked, BTreeSet::from([INLINE_EXEC_MS / 1000]));
    // The environment script travelled as the launched file's text, and the run that timed out was launched again.
    let launches: Vec<String> = api.commands_sent().into_iter().filter(|c| c.contains("WSP_LAUNCHED")).collect();
    assert_eq!(launches.len(), 2);
    for l in &launches {
        assert_eq!(launched_text(l), env_landing_script(&map(&[("A", "1")])));
    }
    // A landing that fails for another reason is the failure at once, and a timeout past the budget too.
    api.on("POST", "/sandboxes/bx_l1/commands", vec![detached_guest(DEADLINE_EXIT, "", "")])
        .on("DELETE", "/sandboxes/bx_l1", vec![reply(202, json!({ "ok": true, "operation": { "id": "bdop_l" } }))]);
    let e = failed(backend.create(MachineSpec { envs: Some(map(&[("A", "1")])), ..spec() }, None, None).await);
    assert!(e.to_string().contains("environment did not land on bx_l1 (exit 124)"), "{e}");
}

/// box-backend.test.ts:489
#[tokio::test]
async fn a_refusal_is_the_providers_own_envelope_the_code_the_message_and_the_request_id_as_the_engines_kinds() {
    let api = FakeBox::new().on("POST", "/sandboxes", vec![refuse(429, "rate_limited", "Too many machine starts this minute.")]);
    let backend = backend(&api, "trial");
    let e = failed(backend.create(spec(), None, None).await);
    // The trial's answer here numbers no window, so the refusal keeps Boat's words, read by its code all the same.
    let w = provider(&e);
    assert_eq!(w.kind, ErrorKind::StartLimit);
    assert_eq!(w.status, 429);
    assert_eq!(w.code.as_deref(), Some("rate_limited"));
    assert_eq!(w.message, "Too many machine starts this minute.");
    assert_eq!(w.request_id.as_deref(), Some("req_b603629401be48d082e623575a1b9111"));
    let bad = FakeBox::new().on("GET", "/limits", vec![refuse(401, "unauthorized", "invalid api key")]);
    let refused = BoxBackend::new(BoxBackendOptions { api_key: "box_wrong".into(), fetch: Some(bad.clone() as Arc<dyn Fetch>), ..Default::default() });
    let e = failed(refused.check_key().await);
    let w = provider(&e);
    assert_eq!(w.kind, ErrorKind::Auth);
    assert_eq!(w.status, 401);
}

/// box-backend.test.ts:500
#[tokio::test]
async fn the_concurrent_box_cap_is_the_concurrency_kind_whatever_status_carries_it() {
    let trial = FakeBox::new().on(
        "POST",
        "/sandboxes",
        vec![refuse(403, "trial_limit", "Trial accounts can run 2 concurrent boxes. End the trial or buy a pack after subscribing to move to 100 concurrent boxes.")],
    );
    let e = failed(backend(&trial, "trial").create(spec(), None, None).await);
    let w = provider(&e);
    assert_eq!((w.kind, w.status, w.code.as_deref()), (ErrorKind::Concurrency, 403, Some("trial_limit")));
    let trial_new = FakeBox::new().on(
        "POST",
        "/sandboxes",
        vec![refuse(403, "trial_limit", "Trial accounts can run 2 concurrent sandboxes. End the trial or buy a pack after subscribing to move to 100 concurrent sandboxes.")],
    );
    let e = failed(backend(&trial_new, "trial").create(spec(), None, None).await);
    let w = provider(&e);
    assert_eq!((w.kind, w.status, w.code.as_deref()), (ErrorKind::Concurrency, 403, Some("trial_limit")));
    let plan = FakeBox::new().on("POST", "/sandboxes", vec![refuse(429, "limit_reached", "Concurrent box limit reached.")]);
    let e = failed(backend(&plan, "trial").create(spec(), None, None).await);
    let w = provider(&e);
    assert_eq!((w.kind, w.code.as_deref()), (ErrorKind::Concurrency, Some("limit_reached")));
    // A refusal about something else keeps its own kind.
    let forbidden = FakeBox::new().on("POST", "/sandboxes", vec![refuse(403, "trial_machine_class_not_allowed", "large was requested on a trial account.")]);
    let e = failed(backend(&forbidden, "trial").create(spec(), None, None).await);
    let w = provider(&e);
    assert_eq!((w.kind, w.code.as_deref()), (ErrorKind::Plan, Some("trial_machine_class_not_allowed")));
    // A size the trial does not sell is refused before Boat is asked.
    let large = FakeBox::new();
    let e = failed(backend(&large, "trial").create(MachineSpec { cpu: Some(n(8.0)), mem_mb: Some(n(16384.0)), ..spec() }, None, None).await);
    assert!(matches!(e, EngineError::Invalid { .. }), "{e:?}");
    assert_eq!(large.calls(), vec!["GET /limits"]);
}

/// box-backend.test.ts:516
#[tokio::test]
async fn checks_the_key_against_the_accounts_limits_and_nothing_else() {
    let api = FakeBox::new();
    let backend = backend(&api, "trial");
    backend.check_key().await.unwrap();
    assert_eq!(api.calls(), vec!["GET /limits"]);
}

/// box-backend.test.ts:615
#[tokio::test]
async fn a_create_whose_box_reads_ready_and_then_cannot_run_a_command_is_deleted_and_fails_with_the_providers_line() {
    let api = FakeBox::new()
        .on("POST", "/sandboxes", vec![reply(202, json!({ "ok": true, "status": "provisioning", "sandbox": box_view("bx_d1", "provisioning", json!({ "name": "wsp" })) }))])
        .on("GET", "/sandboxes/bx_d1", vec![info("bx_d1", "ready", json!({ "name": "wsp" }))])
        .on(
            "POST",
            "/sandboxes/bx_d1/commands",
            vec![command("", Some(127), json!({ "stderr": "bash: error while loading shared libraries: libtinfo.so.6: cannot open shared object file: Error 24\n" }))],
        )
        .on("DELETE", "/sandboxes/bx_d1", vec![reply(202, json!({ "ok": true, "operation": { "id": "bdop_d" } }))]);
    let backend = backend(&api, "trial");
    let e = failed(backend.create(spec(), None, None).await);
    assert!(matches!(e, EngineError::GuestUnusable { .. }), "{e:?}");
    assert!(e.to_string().contains("Boat left bx_d1 running but nothing on it can run"), "{e}");
    assert_eq!(deletes(&api), vec!["DELETE /sandboxes/bx_d1"]);
}

/// box-backend.test.ts:628
#[tokio::test]
async fn a_healthy_create_proves_the_road_once_after_the_box_reads_ready_and_before_the_name_and_the_environment() {
    let api = FakeBox::new()
        .on("POST", "/sandboxes", vec![reply(202, json!({ "ok": true, "status": "provisioning", "sandbox": box_view("bx_p1", "provisioning", json!({ "name": "wsp-mac-default-v1" })) }))])
        .on("GET", "/sandboxes/bx_p1", vec![info("bx_p1", "ready", json!({ "name": "wsp-mac-default-v1" }))])
        .on("PATCH", "/sandboxes/bx_p1", vec![info("bx_p1", "ready", json!({ "name": "wsp" }))])
        .on("POST", "/sandboxes/bx_p1/commands", vec![cmd(""), detached_ok()]);
    let backend = backend(&api, "trial");
    backend
        .create(MachineSpec { from_snapshot: Some("wsp-mac-default-v1".to_string()), envs: Some(map(&[("A", "1")])), ..spec() }, None, None)
        .await
        .unwrap();
    assert_eq!(
        api.calls(),
        vec![
            "GET /limits",
            "POST /sandboxes",
            "GET /sandboxes/bx_p1",
            "POST /sandboxes/bx_p1/commands",
            "PATCH /sandboxes/bx_p1",
            "POST /sandboxes/bx_p1/commands",
            "POST /sandboxes/bx_p1/commands",
            "POST /sandboxes/bx_p1/commands",
        ]
    );
    assert_eq!(api.commands_sent()[0], sudo_command("true"));
}
