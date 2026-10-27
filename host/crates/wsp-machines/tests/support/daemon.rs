// SPDX-License-Identifier: AGPL-3.0-only
//! The fakes of the daemon oracle: a runtime and a kind module that log every call in the oracle's shape, and the
//! `World` a scene's `world` input builds them into. Included with `#[path = "support/daemon.rs"] mod support;`.
//! Where a Rust wire type is shaped differently from the oracle's loose object, the translation is named at its site.
#![allow(dead_code)]

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use async_trait::async_trait;
use futures::channel::oneshot;
use futures::FutureExt;
use indexmap::IndexMap;
use serde_json::{json, Value};

use wsp_core::ids::Ids;
use wsp_core::types::internal::SessionEntry;
use wsp_core::types::internal::{
    EndThread, FolderAdmit, ImportLanded, ImportReport, KindModule, PortRoute, ServerValuesRoad,
};
use wsp_core::types::pending::{Clock, MachineExecOptions, ScopedRoad, TurnWaiting};
use wsp_core::types::wiring::{
    LiveWorkspace, Local, ProjectImportOptions, StageReport, WorkspaceRecord,
};
use wsp_engine::errors::Result as EngineResult;
use wsp_engine::lifecycle::{Workspace, WorkspaceHooks, WorkspaceInitial};
use wsp_engine::machine::{
    BackendPricing, Capabilities, ExecOptions, ExecResult, Machine, MachineBackend, MachineKind,
    MachineLife, MachineListRow, MachineOffers, MachineSpec, MachineState, RunOptions, Size,
    SnapshotOptions, SnapshotStoragePricing, Waiting,
};
use wsp_engine::Cancel;
use wsp_machines::machines::daemon::{
    DaemonArea, DaemonRuntime, DetachedOutcome, DetachedSend, DetachedStart, Started,
};
use wsp_machines::machines::machines::{Entry, Row};
use wsp_wire::views::harness::McpServerSpec;
use wsp_wire::views::session::{SessionView, ThreadCapWait};
use wsp_wire::views::workspace::{Caller, ProjectView, ReachState};
use wsp_wire::views::workspace_more::WorkspaceStatus;
use wsp_wire::wire::capabilities::MachineSizeOffer;
use wsp_wire::wire::events_union::EventUnion;
use wsp_wire::wire::runtime::SessionStartOutcome;
use wsp_words::exit::{HostError, Kind};

use crate::common::{self, FakeClock, FakeIds, Log, T0};

/// What a record the fixture wrote leaves out, filled so the record parses; `record_json` takes these back out.
const FILL: &str = "~fill~";

pub fn err(message: &str) -> HostError {
    HostError::new(message)
}

pub fn err_kind(message: &str, kind: Kind) -> HostError {
    HostError::with_kind(message, kind)
}

pub fn marked(message: &str, mark: &str) -> HostError {
    HostError {
        marks: vec![mark.to_string()],
        ..err(message)
    }
}

/// Floats that are whole numbers become integers, so `2.0` and `2` compare equal as JavaScript's numbers do.
pub fn norm(v: &Value) -> Value {
    match v {
        Value::Number(n) => match n.as_f64() {
            Some(f) if f.fract() == 0.0 && f.abs() < 9.0e15 => json!(f as i64),
            _ => v.clone(),
        },
        Value::Array(a) => Value::Array(a.iter().map(norm).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), norm(x))).collect()),
        other => other.clone(),
    }
}

pub fn same(a: &Value, b: &Value) -> bool {
    norm(a) == norm(b)
}

/// The fixture's record, with the keys it leaves out filled with markers.
pub fn record_of(v: &Value) -> WorkspaceRecord {
    let mut m = v.as_object().expect("a record is an object").clone();
    for key in ["name", "golden", "createdAt"] {
        m.entry(key.to_string()).or_insert_with(|| json!(FILL));
    }
    m.entry("spec".to_string()).or_insert_with(|| json!({}));
    m.entry("size".to_string())
        .or_insert_with(|| json!({"cpu": -1, "memMb": -1}));
    m.entry("firstLife".to_string())
        .or_insert_with(|| json!(false));
    serde_json::from_value(Value::Object(m)).expect("the record parses")
}

