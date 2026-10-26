// SPDX-License-Identifier: AGPL-3.0-only
//! `ReachCtx` over the fake host: every call kept by name, every answer its default unless a test sets a knob. The
//! knobs and the link plan are per test thread, so each `#[test]` starts clean through `reach_host()`.
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use futures::FutureExt;
use indexmap::IndexMap;
use serde_json::{json, Value};
use tokio::sync::{oneshot, watch, Semaphore};

use super::engine_error;
use wsp_core::clock::Cancel;
use wsp_core::reach::{DaemonReach, Frame, ReachError, ReachStats};
use wsp_core::types::internal::SessionEntry;
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_engine::machine::{ExecOptions, Machine, MachineBackend};
use wsp_machines::machines::area_error::{AreaError, AreaResult};
use wsp_machines::machines::machines::Entry;
use wsp_machines::machines::reach::{ReachArea, ReachCtx};
use wsp_wire::types::js_number::JsInt;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::workspace::{ProjectView, WorkspacePhase};
use wsp_wire::wire::capabilities::PauseMode;
use wsp_wire::wire::daemon::DaemonLinkStatus;
use wsp_wire::wire::daemon_events::DaemonEvent;
use wsp_words::exit::HostError;

use super::queue_ctx::{session_entry, snapshot_of};
use super::{entry_id, FakeHost};

/// The commands the fake `sweep_leftovers` runs on the machine, so a test can gate or count them.
pub const SWEEP_STAGED: &str = "wsp-sweep-staged";
pub const SWEEP_CACHES: &str = "rm -rf /root/.npm /root/.cache/wsp-build";
/// What `checkout_of` answers, and this computer's home as `on_this_computer` hands it to a frame.
pub const CWD: &str = "/root/web";
pub const HOME: &str = "/home/here";

// ---------------------------------------------------------------------------------------------------------------
// Knobs: the standing answers the context gives, set by a test.

pub struct Knobs {
    pub area: RefCell<Weak<ReachArea>>,
    /// `unreached_of` by workspace id.
    pub unreached: RefCell<IndexMap<String, String>>,
    pub has_daemon: Cell<bool>,
    pub idle_at: RefCell<IndexMap<String, f64>>,
    pub served: Cell<bool>,
    pub provider: RefCell<Option<String>>,
    /// What `daemon_token_of` answers: `None` is a guest with no token.
    pub token: RefCell<Option<String>>,
    /// Over the backend's lifecycle budget when set.
    pub daemon_answers_ms: Cell<Option<f64>>,
    pub hello_timeout_ms: Cell<f64>,
    /// Project views by id, as JSON.
    pub projects: RefCell<IndexMap<String, Value>>,
    /// What `with_daemon` answers per workspace id (the git.status reply or the failure's words).
    pub status: RefCell<IndexMap<String, Result<Value, String>>>,
    /// Every `with_daemon` waits on this before it answers.
    pub status_gate: RefCell<Option<Arc<Semaphore>>>,
    /// What `on_this_computer` answers for the frame it was handed.
    pub compare: RefCell<Option<Rc<dyn Fn(&Value) -> Result<Value, String>>>>,
    /// When set, `project_held` answers this error (the TS `projectHeld` throws on an unknown project).
    pub project_held_failure: RefCell<Option<HostError>>,
    /// When set, `checkout_of` answers this error (the TS `checkoutOf` reaches `projectHeld`).
    pub checkout_failure: RefCell<Option<HostError>>,
    /// When set, `kind_has_daemon` answers this error (the TS `moduleOf(kind).hasDaemon` throws on an unwired kind).
    pub kind_has_daemon_failure: RefCell<Option<HostError>>,
    /// When set, `connect_daemon` answers this error (the TS `connectDaemon` throws on options it cannot dial).
    pub connect_failure: RefCell<Option<HostError>>,
}

impl Knobs {
    fn new() -> Knobs {
        Knobs {
            area: RefCell::new(Weak::new()),
            unreached: RefCell::new(IndexMap::new()),
            has_daemon: Cell::new(false),
            idle_at: RefCell::new(IndexMap::new()),
            served: Cell::new(false),
            provider: RefCell::new(Some("box".to_string())),
            token: RefCell::new(Some("tok-1".to_string())),
            daemon_answers_ms: Cell::new(None),
            hello_timeout_ms: Cell::new(2_000.0),
            projects: RefCell::new(IndexMap::new()),
            status: RefCell::new(IndexMap::new()),
            status_gate: RefCell::new(None),
            compare: RefCell::new(None),
            project_held_failure: RefCell::new(None),
            checkout_failure: RefCell::new(None),
            kind_has_daemon_failure: RefCell::new(None),
            connect_failure: RefCell::new(None),
        }
    }
}

