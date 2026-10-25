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
use wsp_wire::types::JsNumber;
use wsp_words::js::{is_js_whitespace, js_round, js_trim, to_number};

use crate::child_exec::{run_child, ChildOptions};
use crate::errors::{EngineError, Result};
use crate::machine::{
    BackendPricing, BytesLanded, Capabilities, ExecOptions, ExecResult, Machine, MachineBackend, MachineFacts, MachineKind, MachineLife,
    MachineListRow, MachineOffers, MachineShape, MachineSpec, MachineState, RunOptions, Size, SnapshotOptions, SnapshotStoragePricing,
    Waiting, WireSnapshotStoragePricing,
};
use crate::machine_facts::read_os_name;

/// local-backend.ts:24.
pub const LOCAL_MACHINE_ID: &str = "local";

const GB: f64 = 1024.0 * 1024.0 * 1024.0;

fn other(message: &str) -> EngineError {
    EngineError::Other { message: message.into(), code: None }
}

fn io_error(e: std::io::Error) -> EngineError {
    other(&e.to_string())
}

fn cpu_count() -> u32 {
    // SAFETY: sysconf reads no memory of ours.
    unsafe { libc::sysconf(libc::_SC_NPROCESSORS_ONLN) }.max(0) as u32
}

#[cfg(target_os = "linux")]
fn total_memory_bytes() -> u64 {
    // SAFETY: sysconf reads no memory of ours.
    let (pages, page) = unsafe { (libc::sysconf(libc::_SC_PHYS_PAGES), libc::sysconf(libc::_SC_PAGESIZE)) };
    pages.max(0) as u64 * page.max(0) as u64
}

#[cfg(not(target_os = "linux"))]
fn total_memory_bytes() -> u64 {
    sysctl_read::<u64>("hw.memsize").unwrap_or(0)
}

#[cfg(not(target_os = "linux"))]
fn sysctl_read<T: Copy>(name: &str) -> Option<T> {
    let name = std::ffi::CString::new(name).ok()?;
    let mut value = std::mem::MaybeUninit::<T>::uninit();
    let mut len = std::mem::size_of::<T>();
    // SAFETY: the kernel writes at most `len` bytes, and `value` has room for exactly that many.
    let ok = unsafe { libc::sysctlbyname(name.as_ptr(), value.as_mut_ptr().cast(), &mut len, std::ptr::null_mut(), 0) };
    // SAFETY: a zero answer with the full length means the kernel filled `value`.
    (ok == 0 && len == std::mem::size_of::<T>()).then(|| unsafe { value.assume_init() })
}

#[cfg(target_os = "linux")]
fn uptime_secs() -> f64 {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: `ts` is a valid timespec for the kernel to fill.
    unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, &mut ts) };
    ts.tv_sec as f64 + ts.tv_nsec as f64 / 1e9
}

#[cfg(not(target_os = "linux"))]
fn uptime_secs() -> f64 {
    let boot = sysctl_read::<libc::timeval>("kern.boottime");
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64());
    boot.map_or(0.0, |b| now - (b.tv_sec as f64 + b.tv_usec as f64 / 1e6))
}

/// `type()` and `release()` of node's os module: `uname -s` and `uname -r`.
fn kernel_words() -> String {
    // SAFETY: `u` is a valid utsname for the kernel to fill.
    let mut u: libc::utsname = unsafe { std::mem::zeroed() };
    if unsafe { libc::uname(&mut u) } != 0 {
        return String::new();
    }
    let words = |field: &[libc::c_char]| {
        // SAFETY: the kernel terminates each utsname field with a NUL.
        unsafe { std::ffi::CStr::from_ptr(field.as_ptr()) }.to_string_lossy().into_owned()
    };
    format!("{} {}", words(&u.sysname), words(&u.release))
}

/// local-backend.ts:31. Cores and memory in whole GB; `None` reads this computer's.
pub fn local_shape(cores: Option<u32>, total_bytes: Option<u64>) -> Size {
    let cores = cores.unwrap_or_else(cpu_count);
    let total = total_bytes.unwrap_or_else(total_memory_bytes);
    Size { cpu: f64::from(cores), mem_mb: js_round(total as f64 / GB) * 1024.0 }
}

/// local-backend.ts:37. What this computer has free this moment and in all, in MB.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryHere {
    pub free_mb: u64,
    pub total_mb: u64,
}

