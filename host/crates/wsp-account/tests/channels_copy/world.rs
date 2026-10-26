
use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;
use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use tokio::sync::Notify;
use common::{error_json, in_flight, live_with, ok_exec, oracle, plain, project, record, run, settle, ExecScript, FakeMachine};
use std::sync::Arc;
use wsp_account::account::box_panels::{BoxPanelsCtx, StartedRow};
use wsp_account::account::channels::copy::{ChildRoad, ShellToken};
use wsp_account::account::channels::{ChannelsArea, ChannelsCtx, TerminalSignIn, ThreadLaunch, WORKSPACE_FRAMES};
use wsp_account::account::core_types::{AccountError, AccountResult, Entry, Frame, Release, Served};
use wsp_core::clock::Cancel;
use wsp_core::context::{RuntimeState, StateCell};
use wsp_core::daemon_channel::{ChannelEnd, ChannelError, DaemonChannel};
use wsp_core::devices::{DeviceDoor, HeldDevice, IssuedCode, MintMore, PairedDevice, ScopedRoad};
use wsp_core::types::internal::ThreadRecord;
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_engine::machine::{ExecResult, Machine};
use wsp_wire::views::place_more::{PlaceSettings, PlaceView};
use wsp_wire::views::workspace::{Caller, DaemonReachView, ProjectView};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin};
use wsp_wire::wire::daemon::ProcScope;
use wsp_wire::wire::devices::{DeviceVia, DeviceView};
use wsp_wire::wire::place_link::PlaceReport;
use wsp_words::exit::HostError;































// The fake context ----------------------------------------------------------------------------------------------

/// The ordered effects the area had: frames the daemon got, events the client heard, store writes, revokes,
/// warnings, execs, in the order they happened.
#[derive(Clone, Default)]
struct Fx(Rc<RefCell<Vec<Value>>>);


impl Fx {
    fn push(&self, v: Value) {
        self.0.borrow_mut().push(v);
    }
    fn len(&self) -> usize {
        self.0.borrow().len()
    }
    fn all(&self) -> Vec<Value> {
        self.0.borrow().clone()
    }
}


/// A named wait a step holds and releases.
struct Gate {
    open: Cell<bool>,
    wake: Notify,
}


impl Gate {
    fn new() -> Rc<Gate> {
        Rc::new(Gate { open: Cell::new(false), wake: Notify::new() })
    }
    async fn wait(&self) {
        while !self.open.get() {
            let waiting = self.wake.notified();
            if self.open.get() {
                break;
            }
            waiting.await;
        }
    }
    fn release(&self) {
        self.open.set(true);
        self.wake.notify_waiters();
    }
}


type Gates = RefCell<IndexMap<String, Rc<Gate>>>;


struct Daemon {
    fx: Fx,
    gates: Rc<Gates>,
    ptys: RefCell<Vec<String>>,
    made: Cell<i64>,
    replies: RefCell<IndexMap<String, Vec<Value>>>,
    throws: IndexMap<String, String>,
}


impl Daemon {
    fn scripted(&self, op: &str) -> Option<Value> {
        let mut all = self.replies.borrow_mut();
        let queue = all.get_mut(op)?;
        Some(if queue.len() > 1 { queue.remove(0) } else { queue[0].clone() })
    }

    async fn serve(&self, frame: Frame) -> Result<Frame, String> {
        self.fx.push(json!(["frame", Value::Object(frame.clone())]));
        let op = frame.get("op").and_then(Value::as_str).unwrap_or("").to_string();
        let gate = self.gates.borrow().get(&op).cloned();
        if let Some(gate) = gate {
            gate.wait().await;
        }
        if let Some(m) = self.throws.get(&op) {
            return Err(m.clone());
        }
        if let Some(reply) = self.scripted(&op) {
            return Ok(reply.as_object().expect("a scripted reply is an object").clone());
        }
        let reply = match op.as_str() {
            "pty.create" => {
                self.made.set(self.made.get() + 1);
                let id = format!("p{}", self.made.get());
                self.ptys.borrow_mut().push(id.clone());
                json!({ "ok": true, "ptyId": id, "pid": 4000 + self.made.get() })
            }
            "pty.kill" => {
                let gone = frame.get("ptyId").and_then(Value::as_str).unwrap_or("").to_string();
                self.ptys.borrow_mut().retain(|id| *id != gone);
                json!({ "ok": true })
            }
            "pty.list" => json!({ "ok": true, "ptys": self.ptys.borrow().iter().map(|id| json!({ "id": id })).collect::<Vec<_>>() }),
            _ => json!({ "ok": true }),
        };
        Ok(reply.as_object().unwrap().clone())
    }
}


/// How the machine's exec answers, which a step may change: `ok`, `never`, `gate`, `{throw}` or `{exit, stderr}`.
type ExecMode = Rc<RefCell<Value>>;


