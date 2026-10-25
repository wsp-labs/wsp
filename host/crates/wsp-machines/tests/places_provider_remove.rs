// SPDX-License-Identifier: AGPL-3.0-only
//! Ports test/places-remove-provider.test.ts (the whole-host `remove("box")` cases) at `remove_provider`'s own seam,
//! with a fake provider written after test/stub-backend.ts. Spec: spec/places_provider_remove.md.
mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use futures::FutureExt;
use indexmap::IndexMap;
use serde_json::json;
use tokio_util::sync::CancellationToken;

use wsp_engine::errors::{EngineError, ErrorKind, Result as EResult, WspError};
use wsp_engine::labels::{OWNER_LABEL, WSP_LABEL};
use wsp_engine::machine::{
    BackendOffers, BackendPricing, Capabilities, ExecOptions, ExecResult, Machine, MachineBackend, MachineKind, MachineLife, MachineListRow,
    MachineSpec, MachineState, RunOptions, Size, SnapshotOptions, SnapshotRow, SnapshotStoragePricing, WireSnapshotStoragePricing, Waiting,
};
use wsp_machines::places::provider_remove::{remove_provider, ProviderImages, ProviderRemove};
use wsp_machines::places::types::{HeldImage, KillConfirm, PlaceKeyForgetter, PlaceResult};
use wsp_wire::types::js_number::JsInt;
use wsp_wire::types::JsNumber;
use wsp_wire::views::place_more::PlaceRemoved;
use wsp_words::exit::Kind;
use wsp_words::place_word::provider_key_name;
use wsp_words::words::places::{
    provider_images_kept_line, provider_kept_line, provider_key_refused, provider_left_words, provider_unlisted_line, LeftThere,
};
use wsp_words::words::thread::refusal_line;

use common::places::block;

const OWNER: &str = "acme-host";
const SNAPSHOT: &str = "wsp-acme-default-v1";
const STILL: &str = "Boat's key is also read from /work/acme/.env.";

// ---------------------------------------------------------------------------------------------------------------
// The provider: test/stub-backend.ts's machines and snapshots, each call counted and each answer overridable.

fn error(kind: ErrorKind, status: u16, message: &str) -> EngineError {
    EngineError::Provider(WspError { kind, status, code: None, message: message.to_string(), request_id: None, window: None, said: None })
}

fn missing() -> EngineError {
    error(ErrorKind::Missing, 404, "gone")
}

fn server_error() -> EngineError {
    error(ErrorKind::Unknown, 500, "Internal Server Error")
}

fn rate_limited() -> EngineError {
    error(ErrorKind::Concurrency, 429, "Too Many Requests")
}

#[derive(Clone)]
enum Kill {
    Takes,
    Ignores,
    Fails(EngineError),
    Slowly(Duration),
}

#[derive(Clone)]
enum SnapshotDelete {
    Takes,
    Fails(EngineError),
    Lingers,
}

#[derive(Default)]
struct Gauge {
    now: AtomicUsize,
    most: AtomicUsize,
    started: Mutex<Vec<String>>,
}

struct FakeMachine {
    id: String,
    labels: IndexMap<String, String>,
    killed: AtomicBool,
    kill: Mutex<Kill>,
    kills: AtomicUsize,
    gauge: Arc<Gauge>,
    lie: Mutex<Option<Arc<AtomicBool>>>,
}

impl FakeMachine {
    fn killed(&self) -> bool {
        self.killed.load(Ordering::SeqCst)
    }

    fn kills(&self) -> usize {
        self.kills.load(Ordering::SeqCst)
    }

    fn on_kill(&self, how: Kill) {
        *self.kill.lock().unwrap() = how;
    }
}

