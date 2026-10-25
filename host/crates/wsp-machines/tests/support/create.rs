// SPDX-License-Identifier: AGPL-3.0-only
//! The fakes of the create oracle: a runtime, a landing road and a kind module that log every call in the oracle's
//! shape, and the `World` a scene's input builds them into. Included with `#[path = "support/create.rs"] mod support;`.
//! Where a Rust wire type is shaped differently from the oracle's loose object, the translation is named at its site.
#![allow(dead_code)]

use std::cell::RefCell;
use std::collections::{HashSet, VecDeque};
use std::rc::{Rc, Weak};
use std::sync::Arc;

use async_trait::async_trait;
use futures::FutureExt;
use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Map, Value};

use wsp_core::ids::Ids;
use wsp_core::types::internal::{BuilderLife, EndThread, FolderAdmit, ImportLanded, ImportReport, KindModule, PortRoute, ServerValuesRoad};
use wsp_core::types::pending::{Clock, MachineExecOptions, PlaceDoor, ScopedRoad, TurnWaiting};
use wsp_core::types::wiring::{
    CreateWorkspaceOptions, CreatedWorkspace, IdleWindow, LiveWorkspace, Local, ProjectImportOptions, StageReport, StageSaid, WorkspaceRecord,
    WorkspaceSpec,
};
use wsp_engine::errors::Result as EngineResult;
use wsp_engine::Cancel;
use wsp_engine::lifecycle::{Workspace, WorkspaceHooks, WorkspaceInitial};
use wsp_engine::machine::{
    BackendPricing, Capabilities, ExecOptions, ExecResult, Machine, MachineBackend, MachineKind, MachineLife, MachineListRow, MachineOffers,
    MachineSpec, MachineState, RunOptions, Size, SnapshotOptions, SnapshotStoragePricing, Waiting,
};
use wsp_machines::machines::create::{CreateArea, CreateRuntime, KeptBuilder, LandingRoad, RecordBranchAsk};
use wsp_machines::machines::machines::Entry;
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::views::harness::McpServerSpec;
use wsp_wire::views::place::MachineBind;
use wsp_wire::views::workspace::{BranchOfRecord, ProjectView, ThreadLit};
use wsp_wire::views::workspace_events::WorkspaceCreateStage;
use wsp_wire::views::workspace_more::{ProjectRef, ThreadScope, WorkspaceAgents, WorkspaceView};
use wsp_wire::wire::capabilities::{MachineSizeOffer, WorkspaceSize};
use wsp_wire::wire::events_union::EventUnion;
use wsp_words::exit::{HostError, Kind};

use crate::common::{FakeClock, FakeIds, Log, T0};

/// What a record the fixture wrote leaves out, filled so the record parses.
const FILL: &str = "~fill~";

/// The mark `ctx.view` puts on the views it makes, so the emitted event and the answer show they are that view.
pub const VIEW_MARK: &str = "~view~";

/// What the fake `namedOfRecord` answers as the branch's repository id, and what a named branch is given.
pub const NAMED_REPO: i64 = 41;
pub const GIVEN_REPO: i64 = 42;

pub fn err(message: &str) -> HostError {
    HostError::new(message)
}

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

/// A record the fixture wrote in part (`{id, name, kind, project}`), with the rest filled in.
pub fn record_of(v: &Value) -> WorkspaceRecord {
    let mut m = v.as_object().expect("a record is an object").clone();
    m.entry("machineId".to_string()).or_insert_with(|| json!(""));
    m.entry("phase".to_string()).or_insert_with(|| json!("running"));
    m.entry("golden".to_string()).or_insert_with(|| json!(FILL));
    m.entry("createdAt".to_string()).or_insert_with(|| json!(FILL));
    m.entry("spec".to_string()).or_insert_with(|| json!({}));
    m.entry("size".to_string()).or_insert_with(|| json!({"cpu": 1, "memMb": 1}));
    m.entry("firstLife".to_string()).or_insert_with(|| json!(true));
    serde_json::from_value(Value::Object(m)).expect("the record parses")
}

pub struct FakeMachine {
    pub id: String,
    pub notice: Option<String>,
    pub preview_url: bool,
    pub daemon_answers: bool,
}

