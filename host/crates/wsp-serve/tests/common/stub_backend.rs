// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/test/stub-backend.ts at 65f6b89c7 (the provider `stubBackend()` and its machines).
//!
//! A TS test reassigns a method of the stub (`backend.create = ...`, `m.kill = ...`) or sets a field (`m.killed`,
//! `backend.execImpl`). Here every such method is a `*_fn` pair on the object: `kill_fn()` reads the one in force and
//! `set_kill(f)` replaces it, so a test wraps the old one as the TS does with `.bind`. Every field is a `Cell` or
//! `RefCell` read through a method of the same name.
use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::{Arc, Weak};

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use wsp_core::daemon_token::DAEMON_TOKEN_SET;
use wsp_engine::cancel::Cancel;
use wsp_engine::errors::{EngineError, ErrorKind, RoadError};
use wsp_engine::machine::{
    BackendOffers, BackendPricing, Capabilities, ExecOptions, ExecResult, Lifecycle, LifecycleBudgets, Machine, MachineBackend,
    MachineKind, MachineLife, MachineListRow, MachineOffers, MachineShape, MachineSpec, MachineState, NamedMachine, PreviewReach,
    ProviderBoxState, RunOptions, Seen, Size, SnapshotOptions, SnapshotRow, SnapshotStoragePricing, TemplateRow, Waiting,
    WireSnapshotStoragePricing,
};
use wsp_wire::types::daemon_contract::DAEMON_TOKEN_PATH;
use wsp_wire::types::js_number::JsInt;
use wsp_wire::types::JsNumber;
use wsp_wire::views::activity::iso_time;
use wsp_wire::wire::capabilities::{MachineSizeOffer, PauseMode};

use super::vault::VaultServer;

/// stub-backend.ts:20. The branch the stub guest's checkout is on.
pub const GUEST_BRANCH: &str = "work";

pub type Fut<T> = LocalBoxFuture<'static, Result<T, EngineError>>;
/// `execImpl(m, cmd, stdin)`: the machine, the command, and what was piped in.
pub type ExecImpl = Rc<dyn Fn(Arc<StubMachine>, String, Option<Vec<u8>>) -> Fut<ExecResult>>;
pub type CreateFn = Rc<dyn Fn(MachineSpec, Option<Waiting>, Option<String>) -> Fut<Arc<StubMachine>>>;
pub type GetFn = Rc<dyn Fn(String) -> Fut<Arc<StubMachine>>>;
pub type ReplayFn = Rc<dyn Fn(String, String, String) -> Fut<Option<Arc<StubMachine>>>>;
pub type NamedAfterFn = Rc<dyn Fn(BTreeSet<String>) -> Fut<Vec<NamedMachine>>>;
pub type CreateRequestFn = Rc<dyn Fn(MachineSpec) -> Fut<String>>;
pub type UnitFn = Rc<dyn Fn() -> Fut<()>>;
pub type StateFn = Rc<dyn Fn() -> Fut<MachineState>>;
pub type PreviewFn = Rc<dyn Fn(u16) -> Fut<PreviewReach>>;
pub type AnswersFn = Rc<dyn Fn(Option<u64>) -> Fut<bool>>;
pub type UrlFn = Rc<dyn Fn(String) -> Fut<String>>;

/// A `{ kind: "missing", status: 404 }` error: what the provider answers for a machine it no longer holds.
pub fn missing(message: &str) -> EngineError {
    EngineError::Refused { message: message.into(), kind: Some(ErrorKind::Missing), status: Some(404), code: None }
}

/// `Object.assign(new Error(message), { kind: "conflict", status: 409 })`.
pub fn conflict(message: &str) -> EngineError {
    EngineError::Refused { message: message.into(), kind: Some(ErrorKind::Conflict), status: Some(409), code: None }
}

/// `new TypeError("fetch failed")`: a create whose answer never came back.
pub fn fetch_failed() -> EngineError {
    EngineError::Road(RoadError { name: "TypeError".into(), message: "fetch failed".into(), code: None, cause: None })
}

/// `abortedCall(what)` (stub-backend.ts:151): the failure a call the caller's own signal cut off raises.
pub fn aborted_call(what: &str) -> EngineError {
    EngineError::Road(RoadError { name: "AbortError".into(), message: format!("{what} was aborted"), code: None, cause: None })
}

pub fn exec_ok(stdout: &str) -> ExecResult {
    ExecResult { exit_code: JsInt::from(0), stdout: stdout.into(), stderr: String::new() }
}

pub fn exec_exit(code: i64, stdout: &str, stderr: &str) -> ExecResult {
    ExecResult { exit_code: JsInt::from(code), stdout: stdout.into(), stderr: stderr.into() }
}

/// An `execImpl` from a plain function, as nearly every test writes one.
pub fn exec_fn(f: impl Fn(&Arc<StubMachine>, &str) -> Result<ExecResult, EngineError> + 'static) -> ExecImpl {
    Rc::new(move |m, cmd, _stdin| {
        let answer = f(&m, &cmd);
        Box::pin(async move { answer })
    })
}

/// An `execImpl` that awaits: the test's own gate or sleep sits inside.
pub fn exec_async(f: impl Fn(Arc<StubMachine>, String) -> Fut<ExecResult> + 'static) -> ExecImpl {
    Rc::new(move |m, cmd, _stdin| f(m, cmd))
}

