// SPDX-License-Identifier: AGPL-3.0-only
//! The image areas' fakes: one `World` that implements `RecordsCtx`, `HeldCtx`, `BuildersCtx`, `GoldenCtx` and
//! `ImageCtx` over shared state, a recording `GoldenRoads`, machines and backends with scripted answers. Every call a
//! unit makes on its context or the roads lands in `world.log` in order, as one line. Cross-area calls answer from the
//! store the way the real area does (`copy_of` reads `goldens/<place>/<name>`), unless a test scripts them.
#![allow(dead_code, unused_imports, clippy::type_complexity)]

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::{FutureExt, LocalBoxFuture};
use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Value};

use wsp_core::clock::{Cancel, Clock};
use wsp_core::types::api::PrepareAsk;
use wsp_core::types::internal::{BuilderLife, BuilderRecord};
use wsp_core::types::pending::Store;
use wsp_core::types::wiring::{GoldenRecipe, PlaceBackends, WorkspaceRecord};
use wsp_engine::errors::{EngineError, ErrorKind, WspError};
use wsp_engine::golden_seam::GoldenImport;
use wsp_engine::machine::{
    BackendPricing, Capabilities, ExecOptions, ExecResult, Machine, MachineBackend, MachineKind, MachineLife, MachineListRow,
    MachineShape, MachineSpec, MachineState, RunOptions, Size, SnapshotOptions, SnapshotStoragePricing, Waiting,
};
use wsp_engine::Cancel as EngineCancel;
use wsp_machines::images::builders::life::{CreateStop, Expired, ImportMark};
use wsp_machines::images::builders::places::PlaceAt;
use wsp_machines::images::builders::{BuildPlace, BuildersCtx, BuildersOpts, Claim, CopyRecipe, ImageDoor, StageAt};
use wsp_machines::images::engine::*;
use wsp_machines::images::golden::{GoldenCtx, Preparing};
use wsp_machines::images::held::HeldCtx;
use wsp_machines::images::image::{CopyRow, ImageCtx, Stopped};
use wsp_machines::images::records::{ImageOf, RecordsCtx};
use wsp_machines::images::{BuilderRef, Builders, ImageError, ImageResult, Joined, LiveBuilder};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::golden_image::{
    GoldenBuilderView, GoldenLogin, GoldenManifest, GoldenStage, GoldenStageEvent, GoldenStep, GoldenVersion, RecipeDigest, SealedImage,
    SealedImageBuilt,
};
use wsp_wire::views::place_kind::PlaceKind;
use wsp_wire::views::place_more::PlaceView;
use wsp_wire::views::workspace::ProjectView;
use wsp_wire::wire::capabilities::WorkspaceSize;
use wsp_wire::wire::machine_link::{PreviewReach, SnapshotRow, TemplateRow};
use wsp_words::exit::{HostError, Kind};

#[path = "../rest_support/mod.rs"]
pub mod rest;
pub use rest::{plain_error, run, MemStore, TestClock, Warned};

// --- machines and backends ------------------------------------------------------------------------------------------

pub fn exec_ok(stdout: &str) -> ExecResult {
    ExecResult { exit_code: 0.into(), stdout: stdout.to_string(), stderr: String::new() }
}

pub fn exec_exit(code: i64, stdout: &str, stderr: &str) -> ExecResult {
    ExecResult { exit_code: code.into(), stdout: stdout.to_string(), stderr: stderr.to_string() }
}

/// An engine error the provider answered: `kind` missing reads as a machine that is gone.
pub fn provider_error(kind: ErrorKind, status: u16, message: &str) -> EngineError {
    EngineError::Provider(WspError { kind, status, code: None, message: message.to_string(), request_id: None, window: None, said: None })
}

pub fn missing(message: &str) -> EngineError {
    provider_error(ErrorKind::Missing, 404, message)
}

/// A machine whose `exec` answers its script in order (then exit 0), and logs each command.
pub struct FakeMachine {
    pub id: String,
    pub labels: Option<IndexMap<String, String>>,
    pub stream_url: Option<String>,
    pub execs: RefCell<Vec<String>>,
    pub answers: RefCell<VecDeque<Result<ExecResult, EngineError>>>,
    pub describe: RefCell<Option<Result<MachineShape, EngineError>>>,
    /// Called with each command as it arrives, before it is answered.
    pub on_exec: RefCell<Option<Rc<dyn Fn(&str)>>>,
}

impl FakeMachine {
    pub fn new(id: &str) -> Arc<FakeMachine> {
        Arc::new(FakeMachine::bare(id))
    }
    pub fn bare(id: &str) -> FakeMachine {
        FakeMachine {
            id: id.to_string(),
            labels: None,
            stream_url: None,
            execs: RefCell::new(vec![]),
            answers: RefCell::new(VecDeque::new()),
            describe: RefCell::new(None),
            on_exec: RefCell::new(None),
        }
    }
    pub fn labelled(id: &str, labels: &[(&str, &str)]) -> Arc<FakeMachine> {
        let mut m = FakeMachine::bare(id);
        m.labels = Some(labels.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect());
        Arc::new(m)
    }
    pub fn answer(&self, a: Result<ExecResult, EngineError>) {
        self.answers.borrow_mut().push_back(a);
    }
    pub fn execs(&self) -> Vec<String> {
        self.execs.borrow().clone()
    }
}

#[async_trait(?Send)]
impl Machine for FakeMachine {
    fn id(&self) -> &str {
        &self.id
    }
    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }
    fn stream_url(&self) -> Option<&str> {
        self.stream_url.as_deref()
    }
    fn labels(&self) -> Option<&IndexMap<String, String>> {
        self.labels.as_ref()
    }
    async fn exec(&self, cmd: &str, opts: ExecOptions) -> Result<ExecResult, EngineError> {
        self.execs.borrow_mut().push(match opts.timeout_ms {
            Some(ms) => format!("{cmd} [{ms}ms]"),
            None => cmd.to_string(),
        });
        let hook = self.on_exec.borrow().clone();
        if let Some(h) = hook {
            h(cmd);
        }
        let next = self.answers.borrow_mut().pop_front();
        next.unwrap_or_else(|| Ok(exec_ok("")))
    }
    async fn run(&self, script: &str, _opts: RunOptions) -> Result<ExecResult, EngineError> {
        self.execs.borrow_mut().push(script.to_string());
        Ok(exec_ok(""))
    }
    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> Result<String, EngineError> {
        Err(missing("snapshot is not faked"))
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> Result<(), EngineError> {
        Ok(())
    }
    async fn resume(&self, _signal: Option<&EngineCancel>) -> Result<(), EngineError> {
        Ok(())
    }
    async fn kill(&self) -> Result<(), EngineError> {
        Ok(())
    }
    async fn state(&self) -> Result<MachineState, EngineError> {
        Ok(MachineState::Running)
    }
    async fn download_url(&self, _path: &str) -> Result<String, EngineError> {
        Err(missing("download_url is not faked"))
    }
    async fn upload_url(&self, _path: &str) -> Result<String, EngineError> {
        Err(missing("upload_url is not faked"))
    }
    async fn describe(&self) -> Result<MachineShape, EngineError> {
        self.describe.borrow().clone().unwrap_or_else(|| Err(missing("describe is not offered")))
    }
}

pub struct FakePricing {
    pub default: Size,
    pub rate: f64,
    pub storage: SnapshotStoragePricing,
}

impl BackendPricing for FakePricing {
    fn rate_usd_per_hour(&self, size: Size) -> f64 {
        self.rate * size.cpu
    }
    fn default_size(&self) -> Size {
        self.default.clone()
    }
    fn snapshot_storage(&self) -> SnapshotStoragePricing {
        self.storage.clone()
    }
}

/// A backend with capabilities and pricing a test picks, a snapshot listing, and every create, get and delete logged.
pub struct FakeBackend {
    pub name: String,
    pub caps: RefCell<Capabilities>,
    pub pricing: Arc<FakePricing>,
    pub snapshots: RefCell<Result<Vec<SnapshotRow>, EngineError>>,
    pub listed: RefCell<Vec<Option<Vec<String>>>>,
    pub deleted: RefCell<Vec<String>>,
    pub delete_fails: RefCell<IndexMap<String, EngineError>>,
    pub created: RefCell<Vec<MachineSpec>>,
    pub next_machine: RefCell<VecDeque<Arc<FakeMachine>>>,
    pub machines: RefCell<IndexMap<String, Result<Arc<FakeMachine>, EngineError>>>,
    /// When set, each listing and snapshot delete is also written here as `backend list_snapshots <ids>` and
    /// `backend delete_snapshot <id>`, so a test can read them in order beside the world's log.
    pub trace: RefCell<Option<Rc<RefCell<Vec<String>>>>>,
    /// When true a listing given ids answers those rows alone (no ids, every row; `Some([])`, the named ones), as a
    /// provider does; else every listing answers `snapshots` whole.
    pub filter_ids: std::cell::Cell<bool>,
    /// Answers handed out first, one per listing, before `snapshots` answers.
    pub list_queue: RefCell<VecDeque<Result<Vec<SnapshotRow>, EngineError>>>,
}