/// A machine whose exec logs the call and answers by `mode`, serving daemon frames when `served` is given.
fn sim_machine(id: &str, served: Option<Served>, mode: ExecMode, fx: Fx, gates: Rc<Gates>) -> Arc<FakeMachine> {
    let machine = FakeMachine::new(id);
    *machine.served.borrow_mut() = served;
    let run: ExecScript = Rc::new(move |cmd: String, timeout_ms: i64| {
        let (mode, fx, gates) = (mode.clone(), fx.clone(), gates.clone());
        Box::pin(async move {
            fx.push(json!(["exec", cmd, timeout_ms]));
            let mode = mode.borrow().clone();
            if mode == json!("never") {
                futures::future::pending::<()>().await;
            }
            if mode == json!("gate") {
                let gate = gates.borrow().get("exec").cloned();
                if let Some(gate) = gate {
                    gate.wait().await;
                }
            }
            if let Some(m) = mode.get("throw").and_then(Value::as_str) {
                return Err(m.to_string());
            }
            if mode.get("exit").is_some() {
                return Ok(ExecResult {
                    exit_code: mode["exit"].as_i64().unwrap().into(),
                    stdout: String::new(),
                    stderr: mode["stderr"].as_str().unwrap_or("").to_string(),
                });
            }
            Ok(ok_exec())
        })
    });
    *machine.exec_script.borrow_mut() = Some(run);
    machine
}


/// The clock the fake context hands the area: timers fire only when a step advances it.
struct SimClock {
    now: Cell<i64>,
    seq: Cell<u64>,
    timers: RefCell<Vec<(i64, u64, Rc<Cell<bool>>, Option<Box<dyn FnOnce()>>)>>,
}


impl SimClock {
    fn schedule(&self, ms: i64, f: Box<dyn FnOnce()>) -> Cancel {
        self.seq.set(self.seq.get() + 1);
        let cancelled = Rc::new(Cell::new(false));
        self.timers.borrow_mut().push((self.now.get() + ms, self.seq.get(), cancelled.clone(), Some(f)));
        Rc::new(move || cancelled.set(true))
    }

    async fn advance(&self, ms: i64) {
        let target = self.now.get() + ms;
        loop {
            let next = {
                let mut timers = self.timers.borrow_mut();
                let best = timers
                    .iter()
                    .enumerate()
                    .filter(|(_, t)| t.0 <= target && !t.2.get() && t.3.is_some())
                    .min_by_key(|(_, t)| (t.0, t.1))
                    .map(|(i, _)| i);
                best.map(|i| {
                    let t = &mut timers[i];
                    (t.0, t.3.take().unwrap())
                })
            };
            let Some((at, f)) = next else { break };
            self.now.set(at);
            f();
            settle().await;
        }
        self.now.set(target);
    }
}


struct OwnChannel {
    id: u32,
    sim: Rc<SimShared>,
    daemon: Rc<Daemon>,
    closed: Rc<Gate>,
}


#[async_trait(?Send)]
impl DaemonChannel for OwnChannel {
    async fn send(&self, frame: Frame) -> Result<Frame, ChannelError> {
        self.daemon.serve(frame).await.map_err(|message| ChannelError::Unanswered { message, cause: None })
    }
    fn close(&self) {
        self.sim.listeners.borrow_mut().shift_remove(&self.id);
        self.sim.fx.push(json!(["own-closed"]));
        self.closed.release();
    }
    async fn closed(&self) -> ChannelEnd {
        self.closed.wait().await;
        ChannelEnd { code: 1000, reason: String::new() }
    }
}


struct LinkChannel {
    fx: Fx,
    dropped: Rc<Gate>,
}


#[async_trait(?Send)]
impl DaemonChannel for LinkChannel {
    async fn send(&self, _frame: Frame) -> Result<Frame, ChannelError> {
        Ok(Frame::new())
    }
    fn close(&self) {
        self.fx.push(json!(["link-closed"]));
    }
    async fn closed(&self) -> ChannelEnd {
        self.dropped.wait().await;
        ChannelEnd { code: 1000, reason: String::new() }
    }
}


struct SimDoor {
    fx: Fx,
    revoke_fails: bool,
}


#[async_trait(?Send)]
impl DeviceDoor for SimDoor {
    async fn issue(&self, _now: i64, _ttl_ms: u64, _here: bool) -> Result<IssuedCode, HostError> {
        unreachable!("the copy area issues no code")
    }
    async fn redeem(&self, _code: &str, _name: &str, _now: i64) -> Result<Option<PairedDevice>, HostError> {
        unreachable!()
    }
    async fn spend(&self, _code: &str, _now: i64) -> Result<bool, HostError> {
        unreachable!()
    }
    async fn admit(&self, _name: &str, _now: i64) -> Result<PairedDevice, HostError> {
        unreachable!()
    }
    async fn admit_account(&self, _name: &str, _via: DeviceVia, _now: i64) -> Result<PairedDevice, HostError> {
        unreachable!()
    }
    async fn refuses(&self, _fingerprint: &str) -> Result<bool, HostError> {
        unreachable!()
    }
    async fn mint(&self, _name: &str, _scope: ThreadScope, _now: i64, _more: MintMore) -> Result<PairedDevice, HostError> {
        unreachable!("the copy area mints through ctx.thread_launch")
    }
    async fn revoke_asides(&self) -> Result<(), HostError> {
        unreachable!()
    }
    async fn r#match(&self, _token: &str) -> Result<Option<HeldDevice>, HostError> {
        unreachable!()
    }
    async fn seen(&self, _id: &str, _now: i64) -> Result<Option<HeldDevice>, HostError> {
        unreachable!()
    }
    async fn list(&self) -> Result<Vec<DeviceView>, HostError> {
        unreachable!()
    }
    async fn revoke(&self, id: &str) -> Result<bool, HostError> {
        self.fx.push(json!(["revoke", id]));
        if self.revoke_fails {
            return Err(plain("revoke refused"));
        }
        Ok(true)
    }
}


