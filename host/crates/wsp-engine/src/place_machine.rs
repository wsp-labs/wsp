// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/place-machine.ts at a4033dc34. Spec: spec/place_machine.md.
//!
//! The computer itself as something the engine can run steps on: a command, a long command and bytes over the exec
//! frame its daemon already serves, and every call that belongs to a workspace refused in one sentence.
use std::sync::Arc;

use crate::cancel::Cancel;
use async_trait::async_trait;
use serde_json::{Map, Value};
use tokio::sync::OnceCell;

use crate::errors::Result;
use crate::link_backend::MachineLink;
use crate::machine::{
    BytesLanded, ExecOptions, ExecResult, Framing, Machine, MachineBackend, MachineKind, MachineLife, MachineOffers, MachineState,
    RunOptions, Size, SnapshotOptions,
};
use crate::target_line::TargetLogin;

/// place-machine.ts:17.
pub const NOT_A_WORKSPACE: &str = "this is the computer itself, not a workspace on it";

/// place-machine.ts:23. How much of a file one frame carries.
pub const PLACE_PART_BYTES: usize = 1024 * 1024;

/// place-machine.ts:28. The slowest this road has been measured at, in bytes a millisecond.
pub const LINK_BYTES_PER_MS: u64 = 213;

/// place-machine.ts:33. What one part's frame asks for; `floor_ms` `None` is `INLINE_EXEC_MS`.
pub fn place_part_bound_ms(bytes: usize, floor_ms: Option<u64>) -> u64 {
    todo!()
}

/// place-machine.ts:40. Writes a folder's shell startup files and names them in the launch's environment.
pub fn shell_start_line(dir: &str) -> String {
    todo!()
}

/// place-machine.ts:51. Every command as the login runs it, and the refusal where its PATH leaves no room.
pub struct LoginFraming {
    prefix: Option<String>,
    login: TargetLogin,
}

impl Framing for LoginFraming {
    fn wrap(&self, cmd: &str) -> String {
        todo!()
    }
    fn no_room(&self) -> Option<String> {
        todo!()
    }
}

/// place-machine.ts:58. One computer you own, driven over the link its daemon holds to this host.
pub struct PlaceMachine {
    link: Arc<dyn MachineLink>,
    id: String,
    home: String,
}

impl PlaceMachine {
    /// place-machine.ts:64.
    pub fn new(link: Arc<dyn MachineLink>, id: &str, home: &str) -> Self {
        todo!()
    }
}

#[async_trait(?Send)]
impl Machine for PlaceMachine {
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
    async fn put_bytes(&self, path: &str, bytes: &[u8], timeout_ms: Option<u64>) -> Result<Option<BytesLanded>> {
        todo!()
    }
}

/// place-machine.ts:160. The computer a thread in a folder on it runs on: every command as the owner of the home.
pub struct PlaceFolderMachine {
    base: PlaceMachine,
    home: String,
    path: Option<String>,
    login: OnceCell<TargetLogin>,
}

impl PlaceFolderMachine {
    /// place-machine.ts:164.
    pub fn new(link: Arc<dyn MachineLink>, id: &str, home: &str, path: Option<&str>) -> Arc<Self> {
        todo!()
    }

    /// place-machine.ts:174. Who every command runs as, read once; a read that failed is read again next time.
    pub async fn login_of(&self) -> Result<TargetLogin> {
        todo!()
    }

    /// place-machine.ts:208. This computer as a turn's launch reaches it: each line in the thread's cgroup, with
    /// `path_first` in front of the login's PATH for those lines alone.
    pub fn in_cgroup(self: &Arc<Self>, cgroup: &str, path_first: Option<&str>) -> Arc<dyn Machine> {
        todo!()
    }
}

#[async_trait(?Send)]
impl Machine for PlaceFolderMachine {
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
    async fn framed(&self) -> Result<Arc<dyn Framing>> {
        todo!()
    }
    async fn put_bytes(&self, path: &str, bytes: &[u8], timeout_ms: Option<u64>) -> Result<Option<BytesLanded>> {
        todo!()
    }
    async fn daemon_frame(&self, frame: Map<String, Value>) -> Result<Map<String, Value>> {
        todo!()
    }
}

/// place-machine.ts:236. The backend a folder on a joined computer stands on: that computer as one machine.
pub fn place_folder_backend(machine: Arc<dyn Fn() -> Arc<dyn Machine>>, shape: Size) -> Arc<dyn MachineBackend> {
    todo!()
}
