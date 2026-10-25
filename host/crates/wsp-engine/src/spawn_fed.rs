// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/spawn-fed.ts. Spec: spec/child_exec.md.
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::ExitStatusExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use indexmap::IndexMap;

/// The spawnSync options spawn-fed.ts passes through (`input` and `stdio` are its own).
#[derive(Debug, Clone, Default)]
pub struct FedOptions {
    pub cwd: Option<PathBuf>,
    /// The child's whole set of variables where given.
    pub env: Option<IndexMap<String, String>>,
    /// spawnSync's `timeout`: past it the child is killed with SIGTERM and `error` says it timed out.
    pub timeout: Option<Duration>,
}

/// spawnSync's answer: `status` is `None` where the child died of a signal or never started; `error` is the spawn or
/// timeout failure's message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FedOutput {
    pub status: Option<i32>,
    pub signal: Option<String>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub error: Option<String>,
}

/// spawn-fed.ts:14. Runs the command to its end with stdin read from an unlinked file: written 0600 into a fresh
/// `<tmp>/wsp-stdin-XXXXXX` folder, opened for reading, and the folder removed before the child starts, so nothing of
/// the bytes stays on disk under any name. stdout and stderr are piped.
pub fn spawn_sync_fed(command: &str, args: &[String], stdin: &[u8], opts: &FedOptions) -> FedOutput {
    let failed = |error: String| FedOutput { status: None, signal: None, stdout: Vec::new(), stderr: Vec::new(), error: Some(error) };
    let file = match stdin_file(stdin) {
        Ok(file) => file,
        Err(e) => return failed(e.to_string()),
    };
    let mut cmd = Command::new(command);
    cmd.args(args).stdin(Stdio::from(file)).stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(cwd) = &opts.cwd {
        cmd.current_dir(cwd);
    }
    if let Some(vars) = &opts.env {
        cmd.env_clear().envs(vars);
    }
    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => return failed(format!("spawnSync {command} {}", errno_word(&e))),
    };
    let stdout = child.stdout.take().map(|mut out| thread::spawn(move || read_all(&mut out)));
    let stderr = child.stderr.take().map(|mut err| thread::spawn(move || read_all(&mut err)));
    let deadline = opts.timeout.map(|t| Instant::now() + t);
    let mut timed_out = false;
    let status = loop {
        if let Ok(Some(status)) = child.try_wait() {
            break Some(status);
        }
        if deadline.is_some_and(|d| Instant::now() >= d) {
            timed_out = true;
            // SAFETY: the child is still ours and not yet waited on, so its pid names it.
            unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };
            break child.wait().ok();
        }
        thread::sleep(Duration::from_millis(2));
    };
    let joined = |reader: Option<thread::JoinHandle<Vec<u8>>>| reader.map(|h| h.join().unwrap_or_default()).unwrap_or_default();
    FedOutput {
        status: if timed_out { None } else { status.and_then(|s| s.code()) },
        signal: status.and_then(|s| s.signal()).map(signal_name),
        stdout: joined(stdout),
        stderr: joined(stderr),
        error: timed_out.then(|| format!("spawnSync {command} ETIMEDOUT")),
    }
}

/// The stdin bytes in a file already unlinked: written 0600 into a fresh folder, opened for reading, and the folder
/// removed before the child starts. The open file keeps the bytes; no name is left on disk.
fn stdin_file(bytes: &[u8]) -> std::io::Result<File> {
    let dir = std::env::temp_dir().join(format!("wsp-stdin-{:016x}", rand::random::<u64>()));
    std::fs::create_dir(&dir)?;
    let path = dir.join("in");
    let written = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .and_then(|mut f| f.write_all(bytes))
        .and_then(|()| File::open(&path));
    let _ = std::fs::remove_dir_all(&dir);
    written
}

fn read_all(from: &mut impl Read) -> Vec<u8> {
    let mut out = Vec::new();
    let _ = from.read_to_end(&mut out);
    out
}

/// node's errno word for a failed spawn, as its message names it.
fn errno_word(e: &std::io::Error) -> String {
    match e.kind() {
        std::io::ErrorKind::NotFound => "ENOENT".into(),
        std::io::ErrorKind::PermissionDenied => "EACCES".into(),
        _ => e.to_string(),
    }
}

/// The name node gives a signal: `SIGTERM` and the rest of the common ones, `SIG<n>` for any other.
fn signal_name(sig: i32) -> String {
    let name = match sig {
        1 => "SIGHUP",
        2 => "SIGINT",
        3 => "SIGQUIT",
        6 => "SIGABRT",
        9 => "SIGKILL",
        10 => "SIGUSR1",
        11 => "SIGSEGV",
        12 => "SIGUSR2",
        13 => "SIGPIPE",
        14 => "SIGALRM",
        15 => "SIGTERM",
        _ => return format!("SIG{sig}"),
    };
    name.to_string()
}
