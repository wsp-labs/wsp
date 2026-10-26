// SPDX-License-Identifier: AGPL-3.0-only
//! The fakes and the door builder slice b's tests share: an in-memory store, devices, a clock driven by hand, the
//! recording, the setup area's reads, the link reads, a daemon reach and a socket, and closure roads for the wiring.
//! The TS tests stand up a whole host over a websocket; these build the four areas of slice b straight on the
//! context, the way `make_place_door` does, with fakes where places-a's areas would be. Expected values come from
//! `tests/fixtures/oracle/places_b.json` (written by `oracle/places_b.oracle.ts`).
#![allow(dead_code, clippy::type_complexity)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Map, Value};
use wsp_core::clock::{Cancel, Clock, Timer};
use wsp_core::devices::{HeldDevice, MintMore};
use wsp_core::reach::{ReachError, ReachStats};
use wsp_core::types::pending::{BlobMark, DaemonReach, DeviceDoor, IssuedCode, PairedDevice, Store};
use wsp_core::types::wiring::{HereDaemon, PlaceBackends};
use wsp_engine::link_backend::MachineLink;
use wsp_engine::machine::MachineBackend;
use wsp_machines::places::context::{place_door_context, PlaceDoorContext};
use wsp_machines::places::helpers::{LinkSocket, Live, PendingRecord};
use wsp_machines::places::manage::{ManageDoor, ManageSetup, SetupPicks, SetupStarted, SyncRead};
use wsp_machines::places::records::{LocalFut, PlaceRecordsArea};
use wsp_machines::places::types::*;
use wsp_machines::places::views::{LinkReads, PlaceViewsArea, ViewsSetup};
use wsp_store::keys::{new_place_key_pair, PlaceKeyPair};
use wsp_wire::types::recipe_file::RecipeFile;
use wsp_wire::views::place::{PlaceSetupEvent, PlaceSync, PlaceSyncEvent};
use wsp_wire::views::place_more::{PlaceHolds, PlaceRemovedTook, PlaceView};
use wsp_wire::wire::place_link::PlaceReport;
use wsp_words::exit::HostError;
use wsp_wire::views::workspace_more::ThreadScope;
use wsp_wire::wire::daemon::DaemonLinkStatus;
use wsp_wire::wire::devices::{DeviceVia, DeviceView};

pub type Fails = Result<(), HostError>;

// ---------------------------------------------------------------- the oracle

/// The fixture the oracle wrote, read once per test.
pub fn oracle() -> Value {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/oracle/places_b.json")).expect("the places b oracle fixture");
    serde_json::from_str(&text).expect("the oracle parses")
}

/// A JSON pointer into the oracle, panicking with the path where nothing stands there.
pub fn at(path: &str) -> Value {
    oracle().pointer(path).cloned().unwrap_or_else(|| panic!("nothing in the oracle at {path}"))
}

/// `DAEMON_VERSION` as the oracle's TS read it.
pub fn daemon_version() -> u32 {
    at("/daemonVersion").as_u64().unwrap() as u32
}

pub const NOW: u64 = 1_700_000_000_000;
pub const PLACE: &str = "p_0000000000000001";
pub const PLACE2: &str = "p_0000000000000002";

/// The oracle's joined computer as a JSON document, with `over`'s keys set on top (a `null` takes a key off).
pub fn record_json(over: Value) -> Value {
    let mut base = at("/record");
    merge(&mut base, over);
    base
}

/// The same, read as a record.
pub fn record(over: Value) -> PlaceRecord {
    serde_json::from_value(record_json(over)).expect("a record")
}

/// The oracle record's report with `over` on top.
pub fn report(over: Value) -> PlaceReport {
    let mut base = at("/record/report");
    merge(&mut base, over);
    serde_json::from_value(base).expect("a report")
}

pub fn merge(base: &mut Value, over: Value) {
    if let (Some(b), Value::Object(o)) = (base.as_object_mut(), over) {
        for (k, v) in o {
            if v.is_null() {
                b.shift_remove(&k);
            } else {
                b.insert(k, v);
            }
        }
    }
}

/// A value as JSON, keys in the order the value holds them, for comparing with the oracle.
pub fn js<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).expect("serializes")
}

/// A failure as the oracle writes one: `{err, kind, fix}`.
pub fn err_json(e: &HostError) -> Value {
    json!({ "err": e.message, "kind": e.kind.as_ref().map(|k| k.as_str().to_string()), "fix": e.fix })
}

/// An answer as the oracle writes one: `{ok: value}` or `{err, kind, fix}`.
pub fn outcome<T: serde::Serialize>(r: &Result<T, HostError>) -> Value {
    match r {
        Ok(v) => json!({ "ok": js(v) }),
        Err(e) => err_json(e),
    }
}

// ---------------------------------------------------------------- store

/// An in-memory store: collections and ids in first-write order (spec 02 rule 2.5), a rewrite keeping its place.
#[derive(Default)]
pub struct MemStore {
    pub docs: RefCell<IndexMap<String, IndexMap<String, Value>>>,
    /// Every write, as `put <collection>/<id>` or `delete <collection>/<id>`, in order.
    pub writes: RefCell<Vec<String>>,
    /// Collections whose next write fails with this sentence.
    pub fail_writes: RefCell<IndexMap<String, String>>,
}

impl MemStore {
    pub fn doc(&self, collection: &str, id: &str) -> Option<Value> {
        self.docs.borrow().get(collection).and_then(|c| c.get(id).cloned())
    }
    pub fn all(&self, collection: &str) -> Vec<Value> {
        self.docs.borrow().get(collection).map(|c| c.values().cloned().collect()).unwrap_or_default()
    }
    pub fn seed(&self, collection: &str, id: &str, v: Value) {
        self.docs.borrow_mut().entry(collection.to_string()).or_default().insert(id.to_string(), v);
    }
    pub fn writes_to(&self, prefix: &str) -> Vec<String> {
        self.writes.borrow().iter().filter(|w| w.contains(prefix)).cloned().collect()
    }
}

