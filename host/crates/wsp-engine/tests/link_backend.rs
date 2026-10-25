// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/test/link-backend.test.ts. Machines on another computer, driven a frame at a time. What the
//! far side answers is what its own backend answered, and which optional calls a handle carries is the far side's to say.
mod roads_common;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use futures::FutureExt;
use serde_json::{json, Map, Value};
use wsp_engine::cancel::Cancel;

use roads_common::*;
use wsp_catalog::signin_rows::shares_in;
use wsp_engine::errors::{aborted, is_missing, EngineError, ErrorKind, Result, WspError};
use wsp_engine::exec_detached::INLINE_EXEC_MS;
use wsp_engine::labels::BUILDER_LABEL;
use wsp_engine::link_backend::{is_place_absent, place_absent, LinkAsk, LinkBackend, LinkMachine, CREATE_MS, LINK_MARGIN_MS};
use wsp_engine::machine::{
    Capabilities, DaemonSupervisor, ExecOptions, Machine, MachineBackend, MachineKind, MachineLife, MachineSpec, PlaceCapacity,
    PreviewReach, RunOptions, Size, SnapshotOptions,
};
use wsp_wire::types::JsNumber;
use wsp_wire::wire::machine_link::{BackendFacts, MachineHandle, MACHINE_PUT_PART_BYTES, NO_IMAGES_HERE};

fn capabilities_json() -> Value {
    json!({
        "liveCloneForks": false,
        "pauseMode": "memory",
        "replacesMachine": true,
        "previewUrls": false,
        "signedUrls": false,
        "callbackRelay": true,
        "diskSnapshots": true,
        "images": true,
        "snapshotsAnyLife": false,
        "snapshotListing": true,
        "templates": true,
        "kept": false,
        "copies": true,
        "ownNetwork": true,
        "sizes": [{ "cpu": 2, "memMb": 4096, "rateUsdPerHour": 0 }],
    })
}

fn capabilities() -> Capabilities {
    serde_json::from_value(capabilities_json()).unwrap()
}

fn facts_json() -> Value {
    json!({
        "offer": "docker",
        "capabilities": capabilities_json(),
        "pricing": {
            "defaultSize": { "cpu": 2, "memMb": 4096 },
            "snapshotStorage": { "freeGb": 0, "usdPerGbMonth": 0, "billedFrom": "" },
        },
        "lifecycle": { "budgets": { "wakeAttempts": 1, "daemonAnswersMs": 30000 } },
        "baseTemplates": { "sandbox": "ubuntu:24.04", "desktop": "ubuntu:24.04" },
    })
}

fn facts_with_logins() -> Value {
    let mut f = facts_json();
    f["logins"] = json!("/wsp/logins");
    f
}

fn roads_json() -> Value {
    json!({ "previewUrl": true, "daemonAnswers": true, "putBytes": true, "describe": true, "facts": true, "metrics": true })
}

fn handle_json() -> Value {
    json!({ "id": "c1", "kind": "sandbox", "daemonSupervisor": "entrypoint", "roads": roads_json() })
}

fn handle() -> MachineHandle {
    serde_json::from_value(handle_json()).unwrap()
}

fn spec(v: Value) -> MachineSpec {
    serde_json::from_value(v).unwrap()
}

fn obj(v: Value) -> Map<String, Value> {
    v.as_object().cloned().unwrap()
}

fn err<T>(r: Result<T>) -> EngineError {
    match r {
        Ok(_) => panic!("expected an error, got an answer"),
        Err(e) => e,
    }
}

fn ask(timeout_ms: Option<u64>, key: Option<&str>) -> LinkAsk {
    LinkAsk { timeout_ms, idempotency_key: key.map(str::to_string) }
}

fn params_of(s: &Sent) -> Value {
    Value::Object(s.params.clone())
}

fn last(l: &FakeLink) -> Sent {
    l.sent().last().cloned().expect("no frame was sent")
}

fn frames(l: &FakeLink, op: &str) -> Vec<Sent> {
    l.sent().into_iter().filter(|s| s.op == op).collect()
}

fn assert_frame(s: &Sent, op: &str, params: Value, ask: LinkAsk) {
    assert_eq!(s.op, op);
    assert_eq!(params_of(s), params, "{op} params");
    assert_eq!(s.ask, ask, "{op} ask");
}

fn missing(message: &str) -> EngineError {
    EngineError::Provider(WspError {
        kind: ErrorKind::Missing,
        status: 404,
        code: None,
        message: message.to_string(),
        request_id: None,
        window: None,
        said: None,
    })
}

