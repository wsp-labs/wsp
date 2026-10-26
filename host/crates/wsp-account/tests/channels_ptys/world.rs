
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;
use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use serde_json::{json, Value};
use common::{live_with, oracle, plain, record, run, settle, FakeMachine};
use wsp_account::account::box_panels::{BoxPanelsCtx, StartedRow};
use wsp_account::account::channels::{
    ChannelRow, ChannelsArea, ChannelsCtx, TerminalSignIn, ThreadLaunch, COMPUTER_PROCS, COMPUTER_WATCHES, GUEST_EVENTS,
    PORT_ROOTS_RECHECK_MS, PTY_EVENTS, WORKSPACE_FRAMES,
};
use wsp_account::account::core_types::{AccountError, AccountResult, Entry, Frame, Release};
use wsp_account::account::proc_scopes::{KeptRow, LatestRow, ProcScopesArea, ProcScopesCtx, ScopeRecord, ScopeRow, ThreadTerminal};
use wsp_core::clock::{Cancel, Clock, FakeClock};
use wsp_core::daemon_channel::{ChannelEnd, ChannelError, DaemonChannel};
use wsp_core::devices::{DeviceDoor, HeldDevice, IssuedCode, MintMore, PairedDevice, ScopedRoad};
use wsp_core::types::internal::ThreadRecord;
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_wire::views::place_more::{PlaceSettings, PlaceView};
use wsp_wire::views::workspace::{DaemonReachView, ProjectView};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::ThreadScope;
use wsp_wire::wire::daemon::ProcScope;
use wsp_wire::wire::devices::DeviceVia;
use wsp_wire::wire::place_link::PlaceReport;
use wsp_words::exit::HostError;




























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
    recheck: Rc<RefCell<Option<Cancel>>>,
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
            recheck: Rc::new(RefCell::new(None)),
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
    fn roots_recheck(&self) -> Rc<RefCell<Option<Cancel>>> {
        self.recheck.clone()
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
    oracle("channels_ptys")[name].clone()
}


type Chan = Rc<dyn DaemonChannel>;


async fn ids(c: &Chan) -> Value {
    let r = send(c, json!({ "id": null, "op": "pty.list" })).await;
    Value::Array(r["ptys"].as_array().expect("a list").iter().map(|p| p["id"].clone()).collect())
}


async fn created(c: &Chan) -> String {
    send(c, json!({ "id": null, "op": "pty.create" })).await["ptyId"].as_str().expect("a pty").to_string()
}


const PTY_BY_ID: [&str; 6] = ["pty.attach", "pty.write", "pty.resize", "pty.tab", "pty.detach", "pty.kill"];


/// local-workspace.test.ts:445, PTY_BY_ID: every op a pane sends a pty by its id.
fn by_id(id: i64, op: &str, pty_id: Value) -> Value {
    let mut m = serde_json::Map::new();
    m.insert("id".into(), json!(id));
    m.insert("op".into(), json!(op));
    match op {
        "pty.write" => {
            m.insert("data".into(), json!("x"));
        }
        "pty.resize" => {
            m.insert("cols".into(), json!(80));
            m.insert("rows".into(), json!(24));
        }
        _ => {}
    }
    m.insert("ptyId".into(), pty_id);
    Value::Object(m)
}


fn terminals(list: Vec<ThreadTerminal>) -> Value {
    Value::Array(list.iter().map(|t| json!({ "thread": t.thread, "workspace": t.workspace, "pid": t.pid })).collect())
}


fn doc(w: &World, id: &str) -> Value {
    json!({ id: w.store.doc("pty-threads", id) })
}


/// `turnGroups` as the oracle's JSON holds it: a JS object lists integer keys in ascending order, whatever the Map's.
fn groups_of(area: &ChannelsArea) -> Value {
    let mut out = serde_json::Map::new();
    for (ws, groups) in area.turn_groups.borrow().iter() {
        let mut pairs: Vec<(&i64, &String)> = groups.iter().collect();
        pairs.sort();
        let mut g = serde_json::Map::new();
        for (pid, thread) in pairs {
            g.insert(pid.to_string(), json!(thread));
        }
        out.insert(ws.clone(), Value::Object(g));
    }
    Value::Object(out)
}


