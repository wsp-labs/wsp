// SPDX-License-Identifier: AGPL-3.0-only
//! ssh-backend.ts:617-969 at a4033dc34. Spec: spec/ssh_machine.md.
//!
//! Root over a login that is not root, the one read that adopts a machine, and the machine and backend a host holds
//! for a computer it reaches over ssh.
use std::sync::{Arc, Mutex};

use crate::cancel::Cancel;
use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use indexmap::IndexMap;

use crate::errors::Result;
use crate::machine::{
    BytesLanded, ExecOptions, ExecResult, Machine, MachineFacts, MachineKind, MachineLife, MachineOffers, MachineShape, MachineState,
    RunOptions, SnapshotOptions,
};
use crate::ssh_backend::dial::{SshReach, SshTransport};
use crate::ssh_backend::keys::{KnownHostsEntry, OfferedKey, SshHostKeyReader};

/// ssh-backend.ts:618.
pub const SUDO_ASKS_LINE: &str = "a password is required";

/// ssh-backend.ts:622. How a login reaches root there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SshSudo {
    Root,
    Free,
    Taken,
    Asks,
    Wrong,
    Tty,
    None,
}

/// ssh-backend.ts:625. The read of a login answers one of root, free, asks, tty, none (never taken or wrong).
pub type SshSudoRoad = SshSudo;

/// ssh-backend.ts:639.
pub const SSH_SUDO_READ: &str = concat!(
    "if [ \"$(id -u)\" -eq 0 ]; then echo WSP_SUDO root\n",
    "elif ! command -v sudo >/dev/null 2>&1; then echo WSP_SUDO none\n",
    "elif said=$(LC_ALL=C sudo -n -v 2>&1 && LC_ALL=C sudo -n -l bash -c true 2>&1); then echo WSP_SUDO free\n",
    "else case \"$said\" in *\"must have a tty\"*) echo WSP_SUDO tty ;; *) echo WSP_SUDO asks ;; esac; fi",
);

/// ssh-backend.ts:648.
pub const SSH_SUDO_TRY: &str = "LC_ALL=C sudo -S -k -p '' bash -c 'echo WSP_SUDO taken'";

/// ssh-backend.ts:652. How the login reaches root, read as the login with nothing of the person's sent.
pub async fn read_ssh_sudo(reach: &SshReach, transport: &SshTransport) -> Result<SshSudoRoad> {
    todo!()
}

/// ssh-backend.ts:660. One try of the person's password; answers taken, wrong, tty or none.
pub async fn try_ssh_sudo(reach: &SshReach, password: &str, transport: &SshTransport) -> SshSudo {
    todo!()
}

/// ssh-backend.ts:670. The store variable each harness reads, by name, in catalog order.
pub fn ssh_store_vars() -> Vec<String> {
    todo!()
}

/// ssh-backend.ts:677. What the read asks the machine's own shell for.
pub fn login_read() -> String {
    todo!()
}

/// ssh-backend.ts:687. What one dial reads off a machine before its record exists.
pub fn ssh_read_script() -> String {
    todo!()
}

/// ssh-backend.ts:705. What the machine answers about itself every time a status is built.
pub fn ssh_facts_script() -> String {
    todo!()
}

/// ssh-backend.ts:35. The machine's login environment: the kept store folders in `ssh_store_vars` order, then `HOME`,
/// `USER`, `PATH`.
pub type SshLogin = IndexMap<String, String>;

/// ssh-backend.ts:709's answer.
#[derive(Debug, Clone, PartialEq)]
pub struct SshMachineRead {
    pub login: SshLogin,
    pub shape: MachineShape,
    pub system: Option<String>,
    pub arch: Option<String>,
    pub shell: Option<String>,
}

/// ssh-backend.ts:709. One dial that both proves the machine answers and records what wsp needs of it.
pub async fn read_ssh_machine(reach: &SshReach, transport: &SshTransport) -> Result<SshMachineRead> {
    todo!()
}

/// ssh-backend.ts:748. Whether this dial reaches the computer wsp runs on; `names` are what it answers to.
pub fn ssh_dials_this_computer(reach: &SshReach, names: &[String]) -> bool {
    todo!()
}

/// ssh-backend.ts:757.
pub const DEFAULT_REMOTE_PATH: &str = "/usr/local/bin:/usr/bin:/bin";

/// ssh-backend.ts:764. The machine's login PATH with every folder held to the plain-path rule.
pub fn plain_path(path: Option<&str>) -> String {
    todo!()
}

