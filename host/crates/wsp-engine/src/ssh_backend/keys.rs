// SPDX-License-Identifier: AGPL-3.0-only
//! ssh-backend.ts:377-615 at a4033dc34. Spec: spec/ssh_keys.md.
//!
//! The key a machine over ssh is known by, read off this computer's own client with nothing dialled; the alias and
//! address a typed word names; and the words a refused dial is reported in.
use std::sync::Arc;

use futures::future::BoxFuture;
use indexmap::IndexMap;

use crate::errors::Result;
use crate::machine::ExecResult;
use crate::ssh_backend::dial::{SshReach, SshTransport};

/// ssh-backend.ts:380. How the key a machine holds is read.
pub type SshHostKeyReader = Arc<dyn Fn(SshReach) -> BoxFuture<'static, Option<String>> + Send + Sync>;

/// ssh-backend.ts:384. One command on this computer: file, args, timeout in ms. A test hands its own.
pub type SshLocalRun = Arc<dyn Fn(String, Vec<String>, u64) -> BoxFuture<'static, ExecResult> + Send + Sync>;

/// ssh-backend.ts:386. `run_child(file, args, { env: this process's, timeout_ms })`.
pub fn local_run() -> SshLocalRun {
    todo!()
}

/// ssh-backend.ts:390.
pub const SSH_LOCAL_READ_MS: u64 = 5_000;
/// ssh-backend.ts:395.
pub const SSH_KEYSCAN_S: u64 = 5;
/// ssh-backend.ts:396.
pub const SSH_KEYSCAN_MS: u64 = 10_000;

/// ssh-backend.ts:401. The name the client writes this machine's key under, off its own `ssh -G` answers.
pub fn known_host_target(values: &IndexMap<String, String>) -> Option<String> {
    todo!()
}

/// ssh-backend.ts:415. The files the client checks a host key against, in its order; `home` `None` is the passwd
/// entry's home, never `$HOME`.
pub fn known_host_files(values: &IndexMap<String, String>, home: Option<&str>) -> Vec<String> {
    todo!()
}

/// ssh-backend.ts:433. The key a dial would be checked against, out of `ssh-keygen -F` lines, as `"{type} SHA256:.."`.
pub fn host_key_found(found: &str, algorithms: &str) -> Option<String> {
    todo!()
}

/// ssh-backend.ts:452. Whether the client knows this machine only through an authority.
pub fn trusted_by_authority(found: &str) -> bool {
    todo!()
}

/// What the machine itself answered a scan with, and what in the person's config stopped the scan where one did
/// (ssh-backend.ts:482's answer). Both `None` is the TS's `{}`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OfferedKey {
    pub key: Option<String>,
    /// `"ProxyJump"` or `"ProxyCommand"`.
    pub stopped_by: Option<String>,
}

/// ssh-backend.ts:482.
pub async fn offered_host_key(reach: &SshReach, run: &SshLocalRun) -> OfferedKey {
    todo!()
}

/// ssh-backend.ts:498.
pub fn ssh_word_refusal(word: &str) -> String {
    todo!()
}

/// ssh-backend.ts:505. The dial one typed word names: a login, or an alias the person's config renames.
pub async fn ssh_word_reach(word: &str, port: Option<u16>, key_path: Option<&str>, run: &SshLocalRun) -> Result<SshReach> {
    todo!()
}

/// ssh-backend.ts:522. The address ssh dials for a login; `None` where a jump or a command carries it.
pub async fn ssh_host_name(reach: &SshReach, run: &SshLocalRun) -> Option<String> {
    todo!()
}

/// ssh-backend.ts:536. The key this machine is known by.
pub async fn known_host_key(reach: &SshReach, run: &SshLocalRun) -> Option<String> {
    todo!()
}

/// ssh-backend.ts:560's answer: the file accept-new writes into and the name the entry is written under. Both `None`
/// is the TS's `{}`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KnownHostsEntry {
    pub file: Option<String>,
    pub target: Option<String>,
}

/// ssh-backend.ts:560.
pub async fn known_hosts_written(reach: &SshReach, run: &SshLocalRun) -> KnownHostsEntry {
    todo!()
}

/// ssh-backend.ts:572.
pub fn ssh_identity(host_key: &str, user: &str) -> String {
    todo!()
}

/// ssh-backend.ts:587. The client's own words with its debug log, its known_hosts note and its local forward warnings
/// taken out.
pub fn client_words(text: &str) -> String {
    todo!()
}

/// ssh-backend.ts:599. What ssh itself said when a login would not stand.
pub fn ssh_refusal_line(said: &ExecResult, reach: &SshReach) -> String {
    todo!()
}

/// ssh-backend.ts:615.
pub const SSH_DIAL_MS: u64 = 15_000;

/// ssh-backend.ts:607. One dial and nothing else: `exit 0` as the login.
pub async fn ssh_dial(reach: &SshReach, transport: &SshTransport) -> Result<()> {
    todo!()
}
