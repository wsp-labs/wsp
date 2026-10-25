// SPDX-License-Identifier: AGPL-3.0-only
//! The fakes of the machines oracle (oracle/machines.oracle.ts): a runtime that logs every call as the oracle's
//! recording context does, a provider with scripted answers, machines with scripted states, and the world a scene
//! builds them from. Included with `#[path = "support/machines.rs"] mod support;`.
#![allow(dead_code, unused_imports)]

use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::Future;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::{FutureExt, LocalBoxFuture};
use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde_json::{json, Map, Value};

use wsp_adapters::port::ExecStreamFactory;
use wsp_core::ids::Ids;
use wsp_core::types::internal::{EndThread, FolderAdmit, ImportLanded, ImportReport, KindModule, PortRoute, ServerValuesRoad, SessionEntry, ThreadRecord};
use wsp_core::types::pending::{Clock, KillConfirm, MachineExecOptions, Store, TurnWaiting};
use wsp_core::types::wiring::{
    GoldenExec, GoneOutcome, Joined, LiveWorkspace, Local, PlaceBackends, ProjectImportOptions, StageReport, WorkspaceRecord,
};
use wsp_engine::Cancel;
use wsp_engine::errors::{EngineError, ErrorKind, WspError};
use wsp_engine::lifecycle::{Workspace, WorkspaceHooks, WorkspaceInitial};
use wsp_engine::machine::{
    BackendOffers, BackendPricing, Capabilities, ExecOptions, ExecResult, Lifecycle, LifecycleBudgets, Machine, MachineBackend, MachineKind,
    MachineLife, MachineListRow, MachineOffers, MachineShape, MachineSpec, MachineState, NamedMachine, PreviewReach, ProviderBoxState, RunOptions,
    Seen, SnapshotOptions, SnapshotStoragePricing, Size, Waiting,
};
use wsp_machines::machines::machines::{
    Entry, IdleDoor, ImageRead, MachinesArea, MachinesConfig, MachinesRuntime, Refusal, Row, ThreadOn, Unreached,
};
use wsp_wire::views::golden_image::MachineKind as _WireMachineKind;
use wsp_wire::views::workspace::{MachineFacts, ReachState, WorkspacePhase};
use wsp_wire::views::workspace_more::WorkspaceView;
use wsp_wire::wire::capabilities::WorkspaceSize;
use wsp_wire::wire::events_union::EventUnion;
use wsp_words::exit::{HostError, Kind};

use crate::common::{self, FakeClock, FakeIds, Log, MemStore, StoreLogs, T0};

// ---------------------------------------------------------------------------------------------------------------
// Plumbing

pub fn from<T: DeserializeOwned>(v: Value) -> T {
    serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("{e}: {v}"))
}

async fn tick() {
    tokio::task::yield_now().await;
}

/// The oracle's `missing()`: kind missing, status 404.
pub fn missing(message: &str) -> EngineError {
    EngineError::Provider(WspError { kind: ErrorKind::Missing, status: 404, code: None, message: message.into(), request_id: None, window: None, said: None })
}

/// The oracle's `statusErr(status, message)`: an error carrying only a numeric status. The engine's error type has no
/// status without a kind, so it carries one (conflict).
pub fn status_err(status: u16, message: &str) -> EngineError {
    EngineError::Provider(WspError { kind: ErrorKind::Conflict, status, code: None, message: message.into(), request_id: None, window: None, said: None })
}

/// A plain `new Error(message)`.
pub fn plain(message: &str) -> EngineError {
    EngineError::Other { message: message.into(), code: None }
}

/// The host-side `missing()`: what a rejection of the runtime's own calls carries.
pub fn missing_host(message: &str) -> HostError {
    HostError::with_kind(message, Kind::Missing)
}

pub fn ok_exec(stdout: &str) -> ExecResult {
    from(json!({ "exitCode": 0, "stdout": stdout, "stderr": "" }))
}

pub fn machine_state_name(s: MachineState) -> &'static str {
    match s {
        MachineState::Starting => "starting",
        MachineState::Running => "running",
        MachineState::Paused => "paused",
        MachineState::Gone => "gone",
    }
}

pub fn outcome_str(o: GoneOutcome) -> &'static str {
    match o {
        GoneOutcome::Settled => "settled",
        GoneOutcome::NotGone => "not-gone",
        GoneOutcome::Unchecked => "unchecked",
        GoneOutcome::Moot => "moot",
    }
}

pub fn phase_name(p: WorkspacePhase) -> &'static str {
    match p {
        WorkspacePhase::Running => "running",
        WorkspacePhase::Pausing => "pausing",
        WorkspacePhase::Napping => "napping",
        WorkspacePhase::Waking => "waking",
        WorkspacePhase::Gone => "gone",
        WorkspacePhase::Queued => "queued",
        WorkspacePhase::Released => "released",
    }
}

pub fn reach_name(r: ReachState) -> String {
    serde_json::to_value(r).unwrap().as_str().unwrap().to_string()
}

/// `Date`-free ISO stamp of an epoch ms, as `new Date(ms).toISOString()`.
pub fn iso(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let milli = ms.rem_euclid(1000);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil(days);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{milli:03}Z", rem / 3600, (rem % 3600) / 60, rem % 60)
}

