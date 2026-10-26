// SPDX-License-Identifier: AGPL-3.0-only
//! Review of wsp-machines at 2c4f6cb46: the boot's call order against boot.ts, and the place setup's folders step.
//! Every expected log below is what `packages/runtime/src/machines/boot.ts` wrote under node for the same world (the
//! scene is named in each test; the scratch oracle is the committed boot_ready oracle's world with the same fakes).
#![allow(
    clippy::await_holding_refcell_ref,
    dead_code,
    unused_imports,
    clippy::type_complexity
)]
mod common;
#[path = "support/boot.rs"]
mod support;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::{LocalBoxFuture, Shared};
use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Value};

use common::{flush, run_local, MemStore, T0};
use support::{FakeMachine, Opts, Sight, World};
use wsp_core::ids::Ids;
use wsp_core::types::internal::{KindModule, Reopened, ThreadRecord};
use wsp_core::types::pending::{Clock, Store};
use wsp_core::types::wiring::{FoundMachine, GoneOutcome, PlaceBackends, WorkspaceRecord};
use wsp_engine::machine::{Machine, MachineBackend};
use wsp_machines::machines::boot::entries::{EntryAsk, RowAt};
use wsp_machines::machines::boot::{
    BootArea, BootConfig, BootRuntime, CardAsk, ComputerRow, HeldEnd, LimitRow, ScopedDevice,
};
use wsp_machines::machines::machines::{Entry, Row};
use wsp_wire::views::place_kind::PlaceKind;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::sub_row::SubRow;
use wsp_wire::views::workspace::{Caller, ProjectView};
use wsp_wire::wire::capabilities::WorkspaceSize;
use wsp_wire::wire::events_union::EventUnion;
use wsp_wire::wire::machine_link::MachineShape;
use wsp_words::exit::{HostError, Kind};

/// A test on one thread with a `LocalSet`, as the host runs, and a bound so a hang is a failure.
fn run<F: std::future::Future<Output = ()> + 'static>(f: F) {
    run_local(async move {
        tokio::time::timeout(std::time::Duration::from_secs(20), f)
            .await
            .expect("the test hung");
    });
}

/// What boot.ts logged for one scene, from the scratch oracle's output (see the report for its source).
fn scene(name: &str) -> Value {
    let all: Value = serde_json::from_str(SCENES).expect("the node output");
    all.as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == name)
        .unwrap_or_else(|| panic!("no scene {name}"))
        .clone()
}

/// The whole log, entry by entry in order, markers included.
fn same(name: &str, got: &[Value], want: &Value) {
    let want = want.as_array().unwrap();
    if std::env::var("SHOW").is_ok() {
        for i in 0..got.len().max(want.len()) {
            let g = got.get(i).map(|v| v.to_string()).unwrap_or_default();
            let w = want.get(i).map(|v| v.to_string()).unwrap_or_default();
            eprintln!(
                "{} {:<62} | {}",
                if g == w { " " } else { "!" },
                g.chars().take(60).collect::<String>(),
                w.chars().take(60).collect::<String>()
            );
        }
    }
    for (i, (g, w)) in got.iter().zip(want.iter()).enumerate() {
        assert_eq!(g, w, "{name}: call {i} differs from boot.ts");
    }
    assert_eq!(
        got.len(),
        want.len(),
        "{name}: the log's length differs from boot.ts; got {got:#?}"
    );
}

fn ws(id: &str) -> Value {
    json!({"id": id, "name": format!("w-{id}"), "kind": "cloud", "machineId": format!("m-{id}"), "phase": "running", "golden": "img", "project": "p1"})
}

fn row(id: &str, w: &str, o: Value) -> Value {
    let mut r = json!({"id": id, "workspaceId": w, "harness": "claude", "status": "completed", "threadId": format!("t-{id}"), "startedAt": T0 - 100_000, "turnId": format!("turn-{id}")});
    for (k, v) in o.as_object().into_iter().flatten() {
        r[k] = v.clone();
    }
    r
}

fn owner() -> Value {
    json!({"owner": {"id": {"id": "h_kept"}}, "reads": {"since": {"at": 5}}})
}

fn store_of(parts: Value) -> Value {
    let mut s = owner();
    for (k, v) in parts.as_object().into_iter().flatten() {
        s[k] = v.clone();
    }
    s
}

fn running_rows(w: &str, ids: &[&str]) -> Value {
    json!({"workspaceId": w, "threads": {}, "sessions": ids.iter().map(|i| row(i, w, json!({"status": "running"}))).collect::<Vec<_>>()})
}

/// The boot area over the crate's fake runtime, with a layer in between to make a call fail or take steps.
struct Wrap {
    inner: Rc<support::Fake>,
    failing: RefCell<Vec<&'static str>>,
    fail_remember: RefCell<Option<String>>,
    two_step: Cell<bool>,
    backend: RefCell<Option<Arc<dyn MachineBackend>>>,
    places: RefCell<Option<Rc<dyn PlaceBackends>>>,
}

impl Wrap {
    #[allow(clippy::manual_contains)]
    fn fails(&self, name: &str) -> bool {
        self.failing.borrow().iter().any(|n| *n == name)
    }
}

struct Rig {
    w: World,
    area: Rc<BootArea>,
    wrap: Rc<Wrap>,
}

fn rig(o: Opts) -> Rig {
    let w = World::ready(o);
    let wrap = Rc::new(Wrap {
        inner: w.rt.clone(),
        failing: RefCell::default(),
        fail_remember: RefCell::default(),
        two_step: Cell::new(false),
        backend: RefCell::default(),
        places: RefCell::default(),
    });
    let area = BootArea::new(
        wrap.clone() as Rc<dyn BootRuntime>,
        BootConfig {
            host_id: "host:A".into(),
        },
    );
    *w.rt.area.borrow_mut() = Rc::downgrade(&area);
    Rig { w, area, wrap }
}

/// A store that fails the calls a test names, after doing them (as the scratch oracle's store does).
struct FailStore {
    inner: RefCell<Option<Rc<dyn Store>>>,
    delete: Vec<(&'static str, &'static str)>,
    list: Vec<&'static str>,
    list_error: RefCell<Option<HostError>>,
}

impl FailStore {
    fn new(
        delete: Vec<(&'static str, &'static str)>,
        list: Vec<&'static str>,
        kind: Option<Kind>,
    ) -> Rc<Self> {
        Rc::new(FailStore {
            inner: RefCell::default(),
            delete,
            list,
            list_error: RefCell::new(Some(match kind {
                Some(k) => HostError::with_kind("store.list workspaces failed", k),
                None => HostError::new("store.list workspaces failed"),
            })),
        })
    }
    fn over(&self, w: &World) {
        *self.inner.borrow_mut() = Some(w.rt.mem.clone() as Rc<dyn Store>);
    }
    fn to(&self) -> Rc<dyn Store> {
        self.inner.borrow().clone().expect("the store is wired")
    }
}

#[async_trait(?Send)]
impl Store for FailStore {
    async fn get(&self, c: &str, id: &str) -> Result<Option<Value>, HostError> {
        self.to().get(c, id).await
    }
    async fn put(&self, c: &str, id: &str, v: Value) -> Result<(), HostError> {
        self.to().put(c, id, v).await
    }
    async fn list(&self, c: &str) -> Result<Vec<Value>, HostError> {
        let rows = self.to().list(c).await?;
        if self.list.contains(&c) {
            return Err(self.list_error.borrow().clone().unwrap());
        }
        Ok(rows)
    }
    async fn keys(&self, c: &str) -> Result<Vec<String>, HostError> {
        self.to().keys(c).await
    }
    async fn delete(&self, c: &str, id: &str) -> Result<(), HostError> {
        self.to().delete(c, id).await?;
        if self.delete.contains(&(c, id)) {
            return Err(HostError::new(format!("store.delete {c} {id} failed")));
        }
        Ok(())
    }
    async fn get_blob(&self, c: &str, id: &str) -> Result<Option<Vec<u8>>, HostError> {
        self.to().get_blob(c, id).await
    }
    async fn put_blob(&self, c: &str, id: &str, b: Vec<u8>) -> Result<(), HostError> {
        self.to().put_blob(c, id, b).await
    }
    async fn delete_blob(&self, c: &str, id: &str) -> Result<(), HostError> {
        self.to().delete_blob(c, id).await
    }
    async fn stat_blob(
        &self,
        c: &str,
        id: &str,
    ) -> Result<Option<wsp_core::types::pending::BlobMark>, HostError> {
        self.to().stat_blob(c, id).await
    }
}

