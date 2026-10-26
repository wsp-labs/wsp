// SPDX-License-Identifier: AGPL-3.0-only
//! One state file, one host: the lock names the process serving it and the port it bound, so a second host refuses
//! and other local tools find it. Port of `packages/host/src/host-lock.ts` at 65f6b89c7; the spec is
//! `spec/host_lock.md`. Paths are strings, as the TS joins them (`dirname`, `join`), never resolved.
use std::fs;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wsp_store::own_file::own_folder;
use wsp_wire::types::app_ports::{LOOPBACK, WS_PATH};
use wsp_wire::types::release::HostShape;
use wsp_wire::types::JsNumber;
use wsp_words::app_ports::{authority, is_wildcard, relay_url_of};
use wsp_words::exit::HostError;
use wsp_words::js::js_trim;
use wsp_words::js_date::iso_string;

use crate::repo_root::{basename, dirname, join};

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
        self.started_by.as_ref().and_then(|v| serde_json::from_value(v.clone()).ok())
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
pub fn started_by_env(env: &IndexMap<String, String>) -> Option<HostShape> {
    match env.get(STARTED_BY_ENV).map(String::as_str) {
        Some("verb") => Some(HostShape::Verb),
        Some("service") => Some(HostShape::Service),
        _ => None,
    }
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
    let v = pid.0;
    if v.fract() != 0.0 || !(-2_147_483_648.0..=2_147_483_647.0).contains(&v) {
        return Signalled::Gone;
    }
    // SAFETY: kill with signal 0 only tests whether the target can be signalled; it sends nothing.
    if unsafe { libc::kill(v as libc::pid_t, 0) } == 0 {
        return Signalled::Own;
    }
    if io::Error::last_os_error().raw_os_error() == Some(libc::EPERM) {
        Signalled::Another
    } else {
        Signalled::Gone
    }
}

/// host-lock.ts:75.
pub fn pid_alive(pid: JsNumber) -> bool {
    signalled(pid) != Signalled::Gone
}

/// host-lock.ts:79.
pub fn own_pid(pid: JsNumber) -> bool {
    signalled(pid) == Signalled::Own
}

/// host-lock.ts:84: nothing stands at the path with one trailing ` (deleted)` taken off.
pub fn program_gone(path: &str) -> bool {
    let stripped = path.strip_suffix(" (deleted)").unwrap_or(path);
    !Path::new(stripped).exists()
}

/// host-lock.ts:89-97: the program a live pid runs is gone, read off `/proc`; an unreadable link is never gone.
fn runs_vanished(pid: JsNumber) -> bool {
    match fs::read_link(format!("/proc/{}/exe", pid.js_string())) {
        Ok(exe) => program_gone(&exe.to_string_lossy()),
        Err(_) => false,
    }
}

/// host-lock.ts:100: alive, and running a program that is still there.
fn holds(lock: &HostLock) -> bool {
    pid_alive(lock.pid) && !runs_vanished(lock.pid)
}

/// `join(dirname(statePath), name)` (host-lock.ts:132-184).
fn beside(state_path: &str, name: &str) -> String {
    join(&[dirname(state_path).as_str(), name])
}

/// host-lock.ts:132.
pub fn lock_path_for(state_path: &str) -> String {
    beside(state_path, "host.lock")
}

/// host-lock.ts:137.
pub fn host_token_path(state_path: &str) -> String {
    beside(state_path, "host-token")
}

/// host-lock.ts:143: the token file's text trimmed as JavaScript's `trim()` trims, nothing where it cannot be read.
pub fn host_token_for(state_path: &str) -> Option<String> {
    fs::read_to_string(host_token_path(state_path)).ok().map(|text| js_trim(&text).to_string())
}

/// host-lock.ts:154.
pub fn host_run_dir(state_path: &str) -> String {
    beside(state_path, "runs")
}

/// host-lock.ts:162.
pub fn host_roots_path(state_path: &str) -> String {
    beside(state_path, "roots")
}

