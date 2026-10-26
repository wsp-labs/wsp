// SPDX-License-Identifier: AGPL-3.0-only
//! What `tests/branch_of_record.rs` drives the unit with: a `BranchCtx` whose gh and git answers are scripted per
//! command (every command logged in order, with its timeout), whose device door logs the mint and revoke it is asked
//! for, whose `env_exec` logs the script and the environment, and whose `persist` keeps the records written.
#![allow(dead_code)]

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::{json, Value};
use tokio::sync::Semaphore;

use wsp_core::clock::Clock;
use wsp_core::devices::{MintMore, PairedDevice};
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_engine::errors::EngineError;
use wsp_engine::machine::{
    ExecOptions, Machine, MachineKind, MachineLife, MachineState, RunOptions, SnapshotOptions,
};
use wsp_machines::projects::branch_of_record::{
    BranchCtx, BranchOfRecordArea, MarketplaceFile, OnRecordEntry,
};
use wsp_wire::types::js_number::JsInt;
use wsp_wire::views::workspace::{BranchOfRecord, ProjectView};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin};
use wsp_wire::wire::machine_link::ExecResult;
use wsp_words::exit::HostError;

use super::rest_support::{TestClock, Warned};

/// What a scripted command answers: its result, or the rejection of the exec itself.
#[derive(Clone)]
pub enum Reply {
    Done(i64, String, String),
    Rejected(String),
}

/// A command that exits 0 and prints `stdout`.
pub fn ok(stdout: &str) -> Reply {
    Reply::Done(0, stdout.to_string(), String::new())
}

/// A command that exits `code` printing `stderr`.
pub fn fails(code: i64, stderr: &str) -> Reply {
    Reply::Done(code, String::new(), stderr.to_string())
}

/// An exec that rejects, as a spawn that never started does.
pub fn rejects(message: &str) -> Reply {
    Reply::Rejected(message.to_string())
}

pub fn exec_result(code: i64, stdout: &str, stderr: &str) -> ExecResult {
    ExecResult {
        exit_code: JsInt::from(code),
        stdout: stdout.to_string(),
        stderr: stderr.to_string(),
    }
}

struct Scripted {
    cmd: String,
    reply: Reply,
    gate: Option<Rc<Semaphore>>,
}

/// One `deviceDoor.mint` call.
#[derive(Clone, Debug, PartialEq)]
pub struct Mint {
    pub name: String,
    pub scope: Value,
    pub now: i64,
    pub more: MintMore,
}

/// One `envExec` call.
#[derive(Clone, Debug, PartialEq)]
pub struct EnvRun {
    pub machine: String,
    pub script: String,
    pub env: IndexMap<String, String>,
    pub timeout_ms: u64,
}

/// A machine that is only an id: nothing in the unit execs on it, `env_exec` is the context's.
pub struct StubMachine(pub String);

fn nothing<T>() -> Result<T, EngineError> {
    Err(EngineError::Other {
        message: "the stub machine answers nothing".to_string(),
        code: None,
    })
}

#[async_trait(?Send)]
impl Machine for StubMachine {
    fn id(&self) -> &str {
        &self.0
    }
    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }
    async fn exec(&self, _cmd: &str, _opts: ExecOptions) -> Result<ExecResult, EngineError> {
        nothing()
    }
    async fn run(&self, _script: &str, _opts: RunOptions) -> Result<ExecResult, EngineError> {
        nothing()
    }
    async fn snapshot(
        &self,
        _name: &str,
        _life: MachineLife,
        _opts: SnapshotOptions,
    ) -> Result<String, EngineError> {
        nothing()
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> Result<(), EngineError> {
        nothing()
    }
    async fn resume(
        &self,
        _signal: Option<&wsp_engine::cancel::Cancel>,
    ) -> Result<(), EngineError> {
        nothing()
    }
    async fn kill(&self) -> Result<(), EngineError> {
        nothing()
    }
    async fn state(&self) -> Result<MachineState, EngineError> {
        nothing()
    }
    async fn download_url(&self, _path: &str) -> Result<String, EngineError> {
        nothing()
    }
    async fn upload_url(&self, _path: &str) -> Result<String, EngineError> {
        nothing()
    }
}