/// One dial `connect_daemon` was asked for.
#[derive(Clone, Debug, PartialEq)]
pub struct Dial {
    pub preview_url: String,
    pub token: String,
    pub heartbeat_ms: Option<f64>,
}

/// How the links the area dials behave: up at once or once a test says so, with the hellos they deliver.
pub struct LinkPlan {
    up_tx: watch::Sender<bool>,
    up_rx: watch::Receiver<bool>,
    /// Delivered through `on_event` right after a link is made, in order.
    pub hellos: RefCell<Vec<f64>>,
    pub ping_error: RefCell<Option<String>>,
    pub ready_error: RefCell<Option<String>>,
    pub dials: RefCell<Vec<Dial>>,
    pub links: RefCell<Vec<Rc<FakeLink>>>,
}

impl LinkPlan {
    fn new() -> LinkPlan {
        let (up_tx, up_rx) = watch::channel(false);
        LinkPlan {
            up_tx,
            up_rx,
            hellos: RefCell::new(Vec::new()),
            ping_error: RefCell::new(None),
            ready_error: RefCell::new(None),
            dials: RefCell::new(Vec::new()),
            links: RefCell::new(Vec::new()),
        }
    }
    /// The daemon behind every link now answers.
    pub fn set_up(&self) {
        let _ = self.up_tx.send(true);
    }
    async fn wait_up(&self) {
        let mut rx = self.up_rx.clone();
        loop {
            let up = *rx.borrow();
            if up {
                return;
            }
            if rx.changed().await.is_err() {
                return;
            }
        }
    }
}

/// A link to a daemon that answers `ready` and `ping` once the plan is up.
pub struct FakeLink {
    plan: Rc<LinkPlan>,
    on_event: Rc<dyn Fn(DaemonEvent)>,
    pub closed: Cell<u32>,
    pub requests: RefCell<Vec<String>>,
}

impl FakeLink {
    /// Delivers a `daemon.hello` frame through the callback the area gave.
    pub fn hello(&self, version: f64) {
        (self.on_event)(hello_event(version));
    }
}

/// A `daemon.hello` frame of a version.
fn hello_event(version: f64) -> DaemonEvent {
    DaemonEvent::DaemonHello {
        root: "/root".to_string(),
        version: JsInt::from(version as i64),
    }
}

/// The link's own failure: words only, as the real link's readiness and ping failures carry.
fn link_error(message: &str) -> ReachError {
    ReachError {
        message: message.to_string(),
        kind: None,
        status: None,
        code: None,
    }
}

#[async_trait(?Send)]
impl DaemonReach for FakeLink {
    async fn ready(&self) -> Result<(), ReachError> {
        let err = self.plan.ready_error.borrow().clone();
        if let Some(m) = err {
            return Err(link_error(&m));
        }
        self.plan.wait_up().await;
        Ok(())
    }
    async fn request(&self, op: &str, _params: Frame) -> Result<Frame, ReachError> {
        self.requests.borrow_mut().push(op.to_string());
        let err = self.plan.ping_error.borrow().clone();
        if let Some(m) = err {
            return Err(link_error(&m));
        }
        self.plan.wait_up().await;
        let mut reply = Frame::new();
        reply.insert("ok".to_string(), Value::Bool(true));
        Ok(reply)
    }
    fn status(&self) -> Option<DaemonLinkStatus> {
        None
    }
    fn stats(&self) -> ReachStats {
        ReachStats::default()
    }
    fn close(&self) {
        self.closed.set(self.closed.get() + 1);
    }
}

thread_local! {
    static KNOBS: RefCell<Rc<Knobs>> = RefCell::new(Rc::new(Knobs::new()));
    static PLAN: RefCell<Rc<LinkPlan>> = RefCell::new(Rc::new(LinkPlan::new()));
}

pub fn knobs() -> Rc<Knobs> {
    KNOBS.with(|k| k.borrow().clone())
}