/// ssh-backend.ts:770.
pub const SSH_BYTES_OK: &str = "WSP_BYTES_OK";

/// ssh-backend.ts:777. The script that takes a file's bytes off the connection's stdin.
pub fn put_bytes_script(path: &str, size: usize, tmp: &str) -> String {
    todo!()
}

/// Where a machine says why its facts cannot be read (the TS's `console.warn`).
pub type WarnSink = Arc<dyn Fn(&str)>;

/// ssh-backend.ts:795. A machine reached over ssh.
pub struct SshMachine {
    pub reach: SshReach,
    id: String,
    transport: SshTransport,
    /// ssh-backend.ts:801. The last words warned about this machine failing to say what it is.
    quiet: Mutex<Option<String>>,
    warn: WarnSink,
}

impl SshMachine {
    /// ssh-backend.ts:803. Warnings go to stderr.
    pub fn new(reach: SshReach, transport: SshTransport) -> Result<Self> {
        todo!()
    }

    /// The same with a warning sink of the caller's.
    pub fn with_warn(reach: SshReach, transport: SshTransport, warn: WarnSink) -> Result<Self> {
        todo!()
    }
}

#[async_trait(?Send)]
impl Machine for SshMachine {
    fn id(&self) -> &str {
        todo!()
    }
    fn kind(&self) -> MachineKind {
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

/// ssh-backend.ts:881. Every seam the backend reads through; `None` takes the real one.
#[derive(Clone, Default)]
pub struct SshBackendOptions {
    pub transport: Option<SshTransport>,
    pub host_key: Option<SshHostKeyReader>,
    pub known_hosts: Option<Arc<dyn Fn(SshReach) -> LocalBoxFuture<'static, KnownHostsEntry>>>,
    pub offered_key: Option<Arc<dyn Fn(SshReach) -> LocalBoxFuture<'static, OfferedKey>>>,
    pub host_name: Option<Arc<dyn Fn(SshReach) -> LocalBoxFuture<'static, Option<String>>>>,
}

/// ssh-backend.ts:896. What every script one backend carries rides with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SshRiding {
    SudoPassword(String),
    AsLogin,
}

/// ssh-backend.ts:937's answer.
pub struct SshAdopted {
    pub machine: Arc<SshMachine>,
    pub login: SshLogin,
    pub shape: MachineShape,
    pub system: Option<String>,
    pub arch: Option<String>,
    pub shell: Option<String>,
    pub host_key: Option<String>,
}

/// ssh-backend.ts:900. The reads a host makes of a computer over ssh before it joins it.
#[derive(Clone)]
pub struct SshBackend {
    transport: SshTransport,
    host_key: SshHostKeyReader,
    known_hosts: Arc<dyn Fn(SshReach) -> LocalBoxFuture<'static, KnownHostsEntry>>,
    offered: Arc<dyn Fn(SshReach) -> LocalBoxFuture<'static, OfferedKey>>,
    host_name: Arc<dyn Fn(SshReach) -> LocalBoxFuture<'static, Option<String>>>,
}

impl SshBackend {
    /// ssh-backend.ts:907.
    pub fn new(opts: SshBackendOptions) -> Self {
        todo!()
    }

    /// ssh-backend.ts:916.
    pub async fn sudo_for(&self, reach: &SshReach) -> Result<SshSudoRoad> {
        todo!()
    }

    /// ssh-backend.ts:921.
    pub async fn sudo_try(&self, reach: &SshReach, password: &str) -> SshSudo {
        todo!()
    }

    /// ssh-backend.ts:927. The same backend with every script it carries, and every machine it adopts, riding `riding`.
    pub fn riding(&self, riding: SshRiding) -> SshBackend {
        todo!()
    }

    /// ssh-backend.ts:937. Reads the machine, then the key this computer's client holds for it.
    pub async fn adopt(&self, reach: &SshReach) -> Result<SshAdopted> {
        todo!()
    }

    /// ssh-backend.ts:946.
    pub async fn key_for(&self, reach: &SshReach) -> Option<String> {
        todo!()
    }

    /// ssh-backend.ts:954.
    pub async fn known_hosts_entry(&self, reach: &SshReach) -> KnownHostsEntry {
        todo!()
    }

    /// ssh-backend.ts:960.
    pub async fn offered_key_for(&self, reach: &SshReach) -> OfferedKey {
        todo!()
    }

    /// ssh-backend.ts:966.
    pub async fn host_name_for(&self, reach: &SshReach) -> Option<String> {
        todo!()
    }
}
