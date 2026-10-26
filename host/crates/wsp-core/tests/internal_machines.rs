// SPDX-License-Identifier: AGPL-3.0-only
//! The stand-in machines, the create fingerprint, the body reader and the session file stamp:
//! runtime/src/types/internal.ts:694-1066 at 65f6b89c7, spec/internal-machines.md. Expected values are the TS's own
//! answers (tests/fixtures/core_types/internal.json).
mod common;

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use common::{fixture, wire};
use futures::StreamExt;
use serde_json::json;
use wsp_adapters::port::SessionFile;
use wsp_core::types::internal::*;
use wsp_engine::errors::{EngineError, ErrorKind};
use wsp_engine::machine::*;
use wsp_wire::wire::machine_link::MachineSpec;
use wsp_words::exit::HostError;

// internal.ts:867-873: the sha256 of the spec without its key and its createdAt label, labels moved last and always
// written, so two attempts of one request match and a reordered spec matches.
#[test]
fn fingerprint_matches_the_ts_hash() {
    let f = &fixture("internal")["fingerprint"];
    let spec = |v: serde_json::Value| -> MachineSpec { wire(&v) };
    assert_eq!(fingerprint(&spec(json!({ "kind": "sandbox", "cpu": 2, "memMb": 4096 }))), f["plain"].as_str().unwrap());
    assert_eq!(f["plain"], f["hashOf"], "no labels writes labels: {{}}");
    let keyed =
        spec(json!({ "kind": "sandbox", "cpu": 2, "memMb": 4096, "idempotencyKey": "k1", "labels": { "createdAt": "now", "wsp": "1" } }));
    let keyed2 =
        spec(json!({ "kind": "sandbox", "cpu": 2, "memMb": 4096, "idempotencyKey": "k2", "labels": { "createdAt": "later", "wsp": "1" } }));
    assert_eq!(fingerprint(&keyed), f["keyed"].as_str().unwrap());
    assert_eq!(fingerprint(&keyed2), fingerprint(&keyed));
    let reordered = spec(json!({ "labels": { "wsp": "1" }, "kind": "sandbox", "cpu": 2, "memMb": 4096 }));
    assert_eq!(fingerprint(&reordered), f["labelsFirst"].as_str().unwrap());
    let empty = spec(json!({ "kind": "sandbox", "cpu": 2, "memMb": 4096, "labels": {} }));
    assert_eq!(fingerprint(&empty), f["emptyLabels"].as_str().unwrap());
}

// internal.ts:881-888: a pid that answers signal 0 is alive; a reaped child is not.
#[test]
fn pid_alive_reads_signal_zero() {
    assert!(pid_alive(std::process::id() as i32));
    let mut child = std::process::Command::new("true").spawn().unwrap();
    let pid = child.id() as i32;
    child.wait().unwrap();
    assert!(!pid_alive(pid));
}

async fn calls(m: &dyn Machine) -> Vec<(&'static str, Result<String, EngineError>)> {
    vec![
        ("exec", m.exec("x", ExecOptions::default()).await.map(|_| "ok".into())),
        ("run", m.run("x", RunOptions { deadline_ms: 1, ..Default::default() }).await.map(|_| "ok".into())),
        ("pause", m.pause(None).await.map(|_| "ok".into())),
        ("resume", m.resume(None).await.map(|_| "ok".into())),
        ("kill", m.kill().await.map(|_| "ok".into())),
        ("state", m.state().await.map(|s| serde_json::to_value(s).unwrap().as_str().unwrap().to_string())),
        ("downloadUrl", m.download_url("/p").await),
        ("uploadUrl", m.upload_url("/p").await),
    ]
}

// internal.ts:968-998: every call fails as a missing machine, status 404, but kill, which succeeds, and state, which
// answers gone.
#[tokio::test(flavor = "current_thread")]
async fn a_dead_machine_answers_gone_and_refuses_the_rest() {
    let f = &fixture("internal")["machines"]["dead"];
    let m = dead_machine("m1");
    assert_eq!(m.id(), "m1");
    assert_eq!(serde_json::to_value(m.kind()).unwrap(), f["kind"]);
    assert_eq!(m.stream_url(), None);
    assert!(!is_absent_machine(m.as_ref()));
    for (name, got) in calls(m.as_ref()).await {
        let want = &f["calls"][name];
        match got {
            Ok(v) => assert_eq!(json!(if v == "ok" { serde_json::Value::Null } else { json!(v) }), want["ok"], "{name}"),
            Err(EngineError::Provider(e)) => {
                assert_eq!(e.message, want["err"].as_str().unwrap(), "{name}");
                assert_eq!(e.kind, ErrorKind::Missing, "{name}");
                assert_eq!(e.status, 404, "{name}");
            }
            Err(other) => panic!("{name}: not a missing refusal: {other:?}"),
        }
    }
}