/// stub-backend.ts:92. What every stub guest answers whatever else it is told: the branch read.
pub fn guest_branch_answer(cmd: &str) -> Option<ExecResult> {
    cmd.contains("rev-parse --abbrev-ref HEAD").then(|| exec_ok(&format!("{GUEST_BRANCH}\norigin/{GUEST_BRANCH}\n")))
}

/// stub-backend.ts:97. A guest that has a daemon: the runtime's token write lands and everything else is fine.
pub fn token_guest_answer(cmd: &str) -> ExecResult {
    if cmd.contains(DAEMON_TOKEN_PATH) {
        return exec_ok(&format!("{DAEMON_TOKEN_SET}\n"));
    }
    guest_branch_answer(cmd).unwrap_or_else(|| exec_ok(""))
}

/// stub-backend.ts:97 `tokenGuest` as an `ExecImpl`.
pub fn token_guest() -> ExecImpl {
    exec_fn(|_, cmd| Ok(token_guest_answer(cmd)))
}

/// What a machine's calls came to, counted at the method as vitest's `spyOn` counts them.
#[derive(Default)]
pub struct Calls {
    pub exec: Cell<u32>,
    pub run: Cell<u32>,
    pub snapshot: Cell<u32>,
    pub pause: Cell<u32>,
    pub resume: Cell<u32>,
    pub kill: Cell<u32>,
    pub state: Cell<u32>,
    pub metrics: Cell<u32>,
    pub describe: Cell<u32>,
    pub preview: Cell<u32>,
    pub daemon_answers: Cell<u32>,
}

fn bump(n: &Cell<u32>) {
    n.set(n.get() + 1);
}

/// stub-backend.ts:102 `StubMachine`.
pub struct StubMachine {
    me: Weak<StubMachine>,
    backend: Weak<StubBackend>,
    id: String,
    kind: MachineKind,
    stream_url: Option<String>,
    notice: Option<String>,
    pub spec: RefCell<MachineSpec>,
    labels: RefCell<Option<IndexMap<String, String>>>,
    pub paused: Cell<bool>,
    pub killed: Cell<bool>,
    /// The host lost the VM while the gateway still lists it running: metrics answer 404, the state read still says running.
    pub host_lost: Cell<bool>,
    pub exec_log: RefCell<Vec<String>>,
    pub run_log: RefCell<Vec<String>>,
    pub run_options: RefCell<Vec<RunOptions>>,
    pub resumes: Cell<u32>,
    pub paused_at: RefCell<Option<String>>,
    pub shape: RefCell<MachineShape>,
    pub snapshot_lives: RefCell<Vec<MachineLife>>,
    pub replayed: Cell<bool>,
    pub calls: Calls,
    kill: RefCell<UnitFn>,
    pause: RefCell<UnitFn>,
    resume: RefCell<UnitFn>,
    state: RefCell<StateFn>,
    metrics: RefCell<UnitFn>,
    download_url: RefCell<UrlFn>,
    upload_url: RefCell<UrlFn>,
    preview: RefCell<Option<PreviewFn>>,
    daemon_answers: RefCell<Option<AnswersFn>>,
    prove: RefCell<Option<UnitFn>>,
}

impl std::fmt::Debug for StubMachine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "StubMachine({})", self.id)
    }
}

fn leaked<T>(value: T) -> &'static T {
    Box::leak(Box::new(value))
}

impl StubMachine {
    fn new(backend: Weak<StubBackend>, id: String, spec: MachineSpec, notice: Option<String>, vault: Option<Rc<VaultServer>>) -> Arc<Self> {
        let kind = spec.kind;
        let stream_url = (kind == MachineKind::Desktop).then(|| format!("wss://stub/stream/{id}"));
        let shape = MachineShape {
            cpu: Some(spec.cpu.unwrap_or(JsNumber(2.0))),
            mem_mb: Some(spec.mem_mb.unwrap_or(JsNumber(4096.0))),
            disk_gb: None,
            created_at: Some(iso_time(super::now_ms())),
            used_bytes: None,
        };
        let labels = spec.labels.clone();
        Arc::new_cyclic(|me: &Weak<StubMachine>| {
            let this = |me: &Weak<StubMachine>| me.upgrade().expect("the machine is held");
            let kill_me = me.clone();
            let pause_me = me.clone();
            let resume_me = me.clone();
            let state_me = me.clone();
            let metrics_me = me.clone();
            let down_me = me.clone();
            let down_vault = vault.clone();
            let up_me = me.clone();
            let up_vault = vault;
            StubMachine {
                me: me.clone(),
                backend,
                id,
                kind,
                stream_url,
                notice,
                spec: RefCell::new(spec),
                labels: RefCell::new(labels),
                paused: Cell::new(false),
                killed: Cell::new(false),
                host_lost: Cell::new(false),
                exec_log: RefCell::default(),
                run_log: RefCell::default(),
                run_options: RefCell::default(),
                resumes: Cell::new(0),
                paused_at: RefCell::new(None),
                shape: RefCell::new(shape),
                snapshot_lives: RefCell::default(),
                replayed: Cell::new(false),
                calls: Calls::default(),
                kill: RefCell::new(Rc::new(move || {
                    let m = this(&kill_me);
                    Box::pin(async move {
                        m.killed.set(true);
                        Ok(())
                    })
                })),
                pause: RefCell::new(Rc::new(move || {
                    let m = this(&pause_me);
                    Box::pin(async move {
                        m.paused.set(true);
                        Ok(())
                    })
                })),
                resume: RefCell::new(Rc::new(move || {
                    let m = this(&resume_me);
                    Box::pin(async move {
                        if m.killed.get() {
                            return Err(missing("gone"));
                        }
                        m.resumes.set(m.resumes.get() + 1);
                        m.paused.set(false);
                        Ok(())
                    })
                })),
                state: RefCell::new(Rc::new(move || {
                    let m = this(&state_me);
                    Box::pin(async move { Ok(m.live_state()) })
                })),
                metrics: RefCell::new(Rc::new(move || {
                    let m = this(&metrics_me);
                    Box::pin(async move {
                        if m.killed.get() || m.host_lost.get() {
                            return Err(missing("host no longer knows this VM"));
                        }
                        Ok(())
                    })
                })),
                download_url: RefCell::new(Rc::new(move |path| {
                    let _ = this(&down_me);
                    let vault = down_vault.clone();
                    Box::pin(async move {
                        let origin = vault.expect("a vault server").origin().await;
                        Ok(format!("{origin}/download{path}"))
                    })
                })),
                upload_url: RefCell::new(Rc::new(move |path| {
                    let m = this(&up_me);
                    let vault = up_vault.clone();
                    Box::pin(async move {
                        let origin = vault.expect("a vault server").origin().await;
                        Ok(format!("{origin}/upload?machine={}&path={}", m.id, urlencode(&path)))
                    })
                })),
                preview: RefCell::new(None),
                daemon_answers: RefCell::new(None),
                prove: RefCell::new(None),
            }
        })
    }

