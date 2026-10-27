// SPDX-License-Identifier: AGPL-3.0-only
//! The whole-runtime fixture (#2476 join B): a runtime built by `create_runtime` over a stub backend with scripted
//! guests and fake harness adapters, on the sqlite store in a temp folder. It holds the TS fixture's behaviour
//! (packages/runtime/test/stub-backend.ts, runtime-fixture.ts, held-port.ts, fake-clock.ts, until.ts at 65f6b89c7),
//! not a new design. Each test file declares `mod common;`.
#![allow(dead_code, unused_imports, unused_macros)]

/// `await vi.waitFor(() => expect(...))` / `until(cond)`: polls `$body` with the named Rc handles cloned in, for 2 s.
macro_rules! wait {
    ($label:literal, [$($v:ident),* $(,)?] $body:expr) => {{
        $crate::common::until_named($label, || {
            $(let $v = $v.clone();)*
            async move { $body }
        })
        .await
    }};
    ([$($v:ident),* $(,)?] $body:expr) => {{
        $crate::common::until(|| {
            $(let $v = $v.clone();)*
            async move { $body }
        })
        .await
    }};
}

pub mod adapter;
pub mod comeback;
pub mod daemon;
pub mod driven;
pub mod events;
pub mod gateway;
pub mod gone;
pub mod images;
pub mod lagging;
pub mod local;
pub mod machine_runs;
pub mod ports;
pub mod projects;
pub mod scripted;
pub mod store_wrap;
pub mod stub_backend;
pub mod vault;
pub mod ws_client;
pub mod ws_daemon;

use std::future::Future;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use indexmap::IndexMap;
use wsp_core::clock::FakeClock;
use wsp_core::types::harness::HarnessAdapterFactory;
use wsp_core::types::wiring::RuntimeOptions;
use wsp_engine::machine::MachineBackend;
use wsp_serve::{create_runtime, AssembledRuntime};
use wsp_store::sqlite_store::sqlite_store;
use wsp_store::store::{memory_store, StateWriter, Store};

pub use adapter::{agent, never_ends, AgentOptions, Asked, ForkAt, Held, Road};
pub use driven::{DTurn, Driven};
pub use events::{json, Events};
pub use lagging::Lagging;
pub use machine_runs::{Holder, MachineRuns, Reopened};
pub use ports::{dropping_port, Edge, HeldPort};
pub use projects::{create_on, owned_store, project_on, temp_repo, CreateOn};
pub use scripted::{Scripted, When};
pub use store_wrap::{CountingStore, WrappedStore};
pub use stub_backend::{
    conflict, exec_async, exec_exit, exec_fn, exec_ok, fetch_failed, missing, token_guest, token_guest_answer, StubBackend, StubMachine,
    GUEST_BRANCH,
};
pub use ws_client::{create_over_wire, WsClient};
pub use ws_daemon::{daemon_of, helloing_daemon, HelloingDaemon};
pub use wsp_core::types::api::Runtime as _;
pub use wsp_engine::machine::{Machine, MachineBackend as _};

/// `Date.now()`: the wall clock a stub stamps its machines with.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

/// How long one test may run before it is a hang and not a slow test.
const HANG_LIMIT: Duration = Duration::from_secs(120);

/// Runs `f` on a current-thread runtime inside a `LocalSet`, as the host runs (chapter 03, one thread). A test that
/// outlives `HANG_LIMIT` fails with its own words, so one hang never holds the whole file.
pub fn drive<F: Future<Output = ()>>(f: impl FnOnce() -> F) {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a tokio runtime");
    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, async move {
        if tokio::time::timeout(HANG_LIMIT, f()).await.is_err() {
            panic!("the test did not finish in {} s: a hang in a body, not a slow test", HANG_LIMIT.as_secs());
        }
    });
}

/// `until(cond, ms)` (until.ts): polls `cond` every 10 ms until it holds; panics once the deadline passes.
pub async fn until<Fut: Future<Output = bool>>(mut cond: impl FnMut() -> Fut) {
    until_for(2_000, &mut cond).await;
}

