
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;
use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use serde_json::{json, Value};
use common::{error_json, live_with, oracle, plain, project, record, run, settle, FakeMachine};
use std::sync::Arc;
use wsp_account::account::box_panels::{BoxPanelsCtx, StartedRow};
use wsp_account::account::channels::{
    ChannelRow, ChannelsArea, ChannelsCtx, TerminalSignIn, ThreadLaunch, BLOCKED_READS, HELLO_WAIT_MS, OP_TOKEN_OPS, ORIGIN_READ_MS,
};
use wsp_account::account::core_types::{AccountError, AccountResult, Ask, Entry, Frame, Release, Served};
use wsp_account::account::proc_scopes::ProcScopesArea;
use wsp_core::clock::{Cancel, Clock, FakeClock};
use wsp_core::context::{RuntimeState, StateCell};
use wsp_core::daemon_channel::{ChannelEnd, ChannelError, DaemonChannel};
use wsp_core::devices::{DeviceDoor, HeldDevice, IssuedCode, MintMore, PairedDevice, ScopedRoad};
use wsp_core::types::internal::ThreadRecord;
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_engine::machine::MachineState;
use wsp_wire::views::place_more::{PlaceSettings, PlaceView};
use wsp_wire::views::workspace::{DaemonReachView, ProjectView};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceAgents};
use wsp_wire::wire::devices::DeviceVia;
use wsp_wire::wire::place_link::PlaceReport;
use wsp_words::exit::HostError;
use wsp_account::account::core_types::Work;





























// ---------------------------------------------------------------------------------------------------------------------
// The fake host the channels area runs in: a clock of its own, a store that logs every call, a device door, daemons
// that answer by script, and a context holding every ctx member the TS reads. Nothing sleeps; spawned work runs when a
// test calls `settle().await`.
// ---------------------------------------------------------------------------------------------------------------------

/// A store of documents that logs every call as the oracle's did: `["put", collection, id, value]`,
/// `["delete", collection, id]`, and each read as `[collection, id]`. A put waits while `held` is set.
#[derive(Default)]
struct Store {
    docs: RefCell<IndexMap<(String, String), Value>>,
    ops: RefCell<Vec<Value>>,
    reads: RefCell<Vec<Value>>,
    held: Cell<bool>,
    fail_puts: RefCell<VecDeque<String>>,
    fail_get: RefCell<Option<String>>,
}


impl Store {
    fn doc(&self, collection: &str, id: &str) -> Value {
        self.docs.borrow().get(&(collection.to_string(), id.to_string())).cloned().unwrap_or(Value::Null)
    }
    fn seed(&self, collection: &str, id: &str, value: Value) {
        self.docs.borrow_mut().insert((collection.to_string(), id.to_string()), value);
    }
    fn ops(&self) -> Value {
        Value::Array(self.ops.borrow().clone())
    }
}


/// The device door: records every mint and revoke, and knows which tokens are live.
#[derive(Default)]
struct Door {
    minted: RefCell<Vec<Value>>,
    revoked: RefCell<Vec<String>>,
    live: RefCell<IndexMap<String, String>>,
    fail_revoke: Cell<bool>,
}