fn civil(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

// ---------------------------------------------------------------------------------------------------------------
// Machines

#[derive(Clone)]
pub enum Prove {
    Ok,
    Never,
    Fail(EngineError),
    Gate(Arc<tokio::sync::Notify>),
}

/// What a machine was told, and what it answers next.
#[derive(Default)]
pub struct Script {
    /// `None` reads running.
    pub state: Option<Result<MachineState, EngineError>>,
    pub nap: Option<EngineError>,
    pub prove: Option<Prove>,
    pub exec: VecDeque<Result<ExecResult, EngineError>>,
    pub describe: Option<Result<MachineShape, EngineError>>,
    pub resume_err: Option<EngineError>,
    pub resume_at: bool,
    pub execs: Vec<String>,
    pub pauses: Vec<Option<u64>>,
    pub resumes: Vec<Option<(f64, u64)>>,
    pub kills: usize,
}

/// A machine's definition as the oracle's `machine(id, {replayed, state, labels, seen})` writes it.
#[derive(Clone)]
pub struct MachineDef {
    pub id: String,
    pub replayed: bool,
    pub state: Option<MachineState>,
    pub labels: Option<IndexMap<String, String>>,
    pub seen: Option<MachineState>,
    pub stream_url: Option<String>,
}

impl MachineDef {
    pub fn new(id: &str) -> Self {
        MachineDef { id: id.into(), replayed: false, state: None, labels: None, seen: None, stream_url: None }
    }
    pub fn stream(mut self, url: &str) -> Self {
        self.stream_url = Some(url.into());
        self
    }
    pub fn replayed(mut self, state: MachineState) -> Self {
        self.replayed = true;
        self.state = Some(state);
        self
    }
    pub fn labels(mut self, pairs: &[(&str, &str)]) -> Self {
        self.labels = Some(pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect());
        self
    }
    pub fn seen(mut self, state: MachineState) -> Self {
        self.seen = Some(state);
        self
    }
}

pub struct FakeMachine {
    pub id: String,
    log: Log,
    log_state_id: bool,
    log_nap: bool,
    replayed: bool,
    labels: Option<IndexMap<String, String>>,
    seen: Option<Seen>,
    stream_url: Option<String>,
    pub s: RefCell<Script>,
}

impl FakeMachine {
    /// A machine the provider answers: its `state()` logs `["machine.state", id]`.
    pub fn of_provider(log: &Log, def: &MachineDef) -> Arc<FakeMachine> {
        let m = FakeMachine {
            id: def.id.clone(),
            log: log.clone(),
            log_state_id: true,
            log_nap: false,
            replayed: def.replayed,
            labels: def.labels.clone(),
            seen: def.seen.map(|state| Seen { state, created_at: None, paused_at: None }),
            stream_url: def.stream_url.clone(),
            s: RefCell::new(Script::default()),
        };
        m.s.borrow_mut().state = def.state.map(Ok);
        Arc::new(m)
    }

    /// The machine of the oracle's `entryIn`: `state()` logs `["machine.state"]` and a pause logs `["ws.nap"]`.
    pub fn of_entry(log: &Log) -> Arc<FakeMachine> {
        Arc::new(FakeMachine {
            id: "m-1".into(),
            log: log.clone(),
            log_state_id: false,
            log_nap: true,
            replayed: false,
            labels: None,
            seen: None,
            stream_url: None,
            s: RefCell::new(Script::default()),
        })
    }

    /// A machine that logs nothing but answers what it is told, for the tests that read its own record of calls.
    pub fn quiet(id: &str) -> Arc<FakeMachine> {
        Arc::new(FakeMachine {
            id: id.into(),
            log: Log::default(),
            log_state_id: false,
            log_nap: false,
            replayed: false,
            labels: None,
            seen: None,
            stream_url: None,
            s: RefCell::new(Script::default()),
        })
    }

    pub fn set_state(&self, state: Result<MachineState, EngineError>) {
        self.s.borrow_mut().state = Some(state);
    }

    pub fn execs(&self) -> Vec<String> {
        self.s.borrow_mut().execs.clone()
    }
}

#[async_trait(?Send)]
impl Machine for FakeMachine {
    fn id(&self) -> &str {
        &self.id
    }
    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
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
    fn replayed(&self) -> bool {
        self.replayed
    }
    fn offers(&self) -> MachineOffers {
        let s = self.s.borrow_mut();
        MachineOffers { prove_road: s.prove.is_some(), describe: s.describe.is_some(), resume_at: s.resume_at, ..Default::default() }
    }

    async fn exec(&self, cmd: &str, _opts: ExecOptions) -> Result<ExecResult, EngineError> {
        let next = {
            let mut s = self.s.borrow_mut();
            s.execs.push(cmd.to_string());
            s.exec.pop_front()
        };
        tick().await;
        next.unwrap_or_else(|| Ok(ok_exec("")))
    }

    async fn run(&self, script: &str, _opts: RunOptions) -> Result<ExecResult, EngineError> {
        let next = {
            let mut s = self.s.borrow_mut();
            s.execs.push(script.to_string());
            s.exec.pop_front()
        };
        tick().await;
        next.unwrap_or_else(|| Ok(ok_exec("")))
    }

    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> Result<String, EngineError> {
        Err(plain("snapshot is not faked"))
    }

    async fn pause(&self, wait_ms: Option<u64>) -> Result<(), EngineError> {
        if self.log_nap {
            self.log.push(json!(["ws.nap"]));
        }
        let err = {
            let mut s = self.s.borrow_mut();
            s.pauses.push(wait_ms);
            s.nap.clone()
        };
        tick().await;
        err.map_or(Ok(()), Err)
    }

    async fn resume(&self, _signal: Option<&Cancel>) -> Result<(), EngineError> {
        let err = {
            let mut s = self.s.borrow_mut();
            s.resumes.push(None);
            s.resume_err.clone()
        };
        tick().await;
        err.map_or(Ok(()), Err)
    }

    async fn resume_at(&self, cpu: f64, mem_mb: u64, _signal: Option<&Cancel>) -> Result<(), EngineError> {
        let err = {
            let mut s = self.s.borrow_mut();
            s.resumes.push(Some((cpu, mem_mb)));
            s.resume_err.clone()
        };
        tick().await;
        err.map_or(Ok(()), Err)
    }

    async fn kill(&self) -> Result<(), EngineError> {
        self.s.borrow_mut().kills += 1;
        tick().await;
        Ok(())
    }

    async fn state(&self) -> Result<MachineState, EngineError> {
        if self.log_state_id {
            self.log.push(json!(["machine.state", self.id]));
        } else {
            self.log.push(json!(["machine.state"]));
        }
        tick().await;
        match &self.s.borrow_mut().state {
            None => Ok(MachineState::Running),
            Some(Ok(s)) => Ok(*s),
            Some(Err(e)) => Err(e.clone()),
        }
    }

    async fn download_url(&self, _path: &str) -> Result<String, EngineError> {
        Err(plain("download_url is not faked"))
    }

    async fn upload_url(&self, _path: &str) -> Result<String, EngineError> {
        Err(plain("upload_url is not faked"))
    }

    async fn prove_road(&self, _signal: Option<&Cancel>) -> Result<(), EngineError> {
        let prove = self.s.borrow_mut().prove.clone();
        match prove {
            None | Some(Prove::Ok) => Ok(()),
            Some(Prove::Never) => futures::future::pending::<Result<(), EngineError>>().await,
            Some(Prove::Fail(e)) => Err(e),
            Some(Prove::Gate(n)) => {
                n.notified().await;
                Ok(())
            }
        }
    }

    async fn describe(&self) -> Result<MachineShape, EngineError> {
        let d = self.s.borrow_mut().describe.clone();
        d.unwrap_or_else(|| Err(plain("describe is not faked")))
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The provider

pub enum Made {
    Machine(MachineDef),
    Fail(EngineError),
}

pub enum Replay {
    Machine(MachineDef),
    Fail(EngineError),
    /// `null`: past the key's window, nothing sent.
    Window,
}

#[derive(Default)]
pub struct BackendOpts {
    pub creates: Vec<Made>,
    pub replays: Option<Vec<Replay>>,
    pub request: bool,
    pub named: Option<Vec<NamedMachine>>,
    /// A lifecycle: wake attempts, whether it takes a backstop, whether it reads a quiet figure.
    pub life: Option<(u32, bool, bool)>,
}

/// How `get` answers: as the oracle's `backendWith` (`backend.get`), or as its `readGone` mock reads one state.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GetMode {
    Plain,
    Reads,
}

pub type Read = Result<MachineState, EngineError>;

pub struct BScript {
    pub creates: VecDeque<Made>,
    pub replays: Option<VecDeque<Replay>>,
    pub request: bool,
    pub named: Option<Vec<NamedMachine>>,
    pub mode: GetMode,
    pub reads: VecDeque<Read>,
    series: Option<Read>,
    gone_run: usize,
    pub names_workspace: bool,
    pub created: Vec<MachineSpec>,
    pub gets: Vec<String>,
}

/// The reads in a row the engine's `readGone` needs before it answers gone.
const GONE_READS: usize = 12;

struct Pricing;
impl BackendPricing for Pricing {
    fn rate_usd_per_hour(&self, _size: Size) -> f64 {
        0.0
    }
    fn default_size(&self) -> Size {
        Size { cpu: 1.0, mem_mb: 2048.0 }
    }
    fn snapshot_storage(&self) -> wsp_engine::machine::SnapshotStoragePricing {
        wsp_engine::machine::SnapshotStoragePricing {
            wire: from(json!({ "freeGb": 0, "usdPerGbMonth": 0, "billedFrom": "creation" })),
            free_named: None,
            usd_per_named_month: None,
        }
    }
}

pub struct LifeState {
    pub quiet: Option<Result<Option<u64>, EngineError>>,
    pub backstops: Vec<(String, i64)>,
    pub backstop_err: Option<EngineError>,
}

pub struct FakeLifecycle {
    pub wake_attempts: u32,
    pub has_backstop: bool,
    pub has_quiet: bool,
    pub st: RefCell<LifeState>,
}

#[async_trait(?Send)]
impl Lifecycle for FakeLifecycle {
    fn budgets(&self) -> LifecycleBudgets {
        from(json!({ "wakeAttempts": self.wake_attempts, "daemonAnswersMs": 1000 }))
    }
    fn has_backstop(&self) -> bool {
        self.has_backstop
    }
    async fn backstop(&self, machine: &dyn Machine, until_ms: i64, _exact: bool) -> Result<(), EngineError> {
        let err = {
            let mut s = self.st.borrow_mut();
            let err = s.backstop_err.clone();
            if err.is_none() {
                s.backstops.push((machine.id().to_string(), until_ms));
            }
            err
        };
        err.map_or(Ok(()), Err)
    }
    fn has_quiet_for_ms(&self) -> bool {
        self.has_quiet
    }
    async fn quiet_for_ms(&self, _machine: &dyn Machine) -> Result<Option<u64>, EngineError> {
        match &self.st.borrow_mut().quiet {
            None => Ok(None),
            Some(r) => r.clone(),
        }
    }
}

pub struct FakeBackend {
    pub name: String,
    log: Log,
    pub b: RefCell<BScript>,
    pub life: Option<Arc<FakeLifecycle>>,
}

impl FakeBackend {
    pub fn new(log: &Log, name: &str, o: BackendOpts) -> Arc<FakeBackend> {
        let o_life = o.life;
        Arc::new(FakeBackend {
            name: name.into(),
            log: log.clone(),
            b: RefCell::new(BScript {
                creates: o.creates.into(),
                replays: o.replays.map(Into::into),
                request: o.request,
                named: o.named,
                mode: GetMode::Plain,
                reads: VecDeque::new(),
                series: None,
                gone_run: 0,
                names_workspace: false,
                created: Vec::new(),
                gets: Vec::new(),
            }),
            life: o_life.map(|(wake_attempts, has_backstop, has_quiet)| {
                Arc::new(FakeLifecycle { wake_attempts, has_backstop, has_quiet, st: RefCell::new(LifeState { quiet: None, backstops: Vec::new(), backstop_err: None }) })
            }),
        })
    }

    /// A provider whose lifecycle the area reads: wake attempts, a quiet figure and a backstop call.
    pub fn with_lifecycle(log: &Log, name: &str, wake_attempts: u32, has_backstop: bool, has_quiet: bool) -> Arc<FakeBackend> {
        Arc::new(FakeBackend {
            name: name.into(),
            log: log.clone(),
            b: RefCell::new(BScript {
                creates: VecDeque::new(),
                replays: None,
                request: false,
                named: None,
                mode: GetMode::Plain,
                reads: VecDeque::new(),
                series: None,
                gone_run: 0,
                names_workspace: false,
                created: Vec::new(),
                gets: Vec::new(),
            }),
            life: Some(Arc::new(FakeLifecycle {
                wake_attempts,
                has_backstop,
                has_quiet,
                st: RefCell::new(LifeState { quiet: None, backstops: Vec::new(), backstop_err: None }),
            })),
        })
    }

    pub fn reads(self: &Arc<Self>, reads: Vec<Read>) {
        let mut b = self.b.borrow_mut();
        b.mode = GetMode::Reads;
        b.reads = reads.into();
    }

    fn make(&self, def: &MachineDef) -> Arc<dyn Machine> {
        FakeMachine::of_provider(&self.log, def)
    }

    pub fn created(&self) -> Vec<MachineSpec> {
        self.b.borrow_mut().created.clone()
    }
}

fn labels_json(spec: &MachineSpec) -> Value {
    match &spec.labels {
        Some(l) => serde_json::to_value(l).unwrap(),
        None => Value::Null,
    }
}

/// The request text as the oracle writes it: the object `{"body": "bytes"}` the fixture shows.
pub const REQUEST: &str = r#"{"body":"bytes"}"#;

fn request_json(sent: &Option<String>) -> Value {
    match sent {
        Some(s) => serde_json::from_str(s).unwrap_or_else(|_| Value::String(s.clone())),
        None => Value::Null,
    }
}

#[async_trait(?Send)]
impl MachineBackend for FakeBackend {
    fn capabilities(&self) -> Capabilities {
        from(json!({
            "liveCloneForks": false, "replacesMachine": false, "previewUrls": false, "signedUrls": false, "callbackRelay": false,
            "diskSnapshots": false, "images": false, "snapshotsAnyLife": false, "snapshotListing": false, "templates": false,
            "kept": false, "copies": false, "ownNetwork": false, "sizes": []
        }))
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
        Arc::new(Pricing)
    }
    fn lifecycle(&self) -> Option<Arc<dyn Lifecycle>> {
        self.life.clone().map(|l| l as Arc<dyn Lifecycle>)
    }
    fn names_workspace(&self) -> bool {
        self.b.borrow_mut().names_workspace
    }
    fn offers(&self) -> BackendOffers {
        let b = self.b.borrow_mut();
        BackendOffers { keyed_create: b.request || b.replays.is_some() || b.named.is_some(), ..Default::default() }
    }

    async fn create(&self, spec: MachineSpec, waiting: Option<Waiting>, sent: Option<String>) -> Result<Arc<dyn Machine>, EngineError> {
        self.log.push(json!(["backend.create", spec.idempotency_key, labels_json(&spec), request_json(&sent)]));
        self.b.borrow_mut().created.push(spec.clone());
        if let Some(w) = waiting {
            w("waiting for a box", Some(&ProviderBoxState { id: "box-early".into(), state: String::new() }));
        }
        let next = self.b.borrow_mut().creates.pop_front();
        tick().await;
        match next {
            Some(Made::Fail(e)) => Err(e),
            Some(Made::Machine(def)) => Ok(self.make(&def)),
            None => Ok(self.make(&MachineDef::new("m-made"))),
        }
    }

    async fn get(&self, id: &str) -> Result<Arc<dyn Machine>, EngineError> {
        let mode = self.b.borrow_mut().mode;
        if mode == GetMode::Plain {
            self.log.push(json!(["backend.get", id]));
            self.b.borrow_mut().gets.push(id.to_string());
            tick().await;
            return Ok(self.make(&MachineDef::new(id)));
        }
        let read = {
            let mut b = self.b.borrow_mut();
            if b.series.is_none() {
                self.log.push(json!(["readGone", id]));
                let r = b.reads.pop_front().unwrap_or(Ok(MachineState::Gone));
                b.series = Some(r);
            }
            let r = b.series.clone().unwrap();
            match &r {
                Ok(MachineState::Gone) => {
                    b.gone_run += 1;
                    if b.gone_run >= GONE_READS {
                        b.gone_run = 0;
                        b.series = None;
                    }
                }
                _ => {
                    b.gone_run = 0;
                    b.series = None;
                }
            }
            r
        };
        tick().await;
        match read {
            Err(e) => Err(e),
            Ok(state) => Ok(self.make(&MachineDef::new(id).seen(state))),
        }
    }

    async fn list(&self, _labels: Option<&IndexMap<String, String>>) -> Result<Vec<MachineListRow>, EngineError> {
        Ok(Vec::new())
    }

    async fn delete_snapshot(&self, _id: &str) -> Result<(), EngineError> {
        Ok(())
    }

    async fn create_request(&self, spec: &MachineSpec) -> Result<String, EngineError> {
        self.log.push(json!(["createRequest", labels_json(spec)]));
        Ok(REQUEST.to_string())
    }

    async fn replay_create(&self, key: &str, bytes: &str, made_at: &str) -> Result<Option<Arc<dyn Machine>>, EngineError> {
        self.log.push(json!(["replayCreate", key, request_json(&Some(bytes.to_string())), made_at]));
        let next = self.b.borrow_mut().replays.as_mut().and_then(|r| r.pop_front());
        tick().await;
        match next {
            Some(Replay::Fail(e)) => Err(e),
            Some(Replay::Window) => Ok(None),
            Some(Replay::Machine(def)) => Ok(Some(self.make(&def))),
            None => panic!("replay_create asked with no replay scripted"),
        }
    }

    async fn named_after(&self, names: &std::collections::BTreeSet<String>) -> Result<Vec<NamedMachine>, EngineError> {
        self.log.push(json!(["namedAfter", names.iter().collect::<Vec<_>>()]));
        Ok(self.b.borrow_mut().named.clone().unwrap_or_default())
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The runtime

pub struct FakePlaces {
    pub wired: String,
    pub backends: RefCell<IndexMap<String, Arc<dyn MachineBackend>>>,
}

impl PlaceBackends for FakePlaces {
    fn wired(&self) -> String {
        self.wired.clone()
    }
    fn backend(&self, place: &str) -> Option<Arc<dyn MachineBackend>> {
        self.backends.borrow().get(place).cloned()
    }
    fn list(&self) -> Vec<String> {
        self.backends.borrow().keys().cloned().collect()
    }
}

/// The idle policy's door, which also does what wsp-core's policy does on a touch: it reads the window and hands the
/// backstop instant (now plus the backstop of that window) back to the area, as the TS policy's `arm` does.
pub struct FakeIdle {
    pub calls: RefCell<Vec<(&'static str, String)>>,
    pub area: RefCell<Weak<MachinesArea>>,
    pub clock: Rc<FakeClock>,
}

impl IdleDoor for FakeIdle {
    fn touch(&self, workspace_id: &str) {
        self.calls.borrow_mut().push(("touch", workspace_id.to_string()));
        let area = self.area.borrow().upgrade();
        if let Some(area) = area {
            let window: Option<f64> = area.idle_window_of_id(workspace_id).flatten();
            let backstop = window.map_or(6 * 60 * 60_000, |w| (w * 2.0) as i64);
            area.on_backstop(workspace_id, self.clock.now() + backstop);
        }
    }
    fn forget(&self, workspace_id: &str) {
        self.calls.borrow_mut().push(("forget", workspace_id.to_string()));
    }
}

/// The module of a kind: only `dropped` is faked, the oracle's `{ dropped }`.
pub struct FakeKind {
    pub kind: String,
    pub log: Log,
    pub dropped_err: Option<String>,
}

impl KindModule for FakeKind {
    fn backend(&self, _record: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError> {
        unimplemented!("fake kind")
    }
    fn exec_stream(&self, _entry: &LiveWorkspace, _opts: Option<MachineExecOptions>, _waiting: Option<TurnWaiting>) -> Arc<dyn ExecStreamFactory> {
        unimplemented!("fake kind")
    }
    fn admit_folder(&self, _record: FolderAdmit) -> Option<Local<'static, ()>> {
        unimplemented!("fake kind")
    }
    fn folder(&self, _record: &WorkspaceRecord) -> Option<String> {
        unimplemented!("fake kind")
    }
    fn home(&self, _entry: &LiveWorkspace, _agent_id: &str) -> String {
        unimplemented!("fake kind")
    }
    fn home_dir(&self, _record: &WorkspaceRecord) -> Option<String> {
        unimplemented!("fake kind")
    }
    fn env(&self, _entry: &LiveWorkspace, _agent_id: Option<&str>, _home_of: &dyn Fn(&str) -> String) -> IndexMap<String, String> {
        unimplemented!("fake kind")
    }
    fn login_stands(&self, _entry: &LiveWorkspace, _agent_id: &str) -> bool {
        unimplemented!("fake kind")
    }
    fn import(&self, _entry: &LiveWorkspace, _o: ProjectImportOptions, _report: ImportReport) -> Local<'static, ImportLanded> {
        unimplemented!("fake kind")
    }
    fn roots(&self, _entry: &LiveWorkspace, _dests: &[String]) -> Local<'static, ()> {
        unimplemented!("fake kind")
    }
    fn land_project(&self, _entry: &LiveWorkspace, _project: wsp_wire::views::workspace::ProjectView, _report: StageReport) -> Local<'static, ()> {
        unimplemented!("fake kind")
    }
    fn memory_key(&self, _entry: &LiveWorkspace, _agent_id: &str) -> Option<String> {
        unimplemented!("fake kind")
    }
    fn end_thread(&self, _entry: &LiveWorkspace, _thread_id: &str, _o: EndThread) -> Option<Local<'static, Option<String>>> {
        unimplemented!("fake kind")
    }
    fn end_owed(&self, _entry: &LiveWorkspace, _thread_id: &str) -> Option<Local<'static, Option<Local<'static, ()>>>> {
        unimplemented!("fake kind")
    }
    fn port_reach(&self, _entry: &LiveWorkspace, _port: u16, _standing: bool) -> Option<Local<'static, PortRoute>> {
        unimplemented!("fake kind")
    }
    fn relayed(&self, _machine_id: Option<&str>) -> bool {
        unimplemented!("fake kind")
    }
    fn wsp_mcp(&self, _entry: &LiveWorkspace) -> Option<wsp_wire::views::harness::McpServerSpec> {
        unimplemented!("fake kind")
    }
    fn turn_reach(&self, _entry: &LiveWorkspace) -> Option<Option<String>> {
        unimplemented!("fake kind")
    }
    fn turn_road(&self) -> wsp_core::types::pending::ScopedRoad {
        unimplemented!("fake kind")
    }
    fn keeps_agents(&self) -> bool {
        unimplemented!("fake kind")
    }
    fn server_values(&self) -> ServerValuesRoad {
        unimplemented!("fake kind")
    }
    fn has_daemon(&self, _entry: &LiveWorkspace) -> bool {
        unimplemented!("fake kind")
    }
    fn shared_daemon(&self) -> bool {
        unimplemented!("fake kind")
    }
    fn daemon_road(&self, _entry: &LiveWorkspace) -> Local<'static, wsp_wire::views::workspace::DaemonReachView> {
        unimplemented!("fake kind")
    }
    fn scratch(&self, _entry: &LiveWorkspace) -> String {
        unimplemented!("fake kind")
    }
    fn dropped(&self, entry: &LiveWorkspace) -> Local<'static, ()> {
        self.log.push(json!(["module.dropped", self.kind, entry.record.borrow().id]));
        let err = self.dropped_err.clone();
        async move {
            match err {
                Some(m) => Err(HostError::new(m)),
                None => Ok(()),
            }
        }
        .boxed_local()
    }
    fn daemon_version(&self, _entry: &LiveWorkspace) -> Local<'static, Option<u32>> {
        unimplemented!("fake kind")
    }
    fn restart_daemon(&self, _entry: &LiveWorkspace) -> Option<Local<'static, ()>> {
        unimplemented!("fake kind")
    }
    fn deploy_daemon(&self, _entry: &LiveWorkspace) -> Option<Local<'static, Option<String>>> {
        unimplemented!("fake kind")
    }
}

