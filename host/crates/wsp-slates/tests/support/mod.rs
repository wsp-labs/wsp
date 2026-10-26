// SPDX-License-Identifier: AGPL-3.0-only
//! What every wsp-slates test builds on: wsp-core's clocks, wsp-store's in-memory store, a fake runtime for the slate
//! area, and the process-group guard. `ManualClock` and `MemStore` wrap the real ones for what each lacks: the delays
//! a clock was asked for, and a store's refused `list` and its count.
#![allow(dead_code)]

pub mod entry;

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use futures::FutureExt;
use indexmap::IndexMap;
use serde_json::Value;
use wsp_core::clock::{real_clock, Cancel, Clock, FakeClock, Timer};
use wsp_slates::slate_sources::context::{AccountRow, SessionView, SlateWorkspaceFacts, TurnResult};
use wsp_slates::slate_sources::SlateSourceContext;
use wsp_slates::slates::{create_slates, Delivered, Delivery, DeliveryOutcome, SlateEmit, SlateThreadFacts, Slates, SlatesDeps};
use wsp_store::store::{memory_store, BlobMark, MemoryStore, Store};
use wsp_wire::types::slate_wire::SessionSlateEvent;
use wsp_wire::views::workspace::{Caller, ThreadLit};
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin};
use wsp_words::exit::HostError;

/// Puts this test process in a process group of its own, so a test that kills groups can never take the runner
/// down (#2448 How 4). Idempotent.
pub fn own_group() {
    unsafe {
        libc::setpgid(0, 0);
    }
}

/// Runs a test body on a current-thread runtime inside a `LocalSet`, as the host runs (chapter 03).
pub fn local<F: Future<Output = ()>>(f: F) {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let set = tokio::task::LocalSet::new();
    set.block_on(&rt, f);
}

/// Sleeps on the tokio clock.
pub async fn sleep_ms(ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

/// Polls `check` every 20 ms until it holds or `ms` pass; answers whether it held.
pub async fn until(ms: u64, mut check: impl FnMut() -> bool) -> bool {
    let end = std::time::Instant::now() + Duration::from_millis(ms);
    loop {
        if check() {
            return true;
        }
        if std::time::Instant::now() >= end {
            return check();
        }
        sleep_ms(20).await;
    }
}

/// wsp-core's process clock: system time, timers on the current `LocalSet`.
pub fn test_clock() -> Rc<dyn Clock> {
    real_clock()
}

/// wsp-core's `FakeClock` that also keeps every delay it was asked for, as given.
#[derive(Clone)]
pub struct ManualClock {
    inner: FakeClock,
    pub asked: Rc<RefCell<Vec<i64>>>,
}

impl ManualClock {
    pub fn new(start: i64) -> ManualClock {
        ManualClock { inner: FakeClock::new(start), asked: Rc::new(RefCell::new(Vec::new())) }
    }
    pub fn clock(&self) -> Rc<dyn Clock> {
        Rc::new(self.clone())
    }
    /// Runs every timer due by `now + ms`, earliest first, ties in scheduling order, with `now` at its own time.
    pub fn advance(&self, ms: i64) {
        self.inner.advance(ms);
    }
    pub fn pending(&self) -> usize {
        self.inner.pending()
    }
}

impl Clock for ManualClock {
    fn now(&self) -> i64 {
        self.inner.now()
    }
    fn schedule(&self, f: Timer, ms: i64) -> Cancel {
        self.asked.borrow_mut().push(ms);
        self.inner.schedule(f, ms)
    }
}

/// wsp-store's in-memory store, with a `list` a test can refuse and a count of the `list` calls.
pub struct MemStore {
    inner: MemoryStore,
    /// When set, `list` fails with this message.
    pub list_fails: RefCell<Option<String>>,
    pub lists: Cell<usize>,
}

impl MemStore {
    pub fn new() -> Rc<MemStore> {
        Rc::new(MemStore { inner: memory_store(), list_fails: RefCell::new(None), lists: Cell::new(0) })
    }
    pub fn row(&self, collection: &str, id: &str) -> Option<Value> {
        futures::executor::block_on(self.inner.get(collection, id)).unwrap()
    }
    /// A row written before the host starts.
    pub fn seed(&self, collection: &str, id: &str, value: Value) {
        futures::executor::block_on(self.inner.put(collection, id, value)).unwrap();
    }
}

#[async_trait(?Send)]
impl Store for MemStore {
    async fn get(&self, collection: &str, id: &str) -> Result<Option<Value>, HostError> {
        self.inner.get(collection, id).await
    }
    async fn put(&self, collection: &str, id: &str, value: Value) -> Result<(), HostError> {
        self.inner.put(collection, id, value).await
    }
    async fn list(&self, collection: &str) -> Result<Vec<Value>, HostError> {
        self.lists.set(self.lists.get() + 1);
        if let Some(m) = self.list_fails.borrow().clone() {
            return Err(HostError::new(m));
        }
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
        self.inner.put_blob(collection, id, bytes).await
    }
    async fn delete_blob(&self, collection: &str, id: &str) -> Result<(), HostError> {
        self.inner.delete_blob(collection, id).await
    }
    async fn stat_blob(&self, collection: &str, id: &str) -> Result<Option<BlobMark>, HostError> {
        self.inner.stat_blob(collection, id).await
    }
}

/// A source context with nothing in it.
pub struct NoSources;

impl SlateSourceContext for NoSources {
    fn thread_id(&self) -> &str {
        THREAD
    }
    fn workspace_id(&self) -> &str {
        WORKSPACE
    }
    fn now(&self) -> i64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
    }
    fn rows(&self) -> Vec<SessionView> {
        Vec::new()
    }
    fn results(&self) -> Vec<TurnResult> {
        Vec::new()
    }
    fn account(&self) -> LocalBoxFuture<'_, Option<AccountRow>> {
        async { None }.boxed_local()
    }
    fn workspace(&self) -> Option<SlateWorkspaceFacts> {
        None
    }
}