#[async_trait]
impl Machine for FakeMachine {
    fn id(&self) -> &str {
        &self.id
    }

    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }

    fn labels(&self) -> Option<&IndexMap<String, String>> {
        Some(&self.labels)
    }

    async fn exec(&self, _cmd: &str, _opts: ExecOptions) -> EResult<ExecResult> {
        Err(unused())
    }

    async fn run(&self, _script: &str, _opts: RunOptions) -> EResult<ExecResult> {
        Err(unused())
    }

    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> EResult<String> {
        Err(unused())
    }

    async fn pause(&self, _wait_ms: Option<u64>) -> EResult<()> {
        Err(unused())
    }

    async fn resume(&self, _signal: Option<CancellationToken>) -> EResult<()> {
        Err(unused())
    }

    async fn kill(&self) -> EResult<()> {
        self.kills.fetch_add(1, Ordering::SeqCst);
        let now = self.gauge.now.fetch_add(1, Ordering::SeqCst) + 1;
        self.gauge.most.fetch_max(now, Ordering::SeqCst);
        self.gauge.started.lock().unwrap().push(self.id.clone());
        let how = self.kill.lock().unwrap().clone();
        let answer = match how {
            Kill::Takes => {
                self.killed.store(true, Ordering::SeqCst);
                Ok(())
            }
            Kill::Ignores => Ok(()),
            Kill::Fails(e) => Err(e),
            Kill::Slowly(d) => {
                tokio::time::sleep(d).await;
                self.killed.store(true, Ordering::SeqCst);
                Ok(())
            }
        };
        self.gauge.now.fetch_sub(1, Ordering::SeqCst);
        answer
    }

    async fn state(&self) -> EResult<MachineState> {
        let lie = self.lie.lock().unwrap().clone();
        if self.killed() || lie.is_some_and(|flag| tells_once(&flag)) {
            return Ok(MachineState::Gone);
        }
        Ok(MachineState::Running)
    }

    async fn download_url(&self, _path: &str) -> EResult<String> {
        Err(unused())
    }

    async fn upload_url(&self, _path: &str) -> EResult<String> {
        Err(unused())
    }
}

fn unused() -> EngineError {
    EngineError::Other { message: "the fake provider runs nothing".to_string(), code: None }
}

fn tells_once(flag: &AtomicBool) -> bool {
    !flag.swap(true, Ordering::SeqCst)
}

type ListHook = Arc<dyn Fn(&FakeBox, usize) -> Option<EResult<Vec<MachineListRow>>> + Send + Sync>;
type GetHook = Arc<dyn Fn(&FakeBox, &str) -> Option<EngineError> + Send + Sync>;

struct FakeBox {
    machines: Mutex<Vec<Arc<FakeMachine>>>,
    snapshots: Mutex<Vec<SnapshotRow>>,
    seq: AtomicUsize,
    gauge: Arc<Gauge>,
    list_hook: Mutex<Option<ListHook>>,
    get_hook: Mutex<Option<GetHook>>,
    gone_once: Mutex<Option<(String, Arc<AtomicBool>)>>,
    snapshot_delete: Mutex<SnapshotDelete>,
    lists: AtomicUsize,
    gets: AtomicUsize,
    list_labels: Mutex<Vec<Option<IndexMap<String, String>>>>,
    deleted_snapshots: Mutex<Vec<String>>,
}

impl FakeBox {
    fn new() -> Arc<FakeBox> {
        Arc::new(FakeBox {
            machines: Mutex::new(Vec::new()),
            snapshots: Mutex::new(Vec::new()),
            seq: AtomicUsize::new(0),
            gauge: Arc::new(Gauge::default()),
            list_hook: Mutex::new(None),
            get_hook: Mutex::new(None),
            gone_once: Mutex::new(None),
            snapshot_delete: Mutex::new(SnapshotDelete::Takes),
            lists: AtomicUsize::new(0),
            gets: AtomicUsize::new(0),
            list_labels: Mutex::new(Vec::new()),
            deleted_snapshots: Mutex::new(Vec::new()),
        })
    }

    fn make(&self, owner: &str) -> Arc<FakeMachine> {
        let n = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        let labels = IndexMap::from([(WSP_LABEL.to_string(), "1".to_string()), (OWNER_LABEL.to_string(), owner.to_string())]);
        let made = Arc::new(FakeMachine {
            id: format!("m{n}"),
            labels,
            killed: AtomicBool::new(false),
            kill: Mutex::new(Kill::Takes),
            kills: AtomicUsize::new(0),
            gauge: Arc::clone(&self.gauge),
            lie: Mutex::new(None),
        });
        self.machines.lock().unwrap().push(Arc::clone(&made));
        made
    }

    fn make_ours(&self, count: usize) -> Vec<Arc<FakeMachine>> {
        (0..count).map(|_| self.make(OWNER)).collect()
    }

    fn machine(&self, id: &str) -> Option<Arc<FakeMachine>> {
        self.machines.lock().unwrap().iter().find(|m| m.id == id).cloned()
    }

    fn alive(&self) -> usize {
        self.machines.lock().unwrap().iter().filter(|m| !m.killed()).count()
    }

    fn made(&self) -> usize {
        self.machines.lock().unwrap().len()
    }

    fn rows(&self) -> Vec<MachineListRow> {
        let machines = self.machines.lock().unwrap();
        machines
            .iter()
            .filter(|m| !m.killed())
            .map(|m| MachineListRow { id: m.id.clone(), state: MachineState::Running, labels: m.labels.clone(), size: None })
            .collect()
    }

    fn snapshot_ids(&self) -> Vec<String> {
        self.snapshots.lock().unwrap().iter().map(|r| r.id.clone()).collect()
    }

