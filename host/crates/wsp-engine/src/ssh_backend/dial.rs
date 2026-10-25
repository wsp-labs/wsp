// SPDX-License-Identifier: AGPL-3.0-only
//! ssh-backend.ts:1-375 at a4033dc34. Spec: spec/ssh_dial.md.
//!
//! How wsp dials a machine over ssh: the dial's written forms, the ssh client's argv, the master connection's folder,
//! the person's config carried onto a forward child, and the held forward itself.
use std::path::PathBuf;
use std::sync::Arc;

use futures::future::LocalBoxFuture;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::errors::Result;
use crate::machine::{ExecResult, OnLine};
use crate::ssh_backend::keys::SshLocalRun;

/// ssh-backend.ts:24. How the ssh client is dialled; `key_path` absent leaves ssh its own config and agent.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshReach {
    pub user: String,
    pub host: String,
    pub port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_path: Option<String>,
}

/// ssh-backend.ts:38.
pub const SSH_DEFAULT_PORT: u16 = 22;

/// ssh-backend.ts:43. The one written form of an ssh machine: its id on the record, which is also the dial.
pub fn ssh_machine_id(reach: &SshReach) -> Result<String> {
    todo!()
}

/// ssh-backend.ts:51. The dial a person's `user@host` word names; `port` and `key_path` are what they typed beside it.
pub fn parse_ssh_address(address: &str, port: Option<u16>, key_path: Option<&str>) -> Result<SshReach> {
    todo!()
}

/// ssh-backend.ts:67. The login, and the port only where it is not ssh's own.
pub fn ssh_login_word(reach: &SshReach) -> String {
    todo!()
}

/// ssh-backend.ts:73. The host's first label, or the whole address where cutting would leave a number.
pub fn ssh_machine_name(reach: &SshReach) -> String {
    todo!()
}

/// ssh-backend.ts:85. Who a script runs as: root by whichever road the login has, or the login itself. A read as the
/// login never carries a password, so the enum has no room for both.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SshRide {
    /// Root as the login where it is root, else through `sudo -n`.
    #[default]
    Root,
    /// Root through sudo reading this password off stdin.
    RootByPassword(String),
    /// The login itself.
    Login,
}

/// ssh-backend.ts:85. What one script rides with.
#[derive(Clone, Default)]
pub struct SshTransportOptions {
    pub timeout_ms: Option<u64>,
    pub on_line: Option<OnLine>,
    pub stdin: Option<Vec<u8>>,
    pub ride: SshRide,
}

/// ssh-backend.ts:82. How a script reaches the machine; a test hands its own and reads what it was asked to run.
pub type SshTransport = Arc<dyn Fn(SshReach, String, SshTransportOptions) -> LocalBoxFuture<'static, ExecResult>>;

/// ssh-backend.ts:89.
pub const SSH_CONNECT_TIMEOUT_S: u64 = 10;

/// ssh-backend.ts:92. In UTF-16 units.
pub const SSH_LINE_CAP: usize = 300;

/// ssh-backend.ts:96. Words a box wrote: control characters gone, `cap` UTF-16 units kept (48 where the TS defaults).
pub fn box_word(said: &str, cap: usize) -> String {
    todo!()
}

/// ssh-backend.ts:101.
pub const SSH_CONTROL_PERSIST_S: u64 = 60;

/// ssh-backend.ts:106. `{home}/.wsp/ssh`; `None` is Node's `homedir()` ($HOME when set and non-empty, else passwd).
pub fn ssh_control_dir(home: Option<&str>) -> PathBuf {
    todo!()
}

/// ssh-backend.ts:111.
pub const SSH_CONTROL_DIR_MODE: u32 = 0o700;

/// ssh-backend.ts:115. Makes the folder (parents included) and tightens it to 0700 whatever it was.
pub fn make_ssh_control_dir(home: Option<&str>) -> Result<PathBuf> {
    todo!()
}

/// ssh-backend.ts:125. The master socket for one user, host and port; `dir` `None` is `ssh_control_dir(None)`.
pub fn ssh_control_path(reach: &SshReach, dir: Option<&str>) -> String {
    todo!()
}

/// ssh-backend.ts:136. Who a script runs as, as `ssh_args` spells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SshAs {
    #[default]
    Root,
    RootByPassword,
    Login,
}

/// ssh-backend.ts:141 `opts`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SshArgsOptions {
    pub control_dir: Option<String>,
    /// The one road that carries a file's bytes on stdin, which drops `-n`.
    pub stdin: bool,
    pub as_: SshAs,
}

/// ssh-backend.ts:141. The ssh client's argv for one script.
pub fn ssh_args(reach: &SshReach, script: &str, opts: &SshArgsOptions) -> Vec<String> {
    todo!()
}

/// ssh-backend.ts:166. How every ssh child this host starts is dialled.
pub fn ssh_dial_args(reach: &SshReach) -> Vec<String> {
    todo!()
}