#[derive(Default)]
pub struct RtCfg {
    pub keeps_images: bool,
    pub names_workspace: bool,
    pub pause_keeps_disk: bool,
    pub at_provider: bool,
    pub place_nap_ms: Option<Option<f64>>,
    pub recorded_images: Vec<String>,
    pub image: ImageRead,
    pub shape: Option<MachineShape>,
    pub size_built: Option<WorkspaceSize>,
    pub size_built_calls: Vec<(Option<MachineShape>, WorkspaceSize)>,
    pub read_memory_calls: Vec<(String, String)>,
    pub vault_export: VecDeque<Result<Vec<u8>, HostError>>,
    pub vault_calls: Vec<(String, u64, Option<Vec<String>>)>,
    pub gone_stop_err: Option<HostError>,
    pub ping: Option<String>,
    pub nap_reason: Option<String>,
    pub stop_reason: Option<String>,
    pub dropped_err: Option<String>,
    pub warns: Vec<String>,
    /// A store whose `put` into this collection fails with this message.
    pub fail_put: Option<(String, String)>,
}

/// The store with one collection's writes refused.
pub struct FailPut {
    pub inner: Rc<MemStore>,
    pub collection: String,
    pub message: String,
}

#[async_trait(?Send)]
impl Store for FailPut {
    async fn get(&self, c: &str, id: &str) -> Result<Option<Value>, HostError> {
        self.inner.get(c, id).await
    }
    async fn put(&self, c: &str, id: &str, value: Value) -> Result<(), HostError> {
        if c == self.collection {
            return Err(HostError::new(self.message.clone()));
        }
        self.inner.put(c, id, value).await
    }
    async fn list(&self, c: &str) -> Result<Vec<Value>, HostError> {
        self.inner.list(c).await
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
    async fn put_blob(&self, c: &str, id: &str, bytes: Vec<u8>) -> Result<(), HostError> {
        self.inner.put_blob(c, id, bytes).await
    }
    async fn delete_blob(&self, c: &str, id: &str) -> Result<(), HostError> {
        self.inner.delete_blob(c, id).await
    }
    async fn stat_blob(&self, c: &str, id: &str) -> Result<Option<wsp_core::types::pending::BlobMark>, HostError> {
        self.inner.stat_blob(c, id).await
    }
}

pub struct Rt {
    pub log: Log,
    pub clock: Rc<FakeClock>,
    pub ids: Rc<FakeIds>,
    pub store: Rc<MemStore>,
    pub backend: RefCell<Arc<FakeBackend>>,
    pub places: Rc<FakePlaces>,
    pub area: RefCell<Weak<MachinesArea>>,
    pub on_exec: RefCell<Option<Rc<dyn Fn(&GoldenExec) -> Result<(), String>>>>,
    pub idle: Rc<FakeIdle>,
    pub live: Rc<RefCell<IndexMap<String, Entry>>>,
    pub sessions: Rc<RefCell<IndexMap<String, Row>>>,
    pub thread_records: Rc<RefCell<IndexMap<String, ThreadOn>>>,
    pub unreached: Rc<RefCell<IndexMap<String, Unreached>>>,
    pub nap_refusals: Rc<RefCell<IndexMap<String, Refusal>>>,
    pub stop_refusals: Rc<RefCell<IndexMap<String, Refusal>>>,
    pub projects_held: RefCell<Vec<String>>,
    pub kept: RefCell<Vec<(String, String)>>,
    pub execs: RefCell<Vec<String>>,
    /// Calls the oracle's real maps and sets take without a log line.
    pub unlogged: RefCell<Vec<String>>,
    lazy_records: RefCell<Vec<(usize, String)>>,
    pub cfg: RefCell<RtCfg>,
}

impl Rt {
    fn id_of(entry: &Entry) -> String {
        entry.borrow().record.borrow().id.clone()
    }
}

#[async_trait(?Send)]
impl MachinesRuntime for Rt {
    fn store(&self) -> Rc<dyn Store> {
        match self.cfg.borrow().fail_put.clone() {
            Some((collection, message)) => Rc::new(FailPut { inner: self.store.clone(), collection, message }),
            None => self.store.clone(),
        }
    }
    fn clock(&self) -> Rc<dyn Clock> {
        self.clock.clone()
    }
    fn ids(&self) -> Rc<dyn Ids> {
        self.ids.clone()
    }
    fn backend(&self) -> Arc<dyn MachineBackend> {
        self.backend.borrow().clone()
    }
    fn places(&self) -> Rc<dyn PlaceBackends> {
        self.places.clone()
    }
    fn emit(&self, event: EventUnion) -> Result<(), HostError> {
        self.log.push(json!(["emit", serde_json::to_value(&event).unwrap()]));
        let area = self.area.borrow().upgrade();
        if let Some(a) = area {
            a.on_event(&event);
        }
        Ok(())
    }
    fn warn(&self, line: &str) {
        self.cfg.borrow_mut().warns.push(line.to_string());
        self.log.push(json!(["warn", line]));
    }
    fn owner(&self) -> String {
        "h_0a993e4c".into()
    }
    fn kill_confirm(&self) -> Option<KillConfirm> {
        Some(KillConfirm::default())
    }
    fn on_exec(&self) -> Option<Rc<dyn Fn(&GoldenExec) -> Result<(), String>>> {
        self.on_exec.borrow().clone()
    }
    fn idle(&self) -> Rc<dyn IdleDoor> {
        self.idle.clone()
    }

