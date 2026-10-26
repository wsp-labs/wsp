// SPDX-License-Identifier: AGPL-3.0-only
//! Ports places-facts.test.ts:15, places.test.ts:188 and :278, places-agents.test.ts:87, places-setup.test.ts:1127, :1651-1680, :1705-1771 and :1826-1922, place-forward.test.ts:45-77 and copy-on-press.test.ts:296-333 at the link door's reads.

mod common;

use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use base64::Engine as _;
use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::oneshot;

use common::places::{
    answers_ok, block, boxed, err, held, iso, obj, record, recipe, report, row, settle, until, FakeSocket, Reply, Scene, SceneOpts, Script, T0,
};
use wsp_core::types::pending::DaemonChannel;
use wsp_core::types::pending::DaemonReach;
use wsp_engine::errors::Result as EngineResult;
use wsp_engine::machine::{
    BackendPricing, Capabilities, ExecResult, Machine, MachineBackend, MachineListRow, MachineSpec, Size, SnapshotStoragePricing, Waiting,
    WireSnapshotStoragePricing,
};
use wsp_engine::provision::{EngineStep, ProvisionStage};
use wsp_machines::places::helpers::{shared_logins_script, LandedRow, LinkSocket, PendingRecord};
use wsp_machines::places::link::{Joining, LinkDoor, LinkRecords, LinkViews};
use wsp_machines::places::setup::{under_folders_of, Lander};
use wsp_machines::places::types::{
    is_place_forks_nowhere, is_place_provisioning, FolderComputer, ForwardAsk, JoinAnswer, PlaceChallenge, PlaceDoor, PlaceEvent, PlaceExecOpts, PlaceFor,
    PlaceRecord, PlaceResult, Proved, DEFAULT_COLLECTION, DEFAULT_ID,
};
use wsp_store::keys::Seal;
use wsp_wire::types::recipe_file::RecipeFile;
use wsp_wire::views::place::{PendingComputer, PlaceAddJob, PlaceProvisionRow, PlaceSetAlso, PlaceSetupStep};
use wsp_wire::views::place_more::{
    PlaceDial, PlaceHolds, PlaceRemoved, PlaceSettingWord, PlaceSettings, PlaceSettingsAsk, PlaceUpdateReply, PlaceView,
};
use wsp_wire::views::place_setup_words::PlaceEstimate;
use wsp_wire::wire::daemon_events::DaemonEvent;
use wsp_wire::wire::machine_link::{BackendFacts, SysSample};
use wsp_wire::wire::place_link::{PlaceAuthRequest, PlaceJoinRequest, PlaceProveRequest, PlaceReport};
use wsp_words::exit::Kind;

const LOGINS: &str = "/wsp/logins";
const SIGNED_IN_THERE: &str = "signed in on that computer";

// ---------------------------------------------------------------------------------------------------------------
// The door as the context holds it: the link door's own reads, and an exec over the live link.

struct TestDoor {
    link: Weak<LinkDoor>,
    ctx: Rc<wsp_machines::places::context::PlaceDoorContext>,
    execs: RefCell<Vec<(String, String, PlaceExecOpts)>>,
}

#[async_trait(?Send)]
#[allow(unused_variables)]
impl PlaceDoor for TestDoor {
    fn answer_challenge(&self, subject: &str, nonce: &str, ephemeral: Option<&str>) -> Option<PlaceChallenge> { unreachable!("link_reads reads none of this") }
    async fn join(&self, req: PlaceJoinRequest, from: &str, now: u64) -> PlaceResult<Option<JoinAnswer>> { unreachable!("link_reads reads none of this") }
    async fn auth(&self, req: PlaceAuthRequest, now: u64) -> PlaceResult<wsp_machines::places::types::AuthAnswer> { unreachable!("link_reads reads none of this") }
    fn host_key(&self) -> String { unreachable!("link_reads reads none of this") }
    async fn prove(&self, place_id: &str, req: PlaceProveRequest, expect: &[u8], from: &str, now: u64) -> PlaceResult<Proved> { unreachable!("link_reads reads none of this") }
    async fn attach(&self, place_id: &str, socket: Rc<dyn LinkSocket>, report: PlaceReport, from: &str, now: u64, seal: Option<Seal>) -> PlaceResult<()> { unreachable!("link_reads reads none of this") }
    fn link(&self, place_id: &str) -> Option<Rc<dyn DaemonReach>> { unreachable!("link_reads reads none of this") }
    fn channel(&self, place_id: &str, on_event: Rc<dyn Fn(serde_json::Map<String, serde_json::Value>)>) -> Option<Rc<dyn DaemonChannel>> { unreachable!("link_reads reads none of this") }
    async fn watch_sys(&self, place_id: &str, f: Rc<dyn Fn(SysSample)>) -> PlaceResult<Box<dyn FnOnce()>> { unreachable!("link_reads reads none of this") }
    async fn load(&self) -> PlaceResult<()> { unreachable!("link_reads reads none of this") }
    fn backend_of(&self, place_id: &str) -> Option<Arc<dyn MachineBackend>> {
        self.link.upgrade().expect("the link door").backend_of(place_id)
    }
    fn folder_computer(&self, place_id: &str) -> Option<FolderComputer> { unreachable!("link_reads reads none of this") }
    fn joined(&self, place_id: &str) -> bool { unreachable!("link_reads reads none of this") }
    async fn forking_backend(&self, place_id: &str) -> PlaceResult<Arc<dyn MachineBackend>> { unreachable!("link_reads reads none of this") }
    fn name_of(&self, place_id: &str) -> String { unreachable!("link_reads reads none of this") }
    fn settings_at(&self, place_id: &str) -> PlaceSettings { unreachable!("link_reads reads none of this") }
    fn turn_limit_at(&self, place_id: &str) -> Option<Option<u64>> { unreachable!("link_reads reads none of this") }
    fn threads_at(&self, place_id: &str) -> Option<u64> { unreachable!("link_reads reads none of this") }
    fn machines_at(&self, place_id: &str) -> Option<u64> { unreachable!("link_reads reads none of this") }
    fn sign_ins_at(&self, place_id: &str, agents: Option<&[String]>) -> Option<IndexMap<String, wsp_wire::types::agents_report::AgentSignInState>> { unreachable!("link_reads reads none of this") }
    fn github_from_vault(&self, place_id: &str) -> bool { unreachable!("link_reads reads none of this") }
    fn vault_at(&self, place_id: &str, vault: &IndexMap<String, String>) -> IndexMap<String, String> { unreachable!("link_reads reads none of this") }
    async fn login_landed(&self, place_id: &str, agent: &str) -> PlaceResult<()> { unreachable!("link_reads reads none of this") }
    async fn logins_again(&self, place_id: &str) -> PlaceResult<()> { unreachable!("link_reads reads none of this") }
    async fn key_landed(&self, agent: &str) -> PlaceResult<()> { unreachable!("link_reads reads none of this") }
    async fn gh_there(&self, place_id: &str, stage: ProvisionStage) -> PlaceResult<Vec<PlaceProvisionRow>> { unreachable!("link_reads reads none of this") }
    fn offer_of(&self, place_id: &str) -> Option<String> { unreachable!("link_reads reads none of this") }
    async fn forward(&self, place_id: &str, place_port: u16, o: ForwardAsk) -> PlaceResult<u16> { unreachable!("link_reads reads none of this") }
    fn pane_forwards(&self) -> Rc<wsp_machines::places::pane_ports::PanePorts> { unreachable!("link_reads reads none of this") }
    async fn place_for(&self, word: &str) -> PlaceResult<PlaceFor> { unreachable!("link_reads reads none of this") }
    async fn default_place(&self) -> PlaceResult<PlaceFor> { unreachable!("link_reads reads none of this") }
    async fn mark_used(&self, place_id: Option<&str>) -> PlaceResult<()> { unreachable!("link_reads reads none of this") }
    async fn mark_default_if_none(&self, place_id: &str) -> PlaceResult<()> { unreachable!("link_reads reads none of this") }
    async fn add(&self, req: wsp_machines::places::types::AddAsk, now: u64) -> PlaceResult<wsp_machines::places::types::PlaceAdded> { unreachable!("link_reads reads none of this") }
    async fn set_up(&self, reference: &str, o: wsp_machines::places::types::SetUpAsk) -> PlaceResult<wsp_machines::places::types::PlaceSetUp> { unreachable!("link_reads reads none of this") }
    async fn pending(&self) -> PlaceResult<Vec<PendingComputer>> { unreachable!("link_reads reads none of this") }
    async fn choose(&self, reference: &str, choices: RecipeFile, recipe: Option<String>) -> PlaceResult<PendingComputer> { unreachable!("link_reads reads none of this") }
    async fn dial(&self, place_id: &str, now: u64) -> PlaceResult<PlaceDial> { unreachable!("link_reads reads none of this") }
    async fn set(&self, place_id: &str, set: PlaceSettingsAsk, reset: Vec<PlaceSettingWord>, also: PlaceSetAlso) -> PlaceResult<PlaceView> { unreachable!("link_reads reads none of this") }
    async fn road(&self, place_id: &str) -> PlaceResult<u16> { unreachable!("link_reads reads none of this") }
    async fn exec(&self, place_id: &str, cmd: &str, opts: PlaceExecOpts) -> PlaceResult<ExecResult> {
        self.execs.borrow_mut().push((place_id.to_string(), cmd.to_string(), opts.clone()));
        let reach = self.ctx.live.borrow().get(place_id).map(|l| Rc::clone(&l.reach)).ok_or_else(|| err("no link"))?;
        let mut frame = Map::new();
        frame.insert("cmd".into(), json!(cmd));
        if let Some(ms) = opts.timeout_ms {
            frame.insert("timeoutMs".into(), json!(ms));
        }
        let said = reach.request("exec", frame).await.map_err(|e| err(e.message))?;
        Ok(serde_json::from_value(Value::Object(said)).expect("an exec answer"))
    }
    async fn report_of(&self, place_id: &str) -> PlaceResult<Option<PlaceReport>> { unreachable!("link_reads reads none of this") }
    async fn home_of(&self, place_id: &str) -> PlaceResult<Option<String>> { unreachable!("link_reads reads none of this") }
    async fn list(&self, now: u64) -> PlaceResult<Vec<PlaceView>> { unreachable!("link_reads reads none of this") }
    async fn rows(&self) -> PlaceResult<Vec<PlaceView>> { unreachable!("link_reads reads none of this") }
    fn adds(&self) -> Vec<PlaceAddJob> { unreachable!("link_reads reads none of this") }
    async fn update(&self, place_id: &str, ask: wsp_machines::places::types::RemoveAsk) -> PlaceResult<PlaceUpdateReply> { unreachable!("link_reads reads none of this") }
    async fn holds(&self, place_id: &str) -> PlaceResult<PlaceHolds> { unreachable!("link_reads reads none of this") }
    async fn remove(&self, place_id: &str, ask: wsp_machines::places::types::RemoveAsk) -> PlaceResult<PlaceRemoved> { unreachable!("link_reads reads none of this") }
    async fn find(&self, reference: &str) -> PlaceResult<Vec<PlaceRecord>> { unreachable!("link_reads reads none of this") }
    async fn follow(&self, place_id: &str, recipe: &str) -> PlaceResult<PlaceView> { unreachable!("link_reads reads none of this") }
    async fn recipe_changed(&self, slug: &str, after_ms: u64) -> PlaceResult<Vec<String>> { unreachable!("link_reads reads none of this") }
    async fn followers(&self) -> PlaceResult<IndexMap<String, Vec<String>>> { unreachable!("link_reads reads none of this") }
    async fn skip(&self, place_id: &str, row: &str) -> PlaceResult<PlaceView> { unreachable!("link_reads reads none of this") }
    async fn setup_log(&self, place_id: &str, step: Option<PlaceSetupStep>) -> PlaceResult<Vec<String>> { unreachable!("link_reads reads none of this") }
    async fn estimate(&self, reference: &str, choices: RecipeFile) -> PlaceResult<PlaceEstimate> { unreachable!("link_reads reads none of this") }
    async fn unfollow(&self, slug: &str) -> PlaceResult<Vec<String>> { unreachable!("link_reads reads none of this") }
    async fn picks_of(&self, place_id: &str) -> PlaceResult<Option<RecipeFile>> { unreachable!("link_reads reads none of this") }
    fn on(&self, f: Rc<dyn Fn(&PlaceEvent)>) -> u64 { unreachable!("link_reads reads none of this") }
    fn off(&self, id: u64) { unreachable!("link_reads reads none of this") }
    async fn close(&self) -> PlaceResult<()> { unreachable!("link_reads reads none of this") }
}