/// The context: every member logs what it was asked and answers as scripted.
pub struct FakeCtx {
    pub clock: Rc<TestClock>,
    pub warned: Warned,
    pub home: String,
    pub wired: String,
    /// Place id to the name a person reads.
    pub place_names: RefCell<IndexMap<String, String>>,
    scripts: RefCell<Vec<Scripted>>,
    /// Every command `exec_here` was given, with its timeout, in order.
    pub exec_log: RefCell<Vec<(String, u64)>>,
    pub persisted: RefCell<Vec<WorkspaceRecord>>,
    pub live: RefCell<Vec<Rc<RefCell<WorkspaceRecord>>>>,
    pub mints: RefCell<Vec<Mint>>,
    pub revokes: RefCell<Vec<String>>,
    pub revoke_fails: RefCell<Option<String>>,
    pub env_runs: RefCell<Vec<EnvRun>>,
    env_replies: RefCell<VecDeque<Result<ExecResult, HostError>>>,
    /// Every `on_record_lines` call as `(checkout, branch, origin, held)`.
    pub on_record_calls: RefCell<Vec<(String, String, String, bool)>>,
    /// What the context did, in order: `mint <name>`, `env_exec`, `revoke <id>`, `persist`, and whatever a test adds.
    pub events: RefCell<Vec<String>>,
    pub files: RefCell<IndexMap<String, String>>,
    pub read_log: RefCell<Vec<String>>,
    pub marketplaces: RefCell<Vec<MarketplaceFile>>,
    minted: Cell<u32>,
    /// When set, `turn_road_of` answers this error (the TS `moduleOf(record.kind).turnRoad` throws on an unwired kind).
    pub turn_road_failure: RefCell<Option<HostError>>,
}

impl FakeCtx {
    pub fn new(clock: Rc<TestClock>) -> Rc<FakeCtx> {
        Rc::new(FakeCtx {
            clock,
            warned: Warned::default(),
            home: "/home/zing".to_string(),
            wired: "pl_here".to_string(),
            place_names: RefCell::new(IndexMap::from([
                ("pl_boat".to_string(), "Boat".to_string()),
                ("pl_here".to_string(), "This Mac".to_string()),
            ])),
            scripts: RefCell::default(),
            exec_log: RefCell::default(),
            persisted: RefCell::default(),
            live: RefCell::default(),
            mints: RefCell::default(),
            revokes: RefCell::default(),
            revoke_fails: RefCell::default(),
            env_runs: RefCell::default(),
            env_replies: RefCell::default(),
            on_record_calls: RefCell::default(),
            events: RefCell::default(),
            files: RefCell::default(),
            read_log: RefCell::default(),
            marketplaces: RefCell::default(),
            minted: Cell::new(0),
            turn_road_failure: RefCell::new(None),
        })
    }

    /// Answers the exact command text. A later script for the same command wins.
    pub fn script(&self, cmd: &str, reply: Reply) {
        self.scripts.borrow_mut().insert(
            0,
            Scripted {
                cmd: cmd.to_string(),
                reply,
                gate: None,
            },
        );
    }

    /// Answers the command only once the gate opens; the command is logged when it is asked.
    pub fn script_held(&self, cmd: &str, reply: Reply) -> Rc<Semaphore> {
        let gate = Rc::new(Semaphore::new(0));
        self.scripts.borrow_mut().insert(
            0,
            Scripted {
                cmd: cmd.to_string(),
                reply,
                gate: Some(gate.clone()),
            },
        );
        gate
    }

    pub fn commands(&self) -> Vec<String> {
        self.exec_log
            .borrow()
            .iter()
            .map(|(c, _)| c.clone())
            .collect()
    }

    pub fn mints(&self) -> Vec<Mint> {
        self.mints.borrow().clone()
    }

    pub fn events(&self) -> Vec<String> {
        self.events.borrow().clone()
    }

    pub fn env_runs(&self) -> Vec<EnvRun> {
        self.env_runs.borrow().clone()
    }

    pub fn persisted(&self) -> Vec<WorkspaceRecord> {
        self.persisted.borrow().clone()
    }

    /// What the next `env_exec` answers; none queued answers exit 0 with no output.
    pub fn env_reply(&self, reply: Result<ExecResult, HostError>) {
        self.env_replies.borrow_mut().push_back(reply);
    }

    pub fn put_file(&self, path: &str, text: &str) {
        self.files
            .borrow_mut()
            .insert(path.to_string(), text.to_string());
    }

