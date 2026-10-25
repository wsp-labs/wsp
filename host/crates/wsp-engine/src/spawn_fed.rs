// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/spawn-fed.ts. Spec: spec/child_exec.md.
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::os::unix::io::AsRawFd;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
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

/// A file system error node throws out of `spawnSyncFed` itself, before any child starts: its `code` and its message
/// as node words it (`ENOENT: no such file or directory, mkdtemp '<dir>/wsp-stdin-XXXXXX'`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FsThrown {
    pub code: String,
    pub message: String,
}

/// spawn-fed.ts:14. Runs the command to its end with stdin read from an unlinked file: written 0600 into a fresh
/// `<tmp>/wsp-stdin-XXXXXX` folder, opened for reading, and the folder removed before the child starts, so nothing of
/// the bytes stays on disk under any name. stdout and stderr are piped. A file system error before the child starts is
/// answered as `error`; [`spawn_sync_fed_checked`] hands it out as node throws it.
pub fn spawn_sync_fed(command: &str, args: &[String], stdin: &[u8], opts: &FedOptions) -> FedOutput {
    spawn_sync_fed_checked(command, args, stdin, opts).unwrap_or_else(|thrown| failed(thrown.message))
}

fn failed(error: String) -> FedOutput {
    FedOutput { status: None, signal: None, stdout: Vec::new(), stderr: Vec::new(), error: Some(error) }
}

/// `spawn_sync_fed`, with the folder's and the file's failures returned as the exceptions they are in the TS.
pub fn spawn_sync_fed_checked(command: &str, args: &[String], stdin: &[u8], opts: &FedOptions) -> Result<FedOutput, FsThrown> {
    let file = stdin_file(stdin)?;
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
        Err(e) => return Ok(failed(format!("spawnSync {command} {}", errno_word(&e)))),
    };
    let deadline = opts.timeout.map(|t| Instant::now() + t);
    let mut pipes = [Pipe::of(child.stdout.take()), Pipe::of(child.stderr.take())];
    let mut timed_out = false;
    let mut status = None;
    loop {
        if status.is_none() {
            status = child.try_wait().ok().flatten();
        }
        if status.is_some() && pipes.iter().all(|p| p.file.is_none()) {
            break;
        }
        if deadline.is_some_and(|d| Instant::now() >= d) {
            timed_out = true;
            kill_and_wait(&mut child, &mut status);
            break;
        }
        let wait = deadline.map_or(20, |d| d.saturating_duration_since(Instant::now()).as_millis().min(20) as i32);
        wait_readable(&mut pipes, wait);
    }
    let [out, err] = pipes;
    Ok(FedOutput {
        status: if timed_out { None } else { status.and_then(|s| s.code()) },
        signal: status.and_then(|s| s.signal()).map(signal_name),
        stdout: out.bytes,
        stderr: err.bytes,
        error: timed_out.then(|| format!("spawnSync {command} ETIMEDOUT")),
    })
}

/// spawnSync's timeout: SIGTERM to the child, which is waited for, and its pipes closed at once, whatever else holds
/// their other end.
fn kill_and_wait(child: &mut Child, status: &mut Option<std::process::ExitStatus>) {
    if status.is_none() {
        // SAFETY: the child is still ours and not yet waited on, so its pid names it.
        unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };
        *status = child.wait().ok();
    }
}

/// One of the child's output pipes, read without blocking, and what it has given so far.
struct Pipe {
    file: Option<File>,
    bytes: Vec<u8>,
}

impl Pipe {
    fn of(pipe: Option<impl Into<std::os::fd::OwnedFd>>) -> Pipe {
        let file = pipe.map(|p| File::from(p.into()));
        if let Some(file) = &file {
            // SAFETY: the descriptor is the open pipe `file` owns.
            unsafe {
                let flags = libc::fcntl(file.as_raw_fd(), libc::F_GETFL);
                libc::fcntl(file.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK);
            }
        }
        Pipe { file, bytes: Vec::new() }
    }

    fn drain(&mut self) {
        let Some(file) = self.file.as_mut() else { return };
        let mut buf = [0u8; 65536];
        loop {
            match file.read(&mut buf) {
                Ok(0) => break self.file = None,
                Ok(n) => self.bytes.extend_from_slice(&buf[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => break self.file = None,
            }
        }
    }
}

/// Waits up to `ms` for either open pipe to have something to read or to close, and reads what is there.
fn wait_readable(pipes: &mut [Pipe; 2], ms: i32) {
    let mut fds: Vec<libc::pollfd> = pipes
        .iter()
        .filter_map(|p| p.file.as_ref())
        .map(|f| libc::pollfd { fd: f.as_raw_fd(), events: libc::POLLIN, revents: 0 })
        .collect();
    // SAFETY: `fds` is a live array of `fds.len()` pollfd records.
    unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, ms) };
    for pipe in pipes.iter_mut() {
        pipe.drain();
    }
}