/// A record as the fixture writes it: the filled keys taken back out.
pub fn record_json(r: &WorkspaceRecord) -> Value {
    let mut v = serde_json::to_value(r).expect("a record serializes");
    let m = v.as_object_mut().expect("a record is an object");
    for key in ["name", "golden", "createdAt"] {
        if m.get(key) == Some(&json!(FILL)) {
            m.remove(key);
        }
    }
    if m.get("spec").map(|s| s == &json!({})).unwrap_or(false) {
        m.remove("spec");
    }
    if m.get("size")
        .map(|s| same(s, &json!({"cpu": -1, "memMb": -1})))
        .unwrap_or(false)
    {
        m.remove("size");
    }
    if m.get("firstLife") == Some(&json!(false)) {
        m.remove("firstLife");
    }
    v
}

/// A machine handle: its id and the optional methods the scene gives it.
pub struct FakeMachine {
    pub id: String,
    pub put_bytes: bool,
}

#[async_trait(?Send)]
impl Machine for FakeMachine {
    fn id(&self) -> &str {
        &self.id
    }
    fn kind(&self) -> MachineKind {
        unimplemented!()
    }
    fn offers(&self) -> MachineOffers {
        MachineOffers {
            put_bytes: self.put_bytes,
            ..MachineOffers::default()
        }
    }
    async fn exec(&self, _cmd: &str, _opts: ExecOptions) -> EngineResult<ExecResult> {
        unimplemented!()
    }
    async fn run(&self, _script: &str, _opts: RunOptions) -> EngineResult<ExecResult> {
        unimplemented!()
    }
    async fn snapshot(
        &self,
        _name: &str,
        _life: MachineLife,
        _opts: SnapshotOptions,
    ) -> EngineResult<String> {
        unimplemented!()
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> EngineResult<()> {
        unimplemented!()
    }
    async fn resume(&self, _signal: Option<&Cancel>) -> EngineResult<()> {
        unimplemented!()
    }
    async fn kill(&self) -> EngineResult<()> {
        unimplemented!()
    }
    async fn state(&self) -> EngineResult<MachineState> {
        unimplemented!()
    }
    async fn download_url(&self, _path: &str) -> EngineResult<String> {
        unimplemented!()
    }
    async fn upload_url(&self, _path: &str) -> EngineResult<String> {
        unimplemented!()
    }
}

/// `backendFor(record)`'s answer: only its capabilities are read.
pub struct FakeBackend {
    pub signed_urls: bool,
}

struct FakePricing;

impl BackendPricing for FakePricing {
    fn rate_usd_per_hour(&self, _size: Size) -> f64 {
        unimplemented!()
    }
    fn default_size(&self) -> Size {
        unimplemented!()
    }
    fn snapshot_storage(&self) -> SnapshotStoragePricing {
        unimplemented!()
    }
}

#[async_trait(?Send)]
impl MachineBackend for FakeBackend {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            live_clone_forks: false,
            pause_mode: None,
            replaces_machine: false,
            preview_urls: false,
            signed_urls: self.signed_urls,
            callback_relay: false,
            disk_snapshots: false,
            images: false,
            snapshots_any_life: false,
            snapshot_listing: false,
            templates: false,
            kept: false,
            copies: false,
            own_network: false,
            sizes_last: false,
            sizes: Vec::<MachineSizeOffer>::new(),
        }
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
        Arc::new(FakePricing)
    }
    async fn create(
        &self,
        _spec: MachineSpec,
        _waiting: Option<Waiting>,
        _sent: Option<String>,
    ) -> EngineResult<Arc<dyn Machine>> {
        unimplemented!()
    }
    async fn get(&self, _id: &str) -> EngineResult<Arc<dyn Machine>> {
        unimplemented!()
    }
    async fn list(
        &self,
        _labels: Option<&IndexMap<String, String>>,
    ) -> EngineResult<Vec<MachineListRow>> {
        unimplemented!()
    }
    async fn delete_snapshot(&self, _id: &str) -> EngineResult<()> {
        unimplemented!()
    }
}

/// A live workspace the fixture's entry describes. The engine's `Workspace::new` is todo today, so building one panics
/// until it lands; the test is red either way.
pub fn live_of(record: WorkspaceRecord, machine: Arc<dyn Machine>) -> LiveWorkspace {
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
    let ws = Rc::new(Workspace::new(
        machine.clone(),
        hooks,
        WorkspaceInitial::default(),
    ));
    LiveWorkspace {
        record: Rc::new(RefCell::new(record)),
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
    }
}