    fn live_state(&self) -> MachineState {
        if self.killed.get() {
            MachineState::Gone
        } else if self.paused.get() {
            MachineState::Paused
        } else {
            MachineState::Running
        }
    }

    /// The machine's own `Arc`, for a closure that outlives the borrow.
    pub fn arc(&self) -> Arc<StubMachine> {
        self.me.upgrade().expect("the machine is held")
    }

    pub fn labels_now(&self) -> Option<IndexMap<String, String>> {
        self.labels.borrow().clone()
    }

    /// `m.labels = labels` (lost-create.test.ts: a replay names the machine after the bytes it was asked with).
    pub fn set_labels(&self, labels: IndexMap<String, String>) {
        *self.labels.borrow_mut() = Some(labels);
    }

    pub fn exec_log(&self) -> Vec<String> {
        self.exec_log.borrow().clone()
    }

    pub fn run_log(&self) -> Vec<String> {
        self.run_log.borrow().clone()
    }

    /// The `execLog` entries that start with `prefix`.
    pub fn exec_starting(&self, prefix: &str) -> Vec<String> {
        self.exec_log.borrow().iter().filter(|c| c.starts_with(prefix)).cloned().collect()
    }

    pub fn exec_containing(&self, needle: &str) -> Vec<String> {
        self.exec_log.borrow().iter().filter(|c| c.contains(needle)).cloned().collect()
    }

    pub fn spec_now(&self) -> MachineSpec {
        self.spec.borrow().clone()
    }

    pub fn shape_now(&self) -> MachineShape {
        self.shape.borrow().clone()
    }

    pub fn set_shape(&self, shape: MachineShape) {
        *self.shape.borrow_mut() = shape;
    }

    pub fn set_memory(&self, mem_mb: f64) {
        self.shape.borrow_mut().mem_mb = Some(JsNumber(mem_mb));
    }

    pub fn kill_fn(&self) -> UnitFn {
        self.kill.borrow().clone()
    }
    pub fn set_kill(&self, f: UnitFn) {
        *self.kill.borrow_mut() = f;
    }
    pub fn pause_fn(&self) -> UnitFn {
        self.pause.borrow().clone()
    }
    pub fn set_pause(&self, f: UnitFn) {
        *self.pause.borrow_mut() = f;
    }
    pub fn resume_fn(&self) -> UnitFn {
        self.resume.borrow().clone()
    }
    pub fn set_resume(&self, f: UnitFn) {
        *self.resume.borrow_mut() = f;
    }
    pub fn state_fn(&self) -> StateFn {
        self.state.borrow().clone()
    }
    pub fn set_state(&self, f: StateFn) {
        *self.state.borrow_mut() = f;
    }
    pub fn metrics_fn(&self) -> UnitFn {
        self.metrics.borrow().clone()
    }
    pub fn set_metrics(&self, f: UnitFn) {
        *self.metrics.borrow_mut() = f;
    }
    pub fn download_url_fn(&self) -> UrlFn {
        self.download_url.borrow().clone()
    }
    pub fn set_download_url(&self, f: UrlFn) {
        *self.download_url.borrow_mut() = f;
    }
    /// `m.proveRoad = f` (runtime-comeback.test.ts:203): the machine offers a proof of its road, which `f` answers.
    pub fn set_prove_road(&self, f: Option<UnitFn>) {
        *self.prove.borrow_mut() = f;
    }

