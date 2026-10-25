// SPDX-License-Identifier: AGPL-3.0-only
//! The state and the scripting of `FakeHost`. See mod.rs.
#![allow(dead_code, unused_imports)]
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::sync::Arc;

pub use async_trait::async_trait;
pub use futures::future::{BoxFuture, FutureExt, LocalBoxFuture};
pub use indexmap::{IndexMap, IndexSet};
pub use serde_json::{json, Map, Value};
pub use wsp_adapters::port::*;
pub use wsp_core::clock::{Clock, FakeClock};
pub use wsp_core::ids::Ids;
pub use wsp_core::types::api::*;
pub use wsp_core::types::internal::{EndThread, KeptLaunch, Reopened, SearchedHit, Taken, ThreadRecord, ThreadStamps, ThreadWords, TreeTalk, TurnAsked, TurnLive, TurnWritten};
pub use wsp_core::types::wiring::{SessionHandle, WorkspaceRecord};
pub use wsp_threads::activity::*;
pub use wsp_threads::pending::*;
pub use wsp_threads::threads::launch::*;
pub use wsp_threads::threads::sessions::*;
pub use wsp_threads::threads::trimmed_row::*;
pub use wsp_threads::threads::turns::*;
pub use wsp_threads::threads::waits::*;
pub use wsp_wire::types::harness_picks::RanPicks;
pub use wsp_wire::types::thread_defaults::AccessChoice;
pub use wsp_wire::views::activity::{ActivityEnded, ActivityView, ProcessesAsk};
pub use wsp_wire::views::harness::{HarnessCatalog, McpServerSpec, StartPicks};
pub use wsp_wire::views::place_more::PlaceView;
pub use wsp_wire::views::preferences::{HistoryPage, Preferences, ThreadFacts, ThreadHead};
pub use wsp_wire::views::session::{SessionView, ThreadCapWait};
pub use wsp_wire::views::session_events::{TurnResult, TurnStatus};
pub use wsp_wire::views::session_events_more::SessionEvent;
pub use wsp_wire::views::workspace::Caller;
pub use wsp_wire::views::workspace_kind::WorkspaceKind;
pub use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin};
pub use wsp_wire::wire::daemon::ProcScope;
pub use wsp_words::exit::{HostError, Kind};

/// The fake clock's start, ms epoch.
pub const T0: i64 = 1_760_000_000_000;

/// A joined future already settled.
pub fn done() -> Joined {
    futures::future::ready(()).boxed_local().shared()
}

/// The scripted answers and the call log.
#[derive(Default)]
pub struct Hooks {
    map: RefCell<HashMap<String, Rc<dyn Any>>>,
    calls: RefCell<Vec<String>>,
}

impl Hooks {
    /// Answers `key` (`"<Trait>::<method>"`) with `f`, whose type is the method's own as a `dyn Fn` (an async
    /// method's answers a `LocalBoxFuture<'static, _>`).
    pub fn set<F: ?Sized + 'static>(&self, key: &str, f: Rc<F>) {
        self.map.borrow_mut().insert(key.to_string(), Rc::new(f));
    }
    pub fn get<F: ?Sized + 'static>(&self, key: &str) -> Option<Rc<F>> {
        let any = self.map.borrow().get(key).cloned()?;
        match any.downcast_ref::<Rc<F>>() {
            Some(f) => Some(f.clone()),
            None => panic!("the hook {key} was set with a type other than the method's"),
        }
    }
    pub fn log(&self, name: &str, args: Vec<String>) {
        let line = if args.is_empty() { name.to_string() } else { format!("{name}({})", args.join(", ")) };
        self.calls.borrow_mut().push(line);
    }
    /// Every call, in order.
    pub fn calls(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
    /// The calls of one method, in order.
    pub fn called(&self, name: &str) -> Vec<String> {
        self.calls.borrow().iter().filter(|c| *c == name || c.starts_with(&format!("{name}("))).cloned().collect()
    }
}

/// Ids drawn in order: the scripted ones first, then `uuid-<n>` and bytes of `n`.
#[derive(Default)]
pub struct ScriptedIds {
    pub uuids: RefCell<Vec<String>>,
    pub drawn: Cell<u32>,
}

impl Ids for ScriptedIds {
    fn uuid(&self) -> String {
        let mut q = self.uuids.borrow_mut();
        if !q.is_empty() {
            return q.remove(0);
        }
        self.drawn.set(self.drawn.get() + 1);
        format!("uuid-{}", self.drawn.get())
    }
    fn bytes(&self, n: usize) -> Vec<u8> {
        self.drawn.set(self.drawn.get() + 1);
        vec![self.drawn.get() as u8; n]
    }
    fn random(&self) -> f64 {
        0.5
    }
}

