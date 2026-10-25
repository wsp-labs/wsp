// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/solari-backend.ts. Specs: spec/solari_call.md (the backend, its one HTTP call, create,
//! the listings) and spec/solari_machine.md (a machine's exec, pause, resume and reads).
use std::sync::Arc;

use crate::cancel::Cancel;
use async_trait::async_trait;
use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::errors::{Result, RetryClock};
use crate::fetch::Fetch;
use crate::machine::{
    BackendOffers, BackendPricing, Capabilities, ExecOptions, ExecResult, Lifecycle, LifecycleBudgets, Machine, MachineBackend,
    MachineKind, MachineLife, MachineListRow, MachineOffers, MachineShape, MachineSpec, MachineState, OnLine, PreviewReach, RunOptions,
    Seen, Size, SnapshotOptions, SnapshotRow, SnapshotStoragePricing, TemplateRow, Waiting,
};

/// solari-backend.ts:22. The budgets a pause and a resume run on, in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoveBudgets {
    pub pause_ms: f64,
    pub resume_cap_ms: f64,
    pub state_read_ms: f64,
    /// The cap on one GET whose road names none.
    pub read_ms: f64,
}

/// solari-backend.ts:18 `budgets`: each one absent takes its shipped default.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MoveBudgetsGiven {
    pub pause_ms: Option<f64>,
    pub resume_cap_ms: Option<f64>,
    pub state_read_ms: Option<f64>,
    pub read_ms: Option<f64>,
}

/// solari-backend.ts:10.
#[derive(Clone, Default)]
pub struct SolariBackendOptions {
    pub api_key: String,
    /// `https://api.getsolari.com` when `None`.
    pub base_url: Option<String>,
    /// The platform's fetch when `None`.
    pub fetch: Option<Arc<dyn Fetch>>,
    /// `RealRetryClock` when `None`.
    pub clock: Option<Arc<dyn RetryClock>>,
    pub budgets: MoveBudgetsGiven,
    /// Where the TS `console.warn` lines go, one call per line; stderr when `None`.
    pub warn: Option<OnLine>,
}

/// solari-backend.ts:69. Billed from 2026-10-01, 10 GB free, then $0.05 per GB-month.
pub fn snapshot_storage_pricing() -> SnapshotStoragePricing {
    todo!()
}

/// solari-backend.ts:72. The header a request id is read off, should one appear.
pub const REQUEST_ID_HEADER: &str = "x-request-id";

/// solari-backend.ts:75. `cpu * 0.035 + (mem_mb / 1024) * 0.01` per hour.
pub fn rate_usd_per_hour(size: Size) -> f64 {
    todo!()
}

/// solari-backend.ts:80. The one size a fork has been measured coming up at, and the only one offered.
pub const DEFAULT_SIZE: Size = Size { cpu: 2.0, mem_mb: 4096.0 };

/// solari-backend.ts:85. Readable with no key: the rate, the default size, snapshot storage and a 20 GB builder disk.
pub struct SolariPricing;

impl BackendPricing for SolariPricing {
    fn rate_usd_per_hour(&self, size: Size) -> f64 {
        todo!()
    }
    fn default_size(&self) -> Size {
        todo!()
    }
    fn snapshot_storage(&self) -> SnapshotStoragePricing {
        todo!()
    }
    fn builder_disk_gb(&self) -> Option<f64> {
        todo!()
    }
}

/// solari-backend.ts:95.
pub const RESUME_CAP_MS: u64 = 30_000;
/// solari-backend.ts:98.
pub const PAUSE_MS: u64 = 4 * 60_000;
/// solari-backend.ts:100.
pub const STATE_READ_MS: u64 = 30_000;
/// solari-backend.ts:103. The longest idleTimeoutMs the create API has taken.
pub const IDLE_TIMEOUT_MAX_MS: u64 = 6 * 60 * 60_000;
/// solari-backend.ts:107. The longest one exec may ask the provider for; longer runs detached.
pub const SOLARI_INLINE_MAX_MS: u64 = 26_000;
/// solari-backend.ts:111.
pub const SANDBOX_UNREACHABLE: &str = "Sandbox is not reachable";
/// solari-backend.ts:114.
pub const EXEC_FAILED: &str = "exec failed";
/// solari-backend.ts:117.
pub const NOT_PAUSABLE: &str = "Not pausable";
/// solari-backend.ts:121.
pub const SNAPSHOT_NOT_FOUND: &str = "Snapshot not found";
/// solari-backend.ts:123. How long a fork of a listed snapshot is asked again, and how often.
pub const SNAPSHOT_LISTED_MS: u64 = 3 * 60_000;
pub const SNAPSHOT_LISTED_EVERY_MS: u64 = 15_000;