/// How the oracle's `mod` scripts one kind's module.
#[derive(Clone)]
pub struct ModCfg {
    /// `version`: absent reads the current daemon, `null` is a version not read.
    pub version: Value,
    pub deploy: String,
    pub roots: String,
    pub has_daemon: bool,
    /// Not in the oracle: the `roots` call on the record of this id is held open until `World::open_roots`.
    pub roots_hold: Option<String>,
}

impl ModCfg {
    pub fn of(m: Option<&Value>) -> ModCfg {
        let get = |k: &str| m.and_then(|m| m.get(k));
        ModCfg {
            version: get("version").cloned().unwrap_or_else(|| json!("current")),
            deploy: get("deploy")
                .and_then(Value::as_str)
                .unwrap_or("ok")
                .to_string(),
            roots: get("roots")
                .and_then(Value::as_str)
                .unwrap_or("ok")
                .to_string(),
            has_daemon: get("hasDaemon").and_then(Value::as_bool).unwrap_or(true),
            roots_hold: get("rootsHold").and_then(Value::as_str).map(str::to_string),
        }
    }
}

/// One entry of the oracle's world, beside its record.
#[derive(Clone)]
pub struct EntryCfg {
    pub id: String,
    pub kind: String,
    pub served: bool,
    pub signed_urls: bool,
    pub md: ModCfg,
}

/// The oracle's `moduleOf(kind)`: the first entry of that kind's `mod`.
pub struct FakeModule {
    log: Log,
    cfg: ModCfg,
    daemon_version: u32,
    gates: Rc<RefCell<Vec<oneshot::Sender<()>>>>,
    roots_hold: Rc<RefCell<Option<oneshot::Receiver<()>>>>,
}