type Bus = Rc<RefCell<Vec<Rc<dyn Fn(&str)>>>>;


/// What the context and the box panels' context share.
struct SimShared {
    fx: Fx,
    listeners: RefCell<IndexMap<u32, Rc<dyn Fn(Value)>>>,
}


struct SimPanels {
    fx: Fx,
    bus: Bus,
    w: Value,
}


#[async_trait(?Send)]
impl BoxPanelsCtx for SimPanels {
    fn on_workspace_deleted(&self, f: Rc<dyn Fn(&str)>) {
        self.bus.borrow_mut().push(f);
    }
    fn session_rows(&self) -> Vec<StartedRow> {
        vec![]
    }
    fn thread_records(&self) -> Vec<(String, String)> {
        self.w["threads"]
            .as_array()
            .map(|ts| ts.iter().map(|t| (t["id"].as_str().unwrap().to_string(), t["workspaceId"].as_str().unwrap().to_string())).collect())
            .unwrap_or_default()
    }
    fn link(&self, _place_id: &str) -> Option<Rc<()>> {
        Some(Rc::new(()))
    }
    fn name_of(&self, place_id: &str) -> String {
        door_name(&self.w, place_id)
    }
    async fn daemon_version(&self, place_id: &str) -> Option<i64> {
        self.w["door"]["reports"][place_id]["daemonVersion"].as_i64()
    }
    async fn watch_sys(&self, _place_id: &str) -> Result<Release, String> {
        let fx = self.fx.clone();
        Ok(Rc::new(move || fx.push(json!(["released", "sys"]))))
    }
    fn scopes_on(&self, _place_id: &str) -> Vec<ProcScope> {
        vec![]
    }
    fn thread_env(&self, _entry: &Entry) -> IndexMap<String, String> {
        env_of(&self.w["threadEnv"])
    }
    fn spawn(&self, f: LocalBoxFuture<'static, ()>) {
        tokio::task::spawn_local(f);
    }
}


fn env_of(v: &Value) -> IndexMap<String, String> {
    v.as_object().map(|m| m.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string())).collect()).unwrap_or_default()
}


fn door_name(w: &Value, place_id: &str) -> String {
    w["door"]["names"][place_id].as_str().unwrap_or(place_id).to_string()
}


struct Sim {
    shared: Rc<SimShared>,
    fx: Fx,
    w: Value,
    daemon: Rc<Daemon>,
    gates: Rc<Gates>,
    clock: SimClock,
    docs: RefCell<IndexMap<String, IndexMap<String, Value>>>,
    live: RefCell<Vec<String>>,
    bus: Bus,
    projects: IndexMap<String, ProjectView>,
    threads: Vec<(String, ThreadRecord)>,
    minted: Cell<u32>,
    opened: Cell<u32>,
    link_on_event: RefCell<Option<Rc<dyn Fn(Value)>>>,
    link_dropped: Rc<Gate>,
    origins: RefCell<IndexMap<String, Option<String>>>,
    door: Rc<SimDoor>,
}


impl Sim {
    fn flag(&self, key: &str) -> bool {
        self.w[key] == json!(true)
    }
}


#[async_trait(?Send)]
impl ChannelsCtx for Sim {
    fn index_start(&self, _workspace_id: &str, _thread_id: &str) -> Option<String> {
        None
    }
    fn index_cut(&self, _workspace_id: &str, _thread_id: &str) -> Option<bool> {
        None
    }
    fn index_fact(&self, _workspace_id: &str, _resume: &str, _fact: &str) -> Option<String> {
        None
    }
    fn session_rows(&self) -> Vec<wsp_account::account::channels::ChannelRow> {
        vec![]
    }
    fn thread_record(&self, thread_id: &str) -> Option<ThreadRecord> {
        self.threads.iter().find(|(id, _)| id == thread_id).map(|(_, t)| t.clone())
    }
    fn thread_ids(&self) -> Vec<String> {
        self.threads.iter().map(|(id, _)| id.clone()).collect()
    }
    fn latest_harness(&self, thread_id: &str) -> Option<String> {
        self.w["latest"][thread_id].as_str().map(String::from)
    }