    pub fn set_preview(&self, f: Option<PreviewFn>) {
        *self.preview.borrow_mut() = f;
    }
    pub fn preview_fn(&self) -> Option<PreviewFn> {
        self.preview.borrow().clone()
    }
    /// `m.previewUrl = async () => ({ url, token: "e", expiresAt: Date.now() + 3_600_000 })`.
    pub fn set_preview_url(&self, url: &str) {
        let url = url.to_string();
        self.set_preview(Some(Rc::new(move |_| {
            let reach = PreviewReach { url: url.clone(), token: "e".into(), expires_at: JsNumber((super::now_ms() + 3_600_000) as f64) };
            Box::pin(async move { Ok(reach) })
        })));
    }
    /// `m.previewUrl = async p => ({ url: \`${base}/?port=${p}\`, token: "t", ... })`.
    pub fn set_preview_port_url(&self, base: &str) {
        let base = base.to_string();
        self.set_preview(Some(Rc::new(move |port| {
            let reach = PreviewReach {
                url: format!("{base}/?port={port}"),
                token: "t".into(),
                expires_at: JsNumber((super::now_ms() + 3_600_000) as f64),
            };
            Box::pin(async move { Ok(reach) })
        })));
    }
    pub fn answers_fn(&self) -> Option<AnswersFn> {
        self.daemon_answers.borrow().clone()
    }
    pub fn set_daemon_answers(&self, f: Option<AnswersFn>) {
        *self.daemon_answers.borrow_mut() = f;
    }
    /// `m.daemonAnswers = async () => answer`.
    pub fn set_daemon_answer(&self, answer: bool) {
        self.set_daemon_answers(Some(Rc::new(move |_| Box::pin(async move { Ok(answer) }))));
    }
    /// `m.daemonAnswers = async () => { throw new Error(message) }`.
    pub fn set_daemon_unaskable(&self, message: &str) {
        let message = message.to_string();
        self.set_daemon_answers(Some(Rc::new(move |_| {
            let e = EngineError::Other { message: message.clone(), code: None };
            Box::pin(async move { Err(e) })
        })));
    }

    /// `missesFirstDelete(m)` (stub-backend.ts:126): the gateway's copy that never held the machine takes the first
    /// delete with the same yes and does nothing.
    pub fn misses_first_delete(&self) {
        let kill = self.kill_fn();
        let missed = Rc::new(Cell::new(false));
        self.set_kill(Rc::new(move || {
            if missed.get() {
                return kill();
            }
            missed.set(true);
            Box::pin(async { Ok(()) })
        }));
    }

    fn seen_now(&self) -> Seen {
        let state = self.live_state();
        let paused_at = if self.paused.get() { self.paused_at.borrow().clone() } else { None };
        Seen { state, created_at: self.shape.borrow().created_at.clone(), paused_at }
    }
}

fn urlencode(path: &str) -> String {
    let mut out = String::new();
    for b in path.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[async_trait(?Send)]
impl Machine for StubMachine {
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
        self.labels.borrow().clone().map(|l| leaked(l))
    }
    fn seen(&self) -> Option<&Seen> {
        Some(leaked(self.seen_now()))
    }
    fn replayed(&self) -> bool {
        self.replayed.get()
    }
    fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }
    fn offers(&self) -> MachineOffers {
        MachineOffers {
            preview_url: self.preview.borrow().is_some(),
            daemon_answers: self.daemon_answers.borrow().is_some(),
            prove_road: self.prove.borrow().is_some(),
            describe: true,
            metrics: true,
            ..MachineOffers::default()
        }
    }

    async fn exec(&self, cmd: &str, opts: ExecOptions) -> Result<ExecResult, EngineError> {
        bump(&self.calls.exec);
        if self.killed.get() {
            return Err(missing("gone"));
        }
        self.exec_log.borrow_mut().push(cmd.to_string());
        let run = self.backend.upgrade().expect("the backend is held").exec_impl_fn();
        run(self.arc(), cmd.to_string(), opts.stdin).await
    }
    async fn run(&self, script: &str, opts: RunOptions) -> Result<ExecResult, EngineError> {
        bump(&self.calls.run);
        if self.killed.get() {
            return Err(missing("gone"));
        }
        self.exec_log.borrow_mut().push(script.to_string());
        self.run_log.borrow_mut().push(script.to_string());
        self.run_options.borrow_mut().push(opts);
        let run = self.backend.upgrade().expect("the backend is held").exec_impl_fn();
        run(self.arc(), script.to_string(), None).await
    }
    async fn snapshot(&self, name: &str, life: MachineLife, _opts: SnapshotOptions) -> Result<String, EngineError> {
        bump(&self.calls.snapshot);
        let backend = self.backend.upgrade().expect("the backend is held");
        let first_life = life.first_life;
        self.snapshot_lives.borrow_mut().push(life);
        // The provider's own row says which life a copy may be taken from; the stub refuses the same one it declares.
        if !first_life && !backend.capabilities().snapshots_any_life {
            return Err(EngineError::NotFirstLife { machine_id: self.id.clone(), action: format!("snapshot {name}") });
        }
        backend.take_snapshot(self, name)
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> Result<(), EngineError> {
        bump(&self.calls.pause);
        let f = self.pause_fn();
        f().await
    }
    async fn resume(&self, _signal: Option<&Cancel>) -> Result<(), EngineError> {
        bump(&self.calls.resume);
        let f = self.resume_fn();
        f().await
    }
    async fn kill(&self) -> Result<(), EngineError> {
        bump(&self.calls.kill);
        let f = self.kill_fn();
        f().await
    }
    async fn state(&self) -> Result<MachineState, EngineError> {
        bump(&self.calls.state);
        let f = self.state_fn();
        f().await
    }
    async fn download_url(&self, path: &str) -> Result<String, EngineError> {
        let f = self.download_url_fn();
        f(path.to_string()).await
    }
    async fn upload_url(&self, path: &str) -> Result<String, EngineError> {
        let f = self.upload_url.borrow().clone();
        f(path.to_string()).await
    }
    async fn preview_url(&self, port: u16) -> Result<PreviewReach, EngineError> {
        bump(&self.calls.preview);
        let f = self.preview_fn().ok_or_else(|| EngineError::Other { message: "m.previewUrl is not a function".into(), code: None })?;
        f(port).await
    }
    async fn daemon_answers(&self, timeout_ms: Option<u64>) -> Result<bool, EngineError> {
        bump(&self.calls.daemon_answers);
        let f = self.answers_fn().ok_or_else(|| EngineError::Other { message: "m.daemonAnswers is not a function".into(), code: None })?;
        f(timeout_ms).await
    }
    async fn prove_road(&self, _signal: Option<&Cancel>) -> Result<(), EngineError> {
        let f = self
            .prove
            .borrow()
            .clone()
            .ok_or_else(|| EngineError::Other { message: "m.proveRoad is not a function".into(), code: None })?;
        f().await
    }
    async fn describe(&self) -> Result<MachineShape, EngineError> {
        bump(&self.calls.describe);
        Ok(self.shape_now())
    }
    async fn metrics(&self) -> Result<(), EngineError> {
        bump(&self.calls.metrics);
        let f = self.metrics_fn();
        f().await
    }
}

