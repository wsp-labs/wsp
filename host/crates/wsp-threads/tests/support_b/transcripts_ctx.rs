// SPDX-License-Identifier: AGPL-3.0-only
//! The transcripts area's fake context: the runtime's shared transcript state, rows and records in `StateCell`s, a
//! test clock, bus and store, watched transcript rows that log every call and can be made to fail one road, and a log
//! of every call out, so a test asserts what the area did and in what order.
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::ops::ControlFlow;
use std::rc::Rc;
use std::sync::Arc;

use futures::channel::oneshot;
use futures::future::{FutureExt, LocalBoxFuture};
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_core::clock::Clock;
use wsp_core::context::{Joined, StateCell};
use wsp_core::ids::Ids;
use wsp_core::types::events::EventBus;
use wsp_core::types::internal::TranscriptIndex;
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_engine::machine::Machine;
use wsp_store::sqlite_transcripts::{EventSize, Summed, Total, TranscriptRows};
use wsp_store::store::{BlobMark, Store};
use wsp_threads::threads::threads::{Entry, Sessions, ThreadRecords};
use wsp_threads::threads::transcripts::{TranscriptState, TranscriptsArea, TranscriptsCtx};
use wsp_wire::views::session::SessionView;
use wsp_wire::views::session_events_more::SessionEvent;
use wsp_words::exit::HostError;

use super::{err, BusHandle, CountingIds, FakeMachine, FakeRows, FakeStore, TestBus, TestClock};

/// Random bytes that run on across calls (`0, 1, 2, ...`), so two draws differ; uuids and `random` as `CountingIds`.
#[derive(Default)]
pub struct DrawnIds {
    pub inner: CountingIds,
    pub drawn: Cell<usize>,
}

impl Ids for DrawnIds {
    fn uuid(&self) -> String {
        self.inner.uuid()
    }
    fn bytes(&self, n: usize) -> Vec<u8> {
        let from = self.drawn.get();
        self.drawn.set(from + n);
        (0..n).map(|i| (from + i) as u8).collect()
    }
    fn random(&self) -> f64 {
        self.inner.random()
    }
}

/// `with` on a shared map, as `StateCell::with` reads: the closure gets the map, borrowed only while it runs.
pub trait WithState<T> {
    fn with<R>(&self, f: impl FnOnce(&mut T) -> R) -> R;
}

impl<T> WithState<T> for Rc<RefCell<T>> {
    fn with<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        f(&mut self.borrow_mut())
    }
}

fn spaced(positions: &[i64]) -> String {
    positions.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(",")
}

fn or_dash(thread: Option<&str>) -> &str {
    thread.unwrap_or("-")
}

/// `FakeRows` that logs each call as `name workspace [thread|positions]` and can fail `all` or `put_index` alone.
pub struct WatchedRows {
    pub inner: Rc<FakeRows>,
    pub log: Rc<RefCell<Vec<String>>>,
    pub fail_all: Cell<bool>,
    pub fail_put_index: Cell<bool>,
}

impl WatchedRows {
    fn note(&self, line: String) {
        self.log.borrow_mut().push(line);
    }
}

impl TranscriptRows for WatchedRows {
    fn all(&self, ws: &str) -> Result<Summed, HostError> {
        self.note(format!("all {ws}"));
        if self.fail_all.get() {
            return Err(err("disk I/O error"));
        }
        self.inner.all(ws)
    }
    fn newest(
        &self,
        ws: &str,
        thread: &str,
        before: Option<i64>,
        each: &mut dyn FnMut(SessionEvent) -> ControlFlow<()>,
    ) -> Result<(), HostError> {
        self.note(format!("newest {ws} {thread}"));
        self.inner.newest(ws, thread, before, each)
    }
    fn count(&self, ws: &str, thread: &str) -> Result<u64, HostError> {
        self.note(format!("count {ws} {thread}"));
        self.inner.count(ws, thread)
    }
    fn total(&self, ws: &str, thread: Option<&str>) -> Result<Total, HostError> {
        self.note(format!("total {ws} {}", or_dash(thread)));
        self.inner.total(ws, thread)
    }
    fn newest_pos(&self, ws: &str) -> Result<i64, HostError> {
        self.note(format!("newest_pos {ws}"));
        self.inner.newest_pos(ws)
    }
    fn sizes(&self, ws: &str, thread: Option<&str>) -> Result<Vec<EventSize>, HostError> {
        self.note(format!("sizes {ws} {}", or_dash(thread)));
        self.inner.sizes(ws, thread)
    }
    fn word_events(&self, ws: &str, thread: &str) -> Result<Vec<SessionEvent>, HostError> {
        self.note(format!("word_events {ws} {thread}"));
        self.inner.word_events(ws, thread)
    }
    fn thread(&self, ws: &str, thread: &str) -> Result<Summed, HostError> {
        self.note(format!("thread {ws} {thread}"));
        self.inner.thread(ws, thread)
    }
    fn at(&self, ws: &str, positions: &[i64]) -> Result<Vec<SessionEvent>, HostError> {
        self.note(format!("at {ws} {}", spaced(positions)));
        self.inner.at(ws, positions)
    }
    fn append(&self, ws: &str, events: &[SessionEvent]) -> Result<(), HostError> {
        self.note(format!("append {ws} {}", events.len()));
        self.inner.append(ws, events)
    }
    fn remove(&self, ws: &str, positions: &[i64]) -> Result<(), HostError> {
        self.note(format!("remove {ws} {}", spaced(positions)));
        self.inner.remove(ws, positions)
    }
    fn clear(&self, ws: &str) -> Result<(), HostError> {
        self.note(format!("clear {ws}"));
        self.inner.clear(ws)
    }
    fn index(&self, ws: &str) -> Result<Option<String>, HostError> {
        self.note(format!("index {ws}"));
        self.inner.index(ws)
    }
    fn put_index(&self, ws: &str, json: &str) -> Result<(), HostError> {
        self.note(format!("put_index {ws}"));
        if self.fail_put_index.get() {
            return Err(err("disk I/O error"));
        }
        self.inner.put_index(ws, json)
    }
    fn atomically(&self, f: &mut dyn FnMut() -> Result<(), HostError>) -> Result<(), HostError> {
        self.note("atomically".to_string());
        self.inner.atomically(f)
    }
}