fn opened<F>(answers: F) -> Arc<FakeLink>
where
    F: Fn(&Sent) -> Result<Value> + 'static,
{
    FakeLink::new(move |s| if s.op == "machine.backend" { Ok(facts_json()) } else { answers(s) })
}

fn made(s: &Sent) -> Option<Result<Value>> {
    (s.op == "machine.create" || s.op == "machine.get").then(|| Ok(json!({ "machine": handle_json() })))
}

async fn with_machine<F>(answers: F) -> (Arc<FakeLink>, LinkBackend)
where
    F: Fn(&Sent) -> Result<Value> + 'static,
{
    let l = opened(move |s| made(s).unwrap_or_else(|| answers(s)));
    let backend = LinkBackend::open(l.clone()).await.unwrap();
    (l, backend)
}

fn no_frame_but_the_backend(l: &FakeLink) {
    assert_eq!(l.ops(), vec!["machine.backend"]);
}

#[tokio::test]
async fn asks_once_and_answers_with_what_the_far_side_said() {
    let l = opened(|_| Ok(json!({})));
    let backend = LinkBackend::open(l.clone()).await.unwrap();
    no_frame_but_the_backend(&l);
    assert_frame(&l.sent()[0], "machine.backend", json!({}), LinkAsk::default());
    assert_eq!(backend.capabilities(), capabilities());
    assert_eq!(backend.lifecycle().unwrap().budgets().wake_attempts, 1.into());
    let templates = backend.base_templates().unwrap();
    assert_eq!((templates.sandbox.as_str(), templates.desktop.as_str()), ("ubuntu:24.04", "ubuntu:24.04"));
    assert!(backend.names_workspace());
}

#[tokio::test]
async fn carries_no_snapshot_and_no_template_call_at_all() {
    let backend = LinkBackend::open(opened(|_| Ok(json!({})))).await.unwrap();
    // The link has no frame to send for any of listSnapshots, promoteSnapshot, getTemplate, listTemplates and
    // deleteTemplate, whatever the far side says about itself.
    assert!(!backend.offers().list_snapshots && !backend.offers().templates);
    let l = opened(|_| Ok(json!({})));
    let with_link = LinkBackend::open(l.clone()).await.unwrap();
    assert_eq!(err(with_link.delete_snapshot("sha256:aa").await).to_string(), NO_IMAGES_HERE);
    let machine = LinkMachine::new(l.clone(), handle());
    assert_eq!(err(machine.snapshot("v1", MachineLife { first_life: true }, SnapshotOptions::default()).await).to_string(), NO_IMAGES_HERE);
    no_frame_but_the_backend(&l);
}

#[tokio::test]
async fn prices_a_size_off_the_offers_the_far_side_sent() {
    let backend = LinkBackend::open(opened(|_| Ok(json!({})))).await.unwrap();
    let pricing = backend.pricing();
    assert_eq!(pricing.rate_usd_per_hour(Size { cpu: 2.0, mem_mb: 4096.0 }), 0.0);
    assert_eq!(pricing.rate_usd_per_hour(Size { cpu: 64.0, mem_mb: 9999.0 }), 0.0);
    assert_eq!(pricing.default_size(), Size { cpu: 2.0, mem_mb: 4096.0 });
}

#[tokio::test]
async fn sends_the_spec_whole_and_carries_exactly_the_roads_the_handle_named() {
    let (l, backend) = with_machine(|_| Ok(json!({}))).await;
    let sent_spec = json!({ "kind": "sandbox", "template": "wsp/dev:template", "cpu": 2, "memMb": 4096 });
    let machine = backend.create(spec(sent_spec.clone()), None, None).await.unwrap();
    assert_frame(&last(&l), "machine.create", json!({ "spec": sent_spec }), ask(Some(CREATE_MS), None));
    assert!(machine.offers().preview_url);
    assert!(machine.offers().put_bytes);
    assert_eq!(machine.daemon_supervisor(), Some(DaemonSupervisor::Entrypoint));
}

#[tokio::test]
async fn carries_the_sentence_the_computer_answered_the_create_with() {
    let said = "size 2x4 on 2 cores: cpu clamped to 1 and memory clamped to 2 GB";
    let l = opened(move |s| {
        let mut h = handle_json();
        h["notice"] = json!(said);
        Ok(if s.op == "machine.create" { json!({ "machine": h }) } else { json!({}) })
    });
    let backend = LinkBackend::open(l.clone()).await.unwrap();
    let machine = backend.create(spec(json!({ "kind": "sandbox", "cpu": 2, "memMb": 4096 })), None, None).await.unwrap();
    assert_eq!(machine.notice(), Some(said));
    let (_, backend) = with_machine(|_| Ok(json!({}))).await;
    let machine = backend.create(spec(json!({ "kind": "sandbox" })), None, None).await.unwrap();
    assert_eq!(machine.notice(), None);
}

