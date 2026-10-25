// SPDX-License-Identifier: AGPL-3.0-only
//! One state file, one host: the lock names the process serving it and the port it bound, so a second host refuses
//! and other local tools find it. Port of `packages/host/src/host-lock.ts` at 65f6b89c7; the spec is
//! `spec/host_lock.md`. Paths are strings, as the TS joins them (`dirname`, `join`), never resolved.
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use wsp_wire::types::release::HostShape;
use wsp_wire::types::JsNumber;
use wsp_words::exit::HostError;

/// The lock as `JSON.parse` reads it and `JSON.stringify` writes it (host-lock.ts:10-20). Keys in write order:
/// `pid`, then the `ports` the taker passed in their own order (`port`, `address`, `startedBy`), then `startedAt`.
/// `pid` and `port` are any JSON number (the TS checks `typeof === "number"` only); `startedBy` is kept as written,
/// whatever it holds, since the refusal reads only whether it is there.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostLock {
    pub pid: JsNumber,
    pub port: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_by: Option<Value>,
    pub started_at: String,
}

impl HostLock {
    /// `startedBy` as one of the three roads, nothing where it is absent or any other value.
    pub fn started_by_shape(&self) -> Option<HostShape> {
        todo!()
    }
}

/// What a taker writes beside its pid (host-lock.ts:259): the port asked for or bound, the address, the road.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LockPorts {
    pub port: JsNumber,
    pub address: Option<String>,
    pub started_by: Option<HostShape>,
}

/// host-lock.ts:29.
pub const STARTED_BY_ENV: &str = "WSP_STARTED_BY";

/// host-lock.ts:35: `verb` or `service` exactly; `up` and anything else read as nothing.
pub fn started_by_env(env: &indexmap::IndexMap<String, String>) -> Option<HostShape> {
    todo!()
}

/// host-lock.ts:38.
pub const POLL_MS: u64 = 200;

/// host-lock.ts:43.
pub const SERVICE_WAIT_MS: u64 = 20_000;

/// host-lock.ts:285.
pub const MARKER_MS: f64 = 10_000.0;

/// What signal 0 says about a pid (host-lock.ts:64-71).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Signalled {
    Own,
    Another,
    Gone,
}

/// `process.kill(pid, 0)` read once: a pid node refuses before it signals (not an integer, or outside the 32-bit
/// signed range) is `Gone`; `EPERM` is `Another`; any other failure `Gone`; success `Own`. Zero and negative pids
/// are signalled as node signals them (a process group), so they read `Own`.
pub fn signalled(pid: JsNumber) -> Signalled {
    todo!()
}

/// host-lock.ts:75.
pub fn pid_alive(pid: JsNumber) -> bool {
    todo!()
}

/// host-lock.ts:79.
pub fn own_pid(pid: JsNumber) -> bool {
    todo!()
}

/// host-lock.ts:84: nothing stands at the path with one trailing ` (deleted)` taken off.
pub fn program_gone(path: &str) -> bool {
    todo!()
}

/// `join(dirname(statePath), "host.lock")` (host-lock.ts:132).
pub fn lock_path_for(state_path: &str) -> String {
    todo!()
}

/// host-lock.ts:137.
pub fn host_token_path(state_path: &str) -> String {
    todo!()
}

/// host-lock.ts:143: the token file's text trimmed as JavaScript's `trim()` trims, nothing where it cannot be read.
pub fn host_token_for(state_path: &str) -> Option<String> {
    todo!()
}

/// host-lock.ts:154.
pub fn host_run_dir(state_path: &str) -> String {
    todo!()
}

/// host-lock.ts:162.
pub fn host_roots_path(state_path: &str) -> String {
    todo!()
}

/// host-lock.ts:168.
pub fn door_port_path(state_path: &str) -> String {
    todo!()
}

/// host-lock.ts:173.
pub fn host_readings_dir(state_path: &str) -> String {
    todo!()
}

/// host-lock.ts:179.
pub fn host_inbox_dir(state_path: &str) -> String {
    todo!()
}

/// host-lock.ts:184.
pub fn host_log_path(state_path: &str) -> String {
    todo!()
}

/// Where a host's door answers, as a lock or a hand-over carries it.
#[derive(Clone, Debug, PartialEq)]
pub struct BoundAt {
    pub port: JsNumber,
    pub address: Option<String>,
}

/// host-lock.ts:194-202: the app, runtime ws and state lines, and the public line when a hostname is given.
pub fn address_lines(state_path: &str, at: &BoundAt, public_hostname: Option<&str>) -> Vec<String> {
    todo!()
}

/// host-lock.ts:206.
pub fn state_line(state_path: &str) -> String {
    todo!()
}

/// host-lock.ts:212.
pub fn public_address_line(hostname: &str) -> String {
    todo!()
}