    fn live(&self) -> Rc<RefCell<IndexMap<String, Entry>>> {
        self.live.clone()
    }
    fn sessions(&self) -> Rc<RefCell<IndexMap<String, Row>>> {
        self.sessions.clone()
    }
    fn thread_records(&self) -> Rc<RefCell<IndexMap<String, ThreadOn>>> {
        self.thread_records.clone()
    }
    fn unreached(&self) -> Rc<RefCell<IndexMap<String, Unreached>>> {
        self.unreached.clone()
    }
    fn nap_refusals(&self) -> Rc<RefCell<IndexMap<String, Refusal>>> {
        self.nap_refusals.clone()
    }
    fn stop_refusals(&self) -> Rc<RefCell<IndexMap<String, Refusal>>> {
        self.stop_refusals.clone()
    }
    fn project_held(&self, project_id: &str) -> bool {
        self.projects_held.borrow().iter().any(|p| p == project_id)
    }
    fn kept_agents_on(&self, workspace_id: &str) -> Vec<String> {
        self.kept.borrow().iter().filter(|(_, ws)| ws == workspace_id).map(|(t, _)| t.clone()).collect()
    }

    fn end_execs(&self, workspace_id: &str, reason: &str) {
        for ws in self.execs.borrow().iter() {
            if ws == workspace_id {
                self.log.push(json!(["exec.end", reason]));
            }
        }
    }
    async fn entry_of(&self, id: &str) -> Result<Entry, HostError> {
        tick().await;
        self.live.borrow().get(id).cloned().ok_or_else(|| HostError::new(format!("no entry {id}")))
    }
    fn view(&self, record: &WorkspaceRecord) -> WorkspaceView {
        from(json!({
            "id": record.id, "name": record.name, "machineId": record.machine_id, "phase": phase_name(record.phase),
            "golden": record.golden, "createdAt": record.created_at,
            "project": { "id": record.project, "name": "", "path": "", "computer": "" }
        }))
    }
    async fn persist(&self, record: &WorkspaceRecord) -> Result<(), HostError> {
        self.log.push(json!(["persist", serde_json::to_value(record).unwrap()]));
        tick().await;
        Ok(())
    }
    async fn emit_status(&self, entry: &Entry, reach: ReachState, said: Option<&str>) -> Result<(), HostError> {
        let id = Rt::id_of(entry);
        self.log.push(json!(["emitStatus", id, reach_name(reach), said]));
        tick().await;
        Ok(())
    }
    fn reach_of(&self, _entry: &Entry) -> ReachState {
        ReachState::Reachable
    }
    fn nap_refused_of(&self, entry: &Entry) -> Option<String> {
        let id = Rt::id_of(entry);
        self.nap_refusals.borrow().get(&id).map(|r| r.said.clone())
    }
    fn nap_refused_reason(&self, _entry: &Entry) -> Option<String> {
        self.cfg.borrow().nap_reason.clone()
    }
    fn stop_refused_reason(&self, _entry: &Entry) -> Option<String> {
        self.cfg.borrow().stop_reason.clone()
    }
    async fn says_waking(&self, entry: &Entry, line: &str) -> Result<(), HostError> {
        let id = Rt::id_of(entry);
        self.log.push(json!(["saysWaking", id, line]));
        Ok(())
    }
    async fn ping_daemon(&self, entry: &Entry) -> Option<String> {
        let id = Rt::id_of(entry);
        self.log.push(json!(["pingDaemon", id]));
        self.cfg.borrow().ping.clone()
    }
    fn sync_daemon(&self, entry: &Entry) {
        let id = Rt::id_of(entry);
        self.log.push(json!(["syncDaemon", id]));
    }
    async fn write_daemon_roots(&self, entry: &Entry) -> Result<(), HostError> {
        let id = Rt::id_of(entry);
        self.log.push(json!(["writeDaemonRoots", id]));
        Ok(())
    }
    async fn sub_lost(&self, entry: &Entry, reason: &str) -> Result<GoneOutcome, HostError> {
        let id = Rt::id_of(entry);
        self.log.push(json!(["subLost", id, reason]));
        Ok(GoneOutcome::Settled)
    }
    fn backend_for(&self, _record: &WorkspaceRecord) -> Arc<dyn MachineBackend> {
        self.backend.borrow().clone()
    }
    fn module_of(&self, kind: &str) -> Rc<dyn KindModule> {
        Rc::new(FakeKind { kind: kind.into(), log: self.log.clone(), dropped_err: self.cfg.borrow().dropped_err.clone() })
    }
    fn keeps_images(&self, _backend: &Arc<dyn MachineBackend>) -> bool {
        self.cfg.borrow().keeps_images
    }
    fn names_workspace(&self, _backend: &Arc<dyn MachineBackend>) -> bool {
        self.cfg.borrow().names_workspace
    }
    fn pause_keeps_disk(&self, _backend: &Arc<dyn MachineBackend>) -> bool {
        self.cfg.borrow().pause_keeps_disk
    }
    async fn image_of(&self, _golden: &str) -> Result<ImageRead, HostError> {
        Ok(self.cfg.borrow().image.clone())
    }
    async fn recorded_images(&self) -> Result<Vec<String>, HostError> {
        Ok(self.cfg.borrow().recorded_images.clone())
    }
    fn place_id_of(&self, record: &WorkspaceRecord) -> Option<String> {
        let at = self.log.0.borrow().len();
        self.log.push(json!(["placeIdOf", serde_json::to_value(record).unwrap()]));
        self.lazy_records.borrow_mut().push((at, record.id.clone()));
        record.place.clone()
    }
    fn at_provider(&self, _place_id: Option<&str>) -> bool {
        self.cfg.borrow().at_provider
    }
    fn place_name_of(&self, place: &str) -> String {
        format!("name-of-{place}")
    }
    fn place_nap_ms(&self, _place_id: Option<&str>) -> Option<Option<f64>> {
        self.cfg.borrow().place_nap_ms
    }
    async fn shape_of(&self, _machine: &Arc<dyn Machine>) -> Option<MachineShape> {
        self.cfg.borrow().shape.clone()
    }
    fn size_built(&self, shape: Option<&MachineShape>, asked: WorkspaceSize) -> WorkspaceSize {
        let mut cfg = self.cfg.borrow_mut();
        cfg.size_built_calls.push((shape.cloned(), asked.clone()));
        cfg.size_built.clone().unwrap_or(asked)
    }
    async fn read_memory(&self, record: &Rc<RefCell<WorkspaceRecord>>, machine: &Arc<dyn Machine>, _sizes: &[WorkspaceSize]) -> bool {
        self.cfg.borrow_mut().read_memory_calls.push((record.borrow().id.clone(), machine.id().to_string()));
        true
    }
    async fn vault_export(&self, machine: &Arc<dyn Machine>, max_bytes: u64, leave: Option<Vec<String>>) -> Result<Vec<u8>, HostError> {
        let mut cfg = self.cfg.borrow_mut();
        cfg.vault_calls.push((machine.id().to_string(), max_bytes, leave));
        cfg.vault_export.pop_front().unwrap_or_else(|| Ok(b"vault".to_vec()))
    }
    async fn gone_stop(&self, _at: &Arc<dyn MachineBackend>, machine: &Arc<dyn Machine>) -> Result<(), HostError> {
        let err = self.cfg.borrow().gone_stop_err.clone();
        if let Some(e) = err {
            return Err(e);
        }
        self.log.push(json!(["gone.stop", machine.id()]));
        tick().await;
        Ok(())
    }

