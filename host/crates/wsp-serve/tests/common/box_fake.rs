// SPDX-License-Identifier: AGPL-3.0-only
//! box-nap.test.ts:583 `restoringBox`: a Boat box as the provider's API shows it to a host that finds it running. Its state
//! is whatever `state()` says, and every command is refused with sandbox_restoring until the box's own clock reaches
//! `restored_at`, as a box does while its disk streams in behind a ready reading. The backend's sleeps move that clock and
//! cost nothing. A test can hold the answer to one command until it lets go (an abort of the call ends the wait the way a cut
//! connection does), delete the box, or have the provider's agent fail every command. A stop archives the box, which then
//! refuses every command as not running, and a resume calls `resumed` and takes the archive off. A test can also hold the
//! answer to the next read of the box. `CountedMachine` is `machine.proveRoad = ...` and `machine.state = ...` of the TS: the
//! real Boat machine with the two calls counted or held.
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use wsp_engine::box_backend::machine::{sudo_command, BoxMachine};
use wsp_engine::box_backend::plan::BOX_API_URL;
use wsp_engine::box_backend::request::{BoxBackend, BoxBackendOptions, BoxBudgetsAsk};
use wsp_engine::cancel::Cancel;
use wsp_engine::errors::{EngineError, RetryClock, RoadError};
use wsp_engine::fetch::{Fetch, FetchRequest, FetchResponse};
use wsp_engine::machine::{
    ExecOptions, ExecResult, Machine, MachineFacts, MachineKind, MachineOffers, MachineShape, MachineState, PreviewReach, RunOptions, Seen,
    SnapshotOptions,
};

use super::Gate;

/// The box's own clock: `{ at, now, sleep }` of the TS, which a sleep moves and which costs nothing.
pub struct BoxClock(pub Rc<BoxState>);

#[async_trait(?Send)]
impl RetryClock for BoxClock {
    fn now(&self) -> i64 {
        self.0.at.get()
    }
    async fn sleep(&self, ms: u64) {
        self.0.at.set(self.0.at.get() + ms as i64);
    }
    fn random(&self) -> f64 {
        0.5
    }
}

pub struct BoxState {
    pub id: String,
    pub at: Cell<i64>,
    /// Whether each command the box took found it still restoring.
    pub commands: RefCell<Vec<bool>>,
    pub deleted: Cell<bool>,
    pub agent_fails: Cell<bool>,
    pub no_work_dir: Cell<bool>,
    pub restored_at: Cell<f64>,
    pub hold: RefCell<Option<Rc<Gate>>>,
    pub archived: Cell<bool>,
    pub read_hold: RefCell<Option<Rc<Gate>>>,
    pub read_held: Cell<bool>,
    state: Rc<dyn Fn() -> String>,
    resumed: Option<Rc<dyn Fn()>>,
}

impl BoxState {
    /// `box.hold = new Promise(...)`: the next proof command waits until the returned gate opens.
    pub fn hold_next(&self) -> Rc<Gate> {
        let gate = Gate::new();
        *self.hold.borrow_mut() = Some(gate.clone());
        gate
    }
    /// `box.readHold = new Promise(...)`: the next read of the box waits until the returned gate opens.
    pub fn hold_read(&self) -> Rc<Gate> {
        let gate = Gate::new();
        *self.read_hold.borrow_mut() = Some(gate.clone());
        gate
    }
    pub fn last_command_restoring(&self) -> Option<bool> {
        self.commands.borrow().last().copied()
    }
}

struct Gateway(Rc<BoxState>);

fn reply(status: u16, body: Value) -> FetchResponse {
    FetchResponse { status, headers: vec![("content-type".into(), "application/json".into())], body: body.to_string().into_bytes() }
}

fn not_found() -> FetchResponse {
    reply(404, json!({ "ok": false, "status": 404, "code": "not_found", "message": "Box not found." }))
}