/// Capabilities of a place that forks and builds images, with a snapshot listing.
pub fn building_caps() -> Capabilities {
    serde_json::from_value(json!({
        "liveCloneForks": false, "replacesMachine": false, "previewUrls": true, "signedUrls": false, "callbackRelay": false,
        "diskSnapshots": true, "images": true, "snapshotsAnyLife": false, "snapshotListing": true, "templates": false,
        "kept": true, "copies": true, "ownNetwork": false,
        "sizes": [{"cpu": 2, "memMb": 4096, "rateUsdPerHour": 0.1}, {"cpu": 4, "memMb": 8192, "rateUsdPerHour": 0.2}]
    }))
    .expect("capabilities")
}

/// Capabilities of a place that forks nothing (no sizes).
pub fn forkless_caps() -> Capabilities {
    let mut c = building_caps();
    c.sizes = vec![];
    c.snapshot_listing = false;
    c
}

pub fn storage_pricing(billed_from: &str) -> SnapshotStoragePricing {
    SnapshotStoragePricing {
        wire: serde_json::from_value(json!({"freeGb": 0, "usdPerGbMonth": 0.05, "billedFrom": billed_from})).expect("pricing"),
        free_named: None,
        usd_per_named_month: None,
    }
}

/// Storage priced per named snapshot, as Boat bills it.
pub fn storage_by_count() -> SnapshotStoragePricing {
    SnapshotStoragePricing { free_named: Some(5), usd_per_named_month: Some(0.5), ..storage_pricing("restored") }
}

impl FakeBackend {
    pub fn new(name: &str) -> Arc<FakeBackend> {
        Arc::new(FakeBackend::with(name, building_caps()))
    }
    pub fn forkless(name: &str) -> Arc<FakeBackend> {
        Arc::new(FakeBackend::with(name, forkless_caps()))
    }
    pub fn with(name: &str, caps: Capabilities) -> FakeBackend {
        FakeBackend {
            name: name.to_string(),
            caps: RefCell::new(caps),
            pricing: Arc::new(FakePricing { default: Size { cpu: 2.0, mem_mb: 4096.0 }, rate: 0.05, storage: storage_pricing("restored") }),
            snapshots: RefCell::new(Ok(vec![])),
            listed: RefCell::new(vec![]),
            deleted: RefCell::new(vec![]),
            delete_fails: RefCell::new(IndexMap::new()),
            created: RefCell::new(vec![]),
            next_machine: RefCell::new(VecDeque::new()),
            machines: RefCell::new(IndexMap::new()),
            trace: RefCell::new(None),
            filter_ids: std::cell::Cell::new(false),
            list_queue: RefCell::new(VecDeque::new()),
        }
    }
    /// A backend that forks and lists, whose snapshot storage is billed as `storage` says.
    pub fn priced(name: &str, storage: SnapshotStoragePricing) -> Arc<FakeBackend> {
        let mut b = FakeBackend::with(name, building_caps());
        b.pricing = Arc::new(FakePricing { default: Size { cpu: 2.0, mem_mb: 4096.0 }, rate: 0.05, storage });
        Arc::new(b)
    }
    fn traced(&self, line: String) {
        let t = self.trace.borrow().clone();
        if let Some(t) = t {
            t.borrow_mut().push(line);
        }
    }
}

#[async_trait(?Send)]
impl MachineBackend for FakeBackend {
    fn capabilities(&self) -> Capabilities {
        self.caps.borrow().clone()
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
        self.pricing.clone()
    }
    async fn create(&self, spec: MachineSpec, _waiting: Option<Waiting>, _sent: Option<String>) -> Result<Arc<dyn Machine>, EngineError> {
        self.created.borrow_mut().push(spec);
        let m = self.next_machine.borrow_mut().pop_front().unwrap_or_else(|| FakeMachine::new("m-created"));
        Ok(m)
    }
    async fn get(&self, id: &str) -> Result<Arc<dyn Machine>, EngineError> {
        match self.machines.borrow().get(id) {
            Some(Ok(m)) => Ok(m.clone()),
            Some(Err(e)) => Err(e.clone()),
            None => Err(missing(&format!("machine {id} is gone"))),
        }
    }
    async fn list(&self, _labels: Option<&IndexMap<String, String>>) -> Result<Vec<MachineListRow>, EngineError> {
        Ok(vec![])
    }
    async fn delete_snapshot(&self, id: &str) -> Result<(), EngineError> {
        self.deleted.borrow_mut().push(id.to_string());
        self.traced(format!("backend delete_snapshot {id}"));
        match self.delete_fails.borrow().get(id) {
            Some(e) => Err(e.clone()),
            None => Ok(()),
        }
    }
    async fn list_snapshots(&self, ids: Option<&[String]>) -> Result<Vec<SnapshotRow>, EngineError> {
        self.listed.borrow_mut().push(ids.map(|i| i.to_vec()));
        self.traced(format!("backend list_snapshots {}", ids.map(|i| i.join(",")).unwrap_or_else(|| "*".into())));
        let queued = self.list_queue.borrow_mut().pop_front();
        if let Some(q) = queued {
            return q;
        }
        let all = self.snapshots.borrow().clone()?;
        if !self.filter_ids.get() {
            return Ok(all);
        }
        Ok(match ids {
            None => all,
            Some([]) => all.into_iter().filter(|r| r.name.is_some()).collect(),
            Some(ids) => all.into_iter().filter(|r| ids.contains(&r.id)).collect(),
        })
    }
}

pub fn snapshot_row(id: &str, name: Option<&str>, size: f64, restored: Option<f64>) -> SnapshotRow {
    SnapshotRow { id: id.into(), name: name.map(String::from), size_bytes: size.into(), restored_bytes: restored.map(JsNumber::from), created_at: None, parent: None }
}

// --- the engine's image roads -----------------------------------------------------------------------------------

/// Every road logs `road <args>` into the world's log and answers what the test scripted, else a plain default.
pub struct FakeRoads {
    pub log: Rc<RefCell<Vec<String>>>,
    pub built: RefCell<Option<Result<BuiltGolden, EngineError>>>,
    pub prepared: RefCell<VecDeque<Result<EngineBuilder, ImageError>>>,
    pub sealed: RefCell<VecDeque<Result<SealResult, ImageError>>>,
    pub upgraded: RefCell<VecDeque<Result<EngineBuilder, ImageError>>>,
    pub import_applied: RefCell<Option<Result<Applied, EngineError>>>,
    pub delta_applied: RefCell<Option<Result<Applied, EngineError>>>,
    pub kill_fails: RefCell<Option<EngineError>>,
    pub gone: RefCell<Result<GoneReading, EngineError>>,
    pub drop_template_fails: RefCell<Option<EngineError>>,
    pub templates: RefCell<Option<Rc<FakeTemplates>>>,
    pub promoted: RefCell<VecDeque<Result<Promoted, EngineError>>>,
    pub rolled_back: RefCell<Option<Result<GoldenManifest, EngineError>>>,
    pub reach: RefCell<Option<Result<PreviewReach, EngineError>>>,
    pub vault_import_fails: RefCell<Option<EngineError>>,
    pub foreign: RefCell<Option<String>>,
    /// Asks the roads were handed, for the tests that read their fields.
    pub seal_asks: RefCell<Vec<SealGoldenAsk>>,
    pub prepare_asks: RefCell<Vec<PrepareBuilderAsk>>,
    pub upgrade_asks: RefCell<Vec<UpgradeBuilderAsk>>,
    pub build_asks: RefCell<Vec<BuildGoldenAsk>>,
    pub delta_asks: RefCell<Vec<ApplyDeltaAsk>>,
    pub import_asks: RefCell<Vec<ApplyImportAsk>>,
    /// A gate the seal waits on when set, so a test can act while a seal is in flight.
    pub seal_gate: RefCell<Option<futures::future::Shared<LocalBoxFuture<'static, ()>>>>,
    pub prepare_gate: RefCell<Option<futures::future::Shared<LocalBoxFuture<'static, ()>>>>,
    /// Run when `promote_version` is called, before it answers.
    pub on_promote: RefCell<Option<Rc<dyn Fn()>>>,
}