#[async_trait(?Send)]
#[rustfmt::skip]
impl BootRuntime for Wrap {
    fn store(&self) -> Rc<dyn Store> { self.inner.store() }
    fn clock(&self) -> Rc<dyn Clock> { self.inner.clock() }
    fn ids(&self) -> Rc<dyn Ids> { self.inner.ids() }
    fn places(&self) -> Rc<dyn PlaceBackends> { if let Some(p) = self.places.borrow().clone() { return p; } self.inner.places() }
    fn emit(&self, event: EventUnion) -> Result<(), HostError> { self.inner.emit(event) }
    fn pass(&self, event: EventUnion) -> Result<(), HostError> { self.inner.pass(event) }
    fn warn(&self, line: &str) { self.inner.warn(line) }
    fn owner(&self) -> String { self.inner.owner() }
    fn set_owner(&self, owner: &str) { self.inner.set_owner(owner) }
    fn reads_since(&self) -> f64 { self.inner.reads_since() }
    fn set_reads_since(&self, at: f64) { self.inner.set_reads_since(at) }
    fn closed(&self) -> bool { self.inner.closed() }
    fn closing(&self) -> bool { self.inner.closing() }
    fn hostname(&self) -> String { self.inner.hostname() }
    fn local_wired(&self) -> bool { self.inner.local_wired() }
    fn live(&self) -> Rc<RefCell<IndexMap<String, Entry>>> { self.inner.live() }
    fn sessions(&self) -> Rc<RefCell<IndexMap<String, Row>>> { self.inner.sessions() }
    fn thread_records(&self) -> Rc<RefCell<IndexMap<String, ThreadRecord>>> { self.inner.thread_records() }
    fn builders(&self) -> wsp_machines::images::Builders { self.inner.builders() }
    fn projects_held(&self) -> Rc<RefCell<IndexMap<String, ProjectView>>> { self.inner.projects_held() }
    fn boot_work(&self) -> Rc< RefCell< IndexMap<String, futures::future::Shared<futures::future::LocalBoxFuture<'static, ()>>>, >, > { self.inner.boot_work() }
    fn rereading(&self) -> Rc<RefCell<IndexSet<String>>> { self.inner.rereading() }
    fn viewed_marks(&self) -> Rc<RefCell<IndexMap<String, IndexMap<String, String>>>> { self.inner.viewed_marks() }
    fn has_transcript_index(&self, workspace_id: &str) -> bool { self.inner.has_transcript_index(workspace_id) }
    fn index_pos(&self, workspace_id: &str) -> u64 { self.inner.index_pos(workspace_id) }
    fn index_children(&self, workspace_id: &str, thread_id: &str) -> Vec<serde_json::Value> { self.inner.index_children(workspace_id, thread_id) }
    fn observed(&self, machine: Arc<dyn Machine>) -> Arc<dyn Machine> { self.inner.observed(machine) }
    fn attach(&self, record: &Rc<RefCell<WorkspaceRecord>>, machine: Arc<dyn Machine>) -> Entry { self.inner.attach(record, machine) }
    async fn mark_gone(&self, entry: &Entry, machine_id: &str, reason: &str) -> Result<GoneOutcome, HostError> { let out = self.inner.mark_gone(entry, machine_id, reason).await; if self.fails("mark_gone") { return Err(HostError::new("markGone failed")); } out }
    fn idle_touch(&self, workspace_id: &str) { self.inner.idle_touch(workspace_id) }
    fn sync_daemon(&self, entry: &Entry) { self.inner.sync_daemon(entry) }
    fn backend_for(&self, record: &WorkspaceRecord) -> Arc<dyn MachineBackend> { if let Some(b) = self.backend.borrow().clone() { return b; } self.inner.backend_for(record) }
    fn module_of(&self, kind: &str) -> Rc<dyn KindModule> { self.inner.module_of(kind) }
    async fn persist(&self, record: &WorkspaceRecord) -> Result<(), HostError> { let out = self.inner.persist(record).await; if self.fails("persist") { return Err(HostError::new("persist failed")); } out }
    async fn shape_of(&self, machine: &Arc<dyn Machine>) -> Option<MachineShape> { self.inner.shape_of(machine).await }
    fn size_built(&self, shape: Option<&MachineShape>, asked: WorkspaceSize) -> WorkspaceSize { self.inner.size_built(shape, asked) }
    fn arm_grace(&self, builder_id: &str, sealed_at: &str) { self.inner.arm_grace(builder_id, sealed_at) }
    fn door_backend_of(&self, place: &str) -> Option<Arc<dyn MachineBackend>> { self.inner.door_backend_of(place) }
    async fn image_kind(&self, golden: &str) -> Result<Option<wsp_wire::views::golden_image::MachineKind>, HostError> { let out = self.inner.image_kind(golden).await; if self.fails("image_kind") { return Err(HostError::new("imageKind failed")); } out }
    fn attach_queued(&self, record: &Rc<RefCell<WorkspaceRecord>>) { self.inner.attach_queued(record) }
    async fn migrate_copies(&self) -> Result<(), HostError> { let out = self.inner.migrate_copies().await; if self.fails("migrate_copies") { return Err(HostError::new("migrateCopies failed")); } out }
    async fn place_door_load(&self) -> Result<(), HostError> { let out = self.inner.place_door_load().await; if self.fails("place_door_load") { return Err(HostError::new("placeDoorLoad failed")); } out }
    async fn git_top_of(&self, path: &str) -> Result<String, HostError> { let out = self.inner.git_top_of(path).await; if self.fails("git_top_of") { return Err(HostError::new("gitTopOf failed")); } out }
    async fn remember_project(&self, project: ProjectView) -> Result<(), HostError> { if self.fail_remember.borrow().is_none() { return self.inner.remember_project(project).await; } self.inner.log.push(json!(["rememberProject", project.id])); tokio::task::yield_now().await; self.inner.log.push(json!(["rememberProject.done", project.id])); if self.fail_remember.borrow().as_deref() == Some(project.id.as_str()) { return Err(HostError::new(format!("remember {} failed", project.id))); } Ok(()) }
    async fn setups_load(&self) -> Result<(), HostError> { let out = self.inner.setups_load().await; if self.fails("setups_load") { return Err(HostError::new("setupsLoad failed")); } out }
    async fn move_transcript(&self, record: serde_json::Value) -> Result<(), HostError> { let out = self.inner.move_transcript(record).await; if self.fails("move_transcript") { return Err(HostError::new("moveTranscript failed")); } out }
    fn move_blobs(&self) { self.inner.move_blobs() }
    async fn load_index(&self, workspace_id: &str) -> Result<(), HostError> { let out = self.inner.load_index(workspace_id).await; if self.fails("load_index") { return Err(HostError::new("loadIndex failed")); } out }
    fn wake_at(&self, thread_id: &str, until: f64) { self.inner.wake_at(thread_id, until) }
    fn resume_on_reset(&self, thread_id: &str, at: f64) { self.inner.resume_on_reset(thread_id, at) }
    fn settle_cut(&self, row: &Row, reason: &str, line: &dyn Fn(f64) -> String, stopped: Option<bool>, unreached: Option<bool>) { self.inner.settle_cut(row, reason, line, stopped, unreached) }
    async fn persist_sessions(&self, workspace_id: &str) { self.inner.persist_sessions(workspace_id).await }
    async fn turn_changes_written(&self, workspace_id: &str, thread_id: &str, turn_id: &str) -> Result<bool, HostError> { let out = self.inner.turn_changes_written(workspace_id, thread_id, turn_id).await; if self.fails("turn_changes_written") { return Err(HostError::new("turnChangesWritten failed")); } out }
    async fn read_turn_changes(&self, entry: &Entry, ask: CardAsk) -> Result<bool, HostError> { let out = self.inner.read_turn_changes(entry, ask).await; if self.fails("read_turn_changes") { return Err(HostError::new("readTurnChanges failed")); } out }
    async fn reattach(&self, row: &Row, again: bool) -> Reopened { if self.two_step.get() { let turn = row.borrow().turn_id.clone(); self.inner.log.push(json!(["reattach.open", turn])); tokio::task::yield_now().await; self.inner.log.push(json!(["reattach.read", turn])); return Reopened::Attached; } self.inner.reattach(row, again).await }
    async fn devices(&self) -> Result<Vec<ScopedDevice>, HostError> { let out = self.inner.devices().await; if self.fails("devices") { return Err(HostError::new("devices failed")); } out }
    async fn revoke_device(&self, device_id: &str) -> Result<(), HostError> { let out = self.inner.revoke_device(device_id).await; if self.fails("revoke_device") { return Err(HostError::new("revokeDevice failed")); } out }
    async fn revoke_asides(&self) -> Result<(), HostError> { let out = self.inner.revoke_asides().await; if self.fails("revoke_asides") { return Err(HostError::new("revokeAsides failed")); } out }
    async fn shell_held_devices(&self) -> Result<IndexSet<String>, HostError> { let out = self.inner.shell_held_devices().await; if self.fails("shell_held_devices") { return Err(HostError::new("shellHeldDevices failed")); } out }
    fn thread_runs(&self, thread_id: &str) -> bool { self.inner.thread_runs(thread_id) }
    async fn worktree_mounted(&self, entry: &Entry) -> Result<(), HostError> { let out = self.inner.worktree_mounted(entry).await; if self.fails("worktree_mounted") { return Err(HostError::new("worktreeMounted failed")); } out }
    async fn sweep_runs(&self, entry: &Entry) -> Result<(), HostError> { let out = self.inner.sweep_runs(entry).await; if self.fails("sweep_runs") { return Err(HostError::new("sweepRuns failed")); } out }
    async fn resume_queue(&self) -> Result<(), HostError> { let out = self.inner.resume_queue().await; if self.fails("resume_queue") { return Err(HostError::new("resumeQueue failed")); } out }
    fn notify_end(&self, end: HeldEnd) { self.inner.notify_end(end) }
    fn record(&self, event: EventUnion) { self.inner.record(event) }
    fn arm_subs(&self) { self.inner.arm_subs() }
    async fn deliver_owed(&self) -> Result<(), HostError> { let out = self.inner.deliver_owed().await; if self.fails("deliver_owed") { return Err(HostError::new("deliverOwed failed")); } out }
    async fn move_old_copies(&self) -> Result<(), HostError> { let out = self.inner.move_old_copies().await; if self.fails("move_old_copies") { return Err(HostError::new("moveOldCopies failed")); } out }
    fn arm_sweep(&self) { self.inner.arm_sweep() }
    async fn slates_ready(&self) -> Result<(), HostError> { let out = self.inner.slates_ready().await; if self.fails("slates_ready") { return Err(HostError::new("slatesReady failed")); } out }
    fn drives_thread(&self, thread_id: Option<&str>, caller: Option<&Caller>) -> bool { self.inner.drives_thread(thread_id, caller) }
    fn acts_on(&self, record: &WorkspaceRecord, caller: Option<&Caller>) -> bool { self.inner.acts_on(record, caller) }
    fn talks_to_its_tree(&self, caller: Option<&Caller>) -> bool { self.inner.talks_to_its_tree(caller) }
    fn refuse_relayed(&self, record: &WorkspaceRecord, origin: Option<&Caller>, act: Option<&str>, thread: Option<&str>) -> Result<(), HostError> { self.inner.refuse_relayed(record, origin, act, thread) }
    async fn resolve_workspace(&self, reference: &str, origin: Option<&Caller>) -> Result<String, HostError> { let out = self.inner.resolve_workspace(reference, origin).await; if self.fails("resolve_workspace") { return Err(HostError::new("resolveWorkspace failed")); } out }
    async fn place_rows(&self) -> Option<Vec<ComputerRow>> { self.inner.place_rows().await }
    async fn default_image_head(&self) -> Result<Option<String>, HostError> { let out = self.inner.default_image_head().await; if self.fails("default_image_head") { return Err(HostError::new("defaultImageHead failed")); } out }
    async fn place_for(&self, place: &str) -> Result<Option<String>, HostError> { let out = self.inner.place_for(place).await; if self.fails("place_for") { return Err(HostError::new("placeFor failed")); } out }
    fn restarts(&self) -> IndexMap<String, String> { self.inner.restarts() }
    fn stopped_behind(&self, row: &Row) -> Option<serde_json::Value> { self.inner.stopped_behind(row) }
    fn cap_wait(&self, turn_id: &str) -> Option<serde_json::Value> { self.inner.cap_wait(turn_id) }
    fn agent_pid_of(&self, run: &str) -> Option<u32> { self.inner.agent_pid_of(run) }
    fn setup_refusal(&self, workspace_id: &str, harness: &str) -> Option<String> { self.inner.setup_refusal(workspace_id, harness) }
    fn setup_place(&self, entry: &Entry) -> Option<String> { self.inner.setup_place(entry) }
    fn latest_on(&self, thread_id: &str) -> Option<SessionView> { self.inner.latest_on(thread_id) }
    fn rows_on(&self, thread_id: &str) -> Vec<SessionView> { self.inner.rows_on(thread_id) }
    fn sub_row_of(&self, record: &WorkspaceRecord) -> Option<SubRow> { self.inner.sub_row_of(record) }
    fn disk_kept_until(&self, record: &WorkspaceRecord) -> Option<f64> { self.inner.disk_kept_until(record) }
    fn place_id_of(&self, record: &WorkspaceRecord) -> Option<String> { self.inner.place_id_of(record) }
    async fn sub_read(&self, workspace_id: &str, since: f64) -> Result<(), HostError> { let out = self.inner.sub_read(workspace_id, since).await; if self.fails("sub_read") { return Err(HostError::new("subRead failed")); } out }
    fn title_rows(&self, views: Vec<SessionView>) -> Vec<SessionView> { self.inner.title_rows(views) }
    fn refresh_title(&self, view: SessionView) { self.inner.refresh_title(view) }
    fn reap_kept(&self, thread_id: &str) { self.inner.reap_kept(thread_id) }
    fn cancel_resume(&self, thread_id: &str) { self.inner.cancel_resume(thread_id) }
    fn lead_asks(&self, thread_id: &str) -> bool { self.inner.lead_asks(thread_id) }
    async fn ledger_limits(&self) -> Vec<LimitRow> { self.inner.ledger_limits().await }
    fn account_key_on(&self, entry: &Entry, harness: &str) -> String { self.inner.account_key_on(entry, harness) }
    async fn resume_turn(&self, workspace_id: &str, thread_id: &str, after_limit: f64) -> Result<(), HostError> { let out = self.inner.resume_turn(workspace_id, thread_id, after_limit).await; if self.fails("resume_turn") { return Err(HostError::new("resumeTurn failed")); } out }
}