#[async_trait(?Send)]
impl Fetch for Gateway {
    async fn fetch(&self, req: FetchRequest) -> Result<FetchResponse, RoadError> {
        let b = &self.0;
        let after_host = req.url.split_once("://").map_or(req.url.as_str(), |(_, rest)| rest.split_once('/').map_or("", |(_, p)| p));
        let path = format!("/{after_host}");
        let path = path.split('?').next().unwrap_or("").strip_prefix("/api/v1").unwrap_or(&path).to_string();
        let id = &b.id;
        if path == "/limits" {
            return Ok(reply(200, json!({ "ok": true, "accessTier": "paid" })));
        }
        if req.method == "DELETE" && path == format!("/sandboxes/{id}") {
            b.deleted.set(true);
            return Ok(reply(202, json!({ "ok": true, "operation": { "id": "bdop_x" } })));
        }
        if req.method == "POST" && path == format!("/sandboxes/{id}/stop") {
            b.archived.set(true);
            return Ok(reply(202, json!({ "ok": true })));
        }
        if req.method == "POST" && path == format!("/sandboxes/{id}/resume") {
            b.archived.set(false);
            if let Some(resumed) = &b.resumed {
                resumed();
            }
            return Ok(reply(202, json!({ "ok": true })));
        }
        if req.method == "GET" && path == format!("/sandboxes/{id}") {
            let held = b.read_hold.borrow_mut().take();
            if let Some(held) = held {
                b.read_held.set(true);
                held.wait().await;
            }
            return Ok(if b.deleted.get() {
                not_found()
            } else {
                let state = if b.archived.get() { "archived".to_string() } else { (b.state)() };
                reply(200, json!({ "ok": true, "sandbox": { "id": id, "state": state } }))
            });
        }
        if req.method == "POST" && path == format!("/sandboxes/{id}/commands") {
            let command = req.body.as_deref().and_then(|raw| serde_json::from_slice::<Value>(raw).ok()).map(|v| v["command"].as_str().unwrap_or_default().to_string());
            let held = b.hold.borrow().clone();
            if let (Some(held), Some(command)) = (held, &command) {
                if *command == sudo_command("true") {
                    *b.hold.borrow_mut() = None;
                    let aborted = async {
                        match &req.abort {
                            Some(abort) => abort.token.cancelled().await,
                            None => futures::future::pending().await,
                        }
                    };
                    tokio::select! {
                        _ = held.wait() => {}
                        _ = aborted => {
                            return Err(RoadError { name: "AbortError".into(), message: "This operation was aborted".into(), code: None, cause: None });
                        }
                    }
                }
            }
            if b.deleted.get() {
                return Ok(not_found());
            }
            if b.archived.get() {
                return Ok(reply(409, json!({ "ok": false, "status": 409, "code": "box_not_running", "message": "Box is not running." })));
            }
            if b.no_work_dir.get() {
                return Ok(reply(400, json!({ "ok": false, "status": 400, "code": "invalid_cwd", "message": "cwd must be an existing directory." })));
            }
            if b.agent_fails.get() {
                return Ok(reply(
                    500,
                    json!({ "ok": false, "status": 500, "code": "internal_error", "message": "Cannot read properties of undefined (reading 'on')" }),
                ));
            }
            let restoring = (b.at.get() as f64) < b.restored_at.get();
            b.commands.borrow_mut().push(restoring);
            return Ok(if restoring {
                reply(409, json!({ "ok": false, "status": 409, "code": "sandbox_restoring", "message": "Box is restoring." }))
            } else {
                reply(200, json!({ "ok": true, "exitCode": 0, "stdout": "", "stderr": "", "timedOut": false }))
            });
        }
        Ok(not_found())
    }
}

/// The box and the real Boat machine over it, counted.
pub struct RestoringBox {
    pub state: Rc<BoxState>,
    pub machine: Arc<CountedMachine>,
}

/// `restoringBox(id, { state, restoredAt, resumed })`.
#[allow(clippy::arc_with_non_send_sync)]
pub fn restoring_box(id: &str, state: impl Fn() -> String + 'static, restored_at: f64, resumed: impl Fn() + 'static) -> RestoringBox {
    let shared = Rc::new(BoxState {
        id: id.into(),
        at: Cell::new(0),
        commands: RefCell::default(),
        deleted: Cell::new(false),
        agent_fails: Cell::new(false),
        no_work_dir: Cell::new(false),
        restored_at: Cell::new(restored_at),
        hold: RefCell::default(),
        archived: Cell::new(false),
        read_hold: RefCell::default(),
        read_held: Cell::new(false),
        state: Rc::new(state),
        resumed: Some(Rc::new(resumed)),
    });
    let backend = BoxBackend::new(BoxBackendOptions {
        api_key: "box_x".into(),
        base_url: Some(BOX_API_URL.to_string()),
        fetch: Some(Arc::new(Gateway(shared.clone()))),
        clock: Some(Arc::new(BoxClock(shared.clone()))),
        budgets: BoxBudgetsAsk { poll_ms: Some(10), restore_ms: Some(100), ..Default::default() },
        warn: None,
    });
    let inner: Arc<dyn Machine> = Arc::new(BoxMachine::new(backend, id, MachineKind::Sandbox, None, None, false));
    RestoringBox { state: shared, machine: Arc::new(CountedMachine { inner, proofs: Cell::new(0), before_state: RefCell::new(None) }) }
}

type Before = Rc<dyn Fn() -> LocalBoxFuture<'static, ()>>;

/// The real machine with `proveRoad` counted and `state` optionally held before it reads.
pub struct CountedMachine {
    inner: Arc<dyn Machine>,
    pub proofs: Cell<u32>,
    before_state: RefCell<Option<Before>>,
}