#[tokio::test]
async fn leaves_out_a_road_the_handle_did_not_name() {
    let l = opened(|s| {
        let mut h = handle_json();
        h["roads"]["previewUrl"] = json!(false);
        h["roads"]["metrics"] = json!(false);
        Ok(if s.op == "machine.create" { json!({ "machine": h }) } else { json!({}) })
    });
    let backend = LinkBackend::open(l).await.unwrap();
    let machine = backend.create(spec(json!({ "kind": "sandbox" })), None, None).await.unwrap();
    assert!(!machine.offers().preview_url);
    assert!(!machine.offers().metrics);
    assert!(machine.offers().describe);
}

#[tokio::test]
async fn fills_a_create_with_the_logins_that_computer_signs_in_once() {
    let l = FakeLink::new(|s| match s.op.as_str() {
        "machine.backend" => Ok(facts_with_logins()),
        "machine.create" => Ok(json!({ "machine": handle_json() })),
        _ => Ok(json!({})),
    });
    let backend = LinkBackend::open(l.clone()).await.unwrap();
    backend.create(spec(json!({ "kind": "sandbox" })), None, None).await.unwrap();
    // The catalog's own rows, at the directory that computer said it keeps them in: Codex's auth.json today.
    assert_eq!(last(&l).params["spec"], json!({ "kind": "sandbox", "shares": serde_json::to_value(shares_in("/wsp/logins")).unwrap() }));
    // A builder becomes an image, and a sign-in never sits in one, so it shares nothing at all.
    backend.create(spec(json!({ "kind": "sandbox", "labels": { BUILDER_LABEL: "1" } })), None, None).await.unwrap();
    assert_eq!(last(&l).params["spec"], json!({ "kind": "sandbox", "labels": { BUILDER_LABEL: "1" } }));
    assert!(last(&l).params["spec"].get("shares").is_none());
}

#[tokio::test]
async fn shares_nothing_where_the_computer_names_no_logins_directory() {
    let (l, backend) = with_machine(|_| Ok(json!({}))).await;
    backend.create(spec(json!({ "kind": "sandbox" })), None, None).await.unwrap();
    assert_eq!(last(&l).params["spec"], json!({ "kind": "sandbox" }));
}

#[tokio::test]
async fn execs_with_the_callers_bound() {
    let (l, backend) = with_machine(|_| Ok(json!({ "result": { "exitCode": 0, "stdout": "hi", "stderr": "" } }))).await;
    let machine = backend.create(spec(json!({ "kind": "sandbox" })), None, None).await.unwrap();
    let out = machine.exec("echo hi", ExecOptions { timeout_ms: Some(1000), ..Default::default() }).await.unwrap();
    assert_eq!(out, res(0, "hi", ""));
    assert_frame(
        &last(&l),
        "machine.exec",
        json!({ "cmd": "echo hi", "timeoutMs": 1000, "machineId": "c1" }),
        ask(Some(1000 + LINK_MARGIN_MS), None),
    );
}

#[tokio::test]
async fn sends_an_execs_input_as_base64_beside_the_command() {
    let (l, backend) = with_machine(|_| Ok(json!({ "result": { "exitCode": 0, "stdout": "", "stderr": "" } }))).await;
    let machine = backend.create(spec(json!({ "kind": "sandbox" })), None, None).await.unwrap();
    machine.exec("read -r v", ExecOptions { stdin: Some(b"GH_TOKEN=ghp_secret\0".to_vec()), ..Default::default() }).await.unwrap();
    let sent = last(&l).params;
    let stdin = STANDARD.decode(sent["stdin"].as_str().unwrap()).unwrap();
    assert_eq!(stdin, b"GH_TOKEN=ghp_secret\0".to_vec());
    assert_eq!(sent["cmd"], json!("read -r v"));
    assert!(!sent["cmd"].as_str().unwrap().contains("ghp_secret"));
    assert_eq!(
        Value::Object(sent),
        json!({ "cmd": "read -r v", "timeoutMs": INLINE_EXEC_MS, "stdin": STANDARD.encode(b"GH_TOKEN=ghp_secret\0"), "machineId": "c1" })
    );
    machine.exec("true", ExecOptions::default()).await.unwrap();
    assert!(!last(&l).params.contains_key("stdin"));
    assert_eq!(params_of(&last(&l)), json!({ "cmd": "true", "timeoutMs": INLINE_EXEC_MS, "machineId": "c1" }));
}

