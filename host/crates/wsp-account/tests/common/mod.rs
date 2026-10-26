// SPDX-License-Identifier: AGPL-3.0-only
//! What every account test shares: the oracle fixtures, records and projects built from the TS's plain objects, a
//! fake machine, a live entry, and a one-thread runtime that runs spawned work to quiet before a test reads state.
#![allow(dead_code, unused_imports)]

mod entry;
mod errors;
pub use entry::*;
pub use errors::*;

use std::sync::Arc;

use serde_json::{json, Value};

use wsp_account::account::core_types::Entry;
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_engine::machine::Machine;
use wsp_wire::views::workspace::ProjectView;
use wsp_words::exit::HostError;

/// `tests/fixtures/oracle/<name>.json`, the TS's own answers.
pub fn oracle(name: &str) -> Value {
    let path = format!("{}/tests/fixtures/oracle/{name}.json", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))).unwrap()
}

fn merged(defaults: Value, given: &Value) -> Value {
    let mut out = defaults;
    for (k, v) in given.as_object().expect("an object") {
        out[k] = v.clone();
    }
    out
}

/// A record from the TS's plain object, the keys the TS fixtures leave out filled as a fresh record has them.
pub fn record(given: &Value) -> WorkspaceRecord {
    let mut full = merged(
        json!({ "golden": "", "createdAt": "2026-10-25T00:00:00.000Z", "spec": {}, "size": { "cpu": 2, "memMb": 4096 }, "firstLife": false }),
        given,
    );
    // wsp-wire's WorkspaceAgents still requires `spawn` (written from a3b870a6e); the cut dropped it. Drift.
    if full["agents"].is_object() && full["agents"].get("spawn").is_none() {
        full["agents"]["spawn"] = json!(true);
    }
    serde_json::from_value(full).unwrap_or_else(|e| panic!("record {given}: {e}"))
}

/// A project from the TS's plain object; the oracle's `repo` key (what its fake compared) is dropped.
pub fn project(given: &Value) -> ProjectView {
    let mut given = given.clone();
    given.as_object_mut().unwrap().remove("repo");
    // A git source names its url; the oracle's plain objects leave it to the remote.
    if given["source"]["kind"] == json!("git") && given["source"].get("url").is_none() {
        given["source"]["url"] = given["remote"].clone();
    }
    let full =
        merged(json!({ "defaultBranch": "main", "memoryKey": "", "memoryDir": "", "createdAt": "2026-10-25T00:00:00.000Z" }), &given);
    serde_json::from_value(full).unwrap_or_else(|e| panic!("project {given}: {e}"))
}

/// A live entry over a record and a machine, every in-memory field at its start.
pub fn live_with(record: WorkspaceRecord, machine: Arc<dyn Machine>) -> Entry {
    entry_over(record, machine)
}

/// A live entry over a record, on a fake machine named by the record's `machineId`.
pub fn live(record: WorkspaceRecord) -> Entry {
    let machine = FakeMachine::new(&record.machine_id);
    live_with(record, machine)
}

/// A host error as the TS `Error` the oracle wrote: `{ message, fix?, kind? }`, compared whole (README A4).
pub fn error_json(e: &HostError) -> Value {
    let mut out = json!({ "message": e.message });
    if let Some(fix) = &e.fix {
        out["fix"] = json!(fix);
    }
    if let Some(kind) = &e.kind {
        out["kind"] = json!(kind_word(kind));
    }
    out
}

/// Runs `f` on one thread with a local set, as the host does (chapter 03).
pub fn run<F: std::future::Future<Output = ()>>(f: F) {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    tokio::task::LocalSet::new().block_on(&rt, f);
}

/// Lets every spawned task run until none is ready: the TS's `await settled()`.
pub async fn settle() {
    for _ in 0..64 {
        tokio::task::yield_now().await;
    }
}