/// solari-backend.ts:128. Two wake attempts, the daemon answering in 30 s, resume asked every minute for 30 minutes.
/// One value for every backend; nothing can change it.
pub fn solari_lifecycle_budgets() -> LifecycleBudgets {
    todo!()
}

/// The lifecycle `SolariBackend::lifecycle()` answers: its budgets are `solari_lifecycle_budgets()`, no backstop, no
/// quiet read.
pub struct SolariLifecycle;

#[async_trait(?Send)]
impl Lifecycle for SolariLifecycle {
    fn budgets(&self) -> LifecycleBudgets {
        todo!()
    }
}

/// solari-backend.ts:142. The pt_token exp claim is 60 minutes from mint.
pub const PREVIEW_TTL_MS: i64 = 60 * 60_000;

/// solari-backend.ts:146. Epoch-ms expiry of a preview token: cut at the last `.` (skipped where it is at 0 or absent),
/// then the whole string; each part base64url-decoded leniently as node does, and the first `"exp":<digits>` in it read
/// as a number. `now + PREVIEW_TTL_MS` where neither carries one.
pub fn preview_token_expiry(token: &str, now: i64) -> i64 {
    todo!()
}

/// solari-backend.ts:66. A state word the table never learned reads running: starting, running, paused as themselves;
/// archived, releasing and gone are gone; anything else running.
pub fn state_of(state: &str) -> MachineState {
    todo!()
}

/// solari-backend.ts:161.
pub struct SolariBackend {
    api_key: String,
    base_url: String,
    fetch: Arc<dyn Fetch>,
    clock: Arc<dyn RetryClock>,
    budgets: MoveBudgets,
    warn: OnLine,
}

impl SolariBackend {
    /// solari-backend.ts:191.
    pub fn new(opts: SolariBackendOptions) -> Arc<Self> {
        todo!()
    }

    /// The budgets after defaults: pause `PAUSE_MS`, resume `RESUME_CAP_MS`, read of state `STATE_READ_MS`, plain read
    /// `PROVIDER_READ_CAP_MS`, each replaced where given.
    pub fn budgets(&self) -> MoveBudgets {
        todo!()
    }

    pub fn clock(&self) -> Arc<dyn RetryClock> {
        todo!()
    }

    /// solari-backend.ts:201. One call to the provider; the answer's JSON (`{}` for an empty body). `cap_ms` cuts the call
    /// off (a GET with none takes `read_ms`), `signal` is the caller's stop; both ride the one fetch. Retries and
    /// failures in spec/solari_call.md.
    pub async fn request<T: DeserializeOwned>(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
        cap_ms: Option<f64>,
        signal: Option<&Cancel>,
    ) -> Result<T> {
        todo!()
    }
}

