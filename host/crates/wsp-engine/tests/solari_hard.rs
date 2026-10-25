// SPDX-License-Identifier: AGPL-3.0-only
//! Hard tests of the Solari backend against the TS's own output (tests/fixtures/oracle/solari.json): the create body
//! and the capabilities byte for byte, token expiries as node decodes them, the query and path encodings, the state
//! words. spec/solari_call.md and spec/solari_machine.md name the rules.
#[path = "rest_common/mod.rs"]
mod common;

use std::fs;
use std::path::Path;
use std::sync::Arc;

use indexmap::IndexMap;
use serde_json::{json, Value};

use common::{reply, FakeFetch, NoWaitClock, Play, Warned};
use wsp_engine::machine::{ExecOptions, MachineBackend, MachineSpec};
use wsp_engine::solari_backend::{preview_token_expiry, state_of, SolariBackend, SolariBackendOptions};

fn oracle() -> Value {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/oracle/solari.json");
    serde_json::from_str(&fs::read_to_string(p).unwrap()).unwrap()
}

fn backend(fetch: Arc<FakeFetch>) -> Arc<SolariBackend> {
    SolariBackend::new(SolariBackendOptions {
        api_key: "k".into(),
        fetch: Some(fetch),
        clock: Some(NoWaitClock::new()),
        warn: Some(Warned::default().sink()),
        ..Default::default()
    })
}

fn spec(v: Value) -> MachineSpec {
    serde_json::from_value(v).unwrap()
}

fn raw_bodies(f: &FakeFetch) -> Vec<String> {
    f.calls().iter().map(|c| c.body.as_ref().map(|b| b.to_string()).unwrap_or_default()).collect()
}

// spec/solari_call.md R1, K1
#[tokio::test]
async fn create_body_writes_the_ts_json_byte_for_byte() {
    let f = FakeFetch::fixed(vec![("POST /sandboxes", reply(201, json!({ "sandboxId": "s1", "kind": "sandbox" })))]);
    let b = backend(f.clone());
    b.create(
        spec(json!({ "kind": "sandbox", "template": "base", "fromSnapshot": "snap_1", "cpu": 2, "memMb": 4096, "diskGb": 20,
            "envs": { "HOME": "/root", "B": "2", "A": "1" }, "labels": { "wsp": "1", "wsp-owner": "h1" }, "onIdle": "pause",
            "idleTimeoutMs": 43_200_000, "idempotencyKey": "k1" })),
        None,
        None,
    )
    .await
    .unwrap();
    b.create(spec(json!({ "kind": "desktop" })), None, None).await.unwrap();
    // Zero and the empty string are falsy in the TS spread: they are left out, not sent.
    b.create(spec(json!({ "kind": "sandbox", "cpu": 0, "memMb": 0, "diskGb": 0, "idleTimeoutMs": 0, "template": "" })), None, None)
        .await
        .unwrap();
    let want: Vec<String> = oracle()["bodies"].as_array().unwrap().iter().map(|b| b.as_str().unwrap().to_string()).collect();
    assert_eq!(raw_bodies(&f), want);
    let calls = f.calls();
    assert_eq!(calls[0].headers[0], ("Authorization".to_string(), "Bearer k".to_string()));
    assert_eq!(calls[0].headers[1], ("Idempotency-Key".to_string(), "k1".to_string()));
    assert_eq!(calls[0].headers[2], ("Content-Type".to_string(), "application/json".to_string()));
    assert_eq!(calls[0].url, "https://api.getsolari.com/sandboxes");
}

// spec/solari_call.md C3
#[test]
fn capabilities_write_the_ts_json_byte_for_byte() {
    let b = backend(FakeFetch::fixed(vec![]));
    assert_eq!(serde_json::to_string(&b.capabilities()).unwrap(), oracle()["capabilities"].as_str().unwrap());
}

// spec/solari_call.md E1
#[test]
fn preview_token_expiry_reads_tokens_as_node_decodes_them() {
    for c in oracle()["expiry"].as_array().unwrap() {
        assert_eq!(preview_token_expiry(c["token"].as_str().unwrap(), 1000), c["at"].as_i64().unwrap(), "{}", c["token"]);
    }
}

// spec/solari_call.md G2
#[tokio::test]
async fn a_state_the_provider_names_that_the_table_never_learned_reads_running() {
    let words = [
        ("starting", "starting"),
        ("running", "running"),
        ("paused", "paused"),
        ("archived", "gone"),
        ("releasing", "gone"),
        ("gone", "gone"),
        ("archiving", "running"),
        ("", "running"),
        ("Paused", "running"),
    ];
    for (word, want) in words {
        assert_eq!(serde_json::to_value(state_of(word)).unwrap(), json!(want), "{word:?}");
    }
    // A view with no state at all is a word the table never learned.
    let f = FakeFetch::fixed(vec![("GET /sandboxes/x", reply(200, json!({ "sandboxId": "x", "kind": "sandbox" })))]);
    let m = backend(f).get("x").await.unwrap();
    assert_eq!(serde_json::to_value(m.state().await.unwrap()).unwrap(), json!("running"));
}