/// `FakeStore` that can also fail the writes of blobs alone (`put` of a record still lands), and keeps every record
/// put with its value, in order.
pub struct FlakyStore {
    pub inner: Rc<FakeStore>,
    pub fail_blobs: Cell<bool>,
    /// Every `put` that was asked: collection, id, value.
    pub puts: RefCell<Vec<(String, String, Value)>>,
}

#[async_trait::async_trait(?Send)]
impl Store for FlakyStore {
    async fn get(&self, collection: &str, id: &str) -> Result<Option<Value>, HostError> {
        self.inner.get(collection, id).await
    }
    async fn put(&self, collection: &str, id: &str, value: Value) -> Result<(), HostError> {
        self.puts.borrow_mut().push((collection.to_string(), id.to_string(), value.clone()));
        self.inner.put(collection, id, value).await
    }
    async fn list(&self, collection: &str) -> Result<Vec<Value>, HostError> {
        self.inner.list(collection).await
    }
    async fn keys(&self, collection: &str) -> Result<Vec<String>, HostError> {
        self.inner.keys(collection).await
    }
    async fn delete(&self, collection: &str, id: &str) -> Result<(), HostError> {
        self.inner.delete(collection, id).await
    }
    async fn get_blob(&self, collection: &str, id: &str) -> Result<Option<Vec<u8>>, HostError> {
        self.inner.get_blob(collection, id).await
    }
    async fn put_blob(&self, collection: &str, id: &str, bytes: Vec<u8>) -> Result<(), HostError> {
        if self.fail_blobs.get() {
            self.inner.log.borrow_mut().push(format!("putBlob {collection}/{id} refused"));
            return Err(err("the host went down"));
        }
        self.inner.put_blob(collection, id, bytes).await
    }
    async fn delete_blob(&self, collection: &str, id: &str) -> Result<(), HostError> {
        self.inner.delete_blob(collection, id).await
    }
    async fn stat_blob(&self, collection: &str, id: &str) -> Result<Option<BlobMark>, HostError> {
        self.inner.stat_blob(collection, id).await
    }
}

pub struct FakeTranscriptsCtx {
    pub clock: Rc<TestClock>,
    pub ids: Rc<DrawnIds>,
    pub bus: Rc<TestBus>,
    /// The store the area writes through: `store` with the blob writes that can be failed.
    pub store: Rc<FakeStore>,
    pub flaky: Rc<FlakyStore>,
    /// The rows themselves; the area reaches them through `watched`.
    pub rows: Rc<FakeRows>,
    pub watched: Rc<WatchedRows>,
    pub transcript_state: StateCell<TranscriptState>,
    pub sessions: Sessions,
    pub thread_records: ThreadRecords,
    pub index_flushes: StateCell<IndexMap<String, Joined>>,
    /// The live workspaces, by id.
    pub live: RefCell<IndexMap<String, Entry>>,
    pub closed: Cell<bool>,
    /// Every thread `push_head` was handed, in order.
    pub pushed: RefCell<Vec<String>>,
    /// `rows_on(thread)` answers.
    pub rows_on: RefCell<IndexMap<String, Vec<SessionView>>>,
    /// `latest_on(thread)` answers.
    pub latest_on: RefCell<IndexMap<String, SessionView>>,
    /// `resumed_fact(workspace, session, fact)` answers, keyed by the three.
    pub facts: RefCell<IndexMap<(String, String, String), String>>,
    /// Every `resumed_fact` ask, in order.
    pub fact_asks: RefCell<Vec<(String, String, String)>>,
    /// `daemon_token_seed()`.
    pub seed: RefCell<Option<String>>,
    /// Every rotation asked: machine id, token, path.
    pub rotations: RefCell<Vec<(String, String, Option<String>)>>,
    /// What each rotation answers, in order; `Ok(true)` when none is queued.
    pub rotate_answers: RefCell<VecDeque<Result<bool, HostError>>>,
    /// While set, a rotation does not answer until `release_rotations`.
    pub hold_rotations: Cell<bool>,
    gates: RefCell<Vec<oneshot::Sender<()>>>,
    /// Every call out, as `name arg`, in order.
    pub calls: RefCell<Vec<String>>,
}

