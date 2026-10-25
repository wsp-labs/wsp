// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/machine.ts. Spec: spec/machine.md.
//!
//! The seam every backend sits behind. The data a backend takes and answers is wsp-wire's; what stays here is the
//! two traits with methods, which no wire carries.
//!
//! A TS optional method (`previewUrl?`) is a trait method with a default body here, and the backend says which ones it
//! has in [`MachineOffers`] / [`BackendOffers`]: callers branch on the offer exactly where the TS reads the property.
//! A default body answers [`EngineError::Other`] with `NOT_ON_THIS_ROAD`; a backend that offers a method overrides it.
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::{Map, Value};
use tokio_util::sync::CancellationToken;

pub use wsp_wire::views::golden_image::MachineKind;
pub use wsp_wire::views::place_more::CloudLimits;
pub use wsp_wire::views::workspace::{MachineFacts, MachineState};
pub use wsp_wire::wire::capabilities::{Capabilities, PlaceCapacity};
pub use wsp_wire::wire::machine_link::{
    DaemonSupervisor, ExecResult, LifecycleBudgets, MachineLife, MachineSpecOnIdle, MachineListRow, MachineShape, MachineSpec, PreviewReach,
    SnapshotRow, SnapshotStoragePricing as WireSnapshotStoragePricing, TemplateRow,
    BackendFactsBaseTemplates as BaseTemplates,
};

use crate::box_backend::BoxLimits;
use crate::errors::{EngineError, Result};

/// machine.ts:27. Storage as the provider bills it, and for a provider that bills named snapshots by count (Boat)
/// the free count and the price of each past it.
#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotStoragePricing {
    pub wire: WireSnapshotStoragePricing,
    pub free_named: Option<u32>,
    pub usd_per_named_month: Option<f64>,
}

/// A line callback (`onLine`, `waiting`'s line half).
pub type OnLine = Arc<dyn Fn(&str) + Send + Sync>;

/// machine.ts:44.
#[derive(Clone, Default)]
pub struct RunOptions {
    /// Past it the command's session is killed, pid and group, and the result is exit 124 with the output so far.
    pub deadline_ms: u64,
    /// Each complete line the command writes, stdout and stderr alike, as it is read.
    pub on_line: Option<OnLine>,
    /// Between reads of the command's output; the backend's own pace unless a test shortens it.
    pub poll_ms: Option<u64>,
    /// What the command prints is the person's own: a watcher records that it ran and its exit, never its output.
    pub unlogged: bool,
    /// The caller's cancel: the session is killed as at the deadline and the result is exit 130 with the output so far.
    pub signal: Option<CancellationToken>,
    /// The machine the launch goes through where it is not the one polls, kill and cleanup go through.
    pub launch_on: Option<Arc<dyn Machine>>,
}

/// machine.ts:79. How far a snapshot has got.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SnapshotProgress {
    pub bytes: u64,
    pub total: Option<u64>,
}

/// machine.ts:84.
#[derive(Clone, Default)]
pub struct SnapshotOptions {
    pub on_progress: Option<Arc<dyn Fn(SnapshotProgress) + Send + Sync>>,
}

/// machine.ts:90. What a road that cuts bytes up says about the trip: the pieces it sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BytesLanded {
    pub pieces: u32,
}

/// machine.ts:96. The command a machine's exec sends for the one it is handed, and what an upload is refused with
/// when the wrapper leaves no room under the exec body cap.
pub trait Framing: Send + Sync {
    fn wrap(&self, cmd: &str) -> String;
    fn no_room(&self) -> Option<String>;
}

/// machine.ts:104 `exec`'s options.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExecOptions {
    pub timeout_ms: Option<u64>,
    pub idempotency_key: Option<String>,
    pub stdin: Option<Vec<u8>>,
}

/// machine.ts:107 `seen`: the provider's view when get() made the handle.
#[derive(Debug, Clone, PartialEq)]
pub struct Seen {
    pub state: MachineState,
    pub created_at: Option<String>,
    pub paused_at: Option<String>,
}

/// A machine handle's optional methods: true where the backend has the TS's optional method.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MachineOffers {
    pub framed: bool,
    pub resume_at: bool,
    pub preview_url: bool,
    pub daemon_answers: bool,
    pub prove_road: bool,
    pub start_daemon: bool,
    pub daemon_exits: bool,
    pub put_bytes: bool,
    pub read_bytes: bool,
    pub daemon_frame: bool,
    pub describe: bool,
    pub facts: bool,
    pub metrics: bool,
}

pub(crate) fn not_on_this_road<T>() -> Result<T> {
    Err(EngineError::Other { message: wsp_wire::wire::machine_link::NOT_ON_THIS_ROAD.to_string(), code: None })
}

