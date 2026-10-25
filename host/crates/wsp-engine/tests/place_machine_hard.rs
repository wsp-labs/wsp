// SPDX-License-Identifier: AGPL-3.0-only
//! The computer itself as a machine, against the TS code's own frames (tests/fixtures/roads/place_machine.json, the
//! random names pinned to ab). Spec: spec/place_machine.md.
mod roads_common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use roads_common::*;
use serde_json::{json, Map, Value};
use wsp_engine::machine::{Machine, Size};
use wsp_engine::place_machine::*;

/// The daemon of the oracle's run: the login probe, the login shell's PATH, exit 0 for the rest.
fn daemon(sent: &Sent) -> wsp_engine::errors::Result<Value> {
    let cmd = sent.params.get("cmd").and_then(Value::as_str).unwrap_or("");
    Ok(if cmd.starts_with("uname -s") {
        json!({ "exitCode": 0, "stdout": "Linux\n1000\nmaya\nmaya\n1000\n/home/maya\n/usr/bin:/bin\n", "stderr": "", "truncated": false })
    } else if cmd.contains("-ilc ") {
        json!({ "exitCode": 0, "stdout": "WSP_PATH_BEGIN/home/maya/.local/bin:/usr/bin:/binWSP_PATH_END\n", "stderr": "", "truncated": false })
    } else {
        json!({ "exitCode": 0, "stdout": "", "stderr": "", "truncated": false })
    })
}

/// A frame as the oracle wrote it: op, params, and the ask as `{ timeoutMs, idempotencyKey }`.
fn frame_json(s: &Sent) -> Value {
    let mut opts = Map::new();
    if let Some(t) = s.ask.timeout_ms {
        opts.insert("timeoutMs".into(), json!(t));
    }
    if let Some(k) = &s.ask.idempotency_key {
        opts.insert("idempotencyKey".into(), json!(k));
    }
    json!({ "op": s.op, "params": Value::Object(s.params.clone()), "opts": Value::Object(opts) })
}

#[test]
fn part_bounds_match_the_ts() {
    for case in fixture("place_machine.json")["bounds"].as_array().unwrap() {
        let bytes = case[0].as_u64().unwrap() as usize;
        assert_eq!(place_part_bound_ms(bytes, None), case[1].as_u64().unwrap(), "{bytes}");
        assert_eq!(place_part_bound_ms(bytes, Some(1)), case[2].as_u64().unwrap(), "{bytes} with floor 1");
    }
}

