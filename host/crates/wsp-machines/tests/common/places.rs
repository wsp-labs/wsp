// SPDX-License-Identifier: AGPL-3.0-only
//! The fakes the places-a tests drive: a clock that moves only when told, a link socket and its daemon reach, the
//! records and views areas (slice b's, faked at the traits `setup.rs` and `link.rs` declare), the recording roads and
//! a provisioner whose steps the test holds and answers, after `test/places-setup.test.ts:204-280`.
//!
//! The clock, reach, devices and store here are wsp-core's and wsp-store's real traits.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use futures::FutureExt;
use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Map, Value};
use tokio::sync::oneshot;

use wsp_core::clock::{Cancel, Clock, Timer as ClockTimer};
use wsp_core::devices::{HeldDevice, MintMore};
use wsp_core::reach::{ReachError, ReachStats};
use wsp_core::types::pending::{DaemonReach, DeviceDoor, Store};
use wsp_engine::machine::{Machine, MachineBackend};
use wsp_engine::provision::{EngineStep, ProvisionOn, ProvisionPlan, ProvisionStage, SetupRun};
use wsp_store::keys::{new_place_key_pair, Seal};
use wsp_wire::types::agents_report::{AgentsSignInEvent, SignInLine};
use wsp_wire::types::recipe_file::{RecipeFile, RecipeFileFoldersValue};
use wsp_wire::views::place::{PlaceProvisionRow, PlaceSetupStep};
use wsp_wire::views::place_more::{PlaceHolds, PlaceRemovedTook, PlaceSettings, PlaceView};
use wsp_wire::views::workspace_more::ThreadScope;
use wsp_wire::wire::daemon::DaemonLinkStatus;
use wsp_wire::wire::devices::{DeviceVia, DeviceView};
use wsp_wire::wire::daemon_events::DaemonEvent;
use wsp_wire::wire::machine_link::BackendFacts;
use wsp_wire::wire::place_link::{PlaceAuthRefusal, PlaceProveRequest, PlaceReport};
use wsp_words::exit::HostError;

use wsp_machines::places::context::{place_door_context, PlaceDoorContext};
use wsp_machines::places::helpers::{Live, LinkSocket, PendingRecord};
use wsp_machines::places::link::{Joining, LinkDoor, LinkRecords, LinkViews};
use wsp_machines::places::setup::{PlaceSetupArea, RecordMove, SetupRecords};
use wsp_machines::places::types::{
    FolderMove, HeldImage, HeldRow, HerePlace, PicksWeight, PlaceChallenge, PlaceDoorOptions, PlaceEvent, PlaceProvisioner, PlaceRecord,
    PlaceRecording, PlaceResult, PlaceUndo, PlaceWiring, ProjectOn, RecipeResolver, RemovedRow, Resolved, RowId, SetupEvent, SetupOn,
    SignInEmit, SignInHandle, SpentToday,
};

// ---------------------------------------------------------------------------------------------------------------
// Running a test: one thread, as the host runs (spec 03 rule 3.1).

/// Runs `f` on a current-thread runtime inside a `LocalSet`, as the host does.
pub fn block<F: Future>(f: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a runtime");
    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, f)
}

/// Lets every local task that can make progress make it.
pub async fn settle() {
    for _ in 0..200 {
        tokio::task::yield_now().await;
    }
}

/// Settles until `ok` holds, or fails the test after `rounds` rounds.
pub async fn until(what: &str, mut ok: impl FnMut() -> bool) {
    for _ in 0..50 {
        if ok() {
            return;
        }
        settle().await;
    }
    panic!("never came true: {what}");
}

/// Standard base64 of a text, as the wire carries bytes.
pub fn base64_of(text: &str) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(text)
}

/// A 64-byte signature standing for a text: the wire reads an ed25519 signature as 64 bytes of base64 and no other.
pub fn signature_of(text: &str) -> String {
    use sha2::Digest;
    base64_of_bytes(&sha2::Sha512::digest(text.as_bytes()))
}