/// `KEY:\s*(\d+)\s*kB` under JavaScript's multiline rules: `^` also follows a `\r`, U+2028 or U+2029, and the first
/// match counts.
fn meminfo_kb(text: &str, key: &str) -> Option<f64> {
    for (i, _) in text.char_indices() {
        let Some(rest) = text[i..].strip_prefix(key).and_then(|r| r.strip_prefix(':')) else { continue };
        let at_line_start = i == 0 || text[..i].ends_with(['\n', '\r', '\u{2028}', '\u{2029}']);
        if !at_line_start {
            continue;
        }
        let rest = rest.trim_start_matches(is_js_whitespace);
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            continue;
        }
        if rest[digits..].trim_start_matches(is_js_whitespace).starts_with("kB") {
            return Some(to_number(&rest[..digits]));
        }
    }
    None
}

/// local-backend.ts:43. MemAvailable and MemTotal off /proc/meminfo.
pub fn memory_of_meminfo(meminfo: &str) -> Option<MemoryHere> {
    let mb = |key: &str| meminfo_kb(meminfo, key).map(|kb| js_round(kb / 1024.0) as u64);
    Some(MemoryHere { free_mb: mb("MemAvailable")?, total_mb: mb("MemTotal")? })
}

/// local-backend.ts:54. macOS's share of memory free over the memory the computer has.
pub fn memory_of_pressure(level: &str, total_bytes: u64) -> Option<MemoryHere> {
    let text = js_trim(level);
    if text.is_empty() {
        return None;
    }
    let pct = to_number(text);
    if pct.fract() != 0.0 || !(0.0..=100.0).contains(&pct) {
        return None;
    }
    let total_mb = (total_bytes / (1024 * 1024)) as f64;
    Some(MemoryHere { free_mb: (total_mb * pct / 100.0).floor() as u64, total_mb: total_mb as u64 })
}

/// local-backend.ts:63. This computer's memory as its kernel reads it now; never held.
pub async fn memory_here() -> Option<MemoryHere> {
    let platform = match std::env::consts::OS {
        "macos" => "darwin",
        os => os,
    };
    memory_here_on(platform).await
}

/// local-backend.ts:63 with the platform named (Node's `process.platform` words: `linux`, `darwin`, `win32`, ...).
pub async fn memory_here_on(platform: &str) -> Option<MemoryHere> {
    match platform {
        "linux" => memory_of_meminfo(&tokio::fs::read_to_string("/proc/meminfo").await.unwrap_or_default()),
        "darwin" => {
            let args = ["-n".to_string(), "kern.memorystatus_level".to_string()];
            let read = run_child("sysctl", &args, ChildOptions { timeout_ms: Some(5_000), ..Default::default() }).await;
            let level = if read.exit_code.get() == 0 { read.stdout } else { String::new() };
            memory_of_pressure(&level, total_memory_bytes())
        }
        _ => None,
    }
}

/// local-backend.ts:91. This computer as a Machine.
pub struct LocalMachine {
    root: PathBuf,
    env: IndexMap<String, String>,
    /// local-backend.ts:97. Read once: the system does not change its name while the host runs.
    os: OnceCell<String>,
}

impl LocalMachine {
    async fn shell(&self, cmd: &str, opts: ChildOptions) -> ExecResult {
        let args = ["-c".to_string(), cmd.to_string()];
        run_child("bash", &args, ChildOptions { cwd: Some(self.root.clone()), env: Some(self.env.clone()), ..opts }).await
    }
}