    fn home_dir(&self, _record: &WorkspaceRecord) -> Option<String> {
        None
    }
    async fn daemon_road(&self, _entry: &Entry) -> AccountResult<DaemonReachView> {
        Ok(serde_json::from_value(json!({ "url": "ws://daemon", "daemonToken": "dt", "expiresAt": 0 })).unwrap())
    }
    fn turn_road(&self, _kind: WorkspaceKind) -> Option<ScopedRoad> {
        None
    }

    fn has_door(&self) -> bool {
        !self.w["door"].is_null()
    }
    fn offer_of(&self, _place_id: &str) -> Option<String> {
        None
    }
    fn wired(&self) -> String {
        "solari".into()
    }
    fn settings_at(&self, _place_id: &str) -> Option<PlaceSettings> {
        None
    }
    fn turn_limit_at(&self, _place_id: &str) -> Option<Option<i64>> {
        None
    }
    fn name_of(&self, place_id: &str) -> String {
        door_name(&self.w, place_id)
    }
    fn joined(&self, _place_id: &str) -> bool {
        false
    }
    async fn report_of(&self, place_id: &str) -> Option<PlaceReport> {
        let report = &self.w["door"]["reports"][place_id];
        report.is_object().then(|| serde_json::from_value(report.clone()).unwrap_or_else(|e| panic!("report {report}: {e}")))
    }
    async fn rows(&self) -> Vec<PlaceView> {
        vec![]
    }
    fn link_channel(&self, _place_id: &str, on_event: Rc<dyn Fn(Value)>) -> Option<Rc<dyn DaemonChannel>> {
        if self.w["door"]["link"] == json!(false) {
            return None;
        }
        *self.link_on_event.borrow_mut() = Some(on_event);
        Some(Rc::new(LinkChannel { fx: self.fx.clone(), dropped: self.link_dropped.clone() }))
    }
    async fn spend_today(&self, _rows: &[PlaceView], _now: i64) -> Vec<(String, Option<f64>)> {
        vec![]
    }

    fn projects_held(&self) -> IndexMap<String, ProjectView> {
        self.projects.clone()
    }

    async fn open_channel(&self, _url: &str, _token: &str, on_event: Rc<dyn Fn(Value)>) -> AccountResult<Rc<dyn DaemonChannel>> {
        self.opened.set(self.opened.get() + 1);
        let id = self.opened.get();
        self.shared.listeners.borrow_mut().insert(id, on_event);
        Ok(Rc::new(OwnChannel { id, sim: self.shared.clone(), daemon: self.daemon.clone(), closed: Gate::new() }))
    }
    fn local_has_daemon(&self) -> bool {
        false
    }
    fn local_home_dir(&self) -> String {
        "/home/u".into()
    }
    async fn local_road(&self) -> AccountResult<DaemonReachView> {
        unreachable!("the copy area dials no local road")
    }
    async fn adopt_pause(&self, _entry: &Entry) {}
    fn broker_ready(&self) -> bool {
        self.flag("broker")
    }
    fn root_of(&self, thread_id: &str) -> String {
        format!("root-{thread_id}")
    }
    fn device_door(&self) -> Rc<dyn DeviceDoor> {
        self.door.clone()
    }

    fn host_pid(&self) -> i64 {
        0
    }
    fn host_parent_pid(&self) -> i64 {
        0
    }
    fn has_state_path(&self) -> bool {
        false
    }
    fn state_folder(&self) -> String {
        String::new()
    }
    fn read_link(&self, _path: &str) -> Option<String> {
        None
    }
    fn hostname(&self) -> String {
        "host".into()
    }
    fn pid_alive(&self, _pid: i64) -> bool {
        false
    }
    fn agent_bins(&self) -> Vec<String> {
        vec![]
    }
    fn agent_pid_of(&self, _run: &str) -> Option<i64> {
        None
    }
    fn group_exists(&self, _pgid: i64) -> bool {
        false
    }
    fn proc_scopes_moved(&self) {}
    fn state(&self) -> StateCell<RuntimeState> {
        StateCell::new(RuntimeState::default())
    }
    fn box_panels_ctx(&self) -> Rc<dyn BoxPanelsCtx> {
        Rc::new(SimPanels { fx: self.fx.clone(), bus: self.bus.clone(), w: self.w.clone() })
    }