#[async_trait(?Send)]
impl Machine for FakeMachine {
    fn id(&self) -> &str {
        &self.id
    }
    fn kind(&self) -> MachineKind {
        unimplemented!()
    }
    fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }
    fn offers(&self) -> MachineOffers {
        MachineOffers { preview_url: self.preview_url, daemon_answers: self.daemon_answers, ..MachineOffers::default() }
    }
    async fn exec(&self, _cmd: &str, _opts: ExecOptions) -> EngineResult<ExecResult> {
        unimplemented!()
    }
    async fn run(&self, _script: &str, _opts: RunOptions) -> EngineResult<ExecResult> {
        unimplemented!()
    }
    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> EngineResult<String> {
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

/// The oracle's `landingBackend`'s answer: id "landing", a default size of 1x2048, and three sizes on offer.
pub struct FakeBackend;

struct FakePricing;

impl BackendPricing for FakePricing {
    fn rate_usd_per_hour(&self, _size: Size) -> f64 {
        unimplemented!()
    }
    fn default_size(&self) -> Size {
        Size { cpu: 1.0, mem_mb: 2048.0 }
    }
    fn snapshot_storage(&self) -> SnapshotStoragePricing {
        unimplemented!()
    }
}

#[async_trait(?Send)]
impl MachineBackend for FakeBackend {
    fn capabilities(&self) -> Capabilities {
        let size = |cpu: f64, mem_mb: f64, rate: f64| MachineSizeOffer { cpu: JsNumber(cpu), mem_mb: JsNumber(mem_mb), rate_usd_per_hour: JsNumber(rate), name: None };
        Capabilities {
            live_clone_forks: false,
            pause_mode: None,
            replaces_machine: false,
            preview_urls: false,
            signed_urls: false,
            callback_relay: false,
            disk_snapshots: false,
            images: false,
            snapshots_any_life: false,
            snapshot_listing: false,
            templates: false,
            kept: false,
            copies: false,
            own_network: false,
            sizes: vec![size(1.0, 2048.0, 0.05), size(2.0, 4096.0, 0.1), size(4.0, 8192.0, 0.2)],
        }
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
        Arc::new(FakePricing)
    }
    async fn create(&self, _spec: MachineSpec, _waiting: Option<Waiting>, _sent: Option<String>) -> EngineResult<Arc<dyn Machine>> {
        unimplemented!()
    }
    async fn get(&self, _id: &str) -> EngineResult<Arc<dyn Machine>> {
        unimplemented!()
    }
    async fn list(&self, _labels: Option<&IndexMap<String, String>>) -> EngineResult<Vec<MachineListRow>> {
        unimplemented!()
    }
    async fn delete_snapshot(&self, _id: &str) -> EngineResult<()> {
        unimplemented!()
    }
}

/// A live workspace for a record. The engine's `Workspace::new` is todo today, so building one panics until it lands;
/// the test is red either way.
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
    let ws = Rc::new(Workspace::new(machine.clone(), hooks, WorkspaceInitial::default()));
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

pub fn plain_machine(id: &str) -> Arc<dyn Machine> {
    Arc::new(FakeMachine { id: id.to_string(), notice: None, preview_url: false, daemon_answers: false })
}

/// The oracle's `projectLanding(kind)` answer: `road.kind`, `refusal`, `memoryDir` and `binds` scripted per scene.
struct FakeRoad {
    log: Log,
    road: Value,
}

impl LandingRoad for FakeRoad {
    fn kind(&self) -> String {
        self.road.get("kind").and_then(Value::as_str).unwrap_or("provider").to_string()
    }
    fn refusal(&self, project: &ProjectView, deps: &Value) -> Option<String> {
        self.log.push(json!(["road.refusal", project.id, deps]));
        self.road.get("refusal").and_then(Value::as_str).map(str::to_string)
    }
    fn memory_dir(&self, _project: &ProjectView, memory_key: &str, _deps: &Value) -> String {
        self.log.push(json!(["road.places", memory_key]));
        self.road.get("memoryDir").and_then(Value::as_str).unwrap_or("/mem").to_string()
    }
    fn workspace_binds(&self, project: &ProjectView) -> Vec<MachineBind> {
        self.log.push(json!(["road.workspaceBinds", project.id]));
        // The oracle's binds are `{from, to}`; the Rust bind names `source` and `target`.
        self.road
            .get("binds")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|b| MachineBind { source: b["from"].as_str().expect("from").to_string(), target: b["to"].as_str().expect("to").to_string(), read_only: None })
            .collect()
    }
}

/// The oracle's `moduleOf(kind)`: only `landProject`.
struct FakeModule {
    log: Log,
    kind: String,
    land_fails: bool,
}