// internal.ts:1015-1031: every call, kill and state included, fails with the caller's away error; the handle reads as
// the stand-in.
#[tokio::test(flavor = "current_thread")]
async fn an_absent_machine_refuses_every_call_with_the_away_error() {
    let f = &fixture("internal")["machines"]["absent"];
    let asked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let a = asked.clone();
    let m = absent_machine(
        "m2",
        wire(&json!("sandbox")),
        Arc::new(move || {
            a.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            EngineError::PlaceAbsent { message: "the computer is away".into() }
        }),
    );
    assert_eq!(m.id(), "m2");
    assert!(is_absent_machine(m.as_ref()));
    for (name, got) in calls(m.as_ref()).await {
        match got {
            Err(EngineError::PlaceAbsent { message }) => assert_eq!(message, f["calls"][name]["err"].as_str().unwrap(), "{name}"),
            other => panic!("{name}: not the away error: {other:?}"),
        }
    }
    assert_eq!(asked.load(std::sync::atomic::Ordering::SeqCst), 8, "away is asked once per call");
}

fn broke(message: &str) -> HostError {
    HostError {
        message: message.into(),
        fix: None,
        kind: None,
        errors: None,
        warnings: None,
        left: None,
        host_key: None,
        code: None,
        status: None,
        marks: vec![],
    }
}

/// A body of chunks that counts how many were pulled.
fn body(
    chunks: Vec<Result<&'static [u8], &'static str>>,
    pulled: Rc<Cell<usize>>,
) -> impl futures::Stream<Item = Result<Vec<u8>, HostError>> + Unpin {
    futures::stream::iter(chunks).map(move |c| {
        pulled.set(pulled.get() + 1);
        c.map(|b| b.to_vec()).map_err(broke)
    })
}

// internal.ts:1034-1051: chunks are read while fewer than cap bytes came, cut at cap, decoded with replacement; a body
// that fails answers what came first.
#[tokio::test(flavor = "current_thread")]
async fn read_body_up_to_matches_the_ts() {
    let f = &fixture("internal")["bodies"];
    let cases: Vec<(&str, Vec<Result<&'static [u8], &'static str>>, usize)> = vec![
        ("under", vec![Ok(b"hello "), Ok(b"world")], 2048),
        ("cut", vec![Ok(b"hello "), Ok(b"world")], 8),
        ("cutExact", vec![Ok(b"hello "), Ok(b"world")], 6),
        ("splitChar", vec![Ok("ab😀cd".as_bytes())], 4),
        ("dies", vec![Ok(b"part"), Err("reset")], 100),
        ("empty", vec![], 10),
        ("zeroCap", vec![Ok(b"x")], 0),
    ];
    for (name, chunks, cap) in cases {
        let mut b = body(chunks, Rc::new(Cell::new(0)));
        assert_eq!(read_body_up_to(&mut b, cap).await, f[name].as_str().unwrap(), "{name}");
    }
}

// internal.ts:1040: nothing past the cap is pulled.
#[tokio::test(flavor = "current_thread")]
async fn read_body_up_to_pulls_no_chunk_past_the_cap() {
    let pulled = Rc::new(Cell::new(0));
    let mut b = body(vec![Ok(b"hello "), Ok(b"world"), Ok(b"!")], pulled.clone());
    assert_eq!(read_body_up_to(&mut b, 6).await, "hello ");
    assert_eq!(pulled.get(), 1);
    let pulled = Rc::new(Cell::new(0));
    let mut b = body(vec![Ok(b"x")], pulled.clone());
    assert_eq!(read_body_up_to(&mut b, 0).await, "");
    assert_eq!(pulled.get(), 0);
}

fn layout() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (path, size) in [
        ("a/s-abc.jsonl", 3),
        ("B/s-abc.jsonl", 5),
        ("2026-09-30/s-abc.jsonl", 7),
        ("2026-10-01/s-other.jsonl", 9),
        ("flat-s-abc.jsonl", 11),
    ] {
        let p = dir.path().join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, "x".repeat(size)).unwrap();
    }
    dir
}

fn rel(root: &std::path::Path, p: Option<String>) -> Option<String> {
    p.map(|p| p[root.to_str().unwrap().len()..].to_string())
}