// --- boot, scene by scene ----------------------------------------------------------------------------------------

/// boot.ts:519 and :200-223. A record the provider refused at the load is read again in the background; the TS's
/// `await Promise.resolve().then(() => sightMachine(...))` and the awaits after it put the re-read's own steps
/// (the second attach, idle.touch and syncDaemon of the record) after the whole start, and the sighting's call
/// after the card writes. The Rust ran the whole re-read before the rows were re-opened.
#[test]
fn a_held_record_is_read_again_where_boot_ts_reads_it() {
    let want = scene("a held record alone");
    run(async move {
        let w = World::ready(Opts {
            store: store_of(json!({
                "workspaces": {"b": ws("b"), "c": ws("c")},
                "sessions": {"b": running_rows("b", &["r-b1"])},
            })),
            answers: json!({"m-b": {"throws": {"message": "503 busy"}}}),
            sights: vec![Sight::Found {
                machine: "m-b".into(),
                state: "running".into(),
            }],
            sight_word: Some("sightMachine"),
            ..Opts::default()
        });
        w.area.ready().await.expect("the start");
        flush().await;
        same("a held record alone", &w.log.take(), &want["log"]);
    });
}

/// boot.ts:528. `Promise.all(rows.map(async s => ... await ctx.reattach(s, again)))` starts every call before any of
/// them has gone past its first await. `at_call` (ready.rs:70) polls each call twice inline, so a call whose own first
/// step is a yield runs its second step before the next row's call has begun.
#[test]
fn every_reattach_is_begun_before_any_has_taken_its_second_step() {
    let want = scene("two rows whose re-open takes two steps");
    run(async move {
        let r = rig(Opts {
            store: store_of(
                json!({"workspaces": {"a": ws("a")}, "sessions": {"a": running_rows("a", &["r1", "r2"])}}),
            ),
            ..Opts::default()
        });
        r.wrap.two_step.set(true);
        r.area.ready().await.expect("the start");
        flush().await;
        same(
            "two rows whose re-open takes two steps",
            &r.w.log.take(),
            &want["log"],
        );
    });
}

