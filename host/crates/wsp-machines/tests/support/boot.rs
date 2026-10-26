// SPDX-License-Identifier: AGPL-3.0-only
//! The fakes of the boot area's tests: one recording `BootRuntime` that logs the calls its oracle's recording context
//! logs (`oracle/boot_*.oracle.ts`), a fake provider whose reads answer what the oracles' mocks answered, and the
//! helpers that put a fixture's records into the typed shapes and back. Included with
//! `#[path = "support/boot.rs"] mod support;` beside `mod common;`.
#![allow(dead_code, unused_imports, clippy::type_complexity)]

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::{Rc, Weak};
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::{LocalBoxFuture, Shared};
use futures::FutureExt;
use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Map, Value};

use wsp_adapters::port::ExecStreamFactory;
use wsp_core::ids::Ids;
use wsp_core::types::internal::{
    BuilderLife, BuilderRecord, EndThread, FolderAdmit, KindModule, PortRoute, Reopened,
    ServerValuesRoad, ThreadRecord, TreeTalk,
};
use wsp_core::types::internal::{ImportLanded, ImportReport};
use wsp_core::types::pending::{Clock, MachineExecOptions, ScopedRoad, Store, TurnWaiting};
use wsp_core::types::wiring::{
    GoneOutcome, LiveWorkspace, Local, PlaceBackends, ProjectImportOptions, StageReport,
    WorkspaceRecord,
};
use wsp_engine::errors::{EngineError, ErrorKind, WspError};
use wsp_engine::lifecycle::{Workspace, WorkspaceHooks, WorkspaceInitial};
use wsp_engine::machine::{
    BackendPricing, Capabilities, ExecOptions, ExecResult, Machine, MachineBackend, MachineKind,
    MachineLife, MachineListRow, MachineShape, MachineSpec, RunOptions, Seen, Size,
    SnapshotOptions, SnapshotStoragePricing,
};
use wsp_engine::Cancel;
use wsp_machines::machines::boot::entries::{EntryAsk, RowAt};
use wsp_machines::machines::boot::rows::Stamped;
use wsp_machines::machines::boot::{
    BootArea, BootConfig, BootRuntime, CardAsk, ComputerRow, HeldEnd, LimitRow, ScopedDevice,
};
use wsp_machines::machines::machines::{Entry, Row};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::harness::McpServerSpec;
use wsp_wire::views::place_kind::PlaceKind;
use wsp_wire::views::project_bundle::*;
use wsp_wire::views::session::{SessionView, ThreadPlacement};
use wsp_wire::views::sub_row::SubRow;
use wsp_wire::views::workspace::{Caller, MachineState, ProjectView};
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin};
use wsp_wire::wire::capabilities::WorkspaceSize;
use wsp_wire::wire::events_union::EventUnion;
use wsp_words::exit::{HostError, Kind};

use crate::common::{self, FakeClock, FakeIds, Log, MemStore, StoreLogs, T0};

/// How many reads in a row must answer gone before a provider read says gone (engine golden.ts:96 `GONE_READS`).
pub const GONE_READS: usize = 12;

// --- the log -----------------------------------------------------------------------------------------------------

thread_local! {
    static CUR: RefCell<Option<Log>> = const { RefCell::new(None) };
    static HUNG: RefCell<Vec<futures::channel::oneshot::Sender<()>>> = const { RefCell::new(Vec::new()) };
    static KIND_UNSET: RefCell<HashSet<usize>> = RefCell::new(HashSet::new());
}

/// Every integral float as the integer a fixture reads as, recursively; object key order kept.
pub fn norm(v: Value) -> Value {
    match v {
        Value::Number(n) => match n.as_f64() {
            Some(f) if n.is_f64() && f.fract() == 0.0 && f.abs() < 9.0e15 => json!(f as i64),
            _ => Value::Number(n),
        },
        Value::Array(a) => Value::Array(a.into_iter().map(norm).collect()),
        Value::Object(o) => Value::Object(o.into_iter().map(|(k, x)| (k, norm(x))).collect()),
        other => other,
    }
}

fn put(entry: Value) {
    CUR.with(|c| {
        c.borrow()
            .as_ref()
            .expect("a world is up")
            .push(norm(entry))
    });
}

pub fn to_json<T: serde::Serialize>(v: &T) -> Value {
    norm(serde_json::to_value(v).expect("serializes"))
}

async fn tick() {
    tokio::task::yield_now().await;
}

/// Releases the oldest read a `hang` answer holds (the oracles' `pending.shift()()`).
pub fn release_one() {
    HUNG.with(|h| {
        let tx = h.borrow_mut().remove(0);
        let _ = tx.send(());
    });
}

// --- fixture records in typed shapes -----------------------------------------------------------------------------

const WS_FILL: [(&str, fn() -> Value); 4] = [
    ("createdAt", || json!("")),
    ("spec", || json!({})),
    ("size", || json!({"cpu": 1, "memMb": 1})),
    ("firstLife", || json!(true)),
];

/// A fixture workspace record with the keys the typed record needs and the oracles never wrote.
pub fn fill_workspace(v: &Value) -> Value {
    let mut o = v.as_object().expect("a record").clone();
    for (k, d) in WS_FILL {
        o.entry(k.to_string()).or_insert_with(d);
    }
    if let Some(Value::Object(sub)) = o.get_mut("sub") {
        for k in SUB_FILL {
            sub.entry(k.to_string()).or_insert_with(|| json!(""));
        }
        sub.entry("base".to_string())
            .or_insert_with(|| json!({"branch": "", "sha": ""}));
    }
    Value::Object(o)
}

const SUB_FILL: [&str; 4] = ["on", "repo", "branch", "folder"];

/// The record as the oracle wrote it: the keys `fill_workspace` adds taken off where they still hold the fill.
pub fn strip_workspace(v: Value) -> Value {
    let Value::Object(mut o) = v else { return v };
    for (k, d) in WS_FILL {
        if o.get(k) == Some(&d()) {
            o.shift_remove(k);
        }
    }
    if let Some(Value::Object(sub)) = o.get_mut("sub") {
        for k in SUB_FILL {
            if sub.get(k) == Some(&json!("")) {
                sub.shift_remove(k);
            }
        }
        if sub.get("base") == Some(&json!({"branch": "", "sha": ""})) {
            sub.shift_remove("base");
        }
    }
    Value::Object(o)
}

pub fn workspace_record(v: &Value) -> WorkspaceRecord {
    serde_json::from_value(fill_workspace(v))
        .unwrap_or_else(|e| panic!("workspace record {v}: {e}"))
}

pub fn ws(id: &str, o: Value) -> Value {
    let mut r = json!({"id": id, "name": format!("w-{id}"), "kind": "cloud", "machineId": format!("m-{id}"), "phase": "running", "golden": "img", "project": "p"});
    for (k, x) in o.as_object().into_iter().flatten() {
        r[k] = x.clone();
    }
    r
}

const PROJECT_FILL: [&str; 5] = [
    "remote",
    "defaultBranch",
    "memoryKey",
    "memoryDir",
    "createdAt",
];

pub fn fill_project(v: &Value) -> Value {
    let mut o = v.as_object().expect("a project").clone();
    for k in PROJECT_FILL {
        o.entry(k.to_string()).or_insert_with(|| json!(""));
    }
    o.entry("path".to_string()).or_insert_with(|| json!(""));
    let path = o["path"].clone();
    if let Some(Value::Object(src)) = o.get_mut("source") {
        match src.get("kind").and_then(Value::as_str) {
            Some("folder") => {
                src.entry("path".to_string()).or_insert(path);
            }
            Some("github") => {
                src.entry("repo".to_string()).or_insert_with(|| json!(""));
            }
            _ => {}
        }
    }
    Value::Object(o)
}

pub fn strip_project(v: Value) -> Value {
    let Value::Object(mut o) = v else { return v };
    for k in PROJECT_FILL {
        if o.get(k) == Some(&json!("")) {
            o.shift_remove(k);
        }
    }
    let path = o.get("path").cloned();
    if let Some(Value::Object(src)) = o.get_mut("source") {
        if src.get("kind") == Some(&json!("folder")) && src.get("path") == path.as_ref() {
            src.shift_remove("path");
        }
        if src.get("kind") == Some(&json!("github")) && src.get("repo") == Some(&json!("")) {
            src.shift_remove("repo");
        }
    }
    Value::Object(o)
}

/// What the oracle's `world` gave `projectsHeld`.
pub fn project_p1() -> ProjectView {
    serde_json::from_value(fill_project(
        &json!({"id": "p1", "name": "site", "computer": "hetzner", "source": {"kind": "folder"}}),
    ))
    .expect("project")
}

pub fn view_of(v: &Value) -> SessionView {
    let mut v = v.clone();
    if let Some(Value::Number(n)) = v.get("contextWindow").cloned() {
        v["contextWindow"] = json!(n.to_string());
    }
    serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("session view {v}: {e}"))
}

/// A row as the rows oracle's `world` makes it: `{view: {harness: "claude", status: "completed", ...view}, turnId, pid?, run?, replaces?}`.
pub fn row_of(r: &Value) -> Row {
    let mut view = json!({"harness": "claude", "status": "completed"});
    for (k, x) in r["view"].as_object().expect("a view") {
        view[k] = x.clone();
    }
    let id = view["id"].as_str().expect("an id").to_string();
    Rc::new(RefCell::new(entry_of_view(
        view_of(&view),
        r["turnId"]
            .as_str()
            .map(str::to_string)
            .unwrap_or(format!("turn-{id}")),
        r,
    )))
}

fn entry_of_view(
    view: SessionView,
    turn_id: String,
    r: &Value,
) -> wsp_core::types::internal::SessionEntry {
    wsp_core::types::internal::SessionEntry {
        view,
        turn_id,
        replaces: r["replaces"].as_str().map(str::to_string),
        notify: None,
        notify_by: None,
        notify_road: None,
        turn_token: None,
        scope_device_id: None,
        handle: None,
        end: None,
        turn_live: None,
        run: r["run"].as_str().map(str::to_string),
        from: None,
        asked: None,
        snapshot: None,
        pid: r["pid"].as_u64().map(|p| p as u32),
        launch: None,
        calls: None,
        unanswered: false,
        queued: false,
    }
}