impl KindModule for FakeModule {
    fn backend(&self, _record: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError> {
        unimplemented!()
    }
    fn exec_stream(&self, _entry: &LiveWorkspace, _opts: Option<MachineExecOptions>, _waiting: Option<TurnWaiting>) -> Arc<dyn wsp_adapters::port::ExecStreamFactory> {
        unimplemented!()
    }
    fn admit_folder(&self, _record: FolderAdmit) -> Option<Local<'static, ()>> {
        unimplemented!()
    }
    fn folder(&self, _record: &WorkspaceRecord) -> Option<String> {
        unimplemented!()
    }
    fn home(&self, _entry: &LiveWorkspace, _agent_id: &str) -> String {
        unimplemented!()
    }
    fn home_dir(&self, _record: &WorkspaceRecord) -> Option<String> {
        unimplemented!()
    }
    fn env(&self, _entry: &LiveWorkspace, _agent_id: Option<&str>, _home_of: &dyn Fn(&str) -> String) -> IndexMap<String, String> {
        unimplemented!()
    }
    fn login_stands(&self, _entry: &LiveWorkspace, _agent_id: &str) -> bool {
        unimplemented!()
    }
    fn import(&self, _entry: &LiveWorkspace, _o: ProjectImportOptions, _report: ImportReport) -> Local<'static, ImportLanded> {
        unimplemented!()
    }
    fn roots(&self, _entry: &LiveWorkspace, _dests: &[String]) -> Local<'static, ()> {
        unimplemented!()
    }
    fn land_project(&self, entry: &LiveWorkspace, project: ProjectView, _report: StageReport) -> Local<'static, ()> {
        let (log, kind, id, project_id, fails) = (self.log.clone(), self.kind.clone(), entry.record.borrow().id.clone(), project.id.clone(), self.land_fails);
        async move {
            log.push(json!(["landProject", kind, id, project_id]));
            if fails {
                return Err(err("clone failed"));
            }
            Ok(())
        }
        .boxed_local()
    }
    fn memory_key(&self, _entry: &LiveWorkspace, _agent_id: &str) -> Option<String> {
        unimplemented!()
    }
    fn end_thread(&self, _entry: &LiveWorkspace, _thread_id: &str, _o: EndThread) -> Option<Local<'static, Option<String>>> {
        unimplemented!()
    }
    fn end_owed(&self, _entry: &LiveWorkspace, _thread_id: &str) -> Option<Local<'static, Option<Local<'static, ()>>>> {
        unimplemented!()
    }
    fn port_reach(&self, _entry: &LiveWorkspace, _port: u16, _standing: bool) -> Option<Local<'static, PortRoute>> {
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
    fn has_daemon(&self, _entry: &LiveWorkspace) -> bool {
        unimplemented!()
    }
    fn shared_daemon(&self) -> bool {
        unimplemented!()
    }
    fn daemon_road(&self, _entry: &LiveWorkspace) -> Local<'static, wsp_wire::views::workspace::DaemonReachView> {
        unimplemented!()
    }
    fn scratch(&self, _entry: &LiveWorkspace) -> String {
        unimplemented!()
    }
    fn dropped(&self, _entry: &LiveWorkspace) -> Local<'static, ()> {
        unimplemented!()
    }
    fn daemon_version(&self, _entry: &LiveWorkspace) -> Local<'static, Option<u32>> {
        unimplemented!()
    }
    fn restart_daemon(&self, _entry: &LiveWorkspace) -> Option<Local<'static, ()>> {
        unimplemented!()
    }
    fn deploy_daemon(&self, _entry: &LiveWorkspace) -> Option<Local<'static, Option<String>>> {
        unimplemented!()
    }
}

struct NoDoor;

impl PlaceDoor for NoDoor {}

/// A kind word the wire writes, as the provider kind the oracle calls "provider" is no `Kind`: it is no kind at all.
pub fn kind_of(word: Option<&str>) -> Option<Kind> {
    match word {
        Some("invalid") => Some(Kind::Invalid),
        Some("concurrency") => Some(Kind::Concurrency),
        Some("provider") | None => None,
        Some(other) => panic!("kind {other}"),
    }
}

/// The oracle's fake context, as the create area's runtime trait.
pub struct FakeRuntime {
    pub log: Log,
    pub clock: Rc<FakeClock>,
    pub ids: Rc<FakeIds>,
    pub live: Rc<RefCell<IndexMap<String, Entry>>>,
    /// The oracle's scene input.
    pub s: Value,
    pub project: ProjectView,
    pub backend: Arc<dyn MachineBackend>,
    pub forks: RefCell<VecDeque<String>>,
    pub builders: RefCell<Vec<KeptBuilder>>,
    pub grace: RefCell<IndexSet<String>>,
    pub holders: HashSet<String>,
    pub area: RefCell<Weak<CreateArea>>,
    /// What the live map and the log held the moment each `bind` returned, with no await since the fork called it.
    pub bound: RefCell<Vec<Value>>,
}

fn is_backend(a: &Arc<dyn MachineBackend>, b: &Arc<dyn MachineBackend>) -> bool {
    Arc::as_ptr(a) as *const () == Arc::as_ptr(b) as *const ()
}

impl FakeRuntime {
    fn place_id(&self) -> Option<String> {
        self.s.get("placeId").and_then(Value::as_str).map(str::to_string)
    }