    async fn store_get(&self, collection: &str, id: &str) -> AccountResult<Option<Value>> {
        Ok(self.docs.borrow().get(collection).and_then(|rows| rows.get(id)).cloned())
    }
    async fn store_put(&self, collection: &str, id: &str, value: Value) -> AccountResult<()> {
        if self.flag("putFails") {
            return Err(plain("disk full").into());
        }
        if collection != "pty-threads" {
            self.fx.push(json!(["put", collection, id, value.clone()]));
        }
        self.docs.borrow_mut().entry(collection.to_string()).or_default().insert(id.to_string(), value);
        Ok(())
    }
    async fn store_delete(&self, collection: &str, id: &str) -> AccountResult<()> {
        if collection != "pty-threads" {
            self.fx.push(json!(["store-delete", collection, id]));
        }
        if let Some(rows) = self.docs.borrow_mut().get_mut(collection) {
            rows.shift_remove(id);
        }
        Ok(())
    }
    async fn store_keys(&self, collection: &str) -> AccountResult<Vec<String>> {
        Ok(self.docs.borrow().get(collection).map(|rows| rows.keys().cloned().collect()).unwrap_or_default())
    }
    fn on_workspace_deleted(&self, f: Rc<dyn Fn(&str)>) {
        self.bus.borrow_mut().push(f);
    }
    fn live_has(&self, workspace_id: &str) -> bool {
        self.live.borrow().iter().any(|id| id == workspace_id)
    }

    async fn thread_launch(&self, _entry: &Entry, thread_id: &str, root_thread_id: &str) -> AccountResult<ThreadLaunch> {
        self.fx.push(json!(["launch", thread_id, root_thread_id]));
        let env: IndexMap<String, String> =
            [("LAUNCH".to_string(), "1".to_string()), ("HOST_TOKEN".to_string(), "launch".to_string())].into_iter().collect();
        if self.w["scoped"] == json!(false) {
            return Ok(ThreadLaunch { scoped: None, env });
        }
        self.minted.set(self.minted.get() + 1);
        let n = self.minted.get();
        let device: DeviceView = serde_json::from_value(json!({ "id": format!("d{n}"), "name": "shell", "createdAt": "2026-10-25T00:00:00.000Z", "lastSeenAt": "2026-10-25T00:00:00.000Z" })).unwrap();
        Ok(ThreadLaunch { scoped: Some(PairedDevice { device_id: format!("d{n}"), device_token: format!("tok{n}"), device }), env })
    }
    fn brokered_env(&self, token: &str, _entry: &Entry) -> IndexMap<String, String> {
        [("HOST_TOKEN".to_string(), token.to_string()), ("GH_TOKEN".to_string(), token.to_string())].into_iter().collect()
    }
    fn terminal_sign_in(&self, harness: &str, _entry: &Entry, token: &str) -> Option<TerminalSignIn> {
        match harness {
            "claude" => Some(TerminalSignIn {
                program: Some("claude-bin".into()),
                env: [("SIGN_IN".to_string(), format!("claude:{token}"))].into_iter().collect(),
            }),
            "codex" => Some(TerminalSignIn {
                program: Some("codex-bin".into()),
                env: [("SIGN_IN".to_string(), format!("codex:{token}")), ("CODEX_EXTRA".to_string(), "1".to_string())]
                    .into_iter()
                    .collect(),
            }),
            _ => None,
        }
    }
    fn terminal_line(&self, harness: &str, program: Option<&str>) -> String {
        format!("write-terminal {harness} {}", program.unwrap_or(""))
    }
    async fn thread_folder(&self, _entry: &Entry) -> String {
        self.fx.push(json!(["thread-folder"]));
        self.w["threadFolder"].as_str().unwrap_or("/srv/alpha/work").to_string()
    }
    fn thread_env(&self, _entry: &Entry) -> IndexMap<String, String> {
        env_of(&self.w["threadEnv"])
    }
    async fn persist(&self, record: &WorkspaceRecord) -> AccountResult<()> {
        if self.flag("persistFails") {
            return Err(plain("store refused").into());
        }
        let branch = record.worktree.as_ref().and_then(|t| t.branch.clone());
        self.fx.push(json!(["persist", record.id, branch]));
        Ok(())
    }
    fn read_checkout_soon(&self, entry: &Entry) {
        self.fx.push(json!(["read-checkout", entry.borrow().record.borrow().id]));
    }
    async fn delete_workspace(&self, workspace_id: &str) -> AccountResult<()> {
        self.fx.push(json!(["delete-workspace", workspace_id]));
        if self.flag("deleteFails") {
            return Err(plain("delete refused").into());
        }
        Ok(())
    }
    fn git_origin(&self, path: &str, timeout_ms: i64) -> (Option<i32>, String) {
        self.fx.push(json!(["origin-read", path, timeout_ms]));
        match self.origins.borrow().get(path) {
            Some(Some(remote)) => (Some(0), remote.clone()),
            _ => (Some(128), String::new()),
        }
    }

    fn now(&self) -> i64 {
        self.clock.now.get()
    }
    fn schedule(&self, ms: i64, f: Box<dyn FnOnce()>) -> Cancel {
        self.clock.schedule(ms, f)
    }
    fn spawn(&self, f: LocalBoxFuture<'static, ()>) {
        tokio::task::spawn_local(f);
    }
    fn warn(&self, line: &str) {
        self.fx.push(json!(["warn", line]));
    }
}


// The world and the script ------------------------------------------------------------------------------------

struct Built {
    sim: Rc<Sim>,
    entry: Entry,
    mode: ExecMode,
    area: Rc<ChannelsArea>,
}