/// ssh-backend.ts:182 `"one" | "rest"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarriedShape {
    One,
    Rest,
}

/// ssh-backend.ts:182. The keys of the person's config a forward child carries: `ssh -G` name, option, shape.
pub const CARRIED_SSH_OPTIONS: [(&str, &str, CarriedShape); 13] = [
    ("identityfile", "IdentityFile", CarriedShape::One),
    ("identitiesonly", "IdentitiesOnly", CarriedShape::One),
    ("certificatefile", "CertificateFile", CarriedShape::One),
    ("proxycommand", "ProxyCommand", CarriedShape::Rest),
    ("userknownhostsfile", "UserKnownHostsFile", CarriedShape::Rest),
    ("globalknownhostsfile", "GlobalKnownHostsFile", CarriedShape::Rest),
    ("hostkeyalias", "HostKeyAlias", CarriedShape::One),
    ("checkhostip", "CheckHostIP", CarriedShape::One),
    ("hostkeyalgorithms", "HostKeyAlgorithms", CarriedShape::One),
    ("pubkeyacceptedalgorithms", "PubkeyAcceptedAlgorithms", CarriedShape::One),
    ("identityagent", "IdentityAgent", CarriedShape::One),
    ("addkeystoagent", "AddKeysToAgent", CarriedShape::One),
    ("usekeychain", "UseKeychain", CarriedShape::One),
];

/// ssh-backend.ts:207. The refusal for a known hosts file the config names that is not on this computer.
pub fn missing_known_hosts_line(target: &str, file: &str) -> String {
    todo!()
}

/// ssh-backend.ts:215. A dial with the person's config already read into it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshCarried {
    pub reach: SshReach,
    pub options: Vec<String>,
}

/// ssh-backend.ts:232. The person's config for one dial, read with and without it by `ssh -G`. A known hosts file
/// the config names that is not here fails with `EngineError::MissingKnownHosts(missing_known_hosts_line(..))`.
pub async fn carried_ssh_values(reach: &SshReach, run: &SshLocalRun) -> Result<SshCarried> {
    todo!()
}

/// ssh-backend.ts:262.
pub const SSH_BACK_UP: &str = "WSP_BACK_UP";

/// ssh-backend.ts:268. The forward child's argv.
pub fn ssh_back_args(carried: &SshCarried, box_port: u16, door_port: u16) -> Vec<String> {
    todo!()
}

/// How a held child ended (`exit` with its code or its signal, or `error`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChildEnd {
    /// Exited with this code.
    Exited(i64),
    /// Ended on this signal, named as Node names it (`SIGTERM`), with no code.
    Signalled(String),
    /// Never started, or failed as a process: the error's message.
    Failed(String),
}

/// ssh-backend.ts:291. The part of a started child the holder reads; a test hands its own. `exit` resolves once, when
/// the child ends; a real child's pipes close as it does, and a fake closes them before resolving it.
pub struct HeldChild {
    pub stdin: Box<dyn AsyncWrite + Unpin>,
    pub stdout: Box<dyn AsyncRead + Unpin>,
    pub stderr: Box<dyn AsyncRead + Unpin>,
    /// Sends the named signal; true where it was sent.
    pub kill: Box<dyn FnMut(&str) -> bool>,
    pub exit: LocalBoxFuture<'static, ChildEnd>,
}

/// ssh-backend.ts:300.
pub type SshSpawn = Arc<dyn Fn(&str, &[String]) -> HeldChild>;

/// ssh-backend.ts:302. The real ssh child, with this process's env and three pipes.
pub fn spawn_ssh() -> SshSpawn {
    todo!()
}

/// ssh-backend.ts:313.
pub const HELD_TAIL: usize = 4096;

/// ssh-backend.ts:306. One held forward.
pub struct BackForward {
    inner: Arc<BackForwardState>,
}

/// The holder's own state; its shape is the implementer's.
struct BackForwardState {}

impl BackForward {
    /// Answers once the forward stands; fails with ssh's line when the child ends first. Pending until one happens.
    pub async fn up(&self) -> std::result::Result<(), String> {
        todo!()
    }

    /// Answers that line whenever the child ends.
    pub async fn ended(&self) -> String {
        todo!()
    }

    /// Ends it: the first call closes stdin and sends SIGTERM, later calls nothing.
    pub fn release(&self) {
        todo!()
    }
}

/// ssh-backend.ts:323. Starts the forward child and holds it. Needs a tokio runtime.
pub fn hold_back_forward(carried: &SshCarried, box_port: u16, door_port: u16, spawn: SshSpawn) -> BackForward {
    todo!()
}

/// ssh-backend.ts:366. The ssh client on this computer, carrying one script to the machine.
pub fn ssh_client() -> SshTransport {
    todo!()
}