    /// A refusal at the machine cap: the provider's word rides as the message, which is what the cap line reads as its
    /// `said`, and the kind is concurrency. `status` and `cause` have no field on a `HostError`.
    fn cap(said: &str) -> HostError {
        HostError::with_kind(said, Kind::Concurrency)
    }
}

#[async_trait(?Send)]
impl CreateRuntime for FakeRuntime {
    fn clock(&self) -> Rc<dyn Clock> {
        self.clock.clone()
    }
    fn ids(&self) -> Rc<dyn Ids> {
        self.ids.clone()
    }
    fn emit(&self, event: EventUnion) -> Result<(), HostError> {
        let mut logged = serde_json::to_value(&event).expect("an event serializes");
        if let EventUnion::WorkspaceCreated(created) = &event {
            let w = &created.event.workspace;
            // The oracle's view is `{id, name, view: true}` where `ctx.view` made it; a view from elsewhere has no `view`.
            logged["workspace"] = if w.created_at == VIEW_MARK { json!({"id": w.id, "name": w.name, "view": true}) } else { json!({"id": w.id, "name": w.name}) };
        }
        self.log.push(json!(["emit", logged]));
        let area = self.area.borrow().upgrade();
        if let Some(area) = area {
            area.on_event(&event);
        }
        Ok(())
    }
    fn warn(&self, line: &str) {
        self.log.push(json!(["warn", line]));
    }
    fn live(&self) -> Rc<RefCell<IndexMap<String, Entry>>> {
        self.live.clone()
    }
    fn wired(&self) -> String {
        "solari".to_string()
    }
    fn place_door(&self) -> Option<Rc<dyn PlaceDoor>> {
        Some(Rc::new(NoDoor))
    }
    async fn landing_place(&self, computer: &str) -> Result<Option<String>, HostError> {
        self.log.push(json!(["landingPlace", computer]));
        Ok(self.place_id())
    }
    async fn place_refuses(&self, place_id: Option<&str>) -> Result<(), HostError> {
        self.log.push(json!(["placeRefuses", place_id]));
        Ok(())
    }
    async fn landing_backend(&self, place_id: Option<&str>) -> Result<Arc<dyn MachineBackend>, HostError> {
        self.log.push(json!(["landingBackend", place_id]));
        Ok(self.backend.clone())
    }
    fn landing_kind(&self, computer: &str, at: &Arc<dyn MachineBackend>) -> String {
        // The oracle logs the backend's id; the Rust backend has none, so the log says which backend the area passed.
        self.log.push(json!(["landingKind", computer, if is_backend(at, &self.backend) { "landing" } else { "another backend" }]));
        "provider".to_string()
    }
    async fn landing_deps(&self, computer: &str) -> Result<Value, HostError> {
        self.log.push(json!(["landingDeps", computer]));
        Ok(json!({"on": computer}))
    }
    fn project_landing(&self, kind: &str) -> Result<Rc<dyn LandingRoad>, HostError> {
        self.log.push(json!(["projectLanding", kind]));
        Ok(Rc::new(FakeRoad { log: self.log.clone(), road: self.s.get("road").cloned().unwrap_or_else(|| json!({})) }))
    }
    async fn remember_project(&self, project: ProjectView) -> Result<(), HostError> {
        self.log.push(json!(["rememberProject", project.memory_dir]));
        Ok(())
    }
    fn project_held(&self, project_id: &str) -> ProjectView {
        self.log.push(json!(["projectHeld", project_id]));
        self.project.clone()
    }
    fn keeps_images(&self, at: &Arc<dyn MachineBackend>) -> bool {
        self.log.push(json!(["keepsImages", if is_backend(at, &self.backend) { "landing" } else { "another backend" }]));
        self.s.get("keepsImages").and_then(Value::as_bool).unwrap_or(true)
    }
    async fn copy_for_fork(&self, golden: &str, place_id: Option<&str>, building: &dyn Fn(&str, f64)) -> Result<String, HostError> {
        self.log.push(json!(["copyForFork", golden, place_id]));
        if place_id.is_some() {
            building("box", 0.5);
        }
        Ok(format!("{golden}@copy"))
    }
    async fn image_head(&self) -> Result<String, HostError> {
        self.log.push(json!(["imageHead"]));
        Ok("snap-head".to_string())
    }
    async fn image_size(&self, golden: &str) -> Result<Option<WorkspaceSize>, HostError> {
        self.log.push(json!(["imageOf", golden]));
        Ok(Some(WorkspaceSize { cpu: JsNumber(2.0), mem_mb: JsNumber(4096.0) }))
    }
    fn brokered_start(&self, provider: bool) -> bool {
        self.log.push(json!(["brokeredStart", provider]));
        self.s["road"].get("brokered").and_then(Value::as_bool).unwrap_or(false) && provider
    }
    async fn named_of_record(&self, ask: RecordBranchAsk) -> Result<BranchOfRecord, HostError> {
        let mut logged = json!({"remote": ask.remote, "name": ask.name, "project": ask.project, "on": ask.on, "carried": ask.carried});
        if let Some(base) = &ask.base {
            logged["base"] = json!(base);
        }
        self.log.push(json!(["namedOfRecord", logged]));
        // The oracle's branch is `{owner, name, branch, public: false}`; the Rust one also holds a repository id.
        Ok(BranchOfRecord { repo_id: JsInt::from(NAMED_REPO), owner: "acme".into(), name: "site".into(), branch: "wsp/w1".into(), start_sha: None, public: None })
    }
    fn place_name(&self, place: &str) -> String {
        self.log.push(json!(["placeName", place]));
        format!("name-of-{place}")
    }
    fn place_door_name(&self, place: &str) -> String {
        format!("door-{place}")
    }
    fn spawn_at(&self, place: &str) -> WorkspaceAgents {
        self.log.push(json!(["spawnAt", place]));
        // The oracle's `{machines: 3, threads: 2}`, as the Rust agents write it.
        WorkspaceAgents { spawn: true, max_machines: JsInt::from(3), max_depth: JsInt::from(2) }
    }
    fn tree_of(&self, spawned: Option<&ThreadScope>) -> Map<String, Value> {
        // The oracle's fake answers `{rootThreadId, threadId}`; the runtime's own `treeOf` answers `{parentThreadId,
        // rootThreadId}`, which is what a record holds.
        match spawned {
            None => Map::new(),
            Some(sp) => {
                let mut m = Map::new();
                m.insert("parentThreadId".into(), json!(sp.thread_id));
                m.insert("rootThreadId".into(), json!(sp.root_thread_id));
                m
            }
        }
    }
    fn attach(&self, handle: &Rc<RefCell<WorkspaceRecord>>, machine: Arc<dyn Machine>) -> Entry {
        let record = handle.borrow().clone();
        self.log.push(json!(["attach", record.id, machine.id()]));
        let id = record.id.clone();
        let mut live = live_of(record, machine);
        live.record = Rc::clone(handle);
        let entry: Entry = Rc::new(RefCell::new(live));
        self.live.borrow_mut().insert(id, entry.clone());
        entry
    }
    async fn fork(&self, record: &Rc<RefCell<WorkspaceRecord>>, bind: &dyn for<'m, 'r> Fn(&'m Arc<dyn Machine>, &'r WorkspaceRecord), _report: StageReport) -> Result<Arc<dyn Machine>, HostError> {
        self.log.push(json!(["fork", serde_json::to_value(&*record.borrow()).expect("a record serializes"), null, false]));
        let how = self.forks.borrow_mut().pop_front().unwrap_or_else(|| "ok".to_string());
        match how.as_str() {
            "cap" => Err(FakeRuntime::cap("you have 3 machines")),
            "cap-status" => Err(FakeRuntime::cap("")),
            "fails" => Err(err("provider down")),
            _ => {
                let m = &self.s["machine"];
                let machine: Arc<dyn Machine> = Arc::new(FakeMachine {
                    id: "m-new".to_string(),
                    notice: m.get("notice").and_then(Value::as_str).map(str::to_string),
                    preview_url: m.get("previewUrl").is_some(),
                    daemon_answers: m.get("daemonAnswers").is_some(),
                });
                bind(&machine, &record.borrow());
                let seen = {
                    let live = self.live.borrow();
                    match live.get(&record.borrow().id) {
                        None => json!({"attached": false}),
                        Some(e) => {
                            let e = e.borrow();
                            json!({"attached": true, "creating": e.creating, "opensThread": e.opens_thread, "ofRecordHeld": e.of_record_held, "machineId": e.record.borrow().machine_id.clone()})
                        }
                    }
                };
                let mut seen = seen;
                seen["landed"] = json!(self.log.take().iter().any(|e| e[0] == "landed"));
                self.bound.borrow_mut().push(seen);
                Ok(machine)
            }
        }
    }
    async fn refresh_builders(&self) -> Result<(), HostError> {
        self.log.push(json!(["refreshBuilders"]));
        Ok(())
    }
    fn builders(&self) -> Vec<KeptBuilder> {
        self.builders.borrow().clone()
    }
    fn cancel_grace(&self, builder_id: &str) {
        if self.grace.borrow_mut().shift_remove(builder_id) {
            self.log.push(json!(["grace cancelled", builder_id]));
        }
    }
    async fn kill_builder(&self, builder_id: &str) -> Result<(), HostError> {
        let place = self.builders.borrow().iter().find(|b| b.id == builder_id).and_then(|b| b.place.clone()).unwrap_or_else(|| "solari".to_string());
        self.log.push(json!(["killUntilGone", format!("backend-{place}"), format!("bm-{builder_id}"), "confirm"]));
        Ok(())
    }
    async fn forget_builder(&self, builder_id: &str) -> Result<(), HostError> {
        self.log.push(json!(["forgetBuilder", builder_id]));
        self.builders.borrow_mut().retain(|b| b.id != builder_id);
        Ok(())
    }
    fn holds_slot(&self, record: &WorkspaceRecord) -> bool {
        self.holders.contains(&record.id)
    }
    fn served_by_its_computer(&self, _entry: &Entry) -> Option<String> {
        self.s.get("served").and_then(Value::as_bool).filter(|s| *s).map(|_| "computer".to_string())
    }
    fn daemon_answers_ms(&self, _entry: &Entry) -> u64 {
        30_000
    }
    async fn daemon_reach(&self, _entry: &Entry) -> Result<(), HostError> {
        if self.s.get("reach").and_then(Value::as_str) == Some("fails") {
            return Err(err("no route"));
        }
        Ok(())
    }
    async fn ping_daemon(&self, entry: &Entry) -> Option<String> {
        let id = entry.borrow().record.borrow().id.clone();
        self.log.push(json!(["pingDaemon", id]));
        self.s.get("fault").and_then(Value::as_str).map(str::to_string)
    }
    fn sync_daemon(&self, entry: &Entry) {
        let id = entry.borrow().record.borrow().id.clone();
        self.log.push(json!(["syncDaemon", id]));
    }
    async fn read_memory(&self, record: &Rc<RefCell<WorkspaceRecord>>, _machine: &Arc<dyn Machine>, _sizes: &[WorkspaceSize]) -> bool {
        self.log.push(json!(["readMemory"]));
        match self.s.get("memory") {
            Some(m) if m.is_object() => {
                record.borrow_mut().size = WorkspaceSize { cpu: JsNumber(m["cpu"].as_f64().expect("cpu")), mem_mb: JsNumber(m["memMb"].as_f64().expect("memMb")) };
                true
            }
            _ => false,
        }
    }
    fn module_of(&self, kind: &str) -> Rc<dyn KindModule> {
        Rc::new(FakeModule { log: self.log.clone(), kind: kind.to_string(), land_fails: self.s.get("landFails").and_then(Value::as_bool).unwrap_or(false) })
    }
    async fn write_daemon_roots(&self, entry: &Entry) -> Result<(), HostError> {
        let id = entry.borrow().record.borrow().id.clone();
        self.log.push(json!(["writeDaemonRoots", id]));
        Ok(())
    }
    async fn persist(&self, record: &WorkspaceRecord) -> Result<(), HostError> {
        self.log.push(json!(["persist", serde_json::to_value(record).expect("a record serializes")]));
        Ok(())
    }
    async fn mark_used(&self, place_id: Option<&str>) -> Result<(), HostError> {
        self.log.push(json!(["markUsed", place_id]));
        Ok(())
    }
    fn view(&self, record: &WorkspaceRecord) -> WorkspaceView {
        view_json(json!({
            "id": record.id, "name": record.name, "machineId": record.machine_id, "phase": "running", "golden": record.golden,
            "createdAt": VIEW_MARK, "project": {"id": record.project, "name": "", "path": "", "computer": ""},
        }))
    }
    fn ref_of(&self, project: &ProjectView) -> ProjectRef {
        // The oracle's `refOf` answers `{id, name}`; the Rust ref also names a path and a computer, empty here.
        serde_json::from_value(json!({"id": project.id, "name": project.name, "path": "", "computer": ""})).expect("a project ref")
    }
}