impl KindModule for FakeModule {
    fn backend(&self, _record: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError> {
        unimplemented!()
    }
    fn exec_stream(
        &self,
        _entry: &LiveWorkspace,
        _opts: Option<MachineExecOptions>,
        _waiting: Option<TurnWaiting>,
    ) -> Arc<dyn wsp_adapters::port::ExecStreamFactory> {
        unimplemented!()
    }
    fn admit_folder(&self, _record: FolderAdmit) -> Option<Local<'static, ()>> {
        unimplemented!()
    }
    fn folder(&self, _record: &WorkspaceRecord) -> Result<Option<String>, HostError> {
        unimplemented!()
    }
    fn home(&self, _entry: &LiveWorkspace, _agent_id: &str) -> String {
        unimplemented!()
    }
    fn home_dir(&self, _record: &WorkspaceRecord) -> Option<String> {
        unimplemented!()
    }
    fn env(
        &self,
        _entry: &LiveWorkspace,
        _agent_id: Option<&str>,
        _home_of: &dyn Fn(&str) -> String,
    ) -> IndexMap<String, String> {
        unimplemented!()
    }
    fn login_stands(&self, _entry: &LiveWorkspace, _agent_id: &str) -> bool {
        unimplemented!()
    }
    fn import(
        &self,
        _entry: &LiveWorkspace,
        _o: ProjectImportOptions,
        _report: ImportReport,
    ) -> Local<'static, ImportLanded> {
        unimplemented!()
    }
    fn roots(&self, entry: &LiveWorkspace, dests: &[String]) -> Local<'static, ()> {
        let id = entry.record.borrow().id.clone();
        let dests = dests.to_vec();
        let log = self.log.clone();
        let fails = self.cfg.roots == "fails";
        let hold = self.roots_hold.clone();
        let holds = self.cfg.roots_hold.as_deref() == Some(id.as_str());
        async move {
            log.push(json!(["roots", id, dests]));
            if holds {
                let rx = hold.borrow_mut().take();
                if let Some(rx) = rx {
                    let _ = rx.await;
                }
            }
            if holds {
                // Not in the oracle: where a held call settles, for the test of the chain's order.
                log.push(json!(["roots settled", id]));
            }
            if fails {
                return Err(err(&format!("{} roots refused", "x".repeat(250))));
            }
            Ok(())
        }
        .boxed_local()
    }
    fn land_project(
        &self,
        _entry: &LiveWorkspace,
        _project: ProjectView,
        _report: StageReport,
    ) -> Local<'static, ()> {
        unimplemented!()
    }
    fn memory_key(
        &self,
        _entry: &LiveWorkspace,
        _agent_id: &str,
    ) -> Result<Option<String>, HostError> {
        unimplemented!()
    }
    fn end_thread(
        &self,
        _entry: &LiveWorkspace,
        _thread_id: &str,
        _o: EndThread,
    ) -> Option<Local<'static, Option<String>>> {
        unimplemented!()
    }
    fn end_owed(
        &self,
        _entry: &LiveWorkspace,
        _thread_id: &str,
    ) -> Option<Local<'static, Option<Local<'static, ()>>>> {
        unimplemented!()
    }
    fn port_reach(
        &self,
        _entry: &LiveWorkspace,
        _port: u16,
        _standing: bool,
    ) -> Option<Local<'static, PortRoute>> {
        unimplemented!()
    }
    fn relayed(&self, _machine_id: Option<&str>) -> bool {
        unimplemented!()
    }
    fn wsp_mcp(&self, _entry: &LiveWorkspace) -> Option<McpServerSpec> {
        unimplemented!()
    }
    fn turn_reach(&self, _entry: &LiveWorkspace) -> Option<Option<String>> {
        unimplemented!()
    }
    fn turn_road(&self) -> ScopedRoad {
        unimplemented!()
    }
    fn keeps_agents(&self) -> bool {
        unimplemented!()
    }
    fn server_values(&self) -> ServerValuesRoad {
        unimplemented!()
    }
    fn has_daemon(&self, entry: &LiveWorkspace) -> bool {
        self.log
            .push(json!(["hasDaemon", entry.record.borrow().id.clone()]));
        self.cfg.has_daemon
    }
    fn shared_daemon(&self) -> bool {
        unimplemented!()
    }
    fn daemon_road(
        &self,
        _entry: &LiveWorkspace,
    ) -> Local<'static, wsp_wire::views::workspace::DaemonReachView> {
        unimplemented!()
    }
    fn scratch(&self, _entry: &LiveWorkspace) -> String {
        unimplemented!()
    }
    fn dropped(&self, _entry: &LiveWorkspace) -> Local<'static, ()> {
        unimplemented!()
    }
    fn daemon_version(&self, entry: &LiveWorkspace) -> Local<'static, Option<u32>> {
        let id = entry.record.borrow().id.clone();
        let log = self.log.clone();
        let version = self.cfg.version.clone();
        let current = self.daemon_version;
        async move {
            log.push(json!(["daemonVersion", id]));
            match &version {
                Value::String(s) if s == "throws" => Err(err("hello failed")),
                Value::String(_) => Ok(Some(current)),
                Value::Null => Ok(None),
                n => Ok(Some(n.as_u64().expect("a version number") as u32)),
            }
        }
        .boxed_local()
    }
    fn restart_daemon(&self, _entry: &LiveWorkspace) -> Option<Local<'static, ()>> {
        unimplemented!()
    }
    fn deploy_daemon(&self, entry: &LiveWorkspace) -> Option<Local<'static, Option<String>>> {
        if self.cfg.deploy == "none" {
            return None;
        }
        let id = entry.record.borrow().id.clone();
        let log = self.log.clone();
        let how = self.cfg.deploy.clone();
        let gates = self.gates.clone();
        Some(
            async move {
                log.push(json!(["deployDaemon", id]));
                if how == "hangs" {
                    let (tx, rx) = oneshot::channel();
                    gates.borrow_mut().push(tx);
                    let _ = rx.await;
                }
                match how.as_str() {
                    "lacks" => Err(marked(
                        "gcc is missing on this machine, so the daemon cannot build",
                        "wspMachineLacks",
                    )),
                    "unanswered" => Err(marked("no answer", "wspMachineUnanswered")),
                    "fails" => Err(err("npm ERR! long log")),
                    _ => Ok(None),
                }
            }
            .boxed_local(),
        )
    }
}