/// boot.ts:385. `await Promise.all(moved.map(id => store.delete(TRANSCRIPTS, id)))` starts every delete before the
/// first failure rejects the start; ready.rs:366 deletes one after another and stops at the first failure.
#[test]
fn every_moved_transcript_is_deleted_though_one_delete_fails() {
    let want = scene("a transcript delete that fails");
    run(async move {
        let fail = FailStore::new(vec![("transcripts", "w1")], vec![], None);
        let w = World::ready(Opts {
            store: store_of(
                json!({"transcripts": {"w1": {"workspaceId": "w1"}, "w2": {"workspaceId": "w2"}}}),
            ),
            store_override: Some(fail.clone()),
            ..Opts::default()
        });
        fail.over(&w);
        let said = w.area.ready().await.expect_err("the start fails");
        flush().await;
        assert_eq!(said, want["answer"]["threw"].as_str().unwrap());
        let got = w.log.take();
        let deletes = |log: &[Value]| {
            log.iter()
                .filter(|e| e[0] == "store.delete")
                .cloned()
                .collect::<Vec<_>>()
        };
        assert_eq!(
            deletes(&got),
            deletes(want["log"].as_array().unwrap()),
            "the deletes boot.ts made: {}",
            want["log"]
        );
    });
}

/// boot.ts:356-363. `Promise.all` leaves the other projects' jobs running when one rejects, so the second project's
/// write is finished; `try_join_all` (ready.rs:349) drops the job mid-write at the first failure.
#[test]
fn a_project_whose_remember_fails_does_not_stop_the_next_from_being_remembered() {
    let want = scene("a project whose remember fails beside another");
    run(async move {
        let r = rig(Opts {
            store: store_of(json!({"projects": {
                "p1": {"id": "p1", "name": "one", "computer": "here", "path": "/src/one", "source": {"kind": "folder"}},
                "p2": {"id": "p2", "name": "two", "computer": "here", "path": "/src/two", "source": {"kind": "folder"}},
            }})),
            ..Opts::default()
        });
        *r.wrap.fail_remember.borrow_mut() = Some("p1".into());
        let said = r.area.ready().await.expect_err("the start fails");
        flush().await;
        assert_eq!(said, want["answer"]["threw"].as_str().unwrap());
        let remembered = |log: &[Value]| {
            log.iter()
                .filter(|e| {
                    e[0].as_str()
                        .is_some_and(|n| n.starts_with("rememberProject"))
                })
                .cloned()
                .collect::<Vec<_>>()
        };
        assert_eq!(
            remembered(&r.w.log.take()),
            remembered(want["log"].as_array().unwrap()),
            "the remembers boot.ts made: {}",
            want["log"]
        );
    });
}

/// boot.ts:182 and :170-184. `loadLate` does not catch the re-read's failure, so a late record whose provider read
/// fails and whose machine then reads gone, with the settling failing, fails its boot work: the rows of that workspace
/// are not re-opened, not swept, and the line says so. load.rs's `load_late` drops that failure (`let _ =`, load.rs:473)
/// and the work goes on to re-open and sweep.
#[test]
fn a_late_record_whose_reread_fails_ends_its_boot_work_with_the_line() {
    let want = scene("a late record whose re-read fails to settle");
    run(async move {
        let r = rig(Opts {
            store: store_of(
                json!({"workspaces": {"a": ws("a")}, "sessions": {"a": running_rows("a", &["r-a1"])}}),
            ),
            answers: json!({"m-a": {"hang": true, "throws": {"message": "503 busy"}}}),
            sights: vec![Sight::Gone],
            sight_word: Some("sightMachine"),
            ..Opts::default()
        });
        r.wrap.failing.borrow_mut().push("mark_gone");
        let ready = r.area.ready();
        flush().await;
        r.w.clock.advance(1500);
        flush().await;
        r.w.log.push(json!(["mark", "release the read"]));
        support::release_one();
        ready.await.expect("the start");
        flush().await;
        let got = r.w.log.take();
        let steps = |log: &[Value]| {
            log.iter()
                .filter(|e| match e[0].as_str() {
                    Some("reattach" | "sweepRuns" | "syncDaemon") => true,
                    Some("warn") => e[1].as_str().is_some_and(|w| w.contains("not settled")),
                    _ => false,
                })
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
        };
        let want_log = want["log"].as_array().unwrap();
        assert_eq!(
            steps(&got),
            steps(want_log),
            "what the late record's boot work did: boot.ts logged {}",
            json!(steps(want_log))
        );
    });
}

/// boot.ts:282 with the sighting `readOnce` made: a found machine's state is read once, by the sighting, and the
/// load reads the sighting's. `FoundMachine` (wsp-core pending.rs:97) carries no state, so load.rs:667-670 reads the
/// machine again; the TS comment on :281 says why it must not ("a second read would reset the provider's idle timer").
#[test]
fn a_sighted_machine_is_not_asked_its_state_a_second_time() {
    let want = scene("a found machine with no seen state is read from the sighting");
    run(async move {
        let w = World::ready(Opts::default());
        let machine: Arc<dyn Machine> = Arc::new(FakeMachine {
            id: "m-a".into(),
            kind: wsp_wire::views::golden_image::MachineKind::Sandbox,
            labels: None,
            seen: None,
            stream_url: None,
            state: Ok(wsp_engine::machine::MachineState::Running),
            log_state: true,
        });
        let found = FoundMachine {
            sighting: wsp_core::types::pending::Sighting::default(),
            machine,
        };
        let raw = support::fill_workspace(&ws("a"));
        w.area
            .hydrate_workspace(raw, Some(found), None, None)
            .await
            .expect("hydrate");
        flush().await;
        let got: Vec<Value> = w
            .log
            .take()
            .into_iter()
            .map(|e| {
                if e[0] == "machine.state" {
                    json!(["machine.state", "m-a"])
                } else {
                    e
                }
            })
            .collect();
        same(
            "a found machine with no seen state is read from the sighting",
            &got,
            &want["log"],
        );
    });
}

