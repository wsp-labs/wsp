// SPDX-License-Identifier: AGPL-3.0-only
//! The fakes machines-b's tests drive the five areas with: a clock the test moves, a machine and a backend that keep
//! every call and answer as told, and one host that implements every area's context trait over shared state. The TS
//! tests' `sub-fixture.ts`, `stub-backend.ts` and `fake-clock.ts`, ported to what the areas read.
#![allow(dead_code)]

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::future::Future;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use futures::FutureExt;
use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Value};
use tokio::sync::Semaphore;

use wsp_engine::box_backend::BoxLimits;
use wsp_engine::lifecycle::{Workspace, WorkspaceHooks, WorkspaceInitial};
use wsp_engine::errors::EngineError;
use wsp_engine::machine::{
    BackendPricing, Capabilities, ExecOptions, ExecResult, Lifecycle, LifecycleBudgets, Machine, MachineBackend, MachineKind, MachineLife,
    MachineListRow, MachineOffers, MachineSpec, MachineState, RunOptions, Size, SnapshotOptions, SnapshotStoragePricing, Waiting, BackendOffers, RootDisk,
};
use wsp_core::clock::Cancel;
use wsp_core::types::internal::{KindModule, SessionEntry, SubStartRecord, GONE_REASON, SUB_STARTS};
use wsp_core::types::wiring::{LiveWorkspace, StageReport, SubRecord, WorkspaceRecord};
use wsp_machines::machines::area_error::{AreaError, AreaResult};
use wsp_machines::machines::machines::Entry;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::workspace::DaemonReachView;
use wsp_words::exit::HostError;

// ---------------------------------------------------------------------------------------------------------------
// Running a test on one thread, as the host runs.

/// Runs a test body on a current-thread runtime inside a `LocalSet`, so `spawn_local` works as the host's tasks do.
pub fn run<F: Future<Output = ()>>(body: F) {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, body);
}