/// A row as the store's after-map reads it: the oracle's `Object.entries(s)` less `end`.
pub fn row_json(row: &Row) -> Value {
    let s = row.borrow();
    let mut o = Map::new();
    o.insert("view".into(), to_json(&s.view));
    o.insert("turnId".into(), json!(s.turn_id));
    if let Some(n) = &s.notify {
        o.insert("notify".into(), json!(n));
    }
    if let Some(b) = &s.notify_by {
        o.insert("notifyBy".into(), to_json(b));
    }
    if let Some(r) = &s.notify_road {
        o.insert("notifyRoad".into(), to_json(r));
    }
    if let Some(t) = &s.turn_live {
        let mut l = Map::new();
        if let Some(r) = &t.reply {
            l.insert("reply".into(), to_json(r));
        }
        if let Some(x) = t.told {
            l.insert("told".into(), json!(x));
        }
        if let Some(x) = &t.steered {
            l.insert("steered".into(), to_json(x));
        }
        if let Some(x) = t.waited {
            l.insert("waited".into(), json!(x));
        }
        if let Some(x) = t.waiting_since {
            l.insert("waitingSince".into(), json!(x));
        }
        o.insert("turnLive".into(), Value::Object(l));
    }
    if let Some(x) = &s.run {
        o.insert("run".into(), json!(x));
    }
    if let Some(x) = s.from {
        o.insert("from".into(), json!(x));
    }
    if let Some(x) = &s.asked {
        o.insert("asked".into(), to_json(x));
    }
    if let Some(x) = &s.turn_token {
        o.insert("turnToken".into(), json!(x));
    }
    if let Some(x) = &s.scope_device_id {
        o.insert("scopeDeviceId".into(), json!(x));
    }
    if let Some(x) = &s.snapshot {
        o.insert("snapshot".into(), json!(x));
    }
    if s.unanswered {
        o.insert("unanswered".into(), json!(true));
    }
    norm(Value::Object(o))
}

pub fn thread_scope(v: &Value) -> ThreadScope {
    serde_json::from_value(v.clone()).expect("a thread scope")
}

/// The thread caller the rows oracle names: `{origin: "here", by: {kind: "thread", threadId: "t-a", workspaceId: "a", rootThreadId: "t-a"}}`.
pub fn thread_caller() -> Caller {
    Caller::Scoped {
        origin: WorkspaceOrigin::Here,
        by: thread_scope(
            &json!({"kind": "thread", "threadId": "t-a", "workspaceId": "a", "rootThreadId": "t-a"}),
        ),
    }
}

pub fn caller_json(c: Option<&Caller>) -> Value {
    match c {
        None => Value::Null,
        Some(Caller::Road(o)) => to_json(o),
        Some(Caller::Scoped { origin, by }) => {
            json!({"origin": to_json(origin), "by": to_json(by)})
        }
    }
}

/// The kind word of an error the boot area stamps.
pub fn kind_word(e: &HostError) -> Value {
    match &e.kind {
        Some(Kind::NotFound) => json!("not-found"),
        Some(Kind::Invalid) => json!("invalid"),
        Some(Kind::Usage) => json!("usage"),
        Some(_) => json!("other"),
        None => Value::Null,
    }
}

pub fn entry_id(e: &Entry) -> String {
    e.borrow().record.borrow().id.clone()
}