/// host-lock.ts:219: the bound address, loopback where none is written or the address is a wildcard.
pub fn dial_address(address: Option<&str>) -> String {
    todo!()
}

/// host-lock.ts:225.
pub fn serving_host(state_path: &str) -> Option<HostLock> {
    todo!()
}

/// host-lock.ts:232.
pub fn vanished_host(state_path: &str) -> Option<HostLock> {
    todo!()
}

/// What starts a host on this computer when a line needs one (`HostStarter`, host-start.ts): handed the state path
/// and a line sayer, it answers the lock of the host it brought up, or nothing.
#[async_trait::async_trait]
pub trait HostStarter: Send + Sync {
    async fn start(&self, state_path: &str, say: &(dyn Fn(&str) + Send + Sync)) -> Result<Option<HostLock>, HostError>;
}

/// host-lock.ts:240: the serving host, else what the starter brings up, else nothing. The starter is not called
/// when a host serves.
pub async fn held_or_started(
    state_path: &str,
    start: Option<&std::sync::Arc<dyn HostStarter>>,
    say: &(dyn Fn(&str) + Send + Sync),
) -> Result<Option<HostLock>, HostError> {
    todo!()
}

/// host-lock.ts:249: a held lock at `lock_path` refuses with `heldBy`'s sentence; anything else passes.
pub fn refuse_if_served(lock_path: &str, state_path: &str) -> Result<(), HostError> {
    todo!()
}

/// The file calls `take_lock` makes, one seam so a test can stand in for another start or another file system
/// (the TS test mocks `node:fs`, host-lock-take.test.ts:12-38). `RealFs` is the only production implementation.
pub trait LockFs {
    /// `existsSync`.
    fn exists(&self, path: &str) -> bool;
    /// `readFileSync(path, "utf8")`.
    fn read(&self, path: &str) -> io::Result<String>;
    /// `writeFileSync(path, text)`: create or truncate.
    fn write(&self, path: &str, text: &str) -> io::Result<()>;
    /// `writeFileSync(path, text, { flag: "wx" })`: fails with `AlreadyExists` where anything stands.
    fn write_new(&self, path: &str, text: &str) -> io::Result<()>;
    /// `linkSync(from, to)`.
    fn link(&self, from: &str, to: &str) -> io::Result<()>;
    /// `rmSync(path, { force: true })`: a missing path is no error.
    fn remove(&self, path: &str) -> io::Result<()>;
    /// `renameSync`.
    fn rename(&self, from: &str, to: &str) -> io::Result<()>;
    /// `readdirSync(dir)`: the names, in the order the file system gives them.
    fn names(&self, dir: &str) -> io::Result<Vec<String>>;
    /// `statSync(path).mtimeMs`.
    fn modified_ms(&self, path: &str) -> io::Result<f64>;
    /// `Date.now()`.
    fn now_ms(&self) -> f64;
}

/// The file system itself.
pub struct RealFs;

impl LockFs for RealFs {
    fn exists(&self, path: &str) -> bool {
        Path::new(path).exists()
    }
    fn read(&self, path: &str) -> io::Result<String> {
        todo!()
    }
    fn write(&self, path: &str, text: &str) -> io::Result<()> {
        todo!()
    }
    fn write_new(&self, path: &str, text: &str) -> io::Result<()> {
        todo!()
    }
    fn link(&self, from: &str, to: &str) -> io::Result<()> {
        todo!()
    }
    fn remove(&self, path: &str) -> io::Result<()> {
        todo!()
    }
    fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        todo!()
    }
    fn names(&self, dir: &str) -> io::Result<Vec<String>> {
        todo!()
    }
    fn modified_ms(&self, path: &str) -> io::Result<f64> {
        todo!()
    }
    fn now_ms(&self) -> f64 {
        todo!()
    }
}

/// host-lock.ts:259-280 as this process: `take_lock_with(&RealFs, std::process::id(), ..)`.
pub fn take_lock(lock_path: &str, state_path: &str, ports: &LockPorts) -> Result<HostLock, HostError> {
    todo!()
}

/// host-lock.ts:259-280 as the start whose pid is `pid`, through `fs`. A refusal is a `HostError` with no kind (exit
/// class provider); a file call that fails for any reason the TS does not catch escapes as a `HostError` with no kind
/// whose message opens with node's code name for the errno (`EACCES: ...`).
pub fn take_lock_with(fs: &dyn LockFs, pid: u32, lock_path: &str, state_path: &str, ports: &LockPorts) -> Result<HostLock, HostError> {
    todo!()
}

/// host-lock.ts:350: removes the lock only where it names `std::process::id()`.
pub fn release_lock(lock_path: &str) -> io::Result<()> {
    todo!()
}

/// host-lock.ts:356: written to `<lock>.<pid>` and renamed over the lock.
pub fn rewrite_lock(lock_path: &str, lock: &HostLock) -> io::Result<()> {
    todo!()
}