#[async_trait(?Send)]
impl Store for MemStore {
    async fn get(&self, collection: &str, id: &str) -> Result<Option<Value>, HostError> {
        Ok(self.doc(collection, id))
    }
    async fn put(&self, collection: &str, id: &str, value: Value) -> Result<(), HostError> {
        if let Some(said) = self.fail_writes.borrow_mut().shift_remove(collection) {
            return Err(HostError::new(said));
        }
        self.writes.borrow_mut().push(format!("put {collection}/{id}"));
        self.seed(collection, id, value);
        Ok(())
    }
    async fn list(&self, collection: &str) -> Result<Vec<Value>, HostError> {
        Ok(self.all(collection))
    }
    async fn keys(&self, collection: &str) -> Result<Vec<String>, HostError> {
        Ok(self.docs.borrow().get(collection).map(|c| c.keys().cloned().collect()).unwrap_or_default())
    }
    async fn delete(&self, collection: &str, id: &str) -> Result<(), HostError> {
        self.writes.borrow_mut().push(format!("delete {collection}/{id}"));
        if let Some(c) = self.docs.borrow_mut().get_mut(collection) {
            c.shift_remove(id);
        }
        Ok(())
    }
    async fn get_blob(&self, _: &str, _: &str) -> Result<Option<Vec<u8>>, HostError> {
        Ok(None)
    }
    async fn put_blob(&self, _: &str, _: &str, _: Vec<u8>) -> Result<(), HostError> {
        Ok(())
    }
    async fn delete_blob(&self, _: &str, _: &str) -> Result<(), HostError> {
        Ok(())
    }
    async fn stat_blob(&self, _: &str, _: &str) -> Result<Option<BlobMark>, HostError> {
        Ok(None)
    }
}

// ---------------------------------------------------------------- devices

/// Codes as the devices door issues them: each good once, spent by `spend`.
#[derive(Default)]
pub struct FakeDevices {
    pub issued: RefCell<Vec<String>>,
    pub spent: RefCell<Vec<String>>,
    pub admitted: RefCell<Vec<String>>,
    /// Codes this door holds, spendable once.
    pub good: RefCell<IndexSet<String>>,
}