/// A harness exec that answers every command with nothing.
pub struct NoExec;

impl HarnessExec for NoExec {
    fn exec(&self, _command: &str, _env: Option<&Env>) -> BoxFuture<'static, Result<String, ExecFailure>> {
        futures::future::ready(Ok(String::new())).boxed()
    }
}

/// A live workspace whose record a test reads and writes.
pub struct FakeEntry {
    pub record: RefCell<WorkspaceRecord>,
    pub unchecked: Cell<bool>,
    pub exec: RefCell<Arc<dyn HarnessExec>>,
}

impl EntryHandle for FakeEntry {
    fn id(&self) -> String {
        self.record.borrow().id.clone()
    }
    fn record(&self) -> WorkspaceRecord {
        self.record.borrow().clone()
    }
    fn set_session(&self, session_id: &str) {
        self.record.borrow_mut().claude_session_id = Some(session_id.to_string());
    }
    fn unchecked(&self) -> bool {
        self.unchecked.get()
    }
    fn set_checked(&self) {
        self.unchecked.set(false);
    }
    fn harness_exec(&self, _ms: u64) -> Arc<dyn HarnessExec> {
        self.exec.borrow().clone()
    }
}

/// A workspace record as the store holds one, with `extra` keys laid over the required ones.
pub fn record_doc(id: &str, extra: Value) -> Value {
    let mut doc = json!({
        "id": id, "name": id, "machineId": format!("m-{id}"), "phase": "running", "kind": "local", "golden": "g1",
        "createdAt": "2026-10-01T00:00:00.000Z", "project": "p1", "spec": {}, "size": {"cpu": 2, "memMb": 4096}, "firstLife": true,
    });
    for (k, v) in extra.as_object().expect("extra is an object") {
        doc[k] = v.clone();
    }
    doc
}

pub fn record(id: &str, extra: Value) -> WorkspaceRecord {
    serde_json::from_value(record_doc(id, extra)).expect("a workspace record the fixture builds")
}

pub fn entry(id: &str, extra: Value) -> Rc<FakeEntry> {
    Rc::new(FakeEntry { record: RefCell::new(record(id, extra)), unchecked: Cell::new(false), exec: RefCell::new(Arc::new(NoExec)) })
}

/// A row's view: `id`, `workspaceId`, `harness` and `status` are required; `extra` lays any other key over them.
pub fn view(id: &str, workspace_id: &str, extra: Value) -> SessionView {
    let mut doc = json!({ "id": id, "workspaceId": workspace_id, "harness": "claude", "status": "running" });
    for (k, v) in extra.as_object().expect("extra is an object") {
        doc[k] = v.clone();
    }
    serde_json::from_value(doc).expect("a session view the fixture builds")
}

pub fn shared(v: SessionView) -> SharedView {
    Rc::new(RefCell::new(v))
}

/// A row with only its view and turn id (the fields `SessionEntry::new` would set), built here so the fixture does
/// not wait on that body.
pub fn row(v: SessionView, turn_id: &str) -> SessionEntry {
    SessionEntry {
        view: shared(v),
        turn_id: turn_id.to_string(),
        replaces: None,
        notify: None,
        notify_by: None,
        notify_road: None,
        turn_token: None,
        scope_device_id: None,
        handle: None,
        end: None,
        turn_live: None,
        run: None,
        from: None,
        asked: None,
        snapshot: None,
        pid: None,
        launch: None,
        calls: None,
        unanswered: false,
        queued: false,
    }
}

/// An event as the transcript holds one, from its JSON.
pub fn event(doc: Value) -> SessionEvent {
    serde_json::from_value(doc).expect("a session event the fixture builds")
}

/// A thread record from its JSON (`harness` required).
pub fn thread_record(doc: Value) -> ThreadRecord {
    serde_json::from_value(doc).expect("a thread record the fixture builds")
}

/// A refusal's message, fix and kind, for whole comparison.
pub fn said(e: &HostError) -> (String, Option<String>, Option<Kind>) {
    (e.message.clone(), e.fix.clone(), e.kind.clone())
}