/// The first thread's id; its word is `11111111`.
pub const THREAD: &str = "11111111-aaaa-4bbb-8ccc-000000000001";
pub const WORKSPACE: &str = "ws_0000aaaa";
pub const SESSION: &str = "55555555-5555-4555-8555-000000000001";

/// A fake runtime for the slate area over a temp root: one finished thread on a plain folder, `root/plain`.
pub struct FakeHost {
    pub root: PathBuf,
    pub store: Rc<MemStore>,
    pub clock: Rc<dyn Clock>,
    pub threads: RefCell<IndexMap<String, SlateThreadFacts>>,
    pub under: RefCell<HashMap<String, Vec<String>>>,
    pub tokens: RefCell<HashMap<String, String>>,
    pub recorded: RefCell<Vec<SessionSlateEvent>>,
    pub emitted: RefCell<Vec<SlateEmit>>,
    pub delivered: RefCell<Vec<Delivery>>,
    pub deliver_answer: RefCell<Result<Delivered, HostError>>,
    pub settled_threads: RefCell<HashSet<String>>,
    pub watched: RefCell<Vec<String>>,
    pub env: RefCell<IndexMap<String, String>>,
}

impl FakeHost {
    /// A host over `root` with `THREAD` on `root/plain`, made if missing.
    pub fn new(root: &Path, store: Rc<MemStore>) -> Rc<FakeHost> {
        Self::with_clock(root, store, test_clock())
    }