    fn forget_workspace_maps(&self, workspace_id: &str) {
        self.unlogged.borrow_mut().push(format!("forget_workspace_maps {workspace_id}"));
    }
    fn reap_kept(&self, thread_id: &str) {
        self.log.push(json!(["reapKept", thread_id]));
    }
    async fn forget_slate(&self, thread_id: &str) -> Result<(), HostError> {
        self.log.push(json!(["slates.forget", thread_id]));
        Ok(())
    }
    fn forget_viewed(&self, workspace_id: &str) {
        self.unlogged.borrow_mut().push(format!("forget_viewed {workspace_id}"));
    }
    fn cancel_flush(&self, workspace_id: &str) {
        self.log.push(json!(["cancelFlush", workspace_id]));
    }
    async fn settle_flushes(&self, workspace_id: &str) {
        self.unlogged.borrow_mut().push(format!("settle_flushes {workspace_id}"));
    }
    fn clear_rows(&self, workspace_id: &str) {
        self.log.push(json!(["rows.clear", workspace_id]));
    }
    async fn drop_sent_images(&self, workspace_id: &str) -> Result<(), HostError> {
        self.log.push(json!(["dropSentImages", workspace_id, "all"]));
        Ok(())
    }
    fn forget_kept_writes(&self, workspace_id: &str) {
        self.unlogged.borrow_mut().push(format!("forget_kept_writes {workspace_id}"));
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The world

pub struct Opts {
    /// The scene's `store` seed (collection -> key -> row).
    pub store: Value,
    /// The runtime's own provider; a plain one where `None`.
    pub backend: Option<BackendOpts>,
    /// The `readGone` answers, one per read series, answered by the runtime's provider.
    pub reads: Option<Vec<Read>>,
    pub gone_confirm_ms: u64,
    pub config: Option<MachinesConfig>,
}

impl Default for Opts {
    fn default() -> Self {
        Opts { store: json!({}), backend: None, reads: None, gone_confirm_ms: 0, config: None }
    }
}

pub struct World {
    pub log: Log,
    pub clock: Rc<FakeClock>,
    pub store: Rc<MemStore>,
    pub rt: Rc<Rt>,
    pub area: Rc<MachinesArea>,
    pub backend: Arc<FakeBackend>,
}

pub fn config(gone_confirm_ms: u64) -> MachinesConfig {
    MachinesConfig {
        gone_confirm_ms,
        late_read_ms: 180_000,
        vault_cap_bytes: 1000,
        default_idle_window_ms: Some(1_200_000.0),
        host_id: "host:ABC-1".into(),
        poll_interval_ms: 15_000,
    }
}

/// The seed rows as the store holds them: an attempt's `request` is the text the provider's `create_request` made,
/// where the oracle's fake made the object `{"body": "bytes"}`.
pub fn seed_of(v: &Value) -> Value {
    match v {
        Value::Object(o) => Value::Object(
            o.iter()
                .map(|(k, x)| match (k.as_str(), x) {
                    ("request", Value::Object(_)) => (k.clone(), Value::String(x.to_string())),
                    _ => (k.clone(), seed_of(x)),
                })
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(seed_of).collect()),
        other => other.clone(),
    }
}

impl World {
    pub fn new(o: Opts) -> World {
        let log = Log::default();
        let clock = FakeClock::new(T0);
        let store = MemStore::seeded(&seed_of(&o.store), log.clone(), StoreLogs { put: true, delete: true, blobs: true, ..Default::default() });
        let backend = FakeBackend::new(&log, "solari", o.backend.unwrap_or_default());
        if let Some(reads) = o.reads {
            backend.reads(reads);
        }
        let places = Rc::new(FakePlaces { wired: "solari".into(), backends: RefCell::new(IndexMap::new()) });
        places.backends.borrow_mut().insert("pl1".into(), backend.clone() as Arc<dyn MachineBackend>);
        let rt = Rc::new(Rt {
            log: log.clone(),
            clock: clock.clone(),
            ids: Rc::new(FakeIds::default()),
            store: store.clone(),
            backend: RefCell::new(backend.clone()),
            places,
            area: RefCell::new(Weak::new()),
            on_exec: RefCell::new(None),
            idle: Rc::new(FakeIdle { calls: RefCell::new(Vec::new()), area: RefCell::new(Weak::new()), clock: clock.clone() }),
            live: Rc::new(RefCell::new(IndexMap::new())),
            sessions: Rc::new(RefCell::new(IndexMap::new())),
            thread_records: Rc::new(RefCell::new(IndexMap::new())),
            unreached: Rc::new(RefCell::new(IndexMap::new())),
            nap_refusals: Rc::new(RefCell::new(IndexMap::new())),
            stop_refusals: Rc::new(RefCell::new(IndexMap::new())),
            projects_held: RefCell::new(vec!["p_1".into()]),
            kept: RefCell::new(Vec::new()),
            execs: RefCell::new(Vec::new()),
            unlogged: RefCell::new(Vec::new()),
            lazy_records: RefCell::new(Vec::new()),
            cfg: RefCell::new(RtCfg::default()),
        });
        let area = MachinesArea::new(rt.clone() as Rc<dyn MachinesRuntime>, o.config.unwrap_or_else(|| config(o.gone_confirm_ms)));
        *rt.area.borrow_mut() = Rc::downgrade(&area);
        *rt.idle.area.borrow_mut() = Rc::downgrade(&area);
        World { log, clock, store, rt, area, backend }
    }

    /// The log as the oracle writes it: an object an entry names by its record is read when the scene ends.
    pub fn log_now(&self) -> Vec<Value> {
        let mut log = self.log.take();
        for (at, id) in self.rt.lazy_records.borrow().iter() {
            if let Some(entry) = self.rt.live.borrow().get(id) {
                if let Some(slot) = log.get_mut(*at) {
                    slot[1] = serde_json::to_value(&*entry.borrow().record.borrow()).unwrap();
                }
            }
        }
        log
    }

    /// A provider that logs into this world's log, as the oracle's `backendWith`.
    pub fn provider(&self, o: BackendOpts) -> Arc<FakeBackend> {
        FakeBackend::new(&self.log, "solari", o)
    }

    pub fn warns(&self) -> Vec<String> {
        self.rt.cfg.borrow().warns.clone()
    }

    /// The runtime's calls the oracle's maps take without a log line.
    pub fn unlogged(&self) -> Vec<String> {
        self.rt.unlogged.borrow().clone()
    }

    pub fn idle_calls(&self) -> Vec<(&'static str, String)> {
        self.rt.idle.calls.borrow().clone()
    }

    pub fn live_ids(&self) -> Vec<String> {
        self.rt.live.borrow().keys().cloned().collect()
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Entries and rows

/// A record in the one shared cell every entry of a workspace holds.
pub fn shared(record: WorkspaceRecord) -> Rc<RefCell<WorkspaceRecord>> {
    Rc::new(RefCell::new(record))
}

pub fn record_of(phase: &str, extras: Value, sub: bool) -> WorkspaceRecord {
    let mut v = json!({
        "id": "ws_1", "name": "w1", "kind": "cloud", "machineId": "m-1", "phase": phase,
        "golden": "", "createdAt": "", "project": "", "spec": {}, "size": { "cpu": 1, "memMb": 2048 }, "firstLife": false
    });
    if sub {
        v["sub"] = json!({ "on": "solari", "repo": "", "base": { "branch": "", "sha": "" }, "branch": "", "folder": "" });
    }
    if let Some(extra) = extras.as_object() {
        for (k, x) in extra {
            v[k] = x.clone();
        }
    }
    from(v)
}

pub fn hooks_none() -> WorkspaceHooks {
    WorkspaceHooks {
        golden_snapshot: String::new(),
        wake_attempts: 0,
        resurrect: None,
        vault_export: None,
        vault_import: None,
        stash_vault: None,
        restore_vault: None,
        wake_check: None,
        move_hook: None,
        retire: None,
    }
}

pub fn live_workspace(record: WorkspaceRecord, machine: Arc<dyn Machine>, ws: Workspace, generation: u64) -> LiveWorkspace {
    LiveWorkspace {
        record: Rc::new(RefCell::new(record)),
        opens_thread: None,
        of_record_held: false,
        ws: Rc::new(ws),
        machine,
        generation,
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

/// The oracle's `entryIn` options.
pub struct EntryOpts {
    pub phase: &'static str,
    pub nap: Option<EngineError>,
    pub state: Option<Read>,
    pub prove: Option<Prove>,
    pub sub: bool,
    pub record: Value,
}

impl Default for EntryOpts {
    fn default() -> Self {
        EntryOpts { phase: "running", nap: None, state: None, prove: None, sub: false, record: json!({}) }
    }
}

pub struct Ent {
    pub entry: Entry,
    pub machine: Arc<FakeMachine>,
}

impl Ent {
    pub fn phase(&self) -> WorkspacePhase {
        self.entry.borrow().record.borrow().phase
    }
    pub fn id(&self) -> String {
        self.entry.borrow().record.borrow().id.clone()
    }
}

pub fn ready_joined<T: Clone + 'static>(v: T) -> Joined<T> {
    futures::future::ready(Ok(v)).boxed_local().shared()
}

impl World {
    /// The oracle's `entryIn`: the record `ws_1` on machine `m-1`, live, over an engine workspace that reads running.
    pub fn entry_in(&self, o: EntryOpts) -> Ent {
        let machine = FakeMachine::of_entry(&self.log);
        {
            let mut s = machine.s.borrow_mut();
            s.state = o.state;
            s.nap = o.nap;
            s.prove = o.prove;
        }
        let record = record_of(o.phase, o.record, o.sub);
        let ws = Workspace::new(machine.clone() as Arc<dyn Machine>, hooks_none(), WorkspaceInitial { phase: None, first_life: Some(false) });
        let id = record.id.clone();
        let entry: Entry = Rc::new(RefCell::new(live_workspace(record, machine.clone() as Arc<dyn Machine>, ws, 0)));
        self.rt.live.borrow_mut().insert(id, entry.clone());
        Ent { entry, machine }
    }

    pub fn session_row(&self, id: &str, workspace: &str, thread: Option<&str>) -> Row {
        let mut view = json!({ "id": id, "workspaceId": workspace, "harness": "claude", "status": "running" });
        if let Some(t) = thread {
            view["threadId"] = json!(t);
        }
        let log = self.log.clone();
        let sid = id.to_string();
        Rc::new(RefCell::new(SessionEntry {
            view: from(view),
            turn_id: String::new(),
            replaces: None,
            notify: None,
            notify_by: None,
            notify_road: None,
            turn_token: None,
            scope_device_id: None,
            handle: None,
            end: Some(Rc::new(move |reason, _, _| log.push(json!(["session.end", sid, reason])))),
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
        }))
    }

    /// The oracle's `sessionsOf`: sessions `s1` (ws_1) and `s2` (ws_2) and one exec of ws_1, each logging its end.
    pub fn sessions_of(&self) {
        self.rt.sessions.borrow_mut().insert("s1".into(), self.session_row("s1", "ws_1", None));
        self.rt.sessions.borrow_mut().insert("s2".into(), self.session_row("s2", "ws_2", None));
        self.rt.execs.borrow_mut().push("ws_1".into());
    }

    /// A running row on a workspace, ended by nobody.
    pub fn running_row(&self, id: &str, workspace: &str) {
        self.rt.sessions.borrow_mut().insert(id.into(), self.session_row(id, workspace, None));
    }

    pub fn marks(&self, e: &Entry) -> Vec<&'static str> {
        let e = e.borrow();
        let mut out = Vec::new();
        if e.adopting.is_some() {
            out.push("adopting");
        }
        if e.deleting.is_some() {
            out.push("deleting");
        }
        if e.late_read.is_some() {
            out.push("lateRead");
        }
        if e.napping.is_some() {
            out.push("napping");
        }
        if e.unchecked {
            out.push("unchecked");
        }
        if e.wake_stop.is_some() {
            out.push("wakeStop");
        }
        if e.waking.is_some() {
            out.push("waking");
        }
        out.sort();
        out
    }

    /// The oracle's `after(x, e)`: the record, the marks on the entry, the refusals, the records being read again.
    pub fn after(&self, e: &Entry) -> Value {
        let nap: Vec<Value> = self.rt.nap_refusals.borrow().iter().map(|(k, r)| json!([k, { "machineId": r.machine_id, "said": r.said }])).collect();
        let stop: Vec<Value> = self.rt.stop_refusals.borrow().iter().map(|(k, r)| json!([k, { "machineId": r.machine_id, "said": r.said }])).collect();
        json!({
            "record": serde_json::to_value(&*e.borrow().record.borrow()).unwrap(),
            "marks": self.marks(e),
            "napRefusals": nap,
            "stopRefusals": stop,
            "rereading": self.area.rereading(),
        })
    }

    /// `{ ...extra, ...after(x, e) }`.
    pub fn after_with(&self, e: &Entry, extra: Value) -> Value {
        let mut out = extra;
        for (k, v) in self.after(e).as_object().unwrap() {
            out[k] = v.clone();
        }
        out
    }
}

/// Starts a future on the owner's thread and lets it run to where it first waits, as a TS call runs up to its first
/// `await`; its answer is read at the join handle.
pub async fn started<T: 'static>(f: impl Future<Output = T> + 'static) -> tokio::task::JoinHandle<T> {
    let h = tokio::task::spawn_local(f);
    tokio::task::yield_now().await;
    h
}

/// The view an answer carries as the oracle's `view` writes it.
pub fn view_json(v: &WorkspaceView) -> Value {
    serde_json::to_value(v).unwrap()
}

// ---------------------------------------------------------------------------------------------------------------
// Comparing with the fixtures

/// Every number as an integer where it is a whole number, since a Rust float field writes `5.0` where the oracle wrote `5`.
pub fn norm(v: &Value) -> Value {
    match v {
        Value::Number(n) => match n.as_f64() {
            Some(f) if n.is_f64() && f.fract() == 0.0 && f.abs() < 9.0e15 => json!(f as i64),
            _ => v.clone(),
        },
        Value::Array(a) => Value::Array(a.iter().map(norm).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), norm(x))).collect()),
        other => other.clone(),
    }
}

/// An attempt's `request` read back as the object the oracle's fake made.
pub fn request_back(v: &Value) -> Value {
    match v {
        Value::Object(o) => Value::Object(
            o.iter()
                .map(|(k, x)| match (k.as_str(), x) {
                    ("request", Value::String(s)) => (k.clone(), serde_json::from_str(s).unwrap_or_else(|_| x.clone())),
                    _ => (k.clone(), request_back(x)),
                })
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(request_back).collect()),
        other => other.clone(),
    }
}

/// The keys a Rust record or view writes that the oracle's records did not carry (required fields the oracle's
/// records left out, filled with empty values by the test).
const FILLER: &[&str] = &["name", "machineId", "golden", "createdAt", "project", "spec", "size", "firstLife", "repo", "base", "branch", "folder", "reach", "rateUsdPerHour"];

/// `got` without the filler keys `want` does not name, down to the same depth.
pub fn shed(want: &Value, got: &Value) -> Value {
    match (want, got) {
        (Value::Object(w), Value::Object(g)) => {
            let mut out = Map::new();
            for (k, v) in g {
                match w.get(k) {
                    Some(wv) => {
                        out.insert(k.clone(), shed(wv, v));
                    }
                    None if FILLER.contains(&k.as_str()) => {}
                    None => {
                        out.insert(k.clone(), v.clone());
                    }
                }
            }
            Value::Object(out)
        }
        (Value::Array(w), Value::Array(g)) => Value::Array(g.iter().enumerate().map(|(i, v)| w.get(i).map_or_else(|| v.clone(), |wv| shed(wv, v))).collect()),
        _ => got.clone(),
    }
}

/// How a scene's log is compared.
#[derive(Clone, Copy)]
pub struct Rules {
    /// Records and events carry the test's filler keys.
    pub lenient: bool,
    /// The calls on the engine's own workspace (`ws.notePaused`, `ws.noteRunning`) and the promise identity flags
    /// (`joined`, the third member of `during`) have no Rust record.
    pub drop_oracle_only: bool,
}

pub const CREATES: Rules = Rules { lenient: false, drop_oracle_only: false };
pub const PHASES: Rules = Rules { lenient: true, drop_oracle_only: true };

fn sort_names(entry: &Value) -> Value {
    if entry[0] == "namedAfter" {
        let mut names: Vec<Value> = entry[1].as_array().unwrap().clone();
        names.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
        return json!(["namedAfter", names]);
    }
    entry.clone()
}

/// The scene's expected log under `rules`.
pub fn want_log(scene: &Value, rules: Rules) -> Vec<Value> {
    scene["log"]
        .as_array()
        .expect("a log")
        .iter()
        .filter_map(|e| {
            if rules.drop_oracle_only {
                if e[0] == "ws.notePaused" || e[0] == "ws.noteRunning" || e[0] == "joined" {
                    return None;
                }
                if e[0] == "during" {
                    return Some(json!(["during", e[1]]));
                }
            }
            Some(sort_names(e))
        })
        .collect()
}

pub fn assert_log(scene: &Value, got: &[Value], rules: Rules) {
    let want = want_log(scene, rules);
    let got: Vec<Value> = got.iter().map(|e| norm(&request_back(&sort_names(e)))).collect();
    for (i, (w, g)) in want.iter().zip(got.iter()).enumerate() {
        let g = if rules.lenient { shed(w, g) } else { g.clone() };
        assert_eq!(&g, &norm(w), "scene {:?}: log entry {i} differs", scene["name"]);
    }
    assert_eq!(got.len(), want.len(), "scene {:?}: log length differs; got {:#?}", scene["name"], got);
}

/// `got` against the scene's `answer`, with the test's filler keys shed where the rules say so.
pub fn assert_answer(scene: &Value, got: &Value, rules: Rules) {
    let want = norm(&scene["answer"]);
    let got = norm(&request_back(got));
    let got = if rules.lenient { shed(&want, &got) } else { got };
    assert_eq!(got, want, "scene {:?}: answer differs", scene["name"]);
}

/// The scene's `creates` rows after, as `[[key, row], ...]`.
pub fn assert_creates(w: &World, scene: &Value) {
    let got = norm(&request_back(&w.store.table("creates")));
    assert_eq!(got, norm(&scene["creates"]), "scene {:?}: creates rows differ", scene["name"]);
}

/// An engine answer `Result<String, HostError>` as the oracle's `{threw, kind}` or the value.
pub fn answer_of<T: Into<Value>>(r: Result<T, HostError>) -> Value {
    match r {
        Ok(v) => v.into(),
        Err(e) => json!({ "threw": e.message, "kind": e.kind.as_ref().map(|k| k.as_str().to_string()) }),
    }
}

pub fn reaped_json(r: &wsp_engine::orphans::ReapedMachine) -> Value {
    let mut o = json!({ "id": r.id, "builder": r.builder, "reason": format!("{:?}", r.reason).to_lowercase() });
    if let Some(a) = r.age_ms {
        o["ageMs"] = json!(a);
    }
    if let Some(l) = &r.labels {
        o["labels"] = serde_json::to_value(l).unwrap();
    }
    if let Some(n) = &r.name {
        o["name"] = json!(n);
    }
    o
}

pub fn failure_json(f: &wsp_engine::orphans::ReapFailure) -> Value {
    let mut o = json!({ "message": f.message });
    if let Some(id) = &f.id {
        o["id"] = json!(id);
    }
    o
}

/// The stamp of a time `ms` before the scene clock's start, as the oracle writes `new Date(T0 - ms).toISOString()`.
pub fn before(ms: i64) -> String {
    iso(T0 - ms)
}

/// The names of a log's entries, for a test that reads only the order of the calls.
pub fn names_of(log: &[Value]) -> Vec<String> {
    log.iter().map(|e| e[0].as_str().unwrap_or("?").to_string()).collect()
}

/// The entries of a log whose first word is one of `kinds`.
pub fn log_of(log: &[Value], kinds: &[&str]) -> Vec<Value> {
    log.iter().filter(|e| kinds.iter().any(|k| e[0] == *k)).cloned().collect()
}


// ---------------------------------------------------------------------------------------------------------------
// The creates acts

/// The oracle's `spec`.
pub fn spec_default() -> MachineSpec {
    from(json!({ "kind": "sandbox", "template": "img-1", "cpu": 2, "memMb": 4096, "labels": { "a": "1", "createdAt": "stamp" } }))
}

/// The oracle's `create(x, b, purpose, spec, place)`: one create through `claiming`, logging what the waiting line says
/// and what `inflight` holds once the machine answered.
pub async fn create(w: &World, b: Arc<FakeBackend>, purpose: &str, spec: MachineSpec, place: Option<&str>) -> Result<String, HostError> {
    let log = w.log.clone();
    let area = w.area.clone();
    let waiting_log = w.log.clone();
    w.area
        .claiming(
            purpose,
            move |h| {
                async move {
                    let waiting: Waiting = Arc::new(move |line: &str, bx: Option<&ProviderBoxState>| {
                        waiting_log.push(json!(["waiting", line, bx.map(|b| b.id.clone())]));
                    });
                    let m = h.create(spec, Some(waiting)).await?;
                    log.push(json!(["inflight inside", area.inflight()]));
                    Ok::<String, HostError>(m.id().to_string())
                }
                .boxed_local()
            },
            Some(b as Arc<dyn MachineBackend>),
            place,
        )
        .await
}

/// The body the scene's first attempt carries: what a held row of the same spec must hold to be replayed.
pub fn fingerprint_of(scene: &Value) -> String {
    scene["log"].as_array().unwrap().iter().find(|e| e[0] == "store.put").expect("an attempt put")[3]["body"].as_str().unwrap().to_string()
}

/// The scene's seed with the held attempt's body set to the spec's fingerprint, as the oracle's act sets it.
pub fn store_with_fingerprint(scene: &Value) -> Value {
    let mut store = scene["store"].clone();
    store["creates"]["workspace/ws_1"]["body"] = json!(fingerprint_of(scene));
    store
}

/// What every creates scene checks after its act: the log, the answer, the rows left and `inflight` empty.
pub fn check_create(w: &World, scene: &Value, answer: Value) {
    assert_log(scene, &w.log_now(), CREATES);
    assert_answer(scene, &answer, CREATES);
    assert_creates(w, scene);
    assert_eq!(json!(w.area.inflight()), scene["inflight"], "scene {:?}: inflight differs", scene["name"]);
}

/// The scene's `store` seed, as it was given to the oracle's world.
pub fn store_of(scene: &Value) -> Value {
    scene["store"].clone()
}

// ---------------------------------------------------------------------------------------------------------------
// The phases acts

impl World {
    /// The view of an entry's record, as the runtime's `view` writes it.
    pub fn view_of(&self, entry: &Entry) -> WorkspaceView {
        MachinesRuntime::view(&*self.rt, &entry.borrow().record.borrow())
    }