/// stub-backend.ts:189.
const BUILTIN_TEMPLATES: [&str; 2] = ["base", "default"];

/// Every snapshot's pricing: `pricing.snapshotStorage` of the stub is `SNAPSHOT_STORAGE`, the engine's own constant of
/// fake-backend.ts:59.
fn snapshot_storage() -> SnapshotStoragePricing {
    SnapshotStoragePricing {
        wire: WireSnapshotStoragePricing { free_gb: JsNumber(10.0), usd_per_gb_month: JsNumber(0.15), billed_from: "2026-01-01".into() },
        free_named: None,
        usd_per_named_month: None,
    }
}

struct StubPricing;

impl BackendPricing for StubPricing {
    fn rate_usd_per_hour(&self, size: Size) -> f64 {
        size.cpu * 0.035 + (size.mem_mb / 1024.0) * 0.01
    }
    fn default_size(&self) -> Size {
        Size { cpu: 2.0, mem_mb: 4096.0 }
    }
    fn snapshot_storage(&self) -> SnapshotStoragePricing {
        snapshot_storage()
    }
}

/// stub-backend.ts:253 `lifecycle`: the budgets a test shrinks.
pub struct StubLifecycle {
    budgets: RefCell<LifecycleBudgets>,
}

#[async_trait(?Send)]
impl Lifecycle for StubLifecycle {
    fn budgets(&self) -> LifecycleBudgets {
        self.budgets.borrow().clone()
    }
}

impl StubLifecycle {
    pub fn set_wake_attempts(&self, n: i64) {
        self.budgets.borrow_mut().wake_attempts = JsInt::from(n);
    }
    pub fn set_daemon_answers_ms(&self, ms: f64) {
        self.budgets.borrow_mut().daemon_answers_ms = JsNumber(ms);
    }
}

/// A put the stub's upload URL took: the machine and guest path it was for, and the body.
#[derive(Clone, Debug, PartialEq)]
pub struct Put {
    pub machine: String,
    pub path: String,
    pub body: Vec<u8>,
}

#[derive(Default)]
pub struct BackendCalls {
    pub create: Cell<u32>,
    pub get: Cell<u32>,
    pub list: Cell<u32>,
    /// The id each `get` was asked for, oldest first: vitest's `spyOn(backend, "get").mock.calls`.
    pub get_ids: RefCell<Vec<String>>,
}

type BeforeSnapshot = Rc<dyn Fn(&StubMachine, u32) -> Result<(), EngineError>>;

/// stub-backend.ts:153 `StubBackend`.
pub struct StubBackend {
    me: Weak<StubBackend>,
    mark: Option<String>,
    capabilities: RefCell<Capabilities>,
    lifecycle: Arc<StubLifecycle>,
    names_workspace: Cell<bool>,
    seq: Cell<u32>,
    machines: RefCell<Vec<Arc<StubMachine>>>,
    /// Every command run on the computer itself rather than in a workspace on it, oldest first.
    computer_log: RefCell<Vec<String>>,
    vault: Rc<VaultServer>,
    /// What the computer answers a create with beside the machine; set before a machine is made.
    pub create_notice: RefCell<Option<String>>,
    exec_impl: RefCell<ExecImpl>,
    pub before_snapshot: RefCell<Option<BeforeSnapshot>>,
    pub named_snapshots: Cell<bool>,
    pub snapshots: RefCell<Vec<SnapshotRow>>,
    snapshots_named: RefCell<IndexMap<String, u32>>,
    snapshot_asks: RefCell<IndexMap<String, u32>>,
    pub snapshot_bytes: Cell<u64>,
    pub templates: RefCell<IndexMap<String, (TemplateRow, String)>>,
    pub promoted: RefCell<Vec<(String, String)>>,
    pub calls: BackendCalls,
    create: RefCell<CreateFn>,
    get: RefCell<GetFn>,
    create_request: RefCell<Option<CreateRequestFn>>,
    replay_create: RefCell<Option<ReplayFn>>,
    named_after: RefCell<Option<NamedAfterFn>>,
}