// spec/solari_call.md L1
#[tokio::test]
async fn list_encodes_labels_as_url_search_params_does() {
    let f = FakeFetch::handled(|seen| {
        Some(if seen.query.contains("cursor=") {
            reply(200, json!({ "sandboxes": [] , "nextCursor": "" }))
        } else {
            reply(200, json!({ "nextCursor": "c 2&x" }))
        })
    });
    let b = backend(f.clone());
    let labels: IndexMap<String, String> =
        [("wsp", "1"), ("wsp-owner", "a b&c=d/é")].iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    // A page with no sandboxes key is an empty page; an empty nextCursor ends the walk.
    assert_eq!(b.list(Some(&labels)).await.unwrap(), vec![]);
    let calls = f.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].path, "/sandboxes");
    assert_eq!(calls[0].query, "metadata.wsp=1&metadata.wsp-owner=a+b%26c%3Dd%2F%C3%A9");
    assert_eq!(calls[1].query, "metadata.wsp=1&metadata.wsp-owner=a+b%26c%3Dd%2F%C3%A9&cursor=c+2%26x");
    let empty = FakeFetch::fixed(vec![("GET /sandboxes", reply(200, json!({ "sandboxes": [] })))]);
    backend(empty.clone()).list(None).await.unwrap();
    assert_eq!(empty.calls()[0].url, "https://api.getsolari.com/sandboxes");
}

// spec/solari_machine.md M3
#[tokio::test]
async fn download_and_upload_urls_encode_the_path_as_encode_uri_component() {
    let f = FakeFetch::handled(|seen| {
        Some(match seen.path.as_str() {
            "/sandboxes/a%3Ab.c" => reply(200, json!({ "sandboxId": "a:b.c", "kind": "sandbox", "state": "running" })),
            _ => reply(200, json!({ "url": format!("https://signed{}", seen.query) })),
        })
    });
    let b = backend(f.clone());
    let m = b.get("a:b.c").await.unwrap();
    assert_eq!(m.download_url("/root/my dir/a+b.txt").await.unwrap(), "https://signedpath=%2Froot%2Fmy%20dir%2Fa%2Bb.txt");
    assert_eq!(m.upload_url("/root/x").await.unwrap(), "https://signedpath=%2Froot%2Fx");
    assert_eq!(
        f.keys(),
        vec!["GET /sandboxes/a%3Ab.c", "GET /sandboxes/a%3Ab.c/files/download-url", "GET /sandboxes/a%3Ab.c/files/upload-url"]
    );
}

// spec/solari_machine.md X1
#[tokio::test]
async fn exec_body_keys_and_default_timeout() {
    let f = FakeFetch::fixed(vec![
        ("POST /sandboxes", reply(201, json!({ "sandboxId": "x", "kind": "sandbox" }))),
        ("POST /sandboxes/x/exec", reply(200, json!({ "exitCode": 3, "stdout": "o", "stderr": "e" }))),
    ]);
    let m = backend(f.clone()).create(spec(json!({ "kind": "sandbox" })), None, None).await.unwrap();
    let res = m
        .exec("true", ExecOptions { timeout_ms: None, idempotency_key: Some("ignored".into()), stdin: Some(b"ignored".to_vec()) })
        .await
        .unwrap();
    assert_eq!((res.exit_code, res.stdout.as_str(), res.stderr.as_str()), (3.into(), "o", "e"));
    let body = f.calls()[1].body.clone().unwrap();
    assert_eq!(body.to_string(), r#"{"cmd":"bash","args":["-c","export HOME=/root USER=root\ntrue"],"timeoutMs":20000}"#);
    assert_eq!(f.calls()[1].header("Idempotency-Key"), None);
}

// spec/solari_call.md R6, R7: a 503 is retried twice, then thrown with its words; the sleeps are the backoff's.
#[tokio::test]
async fn a_transient_status_is_answered_three_times_then_thrown() {
    let f = FakeFetch::new(vec![("GET /sandboxes/x", vec![Play::Reply(503, "".into(), vec![])])]);
    let clock = NoWaitClock::new();
    let b = SolariBackend::new(SolariBackendOptions {
        api_key: "k".into(),
        fetch: Some(f.clone()),
        clock: Some(clock.clone()),
        ..Default::default()
    });
    let err = b.get("x").await.err().unwrap();
    assert_eq!(f.sent("GET /sandboxes/x"), 3);
    assert_eq!(err.to_string(), "transient (503)");
    assert_eq!(err.status(), Some(503));
    // backoff_ms(1) and backoff_ms(2) with the clock's draw of 0.5: 1000 + 125, 2000 + 125.
    assert_eq!(clock.waits(), vec![1125, 2125]);
}
