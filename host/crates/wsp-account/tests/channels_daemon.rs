// SPDX-License-Identifier: AGPL-3.0-only
//! channels_daemon (spec/channels_daemon.md): the facts of a record and the roads to a daemon, channels.ts:1-280.
//! Every expected value is the TS's: oracle/channels_daemon.oracle.ts ran `channelsArea` over the same scenarios and
//! wrote tests/fixtures/oracle/channels_daemon.json, inputs beside answers. The sentences it builds (no such project,
//! spend cap, blocked computer, behind daemon, absent computer, no thread) come from @wsp/protocol through that run.
//!
//! The TS cases that reach this file through a whole host, and the test that pins what each one pins here:
//!   channels.test.ts:162 d16_only_the_git_host_ops_carry_a_token       :180 d16_the_token_is_revoked_after_a_refusal
//!   channels.test.ts:189 d16_an_old_or_silent_daemon_gets_no_token     :202 d16_no_thread_is_a_usage_refusal
//!   channels.test.ts:211 d15_brokered_table (the broker not ready)     :226 d15_brokered_table (a joined computer)
//!   channels.test.ts:559 d16_thread_on_picks_the_named_thread_of_this_workspace_else_the_newest
//!   spawn-guard.test.ts:1038 d6_agents_held_record_then_place_then_default
//!   usage.test.ts:422 d13_reply_of_refusal_keeps_the_code            bring-back.test.ts:88 d14_with_daemon_three_roads
//!   bring-back.test.ts:192, :230 d13_reply_of_refusal_keeps_the_code     serve-daemon.test.ts:198 d11_no_token_is_not_answering_yet
//!   runtime-index.test.ts:81, :134 and local-workspace.test.ts:1715 d1_started_as_and_cut_before
//!   runtime-index.test.ts:585, thread-keeps-picks.test.ts:82, :118 d2_resumed_fact_newest_row_then_index
//!   permission-relay.test.ts:504, :551 d2_access_of_record_then_fact
//!   places-settings.test.ts:354, :402 d5_turn_limit_three_states      places-settings.test.ts:441 d19_place_guard_refuses_at_the_limit
//!   box-folder.test.ts:564 d20_copy_blocked_skips_a_folder
//!   slate-box-server.test.ts:242 and slate-box-http.test.ts:186 d17_stdio_road_table
//!   pull-request.test.ts:215, :231, :610 d18_on_this_computer_*
#![allow(dead_code, unused_imports)]
mod common;

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use serde_json::{json, Value};

use common::{error_json, live_with, oracle, plain, project, record, run, settle, FakeMachine};
use std::sync::Arc;
use wsp_engine::machine::MachineState;
use wsp_account::account::box_panels::{BoxPanelsCtx, StartedRow};
use wsp_account::account::channels::{ChannelRow, ChannelsArea, ChannelsCtx, TerminalSignIn, ThreadLaunch, BLOCKED_READS, HELLO_WAIT_MS, OP_TOKEN_OPS, ORIGIN_READ_MS};
use wsp_account::account::core_types::{AccountError, AccountResult, Ask, Frame, Entry, Release, Served};
use wsp_core::clock::{Cancel, Clock, FakeClock};
use wsp_core::types::internal::ThreadRecord;
use wsp_account::account::proc_scopes::ProcScopesArea;
use wsp_core::daemon_channel::{ChannelEnd, ChannelError, DaemonChannel};
use wsp_core::devices::{DeviceDoor, HeldDevice, IssuedCode, MintMore, PairedDevice, ScopedRoad};
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_wire::views::place_more::{PlaceSettings, PlaceView};
use wsp_wire::views::workspace::{DaemonReachView, ProjectView};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceAgents};
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
        let device = serde_json::from_value(json!({ "id": id, "name": name, "createdAt": "2026-10-25T00:00:00.000Z", "lastSeenAt": "2026-10-25T00:00:00.000Z" })).unwrap();
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
        Rc::new(Daemon { answer: RefCell::new(Box::new(answer)), heard: RefCell::new(vec![]), hold_next: Cell::new(false), parked: RefCell::new(None), ended: Cell::new(false), closes: Cell::new(0) })
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
            let ptys: Vec<Value> = s.held.borrow().iter().enumerate().map(|(i, (id, exited))| json!({ "id": id, "pid": 4001 + i as i64, "cols": 80, "rows": 24, "exited": exited })).collect();
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
    fn terminal_line(&self, _harness: &str, _program: &str) -> String {
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

use wsp_account::account::core_types::Work;

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
            facts: ["cwd", "permissionMode", "model", "effort"].iter().filter_map(|k| r[*k].as_str().map(|v| (k.to_string(), v.to_string()))).collect(),
            ..Default::default()
        })
        .collect();
    for (th, t) in f["threads"].as_object().unwrap() {
        w.threads.borrow_mut().insert(th.clone(), ThreadRecord { workspace_id: "ws1".into(), permission_mode: t["permissionMode"].as_str().map(String::from), ..Default::default() });
    }
    (w, f)
}

