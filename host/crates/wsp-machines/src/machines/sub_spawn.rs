// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/machines/sub-spawn.ts at 65f6b89c7: a sub-thread's start on a cloud. Specs:
//! spec/sub_spawn_checks.md (the record and every check before it) and spec/sub_spawn_machine.md (the machine).
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use serde_json::Value;

use wsp_engine::box_backend::BoxLimits;
use wsp_engine::machine::{Machine, MachineBackend};
use wsp_wire::views::golden_image::GoldenManifest;
use wsp_wire::views::session::ThreadCapWait;
use wsp_wire::views::workspace::{BranchOfRecord, Caller, ProjectView, SeedChoice, SeedPlan};
use wsp_wire::views::workspace_more::ThreadScope;
use wsp_words::exit::HostError;

use crate::machines::area_error::AreaResult;
use crate::machines::machines::Entry;
use wsp_core::types::wiring::StageReport;
use wsp_core::types::wiring::WorkspaceRecord;
use crate::machines::sub_life::SessionFile;

pub mod checks;
pub mod machine;

pub use checks::{SpawnAnswer, SpawnAsk, SubSpawned};

/// sub-spawn.ts:41 `Seed`: the picked files a start lands, packed, counted and named, or nothing and why.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Seed {
    pub tar: Option<Vec<u8>>,
    pub files: u64,
    pub paths: Vec<String>,
    pub bytes: u64,
    pub missing: Vec<String>,
    pub stayed: Vec<String>,
    pub why: Option<String>,
}

/// One provider row of `computerRows()` (the fields a spawn reads).
#[derive(Clone, Debug, PartialEq)]
pub struct ProviderRow {
    pub id: String,
    pub name: String,
    pub kind: String,
}

/// The image record a spawn reads (`recordOf("default")`): its logins and the recipe's agent rows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImageRecord {
    pub logins: Option<Vec<(String, String)>>,
    /// `(id, on)` of every recipe row of kind `agent`.
    pub agents: Vec<(String, bool)>,
}

/// A packed archive as the seed wiring answers it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Packed {
    pub tar: Vec<u8>,
    pub files: u64,
    pub bytes: u64,
    pub left: Vec<String>,
}