/// boot.ts:420. `held.resumeAt ?? held.cutOwed` takes `resumeAt` whenever it is there, so a `resumeAt` that is no
/// string leaves no `cutOwed` at all; ready.rs:459-465 falls through to `cutOwed`.
#[test]
fn a_resume_at_that_is_no_string_hides_the_cut_owed_as_it_does_in_boot_ts() {
    let want = scene("a thread record read back");
    run(async move {
        let w = World::ready(Opts {
            store: store_of(json!({
                "workspaces": {"a": ws("a")},
                "sessions": {"a": {"workspaceId": "a", "threads": {"t-1": {"harness": "claude", "resumeAt": 5, "cutOwed": "x"}, "t-2": {"harness": "claude", "cutOwed": "y"}}, "sessions": []}},
            })),
            ..Opts::default()
        });
        w.area.ready().await.expect("the start");
        flush().await;
        assert_eq!(
            support::norm(w.rt.thread_records_json()),
            want["threadRecords"]
        );
    });
}

/// boot.ts:233-247. A queued record is attached as queued before the kind is asked for a module; load.rs:567-577
/// asks the kind first, so a queued record of a kind this host has no module for is left as it was with a warning.
#[test]
fn a_queued_record_of_an_unwired_kind_is_attached_queued() {
    let want = scene("an unknown kind that is queued");
    run(async move {
        let w = World::ready(Opts::default());
        let mut raw = ws("a");
        raw["kind"] = json!("unwired");
        raw["phase"] = json!("queued");
        w.area
            .hydrate_workspace(raw, None, None, None)
            .await
            .expect("hydrate");
        flush().await;
        same(
            "an unknown kind that is queued",
            &w.log.take(),
            &want["log"],
        );
    });
}

/// boot.ts:329 and :628. The start's own error is what every verb that waits on it throws, kind and all
/// (`hydrated` is the one promise). ready.rs:89 keeps only the message, so a failed start reaches every verb with no kind
/// and the exit code is no longer the class the failure was born with (exit.ts).
#[test]
fn a_failed_start_keeps_the_kind_of_its_error_for_every_verb() {
    let want = scene("a start that fails with a stamped error");
    run(async move {
        let fail = FailStore::new(vec![], vec!["workspaces"], Some(Kind::Auth));
        let w = World::ready(Opts {
            store: owner(),
            store_override: Some(fail.clone()),
            ..Opts::default()
        });
        fail.over(&w);
        let said = w
            .area
            .entry_of("a", None, EntryAsk::default())
            .await
            .err()
            .expect("the start failed");
        assert_eq!(said.message, want["answer"]["threw"].as_str().unwrap());
        assert_eq!(
            said.kind,
            Some(Kind::Auth),
            "boot.ts throws the error it was born with: kind {}",
            want["answer"]["kind"]
        );
    });
}

/// ready.rs:93 and rows.rs:452. The start and every row push are `spawn_local` tasks: a caller that is on a plain
/// runtime with no `LocalSet` panics at its first verb, and nothing in the crate says a `LocalSet` is needed.
#[test]
fn the_start_runs_on_a_plain_runtime_without_a_local_set() {
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let w = World::ready(Opts::default());
            w.area.ready().await
        })
    }));
    assert!(caught.is_ok(), "ready() panicked outside a LocalSet");
}

/// boot.ts:349-352 reads every project row as it stands (`raw as ProjectView`, no check), and the committed boot_ready
/// oracle's own rows carry no `remote`, `defaultBranch`, `memoryKey`, `memoryDir` or `createdAt`, which the typed
/// `ProjectView` requires (the crate's test world fills them: support/boot.rs `fill_project`). ready.rs:317 `parse(raw)?`
/// fails the whole start on the first such row; the TS holds it and goes on.
#[test]
fn a_project_row_missing_a_key_the_type_requires_is_held_as_it_stands() {
    run(async {
        let w = World::ready(Opts {
            store: owner(),
            ..Opts::default()
        });
        w.rt.mem.rows.borrow_mut().entry("projects".to_string()).or_default().insert(
            "p1".to_string(),
            json!({"id": "p1", "name": "site", "computer": "here", "path": "/src/site", "source": {"kind": "folder"}}),
        );
        let started = w.area.ready().await;
        assert_eq!(
            started,
            Ok(()),
            "the start refused a stored project row boot.ts holds"
        );
    });
}

/// boot.ts:605-613. The builders' reads start with the load and go on in the background past its one deadline: a read
/// that lands later admits its builder then (and arms its grace). `settle_by` (ready.rs:51-66) races the reads against
/// the deadline and drops them when it wins; `admit_once`'s `Shared` read (load.rs:290) is then polled by nobody until
/// some later refresh asks for the same builder, so the builder is not held and a sealed one's grace is not armed.
#[test]
fn a_builder_whose_read_lands_after_the_deadline_is_admitted_when_it_lands() {
    let want = scene("a builder whose read is still out at the deadline");
    run(async move {
        let w = World::ready(Opts {
            store: store_of(json!({"builders": {"slow": {
                "id": "slow", "name": "b-slow", "kind": "sandbox", "baseTemplate": "base", "setupSha": "sha",
                "createdAt": "2026-09-01T00:00:00.000Z", "firstLife": true, "size": {"cpu": 1, "memMb": 1},
                "sealed": {"version": 3, "at": "2026-09-21T13:56:40.000Z"},
            }}})),
            answers: json!({"slow": {"hang": true}}),
            ..Opts::default()
        });
        let ready = w.area.ready();
        flush().await;
        w.clock.advance(1500);
        flush().await;
        ready.await.expect("the start");
        w.log.push(json!(["mark", "ready answered"]));
        support::release_one();
        flush().await;
        let held: Vec<String> = w.rt.builders.borrow().keys().cloned().collect();
        w.log.push(json!(["mark", "builders held", held]));
        same(
            "a builder whose read is still out at the deadline",
            &w.log.take(),
            &want["log"],
        );
    });
}

/// A provider whose `get` finds the machine and whose `state` then answers 404 (the gateway copies that disagree call
/// to call, golden.ts:96-98, are why a gone machine is read twelve times).
struct MissingState {
    log: common::Log,
    get_fails: bool,
}

/// Every place of the world answers with the one provider.
struct OnePlace(Arc<dyn MachineBackend>);

impl PlaceBackends for OnePlace {
    fn wired(&self) -> String {
        "solari".into()
    }
    fn backend(&self, _place: &str) -> Option<Arc<dyn MachineBackend>> {
        Some(self.0.clone())
    }
    fn list(&self) -> Vec<String> {
        vec!["solari".into()]
    }
}

fn sight_scene(name: &str) -> Value {
    let all: Value = serde_json::from_str(SIGHT).expect("the node output");
    all.as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == name)
        .unwrap_or_else(|| panic!("no scene {name}"))
        .clone()
}

struct MissingStateMachine {
    id: String,
    log: common::Log,
}