impl FakeRoads {
    pub fn new(log: Rc<RefCell<Vec<String>>>) -> Rc<FakeRoads> {
        Rc::new(FakeRoads {
            log,
            built: RefCell::new(None),
            prepared: RefCell::new(VecDeque::new()),
            sealed: RefCell::new(VecDeque::new()),
            upgraded: RefCell::new(VecDeque::new()),
            import_applied: RefCell::new(None),
            delta_applied: RefCell::new(None),
            kill_fails: RefCell::new(None),
            gone: RefCell::new(Ok(GoneReading::Gone)),
            drop_template_fails: RefCell::new(None),
            templates: RefCell::new(None),
            promoted: RefCell::new(VecDeque::new()),
            rolled_back: RefCell::new(None),
            reach: RefCell::new(None),
            vault_import_fails: RefCell::new(None),
            foreign: RefCell::new(None),
            seal_asks: RefCell::new(vec![]),
            prepare_asks: RefCell::new(vec![]),
            upgrade_asks: RefCell::new(vec![]),
            build_asks: RefCell::new(vec![]),
            delta_asks: RefCell::new(vec![]),
            import_asks: RefCell::new(vec![]),
            seal_gate: RefCell::new(None),
            prepare_gate: RefCell::new(None),
            on_promote: RefCell::new(None),
        })
    }
    fn note(&self, line: String) {
        self.log.borrow_mut().push(line);
    }
}

pub fn ledger(hash: &str) -> ImportLedger {
    ImportLedger { recipe_hash: hash.into(), smoke: "true".into(), ..Default::default() }
}

pub fn engine_builder(machine: Arc<FakeMachine>) -> EngineBuilder {
    EngineBuilder {
        machine,
        kind: MachineKind::Sandbox,
        base_template: "base".into(),
        setup_sha: "sha-1".into(),
        created_at: "2026-10-25T10:00:00.000Z".into(),
        first_life: true,
        size: serde_json::from_value(json!({"cpu": 2, "memMb": 4096})).expect("size"),
        import: None,
        parent_snapshot_id: None,
        base: None,
        retired: None,
    }
}