fn pty_pids(map: &IndexMap<String, i64>) -> Value {
    Value::Array(map.iter().map(|(k, v)| json!([k, v])).collect())
}


fn set_ptys(area: &ChannelsArea, ws: &str, pairs: &[(&str, i64)]) {
    area.port_pids.borrow_mut().insert(ws.to_string(), pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect());
}


fn set_groups(area: &ChannelsArea, ws: &str, pairs: &[(i64, &str)]) {
    area.turn_groups.borrow_mut().insert(ws.to_string(), pairs.iter().map(|(k, v)| (*k, v.to_string())).collect());
}


fn guard_world(f: &Value) -> Rc<World> {
    let w = World::new();
    let host = oracle("channels_ptys")["app_main"]["host"].as_str().unwrap().to_string();
    w.host_pid.set(f["hostPid"].as_i64().unwrap());
    w.host_parent_pid.set(f["hostParentPid"].as_i64().unwrap());
    *w.agent_bins.borrow_mut() = f["agentBins"].as_array().unwrap().iter().map(|b| b.as_str().unwrap().to_string()).collect();
    *w.hostname.borrow_mut() = host.clone();
    let lock = oracle("channels_ptys")["app_main"]["lockPath"].clone();
    let path = format!("/state/{}/{}", lock[0].as_str().unwrap(), lock[1].as_str().unwrap());
    w.fs_links.borrow_mut().insert(path, format!("{host}-{}", f["appPid"]));
    w.alive.borrow_mut().insert(f["appPid"].as_i64().unwrap());
    for (run, pid) in f["agentPids"].as_object().unwrap() {
        w.agent_pids.borrow_mut().insert(run.clone(), pid.as_i64().unwrap());
    }
    *w.rows.borrow_mut() = f["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| ChannelRow {
            workspace_id: "ws".into(),
            status: s["view"]["status"].as_str().unwrap().into(),
            pid: s["pid"].as_i64(),
            run: s["run"].as_str().map(String::from),
            ..Default::default()
        })
        .collect();
    w
}


fn lists_pty7() -> Rc<Daemon> {
    let made = Cell::new(0);
    Daemon::new(move |fr| match fr["op"].as_str().unwrap() {
        "pty.create" => {
            made.set(made.get() + 1);
            frame(json!({ "id": null, "ok": true, "ptyId": format!("pty_{}", made.get()) }))
        }
        "pty.list" => frame(json!({ "id": null, "ok": true, "ptys": [{ "id": "pty_7" }, { "id": "pty_1" }] })),
        _ => ok(),
    })
}


async fn watch_roots(area: &Rc<ChannelsArea>, ws: &str) -> Value {
    let inner = Daemon::new(|_| frame(json!({ "id": null, "ok": true, "ports": [] })));
    let c = area.rooted_ports(ws, &format!("/work/{ws}"), inner.clone());
    send(&c, json!({ "id": null, "op": "ports.watch" })).await;
    json!({ "ws": ws, "frame": inner.heard()[0].clone() })
}


// ---------------------------------------------------------------------------------------------------------------------
// proc-scopes.test.ts:195 and :211, which need both areas: the channels keep whose each shell is, the scopes name it.
// ---------------------------------------------------------------------------------------------------------------------

struct ScopesWorld {
    w: Rc<World>,
    area: RefCell<Option<Rc<ChannelsArea>>>,
    records: HashMap<String, ScopeRecord>,
}