#[async_trait(?Send)]
impl Machine for MissingStateMachine {
    fn id(&self) -> &str {
        &self.id
    }
    fn kind(&self) -> wsp_wire::views::golden_image::MachineKind {
        wsp_wire::views::golden_image::MachineKind::Sandbox
    }
    fn stream_url(&self) -> Option<&str> {
        None
    }
    fn labels(&self) -> Option<&IndexMap<String, String>> {
        None
    }
    fn seen(&self) -> Option<&wsp_engine::machine::Seen> {
        None
    }
    async fn exec(
        &self,
        _cmd: &str,
        _opts: wsp_engine::machine::ExecOptions,
    ) -> wsp_engine::errors::Result<wsp_engine::machine::ExecResult> {
        unimplemented!()
    }
    async fn run(
        &self,
        _script: &str,
        _opts: wsp_engine::machine::RunOptions,
    ) -> wsp_engine::errors::Result<wsp_engine::machine::ExecResult> {
        unimplemented!()
    }
    async fn snapshot(
        &self,
        _name: &str,
        _life: wsp_engine::machine::MachineLife,
        _opts: wsp_engine::machine::SnapshotOptions,
    ) -> wsp_engine::errors::Result<String> {
        unimplemented!()
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> wsp_engine::errors::Result<()> {
        unimplemented!()
    }
    async fn resume(&self, _signal: Option<&wsp_engine::Cancel>) -> wsp_engine::errors::Result<()> {
        unimplemented!()
    }
    async fn kill(&self) -> wsp_engine::errors::Result<()> {
        unimplemented!()
    }
    async fn state(&self) -> wsp_engine::errors::Result<wsp_engine::machine::MachineState> {
        self.log.push(json!(["machine.state", self.id]));
        Err(wsp_engine::errors::EngineError::Provider(
            wsp_engine::errors::WspError {
                kind: wsp_engine::errors::ErrorKind::Missing,
                status: 404,
                code: None,
                message: "no such machine".into(),
                request_id: None,
                window: None,
                said: None,
            },
        ))
    }
    async fn download_url(&self, _path: &str) -> wsp_engine::errors::Result<String> {
        unimplemented!()
    }
    async fn upload_url(&self, _path: &str) -> wsp_engine::errors::Result<String> {
        unimplemented!()
    }
}

struct NoPricing;

impl wsp_engine::machine::BackendPricing for NoPricing {
    fn rate_usd_per_hour(&self, _size: wsp_engine::machine::Size) -> f64 {
        0.0
    }
    fn default_size(&self) -> wsp_engine::machine::Size {
        wsp_engine::machine::Size {
            cpu: 1.0,
            mem_mb: 2048.0,
        }
    }
    fn snapshot_storage(&self) -> wsp_engine::machine::SnapshotStoragePricing {
        unimplemented!()
    }
}

#[async_trait(?Send)]
impl MachineBackend for MissingState {
    fn capabilities(&self) -> wsp_engine::machine::Capabilities {
        serde_json::from_value(json!({
            "liveCloneForks": false, "replacesMachine": false, "previewUrls": false, "signedUrls": false, "callbackRelay": false,
            "diskSnapshots": false, "images": false, "snapshotsAnyLife": false, "snapshotListing": false, "templates": false,
            "kept": false, "copies": false, "ownNetwork": false, "sizes": []
        }))
        .expect("capabilities")
    }
    fn pricing(&self) -> Arc<dyn wsp_engine::machine::BackendPricing> {
        Arc::new(NoPricing)
    }
    async fn create(
        &self,
        _spec: wsp_engine::machine::MachineSpec,
        _waiting: Option<wsp_engine::machine::Waiting>,
        _sent: Option<String>,
    ) -> wsp_engine::errors::Result<Arc<dyn Machine>> {
        unimplemented!()
    }
    async fn get(&self, id: &str) -> wsp_engine::errors::Result<Arc<dyn Machine>> {
        self.log.push(json!(["backend.get", id]));
        if self.get_fails {
            return Err(wsp_engine::errors::EngineError::Provider(
                wsp_engine::errors::WspError {
                    kind: wsp_engine::errors::ErrorKind::Missing,
                    status: 404,
                    code: None,
                    message: "no such machine".into(),
                    request_id: None,
                    window: None,
                    said: None,
                },
            ));
        }
        Ok(Arc::new(MissingStateMachine {
            id: id.to_string(),
            log: self.log.clone(),
        }))
    }
    async fn list(
        &self,
        _labels: Option<&IndexMap<String, String>>,
    ) -> wsp_engine::errors::Result<Vec<wsp_engine::machine::MachineListRow>> {
        unimplemented!()
    }
    async fn delete_snapshot(&self, _id: &str) -> wsp_engine::errors::Result<()> {
        unimplemented!()
    }
}

/// golden.ts:130-134. `readOnce` reads a 404 from the machine's `state()` as the machine gone, so twelve such reads in a
/// row settle a held record gone. load.rs:82-102 puts `?` on `machine.state()`, so the first 404 is a failure, the
/// re-read ends there and the record stays held for good.
#[test]
fn a_machine_whose_state_answers_404_is_read_gone_twelve_times_then_settled() {
    let want = sight_scene("a held record whose provider answers 404 to state");
    run(async move {
        let r = rig(Opts {
            store: store_of(json!({"workspaces": {"a": ws("a")}})),
            ..Opts::default()
        });
        *r.wrap.backend.borrow_mut() = Some(Arc::new(MissingState {
            log: r.w.log.clone(),
            get_fails: false,
        }));
        r.area
            .hydrate_workspace(
                support::fill_workspace(&ws("a")),
                None,
                None,
                Some(support::asked_err("503 busy")),
            )
            .await
            .expect("hydrate");
        r.w.log.clear();
        r.area
            .reread_held("a", wsp_words::words::thread::GoneSeenBy::Sweep)
            .await
            .expect("reread");
        flush().await;
        same(
            "a machine whose state answers 404",
            &r.w.log.take(),
            &want["log"],
        );
    });
}

/// golden.ts:138-144, boot.ts:206-218. A record is settled gone only once the provider has answered gone twelve reads in
/// a row (GONE_READS; "seven of eleven machines wsp had read gone once kept running"). load.rs:97 answers gone at the
/// first 404 of `get`, so one 404 from a gateway copy that never held the machine marks the workspace gone while its
/// machine goes on running and billing.
#[test]
fn a_held_record_is_settled_gone_only_after_twelve_404s_in_a_row() {
    let want = sight_scene("a held record whose provider answers 404 to get");
    run(async move {
        let r = rig(Opts {
            store: store_of(json!({"workspaces": {"a": ws("a")}})),
            ..Opts::default()
        });
        *r.wrap.backend.borrow_mut() = Some(Arc::new(MissingState {
            log: r.w.log.clone(),
            get_fails: true,
        }));
        r.area
            .hydrate_workspace(
                support::fill_workspace(&ws("a")),
                None,
                None,
                Some(support::asked_err("503 busy")),
            )
            .await
            .expect("hydrate");
        r.w.log.clear();
        r.area
            .reread_held("a", wsp_words::words::thread::GoneSeenBy::Sweep)
            .await
            .expect("reread");
        flush().await;
        let got = r.w.log.take();
        let gets = |log: &[Value]| log.iter().filter(|e| e[0] == "backend.get").count();
        assert_eq!(
            gets(&got),
            gets(want["log"].as_array().unwrap()),
            "provider reads before the record was settled gone (boot.ts: twelve); Rust log {got:?}"
        );
        same(
            "a held record whose provider answers 404 to get",
            &got,
            &want["log"],
        );
    });
}

/// golden.ts:147 `readGone`, boot.ts:92. A stored builder is dropped from the store only once `readGone` has read it
/// gone twelve times in a row after the load's own 404; load.rs:220 (`sight`, one 404 answers gone) drops its record
/// after one more read, so a builder machine a gateway copy had not heard of loses its record and keeps billing.
#[test]
fn a_builder_record_is_deleted_only_after_twelve_more_404s() {
    let want = sight_scene("a builder whose provider answers 404 to get");
    run(async move {
        let r = rig(Opts {
            store: store_of(
                json!({"builders": {"b1": {"id": "b1", "name": "b-b1", "kind": "sandbox", "baseTemplate": "base", "setupSha": "sha", "createdAt": "2026-09-01T00:00:00.000Z", "firstLife": true, "size": {"cpu": 1, "memMb": 1}}}}),
            ),
            ..Opts::default()
        });
        let backend: Arc<dyn MachineBackend> = Arc::new(MissingState {
            log: r.w.log.clone(),
            get_fails: true,
        });
        *r.wrap.places.borrow_mut() = Some(Rc::new(OnePlace(backend)));
        r.w.log.clear();
        r.area.refresh_builders().await.expect("refresh");
        flush().await;
        let reads = |log: &[Value]| {
            log.iter()
                .filter(|e| e[0] == "backend.get" || e[0] == "store.delete")
                .cloned()
                .collect::<Vec<_>>()
        };
        assert_eq!(
            reads(&r.w.log.take()),
            reads(want["log"].as_array().unwrap()),
            "the reads boot.ts made before it deleted the record"
        );
    });
}

// --- the place setup's folders step ------------------------------------------------------------------------------

mod folders {
    use super::*;
    use common::places::{
        block, held, obj, recipe, report, settle, until, FakeSocket, Reply, Scene, SceneOpts,
        Script,
    };
    use futures::FutureExt;
    use wsp_machines::places::setup::GivenPicks;
    use wsp_machines::places::types::{is_place_provisioning, FolderMove};
    use wsp_wire::types::recipe_file::{RecipeFile, RecipeFileFoldersValue};