/// The scene's views with one row mended: the fixture's `joined_row` leaves out the `default` a `PlaceView` requires.
struct Views(Rc<common::places::FakeViews>);

#[async_trait(?Send)]
impl LinkViews for Views {
    async fn pending_records(&self) -> PlaceResult<Vec<PendingRecord>> {
        self.0.pending_records().await
    }
    async fn put_pending(&self, p: PendingRecord) -> PlaceResult<()> {
        self.0.put_pending(p).await
    }
    fn flooring(&self) -> Rc<RefCell<indexmap::IndexSet<String>>> {
        self.0.flooring()
    }
    async fn floor_once(&self, pending: PendingRecord, place_id: &str) {
        self.0.floor_once(pending, place_id).await
    }
    async fn taken_back(&self, pending: PendingRecord) {
        self.0.taken_back(pending).await
    }
    fn backend_from(&self, place_id: &str, facts: BackendFacts) -> Arc<dyn MachineBackend> {
        self.0.backend_from(place_id, facts)
    }
    fn tunnelled(&self, place_id: &str, e: &DaemonEvent) -> bool {
        self.0.tunnelled(place_id, e)
    }
    fn cut(&self, place_id: &str, reason: &str) {
        self.0.cut(place_id, reason)
    }
    fn joining(&self) -> Rc<RefCell<IndexMap<String, Joining>>> {
        self.0.joining()
    }
    async fn joined(&self, place_id: &str, pending: Joining, req: PlaceProveRequest, expect: &[u8], from: &str, at: u64) -> PlaceResult<Proved> {
        self.0.joined(place_id, pending, req, expect, from, at).await
    }
    async fn joined_row(&self, record: &PlaceRecord, _marked: &str) -> PlaceResult<PlaceView> {
        Ok(serde_json::from_value(json!({ "id": record.id, "kind": "computer", "name": record.name, "default": false })).expect("a row"))
    }
}

struct Rig {
    scene: Scene,
    door: Rc<LinkDoor>,
    test: Rc<TestDoor>,
    _as_door: Rc<dyn PlaceDoor>,
}

fn rig(o: SceneOpts) -> Rig {
    let scene = Scene::new(o);
    let door = LinkDoor::new(
        Rc::clone(&scene.ctx),
        Rc::clone(&scene.records) as Rc<dyn LinkRecords>,
        Rc::clone(&scene.setup),
        Rc::new(Views(Rc::clone(&scene.views))) as Rc<dyn LinkViews>,
    );
    let test = Rc::new(TestDoor { link: Rc::downgrade(&door), ctx: Rc::clone(&scene.ctx), execs: RefCell::new(Vec::new()) });
    let as_door: Rc<dyn PlaceDoor> = test.clone();
    scene.ctx.set_door(Rc::downgrade(&as_door));
    builds(&scene);
    Rig { scene, door, test, _as_door: as_door }
}

// ---------------------------------------------------------------------------------------------------------------
// A backend to hand out, and what a daemon says about the one it forks with.

struct Free;

impl BackendPricing for Free {
    fn rate_usd_per_hour(&self, _size: Size) -> f64 {
        0.0
    }
    fn default_size(&self) -> Size {
        Size { cpu: 2.0, mem_mb: 4096.0 }
    }
    fn snapshot_storage(&self) -> SnapshotStoragePricing {
        let wire: WireSnapshotStoragePricing = serde_json::from_value(json!({ "freeGb": 0, "usdPerGbMonth": 0, "billedFrom": "" })).expect("a pricing");
        SnapshotStoragePricing { wire, free_named: None, usd_per_named_month: None }
    }
}

struct Tag {
    logins: Option<String>,
}

#[async_trait(?Send)]
impl MachineBackend for Tag {
    fn capabilities(&self) -> Capabilities {
        serde_json::from_value(facts_json(None)["capabilities"].clone()).expect("capabilities")
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
        Arc::new(Free)
    }
    fn logins(&self) -> Option<String> {
        self.logins.clone()
    }
    async fn create(&self, _spec: MachineSpec, _waiting: Option<Waiting>, _sent: Option<String>) -> EngineResult<Arc<dyn Machine>> {
        unreachable!("link_reads creates nothing")
    }
    async fn get(&self, _id: &str) -> EngineResult<Arc<dyn Machine>> {
        unreachable!("link_reads gets nothing")
    }
    async fn list(&self, _labels: Option<&IndexMap<String, String>>) -> EngineResult<Vec<MachineListRow>> {
        Ok(Vec::new())
    }
    async fn delete_snapshot(&self, _id: &str) -> EngineResult<()> {
        Ok(())
    }
}

fn tagged(tag: &str) -> Arc<dyn MachineBackend> {
    Arc::new(Tag { logins: Some(tag.to_string()) })
}

/// What `views.backend_from` builds: its tag names the place and the logins the facts carried.
fn builds(scene: &Scene) {
    *scene.views.backend.borrow_mut() = Some(Rc::new(|place: &str, facts: &BackendFacts| {
        Arc::new(Tag { logins: Some(format!("built:{place}:{}", facts.logins.clone().unwrap_or_default())) }) as Arc<dyn MachineBackend>
    }));
}