/// machine.ts:101.
#[async_trait]
pub trait Machine: Send + Sync {
    fn id(&self) -> &str;
    fn kind(&self) -> MachineKind;
    fn stream_url(&self) -> Option<&str> {
        None
    }
    /// The labels the provider reported when this handle was made; `None` on backends that carry none.
    fn labels(&self) -> Option<&IndexMap<String, String>> {
        None
    }
    fn seen(&self) -> Option<&Seen> {
        None
    }
    /// On a handle from create(): the provider answered from an earlier create under the same key.
    fn replayed(&self) -> bool {
        false
    }
    /// On a handle from create(): one sentence about the machine actually made, where it is not the one asked for.
    fn notice(&self) -> Option<&str> {
        None
    }
    /// Set where exec hands `stdin` to the command.
    fn takes_stdin(&self) -> bool {
        false
    }
    fn daemon_token_path(&self) -> Option<&str> {
        None
    }
    fn daemon_answers_prompt_ms(&self) -> Option<u64> {
        None
    }
    fn daemon_supervisor(&self) -> Option<DaemonSupervisor> {
        None
    }
    fn offers(&self) -> MachineOffers {
        MachineOffers::default()
    }

    async fn exec(&self, cmd: &str, opts: ExecOptions) -> Result<ExecResult>;
    async fn run(&self, script: &str, opts: RunOptions) -> Result<ExecResult>;
    async fn snapshot(&self, name: &str, life: MachineLife, opts: SnapshotOptions) -> Result<String>;
    async fn pause(&self, wait_ms: Option<u64>) -> Result<()>;
    async fn resume(&self, signal: Option<CancellationToken>) -> Result<()>;
    async fn kill(&self) -> Result<()>;
    async fn state(&self) -> Result<MachineState>;
    async fn download_url(&self, path: &str) -> Result<String>;
    async fn upload_url(&self, path: &str) -> Result<String>;

    async fn framed(&self) -> Result<Arc<dyn Framing>> {
        not_on_this_road()
    }
    async fn resume_at(&self, _cpu: f64, _mem_mb: u64, _signal: Option<CancellationToken>) -> Result<()> {
        not_on_this_road()
    }
    async fn preview_url(&self, _port: u16) -> Result<PreviewReach> {
        not_on_this_road()
    }
    async fn daemon_answers(&self, _timeout_ms: Option<u64>) -> Result<bool> {
        not_on_this_road()
    }
    async fn prove_road(&self, _signal: Option<CancellationToken>) -> Result<()> {
        not_on_this_road()
    }
    async fn start_daemon(&self) -> Result<ExecResult> {
        not_on_this_road()
    }
    async fn daemon_exits(&self) -> Result<Option<String>> {
        not_on_this_road()
    }
    /// `None` is the TS's `void`: a road that lands the bytes in one call says nothing.
    async fn put_bytes(&self, _path: &str, _bytes: &[u8], _timeout_ms: Option<u64>) -> Result<Option<BytesLanded>> {
        not_on_this_road()
    }
    async fn read_bytes(&self, _path: &str, _size: Option<u64>, _timeout_ms: Option<u64>) -> Result<Vec<u8>> {
        not_on_this_road()
    }
    /// A refusal comes back as the reply object it was, `ok: false` with its code.
    async fn daemon_frame(&self, _frame: Map<String, Value>) -> Result<Map<String, Value>> {
        not_on_this_road()
    }
    async fn describe(&self) -> Result<MachineShape> {
        not_on_this_road()
    }
    async fn facts(&self) -> Result<MachineFacts> {
        not_on_this_road()
    }
    async fn metrics(&self) -> Result<()> {
        not_on_this_road()
    }
}

/// machine.ts:204. A size, as `{ cpu, memMb }`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub cpu: f64,
    pub mem_mb: f64,
}

/// machine.ts:206. What a size costs on this provider, and the shape a spec gets when it names none.
pub trait BackendPricing: Send + Sync {
    fn rate_usd_per_hour(&self, size: Size) -> f64;
    fn default_size(&self) -> Size;
    fn snapshot_storage(&self) -> SnapshotStoragePricing;
    fn builder_disk_gb(&self) -> Option<f64> {
        None
    }
}

/// machine.ts:218. What the runtime reads to drive a machine through naps and wakes on this provider.
#[async_trait]
pub trait Lifecycle: Send + Sync {
    fn budgets(&self) -> LifecycleBudgets;
    fn has_backstop(&self) -> bool {
        false
    }
    async fn backstop(&self, _machine: &dyn Machine, _until_ms: i64, _exact: bool) -> Result<()> {
        not_on_this_road()
    }
    fn has_quiet_for_ms(&self) -> bool {
        false
    }
    async fn quiet_for_ms(&self, _machine: &dyn Machine) -> Result<Option<u64>> {
        Ok(None)
    }
    fn disk_kept_ms(&self) -> Option<u64> {
        None
    }
}