impl FakeTranscriptsCtx {
    pub fn new() -> Rc<FakeTranscriptsCtx> {
        let rows = FakeRows::new();
        let store = FakeStore::new();
        Rc::new(FakeTranscriptsCtx {
            clock: TestClock::new(1_000_000),
            ids: Rc::new(DrawnIds::default()),
            bus: TestBus::new(),
            flaky: Rc::new(FlakyStore { inner: store.clone(), fail_blobs: Cell::new(false), puts: RefCell::default() }),
            store,
            watched: Rc::new(WatchedRows {
                inner: rows.clone(),
                log: Rc::default(),
                fail_all: Cell::new(false),
                fail_put_index: Cell::new(false),
            }),
            rows,
            transcript_state: StateCell::new(TranscriptState::default()),
            sessions: Sessions::default(),
            thread_records: ThreadRecords::default(),
            index_flushes: StateCell::new(IndexMap::new()),
            live: RefCell::default(),
            closed: Cell::new(false),
            pushed: RefCell::default(),
            rows_on: RefCell::default(),
            latest_on: RefCell::default(),
            facts: RefCell::default(),
            fact_asks: RefCell::default(),
            seed: RefCell::new(None),
            rotations: RefCell::default(),
            rotate_answers: RefCell::default(),
            hold_rotations: Cell::new(false),
            gates: RefCell::default(),
            calls: RefCell::default(),
        })
    }
    /// The area over this context.
    pub fn area(self: &Rc<Self>) -> Rc<TranscriptsArea> {
        TranscriptsArea::new(self.clone()).expect("the area builds")
    }
    pub fn call(&self, line: String) {
        self.calls.borrow_mut().push(line);
    }
    pub fn called(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
    /// Every call the area made on the rows, in order, as `name workspace [thread|positions]`.
    pub fn rows_called(&self) -> Vec<String> {
        self.watched.log.borrow().clone()
    }
    /// The calls on the rows whose line starts with one of `names`, in order.
    pub fn rows_called_of(&self, names: &[&str]) -> Vec<String> {
        self.rows_called().into_iter().filter(|l| names.iter().any(|n| l == n || l.starts_with(&format!("{n} ")))).collect()
    }
    pub fn forget_rows_calls(&self) {
        self.watched.log.borrow_mut().clear();
    }
    /// Holds this index for the workspace.
    pub fn hold_index(&self, workspace_id: &str, index: TranscriptIndex) {
        self.transcript_state.with(|s| {
            s.index.insert(workspace_id.to_string(), index);
        });
    }
    /// The index held for the workspace, if any.
    pub fn index_of(&self, workspace_id: &str) -> Option<TranscriptIndex> {
        self.transcript_state.with(|s| s.index.get(workspace_id).cloned())
    }
    /// The events written since the last flush.
    pub fn pending_of(&self, workspace_id: &str) -> Vec<SessionEvent> {
        self.transcript_state.with(|s| s.pending.get(workspace_id).cloned().unwrap_or_default())
    }
    pub fn pending_json(&self, workspace_id: &str) -> Vec<Value> {
        self.pending_of(workspace_id).iter().map(super::json_of).collect()
    }
    pub fn pending_bytes_of(&self, workspace_id: &str) -> Option<u64> {
        self.transcript_state.with(|s| s.pending_bytes.get(workspace_id).copied())
    }
    /// The workspaces whose index did not read.
    pub fn unread(&self) -> Vec<String> {
        self.transcript_state.with(|s| s.unread.iter().cloned().collect())
    }
    /// What the bus was handed, as JSON.
    pub fn emitted_json(&self) -> Vec<Value> {
        self.bus.emitted.borrow().iter().map(super::json_of).collect()
    }
    /// A live workspace, its record read from JSON over the fields a record needs (`id`, `name`, `kind` cloud).
    pub fn add_live(&self, fields: Value) -> Arc<FakeMachine> {
        let (entry, machine) = live_entry(fields);
        let id = entry.borrow().record.borrow().id.clone();
        self.live.borrow_mut().insert(id, entry);
        machine
    }
    /// Lets every rotation held answer.
    pub fn release_rotations(&self) {
        for gate in self.gates.borrow_mut().drain(..) {
            let _ = gate.send(());
        }
    }
    pub fn rotations_asked(&self) -> Vec<(String, String, Option<String>)> {
        self.rotations.borrow().clone()
    }
}

/// A live entry over a fake machine: a full workspace record from JSON, the fields given over these defaults.
pub fn live_entry(fields: Value) -> (Entry, Arc<FakeMachine>) {
    let mut v = json!({
        "id": "ws1",
        "name": "ws1",
        "machineId": "m1",
        "phase": "running",
        "kind": "cloud",
        "golden": "g1",
        "createdAt": "2026-01-01T00:00:00.000Z",
        "project": "p1",
        "spec": {},
        "size": { "cpu": 1, "memMb": 1024 },
        "firstLife": true,
    });
    for (k, x) in fields.as_object().expect("an object").iter() {
        v[k] = x.clone();
    }
    let machine = FakeMachine::new(v["machineId"].as_str().unwrap_or("m1"));
    let record: WorkspaceRecord = serde_json::from_value(v).expect("a WorkspaceRecord");
    (super::entry::entry_over(record, machine.clone()), machine)
}

impl TranscriptsCtx for FakeTranscriptsCtx {
    fn spawn(&self, work: LocalBoxFuture<'static, ()>) {
        super::spawn_detached(work);
    }
    fn clock(&self) -> Rc<dyn Clock> {
        self.clock.clone()
    }
    fn ids(&self) -> Rc<dyn Ids> {
        self.ids.clone()
    }
    fn bus(&self) -> Rc<dyn EventBus> {
        Rc::new(BusHandle(self.bus.clone()))
    }
    fn emit(&self, event: wsp_wire::wire::events_union::EventUnion) -> Result<(), wsp_words::exit::HostError> {
        self.bus.emit(event);
        Ok(())
    }
    fn store(&self) -> Rc<dyn Store> {
        self.flaky.clone()
    }
    fn rows(&self) -> Rc<dyn TranscriptRows> {
        self.watched.clone()
    }
    fn transcript_state(&self) -> StateCell<TranscriptState> {
        self.transcript_state.clone()
    }
    fn sessions(&self) -> Sessions {
        self.sessions.clone()
    }
    fn thread_records(&self) -> ThreadRecords {
        self.thread_records.clone()
    }
    fn index_flushes(&self) -> StateCell<IndexMap<String, Joined>> {
        self.index_flushes.clone()
    }
    fn entry(&self, workspace_id: &str) -> Option<Entry> {
        self.live.borrow().get(workspace_id).cloned()
    }
    fn is_live(&self, workspace_id: &str) -> bool {
        self.live.borrow().contains_key(workspace_id)
    }
    fn closed(&self) -> bool {
        self.closed.get()
    }
    fn push_head(&self, thread_id: &str) {
        self.pushed.borrow_mut().push(thread_id.to_string());
        self.call(format!("pushHead {thread_id}"));
    }
    fn rows_on(&self, thread_id: &str) -> Vec<SessionView> {
        self.rows_on.borrow().get(thread_id).cloned().unwrap_or_default()
    }
    fn latest_on(&self, thread_id: &str) -> Option<SessionView> {
        self.latest_on.borrow().get(thread_id).cloned()
    }
    fn resumed_fact(&self, workspace_id: &str, session: &str, fact: &str) -> Option<String> {
        let key = (workspace_id.to_string(), session.to_string(), fact.to_string());
        self.fact_asks.borrow_mut().push(key.clone());
        self.facts.borrow().get(&key).cloned()
    }
    fn daemon_token_seed(&self) -> Option<String> {
        self.seed.borrow().clone()
    }
    fn rotate_daemon_token(
        &self,
        machine: Arc<dyn Machine>,
        token: String,
        path: Option<String>,
    ) -> LocalBoxFuture<'static, Result<bool, HostError>> {
        self.rotations.borrow_mut().push((machine.id().to_string(), token, path));
        let answer = self.rotate_answers.borrow_mut().pop_front().unwrap_or(Ok(true));
        if self.hold_rotations.get() {
            let (tx, rx) = oneshot::channel();
            self.gates.borrow_mut().push(tx);
            async move {
                let _ = rx.await;
                answer
            }
            .boxed_local()
        } else {
            async move { answer }.boxed_local()
        }
    }
}
