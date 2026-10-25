// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/fake-backend.ts at a4033dc34. Spec: spec/fake_backend.md.
//!
//! A provider that answers out of memory, or out of one records file two host processes share, so a harness can
//! serve a fixture state whose workspaces are forks.
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexMap;
use tokio_util::sync::CancellationToken;

use crate::errors::Result;
use crate::machine::{
    BackendOffers, BackendPricing, BytesLanded, Capabilities, ExecOptions, ExecResult, Lifecycle, Machine, MachineBackend, MachineFacts,
    MachineKind, MachineLife, MachineListRow, MachineOffers, MachineShape, MachineSpec, MachineState, PreviewReach, RunOptions, Seen,
    SnapshotOptions, SnapshotRow, Waiting,
};

/// fake-backend.ts:75. What a road that reaches for the guest is told.
pub const FAKE_NO_GUEST: &str = "this is a stand-in provider for testing: its machines are records in memory, so nothing runs on them and nothing can be opened inside them";

/// fake-backend.ts:94 `shell`'s answer: how a Linux guest's command runs on the computer holding the folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuestShell {
    pub cmd: String,
    pub cwd: PathBuf,
    pub env: IndexMap<String, String>,
}

/// fake-backend.ts:83. A folder on the caller's own computer standing in for a machine's disk.
#[async_trait]
pub trait FakeGuest: Send + Sync {
    /// Where this machine's commands run and its daemon is rooted.
    fn folder(&self, machine_id: &str) -> PathBuf;
    /// The file this machine's daemon reads its token from.
    fn token_path(&self, machine_id: &str) -> String;
    /// The route to one port on this machine.
    async fn reach(&self, machine_id: &str, port: u16) -> Result<PreviewReach>;
    /// Optional: the guest's own answer for a command written for a Linux machine.
    fn shell(&self, _machine_id: &str, _cmd: &str) -> Option<GuestShell> {
        None
    }
    /// Whether the guest lands bytes (the TS's `putBytes` present).
    fn has_put_bytes(&self) -> bool {
        false
    }
    async fn put_bytes(&self, _machine_id: &str, _path: &str, _bytes: &[u8]) -> Result<()> {
        crate::machine::not_on_this_road()
    }
}

/// fake-backend.ts:126. What a reader is told when the file is there and is not records.
pub fn stand_in_records_unreadable(path: &str, why: &str) -> String {
    todo!()
}

/// fake-backend.ts:250.
#[derive(Clone, Default)]
pub struct FakeBackendOptions {
    /// The shape every machine takes when a create names none; the pricing's default size otherwise.
    pub size: Option<(f64, f64)>,
    /// A file this stand-in keeps its machines and snapshots in; this process alone when none is named.
    pub records: Option<PathBuf>,
    pub guest: Option<Arc<dyn FakeGuest>>,
}

/// fake-backend.ts:113. Where this stand-in's records live; the implementer shapes it.
struct Records {}

/// fake-backend.ts:156. One machine this provider holds.
pub struct FakeMachine {
    id: String,
    labels: IndexMap<String, String>,
    shape: MachineShape,
    seen: Seen,
    records: Arc<Records>,
    guest: Option<Arc<dyn FakeGuest>>,
    token_path: Option<String>,
}

#[async_trait]
impl Machine for FakeMachine {
    fn id(&self) -> &str {
        todo!()
    }
    fn kind(&self) -> MachineKind {
        todo!()
    }
    fn labels(&self) -> Option<&IndexMap<String, String>> {
        todo!()
    }
    fn seen(&self) -> Option<&Seen> {
        todo!()
    }
    fn daemon_token_path(&self) -> Option<&str> {
        todo!()
    }
    fn offers(&self) -> MachineOffers {
        todo!()
    }
    async fn exec(&self, cmd: &str, opts: ExecOptions) -> Result<ExecResult> {
        todo!()
    }
    async fn run(&self, script: &str, opts: RunOptions) -> Result<ExecResult> {
        todo!()
    }
    async fn snapshot(&self, name: &str, life: MachineLife, opts: SnapshotOptions) -> Result<String> {
        todo!()
    }
    async fn pause(&self, wait_ms: Option<u64>) -> Result<()> {
        todo!()
    }
    async fn resume(&self, signal: Option<CancellationToken>) -> Result<()> {
        todo!()
    }
    async fn kill(&self) -> Result<()> {
        todo!()
    }
    async fn state(&self) -> Result<MachineState> {
        todo!()
    }
    async fn download_url(&self, path: &str) -> Result<String> {
        todo!()
    }
    async fn upload_url(&self, path: &str) -> Result<String> {
        todo!()
    }
    async fn preview_url(&self, port: u16) -> Result<PreviewReach> {
        todo!()
    }
    async fn put_bytes(&self, path: &str, bytes: &[u8], timeout_ms: Option<u64>) -> Result<Option<BytesLanded>> {
        todo!()
    }
    async fn describe(&self) -> Result<MachineShape> {
        todo!()
    }
    async fn facts(&self) -> Result<MachineFacts> {
        todo!()
    }
}

/// fake-backend.ts:259.
pub struct FakeBackend {
    records: Arc<Records>,
    guest: Option<Arc<dyn FakeGuest>>,
    size: (f64, f64),
}

impl FakeBackend {
    /// fake-backend.ts:269.
    pub fn new(opts: FakeBackendOptions) -> Self {
        todo!()
    }

    /// The fake's machine itself (the TS hands back its own class), for a caller that reads its seen state.
    pub async fn get_fake(&self, machine_id: &str) -> Result<Arc<FakeMachine>> {
        todo!()
    }
}

#[async_trait]
impl MachineBackend for FakeBackend {
    fn capabilities(&self) -> Capabilities {
        todo!()
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
        todo!()
    }
    fn lifecycle(&self) -> Option<Arc<dyn Lifecycle>> {
        todo!()
    }
    fn offers(&self) -> BackendOffers {
        todo!()
    }
    async fn create(&self, spec: MachineSpec, waiting: Option<Waiting>, sent: Option<String>) -> Result<Arc<dyn Machine>> {
        todo!()
    }
    async fn get(&self, id: &str) -> Result<Arc<dyn Machine>> {
        todo!()
    }
    async fn list(&self, labels: Option<&IndexMap<String, String>>) -> Result<Vec<MachineListRow>> {
        todo!()
    }
    async fn list_snapshots(&self, ids: Option<&[String]>) -> Result<Vec<SnapshotRow>> {
        todo!()
    }
    async fn delete_snapshot(&self, id: &str) -> Result<()> {
        todo!()
    }
}