fn facts_json(logins: Option<&str>) -> Value {
    let mut v = json!({
        "offer": "docker",
        "capabilities": {
            "liveCloneForks": false, "pauseMode": "memory", "replacesMachine": true, "previewUrls": false, "signedUrls": false,
            "callbackRelay": true, "diskSnapshots": true, "images": true, "snapshotsAnyLife": false, "snapshotListing": true,
            "templates": true, "kept": false, "copies": true, "ownNetwork": true,
            "sizes": [{ "cpu": 2, "memMb": 4096, "rateUsdPerHour": 0 }],
        },
        "pricing": { "defaultSize": { "cpu": 2, "memMb": 4096 }, "snapshotStorage": { "freeGb": 0, "usdPerGbMonth": 0, "billedFrom": "" } },
        "lifecycle": { "budgets": { "wakeAttempts": 1, "daemonAnswersMs": 30000 } },
        "baseTemplates": { "sandbox": "ubuntu:24.04", "desktop": "ubuntu:24.04" },
    });
    if let Some(l) = logins {
        v["logins"] = json!(l);
    }
    v
}

fn said_facts(logins: Option<&str>) -> Map<String, Value> {
    let mut m = obj(facts_json(logins));
    m.insert("ok".into(), json!(true));
    m
}

/// A daemon answering `machine.backend` with the logins folder in `logins` at the time it is asked.
fn says_its_facts(logins: &Rc<RefCell<Option<String>>>) -> Script {
    let logins = Rc::clone(logins);
    Rc::new(move |op, _| match op {
        "machine.backend" => Reply::Ok(said_facts(logins.borrow().as_deref())),
        _ => Reply::Ok(obj(json!({ "ok": true }))),
    })
}

// ---------------------------------------------------------------------------------------------------------------
// Fixtures.

fn with(r: PlaceRecord, patch: Value) -> PlaceRecord {
    let mut v = serde_json::to_value(&r).expect("a record value");
    for (k, x) in obj(patch) {
        if x.is_null() {
            v.as_object_mut().expect("an object").remove(&k);
        } else {
            v[&k] = x;
        }
    }
    serde_json::from_value(v).expect("a record")
}

fn keep(scene: &Scene, id: &str, name: &str, over: Value, patch: Value) {
    scene.records.keep(with(record(id, name, report(name, over)), patch));
}

fn applied(rows: Value) -> Value {
    json!({ "applied": { "hash": "h1", "at": iso(T0), "rows": rows } })
}

fn settings(v: Value) -> PlaceSettings {
    serde_json::from_value(v).expect("settings")
}

fn rows_of(scene: &Scene, id: &str) -> Value {
    serde_json::to_value(scene.records.get(id).applied.expect("applied rows").rows).expect("rows")
}

fn changed_ids(scene: &Scene) -> Vec<String> {
    scene
        .events
        .borrow()
        .iter()
        .filter_map(|e| if let PlaceEvent::Changed(c) = e { Some(c.place.id.clone()) } else { None })
        .collect()
}

fn writes(scene: &Scene) -> Vec<String> {
    scene.records.writes.borrow().clone()
}

fn the_word(place_id: Option<&str>) -> PlaceFor {
    PlaceFor { place_id: place_id.map(str::to_string) }
}

fn ask() -> ForwardAsk {
    ForwardAsk { pane: None, standing: false }
}

