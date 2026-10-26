// SPDX-License-Identifier: AGPL-3.0-only
//! The test half every projects test shares: the real fake clock, ids drawn in a known order, a store in memory,
//! and `FakeBase`, which implements `seam::Base` over shared state, logs every call in order with its arguments, and
//! answers from a script. A unit's own fake (support/<area>.rs) wraps a `FakeBase` and implements the unit's trait the
//! same way: `self.base.call(name, args)` logs, `self.base.answer(name)` takes the next scripted answer.
#![allow(dead_code, unused_imports)]
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use futures::FutureExt;
use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
#[path = "../common/entry.rs"]
mod entry;
#[path = "../common/errors.rs"]
mod errors;
pub use entry::{entry_over, in_flight, FakeMachine};
pub use errors::{kind_named, kinded, plain, refused};

use wsp_account::projects::workspaces::seam::{
    Base, DaemonAsk, DaemonWork, Entry, EntryAsk, Event, FreeSlot, GitAsk, HereWork, Live, ProjectsHeld,
};
use wsp_core::clock::{Clock, FakeClock};
use wsp_core::ids::Ids;
use wsp_core::types::internal::ThreadRecord;
use wsp_core::types::pending::{BlobMark, Store};
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_wire::views::preferences::Preferences;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::workspace::{Caller, ProjectView};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::WorkspaceView;
use wsp_wire::wire::machine_link::ExecResult;
use wsp_words::exit::{HostError, Kind};

// ---------------------------------------------------------------------------------------------------------------
// Threads

/// A thread record naming its workspace and nothing else a project area reads.
pub fn thread_on(workspace_id: &str) -> ThreadRecord {
    ThreadRecord { workspace_id: workspace_id.to_owned(), harness: "claude".to_owned(), ..ThreadRecord::default() }
}

// ---------------------------------------------------------------------------------------------------------------
// Errors

/// A scripted failure: `{"$error": {"message": .., "kind": ..}}` in a script queue.
pub fn err_answer(message: &str, kind: Option<&str>) -> Value {
    json!({ "$error": { "message": message, "kind": kind } })
}

fn error_of(v: &Value) -> Option<HostError> {
    let e = v.get("$error")?;
    let message = e.get("message").and_then(Value::as_str).unwrap_or("").to_owned();
    let kind = e.get("kind").and_then(Value::as_str).map(kind_named);
    Some(HostError { kind, ..plain(&message) })
}

// ---------------------------------------------------------------------------------------------------------------
// Clock

