// SPDX-License-Identifier: AGPL-3.0-only
//! Stand-ins for the wsp-core types the machines-b areas read (#2448, comment 6400338016): `WorkspaceRecord`,
//! `LiveWorkspace`, `SessionEntry`, `SubStartRecord` and the kind module, ported off `types/wiring.ts` and
//! `types/internal.ts` at 65f6b89c7 with only the fields these areas read. A stored record's other fields ride in
//! `rest`, so a write keeps them. Once wsp-core has these types this file becomes `pub use` lines and nothing that
//! imports from here changes.
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::future::{LocalBoxFuture, Shared};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use wsp_engine::machine::{Machine, MachineBackend};
use wsp_frames::True;
use wsp_wire::types::changes::Checkout;
use wsp_wire::types::tree::{TreeFact, TreeRecord};
use wsp_wire::views::session::SessionView;
use wsp_wire::views::sub_row::{SubResult, SubSpawn};
use wsp_wire::views::workspace::{BranchOfRecord, DaemonReachView, WorkspacePhase};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::ThreadScope;
use wsp_words::exit::HostError;

/// A future a second caller joins (rule 3.9's join), on the one host thread.
pub type Joined<T> = Shared<LocalBoxFuture<'static, T>>;

/// `{ cpu, memMb }` on a record.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordSize {
    pub cpu: f64,
    pub mem_mb: f64,
}

/// types/wiring.ts `lead`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeadRef {
    pub thread_id: String,
    pub workspace_id: String,
}

/// types/wiring.ts `queue`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueMark {
    pub since: String,
    pub why: String,
}

/// types/wiring.ts `sub.base`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubBase {
    pub branch: String,
    pub sha: String,
}

/// types/wiring.ts `sub.held`: `SubHeld & { at }`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubHeldAt {
    pub files: f64,
    pub commits: f64,
    pub at: f64,
}

/// types/wiring.ts:406 `SubSession`. `kept` is a number or `null`, never absent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SubSession {
    pub agent: String,
    pub id: String,
    pub kept: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
}

/// types/wiring.ts:378 `sub`, in the order sub-spawn.ts:239 writes it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubRecord {
    pub on: String,
    pub repo: String,
    pub base: SubBase,
    pub branch: String,
    pub folder: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_usd_per_hour: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<SubResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub held: Option<SubHeldAt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delete_at: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub released_at: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lost: Option<String>,
    /// The TS `true` or absent; a stored `false` reads as absent.
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "true_only")]
    pub cloud_gone: Option<True>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<SubSession>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spawn: Option<SubSpawn>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opening: Option<String>,
}

fn true_only<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<True>, D::Error> {
    Ok(match Option::<Value>::deserialize(d)? {
        Some(Value::Bool(true)) => Some(True),
        _ => None,
    })
}

/// types/wiring.ts:341 `WorkspaceRecord`, the fields these areas read, in the order sub-spawn.ts:223 writes them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRecord {
    pub id: String,
    pub name: String,
    pub kind: WorkspaceKind,
    #[serde(default)]
    pub machine_id: String,
    pub phase: WorkspacePhase,
    #[serde(default)]
    pub golden: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub project: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    pub size: RecordSize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_life: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lead: Option<LeadRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_of_record: Option<BranchOfRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub: Option<SubRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub queue: Option<QueueMark>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tree: Option<TreeRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkout: Option<Checkout>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_gone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub napped_at: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub npm_bin: Option<String>,
    /// Every other field of the stored record, kept as it was read.
    #[serde(flatten)]
    pub rest: IndexMap<String, Value>,
}

/// The road a workspace's daemon is reached by (the engine's `Workspace.daemonReach`).
#[async_trait(?Send)]
pub trait WorkspaceRoad {
    async fn daemon_reach(&self) -> Result<DaemonReachView, HostError>;
}

/// types/wiring.ts:453 `deleteSaid`.
#[derive(Clone, Debug, PartialEq)]
pub struct DeleteSaid {
    pub phase: WorkspacePhase,
    pub line: String,
}