fn tunnel_frames(socket: &FakeSocket) -> Vec<(String, Value)> {
    socket
        .reach
        .frames
        .borrow()
        .iter()
        .filter(|(op, _)| op.starts_with("tunnel."))
        .map(|(op, p)| (op.clone(), Value::Object(p.clone())))
        .collect()
}

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Waits on real sockets: the connection is the kernel's, so it moves on the wall clock.
async fn real_until(what: &str, mut ok: impl FnMut() -> bool) {
    for _ in 0..400 {
        if ok() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("never came true: {what}");
}

async fn dial(port: u16) -> TcpStream {
    TcpStream::connect(("127.0.0.1", port)).await.expect("the forward listens")
}

/// What a reader of a destroyed connection meets: the end of the stream or a reset, never bytes and never silence.
async fn gone(conn: &mut TcpStream) {
    let mut buf = [0u8; 16];
    let read = tokio::time::timeout(Duration::from_secs(2), conn.read(&mut buf)).await.expect("the connection was destroyed, not held");
    assert!(matches!(read, Ok(0) | Err(_)), "bytes came back: {read:?}");
}

// ---------------------------------------------------------------------------------------------------------------
// forking_backend

#[test]
fn the_facts_are_asked_once_and_again_on_a_new_daemon() {
    block(async {
        let r = rig(SceneOpts::default());
        let (scene, door) = (&r.scene, &r.door);
        let logins = Rc::new(RefCell::new(None::<String>));
        let socket = scene.linked("p1", "old-macbook", report("old-macbook", json!({ "daemonVersion": 16 })), says_its_facts(&logins));

        let first = door.forking_backend("p1").await.unwrap_or_else(|e| panic!("{}", e.message));
        assert_eq!(socket.reach.asked("machine.backend"), 1);
        let kept = scene.records.get("p1").backend_facts.expect("the facts are on the record");
        assert_eq!((kept.offer.as_str(), kept.logins.clone()), ("docker", None));
        assert_eq!(door.offer_of("p1").as_deref(), Some("docker"));
        assert_eq!(first.logins().as_deref(), Some("built:p1:"));
        assert!(Arc::ptr_eq(&first, scene.ctx.backends.borrow().get("p1").expect("kept in backends")));

        let same = door.forking_backend("p1").await.unwrap_or_else(|e| panic!("{}", e.message));
        assert_eq!(socket.reach.asked("machine.backend"), 1);
        assert!(Arc::ptr_eq(&first, &same));

        *logins.borrow_mut() = Some(LOGINS.to_string());
        scene.records.keep(record("p1", "old-macbook", report("old-macbook", json!({ "daemonVersion": 17 }))));
        scene.ctx.backends.borrow_mut().shift_remove("p1");
        let newer = door.forking_backend("p1").await.unwrap_or_else(|e| panic!("{}", e.message));
        assert_eq!(socket.reach.asked("machine.backend"), 2);
        assert_eq!(scene.records.get("p1").backend_facts.expect("facts again").logins.as_deref(), Some(LOGINS));
        assert_eq!(door.backend_of("p1").expect("a backend").logins().as_deref(), Some("built:p1:/wsp/logins"));
        assert_eq!(newer.logins().as_deref(), Some("built:p1:/wsp/logins"));
    });
}

#[test]
fn two_roads_asking_at_once_send_one_frame() {
    block(async {
        let r = rig(SceneOpts::default());
        let (scene, door) = (&r.scene, &r.door);
        let (tx, rx) = oneshot::channel::<Result<Map<String, Value>, String>>();
        let rx = RefCell::new(Some(rx));
        let script: Script = Rc::new(move |op, _| match op {
            "machine.backend" => match rx.borrow_mut().take() {
                Some(rx) => Reply::Later(rx),
                None => Reply::Err("asked twice".into()),
            },
            _ => Reply::Ok(obj(json!({ "ok": true }))),
        });
        let socket = scene.linked("p1", "spoo", report("spoo", json!({})), script);
        let release = async {
            until("the one read in flight", || socket.reach.asked("machine.backend") == 1 && scene.ctx.asking.borrow().len() == 1).await;
            settle().await;
            tx.send(Ok(said_facts(None))).expect("the read is waiting");
        };
        let (a, b, ()) = futures::join!(door.forking_backend("p1"), door.forking_backend("p1"), release);
        let (a, b) = (a.unwrap_or_else(|e| panic!("{}", e.message)), b.unwrap_or_else(|e| panic!("{}", e.message)));
        assert_eq!(socket.reach.asked("machine.backend"), 1);
        assert!(Arc::ptr_eq(&a, &b));
        assert!(scene.ctx.asking.borrow().is_empty());
        assert_eq!(writes(scene), vec!["p1".to_string()]);
    });
}

#[test]
fn facts_from_a_daemon_that_was_replaced_are_not_kept() {
    block(async {
        let r = rig(SceneOpts::default());
        let (scene, door) = (&r.scene, &r.door);
        let logins = Rc::new(RefCell::new(Some(LOGINS.to_string())));
        let socket = scene.linked("p1", "spoo", report("spoo", json!({ "daemonVersion": 16 })), says_its_facts(&logins));
        *scene.records.meanwhile.borrow_mut() = Some(Box::new(|r: &mut PlaceRecord| {
            *r = record("p1", "spoo", report("spoo", json!({ "daemonVersion": 17 })));
        }));

        let made = door.forking_backend("p1").await.unwrap_or_else(|e| panic!("{}", e.message));
        assert_eq!(made.logins().as_deref(), Some(LOGINS));
        assert!(scene.records.get("p1").backend_facts.is_none());
        assert!(writes(scene).is_empty());
        assert!(scene.views.built.borrow().is_empty());
        assert!(scene.ctx.backends.borrow().is_empty());
        assert!(scene.ctx.asking.borrow().is_empty());

        let again = door.forking_backend("p1").await.unwrap_or_else(|e| panic!("{}", e.message));
        assert_eq!(socket.reach.asked("machine.backend"), 2);
        assert_eq!(scene.records.get("p1").backend_facts.expect("kept now").logins.as_deref(), Some(LOGINS));
        assert_eq!(again.logins().as_deref(), Some("built:p1:/wsp/logins"));
        assert_eq!(scene.views.built.borrow().len(), 1);
    });
}

#[test]
fn backend_facts_land_on_the_record_as_it_stands() {
    block(async {
        let r = rig(SceneOpts::default());
        let (scene, door) = (&r.scene, &r.door);
        let logins = Rc::new(RefCell::new(None::<String>));
        scene.linked("p1", "spoo", report("spoo", json!({})), says_its_facts(&logins));
        *scene.records.meanwhile.borrow_mut() = Some(Box::new(|r: &mut PlaceRecord| {
            r.recipe = Some("laptop".to_string());
            r.reported_at = Some(iso(T0 + 5));
        }));

        let made = door.forking_backend("p1").await.unwrap_or_else(|e| panic!("{}", e.message));
        let now = scene.records.get("p1");
        assert_eq!(now.recipe.as_deref(), Some("laptop"));
        assert_eq!(now.reported_at, Some(iso(T0 + 5)));
        assert_eq!(now.backend_facts.as_ref().map(|f| f.offer.as_str()), Some("docker"));
        assert_eq!(writes(scene), vec!["p1".to_string()]);
        let in_kept = scene.ctx.kept.borrow().get("p1").cloned().expect("kept");
        assert_eq!(in_kept.recipe.as_deref(), Some("laptop"));
        assert_eq!(in_kept.backend_facts, now.backend_facts);
        assert!(Arc::ptr_eq(&made, scene.ctx.backends.borrow().get("p1").expect("kept in backends")));
    });
}

#[test]
fn a_word_with_no_record_and_no_provider_forks_nowhere() {
    block(async {
        let r = rig(SceneOpts::default());
        let (scene, door) = (&r.scene, &r.door);
        scene.ctx.backends.borrow_mut().insert("ghost".into(), tagged("stale"));

        let refused = door.forking_backend("ghost").await.err().expect("refused");
        assert_eq!(refused.message, "ghost is no longer a place in this wsp, so nothing forks there");
        assert_eq!(refused.kind, None);
        assert!(is_place_forks_nowhere(&refused));
        assert!(scene.ctx.backends.borrow().get("ghost").is_none());

        scene.records.providers.borrow_mut().push("box".into());
        let boat = tagged("boat");
        scene.records.provider_backends.borrow_mut().insert("box".into(), Arc::clone(&boat));
        let made = door.forking_backend("box").await.unwrap_or_else(|e| panic!("{}", e.message));
        assert!(Arc::ptr_eq(&made, &boat));
    });
}

#[test]
fn a_fork_is_refused_while_the_setup_runs_except_its_own_folder_add() {
    block(async {
        let r = rig(SceneOpts::default());
        let (scene, door) = (&r.scene, &r.door);
        let running = json!({
            "setup": { "state": "running", "addId": "a1", "startedAt": iso(T0), "steps": [{ "step": "clis", "state": "running" }], "waiting": [] },
        });
        keep(scene, "p1", "spoo", json!({}), running.clone());
        keep(scene, "p2", "attic", json!({}), json!({}));
        scene.setup.setting.borrow_mut().insert("p1".into());
        let line = "spoo is still being set up (the CLIs); wsp computers shows it, and a workspace there can be made once it is done";

        let refused = door.forking_backend("p1").await.err().expect("refused while it runs");
        assert_eq!(refused.message, line);
        assert_eq!(refused.kind, Some(Kind::Conflict));
        assert!(is_place_provisioning(&refused));

        scene.setup.setting.borrow_mut().shift_remove("p1");
        let left_running = door.forking_backend("p1").await.err().expect("a setup a stopped host left is the same busy");
        assert_eq!(left_running.message, line);

        let built = tagged("built");
        scene.ctx.backends.borrow_mut().insert("p1".into(), Arc::clone(&built));
        let through = under_folders_of("p1", door.forking_backend("p1")).await.unwrap_or_else(|e| panic!("{}", e.message));
        assert!(Arc::ptr_eq(&through, &built));
        let other = under_folders_of("p2", door.forking_backend("p1")).await.err().expect("another place's folder add does not pass");
        assert_eq!(other.message, line);

        keep(scene, "p1", "spoo", json!({}), json!({ "setup": { "state": "done", "addId": "a1", "startedAt": iso(T0), "steps": [], "waiting": [] } }));
        let done = door.forking_backend("p1").await.unwrap_or_else(|e| panic!("{}", e.message));
        assert!(Arc::ptr_eq(&done, &built));
    });
}

// ---------------------------------------------------------------------------------------------------------------
// The reads

#[test]
fn the_place_reads_match_the_ts() {
    block(async {
        let mut o = SceneOpts::default();
        o.here.shape = Some(serde_json::from_value(json!({ "cpu": 2, "memMb": 8192 })).expect("a shape"));
        let r = rig(o);
        let (scene, door) = (&r.scene, &r.door);
        keep(scene, "p1", "old-macbook", json!({}), json!({}));
        scene.records.providers.borrow_mut().push("box".into());

        assert_eq!(door.name_of("here"), "zingzys-mac");
        assert_eq!(door.name_of("p1"), "old-macbook");
        assert_eq!(door.name_of("box"), "Boat");
        assert_eq!(door.name_of("nowhere"), "nowhere");

        assert_eq!(door.settings_at("p1"), settings(json!({})));
        scene.records.settings.borrow_mut().insert("p1".into(), settings(json!({ "threads": 5 })));
        assert_eq!(door.settings_at("p1"), settings(json!({ "threads": 5 })));

        assert_eq!(door.turn_limit_at("here"), Some(None));
        assert_eq!(door.turn_limit_at("p1"), Some(None));
        assert_eq!(door.turn_limit_at("box"), Some(Some(21_600_000)));
        assert_eq!(door.turn_limit_at("nowhere"), None);
        scene.records.settings.borrow_mut().insert("p1".into(), settings(json!({ "turnLimitMs": 7_200_000 })));
        scene.records.settings.borrow_mut().insert("box".into(), settings(json!({ "turnLimitMs": 3_600_000 })));
        assert_eq!(door.turn_limit_at("p1"), Some(Some(7_200_000)));
        assert_eq!(door.turn_limit_at("box"), Some(Some(3_600_000)));
        scene.records.settings.borrow_mut().insert("box".into(), settings(json!({ "turnLimitMs": null })));
        assert_eq!(door.turn_limit_at("box"), Some(None));

        assert_eq!(door.threads_at("here"), Some(4));
        assert_eq!(door.threads_at("p1"), Some(2));
        assert_eq!(door.threads_at("box"), None);
        assert_eq!(door.threads_at("nowhere"), None);
        scene.records.settings.borrow_mut().insert("p1".into(), settings(json!({ "threads": 5 })));
        scene.records.settings.borrow_mut().insert("here".into(), settings(json!({ "threads": 7 })));
        assert_eq!(door.threads_at("p1"), Some(5));
        assert_eq!(door.threads_at("here"), Some(7));

        assert_eq!(door.machines_at("box"), Some(3));
        assert_eq!(door.machines_at("p1"), None);
        assert_eq!(door.machines_at("here"), None);
        scene.records.settings.borrow_mut().insert("box".into(), settings(json!({ "machines": 5 })));
        assert_eq!(door.machines_at("box"), Some(5));

        let bare = rig(SceneOpts::default());
        assert_eq!(bare.door.threads_at("here"), None);
    });
}

#[test]
fn the_sign_in_and_vault_reads_take_the_kept_record() {
    block(async {
        let r = rig(SceneOpts::default());
        let (scene, door) = (&r.scene, &r.door);
        keep(scene, "p1", "spoo", json!({ "agents": ["claude", "codex"], "logins": ["codex/auth.json"] }), json!({}));
        keep(scene, "p2", "attic", json!({ "agents": ["claude", "codex"] }), json!({}));

        assert_eq!(serde_json::to_value(door.sign_ins_at("p1", None)).unwrap(), json!({ "claude": "vault-key", "codex": "signed-in" }));
        assert_eq!(serde_json::to_value(door.sign_ins_at("p2", None)).unwrap(), Value::Null);
        assert_eq!(serde_json::to_value(door.sign_ins_at("nowhere", None)).unwrap(), Value::Null);

        let mut vault = IndexMap::new();
        vault.insert("CLAUDE_CODE_OAUTH_TOKEN".to_string(), "sk-ant-oat01-x".to_string());
        vault.insert("ANTHROPIC_API_KEY".to_string(), "sk-ant-api03-x".to_string());
        let picks = recipe(json!({ "name": "laptop", "agents": { "claude": { "signin": "key" } } }));
        scene.records.keep(with(scene.records.get("p1"), json!({ "picks": serde_json::to_value(picks).unwrap() })));
        let mut keyed = IndexMap::new();
        keyed.insert("ANTHROPIC_API_KEY".to_string(), "sk-ant-api03-x".to_string());
        assert_eq!(door.vault_at("p1", &vault), keyed);
        assert_eq!(door.vault_at("p2", &vault), vault);
        assert_eq!(door.vault_at("nowhere", &vault), vault);

        assert!(door.joined("p1"));
        assert!(!door.joined("here"));
        assert!(!door.joined("nowhere"));
    });
}

#[test]
fn github_from_vault_reads_the_pick_and_the_row() {
    block(async {
        let r = rig(SceneOpts::default());
        let (scene, door) = (&r.scene, &r.door);
        let github = |outcome: &str| applied(json!([{ "id": "github", "label": "GitHub", "outcome": outcome, "step": "github" }]));
        let picks = |configs: Value| json!({ "picks": { "name": "laptop", "configs": configs } });
        let put = |id: &str, picks: Value, applied: Value| {
            let mut patch = obj(picks);
            for (k, v) in obj(applied) {
                patch.insert(k, v);
            }
            keep(scene, id, id, json!({}), Value::Object(patch));
        };

        put("unset", picks(json!({ "github": {} })), github("present"));
        put("vault", picks(json!({ "github": { "signin": "vault" } })), github("present"));
        put("machine", picks(json!({ "github": { "signin": "machine" } })), github("present"));
        put("skip", picks(json!({ "github": { "signin": "skip" } })), github("present"));
        put("installed", picks(json!({ "github": { "signin": "vault" } })), github("installed"));
        put("nogithub", picks(json!({})), github("present"));
        put("norows", picks(json!({ "github": { "signin": "vault" } })), json!({}));
        keep(scene, "nopicks", "nopicks", json!({}), applied(json!([{ "id": "github", "label": "GitHub", "outcome": "present" }])));

        assert!(door.github_from_vault("unset"));
        assert!(door.github_from_vault("vault"));
        assert!(!door.github_from_vault("machine"));
        assert!(!door.github_from_vault("skip"));
        assert!(!door.github_from_vault("installed"));
        assert!(!door.github_from_vault("nogithub"));
        assert!(!door.github_from_vault("norows"));
        assert!(!door.github_from_vault("nopicks"));
        assert!(!door.github_from_vault("nowhere"));
    });
}

#[test]
fn offer_and_backend_follow_the_facts_then_the_provider() {
    block(async {
        let r = rig(SceneOpts::default());
        let (scene, door) = (&r.scene, &r.door);
        keep(scene, "p1", "spoo", json!({}), json!({ "backendFacts": facts_json(Some(LOGINS)) }));
        keep(scene, "p2", "attic", json!({}), json!({}));
        scene.records.providers.borrow_mut().push("box".into());
        scene.records.providers.borrow_mut().push("bare".into());
        let boat = tagged("boat");
        scene.records.provider_backends.borrow_mut().insert("box".into(), Arc::clone(&boat));

        assert_eq!(door.offer_of("p1").as_deref(), Some("docker"));
        assert_eq!(door.offer_of("p2"), None);
        assert_eq!(door.offer_of("box").as_deref(), Some("box"));
        assert_eq!(door.offer_of("nowhere"), None);

        let from_facts = door.backend_of("p1").expect("built from the kept facts");
        assert_eq!(from_facts.logins().as_deref(), Some("built:p1:/wsp/logins"));
        assert_eq!(scene.views.built.borrow().len(), 1);
        let again = door.backend_of("p1").expect("the built one");
        assert!(Arc::ptr_eq(&from_facts, &again));
        assert_eq!(scene.views.built.borrow().len(), 1);

        assert!(door.backend_of("p2").is_none());
        assert!(Arc::ptr_eq(&door.backend_of("box").expect("the provider's"), &boat));
        assert!(door.backend_of("bare").is_none());
        assert!(door.backend_of("nowhere").is_none());

        let prebuilt = tagged("prebuilt");
        scene.ctx.backends.borrow_mut().insert("p2".into(), Arc::clone(&prebuilt));
        assert!(Arc::ptr_eq(&door.backend_of("p2").expect("a built one wins"), &prebuilt));
    });
}

#[test]
fn the_folder_computer_is_one_while_home_and_path_stand() {
    block(async {
        let r = rig(SceneOpts::default());
        let (scene, door) = (&r.scene, &r.door);
        assert!(door.folder_computer("nowhere").is_none());
        keep(scene, "nohome", "nohome", json!({ "login": { "USER": "maya" } }), json!({}));
        assert!(door.folder_computer("nohome").is_none());

        let login = |home: &str, path: Option<&str>| {
            let mut l = json!({ "HOME": home, "USER": "maya" });
            if let Some(p) = path {
                l["PATH"] = json!(p);
            }
            l
        };
        keep(scene, "p1", "spoo", json!({ "login": login("/home/maya", Some("/usr/bin")) }), json!({}));
        let first: FolderComputer = door.folder_computer("p1").expect("a folder computer");
        assert_eq!((first.home.as_str(), first.cpu, first.mem_mb, first.tools), ("/home/maya", 4, 4096, false));
        let same = door.folder_computer("p1").expect("the same");
        assert!(Arc::ptr_eq(&first.machine, &same.machine));

        keep(scene, "p1", "spoo", json!({ "login": login("/home/maya", Some("/usr/bin:/opt/bin")) }), json!({}));
        let path_moved = door.folder_computer("p1").expect("rebuilt");
        assert!(!Arc::ptr_eq(&first.machine, &path_moved.machine));
        assert!(Arc::ptr_eq(&path_moved.machine, &door.folder_computer("p1").expect("held").machine));

        keep(scene, "p1", "spoo", json!({ "login": login("/home/maya", None) }), json!({}));
        let no_path = door.folder_computer("p1").expect("rebuilt");
        assert!(!Arc::ptr_eq(&path_moved.machine, &no_path.machine));
        keep(scene, "p1", "spoo", json!({ "login": login("/home/maya", Some("")) }), json!({}));
        assert!(Arc::ptr_eq(&no_path.machine, &door.folder_computer("p1").expect("an empty PATH is no PATH").machine));
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Landing

#[test]
fn a_login_landed_at_a_terminal_reads_signed_in() {
    block(async {
        let r = rig(SceneOpts::default());
        let (scene, door) = (&r.scene, &r.door);
        keep(scene, "p1", "srv", json!({ "agents": ["codex"], "logins": [] }), json!({}));
        assert_eq!(serde_json::to_value(door.sign_ins_at("p1", None)).unwrap(), json!({ "codex": "none" }));

        door.login_landed("p1", "codex").await.unwrap_or_else(|e| panic!("{}", e.message));
        assert_eq!(scene.records.get("p1").report.logins, Some(vec!["codex/auth.json".to_string()]));
        assert_eq!(serde_json::to_value(door.sign_ins_at("p1", None)).unwrap(), json!({ "codex": "signed-in" }));
        assert!(changed_ids(scene).is_empty());
        assert_eq!(writes(scene), vec!["p1".to_string()]);

        keep(
            scene,
            "p2",
            "box",
            json!({}),
            applied(json!([{ "id": "github", "label": "GitHub", "outcome": "skipped", "note": "skipped for now", "step": "github" }])),
        );
        door.login_landed("p2", "gh").await.unwrap_or_else(|e| panic!("{}", e.message));
        assert_eq!(
            rows_of(scene, "p2"),
            json!([{ "id": "github", "label": "GitHub", "outcome": "installed", "note": SIGNED_IN_THERE, "step": "github" }])
        );
        assert_eq!(changed_ids(scene), vec!["p2".to_string()]);
        assert_eq!(writes(scene), vec!["p1".to_string(), "p2".to_string()]);
    });
}

#[test]
fn a_row_that_did_not_land_emits_nothing() {
    block(async {
        let r = rig(SceneOpts::default());
        let (scene, door) = (&r.scene, &r.door);
        let stood = json!([
            { "id": "signins/codex", "label": "Codex", "outcome": "installed", "note": "ran", "step": "signins" },
            { "id": "signins/claude", "label": "Claude Code", "outcome": "present", "step": "signins" },
        ]);
        keep(scene, "p1", "srv", json!({}), applied(stood.clone()));
        keep(scene, "p2", "bare", json!({}), json!({}));
        let landed = |id: &str, label: Option<&str>| LandedRow {
            row: row(json!({ "id": id, "label": label.unwrap_or("x"), "outcome": "installed", "note": SIGNED_IN_THERE })),
            label: label.map(str::to_string),
        };

        door.land_sign_in("p1", landed("signins/codex", None)).await.unwrap_or_else(|e| panic!("{}", e.message));
        door.land_sign_in("p1", landed("signins/claude", None)).await.unwrap_or_else(|e| panic!("{}", e.message));
        door.land_sign_in("p1", landed("signins/opencode", None)).await.unwrap_or_else(|e| panic!("{}", e.message));
        door.land_sign_in("p2", landed("signins/codex", None)).await.unwrap_or_else(|e| panic!("{}", e.message));
        door.land_sign_in("p9", landed("signins/codex", None)).await.unwrap_or_else(|e| panic!("{}", e.message));

        assert_eq!(rows_of(scene, "p1"), stood);
        assert!(changed_ids(scene).is_empty());
        assert!(writes(scene).is_empty());

        door.land_sign_in("p1", landed("github/gh", Some("GitHub CLI"))).await.unwrap_or_else(|e| panic!("{}", e.message));
        assert_eq!(rows_of(scene, "p1").as_array().map(Vec::len), Some(3));
        assert_eq!(changed_ids(scene), vec!["p1".to_string()]);
    });
}

#[test]
fn a_landing_during_a_run_goes_through_its_lander() {
    block(async {
        let r = rig(SceneOpts::default());
        let (scene, door) = (&r.scene, &r.door);
        keep(
            scene,
            "p1",
            "srv",
            json!({}),
            applied(json!([{ "id": "github", "label": "GitHub", "outcome": "skipped", "note": "skipped for now", "step": "github" }])),
        );
        let seen: Rc<RefCell<Vec<LandedRow>>> = Rc::new(RefCell::new(Vec::new()));
        let lands = Rc::new(RefCell::new(true));
        let lander: Lander = {
            let (seen, records, lands) = (Rc::clone(&seen), Rc::clone(&scene.records), Rc::clone(&lands));
            Rc::new(move |landed: LandedRow| {
                let (seen, records, lands) = (Rc::clone(&seen), Rc::clone(&records), Rc::clone(&lands));
                boxed(async move {
                    seen.borrow_mut().push(landed);
                    if !*lands.borrow() {
                        return false;
                    }
                    let mut now = records.get("p1");
                    now.applied.as_mut().expect("applied").rows[0] =
                        held(json!({ "id": "github", "label": "GitHub", "outcome": "installed", "note": SIGNED_IN_THERE, "step": "github" }));
                    records.keep(now);
                    true
                })
            })
        };
        scene.setup.landers.borrow_mut().insert("p1".into(), lander);

        door.login_landed("p1", "gh").await.unwrap_or_else(|e| panic!("{}", e.message));
        let got = seen.borrow().clone();
        assert_eq!(got.len(), 1);
        assert_eq!((got[0].row.id.as_str(), got[0].row.note.as_deref(), got[0].label.clone()), ("github", Some(SIGNED_IN_THERE), None));
        assert_eq!(serde_json::to_value(&got[0].row).unwrap()["outcome"], "installed");
        assert!(writes(scene).is_empty());
        assert_eq!(changed_ids(scene), vec!["p1".to_string()]);

        *lands.borrow_mut() = false;
        door.login_landed("p1", "gh").await.unwrap_or_else(|e| panic!("{}", e.message));
        assert_eq!(seen.borrow().len(), 2);
        assert!(writes(scene).is_empty());
        assert_eq!(changed_ids(scene), vec!["p1".to_string()]);
    });
}

#[test]
fn logins_are_read_again_off_the_computer() {
    block(async {
        let r = rig(SceneOpts::default());
        let (scene, door, test) = (&r.scene, &r.door, &r.test);
        let stdout = Rc::new(RefCell::new(("codex/auth.json\n".to_string(), 0)));
        let script: Script = {
            let stdout = Rc::clone(&stdout);
            Rc::new(move |op, _| {
                if op != "exec" {
                    return Reply::Ok(obj(json!({ "ok": true })));
                }
                let (out, code) = stdout.borrow().clone();
                Reply::Ok(obj(json!({ "ok": true, "exitCode": code, "stdout": out, "stderr": "", "truncated": false })))
            })
        };
        let facts = json!({ "backendFacts": facts_json(Some(LOGINS)) });

        keep(scene, "nofacts", "nofacts", json!({ "logins": [] }), json!({}));
        scene.live("nofacts", Rc::clone(&script));
        door.logins_again("nofacts").await.unwrap_or_else(|e| panic!("{}", e.message));
        keep(scene, "away", "away", json!({ "logins": [] }), facts.clone());
        door.logins_again("away").await.unwrap_or_else(|e| panic!("{}", e.message));
        assert!(test.execs.borrow().is_empty());
        assert!(writes(scene).is_empty());

        let socket = scene.linked("p1", "srv", report("srv", json!({ "logins": ["notes/readme"] })), Rc::clone(&script));
        scene.records.keep(with(scene.records.get("p1"), facts.clone()));
        door.logins_again("p1").await.unwrap_or_else(|e| panic!("{}", e.message));
        let execs = test.execs.borrow().clone();
        assert_eq!(execs.len(), 1);
        assert_eq!(execs[0].0, "p1");
        assert_eq!(execs[0].1, shared_logins_script(LOGINS));
        assert_eq!(execs[0].2.timeout_ms, Some(10_000));
        assert_eq!(socket.reach.cmds(), vec![shared_logins_script(LOGINS)]);
        assert_eq!(scene.records.get("p1").report.logins, Some(vec!["codex/auth.json".to_string(), "notes/readme".to_string()]));
        assert_eq!(writes(scene), vec!["p1".to_string()]);

        *stdout.borrow_mut() = (String::new(), 0);
        door.logins_again("p1").await.unwrap_or_else(|e| panic!("{}", e.message));
        assert_eq!(scene.records.get("p1").report.logins, Some(vec!["notes/readme".to_string()]));
        assert_eq!(writes(scene).len(), 2);

        door.logins_again("p1").await.unwrap_or_else(|e| panic!("{}", e.message));
        assert_eq!(writes(scene).len(), 2);

        *stdout.borrow_mut() = ("codex/auth.json\n".to_string(), 1);
        door.logins_again("p1").await.unwrap_or_else(|e| panic!("{}", e.message));
        assert_eq!(scene.records.get("p1").report.logins, Some(vec!["notes/readme".to_string()]));

        *stdout.borrow_mut() = ("codex/auth.json\n".to_string(), 0);
        let failing: Script = Rc::new(|_, _| Reply::Err("connection lost".into()));
        scene.live("p1", failing);
        door.logins_again("p1").await.unwrap_or_else(|e| panic!("{}", e.message));
        assert_eq!(scene.records.get("p1").report.logins, Some(vec!["notes/readme".to_string()]));

        scene.live("p1", Rc::clone(&script));
        keep(scene, "listless", "listless", json!({}), facts);
        scene.live("listless", script);
        door.logins_again("listless").await.unwrap_or_else(|e| panic!("{}", e.message));
        assert_eq!(scene.records.get("listless").report.logins, None);
        assert_eq!(writes(scene).len(), 2);
    });
}

#[test]
fn a_key_put_in_the_vault_lands_on_every_computer() {
    block(async {
        let r = rig(SceneOpts::default());
        let (scene, door) = (&r.scene, &r.door);
        let claude = |outcome: &str| applied(json!([{ "id": "signins/claude", "label": "Claude Code", "outcome": outcome, "note": "waits", "step": "signins" }]));
        let pick = |way: &str| json!({ "picks": { "name": "laptop", "agents": { "claude": { "signin": way } } } });
        let with_pick = |way: &str, rows: Value| {
            let mut patch = obj(pick(way));
            for (k, v) in obj(rows) {
                patch.insert(k, v);
            }
            Value::Object(patch)
        };
        keep(scene, "p1", "one", json!({}), with_pick("vault", claude("skipped")));
        keep(scene, "p2", "two", json!({}), with_pick("key", claude("failed")));
        keep(scene, "p3", "three", json!({}), with_pick("vault", claude("installed")));
        keep(scene, "p4", "four", json!({}), pick("vault"));
        scene.vault.borrow_mut().insert("ANTHROPIC_API_KEY".into(), "sk-ant-api03-x".into());

        door.key_landed("claude").await.unwrap_or_else(|e| panic!("{}", e.message));
        assert_eq!(
            rows_of(scene, "p1"),
            json!([{ "id": "signins/claude", "label": "Claude Code", "outcome": "present", "note": "the token made on zingzys-mac", "step": "signins" }])
        );
        assert_eq!(
            rows_of(scene, "p2"),
            json!([{ "id": "signins/claude", "label": "Claude Code", "outcome": "present", "note": "the key in ~/.wsp/.env on zingzys-mac", "step": "signins" }])
        );
        assert_eq!(rows_of(scene, "p3"), json!([{ "id": "signins/claude", "label": "Claude Code", "outcome": "installed", "note": "waits", "step": "signins" }]));
        assert!(scene.records.get("p4").applied.is_none());
        assert_eq!(changed_ids(scene), vec!["p1".to_string(), "p2".to_string()]);
        assert_eq!(writes(scene), vec!["p1".to_string(), "p2".to_string()]);

        door.key_landed("codex").await.unwrap_or_else(|e| panic!("{}", e.message));
        scene.vault.borrow_mut().clear();
        door.key_landed("claude").await.unwrap_or_else(|e| panic!("{}", e.message));
        assert_eq!(changed_ids(scene).len(), 2);
        assert_eq!(writes(scene).len(), 2);
    });
}

#[test]
fn gh_there_puts_gh_on_alone_and_lands_it() {
    block(async {
        let r = rig(SceneOpts::default());
        let (scene, door) = (&r.scene, &r.door);
        let gh_row = row(json!({ "id": "github/gh", "label": "GitHub CLI", "outcome": "installed" }));
        scene.provisioner.rows.borrow_mut().insert(EngineStep::Github, vec![gh_row.clone()]);
        keep(
            scene,
            "p1",
            "spoo",
            json!({}),
            {
                let mut patch = obj(applied(json!([{ "id": "github", "label": "GitHub", "outcome": "skipped", "step": "github" }])));
                patch.insert(
                    "picks".into(),
                    json!({ "name": "laptop", "agents": { "claude": { "signin": "vault" } }, "clis": { "jq": { "via": "brew" } }, "configs": { "github": { "signin": "skip" } } }),
                );
                Value::Object(patch)
            },
        );
        scene.live("p1", answers_ok());
        let lines = Arc::new(Mutex::new(Vec::<String>::new()));
        let stage: ProvisionStage = {
            let lines = Arc::clone(&lines);
            Arc::new(move |line: &str, _, _| lines.lock().expect("lines").push(line.to_string()))
        };

        let rows = door.gh_there("p1", Arc::clone(&stage)).await.unwrap_or_else(|e| panic!("{}", e.message));
        assert_eq!(rows, vec![gh_row.clone()]);
        let only: Vec<Option<Vec<PlaceSetupStep>>> =
            scene.provisioner.planned_only.borrow().iter().map(|o| o.as_ref().map(|s| s.iter().cloned().collect())).collect();
        assert_eq!(only, vec![Some(vec![PlaceSetupStep::Github])]);
        let picked = serde_json::to_value(scene.provisioner.picked.borrow().last().expect("planned")).unwrap();
        assert_eq!(picked["configs"]["github"], json!({ "signin": "machine" }));
        assert_eq!(picked["agents"]["claude"]["signin"], "vault");
        assert!(picked["clis"].as_object().map_or(true, |c| c.is_empty()));
        assert_eq!(*scene.provisioner.ran.borrow(), vec![EngineStep::Github]);
        assert_eq!(scene.provisioner.ons.borrow().get(&EngineStep::Github).expect("run").home, "/home/maya");
        assert_eq!(*lines.lock().expect("lines"), vec!["github under way".to_string()]);
        assert_eq!(
            rows_of(scene, "p1"),
            json!([
                { "id": "github", "label": "GitHub", "outcome": "skipped", "step": "github" },
                { "id": "github/gh", "label": "GitHub CLI", "outcome": "installed", "step": "github" },
            ])
        );
        assert_eq!(changed_ids(scene), vec!["p1".to_string()]);

        let there = row(json!({ "id": "github/gh", "label": "GitHub CLI", "outcome": "present" }));
        scene.provisioner.rows.borrow_mut().insert(EngineStep::Github, vec![there.clone()]);
        assert_eq!(door.gh_there("p1", Arc::clone(&stage)).await.unwrap_or_else(|e| panic!("{}", e.message)), vec![there]);
        let failed = row(json!({ "id": "github/gh", "label": "GitHub CLI", "outcome": "failed", "note": "E: Unable to locate package gh" }));
        scene.provisioner.rows.borrow_mut().insert(EngineStep::Github, vec![failed.clone()]);
        assert_eq!(door.gh_there("p1", Arc::clone(&stage)).await.unwrap_or_else(|e| panic!("{}", e.message)), vec![failed]);
        assert_eq!(rows_of(scene, "p1").as_array().map(Vec::len), Some(2));
        assert_eq!(changed_ids(scene).len(), 1);
        assert_eq!(writes(scene).len(), 1);

        keep(scene, "bare", "attic", json!({}), applied(json!([])));
        scene.linked("bare", "attic", report("attic", json!({})), answers_ok());
        scene.provisioner.rows.borrow_mut().insert(EngineStep::Github, vec![gh_row]);
        door.gh_there("bare", stage).await.unwrap_or_else(|e| panic!("{}", e.message));
        let named = serde_json::to_value(scene.provisioner.picked.borrow().last().expect("planned")).unwrap();
        assert_eq!(named["name"], "attic");
        assert_eq!(named["configs"]["github"], json!({ "signin": "machine" }));
    });
}

#[test]
fn gh_there_answers_nothing_where_it_has_no_road_record_or_home() {
    block(async {
        let stage: ProvisionStage = Arc::new(|_: &str, _, _| {});
        let r = rig(SceneOpts::default());
        let (scene, door) = (&r.scene, &r.door);
        keep(scene, "nohome", "nohome", json!({ "login": { "USER": "maya" } }), json!({}));
        assert!(door.gh_there("nohome", Arc::clone(&stage)).await.unwrap_or_else(|e| panic!("{}", e.message)).is_empty());
        assert!(door.gh_there("nowhere", Arc::clone(&stage)).await.unwrap_or_else(|e| panic!("{}", e.message)).is_empty());
        assert!(scene.provisioner.picked.borrow().is_empty());
        assert!(scene.provisioner.ran.borrow().is_empty());

        let mut o = SceneOpts::default();
        o.provision = false;
        let unwired = rig(o);
        keep(&unwired.scene, "p1", "spoo", json!({}), json!({}));
        assert!(unwired.door.gh_there("p1", stage).await.unwrap_or_else(|e| panic!("{}", e.message)).is_empty());
        assert!(unwired.scene.provisioner.ran.borrow().is_empty());
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Forwards

#[test]
fn a_forward_carries_each_connection_as_a_tunnel() {
    block(async {
        let r = rig(SceneOpts::default());
        let (scene, door) = (&r.scene, &r.door);
        let socket = scene.linked("p1", "old-macbook", report("old-macbook", json!({})), answers_ok());

        let port = door.forward("p1", 34567, ask()).await.unwrap_or_else(|e| panic!("{}", e.message));
        assert!(port > 0);
        assert_eq!(scene.ctx.forwards.borrow().get("p1:34567").expect("held under its key").local_port, port);
        assert_eq!(door.forward("p1", 34567, ask()).await.unwrap_or_else(|e| panic!("{}", e.message)), port);
        assert_eq!(scene.ctx.forwards.borrow().len(), 1);

        let mut first = dial(port).await;
        first.write_all(b"hello").await.unwrap();
        real_until("the bytes went up", || socket.reach.asked("tunnel.write") == 1).await;
        assert_eq!(
            tunnel_frames(&socket),
            vec![
                ("tunnel.open".to_string(), json!({ "tunnelId": "p1", "port": 34567 })),
                ("tunnel.write".to_string(), json!({ "tunnelId": "p1", "data": b64(b"hello") })),
            ]
        );
        let conns = Rc::clone(&scene.ctx.forwards.borrow().get("p1:34567").unwrap().conns);
        assert_eq!(conns.borrow().keys().cloned().collect::<Vec<_>>(), vec!["p1".to_string()]);

        let mut second = dial(port).await;
        second.write_all(b"again").await.unwrap();
        real_until("the second connection's bytes", || socket.reach.asked("tunnel.write") == 2).await;
        assert_eq!(conns.borrow().keys().cloned().collect::<Vec<_>>(), vec!["p1".to_string(), "p2".to_string()]);
        assert_eq!(tunnel_frames(&socket)[2], ("tunnel.open".to_string(), json!({ "tunnelId": "p2", "port": 34567 })));
        assert_eq!(tunnel_frames(&socket)[3], ("tunnel.write".to_string(), json!({ "tunnelId": "p2", "data": b64(b"again") })));

        drop(first);
        real_until("the first tunnel closed", || socket.reach.asked("tunnel.close") == 1).await;
        assert_eq!(tunnel_frames(&socket).last().cloned(), Some(("tunnel.close".to_string(), json!({ "tunnelId": "p1" }))));
        assert_eq!(conns.borrow().keys().cloned().collect::<Vec<_>>(), vec!["p2".to_string()]);
        drop(second);
        real_until("the second tunnel closed", || socket.reach.asked("tunnel.close") == 2).await;
        assert!(conns.borrow().is_empty());
    });
}

#[test]
fn a_refused_tunnel_drops_its_connection() {
    block(async {
        let r = rig(SceneOpts::default());
        let (scene, door) = (&r.scene, &r.door);
        let script: Script = Rc::new(|op, _| if op == "tunnel.open" { Reply::Err("nothing answers on that port".into()) } else { Reply::Ok(obj(json!({ "ok": true }))) });
        let socket = scene.linked("p1", "old-macbook", report("old-macbook", json!({})), script);
        let port = door.forward("p1", 1234, ask()).await.unwrap_or_else(|e| panic!("{}", e.message));

        let mut conn = dial(port).await;
        conn.write_all(b"x").await.unwrap();
        gone(&mut conn).await;
        real_until("the close told the far side", || socket.reach.asked("tunnel.close") == 1).await;
        let ops: Vec<String> = tunnel_frames(&socket).into_iter().map(|(op, _)| op).collect();
        assert_eq!(ops, vec!["tunnel.open".to_string(), "tunnel.close".to_string()]);
        assert!(scene.ctx.forwards.borrow().get("p1:1234").unwrap().conns.borrow().is_empty());
    });
}

#[test]
fn a_failed_write_destroys_its_connection() {
    block(async {
        let r = rig(SceneOpts::default());
        let (scene, door) = (&r.scene, &r.door);
        let script: Script = Rc::new(|op, _| if op == "tunnel.write" { Reply::Err("the tunnel is gone".into()) } else { Reply::Ok(obj(json!({ "ok": true }))) });
        let socket = scene.linked("p1", "old-macbook", report("old-macbook", json!({})), script);
        let port = door.forward("p1", 1234, ask()).await.unwrap_or_else(|e| panic!("{}", e.message));

        let mut conn = dial(port).await;
        conn.write_all(b"x").await.unwrap();
        gone(&mut conn).await;
        real_until("the close told the far side", || socket.reach.asked("tunnel.close") == 1).await;
        let ops: Vec<String> = tunnel_frames(&socket).into_iter().map(|(op, _)| op).collect();
        assert_eq!(ops, vec!["tunnel.open".to_string(), "tunnel.write".to_string(), "tunnel.close".to_string()]);
        assert!(scene.ctx.forwards.borrow().get("p1:1234").unwrap().conns.borrow().is_empty());
    });
}

#[test]
fn a_connection_while_away_is_destroyed() {
    block(async {
        let r = rig(SceneOpts::default());
        let (scene, door) = (&r.scene, &r.door);
        scene.records.keep(record("p1", "old-macbook", report("old-macbook", json!({}))));
        let port = door.forward("p1", 1234, ask()).await.unwrap_or_else(|e| panic!("{}", e.message));

        let mut away = dial(port).await;
        away.write_all(b"x").await.unwrap();
        gone(&mut away).await;
        assert!(scene.ctx.forwards.borrow().get("p1:1234").unwrap().conns.borrow().is_empty());

        let socket = scene.live("p1", answers_ok());
        let mut back = dial(port).await;
        back.write_all(b"y").await.unwrap();
        real_until("the link carries it", || socket.reach.asked("tunnel.write") == 1).await;
        assert_eq!(
            tunnel_frames(&socket),
            vec![
                ("tunnel.open".to_string(), json!({ "tunnelId": "p2", "port": 1234 })),
                ("tunnel.write".to_string(), json!({ "tunnelId": "p2", "data": b64(b"y") })),
            ]
        );
    });
}

#[test]
fn a_pane_forward_opens_under_its_own_key_and_carries_tunnels() {
    block(async {
        let r = rig(SceneOpts::default());
        let (scene, door) = (&r.scene, &r.door);
        let socket = scene.linked("p1", "old-macbook", report("old-macbook", json!({})), answers_ok());
        assert!(Rc::ptr_eq(&door.pane_forwards(), &door.pane_forwards()));

        let asked = ForwardAsk { pane: Some(("w1".to_string(), "spoo".to_string())), standing: false };
        let port = door.forward("p1", 41731, asked).await.unwrap_or_else(|e| panic!("{}", e.message));
        assert!((41731..=41751).contains(&port));
        assert_eq!(scene.ctx.forwards.borrow().get("p1:open:41731").expect("held under the pane key").local_port, port);
        assert!(scene.ctx.forwards.borrow().get("p1:41731").is_none());

        let mut conn = dial(port).await;
        conn.write_all(b"GET /").await.unwrap();
        real_until("the pane's bytes went up", || socket.reach.asked("tunnel.write") == 1).await;
        assert_eq!(
            tunnel_frames(&socket),
            vec![
                ("tunnel.open".to_string(), json!({ "tunnelId": "p1", "port": 41731 })),
                ("tunnel.write".to_string(), json!({ "tunnelId": "p1", "data": b64(b"GET /") })),
            ]
        );
        let conns = Rc::clone(&scene.ctx.forwards.borrow().get("p1:open:41731").unwrap().conns);
        assert_eq!(conns.borrow().keys().cloned().collect::<Vec<_>>(), vec!["p1".to_string()]);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// The place a word names, and the marks

#[test]
fn place_for_names_the_place_a_word_means() {
    block(async {
        let r = rig(SceneOpts::default());
        let (scene, door) = (&r.scene, &r.door);
        keep(scene, "p1", "old-macbook", json!({}), json!({}));
        keep(scene, "p2", "old-macbook", json!({}), json!({}));
        keep(scene, "p3", "attic", json!({}), json!({}));
        scene.records.providers.borrow_mut().push("box".into());

        let two = door.place_for("old-macbook").await.err().expect("two by one word");
        assert_eq!(two.message, "old-macbook: this host holds 2 places by that name; name one by its id (p1, p2).");
        assert_eq!(two.kind, None);
        assert_eq!(door.place_for("p2").await.unwrap_or_else(|e| panic!("{}", e.message)), the_word(Some("p2")));
        assert_eq!(door.place_for("attic").await.unwrap_or_else(|e| panic!("{}", e.message)), the_word(Some("p3")));
        assert_eq!(door.place_for("here").await.unwrap_or_else(|e| panic!("{}", e.message)), the_word(None));
        assert_eq!(door.place_for("zingzys-mac").await.unwrap_or_else(|e| panic!("{}", e.message)), the_word(None));
        assert_eq!(door.place_for("box").await.unwrap_or_else(|e| panic!("{}", e.message)), the_word(Some("box")));
        assert_eq!(door.place_for("boat").await.unwrap_or_else(|e| panic!("{}", e.message)), the_word(Some("box")));

        *scene.records.wired.borrow_mut() = Some("box".into());
        assert_eq!(door.place_for("box").await.unwrap_or_else(|e| panic!("{}", e.message)), the_word(None));
        assert_eq!(door.place_for("Boat").await.unwrap_or_else(|e| panic!("{}", e.message)), the_word(None));

        let unknown = door.place_for("attic-server").await.err().expect("no such place");
        assert_eq!(unknown.message, "no place named attic-server; you have zingzys-mac, old-macbook, old-macbook, attic, Boat");
        assert_eq!(unknown.kind, None);
    });
}

#[test]
fn the_default_place_follows_the_mark() {
    block(async {
        let r = rig(SceneOpts::default());
        let (scene, door) = (&r.scene, &r.door);
        keep(scene, "p1", "old-macbook", json!({}), json!({}));
        scene.records.providers.borrow_mut().push("box".into());
        let mark = |to: Option<&str>| *scene.records.mark.borrow_mut() = to.map(str::to_string);

        assert_eq!(door.default_place().await.unwrap_or_else(|e| panic!("{}", e.message)), the_word(None));
        mark(Some("here"));
        assert_eq!(door.default_place().await.unwrap_or_else(|e| panic!("{}", e.message)), the_word(None));
        mark(Some("p1"));
        assert_eq!(door.default_place().await.unwrap_or_else(|e| panic!("{}", e.message)), the_word(Some("p1")));
        mark(Some("box"));
        assert_eq!(door.default_place().await.unwrap_or_else(|e| panic!("{}", e.message)), the_word(Some("box")));
        *scene.records.wired.borrow_mut() = Some("box".into());
        assert_eq!(door.default_place().await.unwrap_or_else(|e| panic!("{}", e.message)), the_word(None));
        mark(Some("p1"));
        scene.records.remove("p1");
        assert_eq!(door.default_place().await.unwrap_or_else(|e| panic!("{}", e.message)), the_word(None));
    });
}

#[test]
fn mark_used_marks_the_place_else_the_wired_provider_else_here() {
    block(async {
        let r = rig(SceneOpts::default());
        let (scene, door) = (&r.scene, &r.door);
        door.mark_used(Some("p1")).await.unwrap_or_else(|e| panic!("{}", e.message));
        door.mark_used(None).await.unwrap_or_else(|e| panic!("{}", e.message));
        *scene.records.wired.borrow_mut() = Some("box".into());
        door.mark_used(None).await.unwrap_or_else(|e| panic!("{}", e.message));
        door.mark_used(Some("p2")).await.unwrap_or_else(|e| panic!("{}", e.message));
        assert_eq!(*scene.records.marks.borrow(), vec!["p1".to_string(), "here".to_string(), "box".to_string(), "p2".to_string()]);
    });
}

#[test]
fn mark_default_if_none_marks_only_once() {
    block(async {
        let r = rig(SceneOpts::default());
        let (scene, door) = (&r.scene, &r.door);
        let doc = || scene.store.docs.borrow().get(DEFAULT_COLLECTION).and_then(|c| c.get(DEFAULT_ID)).cloned();

        door.mark_default_if_none("solari").await.unwrap_or_else(|e| panic!("{}", e.message));
        assert_eq!(doc(), Some(json!({ "placeId": "solari" })));

        scene.store.docs.borrow_mut().clear();
        *scene.records.mark.borrow_mut() = Some("box".into());
        door.mark_default_if_none("solari").await.unwrap_or_else(|e| panic!("{}", e.message));
        assert_eq!(doc(), None);
    });
}