#[tokio::test]
async fn runs_a_script_over_exec_frames_and_sends_no_run_frame() {
    let (l, backend) = with_machine(|s| {
        let launched = s.params.get("cmd").and_then(Value::as_str).is_some_and(|c| c.contains("WSP_LAUNCHED"));
        Ok(json!({ "result": { "exitCode": 0, "stdout": if launched { "WSP_LAUNCHED" } else { "0" }, "stderr": "" } }))
    })
    .await;
    let machine = backend.create(spec(json!({ "kind": "sandbox" })), None, None).await.unwrap();
    let _ = machine.run("echo hi", RunOptions { deadline_ms: 100, poll_ms: Some(1), ..Default::default() }).await;
    let ops = l.ops();
    assert!(ops.iter().all(|op| op != "machine.run"), "{ops:?}");
    assert!(ops.iter().skip(2).count() > 0, "{ops:?}");
    assert!(ops.iter().skip(2).all(|op| op == "machine.exec"), "{ops:?}");
}

#[tokio::test]
async fn answers_one_daemon_frame_for_this_machine() {
    // A workspace on a computer somebody owns runs no daemon of its own: its files and its git are answered by the
    // daemon of the computer holding it, over the link this backend already holds.
    let (l, backend) = with_machine(|s| {
        Ok(if s.op == "git.status" { json!({ "branch": { "head": "work" }, "entries": [], "root": "/root/work" }) } else { json!({}) })
    })
    .await;
    let machine = backend.get("c1").await.unwrap();
    let reply = machine.daemon_frame(obj(json!({ "op": "git.status", "cwd": "/root/work" }))).await.unwrap();
    assert_frame(&last(&l), "git.status", json!({ "cwd": "/root/work", "machineId": "c1" }), LinkAsk::default());
    assert_eq!(Value::Object(reply), json!({ "ok": true, "branch": { "head": "work" }, "entries": [], "root": "/root/work" }));
    // A refusal comes back as the reply it was, with the code the daemon put on it, so a caller reads the reason:
    // a bring back tells a computer with no signed-in gh from a checkout with no remote by that code alone.
    let refused = opened(|s| {
        made(s).unwrap_or_else(|| {
            Err(EngineError::Other {
                message: "no signed-in command line for github.com".to_string(),
                code: Some("no-host-cli".to_string()),
            })
        })
    });
    let other = LinkBackend::open(refused).await.unwrap().get("c1").await.unwrap();
    let reply = other.daemon_frame(obj(json!({ "op": "git.pr", "cwd": "/root/work" }))).await.unwrap();
    assert_eq!(Value::Object(reply), json!({ "ok": false, "error": "no signed-in command line for github.com", "code": "no-host-cli" }));
}

#[tokio::test]
async fn reads_a_refusal_the_far_sides_own_backend_gave() {
    let l = opened(|s| if s.op == "machine.get" { Err(missing("no such container")) } else { Ok(json!({})) });
    let backend = LinkBackend::open(l).await.unwrap();
    let e = err(backend.get("c9").await);
    assert!(is_missing(&e), "{e:?}");
    assert_eq!(e.to_string(), "no such container");
}

#[tokio::test]
async fn passes_a_place_that_is_not_connected_through() {
    let l = opened(|s| if s.op == "machine.get" { Err(place_absent("srv is not connected right now")) } else { Ok(json!({})) });
    let backend = LinkBackend::open(l).await.unwrap();
    let e = err(backend.get("c1").await);
    assert!(is_place_absent(&e), "{e:?}");
    assert_eq!(e.to_string(), "srv is not connected right now");
}

