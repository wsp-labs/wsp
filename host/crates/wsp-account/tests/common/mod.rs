// SPDX-License-Identifier: AGPL-3.0-only
//! What every account test shares: the oracle fixtures, records and projects built from the TS's plain objects, a
//! fake machine, a live entry, and a one-thread runtime that runs spawned work to quiet before a test reads state.
#![allow(dead_code)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use async_trait::async_trait;
use serde_json::{json, Value};

use wsp_account::account::core_types::{AccountResult, ExecAnswer, EntryMachine, Live, LiveEntry, Served};
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_wire::views::workspace::ProjectView;
use wsp_words::exit::{HostError, Kind};

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
    let mut full = merged(json!({ "golden": "", "createdAt": "2026-10-25T00:00:00.000Z", "spec": {}, "size": { "cpu": 2, "memMb": 4096 }, "firstLife": false }), given);
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
    let full = merged(json!({ "defaultBranch": "main", "memoryKey": "", "memoryDir": "", "createdAt": "2026-10-25T00:00:00.000Z" }), &given);
    serde_json::from_value(full).unwrap_or_else(|e| panic!("project {given}: {e}"))
}

/// A machine that answers what the test set: its state, its exec, and the frames its computer serves.
pub struct FakeMachine {
    pub id: String,
    pub state: RefCell<Result<String, String>>,
    pub served: RefCell<Option<Served>>,
    pub execs: RefCell<Vec<String>>,
    pub exec_answer: RefCell<Result<ExecAnswer, String>>,
}

impl FakeMachine {
    pub fn new(id: &str) -> Rc<FakeMachine> {
        Rc::new(FakeMachine {
            id: id.to_string(),
            state: RefCell::new(Ok("running".into())),
            served: RefCell::new(None),
            execs: RefCell::new(Vec::new()),
            exec_answer: RefCell::new(Ok(ExecAnswer::default())),
        })
    }
}

#[async_trait(?Send)]
impl EntryMachine for FakeMachine {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn daemon_frame(&self) -> Option<Served> {
        self.served.borrow().clone()
    }
    async fn state(&self) -> AccountResult<String> {
        self.state.borrow().clone().map_err(|m| plain(&m).into())
    }
    async fn exec(&self, cmd: &str, _timeout_ms: i64) -> AccountResult<ExecAnswer> {
        self.execs.borrow_mut().push(cmd.to_string());
        self.exec_answer.borrow().clone().map_err(|m| plain(&m).into())
    }
}

/// A live entry over a record and a machine, every in-memory field at its start.
pub fn live_with(record: WorkspaceRecord, machine: Rc<dyn EntryMachine>) -> Live {
    Rc::new(LiveEntry {
        record: RefCell::new(record),
        machine,
        creating: Cell::new(false),
        waking: Cell::new(false),
        deleting: Cell::new(false),
        napping: Cell::new(false),
        generation: Cell::new(0),
        wake_ask: Cell::new(None),
        wake_said: RefCell::new(None),
        delete_said: RefCell::new(None),
        checkout: RefCell::new(None),
        pr: RefCell::new(None),
        pr_reading: RefCell::new(None),
        pr_poll: RefCell::new(None),
        pr_poll_ms: Cell::new(None),
        pr_none_at: RefCell::new(None),
    })
}

/// A live entry over a record, on a fake machine named by the record's `machineId`.
pub fn live(record: WorkspaceRecord) -> Live {
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

/// The TS word of a kind, written here so a test reads it without wsp-words' own table.
pub fn kind_word(kind: &Kind) -> String {
    match kind {
        Kind::Usage => "usage",
        Kind::Invalid => "invalid",
        Kind::NotFound => "not-found",
        Kind::ProcessesLeft => "processes-left",
        Kind::Auth => "auth",
        Kind::Ticket => "ticket",
        Kind::Conflict => "conflict",
        Kind::Concurrency => "concurrency",
        Kind::StartLimit => "startLimit",
        Kind::Plan => "plan",
        Kind::Missing => "missing",
        Kind::SnapshotUnavailable => "snapshotUnavailable",
        Kind::Transient => "transient",
        Kind::Unknown => "unknown",
        Kind::NotFirstLife => "notFirstLife",
        Kind::MachineAlive => "machineAlive",
        Kind::SnapshotFailed => "snapshotFailed",
        Kind::CredentialOnBuilder => "credentialOnBuilder",
        Kind::NoProvider => "noProvider",
        Kind::VaultTooLarge => "vaultTooLarge",
        Kind::Exists => "exists",
        Kind::SshInclude => "sshInclude",
        Kind::HostKey => "host-key",
        Kind::SshBehind => "sshBehind",
        Kind::Unreachable => "unreachable",
        Kind::Refused => "refused",
        Kind::Reauth => "reauth",
        Kind::Other(s) => s,
    }
    .to_string()
}

/// A host error with the message alone, as `new Error(message)` (no wsp-words constructor needed).
pub fn plain(message: &str) -> HostError {
    HostError { message: message.to_string(), fix: None, kind: None, errors: None, warnings: None, left: None, marks: vec![] }
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