pub fn view_json(v: Value) -> WorkspaceView {
    serde_json::from_value(v).expect("a workspace view parses")
}

/// A view of that id and name that `ctx.view` did not make.
pub fn view_of(id: &str, name: &str) -> WorkspaceView {
    view_json(json!({"id": id, "name": name, "machineId": "", "phase": "running", "golden": "", "createdAt": "", "project": {"id": "p", "name": "p", "path": "", "computer": ""}}))
}

/// The project the oracle's scene gives, as a Rust project view.
pub fn project_of(p: &Value) -> ProjectView {
    let mut v = json!({
        "id": "p_1", "name": "site", "computer": "here", "path": "/src/site", "remote": "git@github.com:acme/site.git",
        "memoryKey": "mk", "memoryDir": "/mem",
        "source": {"kind": "folder", "path": "/src/site"}, "defaultBranch": "main", "createdAt": "2026-01-01T00:00:00.000Z",
    });
    for (k, x) in p.as_object().into_iter().flatten() {
        v[k] = x.clone();
    }
    serde_json::from_value(v).expect("a project view parses")
}

/// `{name: "w1", ...s.o}` as the options the Rust create takes.
pub fn options_of(o: &Value) -> CreateWorkspaceOptions {
    let map = |k: &str| -> Option<IndexMap<String, String>> {
        o.get(k).and_then(Value::as_object).map(|m| m.iter().map(|(k, v)| (k.clone(), v.as_str().expect("a string").to_string())).collect())
    };
    // The oracle's `agents` patch names `machines`; the Rust patch names `maxMachines`.
    let agents = o.get("agents").and_then(Value::as_object).map(|a| a.iter().map(|(k, v)| (if k == "machines" { "maxMachines".to_string() } else { k.clone() }, v.clone())).collect::<Map<String, Value>>());
    CreateWorkspaceOptions {
        spec: WorkspaceSpec { cpu: o.get("cpu").and_then(Value::as_f64).map(JsNumber), mem_mb: o.get("memMb").and_then(Value::as_f64).map(JsNumber), envs: map("envs"), labels: map("labels"), binds: None, copy: None },
        project: "p_1".to_string(),
        golden: o.get("golden").and_then(Value::as_str).map(str::to_string),
        name: "w1".to_string(),
        sub: None,
        opens_thread: o.get("opensThread").and_then(Value::as_str).map(str::to_string),
        agents,
        idle_window_ms: o.get("idleWindowMs").and_then(Value::as_f64).map(|n| IdleWindow::Ms(JsNumber(n))).unwrap_or_default(),
        parent: o.get("parent").and_then(Value::as_str).map(str::to_string),
        // The oracle's branch of record has no repository id; the Rust one needs one.
        branch_of_record: o.get("branchOfRecord").map(|b| BranchOfRecord {
            repo_id: JsInt::from(GIVEN_REPO),
            owner: b["owner"].as_str().expect("owner").into(),
            name: b["name"].as_str().expect("name").into(),
            branch: b["branch"].as_str().expect("branch").into(),
            start_sha: None,
            public: None,
        }),
    }
}