/// host-lock.ts:168.
pub fn door_port_path(state_path: &str) -> String {
    beside(state_path, "door-port")
}

/// host-lock.ts:173.
pub fn host_readings_dir(state_path: &str) -> String {
    beside(state_path, "readings")
}

/// host-lock.ts:179.
pub fn host_inbox_dir(state_path: &str) -> String {
    beside(state_path, "inbox")
}

/// host-lock.ts:184.
pub fn host_log_path(state_path: &str) -> String {
    beside(state_path, "host.log")
}

/// Where a host's door answers, as a lock or a hand-over carries it.
#[derive(Clone, Debug, PartialEq)]
pub struct BoundAt {
    pub port: JsNumber,
    pub address: Option<String>,
}

/// host-lock.ts:194-202: the app, runtime ws and state lines, and the public line when a hostname is given.
pub fn address_lines(state_path: &str, at: &BoundAt, public_hostname: Option<&str>) -> Vec<String> {
    let bound = authority(at.address.as_deref().unwrap_or(LOOPBACK), at.port.0);
    let mut lines = vec![
        format!("app         http://{bound}"),
        format!("runtime ws  ws://{bound}{WS_PATH} (token: {})", host_token_path(state_path)),
        state_line(state_path),
    ];
    if let Some(hostname) = public_hostname {
        lines.push(public_address_line(hostname));
    }
    lines
}

/// host-lock.ts:206.
pub fn state_line(state_path: &str) -> String {
    format!("state       {state_path}")
}

/// host-lock.ts:212.
pub fn public_address_line(hostname: &str) -> String {
    format!("public      {}", relay_url_of(hostname))
}

/// host-lock.ts:219: the bound address, loopback where none is written or the address is a wildcard.
pub fn dial_address(address: Option<&str>) -> String {
    match address {
        Some(at) if !is_wildcard(at) => at.to_string(),
        _ => LOOPBACK.to_string(),
    }
}

/// Parses a lock the way `lockOf` reads it (host-lock.ts:116-123): an object with numeric `pid` and `port` and a
/// string `startedAt`.
fn lock_of(text: &str) -> Option<HostLock> {
    let parsed: Value = serde_json::from_str(text).ok()?;
    let object = parsed.as_object()?;
    let numeric = |key: &str| object.get(key).is_some_and(Value::is_number);
    if !(numeric("pid") && numeric("port") && object.get("startedAt").is_some_and(Value::is_string)) {
        return None;
    }
    serde_json::from_value(parsed).ok()
}

/// The text of a file, where it stands and reads (host-lock.ts readText).
fn read_text(fs: &dyn LockFs, path: &str) -> Option<String> {
    if !fs.exists(path) {
        return None;
    }
    fs.read(path).ok()
}

fn read_lock(fs: &dyn LockFs, path: &str) -> Option<HostLock> {
    read_text(fs, path).and_then(|text| lock_of(&text))
}

/// host-lock.ts:225: the host whose lock names this state file, when that process is still alive and its program is
/// still there.
pub fn serving_host(state_path: &str) -> Option<HostLock> {
    read_lock(&RealFs, &lock_path_for(state_path)).filter(holds)
}

/// host-lock.ts:232: the lock of a host still running whose program has gone.
pub fn vanished_host(state_path: &str) -> Option<HostLock> {
    read_lock(&RealFs, &lock_path_for(state_path)).filter(|held| pid_alive(held.pid) && runs_vanished(held.pid))
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
    start: Option<&Arc<dyn HostStarter>>,
    say: &(dyn Fn(&str) + Send + Sync),
) -> Result<Option<HostLock>, HostError> {
    if let Some(held) = serving_host(state_path) {
        return Ok(Some(held));
    }
    match start {
        Some(starter) => starter.start(state_path, say).await,
        None => Ok(None),
    }
}

/// host-lock.ts:249: a held lock at `lock_path` refuses with `heldBy`'s sentence; anything else passes.
pub fn refuse_if_served(lock_path: &str, state_path: &str) -> Result<(), HostError> {
    refuse_if_served_with(&RealFs, lock_path, state_path)
}