    fn save_snapshot(&self, id: &str) {
        let row = SnapshotRow {
            id: id.to_string(),
            name: Some(id.to_string()),
            size_bytes: JsNumber(345_000_000.0),
            restored_bytes: None,
            created_at: None,
            parent: None,
        };
        self.snapshots.lock().unwrap().push(row);
    }

    fn on_list(&self, hook: impl Fn(&FakeBox, usize) -> Option<EResult<Vec<MachineListRow>>> + Send + Sync + 'static) {
        *self.list_hook.lock().unwrap() = Some(Arc::new(hook));
    }

    fn on_get(&self, hook: impl Fn(&FakeBox, &str) -> Option<EngineError> + Send + Sync + 'static) {
        *self.get_hook.lock().unwrap() = Some(Arc::new(hook));
    }

    fn on_snapshot_delete(&self, how: SnapshotDelete) {
        *self.snapshot_delete.lock().unwrap() = how;
    }

    fn lists(&self) -> usize {
        self.lists.load(Ordering::SeqCst)
    }

    fn gets(&self) -> usize {
        self.gets.load(Ordering::SeqCst)
    }

    fn most_kills_at_once(&self) -> usize {
        self.gauge.most.load(Ordering::SeqCst)
    }

    fn kill_starts(&self) -> Vec<String> {
        self.gauge.started.lock().unwrap().clone()
    }
}

/// The gateway's copy that never held the machine, answering one read: the first read of `m` by the backend or by
/// the handle is a 404 (or `gone`) while the machine runs on (stub-backend.ts `answersGoneOnce`).
fn answers_gone_once(boat: &FakeBox, m: &FakeMachine) {
    let lied = Arc::new(AtomicBool::new(false));
    *boat.gone_once.lock().unwrap() = Some((m.id.clone(), Arc::clone(&lied)));
    *m.lie.lock().unwrap() = Some(lied);
}

struct FakePricing;

impl BackendPricing for FakePricing {
    fn rate_usd_per_hour(&self, _size: Size) -> f64 {
        0.0
    }

    fn default_size(&self) -> Size {
        Size { cpu: 2.0, mem_mb: 4096.0 }
    }

    fn snapshot_storage(&self) -> SnapshotStoragePricing {
        SnapshotStoragePricing {
            wire: WireSnapshotStoragePricing { free_gb: JsNumber(0.0), usd_per_gb_month: JsNumber(0.0), billed_from: "2026-01-01".to_string() },
            free_named: None,
            usd_per_named_month: None,
        }
    }
}