#[async_trait(?Send)]
impl DeviceDoor for FakeDevices {
    async fn issue(&self, now: i64, ttl_ms: u64, _here: bool) -> Result<IssuedCode, HostError> {
        let code = format!("CODE{}", self.issued.borrow().len() + 1);
        self.issued.borrow_mut().push(code.clone());
        self.good.borrow_mut().insert(code.clone());
        Ok(IssuedCode { code, expires_at: now + ttl_ms as i64 })
    }
    async fn redeem(&self, _: &str, _: &str, _: i64) -> Result<Option<PairedDevice>, HostError> {
        Ok(None)
    }
    async fn spend(&self, code: &str, _now: i64) -> Result<bool, HostError> {
        self.spent.borrow_mut().push(code.to_string());
        Ok(self.good.borrow_mut().shift_remove(code))
    }
    async fn admit(&self, name: &str, _now: i64) -> Result<PairedDevice, HostError> {
        self.admitted.borrow_mut().push(name.to_string());
        Ok(PairedDevice { device_id: format!("d_{name}"), device_token: format!("token-{name}"), device: device_view(name) })
    }
    async fn admit_account(&self, _name: &str, _via: DeviceVia, _now: i64) -> Result<PairedDevice, HostError> {
        Err(HostError::new("no account here"))
    }
    async fn refuses(&self, _fingerprint: &str) -> Result<bool, HostError> {
        Ok(false)
    }
    async fn mint(&self, _name: &str, _scope: ThreadScope, _now: i64, _more: MintMore) -> Result<PairedDevice, HostError> {
        Err(HostError::new("no scoped tokens here"))
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

// ---------------------------------------------------------------- clock

/// A clock moved by hand: `advance` runs each timer that comes due, in time order, at its own time.
pub struct FakeClock {
    pub now: Cell<i64>,
    pub timers: Rc<RefCell<Vec<(i64, u64, Option<Box<dyn FnOnce()>>)>>>,
    next: Cell<u64>,
}

impl FakeClock {
    pub fn new(now: u64) -> Rc<Self> {
        Rc::new(FakeClock { now: Cell::new(now as i64), timers: Rc::new(RefCell::new(Vec::new())), next: Cell::new(0) })
    }
    pub fn advance(&self, ms: u64) {
        let until = self.now.get() + ms as i64;
        loop {
            let due = {
                let mut timers = self.timers.borrow_mut();
                let pick = timers.iter().enumerate().filter(|(_, t)| t.0 <= until && t.2.is_some()).min_by_key(|(_, t)| (t.0, t.1)).map(|(i, _)| i);
                pick.map(|i| {
                    let at = timers[i].0;
                    (at, timers[i].2.take().unwrap())
                })
            };
            match due {
                Some((at, run)) => {
                    self.now.set(at.max(self.now.get()));
                    run();
                }
                None => break,
            }
        }
        self.now.set(until);
    }
    pub fn pending(&self) -> usize {
        self.timers.borrow().iter().filter(|t| t.2.is_some()).count()
    }
}

impl Clock for FakeClock {
    fn now(&self) -> i64 {
        self.now.get()
    }
    fn schedule(&self, run: Timer, ms: i64) -> Cancel {
        let id = self.next.get();
        self.next.set(id + 1);
        self.timers.borrow_mut().push((self.now.get() + ms, id, Some(run)));
        let timers = self.timers.clone();
        Rc::new(move || {
            if let Some(t) = timers.borrow_mut().iter_mut().find(|t| t.1 == id) {
                t.2 = None;
            }
        })
    }
}

/// Lets every task that can move move, the clock untouched.
pub async fn settle() {
    for _ in 0..50 {
        tokio::task::yield_now().await;
    }
}

/// Moves the clock by `ms` in steps, letting tasks move between them, so a wait on a clock timer inside a running
/// call sees its timer fire.
pub async fn pass(clock: &FakeClock, ms: u64) {
    let step = (ms / 20).max(1);
    let mut gone = 0;
    while gone < ms {
        settle().await;
        let by = step.min(ms - gone);
        clock.advance(by);
        gone += by;
    }
    settle().await;
}

// ---------------------------------------------------------------- the recording

/// The runtime's road as a test sets it: answers per method, and every call written down.
pub struct FakeRecording {
    pub log: RefCell<Vec<String>>,
    pub projects: RefCell<Vec<ProjectOn>>,
    pub holds: RefCell<PlaceHolds>,
    pub dropped: RefCell<Result<PlaceRemovedTook, String>>,
    pub forgotten: RefCell<PlaceRemovedTook>,
    pub running: RefCell<IndexMap<String, u64>>,
    pub machines: RefCell<Option<Vec<String>>>,
    pub sub_holds: RefCell<Option<Vec<String>>>,
    pub has_forget_records: Cell<bool>,
    pub has_release_records: Cell<bool>,
    pub images: RefCell<Option<Vec<HeldImage>>>,
    pub spent: RefCell<Option<Result<Vec<SpentToday>, String>>>,
    pub stores: RefCell<Option<IndexMap<String, String>>>,
}

impl Default for FakeRecording {
    fn default() -> Self {
        FakeRecording {
            log: RefCell::new(Vec::new()),
            projects: RefCell::new(Vec::new()),
            holds: RefCell::new(PlaceHolds { forks: vec![], projects: vec![], unsaved: vec![], away: None, images: None }),
            dropped: RefCell::new(Ok(PlaceRemovedTook { forks: vec![], projects: vec![], away: None, images: None })),
            forgotten: RefCell::new(PlaceRemovedTook { forks: vec![], projects: vec![], away: None, images: None }),
            running: RefCell::new(IndexMap::new()),
            machines: RefCell::new(None),
            sub_holds: RefCell::new(None),
            has_forget_records: Cell::new(false),
            has_release_records: Cell::new(false),
            images: RefCell::new(None),
            spent: RefCell::new(None),
            stores: RefCell::new(None),
        }
    }
}

impl FakeRecording {
    fn note(&self, s: impl Into<String>) {
        self.log.borrow_mut().push(s.into());
    }
    pub fn calls(&self) -> Vec<String> {
        self.log.borrow().clone()
    }
}

#[async_trait(?Send)]
impl PlaceRecording for FakeRecording {
    async fn projects_on(&self, place_id: &str) -> PlaceResult<Vec<ProjectOn>> {
        self.note(format!("projectsOn {place_id}"));
        Ok(self.projects.borrow().clone())
    }
    async fn holds_on(&self, place_id: &str, read: bool) -> PlaceResult<PlaceHolds> {
        self.note(format!("holdsOn {place_id} read={read}"));
        Ok(self.holds.borrow().clone())
    }
    async fn drop_on(&self, place_id: &str) -> PlaceResult<PlaceRemovedTook> {
        self.note(format!("dropOn {place_id}"));
        self.dropped.borrow().clone().map_err(HostError::new)
    }
    async fn forget_on(&self, place_id: &str) -> PlaceResult<PlaceRemovedTook> {
        self.note(format!("forgetOn {place_id}"));
        Ok(self.forgotten.borrow().clone())
    }
    async fn running_on(&self, place_id: &str, _places: &[RowId]) -> PlaceResult<u64> {
        Ok(self.running.borrow().get(place_id).copied().unwrap_or(0))
    }
    async fn machines_on(&self, place_id: &str) -> Option<PlaceResult<Vec<String>>> {
        self.machines.borrow().clone().map(|m| {
            self.note(format!("machinesOn {place_id}"));
            Ok(m)
        })
    }
    async fn forget_records_on(&self, place_id: &str) -> Option<PlaceResult<()>> {
        self.has_forget_records.get().then(|| {
            self.note(format!("forgetRecordsOn {place_id}"));
            Ok(())
        })
    }
    async fn sub_holds_on(&self, _place_id: &str) -> Option<PlaceResult<Vec<String>>> {
        self.sub_holds.borrow().clone().map(Ok)
    }
    async fn release_records_on(&self, place_id: &str) -> Option<PlaceResult<()>> {
        self.has_release_records.get().then(|| {
            self.note(format!("releaseRecordsOn {place_id}"));
            Ok(())
        })
    }
    fn has_images(&self) -> bool {
        self.images.borrow().is_some()
    }
    async fn images_on(&self, _place_id: &str) -> PlaceResult<Vec<HeldImage>> {
        Ok(self.images.borrow().clone().unwrap_or_default())
    }
    async fn drop_image_on(&self, place_id: &str, id: &str) -> PlaceResult<()> {
        self.note(format!("dropImageOn {place_id} {id}"));
        Ok(())
    }
    async fn spent_today(&self, _places: &[PlaceView]) -> Option<PlaceResult<Vec<SpentToday>>> {
        self.spent.borrow().clone().map(|r| r.map_err(HostError::new))
    }
    async fn sign_in(&self, _: &str, _: &str, _: SignInEmit) -> Option<PlaceResult<SignInHandle>> {
        None
    }
    async fn mint_here(&self, _: &str, _: SignInEmit) -> Option<PlaceResult<SignInHandle>> {
        None
    }
    async fn sign_in_line(&self, _: &str, _: &str) -> Option<PlaceResult<wsp_wire::types::agents_report::SignInLine>> {
        None
    }
    fn stores_on(&self, _place_id: &str, _home: &str) -> Option<IndexMap<String, String>> {
        self.stores.borrow().clone()
    }
    async fn login_here(&self, _: &str) -> Option<PlaceResult<Option<Vec<u8>>>> {
        None
    }
    async fn add_folder(&self, _: &str, _: &str, _: &wsp_wire::types::recipe_file::RecipeFileFoldersValue, _: Option<&FolderMove>, _: Option<Rc<dyn Fn(&str)>>) -> Option<PlaceResult<HeldRow>> {
        None
    }
    async fn folder_remote(&self, _: &wsp_wire::types::recipe_file::RecipeFileFoldersValue) -> Option<PlaceResult<Option<String>>> {
        None
    }
    async fn remove_folder(&self, place_id: &str, project_id: &str) -> PlaceResult<()> {
        self.note(format!("removeFolder {place_id} {project_id}"));
        Ok(())
    }
    async fn folder_look(&self, project_id: &str, _: &wsp_wire::types::recipe_file::RecipeFileFoldersValue, _: &wsp_wire::types::recipe_file::RecipeFileFoldersValue) -> PlaceResult<()> {
        self.note(format!("folderLook {project_id}"));
        Ok(())
    }
}

// ---------------------------------------------------------------- the setup area's reads

/// What places-a's setup area answers manage and views, set by the test, every call written down.
#[derive(Default)]
pub struct FakeSetup {
    pub log: RefCell<Vec<String>>,
    pub waiting: RefCell<Vec<String>>,
    pub setting: RefCell<IndexSet<String>>,
    pub syncing: RefCell<IndexSet<String>>,
    pub busy: RefCell<Option<String>>,
    pub skippers: RefCell<IndexMap<String, Rc<dyn Fn() -> LocalFut<'static, PlaceResult<()>>>>>,
    pub started: RefCell<SetupStarted>,
    pub syncs: RefCell<IndexMap<String, SyncRead>>,
    pub sync_frames: RefCell<Vec<PlaceSyncEvent>>,
    pub setup_frames: RefCell<Vec<PlaceSetupEvent>>,
    pub link: RefCell<Option<Arc<dyn MachineLink>>>,
}

impl FakeSetup {
    fn note(&self, s: impl Into<String>) {
        self.log.borrow_mut().push(s.into());
    }
    pub fn calls(&self) -> Vec<String> {
        self.log.borrow().clone()
    }
}

impl ViewsSetup for FakeSetup {
    fn woken(&self, place_id: &str, back: bool) {
        self.note(format!("woken {place_id} {back}"));
    }
    fn forget_closed_at(&self, place_id: &str) {
        self.note(format!("closedAt.delete {place_id}"));
    }
    fn link_to(&self, place_id: &str) -> Arc<dyn MachineLink> {
        self.note(format!("linkTo {place_id}"));
        self.link.borrow().clone().expect("a test that reaches link_to sets FakeSetup.link")
    }
}

#[async_trait(?Send)]
impl ManageSetup for FakeSetup {
    fn waiting_ids(&self) -> Vec<String> {
        self.waiting.borrow().clone()
    }
    fn is_setting(&self, place_id: &str) -> bool {
        self.setting.borrow().contains(place_id)
    }
    fn is_syncing(&self, place_id: &str) -> bool {
        self.syncing.borrow().contains(place_id)
    }
    fn skipper(&self, key: &str) -> Option<Rc<dyn Fn() -> LocalFut<'static, PlaceResult<()>>>> {
        self.skippers.borrow().get(key).cloned()
    }
    fn setting_now(&self, _record: &PlaceRecord) -> Option<String> {
        self.busy.borrow().clone()
    }
    fn recipes_moved(&self, slug: &str) {
        self.note(format!("recipesMoved {slug}"));
    }
    fn sync_frame(&self, event: PlaceSyncEvent) {
        self.note(format!("syncFrame {}", event.place_id));
        self.sync_frames.borrow_mut().push(event);
    }
    fn setup_frame(&self, event: PlaceSetupEvent) {
        self.note(format!("setupFrame {}", event.place_id));
        self.setup_frames.borrow_mut().push(event);
    }
    async fn start_setup(&self, place_id: &str, add_id: &str, picks: Option<SetupPicks>, row: Option<String>) -> PlaceResult<SetupStarted> {
        self.note(format!("startSetup {place_id} {add_id} picks={} recipe={} row={}", picks.as_ref().map(|p| p.picks.name.clone()).unwrap_or("-".into()), picks.as_ref().and_then(|p| p.recipe.clone()).unwrap_or("-".into()), row.unwrap_or("-".into())));
        Ok(self.started.borrow().clone())
    }
    async fn started_or_said(&self, place_id: &str, add_id: &str, picks: SetupPicks) -> PlaceResult<SetupStarted> {
        self.note(format!("startedOrSaid {place_id} {add_id} picks={} recipe={}", picks.picks.name, picks.recipe.unwrap_or("-".into())));
        Ok(self.started.borrow().clone())
    }
    fn clear_sync_timers(&self) {
        self.note("clearSyncTimers");
    }
    async fn sync_of(&self, record: &PlaceRecord, resolved: Option<&Resolved>) -> PlaceResult<Option<SyncRead>> {
        self.note(format!("syncOf {} resolved={}", record.id, resolved.is_some()));
        Ok(self.syncs.borrow().get(&record.id).cloned())
    }
    async fn mark_sync(&self, place_id: &str, sync: PlaceSync) -> PlaceResult<()> {
        self.note(format!("markSync {place_id} {}", serde_json::to_string(&sync).unwrap()));
        Ok(())
    }
    fn sync_soon(&self, place_id: &str, after_ms: u64) {
        self.note(format!("syncSoon {place_id} {after_ms}"));
    }
}

// ---------------------------------------------------------------- link reads, reach, socket

/// What `ctx.door` answers views and manage about links.
#[derive(Default)]
pub struct FakeLinks {
    pub backends: RefCell<IndexMap<String, Arc<dyn MachineBackend>>>,
    pub forking: RefCell<IndexMap<String, Result<Arc<dyn MachineBackend>, String>>>,
    pub asked: RefCell<Vec<String>>,
    /// A computer slower than one round trip: its read of what it forks with never answers.
    pub slow: std::cell::Cell<bool>,
}

#[async_trait(?Send)]
impl LinkReads for FakeLinks {
    fn backend_of(&self, place_id: &str) -> Option<Arc<dyn MachineBackend>> {
        self.backends.borrow().get(place_id).cloned()
    }
    async fn forking_backend(&self, place_id: &str) -> PlaceResult<Arc<dyn MachineBackend>> {
        self.asked.borrow_mut().push(place_id.to_string());
        if self.slow.get() {
            futures::future::pending::<()>().await;
        }
        match self.forking.borrow().get(place_id) {
            Some(Ok(b)) => Ok(b.clone()),
            Some(Err(e)) => Err(HostError::new(e.clone())),
            None => Err(HostError::new(format!("{place_id} has not said what it forks with"))),
        }
    }
    fn folder_computer(&self, _place_id: &str) -> Option<FolderComputer> {
        None
    }
}

/// A daemon reach answering each op with what `answer` says, every request written down with its params.
pub struct FakeReach {
    pub answer: RefCell<Box<dyn Fn(&str, &Map<String, Value>) -> Result<Map<String, Value>, ReachError>>>,
    pub asked: RefCell<Vec<(String, Value)>>,
    pub closed: Cell<bool>,
}

impl FakeReach {
    pub fn new(answer: impl Fn(&str, &Map<String, Value>) -> Result<Map<String, Value>, ReachError> + 'static) -> Rc<Self> {
        Rc::new(FakeReach { answer: RefCell::new(Box::new(answer)), asked: RefCell::new(Vec::new()), closed: Cell::new(false) })
    }
    /// Answers `ping`, `exec` (exit 0, nothing said) and `place.leave` (`swept`), refusing the rest.
    pub fn answering(swept: Vec<&'static str>) -> Rc<Self> {
        FakeReach::new(move |op, _| match op {
            "ping" => Ok(Map::new()),
            "exec" => Ok(obj(json!({ "exitCode": 0, "stdout": "", "stderr": "" }))),
            "place.leave" => Ok(obj(json!({ "swept": swept }))),
            other => Err(reach_err(format!("unknown op {other}"))),
        })
    }
    pub fn ops(&self) -> Vec<String> {
        self.asked.borrow().iter().map(|(op, _)| op.clone()).collect()
    }
}

#[async_trait(?Send)]
impl DaemonReach for FakeReach {
    async fn request(&self, op: &str, params: Map<String, Value>) -> Result<Map<String, Value>, ReachError> {
        self.asked.borrow_mut().push((op.to_string(), Value::Object(params.clone())));
        (self.answer.borrow())(op, &params)
    }
    async fn ready(&self) -> Result<(), ReachError> {
        Ok(())
    }
    fn status(&self) -> Option<DaemonLinkStatus> {
        None
    }
    fn stats(&self) -> ReachStats {
        ReachStats::default()
    }
    fn close(&self) {
        self.closed.set(true);
    }
}

/// What a refused frame answers with.
pub fn reach_err(message: impl Into<String>) -> ReachError {
    ReachError { message: message.into(), kind: None, status: None, code: None }
}

/// The record `admit` lists for a device of that name.
pub fn device_view(name: &str) -> DeviceView {
    serde_json::from_value(json!({ "id": format!("d_{name}"), "name": name, "createdAt": "2023-11-14T22:13:20.000Z", "lastSeenAt": "2023-11-14T22:13:20.000Z" })).expect("a device view")
}

pub fn obj(v: Value) -> Map<String, Value> {
    match v {
        Value::Object(m) => m,
        _ => panic!("not an object"),
    }
}

/// The place link's socket: open until closed, the close's code and reason kept.
#[derive(Default)]
pub struct FakeSocket {
    pub shut: Cell<bool>,
    pub closed: RefCell<Option<(u16, String)>>,
}

impl LinkSocket for FakeSocket {
    fn is_open(&self) -> bool {
        !self.shut.get() && self.closed.borrow().is_none()
    }
    fn close(&self, code: u16, reason: &str) {
        *self.closed.borrow_mut() = Some((code, reason.to_string()));
    }
    fn on_close(&self, _f: Box<dyn FnOnce(u16, String)>) -> u64 {
        0
    }
    fn off_close(&self, _id: u64) {}
    /// Slice b's tests never attach a socket; a link they need goes in with `link()`.
    fn connect(self: Rc<Self>, _seal: Option<wsp_store::keys::Seal>, _on_event: Rc<dyn Fn(&wsp_wire::wire::daemon_events::DaemonEvent)>) -> Rc<dyn DaemonReach> {
        FakeReach::answering(vec![])
    }
}

/// A link held for `place_id`, as places-a's attach leaves one; answers the socket and whether `seen` was cancelled.
pub fn link(d: &Door, place_id: &str, reach: Rc<FakeReach>) -> (Rc<FakeSocket>, Rc<Cell<bool>>) {
    let socket = Rc::new(FakeSocket::default());
    let cancelled = Rc::new(Cell::new(false));
    let c = cancelled.clone();
    d.ctx.live.borrow_mut().insert(place_id.to_string(), Live { socket: socket.clone(), reach, seen: Box::new(move || c.set(true)), forward: None });
    (socket, cancelled)
}

// ---------------------------------------------------------------- roads

macro_rules! road {
    ($name:ident, $tr:ident, fn $f:ident(&self $(, $a:ident: $t:ty)*) -> $r:ty) => {
        pub struct $name(pub Box<dyn Fn($($t),*) -> $r>);
        #[async_trait(?Send)]
        impl $tr for $name {
            async fn $f(&self $(, $a: $t)*) -> $r {
                (self.0)($($a),*)
            }
        }
    };
}

road!(Dial, PlaceDialler, fn dial(&self, login: &PlaceLogin) -> PlaceResult<()>);
road!(Log, PlaceLogReader, fn log(&self, login: &PlaceLogin) -> PlaceResult<Vec<String>>);
road!(Update, PlaceUpdater, fn update(&self, req: PlaceUpdateRequest) -> PlaceResult<Option<PlaceUpdateLanded>>);
road!(Leave, PlaceLeaver, fn leave(&self, req: PlaceLeaveRequest) -> PlaceResult<Vec<String>>);
road!(ForgetKey, PlaceKeyForgetter, fn forget_key(&self, place_id: &str) -> PlaceResult<Option<String>>);
road!(Undo, PlaceUndoer, fn undo(&self, login: &PlaceLogin, script: &str) -> PlaceResult<()>);

pub struct Sudo(pub Box<dyn Fn(&PlaceLogin, Option<&str>, SudoVerb, &str) -> PlaceResult<String>>);
#[async_trait(?Send)]
impl PlaceSudo for Sudo {
    async fn sudo_over(&self, login: &PlaceLogin, sudo_password: Option<&str>, verb: SudoVerb, name: &str) -> PlaceResult<String> {
        (self.0)(login, sudo_password, verb, name)
    }
}

pub struct RunOverRoad(pub Box<dyn Fn(&PlaceLogin, &str, u64, Option<&str>) -> PlaceResult<RunOver>>, pub RefCell<Vec<String>>);
#[async_trait(?Send)]
impl PlaceRunOver for RunOverRoad {
    async fn run_over(&self, login: &PlaceLogin, script: &str, timeout_ms: u64, sudo_password: Option<&str>) -> PlaceResult<RunOver> {
        self.1.borrow_mut().push(script.to_string());
        (self.0)(login, script, timeout_ms, sudo_password)
    }
}

/// The installer: runs `then` with the request and the stage, the door's context in reach so it can land a join.
pub struct Install(pub Box<dyn Fn(PlaceInstallRequest, PlaceStaging) -> LocalFut<'static, PlaceResult<PlaceInstalled>>>);
#[async_trait(?Send)]
impl PlaceInstaller for Install {
    async fn install(&self, req: PlaceInstallRequest, stage: PlaceStaging) -> PlaceResult<PlaceInstalled> {
        (self.0)(req, stage).await
    }
}

/// The back holder: every hold and release written down.
#[derive(Default)]
pub struct Back {
    pub log: RefCell<Vec<String>>,
}
#[async_trait(?Send)]
impl PlaceBackHolder for Back {
    async fn hold(&self, login: &PlaceLogin, back: wsp_wire::views::place::PlaceBack, on: BackOn, _moved: Option<Rc<dyn Fn(wsp_wire::views::place::PlaceBack)>>) -> PlaceResult<wsp_wire::views::place::PlaceBack> {
        self.log.borrow_mut().push(format!("hold {} {} {}", login.ssh, on.home, on.computer.map(|c| c.id).unwrap_or_default()));
        Ok(back)
    }
    fn release(&self, login: &PlaceLogin) {
        self.log.borrow_mut().push(format!("release {}", login.ssh));
    }
    fn door(&self, _at: DoorAt) {}
    fn close(&self) {
        self.log.borrow_mut().push("close".into());
    }
}

/// The provisioner's estimate, undo and floor, set per test; the rest refuses.
#[derive(Default)]
pub struct Provision {
    pub weight: RefCell<Option<PicksWeight>>,
    pub undos: RefCell<Option<Vec<PlaceUndo>>>,
    pub floor_rows: RefCell<Option<Result<Vec<wsp_wire::views::place::PlaceProvisionRow>, String>>>,
    /// Runs inside `floor`, before it answers: what happens on the host while the floor is out.
    pub on_floor: RefCell<Option<Box<dyn Fn()>>>,
    pub log: RefCell<Vec<String>>,
}
#[async_trait(?Send)]
impl PlaceProvisioner for Provision {
    async fn setup(&self, _: &RecipeFile, _: SetupOn, _: Option<&IndexSet<wsp_wire::views::place::PlaceSetupStep>>) -> PlaceResult<wsp_engine::provision::ProvisionPlan> {
        Err(HostError::new("no setup in these tests"))
    }
    async fn floor(&self, _machine: Rc<dyn wsp_engine::machine::Machine>, home: &str, _stage: wsp_engine::provision::ProvisionStage) -> PlaceResult<Vec<wsp_wire::views::place::PlaceProvisionRow>> {
        self.log.borrow_mut().push(format!("floor {home}"));
        if let Some(f) = self.on_floor.borrow().as_ref() {
            f();
        }
        self.floor_rows.borrow().clone().unwrap_or(Ok(vec![])).map_err(HostError::new)
    }
    async fn step(&self, _: Rc<dyn wsp_engine::machine::Machine>, _: &wsp_engine::provision::ProvisionPlan, _: &wsp_engine::provision::EngineStep, _: &wsp_engine::provision::SetupRun, _: wsp_engine::provision::ProvisionStage, _: &wsp_engine::provision::ProvisionOn) -> PlaceResult<Vec<wsp_wire::views::place::PlaceProvisionRow>> {
        Err(HostError::new("no steps in these tests"))
    }
    fn has_undo(&self) -> bool {
        self.undos.borrow().is_some()
    }
    async fn undo(&self, _before: &RecipeFile, removed: &[RemovedRow], home: &str, _stores: Option<&IndexMap<String, String>>) -> PlaceResult<Vec<PlaceUndo>> {
        self.log.borrow_mut().push(format!("undo {home} {}", removed.iter().map(|r| format!("{}/{}", r.kind, r.name)).collect::<Vec<_>>().join(",")));
        Ok(self.undos.borrow().clone().unwrap_or_default())
    }
    fn has_project_servers(&self) -> bool {
        false
    }
    async fn project_servers(&self, _: Rc<dyn wsp_engine::machine::Machine>, _: &RecipeFile, _: &str, _: &str, _: wsp_engine::provision::ProvisionStage, _: &wsp_engine::provision::ProvisionOn, _: Option<&str>) -> PlaceResult<Vec<wsp_wire::views::place::PlaceProvisionRow>> {
        Ok(vec![])
    }
    fn has_estimate(&self) -> bool {
        self.weight.borrow().is_some()
    }
    async fn estimate(&self, _picks: &RecipeFile) -> PlaceResult<PicksWeight> {
        Ok(self.weight.borrow().expect("estimate set"))
    }
}

/// A provider table: the rows it lists, the one it is wired to, and a backend per row.
pub struct Providers {
    pub wired: String,
    pub rows: IndexMap<String, Arc<dyn MachineBackend>>,
}
impl PlaceBackends for Providers {
    fn wired(&self) -> String {
        self.wired.clone()
    }
    fn backend(&self, place: &str) -> Option<Arc<dyn MachineBackend>> {
        self.rows.get(place).cloned()
    }
    fn list(&self) -> Vec<String> {
        self.rows.keys().cloned().collect()
    }
}

/// The daemon beside this host, at a version.
pub struct Here(pub Option<u32>);
#[async_trait(?Send)]
impl HereDaemon for Here {
    async fn version(&self) -> Result<u32, HostError> {
        Ok(self.0.unwrap_or(0))
    }
    fn held(&self) -> Option<Option<u32>> {
        Some(self.0)
    }
    fn fix(&self) -> String {
        "run wsp update here".into()
    }
}

// ---------------------------------------------------------------- the door

/// Everything a test sets before the door is built.
pub struct Seed {
    pub records: Vec<Value>,
    pub caps: Vec<(String, Value)>,
    pub mark: Option<String>,
    pub pending: Vec<Value>,
    pub host_key: PlaceKeyPair,
    pub provider: Option<WiredProvider>,
    pub providers: Option<Rc<Providers>>,
    pub wiring: Box<dyn FnOnce(&mut PlaceWiring)>,
    pub opts: Box<dyn FnOnce(&mut PlaceDoorOptions)>,
    pub recording: FakeRecording,
    pub setup: FakeSetup,
    pub links: FakeLinks,
    pub vault: IndexMap<String, String>,
}

impl Default for Seed {
    fn default() -> Self {
        Seed {
            records: vec![record_json(json!({}))],
            caps: vec![],
            mark: None,
            pending: vec![],
            host_key: new_place_key_pair(),
            provider: Some(WiredProvider { id: "boat".into(), rate_usd_per_hour: 0.25 }),
            providers: None,
            wiring: Box::new(|_| {}),
            opts: Box::new(|_| {}),
            recording: FakeRecording::default(),
            setup: FakeSetup::default(),
            links: FakeLinks::default(),
            vault: IndexMap::from([("ANTHROPIC_API_KEY".to_string(), "k".to_string())]),
        }
    }
}

/// The four areas of slice b on one context, with every fake in reach.
pub struct Door {
    pub ctx: Rc<PlaceDoorContext>,
    pub records: Rc<PlaceRecordsArea>,
    pub views: Rc<PlaceViewsArea>,
    pub manage: Rc<ManageDoor>,
    pub store: Rc<MemStore>,
    pub clock: Rc<FakeClock>,
    pub devices: Rc<FakeDevices>,
    pub recording: Rc<FakeRecording>,
    pub setup: Rc<FakeSetup>,
    pub links: Rc<FakeLinks>,
    pub host_key: PlaceKeyPair,
    pub events: Rc<RefCell<Vec<PlaceEvent>>>,
    pub setups: Rc<RefCell<Vec<SetupEvent>>>,
    pub stages: Rc<RefCell<Vec<wsp_wire::views::place::PlaceStageEvent>>>,
    pub naps: Rc<RefCell<Vec<String>>>,
}

pub fn here_place() -> HerePlace {
    HerePlace {
        name: "zingzys-mac".into(),
        label: None,
        mac: Some(serde_json::from_value(json!("mac-mini")).unwrap()),
        os: Some("macOS 15.0".into()),
        shape: Some(serde_json::from_value(json!({ "cpu": 8, "memMb": 16384 })).unwrap()),
        disk_free_bytes: None,
        home: Some("/home/dev".into()),
    }
}

/// Builds the door over a store seeded as `seed` says, as places-a's `load` would leave it (records in `kept`,
/// settings held), with the clock at `NOW`.
pub fn door(f: impl FnOnce(&mut Seed)) -> Door {
    let mut seed = Seed::default();
    f(&mut seed);
    let store = Rc::new(MemStore::default());
    for r in &seed.records {
        store.seed(PLACES, r["id"].as_str().unwrap(), r.clone());
    }
    for (id, c) in &seed.caps {
        store.seed(CAPS, id, c.clone());
    }
    if let Some(m) = &seed.mark {
        store.seed(DEFAULT_COLLECTION, DEFAULT_ID, json!({ "placeId": m }));
    }
    for p in &seed.pending {
        store.seed("pending-computers", p["id"].as_str().unwrap(), p.clone());
    }
    store.writes.borrow_mut().clear();
    let clock = FakeClock::new(NOW);
    let devices = Rc::new(FakeDevices::default());
    let recording = Rc::new(seed.recording);
    let setup = Rc::new(seed.setup);
    let links = Rc::new(seed.links);
    let provider = seed.provider.clone();
    let mut wiring = PlaceWiring {
        host_key: seed.host_key.clone(),
        provider: Some(Rc::new(move || provider.clone())),
        here: Rc::new(here_place),
        host_name: Rc::new(|| "zingzys-mac".into()),
        install: None,
        dial: None,
        log: None,
        update: None,
        leave: None,
        forget_key: None,
        sudo_over: None,
        provision: None,
        github_token: None,
        undo: None,
        run_over: None,
        back: None,
    };
    (seed.wiring)(&mut wiring);
    let events = Rc::new(RefCell::new(Vec::new()));
    let setups = Rc::new(RefCell::new(Vec::new()));
    let stages = Rc::new(RefCell::new(Vec::new()));
    let naps = Rc::new(RefCell::new(Vec::new()));
    let vault = seed.vault.clone();
    let (s1, s2, s3) = (setups.clone(), stages.clone(), naps.clone());
    let providers = seed.providers.clone();
    let mut opts = PlaceDoorOptions {
        store: store.clone(),
        devices: devices.clone(),
        wiring,
        recording: recording.clone(),
        providers: providers.map(|p| Rc::new(move || p.clone() as Rc<dyn PlaceBackends>) as Rc<dyn Fn() -> Rc<dyn PlaceBackends>>),
        on_daemon_event: None,
        on_stage: Some(Rc::new(move |e| s2.borrow_mut().push(e.clone()))),
        on_setup: Some(Rc::new(move |e| s1.borrow_mut().push(e.clone()))),
        copy_build: None,
        nap_ms: None,
        here_daemon: None,
        nap_changed: Some(Rc::new(move |id| s3.borrow_mut().push(id.to_string()))),
        join_wait_ms: None,
        update_wait_ms: None,
        dial_wait_ms: None,
        frame_wait_ms: None,
        relink_wait_ms: None,
        seen_every_ms: None,
        vault: Some(Rc::new(move || vault.clone())),
        recipes: None,
        owner: None,
        kill_confirm: None,
        clock: Some(clock.clone()),
    };
    (seed.opts)(&mut opts);
    let ctx = place_door_context(opts);
    for r in &seed.records {
        let held: PlaceRecord = serde_json::from_value(r.clone()).unwrap();
        ctx.kept.borrow_mut().insert(held.id.clone(), held);
    }
    let records = PlaceRecordsArea::new(ctx.clone());
    for (id, c) in &seed.caps {
        if let Ok(s) = serde_json::from_value(c.clone()) {
            records.settings_held.borrow_mut().insert(id.clone(), s);
        }
    }
    let views = PlaceViewsArea::new(ctx.clone(), records.clone(), setup.clone(), links.clone());
    let manage = ManageDoor::new(ctx.clone(), records.clone(), setup.clone(), views.clone(), links.clone());
    let e = events.clone();
    std::mem::forget(ctx.watch(Rc::new(move |ev: &PlaceEvent| e.borrow_mut().push(ev.clone()))));
    Door { ctx, records, views, manage, store, clock, devices, recording, setup, links, host_key: seed.host_key, events, setups, stages, naps }
}

impl Door {
    /// The record as the store keeps it now.
    pub fn stored(&self, id: &str) -> Option<Value> {
        self.store.doc(PLACES, id)
    }
    pub fn caps(&self, id: &str) -> Option<Value> {
        self.store.doc(CAPS, id)
    }
    pub fn mark(&self) -> Option<Value> {
        self.store.doc(DEFAULT_COLLECTION, DEFAULT_ID)
    }
    pub fn pending_docs(&self) -> Vec<Value> {
        self.store.all("pending-computers")
    }
    /// The place events emitted so far, as JSON with their `type`.
    pub fn event_types(&self) -> Vec<String> {
        self.events
            .borrow()
            .iter()
            .map(|e| match e {
                PlaceEvent::Joined(_) => "place.joined",
                PlaceEvent::Changed(_) => "place.changed",
                PlaceEvent::Removed(_) => "place.removed",
                PlaceEvent::Absent(_) => "place.absent",
            })
            .map(String::from)
            .collect()
    }
}

/// A pending add as the store keeps one.
pub fn pending_json(id: &str, address: &str, over: Value) -> Value {
    let mut p = json!({ "id": id, "address": address, "step": "choosing", "choices": { "name": address, "agents": {}, "mcp": {}, "clis": {}, "skills": {}, "plugins": {}, "folders": {}, "configs": {} }, "startedAt": "2026-10-02T00:00:00.000Z" });
    merge(&mut p, over);
    p
}

pub fn pending_record(v: Value) -> PendingRecord {
    serde_json::from_value(v).expect("a pending record")
}

/// A recipe file named `name` with the plugins given and every other section empty.
pub fn picks(name: &str, plugins: &[&str]) -> RecipeFile {
    let plugins: Map<String, Value> = plugins.iter().map(|p| (p.to_string(), json!({}))).collect();
    serde_json::from_value(json!({ "name": name, "agents": {}, "mcp": {}, "clis": {}, "skills": {}, "plugins": plugins, "folders": {}, "configs": {} })).expect("picks")
}

/// Runs a test body on a current-thread runtime with a `LocalSet`, as the host runs (spec 03).
pub fn run<F: std::future::Future<Output = ()>>(f: F) {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    tokio::task::LocalSet::new().block_on(&rt, f);
}

// ---------------------------------------------------------------- a backend

/// A machine backend answering only what the place door reads off one: its capabilities (`PLACE_FACTS` of the TS
/// fixture by default), its pricing (`rate` an hour at every size, `default` as its default size), which optional
/// methods it offers, and what `place_limits`, `capacity` and `list_snapshots` answer. Every machine road refuses.
pub struct StubBackend {
    pub capabilities: Value,
    pub rate: f64,
    pub default: (f64, f64),
    pub offers: wsp_engine::machine::BackendOffers,
    pub limits: std::sync::Mutex<Option<Result<Option<wsp_wire::views::place_more::CloudLimits>, String>>>,
    pub capacity: std::sync::Mutex<Option<Result<Value, String>>>,
    pub snapshots: std::sync::Mutex<Option<Result<Vec<Value>, String>>>,
    pub asked: std::sync::Mutex<Vec<String>>,
}

impl StubBackend {
    pub fn new() -> Self {
        StubBackend {
            capabilities: json!({
                "liveCloneForks": false, "pauseMode": "memory", "replacesMachine": true, "previewUrls": false, "signedUrls": false,
                "callbackRelay": true, "diskSnapshots": true, "images": true, "snapshotsAnyLife": false, "snapshotListing": true,
                "templates": true, "kept": false, "copies": true, "ownNetwork": true, "sizes": [{ "cpu": 2, "memMb": 4096, "rateUsdPerHour": 0 }]
            }),
            rate: 0.0,
            default: (2.0, 4096.0),
            offers: wsp_engine::machine::BackendOffers::default(),
            limits: std::sync::Mutex::new(None),
            capacity: std::sync::Mutex::new(None),
            snapshots: std::sync::Mutex::new(None),
            asked: std::sync::Mutex::new(Vec::new()),
        }
    }
    pub fn arc(self) -> Arc<dyn MachineBackend> {
        Arc::new(self)
    }
    fn note(&self, s: &str) {
        self.asked.lock().unwrap().push(s.to_string());
    }
}

struct StubPricing {
    rate: f64,
    default: (f64, f64),
}

impl wsp_engine::machine::BackendPricing for StubPricing {
    fn rate_usd_per_hour(&self, _size: wsp_engine::machine::Size) -> f64 {
        self.rate
    }
    fn default_size(&self) -> wsp_engine::machine::Size {
        wsp_engine::machine::Size { cpu: self.default.0, mem_mb: self.default.1 }
    }
    fn snapshot_storage(&self) -> wsp_engine::machine::SnapshotStoragePricing {
        wsp_engine::machine::SnapshotStoragePricing {
            wire: serde_json::from_value(json!({ "freeGb": 0, "usdPerGbMonth": 0, "billedFrom": "" })).unwrap(),
            free_named: None,
            usd_per_named_month: None,
        }
    }
}

fn refused<T>(what: &str) -> wsp_engine::errors::Result<T> {
    Err(wsp_engine::EngineError::Other { message: format!("{what} is not on this stub"), code: None })
}

#[async_trait(?Send)]
impl MachineBackend for StubBackend {
    fn capabilities(&self) -> wsp_engine::machine::Capabilities {
        serde_json::from_value(self.capabilities.clone()).expect("capabilities")
    }
    fn pricing(&self) -> Arc<dyn wsp_engine::machine::BackendPricing> {
        Arc::new(StubPricing { rate: self.rate, default: self.default })
    }
    fn offers(&self) -> wsp_engine::machine::BackendOffers {
        self.offers.clone()
    }
    async fn create(&self, _: wsp_engine::machine::MachineSpec, _: Option<wsp_engine::machine::Waiting>, _: Option<String>) -> wsp_engine::errors::Result<Arc<dyn wsp_engine::machine::Machine>> {
        refused("create")
    }
    async fn get(&self, _: &str) -> wsp_engine::errors::Result<Arc<dyn wsp_engine::machine::Machine>> {
        refused("get")
    }
    async fn list(&self, _: Option<&IndexMap<String, String>>) -> wsp_engine::errors::Result<Vec<wsp_engine::machine::MachineListRow>> {
        refused("list")
    }
    async fn delete_snapshot(&self, _: &str) -> wsp_engine::errors::Result<()> {
        refused("delete_snapshot")
    }
    async fn place_limits(&self) -> wsp_engine::errors::Result<Option<wsp_wire::views::place_more::CloudLimits>> {
        self.note("placeLimits");
        match self.limits.lock().unwrap().clone() {
            Some(Ok(l)) => Ok(l),
            Some(Err(e)) => Err(wsp_engine::EngineError::Other { message: e, code: None }),
            None => refused("place_limits"),
        }
    }
    async fn capacity(&self) -> wsp_engine::errors::Result<wsp_engine::machine::PlaceCapacity> {
        self.note("capacity");
        match self.capacity.lock().unwrap().clone() {
            Some(Ok(c)) => Ok(serde_json::from_value(c).expect("capacity")),
            Some(Err(e)) => Err(wsp_engine::EngineError::Other { message: e, code: None }),
            None => refused("capacity"),
        }
    }
    async fn list_snapshots(&self, ids: Option<&[String]>) -> wsp_engine::errors::Result<Vec<wsp_engine::machine::SnapshotRow>> {
        self.note(&format!("listSnapshots {}", ids.map(|i| i.join(",")).unwrap_or_default()));
        match self.snapshots.lock().unwrap().clone() {
            Some(Ok(rows)) => Ok(rows.into_iter().map(|r| serde_json::from_value(r).expect("snapshot row")).collect()),
            Some(Err(e)) => Err(wsp_engine::EngineError::Other { message: e, code: None }),
            None => refused("list_snapshots"),
        }
    }
}

// ---------------------------------------------------------------- a machine link

/// The link a `PlaceMachine` rides, answering each frame with what `answer` says; every frame written down.
pub struct FakeMachineLink {
    pub answer: Box<dyn Fn(&str, &Map<String, Value>) -> Result<Map<String, Value>, String> + Send + Sync>,
    pub asked: std::sync::Mutex<Vec<(String, Value)>>,
}

impl FakeMachineLink {
    pub fn new(answer: impl Fn(&str, &Map<String, Value>) -> Result<Map<String, Value>, String> + Send + Sync + 'static) -> Arc<Self> {
        Arc::new(FakeMachineLink { answer: Box::new(answer), asked: std::sync::Mutex::new(Vec::new()) })
    }
    /// Every exec answers exit 0 with `stdout`.
    pub fn exec_says(stdout: &'static str) -> Arc<Self> {
        FakeMachineLink::new(move |_, _| Ok(obj(json!({ "exitCode": 0, "stdout": stdout, "stderr": "", "truncated": false }))))
    }
    pub fn ops(&self) -> Vec<(String, Value)> {
        self.asked.lock().unwrap().clone()
    }
}

#[async_trait(?Send)]
impl MachineLink for FakeMachineLink {
    async fn request(&self, op: &str, params: Map<String, Value>, _ask: wsp_engine::link_backend::LinkAsk) -> wsp_engine::errors::Result<Map<String, Value>> {
        self.asked.lock().unwrap().push((op.to_string(), Value::Object(params.clone())));
        (self.answer)(op, &params).map_err(|message| wsp_engine::EngineError::Other { message, code: None })
    }
    async fn forward(&self, _place_port: u16) -> wsp_engine::errors::Result<u16> {
        Err(wsp_engine::EngineError::Other { message: "no forward on this fake".into(), code: None })
    }
}