fn refuse_if_served_with(fs: &dyn LockFs, lock_path: &str, state_path: &str) -> Result<(), HostError> {
    match read_lock(fs, lock_path) {
        Some(held) if holds(&held) => Err(held_by(&held, state_path)),
        _ => Ok(()),
    }
}

/// host-lock.ts:125-130: the refusal a held lock gives, no kind.
fn held_by(lock: &HostLock, state_path: &str) -> HostError {
    let fix = if lock.started_by.is_some() {
        "wsp down stops it, or point --state at a different file."
    } else {
        "Stop it first, or point --state at a different file."
    };
    HostError::new(format!(
        "another wsp host (pid {}) is already serving {state_path} on port {}. {fix}",
        lock.pid.js_string(),
        lock.port.js_string()
    ))
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
        fs::read_to_string(path)
    }
    fn write(&self, path: &str, text: &str) -> io::Result<()> {
        fs::write(path, text)
    }
    fn write_new(&self, path: &str, text: &str) -> io::Result<()> {
        let mut file = fs::OpenOptions::new().write(true).create_new(true).open(path)?;
        io::Write::write_all(&mut file, text.as_bytes())
    }
    fn link(&self, from: &str, to: &str) -> io::Result<()> {
        fs::hard_link(from, to)
    }
    fn remove(&self, path: &str) -> io::Result<()> {
        match fs::remove_file(path) {
            Err(gone) if gone.kind() == io::ErrorKind::NotFound => Ok(()),
            other => other,
        }
    }
    fn rename(&self, from: &str, to: &str) -> io::Result<()> {
        fs::rename(from, to)
    }
    fn names(&self, dir: &str) -> io::Result<Vec<String>> {
        fs::read_dir(dir)?.map(|entry| entry.map(|e| e.file_name().to_string_lossy().into_owned())).collect()
    }
    fn modified_ms(&self, path: &str) -> io::Result<f64> {
        let modified = fs::metadata(path)?.modified()?;
        Ok(modified.duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64() * 1000.0))
    }
    fn now_ms(&self) -> f64 {
        SystemTime::now().duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_millis() as f64)
    }
}

/// host-lock.ts:259-280 as this process: `take_lock_with(&RealFs, std::process::id(), ..)`.
pub fn take_lock(lock_path: &str, state_path: &str, ports: &LockPorts) -> Result<HostLock, HostError> {
    take_lock_with(&RealFs, std::process::id(), lock_path, state_path, ports)
}

/// host-lock.ts:259-280 as the start whose pid is `pid`, through `fs`. A refusal is a `HostError` with no kind (exit
/// class provider); a file call that fails for any reason the TS does not catch escapes as a `HostError` with no kind
/// whose message opens with node's code name for the errno (`EACCES: ...`).
pub fn take_lock_with(fs: &dyn LockFs, pid: u32, lock_path: &str, state_path: &str, ports: &LockPorts) -> Result<HostLock, HostError> {
    refuse_if_served_with(fs, lock_path, state_path)?;
    let lock = HostLock {
        pid: JsNumber(pid as f64),
        port: ports.port,
        address: ports.address.clone(),
        started_by: ports.started_by.map(|shape| serde_json::to_value(shape).unwrap_or(Value::Null)),
        started_at: iso_string(fs.now_ms()).unwrap_or_default(),
    };
    own_folder(Path::new(&dirname(state_path))).map_err(escape)?;
    own_folder(Path::new(&dirname(lock_path))).map_err(escape)?;
    let mine = format!("{lock_path}.{pid}");
    let text = serde_json::to_string(&lock).map_err(|e| HostError::new(e.to_string()))?;
    fs.write(&mine, &text).map_err(escape)?;
    let outcome = claim(fs, pid, lock_path, state_path, &mine);
    let removed = fs.remove(&mine).map_err(escape);
    outcome?;
    removed?;
    sweep_markers(fs, lock_path)?;
    Ok(lock)
}