/// types/wiring.ts:416 `LiveWorkspace`. The record is shared with every entry `attach` makes for it, as the TS shares
/// the object; the machine is the entry's own. A second entry for the record is told apart with `Rc::ptr_eq`.
pub struct LiveWorkspace {
    pub record: Rc<RefCell<WorkspaceRecord>>,
    pub machine: Arc<dyn Machine>,
    pub ws: Rc<dyn WorkspaceRoad>,
    pub generation: Cell<u64>,
    pub of_record_held: Cell<bool>,
    pub napping: RefCell<Option<Joined<Result<(), String>>>>,
    pub waking: RefCell<Option<Joined<Result<(), String>>>>,
    pub late_read: RefCell<Option<Rc<dyn Fn()>>>,
    pub wake_stop: RefCell<Option<CancellationToken>>,
    pub wake_said: RefCell<Option<String>>,
    pub wake_ask: RefCell<Option<Value>>,
    pub delete_said: RefCell<Option<DeleteSaid>>,
    pub creating: Cell<bool>,
    pub checkout: RefCell<Option<Checkout>>,
    pub checkout_reading: RefCell<Option<Joined<Option<Checkout>>>>,
    pub tree: RefCell<Option<TreeFact>>,
    pub tree_reading: RefCell<Option<Joined<Option<TreeFact>>>>,
    pub pr: RefCell<Option<Value>>,
}

pub type Live = Rc<LiveWorkspace>;

impl LiveWorkspace {
    /// An entry with every in-flight slot empty, as `attach` makes one.
    pub fn new(record: Rc<RefCell<WorkspaceRecord>>, machine: Arc<dyn Machine>, ws: Rc<dyn WorkspaceRoad>) -> Live {
        Rc::new(LiveWorkspace {
            record,
            machine,
            ws,
            generation: Cell::new(0),
            of_record_held: Cell::new(false),
            napping: RefCell::new(None),
            waking: RefCell::new(None),
            late_read: RefCell::new(None),
            wake_stop: RefCell::new(None),
            wake_said: RefCell::new(None),
            wake_ask: RefCell::new(None),
            delete_said: RefCell::new(None),
            creating: Cell::new(false),
            checkout: RefCell::new(None),
            checkout_reading: RefCell::new(None),
            tree: RefCell::new(None),
            tree_reading: RefCell::new(None),
            pr: RefCell::new(None),
        })
    }

    pub fn id(&self) -> String {
        self.record.borrow().id.clone()
    }
}

/// types/internal.ts `SessionEntry`: one row per turn.
#[derive(Clone)]
pub struct SessionEntry {
    pub view: SessionView,
    pub turn_id: String,
    pub launch: Option<Joined<()>>,
    pub queued: bool,
    pub notify: Option<Vec<String>>,
    pub notify_by: Option<ThreadScope>,
    pub notify_road: Option<Value>,
}

/// types/internal.ts `SubStartRecord`: a held spawn's start as the `sub-starts` collection keeps it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubStartRecord {
    pub workspace_id: String,
    pub thread_id: String,
    pub turn_id: String,
    pub at: String,
    /// The `KEPT` keys of the start's options, in `KEPT` order.
    pub opts: IndexMap<String, Value>,
    pub tree: IndexMap<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notify: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<ThreadScope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub road: Option<Value>,
}

/// The stored collections these areas name (types/internal.ts).
pub const SUB_STARTS: &str = "sub-starts";
pub const SEED_CHOICES: &str = "seed-choices";
pub const CREATES: &str = "creates";
/// types/internal.ts:164, the reason a turn on a machine gone is ended with.
pub const GONE_REASON: &str = "machine gone at the provider while the agent was working";

/// The one stage report a create is told through (types/wiring.ts:482).
pub type StageReport = Rc<dyn Fn(&str, &str, Option<&Value>)>;

/// What `standUp` reports, the stage words of `WorkspaceCreateStage` as strings.
pub mod stage {
    pub const FORK_REQUESTED: &str = "fork-requested";
    pub const DAEMON_ANSWERING: &str = "daemon-answering";
    pub const PROJECT_CLONED: &str = "project-cloned";
    pub const READY: &str = "ready";
    pub const FAILED: &str = "failed";
    pub const LANDING: &str = "landing";
}