pub fn plan() -> Rc<LinkPlan> {
    PLAN.with(|p| p.borrow().clone())
}

/// A new host with the knobs and the link plan reset.
pub fn reach_host() -> Rc<FakeHost> {
    KNOBS.with(|k| *k.borrow_mut() = Rc::new(Knobs::new()));
    PLAN.with(|p| *p.borrow_mut() = Rc::new(LinkPlan::new()));
    FakeHost::new()
}

/// The reach area over the host; the host's `row_reason` reads its reasons through the area.
pub fn reach(host: &Rc<FakeHost>) -> Rc<ReachArea> {
    let area = ReachArea::new(host.clone());
    *knobs().area.borrow_mut() = Rc::downgrade(&area);
    area
}

// ---------------------------------------------------------------------------------------------------------------
// Fixtures.

/// A cloud record as the TS writes it, on a machine, in a phase.
pub fn plain_record(id: &str, name: &str, phase: &str, machine_id: &str) -> Value {
    json!({
        "id": id, "name": name, "kind": "cloud", "machineId": machine_id, "phase": phase, "golden": "snap_g",
        "createdAt": "2026-10-12T06:00:00.000Z", "project": "pr_1", "spec": {}, "firstLife": true, "place": "box", "size": { "cpu": 2, "memMb": 4096 }
    })
}

/// A session row with only the keys given, as a turn leaves one.
pub fn put_session(
    host: &FakeHost,
    turn_id: &str,
    workspace_id: &str,
    thread_id: Option<&str>,
    started_at: Option<f64>,
    claude_session_id: Option<&str>,
) {
    let mut v = json!({ "id": turn_id, "workspaceId": workspace_id, "harness": "claude", "status": "completed" });
    let o = v.as_object_mut().unwrap();
    if let Some(t) = thread_id {
        o.insert("threadId".to_string(), json!(t));
    }
    if let Some(s) = started_at {
        o.insert("startedAt".to_string(), json!(s));
    }
    if let Some(c) = claude_session_id {
        o.insert("claudeSessionId".to_string(), json!(c));
    }
    let view: SessionView = serde_json::from_value(v).unwrap();
    host.sessions.borrow_mut().insert(
        turn_id.to_string(),
        session_entry(view, turn_id.to_string()),
    );
}

/// A project view as JSON.
pub fn project_json(id: &str, remote: &str, default_branch: &str, base: Option<&str>) -> Value {
    let mut v = json!({
        "id": id, "name": id, "computer": "this-computer", "source": { "kind": "git", "url": remote }, "path": "/root/web",
        "remote": remote, "defaultBranch": default_branch, "memoryKey": id, "memoryDir": "/root/.claude/projects/x",
        "createdAt": "2026-10-12T06:00:00.000Z"
    });
    if let Some(b) = base {
        v["base"] = json!(b);
    }
    v
}

pub fn set_project(id: &str, remote: &str, default_branch: &str, base: Option<&str>) {
    knobs().projects.borrow_mut().insert(
        id.to_string(),
        project_json(id, remote, default_branch, base),
    );
}

/// A `git.status` reply: one entry per xy code given, the branch one commit ahead.
pub fn status_reply(head: &str, oid: &str, entries: &[&str]) -> Value {
    let rows: Vec<Value> = entries
        .iter()
        .enumerate()
        .map(|(i, xy)| json!({ "xy": xy, "path": format!("f{i}.txt") }))
        .collect();
    json!({ "branch": { "oid": oid, "head": head, "ahead": 1, "behind": 0 }, "entries": rows, "root": "/root/stub" })
}

pub fn answer_status(workspace_id: &str, reply: Value) {
    knobs()
        .status
        .borrow_mut()
        .insert(workspace_id.to_string(), Ok(reply));
}

pub fn fail_status(workspace_id: &str, said: &str) {
    knobs()
        .status
        .borrow_mut()
        .insert(workspace_id.to_string(), Err(said.to_string()));
}

/// What this computer's daemon answers a frame with.
pub fn on_compare(f: impl Fn(&Value) -> Result<Value, String> + 'static) {
    let c: Rc<dyn Fn(&Value) -> Result<Value, String>> = Rc::new(f);
    *knobs().compare.borrow_mut() = Some(c);
}