/// The link of `mine` into the lock's place, or the take-over where a lock already stands.
fn claim(fs: &dyn LockFs, pid: u32, lock_path: &str, state_path: &str, mine: &str) -> Result<(), HostError> {
    if !link_into(fs, mine, lock_path)? {
        take_over_stale(fs, pid, lock_path, state_path, mine)?;
    }
    Ok(())
}

/// The sweep of the lock's folder: every `<lock>.taking.*` goes, and each `<lock>.<N>.taking` whose pid is gone.
fn sweep_markers(fs: &dyn LockFs, lock_path: &str) -> Result<(), HostError> {
    let folder = dirname(lock_path);
    let base = basename(lock_path);
    for name in fs.names(&folder).map_err(escape)? {
        let taking = name.starts_with(&format!("{base}.taking."));
        let crashed = name
            .strip_prefix(&format!("{base}."))
            .and_then(|rest| rest.strip_suffix(".taking"))
            .filter(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|digits| digits.parse::<f64>().ok())
            .is_some_and(|pid| !pid_alive(JsNumber(pid)));
        if taking || crashed {
            fs.remove(&join(&[folder.as_str(), name.as_str()])).map_err(escape)?;
        }
    }
    Ok(())
}

/// host-lock.ts:291-306: takes over the stale lock standing at the path, one start at a time.
fn take_over_stale(fs: &dyn LockFs, pid: u32, lock_path: &str, state_path: &str, mine: &str) -> Result<(), HostError> {
    let Some(stale) = read_text(fs, lock_path) else {
        return if link_into(fs, mine, lock_path)? { Ok(()) } else { Err(took_first(fs, lock_path, state_path)) };
    };
    if let Some(held) = lock_of(&stale).filter(holds) {
        return Err(held_by(&held, state_path));
    }
    let marker = hold_marker(fs, pid, lock_path, state_path)?;
    let outcome: Result<(), HostError> = (|| {
        let now = read_text(fs, lock_path);
        if now.is_some() && now.as_deref() != Some(stale.as_str()) {
            return Err(took_first(fs, lock_path, state_path));
        }
        fs.remove(lock_path).map_err(escape)?;
        if link_into(fs, mine, lock_path)? {
            Ok(())
        } else {
            Err(took_first(fs, lock_path, state_path))
        }
    })();
    let removed = fs.remove(&marker).map_err(escape);
    outcome?;
    removed
}

