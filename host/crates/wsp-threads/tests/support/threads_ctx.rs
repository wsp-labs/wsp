// SPDX-License-Identifier: AGPL-3.0-only
//! The threads area's fake context: the runtime's rows and records in `StateCell`s, a test clock, bus and store,
//! and a log of every call out, so a test asserts what the area did and in what order.
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;

use futures::future::{FutureExt, LocalBoxFuture};
use indexmap::IndexMap;
use serde_json::Value;
use wsp_core::clock::Clock;
use wsp_core::context::{Joined, StateCell};
use wsp_core::ids::Ids;
use wsp_core::types::api::StartOptions;
use wsp_core::types::events::EventBus;
use wsp_core::types::internal::Taken;
use wsp_core::types::wiring::{SessionHandle, WorkspaceRecord};
use wsp_store::store::Store;
use wsp_threads::threads::threads::{AskFailure, Entry, HeldProject, PlaceMove, ReadMark, Sessions, ThreadRecords, ThreadsArea, ThreadsCtx};
use wsp_wire::types::attachments::Attachment;
use wsp_wire::types::usage::AccountLimit;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::session_events_more::SessionEvent;
use wsp_wire::views::workspace::Caller;
use wsp_words::exit::HostError;

use super::{err, CountingIds, FakeStore, TestBus, TestClock};

/// What a `start` answers, in order of calls; with none queued a start fails `no start answer queued`.
pub type StartAnswer = Result<Rc<dyn SessionHandle>, HostError>;

pub struct FakeThreadsCtx {
    pub clock: Rc<TestClock>,
    pub ids: Rc<CountingIds>,
    pub bus: Rc<TestBus>,
    pub store: Rc<FakeStore>,
    pub sessions: StateCell<Sessions>,
    pub thread_records: StateCell<ThreadRecords>,
    pub closing: Cell<bool>,
    pub revoke_fails: Cell<bool>,
    pub entries: RefCell<IndexMap<String, Entry>>,
    pub taken: RefCell<IndexMap<(String, String), Taken>>,
    pub place_door: Cell<bool>,
    pub linked: RefCell<Vec<String>>,
    pub place_listeners: Rc<RefCell<Vec<Rc<dyn Fn(&PlaceMove)>>>>,
    pub mem_mb: RefCell<IndexMap<String, f64>>,
    pub threads_at: RefCell<IndexMap<String, i64>>,
    pub place_ids: RefCell<IndexMap<String, String>>,
    pub roots: RefCell<IndexMap<String, String>>,
    pub interrupts: RefCell<IndexMap<String, Result<String, HostError>>>,
    pub starts: RefCell<VecDeque<StartAnswer>>,
    pub started: RefCell<Vec<(String, StartOptions, Option<Caller>)>>,
    pub sub_lines: RefCell<IndexMap<String, Option<String>>>,
    pub boot_work: RefCell<IndexMap<String, Joined>>,
    pub kept_writes: RefCell<IndexMap<String, Joined>>,
    pub thread_events: RefCell<IndexMap<(String, String), Vec<SessionEvent>>>,
    pub daemon: RefCell<Vec<(String, Result<Value, AskFailure>)>>,
    /// Ops the daemon never answers.
    pub daemon_hangs: RefCell<Vec<String>>,
    pub projects: RefCell<IndexMap<String, HeldProject>>,
    pub reach: RefCell<String>,
    pub snapshot_ms: Cell<Option<i64>>,
    pub vault: RefCell<IndexMap<String, String>>,
    pub limits: RefCell<Option<Result<Vec<AccountLimit>, HostError>>>,
    /// Every event `record` was handed, in order.
    pub recorded: RefCell<Vec<SessionEvent>>,
    /// Every call out, as `name arg`, in order.
    pub calls: RefCell<Vec<String>>,
    /// Daemon frames asked, in order.
    pub frames: RefCell<Vec<Value>>,
}