/// Lets every spawned task run until it waits on something outside (the TS tests' `settle`).
pub async fn settle() {
    for _ in 0..50 {
        tokio::task::yield_now().await;
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The clock (fake-clock.ts): `advance` runs each due timer at its own time, in order.

pub struct FakeClock {
    now: Cell<f64>,
    seq: Cell<u64>,
    timers: RefCell<Vec<(f64, u64, Option<Box<dyn FnOnce()>>)>>,
}

impl FakeClock {
    pub fn new(start: f64) -> Rc<FakeClock> {
        Rc::new(FakeClock { now: Cell::new(start), seq: Cell::new(0), timers: RefCell::new(Vec::new()) })
    }
    pub fn now(&self) -> f64 {
        self.now.get()
    }
    pub fn schedule(self: &Rc<Self>, ms: f64, f: Box<dyn FnOnce()>) -> Cancel {
        let id = self.seq.get() + 1;
        self.seq.set(id);
        self.timers.borrow_mut().push((self.now.get() + ms.max(0.0), id, Some(f)));
        let me = Rc::downgrade(self);
        Rc::new(move || {
            if let Some(c) = me.upgrade() {
                c.timers.borrow_mut().retain(|(_, i, _)| *i != id);
            }
        })
    }
    /// How many timers are armed.
    pub fn pending(&self) -> usize {
        self.timers.borrow().len()
    }
    /// The due times of the armed timers, soonest first.
    pub fn due(&self) -> Vec<f64> {
        let mut d: Vec<f64> = self.timers.borrow().iter().map(|t| t.0).collect();
        d.sort_by(|a, b| a.partial_cmp(b).unwrap());
        d
    }
    /// Moves time on by `ms`, firing every timer it passes at its own time, letting tasks run after each.
    pub async fn advance(&self, ms: f64) {
        let end = self.now.get() + ms;
        loop {
            let next = {
                let t = self.timers.borrow();
                t.iter().filter(|x| x.0 <= end).min_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then(a.1.cmp(&b.1))).map(|x| (x.0, x.1))
            };
            let Some((at, id)) = next else { break };
            self.now.set(at.max(self.now.get()));
            let f = {
                let mut t = self.timers.borrow_mut();
                let i = t.iter().position(|x| x.1 == id).unwrap();
                t.remove(i).2
            };
            if let Some(f) = f {
                f();
            }
            settle().await;
        }
        self.now.set(end);
        settle().await;
    }
}

// ---------------------------------------------------------------------------------------------------------------
// A machine (stub-backend.ts's StubMachine): every command kept, each answered by the first rule that takes it.

/// How a rule answers a command.
#[derive(Clone)]
pub enum Reply {
    Out { code: i64, stdout: String, stderr: String },
    Fail(String),
    /// Waits for the gate to open, then answers the inner reply.
    Gated(Arc<Semaphore>, Box<Reply>),
    /// Never answers.
    Never,
}

pub fn ok(stdout: &str) -> Reply {
    Reply::Out { code: 0, stdout: stdout.to_string(), stderr: String::new() }
}

pub fn exit(code: i64, stderr: &str) -> Reply {
    Reply::Out { code, stdout: String::new(), stderr: stderr.to_string() }
}

type Rule = Box<dyn Fn(&str) -> Option<Reply> + Send + Sync>;

/// A gate a test opens once.
pub fn gate() -> Arc<Semaphore> {
    Arc::new(Semaphore::new(0))
}

pub fn open(g: &Arc<Semaphore>) {
    g.add_permits(1_000);
}

pub struct FakeMachine {
    pub id: String,
    pub exec_log: Mutex<Vec<String>>,
    pub rules: Mutex<Vec<Rule>>,
    pub files: Mutex<Vec<(String, Vec<u8>)>>,
    /// What `read_bytes` answers by path; a path not here fails.
    pub readable: Mutex<IndexMap<String, Reply>>,
    pub read_bytes_bytes: Mutex<IndexMap<String, Vec<u8>>>,
    pub killed: Mutex<bool>,
    pub kill_fails: Mutex<Option<String>>,
    pub paused: Mutex<bool>,
    pub offers: Mutex<MachineOffers>,
    pub daemon_answers: Mutex<VecDeque<Result<bool, String>>>,
    pub daemon_exits: Mutex<VecDeque<Result<Option<String>, String>>>,
    pub start_daemon: Mutex<VecDeque<Reply>>,
    pub starts_asked: Mutex<u32>,
}

impl FakeMachine {
    pub fn new(id: &str) -> Arc<FakeMachine> {
        Arc::new(FakeMachine {
            id: id.to_string(),
            exec_log: Mutex::new(Vec::new()),
            rules: Mutex::new(Vec::new()),
            files: Mutex::new(Vec::new()),
            readable: Mutex::new(IndexMap::new()),
            read_bytes_bytes: Mutex::new(IndexMap::new()),
            killed: Mutex::new(false),
            kill_fails: Mutex::new(None),
            paused: Mutex::new(false),
            offers: Mutex::new(MachineOffers { put_bytes: true, read_bytes: true, ..MachineOffers::default() }),
            daemon_answers: Mutex::new(VecDeque::new()),
            daemon_exits: Mutex::new(VecDeque::new()),
            start_daemon: Mutex::new(VecDeque::new()),
            starts_asked: Mutex::new(0),
        })
    }
    /// A rule tried before every rule given so far.
    pub fn on(&self, rule: impl Fn(&str) -> Option<Reply> + Send + Sync + 'static) {
        self.rules.lock().unwrap().insert(0, Box::new(rule));
    }
    /// Answers every command containing `needle` with `reply`.
    pub fn answer(&self, needle: &str, reply: Reply) {
        let needle = needle.to_string();
        self.on(move |cmd| cmd.contains(&needle).then(|| reply.clone()));
    }
    pub fn log(&self) -> Vec<String> {
        self.exec_log.lock().unwrap().clone()
    }
    pub fn ran(&self, needle: &str) -> bool {
        self.log().iter().any(|c| c.contains(needle))
    }
    pub fn killed(&self) -> bool {
        *self.killed.lock().unwrap()
    }
    pub fn files(&self) -> Vec<(String, Vec<u8>)> {
        self.files.lock().unwrap().clone()
    }
    async fn reply(&self, r: Reply) -> Result<ExecResult, EngineError> {
        let mut r = r;
        loop {
            match r {
                Reply::Out { code, stdout, stderr } => return Ok(ExecResult { exit_code: code.into(), stdout, stderr }),
                Reply::Fail(m) => return Err(engine_error(&m)),
                Reply::Gated(g, inner) => {
                    let _p = g.acquire().await.unwrap();
                    r = *inner;
                }
                Reply::Never => futures::future::pending::<()>().await,
            }
        }
    }
}

/// An engine error with these words and no kind.
pub fn engine_error(message: &str) -> EngineError {
    EngineError::Other { message: message.to_string(), code: None }
}

#[async_trait(?Send)]
impl Machine for FakeMachine {
    fn id(&self) -> &str {
        &self.id
    }
    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }
    async fn preview_url(&self, _port: u16) -> Result<wsp_wire::wire::machine_link::PreviewReach, EngineError> {
        if !self.offers().preview_url {
            return Err(EngineError::Other { message: "not on this road".to_string(), code: None });
        }
        Ok(wsp_wire::wire::machine_link::PreviewReach { url: "http://127.0.0.1:1/".to_string(), token: "preview".to_string(), expires_at: 9_999_999_999_999.0.into() })
    }
    fn offers(&self) -> MachineOffers {
        *self.offers.lock().unwrap()
    }
    async fn exec(&self, cmd: &str, _opts: ExecOptions) -> Result<ExecResult, EngineError> {
        self.exec_log.lock().unwrap().push(cmd.to_string());
        let found = { self.rules.lock().unwrap().iter().find_map(|r| r(cmd)) };
        self.reply(found.unwrap_or_else(|| ok(""))).await
    }
    async fn run(&self, script: &str, _opts: RunOptions) -> Result<ExecResult, EngineError> {
        self.exec(script, ExecOptions::default()).await
    }
    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> Result<String, EngineError> {
        Ok(format!("snap_{}", self.id))
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> Result<(), EngineError> {
        *self.paused.lock().unwrap() = true;
        Ok(())
    }
    async fn resume(&self, _signal: Option<&wsp_engine::cancel::Cancel>) -> Result<(), EngineError> {
        *self.paused.lock().unwrap() = false;
        Ok(())
    }
    async fn kill(&self) -> Result<(), EngineError> {
        if let Some(m) = self.kill_fails.lock().unwrap().clone() {
            return Err(engine_error(&m));
        }
        *self.killed.lock().unwrap() = true;
        Ok(())
    }
    async fn state(&self) -> Result<MachineState, EngineError> {
        Ok(if *self.killed.lock().unwrap() { MachineState::Gone } else { MachineState::Running })
    }
    async fn download_url(&self, path: &str) -> Result<String, EngineError> {
        Ok(format!("https://files.test/{}", path))
    }
    async fn upload_url(&self, path: &str) -> Result<String, EngineError> {
        Ok(format!("https://files.test/{}", path))
    }
    async fn daemon_answers(&self, _timeout_ms: Option<u64>) -> Result<bool, EngineError> {
        let next = self.daemon_answers.lock().unwrap().pop_front();
        match next {
            Some(Ok(b)) => Ok(b),
            Some(Err(m)) => Err(engine_error(&m)),
            None => Ok(false),
        }
    }
    async fn start_daemon(&self) -> Result<ExecResult, EngineError> {
        *self.starts_asked.lock().unwrap() += 1;
        let next = self.start_daemon.lock().unwrap().pop_front();
        self.reply(next.unwrap_or_else(|| ok(""))).await
    }
    async fn daemon_exits(&self) -> Result<Option<String>, EngineError> {
        let next = self.daemon_exits.lock().unwrap().pop_front();
        match next {
            Some(Ok(s)) => Ok(s),
            Some(Err(m)) => Err(engine_error(&m)),
            None => Ok(None),
        }
    }
    async fn put_bytes(&self, path: &str, bytes: &[u8], _timeout_ms: Option<u64>) -> Result<Option<wsp_engine::machine::BytesLanded>, EngineError> {
        self.files.lock().unwrap().push((path.to_string(), bytes.to_vec()));
        Ok(None)
    }
    async fn read_bytes(&self, path: &str, _size: Option<u64>, _timeout_ms: Option<u64>) -> Result<Vec<u8>, EngineError> {
        let gated = self.readable.lock().unwrap().get(path).cloned();
        if let Some(r) = gated {
            self.reply(r).await?;
        }
        let bytes = self.read_bytes_bytes.lock().unwrap().get(path).cloned();
        bytes.ok_or_else(|| engine_error(&format!("nothing packed at {path}")))
    }
}

// ---------------------------------------------------------------------------------------------------------------
// A backend (stub-backend.ts plus sub-fixture.ts's fakeBoat).

pub struct FakePricing;

impl BackendPricing for FakePricing {
    fn rate_usd_per_hour(&self, size: Size) -> f64 {
        if size.cpu >= 8.0 {
            0.072
        } else if size.cpu >= 4.0 {
            0.036
        } else {
            0.018
        }
    }
    fn default_size(&self) -> Size {
        Size { cpu: 2.0, mem_mb: 4096.0 }
    }
    fn snapshot_storage(&self) -> SnapshotStoragePricing {
        SnapshotStoragePricing {
            wire: wsp_wire::wire::machine_link::SnapshotStoragePricing { free_gb: 0.0.into(), usd_per_gb_month: 0.0.into(), billed_from: "named".to_string() },
            free_named: None,
            usd_per_named_month: None,
        }
    }
}

pub struct FakeLifecycle {
    pub daemon_answers_ms: f64,
    pub disk_kept_ms: Mutex<Option<u64>>,
    pub backstops: Mutex<Vec<(String, i64, bool)>>,
}

#[async_trait(?Send)]
impl Lifecycle for FakeLifecycle {
    fn budgets(&self) -> LifecycleBudgets {
        LifecycleBudgets { wake_attempts: 1.into(), daemon_answers_ms: self.daemon_answers_ms.into(), resume_asks: None }
    }
    fn has_backstop(&self) -> bool {
        true
    }
    async fn backstop(&self, machine: &dyn Machine, until_ms: i64, exact: bool) -> Result<(), EngineError> {
        self.backstops.lock().unwrap().push((machine.id().to_string(), until_ms, exact));
        Ok(())
    }
    fn disk_kept_ms(&self) -> Option<u64> {
        *self.disk_kept_ms.lock().unwrap()
    }
}

pub struct FakeBackend {
    pub machines: Mutex<Vec<Arc<FakeMachine>>>,
    pub creates: Mutex<Vec<MachineSpec>>,
    pub capabilities: Mutex<Capabilities>,
    pub limits: Mutex<Option<BoxLimits>>,
    pub limits_reads: Mutex<u32>,
    pub lifecycle: Arc<FakeLifecycle>,
    pub root_disks: Mutex<IndexMap<String, RootDisk>>,
}

impl FakeBackend {
    pub fn new() -> Arc<FakeBackend> {
        Arc::new(FakeBackend {
            machines: Mutex::new(Vec::new()),
            creates: Mutex::new(Vec::new()),
            capabilities: Mutex::new(boat_capabilities()),
            limits: Mutex::new(None),
            limits_reads: Mutex::new(0),
            lifecycle: Arc::new(FakeLifecycle { daemon_answers_ms: 900_000.0, disk_kept_ms: Mutex::new(None), backstops: Mutex::new(Vec::new()) }),
            root_disks: Mutex::new(IndexMap::new()),
        })
    }
    /// A machine this backend holds, as `get` finds it.
    pub fn add(&self, m: Arc<FakeMachine>) {
        self.machines.lock().unwrap().push(m);
    }
}

/// Boat's capabilities as the stub plays it: a stop that snapshots the disk and the four classes.
pub fn boat_capabilities() -> Capabilities {
    let size = |cpu: f64, mem: f64, rate: f64| wsp_wire::wire::capabilities::MachineSizeOffer { cpu: cpu.into(), mem_mb: mem.into(), rate_usd_per_hour: rate.into(), name: None };
    Capabilities {
        live_clone_forks: false,
        pause_mode: Some(wsp_wire::wire::capabilities::PauseMode::Disk),
        replaces_machine: false,
        preview_urls: true,
        signed_urls: true,
        callback_relay: false,
        disk_snapshots: true,
        images: true,
        snapshots_any_life: false,
        snapshot_listing: true,
        templates: false,
        kept: false,
        copies: false,
        own_network: false,
        sizes_last: false,
        sizes: vec![size(2.0, 4096.0, 0.018), size(4.0, 8192.0, 0.036), size(8.0, 16384.0, 0.072), size(16.0, 32768.0, 0.2)],
    }
}

#[async_trait(?Send)]
impl MachineBackend for FakeBackend {
    fn capabilities(&self) -> Capabilities {
        self.capabilities.lock().unwrap().clone()
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
        Arc::new(FakePricing)
    }
    fn lifecycle(&self) -> Option<Arc<dyn Lifecycle>> {
        Some(self.lifecycle.clone())
    }
    fn offers(&self) -> BackendOffers {
        BackendOffers { limits: self.limits.lock().unwrap().is_some(), root_disk_of: !self.root_disks.lock().unwrap().is_empty(), ..BackendOffers::default() }
    }
    fn root_disk_of(&self, size: Size) -> Option<RootDisk> {
        self.root_disks.lock().unwrap().get(&format!("{}x{}", size.cpu, size.mem_mb)).cloned()
    }
    async fn create(&self, spec: MachineSpec, _waiting: Option<Waiting>, _sent: Option<String>) -> Result<Arc<dyn Machine>, EngineError> {
        self.creates.lock().unwrap().push(spec);
        let n = self.machines.lock().unwrap().len() + 1;
        let m = FakeMachine::new(&format!("bx_{n}"));
        self.add(m.clone());
        Ok(m)
    }
    async fn get(&self, id: &str) -> Result<Arc<dyn Machine>, EngineError> {
        let found = self.machines.lock().unwrap().iter().find(|m| m.id == id).cloned();
        found.map(|m| m as Arc<dyn Machine>).ok_or_else(|| engine_error(&format!("machine {id} is gone")))
    }
    async fn list(&self, _labels: Option<&IndexMap<String, String>>) -> Result<Vec<MachineListRow>, EngineError> {
        Ok(Vec::new())
    }
    async fn delete_snapshot(&self, _id: &str) -> Result<(), EngineError> {
        Ok(())
    }
    async fn limits(&self) -> Result<BoxLimits, EngineError> {
        *self.limits_reads.lock().unwrap() += 1;
        self.limits.lock().unwrap().clone().ok_or_else(|| engine_error("no limits"))
    }
}

/// sub-fixture.ts `PAID`: the owner's account at 17:06 UTC on 2026-10-11.
pub fn paid() -> BoxLimits {
    use wsp_engine::box_backend::{BoxClassType, StartCount, StartWindows};
    BoxLimits {
        trial: false,
        plan_key: Some("box_20".to_string()),
        plan_usd: Some(20.0),
        machines_at_once: Some(100.0),
        active_boxes: None,
        starts: Some(StartWindows { minute: StartCount { used: 0.0, limit: 12.0 }, hour: StartCount { used: 9.0, limit: 60.0 }, day: StartCount { used: 187.0, limit: 200.0 } }),
        sizes: vec![BoxClassType::Small, BoxClassType::Default, BoxClassType::Large],
        can_start: true,
        start_blocked_reason: None,
        credit_balance_hours: None,
        billing_status: None,
    }
}

/// A thread's caller, as its token stamps it (`{origin: "here", by: {kind: "thread", ...}}`).
pub fn caller(thread: &str, workspace: &str, root: &str) -> wsp_wire::views::workspace::Caller {
    use wsp_wire::views::workspace::{Caller, ThreadLit};
    use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin};
    Caller::Scoped { origin: WorkspaceOrigin::Here, by: ThreadScope { kind: ThreadLit, thread_id: thread.to_string(), workspace_id: workspace.to_string(), root_thread_id: root.to_string() } }
}

