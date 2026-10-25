// SPDX-License-Identifier: AGPL-3.0-only
//! Fakes every slice-b test file shares: a store, a clock, a bus, a machine, transcript rows, a kept agent, and
//! builders for rows and records. Each area's fake context is in its own file beside this one.
#![allow(dead_code)]

pub mod agents_ctx;
pub mod conversations_ctx;
pub mod forks_ctx;
pub mod threads_ctx;
pub mod transcripts_ctx;

use std::cell::{Cell, RefCell};
use std::ops::ControlFlow;
use std::rc::Rc;

use futures::future::{BoxFuture, FutureExt, LocalBoxFuture, Shared};
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_core::clock::{Cancel, Clock, Timer};
use wsp_core::ids::Ids;
use wsp_core::types::events::{EventBus, EventListener, Since, Unlisten};
use wsp_core::types::internal::{SessionEntry, ThreadRecord};
use wsp_engine::errors::EngineError;
use wsp_engine::machine::{ExecOptions, Machine, MachineKind, MachineLife, MachineState, RunOptions, SnapshotOptions};
use wsp_store::sqlite_transcripts::{EventSize, Summed, Total, TranscriptRows};
use wsp_store::store::{BlobMark, Store};
use wsp_store::transcript_reader::event_bytes;
use wsp_threads::threads::threads::{HeldThread, Sessions, ThreadRecords};
use wsp_wire::views::session::SessionView;
use wsp_wire::views::session_events_more::SessionEvent;
use wsp_wire::wire::events_union::EventUnion;
use wsp_wire::wire::machine_link::ExecResult;
use wsp_words::exit::HostError;

/// A plain error, as `new Error(message)` builds one.
pub fn err(message: &str) -> HostError {
    HostError { message: message.to_string(), fix: None, kind: None, errors: None, warnings: None, left: None, marks: vec![] }
}

/// An error carrying one of the marks `computer_silent` reads.
pub fn silent(message: &str, mark: &str) -> HostError {
    HostError { marks: vec![mark.to_string()], ..err(message) }
}

/// A row's view from JSON; `id`, `workspaceId`, `harness` and `status` default to `r1`, `ws1`, `claude`, `running`.
pub fn view(fields: Value) -> SessionView {
    let mut v = json!({ "id": "r1", "workspaceId": "ws1", "harness": "claude", "status": "running" });
    for (k, x) in fields.as_object().expect("an object").iter() {
        v[k] = x.clone();
    }
    serde_json::from_value(v).expect("a SessionView")
}