#[async_trait(?Send)]
impl ProcScopesCtx for ScopesWorld {
    fn turn_groups(&self) -> Vec<(String, Vec<(i64, String)>)> {
        let area = self.area.borrow().clone().unwrap();
        let groups =
            area.turn_groups.borrow().iter().map(|(ws, g)| (ws.clone(), g.iter().map(|(p, t)| (*p, t.clone())).collect())).collect();
        groups
    }
    fn group_exists(&self, pgid: i64) -> bool {
        self.w.groups.borrow().contains(&pgid)
    }
    fn sessions(&self) -> Vec<ScopeRow> {
        vec![]
    }
    fn kept_agents(&self) -> Vec<(String, KeptRow)> {
        vec![]
    }
    fn latest_on(&self, _thread_id: &str) -> Option<LatestRow> {
        None
    }
    fn thread_records(&self) -> Vec<(String, String)> {
        vec![]
    }
    fn thread_terminals(&self) -> Vec<ThreadTerminal> {
        self.area.borrow().clone().unwrap().thread_terminals()
    }
    fn live_record(&self, workspace_id: &str) -> Option<ScopeRecord> {
        self.records.get(workspace_id).cloned()
    }
    fn shared_daemon(&self, kind: WorkspaceKind) -> bool {
        kind == WorkspaceKind::Local
    }
    async fn channel_here(&self) -> AccountResult<Rc<dyn DaemonChannel>> {
        Err(plain("this host wired no daemon here").into())
    }
    fn spawn(&self, f: LocalBoxFuture<'static, ()>) {
        tokio::task::spawn_local(f);
    }
}


/// Both areas on one host, as the runtime wires them (proc-scopes.test.ts:177): ws_a is local, ws_box a folder on pl_1.
fn both_areas() -> (Rc<World>, Rc<ChannelsArea>, Rc<ProcScopesArea>) {
    let w = World::new();
    let area = w.area();
    let records = HashMap::from([
        ("ws_a".to_string(), ScopeRecord { kind: WorkspaceKind::Local, place: None, checkout: "/work/ws_a".into() }),
        ("ws_box".to_string(), ScopeRecord { kind: WorkspaceKind::Place, place: Some("pl_1".into()), checkout: "/box/ws_box".into() }),
    ]);
    let ctx = Rc::new(ScopesWorld { w: w.clone(), area: RefCell::new(Some(area.clone())), records });
    let scopes = ProcScopesArea::new(ctx);
    *w.forward_moved.borrow_mut() = Some(scopes.clone());
    (w, area, scopes)
}


/// A daemon whose shells lead the pids it hands out in turn.
fn shells(pids: &[i64]) -> Rc<Daemon> {
    let pids = pids.to_vec();
    let n = Cell::new(0usize);
    Daemon::new(move |fr| {
        if fr["op"] == json!("pty.create") {
            let at = n.get();
            n.set(at + 1);
            return frame(json!({ "id": fr["id"], "ok": true, "ptyId": format!("pty_{}", at + 1), "pid": pids[at] }));
        }
        frame(json!({ "id": fr["id"], "ok": true }))
    })
}


/// proc-scopes.test.ts:179: the pane a folder's threads share.
fn opened(area: &Rc<ChannelsArea>, ws: &str, daemon: Rc<Daemon>) -> Chan {
    area.port_pids.borrow_mut().insert(ws.to_string(), IndexMap::new());
    area.rooted_ports(ws, &format!("/work/{ws}"), area.held_to_owner(ws, area.thread_tagged(ws, daemon)))
}


fn terminals_of(scopes: &[ProcScope], thread: &str) -> Vec<i64> {
    let scopes = serde_json::to_value(scopes).unwrap();
    scopes
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["thread"] == json!(thread))
        .and_then(|s| s["pids"].as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter(|p| p["kind"] == json!("terminal"))
        .map(|p| p["pid"].as_i64().unwrap())
        .collect()
}


fn all_pids(scopes: &[ProcScope]) -> Vec<i64> {
    serde_json::to_value(scopes)
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|s| s["pids"].as_array().cloned().unwrap_or_default())
        .map(|p| p["pid"].as_i64().unwrap())
        .collect()
}