#[tokio::test]
async fn sends_a_file_in_parts_under_one_upload() {
    let (l, backend) = with_machine(|_| Ok(json!({}))).await;
    let machine = backend.create(spec(json!({ "kind": "sandbox" })), None, None).await.unwrap();
    let landed = machine.put_bytes("/root/big", &vec![0u8; 9 * 1024 * 1024], None).await.unwrap();
    assert_eq!(landed, None);
    let parts = frames(&l, "machine.putBytes");
    let seqs: Vec<Value> = parts.iter().map(|p| p.params["seq"].clone()).collect();
    assert_eq!(seqs, vec![json!(0), json!(1), json!(2)]);
    let lasts: Vec<Value> = parts.iter().map(|p| p.params["last"].clone()).collect();
    assert_eq!(lasts, vec![json!(false), json!(false), json!(true)]);
    let ids: std::collections::BTreeSet<String> = parts.iter().map(|p| p.params["uploadId"].as_str().unwrap().to_string()).collect();
    assert_eq!(ids.len(), 1);
    let sizes: Vec<usize> = parts.iter().map(|p| STANDARD.decode(p.params["data"].as_str().unwrap()).unwrap().len()).collect();
    assert_eq!(sizes, vec![MACHINE_PUT_PART_BYTES as usize, MACHINE_PUT_PART_BYTES as usize, 1024 * 1024]);
    for p in &parts {
        assert_eq!(p.params["path"], json!("/root/big"));
        assert_eq!(p.params["timeoutMs"], json!(INLINE_EXEC_MS));
        assert_eq!(p.params["machineId"], json!("c1"));
        assert_eq!(p.ask, ask(Some(INLINE_EXEC_MS + LINK_MARGIN_MS), None));
    }
}

#[tokio::test]
async fn names_a_create_by_the_key_its_spec_carries() {
    let (l, backend) = with_machine(|_| Ok(json!({}))).await;
    backend.create(spec(json!({ "kind": "sandbox", "idempotencyKey": "workspace/ws_1:a1" })), None, None).await.unwrap();
    assert_eq!(last(&l).ask, ask(Some(CREATE_MS), Some("workspace/ws_1:a1")));
    backend.create(spec(json!({ "kind": "sandbox" })), None, None).await.unwrap();
    assert_eq!(last(&l).ask, ask(Some(CREATE_MS), None));
    assert_eq!(last(&l).ask.idempotency_key, None);
}

#[tokio::test]
async fn names_an_exec_the_caller_says_lands_the_same_twice() {
    let (l, backend) = with_machine(|_| Ok(json!({ "result": { "exitCode": 0, "stdout": "", "stderr": "" } }))).await;
    let machine = backend.create(spec(json!({ "kind": "sandbox" })), None, None).await.unwrap();
    machine.exec("rm -f /tmp/x", ExecOptions { idempotency_key: Some("clean/1".to_string()), ..Default::default() }).await.unwrap();
    assert_eq!(last(&l).ask, ask(Some(INLINE_EXEC_MS + LINK_MARGIN_MS), Some("clean/1")));
    machine.exec("echo hi", ExecOptions::default()).await.unwrap();
    assert_eq!(last(&l).ask, ask(Some(INLINE_EXEC_MS + LINK_MARGIN_MS), None));
}