    pub fn with_clock(root: &Path, store: Rc<MemStore>, clock: Rc<dyn Clock>) -> Rc<FakeHost> {
        let folder = root.join("plain");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::create_dir_all(root.join("state")).unwrap();
        let host = FakeHost {
            root: root.to_path_buf(),
            store,
            clock,
            threads: RefCell::new(IndexMap::new()),
            under: RefCell::new(HashMap::new()),
            tokens: RefCell::new(HashMap::new()),
            recorded: RefCell::new(Vec::new()),
            emitted: RefCell::new(Vec::new()),
            delivered: RefCell::new(Vec::new()),
            deliver_answer: RefCell::new(Ok(Delivered { outcome: DeliveryOutcome::Started, turn_id: Some("turn-2".into()) })),
            settled_threads: RefCell::new(HashSet::new()),
            watched: RefCell::new(Vec::new()),
            env: RefCell::new(IndexMap::from([
                ("PATH".to_string(), std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into())),
                ("HOME".to_string(), root.display().to_string()),
            ])),
        };
        host.add_thread(THREAD, THREAD, Some(&folder.display().to_string()));
        Rc::new(host)
    }

    /// Another thread of the same workspace; `root` is its lead.
    pub fn add_thread(&self, thread_id: &str, root: &str, folder: Option<&str>) {
        self.threads.borrow_mut().insert(
            thread_id.to_string(),
            SlateThreadFacts {
                workspace_id: WORKSPACE.into(),
                root_thread_id: root.into(),
                session_id: SESSION.into(),
                turn_id: None,
                folder: folder.map(String::from),
                host_folder: None,
                computer: None,
                tool_asks: None,
            },
        );
        self.under.borrow_mut().entry(root.to_string()).or_default().push(thread_id.to_string());
    }

    pub fn folder(&self) -> PathBuf {
        self.root.join("plain")
    }

    pub fn slates_dir(&self) -> PathBuf {
        self.root.join("state").join("slates")
    }

    /// The area over this host.
    pub fn slates(self: &Rc<Self>) -> Rc<dyn Slates> {
        create_slates(self.clone())
    }

    /// A second host over the same root and store, as after a restart.
    pub fn again(&self) -> Rc<FakeHost> {
        FakeHost::with_clock(&self.root, self.store.clone(), self.clock.clone())
    }

    /// The `slate.values` pushes, in order.
    pub fn pushes(&self) -> Vec<wsp_wire::types::slate_wire::SlateValuesEvent> {
        self.emitted.borrow().iter().filter_map(|e| if let SlateEmit::Values(v) = e { Some(v.clone()) } else { None }).collect()
    }
}

#[async_trait(?Send)]
impl SlatesDeps for FakeHost {
    fn store(&self) -> Rc<dyn Store> {
        self.store.clone()
    }
    fn clock(&self) -> Rc<dyn Clock> {
        self.clock.clone()
    }
    fn record(&self, event: SessionSlateEvent) {
        self.recorded.borrow_mut().push(event);
    }
    fn emit(&self, event: SlateEmit) {
        self.emitted.borrow_mut().push(event);
    }
    fn thread(&self, thread_id: &str) -> Option<SlateThreadFacts> {
        self.threads.borrow().get(thread_id).cloned()
    }
    async fn settled(&self, thread_id: &str) -> Option<bool> {
        Some(self.settled_threads.borrow().contains(thread_id))
    }
    fn under(&self, lead: &str) -> Vec<String> {
        self.under.borrow().get(lead).cloned().unwrap_or_default()
    }
    fn thread_of_token(&self, token: &str) -> Result<String, HostError> {
        self.tokens.borrow().get(token).cloned().ok_or_else(|| HostError::new(format!("no turn carries the token {token}")))
    }
    fn sources(&self, _thread_id: &str, _workspace_id: &str) -> Rc<dyn SlateSourceContext> {
        Rc::new(NoSources)
    }
    async fn deliver(&self, o: Delivery) -> Result<Delivered, HostError> {
        self.delivered.borrow_mut().push(o);
        self.deliver_answer.borrow().clone()
    }
    fn watch_pr(&self, workspace_id: &str) {
        self.watched.borrow_mut().push(workspace_id.to_string());
    }
    fn run_env(&self) -> IndexMap<String, String> {
        self.env.borrow().clone()
    }
    fn secrets_file(&self) -> Option<PathBuf> {
        Some(self.root.join("state").join("slates.secrets.json"))
    }
    fn slates_dir(&self) -> Option<PathBuf> {
        Some(self.slates_dir())
    }
}

/// The thread's own token, as the agent calls (`origin: "here"`, scoped to the thread).
pub fn as_thread(thread_id: &str, root: &str) -> Caller {
    Caller::Scoped {
        origin: WorkspaceOrigin::Here,
        by: ThreadScope { kind: ThreadLit, thread_id: thread_id.into(), workspace_id: WORKSPACE.into(), root_thread_id: root.into() },
    }
}

/// A window on this computer.
pub fn here() -> Caller {
    Caller::Road(WorkspaceOrigin::Here)
}

/// A computer the person paired.
pub fn paired() -> Caller {
    Caller::Road(WorkspaceOrigin::Paired)
}

/// A fresh temp root, realpath'd as the TS tests do.
pub fn temp_root(prefix: &str) -> tempfile::TempDir {
    let dir = tempfile::Builder::new().prefix(prefix).tempdir().unwrap();
    dir
}
