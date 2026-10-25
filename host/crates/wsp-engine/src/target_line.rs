// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/target-line.ts. Spec: spec/machine_reads.md.
//!
//! Who a line on a computer runs as. A box's daemon runs as root, so every line that reads or writes the agents' files
//! comes through here and runs as the owner of the home instead.
use std::future::Future;

use indexmap::IndexMap;

pub use crate::daemon_targets::Platform;
use crate::errors::Result;
use crate::machine::Machine;

/// target-line.ts:15.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetLogin {
    pub platform: Platform,
    pub home: String,
    /// Absent where neither the computer nor its shell gave one (an empty PATH is absent).
    pub path: Option<String>,
    pub user: String,
    /// The user every line is handed to, where the road is root and the home is somebody else's.
    pub run_as: Option<String>,
}

/// target-line.ts:30 `given`: the home and PATH the computer reported for its login, which win over the road's own.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GivenLogin {
    pub home: Option<String>,
    pub path: Option<String>,
}

/// target-line.ts:23.
pub const PROBE_MS: u64 = 20_000;
/// target-line.ts:71.
pub const LOGIN_SHELL_S: u64 = 15;

/// target-line.ts:30. One exec of the probe (exact text in spec/machine_reads.md) under `PROBE_MS`. Non-zero exit is
/// `the computer did not say who its lines run as: <first line of trimmed stderr, else stdout>`. Seven lines, trimmed:
/// system, uid, own name, the home's owner, runuser (1/0), the shell's HOME, the shell's PATH. Platform by
/// `platform_of_system`, Linux when unknown; user is the owner, else the own name. Not Linux or uid not 0: answered as
/// is. Root with no owner: `no_home_refusal(home)`. Root owning the home: as is. Root, somebody else's home, no runuser:
/// `no_runuser_refusal(user)`. Else `run_as` is that user.
pub async fn target_login(machine: &dyn Machine, given: &GivenLogin) -> Result<TargetLogin> {
    todo!()
}

/// target-line.ts:58. The stores whose lines this login reaches, in the map's order: all of them where nothing is handed
/// to another login; else only those at or under the home (`under_project`).
pub fn stores_reached(t: &TargetLogin, stores: &IndexMap<String, String>) -> IndexMap<String, String> {
    todo!()
}

/// target-line.ts:64. `export HOME=<q home>[ PATH=<q path>]; cd "$HOME" 2>/dev/null; <line>`, wrapped as
/// `runuser -u <q run_as> -- bash -c <q inner>` where `run_as` is set.
pub fn as_login(t: &TargetLogin, line: &str) -> String {
    todo!()
}

/// target-line.ts:76. Linux only (`None` elsewhere, nothing run). Runs the login shell line as the login under
/// `(LOGIN_SHELL_S + 5) * 1000` ms; the last stdout line trimmed, `None` where the exec failed, exited non-zero or that
/// line is empty. Never fails.
pub async fn login_shell_path(machine: &dyn Machine, t: &TargetLogin) -> Option<String> {
    todo!()
}

/// target-line.ts:92. Makes `/tmp/wsp-land-<12 hex>` with `mkdir -m 0700` (non-zero: `no private folder to stage <what>
/// in: <first>`), lands the bytes at `<dir>/bytes` by `land_bytes` on `land`, then `chmod 0600 <file>` plus
/// ` && chown -R -- <q run_as> <q dir>` where handed (non-zero: `the staging folder for <what> could not be handed to
/// <user>: <first>`), then `use_(file, dir)`. `rm -rf -- <q dir>` runs whatever happened once the folder was made, its
/// own failure ignored. Every exec gets `timeout_ms` where given.
pub async fn stage_as_login<T, F, Fut>(
    machine: &dyn Machine,
    land: &dyn Machine,
    t: &TargetLogin,
    what: &str,
    bytes: &[u8],
    use_: F,
    timeout_ms: Option<u64>,
) -> Result<T>
where
    F: FnOnce(String, String) -> Fut + Send,
    Fut: Future<Output = Result<T>> + Send,
    T: Send,
{
    todo!()
}

/// target-line.ts:120. Stages as above (`what` is `dest`), then as the login either
/// `mkdir -p <q dest> && tar -xzf <q file> -C <q dest>` (unpack) or `mkdir -p <q dirname(dest)> && cat <q file> > <q dest>`;
/// non-zero is `<dest> was not written as <user>: <first>`.
pub async fn land_as_login(machine: &dyn Machine, t: &TargetLogin, dest: &str, bytes: &[u8], unpack: bool, timeout_ms: Option<u64>) -> Result<()> {
    todo!()
}