impl StubBackend {
    /// stub-backend.ts:222. `mark` goes into every snapshot id this backend mints.
    pub fn new(mark: Option<&str>) -> Arc<StubBackend> {
        let capabilities = Capabilities {
            live_clone_forks: true,
            pause_mode: Some(PauseMode::Memory),
            replaces_machine: true,
            preview_urls: true,
            signed_urls: true,
            callback_relay: true,
            disk_snapshots: true,
            images: true,
            snapshots_any_life: false,
            snapshot_listing: true,
            templates: false,
            kept: false,
            copies: true,
            own_network: true,
            sizes: [(2.0, 2048.0, 0.09), (2.0, 4096.0, 0.11), (2.0, 8192.0, 0.15), (4.0, 8192.0, 0.22)]
                .into_iter()
                .map(|(cpu, mem, rate)| MachineSizeOffer {
                    cpu: JsNumber(cpu),
                    mem_mb: JsNumber(mem),
                    rate_usd_per_hour: JsNumber(rate),
                    name: None,
                })
                .collect(),
            sizes_last: false,
        };
        let lifecycle = Arc::new(StubLifecycle {
            budgets: RefCell::new(LifecycleBudgets {
                wake_attempts: JsInt::from(2),
                daemon_answers_ms: JsNumber(30_000.0),
                resume_asks: Some(wsp_wire::wire::machine_link::LifecycleBudgetsResumeAsks {
                    every_ms: JsNumber(60_000.0),
                    for_ms: JsNumber(30.0 * 60_000.0),
                }),
            }),
        });
        Arc::new_cyclic(|me: &Weak<StubBackend>| {
            let make_me = me.clone();
            let get_me = me.clone();
            StubBackend {
                me: me.clone(),
                mark: mark.map(str::to_string),
                capabilities: RefCell::new(capabilities),
                lifecycle,
                names_workspace: Cell::new(false),
                seq: Cell::new(0),
                machines: RefCell::default(),
                computer_log: RefCell::default(),
                vault: VaultServer::new(),
                create_notice: RefCell::new(None),
                // The machine context probe answers with its markers and nothing found, as a bare guest would.
                exec_impl: RefCell::new(exec_fn(|_, cmd| {
                    Ok(guest_branch_answer(cmd)
                        .unwrap_or_else(|| exec_ok(if cmd.contains("echo WSP_CTX") { "WSP_CTX\nWSP_CTX_END\n" } else { "" })))
                })),
                before_snapshot: RefCell::new(None),
                named_snapshots: Cell::new(false),
                snapshots: RefCell::default(),
                snapshots_named: RefCell::default(),
                snapshot_asks: RefCell::default(),
                snapshot_bytes: Cell::new(8_000_000_000),
                templates: RefCell::default(),
                promoted: RefCell::default(),
                calls: BackendCalls::default(),
                create: RefCell::new(Rc::new(move |spec, waiting, sent| {
                    let b = make_me.upgrade().expect("the backend is held");
                    Box::pin(async move { b.make(spec, waiting, sent).await })
                })),
                get: RefCell::new(Rc::new(move |id| {
                    let b = get_me.upgrade().expect("the backend is held");
                    Box::pin(async move { b.find(&id) })
                })),
                create_request: RefCell::new(None),
                replay_create: RefCell::new(None),
                named_after: RefCell::new(None),
            }
        })
    }

    pub fn arc(&self) -> Arc<StubBackend> {
        self.me.upgrade().expect("the backend is held")
    }

    pub fn lifecycle(&self) -> Arc<StubLifecycle> {
        self.lifecycle.clone()
    }

    pub fn machines(&self) -> Vec<Arc<StubMachine>> {
        self.machines.borrow().clone()
    }

    /// `backend.machines[n]`.
    pub fn machine(&self, n: usize) -> Arc<StubMachine> {
        self.machines.borrow().get(n).unwrap_or_else(|| panic!("the stub made no machine {n}")).clone()
    }

    pub fn made(&self) -> usize {
        self.machines.borrow().len()
    }

    pub fn computer_log(&self) -> Vec<String> {
        self.computer_log.borrow().clone()
    }

    pub fn puts(&self) -> Vec<Put> {
        self.vault.puts()
    }

    /// `backend.downloads = path => bytes`: what a download URL serves for a guest path; absent, an empty archive.
    pub fn set_downloads(&self, f: Rc<dyn Fn(&str) -> Vec<u8>>) {
        self.vault.set_downloads(f);
    }

    pub fn set_names_workspace(&self, on: bool) {
        self.names_workspace.set(on);
    }

    pub fn set_capabilities(&self, change: impl FnOnce(&mut Capabilities)) {
        change(&mut self.capabilities.borrow_mut());
    }

    pub fn exec_impl_fn(&self) -> ExecImpl {
        self.exec_impl.borrow().clone()
    }
    pub fn set_exec_impl(&self, f: ExecImpl) {
        *self.exec_impl.borrow_mut() = f;
    }

    pub fn create_fn(&self) -> CreateFn {
        self.create.borrow().clone()
    }
    pub fn set_create(&self, f: CreateFn) {
        *self.create.borrow_mut() = f;
    }
    pub fn get_fn(&self) -> GetFn {
        self.get.borrow().clone()
    }
    pub fn set_get(&self, f: GetFn) {
        *self.get.borrow_mut() = f;
    }
    pub fn create_request_fn(&self) -> Option<CreateRequestFn> {
        self.create_request.borrow().clone()
    }
    pub fn set_create_request(&self, f: Option<CreateRequestFn>) {
        *self.create_request.borrow_mut() = f;
    }
    pub fn replay_create_fn(&self) -> Option<ReplayFn> {
        self.replay_create.borrow().clone()
    }
    pub fn set_replay_create(&self, f: Option<ReplayFn>) {
        *self.replay_create.borrow_mut() = f;
    }
    pub fn set_named_after(&self, f: Option<NamedAfterFn>) {
        *self.named_after.borrow_mut() = f;
    }