/// The timers armed on `clock` are exactly those due at `ats` (each moment named once).
pub fn assert_due(clock: &FakeClock, ats: &[i64]) {
    assert_eq!(clock.pending(), ats.len(), "timers armed, wanted at {ats:?}");
    for at in ats {
        assert!(clock.due_for(*at), "no timer due at {at}");
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Ids

/// Ids in a known order: UUIDs `00000000-0000-4000-8000-00000000000<n>` from 1, and each `bytes(n)` draw filled with
/// one byte, 0x11 first, then 0x22, 0x33 and on, so `ws_<8 hex>` reads `ws_11111111`, then `ws_22222222`.
pub struct TestIds {
    uuids: Cell<u64>,
    draws: Cell<u8>,
}

impl TestIds {
    pub fn new() -> Rc<TestIds> {
        Rc::new(TestIds { uuids: Cell::new(0), draws: Cell::new(0) })
    }
}

impl Ids for TestIds {
    fn uuid(&self) -> String {
        let n = self.uuids.get() + 1;
        self.uuids.set(n);
        format!("00000000-0000-4000-8000-{n:012}")
    }
    fn bytes(&self, n: usize) -> Vec<u8> {
        let k = self.draws.get().wrapping_add(1);
        self.draws.set(k);
        vec![k.wrapping_mul(0x11); n]
    }
    fn random(&self) -> f64 {
        0.5
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Store

/// The store in memory, collections and ids in first-write order, as the SQLite store keeps them.
#[derive(Default)]
pub struct MemStore {
    pub rows: RefCell<IndexMap<String, IndexMap<String, Value>>>,
    pub blobs: RefCell<IndexMap<String, IndexMap<String, Vec<u8>>>>,
    /// Every write in order: `put <collection>/<id>` or `delete <collection>/<id>`.
    pub writes: RefCell<Vec<String>>,
}

impl MemStore {
    pub fn new() -> Rc<MemStore> {
        Rc::new(MemStore::default())
    }
    pub fn row(&self, collection: &str, id: &str) -> Option<Value> {
        self.rows.borrow().get(collection).and_then(|c| c.get(id)).cloned()
    }
    pub fn seed(&self, collection: &str, id: &str, value: Value) {
        self.rows.borrow_mut().entry(collection.to_owned()).or_default().insert(id.to_owned(), value);
    }
}

#[async_trait(?Send)]
impl Store for MemStore {
    async fn get(&self, collection: &str, id: &str) -> Result<Option<Value>, HostError> {
        Ok(self.row(collection, id))
    }
    async fn put(&self, collection: &str, id: &str, value: Value) -> Result<(), HostError> {
        self.writes.borrow_mut().push(format!("put {collection}/{id}"));
        self.seed(collection, id, value);
        Ok(())
    }
    async fn list(&self, collection: &str) -> Result<Vec<Value>, HostError> {
        Ok(self.rows.borrow().get(collection).map(|c| c.values().cloned().collect()).unwrap_or_default())
    }
    async fn keys(&self, collection: &str) -> Result<Vec<String>, HostError> {
        Ok(self.rows.borrow().get(collection).map(|c| c.keys().cloned().collect()).unwrap_or_default())
    }
    async fn delete(&self, collection: &str, id: &str) -> Result<(), HostError> {
        self.writes.borrow_mut().push(format!("delete {collection}/{id}"));
        if let Some(c) = self.rows.borrow_mut().get_mut(collection) {
            c.shift_remove(id);
        }
        Ok(())
    }
    async fn get_blob(&self, collection: &str, id: &str) -> Result<Option<Vec<u8>>, HostError> {
        Ok(self.blobs.borrow().get(collection).and_then(|c| c.get(id)).cloned())
    }
    async fn put_blob(&self, collection: &str, id: &str, bytes: Vec<u8>) -> Result<(), HostError> {
        self.blobs.borrow_mut().entry(collection.to_owned()).or_default().insert(id.to_owned(), bytes);
        Ok(())
    }
    async fn delete_blob(&self, collection: &str, id: &str) -> Result<(), HostError> {
        if let Some(c) = self.blobs.borrow_mut().get_mut(collection) {
            c.shift_remove(id);
        }
        Ok(())
    }
    async fn stat_blob(&self, _collection: &str, _id: &str) -> Result<Option<BlobMark>, HostError> {
        Ok(None)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Records and projects

/// A workspace record from JSON keys over a minimal base: `{id, name, machineId, phase: running, kind, golden: "",
/// createdAt, project, spec: {}, size: {cpu: 2, memMb: 4096}, firstLife: false}` with `over` written on top.
pub fn record(id: &str, kind: &str, project: &str, over: Value) -> WorkspaceRecord {
    let mut base = json!({
        "id": id, "name": id, "machineId": format!("m_{id}"), "phase": "running", "kind": kind, "golden": "",
        "createdAt": "2026-10-25T00:00:00.000Z", "project": project, "spec": {}, "size": { "cpu": 2, "memMb": 4096 },
        "firstLife": false,
    });
    merge(&mut base, over);
    serde_json::from_value(base).expect("a test record reads")
}

/// A project on `computer` (`here`, `pl_<id>`, or a provider) from JSON keys over a minimal base.
pub fn project(id: &str, name: &str, computer: &str, over: Value) -> ProjectView {
    let mut base = json!({
        "id": id, "name": name, "computer": computer, "source": { "kind": "folder", "path": format!("/home/me/{name}") },
        "path": format!("/home/me/{name}"), "remote": format!("git@github.com:acme/{name}.git"), "defaultBranch": "main",
        "memoryKey": format!("-home-me-{name}"), "memoryDir": format!("/home/me/.claude/projects/-home-me-{name}/memory"),
        "createdAt": "2026-10-01T00:00:00.000Z",
    });
    merge(&mut base, over);
    serde_json::from_value(base).expect("a test project reads")
}

/// Writes `over`'s keys onto `base`; a null in `over` removes the key.
pub fn merge(base: &mut Value, over: Value) {
    if let (Value::Object(b), Value::Object(o)) = (base, over) {
        for (k, v) in o {
            if v.is_null() {
                b.shift_remove(&k);
            } else {
                b.insert(k, v);
            }
        }
    }
}

/// The record of an entry as it stands, cloned out.
pub fn rec(entry: &Entry) -> WorkspaceRecord {
    entry.borrow().record.borrow().clone()
}

/// The record of an entry as JSON, keys in the struct's order.
pub fn rec_json(entry: &Entry) -> Value {
    serde_json::to_value(rec(entry)).unwrap()
}

/// The `Caller` of a thread: road `paired` (a thread's token), scope `{kind: thread, threadId, workspaceId, rootThreadId}`.
pub fn thread(thread_id: &str, workspace_id: &str, root: &str) -> Caller {
    let scope =
        serde_json::from_value(json!({ "kind": "thread", "threadId": thread_id, "workspaceId": workspace_id, "rootThreadId": root }))
            .expect("a scope reads");
    let origin = serde_json::from_value(json!("paired")).expect("an origin reads");
    Caller::Scoped { origin, by: scope }
}

/// The person's `Caller`: road `here`.
pub fn person() -> Caller {
    Caller::Road(serde_json::from_value(json!("here")).expect("an origin reads"))
}

// ---------------------------------------------------------------------------------------------------------------
// The fake

/// One logged call: its name and its arguments as JSON.
#[derive(Clone, Debug, PartialEq)]
pub struct Call {
    pub name: String,
    pub args: Value,
}

/// The base context over shared state, logging every call and answering from the script.
pub struct FakeBase {
    pub clock: FakeClock,
    pub ids: Rc<TestIds>,
    pub store: Rc<MemStore>,
    pub live: Live,
    pub projects: ProjectsHeld,
    pub sessions: RefCell<Vec<SessionView>>,
    pub threads: RefCell<Vec<(String, ThreadRecord)>>,
    /// Every record `persist` was handed, in order.
    pub persisted: RefCell<Vec<WorkspaceRecord>>,
    /// Every event `emit` was handed, in order.
    pub events: RefCell<Vec<Value>>,
    pub calls: RefCell<Vec<Call>>,
    /// Answers by call name, taken in order; the last one stays once the others are taken.
    pub script: RefCell<IndexMap<String, VecDeque<Value>>>,
    /// `kindOf(computer)`: `here` is local, a `pl_` id is a place, anything else cloud, unless set here.
    pub kinds: RefCell<IndexMap<String, WorkspaceKind>>,
    pub home: String,
    pub prefs: RefCell<Option<Preferences>>,
}

impl FakeBase {
    pub fn new() -> Rc<FakeBase> {
        Rc::new(FakeBase {
            clock: FakeClock::new(1_000_000),
            ids: TestIds::new(),
            store: MemStore::new(),
            live: Rc::new(RefCell::new(IndexMap::new())),
            projects: Rc::new(RefCell::new(IndexMap::new())),
            sessions: RefCell::new(vec![]),
            threads: RefCell::new(vec![]),
            persisted: RefCell::new(vec![]),
            events: RefCell::new(vec![]),
            calls: RefCell::new(vec![]),
            script: RefCell::new(IndexMap::new()),
            kinds: RefCell::new(IndexMap::new()),
            home: "/home/me".to_owned(),
            prefs: RefCell::new(None),
        })
    }

    /// Logs a call.
    pub fn call(&self, name: &str, args: Value) {
        self.calls.borrow_mut().push(Call { name: name.to_owned(), args });
    }

    /// Queues answers for a call name.
    pub fn answer_with(&self, name: &str, answers: Vec<Value>) {
        self.script.borrow_mut().entry(name.to_owned()).or_default().extend(answers);
    }

    /// The next scripted answer for a name, the last one repeating; none where nothing is scripted.
    pub fn answer(&self, name: &str) -> Option<Value> {
        let mut script = self.script.borrow_mut();
        let q = script.get_mut(name)?;
        if q.len() > 1 {
            q.pop_front()
        } else {
            q.front().cloned()
        }
    }

    /// The scripted answer as `T`, an `$error` answer as the error, `default` where nothing is scripted.
    pub fn answer_as<T: DeserializeOwned>(&self, name: &str, default: impl FnOnce() -> T) -> Result<T, HostError> {
        match self.answer(name) {
            None => Ok(default()),
            Some(v) => match error_of(&v) {
                Some(e) => Err(e),
                None => Ok(serde_json::from_value(v).unwrap_or_else(|e| panic!("scripted answer for {name} does not read: {e}"))),
            },
        }
    }

    /// The names of every call, in order.
    pub fn names(&self) -> Vec<String> {
        self.calls.borrow().iter().map(|c| c.name.clone()).collect()
    }

    /// The names of the calls among `of`, in order: the effects a test pins, without the reads it does not.
    pub fn names_of(&self, of: &[&str]) -> Vec<String> {
        self.names().into_iter().filter(|n| of.contains(&n.as_str())).collect()
    }

    /// The arguments of every call of a name, in order.
    pub fn args_of(&self, name: &str) -> Vec<Value> {
        self.calls.borrow().iter().filter(|c| c.name == name).map(|c| c.args.clone()).collect()
    }

    /// Every daemon op asked, in order (the `op` frames handed to an `ask`).
    pub fn asks(&self) -> Vec<Value> {
        self.args_of("ask")
    }

    /// Holds a project.
    pub fn hold_project(&self, p: ProjectView) {
        self.projects.borrow_mut().insert(p.id.clone(), p);
    }

    /// Holds a live entry over a record.
    pub fn hold(&self, r: WorkspaceRecord) -> Entry {
        let id = r.id.clone();
        let entry = entry_over(r, FakeMachine::new(&id));
        self.live.borrow_mut().insert(id, entry.clone());
        entry
    }

    /// A daemon ask that logs each op and answers from the `ask` script (or `ask:<op>` first).
    pub fn ask_fn(self: &Rc<Self>) -> DaemonAsk {
        let me = self.clone();
        Rc::new(move |frame: Value| {
            let me = me.clone();
            async move {
                me.call("ask", frame.clone());
                let op = frame.get("op").and_then(Value::as_str).unwrap_or("").to_owned();
                let scripted = me.answer(&format!("ask:{op}")).or_else(|| me.answer("ask")).unwrap_or(json!({}));
                match error_of(&scripted) {
                    Some(e) => Err(e),
                    None => Ok(scripted),
                }
            }
            .boxed_local()
        })
    }
}

/// `FakeBase` as the trait object, so a unit fake can hand out `Rc<FakeBase>` methods.
pub struct BaseOf(pub Rc<FakeBase>);

#[async_trait(?Send)]
impl Base for BaseOf {
    async fn ready(&self) -> Result<(), HostError> {
        self.0.call("ready", json!(null));
        self.0.answer_as("ready", || ())
    }
    fn clock(&self) -> Rc<dyn Clock> {
        self.0.clock.clock()
    }
    fn ids(&self) -> Rc<dyn Ids> {
        self.0.ids.clone()
    }
    fn store(&self) -> Rc<dyn Store> {
        self.0.store.clone()
    }
    fn live(&self) -> Live {
        self.0.live.clone()
    }
    fn projects_held(&self) -> ProjectsHeld {
        self.0.projects.clone()
    }
    fn project_held(&self, project_id: &str) -> Result<ProjectView, HostError> {
        self.0.projects.borrow().get(project_id).cloned().ok_or_else(|| plain(&format!("no project {project_id}")))
    }
    fn session_views(&self) -> Vec<SessionView> {
        self.0.sessions.borrow().clone()
    }
    fn thread_records(&self) -> Vec<(String, ThreadRecord)> {
        self.0.threads.borrow().clone()
    }
    async fn persist(&self, record: WorkspaceRecord) -> Result<(), HostError> {
        self.0.call("persist", json!({ "id": record.id }));
        self.0.persisted.borrow_mut().push(record);
        self.0.answer_as("persist", || ())
    }
    fn view(&self, record: &WorkspaceRecord) -> WorkspaceView {
        view_of(&self.0, record)
    }
    fn emit(&self, event: Event) -> Result<(), HostError> {
        self.0.call("emit", json!({ "type": event.get("type").cloned().unwrap_or(Value::Null) }));
        self.0.events.borrow_mut().push(event);
        Ok(())
    }
    async fn entry_of(&self, id: &str, origin: Option<Caller>, ask: EntryAsk) -> Result<Entry, HostError> {
        self.0.call(
            "entry_of",
            json!({ "id": id, "thread": ask.thread, "act": ask.act, "scoped": matches!(origin, Some(Caller::Scoped { .. })) }),
        );
        if let Some(v) = self.0.answer("entry_of") {
            if let Some(e) = error_of(&v) {
                return Err(e);
            }
        }
        self.0.live.borrow().get(id).cloned().ok_or_else(|| kinded(&format!("no workspace {id}"), Kind::NotFound))
    }
    fn spawn_guard(&self, act: &str, origin: Option<&Caller>) -> Result<FreeSlot, HostError> {
        self.0.call("spawn_guard", json!({ "act": act, "scoped": matches!(origin, Some(Caller::Scoped { .. })) }));
        self.0.answer_as("spawn_guard", || ())?;
        let me = self.0.clone();
        Ok(Rc::new(move || me.call("free_slot", json!(null))))
    }
    fn kind_of(&self, computer: &str) -> WorkspaceKind {
        if let Some(k) = self.0.kinds.borrow().get(computer) {
            return *k;
        }
        if computer == "here" {
            WorkspaceKind::Local
        } else if computer.starts_with("pl_") {
            WorkspaceKind::Place
        } else {
            WorkspaceKind::Cloud
        }
    }
    fn place_name(&self, place_id: &str) -> String {
        match place_id {
            "here" => "Mac".to_owned(),
            other => other.trim_start_matches("pl_").to_owned(),
        }
    }
    async fn copy_blocked(&self, entry: &Entry) -> Result<(), HostError> {
        self.0.call("copy_blocked", json!({ "id": rec(entry).id }));
        self.0.answer_as("copy_blocked", || ())
    }
    async fn with_daemon(&self, entry: &Entry, work: DaemonWork) -> Result<Value, HostError> {
        self.0.call("with_daemon", json!({ "id": rec(entry).id }));
        work(self.0.ask_fn()).await
    }
    async fn on_this_computer(&self, work: HereWork) -> Result<Value, HostError> {
        self.0.call("on_this_computer", json!(null));
        work(self.0.ask_fn(), self.0.home.clone()).await
    }
    async fn git_here(&self, ask: GitAsk) -> Result<ExecResult, HostError> {
        self.0.call("git", json!({ "cwd": ask.cwd, "args": ask.args, "timeoutMs": ask.timeout_ms, "input": ask.input }));
        let key = format!("git:{}", ask.args.first().cloned().unwrap_or_default());
        let scripted =
            self.0.answer(&format!("git:{}", ask.args.join(" "))).or_else(|| self.0.answer(&key)).or_else(|| self.0.answer("git"));
        match scripted {
            None => Ok(ExecResult { exit_code: 0.into(), stdout: String::new(), stderr: String::new() }),
            Some(v) => match error_of(&v) {
                Some(e) => Err(e),
                None => Ok(serde_json::from_value(v).expect("a scripted git result reads")),
            },
        }
    }
    fn checkout_of(&self, record: &WorkspaceRecord) -> String {
        record.worktree.as_ref().map(|w| w.path.clone()).unwrap_or_else(|| format!("/checkouts/{}", record.id))
    }
    async fn preferences(&self) -> Result<Preferences, HostError> {
        self.0.call("preferences", json!(null));
        Ok(self.0.prefs.borrow().clone().expect("the test set preferences"))
    }
    async fn write_daemon_roots(&self, entry: &Entry, strict: bool) -> Result<(), HostError> {
        self.0.call("write_daemon_roots", json!({ "id": rec(entry).id, "strict": strict }));
        self.0.answer_as("write_daemon_roots", || ())
    }
    fn home_dir(&self) -> String {
        self.0.home.clone()
    }
    fn machine_id(&self, entry: &Entry) -> String {
        rec(entry).machine_id
    }
}

/// The fake's view of a record: the record's keys less those the view lacks, with the project as a ref.
pub fn view_of(base: &FakeBase, record: &WorkspaceRecord) -> WorkspaceView {
    let mut v = serde_json::to_value(record).unwrap();
    let project = base.projects.borrow().get(&record.project).cloned();
    let pref = match project {
        Some(p) => json!({ "id": p.id, "name": p.name, "path": p.path, "computer": p.computer }),
        None => json!({ "id": record.project, "name": record.project, "path": "", "computer": "here" }),
    };
    if let Value::Object(m) = &mut v {
        let keep: Vec<String> = m.keys().cloned().collect();
        for k in keep {
            if [
                "spec",
                "idleWindowMs",
                "size",
                "firstLife",
                "shape",
                "npmBin",
                "base",
                "nappedAt",
                "daemon",
                "pr",
                "branchThreads",
                "checkout",
                "tree",
                "reviewThread",
                "prSent",
                "landed",
                "lead",
                "sub",
            ]
            .contains(&k.as_str())
            {
                m.shift_remove(&k);
            }
        }
        m.insert("project".to_owned(), pref);
    }
    serde_json::from_value(v).expect("a test view reads")
}

/// Runs a future to its end on a current-thread runtime with a local set, as the host runs.
pub fn run<F: std::future::Future>(f: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, f)
}

/// A boxed local future answering a value, for `Queued` work in tests.
pub fn ready_value(v: Value) -> LocalBoxFuture<'static, Result<Value, HostError>> {
    async move { Ok(v) }.boxed_local()
}