/// `until` that says which wait it was when it gives up.
pub async fn until_named<Fut: Future<Output = bool>>(label: &str, mut cond: impl FnMut() -> Fut) {
    let deadline = Instant::now() + Duration::from_millis(2_000);
    while !cond().await {
        assert!(Instant::now() <= deadline, "condition not met in time: {label}");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

pub async fn until_for<Fut: Future<Output = bool>>(ms: u64, cond: &mut impl FnMut() -> Fut) {
    let deadline = Instant::now() + Duration::from_millis(ms);
    while !cond().await {
        assert!(Instant::now() <= deadline, "condition not met in time");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// `settle()` of runtime-fixture.ts:158.
pub async fn settle() {
    tokio::time::sleep(Duration::from_millis(20)).await;
}

/// `await new Promise(r => setTimeout(r, ms))`.
pub async fn sleep_ms(ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

/// The state file's writer stamp: any value the store accepts.
fn writer() -> StateWriter {
    StateWriter { wsp: "test".into(), daemon: 1.into(), bin: "/usr/local/bin/wsp".into() }
}

/// A state in a temp folder on the SQLite store, which lives as long as the folder does.
pub struct State {
    pub store: Rc<dyn Store>,
    pub dir: Rc<tempfile::TempDir>,
}

/// A fresh sqlite store in a folder of its own.
pub fn sqlite_state() -> State {
    let dir = Rc::new(tempfile::tempdir().expect("a folder"));
    let store: Rc<dyn Store> = Rc::new(sqlite_store(&dir.path().join("state.json"), writer()));
    State { store, dir }
}

/// The memory store, for the one test that wants the TS's own `memoryStore()`.
pub fn memory_state() -> Rc<dyn Store> {
    Rc::new(memory_store())
}

type Extra = Box<dyn FnOnce(&mut RuntimeOptions)>;

/// `createRuntime({ backend, store, adapters, ...extra })`: the wiring a test names, built when it says.
pub struct Wiring {
    backend: Arc<dyn MachineBackend>,
    store: Option<Rc<dyn Store>>,
    clock: Option<FakeClock>,
    adapters: IndexMap<String, HarnessAdapterFactory>,
    extra: Vec<Extra>,
}

/// The runtime a test wires over `backend`; call `.build()` for it. Every `createRuntime` of a TS test over the
/// same backend is one `runtime(&backend)` here.
pub fn runtime(backend: &Arc<StubBackend>) -> Wiring {
    over(backend.clone())
}

/// As `runtime`, over a backend that is not the stub (`NoProviderBackend`, a real one over a fake gateway).
pub fn over(backend: Arc<dyn MachineBackend>) -> Wiring {
    Wiring { backend, store: None, clock: None, adapters: IndexMap::new(), extra: vec![] }
}

impl Wiring {
    pub fn store(mut self, store: &Rc<dyn Store>) -> Self {
        self.store = Some(store.clone());
        self
    }
    pub fn clock(mut self, clock: &FakeClock) -> Self {
        self.clock = Some(clock.clone());
        self
    }
    pub fn adapter(mut self, name: &str, factory: HarnessAdapterFactory) -> Self {
        self.adapters.insert(name.into(), factory);
        self
    }
    /// What the TS passes beside `backend`, `store` and `adapters`: `hostId`, `daemonToken`, `status`, `wake`...
    pub fn with(mut self, change: impl FnOnce(&mut RuntimeOptions) + 'static) -> Self {
        self.extra.push(Box::new(change));
        self
    }

    /// Builds the runtime. With no store named, a sqlite store in a temp folder the runtime keeps alive.
    pub fn build(self) -> Fx {
        let state = self.store.is_none().then(sqlite_state);
        let store = self.store.unwrap_or_else(|| state.as_ref().expect("a state").store.clone());
        let mut opts = RuntimeOptions {
            backend: Some(self.backend.clone()),
            store: Some(store.clone()),
            adapters: self.adapters,
            clock: self.clock.as_ref().map(FakeClock::clock),
            ..Default::default()
        };
        for change in self.extra {
            change(&mut opts);
        }
        let rt = create_runtime(opts).unwrap_or_else(|e| panic!("create_runtime refused: {}", e.message));
        Fx { rt, store, _dir: state.map(|s| s.dir) }
    }
}

/// A built runtime and the state it stands on.
pub struct Fx {
    pub rt: Rc<AssembledRuntime>,
    pub store: Rc<dyn Store>,
    _dir: Option<Rc<tempfile::TempDir>>,
}

impl Fx {
    /// `rt.events.on("*", ...)`: everything the runtime says from now on.
    pub fn events(&self) -> Events {
        use wsp_core::types::api::Runtime as _;
        Events::on(&*self.rt.events())
    }
}

/// `rt.status.list()`: the rows of every workspace, as the JSON the wire carries.
pub async fn status_rows(rt: &AssembledRuntime) -> Vec<serde_json::Value> {
    use wsp_core::types::api::Runtime as _;
    use wsp_core::types::wiring::OriginStatusApi;
    let rows = OriginStatusApi::list(&*rt.status(), None, None).await.unwrap_or_else(|e| panic!("status.list failed: {}", e.message));
    rows.iter().map(|r| serde_json::to_value(r).expect("a status row")).collect()
}

/// A value as the JSON the wire carries, for the assertions the TS writes with `toMatchObject` and `toEqual`.
pub fn j<T: serde::Serialize>(value: &T) -> serde_json::Value {
    serde_json::to_value(value).expect("a value as JSON")
}

/// vitest's `toMatchObject`: every key of `want` is in `have` with a value that matches, arrays by position and length.
pub fn matches(have: &serde_json::Value, want: &serde_json::Value) -> bool {
    use serde_json::Value;
    match (have, want) {
        (Value::Object(h), Value::Object(w)) => w.iter().all(|(k, v)| h.get(k).is_some_and(|x| matches(x, v))),
        (Value::Array(h), Value::Array(w)) => h.len() == w.len() && h.iter().zip(w).all(|(x, v)| matches(x, v)),
        _ => have == want,
    }
}

/// `expect(have).toMatchObject(want)`.
pub fn assert_matches(have: &serde_json::Value, want: serde_json::Value) {
    assert!(matches(have, &want), "expected a value matching\n{want:#}\nbut got\n{have:#}");
}

/// A regular expression of the shape `a.*b.*c`: every part in `parts` is in `text`, in that order.
pub fn in_order(text: &str, parts: &[&str]) -> bool {
    let mut at = 0;
    for part in parts {
        match text[at..].find(part) {
            Some(found) => at += found + part.len(),
            None => return false,
        }
    }
    true
}

/// A promise a test settles by hand: `kk.hold = new Promise(r => release = r)`. Waiting on an opened gate returns at once.
#[derive(Default)]
pub struct Gate {
    open: std::cell::Cell<bool>,
    wake: tokio::sync::Notify,
}

impl Gate {
    pub fn new() -> Rc<Gate> {
        Rc::new(Gate::default())
    }

    pub fn open(&self) {
        self.open.set(true);
        self.wake.notify_waiters();
    }

    pub async fn wait(&self) {
        while !self.open.get() {
            let woken = self.wake.notified();
            tokio::pin!(woken);
            // Enabled before the flag is read again, so an `open` between the two is never missed.
            woken.as_mut().enable();
            if self.open.get() {
                return;
            }
            woken.await;
        }
    }
}

/// place-join.ts:16 `HERE` and `wiring(hostKey)`: the wiring a host holds for its places.
pub fn place_wiring(host_key: wsp_store::keys::PlaceKeyPair) -> Rc<dyn wsp_core::types::pending::PlaceWiring> {
    use wsp_machines::places::types::{HerePlace, PlaceWiring};
    use wsp_wire::views::place_more::MacKind;
    use wsp_wire::wire::capabilities::WorkspaceSize;
    Rc::new(PlaceWiring {
        host_key,
        provider: Some(Rc::new(|| None)),
        here: Rc::new(|| HerePlace {
            name: "zingzys-mac".into(),
            os: Some("macOS 15.0".into()),
            shape: Some(WorkspaceSize { cpu: wsp_wire::types::JsNumber(8.0), mem_mb: wsp_wire::types::JsNumber(16384.0) }),
            mac: Some(MacKind::MacMini),
            home: Some("/home/dev".into()),
            ..Default::default()
        }),
        host_name: Rc::new(|| "zingzys-mac".to_string()),
        install: None,
        dial: None,
        log: None,
        update: None,
        leave: None,
        forget_key: None,
        sudo_over: None,
        provision: None,
        github_token: None,
        undo: None,
        run_over: None,
        back: None,
    })
}

/// lost-create.test.ts:166 `PlaceBackends`: a table of places by name over the backends a test hands in.
pub struct Places {
    pub wired: String,
    pub backends: Vec<(String, Arc<dyn MachineBackend>)>,
    pub list: Vec<String>,
}

impl wsp_core::types::wiring::PlaceBackends for Places {
    fn wired(&self) -> String {
        self.wired.clone()
    }
    fn backend(&self, place: &str) -> Option<Arc<dyn MachineBackend>> {
        self.backends.iter().find(|(name, _)| name == place).map(|(_, b)| b.clone())
    }
    fn list(&self) -> Vec<String> {
        self.list.clone()
    }
}