/// node's `os.tmpdir()`: the first of TMPDIR, TMP and TEMP that is not empty, else `/tmp`, with a trailing slash cut.
fn node_tmpdir() -> String {
    let named = ["TMPDIR", "TMP", "TEMP"]
        .iter()
        .find_map(|k| std::env::var_os(k).map(|v| v.to_string_lossy().into_owned()).filter(|v| !v.is_empty()));
    let mut dir = named.unwrap_or_else(|| "/tmp".to_string());
    if dir.len() > 1 && dir.ends_with('/') {
        dir.pop();
    }
    dir
}

/// node's words for a file system error: `<CODE>: <what libuv says>, <syscall> '<path>'`.
fn fs_thrown(e: &std::io::Error, syscall: &str, path: &str) -> FsThrown {
    let code = crate::child_exec::errno_name(e).unwrap_or("EIO");
    let said = match code {
        "ENOENT" => "no such file or directory",
        "EACCES" => "permission denied",
        "EPERM" => "operation not permitted",
        "ENOTDIR" => "not a directory",
        "ENOSPC" => "no space left on device",
        "EMFILE" => "too many open files",
        "ENFILE" => "file table overflow",
        "ENOMEM" => "not enough memory",
        "ENAMETOOLONG" => "name too long",
        "ELOOP" => "too many symbolic links encountered",
        "EISDIR" => "illegal operation on a directory",
        _ => "i/o error",
    };
    FsThrown { code: code.to_string(), message: format!("{code}: {said}, {syscall} '{path}'") }
}

/// `mkdtempSync(join(tmpdir(), "wsp-stdin-"))`: a 0700 folder named with six random letters or digits.
fn make_temp_dir() -> Result<PathBuf, FsThrown> {
    const LETTERS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let base = node_tmpdir();
    let template = format!("{base}/wsp-stdin-XXXXXX");
    for _ in 0..100 {
        let suffix: String = (0..6).map(|_| char::from(LETTERS[rand::random::<usize>() % LETTERS.len()])).collect();
        let dir = PathBuf::from(format!("{base}/wsp-stdin-{suffix}"));
        match std::fs::DirBuilder::new().mode(0o700).create(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(fs_thrown(&e, "mkdtemp", &template)),
        }
    }
    Err(fs_thrown(&std::io::Error::from_raw_os_error(libc::EEXIST), "mkdtemp", &template))
}

/// The stdin bytes in a file already unlinked: written 0600 into a fresh folder, opened for reading, and the folder
/// removed before the child starts. The open file keeps the bytes; no name is left on disk.
fn stdin_file(bytes: &[u8]) -> Result<File, FsThrown> {
    let dir = make_temp_dir()?;
    let path = dir.join("in");
    let shown = path.display().to_string();
    let written = write_then_open(&path, bytes, &shown);
    let _ = std::fs::remove_dir_all(&dir);
    written
}

fn write_then_open(path: &Path, bytes: &[u8], shown: &str) -> Result<File, FsThrown> {
    let mut file = OpenOptions::new().write(true).create_new(true).mode(0o600).open(path).map_err(|e| fs_thrown(&e, "open", shown))?;
    file.write_all(bytes).map_err(|e| fs_thrown(&e, "write", shown))?;
    drop(file);
    File::open(path).map_err(|e| fs_thrown(&e, "open", shown))
}

/// node's errno word for a failed spawn, as its message names it.
fn errno_word(e: &std::io::Error) -> String {
    crate::child_exec::errno_name(e).map_or_else(|| e.to_string(), str::to_string)
}

/// The name node gives a signal: `SIGTERM` and the rest of Linux's, `SIG<n>` for any other.
fn signal_name(sig: i32) -> String {
    let name = match sig {
        libc::SIGHUP => "SIGHUP",
        libc::SIGINT => "SIGINT",
        libc::SIGQUIT => "SIGQUIT",
        libc::SIGILL => "SIGILL",
        libc::SIGTRAP => "SIGTRAP",
        libc::SIGABRT => "SIGABRT",
        libc::SIGBUS => "SIGBUS",
        libc::SIGFPE => "SIGFPE",
        libc::SIGKILL => "SIGKILL",
        libc::SIGUSR1 => "SIGUSR1",
        libc::SIGSEGV => "SIGSEGV",
        libc::SIGUSR2 => "SIGUSR2",
        libc::SIGPIPE => "SIGPIPE",
        libc::SIGALRM => "SIGALRM",
        libc::SIGTERM => "SIGTERM",
        libc::SIGSTKFLT => "SIGSTKFLT",
        libc::SIGCHLD => "SIGCHLD",
        libc::SIGCONT => "SIGCONT",
        libc::SIGSTOP => "SIGSTOP",
        libc::SIGTSTP => "SIGTSTP",
        libc::SIGTTIN => "SIGTTIN",
        libc::SIGTTOU => "SIGTTOU",
        libc::SIGURG => "SIGURG",
        libc::SIGXCPU => "SIGXCPU",
        libc::SIGXFSZ => "SIGXFSZ",
        libc::SIGVTALRM => "SIGVTALRM",
        libc::SIGPROF => "SIGPROF",
        libc::SIGWINCH => "SIGWINCH",
        libc::SIGIO => "SIGIO",
        libc::SIGPWR => "SIGPWR",
        libc::SIGSYS => "SIGSYS",
        _ => return format!("SIG{sig}"),
    };
    name.to_string()
}