#[async_trait(?Send)]
impl MachineBackend for SolariBackend {
    /// solari-backend.ts:162. Every field in spec/solari_call.md; sizes are `[DEFAULT_SIZE]` with its rate.
    fn capabilities(&self) -> Capabilities {
        todo!()
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
        todo!()
    }
    fn lifecycle(&self) -> Option<Arc<dyn Lifecycle>> {
        todo!()
    }
    /// check_key, list_snapshots and the four template methods.
    fn offers(&self) -> BackendOffers {
        todo!()
    }
    /// solari-backend.ts:256. `waiting` hears each wait as `image_served_wait_line("Solari", SNAPSHOT_LISTED_EVERY_MS)`,
    /// with no provider box. `sent` is not read.
    async fn create(&self, spec: MachineSpec, waiting: Option<Waiting>, sent: Option<String>) -> Result<Arc<dyn Machine>> {
        todo!()
    }
    /// solari-backend.ts:309.
    async fn get(&self, id: &str) -> Result<Arc<dyn Machine>> {
        todo!()
    }
    /// solari-backend.ts:317.
    async fn list(&self, labels: Option<&IndexMap<String, String>>) -> Result<Vec<MachineListRow>> {
        todo!()
    }
    /// solari-backend.ts:341.
    async fn delete_snapshot(&self, id: &str) -> Result<()> {
        todo!()
    }
    /// solari-backend.ts:305. Lists the templates; answers `Value::Null` (the TS `void`).
    async fn check_key(&self) -> Result<Value> {
        todo!()
    }
    /// solari-backend.ts:364. `ids` is not read: the provider's whole listing.
    async fn list_snapshots(&self, ids: Option<&[String]>) -> Result<Vec<SnapshotRow>> {
        todo!()
    }
    /// solari-backend.ts:345.
    async fn promote_snapshot(&self, snapshot_id: &str, name: &str) -> Result<String> {
        todo!()
    }
    /// solari-backend.ts:350.
    async fn get_template(&self, id: &str) -> Result<TemplateRow> {
        todo!()
    }
    /// solari-backend.ts:354.
    async fn list_templates(&self) -> Result<Vec<TemplateRow>> {
        todo!()
    }
    /// solari-backend.ts:360.
    async fn delete_template(&self, id: &str) -> Result<()> {
        todo!()
    }
}

/// solari-backend.ts:382. A machine handle: its id, kind, stream URL, labels, what the provider said when it was made,
/// and whether a create was a replay.
pub struct SolariMachine {
    backend: Arc<SolariBackend>,
    id: String,
    kind: MachineKind,
    stream_url: Option<String>,
    labels: Option<IndexMap<String, String>>,
    seen: Option<Seen>,
    replayed: bool,
}

#[async_trait(?Send)]
impl Machine for SolariMachine {
    fn id(&self) -> &str {
        todo!()
    }
    fn kind(&self) -> MachineKind {
        todo!()
    }
    fn stream_url(&self) -> Option<&str> {
        todo!()
    }
    fn labels(&self) -> Option<&IndexMap<String, String>> {
        todo!()
    }
    fn seen(&self) -> Option<&Seen> {
        todo!()
    }
    fn replayed(&self) -> bool {
        todo!()
    }
    /// preview_url, describe and metrics.
    fn offers(&self) -> MachineOffers {
        todo!()
    }
    /// solari-backend.ts:397.
    async fn exec(&self, cmd: &str, opts: ExecOptions) -> Result<ExecResult> {
        todo!()
    }
    /// solari-backend.ts:417. exec-detached's `exec_detached(self, script, opts)`.
    async fn run(&self, script: &str, opts: RunOptions) -> Result<ExecResult> {
        todo!()
    }
    /// solari-backend.ts:423. `opts` is not read.
    async fn snapshot(&self, name: &str, life: MachineLife, opts: SnapshotOptions) -> Result<String> {
        todo!()
    }
    /// solari-backend.ts:434. `wait_ms` is not read: the budget is the backend's.
    async fn pause(&self, wait_ms: Option<u64>) -> Result<()> {
        todo!()
    }
    /// solari-backend.ts:467.
    async fn resume(&self, signal: Option<&Cancel>) -> Result<()> {
        todo!()
    }
    /// solari-backend.ts:490.
    async fn kill(&self) -> Result<()> {
        todo!()
    }
    /// solari-backend.ts:494.
    async fn state(&self) -> Result<MachineState> {
        todo!()
    }
    /// solari-backend.ts:525.
    async fn download_url(&self, path: &str) -> Result<String> {
        todo!()
    }
    /// solari-backend.ts:532.
    async fn upload_url(&self, path: &str) -> Result<String> {
        todo!()
    }
    /// solari-backend.ts:518.
    async fn preview_url(&self, port: u16) -> Result<PreviewReach> {
        todo!()
    }
    /// solari-backend.ts:504.
    async fn describe(&self) -> Result<MachineShape> {
        todo!()
    }
    /// solari-backend.ts:514.
    async fn metrics(&self) -> Result<()> {
        todo!()
    }
}