#[async_trait]
impl MachineBackend for FakeBox {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            live_clone_forks: true,
            pause_mode: None,
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
            sizes: Vec::new(),
        }
    }

    fn pricing(&self) -> Arc<dyn BackendPricing> {
        Arc::new(FakePricing)
    }

    fn offers(&self) -> BackendOffers {
        BackendOffers { list_snapshots: true, ..BackendOffers::default() }
    }

    async fn create(&self, _spec: MachineSpec, _waiting: Option<Waiting>, _sent: Option<String>) -> EResult<Arc<dyn Machine>> {
        Err(unused())
    }

    async fn get(&self, id: &str) -> EResult<Arc<dyn Machine>> {
        self.gets.fetch_add(1, Ordering::SeqCst);
        let hook = self.get_hook.lock().unwrap().clone();
        if let Some(e) = hook.and_then(|h| h(self, id)) {
            return Err(e);
        }
        let lie = self.gone_once.lock().unwrap().clone();
        if lie.is_some_and(|(lie_id, flag)| lie_id == id && tells_once(&flag)) {
            return Err(missing());
        }
        match self.machine(id) {
            Some(m) if !m.killed() => Ok(m),
            _ => Err(missing()),
        }
    }

    async fn list(&self, labels: Option<&IndexMap<String, String>>) -> EResult<Vec<MachineListRow>> {
        let nth = self.lists.fetch_add(1, Ordering::SeqCst) + 1;
        self.list_labels.lock().unwrap().push(labels.cloned());
        let hook = self.list_hook.lock().unwrap().clone();
        if let Some(answer) = hook.and_then(|h| h(self, nth)) {
            return answer;
        }
        Ok(self.rows())
    }

    async fn delete_snapshot(&self, id: &str) -> EResult<()> {
        self.deleted_snapshots.lock().unwrap().push(id.to_string());
        let how = self.snapshot_delete.lock().unwrap().clone();
        match how {
            SnapshotDelete::Fails(e) => Err(e),
            SnapshotDelete::Lingers => Ok(()),
            SnapshotDelete::Takes => {
                self.snapshots.lock().unwrap().retain(|r| r.id != id);
                Ok(())
            }
        }
    }

    async fn list_snapshots(&self, _ids: Option<&[String]>) -> EResult<Vec<SnapshotRow>> {
        Ok(self.snapshots.lock().unwrap().clone())
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The host's side: the key forgotten and the image records, as `holdingBoat` and `imagesAt` stand them.

/// What the host's `forgetKey` saw when it ran: how many machines still stood and which snapshots were there.
struct HoldingBoat {
    boat: Arc<FakeBox>,
    still: Option<String>,
    forgot: RefCell<Vec<String>>,
    alive: Cell<Option<usize>>,
    snapshots: RefCell<Vec<String>>,
}

impl HoldingBoat {
    fn forgot(&self) -> Vec<String> {
        self.forgot.borrow().clone()
    }

    fn alive_at_forget(&self) -> Option<usize> {
        self.alive.get()
    }

    fn snapshots_at_forget(&self) -> Vec<String> {
        self.snapshots.borrow().clone()
    }
}

#[async_trait(?Send)]
impl PlaceKeyForgetter for HoldingBoat {
    async fn forget_key(&self, place_id: &str) -> PlaceResult<Option<String>> {
        self.alive.set(Some(self.boat.alive()));
        *self.snapshots.borrow_mut() = self.boat.snapshot_ids();
        self.forgot.borrow_mut().push(place_id.to_string());
        Ok(self.still.clone())
    }
}

fn done<T: 'static>(value: T) -> LocalBoxFuture<'static, PlaceResult<T>> {
    async move { Ok(value) }.boxed_local()
}

type ImageRows = Rc<RefCell<Vec<(String, HeldImage)>>>;

struct Run {
    boat: Arc<FakeBox>,
    held: Rc<HoldingBoat>,
    rows: ImageRows,
    dropped: Rc<RefCell<Vec<String>>>,
}

impl Run {
    fn new(still: Option<&str>) -> Run {
        let boat = FakeBox::new();
        let held = Rc::new(HoldingBoat {
            boat: Arc::clone(&boat),
            still: still.map(str::to_string),
            forgot: RefCell::new(Vec::new()),
            alive: Cell::new(None),
            snapshots: RefCell::new(Vec::new()),
        });
        Run { boat, held, rows: Rc::new(RefCell::new(Vec::new())), dropped: Rc::new(RefCell::new(Vec::new())) }
    }

    /// This host's image sealed at Boat, and the wired cloud's own record at another provider (imagesAt).
    fn hold_images(&self) {
        self.boat.save_snapshot(SNAPSHOT);
        self.rows.borrow_mut().push(("box".to_string(), HeldImage { id: SNAPSHOT.to_string(), template_id: None }));
        self.rows.borrow_mut().push(("solari".to_string(), HeldImage { id: "snap_solari".to_string(), template_id: None }));
    }

    fn images(&self) -> ProviderImages {
        let rows = Rc::clone(&self.rows);
        let held = move || {
            let mine: Vec<HeldImage> = rows.borrow().iter().filter(|(provider, _)| provider == "box").map(|(_, image)| image.clone()).collect();
            done(mine)
        };
        let rows = Rc::clone(&self.rows);
        let dropped = Rc::clone(&self.dropped);
        let drop = move |id: String| {
            rows.borrow_mut().retain(|(_, image)| image.id != id);
            dropped.borrow_mut().push(id);
            done(())
        };
        ProviderImages { held: Rc::new(held), drop: Rc::new(drop) }
    }

    fn record_ids(&self) -> Vec<String> {
        self.rows.borrow().iter().map(|(_, image)| image.id.clone()).collect()
    }

    async fn remove(&self, known: Vec<String>, with_images: bool) -> PlaceResult<PlaceRemoved> {
        remove_provider(ProviderRemove {
            place_id: "box".to_string(),
            backend: Arc::clone(&self.boat) as Arc<dyn MachineBackend>,
            owner: OWNER.to_string(),
            known,
            forget_key: Some(Rc::clone(&self.held) as Rc<dyn PlaceKeyForgetter>),
            confirm: Some(KillConfirm { grace_ms: Some(20), poll_ms: Some(1) }),
            images: with_images.then(|| self.images()),
        })
        .await
    }
}

fn ids(machines: &[Arc<FakeMachine>]) -> Vec<String> {
    machines.iter().map(|m| m.id.clone()).collect()
}

fn answer_of(removed: bool, deleted: i64, snapshots: Option<i64>, note: Option<String>) -> PlaceRemoved {
    PlaceRemoved { removed, took: None, swept: Vec::new(), note, deleted: Some(JsInt::from(deleted)), snapshots: snapshots.map(JsInt::from) }
}

fn left(id: &str, said: String) -> LeftThere {
    LeftThere { id: id.to_string(), said }
}

fn boat_name() -> String {
    provider_key_name("box")
}

// ---------------------------------------------------------------------------------------------------------------
// places-remove-provider.test.ts

#[test]
fn deletes_every_marked_machine_then_forgets_the_key() {
    block(async {
        let run = Run::new(None);
        let ours = run.boat.make_ours(2);
        let known = ids(&ours);
        let other = run.boat.make("another-host");
        assert_eq!(run.boat.alive(), 3);

        let answer = run.remove(known, false).await.expect("removes");
        assert_eq!(answer, answer_of(true, 2, None, None));
        assert!(ours.iter().all(|m| m.killed()));
        assert!(!other.killed(), "another host's machine on the same account is not this host's to delete");
        assert_eq!(run.held.forgot(), vec!["box"]);
        assert_eq!(run.held.alive_at_forget(), Some(1));
        assert_eq!(run.boat.list_labels.lock().unwrap()[0], Some(IndexMap::from([(OWNER_LABEL.to_string(), OWNER.to_string())])));
    });
}

#[test]
fn names_a_machine_that_stays_and_keeps_the_key() {
    block(async {
        let run = Run::new(None);
        let ours = run.boat.make_ours(2);
        let known = ids(&ours);
        let stubborn = &ours[1];
        stubborn.on_kill(Kill::Ignores);

        let answer = run.remove(known.clone(), false).await.expect("answers");
        let note = provider_kept_line(&boat_name(), 1.0, &[left(&stubborn.id, provider_left_words::running("running"))]);
        assert_eq!(answer, answer_of(false, 1, None, Some(note)));
        let said = answer.note.clone().unwrap();
        assert!(said.contains(&format!("{} (still running after three asks)", stubborn.id)));
        assert!(ours[0].killed());
        assert!(!stubborn.killed());
        assert_eq!(stubborn.kills(), 3);
        assert_eq!(run.held.forgot(), Vec::<String>::new());

        stubborn.on_kill(Kill::Takes);
        let again = run.remove(known, false).await.expect("finishes");
        assert_eq!(again, answer_of(true, 1, None, None));
        assert_eq!(run.held.forgot(), vec!["box"]);
    });
}

#[test]
fn deletes_a_known_id_the_listing_missed() {
    block(async {
        let run = Run::new(None);
        let ours = run.boat.make_ours(3);
        let known = ids(&ours);
        ours[2].killed.store(true, Ordering::SeqCst);
        let missed = ours[1].id.clone();
        run.boat.on_list(move |b, _| Some(Ok(b.rows().into_iter().filter(|r| r.id != missed).collect())));

        let answer = run.remove(known, false).await.expect("removes");
        assert_eq!(answer, answer_of(true, 2, None, None));
        assert!(ours[0].killed() && ours[1].killed());
        assert_eq!(ours[2].kills(), 0, "one already gone is not asked");
        assert_eq!(run.held.forgot(), vec!["box"]);
    });
}

#[test]
fn a_provider_with_nothing_of_ours_forgets_the_key() {
    block(async {
        let run = Run::new(None);
        let answer = run.remove(Vec::new(), false).await.expect("removes");
        assert_eq!(answer, answer_of(true, 0, None, None));
        assert_eq!(run.held.forgot(), vec!["box"]);
    });
}

#[test]
fn one_404_from_a_copy_is_not_gone() {
    block(async {
        let run = Run::new(None);
        let ours = run.boat.make_ours(2);
        let known = ids(&ours);
        answers_gone_once(&run.boat, &ours[0]);

        let answer = run.remove(known, false).await.expect("removes");
        assert_eq!(answer, answer_of(true, 2, None, None));
        assert_eq!(run.boat.alive(), 0);
        assert!(ours[0].killed(), "the machine that read gone once was still running, and is deleted");
        assert_eq!(run.held.alive_at_forget(), Some(0));
    });
}

#[test]
fn deletes_a_machine_made_after_the_first_listing() {
    block(async {
        let run = Run::new(None);
        let ours = run.boat.make_ours(1);
        let known = ids(&ours);
        run.boat.on_list(|b, nth| {
            if nth == 2 {
                b.make(OWNER);
            }
            None
        });

        let answer = run.remove(known, false).await.expect("removes");
        assert_eq!(answer, answer_of(true, 2, None, None));
        assert_eq!(run.boat.made(), 2);
        assert_eq!(run.boat.alive(), 0);
        assert_eq!(run.held.forgot(), vec!["box"]);
        assert_eq!(run.held.alive_at_forget(), Some(0));
    });
}

#[test]
fn names_a_machine_still_showing_after_the_last_listing() {
    block(async {
        let run = Run::new(None);
        let ours = run.boat.make_ours(1);
        let known = ids(&ours);
        run.boat.on_list(|b, nth| {
            if nth > 1 {
                b.make(OWNER);
            }
            None
        });

        let answer = run.remove(known, false).await.expect("answers");
        assert_eq!(run.boat.lists(), 5, "the first listing and three more each delete what they found; the fifth is named");
        let late = run.boat.machines.lock().unwrap().last().cloned().unwrap();
        let note = provider_kept_line(&boat_name(), 4.0, &[left(&late.id, provider_left_words::LATE.to_string())]);
        assert_eq!(answer, answer_of(false, 4, None, Some(note)));
        assert_eq!(late.kills(), 0);
        assert!(!late.killed());
        assert_eq!(run.held.forgot(), Vec::<String>::new());
    });
}

#[test]
fn says_a_machine_that_could_not_be_read_names_five_and_counts_the_rest() {
    block(async {
        let run = Run::new(None);
        let ours = run.boat.make_ours(7);
        let known = ids(&ours);
        run.boat.on_get(|_, _| Some(rate_limited()));

        let answer = run.remove(known.clone(), false).await.expect("answers");
        let note = answer.note.clone().expect("a note");
        assert_eq!((answer.removed, answer.deleted, answer.snapshots), (false, Some(JsInt::from(0)), None));
        assert!(note.contains(&format!("({})", provider_left_words::unread("Too Many Requests"))));
        assert!(!note.contains("would not"));
        assert!(note.starts_with("7 machines wsp made at Boat are not gone: "));
        assert!(note.contains(" and 2 more."));
        assert_eq!(known.iter().filter(|id| note.contains(id.as_str())).count(), 5);
        let expected = provider_kept_line(&boat_name(), 0.0, &named_in(&note, &known, provider_left_words::unread("Too Many Requests")));
        assert_eq!(note, expected);
        assert_eq!(run.held.forgot(), Vec::<String>::new());
    });
}

fn named_in(note: &str, known: &[String], said: String) -> Vec<LeftThere> {
    let mut named: Vec<(usize, &String)> = known.iter().filter_map(|id| note.find(&format!("{id} (")).map(|at| (at, id))).collect();
    named.sort();
    let mut rows: Vec<LeftThere> = named.into_iter().map(|(_, id)| left(id, said.clone())).collect();
    let unnamed: Vec<LeftThere> = known.iter().filter(|id| !rows.iter().any(|r| &r.id == *id)).map(|id| left(id, said.clone())).collect();
    rows.extend(unnamed);
    rows
}

#[test]
fn refuses_in_two_halves_when_the_key_is_refused() {
    block(async {
        let run = Run::new(None);
        let ours = run.boat.make_ours(1);
        let known = ids(&ours);
        run.boat.on_list(|_, _| Some(Err(error(ErrorKind::Auth, 401, "unauthorized"))));

        let err = run.remove(known, false).await.expect_err("refused");
        let refused = provider_key_refused(&boat_name(), "unauthorized");
        assert_eq!(err.message, refusal_line(&refused.said, &refused.fix));
        assert_eq!(
            err.message,
            "Boat refused the key (unauthorized), so wsp cannot see or delete the machines it made there; the key is kept. Save a working key and remove Boat again, or remove it with --forget."
        );
        assert_eq!(err.kind, Some(Kind::Auth));
        assert_eq!(err.fix.as_deref(), Some(refused.fix.as_str()));
        assert_eq!(run.held.forgot(), Vec::<String>::new());
        assert_eq!((ours[0].kills(), ours[0].killed(), run.boat.gets()), (0, false, 0), "nothing deleted and nothing read");
    });
}

#[test]
fn tells_an_unread_machine_from_an_undeleted_one() {
    block(async {
        let run = Run::new(None);
        let ours = run.boat.make_ours(2);
        let known = ids(&ours);
        let (unread, undeleted) = (&ours[0], &ours[1]);
        let unread_id = unread.id.clone();
        run.boat.on_get(move |b, id| (id == unread_id && b.machine(id).is_some_and(|m| m.killed())).then(rate_limited));
        undeleted.on_kill(Kill::Fails(server_error()));

        let answer = run.remove(known, false).await.expect("answers");
        let note = answer.note.clone().expect("a note");
        assert_eq!((answer.removed, answer.deleted), (false, Some(JsInt::from(0))));
        assert!(note.contains(&format!("{} ({})", unread.id, provider_left_words::unread("Too Many Requests"))));
        assert!(note.contains(&format!("{} ({})", undeleted.id, provider_left_words::undeleted("Internal Server Error"))));
        assert_eq!(run.held.forgot(), Vec::<String>::new());
    });
}

#[test]
fn says_a_failed_relist_with_how_many_went() {
    block(async {
        let run = Run::new(None);
        let ours = run.boat.make_ours(2);
        let known = ids(&ours);
        run.boat.on_list(|_, nth| (nth == 2).then(|| Err(server_error())));

        let answer = run.remove(known, false).await.expect("answers");
        let note = provider_unlisted_line(&boat_name(), 2.0, "Internal Server Error");
        assert_eq!(answer, answer_of(false, 2, None, Some(note)));
        assert_eq!(run.boat.alive(), 0);
        assert_eq!(run.held.forgot(), Vec::<String>::new());
    });
}

#[test]
fn answers_where_the_key_still_is() {
    block(async {
        let run = Run::new(Some(STILL));
        let ours = run.boat.make_ours(1);
        let known = ids(&ours);

        let answer = run.remove(known, false).await.expect("removes");
        assert_eq!(answer, answer_of(true, 1, None, Some(STILL.to_string())));
        assert_eq!(run.held.forgot(), vec!["box"]);
    });
}

#[test]
fn a_failed_relist_with_nothing_deleted_drops_the_count() {
    block(async {
        let run = Run::new(None);
        let ours = run.boat.make_ours(1);
        let known = ids(&ours);
        ours[0].killed.store(true, Ordering::SeqCst);
        run.boat.on_list(|_, nth| (nth == 2).then(|| Err(rate_limited())));

        let answer = run.remove(known, false).await.expect("answers");
        let note = provider_unlisted_line(&boat_name(), 0.0, "Too Many Requests");
        assert_eq!(answer, answer_of(false, 0, None, Some(note)));
        assert_eq!(
            answer.note.as_deref(),
            Some("Boat's machines could not be listed again (Too Many Requests), so one made meanwhile may still be there and the key is kept. Remove Boat again to finish.")
        );
        assert_eq!(run.held.forgot(), Vec::<String>::new());
    });
}

#[test]
fn deletes_each_image_snapshot_before_the_key() {
    block(async {
        let run = Run::new(None);
        let solari = FakeBox::new();
        solari.save_snapshot("snap_solari");
        run.hold_images();
        let ours = run.boat.make_ours(1);
        let known = ids(&ours);

        let answer = run.remove(known, true).await.expect("removes");
        assert_eq!(answer, answer_of(true, 1, Some(1), None));
        assert_eq!(run.held.forgot(), vec!["box"]);
        assert_eq!(run.held.snapshots_at_forget(), Vec::<String>::new());
        assert_eq!(run.boat.snapshot_ids(), Vec::<String>::new());
        assert_eq!(*run.boat.deleted_snapshots.lock().unwrap(), vec![SNAPSHOT.to_string()]);
        assert_eq!(*run.dropped.borrow(), vec![SNAPSHOT.to_string()]);
        assert_eq!(run.record_ids(), vec!["snap_solari".to_string()], "another provider's image keeps its record");
        assert_eq!(solari.snapshot_ids(), vec!["snap_solari".to_string()]);
        assert!(solari.deleted_snapshots.lock().unwrap().is_empty());
    });
}

#[test]
fn names_a_snapshot_that_stays_and_keeps_its_record() {
    block(async {
        let run = Run::new(None);
        run.hold_images();
        let ours = run.boat.make_ours(1);
        let known = ids(&ours);
        run.boat.on_snapshot_delete(SnapshotDelete::Fails(server_error()));

        let answer = run.remove(known.clone(), true).await.expect("answers");
        let note = provider_images_kept_line(&boat_name(), 1.0, &[left(SNAPSHOT, provider_left_words::undeleted("Internal Server Error"))]);
        assert_eq!(answer, answer_of(false, 1, None, Some(note)));
        assert!(answer.note.clone().unwrap().contains(SNAPSHOT));
        assert_eq!(run.held.forgot(), Vec::<String>::new());
        assert_eq!(run.boat.snapshot_ids(), vec![SNAPSHOT.to_string()]);
        assert!(run.dropped.borrow().is_empty());
        assert_eq!(run.record_ids(), vec![SNAPSHOT.to_string(), "snap_solari".to_string()]);

        run.boat.on_snapshot_delete(SnapshotDelete::Takes);
        let again = run.remove(known, true).await.expect("finishes");
        assert_eq!(again, answer_of(true, 0, Some(1), None));
        assert_eq!(run.boat.snapshot_ids(), Vec::<String>::new());
        assert_eq!(run.held.forgot(), vec!["box"]);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// Rules with no TS case of their own.

#[test]
fn a_snapshot_still_listed_after_its_window_is_named() {
    block(async {
        let run = Run::new(None);
        run.hold_images();
        run.boat.on_snapshot_delete(SnapshotDelete::Lingers);

        let answer = run.remove(Vec::new(), true).await.expect("answers");
        let note = provider_images_kept_line(&boat_name(), 0.0, &[left(SNAPSHOT, provider_left_words::listed(20.0))]);
        assert_eq!(answer, answer_of(false, 0, None, Some(note)));
        assert_eq!(run.record_ids(), vec![SNAPSHOT.to_string(), "snap_solari".to_string()], "a still-listed snapshot keeps its record");
        assert!(run.dropped.borrow().is_empty());
        assert_eq!(run.held.forgot(), Vec::<String>::new());
    });
}

#[test]
fn a_snapshot_the_provider_lost_is_dropped_and_counted() {
    block(async {
        let run = Run::new(None);
        run.hold_images();
        run.boat.on_snapshot_delete(SnapshotDelete::Fails(missing()));

        let answer = run.remove(Vec::new(), true).await.expect("removes");
        assert_eq!(answer, answer_of(true, 0, Some(1), None));
        assert_eq!(*run.dropped.borrow(), vec![SNAPSHOT.to_string()]);
        assert_eq!(run.record_ids(), vec!["snap_solari".to_string()]);
        assert_eq!(run.held.forgot(), vec!["box"]);
    });
}

#[test]
fn never_runs_more_than_four_deletes_at_once() {
    block(async {
        let run = Run::new(None);
        let ours = run.boat.make_ours(9);
        let known = ids(&ours);
        for m in &ours {
            m.on_kill(Kill::Slowly(Duration::from_millis(5)));
        }

        let answer = run.remove(known.clone(), false).await.expect("removes");
        assert_eq!(answer, answer_of(true, 9, None, None));
        assert_eq!(run.boat.most_kills_at_once(), 4);
        let mut first_four = run.boat.kill_starts()[..4].to_vec();
        first_four.sort();
        assert_eq!(first_four, known[..4].to_vec(), "each worker takes the next id off one queue in order");
        let mut every = run.boat.kill_starts();
        every.sort();
        let mut want = known;
        want.sort();
        assert_eq!(every, want);
    });
}

#[test]
fn a_relist_failure_after_a_left_machine_answers_the_kept_line() {
    block(async {
        let run = Run::new(None);
        let ours = run.boat.make_ours(1);
        let known = ids(&ours);
        ours[0].on_kill(Kill::Ignores);
        run.boat.on_list(|_, nth| (nth == 2).then(|| Err(server_error())));

        let answer = run.remove(known, false).await.expect("answers");
        let note = provider_kept_line(&boat_name(), 0.0, &[left(&ours[0].id, provider_left_words::running("running"))]);
        assert_eq!(answer, answer_of(false, 0, None, Some(note)));
        assert_eq!(run.held.forgot(), Vec::<String>::new());
    });
}

#[test]
fn a_first_listing_that_fails_passes_through() {
    block(async {
        let run = Run::new(None);
        let ours = run.boat.make_ours(1);
        let known = ids(&ours);
        run.boat.on_list(|_, _| Some(Err(server_error())));

        let err = run.remove(known, false).await.expect_err("fails as it came");
        assert_eq!(err.message, "Internal Server Error");
        assert_eq!((ours[0].kills(), run.boat.gets()), (0, 0));
        assert_eq!(run.held.forgot(), Vec::<String>::new());
    });
}

#[test]
fn the_answer_sets_deleted_and_never_took() {
    block(async {
        let run = Run::new(None);
        let ours = run.boat.make_ours(1);
        let known = ids(&ours);
        let answer = run.remove(known, false).await.expect("removes");
        assert_eq!(answer.took, None);
        assert_eq!(serde_json::to_value(&answer).unwrap(), json!({ "removed": true, "swept": [], "deleted": 1 }));

        let empty = Run::new(None);
        let none = empty.remove(Vec::new(), false).await.expect("removes");
        assert_eq!(none.took, None);
        assert_eq!(serde_json::to_value(&none).unwrap(), json!({ "removed": true, "swept": [], "deleted": 0 }));

        let kept = Run::new(None);
        let stays = kept.boat.make_ours(1);
        stays[0].on_kill(Kill::Ignores);
        let refused = kept.remove(ids(&stays), false).await.expect("answers");
        assert_eq!(refused.took, None);
        assert_eq!(refused.deleted, Some(JsInt::from(0)));
        assert_eq!(refused.swept, Vec::<String>::new());
    });
}