/// The oracle's fake context, as the daemon area's runtime trait.
pub struct FakeRuntime {
    pub log: Log,
    pub clock: Rc<FakeClock>,
    pub ids: Rc<FakeIds>,
    pub live: Rc<RefCell<IndexMap<String, Entry>>>,
    pub sessions: Rc<RefCell<IndexMap<String, Row>>>,
    pub notes: Rc<RefCell<IndexMap<String, String>>>,
    pub closed: bool,
    pub starts: RefCell<VecDeque<String>>,
    pub cfgs: Vec<EntryCfg>,
    pub daemon_version: u32,
    pub gates: Rc<RefCell<Vec<oneshot::Sender<()>>>>,
    pub roots_hold: Rc<RefCell<Option<oneshot::Receiver<()>>>>,
    pub area: RefCell<Weak<DaemonArea>>,
    pub request_ids: RefCell<Vec<String>>,
    /// When set, `module_of` answers this error (the TS `moduleOf` throws for a kind it does not wire).
    pub module_of_failure: RefCell<Option<HostError>>,
    /// When set, `project_path` answers this error (the TS `projectHeld(id).path` throws on an unknown project).
    pub project_path_failure: RefCell<Option<HostError>>,
    /// When set, `checkout_of` answers this error (the TS `checkoutOf` reaches `projectHeld`).
    pub checkout_failure: RefCell<Option<HostError>>,
    /// When set, `backend_for` answers this error (the TS `backendFor` throws for a kind with no module).
    pub backend_for_failure: RefCell<Option<HostError>>,
}

impl FakeRuntime {
    /// `bus.emit`: logged as the oracle's scene wrote the event, then heard by the area as the wiring would hand it.
    pub fn emit(&self, abbrev: Value, full: Value) {
        self.log.push(json!(["emit", abbrev]));
        let event: EventUnion = serde_json::from_value(full).expect("the event parses");
        let area = self.area.borrow().upgrade();
        if let Some(area) = area {
            area.on_event(&event);
        }
    }

    fn cfg_of(&self, id: &str) -> Option<&EntryCfg> {
        self.cfgs.iter().find(|c| c.id == id)
    }
}

/// The origin as the oracle's scene wrote it: `{kind: <road>}`.
fn caller_json(origin: Option<&Caller>) -> Value {
    match origin {
        None => Value::Null,
        Some(Caller::Road(o)) => {
            json!({"kind": serde_json::to_value(o).expect("a road serializes")})
        }
        Some(Caller::Scoped { origin, .. }) => {
            json!({"kind": serde_json::to_value(origin).expect("a road serializes")})
        }
    }
}