// D1, TS: runtime-index.test.ts:81, :134, local-workspace.test.ts:1715
#[test]
fn d1_started_as_and_cut_before() {
    run(async {
        let (w, f) = facts_world();
        let area = w.area();
        for q in f["startedAs"].as_array().unwrap() {
            assert_eq!(opt(area.started_as(q["ws"].as_str().unwrap(), q["th"].as_str().unwrap())), q["answer"], "startedAs {q}");
        }
        for q in f["cutBefore"].as_array().unwrap() {
            assert_eq!(json!(area.cut_before(q["ws"].as_str().unwrap(), q["th"].as_str().unwrap())), q["answer"], "cutBefore {q}");
        }
    });
}

// D2, TS: runtime-index.test.ts:585, thread-keeps-picks.test.ts:82, :118
#[test]
fn d2_resumed_fact_newest_row_then_index() {
    run(async {
        let (w, f) = facts_world();
        let area = w.area();
        for q in f["resumedFact"].as_array().unwrap() {
            let got = area.resumed_fact(q["ws"].as_str().unwrap(), q["resume"].as_str().unwrap(), q["fact"].as_str().unwrap());
            assert_eq!(opt(got), q["answer"], "resumedFact {q}");
        }
        for q in f["folderOf"].as_array().unwrap() {
            assert_eq!(opt(area.folder_of(q["ws"].as_str().unwrap(), q["resume"].as_str().unwrap())), q["answer"], "folderOf {q}");
        }
    });
}

// D2, TS: permission-relay.test.ts:504, :551
#[test]
fn d2_access_of_record_then_fact() {
    run(async {
        let (w, f) = facts_world();
        let area = w.area();
        for q in f["accessOf"].as_array().unwrap() {
            let got = area.access_of(q["ws"].as_str().unwrap(), q["th"].as_str().unwrap(), q["resume"].as_str());
            assert_eq!(opt(got), q["answer"], "accessOf {q}");
        }
    });
}