/// types/internal.ts `KindModule`: what each kind of workspace answers. Methods the TS marks optional answer `None`
/// where a kind has none; every async one is `async fn` where the TS returns a Promise.
#[async_trait(?Send)]
pub trait KindModule {
    fn backend(&self, record: &WorkspaceRecord) -> Result<Arc<dyn MachineBackend>, HostError>;
    fn folder(&self, record: &WorkspaceRecord) -> Option<String>;
    fn home(&self, entry: &Live, agent: &str) -> Result<String, HostError>;
    fn home_dir(&self, record: &WorkspaceRecord) -> Result<String, HostError>;
    fn env(&self, entry: &Live, agent: Option<&str>, home_of: &dyn Fn(&str) -> String) -> Result<IndexMap<String, String>, HostError>;
    fn login_stands(&self, entry: &Live, agent: &str) -> bool;
    fn relayed(&self) -> bool;
    fn wsp_mcp(&self, entry: &Live) -> Option<Value>;
    fn turn_reach(&self) -> Option<IndexMap<String, String>>;
    fn turn_road(&self) -> &'static str;
    fn keeps_agents(&self) -> bool;
    fn server_values(&self) -> &'static str;
    fn has_daemon(&self, entry: &Live) -> bool;
    fn shared_daemon(&self) -> bool;
    async fn daemon_road(&self, entry: &Live) -> Result<DaemonReachView, HostError>;
    fn scratch(&self, entry: &Live) -> Result<String, HostError>;
    async fn daemon_version(&self, entry: &Live) -> Result<Option<f64>, HostError>;
    async fn dropped(&self, entry: &Live) -> Result<(), HostError>;
    /// Present only where the recipe wires a deploy (cloud) .
    fn deploys_daemon(&self) -> bool {
        false
    }
    async fn deploy_daemon(&self, _entry: &Live) -> Result<(), HostError> {
        Ok(())
    }
    fn restarts_daemon(&self) -> bool {
        false
    }
    async fn import(&self, entry: &Live, o: Value, report: Rc<dyn Fn(&str, &str)>) -> Result<Value, HostError>;
    async fn roots(&self, entry: &Live, dests: &[String]) -> Result<(), HostError>;
    async fn land_project(&self, entry: &Live, project: &Value, report: StageReport) -> Result<(), HostError>;
    fn memory_key(&self, entry: &Live, agent: &str) -> Option<String>;
    /// The place kind's own: refuses a folder another login owns.
    async fn admit_folder(&self, _record: &WorkspaceRecord) -> Result<(), HostError> {
        Ok(())
    }
    /// The place kind's own: a port of that computer at the same number here.
    async fn port_reach(&self, _entry: &Live, _port: u16, _standing: bool) -> Result<Option<DaemonReachView>, HostError> {
        Ok(None)
    }
}

/// A failure as the runtime's areas pass it between them: an engine road's own error, kept whole so `fault_of` and
/// `is_missing` can read its variant, or a host error with its kind. The TS throws either through the same `catch`.
#[derive(Debug, Clone)]
pub enum AreaError {
    Engine(wsp_engine::errors::EngineError),
    Host(HostError),
}

impl AreaError {
    /// `e instanceof Error ? e.message : String(e)`: the engine error's Display, or the host error's message.
    pub fn message(&self) -> String {
        todo!()
    }
    /// The kind stamped on it: the engine error's kind string, or the host error's kind, as the TS string.
    pub fn kind(&self) -> Option<String> {
        todo!()
    }
    /// The `fix` a refusal carries; none for an engine error.
    pub fn fix(&self) -> Option<String> {
        todo!()
    }
    /// engine errors.ts `isMissing`: the engine's own reading, or a host error of kind `missing`.
    pub fn is_missing(&self) -> bool {
        todo!()
    }
    /// types/internal.ts:879 `isCapRefusal`: kind `concurrency`.
    pub fn is_cap_refusal(&self) -> bool {
        todo!()
    }
    /// engine errors.ts `failureWords` for an engine error; the message for a host error.
    pub fn failure_words(&self) -> String {
        todo!()
    }
    /// At the area's edge: the same words and kind as a `HostError`.
    pub fn into_host(self) -> HostError {
        todo!()
    }
}

impl From<HostError> for AreaError {
    fn from(e: HostError) -> Self {
        AreaError::Host(e)
    }
}

impl From<wsp_engine::errors::EngineError> for AreaError {
    fn from(e: wsp_engine::errors::EngineError) -> Self {
        AreaError::Engine(e)
    }
}

pub type AreaResult<T> = Result<T, AreaError>;

/// A one-shot cancel of a scheduled timer (the clock's `schedule` answer).
pub type Cancel = Rc<dyn Fn()>;
