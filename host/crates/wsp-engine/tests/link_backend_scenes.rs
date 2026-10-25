// SPDX-License-Identifier: AGPL-3.0-only
//! Link frames no other test asserts, held to what the TS sent (tests/fixtures/scenes/roads.json `links`, from
//! oracle/roads_scenes.oracle.ts; spec/link_backend.md rules L-1..L-4): every frame's op, params byte for byte, and
//! ask, then the answer or the failure's words.
mod roads_common;

use indexmap::IndexMap;
use roads_common::*;
use serde_json::{json, Map, Value};
use wsp_engine::errors::EngineError;
use wsp_engine::link_backend::LinkBackend;
use wsp_engine::machine::{ExecOptions, Machine, MachineBackend};

fn want(name: &str) -> Value {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/scenes/roads.json")).unwrap();
    serde_json::from_str::<Value>(&text).unwrap()["links"].as_array().unwrap().iter().find(|s| s["name"] == name).unwrap().clone()
}

fn facts() -> Value {
    json!({
        "offer": "docker",
        "capabilities": { "liveCloneForks": false, "replacesMachine": true, "previewUrls": true, "signedUrls": false, "callbackRelay": true, "diskSnapshots": true, "images": true, "snapshotsAnyLife": false, "snapshotListing": true, "templates": true, "kept": false, "copies": true, "ownNetwork": true, "sizes": [{ "cpu": 2, "memMb": 4096, "rateUsdPerHour": 0.5 }] },
        "pricing": { "defaultSize": { "cpu": 2, "memMb": 4096 }, "snapshotStorage": { "freeGb": 0, "usdPerGbMonth": 0, "billedFrom": "" } },
    })
}

/// The scene's link: the oracle's answers per op, the route its preview frame answers.
fn link(preview_url: String) -> std::sync::Arc<FakeLink> {
    FakeLink::new(move |s| match s.op.as_str() {
        "machine.backend" => Ok(facts()),
        "machine.get" => Ok(
            json!({ "machine": { "id": "c1", "kind": "sandbox", "roads": { "previewUrl": true, "daemonAnswers": true, "putBytes": true, "describe": true, "facts": true, "metrics": true } } }),
        ),
        "machine.list" => Ok(json!({ "machines": [] })),
        "machine.exec" => Ok(json!({ "result": { "exitCode": 0, "stdout": "", "stderr": "" } })),
        "machine.state" => Ok(json!({ "state": "running" })),
        "machine.downloadUrl" | "machine.uploadUrl" => Ok(json!({ "url": "https://u" })),
        "machine.describe" => Ok(json!({ "shape": { "cpu": 2 } })),
        "machine.previewUrl" => Ok(json!({ "reach": { "url": preview_url, "token": "", "expiresAt": 0 } })),
        "git.refused" => Err(EngineError::Other { message: String::new(), code: None }),
        "git.coded" => Err(EngineError::Other { message: "no remote".into(), code: Some("no-remote".into()) }),
        "git.ok" => Ok(json!({ "ok": false, "x": 1 })),
        _ => Ok(json!({})),
    })
}

/// Every frame sent after the machine was got, against the TS's, params and ask byte for byte.
fn same_frames(l: &FakeLink, skip: usize, w: &Value) {
    let got: Vec<Value> = l.sent()[skip..]
        .iter()
        .map(|s| {
            let mut opts = Map::new();
            if let Some(t) = s.ask.timeout_ms {
                opts.insert("timeoutMs".into(), json!(t));
            }
            if let Some(k) = &s.ask.idempotency_key {
                opts.insert("idempotencyKey".into(), json!(k));
            }
            json!({ "op": s.op, "params": serde_json::to_string(&s.params).unwrap(), "opts": serde_json::to_string(&opts).unwrap() })
        })
        .collect();
    let want: Vec<Value> = w["sent"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| json!({ "op": s["op"], "params": serde_json::to_string(&s["params"]).unwrap(), "opts": serde_json::to_string(&s["opts"]).unwrap() }))
        .collect();
    assert_eq!(got, want);
}

async fn opened(name: &str) -> (Value, std::sync::Arc<FakeLink>, LinkBackend, std::sync::Arc<dyn Machine>, usize) {
    let w = want(name);
    let l = link(w["previewUrl"].as_str().unwrap().to_string());
    let backend = LinkBackend::open(l.clone()).await.unwrap();
    let machine = backend.get("c1").await.unwrap();
    let skip = l.sent().len();
    (w, l, backend, machine, skip)
}

/// L-1: pause, kill, state, the two URLs, describe, metrics, get and list each send their frame with the machine named
/// last, no ask options; `list(None)` sends `{}` and `list(Some({}))` sends `{"labels":{}}`.
#[tokio::test]
async fn frames_of_the_roads_no_other_test_sends() {
    let (w, l, backend, m, skip) = opened("frames_of_the_roads_no_other_test_sends").await;
    m.pause(None).await.unwrap();
    m.kill().await.unwrap();
    let state = m.state().await.unwrap();
    let down = m.download_url("/a b").await.unwrap();
    let up = m.upload_url("/x").await.unwrap();
    let shape = m.describe().await.unwrap();
    m.metrics().await.unwrap();
    backend.get("c1").await.unwrap();
    backend.list(None).await.unwrap();
    backend.list(Some(&IndexMap::new())).await.unwrap();
    same_frames(&l, skip, &w);
    assert_eq!(json!({ "state": state, "down": down, "up": up, "shape": serde_json::to_value(shape).unwrap() }), w["answer"]);
}