pub struct FakeHost {
    pub hooks: Hooks,
    pub clock: FakeClock,
    pub ids: Rc<ScriptedIds>,
    pub rows: Rows,
    pub records: Records,
    pub entries: RefCell<IndexMap<String, Entry>>,
    pub recorded: RefCell<Vec<SessionEvent>>,
    pub emitted: RefCell<Vec<Value>>,
    pub passed: RefCell<Vec<Value>>,
    pub closing: Cell<bool>,
    pub held_at_close: HeldAtClose,
    pub checkpoints_landing: Rc<RefCell<IndexMap<String, Joined>>>,
    pub lead_asks: Rc<RefCell<IndexMap<String, PermissionAsk>>>,
    pub turn_groups: Rc<RefCell<IndexMap<String, IndexMap<u32, String>>>>,
    me: RefCell<Weak<FakeHost>>,
}

impl FakeHost {
    pub fn new() -> Rc<Self> {
        let host = Rc::new(FakeHost {
            hooks: Hooks::default(),
            clock: FakeClock::new(T0),
            ids: Rc::new(ScriptedIds::default()),
            rows: Rc::new(RefCell::new(IndexMap::new())),
            records: Rc::new(RefCell::new(IndexMap::new())),
            entries: RefCell::new(IndexMap::new()),
            recorded: RefCell::new(Vec::new()),
            emitted: RefCell::new(Vec::new()),
            passed: RefCell::new(Vec::new()),
            closing: Cell::new(false),
            held_at_close: Rc::new(RefCell::new(Vec::new())),
            checkpoints_landing: Rc::new(RefCell::new(IndexMap::new())),
            lead_asks: Rc::new(RefCell::new(IndexMap::new())),
            turn_groups: Rc::new(RefCell::new(IndexMap::new())),
            me: RefCell::new(Weak::new()),
        });
        *host.me.borrow_mut() = Rc::downgrade(&host);
        host
    }

    pub fn me(&self) -> Rc<FakeHost> {
        self.me.borrow().upgrade().expect("the host is alive")
    }

    /// Holds a live workspace, answering `live_entry` and the entry reads for it.
    pub fn add_entry(&self, e: Rc<FakeEntry>) -> Entry {
        let id = e.record.borrow().id.clone();
        let entry: Entry = e;
        self.entries.borrow_mut().insert(id, entry.clone());
        entry
    }

    pub fn add_row(&self, key: &str, r: SessionEntry) {
        self.rows.borrow_mut().insert(key.to_string(), r);
    }

    pub fn add_record(&self, thread_id: &str, workspace_id: &str, record: ThreadRecord) {
        self.records.borrow_mut().insert(thread_id.to_string(), HeldThread { workspace_id: workspace_id.to_string(), record });
    }

    /// The recorded events as JSON, in order.
    pub fn recorded_json(&self) -> Vec<Value> {
        self.recorded.borrow().iter().map(|e| serde_json::to_value(e).expect("an event serializes")).collect()
    }

    /// The recorded events of one type, as JSON.
    pub fn recorded_of(&self, kind: &str) -> Vec<Value> {
        self.recorded_json().into_iter().filter(|e| e["type"] == kind).collect()
    }

    pub fn live(&self, workspace_id: &str) -> Option<Entry> {
        self.entries.borrow().get(workspace_id).cloned()
    }

    /// TS `latestOn`: the thread's row started latest (a missing start reads 0, the later row wins a tie).
    pub fn latest_row(&self, thread_id: &str) -> Option<SessionView> {
        let rows = self.rows.borrow();
        let mut latest: Option<SessionView> = None;
        for r in rows.values() {
            let v = r.view.borrow();
            if v.thread_id.as_deref() != Some(thread_id) {
                continue;
            }
            let at = v.started_at.map(|n| n.0).unwrap_or(0.0);
            if latest.as_ref().map(|l| at >= l.started_at.map(|n| n.0).unwrap_or(0.0)).unwrap_or(true) {
                latest = Some(v.clone());
            }
        }
        latest
    }

    /// The transcript events of one thread recorded so far.
    pub fn thread_events(&self, thread_id: &str) -> Vec<SessionEvent> {
        self.recorded.borrow().iter().filter(|e| serde_json::to_value(e).map(|v| v["threadId"] == thread_id).unwrap_or(false)).cloned().collect()
    }
}

/// Runs `f` on a current-thread runtime inside a `LocalSet`, as the host runs.
pub fn run_local<F: std::future::Future<Output = ()>>(f: F) {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a runtime");
    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, f);
}

/// Lets detached work run until nothing is ready (never a sleep).
pub async fn settle() {
    for _ in 0..64 {
        tokio::task::yield_now().await;
    }
}