fn base64_of_bytes(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// The text a `putFiles` command writes: the base64 inside its `printf %s '...'` (places-setup.test.ts:1156 `written`).
pub fn written(cmd: &str) -> String {
    use base64::Engine;
    let from = cmd.find("printf %s '").map_or(0, |at| at + "printf %s '".len());
    let b64: String = cmd[from..].chars().take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=')).collect();
    String::from_utf8(base64::engine::general_purpose::STANDARD.decode(b64).unwrap_or_default()).unwrap_or_default()
}

/// A plain error with that message.
pub fn err(message: impl Into<String>) -> HostError {
    HostError::new(message)
}

/// What a lost link answers a frame with.
pub fn reach_err(message: impl Into<String>) -> ReachError {
    ReachError { message: message.into(), kind: None, status: None, code: None }
}

/// The engine's `PlaceAbsent` as the areas read it once it is a `HostError`: kind `absent`.
pub fn absent(message: impl Into<String>) -> HostError {
    HostError::with_kind(message, wsp_words::exit::Kind::Other("absent".into()))
}

// ---------------------------------------------------------------------------------------------------------------
// The clock.

struct Timer {
    id: u64,
    at: i64,
    run: Option<Box<dyn FnOnce()>>,
}

/// A clock that moves only when the test moves it; a timer fires when the clock passes its time, in time order.
pub struct TestClock {
    now: Cell<i64>,
    timers: Rc<RefCell<Vec<Timer>>>,
    seq: Cell<u64>,
}

/// 2026-10-25T00:00:00.000Z.
pub const T0: i64 = 1_793_404_800_000;

impl TestClock {
    pub fn new() -> Rc<TestClock> {
        Rc::new(TestClock { now: Cell::new(T0), timers: Rc::new(RefCell::new(Vec::new())), seq: Cell::new(0) })
    }

    /// Timers not yet fired or cancelled.
    pub fn pending(&self) -> usize {
        self.timers.borrow().iter().filter(|t| t.run.is_some()).count()
    }

    /// Moves the clock `ms` on, firing each timer due on the way at its own time and settling after each.
    pub async fn pass(&self, ms: i64) {
        let end = self.now.get() + ms;
        loop {
            let next = {
                let timers = self.timers.borrow();
                timers.iter().filter(|t| t.run.is_some() && t.at <= end).min_by_key(|t| (t.at, t.id)).map(|t| t.id)
            };
            let Some(id) = next else { break };
            let run = {
                let mut timers = self.timers.borrow_mut();
                let t = timers.iter_mut().find(|t| t.id == id).expect("the timer");
                self.now.set(self.now.get().max(t.at));
                t.run.take()
            };
            if let Some(run) = run {
                run();
            }
            settle().await;
        }
        self.now.set(end);
        settle().await;
    }
}

impl Clock for TestClock {
    fn now(&self) -> i64 {
        self.now.get()
    }

    fn schedule(&self, run: ClockTimer, ms: i64) -> Cancel {
        let id = self.seq.get() + 1;
        self.seq.set(id);
        self.timers.borrow_mut().push(Timer { id, at: self.now.get() + ms, run: Some(run) });
        let timers = Rc::clone(&self.timers);
        Rc::new(move || {
            if let Some(t) = timers.borrow_mut().iter_mut().find(|t| t.id == id) {
                t.run = None;
            }
        })
    }
}

/// An ISO stamp of a clock reading, as `new Date(ms).toISOString()` writes it.
pub fn iso(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let milli = ms.rem_euclid(1000);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil(days);
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{milli:03}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

fn civil(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

// ---------------------------------------------------------------------------------------------------------------
// The link: a socket and the daemon reach over it.

/// How the fake daemon answers one frame.
pub enum Reply {
    Ok(Map<String, Value>),
    Err(String),
    /// Never answers until the link closes, then fails as a lost frame does.
    Hang,
    /// Answers when the test sends on the paired sender.
    Later(oneshot::Receiver<Result<Map<String, Value>, String>>),
}

pub type Script = Rc<dyn Fn(&str, &Map<String, Value>) -> Reply>;

/// A script answering every frame `{ ok: true }`, an `exec` with exit 0 and nothing printed.
pub fn answers_ok() -> Script {
    Rc::new(|op, _| if op == "exec" { Reply::Ok(obj(json!({ "ok": true, "exitCode": 0, "stdout": "", "stderr": "", "truncated": false }))) } else { Reply::Ok(obj(json!({ "ok": true }))) })
}

pub fn obj(v: Value) -> Map<String, Value> {
    match v {
        Value::Object(m) => m,
        other => panic!("not an object: {other}"),
    }
}

/// The daemon at the far end of one link: every frame sent, in order, answered by the script.
pub struct FakeReach {
    pub frames: RefCell<Vec<(String, Map<String, Value>)>>,
    pub script: RefCell<Script>,
    pub closed: Cell<bool>,
    lost: RefCell<Vec<oneshot::Sender<()>>>,
}

impl FakeReach {
    pub fn new(script: Script) -> Rc<FakeReach> {
        Rc::new(FakeReach { frames: RefCell::new(Vec::new()), script: RefCell::new(script), closed: Cell::new(false), lost: RefCell::new(Vec::new()) })
    }

    /// How many frames of this op went up.
    pub fn asked(&self, op: &str) -> usize {
        self.frames.borrow().iter().filter(|(o, _)| o == op).count()
    }

    /// Every command an `exec` frame carried, in order.
    pub fn cmds(&self) -> Vec<String> {
        self.frames.borrow().iter().filter(|(o, _)| o == "exec").filter_map(|(_, p)| p.get("cmd").and_then(Value::as_str).map(str::to_string)).collect()
    }

    /// Fails every frame still waiting, as a link that went does.
    pub fn lose(&self) {
        for tx in self.lost.borrow_mut().drain(..) {
            let _ = tx.send(());
        }
    }
}

#[async_trait(?Send)]
impl DaemonReach for FakeReach {
    async fn ready(&self) -> Result<(), ReachError> {
        Ok(())
    }

    async fn request(&self, op: &str, params: Map<String, Value>) -> Result<Map<String, Value>, ReachError> {
        self.frames.borrow_mut().push((op.to_string(), params.clone()));
        if self.closed.get() {
            return Err(reach_err("connection lost"));
        }
        let reply = (self.script.borrow().clone())(op, &params);
        match reply {
            Reply::Ok(m) => Ok(m),
            Reply::Err(e) => Err(reach_err(e)),
            Reply::Hang => {
                let (tx, rx) = oneshot::channel();
                self.lost.borrow_mut().push(tx);
                let _ = rx.await;
                Err(reach_err("connection lost"))
            }
            Reply::Later(rx) => match rx.await {
                Ok(Ok(m)) => Ok(m),
                Ok(Err(e)) => Err(reach_err(e)),
                Err(_) => Err(reach_err("connection lost")),
            },
        }
    }

    fn status(&self) -> Option<DaemonLinkStatus> {
        None
    }

    fn stats(&self) -> ReachStats {
        ReachStats::default()
    }

    fn close(&self) {
        self.closed.set(true);
        self.lose();
    }
}

/// The websocket a place dialled in on.
pub struct FakeSocket {
    pub open: Cell<bool>,
    pub closed_with: RefCell<Option<(u16, String)>>,
    listeners: RefCell<IndexMap<u64, Box<dyn FnOnce(u16, String)>>>,
    seq: Cell<u64>,
    pub reach: Rc<FakeReach>,
    /// The event callback the attach handed `connect`; the test pushes the daemon's events through it.
    pub on_event: RefCell<Option<Rc<dyn Fn(&DaemonEvent)>>>,
}

impl FakeSocket {
    pub fn new(script: Script) -> Rc<FakeSocket> {
        Rc::new(FakeSocket { open: Cell::new(true), closed_with: RefCell::new(None), listeners: RefCell::new(IndexMap::new()), seq: Cell::new(0), reach: FakeReach::new(script), on_event: RefCell::new(None) })
    }

    /// The far side closing the socket: the reach fails what waits, every close listener hears it once.
    pub fn drop_link(&self, code: u16, reason: &str) {
        if !self.open.replace(false) {
            return;
        }
        self.reach.closed.set(true);
        self.reach.lose();
        let listeners: Vec<_> = self.listeners.borrow_mut().drain(..).map(|(_, f)| f).collect();
        for f in listeners {
            f(code, reason.to_string());
        }
    }

    /// The daemon pushing one event up the link.
    pub fn push(&self, e: &DaemonEvent) {
        let f = self.on_event.borrow().clone().expect("the link was attached");
        f(e);
    }

    pub fn listening(&self) -> usize {
        self.listeners.borrow().len()
    }
}

impl LinkSocket for FakeSocket {
    fn is_open(&self) -> bool {
        self.open.get()
    }

    fn close(&self, code: u16, reason: &str) {
        *self.closed_with.borrow_mut() = Some((code, reason.to_string()));
        self.drop_link(code, reason);
    }

    fn on_close(&self, f: Box<dyn FnOnce(u16, String)>) -> u64 {
        let id = self.seq.get() + 1;
        self.seq.set(id);
        self.listeners.borrow_mut().insert(id, f);
        id
    }

    fn off_close(&self, id: u64) {
        self.listeners.borrow_mut().shift_remove(&id);
    }

    fn connect(self: Rc<Self>, _seal: Option<Seal>, on_event: Rc<dyn Fn(&DaemonEvent)>) -> Rc<dyn DaemonReach> {
        *self.on_event.borrow_mut() = Some(on_event);
        Rc::clone(&self.reach) as Rc<dyn DaemonReach>
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Records, reports and the store.

/// What a computer reports at its join, as `test/place-join.ts:22` writes it, with `over` spread on top.
pub fn report(name: &str, over: Value) -> PlaceReport {
    let mut base = obj(json!({
        "name": name,
        "platform": "linux",
        "arch": "x64",
        "os": "Ubuntu 24.04",
        "shape": { "cpu": 4, "memMb": 4096 },
        "diskFreeBytes": 892_278_013_952_i64,
        "login": { "HOME": "/home/maya", "USER": "maya", "PATH": "/usr/bin" },
        "runsWorkspaces": true,
        "engine": "none",
        "daemonVersion": 17,
        "agents": [],
        "wsp": ["/home/maya/.npm-global/bin/wsp"],
        "dialed": "http://192.168.1.20:14621",
    }));
    for (k, v) in obj(over) {
        base.insert(k, v);
    }
    serde_json::from_value(Value::Object(base)).expect("a report")
}

/// A joined computer's record, stamped at T0.
pub fn record(id: &str, name: &str, report: PlaceReport) -> PlaceRecord {
    serde_json::from_value(json!({
        "id": id, "name": name, "publicKey": "MCowBQYDK2VwAyEAGb9ECWmEzf6FQbrBZ9w7lshQhqowtrbLDFw4rXAxZuE=",
        "joinedAt": iso(T0), "lastSeenAt": iso(T0), "report": report,
    }))
    .expect("a record")
}

pub fn recipe(v: Value) -> RecipeFile {
    serde_json::from_value(v).expect("a recipe file")
}

pub fn row(v: Value) -> PlaceProvisionRow {
    serde_json::from_value(v).expect("a row")
}

pub fn held(v: Value) -> HeldRow {
    serde_json::from_value(v).expect("a held row")
}

/// The host's store as a memory map, by collection then id.
#[derive(Default)]
pub struct FakeStore {
    pub docs: RefCell<IndexMap<String, IndexMap<String, Value>>>,
}

#[async_trait(?Send)]
impl Store for FakeStore {
    async fn get(&self, collection: &str, id: &str) -> Result<Option<Value>, HostError> {
        Ok(self.docs.borrow().get(collection).and_then(|c| c.get(id)).cloned())
    }
    async fn put(&self, collection: &str, id: &str, value: Value) -> Result<(), HostError> {
        self.docs.borrow_mut().entry(collection.to_string()).or_default().insert(id.to_string(), value);
        Ok(())
    }
    async fn list(&self, collection: &str) -> Result<Vec<Value>, HostError> {
        Ok(self.docs.borrow().get(collection).map(|c| c.values().cloned().collect()).unwrap_or_default())
    }
    async fn keys(&self, collection: &str) -> Result<Vec<String>, HostError> {
        Ok(self.docs.borrow().get(collection).map(|c| c.keys().cloned().collect()).unwrap_or_default())
    }
    async fn delete(&self, collection: &str, id: &str) -> Result<(), HostError> {
        if let Some(c) = self.docs.borrow_mut().get_mut(collection) {
            c.shift_remove(id);
        }
        Ok(())
    }
    async fn get_blob(&self, _collection: &str, _id: &str) -> Result<Option<Vec<u8>>, HostError> {
        Ok(None)
    }
    async fn put_blob(&self, _collection: &str, _id: &str, _bytes: Vec<u8>) -> Result<(), HostError> {
        Ok(())
    }
    async fn delete_blob(&self, _collection: &str, _id: &str) -> Result<(), HostError> {
        Ok(())
    }
    async fn stat_blob(&self, _collection: &str, _id: &str) -> Result<Option<wsp_core::types::pending::BlobMark>, HostError> {
        Ok(None)
    }
}

/// The paired devices: nothing places-a reads.
pub struct FakeDevices;

#[async_trait(?Send)]
impl DeviceDoor for FakeDevices {
    async fn issue(&self, _now: i64, _ttl_ms: u64, _here: bool) -> Result<wsp_core::types::pending::IssuedCode, HostError> {
        unreachable!("not read by places-a")
    }
    async fn redeem(&self, _code: &str, _name: &str, _now: i64) -> Result<Option<wsp_core::types::pending::PairedDevice>, HostError> {
        unreachable!("not read by places-a")
    }
    async fn spend(&self, _code: &str, _now: i64) -> Result<bool, HostError> {
        unreachable!("not read by places-a")
    }
    async fn admit(&self, _name: &str, _now: i64) -> Result<wsp_core::types::pending::PairedDevice, HostError> {
        unreachable!("not read by places-a")
    }
    async fn admit_account(&self, _name: &str, _via: DeviceVia, _now: i64) -> Result<wsp_core::types::pending::PairedDevice, HostError> {
        unreachable!("not read by places-a")
    }
    async fn refuses(&self, _fingerprint: &str) -> Result<bool, HostError> {
        Ok(false)
    }
    async fn mint(&self, _name: &str, _scope: ThreadScope, _now: i64, _more: MintMore) -> Result<wsp_core::types::pending::PairedDevice, HostError> {
        unreachable!("not read by places-a")
    }
    async fn revoke_asides(&self) -> Result<(), HostError> {
        Ok(())
    }
    async fn r#match(&self, _token: &str) -> Result<Option<HeldDevice>, HostError> {
        Ok(None)
    }
    async fn seen(&self, _id: &str, _now: i64) -> Result<Option<HeldDevice>, HostError> {
        Ok(None)
    }
    async fn list(&self) -> Result<Vec<DeviceView>, HostError> {
        Ok(Vec::new())
    }
    async fn revoke(&self, _id: &str) -> Result<bool, HostError> {
        Ok(false)
    }
}

/// The records area as the setup and link areas read it: a memory map kept in step with `ctx.kept`, every change
/// in one turn per place from the record as it stands.
pub struct FakeRecords {
    pub ctx: RefCell<Option<Rc<PlaceDoorContext>>>,
    pub map: RefCell<IndexMap<String, PlaceRecord>>,
    /// Every place a change wrote, in order.
    pub writes: RefCell<Vec<String>>,
    pub settings: Rc<RefCell<IndexMap<String, PlaceSettings>>>,
    pub stored_settings: RefCell<IndexMap<String, PlaceSettings>>,
    pub awaiting: RefCell<Vec<(String, Rc<dyn Fn(&str)>)>>,
    pub held_back: RefCell<Vec<String>>,
    pub mark: RefCell<Option<String>>,
    pub marks: RefCell<Vec<String>>,
    pub wired: RefCell<Option<String>>,
    pub providers: RefCell<Vec<String>>,
    pub provider_backends: RefCell<IndexMap<String, Arc<dyn MachineBackend>>>,
    /// What `challenge` answers; `None` is a daemon older than the seal.
    pub challenge: RefCell<Option<Rc<dyn Fn(&str) -> PlaceChallenge>>>,
    pub seen: RefCell<Vec<(String, u64)>>,
    /// A change made before the next `change` of this place reads the record: what another writer wrote meanwhile.
    pub meanwhile: RefCell<Option<Box<dyn FnOnce(&mut PlaceRecord)>>>,
}

impl FakeRecords {
    pub fn new() -> Rc<FakeRecords> {
        Rc::new(FakeRecords {
            ctx: RefCell::new(None),
            map: RefCell::new(IndexMap::new()),
            writes: RefCell::new(Vec::new()),
            settings: Rc::new(RefCell::new(IndexMap::new())),
            stored_settings: RefCell::new(IndexMap::new()),
            awaiting: RefCell::new(Vec::new()),
            held_back: RefCell::new(Vec::new()),
            mark: RefCell::new(None),
            marks: RefCell::new(Vec::new()),
            wired: RefCell::new(None),
            providers: RefCell::new(Vec::new()),
            provider_backends: RefCell::new(IndexMap::new()),
            challenge: RefCell::new(None),
            seen: RefCell::new(Vec::new()),
            meanwhile: RefCell::new(None),
        })
    }

    /// Puts a record in the store and in `kept`, as a join's `keep` does.
    pub fn keep(&self, r: PlaceRecord) {
        if let Some(ctx) = self.ctx.borrow().as_ref() {
            ctx.kept.borrow_mut().insert(r.id.clone(), r.clone());
        }
        self.map.borrow_mut().insert(r.id.clone(), r);
    }

    pub fn get(&self, id: &str) -> PlaceRecord {
        self.map.borrow().get(id).cloned().unwrap_or_else(|| panic!("no record {id}"))
    }

    pub fn remove(&self, id: &str) {
        self.map.borrow_mut().shift_remove(id);
        if let Some(ctx) = self.ctx.borrow().as_ref() {
            ctx.kept.borrow_mut().shift_remove(id);
        }
    }
}

#[async_trait(?Send)]
impl SetupRecords for FakeRecords {
    async fn record_of(&self, place_id: &str) -> PlaceResult<Option<PlaceRecord>> {
        Ok(self.map.borrow().get(place_id).cloned())
    }

    async fn change(&self, place_id: &str, mv: RecordMove) -> PlaceResult<Option<PlaceRecord>> {
        if let Some(f) = self.meanwhile.borrow_mut().take() {
            if let Some(r) = self.map.borrow_mut().get_mut(place_id) {
                f(r);
            }
        }
        let now = self.map.borrow().get(place_id).cloned();
        let Some(now) = now else { return Ok(None) };
        let Some(next) = mv(&now) else { return Ok(None) };
        self.writes.borrow_mut().push(place_id.to_string());
        self.keep(next.clone());
        Ok(Some(next))
    }

    async fn login_listed(&self, place_id: &str, agent: &str) -> PlaceResult<Option<PlaceRecord>> {
        let file = wsp_machines::places::helpers::shared_login_file(agent);
        self.change(
            place_id,
            Box::new(move |now| {
                let file = file?;
                let listed = now.report.logins.clone()?;
                if listed.contains(&file) {
                    return None;
                }
                let mut next = now.clone();
                let mut logins = listed;
                logins.push(file);
                logins.sort();
                next.report.logins = Some(logins);
                Some(next)
            }),
        )
        .await
    }
}

#[async_trait(?Send)]
impl LinkRecords for FakeRecords {
    async fn records(&self) -> PlaceResult<Vec<PlaceRecord>> {
        let mut all: Vec<PlaceRecord> = self.map.borrow().values().cloned().collect();
        all.sort_by(|a, b| a.joined_at.cmp(&b.joined_at));
        Ok(all)
    }
    fn wired_provider(&self) -> Option<String> {
        self.wired.borrow().clone()
    }
    fn provider_ids(&self) -> Vec<String> {
        self.providers.borrow().clone()
    }
    fn provider_backend(&self, place_id: &str) -> Option<Arc<dyn MachineBackend>> {
        self.provider_backends.borrow().get(place_id).cloned()
    }
    async fn settings_of(&self, place_id: &str) -> PlaceResult<PlaceSettings> {
        Ok(self.stored_settings.borrow().get(place_id).cloned().unwrap_or_else(|| serde_json::from_value(json!({})).expect("no settings")))
    }
    fn settings_held(&self) -> Rc<RefCell<IndexMap<String, PlaceSettings>>> {
        Rc::clone(&self.settings)
    }
    fn awaiting_for(&self, place_id: &str) -> Vec<Rc<dyn Fn(&str)>> {
        self.awaiting.borrow().iter().filter(|(p, _)| p == place_id).map(|(_, f)| Rc::clone(f)).collect()
    }
    fn hold_back(&self, record: &PlaceRecord) {
        self.held_back.borrow_mut().push(record.id.clone());
    }
    async fn default_id(&self) -> PlaceResult<Option<String>> {
        Ok(self.mark.borrow().clone())
    }
    async fn in_turn(&self, write: Box<dyn FnOnce() -> LocalBoxFuture<'static, PlaceResult<()>>>) -> PlaceResult<()> {
        write().await
    }
    async fn mark_default(&self, place_id: &str) -> PlaceResult<()> {
        *self.mark.borrow_mut() = Some(place_id.to_string());
        self.marks.borrow_mut().push(place_id.to_string());
        Ok(())
    }
    async fn mark_held(&self) -> PlaceResult<Option<String>> {
        Ok(self.mark.borrow().clone())
    }
    fn challenge(&self, subject: &str, _their_nonce: &str, _their_ephemeral: Option<&str>) -> Option<PlaceChallenge> {
        self.challenge.borrow().as_ref().map(|f| f(subject))
    }
    fn signed_refusal(&self, place_id: &str, place_nonce: &str, sentence: &str) -> PlaceAuthRefusal {
        serde_json::from_value(json!({ "hostPublicKey": "MCowBQYDK2VwAyEAGb9ECWmEzf6FQbrBZ9w7lshQhqowtrbLDFw4rXAxZuE=", "signature": signature_of(&format!("signed:{place_id}:{place_nonce}:{sentence}")) })).expect("a signed refusal")
    }
    async fn write_seen(&self, place_id: &str, at: u64) -> PlaceResult<()> {
        self.seen.borrow_mut().push((place_id.to_string(), at));
        Ok(())
    }
    async fn with_cap(&self, row: PlaceView, _ids: &[RowId]) -> PlaceResult<PlaceView> {
        Ok(row)
    }
    async fn row_ids(&self) -> PlaceResult<Vec<RowId>> {
        Ok(Vec::new())
    }
}

/// The views area as the link reads it.
pub struct FakeViews {
    pub pending: RefCell<Vec<PendingRecord>>,
    pub flooring: Rc<RefCell<IndexSet<String>>>,
    pub floored: RefCell<Vec<(String, String)>>,
    pub taken_back: RefCell<Vec<String>>,
    pub built: RefCell<Vec<(String, BackendFacts)>>,
    /// The tunnel ids whose bytes the forward owns.
    pub tunnels: RefCell<IndexSet<String>>,
    pub tunnel_events: RefCell<Vec<DaemonEvent>>,
    pub cuts: RefCell<Vec<(String, String)>>,
    pub joining: Rc<RefCell<IndexMap<String, Joining>>>,
    pub joined_calls: RefCell<Vec<String>>,
    pub backend: RefCell<Option<Rc<dyn Fn(&str, &BackendFacts) -> Arc<dyn MachineBackend>>>>,
    /// The context, for `cut` to take the live link out as the views' own `cut` does.
    pub ctx: RefCell<Option<Rc<PlaceDoorContext>>>,
}

impl FakeViews {
    pub fn new() -> Rc<FakeViews> {
        Rc::new(FakeViews {
            pending: RefCell::new(Vec::new()),
            flooring: Rc::new(RefCell::new(IndexSet::new())),
            floored: RefCell::new(Vec::new()),
            taken_back: RefCell::new(Vec::new()),
            built: RefCell::new(Vec::new()),
            tunnels: RefCell::new(IndexSet::new()),
            tunnel_events: RefCell::new(Vec::new()),
            cuts: RefCell::new(Vec::new()),
            joining: Rc::new(RefCell::new(IndexMap::new())),
            joined_calls: RefCell::new(Vec::new()),
            backend: RefCell::new(None),
            ctx: RefCell::new(None),
        })
    }
}

#[async_trait(?Send)]
impl LinkViews for FakeViews {
    async fn pending_records(&self) -> PlaceResult<Vec<PendingRecord>> {
        Ok(self.pending.borrow().clone())
    }
    async fn put_pending(&self, p: PendingRecord) -> PlaceResult<()> {
        let mut all = self.pending.borrow_mut();
        if let Some(at) = all.iter().position(|x| x.view.id == p.view.id) {
            all[at] = p;
        } else {
            all.push(p);
        }
        Ok(())
    }
    fn flooring(&self) -> Rc<RefCell<IndexSet<String>>> {
        Rc::clone(&self.flooring)
    }
    async fn floor_once(&self, pending: PendingRecord, place_id: &str) {
        self.floored.borrow_mut().push((pending.view.id.clone(), place_id.to_string()));
    }
    async fn taken_back(&self, pending: PendingRecord) {
        self.taken_back.borrow_mut().push(pending.view.id.clone());
    }
    fn backend_from(&self, place_id: &str, facts: BackendFacts) -> Arc<dyn MachineBackend> {
        self.built.borrow_mut().push((place_id.to_string(), facts.clone()));
        let made = (self.backend.borrow().clone().expect("a backend road for the test"))(place_id, &facts);
        if let Some(ctx) = self.ctx.borrow().as_ref() {
            ctx.backends.borrow_mut().insert(place_id.to_string(), Arc::clone(&made));
        }
        made
    }
    fn tunnelled(&self, _place_id: &str, e: &DaemonEvent) -> bool {
        let id = match e {
            DaemonEvent::TunnelData { tunnel_id, .. } | DaemonEvent::TunnelEnd { tunnel_id, .. } => tunnel_id.clone(),
            _ => return false,
        };
        if !self.tunnels.borrow().contains(&id) {
            return false;
        }
        self.tunnel_events.borrow_mut().push(e.clone());
        true
    }
    fn cut(&self, place_id: &str, reason: &str) {
        self.cuts.borrow_mut().push((place_id.to_string(), reason.to_string()));
        let ctx = self.ctx.borrow().clone();
        if let Some(ctx) = ctx {
            let held = ctx.live.borrow_mut().shift_remove(place_id);
            if let Some(held) = held {
                (held.seen)();
                held.reach.close();
                held.socket.close(1000, reason);
            }
        }
    }
    fn joining(&self) -> Rc<RefCell<IndexMap<String, Joining>>> {
        Rc::clone(&self.joining)
    }
    async fn joined(&self, place_id: &str, _pending: Joining, req: PlaceProveRequest, _expect: &[u8], _from: &str, _at: u64) -> PlaceResult<wsp_machines::places::types::Proved> {
        self.joined_calls.borrow_mut().push(place_id.to_string());
        Ok(wsp_machines::places::types::Proved::Report { report: req.report, device: None })
    }
    async fn joined_row(&self, record: &PlaceRecord, _marked: &str) -> PlaceResult<PlaceView> {
        Ok(serde_json::from_value(json!({ "id": record.id, "kind": "computer", "name": record.name, "default": false })).unwrap_or_else(|_| panic!("a row for {}", record.id)))
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The recording roads.

/// A sign-in road as the app runs it, each waiting on the test: the page and the code, then the end
/// (`test/places-setup.test.ts:282`).
#[derive(Default)]
pub struct SignIns {
    pub started: RefCell<Vec<String>>,
    pub stopped: Rc<RefCell<Vec<String>>>,
    pub left: Rc<RefCell<Vec<String>>>,
    ends: RefCell<IndexMap<String, SignInEmit>>,
    /// The line's status command, where the catalog has one; false leaves it out.
    pub status: Cell<bool>,
}

impl SignIns {
    pub fn new() -> Rc<SignIns> {
        let s = SignIns::default();
        s.status.set(true);
        Rc::new(s)
    }

    /// The person finishing (or failing) the sign-in of `agent`.
    pub fn end(&self, agent: &str, e: AgentsSignInEvent) {
        let f = self.ends.borrow().get(agent).cloned().unwrap_or_else(|| panic!("no sign-in of {agent} started"));
        f(e);
    }
}

pub fn sign_in_event(mut v: Value) -> AgentsSignInEvent {
    let agent = v.get("agent").and_then(Value::as_str).unwrap_or("").to_string();
    let object = v.as_object_mut().expect("a sign-in event is an object");
    object.entry("type").or_insert(json!("agents.signIn"));
    object.entry("signInId").or_insert(json!(agent));
    serde_json::from_value(v).expect("a sign-in event")
}

/// The recording: each road present or absent as the test wires it.
#[derive(Default)]
pub struct FakeRecording {
    pub sign_ins: RefCell<Option<Rc<SignIns>>>,
    pub mint: RefCell<Option<Rc<SignIns>>>,
    pub login: RefCell<Option<Option<Vec<u8>>>>,
    pub stores: RefCell<Option<IndexMap<String, String>>>,
    pub projects: RefCell<Vec<ProjectOn>>,
    pub add_folder: RefCell<Option<Rc<dyn Fn(&str, &RecipeFileFoldersValue, Option<&FolderMove>) -> PlaceResult<HeldRow>>>>,
    pub added: RefCell<Vec<(String, Option<String>)>>,
    pub remotes: RefCell<IndexMap<String, String>>,
    pub removed_folders: RefCell<Vec<String>>,
    pub looks: RefCell<Vec<String>>,
    pub images: RefCell<Vec<HeldImage>>,
}

#[async_trait(?Send)]
impl PlaceRecording for FakeRecording {
    async fn projects_on(&self, _place_id: &str) -> PlaceResult<Vec<ProjectOn>> {
        Ok(self.projects.borrow().clone())
    }
    async fn holds_on(&self, _place_id: &str, _read: bool) -> PlaceResult<PlaceHolds> {
        unreachable!("not read by places-a")
    }
    async fn drop_on(&self, _place_id: &str) -> PlaceResult<PlaceRemovedTook> {
        unreachable!("not read by places-a")
    }
    async fn forget_on(&self, _place_id: &str) -> PlaceResult<PlaceRemovedTook> {
        unreachable!("not read by places-a")
    }
    async fn running_on(&self, _place_id: &str, _places: &[RowId]) -> PlaceResult<u64> {
        Ok(0)
    }
    async fn machines_on(&self, _place_id: &str) -> Option<PlaceResult<Vec<String>>> {
        None
    }
    async fn forget_records_on(&self, _place_id: &str) -> Option<PlaceResult<()>> {
        None
    }
    async fn sub_holds_on(&self, _place_id: &str) -> Option<PlaceResult<Vec<String>>> {
        None
    }
    async fn release_records_on(&self, _place_id: &str) -> Option<PlaceResult<()>> {
        None
    }
    fn has_images(&self) -> bool {
        true
    }
    async fn images_on(&self, _place_id: &str) -> PlaceResult<Vec<HeldImage>> {
        Ok(self.images.borrow().clone())
    }
    async fn drop_image_on(&self, _place_id: &str, id: &str) -> PlaceResult<()> {
        self.images.borrow_mut().retain(|i| i.id != id);
        Ok(())
    }
    async fn spent_today(&self, _places: &[PlaceView]) -> Option<PlaceResult<Vec<SpentToday>>> {
        None
    }
    async fn sign_in(&self, _place_id: &str, agent: &str, emit: SignInEmit) -> Option<PlaceResult<SignInHandle>> {
        let s = self.sign_ins.borrow().clone()?;
        Some(Ok(start_sign_in(&s, agent, emit)))
    }
    async fn mint_here(&self, agent: &str, emit: SignInEmit) -> Option<PlaceResult<SignInHandle>> {
        let s = self.mint.borrow().clone()?;
        Some(Ok(start_sign_in(&s, agent, emit)))
    }
    async fn sign_in_line(&self, _place_id: &str, agent: &str) -> Option<PlaceResult<SignInLine>> {
        let s = self.sign_ins.borrow().clone()?;
        let status = if s.status.get() { wsp_catalog::catalog::login_sign_in(agent).and_then(|l| l.status().map(|st| st.command.to_string())) } else { None };
        let mut line = json!({ "command": format!("{agent} login") });
        if let Some(status) = status {
            line["status"] = json!(status);
        }
        Some(Ok(serde_json::from_value(line).expect("a sign-in line")))
    }
    fn stores_on(&self, _place_id: &str, _home: &str) -> Option<IndexMap<String, String>> {
        self.stores.borrow().clone()
    }
    async fn login_here(&self, _agent: &str) -> Option<PlaceResult<Option<Vec<u8>>>> {
        self.login.borrow().clone().map(Ok)
    }
    async fn add_folder(&self, _place_id: &str, key: &str, folder: &RecipeFileFoldersValue, mv: Option<&FolderMove>, _stage: Option<Rc<dyn Fn(&str)>>) -> Option<PlaceResult<HeldRow>> {
        let f = self.add_folder.borrow().clone()?;
        self.added.borrow_mut().push((key.to_string(), mv.map(|m| m.id.clone())));
        Some(f(key, folder, mv))
    }
    async fn folder_remote(&self, folder: &RecipeFileFoldersValue) -> Option<PlaceResult<Option<String>>> {
        let from = serde_json::to_value(folder).ok()?.get("from")?.as_str()?.to_string();
        Some(Ok(self.remotes.borrow().get(&from).cloned()))
    }
    async fn remove_folder(&self, _place_id: &str, project_id: &str) -> PlaceResult<()> {
        self.removed_folders.borrow_mut().push(project_id.to_string());
        Ok(())
    }
    async fn folder_look(&self, project_id: &str, _was: &RecipeFileFoldersValue, _now: &RecipeFileFoldersValue) -> PlaceResult<()> {
        self.looks.borrow_mut().push(project_id.to_string());
        Ok(())
    }
}

fn start_sign_in(s: &Rc<SignIns>, agent: &str, emit: SignInEmit) -> SignInHandle {
    s.started.borrow_mut().push(agent.to_string());
    let n = s.started.borrow().len();
    s.ends.borrow_mut().insert(agent.to_string(), Rc::clone(&emit));
    emit(sign_in_event(json!({ "agent": agent, "state": "running" })));
    emit(sign_in_event(json!({ "agent": agent, "state": "waiting", "url": format!("https://auth.example/{agent}/{n}"), "code": format!("CODE-{n}") })));
    let (stopped, left, a1, a2) = (Rc::clone(&s.stopped), Rc::clone(&s.left), agent.to_string(), agent.to_string());
    SignInHandle { leave: Box::new(move || left.borrow_mut().push(a1.clone())), stop: Some(Box::new(move || stopped.borrow_mut().push(a2.clone()))) }
}

// ---------------------------------------------------------------------------------------------------------------
// The provisioner.

/// One step's hold: the step waits at its rows until the test lets it go.
struct Hold {
    tx: RefCell<Option<oneshot::Sender<()>>>,
    rx: RefCell<Option<oneshot::Receiver<()>>>,
}

/// A planner and the engine's steps as the host wires them, each step's rows under the test's hand
/// (`test/places-setup.test.ts:206`): which steps ran, in order, a step held until the test lets it go.
pub struct FakeProvisioner {
    pub plan: RefCell<ProvisionPlan>,
    pub rows: RefCell<IndexMap<EngineStep, Vec<PlaceProvisionRow>>>,
    pub said: RefCell<IndexMap<EngineStep, Vec<PlaceProvisionRow>>>,
    pub throws: RefCell<Option<(EngineStep, HostError)>>,
    /// The undo road failing with this.
    pub undo_fails: RefCell<Option<HostError>>,
    holds: RefCell<IndexMap<EngineStep, Rc<Hold>>>,
    pub ran: RefCell<Vec<EngineStep>>,
    pub ended: RefCell<Vec<EngineStep>>,
    pub picked: RefCell<Vec<RecipeFile>>,
    pub planned_only: RefCell<Vec<Option<IndexSet<PlaceSetupStep>>>>,
    pub plans: RefCell<IndexMap<EngineStep, ProvisionPlan>>,
    pub ons: RefCell<IndexMap<EngineStep, ProvisionOn>>,
    stages: RefCell<IndexMap<EngineStep, ProvisionStage>>,
    pub undone: RefCell<Vec<Vec<String>>>,
    pub undo_plan: RefCell<Option<Rc<dyn Fn(&[RemovedRow]) -> Vec<PlaceUndo>>>>,
    pub project_servers: Cell<bool>,
    pub carried: RefCell<Vec<(String, String, Vec<EngineStep>)>>,
}

/// The plan the fake planner answers: one agent install, then a CLI (`test/places-setup.test.ts:186`).
pub fn plan() -> ProvisionPlan {
    let mut p = ProvisionPlan::default();
    p.recipe_at = "laptop".into();
    p.path = "/root/.local/bin:/usr/local/bin:/usr/bin:/bin".into();
    p.steps = vec![tool("agents/claude", "Claude Code", "script", None), tool("tools/brew/gh", "GitHub CLI", "brew", None)];
    p.agents = 1;
    p
}

pub fn tool(id: &str, label: &str, manager: &str, after: Option<&str>) -> wsp_engine::golden_seam::ToolInstall {
    wsp_engine::golden_seam::ToolInstall { id: id.into(), label: label.into(), manager: manager.into(), cmd: format!("{id}-step"), after: after.map(Into::into), ..Default::default() }
}

impl FakeProvisioner {
    pub fn new() -> Rc<FakeProvisioner> {
        let mut rows = IndexMap::new();
        rows.insert(EngineStep::Agents, vec![row(json!({ "id": "agents/claude", "label": "Claude Code", "outcome": "installed" }))]);
        rows.insert(EngineStep::Clis, vec![row(json!({ "id": "tools/brew/gh", "label": "GitHub CLI", "outcome": "installed" }))]);
        Rc::new(FakeProvisioner {
            plan: RefCell::new(plan()),
            rows: RefCell::new(rows),
            said: RefCell::new(IndexMap::new()),
            throws: RefCell::new(None),
            undo_fails: RefCell::new(None),
            holds: RefCell::new(IndexMap::new()),
            ran: RefCell::new(Vec::new()),
            ended: RefCell::new(Vec::new()),
            picked: RefCell::new(Vec::new()),
            planned_only: RefCell::new(Vec::new()),
            plans: RefCell::new(IndexMap::new()),
            ons: RefCell::new(IndexMap::new()),
            stages: RefCell::new(IndexMap::new()),
            undone: RefCell::new(Vec::new()),
            undo_plan: RefCell::new(None),
            project_servers: Cell::new(false),
            carried: RefCell::new(Vec::new()),
        })
    }

    /// Holds `step` at its rows until [`FakeProvisioner::let_go`].
    pub fn hold(&self, step: EngineStep) {
        let (tx, rx) = oneshot::channel();
        self.holds.borrow_mut().insert(step, Rc::new(Hold { tx: RefCell::new(Some(tx)), rx: RefCell::new(Some(rx)) }));
    }

    pub fn let_go(&self, step: EngineStep) {
        if let Some(h) = self.holds.borrow().get(&step) {
            if let Some(tx) = h.tx.borrow_mut().take() {
                let _ = tx.send(());
            }
        }
    }

    /// A running step saying a row has landed, as the tools loop does while the rest install.
    pub fn say(&self, step: EngineStep, r: &PlaceProvisionRow) {
        let stage = self.stages.borrow().get(&step).cloned().expect("the step is running");
        stage(&format!("{}: {:?}", r.label, r.outcome), None, Some(r));
    }

    pub fn ran_words(&self) -> Vec<&'static str> {
        self.ran.borrow().iter().map(|s| s.word()).collect()
    }
}

#[async_trait(?Send)]
impl PlaceProvisioner for FakeProvisioner {
    async fn setup(&self, picks: &RecipeFile, _on: SetupOn, only: Option<&IndexSet<PlaceSetupStep>>) -> PlaceResult<ProvisionPlan> {
        self.picked.borrow_mut().push(picks.clone());
        self.planned_only.borrow_mut().push(only.cloned());
        Ok(self.plan.borrow().clone())
    }
    async fn floor(&self, _machine: Rc<dyn Machine>, _home: &str, _stage: ProvisionStage) -> PlaceResult<Vec<PlaceProvisionRow>> {
        Ok(Vec::new())
    }
    async fn step(&self, _machine: Rc<dyn Machine>, plan: &ProvisionPlan, step: &EngineStep, _run: &SetupRun, stage: ProvisionStage, on: &ProvisionOn) -> PlaceResult<Vec<PlaceProvisionRow>> {
        let step = *step;
        self.ran.borrow_mut().push(step);
        self.plans.borrow_mut().insert(step, plan.clone());
        self.ons.borrow_mut().insert(step, on.clone());
        self.stages.borrow_mut().insert(step, stage.clone());
        stage(&format!("{} under way", step.word()), None, None);
        for r in self.said.borrow().get(&step).cloned().unwrap_or_default() {
            stage(&format!("{}: said", r.label), None, Some(&r));
        }
        let hold = self.holds.borrow().get(&step).cloned();
        if let Some(h) = hold {
            let rx = h.rx.borrow_mut().take();
            if let Some(rx) = rx {
                let _ = rx.await;
            }
        }
        if let Some((s, e)) = self.throws.borrow().clone() {
            if s == step {
                return Err(e);
            }
        }
        self.ended.borrow_mut().push(step);
        Ok(self.rows.borrow().get(&step).cloned().unwrap_or_default())
    }
    fn has_undo(&self) -> bool {
        true
    }
    async fn undo(&self, _before: &RecipeFile, removed: &[RemovedRow], _home: &str, _stores: Option<&IndexMap<String, String>>) -> PlaceResult<Vec<PlaceUndo>> {
        self.undone.borrow_mut().push(removed.iter().map(|r| format!("{}/{}", r.kind, r.name)).collect());
        if let Some(e) = self.undo_fails.borrow().clone() {
            return Err(e);
        }
        Ok(self.undo_plan.borrow().clone().map(|f| f(removed)).unwrap_or_default())
    }
    fn has_project_servers(&self) -> bool {
        self.project_servers.get()
    }
    async fn project_servers(&self, _machine: Rc<dyn Machine>, _picks: &RecipeFile, key: &str, path: &str, _stage: ProvisionStage, _on: &ProvisionOn, _recipe: Option<&str>) -> PlaceResult<Vec<PlaceProvisionRow>> {
        self.carried.borrow_mut().push((key.to_string(), path.to_string(), self.ended.borrow().clone()));
        Ok(vec![row(json!({ "id": format!("agents/mcp/claude/@{path}/kept"), "label": "Claude Code kept", "outcome": "installed" }))])
    }
    fn has_estimate(&self) -> bool {
        true
    }
    async fn estimate(&self, picks: &RecipeFile) -> PlaceResult<PicksWeight> {
        Ok(PicksWeight { bytes: picks.agents.len() as u64 * 1024 * 1024 * 1024, unmeasured: picks.plugins.len() as u64 })
    }
}

/// A recipe shelf the test moves between syncs (`test/places-setup.test.ts:420`).
pub struct Shelf {
    pub now: RefCell<Resolved>,
    pub resolves: Cell<u32>,
}

#[async_trait(?Send)]
impl RecipeResolver for Shelf {
    async fn resolve(&self, _slug: &str) -> PlaceResult<Resolved> {
        self.resolves.set(self.resolves.get() + 1);
        Ok(self.now.borrow().clone())
    }
    async fn resolve_file(&self, file: &RecipeFile) -> PlaceResult<Resolved> {
        Ok(Resolved { file: file.clone(), items: self.now.borrow().items.clone(), hash: "picks".into() })
    }
}

// ---------------------------------------------------------------------------------------------------------------
// A door's areas, stood up over the fakes.

/// What a scene is built with.
pub struct SceneOpts {
    pub vault: IndexMap<String, String>,
    pub provision: bool,
    pub relink_wait_ms: Option<u64>,
    pub frame_wait_ms: Option<u64>,
    pub shelf: Option<Rc<Shelf>>,
    pub here: HerePlace,
}

impl Default for SceneOpts {
    fn default() -> Self {
        let mut vault = IndexMap::new();
        vault.insert("CLAUDE_CODE_OAUTH_TOKEN".to_string(), "sk-ant-oat01-x".to_string());
        SceneOpts { vault, provision: true, relink_wait_ms: None, frame_wait_ms: None, shelf: None, here: HerePlace { name: "zingzys-mac".into(), ..Default::default() } }
    }
}

/// The context, the fake areas around it and the real setup and link areas over them.
pub struct Scene {
    pub clock: Rc<TestClock>,
    pub store: Rc<FakeStore>,
    pub ctx: Rc<PlaceDoorContext>,
    pub records: Rc<FakeRecords>,
    pub views: Rc<FakeViews>,
    pub recording: Rc<FakeRecording>,
    pub provisioner: Rc<FakeProvisioner>,
    pub setup: Rc<PlaceSetupArea>,
    pub frames: Rc<RefCell<Vec<SetupEvent>>>,
    pub events: Rc<RefCell<Vec<PlaceEvent>>>,
    pub vault: Rc<RefCell<IndexMap<String, String>>>,
}

impl Scene {
    pub fn new(o: SceneOpts) -> Scene {
        let clock = TestClock::new();
        let store = Rc::new(FakeStore::default());
        let records = FakeRecords::new();
        let views = FakeViews::new();
        let recording = Rc::new(FakeRecording::default());
        let provisioner = FakeProvisioner::new();
        let frames: Rc<RefCell<Vec<SetupEvent>>> = Rc::new(RefCell::new(Vec::new()));
        let vault = Rc::new(RefCell::new(o.vault));
        let here = o.here.clone();
        let wiring = PlaceWiring {
            host_key: new_place_key_pair(),
            provider: None,
            here: Rc::new(move || here.clone()),
            host_name: Rc::new(|| "zingzys-mac".to_string()),
            install: None,
            dial: None,
            log: None,
            update: None,
            leave: None,
            forget_key: None,
            sudo_over: None,
            provision: if o.provision { Some(Rc::clone(&provisioner) as Rc<dyn PlaceProvisioner>) } else { None },
            github_token: None,
            undo: None,
            run_over: None,
            back: None,
        };
        let (f2, v2) = (Rc::clone(&frames), Rc::clone(&vault));
        let shelf = o.shelf.clone();
        let opts = PlaceDoorOptions {
            store: Rc::clone(&store) as Rc<dyn Store>,
            devices: Rc::new(FakeDevices),
            wiring,
            recording: Rc::clone(&recording) as Rc<dyn PlaceRecording>,
            providers: None,
            on_daemon_event: None,
            on_stage: None,
            on_setup: Some(Rc::new(move |e: &SetupEvent| f2.borrow_mut().push(e.clone()))),
            copy_build: None,
            nap_ms: None,
            here_daemon: None,
            nap_changed: None,
            join_wait_ms: None,
            update_wait_ms: None,
            dial_wait_ms: None,
            frame_wait_ms: o.frame_wait_ms,
            relink_wait_ms: o.relink_wait_ms,
            seen_every_ms: None,
            vault: Some(Rc::new(move || v2.borrow().clone())),
            recipes: Some(Rc::new(move || shelf.clone().map(|s| s as Rc<dyn RecipeResolver>))),
            owner: None,
            kill_confirm: None,
            clock: Some(Rc::clone(&clock) as Rc<dyn Clock>),
        };
        let ctx = place_door_context(opts);
        *records.ctx.borrow_mut() = Some(Rc::clone(&ctx));
        *views.ctx.borrow_mut() = Some(Rc::clone(&ctx));
        let events: Rc<RefCell<Vec<PlaceEvent>>> = Rc::new(RefCell::new(Vec::new()));
        let e2 = Rc::clone(&events);
        let _ = ctx.watch(Rc::new(move |e: &PlaceEvent| e2.borrow_mut().push(e.clone())));
        let setup = PlaceSetupArea::new(Rc::clone(&ctx), Rc::clone(&records) as Rc<dyn SetupRecords>);
        Scene { clock, store, ctx, records, views, recording, provisioner, setup, frames, events, vault }
    }

    /// The link door over this scene's areas, set on the context as the door (its `forking_backend`, `exec` and
    /// `forward` are what the setup area reads off `ctx.door()`).
    pub fn link(&self) -> Rc<LinkDoor> {
        LinkDoor::new(Rc::clone(&self.ctx), Rc::clone(&self.records) as Rc<dyn LinkRecords>, Rc::clone(&self.setup), Rc::clone(&self.views) as Rc<dyn LinkViews>)
    }

    /// A computer joined and linked: its record kept, a live link on a fresh socket answering by `script`.
    pub fn linked(&self, id: &str, name: &str, report: PlaceReport, script: Script) -> Rc<FakeSocket> {
        self.records.keep(record(id, name, report));
        self.live(id, script)
    }

    /// A fresh live link for a place already kept, put straight into `ctx.live` (what an attach leaves).
    pub fn live(&self, id: &str, script: Script) -> Rc<FakeSocket> {
        let socket = FakeSocket::new(script);
        let reach = Rc::clone(&socket.reach) as Rc<dyn DaemonReach>;
        self.ctx.live.borrow_mut().insert(id.to_string(), Live { socket: Rc::clone(&socket) as Rc<dyn LinkSocket>, reach, seen: Box::new(|| {}), forward: None });
        socket
    }

    /// The link going away as an attach's close handler records it: out of `live`, its close stamped.
    pub fn away(&self, id: &str, socket: &FakeSocket) {
        socket.drop_link(1006, "gone");
        self.ctx.live.borrow_mut().shift_remove(id);
        self.setup.closed_at.borrow_mut().insert(id.to_string(), self.clock.now() as u64);
    }

    /// Every `place.setup` frame said so far.
    pub fn setup_frames(&self) -> Vec<wsp_wire::views::place::PlaceSetupEvent> {
        self.frames.borrow().iter().filter_map(|f| if let SetupEvent::Setup(e) = f { Some(e.clone()) } else { None }).collect()
    }

    pub fn sync_frames(&self) -> Vec<wsp_wire::views::place::PlaceSyncEvent> {
        self.frames.borrow().iter().filter_map(|f| if let SetupEvent::Sync(e) = f { Some(e.clone()) } else { None }).collect()
    }

    /// The ends said on the setup stream, in order.
    pub fn ends(&self) -> Vec<String> {
        self.setup_frames().iter().filter_map(|f| f.end.as_ref().map(|e| serde_json::to_value(e).unwrap().as_str().unwrap().to_string())).collect()
    }
}

/// `Rc<VecDeque>` of answers for a queue a fake hands out in order.
pub type Queue<T> = Rc<RefCell<VecDeque<T>>>;

/// A boxed future, for the roads a test hands the area.
pub fn boxed<T: 'static>(f: impl Future<Output = T> + 'static) -> LocalBoxFuture<'static, T> {
    f.boxed_local()
}