#[async_trait(?Send)]
impl DaemonRuntime for FakeRuntime {
    fn clock(&self) -> Rc<dyn Clock> {
        self.clock.clone()
    }
    fn ids(&self) -> Rc<dyn Ids> {
        self.ids.clone()
    }
    fn warn(&self, line: &str) {
        self.log.push(json!(["warn", line]));
    }
    fn daemon_hello_timeout_ms(&self) -> u64 {
        8_000
    }
    fn live(&self) -> Rc<RefCell<IndexMap<String, Entry>>> {
        self.live.clone()
    }
    fn sessions(&self) -> Rc<RefCell<IndexMap<String, Row>>> {
        self.sessions.clone()
    }
    fn daemon_notes(&self) -> Rc<RefCell<IndexMap<String, String>>> {
        self.notes.clone()
    }
    fn closed(&self) -> bool {
        self.closed
    }
    async fn sessions_start(
        &self,
        workspace_id: &str,
        ask: DetachedStart,
        origin: Option<&Caller>,
    ) -> Result<Started, HostError> {
        self.request_ids.borrow_mut().push(ask.request_id.clone());
        self.log.push(json!(["start", workspace_id, {"prompt": ask.prompt, "thread": ask.thread, "requestId": "string"}, caller_json(origin)]));
        let how = self
            .starts
            .borrow_mut()
            .pop_front()
            .unwrap_or_else(|| "started".to_string());
        match how.as_str() {
            "fails" => Err(err("refused")),
            "panics" => panic!("the start broke"),
            "queued" => {
                // The oracle's events carry a request id and a harness; the Rust event also needs a thread and a prompt, and
                // the thread differs from the ask's so an answer read off the event shows.
                let queued = |request: &str, harness: &str| {
                    (
                        json!({"type": "session.queued", "requestId": request, "harness": harness}),
                        json!({"type": "session.queued", "workspaceId": workspace_id, "threadId": "t-event", "harness": harness, "prompt": "hi", "requestId": request}),
                    )
                };
                let (abbrev, full) = queued("other", "claude");
                self.emit(abbrev, full);
                let (abbrev, full) = queued(&ask.request_id, "codex");
                self.emit(abbrev, full);
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                Err(err("later failure"))
            }
            "held" => {
                let view = session_view(
                    json!({"id": "row-1", "workspaceId": workspace_id, "harness": "claude", "status": "running"}),
                );
                // The oracle's wait is `{place, running, limit}`; the Rust wait names its place and says `atOnce`.
                let wait: ThreadCapWait = serde_json::from_value(
                    json!({"placeId": "box", "place": "box", "running": 2, "atOnce": 2}),
                )
                .expect("a cap wait parses");
                (ask.on_held)(view, wait);
                std::future::pending::<Result<Started, HostError>>().await
            }
            other => {
                let outcome = if other == "steered" {
                    SessionStartOutcome::Steered
                } else {
                    SessionStartOutcome::Started
                };
                let mut view = json!({"id": "s-9", "workspaceId": workspace_id, "harness": "claude", "status": "running"});
                if other == "steered" {
                    view["threadId"] = json!("t-steer");
                }
                Ok(Started {
                    outcome,
                    view: session_view(view),
                })
            }
        }
    }
    async fn push_status(&self, entry: &Entry) -> Result<(), HostError> {
        let id = entry.borrow().record.borrow().id.clone();
        let note = self.notes.borrow().get(&id).cloned();
        self.log.push(json!(["pushStatus", id, note]));
        Ok(())
    }
    async fn persist(&self, record: &WorkspaceRecord) -> Result<(), HostError> {
        self.log
            .push(json!(["persist", record.id, record_json(record)]));
        Ok(())
    }
    fn served_by_its_computer(&self, entry: &Entry) -> Option<String> {
        let id = entry.borrow().record.borrow().id.clone();
        self.cfg_of(&id)
            .filter(|c| c.served)
            .map(|_| "computer".to_string())
    }
    fn module_of(&self, kind: &str) -> Result<Rc<dyn KindModule>, HostError> {
        if let Some(e) = self.module_of_failure.borrow().clone() {
            return Err(e);
        }
        let cfg = self
            .cfgs
            .iter()
            .find(|c| c.kind == kind)
            .expect("an entry of that kind")
            .md
            .clone();
        Ok(Rc::new(FakeModule {
            log: self.log.clone(),
            cfg,
            daemon_version: self.daemon_version,
            gates: self.gates.clone(),
            roots_hold: self.roots_hold.clone(),
        }))
    }
    fn project_path(&self, project_id: &str) -> Result<String, HostError> {
        if let Some(e) = self.project_path_failure.borrow().clone() {
            return Err(e);
        }
        Ok(format!("/work/{project_id}"))
    }
    fn checkout_of(&self, record: &WorkspaceRecord) -> Result<String, HostError> {
        if let Some(e) = self.checkout_failure.borrow().clone() {
            return Err(e);
        }
        Ok(format!("/work/{}/{}", record.project, record.id))
    }
    fn backend_for(&self, record: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError> {
        if let Some(e) = self.backend_for_failure.borrow().clone() {
            return Err(e);
        }
        let signed_urls = self
            .cfg_of(&record.id)
            .map(|c| c.signed_urls)
            .unwrap_or(true);
        Ok(Arc::new(FakeBackend { signed_urls }))
    }
}

pub fn session_view(v: Value) -> SessionView {
    serde_json::from_value(v).expect("a session view parses")
}

/// A status a poll read: the phase and the reach state, and `facts` where the poll carried them. The oracle's `{}` for
/// facts is a full `{os, uptimeMs, folder}` here, since the Rust facts name their three fields.
pub fn status(phase: &str, state: &str, facts: bool) -> WorkspaceStatus {
    let mut v = json!({
        "id": "a", "name": FILL, "machineId": "m-a", "phase": phase, "golden": FILL, "createdAt": FILL,
        "project": {"id": "p", "name": "p", "path": "/work/p", "computer": "here"},
        "machineState": "running", "reach": {"state": state}, "size": {"cpu": 1, "memMb": 1}, "rateUsdPerHour": 0,
    });
    if facts {
        v["facts"] = json!({"os": "linux", "uptimeMs": 0, "folder": "/"});
    }
    serde_json::from_value(v).expect("a status parses")
}

pub fn reach(state: &str) -> ReachState {
    serde_json::from_value(json!(state)).expect("a reach state")
}

/// A session event as the oracle's scene wrote it, and the full one the Rust union reads.
pub fn session_event(kind: &str, workspace_id: &str) -> (Value, Value) {
    let abbrev = json!({"type": kind, "workspaceId": workspace_id});
    let mut full = abbrev.clone();
    full["sessionId"] = json!("s-event");
    match kind {
        "session.delta" => {
            full["kind"] = json!("text");
            full["text"] = json!("");
        }
        "session.done" => full["result"] = json!({"status": "completed"}),
        "session.end" => {
            full["exitCode"] = json!(0);
            full["sawResult"] = json!(true);
        }
        other => panic!("no event {other}"),
    }
    (abbrev, full)
}

pub fn moments_json(map: &IndexMap<String, wsp_core::types::internal::MachineMoment>) -> Value {
    Value::Array(
        map.iter()
            .map(|(k, m)| json!([k, {"machineId": m.machine_id, "at": m.at.0 as i64}]))
            .collect(),
    )
}

/// The oracle's `world(w)`: the runtime and the area over it.
pub struct World {
    pub log: Log,
    pub rt: Rc<FakeRuntime>,
    pub area: Rc<DaemonArea>,
    pub clock: Rc<FakeClock>,
    roots_release: RefCell<Option<oneshot::Sender<()>>>,
}

impl World {
    pub fn new(w: &Value) -> World {
        let log = Log::default();
        let clock = FakeClock::new(T0);
        let live: Rc<RefCell<IndexMap<String, Entry>>> = Rc::default();
        let sessions: Rc<RefCell<IndexMap<String, Row>>> = Rc::default();
        let mut cfgs = Vec::new();
        for e in w["entries"].as_array().expect("entries") {
            let record = record_of(&e["record"]);
            let md = ModCfg::of(e.get("mod"));
            let machine_id = e["machine"]["id"]
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| record.machine_id.clone());
            let put_bytes = e["machine"].get("putBytes").is_some();
            cfgs.push(EntryCfg {
                id: record.id.clone(),
                kind: e["record"]["kind"].as_str().expect("a kind").to_string(),
                served: e.get("served").and_then(Value::as_bool).unwrap_or(false),
                signed_urls: e.get("signedUrls").and_then(Value::as_bool).unwrap_or(true),
                md,
            });
            let machine: Arc<dyn Machine> = Arc::new(FakeMachine {
                id: machine_id,
                put_bytes,
            });
            let id = record.id.clone();
            live.borrow_mut()
                .insert(id, Rc::new(RefCell::new(live_of(record, machine))));
        }
        for s in w
            .get("sessions")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let mut view = json!({"id": s["id"], "workspaceId": s["workspaceId"], "harness": "claude",
                // The oracle's `done` is the Rust `completed`.
                "status": if s["status"] == "done" { json!("completed") } else { s["status"].clone() }});
            for key in ["threadId", "startedAt"] {
                if let Some(v) = s.get(key) {
                    view[key] = v.clone();
                }
            }
            let row = SessionEntry {
                view: session_view(view),
                turn_id: String::new(),
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
                queued: s.get("queued").is_some(),
            };
            sessions.borrow_mut().insert(
                s["id"].as_str().expect("a session id").to_string(),
                Rc::new(RefCell::new(row)),
            );
        }
        let hold = cfgs
            .iter()
            .find(|c| c.md.roots_hold.is_some())
            .map(|_| oneshot::channel::<()>());
        let (roots_release, roots_hold) = match hold {
            Some((tx, rx)) => (Some(tx), Some(rx)),
            None => (None, None),
        };
        let rt = Rc::new(FakeRuntime {
            log: log.clone(),
            clock: clock.clone(),
            ids: Rc::new(FakeIds::default()),
            live,
            sessions,
            notes: Rc::default(),
            closed: w.get("closed").and_then(Value::as_bool).unwrap_or(false),
            starts: RefCell::new(
                w.get("starts")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .map(|s| s.as_str().expect("a start").to_string())
                    .collect(),
            ),
            cfgs,
            daemon_version: common::fixture("daemon")["DAEMON_VERSION"]
                .as_u64()
                .expect("DAEMON_VERSION") as u32,
            gates: Rc::default(),
            roots_hold: Rc::new(RefCell::new(roots_hold)),
            area: RefCell::new(Weak::new()),
            module_of_failure: RefCell::new(None),
            project_path_failure: RefCell::new(None),
            checkout_failure: RefCell::new(None),
            backend_for_failure: RefCell::new(None),
            request_ids: RefCell::new(vec![]),
        });
        let area = DaemonArea::new(rt.clone());
        *rt.area.borrow_mut() = Rc::downgrade(&area);
        World {
            log,
            rt,
            area,
            clock,
            roots_release: RefCell::new(roots_release),
        }
    }

    /// A second area over the same runtime and records: the host after a restart.
    pub fn fresh_area(&self) -> Rc<DaemonArea> {
        let area = DaemonArea::new(self.rt.clone());
        *self.rt.area.borrow_mut() = Rc::downgrade(&area);
        area
    }

    pub fn entry(&self, id: &str) -> Entry {
        self.rt
            .live
            .borrow()
            .get(id)
            .unwrap_or_else(|| panic!("no entry {id}"))
            .clone()
    }

    /// The oracle's `x.bus.emit` with an event the scene wrote.
    pub fn emit(&self, event: (Value, Value)) {
        self.rt.emit(event.0, event.1);
    }

    /// `x.gates.shift()!()`: lets the oldest hung deploy end.
    pub fn open_deploy(&self) {
        let tx = self.rt.gates.borrow_mut().remove(0);
        let _ = tx.send(());
    }

    pub fn open_roots(&self) {
        let tx = self
            .roots_release
            .borrow_mut()
            .take()
            .expect("a held roots call");
        let _ = tx.send(());
    }

    pub fn push(&self, entry: Value) {
        self.log.push(entry);
    }

    /// The records in `live` order, as the fixture writes them.
    pub fn records(&self) -> Vec<Value> {
        self.rt
            .live
            .borrow()
            .values()
            .map(|e| record_json(&e.borrow().record.borrow()))
            .collect()
    }

    pub fn set_phase(&self, id: &str, phase: &str) {
        self.entry(id).borrow().record.borrow_mut().phase =
            serde_json::from_value(json!(phase)).expect("a phase");
    }

    pub fn set_session_status(&self, id: &str, status: &str) {
        let row = self.rt.sessions.borrow().get(id).expect("a row").clone();
        row.borrow_mut().view.status = serde_json::from_value(json!(status)).expect("a status");
    }

    pub fn last_request_id(&self) -> String {
        self.rt
            .request_ids
            .borrow()
            .last()
            .cloned()
            .expect("a start was asked")
    }

    /// The scene's log in order and its records after, as the fixture wrote them.
    pub fn check(&self, scene: &Value, skip: &[&str]) {
        self.check_with(scene, skip, &|_| {});
    }

    /// `check`, with `fix` rewriting the expected log where a Rust wire type is shaped differently from the oracle's
    /// loose object (each such rewrite is named at its call).
    pub fn check_with(&self, scene: &Value, skip: &[&str], fix: &dyn Fn(&mut Vec<Value>)) {
        let mut want: Vec<Value> = scene["log"]
            .as_array()
            .expect("a log")
            .iter()
            .filter(|e| !skip.contains(&e[0].as_str().unwrap_or("")))
            .cloned()
            .collect();
        fix(&mut want);
        let got = self.log.take();
        for (i, (w, g)) in want.iter().zip(got.iter()).enumerate() {
            assert!(
                same(g, w),
                "scene {:?}: log entry {i} differs\n got {g}\nwant {w}",
                scene["name"]
            );
        }
        assert_eq!(
            got.len(),
            want.len(),
            "scene {:?}: log length differs; got {:#?}",
            scene["name"],
            got
        );
        let records = self.records();
        let want_records = scene["records"].as_array().expect("records");
        assert_eq!(
            records.len(),
            want_records.len(),
            "scene {:?}: record count",
            scene["name"]
        );
        for (g, w) in records.iter().zip(want_records) {
            assert!(
                same(g, w),
                "scene {:?}: record differs\n got {g}\nwant {w}",
                scene["name"]
            );
        }
    }
}

/// `DetachedSend` as the fixture's `answer` writes it, with the cap wait as the Rust wire names it.
pub fn answer_json(a: &DetachedSend) -> Value {
    let outcome = match a.outcome {
        DetachedOutcome::Steered => "steered",
        DetachedOutcome::Queued => "queued",
        DetachedOutcome::Started => "started",
        DetachedOutcome::Held => "held",
    };
    let mut v = json!({"outcome": outcome, "threadId": a.thread_id, "harness": a.harness});
    if let Some(w) = &a.capped {
        v["capped"] = serde_json::to_value(w).expect("a wait serializes");
    }
    v
}

pub fn spawn<F: std::future::Future<Output = T> + 'static, T: 'static>(
    f: F,
) -> tokio::task::JoinHandle<T> {
    tokio::task::spawn_local(f)
}

pub fn flag() -> Rc<Cell<bool>> {
    Rc::new(Cell::new(false))
}