    /// A catalog agent's marketplace file: `path` as the catalog writes it (`~/...`), and the repos its text names, one
    /// per whitespace-separated word of the text that holds a `/`.
    pub fn marketplace(&self, path: &str) {
        self.marketplaces.borrow_mut().push(MarketplaceFile {
            path: path.to_string(),
            github_repos: Rc::new(|text: &str| {
                text.split_whitespace()
                    .filter(|w| w.contains('/'))
                    .map(str::to_string)
                    .collect()
            }),
        });
    }

    pub fn open(&self, gate: &Rc<Semaphore>) {
        gate.add_permits(1000);
    }
}

/// What `brokered_git_env` returns here: the token and the branch it was asked for.
pub fn env_of(token: &str, branch: Option<&str>) -> IndexMap<String, String> {
    IndexMap::from([
        ("WSP_FAKE_TOKEN".to_string(), token.to_string()),
        (
            "WSP_FAKE_BRANCH".to_string(),
            branch.unwrap_or("").to_string(),
        ),
    ])
}

/// What `on_record_lines` returns here, whatever the arguments, as two lines.
pub fn on_record_of(checkout: &str, branch: &str, origin: &str, held: bool) -> Vec<String> {
    vec![
        format!("on-record {checkout} {branch} {origin} held={held}"),
        "echo on-record-end".to_string(),
    ]
}

#[async_trait(?Send)]
impl BranchCtx for FakeCtx {
    fn clock(&self) -> Rc<dyn Clock> {
        self.clock.clone()
    }
    fn warn(&self, line: &str) {
        self.warned.push(line);
    }
    fn broker_ready(&self) -> bool {
        true
    }
    async fn exec_here(&self, cmd: &str, timeout_ms: u64) -> Result<ExecResult, HostError> {
        self.exec_log
            .borrow_mut()
            .push((cmd.to_string(), timeout_ms));
        let found = self
            .scripts
            .borrow()
            .iter()
            .find(|s| s.cmd == cmd)
            .map(|s| (s.reply.clone(), s.gate.clone()));
        let (reply, gate) = found.unwrap_or((fails(1, "gh: Not Found (HTTP 404)"), None));
        if let Some(gate) = gate {
            gate.acquire().await.expect("the gate stays open").forget();
        }
        match reply {
            Reply::Done(code, stdout, stderr) => Ok(exec_result(code, &stdout, &stderr)),
            Reply::Rejected(message) => Err(HostError::new(message)),
        }
    }
    async fn persist(&self, record: &WorkspaceRecord) -> Result<(), HostError> {
        self.events.borrow_mut().push("persist".to_string());
        self.persisted.borrow_mut().push(record.clone());
        Ok(())
    }
    fn live_records(&self) -> Vec<Rc<RefCell<WorkspaceRecord>>> {
        self.live.borrow().clone()
    }
    async fn mint(
        &self,
        name: &str,
        scope: ThreadScope,
        now: i64,
        more: MintMore,
    ) -> Result<PairedDevice, HostError> {
        self.events.borrow_mut().push(format!("mint {name}"));
        let n = self.minted.get() + 1;
        self.minted.set(n);
        self.mints.borrow_mut().push(Mint {
            name: name.to_string(),
            scope: serde_json::to_value(&scope).expect("a scope writes"),
            now,
            more,
        });
        let id = format!("dev_{n}");
        let view = serde_json::from_value(json!({ "id": id, "name": name, "createdAt": "2023-11-14T22:13:20.000Z", "lastSeenAt": "2023-11-14T22:13:20.000Z" })).expect("a device view");
        Ok(PairedDevice {
            device_id: id,
            device_token: format!("tok_{n}"),
            device: view,
        })
    }
    async fn revoke(&self, id: &str) -> Result<bool, HostError> {
        self.events.borrow_mut().push(format!("revoke {id}"));
        self.revokes.borrow_mut().push(id.to_string());
        match self.revoke_fails.borrow().clone() {
            Some(message) => Err(HostError::new(message)),
            None => Ok(true),
        }
    }
    fn turn_road_of(&self, record: &WorkspaceRecord) -> Result<MintMore, HostError> {
        if let Some(e) = self.turn_road_failure.borrow().clone() {
            return Err(e);
        }
        let road = if record.kind == WorkspaceKind::Cloud {
            WorkspaceOrigin::Relayed
        } else {
            WorkspaceOrigin::Here
        };
        Ok(MintMore {
            road: Some(road),
            aside: false,
        })
    }
    fn place_name(&self, place: &str) -> String {
        self.place_names
            .borrow()
            .get(place)
            .cloned()
            .unwrap_or_else(|| place.to_string())
    }
    fn wired(&self) -> String {
        self.wired.clone()
    }
    fn home_dir(&self) -> String {
        self.home.clone()
    }
    async fn read_file(&self, path: &str) -> Result<String, HostError> {
        self.read_log.borrow_mut().push(path.to_string());
        self.files.borrow().get(path).cloned().ok_or_else(|| {
            HostError::new(format!("ENOENT: no such file or directory, open '{path}'"))
        })
    }
    fn marketplace_files(&self) -> Vec<MarketplaceFile> {
        self.marketplaces.borrow().clone()
    }
    fn brokered_git_env(
        &self,
        token: &str,
        of_record: Option<&BranchOfRecord>,
    ) -> IndexMap<String, String> {
        env_of(token, of_record.map(|r| r.branch.as_str()))
    }
    fn on_record_lines(
        &self,
        checkout: &str,
        of_record: &BranchOfRecord,
        origin: &str,
        held: bool,
    ) -> Vec<String> {
        self.on_record_calls.borrow_mut().push((
            checkout.to_string(),
            of_record.branch.clone(),
            origin.to_string(),
            held,
        ));
        on_record_of(checkout, &of_record.branch, origin, held)
    }
    async fn env_exec(
        &self,
        machine: Arc<dyn Machine>,
        script: &str,
        env: IndexMap<String, String>,
        timeout_ms: u64,
    ) -> Result<ExecResult, HostError> {
        self.events.borrow_mut().push("env_exec".to_string());
        self.env_runs.borrow_mut().push(EnvRun {
            machine: machine.id().to_string(),
            script: script.to_string(),
            env,
            timeout_ms,
        });
        self.env_replies
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| Ok(exec_result(0, "", "")))
    }
}