#[async_trait(?Send)]
impl GoldenRoads for FakeRoads {
    async fn build_golden(&self, ask: BuildGoldenAsk) -> Result<BuiltGolden, EngineError> {
        self.note(format!("road build_golden {}", ask.name));
        self.build_asks.borrow_mut().push(ask);
        self.built.borrow().clone().unwrap_or_else(|| Err(missing("build_golden is not scripted")))
    }
    async fn prepare_builder(&self, ask: PrepareBuilderAsk) -> Result<EngineBuilder, ImageError> {
        self.note("road prepare_builder".into());
        self.prepare_asks.borrow_mut().push(ask);
        let gate = self.prepare_gate.borrow().clone();
        if let Some(g) = gate {
            g.await;
        }
        self.prepared.borrow_mut().pop_front().unwrap_or_else(|| Err(ImageError::Host(HostError::new("prepare_builder is not scripted"))))
    }
    async fn seal_golden(&self, builder: EngineBuilder, ask: SealGoldenAsk) -> Result<SealResult, ImageError> {
        self.note(format!("road seal_golden {} keep={}", builder.machine.id(), ask.keep_builder));
        self.seal_asks.borrow_mut().push(ask);
        let gate = self.seal_gate.borrow().clone();
        if let Some(g) = gate {
            g.await;
        }
        self.sealed.borrow_mut().pop_front().unwrap_or_else(|| Err(ImageError::Host(HostError::new("seal_golden is not scripted"))))
    }
    async fn upgrade_builder(&self, ask: UpgradeBuilderAsk) -> Result<EngineBuilder, ImageError> {
        self.note("road upgrade_builder".into());
        self.upgrade_asks.borrow_mut().push(ask);
        self.upgraded.borrow_mut().pop_front().unwrap_or_else(|| Err(ImageError::Host(HostError::new("upgrade_builder is not scripted"))))
    }
    async fn apply_golden_import(&self, machine: Arc<dyn Machine>, ask: ApplyImportAsk) -> Result<Applied, EngineError> {
        self.note(format!("road apply_golden_import {}", machine.id()));
        self.import_asks.borrow_mut().push(ask);
        self.import_applied.borrow().clone().unwrap_or_else(|| Ok(Applied { ledger: ledger("h-applied") }))
    }
    async fn apply_delta(&self, machine: Arc<dyn Machine>, _delta: GoldenDelta, ask: ApplyDeltaAsk) -> Result<Applied, EngineError> {
        self.note(format!("road apply_delta {}", machine.id()));
        self.delta_asks.borrow_mut().push(ask);
        self.delta_applied.borrow().clone().unwrap_or_else(|| Ok(Applied { ledger: ledger("h-delta") }))
    }
    fn next_setup_sha(&self, previous: &str, setup: &str, _import: &GoldenImport) -> String {
        format!("{previous}+{}", setup.len())
    }
    async fn kill_until_gone(&self, at: Arc<dyn MachineBackend>, machine: Arc<dyn Machine>, _confirm: Option<KillConfirm>) -> Result<(), EngineError> {
        let _ = at;
        self.note(format!("road kill_until_gone {}", machine.id()));
        match self.kill_fails.borrow().clone() {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
    async fn read_gone(&self, _at: Arc<dyn MachineBackend>, id: &str) -> Result<GoneReading, EngineError> {
        self.note(format!("road read_gone {id}"));
        self.gone.borrow().clone()
    }
    async fn drop_template(&self, _at: Arc<dyn MachineBackend>, template_id: Option<&str>) -> Result<(), EngineError> {
        self.note(format!("road drop_template {}", template_id.unwrap_or("-")));
        match self.drop_template_fails.borrow().clone() {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
    fn templates_of(&self, _at: Arc<dyn MachineBackend>) -> Option<Rc<dyn Templates>> {
        self.templates.borrow().clone().map(|t| t as Rc<dyn Templates>)
    }
    async fn promote_version(&self, _templates: Rc<dyn Templates>, snapshot_id: &str, name: &str) -> Result<Promoted, EngineError> {
        self.note(format!("road promote_version {snapshot_id} {name}"));
        let hook = self.on_promote.borrow().clone();
        if let Some(h) = hook {
            h();
        }
        self.promoted.borrow_mut().pop_front().unwrap_or_else(|| Err(missing("promote is not scripted")))
    }
    fn rollback(&self, manifest: &GoldenManifest, version: u32) -> Result<GoldenManifest, EngineError> {
        self.note(format!("road rollback {version}"));
        self.rolled_back.borrow().clone().unwrap_or_else(|| Ok(manifest.clone()))
    }
    async fn refresh_preview_token(&self, machine: Arc<dyn Machine>, port: u16, _held: Option<PreviewReach>) -> Result<PreviewReach, EngineError> {
        self.note(format!("road refresh_preview_token {} {port}", machine.id()));
        self.reach.borrow().clone().unwrap_or_else(|| Err(missing("reach is not scripted")))
    }
    async fn import_image_vault(&self, machine: Arc<dyn Machine>, tar: Vec<u8>) -> Result<(), EngineError> {
        self.note(format!("road import_image_vault {} {}", machine.id(), tar.len()));
        match self.vault_import_fails.borrow().clone() {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
    fn refuse_foreign_members(&self, road: &str, _tar: &[u8], held: &[String]) -> Result<(), String> {
        self.note(format!("road refuse_foreign_members {road} {}", held.join(",")));
        match self.foreign.borrow().clone() {
            Some(m) => Err(m),
            None => Ok(()),
        }
    }
    fn image_hash(&self, recipe_hash: &str, vault_sha256: Option<&str>) -> String {
        format!("img:{recipe_hash}:{}", vault_sha256.unwrap_or("-"))
    }
}

/// A template road that logs and answers its list.
pub struct FakeTemplates {
    pub log: Rc<RefCell<Vec<String>>>,
    pub rows: RefCell<Vec<TemplateRow>>,
    pub delete_fails: RefCell<IndexMap<String, EngineError>>,
}

#[async_trait(?Send)]
impl Templates for FakeTemplates {
    async fn promote(&self, snapshot_id: &str, name: &str) -> Result<String, EngineError> {
        self.log.borrow_mut().push(format!("templates promote {snapshot_id} {name}"));
        Ok(format!("t-{snapshot_id}"))
    }
    async fn get(&self, id: &str) -> Result<TemplateRow, EngineError> {
        Err(missing(&format!("template {id}")))
    }
    async fn list(&self) -> Result<Vec<TemplateRow>, EngineError> {
        self.log.borrow_mut().push("templates list".into());
        Ok(self.rows.borrow().clone())
    }
    async fn delete(&self, id: &str) -> Result<(), EngineError> {
        self.log.borrow_mut().push(format!("templates delete {id}"));
        match self.delete_fails.borrow().get(id) {
            Some(e) => Err(e.clone()),
            None => Ok(()),
        }
    }
}

// --- the door and the place table -------------------------------------------------------------------------------

/// A place door with named computers, each with an optional forking backend; every call logged.
pub struct FakeDoor {
    pub log: Rc<RefCell<Vec<String>>>,
    pub names: RefCell<IndexMap<String, String>>,
    pub backends: RefCell<IndexMap<String, Arc<FakeBackend>>>,
    pub forking_fails: RefCell<IndexMap<String, HostError>>,
    pub words: RefCell<IndexMap<String, Result<Option<String>, HostError>>>,
    pub marked: RefCell<Result<Option<String>, HostError>>,
    pub mark_fails: RefCell<Option<HostError>>,
    pub rows: RefCell<Vec<PlaceView>>,
}

impl FakeDoor {
    pub fn new(log: Rc<RefCell<Vec<String>>>) -> Rc<FakeDoor> {
        Rc::new(FakeDoor {
            log,
            names: RefCell::new(IndexMap::new()),
            backends: RefCell::new(IndexMap::new()),
            forking_fails: RefCell::new(IndexMap::new()),
            words: RefCell::new(IndexMap::new()),
            marked: RefCell::new(Ok(None)),
            mark_fails: RefCell::new(None),
            rows: RefCell::new(vec![]),
        })
    }
    /// A joined computer `id` named `name`, forking on `backend`, listed by the door.
    pub fn computer(&self, id: &str, name: &str, backend: Option<Arc<FakeBackend>>) {
        self.names.borrow_mut().insert(id.into(), name.into());
        if let Some(b) = backend {
            self.backends.borrow_mut().insert(id.into(), b);
        }
        self.rows.borrow_mut().push(computer_row(id, name));
    }
}

/// A joined computer's row as the door lists it.
pub fn computer_row(id: &str, name: &str) -> PlaceView {
    serde_json::from_value(json!({"id": id, "name": name, "kind": "computer", "default": false})).unwrap_or_else(|e| panic!("a computer row: {e}"))
}

#[async_trait(?Send)]
impl ImageDoor for FakeDoor {
    fn backend_of(&self, place_id: &str) -> Option<Arc<dyn MachineBackend>> {
        self.backends.borrow().get(place_id).map(|b| b.clone() as Arc<dyn MachineBackend>)
    }
    async fn forking_backend(&self, place_id: &str) -> Result<Arc<dyn MachineBackend>, HostError> {
        self.log.borrow_mut().push(format!("door forking_backend {place_id}"));
        if let Some(e) = self.forking_fails.borrow().get(place_id) {
            return Err(e.clone());
        }
        self.backends.borrow().get(place_id).map(|b| b.clone() as Arc<dyn MachineBackend>).ok_or_else(|| HostError::new(format!("{place_id} forks nothing here")))
    }
    fn name_of(&self, place_id: &str) -> Option<String> {
        self.names.borrow().get(place_id).cloned()
    }
    async fn place_for(&self, word: &str) -> Result<Option<String>, HostError> {
        self.log.borrow_mut().push(format!("door place_for {word}"));
        self.words.borrow().get(word).cloned().unwrap_or(Ok(None))
    }
    async fn default_place(&self) -> Result<Option<String>, HostError> {
        self.log.borrow_mut().push("door default_place".into());
        self.marked.borrow().clone()
    }
    async fn mark_default_if_none(&self, place_id: &str) -> Result<(), HostError> {
        self.log.borrow_mut().push(format!("door mark_default_if_none {place_id}"));
        match self.mark_fails.borrow().clone() {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
    async fn list(&self, now: i64) -> Result<Vec<PlaceView>, HostError> {
        self.log.borrow_mut().push(format!("door list {now}"));
        Ok(self.rows.borrow().clone())
    }
}

/// The provider table: the wired id and its rows.
pub struct FakePlaces {
    pub wired: RefCell<String>,
    pub rows: RefCell<IndexMap<String, Arc<FakeBackend>>>,
}

impl PlaceBackends for FakePlaces {
    fn wired(&self) -> String {
        self.wired.borrow().clone()
    }
    fn backend(&self, place: &str) -> Option<Arc<dyn MachineBackend>> {
        self.rows.borrow().get(place).map(|b| b.clone() as Arc<dyn MachineBackend>)
    }
    fn list(&self) -> Vec<String> {
        self.rows.borrow().keys().cloned().collect()
    }
}

/// A claim whose backend is the one asked, logged as `claim <purpose> @<place>`.
pub struct FakeClaim {
    pub at: Arc<dyn MachineBackend>,
}

impl Claim for FakeClaim {
    fn backend(&self) -> Arc<dyn MachineBackend> {
        self.at.clone()
    }
}

// --- records ----------------------------------------------------------------------------------------------------

/// A manifest of versions `(version, snapshot, template)`, head the last.
pub fn manifest(versions: &[(u32, &str, Option<&str>)]) -> GoldenManifest {
    let vs: Vec<Value> = versions.iter().map(|(v, s, t)| version_json(*v, s, *t)).collect();
    let head = versions.last().map(|v| v.0).unwrap_or(0);
    serde_json::from_value(json!({"head": head, "versions": vs})).expect("a manifest")
}

pub fn version_json(v: u32, snapshot: &str, template: Option<&str>) -> Value {
    let mut j = json!({
        "version": v, "snapshotId": snapshot, "baseTemplate": "base", "setupSha": format!("sha-{v}"),
        "createdAt": "2026-10-25T09:00:00.000Z", "smoke": {"cmd": "true", "exitCode": 0}
    });
    if let Some(t) = template {
        j["templateId"] = json!(t);
    }
    j
}

pub fn version(v: u32, snapshot: &str, template: Option<&str>) -> GoldenVersion {
    serde_json::from_value(version_json(v, snapshot, template)).expect("a version")
}

pub fn sealed_image(name: &str, version: u32, hash: &str, place: Option<&str>) -> SealedImage {
    let mut j = json!({
        "name": name, "version": version, "hash": hash, "recipeHash": "r", "logins": [], "sealedAt": "2026-10-25T09:00:00.000Z",
        "sealedFrom": "host-1"
    });
    if let Some(p) = place {
        j["place"] = json!(p);
    }
    serde_json::from_value(j).expect("a sealed image")
}

pub fn builder_record(id: &str, name: &str, place: &str) -> BuilderRecord {
    serde_json::from_value(json!({
        "id": id, "name": name, "kind": "sandbox", "baseTemplate": "base", "setupSha": "sha-1", "createdAt": "2026-10-25T10:00:00.000Z",
        "size": {"cpu": 2, "memMb": 4096}, "firstLife": true, "place": place
    }))
    .expect("a builder record")
}

pub fn live_builder(id: &str, name: &str, place: &str, life: BuilderLife) -> BuilderRef {
    Rc::new(RefCell::new(LiveBuilder {
        record: builder_record(id, name, place),
        builder: engine_builder(FakeMachine::new(id)),
        sealable: true,
        life,
        reach: None,
        recipe: None,
    }))
}

pub fn recipe(setup: &str) -> GoldenRecipe {
    GoldenRecipe { setup: setup.into(), smoke: "true".into(), ..Default::default() }
}

pub fn workspace_record(id: &str, name: &str, golden: &str) -> WorkspaceRecord {
    serde_json::from_value(json!({
        "id": id, "name": name, "machineId": format!("m-{id}"), "phase": "running", "kind": "cloud", "golden": golden,
        "createdAt": "2026-10-25T08:00:00.000Z", "project": "p_spoo", "base": "main",
        "spec": {}, "size": {"cpu": 2, "memMb": 4096}, "firstLife": true
    }))
    .unwrap_or_else(|e| panic!("a workspace record: {e}"))
}

// --- the world --------------------------------------------------------------------------------------------------

pub const NOW: i64 = 1_792_918_800_000; // 2026-10-25T09:00:00Z

/// Everything the image areas' contexts read, shared.
pub struct World {
    pub store: Rc<MemStore>,
    pub clock: Rc<TestClock>,
    pub warned: Warned,
    pub log: Rc<RefCell<Vec<String>>>,
    pub roads: Rc<FakeRoads>,
    pub places: Rc<FakePlaces>,
    pub door: RefCell<Option<Rc<FakeDoor>>>,
    pub runtime_backend: Arc<FakeBackend>,
    pub builders: Builders,
    pub stage_at: Rc<RefCell<IndexMap<String, StageAt>>>,
    pub preparing: Rc<RefCell<IndexMap<String, Preparing>>>,
    pub grace_timers: Rc<RefCell<IndexMap<String, Cancel>>>,
    pub copy_builds: Rc<RefCell<IndexMap<String, Joined<SealedImageBuilt>>>>,
    pub copy_rows: RefCell<IndexMap<String, Rc<CopyRow>>>,
    pub away: RefCell<IndexSet<String>>,
    pub live: RefCell<Vec<Rc<RefCell<WorkspaceRecord>>>>,
    pub generations: RefCell<IndexMap<String, u64>>,
    pub emitted: RefCell<Vec<GoldenStageEvent>>,
    pub framed: RefCell<Vec<(String, String, String)>>,
    pub opts: RefCell<BuildersOpts>,
    pub owner: RefCell<String>,
    pub closed: Cell<bool>,
    pub beat: RefCell<Option<Cancel>>,
    pub broker: Cell<bool>,
    pub refresh_fails: RefCell<Option<HostError>>,
    pub gone_stop_fails: RefCell<IndexMap<String, HostError>>,
    pub backend_for_fails: Cell<bool>,
    pub daemon_notes: RefCell<IndexMap<String, String>>,
    pub prepared_views: RefCell<VecDeque<ImageResult<GoldenBuilderView>>>,
    /// What `GoldenCtx::seal_entry` and `ImageCtx::seal_entry` answer, in order.
    pub seals: RefCell<VecDeque<ImageResult<SealResult>>>,
    /// What `GoldenCtx::settle_builder` hands back: the placeholder, else a new own entry.
    pub settled: RefCell<Vec<String>>,
    pub expired: RefCell<Option<ImageResult<Expired>>>,
    pub spawned: RefCell<Vec<LocalBoxFuture<'static, ()>>>,
    /// A gate `ImageCtx::golden_prepare` waits on when set, so a copy build stays in flight while a test acts.
    pub copy_gate: RefCell<Option<futures::future::Shared<LocalBoxFuture<'static, ()>>>>,
    /// Runs when `GoldenCtx::seal_entry` is called, before it answers, so a test can write the copy a seal leaves.
    pub seal_hook: RefCell<Option<Rc<dyn Fn()>>>,
    /// The host's image mark and the ids anything records, as the golden context answers them.
    pub mark: RefCell<String>,
    pub recorded: RefCell<IndexSet<String>>,
    /// Snapshot ids whose `drop_image` fails, with the failure.
    pub drop_image_fails: RefCell<IndexMap<String, ImageError>>,
    /// A placeholder `recording_creates` hands its `made` callback, as a create does the moment it lands.
    pub placeholder_made: RefCell<Option<BuilderRef>>,
    /// The recipe hash `import_mark` reads off any import.
    pub import_hash: RefCell<String>,
    /// The `made` callback the golden area handed `recording_creates` last.
    pub made: RefCell<Option<Rc<dyn Fn(BuilderRef)>>>,
}

impl World {
    /// A world whose wired provider is `boat`, building images, with no door.
    pub fn new() -> Rc<World> {
        World::on(FakeBackend::new("boat"))
    }
    /// A world whose wired provider `boat` is this backend.
    pub fn on(boat: Arc<FakeBackend>) -> Rc<World> {
        let log = Rc::new(RefCell::new(vec![]));
        let places = Rc::new(FakePlaces { wired: RefCell::new("boat".into()), rows: RefCell::new(IndexMap::new()) });
        places.rows.borrow_mut().insert("boat".into(), boat.clone());
        Rc::new(World {
            store: MemStore::new(),
            clock: TestClock::at(NOW),
            warned: Warned::default(),
            roads: FakeRoads::new(log.clone()),
            log,
            places,
            door: RefCell::new(None),
            runtime_backend: boat,
            builders: Rc::new(RefCell::new(IndexMap::new())),
            stage_at: Rc::new(RefCell::new(IndexMap::new())),
            preparing: Rc::new(RefCell::new(IndexMap::new())),
            grace_timers: Rc::new(RefCell::new(IndexMap::new())),
            copy_builds: Rc::new(RefCell::new(IndexMap::new())),
            copy_rows: RefCell::new(IndexMap::new()),
            away: RefCell::new(IndexSet::new()),
            live: RefCell::new(vec![]),
            generations: RefCell::new(IndexMap::new()),
            emitted: RefCell::new(vec![]),
            framed: RefCell::new(vec![]),
            opts: RefCell::new(BuildersOpts::default()),
            owner: RefCell::new("owner-1".into()),
            closed: Cell::new(false),
            beat: RefCell::new(None),
            broker: Cell::new(false),
            refresh_fails: RefCell::new(None),
            gone_stop_fails: RefCell::new(IndexMap::new()),
            backend_for_fails: Cell::new(false),
            daemon_notes: RefCell::new(IndexMap::new()),
            prepared_views: RefCell::new(VecDeque::new()),
            seals: RefCell::new(VecDeque::new()),
            settled: RefCell::new(vec![]),
            expired: RefCell::new(None),
            spawned: RefCell::new(vec![]),
            mark: RefCell::new("mark-1".into()),
            recorded: RefCell::new(["snap-recorded".to_string()].into_iter().collect()),
            drop_image_fails: RefCell::new(IndexMap::new()),
            placeholder_made: RefCell::new(None),
            copy_gate: RefCell::new(None),
            seal_hook: RefCell::new(None),
            import_hash: RefCell::new("h1".into()),
            made: RefCell::new(None),
        })
    }
    pub fn with_door(&self) -> Rc<FakeDoor> {
        let d = FakeDoor::new(self.log.clone());
        *self.door.borrow_mut() = Some(d.clone());
        d
    }
    pub fn add_builder(&self, b: BuilderRef) {
        let id = b.borrow().record.id.clone();
        self.builders.borrow_mut().insert(id, b);
    }
    pub fn note(&self, line: impl Into<String>) {
        self.log.borrow_mut().push(line.into());
    }
    pub fn log(&self) -> Vec<String> {
        self.log.borrow().clone()
    }
    /// The log lines that start with `prefix`.
    pub fn lines(&self, prefix: &str) -> Vec<String> {
        self.log.borrow().iter().filter(|l| l.starts_with(prefix)).cloned().collect()
    }
    pub fn put_copy_now(&self, place: &str, name: &str, m: &GoldenManifest) {
        self.store.seed("goldens", &format!("{place}/{name}"), serde_json::to_value(m).expect("manifest json"));
    }
    pub fn put_image_now(&self, image: &SealedImage) {
        self.store.seed("images", &image.name, serde_json::to_value(image).expect("image json"));
    }
    pub fn copy_now(&self, place: &str, name: &str) -> Option<GoldenManifest> {
        self.store.doc("goldens", &format!("{place}/{name}")).map(|v| serde_json::from_value(v).expect("manifest"))
    }
    fn backend_by_place(&self, place: &str) -> Option<Arc<dyn MachineBackend>> {
        self.places.backend(place).or_else(|| self.door.borrow().as_ref().and_then(|d| d.backend_of(place)))
    }
}

fn read<T: serde::de::DeserializeOwned>(v: Option<Value>) -> Option<T> {
    v.and_then(|v| serde_json::from_value(v).ok())
}

#[async_trait(?Send)]
impl RecordsCtx for World {
    fn store(&self) -> Rc<dyn Store> {
        self.store.clone()
    }
    fn clock(&self) -> Rc<dyn Clock> {
        self.clock.clone()
    }
    fn wired(&self) -> String {
        self.places.wired()
    }
    fn warn(&self, line: &str) {
        self.warned.push(line);
    }
    fn module_folder(&self, r: &WorkspaceRecord) -> Option<String> {
        self.note(format!("module_folder {}", r.id));
        Some(format!("/root/{}", r.name))
    }
    fn home_of(&self, _r: &WorkspaceRecord) -> Option<String> {
        Some("/root".into())
    }
    fn project_ref(&self, project: &str) -> ProjectView {
        serde_json::from_value(json!({"id": project, "name": "spoo", "path": "/root/spoo", "computer": "here", "createdAt": "2026-10-01T00:00:00.000Z"}))
            .unwrap_or_else(|e| panic!("a project view: {e}"))
    }
    fn daemon_note(&self, workspace_id: &str) -> Option<String> {
        self.daemon_notes.borrow().get(workspace_id).cloned()
    }
    fn agents_of(&self, _r: &WorkspaceRecord) -> Option<Value> {
        None
    }
    fn provider_of(&self, _r: &WorkspaceRecord) -> Option<String> {
        Some("boat".into())
    }
    fn backend_for(&self, _r: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError> {
        if self.backend_for_fails.get() {
            return Err(HostError::new("the daemon has not said what it forks with"));
        }
        Ok(self.runtime_backend.clone())
    }
    fn live_records(&self) -> Vec<Rc<RefCell<WorkspaceRecord>>> {
        self.live.borrow().clone()
    }
    fn bump_generation(&self, workspace_id: &str) -> bool {
        self.note(format!("bump_generation {workspace_id}"));
        let live = self.live.borrow().iter().any(|r| r.borrow().id == workspace_id);
        if live {
            *self.generations.borrow_mut().entry(workspace_id.into()).or_default() += 1;
        }
        live
    }
}

#[async_trait(?Send)]
impl HeldCtx for World {
    fn store(&self) -> Rc<dyn Store> {
        self.store.clone()
    }
    fn wired(&self) -> String {
        self.places.wired()
    }
    async fn ready(&self) -> Result<(), HostError> {
        self.note("ready");
        Ok(())
    }
    async fn drop_copy_recipe(&self, place: &str, name: &str, version: u32) -> Result<(), HostError> {
        self.note(format!("drop_copy_recipe {place} {name} {version}"));
        self.store.delete("golden-recipes", &format!("{place}/{name}@v{version}")).await
    }
}

#[async_trait(?Send)]
impl BuildersCtx for World {
    fn store(&self) -> Rc<dyn Store> {
        self.store.clone()
    }
    fn clock(&self) -> Rc<dyn Clock> {
        self.clock.clone()
    }
    fn roads(&self) -> Rc<dyn GoldenRoads> {
        self.roads.clone()
    }
    fn opts(&self) -> BuildersOpts {
        self.opts.borrow().clone()
    }
    fn places(&self) -> Rc<dyn PlaceBackends> {
        self.places.clone()
    }
    fn door(&self) -> Option<Rc<dyn ImageDoor>> {
        self.door.borrow().clone().map(|d| d as Rc<dyn ImageDoor>)
    }
    fn place_door_of(&self) -> Result<Rc<dyn ImageDoor>, HostError> {
        self.door.borrow().clone().map(|d| d as Rc<dyn ImageDoor>).ok_or_else(|| HostError::new("this runtime has no place door"))
    }
    fn backend(&self) -> Arc<dyn MachineBackend> {
        self.runtime_backend.clone()
    }
    fn backend_of_kind_cloud(&self, place_id: Option<&str>) -> Arc<dyn MachineBackend> {
        self.note(format!("backend_of_kind cloud {}", place_id.unwrap_or("-")));
        place_id.and_then(|p| self.backend_by_place(p)).unwrap_or_else(|| self.runtime_backend.clone())
    }
    fn broker_ready(&self) -> bool {
        self.broker.get()
    }
    fn host_id(&self) -> String {
        "host-1".into()
    }
    fn pid(&self) -> u32 {
        4242
    }
    fn hostname(&self) -> String {
        "laptop".into()
    }
    fn owner(&self) -> String {
        self.owner.borrow().clone()
    }
    fn closed(&self) -> bool {
        self.closed.get()
    }
    fn beat(&self) -> Option<Cancel> {
        self.beat.borrow().clone()
    }
    fn set_beat(&self, beat: Option<Cancel>) {
        *self.beat.borrow_mut() = beat;
    }
    fn set_ticking(&self, ticking: LocalBoxFuture<'static, ()>) {
        self.note("set_ticking");
        self.spawned.borrow_mut().push(ticking);
    }
    fn warn(&self, line: &str) {
        self.warned.push(line);
    }
    fn emit_stage(&self, event: GoldenStageEvent) {
        self.emitted.borrow_mut().push(event);
    }
    fn spawn(&self, f: LocalBoxFuture<'static, ()>) {
        self.note("spawn");
        self.spawned.borrow_mut().push(f);
    }
    fn builders(&self) -> Builders {
        self.builders.clone()
    }
    fn stage_at(&self) -> Rc<RefCell<IndexMap<String, StageAt>>> {
        self.stage_at.clone()
    }
    fn image_mark(&self) -> String {
        self.mark.borrow().clone()
    }
    async fn refresh_builders(&self) -> Result<(), HostError> {
        self.note("refresh_builders");
        // The TS awaits here, so a second caller in the same tick finds the first pass in flight.
        tokio::task::yield_now().await;
        match self.refresh_fails.borrow().clone() {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
    async fn gone_stop(&self, _at: Arc<dyn MachineBackend>, machine: Arc<dyn Machine>) -> Result<(), HostError> {
        self.note(format!("gone_stop {}", machine.id()));
        match self.gone_stop_fails.borrow().get(machine.id()) {
            Some(e) => Err(e.clone()),
            None => Ok(()),
        }
    }
    fn claiming(&self, purpose: &str, at: Option<Arc<dyn MachineBackend>>, place: Option<&str>) -> Rc<dyn Claim> {
        self.note(format!("claim {purpose} @{}", place.unwrap_or("-")));
        Rc::new(FakeClaim { at: at.unwrap_or_else(|| self.runtime_backend.clone()) })
    }
    fn observed(&self, machine: Arc<dyn Machine>) -> Arc<dyn Machine> {
        self.note(format!("observed {}", machine.id()));
        machine
    }
    fn observing(&self, backend: Arc<dyn MachineBackend>) -> Arc<dyn MachineBackend> {
        self.note("observing");
        backend
    }
    async fn record_of(&self, name: &str) -> Result<Option<SealedImage>, HostError> {
        Ok(read(self.store.doc("images", name)))
    }
    async fn copy_of(&self, place: &str, name: &str) -> Result<Option<GoldenManifest>, HostError> {
        Ok(self.copy_now(place, name))
    }
    async fn put_copy(&self, place: &str, name: &str, manifest: &GoldenManifest) -> Result<(), HostError> {
        self.note(format!("put_copy {place} {name}"));
        self.store.put("goldens", &format!("{place}/{name}"), serde_json::to_value(manifest).expect("json")).await
    }
    async fn put_copy_recipe(&self, place: &str, name: &str, version: u32, digest: &RecipeDigest) -> Result<(), HostError> {
        self.note(format!("put_copy_recipe {place} {name} {version}"));
        self.store.put("golden-recipes", &format!("{place}/{name}@v{version}"), serde_json::to_value(digest).expect("json")).await
    }
}

/// A view of a builder as the fake answers it: id, name and place only, enough to tell which builder was shown.
pub fn view_of(entry: &BuilderRef) -> GoldenBuilderView {
    let b = entry.borrow();
    serde_json::from_value(json!({
        "id": b.record.id, "name": b.record.name, "kind": "sandbox", "createdAt": b.record.created_at,
        "placeId": b.record.place.clone().unwrap_or_default(), "placeName": b.record.place.clone().unwrap_or_default(),
        "size": {"cpu": 2, "memMb": 4096}, "sealable": b.sealable
    }))
    .unwrap_or_else(|e| panic!("a builder view: {e}"))
}

#[async_trait(?Send)]
impl GoldenCtx for World {
    fn store(&self) -> Rc<dyn Store> {
        self.store.clone()
    }
    fn clock(&self) -> Rc<dyn Clock> {
        self.clock.clone()
    }
    fn roads(&self) -> Rc<dyn GoldenRoads> {
        self.roads.clone()
    }
    fn places(&self) -> Rc<dyn PlaceBackends> {
        self.places.clone()
    }
    fn door(&self) -> Option<Rc<dyn ImageDoor>> {
        self.door.borrow().clone().map(|d| d as Rc<dyn ImageDoor>)
    }
    fn backend(&self) -> Arc<dyn MachineBackend> {
        self.runtime_backend.clone()
    }
    fn kill_confirm(&self) -> Option<KillConfirm> {
        None
    }
    fn owner(&self) -> String {
        self.owner.borrow().clone()
    }
    fn image_mark(&self) -> String {
        self.mark.borrow().clone()
    }
    fn import_mark(&self, _imp: &GoldenImport) -> ImportMark {
        ImportMark { recipe_hash: self.import_hash.borrow().clone(), recipe: None }
    }
    fn warn(&self, line: &str) {
        self.warned.push(line);
    }
    fn builders(&self) -> Builders {
        self.builders.clone()
    }
    fn preparing(&self) -> Rc<RefCell<IndexMap<String, Preparing>>> {
        self.preparing.clone()
    }
    fn grace_timers(&self) -> Rc<RefCell<IndexMap<String, Cancel>>> {
        self.grace_timers.clone()
    }
    async fn ready(&self) -> Result<(), HostError> {
        self.note("ready");
        Ok(())
    }
    async fn refresh_builders(&self) -> Result<(), HostError> {
        self.note("refresh_builders");
        Ok(())
    }
    fn claiming(&self, purpose: &str, at: Option<Arc<dyn MachineBackend>>, place: Option<&str>) -> Rc<dyn Claim> {
        self.note(format!("claim {purpose} @{}", place.unwrap_or("-")));
        Rc::new(FakeClaim { at: at.unwrap_or_else(|| self.runtime_backend.clone()) })
    }
    async fn daemon_token_of(&self, machine: Arc<dyn Machine>) -> Result<Option<String>, HostError> {
        Ok(Some(format!("token-{}", machine.id())))
    }
    fn conflict(&self, message: &str) -> HostError {
        HostError { kind: Some(Kind::Conflict), ..HostError::new(message) }
    }
    async fn record_of(&self, name: &str) -> Result<Option<SealedImage>, HostError> {
        Ok(read(self.store.doc("images", name)))
    }
    async fn copy_of(&self, place: &str, name: &str) -> Result<Option<GoldenManifest>, HostError> {
        Ok(self.copy_now(place, name))
    }
    async fn put_copy(&self, place: &str, name: &str, manifest: &GoldenManifest) -> Result<(), HostError> {
        self.note(format!("put_copy {place} {name}"));
        self.store.put("goldens", &format!("{place}/{name}"), serde_json::to_value(manifest).expect("json")).await
    }
    async fn copy_recipe_of(&self, place: &str, name: &str, version: u32) -> Result<Option<RecipeDigest>, HostError> {
        Ok(read(self.store.doc("golden-recipes", &format!("{place}/{name}@v{version}"))))
    }
    async fn drop_copy_recipe(&self, place: &str, name: &str, version: u32) -> Result<(), HostError> {
        self.note(format!("drop_copy_recipe {place} {name} {version}"));
        self.store.delete("golden-recipes", &format!("{place}/{name}@v{version}")).await
    }
    async fn recorded_images(&self) -> Result<IndexSet<String>, HostError> {
        self.note("recorded_images");
        Ok(self.recorded.borrow().clone())
    }
    fn forked_from(&self, snapshot_id: &str) -> Vec<String> {
        self.live.borrow().iter().filter(|r| r.borrow().golden == snapshot_id).map(|r| r.borrow().name.clone()).collect()
    }
    async fn image_place(&self, name: &str) -> Result<String, HostError> {
        let image: Option<SealedImage> = read(self.store.doc("images", name));
        Ok(image.and_then(|i| i.place).unwrap_or_else(|| self.places.wired()))
    }
    async fn place_at(&self, word: &str) -> Result<PlaceAt, HostError> {
        self.note(format!("place_at {word}"));
        self.backend_by_place(word).map(|at| PlaceAt { place: word.into(), at }).ok_or_else(|| HostError { kind: Some(Kind::Missing), ..HostError::new(format!("no place {word}")) })
    }
    fn backend_at(&self, place: &str) -> Result<Arc<dyn MachineBackend>, HostError> {
        self.backend_by_place(place).ok_or_else(|| HostError { kind: Some(Kind::Missing), ..HostError::new(format!("no place {place}")) })
    }
    fn place_name(&self, place: &str) -> String {
        self.door.borrow().as_ref().and_then(|d| d.name_of(place)).unwrap_or_else(|| place.to_string())
    }
    fn place_kind_of(&self, place: &str) -> PlaceKind {
        if self.places.rows.borrow().contains_key(place) { PlaceKind::Provider } else { PlaceKind::Computer }
    }
    async fn build_places(&self) -> Result<Vec<BuildPlace>, HostError> {
        self.note("build_places");
        let mut rows = vec![];
        for (id, b) in self.places.rows.borrow().iter() {
            if !b.caps.borrow().sizes.is_empty() {
                rows.push(BuildPlace { place: id.clone(), name: id.clone(), kind: PlaceKind::Provider, backend: b.clone() });
            }
        }
        if let Some(d) = self.door.borrow().as_ref() {
            for (id, b) in d.backends.borrow().iter() {
                let name = d.names.borrow().get(id).cloned().unwrap_or_default();
                rows.push(BuildPlace { place: id.clone(), name, kind: PlaceKind::Computer, backend: b.clone() });
            }
        }
        Ok(rows)
    }
    fn recipe_or_throw(&self, named: Option<GoldenRecipe>) -> Result<GoldenRecipe, HostError> {
        named.or_else(|| self.opts.borrow().golden_recipe.clone()).ok_or_else(|| HostError::new("this runtime has no golden recipe; the host wires one (setup + smoke) before the wizard can run"))
    }
    fn builder_labels(&self, extra: Option<&IndexMap<String, String>>) -> IndexMap<String, String> {
        let mut l = extra.cloned().unwrap_or_default();
        l.insert("wsp-builder".into(), "1".into());
        l
    }
    fn stage_of(&self, name: &str, on: Option<&str>, named: bool) -> StageFn {
        self.note(format!("stage_of {name} {} {named}", on.unwrap_or("-")));
        let log = self.log.clone();
        Rc::new(move |stage, detail, _step, _left| {
            log.borrow_mut().push(format!("stage {} {}", serde_json::to_value(stage).unwrap_or_default().as_str().unwrap_or("?"), detail.unwrap_or_default()));
        })
    }
    fn recording_creates(&self, b: Arc<dyn MachineBackend>, name: &str, imp: Option<ImportMark>, made: Rc<dyn Fn(BuilderRef)>, stop: Option<CreateStop>, place: &str) -> Arc<dyn MachineBackend> {
        *self.made.borrow_mut() = Some(made.clone());
        let placeholder = self.placeholder_made.borrow().clone();
        if let Some(p) = placeholder {
            made(p);
        }
        self.note(format!("recording_creates {name} {place} import={} stop={}", imp.map(|i| i.recipe_hash).unwrap_or_default(), stop.is_some()));
        b
    }
    async fn settle_builder(&self, name: &str, builder: EngineBuilder, placeholder: Option<BuilderRef>, place: &str) -> Result<BuilderRef, HostError> {
        self.note(format!("settle_builder {name} {} {place}", builder.machine.id()));
        let entry = placeholder.unwrap_or_else(|| live_builder(builder.machine.id(), name, place, BuilderLife::Own));
        entry.borrow_mut().builder = builder;
        let id = entry.borrow().record.id.clone();
        self.builders.borrow_mut().insert(id, entry.clone());
        Ok(entry)
    }
    async fn seal_entry(&self, entry: &BuilderRef, keep: bool, logins: Option<Vec<GoldenLogin>>, copy: Option<SealedImage>) -> ImageResult<SealResult> {
        self.note(format!("seal_entry {} keep={keep} logins={} copy={}", entry.borrow().record.id, logins.map(|l| l.len()).unwrap_or(0), copy.map(|c| c.hash).unwrap_or_default()));
        let hook = self.seal_hook.borrow().clone();
        if let Some(h) = hook {
            h();
        }
        self.seals.borrow_mut().pop_front().unwrap_or_else(|| Err(ImageError::Host(HostError::new("seal_entry is not scripted"))))
    }
    async fn hold(&self, b: &BuilderRef) -> Result<(), HostError> {
        self.note(format!("hold {}", b.borrow().record.id));
        Ok(())
    }
    async fn forget_builder(&self, id: &str) -> Result<(), HostError> {
        self.note(format!("forget_builder {id}"));
        self.builders.borrow_mut().shift_remove(id);
        Ok(())
    }
    async fn forget_if_gone(&self, entry: &BuilderRef, _at: Arc<dyn MachineBackend>) -> Result<(), HostError> {
        self.note(format!("forget_if_gone {}", entry.borrow().record.id));
        Ok(())
    }
    fn builder_view(&self, entry: &BuilderRef) -> GoldenBuilderView {
        view_of(entry)
    }
    fn refuse_untouchable(&self, entry: &BuilderRef) -> Result<(), HostError> {
        self.note(format!("refuse_untouchable {}", entry.borrow().record.id));
        match entry.borrow().life {
            BuilderLife::Foreign | BuilderLife::Held => Err(HostError::new(format!("{} is untouchable", entry.borrow().record.id))),
            _ => Ok(()),
        }
    }
    async fn expire_grace(&self) -> ImageResult<Expired> {
        self.note("expire_grace");
        self.expired.borrow_mut().take().unwrap_or_else(|| Ok(Expired::default()))
    }
    fn in_window(&self, sealed_at: &str) -> bool {
        sealed_at >= "2026-10-25T08:51:00.000Z"
    }
    async fn drop_image(&self, v: &GoldenVersion, _at: Option<Arc<dyn MachineBackend>>) -> ImageResult<()> {
        self.note(format!("drop_image {}", v.snapshot_id));
        if let Some(e) = self.drop_image_fails.borrow().get(&v.snapshot_id) {
            return Err(e.clone());
        }
        match self.roads.drop_template_fails.borrow().clone() {
            Some(e) => Err(ImageError::Engine(e)),
            None => Ok(()),
        }
    }
}

#[async_trait(?Send)]
impl ImageCtx for World {
    fn store(&self) -> Rc<dyn Store> {
        self.store.clone()
    }
    fn roads(&self) -> Rc<dyn GoldenRoads> {
        self.roads.clone()
    }
    fn places(&self) -> Rc<dyn PlaceBackends> {
        self.places.clone()
    }
    fn door(&self) -> Option<Rc<dyn ImageDoor>> {
        self.door.borrow().clone().map(|d| d as Rc<dyn ImageDoor>)
    }
    fn kill_confirm(&self) -> Option<KillConfirm> {
        None
    }
    fn builders(&self) -> Builders {
        self.builders.clone()
    }
    fn copy_builds(&self) -> Rc<RefCell<IndexMap<String, Joined<SealedImageBuilt>>>> {
        self.copy_builds.clone()
    }
    fn copy_row(&self, place: &str) -> Option<Rc<CopyRow>> {
        self.copy_rows.borrow().get(place).cloned()
    }
    fn row_says_failure(&self, place: &str, _e: &ImageError) -> bool {
        self.note(format!("row_says_failure {place}"));
        true
    }
    fn place_away(&self, place: &str) -> bool {
        self.away.borrow().contains(place)
    }
    fn frame_stopped(&self, place: &str, name: &str, why: Stopped) {
        let why = match why {
            Stopped::Line(l) => l,
            Stopped::Error(e) => format!("error: {e:?}"),
        };
        self.framed.borrow_mut().push((place.into(), name.into(), why));
    }
    async fn ready(&self) -> Result<(), HostError> {
        self.note("ready");
        Ok(())
    }
    async fn record_of(&self, name: &str) -> Result<Option<SealedImage>, HostError> {
        Ok(read(self.store.doc("images", name)))
    }
    async fn copy_of(&self, place: &str, name: &str) -> Result<Option<GoldenManifest>, HostError> {
        Ok(self.copy_now(place, name))
    }
    async fn copy_recipe_of(&self, place: &str, name: &str, version: u32) -> Result<Option<RecipeDigest>, HostError> {
        Ok(read(self.store.doc("golden-recipes", &format!("{place}/{name}@v{version}"))))
    }
    async fn put_copy_recipe(&self, place: &str, name: &str, version: u32, digest: &RecipeDigest) -> Result<(), HostError> {
        self.note(format!("put_copy_recipe {place} {name} {version}"));
        self.store.put("golden-recipes", &format!("{place}/{name}@v{version}"), serde_json::to_value(digest).expect("json")).await
    }
    async fn image_of(&self, snapshot_id: &str) -> Result<ImageOf, HostError> {
        for raw in self.store.list("goldens").await? {
            let m: GoldenManifest = serde_json::from_value(raw).expect("manifest");
            if let Some(v) = m.versions.iter().find(|v| v.snapshot_id == snapshot_id) {
                return Ok(ImageOf { golden: snapshot_id.into(), version: Some(v.clone()) });
            }
        }
        Ok(ImageOf { golden: snapshot_id.into(), version: None })
    }
    async fn place_at(&self, word: &str) -> Result<PlaceAt, HostError> {
        GoldenCtx::place_at(self, word).await
    }
    fn place_name(&self, place: &str) -> String {
        GoldenCtx::place_name(self, place)
    }
    fn place_kind_of(&self, place: &str) -> PlaceKind {
        GoldenCtx::place_kind_of(self, place)
    }
    async fn image_place(&self, name: &str) -> Result<String, HostError> {
        GoldenCtx::image_place(self, name).await
    }
    fn copy_recipe_or_throw(&self) -> Result<CopyRecipe, HostError> {
        self.opts.borrow().copy_recipe.clone().ok_or_else(|| HostError::new("this runtime cannot compose the recipe a copy builds from; the host that serves the app wires one"))
    }
    fn stage_of(&self, name: &str, on: Option<&str>, named: bool) -> StageFn {
        GoldenCtx::stage_of(self, name, on, named)
    }
    async fn forget_if_gone(&self, entry: &BuilderRef, at: Arc<dyn MachineBackend>) -> Result<(), HostError> {
        GoldenCtx::forget_if_gone(self, entry, at).await
    }
    async fn seal_entry(&self, entry: &BuilderRef, keep: bool, logins: Option<Vec<GoldenLogin>>, copy: Option<SealedImage>) -> ImageResult<SealResult> {
        GoldenCtx::seal_entry(self, entry, keep, logins, copy).await
    }
    async fn golden_prepare(&self, ask: PrepareAsk) -> ImageResult<GoldenBuilderView> {
        self.note(format!("golden_prepare {} {} copy={}", ask.name.unwrap_or_default(), ask.place.unwrap_or_default(), ask.copy == Some(true)));
        let gate = self.copy_gate.borrow().clone();
        if let Some(g) = gate {
            g.await;
        }
        self.prepared_views.borrow_mut().pop_front().unwrap_or_else(|| Err(ImageError::Host(HostError::new("golden_prepare is not scripted"))))
    }
}

/// A seal's result for version `v` of snapshot `snap`, its manifest holding `versions` and it.
pub fn seal_result(v: u32, snap: &str, kept: bool) -> SealResult {
    SealResult { manifest: manifest(&[(v, snap, None)]), version: version(v, snap, None), builder_kept: kept, vault: None }
}

// --- appended for the golden storage and upgrade tests ------------------------------------------------------------------

/// Boat's storage pricing: no GB price, ten named snapshots free, then $1.70 a month each (box-backend.ts:62).
pub fn boat_pricing() -> SnapshotStoragePricing {
    SnapshotStoragePricing {
        wire: serde_json::from_value(json!({"freeGb": 0, "usdPerGbMonth": 0, "billedFrom": ""})).expect("pricing"),
        free_named: Some(10),
        usd_per_named_month: Some(1.7),
    }
}

/// Storage priced by the GB past ten free, as the TS stub backend bills it.
pub fn gb_pricing() -> SnapshotStoragePricing {
    SnapshotStoragePricing {
        wire: serde_json::from_value(json!({"freeGb": 10, "usdPerGbMonth": 0.05, "billedFrom": "2026-10-01"})).expect("pricing"),
        free_named: None,
        usd_per_named_month: None,
    }
}

/// A snapshot row stamped with the time it was taken.
pub fn snapshot_at(id: &str, name: Option<&str>, size: f64, created_at: &str) -> SnapshotRow {
    SnapshotRow { created_at: Some(created_at.into()), ..snapshot_row(id, name, size, None) }
}

/// A ready template row.
pub fn template_row(id: &str, name: &str, created_at: Option<&str>) -> TemplateRow {
    let mut j = json!({"id": id, "name": name, "status": "ready"});
    if let Some(c) = created_at {
        j["createdAt"] = json!(c);
    }
    serde_json::from_value(j).expect("a template row")
}

impl FakeTemplates {
    pub fn new(log: Rc<RefCell<Vec<String>>>, rows: Vec<TemplateRow>) -> Rc<FakeTemplates> {
        Rc::new(FakeTemplates { log, rows: RefCell::new(rows), delete_fails: RefCell::new(IndexMap::new()) })
    }
}

impl World {
    /// Wires a template road onto the engine roads, logging into the world's log.
    pub fn with_templates(&self, rows: Vec<TemplateRow>) -> Rc<FakeTemplates> {
        let t = FakeTemplates::new(self.log.clone(), rows);
        *self.roads.templates.borrow_mut() = Some(t.clone());
        t
    }
    /// Reads each listing and snapshot delete of the runtime backend into the world's log.
    pub fn trace_backend(&self) {
        *self.runtime_backend.trace.borrow_mut() = Some(self.log.clone());
    }
    /// A live workspace forked from `golden`, named `name`.
    pub fn add_workspace(&self, id: &str, name: &str, golden: &str) {
        self.live.borrow_mut().push(Rc::new(RefCell::new(workspace_record(id, name, golden))));
    }
    /// Files a copy of a manifest at a place with a recipe digest per version.
    pub fn put_recipe_now(&self, place: &str, name: &str, version: u32) {
        self.store.seed("golden-recipes", &format!("{place}/{name}@v{version}"), json!({"ticks": [], "files": []}));
    }
    pub fn recipe_now(&self, place: &str, name: &str, version: u32) -> Option<Value> {
        self.store.doc("golden-recipes", &format!("{place}/{name}@v{version}"))
    }
}