// internal.ts:700-716: depth 0 is a file in the folder; deeper, folders newest name first by localeCompare, which puts
// "B" before "a" where byte order would not; a folder that does not read is none.
#[test]
fn find_session_file_walks_folders_in_locale_order() {
    let f = &fixture("internal")["files"];
    let dir = layout();
    let root = dir.path();
    let at = |depth| rel(root, find_session_file(root.to_str().unwrap(), "s-abc.jsonl", depth));
    assert_eq!(at(1).as_deref(), f["depth1"].as_str());
    assert_eq!(at(0).as_deref(), f["depth0"].as_str());
    assert_eq!(at(2), None);
    assert_eq!(find_session_file(root.join("nope").to_str().unwrap(), "s-abc.jsonl", 1), None);
}

// internal.ts:711: the whole descending localeCompare order, as node sorted it: each folder wins once every folder
// before it lacks the file.
#[test]
fn find_session_file_follows_node_locale_order() {
    let order: Vec<String> =
        fixture("internal")["files"]["order"].as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect();
    let dir = tempfile::tempdir().unwrap();
    for name in &order {
        let d = dir.path().join(name);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("s.jsonl"), "x").unwrap();
    }
    for name in &order {
        let found = find_session_file(dir.path().to_str().unwrap(), "s.jsonl", 1).expect("a folder still holds it");
        assert_eq!(found, dir.path().join(name).join("s.jsonl").to_str().unwrap());
        std::fs::remove_file(&found).unwrap();
    }
}

// internal.ts:718-724.
#[test]
fn size_of_is_minus_one_for_a_missing_file() {
    let dir = layout();
    assert_eq!(size_of(dir.path().join("B/s-abc.jsonl").to_str().unwrap()), 5);
    assert_eq!(size_of(dir.path().join("nope").to_str().unwrap()), fixture("internal")["files"]["sizeMissing"].as_i64().unwrap());
}

fn file_at(root: &std::path::Path, depth: u32) -> SessionFile {
    SessionFile { folder: root.to_str().unwrap().to_string(), name: "s-abc.jsonl".into(), depth }
}

// internal.ts:726-738: a stamp is the path found and its size, or no path and -1; the file stands while both hold;
// an agent that names no file is taken at its word.
#[test]
fn a_session_file_stamp_holds_while_path_and_size_hold() {
    let dir = layout();
    let root = dir.path();
    let at = file_at(root, 1);
    assert_eq!(stamp_session_file(None, None), None);
    let stamp = stamp_session_file(Some(&at), None).unwrap();
    assert_eq!(stamp, SessionFileStamp { path: Some(root.join("B/s-abc.jsonl").to_str().unwrap().into()), size: 5 });
    assert!(same_session_file(Some(&at), Some(&stamp)));
    assert!(same_session_file(None, Some(&stamp)));
    assert!(same_session_file(Some(&at), None));
    std::fs::write(root.join("B/s-abc.jsonl"), "longer now").unwrap();
    assert!(!same_session_file(Some(&at), Some(&stamp)));
    let none = stamp_session_file(Some(&file_at(root, 2)), None).unwrap();
    assert_eq!(none, SessionFileStamp { path: None, size: -1 });
    assert!(same_session_file(Some(&file_at(root, 2)), Some(&none)));
    let deep = root.join("x/y");
    std::fs::create_dir_all(&deep).unwrap();
    std::fs::write(deep.join("s-abc.jsonl"), "z").unwrap();
    assert!(!same_session_file(Some(&file_at(root, 2)), Some(&none)), "a file where none was is a change");
}

// internal.ts:728: a known path is stamped as it is, its size -1 where it is gone, and never searched for.
#[test]
fn a_known_path_is_stamped_without_a_search() {
    let dir = layout();
    let root = dir.path();
    let known = root.join("a/s-abc.jsonl");
    let stamp = stamp_session_file(Some(&file_at(root, 1)), Some(known.to_str().unwrap())).unwrap();
    assert_eq!(stamp, SessionFileStamp { path: Some(known.to_str().unwrap().into()), size: 3 });
    let gone = root.join("gone.jsonl");
    let stamp = stamp_session_file(Some(&file_at(root, 1)), Some(gone.to_str().unwrap())).unwrap();
    assert_eq!(stamp, SessionFileStamp { path: Some(gone.to_str().unwrap().into()), size: -1 });
}

// internal.ts:1002.
#[test]
fn the_released_backend_says_its_line() {
    assert_eq!(RELEASED_LINE, "this sub-thread's machine was deleted at its cloud, so there is none to ask");
    let _ = released_backend();
}