fn build(w: &Value) -> Built {
    let fx = Fx::default();
    let gates: Rc<Gates> = Rc::new(RefCell::new(IndexMap::new()));
    let replies = w["daemon"]["replies"]
        .as_object()
        .map(|m| m.iter().map(|(op, r)| (op.clone(), r.as_array().cloned().unwrap_or_else(|| vec![r.clone()]))).collect())
        .unwrap_or_default();
    let throws = w["daemon"]["throws"]
        .as_object()
        .map(|m| m.iter().map(|(op, r)| (op.clone(), r.as_str().unwrap().to_string())).collect())
        .unwrap_or_default();
    let daemon = Rc::new(Daemon {
        fx: fx.clone(),
        gates: gates.clone(),
        ptys: RefCell::new(vec![]),
        made: Cell::new(0),
        replies: RefCell::new(replies),
        throws,
    });
    let served: Option<Served> = (w["machine"]["served"] == json!(true)).then(|| {
        let d = daemon.clone();
        let f: Served = Rc::new(move |frame: Frame| {
            let d = d.clone();
            Box::pin(async move { d.serve(frame).await.map_err(|m| AccountError::from(plain(&m))) })
                as LocalBoxFuture<'static, AccountResult<Frame>>
        });
        f
    });
    let rec = record(&w["record"]);
    let mode: ExecMode = Rc::new(RefCell::new(w["machine"].get("exec").cloned().unwrap_or(json!("ok"))));
    let machine = sim_machine(&rec.machine_id, served, mode.clone(), fx.clone(), gates.clone());
    let entry = live_with(rec, machine);
    if w["deleting"] == json!(true) {
        entry.borrow_mut().deleting = Some(in_flight());
    }

    let docs = w["store"]
        .as_object()
        .map(|m| {
            m.iter().map(|(c, rows)| (c.clone(), rows.as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.clone())).collect())).collect()
        })
        .unwrap_or_default();
    let live: Vec<String> = match w["live"].as_array() {
        Some(ids) => ids.iter().map(|v| v.as_str().unwrap().to_string()).collect(),
        None => vec![w["record"]["id"].as_str().unwrap().to_string()],
    };
    let threads = w["threads"]
        .as_array()
        .map(|ts| {
            ts.iter()
                .map(|t| {
                    (
                        t["id"].as_str().unwrap().to_string(),
                        ThreadRecord {
                            workspace_id: t["workspaceId"].as_str().unwrap().to_string(),
                            harness: t["harness"].as_str().unwrap_or("").to_string(),
                            ..Default::default()
                        },
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let projects = w["projects"].as_array().unwrap().iter().map(|p| (p["id"].as_str().unwrap().to_string(), project(p))).collect();
    let shared = Rc::new(SimShared { fx: fx.clone(), listeners: RefCell::new(IndexMap::new()) });
    let sim = Rc::new(Sim {
        shared,
        fx: fx.clone(),
        w: w.clone(),
        daemon,
        gates,
        clock: SimClock { now: Cell::new(0), seq: Cell::new(0), timers: RefCell::new(vec![]) },
        docs: RefCell::new(docs),
        live: RefCell::new(live),
        bus: Rc::new(RefCell::new(vec![])),
        projects,
        threads,
        minted: Cell::new(0),
        opened: Cell::new(0),
        link_on_event: RefCell::new(None),
        link_dropped: Gate::new(),
        origins: RefCell::new(IndexMap::new()),
        door: Rc::new(SimDoor { fx, revoke_fails: w["revokeFails"] == json!(true) }),
    });
    let area = ChannelsArea::new(sim.clone());
    Built { sim, entry, mode, area }
}


fn thrown(e: &AccountError) -> Value {
    match e {
        AccountError::Host(h) => error_json(h),
        AccountError::Refusal(r) => json!({ "message": r.message }),
    }
}


async fn send_value(channel: Rc<dyn DaemonChannel>, frame: Value) -> Value {
    let op = frame["op"].clone();
    match channel.send(frame.as_object().unwrap().clone()).await {
        Ok(reply) => json!({ "op": op, "reply": Value::Object(reply) }),
        Err(e) => json!({ "op": op, "thrown": e.to_string() }),
    }
}


#[derive(Clone, Copy, PartialEq)]
enum Mode {
    /// `copy_channel`, the road a client takes.
    Copy,
    /// `served_channel` alone, for the scenarios where copy_channel adds nothing of its own.
    Served,
}


/// What each scenario's script leaves: the answer of each step and every effect, as the TS wrote them.
async fn play(world: &Value, script: &[Value], mode: Mode) -> (Value, Value) {
    let mut b = build(world);
    let carries: Vec<&str> = WORKSPACE_FRAMES.to_vec();
    let mut steps: Vec<Value> = vec![];
    let mut channels: IndexMap<String, Rc<dyn DaemonChannel>> = IndexMap::new();
    let mut background: IndexMap<String, tokio::task::JoinHandle<Value>> = IndexMap::new();
    for step in script {
        let on = step["on"].as_str().unwrap_or("a").to_string();
        match step["step"].as_str().unwrap() {
            "open" => {
                let as_ = step["as"].as_str().unwrap_or("a").to_string();
                let fx = b.sim.fx.clone();
                let tag = as_.clone();
                let on_event: Rc<dyn Fn(Value)> = Rc::new(move |e: Value| fx.push(json!(["client", tag, e])));
                let opened = match mode {
                    Mode::Copy => b.area.copy_channel(&b.entry, on_event, &carries).await,
                    Mode::Served => {
                        let machine = b.entry.borrow().machine.clone();
                        assert!(machine.offers().daemon_frame, "a served scenario");
                        let served: Served = Rc::new(move |frame: Frame| {
                            let machine = machine.clone();
                            Box::pin(
                                async move { machine.daemon_frame(frame).await.map_err(|e| AccountError::from(plain(&e.to_string()))) },
                            )
                        });
                        b.area.served_channel(&b.entry, served, on_event, &carries).await
                    }
                };
                match opened {
                    Ok(channel) => {
                        channels.insert(as_, channel);
                        steps.push(json!({ "open": "ok" }));
                    }
                    Err(e) => steps.push(json!({ "open": { "thrown": thrown(&e) } })),
                }
            }
            "send" => {
                let frame = step["frame"].clone();
                let channel = channels[&on].clone();
                match step["bg"].as_str() {
                    Some(name) => {
                        background.insert(name.to_string(), tokio::task::spawn_local(send_value(channel, frame)));
                    }
                    None => steps.push(send_value(channel, frame).await),
                }
            }
            "finish" => steps.push(background.shift_remove(step["name"].as_str().unwrap()).unwrap().await.unwrap()),
            "close" => channels[&on].close(),
            "push" => {
                let event = step["event"].clone();
                if world["machine"]["served"] == json!(true) {
                    let f = b.sim.link_on_event.borrow().clone();
                    if let Some(f) = f {
                        f(event);
                    }
                } else {
                    let fs: Vec<Rc<dyn Fn(Value)>> = b.sim.shared.listeners.borrow().values().cloned().collect();
                    for f in fs {
                        f(event.clone());
                    }
                }
            }
            "linkDrop" => b.sim.link_dropped.release(),
            "hold" => {
                b.sim.gates.borrow_mut().insert(step["gate"].as_str().unwrap().to_string(), Gate::new());
            }
            "release" => {
                let gate = b.sim.gates.borrow().get(step["gate"].as_str().unwrap()).cloned().unwrap();
                gate.release();
            }
            "machine" => *b.mode.borrow_mut() = step["exec"].clone(),
            "daemon" => {
                if let Some(id) = step["drop"].as_str() {
                    b.sim.daemon.ptys.borrow_mut().retain(|p| p != id);
                }
                if let Some(replies) = step["replies"].as_object() {
                    for (op, r) in replies {
                        b.sim.daemon.replies.borrow_mut().insert(op.clone(), vec![r.clone()]);
                    }
                }
            }
            "advance" => b.sim.clock.advance(step["ms"].as_i64().unwrap()).await,
            "delete" => {
                let id = step["workspace"].as_str().unwrap().to_string();
                b.sim.live.borrow_mut().retain(|l| *l != id);
                let listeners: Vec<Rc<dyn Fn(&str)>> = b.sim.bus.borrow().clone();
                for f in listeners {
                    f(&id);
                }
            }
            "restart" => {
                b.sim.bus.borrow_mut().clear();
                b.area = ChannelsArea::new(b.sim.clone());
            }
            "heldDevices" => {
                let held: BTreeSet<String> = b.area.shell_held_devices().await.expect("the store reads");
                steps.push(json!({ "held": held.into_iter().collect::<Vec<_>>() }));
            }
            "lead" => {
                let road = if step["road"] == json!("run") { ChildRoad::Run } else { ChildRoad::Fork };
                let child = step["child"].as_str().unwrap();
                steps.push(match b.area.lead_start(&b.entry, child, road).await {
                    Ok(cs) => json!({ "lead": { "base": cs.base, "onLeads": cs.on_leads, "lines": cs.lines } }),
                    Err(e) => json!({ "lead": { "thrown": thrown(&e) } }),
                });
            }
            "startChild" => {
                let (branch, lead) = (step["branch"].as_str().unwrap(), step["lead"].as_str().unwrap());
                steps.push(match b.area.start_child_on(&b.entry, branch, lead).await {
                    Ok(line) => json!({ "startChild": line }),
                    Err(e) => json!({ "startChild": { "thrown": thrown(&e) } }),
                });
            }
            "origins" => {
                *b.sim.origins.borrow_mut() =
                    step["map"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_str().map(String::from))).collect();
            }
            "same" => {
                let caller = match step["caller"].as_str() {
                    Some("scoped") => Some(Caller::Scoped {
                        origin: WorkspaceOrigin::Here,
                        by: serde_json::from_value::<ThreadScope>(
                            json!({ "kind": "thread", "threadId": "t1", "workspaceId": "w1", "rootThreadId": "t1" }),
                        )
                        .unwrap(),
                    }),
                    Some("here") => Some(Caller::Road(WorkspaceOrigin::Here)),
                    _ => None,
                };
                let a = b.sim.projects[step["a"].as_str().unwrap()].clone();
                let other = b.sim.projects[step["b"].as_str().unwrap()].clone();
                let before = b.sim.fx.len();
                let answer = b.area.same_repository(&a, &other, caller.as_ref());
                let reads: Vec<Value> = b.sim.fx.all()[before..].iter().map(|e| e[1].clone()).collect();
                steps.push(json!({ "same": answer, "reads": reads }));
            }
            "settle" => {}
            "probe" => {
                let frames = b.sim.fx.all().iter().filter(|e| e[0] == json!("frame")).count();
                steps.push(json!({ "probe": frames }));
            }
            other => panic!("unknown step {other}"),
        }
        settle().await;
    }
    (Value::Array(steps), Value::Array(canon(b.sim.fx.all())))
}


/// Closing a channel closes the link, releases the readings hold and the like in no order a port must keep, so each
/// run of those entries is put in name order, as the oracle does. Every other effect keeps the order it happened in.
fn canon(fx: Vec<Value>) -> Vec<Value> {
    let loose = |e: &Value| ["link-closed", "own-closed", "released"].contains(&e[0].as_str().unwrap_or(""));
    let mut out: Vec<Value> = vec![];
    let mut run: Vec<Value> = vec![];
    for e in fx {
        if loose(&e) {
            run.push(e);
        } else {
            run.sort_by_key(|e| e.to_string());
            out.append(&mut run);
            out.push(e);
        }
    }
    run.sort_by_key(|e| e.to_string());
    out.append(&mut run);
    out
}


fn scenarios() -> Map<String, Value> {
    oracle("channels_copy")["scenarios"].as_object().unwrap().clone()
}


/// Runs every scenario whose name starts with one of `prefixes` and compares what each left with the TS's.
fn check(prefixes: &[&str]) {
    check_where(&format!("{prefixes:?}"), |name| prefixes.iter().any(|p| name.starts_with(p)));
}


/// The same for every scenario `pick` takes.
fn check_where(what: &str, pick: impl Fn(&str) -> bool) {
    let fixture = oracle("channels_copy");
    let c = &fixture["constants"];
    assert_eq!(c["INLINE_EXEC_MS"], json!(wsp_engine::exec_detached::put::INLINE_EXEC_MS));
    assert_eq!(c["THREAD_ENV"], json!(wsp_wire::types::env::THREAD_ENV));
    assert_eq!(c["WORKSPACE_FRAMES"], json!(WORKSPACE_FRAMES));
    let all = scenarios();
    for (name, s) in &all {
        for (place, report) in s["world"]["door"]["reports"].as_object().into_iter().flatten() {
            serde_json::from_value::<PlaceReport>(report.clone()).unwrap_or_else(|e| panic!("{name}: the report of {place}: {e}"));
        }
    }
    let mut ran = 0;
    for (name, s) in &all {
        if !pick(name) {
            continue;
        }
        ran += 1;
        for mode in [Mode::Copy, Mode::Served] {
            if mode == Mode::Served && !served_alone(name) {
                continue;
            }
            let (world, script) = (s["world"].clone(), s["script"].as_array().unwrap().clone());
            let (steps, fx) = run_one(world, script, mode);
            assert_eq!(
                serde_json::to_string(&steps).unwrap(),
                serde_json::to_string(&s["trace"]["steps"]).unwrap(),
                "{name}: the steps' answers"
            );
            assert_eq!(
                serde_json::to_string(&fx).unwrap(),
                serde_json::to_string(&s["trace"]["fx"]).unwrap(),
                "{name}: the effects, in order"
            );
        }
    }
    assert!(ran > 0, "no scenario for {what}");
}


fn run_one(world: Value, script: Vec<Value>, mode: Mode) -> (Value, Value) {
    let out = Rc::new(RefCell::new(None));
    let slot = out.clone();
    run(async move {
        *slot.borrow_mut() = Some(play(&world, &script, mode).await);
    });
    let taken = out.borrow_mut().take().unwrap();
    taken
}


/// The scenarios where `copy_channel` adds nothing of its own to a served machine's channel: no pty.create, which it
/// gives a folder, and no blocked computer, so `served_channel` alone must leave the same trace.
fn served_alone(name: &str) -> bool {
    [
        "c1 no place on the record",
        "c1 no door",
        "c1 no link",
        "c1 no place on a folder record",
        "c2 hello with the report's version",
        "c2 hello without a report",
        "c2 hello root is the worktree",
        "c3 listed ptys are named",
        "c3 the panels hear first on a folder",
        "c4 frame table",
        "c5 close detaches what stands attached",
        "c5 a refused attach is not held",
        "c5 the link dropping closes the panels",
        "c5 close closes the panels and the link",
    ]
    .contains(&name)
}