/// The unit over a fresh context and a clock at `now`.
pub struct Rig {
    pub ctx: Rc<FakeCtx>,
    pub area: Rc<BranchOfRecordArea>,
    pub clock: Rc<TestClock>,
}

impl Rig {
    pub fn new() -> Rig {
        Rig::at(1_700_000_000_000)
    }
    pub fn at(now: i64) -> Rig {
        let clock = TestClock::at(now);
        let ctx = FakeCtx::new(clock.clone());
        let area = BranchOfRecordArea::new(ctx.clone());
        Rig { ctx, area, clock }
    }
}

/// Lets every task that can run, run: a pending read gets as far as the command it waits on.
pub async fn settle() {
    for _ in 0..64 {
        tokio::task::yield_now().await;
    }
}

/// A workspace record, the keys of `extra` laid over a cloud workspace `ws_1` called "pricing page".
pub fn record(extra: Value) -> WorkspaceRecord {
    let mut v = json!({
        "id": "ws_1", "name": "pricing page", "machineId": "bx_1", "phase": "running", "kind": "cloud", "golden": "snap_g",
        "createdAt": "2026-10-01T00:00:00.000Z", "project": "pr_1", "spec": {}, "size": { "cpu": 2, "memMb": 4096 }, "firstLife": true,
    });
    for (k, x) in extra.as_object().expect("an object") {
        v[k] = x.clone();
    }
    serde_json::from_value(v).expect("a workspace record")
}

pub fn branch_of(v: Value) -> BranchOfRecord {
    serde_json::from_value(v).expect("a branch of record")
}

/// A live workspace on the context: the record shared with the context's live map, and its entry.
pub fn entry_of(
    ctx: &FakeCtx,
    extra: Value,
    opens_thread: Option<&str>,
    of_record_held: bool,
) -> (Rc<RefCell<WorkspaceRecord>>, OnRecordEntry) {
    let shared = Rc::new(RefCell::new(record(extra)));
    ctx.live.borrow_mut().push(shared.clone());
    let machine: Arc<dyn Machine> = Arc::new(StubMachine("bx_1".to_string()));
    (
        shared.clone(),
        OnRecordEntry {
            record: shared,
            opens_thread: opens_thread.map(str::to_string),
            of_record_held,
            machine,
        },
    )
}

/// The project a land reads: acme/web at `/root/web`.
pub fn project(remote: &str) -> ProjectView {
    serde_json::from_value(json!({
        "id": "pr_1", "name": "web", "computer": "pl_boat", "source": { "kind": "github", "repo": "acme/web" }, "path": "/root/web",
        "remote": remote, "defaultBranch": "main", "memoryKey": "web", "memoryDir": "/tmp/memory", "createdAt": "2026-10-01T00:00:00.000Z",
    }))
    .expect("a project view")
}