/// L-2: an empty stdin is bytes: `"stdin":""` is sent, between `timeoutMs` and `machineId`.
#[tokio::test]
async fn an_empty_stdin_is_sent_empty() {
    let (w, l, _b, m, skip) = opened("an_empty_stdin_is_sent_empty").await;
    m.exec("cat", ExecOptions { stdin: Some(Vec::new()), ..Default::default() }).await.unwrap();
    same_frames(&l, skip, &w);
}

/// L-3: the frame's own `machineId` is overwritten in its place, else appended; a reply's own `ok` overrides the value
/// in the first place; a refusal is `{ ok: false, error }` with `code` only where it is a string.
#[tokio::test]
async fn a_daemon_frame_keeps_its_own_machine_id_place_and_its_replys_ok() {
    let (w, l, _b, m, skip) = opened("a_daemon_frame_keeps_its_own_machine_id_place_and_its_replys_ok").await;
    let frame = |v: Value| v.as_object().cloned().unwrap();
    let answers = [
        m.daemon_frame(frame(json!({ "op": "git.ok", "machineId": "other", "a": 1 }))).await.unwrap(),
        m.daemon_frame(frame(json!({ "a": 2, "op": "git.refused" }))).await.unwrap(),
        m.daemon_frame(frame(json!({ "op": "git.coded" }))).await.unwrap(),
    ];
    same_frames(&l, skip, &w);
    let got: Vec<String> = answers.iter().map(|a| serde_json::to_string(a).unwrap()).collect();
    let want: Vec<String> = w["answer"].as_array().unwrap().iter().map(|a| serde_json::to_string(a).unwrap()).collect();
    assert_eq!(got, want);
}

async fn route(name: &str) {
    let (w, l, _b, m, skip) = opened(name).await;
    let got = m.preview_url(3000).await;
    same_frames(&l, skip, &w);
    let forwarded: Vec<u64> = l.forwarded.lock().unwrap().iter().map(|p| *p as u64).collect();
    assert_eq!(json!(forwarded), w["forwarded"]);
    match got {
        Ok(r) => assert_eq!(serde_json::to_value(r).unwrap(), w["answer"]),
        Err(e) => assert_eq!(e.to_string(), w["err"].as_str().unwrap()),
    }
}

/// L-4: the place's port is WHATWG `URL.port`: the scheme's own port reads `""`, which names no port; a scheme with no
/// default keeps it; `[::1]:8080` is 8080; a URL the reader refuses fails `Invalid URL` with nothing forwarded.
#[tokio::test]
async fn a_route_on_the_schemes_own_port_names_no_port() {
    route("a_route_on_the_schemes_own_port_names_no_port").await;
}

#[tokio::test]
async fn a_route_on_a_scheme_with_no_default_keeps_its_port() {
    route("a_route_on_a_scheme_with_no_default_keeps_its_port").await;
}

#[tokio::test]
async fn a_route_on_an_ipv6_loopback_reads_its_port() {
    route("a_route_on_an_ipv6_loopback_reads_its_port").await;
}

#[tokio::test]
async fn a_route_the_url_reader_refuses_is_invalid_url() {
    route("a_route_the_url_reader_refuses_is_invalid_url").await;
}

/// L-5: a part that fails asks `dials_back` only below the third attempt (link-backend.ts:266 reads `attempt >= 3`
/// first), with no wait between attempts and each attempt from seq 0 under a new id: three parts sent, two asks.
#[tokio::test]
async fn a_failing_upload_asks_whether_the_link_dials_back_twice_and_sends_three_times() {
    let asked = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let a = asked.clone();
    let l = FakeLink::dialling(
        |s| match s.op.as_str() {
            "machine.backend" => Ok(facts()),
            "machine.get" => Ok(
                json!({ "machine": { "id": "c1", "kind": "sandbox", "roads": { "previewUrl": false, "daemonAnswers": false, "putBytes": true, "describe": false, "facts": false, "metrics": false } } }),
            ),
            _ => Err(EngineError::Other { message: "link dropped".into(), code: None }),
        },
        move || {
            a.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            true
        },
    );
    let backend = LinkBackend::open(l.clone()).await.unwrap();
    let m = backend.get("c1").await.unwrap();
    let e = m.put_bytes("/root/f", &[1, 2, 3], None).await.unwrap_err();
    assert_eq!(e.to_string(), "link dropped");
    assert_eq!(asked.load(std::sync::atomic::Ordering::SeqCst), 2);
    let parts: Vec<Sent> = l.sent().into_iter().filter(|s| s.op == "machine.putBytes").collect();
    assert_eq!(parts.len(), 3);
    let ids: std::collections::BTreeSet<String> = parts.iter().map(|p| p.params["uploadId"].as_str().unwrap().to_string()).collect();
    assert_eq!(ids.len(), 3, "a new id each attempt");
    assert!(parts.iter().all(|p| p.params["seq"] == json!(0) && p.params["last"] == json!(true)));
}