/// host-lock.ts:311-324: the first take-over marker nobody live holds, linked whole as the lock is.
fn hold_marker(fs: &dyn LockFs, pid: u32, lock_path: &str, state_path: &str) -> Result<String, HostError> {
    let own = format!("{lock_path}.{pid}.taking");
    fs.write(&own, &format!(r#"{{"pid":{pid}}}"#)).map_err(escape)?;
    let found: Result<String, HostError> = (|| {
        let mut n = 0;
        loop {
            let marker = format!("{lock_path}.taking.{n}");
            if link_into(fs, &own, &marker)? {
                return Ok(marker);
            }
            if let Some(holder) = marker_holder(fs, &marker) {
                let who = match holder {
                    Holder::Pid(p) => format!(" (pid {})", JsNumber(p).js_string()),
                    Holder::Unknown => String::new(),
                };
                return Err(HostError::new(format!("another wsp host{who} is starting on {state_path} right now; it serves in a moment.")));
            }
            n += 1;
        }
    })();
    let removed = fs.remove(&own).map_err(escape);
    let marker = found?;
    removed?;
    Ok(marker)
}

/// Who a take-over marker is held by: a live pid, or a start still writing its marker that has not yet gone stale.
enum Holder {
    Pid(f64),
    Unknown,
}

/// host-lock.ts:328-341.
fn marker_holder(fs: &dyn LockFs, marker: &str) -> Option<Holder> {
    let pid = match fs.read(marker) {
        Ok(text) => serde_json::from_str::<Value>(&text).ok().and_then(|v| v.get("pid").and_then(Value::as_f64)),
        Err(gone) if gone.kind() == io::ErrorKind::NotFound => return None,
        Err(_) => None,
    };
    if let Some(p) = pid {
        return pid_alive(JsNumber(p)).then_some(Holder::Pid(p));
    }
    match fs.modified_ms(marker) {
        Ok(mtime) if fs.now_ms() - mtime > MARKER_MS => None,
        Ok(_) => Some(Holder::Unknown),
        Err(_) => None,
    }
}

/// host-lock.ts:343-346: the winner's refusal when a lock reads, else the words for a start that took it first.
fn took_first(fs: &dyn LockFs, lock_path: &str, state_path: &str) -> HostError {
    match read_lock(fs, lock_path) {
        Some(winner) => held_by(&winner, state_path),
        None => HostError::new(format!("another wsp host just took {lock_path}")),
    }
}

/// host-lock.ts:350-352: removes the lock only where it names this process.
pub fn release_lock(lock_path: &str) -> io::Result<()> {
    if read_lock(&RealFs, lock_path).is_some_and(|held| held.pid.0 == f64::from(std::process::id())) {
        RealFs.remove(lock_path)?;
    }
    Ok(())
}

/// host-lock.ts:356-360: the lock rewritten by the host holding it, swapped in whole.
pub fn rewrite_lock(lock_path: &str, lock: &HostLock) -> io::Result<()> {
    let next = format!("{lock_path}.{}", std::process::id());
    RealFs.write(&next, &serde_json::to_string(lock).map_err(io::Error::other)?)?;
    RealFs.rename(&next, lock_path)
}

/// What link() says on a file system with no hard links: exFAT, and some network mounts.
const NO_LINKS: [i32; 3] = [libc::EPERM, libc::ENOTSUP, libc::EXDEV];

/// host-lock.ts:368-383: links a whole lock into place; false where a lock already stands. A file system with no hard
/// links gets the exclusive create instead.
fn link_into(fs: &dyn LockFs, from: &str, to: &str) -> Result<bool, HostError> {
    match fs.link(from, to) {
        Ok(()) => Ok(true),
        Err(e) => match e.raw_os_error() {
            Some(libc::EEXIST) => Ok(false),
            Some(code) if NO_LINKS.contains(&code) => exclusive_create(fs, from, to),
            _ => Err(escape(e)),
        },
    }
}

fn exclusive_create(fs: &dyn LockFs, from: &str, to: &str) -> Result<bool, HostError> {
    let bytes = fs.read(from).map_err(escape)?;
    match fs.write_new(to, &bytes) {
        Ok(()) => Ok(true),
        Err(e) if e.raw_os_error() == Some(libc::EEXIST) => Ok(false),
        Err(e) => Err(escape(e)),
    }
}

/// A file call's failure as node's error reads: the errno's name, then the system's words.
fn escape(e: io::Error) -> HostError {
    let code = e.raw_os_error().map_or("UNKNOWN", errno_name);
    HostError::new(format!("{code}: {e}"))
}

fn errno_name(code: i32) -> &'static str {
    match code {
        libc::EACCES => "EACCES",
        libc::EPERM => "EPERM",
        libc::ENOENT => "ENOENT",
        libc::EEXIST => "EEXIST",
        libc::EXDEV => "EXDEV",
        libc::EISDIR => "EISDIR",
        libc::ENOTDIR => "ENOTDIR",
        libc::ENOSPC => "ENOSPC",
        libc::EROFS => "EROFS",
        libc::EBUSY => "EBUSY",
        libc::EIO => "EIO",
        libc::ENAMETOOLONG => "ENAMETOOLONG",
        libc::ELOOP => "ELOOP",
        libc::ENOTEMPTY => "ENOTEMPTY",
        libc::EMFILE => "EMFILE",
        libc::ENFILE => "ENFILE",
        libc::EINVAL => "EINVAL",
        libc::ENOTSUP => "ENOTSUP",
        libc::EDQUOT => "EDQUOT",
        libc::ETXTBSY => "ETXTBSY",
        _ => "UNKNOWN",
    }
}