#[tokio::test]
async fn carries_no_key_on_a_part_and_sends_the_whole_upload_again() {
    let broken = Arc::new(AtomicBool::new(true));
    let down = broken.clone();
    let back = broken.clone();
    let l = FakeLink::dialling(
        move |s| match s.op.as_str() {
            "machine.backend" => Ok(facts_json()),
            "machine.create" => Ok(json!({ "machine": handle_json() })),
            "machine.putBytes" if down.load(Ordering::SeqCst) && s.params.get("seq") == Some(&json!(1)) => Err(plain("connection lost")),
            _ => Ok(json!({})),
        },
        move || {
            back.store(false, Ordering::SeqCst);
            true
        },
    );
    let machine = LinkBackend::open(l.clone()).await.unwrap().create(spec(json!({ "kind": "sandbox" })), None, None).await.unwrap();
    machine.put_bytes("/root/big", &vec![0u8; 9 * 1024 * 1024], None).await.unwrap();
    let parts = frames(&l, "machine.putBytes");
    // A part appended twice would splice the file, so no part names itself as one to ask again.
    assert!(parts.iter().all(|p| p.ask.idempotency_key.is_none()));
    // Two parts of the broken upload, then all three again under a name the far side holds nothing under.
    let seqs: Vec<Value> = parts.iter().map(|p| p.params["seq"].clone()).collect();
    assert_eq!(seqs, vec![json!(0), json!(1), json!(0), json!(1), json!(2)]);
    let mut ids: Vec<String> = Vec::new();
    for p in &parts {
        let id = p.params["uploadId"].as_str().unwrap().to_string();
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    assert_eq!(ids.len(), 2);
    assert!(parts[..2].iter().all(|p| p.params["uploadId"] == json!(ids[0])));
    assert!(parts[2..].iter().all(|p| p.params["uploadId"] == json!(ids[1])));
}

#[tokio::test]
async fn fails_an_upload_with_the_parts_own_answer() {
    let away = FakeLink::dialling(
        |s| match s.op.as_str() {
            "machine.backend" => Ok(facts_json()),
            "machine.create" => Ok(json!({ "machine": handle_json() })),
            "machine.putBytes" => Err(plain("connection lost")),
            _ => Ok(json!({})),
        },
        || false,
    );
    let gone = LinkBackend::open(away.clone()).await.unwrap().create(spec(json!({ "kind": "sandbox" })), None, None).await.unwrap();
    assert_eq!(err(gone.put_bytes("/root/big", &[1], None).await).to_string(), "connection lost");
    assert_eq!(frames(&away, "machine.putBytes").len(), 1);

    let refused = FakeLink::dialling(
        |s| match s.op.as_str() {
            "machine.backend" => Ok(facts_json()),
            "machine.create" => Ok(json!({ "machine": handle_json() })),
            "machine.putBytes" => Err(plain("no space left on device")),
            _ => Ok(json!({})),
        },
        || false,
    );
    let full = LinkBackend::open(refused.clone()).await.unwrap().create(spec(json!({ "kind": "sandbox" })), None, None).await.unwrap();
    assert_eq!(err(full.put_bytes("/root/big", &[1], None).await).to_string(), "no space left on device");
    assert_eq!(frames(&refused, "machine.putBytes").len(), 1);
}

#[tokio::test]
async fn sends_one_part_for_one_byte() {
    let (l, backend) = with_machine(|_| Ok(json!({}))).await;
    let machine = backend.create(spec(json!({ "kind": "sandbox" })), None, None).await.unwrap();
    machine.put_bytes("/root/one", &[7], None).await.unwrap();
    let parts = frames(&l, "machine.putBytes");
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].params["last"], json!(true));
    assert_eq!(parts[0].params["seq"], json!(0));
    assert_eq!(parts[0].params["data"], json!(STANDARD.encode([7u8])));
}

#[tokio::test]
async fn turns_the_far_sides_own_loopback_route_into_one_on_this_computer() {
    let (l, backend) = with_machine(|s| {
        Ok(if s.op == "machine.previewUrl" {
            json!({ "reach": { "url": "http://127.0.0.1:32773", "token": "", "expiresAt": 1 } })
        } else {
            json!({})
        })
    })
    .await;
    let machine = backend.create(spec(json!({ "kind": "sandbox" })), None, None).await.unwrap();
    let reach = machine.preview_url(7070).await.unwrap();
    assert_frame(
        &frames(&l, "machine.previewUrl")[0],
        "machine.previewUrl",
        json!({ "port": 7070, "machineId": "c1" }),
        LinkAsk::default(),
    );
    assert_eq!(*l.forwarded.lock().unwrap(), vec![32773]);
    assert_eq!(
        reach,
        PreviewReach { url: "http://127.0.0.1:51001".to_string(), token: String::new(), expires_at: JsNumber::from(9007199254740991_i64) }
    );
}

#[tokio::test]
async fn asks_the_machine_itself_whether_its_daemon_answers() {
    let (l, backend) = with_machine(|_| Ok(json!({ "answers": true }))).await;
    let machine = backend.create(spec(json!({ "kind": "sandbox" })), None, None).await.unwrap();
    assert!(machine.daemon_answers(None).await.unwrap());
    assert_frame(
        &last(&l),
        "machine.daemonAnswers",
        json!({ "timeoutMs": INLINE_EXEC_MS, "machineId": "c1" }),
        ask(Some(INLINE_EXEC_MS + LINK_MARGIN_MS), None),
    );
}

#[tokio::test]
async fn stops_waiting_on_a_resume_the_caller_gave_up_on() {
    let l = FakeLink::with_future(Arc::new(|s: &Sent| {
        let op = s.op.clone();
        async move {
            match op.as_str() {
                "machine.backend" => Ok(obj(facts_json())),
                "machine.create" => Ok(obj(json!({ "machine": handle_json() }))),
                "machine.resume" => futures::future::pending().await,
                _ => Ok(Map::new()),
            }
        }
        .boxed_local()
    }));
    let backend = LinkBackend::open(l.clone()).await.unwrap();
    let machine = backend.create(spec(json!({ "kind": "sandbox" })), None, None).await.unwrap();
    let stop = Cancel::new();
    let canceller = stop.clone();
    let (resumed, ()) = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::join!(machine.resume(Some(&stop)), async move {
            tokio::task::yield_now().await;
            canceller.cancel(None);
        })
    })
    .await
    .expect("the resume kept waiting after its caller gave up");
    // The TS caller aborted with no reason here, so the error is the engine's own abort.
    assert_eq!(err(resumed).to_string(), aborted().to_string());
    let sent = frames(&l, "machine.resume");
    assert_eq!(sent.len(), 1);
    assert_frame(&sent[0], "machine.resume", json!({ "machineId": "c1" }), LinkAsk::default());
}