    /// `bus.emit`.
    pub fn emit(&self, event: EventUnion) {
        MachinesRuntime::emit(&*self.rt, event).expect("the bus takes it");
    }

    /// The oracle's `.catch(err => log.push(["rejected", err.message]))` (with the kind where the scene logs one).
    pub fn rejected(&self, err: &HostError, with_kind: bool) {
        if with_kind {
            self.log.push(json!(["rejected", err.message, err.kind.as_ref().map(|k| k.as_str().to_string())]));
        } else {
            self.log.push(json!(["rejected", err.message]));
        }
    }

    pub fn status_event(&self, id: &str, phase: &str, machine_state: &str) -> EventUnion {
        from(json!({
            "type": "workspace.status",
            "status": {
                "id": id, "name": "w1", "machineId": "m-1", "phase": phase, "golden": "", "createdAt": "",
                "project": { "id": "", "name": "", "path": "", "computer": "" }, "machineState": machine_state,
                "reach": { "state": "reachable" }, "size": { "cpu": 1, "memMb": 2048 }, "rateUsdPerHour": 0
            }
        }))
    }
}

/// The phase the engine's workspace must read where the oracle's last call on it was a `notePaused` or `noteRunning`:
/// those calls are not logged here (the workspace is the engine's own), so the phase they leave is read instead.
pub fn assert_notes(scene: &Value, e: &Ent) {
    use wsp_engine::lifecycle::WorkspacePhase as P;
    let mut want: Option<P> = None;
    for entry in scene["log"].as_array().unwrap() {
        match entry[0].as_str() {
            Some("ws.notePaused") => want = Some(P::Napping),
            Some("ws.noteRunning") => want = Some(P::Running),
            Some("ws.nap") => want = None,
            _ => {}
        }
    }
    if let Some(p) = want {
        assert_eq!(e.entry.borrow().ws.current_phase(), p, "scene {:?}: the engine's workspace phase differs", scene["name"]);
    }
}

/// What every phases scene checks after its act: the log, the answer and the engine's phase.
pub fn check_phases(w: &World, scene: &Value, answer: Value, e: &Ent) {
    assert_log(scene, &w.log_now(), PHASES);
    assert_answer(scene, &answer, PHASES);
    assert_notes(scene, e);
}


/// Runs `f` on the engine workspace of an entry, which the entry holds as an `Rc` while the engine's calls take
/// `&mut self`: the workspace is taken out of the entry for the call and put back, so the hooks it runs may borrow
/// the entry.
pub async fn with_ws<T>(entry: &Entry, f: impl for<'a> FnOnce(&'a mut Workspace) -> LocalBoxFuture<'a, T>) -> T {
    let placeholder = {
        let e = entry.borrow();
        Rc::new(Workspace::new(e.machine.clone(), hooks_none(), WorkspaceInitial::default()))
    };
    let mut ws = std::mem::replace(&mut entry.borrow_mut().ws, placeholder);
    let out = f(Rc::get_mut(&mut ws).expect("the entry is the engine workspace's only holder")).await;
    entry.borrow_mut().ws = ws;
    out
}

pub async fn ws_nap(entry: &Entry) -> Result<(), EngineError> {
    with_ws(entry, |ws| ws.nap().boxed_local()).await
}

pub async fn ws_wake(entry: &Entry) -> Result<wsp_engine::lifecycle::WakeResult, EngineError> {
    with_ws(entry, |ws| ws.wake(false).boxed_local()).await
}

pub async fn ws_upgrade(entry: &Entry, spec: Option<MachineSpec>) -> Result<(), EngineError> {
    with_ws(entry, |ws| ws.upgrade(spec).boxed_local()).await
}

pub async fn ws_rebuild(entry: &Entry) -> Result<(), EngineError> {
    with_ws(entry, |ws| ws.rebuild().boxed_local()).await
}