pub fn scope_of(sp: &Value) -> ThreadScope {
    ThreadScope {
        kind: ThreadLit,
        thread_id: sp["threadId"].as_str().expect("threadId").to_string(),
        workspace_id: "ws_asker".to_string(),
        root_thread_id: sp["rootThreadId"].as_str().expect("rootThreadId").to_string(),
    }
}

/// A stage report that logs `["report", stage, message, said ?? null]` as the oracle's scene does.
pub fn logging_report(log: &Log) -> StageReport {
    let log = log.clone();
    Rc::new(move |stage: WorkspaceCreateStage, message: &str, said: Option<StageSaid>| {
        let stage = serde_json::to_value(stage).expect("a stage serializes");
        let said = said.map(|s| {
            let mut m = Map::new();
            if let Some(n) = s.notice {
                m.insert("notice".into(), json!(n));
            }
            if let Some(d) = s.detail {
                m.insert("detail".into(), json!(d));
            }
            if s.waiting {
                m.insert("waiting".into(), json!(true));
            }
            if let Some(b) = s.r#box {
                m.insert("box".into(), json!({"id": b.id, "state": b.state}));
            }
            Value::Object(m)
        });
        log.push(json!(["report", stage, message, said]));
    })
}

/// A fixture record as the Rust record writes it: the oracle's `agents` `{machines, threads}` are `{spawn, maxMachines,
/// maxDepth}` (the oracle's fake held `threads: 2`, which is `maxDepth`), the oracle's binds `{from, to}` are
/// `{source, target}`, the tree's `threadId` is the record's `parentThreadId`, and a branch of record holds a repository
/// id and no `public: false`.
pub fn rustify_record(rec: &mut Value) {
    let Some(m) = rec.as_object_mut() else { return };
    if let Some(a) = m.get("agents").cloned() {
        m.insert("agents".into(), json!({"spawn": true, "maxMachines": a["machines"], "maxDepth": a["threads"]}));
    }
    if let Some(t) = m.remove("threadId") {
        m.insert("parentThreadId".into(), t);
    }
    if let Some(binds) = m.get_mut("spec").and_then(|s| s.get_mut("binds")).and_then(Value::as_array_mut) {
        for b in binds {
            *b = json!({"source": b["from"], "target": b["to"]});
        }
    }
    if let Some(b) = m.get_mut("branchOfRecord").and_then(Value::as_object_mut) {
        b.remove("public");
        b.insert("repoId".into(), json!(if b["branch"] == "given" { GIVEN_REPO } else { NAMED_REPO }));
    }
}