#[tokio::test]
async fn reads_how_long_the_machine_has_been_quiet() {
    let reading = json!({ "state": "running", "cgroup": "/sys/fs/cgroup/wsp/c1", "upper": "/wsp/run/c1/upper" });
    let loud = reading.clone();
    let l = opened(move |s| match s.op.as_str() {
        "machine.create" => Ok(json!({ "machine": handle_json() })),
        "machine.metrics" => {
            let mut r = loud.clone();
            r["quietForMs"] = json!(143_000);
            Ok(json!({ "reading": r }))
        }
        _ => Ok(json!({})),
    });
    let backend = LinkBackend::open(l.clone()).await.unwrap();
    let machine = backend.create(spec(json!({ "kind": "sandbox" })), None, None).await.unwrap();
    let lifecycle = backend.lifecycle().unwrap();
    assert!(lifecycle.has_quiet_for_ms());
    assert_eq!(lifecycle.quiet_for_ms(&*machine).await.unwrap(), Some(143_000));
    assert_frame(&last(&l), "machine.metrics", json!({ "machineId": "c1" }), LinkAsk::default());
    // A daemon that does not count it answers a reading without the figure, and the host is left with its own
    // clock: undefined and not zero, since zero would read as a workspace that was busy a moment ago.
    let quiet = opened(move |s| match s.op.as_str() {
        "machine.create" => Ok(json!({ "machine": handle_json() })),
        "machine.metrics" => Ok(json!({ "reading": reading.clone() })),
        _ => Ok(json!({})),
    });
    let older = LinkBackend::open(quiet).await.unwrap();
    let machine = older.create(spec(json!({ "kind": "sandbox" })), None, None).await.unwrap();
    assert_eq!(older.lifecycle().unwrap().quiet_for_ms(&*machine).await.unwrap(), None);
}

#[tokio::test]
async fn reads_the_capacity_of_the_computer_on_the_far_side() {
    let capacity = json!({
        "cores": 3, "memMb": 3900, "memRoomMb": 1950, "machineMemMb": 1950, "diskFreeBytes": 10,
        "images": [], "machines": { "running": 1, "paused": 0 },
    });
    let answered = capacity.clone();
    let l = opened(move |s| Ok(if s.op == "machine.capacity" { answered.clone() } else { json!({}) }));
    let backend = LinkBackend::open(l.clone()).await.unwrap();
    let read = backend.capacity().await.unwrap();
    assert_eq!(read, serde_json::from_value::<PlaceCapacity>(capacity).unwrap());
    assert_frame(&last(&l), "machine.capacity", json!({}), LinkAsk::default());
}

#[tokio::test]
async fn gives_up_after_three_attempts_on_a_link_that_keeps_flapping() {
    let l = FakeLink::dialling(
        |s| match s.op.as_str() {
            "machine.backend" => Ok(facts_json()),
            "machine.create" => Ok(json!({ "machine": handle_json() })),
            "machine.putBytes" => Err(plain("connection lost")),
            _ => Ok(json!({})),
        },
        || true,
    );
    let machine = LinkBackend::open(l.clone()).await.unwrap().create(spec(json!({ "kind": "sandbox" })), None, None).await.unwrap();
    let e = err(machine.put_bytes("/root/f", &[1, 2, 3], None).await);
    assert_eq!(e.to_string(), "connection lost");
    let parts = frames(&l, "machine.putBytes");
    assert_eq!(parts.len(), 3);
    let ids: std::collections::BTreeSet<String> = parts.iter().map(|p| p.params["uploadId"].as_str().unwrap().to_string()).collect();
    assert_eq!(ids.len(), 3);
    let seqs: Vec<Value> = parts.iter().map(|p| p.params["seq"].clone()).collect();
    assert_eq!(seqs, vec![json!(0), json!(0), json!(0)]);
    assert!(parts.iter().all(|p| p.params["last"] == json!(true) && p.ask.idempotency_key.is_none()));
}