#[tokio::test]
async fn put_frames_match_the_ts() {
    let d = fixture("place_machine.json");
    let link = FakeLink::new(daemon);
    let machine = PlaceMachine::new(link.clone(), "spoo", "/root");
    let landed = machine.put_bytes("/root/x y/f", &vec![1u8; PLACE_PART_BYTES + 3], None).await.unwrap();
    assert_eq!(landed.map(|l| l.pieces), Some(2));
    let sent = link.sent();
    let id = sent[0].ask.idempotency_key.as_deref().unwrap().split('/').next().unwrap().to_string();
    assert!(id.len() == 12 && id.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')), "{id}");
    let want = d["putFrames"].as_array().unwrap();
    assert_eq!(sent.len(), want.len());
    for (s, w) in sent.iter().zip(want) {
        let mut got = frame_json(s);
        if let Some(stdin) = got["params"]["stdin"].as_str().map(str::to_string) {
            got["params"]["stdin"] = json!(format!("{} base64 chars", stdin.len()));
        }
        if let Some(k) = got["opts"]["idempotencyKey"].as_str().map(|k| k.replace(&id, "abababababab")) {
            got["opts"]["idempotencyKey"] = json!(k);
        }
        assert_eq!(serde_json::to_string(&got).unwrap(), serde_json::to_string(w).unwrap());
    }
}

#[tokio::test]
async fn folder_and_cgroup_frames_match_the_ts() {
    let d = fixture("place_machine.json");
    let link = FakeLink::new(daemon);
    let folder = PlaceFolderMachine::new(link.clone(), "spoo", "/home/maya", Some("/usr/bin:/bin"));
    folder.exec("echo hi", Default::default()).await.unwrap();
    let got: Vec<Value> = link.sent().iter().map(frame_json).collect();
    assert_eq!(serde_json::to_string(&got).unwrap(), serde_json::to_string(&d["folderFrames"]).unwrap());
    link.sent.lock().unwrap().clear();
    let grouped = folder.in_cgroup(&wsp_words::thread_cgroup::thread_cgroup("thread-1"), Some("/home/maya/.wsp/bin"));
    grouped.exec("echo hi", Default::default()).await.unwrap();
    // The login was read once: the grouped machine shares it and sends only its own command.
    let got: Vec<Value> = link.sent().iter().map(frame_json).collect();
    assert_eq!(serde_json::to_string(&got).unwrap(), serde_json::to_string(&d["groupedFrames"]).unwrap());
    let framed = folder.framed().await.unwrap();
    assert_eq!(framed.wrap("echo hi"), d["framedWrap"].as_str().unwrap());
    assert_eq!(framed.no_room().as_deref(), d["framedNoRoom"].as_str());
}

#[tokio::test]
async fn a_rejected_frame_also_cleans_up() {
    let link = FakeLink::new(|s: &Sent| {
        let cmd = s.params.get("cmd").and_then(Value::as_str).unwrap_or("");
        if cmd.ends_with(".part1") {
            Err(plain("the link went"))
        } else {
            Ok(json!({ "exitCode": 0, "stdout": "", "stderr": "", "truncated": false }))
        }
    });
    let machine = PlaceMachine::new(link.clone(), "spoo", "/root");
    let e = machine.put_bytes("/root/in.tgz", &vec![7u8; 3 * PLACE_PART_BYTES], None).await.unwrap_err();
    assert_eq!(e.to_string(), "the link went");
    let last = link.sent().last().unwrap().clone();
    assert_eq!(last.params["cmd"], "rm -f '/root/in.tgz'.part{0..2}");
    assert_eq!(last.params["timeoutMs"], 20_000);
    // Two parts went, the cleanup after them; the join never did.
    assert_eq!(link.sent().len(), 3);
}

#[tokio::test]
async fn the_login_is_read_once_and_a_failed_read_is_read_again() {
    let probes = Arc::new(AtomicUsize::new(0));
    let p = probes.clone();
    let link = FakeLink::new(move |s: &Sent| {
        let cmd = s.params.get("cmd").and_then(Value::as_str).unwrap_or("");
        if cmd.starts_with("uname -s") && p.fetch_add(1, Ordering::SeqCst) == 0 {
            return Err(plain("srv is not connected right now"));
        }
        daemon(s)
    });
    let folder = PlaceFolderMachine::new(link.clone(), "spoo", "/home/maya", Some("/usr/bin:/bin"));
    assert_eq!(folder.exec("echo one", Default::default()).await.unwrap_err().to_string(), "srv is not connected right now");
    folder.exec("echo two", Default::default()).await.unwrap();
    folder.exec("echo three", Default::default()).await.unwrap();
    assert_eq!(probes.load(Ordering::SeqCst), 2);
    let cmds: Vec<String> = link.sent().iter().map(|s| s.params["cmd"].as_str().unwrap().to_string()).collect();
    assert!(cmds.iter().any(|c| c.ends_with("echo two")) && cmds.last().unwrap().ends_with("echo three"));
    // Neither failed command went out at all.
    assert!(!cmds.iter().any(|c| c.ends_with("echo one")));
}

#[tokio::test]
async fn the_folder_backend_is_the_computer_as_one_machine() {
    let link = FakeLink::new(daemon);
    let machine: Arc<dyn Machine> = Arc::new(PlaceMachine::new(link, "spoo", "/root"));
    let held = machine.clone();
    let backend = place_folder_backend(Arc::new(move || held.clone()), Size { cpu: 4.0, mem_mb: 8192.0 });
    let caps = serde_json::to_value(backend.capabilities()).unwrap();
    assert_eq!(
        serde_json::to_string(&caps).unwrap(),
        r#"{"liveCloneForks":false,"replacesMachine":false,"previewUrls":false,"signedUrls":false,"callbackRelay":false,"diskSnapshots":false,"images":false,"snapshotsAnyLife":false,"snapshotListing":false,"templates":false,"sizes":[],"kept":true,"copies":false,"ownNetwork":false}"#
    );
    assert!(Arc::ptr_eq(&backend.get("anything").await.unwrap(), &machine));
    assert!(backend.list(None).await.unwrap().is_empty());
    assert_eq!(
        backend.create(serde_json::from_value(json!({ "kind": "sandbox" })).unwrap(), None, None).await.err().unwrap().to_string(),
        NOT_A_WORKSPACE
    );
    assert_eq!(backend.delete_snapshot("x").await.unwrap_err().to_string(), NOT_A_WORKSPACE);
    let pricing = backend.pricing();
    assert_eq!(
        (pricing.rate_usd_per_hour(Size { cpu: 64.0, mem_mb: 1.0 }), pricing.default_size()),
        (0.0, Size { cpu: 4.0, mem_mb: 8192.0 })
    );
    assert!(backend.lifecycle().is_none());
}

#[tokio::test]
async fn a_daemon_frame_goes_up_naming_no_machine() {
    let link = FakeLink::new(|s: &Sent| match s.op.as_str() {
        "git.status" => Ok(json!({ "root": "/home/maya/work" })),
        _ => Err(wsp_engine::errors::EngineError::Other {
            message: "no signed-in command line for github.com".into(),
            code: Some("no-host-cli".into()),
        }),
    });
    let folder = PlaceFolderMachine::new(link.clone(), "spoo", "/home/maya", None);
    let frame = |v: Value| v.as_object().unwrap().clone();
    let reply = folder.daemon_frame(frame(json!({ "op": "git.status", "cwd": "/home/maya/work" }))).await.unwrap();
    assert_eq!(Value::Object(reply), json!({ "ok": true, "root": "/home/maya/work" }));
    let sent = link.sent()[0].clone();
    assert_eq!((sent.op.as_str(), Value::Object(sent.params)), ("git.status", json!({ "cwd": "/home/maya/work" })));
    assert_eq!(sent.ask, Default::default());
    let refused = folder.daemon_frame(frame(json!({ "op": "git.pr", "cwd": "/w" }))).await.unwrap();
    assert_eq!(Value::Object(refused), json!({ "ok": false, "error": "no signed-in command line for github.com", "code": "no-host-cli" }));
    // A place that is not there has no code to give.
    let absent = FakeLink::new(|_s: &Sent| Err(wsp_engine::link_backend::place_absent("srv is not connected right now")));
    let away = PlaceFolderMachine::new(absent, "spoo", "/home/maya", None);
    let reply = away.daemon_frame(frame(json!({ "op": "git.status" }))).await.unwrap();
    assert_eq!(Value::Object(reply), json!({ "ok": false, "error": "srv is not connected right now" }));
}

#[tokio::test]
async fn the_grouped_machine_lands_bytes_through_its_own_framing() {
    let link = FakeLink::new(daemon);
    let folder = PlaceFolderMachine::new(link.clone(), "spoo", "/home/maya", Some("/usr/bin:/bin"));
    let grouped = folder.in_cgroup("/sys/fs/cgroup/wsp-threads/thread-1", None);
    grouped.put_bytes("/home/maya/f", b"abc", None).await.unwrap();
    let join = wsp_words::thread_cgroup::cgroup_join_line("/sys/fs/cgroup/wsp-threads/thread-1");
    let byte_frames: Vec<String> =
        link.sent().iter().map(|s| s.params["cmd"].as_str().unwrap().to_string()).filter(|c| c.contains("/home/maya/f")).collect();
    assert_eq!(byte_frames.len(), 2);
    assert!(byte_frames.iter().all(|c| c.starts_with(&format!("{join}\nexport HOME='/home/maya'"))), "{byte_frames:?}");
    assert_eq!(grouped.id(), "spoo");
}