impl CountedMachine {
    /// `machine.state = async () => { await barrier; return read(); }`.
    pub fn before_each_state_read(&self, before: Before) {
        *self.before_state.borrow_mut() = Some(before);
    }
}

#[async_trait(?Send)]
impl Machine for CountedMachine {
    fn id(&self) -> &str {
        self.inner.id()
    }
    fn kind(&self) -> MachineKind {
        self.inner.kind()
    }
    fn stream_url(&self) -> Option<&str> {
        self.inner.stream_url()
    }
    fn labels(&self) -> Option<&IndexMap<String, String>> {
        self.inner.labels()
    }
    fn seen(&self) -> Option<&Seen> {
        self.inner.seen()
    }
    fn replayed(&self) -> bool {
        self.inner.replayed()
    }
    fn notice(&self) -> Option<&str> {
        self.inner.notice()
    }
    fn takes_stdin(&self) -> bool {
        self.inner.takes_stdin()
    }
    fn daemon_token_path(&self) -> Option<&str> {
        self.inner.daemon_token_path()
    }
    fn daemon_answers_prompt_ms(&self) -> Option<u64> {
        self.inner.daemon_answers_prompt_ms()
    }
    fn daemon_supervisor(&self) -> Option<wsp_engine::machine::DaemonSupervisor> {
        self.inner.daemon_supervisor()
    }
    fn offers(&self) -> MachineOffers {
        self.inner.offers()
    }
    async fn exec(&self, cmd: &str, opts: ExecOptions) -> Result<ExecResult, EngineError> {
        self.inner.exec(cmd, opts).await
    }
    async fn run(&self, script: &str, opts: RunOptions) -> Result<ExecResult, EngineError> {
        self.inner.run(script, opts).await
    }
    async fn snapshot(&self, name: &str, life: wsp_engine::machine::MachineLife, opts: SnapshotOptions) -> Result<String, EngineError> {
        self.inner.snapshot(name, life, opts).await
    }
    async fn pause(&self, wait_ms: Option<u64>) -> Result<(), EngineError> {
        self.inner.pause(wait_ms).await
    }
    async fn resume(&self, signal: Option<&Cancel>) -> Result<(), EngineError> {
        self.inner.resume(signal).await
    }
    async fn kill(&self) -> Result<(), EngineError> {
        self.inner.kill().await
    }
    async fn state(&self) -> Result<MachineState, EngineError> {
        let before = self.before_state.borrow().clone();
        if let Some(before) = before {
            before().await;
        }
        self.inner.state().await
    }
    async fn download_url(&self, path: &str) -> Result<String, EngineError> {
        self.inner.download_url(path).await
    }
    async fn upload_url(&self, path: &str) -> Result<String, EngineError> {
        self.inner.upload_url(path).await
    }
    async fn framed(&self) -> Result<Arc<dyn wsp_engine::machine::Framing>, EngineError> {
        self.inner.framed().await
    }
    async fn resume_at(&self, cpu: f64, mem_mb: u64, signal: Option<&Cancel>) -> Result<(), EngineError> {
        self.inner.resume_at(cpu, mem_mb, signal).await
    }
    async fn preview_url(&self, port: u16) -> Result<PreviewReach, EngineError> {
        self.inner.preview_url(port).await
    }
    async fn daemon_answers(&self, timeout_ms: Option<u64>) -> Result<bool, EngineError> {
        self.inner.daemon_answers(timeout_ms).await
    }
    async fn prove_road(&self, signal: Option<&Cancel>) -> Result<(), EngineError> {
        self.proofs.set(self.proofs.get() + 1);
        self.inner.prove_road(signal).await
    }
    async fn start_daemon(&self) -> Result<ExecResult, EngineError> {
        self.inner.start_daemon().await
    }
    async fn daemon_exits(&self) -> Result<Option<String>, EngineError> {
        self.inner.daemon_exits().await
    }
    async fn put_bytes(&self, path: &str, bytes: &[u8], timeout_ms: Option<u64>) -> Result<Option<wsp_engine::machine::BytesLanded>, EngineError> {
        self.inner.put_bytes(path, bytes, timeout_ms).await
    }
    async fn read_bytes(&self, path: &str, size: Option<u64>, timeout_ms: Option<u64>) -> Result<Vec<u8>, EngineError> {
        self.inner.read_bytes(path, size, timeout_ms).await
    }
    async fn daemon_frame(&self, frame: Map<String, Value>) -> Result<Map<String, Value>, EngineError> {
        self.inner.daemon_frame(frame).await
    }
    async fn describe(&self) -> Result<MachineShape, EngineError> {
        self.inner.describe().await
    }
    async fn facts(&self) -> Result<MachineFacts, EngineError> {
        self.inner.facts().await
    }
    async fn metrics(&self) -> Result<(), EngineError> {
        self.inner.metrics().await
    }
}