    const P: &str = "p_spoo";

    fn script() -> Script {
        Rc::new(|op, _params| {
            if op != "exec" {
                return Reply::Ok(obj(json!({ "ok": true })));
            }
            Reply::Ok(obj(
                json!({ "ok": true, "exitCode": 0, "stdout": "", "stderr": "", "truncated": false }),
            ))
        })
    }

    fn spoo_report() -> wsp_wire::wire::place_link::PlaceReport {
        report(
            "spoo",
            json!({ "daemonVersion": 200, "login": { "HOME": "/root", "PATH": "/usr/bin" } }),
        )
    }

    fn picks_of(keys: &[&str]) -> Option<GivenPicks> {
        let folders: serde_json::Map<String, Value> = keys
            .iter()
            .map(|k| {
                (
                    k.to_string(),
                    json!({ "from": format!("/repos/{k}"), "keep": [] }),
                )
            })
            .collect();
        Some(GivenPicks {
            picks: recipe(
                json!({ "name": "laptop", "agents": { "claude": { "signin": "vault" } }, "folders": folders }),
            ),
            recipe: None,
        })
    }

    /// setup.ts:190 and :651, link.ts:431. The place a setup is on is let through the fork gate by every call the
    /// folder add makes, inline and in each continuation it starts (an `AsyncLocalStorage` follows both). The gate
    /// reads `FOLDERS_OF`, a `tokio::task_local` (setup.rs:224) that follows the scoped future alone: a call the add
    /// hands to `spawn_local` finds the computer busy setting up and is refused with its provisioning line.
    /// The trait methods the TS road runs inline and the Rust road must therefore never spawn are
    /// `PlaceRecording::add_folder` and everything it awaits (the projects door's `add`, `ProjectsContext::forking_at`
    /// and `landing_backend`, which end at `LinkDoor::forking_backend`, link.rs:1208).
    #[test]
    fn the_setups_own_folder_add_is_let_through_the_gate_in_every_task_it_starts() {
        block(async {
            tokio::time::timeout(std::time::Duration::from_secs(20), async {
            let scene = Scene::new(SceneOpts::default());
            let door = scene.link();
            let _socket: Rc<FakeSocket> = scene.linked(P, "spoo", spoo_report(), script());
            let seen: Rc<RefCell<Vec<(&'static str, bool)>>> = Rc::default();
            let road = {
                let (door, seen) = (door.clone(), seen.clone());
                move |key: &str, f: &RecipeFileFoldersValue, _mv: Option<&FolderMove>| -> Result<wsp_machines::places::types::HeldRow, HostError> {
                    let inline = door.forking_backend(P).now_or_never();
                    seen.borrow_mut().push(("inline", matches!(&inline, Some(Err(e)) if is_place_provisioning(e))));
                    let (door, seen) = (door.clone(), seen.clone());
                    tokio::task::spawn_local(async move {
                        let later = door.forking_backend(P).await;
                        seen.borrow_mut().push(("in a task the add started", matches!(&later, Err(e) if is_place_provisioning(e))));
                    });
                    Ok(held(json!({
                        "id": format!("folders/{key}"), "label": f.name.clone().unwrap_or_else(|| key.to_string()), "outcome": "installed",
                        "project": { "id": format!("proj_{key}") }, "pick": f, "createdAt": common::places::iso(T0),
                    })))
                }
            };
            *scene.recording.add_folder.borrow_mut() = Some(Rc::new(road));
            scene.provisioner.hold(wsp_engine::provision::EngineStep::Context);
            scene.setup.start_setup(P, "a1", picks_of(&["app"]), None).await.expect("started");
            until("both probes ran while the setup still runs", || seen.borrow().len() == 2).await;
            scene.provisioner.let_go(wsp_engine::provision::EngineStep::Context);
            until("the setup ended", || scene.records.get(P).setup.is_some_and(|s| serde_json::to_value(s.state).unwrap() == json!("done"))).await;
            settle().await;
            let refused: Vec<&str> = seen.borrow().iter().filter(|(_, r)| *r).map(|(w, _)| *w).collect();
            assert!(refused.is_empty(), "the setup's own folder add was refused by its own gate: {refused:?}; probes {:?}", seen.borrow());
          }).await.expect("the test hung");
        });
    }
}

mod folder_servers {
    use super::*;
    use common::places::{
        block, held, iso, obj, recipe, report, settle, until, FakeDevices, FakeProvisioner,
        FakeRecording, FakeRecords, FakeStore, FakeViews, Reply, Scene, Script, TestClock,
    };
    use wsp_machines::places::context::place_door_context;
    use wsp_machines::places::setup::{GivenPicks, PlaceSetupArea, SetupRecords};
    use wsp_machines::places::types::{
        FolderMove, HeldImage, HeldRow, HerePlace, PlaceDoorOptions, PlaceEvent, PlaceProvisioner,
        PlaceRecording, PlaceResult, PlaceWiring, ProjectOn, RowId, SetupEvent, SignInEmit,
        SignInHandle, SpentToday,
    };
    use wsp_wire::types::agents_report::SignInLine;
    use wsp_wire::types::recipe_file::RecipeFileFoldersValue;
    use wsp_wire::views::place_more::{PlaceHolds, PlaceRemovedTook, PlaceView};

    const P: &str = "p_spoo";

    /// The crate's recording with one thing added: `projects_on` fails from its `fail_from`-th call.
    struct Rec {
        inner: Rc<FakeRecording>,
        projects_calls: Cell<usize>,
        fail_from: usize,
    }

    #[async_trait(?Send)]
    #[rustfmt::skip]
    impl PlaceRecording for Rec {
        async fn projects_on(&self, place_id: &str) -> PlaceResult<Vec<ProjectOn>> { let n = self.projects_calls.get() + 1; self.projects_calls.set(n); if n >= self.fail_from { return Err(HostError::new("projects_on failed")); } self.inner.projects_on(place_id).await }
        async fn holds_on(&self, place_id: &str, read: bool) -> PlaceResult<PlaceHolds> { self.inner.holds_on(place_id, read).await }
        async fn drop_on(&self, place_id: &str) -> PlaceResult<PlaceRemovedTook> { self.inner.drop_on(place_id).await }
        async fn forget_on(&self, place_id: &str) -> PlaceResult<PlaceRemovedTook> { self.inner.forget_on(place_id).await }
        async fn running_on(&self, place_id: &str, places: &[RowId]) -> PlaceResult<u64> { self.inner.running_on(place_id, places).await }
        async fn machines_on(&self, place_id: &str) -> Option<PlaceResult<Vec<String>>> { self.inner.machines_on(place_id).await }
        async fn forget_records_on(&self, place_id: &str) -> Option<PlaceResult<()>> { self.inner.forget_records_on(place_id).await }
        async fn sub_holds_on(&self, place_id: &str) -> Option<PlaceResult<Vec<String>>> { self.inner.sub_holds_on(place_id).await }
        async fn release_records_on(&self, place_id: &str) -> Option<PlaceResult<()>> { self.inner.release_records_on(place_id).await }
        fn has_images(&self) -> bool { self.inner.has_images() }
        async fn images_on(&self, place_id: &str) -> PlaceResult<Vec<HeldImage>> { self.inner.images_on(place_id).await }
        async fn drop_image_on(&self, place_id: &str, id: &str) -> PlaceResult<()> { self.inner.drop_image_on(place_id, id).await }
        async fn spent_today(&self, places: &[PlaceView]) -> Option<PlaceResult<Vec<SpentToday>>> { self.inner.spent_today(places).await }
        async fn sign_in(&self, place_id: &str, agent: &str, emit: SignInEmit, ) -> Option<PlaceResult<SignInHandle>> { self.inner.sign_in(place_id, agent, emit).await }
        async fn mint_here(&self, agent: &str, emit: SignInEmit) -> Option<PlaceResult<SignInHandle>> { self.inner.mint_here(agent, emit).await }
        async fn sign_in_line(&self, place_id: &str, agent: &str) -> Option<PlaceResult<SignInLine>> { self.inner.sign_in_line(place_id, agent).await }
        fn stores_on(&self, place_id: &str, home: &str) -> Option<IndexMap<String, String>> { self.inner.stores_on(place_id, home) }
        async fn login_here(&self, agent: &str) -> Option<PlaceResult<Option<Vec<u8>>>> { self.inner.login_here(agent).await }
        async fn add_folder(&self, place_id: &str, key: &str, folder: &RecipeFileFoldersValue, mv: Option<&FolderMove>, stage: Option<Rc<dyn Fn(&str)>>, ) -> Option<PlaceResult<HeldRow>> { self.inner.add_folder(place_id, key, folder, mv, stage).await }
        async fn folder_remote(&self, folder: &RecipeFileFoldersValue, ) -> Option<PlaceResult<Option<String>>> { self.inner.folder_remote(folder).await }
        async fn remove_folder(&self, place_id: &str, project_id: &str) -> PlaceResult<()> { self.inner.remove_folder(place_id, project_id).await }
        async fn folder_look(&self, project_id: &str, was: &RecipeFileFoldersValue, now: &RecipeFileFoldersValue, ) -> PlaceResult<()> { self.inner.folder_look(project_id, was, now).await }
    }