// D3
#[test]
fn d3_home_of_is_the_home_the_kind_answers_else_nothing() {
    run(async {
        let f = fx("home");
        let w = World::new();
        let area = w.area();
        let r = record(&rec(json!({})));
        *w.home.borrow_mut() = Some("/home/dev".into());
        assert_eq!(opt(area.home_of(&r)), f["with"]["home"]);
        *w.home.borrow_mut() = None;
        assert_eq!(f["without"], json!({}));
        assert_eq!(area.home_of(&r), None);
    });
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

// D4
#[test]
fn d4_provider_and_place_of_each_kind() {
    run(async {
        let f = fx("places");
        for (door, rows, settings) in [(true, "withDoor", "settingsWithDoor"), (false, "noDoor", "settingsNoDoor")] {
            let w = places_world(door);
            let area = w.area();
            for (case, expected) in f["records"].as_array().unwrap().iter().zip(f[rows].as_array().unwrap()) {
                let r = record(&case["record"]);
                let name = case["name"].as_str().unwrap();
                assert_eq!(opt(area.provider_of(&r)), expected["provider"], "{rows}: provider of {name}");
                assert_eq!(opt(area.place_id_of(&r)), expected["placeId"], "{rows}: place of {name}");
            }
            for q in f[settings].as_array().unwrap() {
                let got = area.settings_at(q["place"].as_str());
                assert_eq!(serde_json::to_value(got).unwrap().to_string(), q["answer"].to_string(), "{settings} {q}");
            }
        }
    });
}

// D5, TS: places-settings.test.ts:354, :402
#[test]
fn d5_turn_limit_three_states() {
    run(async {
        let f = fx("places");
        for (door, rows) in [(true, "withDoor"), (false, "noDoor")] {
            let w = places_world(door);
            let area = w.area();
            for (case, expected) in f["records"].as_array().unwrap().iter().zip(f[rows].as_array().unwrap()) {
                let got = match area.turn_limit_of(&record(&case["record"])) {
                    None => Value::Null,
                    Some(wall) if wall.deadline_ms.is_infinite() => json!({ "deadlineMs": "Infinity" }),
                    Some(wall) => json!({ "deadlineMs": wall.deadline_ms as i64 }),
                };
                assert_eq!(got, expected["turnLimit"], "{rows}: {}", case["name"]);
            }
        }
    });
}

// D6, TS: spawn-guard.test.ts:1038
#[test]
fn d6_agents_held_record_then_place_then_default() {
    run(async {
        let f = fx("places");
        for (door, rows) in [(true, "withDoor"), (false, "noDoor")] {
            let w = places_world(door);
            let area = w.area();
            for (case, expected) in f["records"].as_array().unwrap().iter().zip(f[rows].as_array().unwrap()) {
                let mut json_record = case["record"].clone();
                // The record's own agents as wsp-wire holds them (the cut dropped `spawn`).
                if json_record["agents"].is_object() {
                    json_record["agents"]["spawn"] = json!(true);
                }
                let got = agents_json(&area.agents_held(&record(&json_record)));
                assert_eq!(got.to_string(), expected["agents"].to_string(), "{rows}: {}", case["name"]);
            }
        }
        let w = places_world(true);
        let area = w.area();
        for q in f["spawnAt"].as_array().unwrap() {
            assert_eq!(agents_json(&area.spawn_at(q["place"].as_str())).to_string(), q["answer"].to_string(), "spawnAt {q}");
        }
        assert_eq!(agents_json(&area.spawn_at(None)).to_string(), fx("constants")["AGENTS_ON"].to_string());
    });
}

fn project_json(id: &str, name: &str) -> Value {
    json!({ "id": id, "name": name, "computer": "here", "path": format!("/home/u/{name}"), "source": { "kind": "folder", "path": format!("/home/u/{name}") }, "remote": "" })
}

// D7
#[test]
fn d7_project_held_refuses_in_its_words() {
    run(async {
        let f = fx("projects");
        for (names, expected) in [(vec![], &f["empty"]), (vec!["alpha", "beta"], &f["two"])] {
            let w = World::new();
            for (i, n) in names.iter().enumerate() {
                w.projects.borrow_mut().insert(format!("p{i}"), project(&project_json(&format!("p{i}"), n)));
            }
            let area = w.area();
            let got = match area.project_held("nope") {
                Ok(_) => json!({ "ok": null }),
                Err(e) => json!({ "error": error_json(&e) }),
            };
            assert_eq!(&got, expected);
        }
        let w = World::new();
        let held = project(&project_json("pA", "alpha"));
        w.projects.borrow_mut().insert("pA".into(), held.clone());
        let area = w.area();
        assert_eq!(area.project_held("pA").unwrap(), held);
        // The four facts a row needs, in this order.
        let r = area.ref_of(&held);
        assert_eq!(serde_json::to_value(&r).unwrap().to_string(), json!({ "id": "pA", "name": "alpha", "path": "/home/u/alpha", "computer": "here" }).to_string());
        assert_eq!(f["refKeys"], json!(["id", "name", "path", "computer"]));
    });
}

// D8
#[test]
fn d8_checkout_of_table() {
    run(async {
        let f = fx("checkout");
        let w = World::new();
        for p in f["projects"].as_array().unwrap() {
            w.projects.borrow_mut().insert(p["id"].as_str().unwrap().into(), project(p));
        }
        let area = w.area();
        for c in f["cases"].as_array().unwrap() {
            let got = match area.checkout_of(&record(&c["record"])) {
                Ok(path) => json!({ "ok": path }),
                Err(e) => json!({ "error": error_json(&e) }),
            };
            assert_eq!(got, c["answer"], "{}", c["name"]);
        }
    });
}

// D9
#[test]
fn d9_computer_of_names_the_place_or_the_kind() {
    run(async {
        let f = fx("computer");
        for (door, key) in [(true, "withDoor"), (false, "noDoor")] {
            let w = World::new();
            w.has_door.set(door);
            for (p, n) in f["names"].as_object().unwrap() {
                w.names.borrow_mut().insert(p.clone(), n.as_str().unwrap().into());
            }
            let area = w.area();
            for c in f["cases"].as_array().unwrap() {
                assert_eq!(json!(area.computer_of(&entry(&c["record"], &FakeMachine::new("m1")))), c[key], "{key}: {}", c["name"]);
            }
        }
    });
}

// D10
#[test]
fn d10_served_by_its_computer_is_the_machines_daemon_frame() {
    run(async {
        let f = fx("served");
        let w = World::new();
        let area = w.area();
        let heard: Rc<RefCell<Vec<Value>>> = Rc::default();
        let m = serving(heard.clone(), |fr| frame(json!({ "ok": true, "id": fr["id"] })));
        let served = area.served_by_its_computer(&entry(&rec(json!({})), &m)).expect("a machine with a daemon frame is served");
        let reply = served(frame(json!({ "op": "git.status", "id": 4 }))).await.unwrap();
        assert_eq!(Value::Object(reply).to_string(), f["reply"].to_string());
        assert_eq!(json!(heard.borrow().clone()).to_string(), f["heard"].to_string());
        assert_eq!(f["with"], json!(true));
        assert!(area.served_by_its_computer(&entry(&rec(json!({})), &FakeMachine::new("m1"))).is_none());
        assert_eq!(f["without"], json!(false));
    });
}

// ----- The roads -------------------------------------------------------------------------------------------------------

// D11, TS: serve-daemon.test.ts:198
#[test]
fn d11_no_token_is_not_answering_yet() {
    run(async {
        let f = fx("channel_over");
        let w = World::new();
        let area = w.area();
        let none: DaemonReachView = serde_json::from_value(json!({ "url": "wss://x", "expiresAt": 1 })).unwrap();
        let got = area.channel_over(&none, "alpha", quiet()).await;
        assert_eq!(outcome(got, |_| json!(null)), f["none"]);
        assert_eq!(json!(w.dialled.borrow().clone()), f["dialledAfterNone"], "nothing is dialled without a token");
        // With a token: one dial of the reach's url, and the events go to the caller's listener.
        let d = Daemon::ok();
        w.dials.borrow_mut().push_back(Ok(d.clone()));
        let events: Rc<RefCell<Vec<Value>>> = Rc::default();
        let with_token: DaemonReachView = serde_json::from_value(json!({ "url": "wss://x/ws", "expiresAt": 1, "daemonToken": "tok" })).unwrap();
        let opened = area.channel_over(&with_token, "alpha", on_event(events.clone())).await.expect("the dial answers");
        assert_eq!(Rc::as_ptr(&opened) as *const (), Rc::as_ptr(&d) as *const ());
        let listener = w.listeners.borrow()[0].clone();
        listener(json!({ "type": "daemon.hello", "version": 1 }));
        assert_eq!(json!(w.dialled.borrow().iter().map(|(u, t)| json!({ "url": u, "token": t })).collect::<Vec<_>>()), f["dialled"]);
        assert_eq!(json!(events.borrow().clone()), f["events"]);
    });
}

// D12
#[test]
fn d12_a_failed_dial_adopts_a_pause() {
    run(async {
        let f = fx("own_channel");
        for c in f.as_array().unwrap() {
            let name = c["name"].as_str().unwrap();
            let w = World::new();
            let area = w.area();
            let m = FakeMachine::new("m1");
            *m.state.borrow_mut() = match name {
                n if n.contains("failed state read") => Err("no answer".into()),
                n if n.contains("running machine") => Ok(MachineState::Running),
                n if n.contains("something else") => Ok(MachineState::Starting),
                _ => Ok(MachineState::Paused),
            };
            *w.road.borrow_mut() = match c["roadError"].as_str() {
                Some(why) => Err(plain(why).into()),
                None => Ok(reach()),
            };
            if c["dial"] == json!("ok") {
                w.dials.borrow_mut().push_back(Ok(Daemon::ok()));
            } else {
                w.dials.borrow_mut().push_back(Err(plain("dial refused").into()));
            }
            let e = entry(&rec(json!({ "phase": c["phase"] })), &m);
            let got = outcome(area.own_daemon_channel(&e, quiet()).await, |_| json!("opened"));
            assert_eq!(got, c["result"], "{name}: the answer");
            assert_eq!(json!(m.state_calls.get()), c["stateAsked"], "{name}: how often the machine was asked its state");
            assert_eq!(json!(w.adopted.borrow().clone()), c["adopted"], "{name}: the pause adopted");
            assert_eq!(json!(w.roads_asked.borrow().clone()), c["roadsAsked"], "{name}: the road asked");
        }
    });
}

// D13, TS: usage.test.ts:422, bring-back.test.ts:192, :230
#[test]
fn d13_reply_of_refusal_keeps_the_code() {
    run(async {
        let f = fx("reply_of");
        let w = World::new();
        let area = w.area();
        for c in f.as_array().unwrap() {
            let asked = frame(c["frame"].clone());
            let reply = frame(c["reply"].clone());
            assert_eq!(frame_outcome(area.reply_of(&asked, reply)), c["answer"], "{}", c["reply"]);
        }
    });
}

// D13
#[test]
fn d13_over_channel_closes_on_error() {
    run(async {
        let f = fx("over_channel");
        let w = World::new();
        let area = w.area();
        let cases = f.as_array().unwrap();
        // The work's answer comes back and the channel closes.
        let d = Daemon::new(|_| frame(json!({ "ok": true, "v": 1 })));
        let got = area
            .over_channel(d.clone(), work(|ask| async move { Ok(json!({ "first": frame_outcome(ask(frame(json!({ "op": "git.status" }))).await)["ok"], "second": frame_outcome(ask(frame(json!({ "op": "ping" }))).await)["ok"] })) }))
            .await;
        assert_eq!(outcome(got, |v| v), cases[0]["result"]);
        assert_eq!(json!(d.heard()), cases[0]["heard"]);
        assert_eq!(json!(d.closes.get()), cases[0]["closes"]);
        // A refused reply throws its refusal, and the channel closes.
        let d = Daemon::new(|_| frame(json!({ "ok": false, "code": "busy", "error": "x" })));
        let got = area.over_channel(d.clone(), work(|ask| async move { ask(frame(json!({ "op": "git.push" }))).await.map(Value::Object) })).await;
        assert_eq!(outcome(got, |v| v), cases[1]["result"]);
        assert_eq!(json!(d.heard()), cases[1]["heard"]);
        assert_eq!(json!(d.closes.get()), cases[1]["closes"]);
        // The work's own error closes it too.
        let d = Daemon::ok();
        let got: AccountResult<Value> = area.over_channel(d.clone(), work(|_| async move { Err(plain("the work failed").into()) })).await;
        assert_eq!(outcome(got, |v| v), cases[2]["result"]);
        assert_eq!(json!(d.heard()), cases[2]["heard"]);
        assert_eq!(json!(d.closes.get()), cases[2]["closes"]);
    });
}

// D14, TS: bring-back.test.ts:88
#[test]
fn d14_with_daemon_three_roads() {
    run(async {
        let f = fx("with_daemon");
        // A served machine's frames go up its computer's link, a refusal thrown with its code, and nothing is dialled.
        let w = World::new();
        let area = w.area();
        let heard: Rc<RefCell<Vec<Value>>> = Rc::default();
        let m = serving(heard.clone(), |fr| if fr["op"] == json!("git.diff") { frame(json!({ "ok": false, "code": "no-diff", "error": "nothing to diff" })) } else { frame(json!({ "ok": true, "id": null })) });
        let got = area
            .with_daemon(
                &entry(&rec(json!({})), &m),
                work(|ask| async move {
                    let first = frame_outcome(ask(frame(json!({ "op": "git.status" }))).await)["ok"].clone();
                    let second = frame_outcome(ask(frame(json!({ "op": "git.diff", "cwd": "/x" }))).await);
                    Ok(json!([first, second]))
                }),
            )
            .await;
        assert_eq!(outcome(got, |v| v), f["served"]["result"]);
        assert_eq!(json!(heard.borrow().clone()), f["served"]["heard"]);
        assert_eq!(json!(w.dialled.borrow().clone()), f["served"]["dialled"]);
        assert_eq!(json!(w.roads_asked.borrow().clone()), f["served"]["roadsAsked"]);
        // An unbrokered machine: its daemon dialled at its road's url with its token, and closed after.
        let w = World::new();
        let area = w.area();
        *w.road.borrow_mut() = Ok(reach());
        let d = Daemon::new(|fr| frame(json!({ "ok": true, "op": fr["op"] })));
        w.dials.borrow_mut().push_back(Ok(d.clone()));
        let got = area.with_daemon(&entry(&rec(json!({})), &FakeMachine::new("m1")), work(|ask| async move { ask(frame(json!({ "op": "git.status" }))).await.map(Value::Object) })).await;
        assert_eq!(outcome(got, |v| v), f["unbrokered"]["result"]);
        assert_eq!(json!(d.heard()), f["unbrokered"]["heard"]);
        assert_eq!(json!(w.dialled.borrow().iter().map(|(u, t)| json!({ "url": u, "token": t })).collect::<Vec<_>>()), f["unbrokered"]["dialled"]);
        assert_eq!(json!(d.closes.get()), f["unbrokered"]["closes"]);
        assert_eq!(json!(w.listeners.borrow().len()), f["unbrokered"]["listeners"]);
    });
}

// D15, TS: channels.test.ts:211, :226
#[test]
fn d15_brokered_table() {
    run(async {
        for c in fx("brokered").as_array().unwrap() {
            let w = World::new();
            w.broker.set(c["ready"] == json!(true));
            w.has_door.set(c["door"] != json!(false));
            if c["joined"] == json!(true) {
                w.joined.borrow_mut().insert(c["place"].as_str().unwrap().into());
            }
            let area = w.area();
            let m = if c["served"] == json!(true) { serving(Rc::default(), |_| frame(json!({ "ok": true }))) } else { FakeMachine::new("m1") };
            let mut r = rec(json!({ "kind": c["kind"] }));
            if let Some(p) = c["place"].as_str() {
                r["place"] = json!(p);
            }
            assert_eq!(json!(area.brokered(&entry(&r, &m))), c["expected"], "{}", c["name"]);
        }
    });
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
    fx("with_daemon")["brokered"].as_array().unwrap().iter().find(|c| c["name"].as_str().unwrap().contains(name_part)).cloned().unwrap_or_else(|| panic!("no case {name_part}"))
}

fn now_of(f: &Value) -> i64 {
    f["now"].as_i64().unwrap()
}

// D16, TS: channels.test.ts:162
#[test]
fn d16_only_the_git_host_ops_carry_a_token() {
    run(async {
        let now = now_of(&fx("with_daemon"));
        for part in ["a git push and a git pr", "every op of the set", "a version above the least"] {
            let c = brokered_case(part);
            assert_eq!(hello_case(&c, now).await.to_string(), c.to_string(), "{part}");
        }
    });
}

// D16
#[test]
fn d16_constants_match_the_ts() {
    run(async {
        World::new().area();
    });
    let c = fx("constants");
    assert_eq!(json!(HELLO_WAIT_MS), c["HELLO_WAIT_MS"]);
    assert_eq!(json!(ORIGIN_READ_MS), c["ORIGIN_READ_MS"]);
    assert_eq!(json!(OP_TOKEN_OPS), c["OP_TOKEN_OPS"]);
    assert_eq!(json!(BLOCKED_READS), c["BLOCKED_READS"]);
}

// D16, TS: channels.test.ts:202
#[test]
fn d16_no_thread_is_a_usage_refusal() {
    run(async {
        let now = now_of(&fx("with_daemon"));
        let c = brokered_case("no thread is refused");
        let got = hello_case(&c, now).await;
        assert_eq!(got.to_string(), c.to_string());
        // The refusal in its words, whole: the sentence, its fix and its kind.
        assert_eq!(got["result"]["ok"][0]["error"], json!({
            "message": "alpha has no thread yet, so wsp cannot reach GitHub for it. Start a thread on it first.",
            "fix": "Start a thread on it first.",
            "kind": "usage",
        }));
    });
}

// D16, TS: channels.test.ts:189
#[test]
fn d16_an_old_or_silent_daemon_gets_no_token() {
    run(async {
        let now = now_of(&fx("with_daemon"));
        for part in ["a daemon from before op tokens", "a hello naming no number"] {
            let c = brokered_case(part);
            let got = hello_case(&c, now).await;
            assert_eq!(got.to_string(), c.to_string(), "{part}");
            assert_eq!(got["minted"], json!([]), "{part}: no device is minted");
        }
    });
}

// D16, TS: channels.test.ts:180
#[test]
fn d16_the_token_is_revoked_after_a_refusal() {
    run(async {
        let now = now_of(&fx("with_daemon"));
        let c = brokered_case("a refused op's token");
        let got = hello_case(&c, now).await;
        assert_eq!(got.to_string(), c.to_string());
        assert_eq!(got["liveAtOp"], json!([true]));
        assert_eq!(got["revoked"], json!(["dev_1"]));
    });
}

// D16
#[test]
fn d16_hello_waits_two_seconds_on_the_clock() {
    run(async {
        let f = fx("with_daemon");
        let w = World::new();
        w.broker.set(true);
        w.has_door.set(true);
        *w.road.borrow_mut() = Ok(reach());
        w.threads.borrow_mut().insert("th_new".into(), ThreadRecord { workspace_id: "ws1".into(), ..Default::default() });
        let area = w.area();
        let d = Daemon::ok();
        w.dials.borrow_mut().push_back(Ok(d.clone()));
        let done = Rc::new(Cell::new(false));
        let (flag, a, e) = (done.clone(), area.clone(), entry(&rec(json!({})), &FakeMachine::new("m1")));
        let task = tokio::task::spawn_local(async move {
            let r = a.with_daemon(&e, work(|ask| async move { ask(frame(json!({ "op": "git.push" }))).await.map(Value::Object) })).await;
            flag.set(true);
            r
        });
        settle().await;
        w.advance(1_999);
        settle().await;
        let (heard_at, settled_at) = (d.heard().len(), done.get());
        w.advance(1);
        settle().await;
        task.await.unwrap().expect("the op goes without a token");
        let got = json!({ "heardAt1999": heard_at, "settledAt1999": settled_at, "heardAfter": d.heard(), "minted": w.door.minted.borrow().clone() });
        assert_eq!(got.to_string(), f["hello_wait"].to_string());
    });
}

// D16
#[test]
fn d16_a_failed_revoke_warns_and_the_reply_stands() {
    run(async {
        let f = fx("with_daemon");
        let w = World::new();
        w.broker.set(true);
        w.has_door.set(true);
        w.turn_road.set(Some(ScopedRoad::Relayed));
        w.clock.advance(now_of(&f));
        w.door.fail_revoke.set(true);
        *w.road.borrow_mut() = Ok(reach());
        w.threads.borrow_mut().insert("th_new".into(), ThreadRecord { workspace_id: "ws1".into(), ..Default::default() });
        let area = w.area();
        w.dials.borrow_mut().push_back(Ok(Daemon::new(|_| frame(json!({ "ok": true, "v": 1 })))));
        let (a, e) = (area.clone(), entry(&rec(json!({ "name": "pricing" })), &FakeMachine::new("m1")));
        let task = tokio::task::spawn_local(async move { a.with_daemon(&e, work(|ask| async move { ask(frame(json!({ "op": "git.pr" }))).await.map(Value::Object) })).await });
        settle().await;
        let hello = w.listeners.borrow()[0].clone();
        hello(json!({ "type": "daemon.hello", "version": fx("constants")["PIPE_DAEMON_VERSION"].clone() }));
        let got = task.await.unwrap();
        let out = json!({ "result": outcome(got, |v| v), "warns": w.warns(), "minted": w.door.minted.borrow().clone(), "revoked": w.door.revoked.borrow().clone() });
        assert_eq!(out.to_string(), f["failed_revoke"].to_string());
    });
}

// D16, TS: channels.test.ts:559 (channels.ts:180-185, which the TS area does not expose; the table is what those lines say)
#[test]
fn d16_thread_on_picks_the_named_thread_of_this_workspace_else_the_newest() {
    run(async {
        let w = World::new();
        for (th, ws) in [("th_old", "ws1"), ("th_new", "ws1"), ("th_other", "ws2")] {
            w.threads.borrow_mut().insert(th.into(), ThreadRecord { workspace_id: ws.into(), ..Default::default() });
        }
        let area = w.area();
        assert_eq!(area.thread_on("ws1", Some(&json!("th_old"))), Some("th_old".into()), "a named thread of this workspace wins");
        assert_eq!(area.thread_on("ws1", Some(&json!("th_other"))), Some("th_new".into()), "a thread of another workspace is ignored, the newest here stands");
        assert_eq!(area.thread_on("ws1", Some(&json!("th_gone"))), Some("th_new".into()), "an unknown thread is ignored");
        assert_eq!(area.thread_on("ws1", Some(&json!(7))), Some("th_new".into()), "a name that is no string is ignored");
        assert_eq!(area.thread_on("ws1", None), Some("th_new".into()), "none named: the last thread of the workspace in map order");
        assert_eq!(area.thread_on("ws2", None), Some("th_other".into()));
        assert_eq!(area.thread_on("ws3", Some(&json!("th_old"))), None, "a workspace with no thread has none");
    });
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

// D17, TS: slate-box-server.test.ts:242, slate-box-http.test.ts:186
#[test]
fn d17_stdio_road_table() {
    run(async {
        let f = fx("stdio");
        let consts = fx("constants");
        let default_need = consts["STDIO_DAEMON_VERSION"].as_i64().unwrap();
        let cases = f["cases"].as_array().unwrap();
        // Unserved: the machine's daemon dialled with the listeners as its events.
        {
            let c = &cases[0];
            let w = World::new();
            let area = w.area();
            *w.road.borrow_mut() = Ok(reach());
            let d = Daemon::new(|fr| if fr["op"] == json!("stdio.fail") { frame(json!({ "ok": false, "code": "bad", "error": "the daemon refused" })) } else { frame(json!({ "ok": true, "op": fr["op"] })) });
            w.dials.borrow_mut().push_back(Ok(d.clone()));
            let road = area.stdio_road(&entry(&rec(json!({})), &FakeMachine::new("m1")), default_need).await.expect("the road opens");
            let (a, b): (Rc<RefCell<Vec<Value>>>, Rc<RefCell<Vec<Value>>>) = (Rc::default(), Rc::default());
            let off_a = (road.listen)(on_event(a.clone()));
            (road.listen)(on_event(b.clone()));
            let hear = w.listeners.borrow()[0].clone();
            hear(json!({ "type": "stdio.data", "n": 1 }));
            off_a();
            hear(json!({ "type": "stdio.data", "n": 2 }));
            let sent = outcome((road.send)(frame(json!({ "op": "stdio.open" }))).await, Value::Object);
            let refused = outcome((road.send)(frame(json!({ "op": "stdio.fail" }))).await, Value::Object);
            let ended = Rc::new(Cell::new(false));
            let (chan, flag) = (road.channel.clone(), ended.clone());
            tokio::task::spawn_local(async move {
                chan.closed().await;
                flag.set(true);
            });
            road.channel.close();
            settle().await;
            assert_eq!(sent, c["sent"]);
            assert_eq!(refused, c["refused"]);
            assert_eq!(json!(a.borrow().clone()), c["heardA"], "a listener taken off hears nothing more");
            assert_eq!(json!(b.borrow().clone()), c["heardB"]);
            assert_eq!(json!(w.dialled.borrow().iter().map(|(u, t)| json!({ "url": u, "token": t })).collect::<Vec<_>>()), c["dialled"]);
            assert_eq!(json!(d.heard()), c["heard"]);
            assert_eq!(json!(d.closes.get()), c["closes"]);
            assert_eq!(json!(ended.get()), c["closedSettles"]);
        }
        // Served: the computer's link, its version, its frames.
        for c in &cases[1..] {
            let name = c["name"].as_str().unwrap();
            let w = World::new();
            w.has_door.set(c["door"] != json!(false));
            w.names.borrow_mut().insert("pl_x".into(), "Hetzner box".into());
            if let Some(v) = c["report"]["daemonVersion"].as_i64() {
                w.reports.borrow_mut().insert("pl_x".into(), serde_json::from_value::<PlaceReport>(report_json(Some(v)).unwrap()).unwrap());
            }
            let link = Daemon::new(|fr| serving_reply(fr));
            if c["link"] != json!(false) {
                w.links.borrow_mut().insert("pl_x".into(), link.clone());
            }
            let area = w.area();
            let frames: Rc<RefCell<Vec<Value>>> = Rc::default();
            let m = serving(frames.clone(), serving_reply);
            let mut r = rec(json!({ "kind": "place" }));
            if let Some(p) = c["place"].as_str() {
                r["place"] = json!(p);
            }
            let need = c["need"].as_i64().unwrap_or(default_need);
            let got = match area.stdio_road(&entry(&r, &m), need).await {
                Err(e) => json!({ "error": err_json(&e) }),
                Ok(road) => {
                    let heard: Rc<RefCell<Vec<Value>>> = Rc::default();
                    (road.listen)(on_event(heard.clone()));
                    let hear = w.link_listeners.borrow()[0].1.clone();
                    hear(json!({ "type": "stdio.data", "n": 1 }));
                    let sent = outcome((road.send)(frame(json!({ "op": "stdio.open" }))).await, Value::Object);
                    let refused = outcome((road.send)(frame(json!({ "op": "stdio.fail" }))).await, Value::Object);
                    road.channel.close();
                    json!({ "ok": { "sent": sent, "refused": refused, "heard": heard.borrow().clone(), "frames": frames.borrow().clone(), "linkCloses": link.closes.get(), "linkHeard": link.heard() } })
                }
            };
            let out = json!({
                "name": c["name"], "place": c["place"], "door": c["door"], "report": c["report"], "link": c["link"], "need": c["need"],
                "road": got, "reportsAsked": w.reports_asked.borrow().clone(),
            });
            assert_eq!(out.to_string(), c.to_string(), "{name}");
        }
    });
}

// D18, TS: pull-request.test.ts:215, :231, :610
#[test]
fn d18_on_this_computer_without_a_daemon_is_no_host_cli() {
    run(async {
        let f = fx("on_this_computer");
        let w = World::new();
        let area = w.area();
        let got = area.on_this_computer::<Value>(Box::new(|_, _| Box::pin(async { Ok(json!("never")) }))).await;
        assert_eq!(outcome(got, |v| v), f["none"]);
        assert_eq!(w.local_roads_asked.get(), 0, "no road is asked");
    });
}

// D18, TS: pull-request.test.ts:215, :610
#[test]
fn d18_on_this_computer_dials_its_own_daemon_in_its_home_and_closes_it() {
    run(async {
        let f = fx("on_this_computer");
        let w = World::new();
        w.local_daemon.set(true);
        *w.local_road.borrow_mut() = Ok(reach());
        let area = w.area();
        let d = Daemon::new(|fr| frame(json!({ "ok": true, "op": fr["op"] })));
        w.dials.borrow_mut().push_back(Ok(d.clone()));
        let got = area
            .on_this_computer(Box::new(|ask, home| Box::pin(async move { Ok(json!({ "home": home, "reply": ask(frame(json!({ "op": "gh.pr" }))).await.map(Value::Object)? })) })))
            .await;
        assert_eq!(outcome(got, |v| v), f["done"]["result"]);
        assert_eq!(json!(w.dialled.borrow().iter().map(|(u, t)| json!({ "url": u, "token": t })).collect::<Vec<_>>()), f["done"]["dialled"]);
        assert_eq!(json!(d.heard()), f["done"]["heard"]);
        assert_eq!(json!(d.closes.get()), f["done"]["closes"]);
    });
}

// D18, TS: pull-request.test.ts:231
#[test]
fn d18_on_this_computer_carries_a_refusal_with_its_code_and_a_missing_token() {
    run(async {
        let f = fx("on_this_computer");
        let w = World::new();
        w.local_daemon.set(true);
        *w.local_road.borrow_mut() = Ok(reach());
        let area = w.area();
        let d = Daemon::new(|_| frame(json!({ "ok": false, "code": "no-host-cli", "error": "no gh on this computer" })));
        w.dials.borrow_mut().push_back(Ok(d.clone()));
        let got = area.on_this_computer(Box::new(|ask, _| Box::pin(async move { ask(frame(json!({ "op": "gh.pr" }))).await.map(Value::Object) }))).await;
        assert_eq!(outcome(got, |v| v), f["refused"]["result"]);
        assert_eq!(json!(d.closes.get()), f["refused"]["closes"]);
        // A road with no token is not answering yet, in this computer's name, before any dial.
        let w = World::new();
        w.local_daemon.set(true);
        *w.local_road.borrow_mut() = Ok(serde_json::from_value(json!({ "url": "wss://here", "expiresAt": 1 })).unwrap());
        let area = w.area();
        let got = area.on_this_computer::<Value>(Box::new(|_, _| Box::pin(async { Ok(json!("never")) }))).await;
        assert_eq!(outcome(got, |v| v), f["noToken"]["result"]);
        assert_eq!(json!(w.dialled.borrow().len()), json!(0));
    });
}

// D19, TS: places-settings.test.ts:441
#[test]
fn d19_place_guard_refuses_at_the_limit() {
    run(async {
        let f = fx("guard");
        for c in f["cases"].as_array().unwrap() {
            let name = c["name"].as_str().unwrap();
            let w = World::new();
            w.has_door.set(c["door"] != json!(false));
            w.clock.advance(f["now"].as_i64().unwrap());
            *w.place_rows.borrow_mut() = f["rows"].as_array().unwrap().iter().map(|r| serde_json::from_value::<PlaceView>(r.clone()).unwrap()).collect();
            *w.spend.borrow_mut() = c["spend"].as_array().unwrap().iter().map(|s| (s["place"].as_str().unwrap().to_string(), s["todayUsd"].as_f64())).collect();
            let area = w.area();
            let got = host_outcome(area.place_guard(c["place"].as_str().unwrap()).await);
            assert_eq!(got, c["result"], "{name}");
            let asked: Vec<Value> = w.spend_asked.borrow().iter().map(|(ids, now)| json!({ "ids": ids, "now": now })).collect();
            assert_eq!(json!(asked), c["spendAsked"], "{name}: what the spend was asked over");
        }
    });
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

// D20
#[test]
fn d20_place_refuses_a_blocked_computer() {
    run(async {
        let f = fx("refuses");
        for c in f["cases"].as_array().unwrap() {
            let name = c["name"].as_str().unwrap();
            let w = blocked_world(&c["reports"], c["door"] != json!(false));
            let area = w.area();
            let got = host_outcome(area.place_refuses(c["place"].as_str()).await);
            assert_eq!(got, c["result"], "{name}");
            assert_eq!(json!(w.reports_asked.borrow().clone()), c["reportsAsked"], "{name}: reports asked");
        }
        let w = blocked_world(&f["cases"][0]["reports"], true);
        let area = w.area();
        assert_eq!(area.blocked_line(Some("pl_x")).await, Some(f["sentences"]["blocked"].as_str().unwrap().to_string()));
        assert_eq!(area.blocked_line(None).await, None);
    });
}

// D20, TS: box-folder.test.ts:564
#[test]
fn d20_copy_blocked_skips_a_folder() {
    run(async {
        let f = fx("refuses");
        for c in f["copy"].as_array().unwrap() {
            let name = c["name"].as_str().unwrap();
            let w = blocked_world(&c["reports"], true);
            let area = w.area();
            let mut r = rec(json!({ "kind": c["kind"] }));
            if let Some(p) = c["place"].as_str() {
                r["place"] = json!(p);
            }
            let got = host_outcome(area.copy_blocked(&entry(&r, &FakeMachine::new("m1"))).await);
            assert_eq!(got, c["result"], "{name}");
            assert_eq!(json!(w.reports_asked.borrow().clone()), c["reportsAsked"], "{name}: reports asked");
        }
    });
}