/// What a sub-thread's start reads off the runtime's context (sub-spawn.ts:44 and every `ctx.` call below it).
#[async_trait(?Send)]
pub trait SubSpawnCtx {
    fn live(&self, id: &str) -> Option<Entry>;
    fn remove_live(&self, id: &str);
    /// types/internal.ts `isAbsentMachine(entry.machine)`: the entry stands on the queue's stand-in, no machine yet.
    fn is_absent_machine(&self, entry: &Entry) -> bool;
    fn thread_records(&self) -> Vec<(String, String)>;
    fn push_head(&self, thread_id: &str);
    /// The local backend's machine (`moduleOf("local").backend(...).get(LOCAL_MACHINE_ID)`).
    async fn local_machine(&self) -> AreaResult<Arc<dyn Machine>>;
    /// `places.backend(id) ?? placeDoor?.backendOf(id)`.
    fn provider_backend(&self, id: &str) -> Option<Arc<dyn MachineBackend>>;
    async fn computer_rows(&self) -> AreaResult<Vec<ProviderRow>>;
    /// `landingPlace(rowId).placeId`.
    async fn landing_place(&self, row_id: &str) -> AreaResult<Option<String>>;
    /// `placeDoor?.settingsAt(place)` as the TS object.
    fn place_settings(&self, place: &str) -> Option<serde_json::Map<String, Value>>;
    /// `settingsAt(place)?.deleteAfterMs`.
    fn delete_after_ms(&self, place: &str) -> Option<f64>;
    fn place_name(&self, computer: &str) -> String;
    fn place_id_of(&self, record: &WorkspaceRecord) -> Option<String>;
    fn backend_for(&self, record: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError>;
    fn name_given(&self, name: &str) -> String;
    /// `spawnGuard("fork", origin)`: the release on success.
    fn spawn_guard(&self, origin: Option<&Caller>) -> Result<Box<dyn FnOnce()>, HostError>;
    fn project_held(&self, id: &str) -> ProjectView;
    async fn copy_blocked(&self, lead: &Entry) -> AreaResult<()>;
    /// One frame through the workspace's own daemon (`withDaemon`).
    async fn daemon_frame(&self, entry: &Entry, frame: Value) -> AreaResult<Value>;
    fn checkout_of(&self, record: &WorkspaceRecord) -> String;
    /// `ctx.brokered({kind, place})`.
    fn brokered(&self, kind: &str, place: Option<&str>) -> bool;
    /// branch-of-record.ts `brokeredStart(ctx, true)`.
    fn brokered_start(&self) -> bool;
    /// branch-of-record.ts `repoOfRecord(ctx, remote, name, on)` as the TS object.
    async fn repo_of_record(&self, remote: &str, project: &str, on: &str) -> AreaResult<serde_json::Map<String, Value>>;
    /// branch-of-record.ts `freeBranchName(ctx, wanted, heads)`.
    fn free_branch_name(&self, wanted: &str, heads: Option<&str>) -> String;
    async fn copy_of(&self, place: &str, image: &str) -> AreaResult<Option<GoldenManifest>>;
    async fn record_of(&self, image: &str) -> AreaResult<Option<ImageRecord>>;
    async fn place_guard(&self, place: &str) -> AreaResult<()>;
    async fn default_agent_of(&self, lead: &Entry) -> AreaResult<String>;
    /// brokered.ts `unbrokeredRefusal(harness)`.
    fn unbrokered_refusal(&self, harness: &str) -> Option<HostError>;
    fn name_refusal(&self, name: &str) -> Option<String>;
    fn tree_of(&self, scope: &ThreadScope) -> IndexMap<String, Value>;
    fn forking_add(&self, name: &str);
    fn forking_remove(&self, name: &str);
    fn supersede_failed(&self, name: &str);
    /// The queue's `enqueue` and `queueWait`.
    async fn enqueue(&self, record: &Rc<RefCell<WorkspaceRecord>>, wait: Option<ThreadCapWait>) -> AreaResult<()>;
    fn queue_wait(&self, place: &str, limits: Option<&BoxLimits>) -> Option<ThreadCapWait>;
    fn view(&self, record: &WorkspaceRecord) -> Value;
    /// `stageReporter(id, name, startedAt, askedBy)`, `askedBy` as the TS object.
    fn stage_reporter(&self, id: &str, name: &str, started_at: f64, asked_by: Option<Value>) -> StageReport;
    /// `ctx.fork(record, onMachine, undefined, report, false)`.
    async fn fork(&self, record: &Rc<RefCell<WorkspaceRecord>>, on_machine: Box<dyn Fn(Arc<dyn Machine>)>, report: StageReport) -> AreaResult<()>;
    /// `attach(record, machine)`.
    fn attach(&self, record: &Rc<RefCell<WorkspaceRecord>>, machine: Arc<dyn Machine>) -> Entry;
    /// `attach(record, deadMachine(""))`.
    fn attach_dead(&self, record: &Rc<RefCell<WorkspaceRecord>>) -> Entry;
    async fn ping_daemon(&self, entry: &Entry) -> Option<String>;
    fn sync_daemon(&self, entry: &Entry);
    async fn persist(&self, record: &WorkspaceRecord) -> AreaResult<()>;
    async fn emit_status_reachable(&self, entry: &Entry) -> AreaResult<()>;
    async fn unfork(&self, entry: &Entry) -> AreaResult<()>;
    fn broker_lift(&self, workspace_id: &str);
    async fn rows_sent(&self);
    fn session_file(&self, entry: &Entry, agent: &str, session_id: &str) -> Option<SessionFile>;
    async fn get_seed_choice(&self, folder: &str) -> AreaResult<Option<SeedChoice>>;
    async fn get_blob(&self, collection: &str, key: &str) -> AreaResult<Option<Vec<u8>>>;
    async fn delete_create(&self, key: &str) -> AreaResult<()>;
    /// Whether the host wired a seed (`opts.seed`).
    fn seeds(&self) -> bool;
    async fn homes_here(&self) -> AreaResult<Value>;
    async fn seed_plan(&self, folder: &str, homes: &Value) -> AreaResult<SeedPlan>;
    async fn seed_pack(&self, plan: &SeedPlan, files: &[String], homes: &Value) -> AreaResult<Packed>;
    /// `folderNamed(path)`.
    fn folder_named(&self, path: &str) -> String;
    fn emit(&self, event: Value);
    fn now(&self) -> f64;
    /// `randomBytes(n).toString("hex")` (the `Ids` trait).
    fn random_hex(&self, bytes: usize) -> String;
    fn spawn(&self, f: LocalBoxFuture<'static, ()>);
    fn warn(&self, line: &str);
}

/// sub-spawn.ts:43 `subSpawnArea(ctx)`.
pub struct SubSpawnArea {
    pub(crate) ctx: Rc<dyn SubSpawnCtx>,
    pub(crate) me: Weak<SubSpawnArea>,
    /// sub-spawn.ts:496. The resume in flight per record (rule 9 of sub_spawn_machine).
    pub(crate) resuming: RefCell<IndexMap<String, wsp_core::types::wiring::Joined<()>>>,
}

impl SubSpawnArea {
    pub fn new(ctx: Rc<dyn SubSpawnCtx>) -> Rc<SubSpawnArea> {
        todo!()
    }
}