/// Every number as an integer where it is one, so a float 2.0 and a 2 read the same; key order is kept.
pub fn canon(v: &Value) -> Value {
    match v {
        Value::Number(n) => match n.as_f64() {
            Some(f) if f.fract() == 0.0 && f.abs() < 9e15 => json!(f as i64),
            _ => v.clone(),
        },
        Value::Array(a) => Value::Array(a.iter().map(canon).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), canon(x))).collect()),
        other => other.clone(),
    }
}

/// The JSON text of a value in its key order with whole numbers written as JS writes them.
pub fn js_text(v: &Value) -> String {
    serde_json::to_string(&canon(v)).unwrap()
}

// ---------------------------------------------------------------------------------------------------------------
// The context.

#[async_trait(?Send)]
impl ReachCtx for FakeHost {
    fn live(&self, id: &str) -> Option<Entry> {
        self.live.borrow().get(id).cloned()
    }
    fn live_all(&self) -> Vec<Entry> {
        self.live.borrow().values().cloned().collect()
    }
    fn session_entries(&self) -> Vec<SessionEntry> {
        self.sessions.borrow().values().map(snapshot_of).collect()
    }
    fn backend_for(&self, _record: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError> {
        Ok(self.backend.clone())
    }
    fn pause_keeps_disk(&self, backend: &dyn MachineBackend) -> bool {
        backend.capabilities().pause_mode == Some(PauseMode::Disk)
    }
    async fn sweep_leftovers(&self, machine: &Arc<dyn Machine>) -> AreaResult<()> {
        self.call("sweep_leftovers", &[machine.id()]);
        if let Some(Err(e)) = self.next("sweep_leftovers") {
            return Err(e);
        }
        machine
            .exec(SWEEP_STAGED, ExecOptions::default())
            .await
            .map_err(AreaError::Engine)?;
        machine
            .exec(SWEEP_CACHES, ExecOptions::default())
            .await
            .map_err(AreaError::Engine)?;
        Ok(())
    }
    fn closing(&self) -> bool {
        self.closing.get()
    }
    fn provider_of(&self, _record: &WorkspaceRecord) -> Option<String> {
        knobs().provider.borrow().clone()
    }
    fn row_reason(&self, entry: &Entry) -> Option<String> {
        let area = knobs().area.borrow().upgrade()?;
        let said = entry.borrow().wake_said.clone();
        said.or_else(|| area.nap_refused_reason(entry))
            .or_else(|| area.stop_refused_reason(entry))
            .or_else(|| area.disk_reason(entry))
    }
    fn unreached_of(&self, entry: &Entry) -> Option<String> {
        knobs().unreached.borrow().get(&entry_id(entry)).cloned()
    }
    fn kind_has_daemon(&self, _entry: &Entry) -> AreaResult<bool> {
        if let Some(e) = knobs().kind_has_daemon_failure.borrow().clone() {
            return Err(AreaError::Host(e));
        }
        Ok(knobs().has_daemon.get())
    }
    fn view(&self, record: &WorkspaceRecord) -> Value {
        json!({ "id": record.id, "name": record.name, "phase": record.phase })
    }
    fn machine_state_of(&self, phase: &WorkspacePhase) -> String {
        let p = serde_json::to_value(phase).unwrap();
        let word = match p.as_str().unwrap_or("") {
            "running" | "pausing" => "running",
            "napping" => "paused",
            "waking" | "queued" => "starting",
            _ => "gone",
        };
        word.to_string()
    }
    fn idle_at(&self, workspace_id: &str) -> Option<f64> {
        knobs().idle_at.borrow().get(workspace_id).copied()
    }
    fn daemon_answers_ms(&self, _entry: &Entry) -> AreaResult<f64> {
        Ok(knobs()
            .daemon_answers_ms
            .get()
            .unwrap_or(self.backend.lifecycle.daemon_answers_ms))
    }
    fn daemon_hello_timeout_ms(&self) -> f64 {
        knobs().hello_timeout_ms.get()
    }
    async fn daemon_token_of(&self, machine: &Arc<dyn Machine>) -> AreaResult<Option<String>> {
        self.call("daemon_token_of", &[machine.id()]);
        if let Some(Err(e)) = self.next("daemon_token_of") {
            return Err(e);
        }
        Ok(knobs().token.borrow().clone())
    }
    fn connect_daemon(
        &self,
        preview_url: &str,
        token: &str,
        on_event: Rc<dyn Fn(DaemonEvent)>,
        heartbeat_ms: Option<i64>,
    ) -> AreaResult<Rc<dyn DaemonReach>> {
        self.call("connect_daemon", &[preview_url, token]);
        if let Some(e) = knobs().connect_failure.borrow().clone() {
            return Err(AreaError::Host(e));
        }
        let plan = plan();
        plan.dials.borrow_mut().push(Dial {
            preview_url: preview_url.to_string(),
            token: token.to_string(),
            heartbeat_ms: heartbeat_ms.map(|h| h as f64),
        });
        let link = Rc::new(FakeLink {
            plan: plan.clone(),
            on_event: on_event.clone(),
            closed: Cell::new(0),
            requests: RefCell::new(Vec::new()),
        });
        plan.links.borrow_mut().push(link.clone());
        let hellos = plan.hellos.borrow().clone();
        if !hellos.is_empty() {
            let deliver = on_event.clone();
            self.spawn_local(
                async move {
                    tokio::task::yield_now().await;
                    for v in hellos {
                        deliver(hello_event(v));
                    }
                }
                .boxed_local(),
            );
        }
        Ok(link)
    }
    fn project_held(&self, id: &str) -> AreaResult<ProjectView> {
        if let Some(e) = knobs().project_held_failure.borrow().clone() {
            return Err(AreaError::Host(e));
        }
        let v = knobs()
            .projects
            .borrow()
            .get(id)
            .cloned()
            .unwrap_or_else(|| panic!("the test set no project {id}"));
        Ok(serde_json::from_value(v).expect("a project view"))
    }
    async fn with_daemon(&self, entry: &Entry, frame: Value) -> AreaResult<Value> {
        let id = entry_id(entry);
        self.call("with_daemon", &[&id, &js_text(&frame)]);
        let gate = knobs().status_gate.borrow().clone();
        if let Some(g) = gate {
            let _held = g.acquire().await.unwrap();
        }
        let answer = knobs().status.borrow().get(&id).cloned();
        match answer {
            Some(Ok(v)) => Ok(v),
            Some(Err(m)) => Err(AreaError::Engine(engine_error(&m))),
            None => Err(AreaError::Engine(engine_error(
                "no daemon answer was set for this workspace",
            ))),
        }
    }
    async fn on_this_computer(&self, frame: Box<dyn Fn(&str) -> Value>) -> AreaResult<Value> {
        let f = frame(HOME);
        self.call("on_this_computer", &[&js_text(&f)]);
        let answer = knobs().compare.borrow().clone();
        match answer {
            Some(c) => c(&f).map_err(|m| AreaError::Engine(engine_error(&m))),
            None => Err(AreaError::Engine(engine_error(
                "this computer's daemon answers nothing",
            ))),
        }
    }
    fn checkout_of(&self, _record: &WorkspaceRecord) -> AreaResult<String> {
        if let Some(e) = knobs().checkout_failure.borrow().clone() {
            return Err(AreaError::Host(e));
        }
        Ok(CWD.to_string())
    }
    fn served_by_its_computer(&self, _entry: &Entry) -> bool {
        knobs().served.get()
    }
    async fn persist(&self, record: &WorkspaceRecord) -> AreaResult<()> {
        self.call("persist", &[&record.id]);
        if let Some(Err(e)) = self.next("persist") {
            return Err(e);
        }
        self.persist_now(record);
        Ok(())
    }
    fn emit(&self, event: Value) {
        self.emit_now(event);
    }
    fn record(&self, event: Value) {
        self.recorded.borrow_mut().push(event);
    }
    fn now(&self) -> f64 {
        self.clock.now()
    }
    fn schedule(&self, ms: f64, f: Box<dyn FnOnce()>) -> Cancel {
        self.clock.schedule(ms, f)
    }
    fn sleep(&self, ms: f64) -> LocalBoxFuture<'static, ()> {
        let (tx, rx) = oneshot::channel::<()>();
        self.clock.schedule(
            ms,
            Box::new(move || {
                let _ = tx.send(());
            }),
        );
        async move {
            let _ = rx.await;
        }
        .boxed_local()
    }
    fn spawn(&self, f: LocalBoxFuture<'static, ()>) {
        self.spawn_local(f);
    }
    fn warn(&self, line: &str) {
        self.warns.borrow_mut().push(line.to_string());
    }
}