#[async_trait(?Send)]
impl Machine for LocalMachine {
    fn id(&self) -> &str {
        LOCAL_MACHINE_ID
    }
    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }
    fn takes_stdin(&self) -> bool {
        true
    }
    fn offers(&self) -> MachineOffers {
        MachineOffers { describe: true, facts: true, put_bytes: true, ..Default::default() }
    }
    async fn exec(&self, cmd: &str, opts: ExecOptions) -> Result<ExecResult> {
        Ok(self.shell(cmd, ChildOptions { timeout_ms: opts.timeout_ms, stdin: opts.stdin, ..Default::default() }).await)
    }
    async fn run(&self, script: &str, opts: RunOptions) -> Result<ExecResult> {
        Ok(self.shell(script, ChildOptions { timeout_ms: Some(opts.deadline_ms), on_line: opts.on_line, ..Default::default() }).await)
    }
    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> Result<String> {
        Err(other("this computer cannot be snapshotted"))
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> Result<()> {
        Err(other("this computer cannot be paused"))
    }
    async fn resume(&self, _signal: Option<&Cancel>) -> Result<()> {
        Err(other("this computer cannot be resumed"))
    }
    async fn kill(&self) -> Result<()> {
        Ok(())
    }
    async fn state(&self) -> Result<MachineState> {
        Ok(MachineState::Running)
    }
    async fn download_url(&self, _path: &str) -> Result<String> {
        Err(other("this computer serves no signed download URL; its files are read on it directly"))
    }
    async fn upload_url(&self, _path: &str) -> Result<String> {
        Err(other("this computer serves no signed upload URL; its files are written on it directly"))
    }
    async fn describe(&self) -> Result<MachineShape> {
        let shape = local_shape(None, None);
        Ok(MachineShape {
            cpu: Some(JsNumber(shape.cpu)),
            mem_mb: Some(JsNumber(shape.mem_mb)),
            disk_gb: None,
            created_at: None,
            used_bytes: None,
        })
    }
    async fn facts(&self) -> Result<MachineFacts> {
        let os = self
            .os
            .get_or_init(|| async {
                read_os_name(move |cmd: String| async move { self.exec(&cmd, ExecOptions { timeout_ms: Some(5_000), ..Default::default() }).await })
                    .await
                    .unwrap_or_else(kernel_words)
            })
            .await;
        Ok(MachineFacts {
            os: os.clone(),
            uptime_ms: JsNumber(js_round(uptime_secs() * 1_000.0)),
            folder: self.root.to_string_lossy().into_owned(),
        })
    }
    async fn put_bytes(&self, path: &str, bytes: &[u8], _timeout_ms: Option<u64>) -> Result<Option<BytesLanded>> {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        if let Some(parent) = Path::new(path).parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(io_error)?;
        }
        let mut file = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(path).map_err(io_error)?;
        file.write_all(bytes).map_err(io_error)?;
        Ok(None)
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
        let env: IndexMap<String, String> = opts.env.unwrap_or_else(|| {
            std::env::vars_os().map(|(k, v)| (k.to_string_lossy().into_owned(), v.to_string_lossy().into_owned())).collect()
        });
        let machine = LocalMachine { root: opts.root.clone(), env, os: OnceCell::new() };
        LocalBackend { machine: Arc::new(machine), folder: opts.root }
    }

    /// local-backend.ts:206. Makes the folder (parents included) and answers it.
    pub fn work_folder(&self) -> Result<&Path> {
        std::fs::create_dir_all(&self.folder).map_err(io_error)?;
        Ok(&self.folder)
    }
}

/// The one local workspace's price: nothing bills on this computer, and no snapshot storage is kept.
struct LocalPricing;

impl BackendPricing for LocalPricing {
    fn rate_usd_per_hour(&self, _size: Size) -> f64 {
        0.0
    }
    fn default_size(&self) -> Size {
        local_shape(None, None)
    }
    fn snapshot_storage(&self) -> SnapshotStoragePricing {
        SnapshotStoragePricing {
            wire: WireSnapshotStoragePricing { free_gb: JsNumber(0.0), usd_per_gb_month: JsNumber(0.0), billed_from: String::new() },
            free_named: None,
            usd_per_named_month: None,
        }
    }
}

#[async_trait(?Send)]
impl MachineBackend for LocalBackend {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            live_clone_forks: false,
            pause_mode: None,
            replaces_machine: false,
            preview_urls: false,
            signed_urls: false,
            callback_relay: false,
            disk_snapshots: false,
            images: false,
            snapshots_any_life: false,
            snapshot_listing: false,
            templates: false,
            sizes: vec![],
            kept: true,
            copies: true,
            own_network: false,
        }
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
        Arc::new(LocalPricing)
    }
    async fn create(&self, _spec: MachineSpec, _waiting: Option<Waiting>, _sent: Option<String>) -> Result<Arc<dyn Machine>> {
        Err(other("this computer already exists; it is not created, it is the computer wsp already runs on"))
    }
    async fn get(&self, _id: &str) -> Result<Arc<dyn Machine>> {
        self.work_folder()?;
        Ok(self.machine.clone())
    }
    async fn list(&self, _labels: Option<&IndexMap<String, String>>) -> Result<Vec<MachineListRow>> {
        Ok(vec![MachineListRow { id: LOCAL_MACHINE_ID.into(), state: MachineState::Running, labels: IndexMap::new(), size: None }])
    }
    async fn delete_snapshot(&self, _id: &str) -> Result<()> {
        Err(other("this computer holds no snapshots"))
    }
}