/// The fixture's log as the Rust fakes write it.
pub fn expected_log(scene: &Value) -> Vec<Value> {
    scene["log"]
        .as_array()
        .expect("a log")
        .iter()
        .cloned()
        .map(|mut e| {
            if e[0] == "fork" || e[0] == "persist" {
                rustify_record(&mut e[1]);
            }
            e
        })
        .collect()
}

/// The oracle's scene as a world: the runtime, the area, the project.
pub struct World {
    pub log: Log,
    pub rt: Rc<FakeRuntime>,
    pub area: Rc<CreateArea>,
    pub clock: Rc<FakeClock>,
}

impl World {
    /// The runtime of `run(name, s)`: `s` is the scene's input, `{}` for the small facts.
    pub fn new(s: &Value) -> World {
        let log = Log::default();
        let clock = FakeClock::new(T0);
        let live: Rc<RefCell<IndexMap<String, Entry>>> = Rc::default();
        let mut holders = HashSet::new();
        for (i, o) in s.get("liveOthers").and_then(Value::as_array).into_iter().flatten().enumerate() {
            let id = format!("other{i}");
            let record = record_of(&json!({"id": id, "name": o["name"], "kind": "cloud", "project": "p"}));
            live.borrow_mut().insert(id.clone(), Rc::new(RefCell::new(live_of(record, plain_machine("m-other")))));
            if o["holds"] == true {
                holders.insert(id);
            }
        }
        let builders: Vec<KeptBuilder> = s
            .get("builders")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|b| KeptBuilder {
                id: b["id"].as_str().expect("id").into(),
                name: b["name"].as_str().expect("name").into(),
                life: match b["life"].as_str().expect("life") {
                    "own" => BuilderLife::Own,
                    "reusable" => BuilderLife::Reusable,
                    "stale" => BuilderLife::Stale,
                    "foreign" => BuilderLife::Foreign,
                    "held" => BuilderLife::Held,
                    other => panic!("a builder life the oracle never writes: {other}"),
                },
                sealed_version: b.get("sealed").and_then(Value::as_u64).map(|n| n as u32),
                place: b.get("place").and_then(Value::as_str).map(str::to_string),
            })
            .collect();
        let rt = Rc::new(FakeRuntime {
            log: log.clone(),
            clock: clock.clone(),
            ids: Rc::new(FakeIds::default()),
            live,
            s: s.clone(),
            project: project_of(s.get("project").unwrap_or(&json!({}))),
            backend: Arc::new(FakeBackend),
            forks: RefCell::new(s.get("forks").and_then(Value::as_array).map(|f| f.iter().map(|x| x.as_str().expect("a fork").to_string()).collect()).unwrap_or_else(|| VecDeque::from(["ok".to_string()]))),
            builders: RefCell::new(builders),
            grace: RefCell::new(IndexSet::from(["b1".to_string()])),
            holders,
            area: RefCell::new(Weak::new()),
            bound: RefCell::new(vec![]),
        });
        let area = CreateArea::new(rt.clone());
        *rt.area.borrow_mut() = Rc::downgrade(&area);
        World { log, rt, area, clock }
    }

    /// The oracle's `small()`: the live entries a, b (deleting), c (a folder) and d, and nothing else.
    pub fn small() -> World {
        let w = World::new(&json!({}));
        for (id, name, kind, project) in [("a", "taken", "cloud", "p1"), ("b", "going", "cloud", "p1"), ("c", "folder", "local", "p1"), ("d", "other", "local", "p2")] {
            let record = record_of(&json!({"id": id, "name": name, "kind": kind, "project": project}));
            let entry: Entry = Rc::new(RefCell::new(live_of(record, plain_machine("m"))));
            if id == "b" {
                entry.borrow_mut().deleting = Some(futures::future::ready(Ok(())).boxed_local().shared());
            }
            w.rt.live.borrow_mut().insert(id.to_string(), entry);
        }
        w
    }

    /// The act of every scene: `createStaged({name: "w1", ...s.o}, project, "ws_1", report, s.spawned, landed, s.childBase)`.
    pub async fn create(&self) -> Result<CreatedWorkspace, HostError> {
        let s = &self.rt.s;
        let o = options_of(s.get("o").unwrap_or(&json!({})));
        let spawned = s.get("spawned").map(scope_of);
        let log = self.log.clone();
        let landed: Box<dyn Fn()> = Box::new(move || log.push(json!(["landed"])));
        self.area
            .create_staged(&o, &self.rt.project, "ws_1", logging_report(&self.log), spawned.as_ref(), Some(landed), s.get("childBase").and_then(Value::as_str))
            .await
    }

    pub fn entry(&self, id: &str) -> Option<Entry> {
        self.rt.live.borrow().get(id).cloned()
    }
}