    /// `withDaemonRoads(backend)` (stub-backend.ts:77): every machine the stub makes has its daemon's road from the
    /// moment it exists, and a daemon-token write lands.
    pub fn with_daemon_roads(self: &Arc<Self>) -> Arc<Self> {
        let answer = self.exec_impl_fn();
        self.set_exec_impl(Rc::new(move |m, cmd, stdin| {
            if cmd.contains(DAEMON_TOKEN_PATH) {
                return Box::pin(async { Ok(exec_ok(&format!("{DAEMON_TOKEN_SET}\n"))) });
            }
            answer(m, cmd, stdin)
        }));
        let make = self.create_fn();
        self.set_create(Rc::new(move |spec, waiting, sent| {
            let made = make(spec, waiting, sent);
            Box::pin(async move {
                let m = made.await?;
                if m.preview_fn().is_none() {
                    m.set_preview_url("http://127.0.0.1:9");
                }
                if m.answers_fn().is_none() {
                    m.set_daemon_answer(true);
                }
                Ok(m)
            })
        }));
        self.clone()
    }

    /// `answersGoneOnce(backend, m, when)` (stub-backend.ts:137): the gateway's copy that never held the machine
    /// answering one read, the first read of `m` once `when` holds, by the backend or the handle, is a 404.
    pub fn answers_gone_once(self: &Arc<Self>, m: &Arc<StubMachine>, when: Rc<dyn Fn() -> bool>) {
        let lied = Rc::new(Cell::new(false));
        let lie = {
            let lied = lied.clone();
            move || {
                let lie = !lied.get() && when();
                if lie {
                    lied.set(true);
                }
                lie
            }
        };
        let get = self.get_fn();
        let id = m.id().to_string();
        let lie_get = lie.clone();
        self.set_get(Rc::new(move |asked| {
            if asked == id && lie_get() {
                return Box::pin(async { Err(missing("Not found")) });
            }
            get(asked)
        }));
        let state = m.state_fn();
        m.set_state(Rc::new(move || {
            if lie() {
                return Box::pin(async { Ok(MachineState::Gone) });
            }
            state()
        }));
    }

    fn find(&self, id: &str) -> Result<Arc<StubMachine>, EngineError> {
        match self.machines.borrow().iter().find(|m| m.id == id) {
            Some(m) if !m.killed.get() => Ok(m.clone()),
            _ => Err(missing("gone")),
        }
    }

    /// The create the stub itself does: stub-backend.ts:270.
    pub async fn make(&self, spec: MachineSpec, _waiting: Option<Waiting>, _sent: Option<String>) -> Result<Arc<StubMachine>, EngineError> {
        if let Some(template) = &spec.template {
            let ready = self
                .templates
                .borrow()
                .get(template)
                .is_some_and(|(row, _)| row.status == wsp_wire::wire::machine_link::TemplateRowStatus::Ready);
            if !BUILTIN_TEMPLATES.contains(&template.as_str()) && !ready {
                return Err(missing(&format!("TemplateNotReady {template}")));
            }
        }
        self.seq.set(self.seq.get() + 1);
        let id = format!("m{}", self.seq.get());
        let m = StubMachine::new(self.me.clone(), id, spec, self.create_notice.borrow().clone(), Some(self.vault.clone()));
        self.machines.borrow_mut().push(m.clone());
        Ok(m)
    }

    fn take_snapshot(&self, m: &StubMachine, name: &str) -> Result<String, EngineError> {
        let nth_ask = {
            let mut asks = self.snapshot_asks.borrow_mut();
            let n = asks.get(&m.id).copied().unwrap_or(0) + 1;
            asks.insert(m.id.clone(), n);
            n
        };
        if let Some(before) = self.before_snapshot.borrow().clone() {
            before(m, nth_ask)?;
        }
        // The provider mints an id per call; a repeated name (two in one millisecond) must not fold into one row.
        let nth = {
            let mut named = self.snapshots_named.borrow_mut();
            let n = named.get(name).copied().unwrap_or(0) + 1;
            named.insert(name.to_string(), n);
            n
        };
        let at = self.mark.as_ref().map(|m| format!("{m}-")).unwrap_or_default();
        let id = if self.named_snapshots.get() {
            name.to_string()
        } else if nth == 1 {
            format!("snap_{at}{name}")
        } else {
            format!("snap_{at}{name}-{nth}")
        };
        let mut rows = self.snapshots.borrow_mut();
        if let Some(held) = rows.iter().position(|r| r.id == id) {
            rows.remove(held);
        }
        let bytes = JsNumber(self.snapshot_bytes.get() as f64);
        rows.push(SnapshotRow {
            id: id.clone(),
            name: Some(name.to_string()),
            size_bytes: bytes,
            restored_bytes: Some(bytes),
            created_at: Some(iso_time(super::now_ms())),
            parent: None,
        });
        Ok(id)
    }
}

#[async_trait(?Send)]
impl MachineBackend for StubBackend {
    fn capabilities(&self) -> Capabilities {
        self.capabilities.borrow().clone()
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
        Arc::new(StubPricing)
    }
    fn lifecycle(&self) -> Option<Arc<dyn Lifecycle>> {
        Some(self.lifecycle.clone())
    }
    fn names_workspace(&self) -> bool {
        self.names_workspace.get()
    }
    fn offers(&self) -> BackendOffers {
        BackendOffers {
            on_computer: true,
            keyed_create: self.create_request.borrow().is_some()
                && self.replay_create.borrow().is_some()
                && self.named_after.borrow().is_some(),
            list_snapshots: true,
            templates: true,
            ..BackendOffers::default()
        }
    }