    /// `Scene::new` (tests/common/places.rs:1478) over a recording of the test's own.
    fn scene_over(recording: Rc<Rec>) -> Scene {
        let clock = TestClock::new();
        let store = Rc::new(FakeStore::default());
        let records = FakeRecords::new();
        let views = FakeViews::new();
        let provisioner = FakeProvisioner::new();
        let frames: Rc<RefCell<Vec<SetupEvent>>> = Rc::new(RefCell::new(Vec::new()));
        let vault = Rc::new(RefCell::new(IndexMap::from([(
            "CLAUDE_CODE_OAUTH_TOKEN".to_string(),
            "sk-ant-oat01-x".to_string(),
        )])));
        let wiring = PlaceWiring {
            host_key: wsp_store::keys::new_place_key_pair(),
            provider: None,
            here: Rc::new(|| HerePlace {
                name: "zingzys-mac".into(),
                ..Default::default()
            }),
            host_name: Rc::new(|| "zingzys-mac".to_string()),
            install: None,
            dial: None,
            log: None,
            update: None,
            leave: None,
            forget_key: None,
            sudo_over: None,
            provision: Some(Rc::clone(&provisioner) as Rc<dyn PlaceProvisioner>),
            github_token: None,
            undo: None,
            run_over: None,
            back: None,
        };
        let (f2, v2) = (Rc::clone(&frames), Rc::clone(&vault));
        let opts = PlaceDoorOptions {
            store: Rc::clone(&store) as Rc<dyn Store>,
            devices: Rc::new(FakeDevices),
            wiring,
            recording: Rc::clone(&recording) as Rc<dyn PlaceRecording>,
            providers: None,
            on_daemon_event: None,
            on_stage: None,
            on_setup: Some(Rc::new(move |e: &SetupEvent| {
                f2.borrow_mut().push(e.clone())
            })),
            copy_build: None,
            nap_ms: None,
            here_daemon: None,
            nap_changed: None,
            join_wait_ms: None,
            update_wait_ms: None,
            dial_wait_ms: None,
            frame_wait_ms: None,
            relink_wait_ms: None,
            seen_every_ms: None,
            vault: Some(Rc::new(move || v2.borrow().clone())),
            recipes: Some(Rc::new(|| None)),
            owner: None,
            kill_confirm: None,
            clock: Some(Rc::clone(&clock) as Rc<dyn wsp_core::types::pending::Clock>),
        };
        let ctx = place_door_context(opts);
        *records.ctx.borrow_mut() = Some(Rc::clone(&ctx));
        *views.ctx.borrow_mut() = Some(Rc::clone(&ctx));
        let events: Rc<RefCell<Vec<PlaceEvent>>> = Rc::new(RefCell::new(Vec::new()));
        let setup =
            PlaceSetupArea::new(Rc::clone(&ctx), Rc::clone(&records) as Rc<dyn SetupRecords>);
        Scene {
            clock,
            store,
            ctx,
            records,
            views,
            recording: Rc::clone(&recording.inner),
            provisioner,
            setup,
            frames,
            events,
            vault,
        }
    }

    /// setup.ts:679-681. A project's servers are carried only after `recording.projectsOn` has said where the project
    /// stands there; the TS does not catch its failure, so the step fails as one row ("folderServers/stopped") with the
    /// error's first line, and the setup says the folders' servers did not land. folders.rs:159-170 turns the failure into
    /// "no path" and the step ends clean with no row.
    #[test]
    fn a_failed_read_of_the_projects_there_fails_the_folder_servers_step() {
        block(async {
            tokio::time::timeout(std::time::Duration::from_secs(20), async {
                let rec = Rc::new(Rec { inner: Rc::new(FakeRecording::default()), projects_calls: Cell::new(0), fail_from: 2 });
                let scene = scene_over(rec.clone());
                scene.provisioner.project_servers.set(true);
                let landed = |key: &str, f: &RecipeFileFoldersValue, _mv: Option<&FolderMove>| -> PlaceResult<HeldRow> {
                    Ok(held(json!({
                        "id": format!("folders/{key}"), "label": f.name.clone().unwrap_or_else(|| key.to_string()), "outcome": "installed",
                        "project": { "id": format!("proj_{key}") }, "pick": f, "createdAt": iso(T0),
                    })))
                };
                *scene.recording.add_folder.borrow_mut() = Some(Rc::new(landed));
                let script: Script = Rc::new(|op, _p| {
                    if op != "exec" {
                        return Reply::Ok(obj(json!({ "ok": true })));
                    }
                    Reply::Ok(obj(json!({ "ok": true, "exitCode": 0, "stdout": "", "stderr": "", "truncated": false })))
                });
                let _socket = scene.linked(P, "spoo", report("spoo", json!({ "daemonVersion": 200, "login": { "HOME": "/root", "PATH": "/usr/bin" } })), script);
                let picks = Some(GivenPicks {
                    picks: recipe(json!({ "name": "laptop", "agents": { "claude": { "signin": "vault" } }, "folders": { "app": { "from": "/repos/app", "keep": [] } } })),
                    recipe: None,
                });
                scene.setup.start_setup(P, "a1", picks, None).await.expect("started");
                until("the setup ended", || scene.records.get(P).setup.is_some_and(|s| serde_json::to_value(s.state).unwrap() != json!("running"))).await;
                settle().await;
                assert!(rec.projects_calls.get() >= 2, "the folders step and then the servers step each read the projects there; reads: {}", rec.projects_calls.get());
                let rows: Vec<String> = scene.records.get(P).applied.map(|a| a.rows.into_iter().map(|r| r.row.id).collect()).unwrap_or_default();
                assert!(rows.iter().any(|id| id == "folderServers/stopped"), "boot.ts's step row for a failed read is folderServers/stopped (label \"the projects' own MCP servers\"); rows: {rows:?}");
            })
            .await
            .expect("the test hung");
        });
    }
}

/// What boot.ts logged for the scenes above, as node wrote them (scratch oracle ready_more.oracle.ts).
const SCENES: &str = include_str!("fixtures/review_boot/scenes.json");

/// What boot.ts logged with the engine's own `sightMachine` and `readGone` over a provider that answers 404 (scratch oracle sight_real.oracle.ts).
const SIGHT: &str = include_str!("fixtures/review_boot/sight.json");