impl FakeThreadsCtx {
    pub fn new() -> Rc<FakeThreadsCtx> {
        Rc::new(FakeThreadsCtx {
            clock: TestClock::new(1_000_000),
            ids: Rc::new(CountingIds::default()),
            bus: TestBus::new(),
            store: FakeStore::new(),
            sessions: StateCell::new(Sessions::new()),
            thread_records: StateCell::new(ThreadRecords::new()),
            closing: Cell::new(false),
            revoke_fails: Cell::new(false),
            entries: RefCell::default(),
            taken: RefCell::default(),
            place_door: Cell::new(true),
            linked: RefCell::default(),
            place_listeners: Rc::default(),
            mem_mb: RefCell::default(),
            threads_at: RefCell::default(),
            place_ids: RefCell::default(),
            roots: RefCell::default(),
            interrupts: RefCell::default(),
            starts: RefCell::default(),
            started: RefCell::default(),
            sub_lines: RefCell::default(),
            boot_work: RefCell::default(),
            kept_writes: RefCell::default(),
            thread_events: RefCell::default(),
            daemon: RefCell::default(),
            daemon_hangs: RefCell::default(),
            projects: RefCell::default(),
            reach: RefCell::new("reachable".into()),
            snapshot_ms: Cell::new(None),
            vault: RefCell::default(),
            limits: RefCell::new(None),
            recorded: RefCell::default(),
            calls: RefCell::default(),
            frames: RefCell::default(),
        })
    }
    /// The area over this context.
    pub fn area(self: &Rc<Self>) -> Rc<ThreadsArea> {
        ThreadsArea::new(self.clone())
    }
    pub fn call(&self, line: String) {
        self.calls.borrow_mut().push(line);
    }
    pub fn called(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
    /// The recorded events as JSON.
    pub fn recorded_json(&self) -> Vec<Value> {
        self.recorded.borrow().iter().map(super::json_of).collect()
    }
    /// A place door event, to every listener.
    pub fn place_move(&self, m: PlaceMove) {
        let listeners = self.place_listeners.borrow().clone();
        for l in listeners {
            l(&m);
        }
    }
    /// The daemon answers a frame whose `op` is `op` with this.
    pub fn daemon_answers(&self, op: &str, answer: Result<Value, AskFailure>) {
        self.daemon.borrow_mut().push((op.to_string(), answer));
    }
    pub fn add_entry(&self, entry: Entry) {
        self.entries.borrow_mut().insert(entry.record.id.clone(), entry);
    }
}

impl ThreadsCtx for FakeThreadsCtx {
    fn clock(&self) -> Rc<dyn Clock> {
        self.clock.clone()
    }
    fn ids(&self) -> Rc<dyn Ids> {
        self.ids.clone()
    }
    fn bus(&self) -> Rc<dyn EventBus> {
        Rc::new(super::BusHandle(self.bus.clone()))
    }
    fn store(&self) -> Rc<dyn Store> {
        self.store.clone()
    }
    fn sessions(&self) -> StateCell<Sessions> {
        self.sessions.clone()
    }
    fn thread_records(&self) -> StateCell<ThreadRecords> {
        self.thread_records.clone()
    }
    fn closing(&self) -> bool {
        self.closing.get()
    }
    fn entry(&self, workspace_id: &str) -> Option<Entry> {
        self.entries.borrow().get(workspace_id).cloned()
    }
    fn live_records(&self) -> Vec<WorkspaceRecord> {
        self.entries.borrow().values().map(|e| e.record.clone()).collect()
    }
    fn taken(&self, workspace_id: &str, request_id: &str) -> Option<Taken> {
        self.taken.borrow().get(&(workspace_id.to_string(), request_id.to_string())).cloned()
    }
    fn has_place_door(&self) -> bool {
        self.place_door.get()
    }
    fn place_linked(&self, place: &str) -> bool {
        self.linked.borrow().iter().any(|p| p == place)
    }
    fn on_place_move(&self, listener: Rc<dyn Fn(&PlaceMove)>) -> Option<Box<dyn Fn()>> {
        if !self.place_door.get() {
            return None;
        }
        self.call("door.on".into());
        self.place_listeners.borrow_mut().push(listener.clone());
        let list = self.place_listeners.clone();
        Some(Box::new(move || list.borrow_mut().retain(|l| !Rc::ptr_eq(l, &listener))))
    }
    fn place_mem_mb(&self, place: &str) -> Option<f64> {
        self.mem_mb.borrow().get(place).copied()
    }
    fn place_threads_at(&self, place: &str) -> Option<i64> {
        self.threads_at.borrow().get(place).copied()
    }
    fn place_name(&self, place: &str) -> String {
        format!("{place}-name")
    }
    fn place_id_of(&self, record: &WorkspaceRecord) -> Option<String> {
        self.place_ids.borrow().get(&record.id).cloned()
    }
    fn provider_of(&self, _record: &WorkspaceRecord) -> Option<String> {
        None
    }
    fn revoke_device(&self, device_id: &str) -> LocalBoxFuture<'static, Result<(), HostError>> {
        self.call(format!("revoke {device_id}"));
        let fails = self.revoke_fails.get();
        async move { if fails { Err(err("the device door is shut")) } else { Ok(()) } }.boxed_local()
    }
    fn proc_scopes_moved(&self) {
        self.call("procScopesMoved".into());
    }
    fn root_of(&self, thread_id: &str) -> Option<String> {
        self.roots.borrow().get(thread_id).cloned()
    }
    fn interrupt(&self, row_id: &str, _caller: Option<Caller>) -> LocalBoxFuture<'static, Result<String, HostError>> {
        self.call(format!("interrupt {row_id}"));
        let answer = self.interrupts.borrow().get(row_id).cloned().unwrap_or_else(|| Ok("accepted".into()));
        async move { answer }.boxed_local()
    }
    fn start(&self, workspace_id: &str, options: StartOptions, caller: Option<Caller>) -> LocalBoxFuture<'static, Result<Rc<dyn SessionHandle>, HostError>> {
        self.call(format!("start {workspace_id}"));
        self.started.borrow_mut().push((workspace_id.to_string(), options, caller));
        let answer = self.starts.borrow_mut().pop_front().unwrap_or_else(|| Err(err("no start answer queued")));
        async move { answer }.boxed_local()
    }
    fn stop_try(&self, row_id: &str) -> LocalBoxFuture<'static, Result<(), HostError>> {
        self.call(format!("stopTry {row_id}"));
        async { Ok(()) }.boxed_local()
    }
    fn mark(&self, threads: Vec<String>, mark: ReadMark, _caller: Option<Caller>) -> LocalBoxFuture<'static, Result<(), HostError>> {
        self.call(format!("mark {} {}", threads.join(","), mark.read_at));
        async { Ok(()) }.boxed_local()
    }
    fn sub_line(&self, workspace_id: &str, since: i64) -> LocalBoxFuture<'static, Option<String>> {
        self.call(format!("subLine {workspace_id} {since}"));
        let tail = self.sub_lines.borrow().get(workspace_id).cloned().flatten();
        async move { tail }.boxed_local()
    }
    fn boot_work(&self, workspace_id: &str) -> Option<Joined> {
        self.boot_work.borrow().get(workspace_id).cloned()
    }
    fn kept_writes(&self, workspace_id: &str) -> Option<Joined> {
        self.kept_writes.borrow().get(workspace_id).cloned()
    }
    fn keep_sent_images(&self, workspace_id: &str, thread_id: &str, request_id: Option<String>, attachments: Vec<Attachment>) -> LocalBoxFuture<'static, ()> {
        self.call(format!("keepSentImages {workspace_id} {thread_id} {} {}", request_id.unwrap_or_default(), attachments.len()));
        async {}.boxed_local()
    }
    fn record(&self, event: SessionEvent) {
        self.recorded.borrow_mut().push(event);
    }
    fn persist_sessions(&self, workspace_id: &str) -> LocalBoxFuture<'static, ()> {
        self.call(format!("persist {workspace_id}"));
        async {}.boxed_local()
    }
    fn open_thread(&self, workspace_id: &str, thread_id: &str) -> LocalBoxFuture<'static, Result<Vec<SessionEvent>, HostError>> {
        let events = self.thread_events.borrow().get(&(workspace_id.to_string(), thread_id.to_string())).cloned().unwrap_or_default();
        async move { Ok(events) }.boxed_local()
    }
    fn idle_hold(&self, workspace_id: &str) {
        self.call(format!("idle.hold {workspace_id}"));
    }
    fn idle_release(&self, workspace_id: &str) {
        self.call(format!("idle.release {workspace_id}"));
    }
    fn port_roots_moved(&self, workspace_id: &str) {
        self.call(format!("portRootsMoved {workspace_id}"));
    }
    fn end_snooze_for(&self, view: &SessionView) {
        self.call(format!("endSnoozeFor {}", view.id));
    }
    fn with_daemon(&self, _entry: &Entry, frame: Value) -> LocalBoxFuture<'static, Result<Value, AskFailure>> {
        let op = frame["op"].as_str().unwrap_or("").to_string();
        self.frames.borrow_mut().push(frame);
        if self.daemon_hangs.borrow().contains(&op) {
            return futures::future::pending().boxed_local();
        }
        let answer = self.daemon.borrow().iter().find(|(o, _)| *o == op).map(|(_, a)| a.clone());
        let answer = answer.unwrap_or_else(|| Err(AskFailure { error: err(&format!("no answer for {op}")), code: None }));
        async move { answer }.boxed_local()
    }
    fn checkout_of(&self, record: &WorkspaceRecord) -> String {
        format!("/work/{}", record.id)
    }
    fn project_held(&self, project: &str) -> HeldProject {
        self.projects.borrow().get(project).cloned().unwrap_or(HeldProject { id: project.to_string(), git_top: Some(format!("/repo/{project}")), ..Default::default() })
    }
    fn reach_of(&self, _entry: &Entry) -> String {
        self.reach.borrow().clone()
    }
    fn turn_snapshot_ms(&self) -> Option<i64> {
        self.snapshot_ms.get()
    }
    fn vault(&self) -> IndexMap<String, String> {
        self.vault.borrow().clone()
    }
    fn vault_on(&self, _entry: &Entry) -> IndexMap<String, String> {
        self.vault.borrow().clone()
    }
    fn login_stands(&self, _entry: &Entry, _harness: &str) -> bool {
        false
    }
    fn computer_of(&self, entry: &Entry) -> String {
        format!("{}-computer", entry.record.id)
    }
    fn ledger_limits(&self) -> LocalBoxFuture<'static, Result<Vec<AccountLimit>, HostError>> {
        let answer = self.limits.borrow().clone().unwrap_or_else(|| Err(err("no ledger")));
        async move { answer }.boxed_local()
    }
}

/// A live entry over a fake machine, its record read from JSON (`id`, `name`, `kind`, `project` default).
pub fn entry(fields: Value) -> (Entry, Arc<super::FakeMachine>) {
    let mut v = serde_json::json!({
        "id": "ws1", "name": "ws1", "machineId": "m-ws1", "kind": "local", "golden": "snap_g", "createdAt": "2026-10-01T12:00:00.000Z",
        "project": "p1", "phase": "running", "spec": {}, "size": { "cpu": 2, "memMb": 4096 }, "firstLife": false,
    });
    for (k, x) in fields.as_object().expect("an object").iter() {
        v[k] = x.clone();
    }
    if fields.get("machineId").is_none() {
        v["machineId"] = serde_json::json!(format!("m-{}", v["id"].as_str().unwrap_or("ws1")));
    }
    let machine = super::FakeMachine::new(v["machineId"].as_str().expect("a machine id"));
    let record: WorkspaceRecord = serde_json::from_value(v).expect("a WorkspaceRecord");
    (Entry { record, machine: machine.clone() }, machine)
}