#[async_trait(?Send)]
impl DeviceDoor for Door {
    async fn issue(&self, _now: i64, _ttl_ms: u64, _here: bool) -> Result<IssuedCode, HostError> {
        unreachable!("the channels do not issue codes")
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
    async fn mint(&self, name: &str, scope: ThreadScope, now: i64, more: MintMore) -> Result<PairedDevice, HostError> {
        let n = self.minted.borrow().len() + 1;
        let (id, token) = (format!("dev_{n}"), format!("tok_{n}"));
        let more = match more.road {
            Some(road) => json!({ "road": serde_json::to_value(road).unwrap() }),
            None => json!({}),
        };
        self.minted.borrow_mut().push(json!({ "name": name, "scope": serde_json::to_value(&scope).unwrap(), "now": now, "more": more }));
        self.live.borrow_mut().insert(token.clone(), name.to_string());
        let device = serde_json::from_value(
            json!({ "id": id, "name": name, "createdAt": "2026-10-25T00:00:00.000Z", "lastSeenAt": "2026-10-25T00:00:00.000Z" }),
        )
        .unwrap();
        Ok(PairedDevice { device_id: id, device_token: token, device })
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
    async fn list(&self) -> Result<Vec<wsp_wire::wire::devices::DeviceView>, HostError> {
        unreachable!()
    }
    async fn revoke(&self, id: &str) -> Result<bool, HostError> {
        self.revoked.borrow_mut().push(id.to_string());
        let token = format!("tok_{}", id.trim_start_matches("dev_"));
        self.live.borrow_mut().shift_remove(&token);
        if self.fail_revoke.get() {
            return Err(plain("the door is shut"));
        }
        Ok(true)
    }
}


/// A daemon (or a link) that answers each frame with `answer`, keeping every frame it heard. `hold_next` parks the next
/// reply until `land`; `end` is the socket ending.
struct Daemon {
    answer: RefCell<Box<dyn FnMut(&Frame) -> Frame>>,
    heard: RefCell<Vec<Frame>>,
    hold_next: Cell<bool>,
    parked: RefCell<Option<Rc<Cell<bool>>>>,
    ended: Cell<bool>,
    closes: Cell<usize>,
}


fn frame(v: Value) -> Frame {
    v.as_object().expect("a frame is an object").clone()
}


fn ok() -> Frame {
    frame(json!({ "id": null, "ok": true }))
}


impl Daemon {
    fn new(answer: impl FnMut(&Frame) -> Frame + 'static) -> Rc<Daemon> {
        Rc::new(Daemon {
            answer: RefCell::new(Box::new(answer)),
            heard: RefCell::new(vec![]),
            hold_next: Cell::new(false),
            parked: RefCell::new(None),
            ended: Cell::new(false),
            closes: Cell::new(0),
        })
    }
    fn ok() -> Rc<Daemon> {
        Daemon::new(|_| ok())
    }
    fn heard(&self) -> Vec<Value> {
        self.heard.borrow().iter().cloned().map(Value::Object).collect()
    }
    fn land(&self) {
        if let Some(p) = self.parked.borrow().as_ref() {
            p.set(true);
        }
    }
    fn end(&self) {
        self.ended.set(true);
    }
}


#[async_trait(?Send)]
impl DaemonChannel for Daemon {
    async fn send(&self, frame: Frame) -> Result<Frame, ChannelError> {
        self.heard.borrow_mut().push(frame.clone());
        let reply = (self.answer.borrow_mut())(&frame);
        if self.hold_next.replace(false) {
            let landed = Rc::new(Cell::new(false));
            *self.parked.borrow_mut() = Some(landed.clone());
            while !landed.get() {
                tokio::task::yield_now().await;
            }
        }
        Ok(reply)
    }
    fn close(&self) {
        self.closes.set(self.closes.get() + 1);
        self.ended.set(true);
    }
    async fn closed(&self) -> ChannelEnd {
        while !self.ended.get() {
            tokio::task::yield_now().await;
        }
        ChannelEnd { code: 1000, reason: String::new() }
    }
}


/// The one daemon every folder dials: lists every pty it holds to whoever asks, numbers them from pty_1 up.
#[derive(Default)]
struct Shared {
    held: RefCell<IndexMap<String, bool>>,
    heard: RefCell<Vec<Frame>>,
    made: Cell<i64>,
    list_held: Cell<bool>,
}


struct SharedChannel(Rc<Shared>);


impl Shared {
    fn new() -> Rc<Shared> {
        Rc::new(Shared::default())
    }
    fn channel(self: &Rc<Self>) -> Rc<dyn DaemonChannel> {
        Rc::new(SharedChannel(self.clone()))
    }
    fn heard(&self) -> Vec<Value> {
        self.heard.borrow().iter().cloned().map(Value::Object).collect()
    }
    fn restart(&self) {
        self.held.borrow_mut().clear();
        self.made.set(0);
    }
}


#[async_trait(?Send)]
impl DaemonChannel for SharedChannel {
    async fn send(&self, f: Frame) -> Result<Frame, ChannelError> {
        let s = &self.0;
        s.heard.borrow_mut().push(f.clone());
        let op = f.get("op").and_then(Value::as_str).unwrap_or("");
        if op == "pty.create" {
            s.made.set(s.made.get() + 1);
            let pty_id = format!("pty_{}", s.made.get());
            s.held.borrow_mut().insert(pty_id.clone(), false);
            return Ok(frame(json!({ "id": null, "ok": true, "ptyId": pty_id, "pid": 4000 + s.made.get() })));
        }
        if op == "pty.list" {
            let ptys: Vec<Value> = s
                .held
                .borrow()
                .iter()
                .enumerate()
                .map(|(i, (id, exited))| json!({ "id": id, "pid": 4001 + i as i64, "cols": 80, "rows": 24, "exited": exited }))
                .collect();
            while s.list_held.get() {
                tokio::task::yield_now().await;
            }
            return Ok(frame(json!({ "id": null, "ok": true, "ptys": ptys })));
        }
        if let Some(asked) = f.get("ptyId") {
            let asked = match asked {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            if !s.held.borrow().contains_key(&asked) {
                return Ok(frame(json!({ "id": null, "ok": false, "error": format!("no such pty: {asked}") })));
            }
            if op == "pty.kill" {
                s.held.borrow_mut().shift_remove(&asked);
            }
        }
        Ok(ok())
    }
    fn close(&self) {}
    async fn closed(&self) -> ChannelEnd {
        std::future::pending().await
    }
}


/// The context of the channels area, every member the TS reads off its ctx.
struct World {
    clock: FakeClock,
    warns: RefCell<Vec<String>>,
    store: Rc<Store>,
    door: Rc<Door>,
    // transcript index and rows
    starts: RefCell<HashMap<(String, String), String>>,
    cuts: RefCell<HashMap<(String, String), bool>>,
    facts: RefCell<HashMap<(String, String, String), String>>,
    rows: RefCell<Vec<ChannelRow>>,
    threads: RefCell<IndexMap<String, ThreadRecord>>,
    // the kind's module
    home: RefCell<Option<String>>,
    road: RefCell<AccountResult<DaemonReachView>>,
    roads_asked: RefCell<Vec<String>>,
    turn_road: Cell<Option<ScopedRoad>>,
    // the place door
    has_door: Cell<bool>,
    offers: RefCell<HashMap<String, String>>,
    wired: RefCell<String>,
    settings: RefCell<HashMap<String, PlaceSettings>>,
    turn_limits: RefCell<HashMap<String, Option<i64>>>,
    names: RefCell<HashMap<String, String>>,
    joined: RefCell<HashSet<String>>,
    reports: RefCell<HashMap<String, PlaceReport>>,
    reports_asked: RefCell<Vec<String>>,
    place_rows: RefCell<Vec<PlaceView>>,
    links: RefCell<HashMap<String, Rc<Daemon>>>,
    link_listeners: RefCell<Vec<(String, Rc<dyn Fn(Value)>)>>,
    spend: RefCell<Vec<(String, Option<f64>)>>,
    spend_asked: RefCell<Vec<(Vec<String>, i64)>>,
    projects: RefCell<IndexMap<String, ProjectView>>,
    // dials
    dials: RefCell<VecDeque<AccountResult<Rc<Daemon>>>>,
    dialled: RefCell<Vec<(String, String)>>,
    listeners: RefCell<Vec<Rc<dyn Fn(Value)>>>,
    local_daemon: Cell<bool>,
    local_home: RefCell<String>,
    local_road: RefCell<AccountResult<DaemonReachView>>,
    local_roads_asked: Cell<usize>,
    adopted: RefCell<Vec<String>>,
    broker: Cell<bool>,
    roots: RefCell<HashMap<String, String>>,
    // this process
    host_pid: Cell<i64>,
    host_parent_pid: Cell<i64>,
    state_path: Cell<bool>,
    state_folder: RefCell<String>,
    fs_links: RefCell<HashMap<String, String>>,
    link_reads: RefCell<Vec<String>>,
    hostname: RefCell<String>,
    alive: RefCell<HashSet<i64>>,
    groups: RefCell<HashSet<i64>>,
    agent_pids: RefCell<HashMap<String, i64>>,
    agent_bins: RefCell<Vec<String>>,
    moved: Cell<usize>,
    state: StateCell<RuntimeState>,
    deleted: RefCell<Vec<Rc<dyn Fn(&str)>>>,
    live_ids: RefCell<HashSet<String>>,
    forward_moved: RefCell<Option<Rc<ProcScopesArea>>>,
}


fn unanswered() -> AccountError {
    plain("no answer is set for this").into()
}


impl World {
    fn new() -> Rc<World> {
        Rc::new(World {
            clock: FakeClock::new(0),
            warns: RefCell::default(),
            store: Rc::new(Store::default()),
            door: Rc::new(Door::default()),
            starts: RefCell::default(),
            cuts: RefCell::default(),
            facts: RefCell::default(),
            rows: RefCell::default(),
            threads: RefCell::default(),
            home: RefCell::default(),
            road: RefCell::new(Err(unanswered())),
            roads_asked: RefCell::default(),
            turn_road: Cell::new(None),
            has_door: Cell::new(false),
            offers: RefCell::default(),
            wired: RefCell::new("solari".into()),
            settings: RefCell::default(),
            turn_limits: RefCell::default(),
            names: RefCell::default(),
            joined: RefCell::default(),
            reports: RefCell::default(),
            reports_asked: RefCell::default(),
            place_rows: RefCell::default(),
            links: RefCell::default(),
            link_listeners: RefCell::default(),
            spend: RefCell::default(),
            spend_asked: RefCell::default(),
            projects: RefCell::default(),
            dials: RefCell::default(),
            dialled: RefCell::default(),
            listeners: RefCell::default(),
            local_daemon: Cell::new(false),
            local_home: RefCell::new("/home/me".into()),
            local_road: RefCell::new(Err(unanswered())),
            local_roads_asked: Cell::new(0),
            adopted: RefCell::default(),
            broker: Cell::new(false),
            roots: RefCell::default(),
            host_pid: Cell::new(100),
            host_parent_pid: Cell::new(99),
            state_path: Cell::new(true),
            state_folder: RefCell::new("/state".into()),
            fs_links: RefCell::default(),
            link_reads: RefCell::default(),
            hostname: RefCell::new("host".into()),
            alive: RefCell::default(),
            groups: RefCell::default(),
            agent_pids: RefCell::default(),
            agent_bins: RefCell::default(),
            moved: Cell::new(0),
            state: StateCell::new(RuntimeState::default()),
            deleted: RefCell::default(),
            live_ids: RefCell::default(),
            forward_moved: RefCell::default(),
        })
    }
    /// A second host over the same store: what a restart is.
    fn again(self: &Rc<Self>) -> Rc<World> {
        let w = World::new();
        let store = self.store.clone();
        let w = Rc::try_unwrap(w).ok().expect("fresh");
        Rc::new(World { store, ..w })
    }
    fn area(self: &Rc<Self>) -> Rc<ChannelsArea> {
        ChannelsArea::new(self.clone())
    }
    fn warns(&self) -> Value {
        json!(self.warns.borrow().clone())
    }
    fn delete_workspace_event(&self, id: &str) {
        let listeners: Vec<_> = self.deleted.borrow().clone();
        for f in listeners {
            f(id);
        }
    }
    fn advance(&self, ms: i64) {
        self.clock.advance(ms);
    }
}


struct NoPanels;


#[async_trait(?Send)]
impl BoxPanelsCtx for NoPanels {
    fn on_workspace_deleted(&self, _f: Rc<dyn Fn(&str)>) {}
    fn session_rows(&self) -> Vec<StartedRow> {
        vec![]
    }
    fn thread_records(&self) -> Vec<(String, String)> {
        vec![]
    }
    fn link(&self, _place_id: &str) -> Option<Rc<()>> {
        None
    }
    fn name_of(&self, place_id: &str) -> String {
        place_id.to_string()
    }
    async fn daemon_version(&self, _place_id: &str) -> Option<i64> {
        None
    }
    async fn watch_sys(&self, _place_id: &str) -> Result<Release, String> {
        Err("no".into())
    }
    fn scopes_on(&self, _place_id: &str) -> Vec<wsp_wire::wire::daemon::ProcScope> {
        vec![]
    }
    fn thread_env(&self, _entry: &Entry) -> IndexMap<String, String> {
        IndexMap::new()
    }
    fn spawn(&self, f: LocalBoxFuture<'static, ()>) {
        tokio::task::spawn_local(f);
    }
}


#[async_trait(?Send)]
impl ChannelsCtx for World {
    fn index_start(&self, workspace_id: &str, thread_id: &str) -> Option<String> {
        self.starts.borrow().get(&(workspace_id.to_string(), thread_id.to_string())).cloned()
    }
    fn index_cut(&self, workspace_id: &str, thread_id: &str) -> Option<bool> {
        self.cuts.borrow().get(&(workspace_id.to_string(), thread_id.to_string())).copied()
    }
    fn index_fact(&self, workspace_id: &str, resume: &str, fact: &str) -> Option<String> {
        self.facts.borrow().get(&(workspace_id.to_string(), resume.to_string(), fact.to_string())).cloned()
    }
    fn session_rows(&self) -> Vec<ChannelRow> {
        self.rows.borrow().clone()
    }
    fn thread_record(&self, thread_id: &str) -> Option<ThreadRecord> {
        self.threads.borrow().get(thread_id).cloned()
    }
    fn thread_ids(&self) -> Vec<String> {
        self.threads.borrow().keys().cloned().collect()
    }
    fn latest_harness(&self, _thread_id: &str) -> Option<String> {
        None
    }
    fn home_dir(&self, _record: &WorkspaceRecord) -> Option<String> {
        self.home.borrow().clone()
    }
    async fn daemon_road(&self, entry: &Entry) -> AccountResult<DaemonReachView> {
        self.roads_asked.borrow_mut().push(entry.borrow().record.borrow().id.clone());
        self.road.borrow().clone()
    }
    fn turn_road(&self, _kind: WorkspaceKind) -> Option<ScopedRoad> {
        self.turn_road.get()
    }
    fn has_door(&self) -> bool {
        self.has_door.get()
    }
    fn offer_of(&self, place_id: &str) -> Option<String> {
        self.offers.borrow().get(place_id).cloned()
    }
    fn wired(&self) -> String {
        self.wired.borrow().clone()
    }
    fn settings_at(&self, place_id: &str) -> Option<PlaceSettings> {
        self.settings.borrow().get(place_id).cloned()
    }
    fn turn_limit_at(&self, place_id: &str) -> Option<Option<i64>> {
        self.turn_limits.borrow().get(place_id).copied()
    }
    fn name_of(&self, place_id: &str) -> String {
        self.names.borrow().get(place_id).cloned().unwrap_or_else(|| place_id.to_string())
    }
    fn joined(&self, place_id: &str) -> bool {
        self.joined.borrow().contains(place_id)
    }
    async fn report_of(&self, place_id: &str) -> Option<PlaceReport> {
        self.reports_asked.borrow_mut().push(place_id.to_string());
        self.reports.borrow().get(place_id).cloned()
    }
    async fn rows(&self) -> Vec<PlaceView> {
        self.place_rows.borrow().clone()
    }
    fn link_channel(&self, place_id: &str, on_event: Rc<dyn Fn(Value)>) -> Option<Rc<dyn DaemonChannel>> {
        let link = self.links.borrow().get(place_id).cloned()?;
        self.link_listeners.borrow_mut().push((place_id.to_string(), on_event));
        Some(link)
    }
    async fn spend_today(&self, rows: &[PlaceView], now: i64) -> Vec<(String, Option<f64>)> {
        self.spend_asked.borrow_mut().push((rows.iter().map(|r| r.id.clone()).collect(), now));
        self.spend.borrow().clone()
    }
    fn projects_held(&self) -> IndexMap<String, ProjectView> {
        self.projects.borrow().clone()
    }
    async fn open_channel(&self, url: &str, token: &str, on_event: Rc<dyn Fn(Value)>) -> AccountResult<Rc<dyn DaemonChannel>> {
        self.dialled.borrow_mut().push((url.to_string(), token.to_string()));
        self.listeners.borrow_mut().push(on_event);
        match self.dials.borrow_mut().pop_front() {
            Some(Ok(d)) => Ok(d),
            Some(Err(e)) => Err(e),
            None => Err(unanswered()),
        }
    }
    fn local_has_daemon(&self) -> bool {
        self.local_daemon.get()
    }
    fn local_home_dir(&self) -> String {
        self.local_home.borrow().clone()
    }
    async fn local_road(&self) -> AccountResult<DaemonReachView> {
        self.local_roads_asked.set(self.local_roads_asked.get() + 1);
        self.local_road.borrow().clone()
    }
    async fn adopt_pause(&self, entry: &Entry) {
        self.adopted.borrow_mut().push(entry.borrow().record.borrow().id.clone());
    }
    fn broker_ready(&self) -> bool {
        self.broker.get()
    }
    fn root_of(&self, thread_id: &str) -> String {
        self.roots.borrow().get(thread_id).cloned().unwrap_or_else(|| thread_id.to_string())
    }
    fn device_door(&self) -> Rc<dyn DeviceDoor> {
        self.door.clone()
    }
    fn host_pid(&self) -> i64 {
        self.host_pid.get()
    }
    fn host_parent_pid(&self) -> i64 {
        self.host_parent_pid.get()
    }
    fn has_state_path(&self) -> bool {
        self.state_path.get()
    }
    fn state_folder(&self) -> String {
        self.state_folder.borrow().clone()
    }
    fn read_link(&self, path: &str) -> Option<String> {
        self.link_reads.borrow_mut().push(path.to_string());
        self.fs_links.borrow().get(path).cloned()
    }
    fn hostname(&self) -> String {
        self.hostname.borrow().clone()
    }
    fn pid_alive(&self, pid: i64) -> bool {
        self.alive.borrow().contains(&pid)
    }
    fn agent_bins(&self) -> Vec<String> {
        self.agent_bins.borrow().clone()
    }
    fn agent_pid_of(&self, run: &str) -> Option<i64> {
        self.agent_pids.borrow().get(run).copied()
    }
    fn group_exists(&self, pgid: i64) -> bool {
        self.groups.borrow().contains(&pgid)
    }
    fn proc_scopes_moved(&self) {
        self.moved.set(self.moved.get() + 1);
        let forward = self.forward_moved.borrow().clone();
        if let Some(scopes) = forward {
            scopes.proc_scopes_moved();
        }
    }
    fn state(&self) -> StateCell<RuntimeState> {
        self.state.clone()
    }
    fn box_panels_ctx(&self) -> Rc<dyn BoxPanelsCtx> {
        Rc::new(NoPanels)
    }
    async fn store_get(&self, collection: &str, id: &str) -> AccountResult<Option<Value>> {
        self.store.reads.borrow_mut().push(json!([collection, id]));
        if let Some(why) = self.store.fail_get.borrow().clone() {
            return Err(plain(&why).into());
        }
        Ok(self.store.docs.borrow().get(&(collection.to_string(), id.to_string())).cloned())
    }
    async fn store_put(&self, collection: &str, id: &str, value: Value) -> AccountResult<()> {
        self.store.ops.borrow_mut().push(json!(["put", collection, id, value.clone()]));
        while self.store.held.get() {
            tokio::task::yield_now().await;
        }
        if let Some(why) = self.store.fail_puts.borrow_mut().pop_front() {
            return Err(plain(&why).into());
        }
        self.store.seed(collection, id, value);
        Ok(())
    }
    async fn store_delete(&self, collection: &str, id: &str) -> AccountResult<()> {
        self.store.ops.borrow_mut().push(json!(["delete", collection, id]));
        self.store.docs.borrow_mut().shift_remove(&(collection.to_string(), id.to_string()));
        Ok(())
    }
    async fn store_keys(&self, collection: &str) -> AccountResult<Vec<String>> {
        Ok(self.store.docs.borrow().keys().filter(|(c, _)| c == collection).map(|(_, id)| id.clone()).collect())
    }
    fn on_workspace_deleted(&self, f: Rc<dyn Fn(&str)>) {
        self.deleted.borrow_mut().push(f);
    }
    fn live_has(&self, workspace_id: &str) -> bool {
        self.live_ids.borrow().contains(workspace_id)
    }
    async fn thread_launch(&self, _entry: &Entry, _thread_id: &str, _root_thread_id: &str) -> AccountResult<ThreadLaunch> {
        unreachable!("the copy's acts are not read by these tests")
    }
    fn brokered_env(&self, _token: &str, _entry: &Entry) -> IndexMap<String, String> {
        unreachable!()
    }
    fn terminal_sign_in(&self, _harness: &str, _entry: &Entry, _token: &str) -> Option<TerminalSignIn> {
        unreachable!()
    }
    fn terminal_line(&self, _harness: &str, _program: Option<&str>) -> String {
        unreachable!()
    }
    async fn thread_folder(&self, _entry: &Entry) -> String {
        unreachable!()
    }
    fn thread_env(&self, _entry: &Entry) -> IndexMap<String, String> {
        unreachable!()
    }
    async fn persist(&self, _record: &WorkspaceRecord) -> AccountResult<()> {
        unreachable!()
    }
    fn read_checkout_soon(&self, _entry: &Entry) {
        unreachable!()
    }
    async fn delete_workspace(&self, _workspace_id: &str) -> AccountResult<()> {
        unreachable!()
    }
    fn git_origin(&self, _path: &str, _timeout_ms: i64) -> (Option<i32>, String) {
        unreachable!()
    }
    fn now(&self) -> i64 {
        self.clock.now()
    }
    fn schedule(&self, ms: i64, f: Box<dyn FnOnce()>) -> Cancel {
        self.clock.schedule(f, ms)
    }
    fn spawn(&self, f: LocalBoxFuture<'static, ()>) {
        tokio::task::spawn_local(f);
    }
    fn warn(&self, line: &str) {
        self.warns.borrow_mut().push(line.to_string());
    }
}


/// A value the oracle wrote, compared as the JSON string it serializes to, so key order counts.
fn same(actual: impl Into<Value>, expected: &Value) {
    assert_eq!(actual.into().to_string(), expected.to_string());
}


fn val(frame: Frame) -> Value {
    Value::Object(frame)
}


fn strs(list: &[&str]) -> Value {
    json!(list)
}


/// A frame sent down a channel, its reply as a value.
async fn send(c: &Rc<dyn DaemonChannel>, f: Value) -> Value {
    Value::Object(c.send(frame(f)).await.expect("the channel answered"))
}


// ---------------------------------------------------------------------------------------------------------------------
// The tests
// ---------------------------------------------------------------------------------------------------------------------



fn fx(name: &str) -> Value {
    oracle("channels_daemon")[name].clone()
}


/// A machine named `m1` that serves every daemon frame through `answer` and keeps each one it heard.
fn serving(frames: Rc<RefCell<Vec<Value>>>, answer: impl Fn(&Frame) -> Frame + 'static) -> Arc<FakeMachine> {
    let m = FakeMachine::new("m1");
    *m.served.borrow_mut() = Some(Rc::new(move |fr: Frame| {
        frames.borrow_mut().push(Value::Object(fr.clone()));
        let reply = answer(&fr);
        Box::pin(async move { Ok(reply) })
    }));
    m
}


fn entry(rec: &Value, m: &Arc<FakeMachine>) -> Entry {
    live_with(record(rec), m.clone())
}


fn rec(extra: Value) -> Value {
    let mut base = json!({ "id": "ws1", "name": "alpha", "machineId": "m1", "phase": "running", "kind": "cloud", "project": "p1" });
    for (k, v) in extra.as_object().unwrap() {
        base[k] = v.clone();
    }
    base
}


/// The TS's `said`: what an error carries, as the oracle wrote it.
fn err_json(e: &AccountError) -> Value {
    match e {
        AccountError::Host(h) => error_json(h),
        AccountError::Refusal(r) => json!({ "message": r.message, "refusal": true, "code": r.code }),
    }
}


fn frame_outcome(r: AccountResult<Frame>) -> Value {
    match r {
        Ok(f) => json!({ "ok": f }),
        Err(e) => json!({ "error": err_json(&e) }),
    }
}


fn outcome<T>(r: AccountResult<T>, f: impl FnOnce(T) -> Value) -> Value {
    match r {
        Ok(v) => json!({ "ok": f(v) }),
        Err(e) => json!({ "error": err_json(&e) }),
    }
}


fn host_outcome(r: Result<(), HostError>) -> Value {
    match r {
        Ok(()) => json!({ "ok": null }),
        Err(e) => json!({ "error": error_json(&e) }),
    }
}


fn work<T: 'static, Fut>(f: impl FnOnce(Ask) -> Fut + 'static) -> Work<T>
where
    Fut: std::future::Future<Output = AccountResult<T>> + 'static,
{
    Box::new(move |ask| Box::pin(f(ask)))
}


fn reach() -> DaemonReachView {
    serde_json::from_value(json!({ "url": "wss://box/ws1", "expiresAt": 9999999, "daemonToken": "daemon-token" })).unwrap()
}


fn agents_json(a: &WorkspaceAgents) -> Value {
    // wsp-wire's WorkspaceAgents still carries `spawn` (written from a3b870a6e); the TS at 65f6b89c7 has none.
    let v = serde_json::to_value(a).unwrap();
    Value::Object(v.as_object().unwrap().iter().filter(|(k, _)| *k != "spawn").map(|(k, v)| (k.clone(), v.clone())).collect())
}


fn opt(v: Option<String>) -> Value {
    json!(v)
}


fn on_event(into: Rc<RefCell<Vec<Value>>>) -> Rc<dyn Fn(Value)> {
    Rc::new(move |e| into.borrow_mut().push(e))
}


fn quiet() -> Rc<dyn Fn(Value)> {
    Rc::new(|_| {})
}


// ----- The facts of a record -----------------------------------------------------------------------------------------

/// The oracle's world: one workspace's transcript index, four session rows, two thread records.
fn facts_world() -> (Rc<World>, Value) {
    let f = fx("facts");
    let w = World::new();
    for pair in f["index"]["starts"].as_array().unwrap() {
        w.starts.borrow_mut().insert(("ws1".into(), pair[0].as_str().unwrap().into()), pair[1].as_str().unwrap().into());
    }
    for pair in f["index"]["cut"].as_array().unwrap() {
        w.cuts.borrow_mut().insert(("ws1".into(), pair[0].as_str().unwrap().into()), pair[1].as_bool().unwrap());
    }
    for pair in f["index"]["facts"].as_array().unwrap() {
        for (fact, v) in pair[1].as_object().unwrap() {
            w.facts.borrow_mut().insert(("ws1".into(), pair[0].as_str().unwrap().into(), fact.clone()), v.as_str().unwrap().into());
        }
    }
    *w.rows.borrow_mut() = f["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| ChannelRow {
            workspace_id: r["workspaceId"].as_str().unwrap().into(),
            claude_session_id: r["claudeSessionId"].as_str().map(String::from),
            facts: ["cwd", "permissionMode", "model", "effort"]
                .iter()
                .filter_map(|k| r[*k].as_str().map(|v| (k.to_string(), v.to_string())))
                .collect(),
            ..Default::default()
        })
        .collect();
    for (th, t) in f["threads"].as_object().unwrap() {
        w.threads.borrow_mut().insert(
            th.clone(),
            ThreadRecord {
                workspace_id: "ws1".into(),
                permission_mode: t["permissionMode"].as_str().map(String::from),
                ..Default::default()
            },
        );
    }
    (w, f)
}


fn places_world(door: bool) -> Rc<World> {
    let f = fx("places");
    let w = World::new();
    w.has_door.set(door);
    for (p, o) in f["offers"].as_object().unwrap() {
        w.offers.borrow_mut().insert(p.clone(), o.as_str().unwrap().into());
    }
    for (p, s) in f["settings"].as_object().unwrap() {
        w.settings.borrow_mut().insert(p.clone(), serde_json::from_value::<PlaceSettings>(s.clone()).unwrap());
    }
    for (p, l) in f["turnLimits"].as_object().unwrap() {
        match l {
            Value::String(_) => {}
            Value::Null => {
                w.turn_limits.borrow_mut().insert(p.clone(), None);
            }
            n => {
                w.turn_limits.borrow_mut().insert(p.clone(), Some(n.as_i64().unwrap()));
            }
        }
    }
    *w.wired.borrow_mut() = f["wired"].as_str().unwrap().into();
    w
}


fn project_json(id: &str, name: &str) -> Value {
    json!({ "id": id, "name": name, "computer": "here", "path": format!("/home/u/{name}"), "source": { "kind": "folder", "path": format!("/home/u/{name}") }, "remote": "" })
}


/// One withDaemon on a brokered cloud fork, as the oracle drove it: the dial's listener hears the hello, then each op
/// is asked.
async fn hello_case(c: &Value, now: i64) -> Value {
    let w = World::new();
    w.broker.set(true);
    w.has_door.set(true);
    w.turn_road.set(Some(ScopedRoad::Relayed));
    w.clock.advance(now);
    w.roots.borrow_mut().insert("th_new".into(), "th_root".into());
    *w.road.borrow_mut() = Ok(reach());
    if c["threads"] == json!(true) {
        for (th, ws) in [("th_old", "ws1"), ("th_new", "ws1"), ("th_other", "ws2")] {
            w.threads.borrow_mut().insert(th.into(), ThreadRecord { workspace_id: ws.into(), ..Default::default() });
        }
    }
    let area = w.area();
    let ops: Vec<String> = c["ops"].as_array().unwrap().iter().map(|o| o.as_str().unwrap().to_string()).collect();
    let refuse = ops.iter().any(|o| o == "refuse");
    let live_at: Rc<RefCell<Vec<bool>>> = Rc::default();
    let (door, log) = (w.door.clone(), live_at.clone());
    let d = Daemon::new(move |fr| {
        let token = fr.get("token").and_then(Value::as_str).unwrap_or("undefined").to_string();
        log.borrow_mut().push(door.live.borrow().contains_key(&token));
        if refuse && fr["op"] == json!("git.push") {
            frame(json!({ "ok": false, "error": "git.push was refused" }))
        } else {
            ok_reply()
        }
    });
    w.dials.borrow_mut().push_back(Ok(d.clone()));
    let version = c["version"].clone();
    let heard_by = w.clone();
    let asked = ops.clone();
    let got = area
        .with_daemon(
            &entry(&rec(json!({})), &FakeMachine::new("m1")),
            work(move |ask| async move {
                let hello = heard_by.listeners.borrow()[0].clone();
                hello(json!({ "type": "daemon.hello", "version": version }));
                let mut answers = vec![];
                for op in asked.iter().filter(|o| *o != "refuse") {
                    answers.push(frame_outcome(ask(frame(json!({ "op": op, "cwd": "/x" }))).await));
                }
                Ok(Value::Array(answers))
            }),
        )
        .await;
    json!({
        "name": c["name"], "version": c["version"], "ops": c["ops"], "threads": c["threads"],
        "result": outcome(got, |v| v), "heard": d.heard(), "liveAtOp": live_at.borrow().clone(),
        "minted": w.door.minted.borrow().clone(), "revoked": w.door.revoked.borrow().clone(),
        "closes": d.closes.get(), "warns": w.warns(),
    })
}


fn ok_reply() -> Frame {
    frame(json!({ "ok": true }))
}


fn brokered_case(name_part: &str) -> Value {
    fx("with_daemon")["brokered"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"].as_str().unwrap().contains(name_part))
        .cloned()
        .unwrap_or_else(|| panic!("no case {name_part}"))
}


fn now_of(f: &Value) -> i64 {
    f["now"].as_i64().unwrap()
}


fn report_json(version: Option<i64>) -> Option<Value> {
    version.map(|v| json!({ "name": "Hetzner box", "platform": "linux", "arch": "x64", "os": "debian", "shape": { "cpu": 4, "memMb": 8192 }, "login": {}, "runsWorkspaces": true, "engine": "none", "daemonVersion": v, "wsp": [], "agents": [], "dialed": "ssh" }))
}


fn serving_reply(fr: &Frame) -> Frame {
    if fr["op"] == json!("stdio.fail") {
        frame(json!({ "ok": false, "code": "bad", "error": "the computer refused" }))
    } else {
        frame(json!({ "ok": true, "op": fr["op"] }))
    }
}


fn blocked_world(reports: &Value, door: bool) -> Rc<World> {
    let w = World::new();
    w.has_door.set(door);
    w.names.borrow_mut().insert("pl_x".into(), "Hetzner box".into());
    for (p, r) in reports.as_object().unwrap() {
        w.reports.borrow_mut().insert(p.clone(), serde_json::from_value::<PlaceReport>(r.clone()).unwrap());
    }
    w
}
