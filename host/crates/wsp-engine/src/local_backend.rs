// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/local-backend.ts at a4033dc34. Spec: spec/local_backend.md.
//!
//! The backend for a local workspace: this computer itself, one machine that already exists. Every capability a
//! provider fork has and this computer does not is false; the moves a provider fork takes fail.
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::cancel::Cancel;
use async_trait::async_trait;
use indexmap::IndexMap;
use tokio::sync::OnceCell;

use crate::errors::Result;
use crate::machine::{
    BackendPricing, BytesLanded, Capabilities, ExecOptions, ExecResult, Machine, MachineBackend, MachineFacts, MachineKind, MachineLife,
    MachineListRow, MachineOffers, MachineShape, MachineSpec, MachineState, RunOptions, Size, SnapshotOptions, Waiting,
};

/// local-backend.ts:24.
pub const LOCAL_MACHINE_ID: &str = "local";

/// local-backend.ts:31. Cores and memory in whole GB; `None` reads this computer's.
pub fn local_shape(cores: Option<u32>, total_bytes: Option<u64>) -> Size {
    todo!()
}

/// local-backend.ts:37. What this computer has free this moment and in all, in MB.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryHere {
    pub free_mb: u64,
    pub total_mb: u64,
}

/// local-backend.ts:43. MemAvailable and MemTotal off /proc/meminfo.
pub fn memory_of_meminfo(meminfo: &str) -> Option<MemoryHere> {
    todo!()
}

/// local-backend.ts:54. macOS's share of memory free over the memory the computer has.
pub fn memory_of_pressure(level: &str, total_bytes: u64) -> Option<MemoryHere> {
    todo!()
}

/// local-backend.ts:63. This computer's memory as its kernel reads it now; never held.
pub async fn memory_here() -> Option<MemoryHere> {
    todo!()
}

/// local-backend.ts:63 with the platform named (Node's `process.platform` words: `linux`, `darwin`, `win32`, ...).
pub async fn memory_here_on(platform: &str) -> Option<MemoryHere> {
    todo!()
}

/// local-backend.ts:91. This computer as a Machine.
pub struct LocalMachine {
    root: PathBuf,
    env: IndexMap<String, String>,
    /// local-backend.ts:97. Read once: the system does not change its name while the host runs.
    os: OnceCell<String>,
}

#[async_trait(?Send)]
impl Machine for LocalMachine {
    fn id(&self) -> &str {
        todo!()
    }
    fn kind(&self) -> MachineKind {
        todo!()
    }
    fn takes_stdin(&self) -> bool {
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
    async fn resume(&self, signal: Option<&Cancel>) -> Result<()> {
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
    async fn describe(&self) -> Result<MachineShape> {
        todo!()
    }
    async fn facts(&self) -> Result<MachineFacts> {
        todo!()
    }
    async fn put_bytes(&self, path: &str, bytes: &[u8], timeout_ms: Option<u64>) -> Result<Option<BytesLanded>> {
        todo!()
    }
}

/// local-backend.ts:72.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocalBackendOptions {
    /// The folder every exec and run starts in.
    pub root: PathBuf,
    /// The environment every exec and run runs under; this process's when `None`.
    pub env: Option<IndexMap<String, String>>,
}

/// local-backend.ts:158. The backend for the one local workspace a host holds.
pub struct LocalBackend {
    machine: Arc<LocalMachine>,
    /// local-backend.ts:191. The folder every command on this computer starts in when the caller names none.
    pub folder: PathBuf,
}

impl LocalBackend {
    /// local-backend.ts:193. Touches nothing on disk.
    pub fn new(opts: LocalBackendOptions) -> Self {
        todo!()
    }

    /// local-backend.ts:206. Makes the folder (parents included) and answers it.
    pub fn work_folder(&self) -> Result<&Path> {
        todo!()
    }
}

#[async_trait(?Send)]
impl MachineBackend for LocalBackend {
    fn capabilities(&self) -> Capabilities {
        todo!()
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
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
    async fn delete_snapshot(&self, id: &str) -> Result<()> {
        todo!()
    }
}