/// A live workspace on `machine` with nothing in flight.
pub fn live_workspace(record: WorkspaceRecord, machine: Arc<dyn Machine>) -> Entry {
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
    let ws = Workspace::new(machine.clone(), hooks, WorkspaceInitial::default());
    Rc::new(RefCell::new(LiveWorkspace {
        record: Rc::new(RefCell::new(record)),
        opens_thread: None,
        of_record_held: false,
        ws: Rc::new(ws),
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

// --- machines and the provider -----------------------------------------------------------------------------------

pub fn state_of(word: &str) -> MachineState {
    serde_json::from_value(json!(word)).unwrap_or_else(|e| panic!("machine state {word}: {e}"))
}

pub struct FakeMachine {
    pub id: String,
    pub kind: MachineKind,
    pub labels: Option<IndexMap<String, String>>,
    pub seen: Option<Seen>,
    pub stream_url: Option<String>,
    pub state: Result<MachineState, String>,
    pub log_state: bool,
}

impl FakeMachine {
    pub fn plain(id: &str) -> Arc<dyn Machine> {
        Arc::new(FakeMachine {
            id: id.to_string(),
            kind: MachineKind::Sandbox,
            labels: None,
            seen: None,
            stream_url: None,
            state: Ok(state_of("running")),
            log_state: false,
        })
    }

    /// A machine the oracle gave no `kind`: its attach logs `null`.
    pub fn without_kind(id: &str, state: &str) -> Arc<dyn Machine> {
        let m: Arc<dyn Machine> = Arc::new(FakeMachine {
            id: id.to_string(),
            kind: MachineKind::Sandbox,
            labels: None,
            seen: Some(Seen {
                state: state_of(state),
                created_at: None,
                paused_at: None,
            }),
            stream_url: None,
            state: Ok(state_of(state)),
            log_state: false,
        });
        KIND_UNSET.with(|k| k.borrow_mut().insert(Arc::as_ptr(&m) as *const () as usize));
        m
    }
}

pub fn kind_json(m: &Arc<dyn Machine>) -> Value {
    if KIND_UNSET.with(|k| k.borrow().contains(&(Arc::as_ptr(m) as *const () as usize))) {
        Value::Null
    } else {
        to_json(&m.kind())
    }
}

#[async_trait(?Send)]
impl Machine for FakeMachine {
    fn id(&self) -> &str {
        &self.id
    }
    fn kind(&self) -> MachineKind {
        self.kind
    }
    fn stream_url(&self) -> Option<&str> {
        self.stream_url.as_deref()
    }
    fn labels(&self) -> Option<&IndexMap<String, String>> {
        self.labels.as_ref()
    }
    fn seen(&self) -> Option<&Seen> {
        self.seen.as_ref()
    }
    async fn exec(&self, _cmd: &str, _opts: ExecOptions) -> wsp_engine::errors::Result<ExecResult> {
        unimplemented!()
    }
    async fn run(
        &self,
        _script: &str,
        _opts: RunOptions,
    ) -> wsp_engine::errors::Result<ExecResult> {
        unimplemented!()
    }
    async fn snapshot(
        &self,
        _name: &str,
        _life: MachineLife,
        _opts: SnapshotOptions,
    ) -> wsp_engine::errors::Result<String> {
        unimplemented!()
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> wsp_engine::errors::Result<()> {
        unimplemented!()
    }
    async fn resume(&self, _signal: Option<&Cancel>) -> wsp_engine::errors::Result<()> {
        unimplemented!()
    }
    async fn kill(&self) -> wsp_engine::errors::Result<()> {
        unimplemented!()
    }
    async fn state(&self) -> wsp_engine::errors::Result<MachineState> {
        if self.log_state {
            put(json!(["machine.state", self.id]));
        }
        match &self.state {
            Ok(s) => Ok(*s),
            Err(m) => Err(EngineError::Other {
                message: m.clone(),
                code: None,
            }),
        }
    }
    async fn download_url(&self, _path: &str) -> wsp_engine::errors::Result<String> {
        unimplemented!()
    }
    async fn upload_url(&self, _path: &str) -> wsp_engine::errors::Result<String> {
        unimplemented!()
    }
}

/// What a provider says to a read of one machine: the oracles' `Answer`.
#[derive(Clone, Default, Debug)]
pub struct Answer {
    pub state: Option<String>,
    pub labels: Option<IndexMap<String, String>>,
    pub seen: Option<Value>,
    pub stream_url: Option<String>,
    pub throws: Option<Value>,
    pub hang: bool,
    pub state_throws: bool,
}

impl Answer {
    pub fn from_json(v: &Value) -> Answer {
        Answer {
            state: v["state"].as_str().map(str::to_string),
            labels: v["labels"].as_object().map(|o| {
                o.iter()
                    .map(|(k, x)| (k.clone(), x.as_str().unwrap_or_default().to_string()))
                    .collect()
            }),
            seen: v.get("seen").cloned(),
            stream_url: v["streamUrl"].as_str().map(str::to_string),
            throws: v.get("throws").cloned(),
            hang: v["hang"].as_bool().unwrap_or(false),
            state_throws: v["stateThrows"].as_bool().unwrap_or(false),
        }
    }
}

/// The provider error a scene's `throws` names: `Object.assign(new Error(message), throws)`.
pub fn error_of(throws: &Value) -> EngineError {
    let message = throws["message"].as_str().unwrap_or("refused").to_string();
    match throws["kind"].as_str() {
        Some("missing") => EngineError::Provider(WspError {
            kind: ErrorKind::Missing,
            status: throws["status"].as_u64().unwrap_or(404) as u16,
            code: None,
            message,
            request_id: None,
            window: None,
            said: None,
        }),
        Some("absent") => EngineError::PlaceAbsent { message },
        Some("noProvider") => EngineError::NoProvider { line: message },
        _ => EngineError::Other {
            message,
            code: None,
        },
    }
}

/// What the oracles' mocked `readGone` and `sightMachine` answer, in the order they are asked.
#[derive(Clone, Debug)]
pub enum Sight {
    Gone,
    Running,
    Found { machine: String, state: String },
    Fails(String),
}

pub struct BackendShared {
    pub mode: Mode,
    pub answers: IndexMap<String, Answer>,
    pub snapshots_any_life: bool,
    /// The word the oracle's mock logs for a read made to look at a machine, `readGone` or `sightMachine`.
    pub sight_word: &'static str,
    pub sights: VecDeque<Sight>,
    /// Whether the first read of a record is the load's own (`backend.get`); a read made first by a sighting is not.
    pub first_read_is_load: bool,
    reads: HashMap<String, usize>,
    runs: HashMap<String, (usize, Sight)>,
}

pub struct FakeBackend {
    pub place: String,
    pub shared: Rc<RefCell<BackendShared>>,
}

struct Pricing;

impl BackendPricing for Pricing {
    fn rate_usd_per_hour(&self, _size: Size) -> f64 {
        0.0
    }
    fn default_size(&self) -> Size {
        Size {
            cpu: 1.0,
            mem_mb: 2048.0,
        }
    }
    fn snapshot_storage(&self) -> SnapshotStoragePricing {
        unimplemented!()
    }
}

fn machine_for(shared: &BackendShared, id: &str, a: &Answer) -> Arc<dyn Machine> {
    let ready_default = if shared.mode == Mode::Ready {
        Some(json!({"state": "running"}))
    } else {
        None
    };
    let seen = a.seen.as_ref().or(ready_default.as_ref()).map(|s| Seen {
        state: state_of(s["state"].as_str().unwrap_or("running")),
        created_at: s["createdAt"].as_str().map(str::to_string),
        paused_at: s["pausedAt"].as_str().map(str::to_string),
    });
    let state = if a.state_throws {
        Err("state read failed".to_string())
    } else {
        Ok(state_of(a.state.as_deref().unwrap_or("running")))
    };
    Arc::new(FakeMachine {
        id: id.to_string(),
        kind: MachineKind::Sandbox,
        labels: a.labels.clone(),
        seen,
        stream_url: a.stream_url.clone(),
        state,
        log_state: shared.mode == Mode::Load,
    })
}

#[async_trait(?Send)]
impl MachineBackend for FakeBackend {
    fn capabilities(&self) -> Capabilities {
        let any = self.shared.borrow_mut().snapshots_any_life;
        serde_json::from_value(json!({
            "liveCloneForks": false, "replacesMachine": false, "previewUrls": false, "signedUrls": false, "callbackRelay": false,
            "diskSnapshots": false, "images": false, "snapshotsAnyLife": any, "snapshotListing": false, "templates": false,
            "kept": false, "copies": false, "ownNetwork": false, "sizes": []
        }))
        .expect("capabilities")
    }

    fn pricing(&self) -> Arc<dyn BackendPricing> {
        Arc::new(Pricing)
    }

    async fn create(
        &self,
        _spec: MachineSpec,
        _waiting: Option<wsp_engine::machine::Waiting>,
        _sent: Option<String>,
    ) -> wsp_engine::errors::Result<Arc<dyn Machine>> {
        unimplemented!()
    }

    async fn get(&self, id: &str) -> wsp_engine::errors::Result<Arc<dyn Machine>> {
        enum Step {
            Load { hang: bool, answer: Answer },
            Sight(Result<Arc<dyn Machine>, EngineError>),
        }
        let step = {
            let mut s = self.shared.borrow_mut();
            let n = {
                let c = s.reads.entry(id.to_string()).or_insert(0);
                *c += 1;
                *c
            };
            if n == 1 && s.first_read_is_load {
                let a = s.answers.get(id).cloned().unwrap_or_default();
                put(match s.mode {
                    Mode::Load => json!(["backend.get", self.place, id]),
                    _ => json!(["backend.get", id]),
                });
                Step::Load {
                    hang: a.hang,
                    answer: a,
                }
            } else {
                let open = s.runs.get(id).cloned();
                let (count, sight) = match open {
                    Some((c, sight)) => (c, sight),
                    None => {
                        let word = s.sight_word;
                        put(json!([word, id]));
                        let sight = s.sights.pop_front().unwrap_or(Sight::Gone);
                        (0, sight)
                    }
                };
                let missing = || {
                    EngineError::Provider(WspError {
                        kind: ErrorKind::Missing,
                        status: 404,
                        code: None,
                        message: "gone".into(),
                        request_id: None,
                        window: None,
                        said: None,
                    })
                };
                let found = |state: &str| {
                    Arc::new(FakeMachine {
                        id: id.to_string(),
                        kind: MachineKind::Sandbox,
                        labels: None,
                        seen: Some(Seen {
                            state: state_of(state),
                            created_at: None,
                            paused_at: None,
                        }),
                        stream_url: None,
                        state: Ok(state_of(state)),
                        log_state: false,
                    }) as Arc<dyn Machine>
                };
                let out = match &sight {
                    Sight::Gone => {
                        if count + 1 >= GONE_READS {
                            s.runs.remove(id);
                        } else {
                            s.runs.insert(id.to_string(), (count + 1, sight.clone()));
                        }
                        Err(missing())
                    }
                    Sight::Running => {
                        s.runs.remove(id);
                        Ok(found("running"))
                    }
                    Sight::Found { machine, state } => {
                        s.runs.remove(id);
                        let m = Arc::new(FakeMachine {
                            id: machine.clone(),
                            kind: MachineKind::Sandbox,
                            labels: None,
                            seen: Some(Seen {
                                state: state_of(state),
                                created_at: None,
                                paused_at: None,
                            }),
                            stream_url: None,
                            state: Ok(state_of(state)),
                            log_state: false,
                        }) as Arc<dyn Machine>;
                        KIND_UNSET
                            .with(|k| k.borrow_mut().insert(Arc::as_ptr(&m) as *const () as usize));
                        Ok(m)
                    }
                    Sight::Fails(m) => {
                        s.runs.remove(id);
                        Err(EngineError::Other {
                            message: m.clone(),
                            code: None,
                        })
                    }
                };
                Step::Sight(out)
            }
        };
        match step {
            Step::Sight(r) => r,
            Step::Load { hang, answer } => {
                if hang {
                    let (tx, rx) = futures::channel::oneshot::channel::<()>();
                    HUNG.with(|h| h.borrow_mut().push(tx));
                    let _ = rx.await;
                }
                if let Some(t) = &answer.throws {
                    return Err(error_of(t));
                }
                let s = self.shared.borrow_mut();
                Ok(machine_for(&s, id, &answer))
            }
        }
    }

    async fn list(
        &self,
        _labels: Option<&IndexMap<String, String>>,
    ) -> wsp_engine::errors::Result<Vec<MachineListRow>> {
        unimplemented!()
    }

    async fn delete_snapshot(&self, _id: &str) -> wsp_engine::errors::Result<()> {
        unimplemented!()
    }
}

pub struct FakePlaces {
    pub wired: String,
    pub shared: Rc<RefCell<BackendShared>>,
    pub no_backend_at: Vec<String>,
    pub backends: bool,
}

impl PlaceBackends for FakePlaces {
    fn wired(&self) -> String {
        self.wired.clone()
    }
    fn backend(&self, place: &str) -> Option<Arc<dyn MachineBackend>> {
        if !self.backends || self.no_backend_at.iter().any(|p| p == place) {
            return None;
        }
        Some(Arc::new(FakeBackend {
            place: place.to_string(),
            shared: self.shared.clone(),
        }))
    }
    fn list(&self) -> Vec<String> {
        vec![self.wired.clone()]
    }
}

struct FakeModule {
    backend: Arc<dyn MachineBackend>,
}

impl KindModule for FakeModule {
    fn backend(&self, _record: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError> {
        Ok(self.backend.clone())
    }
    fn exec_stream(
        &self,
        _entry: &LiveWorkspace,
        _opts: Option<MachineExecOptions>,
        _waiting: Option<TurnWaiting>,
    ) -> Arc<dyn ExecStreamFactory> {
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
    fn roots(&self, _entry: &LiveWorkspace, _dests: &[String]) -> Local<'static, ()> {
        unimplemented!()
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
    fn has_daemon(&self, _entry: &LiveWorkspace) -> bool {
        unimplemented!()
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

// --- the runtime ---------------------------------------------------------------------------------------------------

/// Which oracle's recording context a world stands for: the three differ in what they log and what they answer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Load,
    Ready,
    Rows,
}

/// Puts a store's writes through `norm`, so an integral float is the integer a fixture reads.
struct NormStore(Rc<MemStore>);

#[async_trait(?Send)]
impl Store for NormStore {
    async fn get(&self, c: &str, id: &str) -> Result<Option<Value>, HostError> {
        self.0.get(c, id).await
    }
    async fn put(&self, c: &str, id: &str, v: Value) -> Result<(), HostError> {
        self.0.put(c, id, norm(v)).await
    }
    async fn list(&self, c: &str) -> Result<Vec<Value>, HostError> {
        self.0.list(c).await
    }
    async fn keys(&self, c: &str) -> Result<Vec<String>, HostError> {
        self.0.keys(c).await
    }
    async fn delete(&self, c: &str, id: &str) -> Result<(), HostError> {
        self.0.delete(c, id).await
    }
    async fn get_blob(&self, c: &str, id: &str) -> Result<Option<Vec<u8>>, HostError> {
        self.0.get_blob(c, id).await
    }
    async fn put_blob(&self, c: &str, id: &str, b: Vec<u8>) -> Result<(), HostError> {
        self.0.put_blob(c, id, b).await
    }
    async fn delete_blob(&self, c: &str, id: &str) -> Result<(), HostError> {
        self.0.delete_blob(c, id).await
    }
    async fn stat_blob(
        &self,
        c: &str,
        id: &str,
    ) -> Result<Option<wsp_core::types::pending::BlobMark>, HostError> {
        self.0.stat_blob(c, id).await
    }
}

/// What a scene gives its world, each field the oracle's option of the same meaning.
#[derive(Default)]
pub struct Opts {
    pub store: Value,
    pub store_override: Option<Rc<dyn Store>>,
    pub answers: Value,
    pub no_backend_at: Vec<&'static str>,
    pub gone: Vec<&'static str>,
    pub sights: Vec<Sight>,
    pub sight_word: Option<&'static str>,
    pub sight_first: bool,
    pub snapshots_any_life: bool,
    pub no_local: bool,
    pub states: Vec<(&'static str, &'static str)>,
    pub reattach: Vec<(&'static str, &'static str)>,
    pub devices: Value,
    pub shells: Vec<&'static str>,
    pub running: Vec<&'static str>,
    pub live: Value,
    pub rows: Value,
    pub records: Value,
    pub reads_since: Option<f64>,
    pub hostname: Option<String>,
    pub listener: bool,
}

pub struct Fake {
    pub mode: Mode,
    pub log: Log,
    /// Calls the oracle's recording missed or made no part of its log (named at each use).
    pub extras: Log,
    pub clock: Rc<FakeClock>,
    pub ids: Rc<FakeIds>,
    pub store: Rc<dyn Store>,
    pub mem: Rc<MemStore>,
    pub places: Rc<FakePlaces>,
    pub shared: Rc<RefCell<BackendShared>>,
    pub owner: RefCell<String>,
    pub reads_since: Cell<f64>,
    pub closed: Cell<bool>,
    pub closing: Cell<bool>,
    pub hostname: String,
    pub local: bool,
    pub live: Rc<RefCell<IndexMap<String, Entry>>>,
    pub sessions: Rc<RefCell<IndexMap<String, Row>>>,
    pub thread_records: Rc<RefCell<IndexMap<String, ThreadRecord>>>,
    pub builders: wsp_machines::images::Builders,
    pub projects_held: Rc<RefCell<IndexMap<String, ProjectView>>>,
    pub boot_work: Rc<RefCell<IndexMap<String, Shared<LocalBoxFuture<'static, ()>>>>>,
    pub rereading: Rc<RefCell<IndexSet<String>>>,
    pub viewed_marks: Rc<RefCell<IndexMap<String, IndexMap<String, String>>>>,
    pub indexes: RefCell<IndexSet<String>>,
    pub index_pos: RefCell<IndexMap<String, u64>>,
    pub index_children: RefCell<IndexMap<(String, String), Vec<Value>>>,
    pub area: RefCell<Weak<BootArea>>,
    pub wire_listener: bool,
    pub reattach_answers: RefCell<IndexMap<String, String>>,
    pub device_list: RefCell<Vec<ScopedDevice>>,
    pub shells: RefCell<Vec<String>>,
    pub running: RefCell<Vec<String>>,
    pub ends: RefCell<Vec<HeldEnd>>,
    pub pids: RefCell<IndexMap<String, u32>>,
    pub cap_waits: RefCell<IndexMap<String, Value>>,
    pub behind: RefCell<IndexMap<String, Value>>,
    pub restarts: RefCell<IndexMap<String, String>>,
    pub setup_places: RefCell<IndexMap<String, String>>,
    pub setup_refusals: RefCell<IndexMap<(String, String), String>>,
    pub latest_on: RefCell<IndexMap<String, SessionView>>,
    pub rows_on: RefCell<IndexMap<String, Vec<SessionView>>>,
    pub sub_rows: RefCell<IndexMap<String, SubRow>>,
    pub kept_until: RefCell<IndexMap<String, f64>>,
    pub place_ids: RefCell<IndexMap<String, String>>,
    pub lead_asks: RefCell<IndexSet<String>>,
    pub limits: RefCell<Vec<LimitRow>>,
    pub armed_timers: RefCell<IndexSet<String>>,
    pub door_rows: RefCell<Option<Vec<ComputerRow>>>,
    pub default_head: RefCell<Option<String>>,
    pub places_for: RefCell<IndexMap<String, Option<String>>>,
    pub talks_to_tree: Cell<bool>,
    pub title_rows_of: RefCell<Vec<SessionView>>,
    pub resume_fails: RefCell<Option<String>>,
    /// Runs when `sub_read` is called, to move what the read would have moved.
    pub on_sub_read: RefCell<Option<Box<dyn Fn()>>>,
    /// How many `pass` calls still fail (`bus down`) before one succeeds.
    pub fail_pass: Cell<usize>,
    /// When set, `backend_for` answers this error (the TS `backendFor` throws for a kind with no module).
    pub backend_for_failure: RefCell<Option<HostError>>,
    /// When set, `module_of` answers this error (the TS `moduleOf` throws for a kind it does not wire).
    pub module_of_failure: RefCell<Option<HostError>>,
    /// When set, `talks_to_its_tree` answers this error (the TS `boxOf` reads `projectHeld` of the caller's workspace).
    pub talks_to_tree_failure: RefCell<Option<HostError>>,
}

pub struct World {
    pub rt: Rc<Fake>,
    pub area: Rc<BootArea>,
    pub log: Log,
    pub clock: Rc<FakeClock>,
}

impl World {
    pub fn load(o: Opts) -> World {
        World::new(Mode::Load, o)
    }
    pub fn ready(o: Opts) -> World {
        World::new(Mode::Ready, o)
    }
    pub fn rows(o: Opts) -> World {
        World::new(Mode::Rows, o)
    }

    pub fn new(mode: Mode, o: Opts) -> World {
        let log = Log::default();
        CUR.with(|c| *c.borrow_mut() = Some(log.clone()));
        HUNG.with(|h| h.borrow_mut().clear());
        KIND_UNSET.with(|k| k.borrow_mut().clear());
        let clock = FakeClock::new(T0);
        let mut answers: IndexMap<String, Answer> = o
            .answers
            .as_object()
            .into_iter()
            .flatten()
            .map(|(k, v)| (k.clone(), Answer::from_json(v)))
            .collect();
        for (id, state) in &o.states {
            answers.insert(
                id.to_string(),
                Answer {
                    seen: Some(json!({"state": state})),
                    ..Answer::default()
                },
            );
        }
        let mut sights: VecDeque<Sight> = o
            .gone
            .iter()
            .map(|g| {
                if *g == "gone" {
                    Sight::Gone
                } else {
                    Sight::Running
                }
            })
            .collect();
        sights.extend(o.sights.clone());
        let shared = Rc::new(RefCell::new(BackendShared {
            mode,
            answers,
            snapshots_any_life: o.snapshots_any_life,
            sight_word: o.sight_word.unwrap_or("readGone"),
            sights,
            first_read_is_load: !o.sight_first,
            reads: HashMap::new(),
            runs: HashMap::new(),
        }));
        let mut seed = o.store.clone();
        if let Some(rows) = seed.get_mut("workspaces").and_then(Value::as_object_mut) {
            for (_, r) in rows.iter_mut() {
                *r = fill_workspace(r);
            }
        }
        if let Some(rows) = seed.get_mut("projects").and_then(Value::as_object_mut) {
            for (_, r) in rows.iter_mut() {
                *r = fill_project(r);
            }
        }
        let logs = match mode {
            Mode::Load => StoreLogs {
                put: true,
                delete: true,
                ..StoreLogs::default()
            },
            Mode::Ready => StoreLogs {
                put: true,
                delete: true,
                list: true,
                ..StoreLogs::default()
            },
            Mode::Rows => StoreLogs::default(),
        };
        let mem = MemStore::seeded(&seed, log.clone(), logs);
        let store: Rc<dyn Store> = o
            .store_override
            .clone()
            .unwrap_or_else(|| Rc::new(NormStore(mem.clone())));
        let places = Rc::new(FakePlaces {
            wired: "solari".into(),
            shared: shared.clone(),
            no_backend_at: o.no_backend_at.iter().map(|s| s.to_string()).collect(),
            backends: mode != Mode::Rows,
        });
        let devices: Vec<ScopedDevice> = o
            .devices
            .as_array()
            .into_iter()
            .flatten()
            .map(|d| ScopedDevice {
                id: d["id"].as_str().unwrap().to_string(),
                thread_id: d["scope"]["threadId"].as_str().map(str::to_string),
            })
            .collect();
        let rt = Rc::new(Fake {
            mode,
            log: log.clone(),
            extras: Log::default(),
            clock: clock.clone(),
            ids: Rc::new(FakeIds::default()),
            store,
            mem,
            places,
            shared,
            owner: RefCell::new(if mode == Mode::Ready {
                String::new()
            } else {
                "h_me".into()
            }),
            reads_since: Cell::new(match mode {
                Mode::Rows => o.reads_since.unwrap_or((T0 - 1000) as f64),
                _ => 0.0,
            }),
            closed: Cell::new(false),
            closing: Cell::new(false),
            hostname: o.hostname.clone().unwrap_or_else(|| "this-computer".into()),
            local: mode == Mode::Ready && !o.no_local,
            live: Rc::default(),
            sessions: Rc::default(),
            thread_records: Rc::default(),
            builders: Rc::default(),
            projects_held: Rc::default(),
            boot_work: Rc::default(),
            rereading: Rc::default(),
            viewed_marks: Rc::default(),
            indexes: RefCell::default(),
            index_pos: RefCell::default(),
            index_children: RefCell::default(),
            area: RefCell::new(Weak::new()),
            wire_listener: o.listener || mode == Mode::Rows,
            reattach_answers: RefCell::new(
                o.reattach
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            ),
            device_list: RefCell::new(devices),
            shells: RefCell::new(o.shells.iter().map(|s| s.to_string()).collect()),
            running: RefCell::new(o.running.iter().map(|s| s.to_string()).collect()),
            ends: RefCell::default(),
            pids: RefCell::default(),
            cap_waits: RefCell::default(),
            behind: RefCell::default(),
            restarts: RefCell::default(),
            setup_places: RefCell::default(),
            setup_refusals: RefCell::default(),
            latest_on: RefCell::default(),
            rows_on: RefCell::default(),
            sub_rows: RefCell::default(),
            kept_until: RefCell::default(),
            place_ids: RefCell::default(),
            lead_asks: RefCell::default(),
            limits: RefCell::default(),
            armed_timers: RefCell::new(if mode == Mode::Rows {
                IndexSet::from(["t-armed".to_string()])
            } else {
                IndexSet::new()
            }),
            door_rows: RefCell::default(),
            default_head: RefCell::default(),
            places_for: RefCell::default(),
            talks_to_tree: Cell::new(false),
            title_rows_of: RefCell::default(),
            resume_fails: RefCell::default(),
            on_sub_read: RefCell::default(),
            fail_pass: Cell::new(0),
            backend_for_failure: RefCell::new(None),
            module_of_failure: RefCell::new(None),
            talks_to_tree_failure: RefCell::new(None),
        });
        if mode == Mode::Rows {
            rt.projects_held
                .borrow_mut()
                .insert("p1".into(), project_p1());
            for (id, r) in o.live.as_object().into_iter().flatten() {
                rt.add_live(id, r);
            }
            for r in o.rows.as_array().into_iter().flatten() {
                let row = row_of(r);
                let id = row.borrow().view.id.clone();
                rt.sessions.borrow_mut().insert(id, row);
            }
            for (id, r) in o.records.as_object().into_iter().flatten() {
                rt.thread_records.borrow_mut().insert(
                    id.clone(),
                    serde_json::from_value(r.clone())
                        .unwrap_or_else(|e| panic!("thread record {r}: {e}")),
                );
            }
        }
        let area = BootArea::new(
            rt.clone() as Rc<dyn BootRuntime>,
            BootConfig {
                host_id: "host:A".into(),
            },
        );
        *rt.area.borrow_mut() = Rc::downgrade(&area);
        World {
            rt,
            area,
            log,
            clock,
        }
    }

    /// The rows oracle's start: `ready()` on an empty store, then its log dropped.
    pub async fn started(&self) {
        self.area.ready().await.expect("the start");
        common::flush().await;
        self.log.clear();
    }
}

fn short_entry(e: &Entry) -> Value {
    json!(format!("entry:{}", e.borrow().record.borrow().id))
}

impl Fake {
    /// What the rows oracle's `world` gives `live`: `{record: {id, name: w-id, kind: cloud, machineId: m-id, phase: running, project: p1, ...r}}`.
    pub fn add_live(&self, id: &str, r: &Value) -> Entry {
        let mut rec = json!({"id": id, "name": format!("w-{id}"), "kind": "cloud", "machineId": format!("m-{id}"), "phase": "running", "golden": "", "project": "p1"});
        for (k, x) in r.as_object().into_iter().flatten() {
            rec[k] = x.clone();
        }
        // The record's lead names both its thread and its workspace (sub-spawn.ts:236); the scene names the workspace alone.
        if let Some(lead) = rec.get_mut("lead").and_then(Value::as_object_mut) {
            lead.entry("threadId").or_insert(json!("t-lead"));
        }
        let entry = live_workspace(
            workspace_record(&rec),
            FakeMachine::plain(&format!("m-{id}")),
        );
        self.live.borrow_mut().insert(id.to_string(), entry.clone());
        entry
    }

    /// A live workspace on a machine that is not a stand-in, for the record as given.
    pub fn add_plain_live(&self, id: &str, record: &Value) -> Entry {
        let entry = live_workspace(
            workspace_record(record),
            FakeMachine::plain(&format!("m-{id}")),
        );
        self.live.borrow_mut().insert(id.to_string(), entry.clone());
        entry
    }

    pub fn add_row(&self, r: &Value) -> Row {
        let row = row_of(r);
        let id = row.borrow().view.id.clone();
        self.sessions.borrow_mut().insert(id, row.clone());
        row
    }

    pub fn add_record(&self, thread: &str, r: Value) {
        self.thread_records.borrow_mut().insert(
            thread.to_string(),
            serde_json::from_value(r).expect("thread record"),
        );
    }

    pub fn thread_records_json(&self) -> Value {
        Value::Array(
            self.thread_records
                .borrow()
                .iter()
                .map(|(k, v)| json!([k, to_json(v)]))
                .collect(),
        )
    }

    fn gen(&self, name: &str, args: Vec<Value>) {
        let mut e = vec![json!(name)];
        e.extend(args);
        self.log.push(norm(Value::Array(e)));
    }

    /// An event handed to the bus by a scene's act: logged as the oracle logs it, then heard by the area's listener.
    pub fn inject(&self, raw: Value) {
        self.log.push(norm(json!(["emit", raw.clone()])));
        let mut v = raw;
        let fill: &[(&str, Value)] = match v["type"].as_str() {
            Some("session.end") => &[("sawResult", json!(false)), ("exitCode", Value::Null)],
            Some("session.capped") => &[
                ("sessionId", json!("")),
                ("placeId", json!("")),
                ("place", json!("")),
                ("running", json!(0)),
                ("atOnce", json!(0)),
            ],
            _ => &[],
        };
        for (k, d) in fill {
            if v.get(*k).is_none() {
                v[*k] = d.clone();
            }
        }
        let event: EventUnion =
            serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("event {v}: {e}"));
        if let Some(a) = self.area.borrow().upgrade() {
            a.on_event(&event);
        }
    }

    pub fn module_for(&self, kind: &str) -> Rc<dyn KindModule> {
        if kind == "unwired" {
            panic!("no module wired for unwired");
        }
        Rc::new(FakeModule {
            backend: Arc::new(FakeBackend {
                place: "solari".into(),
                shared: self.shared.clone(),
            }),
        })
    }
}

#[async_trait(?Send)]
impl BootRuntime for Fake {
    fn store(&self) -> Rc<dyn Store> {
        self.store.clone()
    }
    fn clock(&self) -> Rc<dyn Clock> {
        self.clock.clone()
    }
    fn ids(&self) -> Rc<dyn Ids> {
        self.ids.clone()
    }
    fn places(&self) -> Rc<dyn PlaceBackends> {
        self.places.clone()
    }
    fn emit(&self, event: EventUnion) -> Result<(), HostError> {
        self.log.push(norm(json!([
            "emit",
            serde_json::to_value(&event).expect("event")
        ])));
        if self.wire_listener {
            if let Some(a) = self.area.borrow().upgrade() {
                a.on_event(&event);
            }
        }
        Ok(())
    }
    fn pass(&self, event: EventUnion) -> Result<(), HostError> {
        if self.mode != Mode::Ready {
            self.log.push(norm(json!([
                "pass",
                serde_json::to_value(&event).expect("event")
            ])));
        }
        if self.fail_pass.get() > 0 {
            self.fail_pass.set(self.fail_pass.get() - 1);
            return Err(HostError::new("bus down"));
        }
        Ok(())
    }
    fn warn(&self, line: &str) {
        self.log.push(json!(["warn", line]));
    }
    fn owner(&self) -> String {
        self.owner.borrow().clone()
    }
    fn set_owner(&self, owner: &str) {
        *self.owner.borrow_mut() = owner.to_string();
    }
    fn reads_since(&self) -> f64 {
        self.reads_since.get()
    }
    fn set_reads_since(&self, at: f64) {
        self.reads_since.set(at);
    }
    fn closed(&self) -> bool {
        self.closed.get()
    }
    fn closing(&self) -> bool {
        self.closing.get()
    }
    fn hostname(&self) -> String {
        self.hostname.clone()
    }
    fn local_wired(&self) -> bool {
        self.local
    }

    fn live(&self) -> Rc<RefCell<IndexMap<String, Entry>>> {
        self.live.clone()
    }
    fn sessions(&self) -> Rc<RefCell<IndexMap<String, Row>>> {
        self.sessions.clone()
    }
    fn thread_records(&self) -> Rc<RefCell<IndexMap<String, ThreadRecord>>> {
        self.thread_records.clone()
    }
    fn builders(&self) -> wsp_machines::images::Builders {
        self.builders.clone()
    }
    fn projects_held(&self) -> Rc<RefCell<IndexMap<String, ProjectView>>> {
        self.projects_held.clone()
    }
    fn boot_work(&self) -> Rc<RefCell<IndexMap<String, Shared<LocalBoxFuture<'static, ()>>>>> {
        self.boot_work.clone()
    }
    fn rereading(&self) -> Rc<RefCell<IndexSet<String>>> {
        self.rereading.clone()
    }
    fn viewed_marks(&self) -> Rc<RefCell<IndexMap<String, IndexMap<String, String>>>> {
        self.viewed_marks.clone()
    }
    fn has_transcript_index(&self, workspace_id: &str) -> bool {
        self.indexes.borrow().contains(workspace_id)
    }
    fn index_pos(&self, workspace_id: &str) -> u64 {
        self.index_pos
            .borrow()
            .get(workspace_id)
            .copied()
            .unwrap_or(0)
    }
    fn index_children(&self, workspace_id: &str, thread_id: &str) -> Vec<Value> {
        self.index_children
            .borrow()
            .get(&(workspace_id.to_string(), thread_id.to_string()))
            .cloned()
            .unwrap_or_default()
    }

    fn observed(&self, machine: Arc<dyn Machine>) -> Arc<dyn Machine> {
        machine
    }
    fn attach(
        &self,
        handle: &Rc<RefCell<WorkspaceRecord>>,
        machine: Arc<dyn Machine>,
    ) -> Result<Entry, HostError> {
        let record = handle.borrow().clone();
        match self.mode {
            Mode::Load => self.log.push(norm(json!([
                "attach",
                strip_workspace(to_json(&record)),
                machine.id(),
                kind_json(&machine)
            ]))),
            Mode::Ready => {
                self.log
                    .push(norm(json!(["attach", record.id, to_json(&record.phase)])))
            }
            Mode::Rows => self.gen("attach", vec![json!(record.id)]),
        }
        let id = record.id.clone();
        let entry = live_workspace(record, machine);
        entry.borrow_mut().record = Rc::clone(handle);
        self.live.borrow_mut().insert(id, entry.clone());
        Ok(entry)
    }
    async fn mark_gone(
        &self,
        entry: &Entry,
        machine_id: &str,
        reason: &str,
    ) -> Result<GoneOutcome, HostError> {
        tick().await;
        self.gen(
            "markGone",
            vec![json!(entry_id(entry)), json!(machine_id), json!(reason)],
        );
        Ok(GoneOutcome::Settled)
    }
    fn idle_touch(&self, workspace_id: &str) {
        self.gen("idle.touch", vec![json!(workspace_id)]);
    }
    fn sync_daemon(&self, entry: &Entry) {
        self.gen("syncDaemon", vec![json!(entry_id(entry))]);
    }
    fn backend_for(&self, _record: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError> {
        if let Some(e) = self.backend_for_failure.borrow().clone() {
            return Err(e);
        }
        Ok(Arc::new(FakeBackend {
            place: "solari".into(),
            shared: self.shared.clone(),
        }))
    }
    fn module_of(&self, kind: &str) -> Result<Rc<dyn KindModule>, HostError> {
        if let Some(e) = self.module_of_failure.borrow().clone() {
            return Err(e);
        }
        Ok(self.module_for(kind))
    }
    async fn persist(&self, record: &WorkspaceRecord) -> Result<(), HostError> {
        tick().await;
        match self.mode {
            Mode::Ready => {
                self.log
                    .push(norm(json!(["persist", record.id, to_json(&record.phase)])))
            }
            _ => self
                .log
                .push(norm(json!(["persist", strip_workspace(to_json(record))]))),
        }
        Ok(())
    }
    async fn shape_of(&self, machine: &Arc<dyn Machine>) -> Option<MachineShape> {
        tick().await;
        self.gen("shapeOf", vec![json!(machine.id())]);
        Some(serde_json::from_value(json!({"cpu": 2, "memMb": 4096})).expect("shape"))
    }
    fn size_built(&self, shape: Option<&MachineShape>, asked: WorkspaceSize) -> WorkspaceSize {
        self.gen(
            "sizeBuilt",
            vec![shape.map(to_json).unwrap_or(Value::Null), to_json(&asked)],
        );
        serde_json::from_value(json!({"cpu": 2, "memMb": 4096})).expect("size")
    }
    fn arm_grace(&self, builder_id: &str, sealed_at: &str) {
        self.gen("armGrace", vec![json!(builder_id), json!(sealed_at)]);
    }
    fn door_backend_of(&self, _place: &str) -> Option<Arc<dyn MachineBackend>> {
        None
    }
    async fn image_kind(&self, golden: &str) -> Result<Option<MachineKind>, HostError> {
        tick().await;
        if self.mode == Mode::Load {
            self.gen("imageOf", vec![json!(golden)]);
            return match golden {
                "desktop-img" => Ok(Some(MachineKind::Desktop)),
                "bad" => Err(HostError::new("no image")),
                _ => Ok(None),
            };
        }
        Ok(None)
    }
    fn attach_queued(&self, record: &Rc<RefCell<WorkspaceRecord>>) -> Result<(), HostError> {
        self.gen("attachQueued", vec![json!(record.borrow().id)]);
        Ok(())
    }

    async fn migrate_copies(&self) -> Result<(), HostError> {
        tick().await;
        self.gen("migrateCopies", vec![]);
        Ok(())
    }
    async fn place_door_load(&self) -> Result<(), HostError> {
        tick().await;
        self.gen("placeDoor.load", vec![]);
        Ok(())
    }
    async fn git_top_of(&self, path: &str) -> Result<String, HostError> {
        tick().await;
        self.gen("gitTopOf", vec![json!(path)]);
        if path == "/nogit" {
            Err(HostError::new("not a repo"))
        } else {
            Ok(format!("{path}-top"))
        }
    }
    async fn remember_project(&self, project: ProjectView) -> Result<(), HostError> {
        tick().await;
        self.gen("rememberProject", vec![strip_project(to_json(&project))]);
        Ok(())
    }
    async fn setups_load(&self) -> Result<(), HostError> {
        tick().await;
        self.gen("setups.load", vec![]);
        Ok(())
    }
    async fn move_transcript(&self, record: Value) -> Result<(), HostError> {
        tick().await;
        self.gen("moveTranscript", vec![record]);
        Ok(())
    }
    fn move_blobs(&self) {}
    async fn load_index(&self, workspace_id: &str) -> Result<(), HostError> {
        tick().await;
        self.gen("loadIndex", vec![json!(workspace_id)]);
        Ok(())
    }
    fn wake_at(&self, thread_id: &str, until: f64) {
        self.gen("wakeAt", vec![json!(thread_id), json!(until)]);
    }
    fn resume_on_reset(&self, thread_id: &str, at: f64) {
        self.gen("resumeOnReset", vec![json!(thread_id), json!(at)]);
    }
    fn settle_cut(
        &self,
        row: &Row,
        reason: &str,
        line: &dyn Fn(f64) -> String,
        _stopped: Option<bool>,
        _unreached: Option<bool>,
    ) {
        let turn = row.borrow().turn_id.clone();
        self.gen(
            "settleCut",
            vec![
                json!(turn),
                json!(reason),
                json!(line((T0 + 65_000) as f64)),
            ],
        );
        row.borrow_mut().view.status = wsp_wire::views::session::SessionStatus::Failed;
    }
    async fn persist_sessions(&self, workspace_id: &str) {
        tick().await;
        self.gen("persistSessions", vec![json!(workspace_id)]);
    }
    async fn turn_changes_written(
        &self,
        workspace_id: &str,
        thread_id: &str,
        _turn_id: &str,
    ) -> Result<bool, HostError> {
        tick().await;
        self.gen("openThread", vec![json!(workspace_id), json!(thread_id)]);
        Ok(false)
    }
    async fn read_turn_changes(&self, entry: &Entry, ask: CardAsk) -> Result<bool, HostError> {
        tick().await;
        self.gen(
            "readTurnChanges",
            vec![short_entry(entry), json!({"sessionId": ask.session_id, "turnId": ask.turn_id, "threadId": ask.thread_id, "cwd": ask.cwd, "from": ask.from, "startedAt": ask.started_at})],
        );
        Ok(false)
    }
    async fn reattach(&self, row: &Row, again: bool) -> Reopened {
        let turn = row.borrow().turn_id.clone();
        self.gen("reattach", vec![json!(turn), json!(again)]);
        match self
            .reattach_answers
            .borrow()
            .get(&turn)
            .map(String::as_str)
        {
            Some("cannot") => Reopened::Cannot,
            Some("gone") => Reopened::Gone,
            Some("unreached") => Reopened::Unreached,
            _ => Reopened::Attached,
        }
    }
    async fn devices(&self) -> Result<Vec<ScopedDevice>, HostError> {
        self.gen("deviceDoor.list", vec![]);
        Ok(self.device_list.borrow().clone())
    }
    async fn revoke_device(&self, device_id: &str) -> Result<(), HostError> {
        self.gen("deviceDoor.revoke", vec![json!(device_id)]);
        Ok(())
    }
    async fn revoke_asides(&self) -> Result<(), HostError> {
        self.gen("deviceDoor.revokeAsides", vec![]);
        Ok(())
    }
    async fn shell_held_devices(&self) -> Result<IndexSet<String>, HostError> {
        self.gen("shellHeldDevices", vec![]);
        Ok(self.shells.borrow().iter().cloned().collect())
    }
    fn thread_runs(&self, thread_id: &str) -> bool {
        self.running.borrow().iter().any(|t| t == thread_id)
    }
    async fn worktree_mounted(&self, entry: &Entry) -> Result<(), HostError> {
        self.gen("worktreeMounted", vec![json!(entry_id(entry))]);
        Ok(())
    }
    async fn sweep_runs(&self, entry: &Entry) -> Result<(), HostError> {
        self.gen("sweepRuns", vec![json!(entry_id(entry))]);
        Ok(())
    }
    async fn resume_queue(&self) -> Result<(), HostError> {
        self.gen("resumeQueue", vec![]);
        Ok(())
    }
    fn notify_end(&self, end: HeldEnd) {
        let mut tell = Map::new();
        if let Some(b) = &end.notify_by {
            tell.insert("notifyBy".into(), to_json(b));
        }
        if let Some(r) = &end.notify_road {
            tell.insert("notifyRoad".into(), to_json(r));
        }
        self.gen(
            "notifyEnd",
            vec![
                json!(format!("row:{}", end.turn_id)),
                json!(end.notify),
                json!({"tellAs": Value::Object(tell)}),
                to_json(&end.result),
            ],
        );
        self.ends.borrow_mut().push(end);
    }
    fn record(&self, event: EventUnion) {
        self.gen("record", vec![serde_json::to_value(&event).expect("event")]);
    }
    fn arm_subs(&self) {
        self.gen("armSubs", vec![]);
    }
    async fn deliver_owed(&self) -> Result<(), HostError> {
        self.gen("deliverOwed", vec![]);
        Ok(())
    }
    async fn move_old_copies(&self) -> Result<(), HostError> {
        self.gen("moveOldCopies", vec![]);
        Ok(())
    }
    fn arm_sweep(&self) {
        self.gen("armSweep", vec![]);
    }
    async fn slates_ready(&self) -> Result<(), HostError> {
        self.gen("slates.ready", vec![]);
        Ok(())
    }

    fn drives_thread(&self, thread_id: Option<&str>, caller: Option<&Caller>) -> bool {
        self.gen(
            "drivesThread",
            vec![
                thread_id.map(|t| json!(t)).unwrap_or(Value::Null),
                caller_json(caller),
            ],
        );
        match (caller, thread_id) {
            (Some(Caller::Scoped { by, .. }), Some(t)) => t.starts_with(&by.root_thread_id),
            _ => true,
        }
    }
    fn acts_on(&self, record: &WorkspaceRecord, caller: Option<&Caller>) -> bool {
        self.gen("actsOn", vec![json!(record.id), caller_json(caller)]);
        true
    }
    fn talks_to_its_tree(&self, _caller: Option<&Caller>) -> Result<bool, HostError> {
        if let Some(e) = self.talks_to_tree_failure.borrow().clone() {
            return Err(e);
        }
        Ok(self.talks_to_tree.get())
    }
    fn refuse_relayed(
        &self,
        record: &WorkspaceRecord,
        origin: Option<&Caller>,
        act: Option<&str>,
        thread: Option<&str>,
    ) -> Result<(), HostError> {
        self.gen(
            "refuseRelayed",
            vec![
                json!(record.id),
                caller_json(origin),
                act.map(|a| json!(a)).unwrap_or(Value::Null),
                thread.map(|t| json!(t)).unwrap_or(Value::Null),
            ],
        );
        Ok(())
    }
    async fn resolve_workspace(
        &self,
        reference: &str,
        _origin: Option<&Caller>,
    ) -> Result<String, HostError> {
        tick().await;
        Ok(reference.to_string())
    }
    async fn place_rows(&self) -> Option<Vec<ComputerRow>> {
        tick().await;
        self.door_rows.borrow().clone()
    }
    async fn default_image_head(&self) -> Result<Option<String>, HostError> {
        tick().await;
        Ok(self.default_head.borrow().clone())
    }
    async fn place_for(&self, place: &str) -> Result<Option<String>, HostError> {
        tick().await;
        self.extras.push(json!(["placeFor", place]));
        Ok(self.places_for.borrow().get(place).cloned().flatten())
    }

    fn restarts(&self) -> IndexMap<String, String> {
        self.restarts.borrow().clone()
    }
    fn stopped_behind(&self, row: &Row) -> Option<Value> {
        self.behind.borrow().get(&row.borrow().turn_id).cloned()
    }
    fn cap_wait(&self, turn_id: &str) -> Option<Value> {
        self.cap_waits.borrow().get(turn_id).cloned()
    }
    fn agent_pid_of(&self, run: &str) -> Option<u32> {
        self.gen("agentPidOf", vec![json!(run)]);
        self.pids.borrow().get(run).copied()
    }
    fn setup_refusal(&self, workspace_id: &str, harness: &str) -> Option<String> {
        self.setup_refusals
            .borrow()
            .get(&(workspace_id.to_string(), harness.to_string()))
            .cloned()
    }
    fn setup_place(&self, entry: &Entry) -> Option<String> {
        self.setup_places.borrow().get(&entry_id(entry)).cloned()
    }
    fn latest_on(&self, thread_id: &str) -> Option<SessionView> {
        self.latest_on.borrow().get(thread_id).cloned()
    }
    fn rows_on(&self, thread_id: &str) -> Vec<SessionView> {
        self.rows_on
            .borrow()
            .get(thread_id)
            .cloned()
            .unwrap_or_default()
    }
    fn sub_row_of(&self, record: &WorkspaceRecord) -> Option<SubRow> {
        self.sub_rows.borrow().get(&record.id).cloned()
    }
    fn disk_kept_until(&self, record: &WorkspaceRecord) -> Option<f64> {
        self.kept_until.borrow().get(&record.id).copied()
    }
    fn place_id_of(&self, record: &WorkspaceRecord) -> Option<String> {
        self.place_ids.borrow().get(&record.id).cloned()
    }
    async fn sub_read(&self, workspace_id: &str, since: f64) -> Result<(), HostError> {
        tick().await;
        self.gen("subRead", vec![json!(workspace_id), json!(since)]);
        if let Some(f) = self.on_sub_read.borrow().as_ref() {
            f();
        }
        Ok(())
    }
    fn title_rows(&self, _views: Vec<SessionView>) -> Vec<SessionView> {
        self.title_rows_of.borrow().clone()
    }
    fn refresh_title(&self, view: SessionView) {
        self.gen("refreshTitle", vec![json!(view.id)]);
    }
    fn reap_kept(&self, thread_id: &str) {
        self.gen("reapKept", vec![json!(thread_id)]);
    }
    fn cancel_resume(&self, thread_id: &str) {
        if self.armed_timers.borrow().contains(thread_id) {
            self.gen("resume timer cancelled", vec![json!(thread_id)]);
        }
    }
    fn lead_asks(&self, thread_id: &str) -> bool {
        self.lead_asks.borrow().contains(thread_id)
    }
    async fn ledger_limits(&self) -> Vec<LimitRow> {
        tick().await;
        self.limits.borrow().clone()
    }
    fn account_key_on(&self, _entry: &Entry, harness: &str) -> String {
        let limits: Vec<Value> = self
            .limits
            .borrow()
            .iter()
            .map(|l| {
                let windows: Vec<Value> = l
                    .windows
                    .iter()
                    .map(|w| {
                        let mut v = to_json(w);
                        v.as_object_mut().unwrap().shift_remove("kind");
                        v
                    })
                    .collect();
                json!({"key": l.key, "windows": windows})
            })
            .collect();
        self.gen("accountOnComputer", vec![json!({"agent": harness, "agentName": "Claude Code", "computer": {"id": "box-1", "name": "Box"}, "limits": limits, "vaulted": false})]);
        "acct-1".to_string()
    }
    async fn resume_turn(
        &self,
        workspace_id: &str,
        thread_id: &str,
        after_limit: f64,
    ) -> Result<(), HostError> {
        tick().await;
        self.gen("sessionsApi.start", vec![json!(workspace_id), json!({"prompt": wsp_wire::types::usage::LIMIT_RESUME_PROMPT, "thread": thread_id, "afterLimit": after_limit})]);
        Ok(())
    }
}

// --- the scenes of boot_rows.json ------------------------------------------------------------------------------------

use wsp_machines::machines::boot::ComputerRow as Computer;
use wsp_wire::types::usage::{LimitKind, LimitWindow};
use wsp_words::words::computer::AcrossAct;

/// `entry:<id>`, as the rows oracle's `short` writes an entry.
pub fn short(e: &Entry) -> Value {
    json!(format!("entry:{}", entry_id(e)))
}

fn said_of(r: Result<Entry, HostError>) -> Value {
    match r {
        Ok(e) => short(&e),
        Err(e) => json!({"threw": e.message, "kind": kind_word(&e)}),
    }
}

fn opt_short(r: Result<Option<Entry>, HostError>) -> Value {
    match r.expect("an answer") {
        Some(e) => short(&e),
        None => Value::Null,
    }
}

/// The wait the oracle's cap map named (`{place, running, limit}`), as the wire shape reads it.
pub fn typed_cap() -> Value {
    json!({"placeId": "box", "place": "box", "running": 2, "atOnce": 2})
}

/// The prompt another thread's row is stopped behind (`{threadId, title}` in the oracle), as the wire shape reads it.
pub fn typed_behind() -> Value {
    json!({"threadId": "t9", "workspaceId": "a", "sessionId": "s9", "title": "x", "prompt": {"askId": "ask-1", "toolName": "Bash", "input": "ls", "options": []}})
}

fn limit_window(resets_at: i64) -> LimitWindow {
    LimitWindow {
        kind: LimitKind::Session,
        used_percent: JsNumber(100.0),
        resets_at: Some(JsNumber(resets_at as f64)),
    }
}

fn stamps(v: Value) -> Map<String, Value> {
    v.as_object().expect("stamps").clone()
}

/// A rejected call, as the oracle's `.catch` logs it.
fn rejected(w: &World, e: HostError) {
    w.log.push(json!(["rejected", e.message, kind_word(&e)]));
}

/// Runs the act the scene's oracle ran, in a world built from its `given`. The answer comes back as the oracle wrote it.
pub async fn replay(scene: &Value) -> (World, Value) {
    let name = scene["name"].as_str().expect("a scene name").to_string();
    let given = &scene["given"];
    let mut o = Opts {
        live: given["live"].clone(),
        rows: given["rows"].clone(),
        records: given["records"].clone(),
        reads_since: given["readsSince"].as_f64(),
        ..Opts::default()
    };
    if name.starts_with("computer rows: with no door") {
        o.hostname = scene["answer"]["rows"][0]["name"]
            .as_str()
            .map(str::to_string);
    }
    let w = World::rows(o);
    let rt = w.rt.clone();
    let area = w.area.clone();
    if let Some(pids) = given["pids"].as_object() {
        for (run, pid) in pids {
            rt.pids
                .borrow_mut()
                .insert(run.clone(), pid.as_u64().unwrap() as u32);
        }
    }
    if name.starts_with("listed rows") {
        rt.cap_waits
            .borrow_mut()
            .insert("turn-r1".into(), typed_cap());
        rt.behind
            .borrow_mut()
            .insert("turn-r1".into(), typed_behind());
        rt.restarts
            .borrow_mut()
            .insert("t2".into(), "t2-new".into());
        rt.setup_places
            .borrow_mut()
            .insert("a".into(), "hetzner".into());
        rt.setup_refusals.borrow_mut().insert(
            ("a".into(), "claude".into()),
            "claude is refused on hetzner".into(),
        );
        rt.rows_on.borrow_mut().insert("t-src".into(), vec![view_of(&json!({"id": "src", "workspaceId": "a", "harness": "claude", "status": "completed", "threadId": "t-src", "title": "new title"}))]);
    }
    if name.starts_with("resume after limit") {
        let f = |k: &str| name.contains(k);
        if f("waiting on the person") {
            rt.lead_asks.borrow_mut().insert("t1".into());
        }
        let windows = if f("held past the reset") {
            vec![limit_window(T0 + 7_200_000)]
        } else {
            vec![]
        };
        rt.limits.borrow_mut().push(LimitRow {
            key: "acct-1".into(),
            windows,
        });
    }
    if name == "computer rows: a door that lists the provider is read as it is" {
        *rt.door_rows.borrow_mut() = Some(vec![
            Computer {
                id: "here".into(),
                name: "Mac".into(),
                kind: PlaceKind::Computer,
            },
            Computer {
                id: "solari".into(),
                name: "Solari".into(),
                kind: PlaceKind::Provider,
            },
        ]);
    }
    w.started().await;
    let thread = thread_caller();
    let answer = match name.as_str() {
        n if n.starts_with("listed rows") => {
            let held: Vec<Row> = rt.sessions.borrow().values().cloned().collect();
            to_json(&area.listed_rows(&held))
        }
        n if n.starts_with("thread facts") => {
            json!({"facts": area.thread_facts("t1").map(|f| to_json(&f)).unwrap_or(Value::Null), "none": area.thread_facts("nobody").map(|f| to_json(&f)).unwrap_or(Value::Null)})
        }
        n if n.starts_with("settled now") => json!({
            "quiet": area.settled_now("t1", Some(3_600_000.0)),
            "off": area.settled_now("t1", None),
            "stampOnly": area.settled_now("t-norow", Some(1.0)),
            "nothing": area.settled_now("t-none", Some(1.0)),
        }),
        n if n.starts_with("entry of row") => {
            let row = |t: &str, ws: &str| RowAt {
                thread_id: Some(t.to_string()),
                workspace_id: ws.to_string(),
            };
            let mut said = vec![];
            said.push(opt_short(
                area.entry_of_row(&row("t-a-own", "a"), Some(&thread), None, false)
                    .await,
            ));
            said.push(opt_short(
                area.entry_of_row(&row("t-b", "a"), Some(&thread), None, false)
                    .await,
            ));
            said.push(opt_short(
                area.entry_of_row(&row("t-a-own", "none"), Some(&thread), None, false)
                    .await,
            ));
            said.push(opt_short(
                area.entry_of_row(&row("t-b", "a"), None, None, false).await,
            ));
            json!(said)
        }
        n if n.starts_with("entry of") => {
            let mut said: Vec<Value> = vec![];
            said.push(said_of(
                area.entry_of("zzz", None, EntryAsk::default()).await,
            ));
            said.push(said_of(
                area.entry_of("zzz", Some(&thread), EntryAsk::default())
                    .await,
            ));
            rt.live.borrow().get("c").unwrap().borrow_mut().creating = true;
            said.push(said_of(area.entry_of("c", None, EntryAsk::default()).await));
            said.push(said_of(
                area.entry_of(
                    "c",
                    None,
                    EntryAsk {
                        being_made: true,
                        ..EntryAsk::default()
                    },
                )
                .await,
            ));
            rt.add_record("t-a1", json!({"workspaceId": "a", "harness": "claude"}));
            said.push(said_of(
                area.entry_of(
                    "a",
                    Some(&thread),
                    EntryAsk {
                        act: Some(AcrossAct::Commit),
                        thread: Some("t-a1".into()),
                        ..EntryAsk::default()
                    },
                )
                .await,
            ));
            said.push(said_of(
                area.entry_of(
                    "a",
                    Some(&thread),
                    EntryAsk {
                        thread: Some("t-other".into()),
                        ..EntryAsk::default()
                    },
                )
                .await,
            ));
            let (tx, rx) = futures::channel::oneshot::channel::<()>();
            rt.boot_work.borrow_mut().insert(
                "a".into(),
                async move {
                    let _ = rx.await;
                }
                .boxed_local()
                .shared(),
            );
            let waiting = area.clone();
            let waited = tokio::task::spawn_local(async move {
                waiting.entry_of("a", None, EntryAsk::default()).await
            });
            let now = area
                .entry_of(
                    "a",
                    None,
                    EntryAsk {
                        now: true,
                        ..EntryAsk::default()
                    },
                )
                .await;
            said.push(json!(["now answered first", said_of(now)]));
            rt.add_live("a", &json!({"name": "w-a2"}));
            let _ = tx.send(());
            let after = waited
                .await
                .unwrap()
                .expect("the entry after the boot work");
            said.push(json!([
                "after boot work",
                short(&after),
                after.borrow().record.borrow().name.clone()
            ]));
            json!(said)
        }
        n if n.starts_with("child of") => {
            let lead = rt.live.borrow().get("lead").cloned().expect("the lead");
            let mut said: Vec<Value> = vec![];
            for r in ["kid", "kid2", "other"] {
                said.push(said_of(area.child_of(&lead, r, None, None, None).await));
            }
            if let Err(e) = area
                .child_of(
                    &lead,
                    "other",
                    None,
                    Some("t-lead00000"),
                    Some("t-kid000000"),
                )
                .await
            {
                said.push(json!({"threw": e.message}));
            }
            json!(said)
        }
        n if n.starts_with("mark") => {
            area.mark(
                &["t1".into(), "t2".into(), "t3".into()],
                Stamped::Same(stamps(
                    json!({"settledAt": T0, "readAt": T0, "pinnedAt": null}),
                )),
                None,
            )
            .await
            .expect("mark");
            let each = |id: &str| stamps(json!({"snoozedUntil": T0 + 5, "note": id}));
            area.mark(&["t1".into()], Stamped::Each(&each), None)
                .await
                .expect("mark");
            area.mark(
                &["t-armed".into()],
                Stamped::Same(stamps(json!({"limitResume": null}))),
                None,
            )
            .await
            .expect("mark");
            if let Err(e) = area
                .mark(
                    &["t1".into(), "t-nobody".into()],
                    Stamped::Same(stamps(json!({"readAt": 1}))),
                    None,
                )
                .await
            {
                rejected(&w, e);
            }
            Value::Null
        }
        n if n.starts_with("end snooze") => {
            area.end_snooze_for(Some("t1"), Some("root"));
            common::flush().await;
            area.end_snooze_for(Some("root"), None);
            Value::Null
        }
        n if n.starts_with("arm resume") => {
            area.arm_resume(&["t1".into()], true, None)
                .await
                .expect("arm");
            if let Err(e) = area.arm_resume(&["t2".into()], true, None).await {
                rejected(&w, e);
            }
            if let Err(e) = area.arm_resume(&["t-none".into()], true, None).await {
                rejected(&w, e);
            }
            area.arm_resume(&["t1".into()], false, None)
                .await
                .expect("arm off");
            Value::Null
        }
        n if n.starts_with("resume after limit") => {
            area.resume_after_limit("t1").await.expect("resume");
            Value::Null
        }
        n if n.starts_with("row push") => {
            let ev = |v: Value| rt.inject(v);
            ev(json!({"type": "session.held", "workspaceId": "a", "threadId": "t1"}));
            ev(json!({"type": "session.held", "workspaceId": "a", "threadId": "t1"}));
            ev(json!({"type": "thread.marked", "workspaceId": "a", "threadIds": ["t2"]}));
            ev(
                json!({"type": "session.start", "workspaceId": "a", "sessionId": "s-new", "turnId": "turn-old", "threadId": "t1"}),
            );
            w.log.push(json!(["mark", "before the tick"]));
            area.rows_sent().await;
            w.log.push(json!(["mark", "second push"]));
            ev(json!({"type": "session.capped", "workspaceId": "a", "threadId": "t1"}));
            area.rows_sent().await;
            ev(
                json!({"type": "session.end", "workspaceId": "a", "sessionId": "s-ended", "turnId": "turn-ended", "threadId": "t9"}),
            );
            ev(
                json!({"type": "session.end", "workspaceId": "a", "sessionId": "r1", "turnId": "turn-r1", "threadId": "t1"}),
            );
            area.rows_sent().await;
            Value::Null
        }
        n if n.starts_with("computer rows: with no door") => {
            let rows = area.computer_rows().await;
            let named = wsp_machines::machines::boot::BootArea::name_of_computer(
                "x",
                &[Computer {
                    id: "x".into(),
                    name: "Box".into(),
                    kind: PlaceKind::Computer,
                }],
            );
            let unnamed = wsp_machines::machines::boot::BootArea::name_of_computer("y", &[]);
            json!({"rows": rows.iter().map(computer_json).collect::<Vec<_>>(), "named": named, "unnamed": unnamed})
        }
        n if n.starts_with("computer rows") => json!(area
            .computer_rows()
            .await
            .iter()
            .map(computer_json)
            .collect::<Vec<_>>()),
        n if n.starts_with("image head") => {
            let none = area.image_head_or_none().await.expect("head or none");
            let head = match area.image_head().await {
                Ok(h) => json!(h),
                Err(e) => json!({"threw": e.message}),
            };
            json!({"none": none, "head": head})
        }
        n if n.starts_with("tree records") => {
            let ids = |v: Vec<(String, String, ThreadRecord)>| {
                v.into_iter().map(|(id, _, _)| id).collect::<Vec<_>>()
            };
            json!({
                "tree": ids(area.tree_records(Some(&thread), None, None).expect("the tree answers")),
                "all": ids(area.tree_records(None, None, None).expect("the tree answers")),
                "on": area.tree_stands_on("a", Some(&thread), None).expect("the tree answers"),
                "person": area.tree_stands_on("a", None, None).expect("the tree answers"),
                "talks": area.talks_to_tree_on("a", Some(&thread)).expect("the tree answers"),
            })
        }
        other => panic!("no act for scene {other:?}"),
    };
    common::flush().await;
    (w, norm(answer))
}

pub fn computer_json(c: &ComputerRow) -> Value {
    json!({"id": c.id, "name": c.name, "kind": to_json(&c.kind)})
}

/// The scene as a Rust world can be held to it: the oracle's loose test values taken to the shapes the wire reads
/// (named at each change in the report).
pub fn expected(scene: &Value) -> Value {
    let mut s = scene.clone();
    let name = scene["name"].as_str().unwrap();
    if name.starts_with("entry of:") {
        for e in s["log"].as_array_mut().unwrap() {
            if e[0] == "refuseRelayed" && e[3] == "merge" {
                e[3] = json!("commit");
            }
        }
    }
    if name.starts_with("listed rows") {
        let r1 = &mut s["answer"][0];
        r1["capped"] = typed_cap();
        r1["waitingOn"] = typed_behind();
    }
    if name.starts_with("mark:") {
        for r in s["records"].as_array_mut().unwrap() {
            if let Some(o) = r[1].as_object_mut() {
                o.shift_remove("note");
            }
        }
    }
    s
}

/// An event's top-level keys in the order the oracle wrote them, for every `emit` and `pass` entry.
pub fn assert_event_key_order(want: &Value, got: &[Value]) {
    for (w, g) in want["log"].as_array().unwrap().iter().zip(got) {
        if matches!(w[0].as_str(), Some("emit" | "pass")) {
            let keys = |v: &Value| {
                v[1].as_object()
                    .map(|o| o.keys().cloned().collect::<Vec<_>>())
            };
            assert_eq!(
                keys(g),
                keys(w),
                "scene {}: key order of {}",
                want["name"],
                w[1]["type"]
            );
        }
    }
}

/// The whole of a rows scene: the log, the answer and the thread records after.
pub fn check_rows_scene(name: &'static str) {
    let fx = common::fixture("boot_rows");
    let scene = common::scene(&fx, name).clone();
    common::run_local(async move {
        let (w, answer) = replay(&scene).await;
        let want = expected(&scene);
        let got = w.log.take();
        common::assert_log(&want, &got);
        assert_event_key_order(&want, &got);
        assert_eq!(answer, want["answer"], "scene {name}: the answer");
        assert_eq!(
            norm(w.rt.thread_records_json()),
            want["records"],
            "scene {name}: the thread records after"
        );
    });
}

/// A provider read already out that failed with `msg`, as the load's `asked` for a late or a held record.
pub fn asked_err(msg: &str) -> wsp_machines::machines::boot::load::Asked {
    futures::future::ready(Err(HostError::new(msg)))
        .boxed_local()
        .shared()
}

/// The same read answered 404: an error of kind missing whose message is the provider's words, status first.
pub fn asked_missing(msg: &str) -> wsp_machines::machines::boot::load::Asked {
    futures::future::ready(Err(HostError::with_kind(msg, Kind::Missing)))
        .boxed_local()
        .shared()
}

/// A store whose first listing of `builders` waits for `gate` after it has read the rows (the TS test's gated store).
pub struct GatedStore {
    pub inner: Rc<MemStore>,
    pub gate: RefCell<Option<futures::channel::oneshot::Receiver<()>>>,
    pub listed: Rc<Cell<bool>>,
}

#[async_trait(?Send)]
impl Store for GatedStore {
    async fn get(&self, c: &str, id: &str) -> Result<Option<Value>, HostError> {
        self.inner.get(c, id).await
    }
    async fn put(&self, c: &str, id: &str, v: Value) -> Result<(), HostError> {
        self.inner.put(c, id, norm(v)).await
    }
    async fn list(&self, c: &str) -> Result<Vec<Value>, HostError> {
        let rows = self.inner.list(c).await?;
        let gate = if c == "builders" {
            self.gate.borrow_mut().take()
        } else {
            None
        };
        if let Some(gate) = gate {
            self.listed.set(true);
            let _ = gate.await;
        }
        Ok(rows)
    }
    async fn keys(&self, c: &str) -> Result<Vec<String>, HostError> {
        self.inner.keys(c).await
    }
    async fn delete(&self, c: &str, id: &str) -> Result<(), HostError> {
        self.inner.delete(c, id).await
    }
    async fn get_blob(&self, c: &str, id: &str) -> Result<Option<Vec<u8>>, HostError> {
        self.inner.get_blob(c, id).await
    }
    async fn put_blob(&self, c: &str, id: &str, b: Vec<u8>) -> Result<(), HostError> {
        self.inner.put_blob(c, id, b).await
    }
    async fn delete_blob(&self, c: &str, id: &str) -> Result<(), HostError> {
        self.inner.delete_blob(c, id).await
    }
    async fn stat_blob(
        &self,
        c: &str,
        id: &str,
    ) -> Result<Option<wsp_core::types::pending::BlobMark>, HostError> {
        self.inner.stat_blob(c, id).await
    }
}

/// The hydration a held record starts from: `ws("a")` read while the provider's answer is the error `msg`.
pub async fn held_for_test(w: &World, msg: &str) {
    w.area
        .hydrate_workspace(ws("a", json!({})), None, None, Some(asked_err(msg)))
        .await
        .expect("hydrate");
}
