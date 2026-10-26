// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the deep-input round (wsp-labs/wsp-map#2448): readers that clone a capped tree of objects. A Value of objects
//! clones to 627 levels on a 1 MiB debug stack, under the 1,000 the port keeps.
mod common;

use common::fake_host::*;
use futures::executor::block_on;
use wsp_collect::history::reader::try_json;
use wsp_collect::history::{read_store, StoreOptions};
use wsp_collect::project::reader::ProjectFile;
use wsp_collect::project::readers::package_json;

fn in_child(name: &str, body: fn()) {
    if std::env::var("REVIEW_2448_CHILD").as_deref() == Ok(name) {
        std::thread::Builder::new().stack_size(1024 * 1024).spawn(body).unwrap().join().expect("the body held");
        return;
    }
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env("REVIEW_2448_CHILD", name)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "the whole process died ({}): {}",
        out.status,
        String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("")
    );
}

fn objects(n: usize) -> String {
    format!("{}1{}", "{\"a\":".repeat(n), "}".repeat(n))
}

// package_json.rs:20,24 clone each table it reads; a repo's own package.json reaches it. node (package-json.ts) reads it.
fn package_json_cap_objects_body() {
    let text = format!(r#"{{"packageManager":"pnpm@9","engines":{{"k":{},"j":"v"}}}}"#, objects(498));
    assert!(!package_json::read(&ProjectFile { path: "package.json".into(), text }).is_empty());
}

#[test]
fn a_package_json_holding_object_levels_to_the_cap_is_read() {
    in_child("a_package_json_holding_object_levels_to_the_cap_is_read", package_json_cap_objects_body);
}

// history/codex.rs:59 copies an `arguments` object; a Codex rollout line carries a tool call's input. node counts 2 calls.
fn codex_rollout_cap_objects_body() {
    use wsp_collect::history::codex::CodexReader;
    let rollout = [
        r#"{"type":"session_meta","payload":{"id":"t1","cwd":"/Users/dev/proj"}}"#.to_string(),
        format!(r#"{{"type":"response_item","payload":{{"type":"function_call","name":"exec_command","arguments":{}}}}}"#, objects(498)),
        r#"{"type":"response_item","payload":{"type":"function_call","name":"exec_command","arguments":"{\"cmd\":\"ls\"}"}}"#.to_string(),
    ]
    .join("\n");
    let host = fake_host(FakeLaptop::new().file("~/.codex/sessions/2026/06/01/rollout-2026-06-01T10-00-00-t1.jsonl", rollout));
    let usage = block_on(read_store(&host, &CodexReader, "/Users/dev/.codex/sessions", StoreOptions::default())).unwrap();
    assert_eq!(usage.calls, 2);
}

#[test]
fn a_codex_rollout_call_holding_object_levels_to_the_cap_is_counted() {
    in_child("a_codex_rollout_call_holding_object_levels_to_the_cap_is_counted", codex_rollout_cap_objects_body);
}

// history/hermes.rs:32,42 clone the tool call. node counts 1 call.
fn hermes_cap_objects_body() {
    use wsp_collect::history::hermes::hermes_calls;
    let list = try_json(&format!(r#"[{{"type":"function","function":{{"name":"terminal","arguments":{}}}}}]"#, objects(497)))
        .expect("under the cap");
    assert_eq!(hermes_calls("h1", Some(&list), None).len(), 1);
}

#[test]
fn a_hermes_tool_call_holding_object_levels_to_the_cap_is_counted() {
    in_child("a_hermes_tool_call_holding_object_levels_to_the_cap_is_counted", hermes_cap_objects_body);
}