/// machine.ts:239. A machine the provider lists under a name of wsp's that carries no labels.
#[derive(Debug, Clone, PartialEq)]
pub struct NamedMachine {
    pub id: String,
    pub name: String,
    pub state: MachineState,
    pub created_at: Option<String>,
}

/// machine.ts:247. A provider's machine by its own id, in the provider's own word for its state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderBoxState {
    pub id: String,
    pub state: String,
}

/// `create`'s `waiting`: each wait the provider puts the create through, with the provider's box where it says one.
pub type Waiting = Arc<dyn Fn(&str, Option<&ProviderBoxState>) + Send + Sync>;

/// A backend's optional methods: true where the backend has the TS's optional method.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BackendOffers {
    pub on_computer: bool,
    /// createRequest, replayCreate and namedAfter, the three together.
    pub keyed_create: bool,
    pub check_key: bool,
    pub place_limits: bool,
    pub capacity: bool,
    pub root_disk_of: bool,
    pub limits: bool,
    pub list_snapshots: bool,
    /// promoteSnapshot, getTemplate, listTemplates and deleteTemplate, the four together.
    pub templates: bool,
}

/// What a provider's root disk is for a size (machine.ts:298).
#[derive(Debug, Clone, PartialEq)]
pub struct RootDisk {
    pub name: String,
    pub disk_gb: f64,
}

/// machine.ts:252.
#[async_trait]
pub trait MachineBackend: Send + Sync {
    /// Owned: a backend may compute it per read (Box's sizes follow its last read of /limits), and a slot that swaps
    /// backends under its callers holds none it could lend from.
    fn capabilities(&self) -> Capabilities;
    fn pricing(&self) -> Arc<dyn BackendPricing>;
    /// Present on every backend whose capabilities carry a pauseMode.
    fn lifecycle(&self) -> Option<Arc<dyn Lifecycle>> {
        None
    }
    /// What each kind boots from: wsp-wire's `{ sandbox, desktop }`.
    fn base_templates(&self) -> Option<BaseTemplates> {
        None
    }
    fn names_workspace(&self) -> bool {
        false
    }
    fn logins(&self) -> Option<String> {
        None
    }
    fn projects(&self) -> Option<String> {
        None
    }
    fn offers(&self) -> BackendOffers {
        BackendOffers::default()
    }
    /// Synchronous in the TS: `None` for a size nobody measured, and for a backend that does not offer it.
    fn root_disk_of(&self, _size: Size) -> Option<RootDisk> {
        None
    }

    async fn create(&self, spec: MachineSpec, waiting: Option<Waiting>, sent: Option<String>) -> Result<Arc<dyn Machine>>;
    async fn get(&self, id: &str) -> Result<Arc<dyn Machine>>;
    async fn list(&self, labels: Option<&IndexMap<String, String>>) -> Result<Vec<MachineListRow>>;
    async fn delete_snapshot(&self, id: &str) -> Result<()>;

    async fn on_computer(&self, _cmd: &str, _timeout_ms: Option<u64>) -> Result<ExecResult> {
        not_on_this_road()
    }
    async fn create_request(&self, _spec: &MachineSpec) -> Result<String> {
        not_on_this_road()
    }
    /// `Ok(None)` is an attempt past the provider's key window, nothing sent.
    async fn replay_create(&self, _key: &str, _bytes: &str, _made_at: &str) -> Result<Option<Arc<dyn Machine>>> {
        not_on_this_road()
    }
    async fn named_after(&self, _names: &std::collections::BTreeSet<String>) -> Result<Vec<NamedMachine>> {
        not_on_this_road()
    }
    async fn check_key(&self) -> Result<Value> {
        not_on_this_road()
    }
    /// `Ok(None)` where the read failed.
    async fn place_limits(&self) -> Result<Option<CloudLimits>> {
        not_on_this_road()
    }
    async fn capacity(&self) -> Result<PlaceCapacity> {
        not_on_this_road()
    }
    async fn limits(&self) -> Result<BoxLimits> {
        not_on_this_road()
    }
    async fn list_snapshots(&self, _ids: Option<&[String]>) -> Result<Vec<SnapshotRow>> {
        not_on_this_road()
    }
    async fn promote_snapshot(&self, _snapshot_id: &str, _name: &str) -> Result<String> {
        not_on_this_road()
    }
    async fn get_template(&self, _id: &str) -> Result<TemplateRow> {
        not_on_this_road()
    }
    async fn list_templates(&self) -> Result<Vec<TemplateRow>> {
        not_on_this_road()
    }
    async fn delete_template(&self, _id: &str) -> Result<()> {
        not_on_this_road()
    }
}