#[tokio::test]
async fn a_route_with_no_port_is_refused() {
    let (l, backend) = with_machine(|s| {
        Ok(if s.op == "machine.previewUrl" {
            json!({ "reach": { "url": "http://127.0.0.1", "token": "", "expiresAt": 1 } })
        } else {
            json!({})
        })
    })
    .await;
    let machine = backend.create(spec(json!({ "kind": "sandbox" })), None, None).await.unwrap();
    let e = err(machine.preview_url(7070).await);
    assert_eq!(e.to_string(), "c1 answered http://127.0.0.1, which names no port this host can be carried to");
    assert!(l.forwarded.lock().unwrap().is_empty());
    assert_eq!(frames(&l, "machine.previewUrl").len(), 1);
}

#[tokio::test]
async fn frames_match_the_ts_byte_for_byte() {
    let fx = fixture("link_backend.json");
    let mut facts = facts_with_logins();
    facts["capabilities"]["sizes"] = json!([{ "cpu": 2, "memMb": 4096, "rateUsdPerHour": 0.5 }]);
    facts["capabilities"].as_object_mut().unwrap().shift_remove("pauseMode");
    facts.as_object_mut().unwrap().shift_remove("lifecycle");
    facts.as_object_mut().unwrap().shift_remove("baseTemplates");
    serde_json::from_value::<BackendFacts>(facts.clone()).unwrap();
    let l = FakeLink::new(move |s| match s.op.as_str() {
        "machine.backend" => Ok(facts.clone()),
        "machine.create" | "machine.get" => Ok(json!({ "machine": { "id": "c1", "kind": "sandbox", "roads": roads_json() } })),
        "machine.exec" => Ok(json!({ "result": { "exitCode": 0, "stdout": "", "stderr": "" } })),
        "exec" => Ok(json!({ "exitCode": 0, "stdout": "up\n", "stderr": "", "truncated": false })),
        _ => Ok(json!({})),
    });
    let backend = LinkBackend::open(l.clone()).await.unwrap();
    let machine = backend
        .create(spec(json!({ "kind": "sandbox", "labels": { "a": "1" }, "idempotencyKey": "k1", "cpu": 2 })), None, None)
        .await
        .unwrap();
    machine
        .exec("echo", ExecOptions { stdin: Some(vec![0, 255]), idempotency_key: Some("e1".to_string()), timeout_ms: Some(7) })
        .await
        .unwrap();
    machine.put_bytes("/root/f", &[1, 2, 3], None).await.unwrap();
    backend.on_computer("uptime", None).await.unwrap();

    let want = fx["sent"].as_array().unwrap();
    let got = l.sent();
    assert_eq!(got.len(), want.len(), "{:?}", l.ops());
    for (g, w) in got.iter().zip(want) {
        assert_eq!(g.op, w["op"].as_str().unwrap());
        let mut params = g.params.clone();
        if g.op == "machine.putBytes" {
            let id = params["uploadId"].as_str().unwrap().to_string();
            assert!(id.len() == 16 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')), "uploadId {id}");
            params.insert("uploadId".to_string(), w["params"]["uploadId"].clone());
        }
        if g.op == "machine.create" {
            // The TS sends the spec in its caller's key order; the Rust spec is a struct in the schema's order. Same
            // keys and values, compared as a map (an open point on #2446).
            let sorted = |v: &Value| {
                v.as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.clone())).collect::<std::collections::BTreeMap<_, _>>()
            };
            assert_eq!(sorted(&params["spec"]), sorted(&w["params"]["spec"]), "machine.create spec");
            params.insert("spec".to_string(), w["params"]["spec"].clone());
        }
        assert_eq!(serde_json::to_string(&Value::Object(params)).unwrap(), serde_json::to_string(&w["params"]).unwrap(), "{} params", g.op);
        let mut opts = Map::new();
        if let Some(t) = g.ask.timeout_ms {
            opts.insert("timeoutMs".to_string(), json!(t));
        }
        if let Some(k) = &g.ask.idempotency_key {
            opts.insert("idempotencyKey".to_string(), json!(k));
        }
        assert_eq!(serde_json::to_string(&Value::Object(opts)).unwrap(), serde_json::to_string(&w["opts"]).unwrap(), "{} ask", g.op);
    }

    let pricing = backend.pricing();
    let rates =
        vec![pricing.rate_usd_per_hour(Size { cpu: 2.0, mem_mb: 4096.0 }), pricing.rate_usd_per_hour(Size { cpu: 4.0, mem_mb: 4096.0 })];
    let want_rates: Vec<f64> = fx["rates"].as_array().unwrap().iter().map(|r| r.as_f64().unwrap()).collect();
    assert_eq!(rates, want_rates);
}
