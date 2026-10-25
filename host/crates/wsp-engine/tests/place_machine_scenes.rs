// SPDX-License-Identifier: AGPL-3.0-only
//! A place's frames on edge inputs, held to what the TS sent (tests/fixtures/scenes/roads.json `places`, from
//! oracle/roads_scenes.oracle.ts; spec/place_machine.md rules PL-1..PL-3): every frame byte for byte, the upload's
//! random id as the TS's pinned `abababababab`.
mod roads_common;

use roads_common::*;
use serde_json::{json, Map, Value};
use wsp_engine::machine::{ExecOptions, Machine};
use wsp_engine::place_machine::*;

fn want(name: &str) -> Value {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/scenes/roads.json")).unwrap();
    serde_json::from_str::<Value>(&text).unwrap()["places"].as_array().unwrap().iter().find(|s| s["name"] == name).unwrap().clone()
}

fn daemon(part_fails: bool) -> impl Fn(&Sent) -> wsp_engine::errors::Result<Value> + Send + Sync + 'static {
    move |sent: &Sent| {
        let cmd = sent.params.get("cmd").and_then(Value::as_str).unwrap_or("");
        Ok(if cmd.starts_with("uname -s") {
            json!({ "exitCode": 0, "stdout": "Linux\n1000\nmaya\nmaya\n1000\n/home/maya\n/usr/bin:/bin\n", "stderr": "", "truncated": false })
        } else if cmd.contains("-ilc ") {
            json!({ "exitCode": 0, "stdout": "WSP_PATH_BEGIN/home/maya/.local/bin:/usr/bin:/binWSP_PATH_END\n", "stderr": "", "truncated": false })
        } else if part_fails && cmd.contains(".part0") {
            json!({ "exitCode": 1, "stdout": "", "stderr": "disk full", "truncated": false })
        } else {
            json!({ "exitCode": 0, "stdout": "", "stderr": "", "truncated": false })
        })
    }
}

/// Every frame as the oracle wrote it, the upload id swapped for the TS's.
fn same_frames(link: &FakeLink, w: &Value) {
    let got: Vec<String> = link
        .sent()
        .iter()
        .map(|s| {
            let mut opts = Map::new();
            if let Some(t) = s.ask.timeout_ms {
                opts.insert("timeoutMs".into(), json!(t));
            }
            if let Some(k) = &s.ask.idempotency_key {
                let (id, rest) = k.split_once('/').unwrap_or((k.as_str(), ""));
                assert!(id.len() == 12 && id.chars().all(|c| c.is_ascii_hexdigit()), "{k}");
                opts.insert("idempotencyKey".into(), json!(format!("abababababab/{rest}")));
            }
            serde_json::to_string(&json!({ "op": s.op, "params": Value::Object(s.params.clone()), "opts": Value::Object(opts) })).unwrap()
        })
        .collect();
    let want: Vec<String> = w["frames"].as_array().unwrap().iter().map(|f| serde_json::to_string(f).unwrap()).collect();
    assert_eq!(got, want);
}

/// PL-1: no bytes are one part: `max(1, ceil(0 / 1 MiB))`, its frame carrying `"stdin":""`, then the join; one piece.
#[tokio::test]
async fn no_bytes_are_one_part_with_an_empty_stdin() {
    let w = want("no_bytes_are_one_part_with_an_empty_stdin");
    let link = FakeLink::new(daemon(false));
    let landed = PlaceMachine::new(link.clone(), "spoo", "/root").put_bytes("/root/empty", &[], None).await.unwrap();
    same_frames(&link, &w);
    assert_eq!(json!({ "pieces": landed.map(|l| l.pieces) }), w["answer"]);
}

/// PL-2: on a folder machine every frame of an upload, the parts, the join and the cleanup after a refused part, goes
/// through its own `exec`, so each is wrapped as the login; the stdin never is.
#[tokio::test]
async fn a_folder_machines_parts_join_and_cleanup_ride_as_the_login() {
    let w = want("a_folder_machines_parts_join_and_cleanup_ride_as_the_login");
    let link = FakeLink::new(daemon(true));
    let folder = PlaceFolderMachine::new(link.clone(), "spoo", "/home/maya", Some("/usr/bin:/bin"));
    let e = folder.put_bytes("/home/maya/f", &[1, 2], None).await.unwrap_err();
    same_frames(&link, &w);
    assert_eq!(e.to_string(), w["err"].as_str().unwrap());
}

/// PL-3: `path_first` is tested against `None` only: `Some("")` writes its shells' startup files under `/` and puts
/// `''` in front of PATH, as the TS does.
#[tokio::test]
async fn an_empty_path_first_is_a_path_first() {
    let w = want("an_empty_path_first_is_a_path_first");
    let link = FakeLink::new(daemon(false));
    let folder = PlaceFolderMachine::new(link.clone(), "spoo", "/home/maya", Some("/usr/bin:/bin"));
    let grouped = folder.in_cgroup(&wsp_words::thread_cgroup::thread_cgroup("thread-1"), Some(""));
    grouped.exec("echo hi", ExecOptions::default()).await.unwrap();
    same_frames(&link, &w);
}