    async fn on_computer(&self, cmd: &str, _timeout_ms: Option<u64>) -> Result<ExecResult, EngineError> {
        self.computer_log.borrow_mut().push(cmd.to_string());
        // The machine handed to the answer stands for the computer and is in no listing.
        let stand_in = StubMachine::new(
            self.me.clone(),
            "computer".into(),
            serde_json::from_value(serde_json::json!({ "kind": "sandbox" })).expect("a machine spec"),
            None,
            None,
        );
        *stand_in.exec_log.borrow_mut() = self.computer_log.borrow().clone();
        let run = self.exec_impl_fn();
        run(stand_in, cmd.to_string(), None).await
    }
    async fn create(&self, spec: MachineSpec, waiting: Option<Waiting>, sent: Option<String>) -> Result<Arc<dyn Machine>, EngineError> {
        bump(&self.calls.create);
        let f = self.create_fn();
        Ok(f(spec, waiting, sent).await?)
    }
    async fn get(&self, id: &str) -> Result<Arc<dyn Machine>, EngineError> {
        bump(&self.calls.get);
        self.calls.get_ids.borrow_mut().push(id.to_string());
        let f = self.get_fn();
        Ok(f(id.to_string()).await?)
    }
    async fn list(&self, _labels: Option<&IndexMap<String, String>>) -> Result<Vec<MachineListRow>, EngineError> {
        bump(&self.calls.list);
        Ok(self
            .machines
            .borrow()
            .iter()
            .filter(|m| !m.killed.get())
            .map(|m| MachineListRow {
                id: m.id.clone(),
                state: if m.paused.get() { MachineState::Paused } else { MachineState::Running },
                labels: m.spec.borrow().labels.clone().unwrap_or_default(),
                size: None,
            })
            .collect())
    }
    // Solari refuses a snapshot with live machines forked from it (409 SnapshotHasChildren) and one a template stands
    // on (409 SnapshotBacksTemplate); forks from the template hold no such dependency.
    async fn delete_snapshot(&self, id: &str) -> Result<(), EngineError> {
        if self.machines.borrow().iter().any(|m| !m.killed.get() && m.spec.borrow().from_snapshot.as_deref() == Some(id)) {
            return Err(conflict("SnapshotHasChildren"));
        }
        if self.templates.borrow().values().any(|(_, snapshot)| snapshot == id) {
            return Err(conflict("SnapshotBacksTemplate"));
        }
        let mut rows = self.snapshots.borrow_mut();
        if let Some(at) = rows.iter().position(|r| r.id == id) {
            rows.remove(at);
        }
        Ok(())
    }
    async fn create_request(&self, spec: &MachineSpec) -> Result<String, EngineError> {
        match self.create_request_fn() {
            Some(f) => f(spec.clone()).await,
            None => Err(EngineError::Other { message: "backend.createRequest is not a function".into(), code: None }),
        }
    }
    async fn replay_create(&self, key: &str, bytes: &str, made_at: &str) -> Result<Option<Arc<dyn Machine>>, EngineError> {
        match self.replay_create_fn() {
            Some(f) => Ok(f(key.to_string(), bytes.to_string(), made_at.to_string()).await?.map(|m| m as Arc<dyn Machine>)),
            None => Err(EngineError::Other { message: "backend.replayCreate is not a function".into(), code: None }),
        }
    }
    async fn named_after(&self, names: &BTreeSet<String>) -> Result<Vec<NamedMachine>, EngineError> {
        let f = self.named_after.borrow().clone();
        match f {
            Some(f) => f(names.clone()).await,
            None => Err(EngineError::Other { message: "backend.namedAfter is not a function".into(), code: None }),
        }
    }
    async fn list_snapshots(&self, _ids: Option<&[String]>) -> Result<Vec<SnapshotRow>, EngineError> {
        Ok(self.snapshots.borrow().clone())
    }
    async fn promote_snapshot(&self, snapshot_id: &str, name: &str) -> Result<String, EngineError> {
        if !self.snapshots.borrow().iter().any(|r| r.id == snapshot_id) {
            return Err(missing("Not found"));
        }
        self.promoted.borrow_mut().push((snapshot_id.to_string(), name.to_string()));
        let id = format!("tpl_{name}");
        let row = TemplateRow {
            id: id.clone(),
            name: name.to_string(),
            status: wsp_wire::wire::machine_link::TemplateRowStatus::Ready,
            error: None,
            created_at: None,
        };
        self.templates.borrow_mut().insert(id.clone(), (row, snapshot_id.to_string()));
        Ok(id)
    }
    async fn get_template(&self, id: &str) -> Result<TemplateRow, EngineError> {
        self.templates.borrow().get(id).map(|(row, _)| row.clone()).ok_or_else(|| missing("Not found"))
    }
    async fn list_templates(&self) -> Result<Vec<TemplateRow>, EngineError> {
        Ok(self.templates.borrow().values().map(|(row, _)| row.clone()).collect())
    }
    async fn delete_template(&self, id: &str) -> Result<(), EngineError> {
        self.templates.borrow_mut().shift_remove(id).map(|_| ()).ok_or_else(|| missing("Not found"))
    }
}

/// `replayed` handed to a wait the way `waiting?.("replayed", { id, state: "running" })` does.
pub fn say_replayed(waiting: &Option<Waiting>, id: &str) {
    if let Some(waiting) = waiting {
        waiting("replayed", Some(&ProviderBoxState { id: id.to_string(), state: "running".into() }));
    }
}