/// A caller in words, for keeping it on a call: `<thread>@<workspace>`, `road` for a bare road, `none`.
pub fn caller_words(c: Option<&wsp_wire::views::workspace::Caller>) -> String {
    use wsp_wire::views::workspace::Caller;
    match c {
        None => "none".to_string(),
        Some(Caller::Road(_)) => "road".to_string(),
        Some(Caller::Scoped { by, .. }) => format!("{}@{}", by.thread_id, by.workspace_id),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// A live entry on the real wsp-core and engine types, with nothing in flight.

/// The engine's `Workspace` has no constructor yet (`Workspace::new` is `todo!()`), so a test that builds an entry
/// stops there until the engine's lifecycle lands.
pub fn live_entry(record: Rc<RefCell<WorkspaceRecord>>, machine: Arc<dyn Machine>) -> Entry {
    let hooks = WorkspaceHooks {
        golden_snapshot: String::new(),
        wake_attempts: 1,
        resurrect: None,
        vault_export: None,
        vault_import: None,
        stash_vault: None,
        restore_vault: None,
        wake_check: None,
        move_hook: None,
        retire: None,
    };
    let ws = Rc::new(Workspace::new(machine.clone(), hooks, WorkspaceInitial::default()));
    live_entry_on(record, machine, ws)
}

/// An entry for a record on a machine, sharing the workspace another entry made (`attach` on a held workspace).
pub fn live_entry_on(record: Rc<RefCell<WorkspaceRecord>>, machine: Arc<dyn Machine>, ws: Rc<Workspace>) -> Entry {
    Rc::new(RefCell::new(LiveWorkspace {
        record,
        opens_thread: None,
        of_record_held: false,
        ws,
        machine,
        generation: 0,
        waking: None,
        unchecked: false,
        napping: None,
        adopting: None,
        deleting: None,
        late_read: None,
        wake_stop: None,
        resize_to: None,
        nap_wait_ms: None,
        wake_said: None,
        delete_said: None,
        wake_ask: None,
        creating: false,
        checkout: None,
        checkout_reading: None,
        tree: None,
        tree_reading: None,
        pr: None,
        pr_reading: None,
        pr_poll: None,
        pr_poll_ms: None,
        pr_none_at: None,
    }))
}

/// The id of the record an entry holds.
pub fn entry_id(e: &Entry) -> String {
    e.borrow().record.borrow().id.clone()
}

// ---------------------------------------------------------------------------------------------------------------
// The host: one fake behind every area's context trait. Each area's trait impl lives in the test file that drives
// it; the shared state and the helpers to read it back live here.

/// A call the host was asked, by name and its arguments as words.
pub type Call = (String, Vec<String>);

pub struct FakeHost {
    pub clock: Rc<FakeClock>,
    pub live: RefCell<IndexMap<String, Entry>>,
    /// Stored documents by collection, then key, as JSON (the TS store).
    pub store: RefCell<IndexMap<String, IndexMap<String, Value>>>,
    pub blobs: RefCell<IndexMap<(String, String), Vec<u8>>>,
    pub sessions: RefCell<IndexMap<String, SessionEntry>>,
    pub thread_records: RefCell<Vec<(String, String)>>,
    pub running: RefCell<IndexSet<String>>,
    pub launching: RefCell<IndexSet<String>>,
    pub events: RefCell<Vec<Value>>,
    pub recorded: RefCell<Vec<Value>>,
    pub warns: RefCell<Vec<String>>,
    pub calls: RefCell<Vec<Call>>,
    pub listeners: RefCell<Vec<(String, Rc<dyn Fn(&Value)>)>>,
    pub backend: Arc<FakeBackend>,
    pub settings_delete_after: RefCell<IndexMap<String, f64>>,
    /// What the next calls of a name answer, in order; a name with none answers its default.
    pub scripted: RefCell<IndexMap<String, VecDeque<Result<Value, AreaError>>>>,
    pub closing: Cell<bool>,
    /// The gate a scripted `"gate"` stand-up waits on.
    pub stand_up_gate: RefCell<Option<Arc<Semaphore>>>,
    /// `--machines` per place, as the place door keeps it.
    pub machines_at: RefCell<IndexMap<String, f64>>,
}

impl FakeHost {
    pub fn new() -> Rc<FakeHost> {
        Rc::new(FakeHost {
            clock: FakeClock::new(1_760_000_000_000.0),
            live: RefCell::new(IndexMap::new()),
            store: RefCell::new(IndexMap::new()),
            blobs: RefCell::new(IndexMap::new()),
            sessions: RefCell::new(IndexMap::new()),
            thread_records: RefCell::new(Vec::new()),
            running: RefCell::new(IndexSet::new()),
            launching: RefCell::new(IndexSet::new()),
            events: RefCell::new(Vec::new()),
            recorded: RefCell::new(Vec::new()),
            warns: RefCell::new(Vec::new()),
            calls: RefCell::new(Vec::new()),
            listeners: RefCell::new(Vec::new()),
            backend: FakeBackend::new(),
            settings_delete_after: RefCell::new(IndexMap::new()),
            scripted: RefCell::new(IndexMap::new()),
            closing: Cell::new(false),
            stand_up_gate: RefCell::new(None),
            machines_at: RefCell::new(IndexMap::new()),
        })
    }

    /// Keeps a call by name.
    pub fn call(&self, name: &str, args: &[&str]) {
        self.calls.borrow_mut().push((name.to_string(), args.iter().map(|a| a.to_string()).collect()));
    }
    /// The calls of a name, in order.
    pub fn calls_of(&self, name: &str) -> Vec<Vec<String>> {
        self.calls.borrow().iter().filter(|(n, _)| n == name).map(|(_, a)| a.clone()).collect()
    }
    /// The next scripted answer of a name, if any.
    pub fn next(&self, name: &str) -> Option<Result<Value, AreaError>> {
        self.scripted.borrow_mut().get_mut(name).and_then(|q| q.pop_front())
    }
    /// Scripts the next answer of a name.
    pub fn script(&self, name: &str, answer: Result<Value, AreaError>) {
        self.scripted.borrow_mut().entry(name.to_string()).or_default().push_back(answer);
    }

    /// A live entry for a record read off the TS JSON, on a machine of the fake backend.
    pub fn put_live(&self, record: Value, machine: Arc<FakeMachine>) -> Entry {
        let record: WorkspaceRecord = serde_json::from_value(record).expect("the record reads as the TS writes it");
        let id = record.id.clone();
        self.backend.add(machine.clone());
        let entry = live_entry(Rc::new(RefCell::new(record)), machine);
        self.live.borrow_mut().insert(id, entry.clone());
        entry
    }
    /// A new entry for the same record on another machine, as `attach` makes one.
    pub fn reattach(&self, entry: &Entry, machine: Arc<dyn Machine>) -> Entry {
        let (record, ws) = {
            let held = entry.borrow();
            (held.record.clone(), held.ws.clone())
        };
        let next = live_entry_on(record, machine, ws);
        self.live.borrow_mut().insert(entry_id(entry), next.clone());
        next
    }
    /// The record as last persisted, as JSON.
    pub fn stored(&self, id: &str) -> Option<Value> {
        self.store.borrow().get("workspaces").and_then(|c| c.get(id)).cloned()
    }
    pub fn persist_now(&self, record: &WorkspaceRecord) {
        let v = serde_json::to_value(record).unwrap();
        self.store.borrow_mut().entry("workspaces".to_string()).or_default().insert(record.id.clone(), v);
    }
    pub fn blob(&self, collection: &str, key: &str) -> Option<Vec<u8>> {
        self.blobs.borrow().get(&(collection.to_string(), key.to_string())).cloned()
    }
    /// Emits on the fake bus: listeners of the type, then `"*"`, inline.
    pub fn emit_now(&self, event: Value) {
        self.events.borrow_mut().push(event.clone());
        let kind = event["type"].as_str().unwrap_or("").to_string();
        let ls: Vec<_> = self.listeners.borrow().iter().filter(|(k, _)| *k == kind).map(|(_, f)| f.clone()).collect();
        for f in ls {
            f(&event);
        }
        let all: Vec<_> = self.listeners.borrow().iter().filter(|(k, _)| k == "*").map(|(_, f)| f.clone()).collect();
        for f in all {
            f(&event);
        }
    }
    pub fn events_of(&self, kind: &str) -> Vec<Value> {
        self.events.borrow().iter().filter(|e| e["type"] == kind).cloned().collect()
    }
    /// A session row for a thread on a workspace, as a turn leaves one.
    pub fn put_row(&self, turn_id: &str, workspace_id: &str, thread_id: &str, started_at: f64, claude_session_id: Option<&str>) {
        let view: SessionView = serde_json::from_value(json!({
            "id": turn_id, "workspaceId": workspace_id, "harness": "claude", "status": "completed", "threadId": thread_id,
            "startedAt": started_at, "claudeSessionId": claude_session_id,
        }))
        .unwrap();
        self.sessions.borrow_mut().insert(turn_id.to_string(), queue_ctx::session_entry(view, turn_id.to_string()));
    }
    pub fn spawn_local(&self, f: LocalBoxFuture<'static, ()>) {
        tokio::task::spawn_local(f);
    }
}

/// A sub-thread's record as sub-spawn writes it and a done leaves it, on Boat (sub-fixture.ts), as JSON.
pub fn sub_record(id: &str, phase: &str) -> Value {
    json!({
        "id": id, "name": "Fix the flaky upload test", "kind": "cloud", "machineId": "bx_1", "phase": phase, "golden": "wsp-host-default-v1",
        "createdAt": "2026-10-12T06:00:00.000Z", "project": "pr_1", "spec": {}, "place": "box", "size": { "cpu": 2, "memMb": 4096 },
        "firstLife": true, "parentThreadId": "t_lead", "rootThreadId": "t_lead", "lead": { "threadId": "t_lead", "workspaceId": "ws_lead" },
        "sub": {
            "on": "boat", "repo": "https://github.com/acme/web.git", "base": { "branch": "feat/upload", "sha": BASE_SHA },
            "branch": BRANCH, "folder": "/root/web", "rateUsdPerHour": 0.018, "sizeName": "small"
        }
    })
}

pub const BRANCH: &str = "wsp/fix-the-flaky-upload-test";
pub const BASE_SHA: &str = "4be1c0a9d2c4e1f07a3b9e5d6c8f2a1b0e9d7c6a";
pub const PUSHED_SHA: &str = "9c2e1d4b7a0f3e8c5d6a1b2c3d4e5f6a7b8c9d0e";
/// sub-fixture.ts `PUSHED`: the branch pushed, three commits on the lead's.
pub const PUSHED: &str = "branch wsp/fix-the-flaky-upload-test\npushed 9c2e1d4b7a0f3e8c5d6a1b2c3d4e5f6a7b8c9d0e\nunpushed 0\nchanged 0\ncommits 3\n";
/// sub-fixture.ts `UNPUSHED`: two commits nobody pushed and three files changed.
pub const UNPUSHED: &str = "branch wsp/fix-the-flaky-upload-test\npushed none\nunpushed 2\nchanged 3\n";
pub const MIN: f64 = 60_000.0;

/// A machine answering the result read with `read` and the session's pack as `session` says (sub-fixture.ts
/// `answering`): bytes kept at `/tmp/wsp-session-<thread>.gz`, a size only, or no file.
pub fn answering(m: &FakeMachine, read: Reply, session: Option<Vec<u8>>, thread: &str) {
    m.answer("git rev-list --count --branches --not --remotes", read);
    match session {
        None => m.answer("gzip -c", ok("nofile\n")),
        Some(bytes) => {
            let at = format!("/tmp/wsp-session-{thread}.gz");
            m.answer("gzip -c", ok(&format!("path /root/.claude-cfg/projects/-root-web/s2.jsonl\nsize {}\n", bytes.len())));
            m.read_bytes_bytes.lock().unwrap().insert(at, bytes);
        }
    }
}

/// The error a test scripts as an engine road's own.
pub fn failed(message: &str) -> AreaError {
    AreaError::Engine(engine_error(message))
}

/// The JSON text of a value, for key-order comparisons (rule 4.26).
pub fn text<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap()
}

/// A boxed local future of a value.
pub fn ready<T: 'static>(v: T) -> LocalBoxFuture<'static, T> {
    async move { v }.boxed_local()
}

pub mod life_ctx;
pub mod queue_ctx;
pub mod spawn_ctx;
pub mod reach_ctx;
pub mod kinds_ctx;