/// A runtime row with nothing but its view and turn.
pub fn row(view: SessionView, turn_id: &str) -> SessionEntry {
    SessionEntry {
        view,
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

/// A thread record from JSON on a workspace.
pub fn record(workspace_id: &str, fields: Value) -> HeldThread {
    let mut v = json!({ "harness": "claude" });
    for (k, x) in fields.as_object().expect("an object").iter() {
        v[k] = x.clone();
    }
    HeldThread { workspace_id: workspace_id.to_string(), record: serde_json::from_value::<ThreadRecord>(v).expect("a ThreadRecord") }
}

/// An event from JSON (its `type` names the variant).
pub fn event(v: Value) -> SessionEvent {
    serde_json::from_value(v).expect("a SessionEvent")
}

/// The event as JSON, for comparing fields.
pub fn json_of<T: serde::Serialize>(x: &T) -> Value {
    serde_json::to_value(x).expect("serializes")
}

/// Rows keyed by their view's id, in the order given.
pub fn sessions_of(rows: Vec<SessionEntry>) -> Sessions {
    rows.into_iter().map(|r| (r.view.id.clone(), r)).collect()
}

/// Records keyed by thread id, in the order given.
pub fn records_of(records: Vec<(&str, HeldThread)>) -> ThreadRecords {
    records.into_iter().map(|(id, r)| (id.to_string(), r)).collect()
}

// ---- clock -------------------------------------------------------------------------------------------------------

struct Due {
    id: u64,
    at: i64,
    f: Option<Timer>,
}

/// A clock the test moves: `advance` runs each due timer at its own time, earliest first, ties in schedule order, a
/// timer scheduled while advancing included when it falls inside the step.
pub struct TestClock {
    now: Cell<i64>,
    next: Cell<u64>,
    due: Rc<RefCell<Vec<Due>>>,
}

impl TestClock {
    pub fn new(start: i64) -> Rc<TestClock> {
        Rc::new(TestClock { now: Cell::new(start), next: Cell::new(0), due: Rc::new(RefCell::new(Vec::new())) })
    }
    pub fn advance(&self, ms: i64) {
        let until = self.now.get() + ms;
        loop {
            let pick = {
                let due = self.due.borrow();
                due.iter().filter(|d| d.at <= until && d.f.is_some()).min_by_key(|d| (d.at, d.id)).map(|d| d.id)
            };
            let Some(id) = pick else { break };
            let (at, f) = {
                let mut due = self.due.borrow_mut();
                let d = due.iter_mut().find(|d| d.id == id).expect("due");
                (d.at, d.f.take())
            };
            self.now.set(at.max(self.now.get()));
            if let Some(f) = f {
                f();
            }
        }
        self.now.set(until);
        self.due.borrow_mut().retain(|d| d.f.is_some());
    }
    /// How many timers are still due.
    pub fn pending(&self) -> usize {
        self.due.borrow().iter().filter(|d| d.f.is_some()).count()
    }
}

impl Clock for TestClock {
    fn now(&self) -> i64 {
        self.now.get()
    }
    fn schedule(&self, f: Timer, ms: i64) -> Cancel {
        let id = self.next.get();
        self.next.set(id + 1);
        self.due.borrow_mut().push(Due { id, at: self.now.get() + ms.max(0), f: Some(f) });
        let due = self.due.clone();
        Rc::new(move || {
            if let Some(d) = due.borrow_mut().iter_mut().find(|d| d.id == id) {
                d.f = None;
            }
        })
    }
}

// ---- ids ---------------------------------------------------------------------------------------------------------

/// Ids that count: `uuid-1`, `uuid-2`, ...; bytes `0, 1, 2, ...`; `random` 0.5.
#[derive(Default)]
pub struct CountingIds {
    pub n: Cell<u64>,
}

impl Ids for CountingIds {
    fn uuid(&self) -> String {
        self.n.set(self.n.get() + 1);
        format!("uuid-{}", self.n.get())
    }
    fn bytes(&self, n: usize) -> Vec<u8> {
        (0..n).map(|i| i as u8).collect()
    }
    fn random(&self) -> f64 {
        0.5
    }
}

// ---- bus ---------------------------------------------------------------------------------------------------------

/// A bus that hands each emitted event to its listeners, typed ones first, then `*`, in the order added.
#[derive(Default)]
pub struct TestBus {
    listeners: RefCell<Vec<(u64, String, EventListener)>>,
    next: Cell<u64>,
    pub emitted: RefCell<Vec<EventUnion>>,
}

impl TestBus {
    pub fn new() -> Rc<TestBus> {
        Rc::new(TestBus::default())
    }
    pub fn emit(&self, e: EventUnion) {
        self.emitted.borrow_mut().push(e.clone());
        let ty = wsp_core::types::events::event_type(&e).to_string();
        let ids: Vec<u64> = {
            let l = self.listeners.borrow();
            l.iter().filter(|(_, t, _)| *t == ty).chain(l.iter().filter(|(_, t, _)| t == "*")).map(|(id, _, _)| *id).collect()
        };
        for id in ids {
            let f = self.listeners.borrow().iter().find(|(i, _, _)| *i == id).map(|(_, _, f)| f.clone());
            if let Some(f) = f {
                f(&e).expect("a listener failed");
            }
        }
    }
    /// An event from JSON, emitted.
    pub fn emit_json(&self, v: Value) {
        self.emit(serde_json::from_value(v).expect("an EventUnion"));
    }
    pub fn listening(&self, ty: &str) -> usize {
        self.listeners.borrow().iter().filter(|(_, t, _)| t == ty).count()
    }
}

/// The bus as an area holds it.
pub struct BusHandle(pub Rc<TestBus>);

impl EventBus for BusHandle {
    fn on(&self, event_type: &str, listener: EventListener) -> Unlisten {
        let bus = &self.0;
        let id = bus.next.get();
        bus.next.set(id + 1);
        bus.listeners.borrow_mut().push((id, event_type.to_string(), listener));
        let me = Rc::downgrade(bus);
        Box::new(move || {
            if let Some(me) = me.upgrade() {
                me.listeners.borrow_mut().retain(|(i, _, _)| *i != id);
            }
        })
    }
    fn since(&self, _after: Option<u64>, _stream: Option<&str>) -> Since {
        Since { stream: "test".into(), head: 0, events: vec![], gap: false }
    }
}

// ---- store -------------------------------------------------------------------------------------------------------

/// An in-memory store that logs every write and can be told to fail one kind of call.
#[derive(Default)]
pub struct FakeStore {
    pub records: RefCell<IndexMap<String, IndexMap<String, Value>>>,
    pub blobs: RefCell<IndexMap<String, IndexMap<String, Vec<u8>>>>,
    /// `put c/id`, `delete c/id`, `putBlob c/id`, `deleteBlob c/id`, in call order.
    pub log: RefCell<Vec<String>>,
    pub fail_put: Cell<bool>,
    pub fail_delete: Cell<bool>,
    pub fail_list: Cell<bool>,
    writes: Cell<f64>,
}

impl FakeStore {
    pub fn new() -> Rc<FakeStore> {
        Rc::new(FakeStore::default())
    }
    pub fn get_now(&self, c: &str, id: &str) -> Option<Value> {
        self.records.borrow().get(c).and_then(|m| m.get(id)).cloned()
    }
    pub fn seed(&self, c: &str, id: &str, v: Value) {
        self.records.borrow_mut().entry(c.to_string()).or_default().insert(id.to_string(), v);
    }
    pub fn blob_now(&self, c: &str, id: &str) -> Option<Vec<u8>> {
        self.blobs.borrow().get(c).and_then(|m| m.get(id)).cloned()
    }
    pub fn seed_blob(&self, c: &str, id: &str, bytes: &[u8]) {
        self.blobs.borrow_mut().entry(c.to_string()).or_default().insert(id.to_string(), bytes.to_vec());
    }
    pub fn logged(&self) -> Vec<String> {
        self.log.borrow().clone()
    }
}

#[async_trait::async_trait(?Send)]
impl Store for FakeStore {
    async fn get(&self, collection: &str, id: &str) -> Result<Option<Value>, HostError> {
        Ok(self.get_now(collection, id))
    }
    async fn put(&self, collection: &str, id: &str, value: Value) -> Result<(), HostError> {
        self.log.borrow_mut().push(format!("put {collection}/{id}"));
        if self.fail_put.get() {
            return Err(err("the disk is full"));
        }
        self.seed(collection, id, value);
        Ok(())
    }
    async fn list(&self, collection: &str) -> Result<Vec<Value>, HostError> {
        if self.fail_list.get() {
            return Err(err("the store did not list"));
        }
        Ok(self.records.borrow().get(collection).map(|m| m.values().cloned().collect()).unwrap_or_default())
    }
    async fn keys(&self, collection: &str) -> Result<Vec<String>, HostError> {
        Ok(self.records.borrow().get(collection).map(|m| m.keys().cloned().collect()).unwrap_or_default())
    }
    async fn delete(&self, collection: &str, id: &str) -> Result<(), HostError> {
        self.log.borrow_mut().push(format!("delete {collection}/{id}"));
        if self.fail_delete.get() {
            return Err(err("the disk is gone"));
        }
        if let Some(m) = self.records.borrow_mut().get_mut(collection) {
            m.shift_remove(id);
        }
        Ok(())
    }
    async fn get_blob(&self, collection: &str, id: &str) -> Result<Option<Vec<u8>>, HostError> {
        Ok(self.blob_now(collection, id))
    }
    async fn put_blob(&self, collection: &str, id: &str, bytes: Vec<u8>) -> Result<(), HostError> {
        self.log.borrow_mut().push(format!("putBlob {collection}/{id}"));
        if self.fail_put.get() {
            return Err(err("the disk is full"));
        }
        self.seed_blob(collection, id, &bytes);
        self.writes.set(self.writes.get() + 1.0);
        Ok(())
    }
    async fn delete_blob(&self, collection: &str, id: &str) -> Result<(), HostError> {
        self.log.borrow_mut().push(format!("deleteBlob {collection}/{id}"));
        if let Some(m) = self.blobs.borrow_mut().get_mut(collection) {
            m.shift_remove(id);
        }
        Ok(())
    }
    async fn stat_blob(&self, collection: &str, id: &str) -> Result<Option<BlobMark>, HostError> {
        Ok(self.blob_now(collection, id).map(|b| BlobMark { bytes: wsp_wire::types::js_number::JsNumber(b.len() as f64), at: wsp_wire::types::js_number::JsNumber(self.writes.get()) }))
    }
}

// ---- machine -----------------------------------------------------------------------------------------------------

/// A machine that answers each exec from a script: the first rule whose needle the command contains answers, else
/// exit 0 with no output. Every exec is logged with its timeout.
#[derive(Default)]
pub struct FakeMachine {
    pub id: String,
    pub rules: RefCell<Vec<(String, Result<ExecResult, String>)>>,
    pub execs: RefCell<Vec<(String, Option<u64>)>>,
    pub put: RefCell<Vec<(String, Vec<u8>)>>,
}

impl FakeMachine {
    pub fn new(id: &str) -> std::sync::Arc<FakeMachine> {
        std::sync::Arc::new(FakeMachine { id: id.to_string(), ..Default::default() })
    }
    /// Answers a command containing `needle` with this exit and output.
    pub fn answer(&self, needle: &str, exit_code: i64, stdout: &str) {
        self.rules.borrow_mut().push((needle.to_string(), Ok(exec_result(exit_code, stdout, ""))));
    }
    /// Fails a command containing `needle`.
    pub fn fail(&self, needle: &str, message: &str) {
        self.rules.borrow_mut().push((needle.to_string(), Err(message.to_string())));
    }
    pub fn commands(&self) -> Vec<String> {
        self.execs.borrow().iter().map(|(c, _)| c.clone()).collect()
    }
}

pub fn exec_result(exit_code: i64, stdout: &str, stderr: &str) -> ExecResult {
    serde_json::from_value(json!({ "exitCode": exit_code, "stdout": stdout, "stderr": stderr })).expect("an ExecResult")
}

fn engine_err(message: &str) -> EngineError {
    EngineError::Other { message: message.to_string(), code: None }
}

#[async_trait::async_trait(?Send)]
impl Machine for FakeMachine {
    fn id(&self) -> &str {
        &self.id
    }
    fn kind(&self) -> MachineKind {
        serde_json::from_value(json!("local")).expect("a MachineKind")
    }
    async fn exec(&self, cmd: &str, opts: ExecOptions) -> wsp_engine::errors::Result<ExecResult> {
        self.execs.borrow_mut().push((cmd.to_string(), opts.timeout_ms));
        let rule = self.rules.borrow().iter().find(|(n, _)| cmd.contains(n.as_str())).map(|(_, r)| r.clone());
        match rule {
            Some(Ok(r)) => Ok(r),
            Some(Err(m)) => Err(engine_err(&m)),
            None => Ok(exec_result(0, "", "")),
        }
    }
    async fn run(&self, script: &str, opts: RunOptions) -> wsp_engine::errors::Result<ExecResult> {
        let _ = opts;
        self.exec(script, ExecOptions { timeout_ms: None, idempotency_key: None, stdin: None }).await
    }
    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> wsp_engine::errors::Result<String> {
        Err(engine_err("no snapshot here"))
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> wsp_engine::errors::Result<()> {
        Ok(())
    }
    async fn resume(&self, _signal: Option<&wsp_engine::Cancel>) -> wsp_engine::errors::Result<()> {
        Ok(())
    }
    async fn kill(&self) -> wsp_engine::errors::Result<()> {
        Ok(())
    }
    async fn state(&self) -> wsp_engine::errors::Result<MachineState> {
        Err(engine_err("no state here"))
    }
    async fn download_url(&self, _path: &str) -> wsp_engine::errors::Result<String> {
        Err(engine_err("no url here"))
    }
    async fn upload_url(&self, _path: &str) -> wsp_engine::errors::Result<String> {
        Err(engine_err("no url here"))
    }
    async fn put_bytes(&self, path: &str, bytes: &[u8], _timeout_ms: Option<u64>) -> wsp_engine::errors::Result<Option<wsp_engine::machine::BytesLanded>> {
        self.put.borrow_mut().push((path.to_string(), bytes.to_vec()));
        Ok(None)
    }
}

// ---- transcript rows ---------------------------------------------------------------------------------------------

/// One kept row.
#[derive(Clone, Debug)]
pub struct KeptRow {
    pub pos: i64,
    pub thread: String,
    pub kind: String,
    pub size: u64,
    pub event: SessionEvent,
}

fn pos_of(e: &SessionEvent) -> i64 {
    json_of(e)["pos"].as_i64().unwrap_or(0)
}

fn thread_of(e: &SessionEvent) -> String {
    json_of(e)["threadId"].as_str().unwrap_or("").to_string()
}

fn type_of(e: &SessionEvent) -> String {
    json_of(e)["type"].as_str().unwrap_or("").to_string()
}

/// Transcript rows in memory, as the SQLite module keeps them: one row per event keyed by workspace and `pos`, an
/// index row per workspace, transactions that undo their writes when they fail. `fail` makes every call fail.
#[derive(Default)]
pub struct FakeRows {
    pub rows: RefCell<IndexMap<String, Vec<KeptRow>>>,
    pub indexes: RefCell<IndexMap<String, String>>,
    pub fail: Cell<bool>,
    pub fail_append: Cell<bool>,
    pub transactions: Cell<u32>,
    depth: Cell<u32>,
}

impl FakeRows {
    pub fn new() -> Rc<FakeRows> {
        Rc::new(FakeRows::default())
    }
    pub fn seed(&self, ws: &str, events: Vec<SessionEvent>) {
        let mut rows = self.rows.borrow_mut();
        let kept = rows.entry(ws.to_string()).or_default();
        for e in events {
            kept.push(KeptRow { pos: pos_of(&e), thread: thread_of(&e), kind: type_of(&e), size: event_bytes(&e), event: e });
        }
    }
    pub fn kept(&self, ws: &str) -> Vec<SessionEvent> {
        self.rows.borrow().get(ws).map(|r| r.iter().map(|k| k.event.clone()).collect()).unwrap_or_default()
    }
    pub fn positions(&self, ws: &str) -> Vec<i64> {
        self.rows.borrow().get(ws).map(|r| r.iter().map(|k| k.pos).collect()).unwrap_or_default()
    }
    fn check(&self) -> Result<(), HostError> {
        if self.fail.get() {
            Err(err("database is locked"))
        } else {
            Ok(())
        }
    }
    fn of(&self, ws: &str, thread: Option<&str>) -> Vec<KeptRow> {
        self.rows.borrow().get(ws).map(|r| r.iter().filter(|k| thread.is_none_or(|t| k.thread == t)).cloned().collect()).unwrap_or_default()
    }
}

impl TranscriptRows for FakeRows {
    fn all(&self, ws: &str) -> Result<Summed, HostError> {
        self.check()?;
        let rows = self.of(ws, None);
        Ok(Summed { size: rows.iter().map(|k| k.size).sum(), events: rows.into_iter().map(|k| k.event).collect() })
    }
    fn newest(&self, ws: &str, thread: &str, before: Option<i64>, each: &mut dyn FnMut(SessionEvent) -> ControlFlow<()>) -> Result<(), HostError> {
        self.check()?;
        for k in self.of(ws, Some(thread)).into_iter().rev() {
            if before.is_some_and(|b| k.pos >= b) {
                continue;
            }
            if each(k.event).is_break() {
                break;
            }
        }
        Ok(())
    }
    fn count(&self, ws: &str, thread: &str) -> Result<u64, HostError> {
        self.check()?;
        Ok(self.of(ws, Some(thread)).len() as u64)
    }
    fn total(&self, ws: &str, thread: Option<&str>) -> Result<Total, HostError> {
        self.check()?;
        let rows = self.of(ws, thread);
        Ok(Total { count: rows.len() as u64, size: rows.iter().map(|k| k.size).sum() })
    }
    fn newest_pos(&self, ws: &str) -> Result<i64, HostError> {
        self.check()?;
        Ok(self.of(ws, None).iter().map(|k| k.pos).max().unwrap_or(0))
    }
    fn sizes(&self, ws: &str, thread: Option<&str>) -> Result<Vec<EventSize>, HostError> {
        self.check()?;
        Ok(self.of(ws, thread).into_iter().map(|k| EventSize { pos: k.pos, thread_id: (!k.thread.is_empty()).then_some(k.thread), kind: k.kind, size: k.size }).collect())
    }
    fn word_events(&self, ws: &str, thread: &str) -> Result<Vec<SessionEvent>, HostError> {
        self.check()?;
        Ok(self
            .of(ws, Some(thread))
            .into_iter()
            .filter(|k| matches!(k.kind.as_str(), "session.start" | "session.steer" | "session.earlier") || (k.kind == "session.delta" && json_of(&k.event)["kind"] == "text"))
            .map(|k| k.event)
            .collect())
    }
    fn thread(&self, ws: &str, thread: &str) -> Result<Summed, HostError> {
        self.check()?;
        let rows = self.of(ws, Some(thread));
        Ok(Summed { size: rows.iter().map(|k| k.size).sum(), events: rows.into_iter().map(|k| k.event).collect() })
    }
    fn at(&self, ws: &str, positions: &[i64]) -> Result<Vec<SessionEvent>, HostError> {
        self.check()?;
        let rows = self.of(ws, None);
        Ok(positions.iter().filter_map(|p| rows.iter().find(|k| k.pos == *p).map(|k| k.event.clone())).collect())
    }
    fn append(&self, ws: &str, events: &[SessionEvent]) -> Result<(), HostError> {
        self.check()?;
        if self.fail_append.get() {
            return Err(err("UNIQUE constraint failed: events.workspace, events.pos"));
        }
        if events.iter().any(|e| self.of(ws, None).iter().any(|k| k.pos == pos_of(e))) {
            return Err(err("UNIQUE constraint failed: events.workspace, events.pos"));
        }
        self.seed(ws, events.to_vec());
        Ok(())
    }
    fn remove(&self, ws: &str, positions: &[i64]) -> Result<(), HostError> {
        self.check()?;
        if let Some(r) = self.rows.borrow_mut().get_mut(ws) {
            r.retain(|k| !positions.contains(&k.pos));
        }
        Ok(())
    }
    fn clear(&self, ws: &str) -> Result<(), HostError> {
        self.check()?;
        self.rows.borrow_mut().shift_remove(ws);
        self.indexes.borrow_mut().shift_remove(ws);
        Ok(())
    }
    fn index(&self, ws: &str) -> Result<Option<String>, HostError> {
        self.check()?;
        Ok(self.indexes.borrow().get(ws).cloned())
    }
    fn put_index(&self, ws: &str, json: &str) -> Result<(), HostError> {
        self.check()?;
        self.indexes.borrow_mut().insert(ws.to_string(), json.to_string());
        Ok(())
    }
    fn atomically(&self, f: &mut dyn FnMut() -> Result<(), HostError>) -> Result<(), HostError> {
        self.check()?;
        if self.depth.get() > 0 {
            return f();
        }
        self.transactions.set(self.transactions.get() + 1);
        let rows = self.rows.borrow().clone();
        let indexes = self.indexes.borrow().clone();
        self.depth.set(1);
        let out = f();
        self.depth.set(0);
        if out.is_err() {
            *self.rows.borrow_mut() = rows;
            *self.indexes.borrow_mut() = indexes;
        }
        out
    }
}

// ---- kept agent --------------------------------------------------------------------------------------------------

/// A kept agent process: counts its closes, exits when told, and says whether it works and where its file is.
pub struct FakeKeptAgent {
    pub closed: std::sync::Arc<std::sync::Mutex<Vec<bool>>>,
    pub exit: std::sync::Arc<std::sync::Mutex<Option<futures::channel::oneshot::Sender<()>>>>,
    exited: Shared<BoxFuture<'static, ()>>,
    pub working: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub file: Option<wsp_adapters::port::SessionFile>,
    pub close_fails: bool,
}

impl FakeKeptAgent {
    pub fn new(file: Option<wsp_adapters::port::SessionFile>) -> FakeKeptAgent {
        let (tx, rx) = futures::channel::oneshot::channel::<()>();
        FakeKeptAgent {
            closed: Default::default(),
            exit: std::sync::Arc::new(std::sync::Mutex::new(Some(tx))),
            exited: rx.map(|_| ()).boxed().shared(),
            working: Default::default(),
            file,
            close_fails: false,
        }
    }
}

impl wsp_adapters::port::KeptAgent for FakeKeptAgent {
    fn next(&self, _turn: wsp_adapters::port::KeptTurn) -> Result<std::sync::Arc<dyn wsp_adapters::port::HarnessSession>, wsp_adapters::port::AdapterError> {
        panic!("a kept agent's next turn is not these tests'")
    }
    fn close(&self, now: bool) -> BoxFuture<'static, ()> {
        self.closed.lock().expect("closed").push(now);
        async {}.boxed()
    }
    fn exited(&self) -> wsp_adapters::port::SharedFuture<()> {
        self.exited.clone()
    }
    fn session_file(&self) -> Option<wsp_adapters::port::SessionFile> {
        self.file.clone()
    }
    fn working(&self) -> Option<bool> {
        Some(self.working.load(std::sync::atomic::Ordering::SeqCst))
    }
}

/// Runs `f` on a current-thread runtime inside a `LocalSet`, as the host runs.
pub fn local<F: std::future::Future<Output = ()>>(f: F) {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a runtime");
    tokio::task::LocalSet::new().block_on(&rt, f);
}

/// Lets every spawned local task run until none can.
pub async fn settle() {
    for _ in 0..50 {
        tokio::task::yield_now().await;
    }
}

/// A future that is already done.
pub fn done<T: Clone + 'static>(v: T) -> LocalBoxFuture<'static, T> {
    async move { v }.boxed_local()
}
