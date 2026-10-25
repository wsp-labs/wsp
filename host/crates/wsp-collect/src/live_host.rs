// SPDX-License-Identifier: AGPL-3.0-only
//! The live Host: this computer's disk and processes behind the Host seam. Port of packages/collect/src/live-host.ts
//! at a3b870a6e; the spec is spec/live_host.md. The one spawn helper here is chapter 06's `spawnRun`.
use std::cmp::Ordering;
use std::collections::{HashMap, VecDeque};
use std::ffi::{CStr, CString, OsStr};
use std::io::{self, Read};
use std::os::unix::fs::MetadataExt;
use std::process::{ExitStatus, Stdio};
use std::rc::Rc;
use std::time::Duration;

use futures::future::LocalBoxFuture;
use futures::stream::LocalBoxStream;
use futures::{FutureExt, StreamExt};
use indexmap::IndexMap;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use wsp_words::exit::HostError;
use wsp_words::js::{is_js_whitespace, js_trim, to_number};

pub use crate::host::SpawnRefusal;
use crate::host::{Host, HostExec, HostFs, Platform, RunOptions, Stat, StatKind};

/// Folders a size and a walk never enter (live-host.ts:11).
pub const SKIP_DIRS: [&str; 2] = [".git", "node_modules"];
/// How much of a file one read of its lines takes, in bytes (live-host.ts:129).
pub const LINE_CHUNK: usize = 1 << 20;
/// stdout past this many bytes fails the run (live-host.ts:146).
pub const MAX_OUTPUT: usize = 64 * 1024 * 1024;
/// How long a run waits after the child exits, or after the budget's SIGTERM, before it settles (live-host.ts:148).
pub const CLOSE_GRACE_MS: u64 = 1000;
/// The longest timer node sets: a longer budget is `setTimeout`'s one millisecond.
const MAX_TIMER_MS: u64 = 0x7fff_ffff;
/// The budget when `RunOptions.timeout_ms` is unset (live-host.ts:203).
pub const DEFAULT_TIMEOUT_MS: u64 = 120_000;

const LF: u8 = 0x0a;
const CR: u8 = 0x0d;

fn utf16_order(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// `path.join(folder, name)` for POSIX paths: empty segments are skipped, then the result is normalised, so `.` segments
/// go, `..` steps back and a trailing slash stays.
fn join_path(folder: &str, name: &str) -> String {
    let joined = match (folder.is_empty(), name.is_empty()) {
        (true, true) => return ".".to_string(),
        (true, false) => name.to_string(),
        (false, true) => folder.to_string(),
        (false, false) => format!("{folder}/{name}"),
    };
    let absolute = joined.starts_with('/');
    let trailing = joined.ends_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for seg in joined.split('/') {
        match seg {
            "" | "." => {}
            ".." if parts.last().is_some_and(|last| *last != "..") => {
                parts.pop();
            }
            ".." if absolute => {}
            other => parts.push(other),
        }
    }
    let body = parts.join("/");
    match (body.is_empty(), absolute, trailing) {
        (true, true, _) => "/".to_string(),
        (true, false, true) => "./".to_string(),
        (true, false, false) => ".".to_string(),
        (false, abs, trail) => format!("{}{body}{}", if abs { "/" } else { "" }, if trail { "/" } else { "" }),
    }
}

/// An OS string as node reads one: UTF-8, each bad sequence one U+FFFD.
fn lossy(text: &OsStr) -> String {
    text.to_string_lossy().into_owned()
}

/// Runs a blocking read of the disk off the caller's thread, as libuv's pool does for node: a read stuck on a pipe or a
/// hung mount leaves the thread's timers free to fire.
async fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    match tokio::task::spawn_blocking(work).await {
        Ok(out) => out,
        Err(e) => std::panic::resume_unwind(e.into_panic()),
    }
}

fn signal(target: i32, sig: i32) {
    unsafe {
        libc::kill(target, sig);
    }
}

/// Every regular file's bytes under dir, links skipped, SKIP_DIRS not entered, unreadable entries counted as 0. A name
/// that is not UTF-8 is read as node reads it, with U+FFFD, and the path made from that is looked up as it stands.
pub async fn tree_bytes(dir: &str) -> u64 {
    let dir = dir.to_string();
    blocking(move || tree_bytes_now(&dir)).await
}

fn tree_bytes_now(dir: &str) -> u64 {
    let mut total = 0;
    let mut folders = vec![dir.to_string()];
    while let Some(folder) = folders.pop() {
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_symlink() {
                continue;
            }
            let name = lossy(&entry.file_name());
            let path = join_path(&folder, &name);
            if kind.is_dir() {
                if !SKIP_DIRS.contains(&name.as_str()) {
                    folders.push(path);
                }
            } else if kind.is_file() {
                if let Ok(meta) = std::fs::metadata(&path) {
                    total += meta.len();
                }
            }
        }
    }
    total
}

/// The real disk (live-host.ts:53).
#[derive(Debug, Clone, Copy, Default)]
pub struct NodeFs;

/// The state `lines` carries from one read of the file to the next.
struct LineReader<'a> {
    path: &'a str,
    needle: Option<Vec<u8>>,
    started: bool,
    done: bool,
    file: Option<tokio::fs::File>,
    chunk: Vec<u8>,
    pieces: Vec<u8>,
    ready: VecDeque<String>,
}

impl LineReader<'_> {
    async fn read_chunk(&mut self) {
        if !self.started {
            self.started = true;
            match tokio::fs::File::open(self.path).await {
                Ok(file) => {
                    self.file = Some(file);
                    self.chunk = vec![0; LINE_CHUNK];
                }
                Err(_) => {
                    self.done = true;
                    return;
                }
            }
        }
        let Some(file) = self.file.as_mut() else {
            self.done = true;
            return;
        };
        let n = match file.read(&mut self.chunk).await {
            Ok(n) => n,
            Err(_) => {
                self.done = true;
                self.file = None;
                return;
            }
        };
        let needle = self.needle.as_deref();
        if n == 0 {
            if !self.pieces.is_empty() {
                self.ready.extend(take_line(&self.pieces, needle));
            }
            self.done = true;
            self.file = None;
            return;
        }
        let data = &self.chunk[..n];
        let mut start = 0;
        while let Some(off) = data[start..].iter().position(|&b| b == LF) {
            let nl = start + off;
            let line = if self.pieces.is_empty() {
                take_line(&data[start..nl], needle)
            } else {
                self.pieces.extend_from_slice(&data[start..nl]);
                let line = take_line(&self.pieces, needle);
                self.pieces.clear();
                line
            };
            self.ready.extend(line);
            start = nl + 1;
        }
        if start < n {
            self.pieces.extend_from_slice(&data[start..]);
        }
    }
}

/// One line's bytes as the TS `take` reads them: skipped without `needle`, one trailing CR dropped, decoded lossily.
fn take_line(line: &[u8], needle: Option<&[u8]>) -> Option<String> {
    if needle.is_some_and(|n| !contains(line, n)) {
        return None;
    }
    let end = if line.last() == Some(&CR) { line.len() - 1 } else { line.len() };
    Some(String::from_utf8_lossy(&line[..end]).into_owned())
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    needle.is_empty() || hay.windows(needle.len()).any(|w| w == needle)
}

/// The longest string node can make, in bytes of the file it decodes (`0x1fffffe8`): a longer file reads as nothing.
const MAX_TEXT_BYTES: u64 = 0x1fff_ffe8;

fn stat_now(path: &str) -> Option<Stat> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime_ms = meta.mtime() as f64 * 1e3 + meta.mtime_nsec() as f64 / 1e6;
    if meta.is_dir() {
        Some(Stat { kind: StatKind::Dir, bytes: tree_bytes_now(path), mtime_ms })
    } else if meta.is_file() {
        Some(Stat { kind: StatKind::File, bytes: meta.len(), mtime_ms })
    } else {
        None
    }
}

fn list_now(dir: &str) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries.flatten().map(|e| lossy(&e.file_name())).collect();
    names.sort_by(|a, b| utf16_order(a, b));
    names
}

/// `readFile(path, "utf8")`: the bytes, up to the longest string node holds, decoded with U+FFFD for bad bytes.
fn read_text_now(path: &str) -> Option<String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path).ok()?.take(MAX_TEXT_BYTES + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() as u64 > MAX_TEXT_BYTES {
        return None;
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn walk_now(dir: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut folders = vec![dir.to_string()];
    while let Some(folder) = folders.pop() {
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_symlink() {
                continue;
            }
            let name = lossy(&entry.file_name());
            let path = join_path(&folder, &name);
            if kind.is_dir() {
                if !SKIP_DIRS.contains(&name.as_str()) {
                    folders.push(path);
                }
            } else if kind.is_file() {
                out.push(path);
            }
        }
    }
    out.sort_by(|a, b| utf16_order(a, b));
    out
}

impl HostFs for NodeFs {
    fn stat<'a>(&'a self, path: &'a str) -> LocalBoxFuture<'a, Option<Stat>> {
        let path = path.to_string();
        async move { blocking(move || stat_now(&path)).await }.boxed_local()
    }

    fn list<'a>(&'a self, dir: &'a str) -> LocalBoxFuture<'a, Vec<String>> {
        let dir = dir.to_string();
        async move { blocking(move || list_now(&dir)).await }.boxed_local()
    }

    fn read_text<'a>(&'a self, path: &'a str) -> LocalBoxFuture<'a, Option<String>> {
        let path = path.to_string();
        async move { blocking(move || read_text_now(&path)).await }.boxed_local()
    }

    fn walk<'a>(&'a self, dir: &'a str) -> LocalBoxFuture<'a, Vec<String>> {
        let dir = dir.to_string();
        async move { blocking(move || walk_now(&dir)).await }.boxed_local()
    }

    /// Bytes are searched for `holding` before anything is decoded; a line is the bytes up to LF, one trailing CR
    /// dropped, decoded as UTF-8 with U+FFFD for bad bytes; the last line is given without an LF and only when it has
    /// bytes. A read error ends the stream quietly, as the TS does (never an `Err` item).
    fn lines<'a>(&'a self, path: &'a str, holding: Option<&'a str>) -> LocalBoxStream<'a, std::io::Result<String>> {
        let reader = LineReader {
            path,
            needle: holding.map(|h| h.as_bytes().to_vec()),
            started: false,
            done: false,
            file: None,
            chunk: Vec::new(),
            pieces: Vec::new(),
            ready: VecDeque::new(),
        };
        futures::stream::unfold(reader, |mut r| async move {
            loop {
                if let Some(line) = r.ready.pop_front() {
                    return Some((Ok(line), r));
                }
                if r.done {
                    return None;
                }
                r.read_chunk().await;
            }
        })
        .boxed_local()
    }
}

/// Whether `bin` is an executable file in a folder of PATH; empty PATH entries skipped (live-host.ts:133).
pub async fn on_path(bin: &str, path_var: Option<&str>) -> bool {
    for dir in path_var.unwrap_or("").split(':') {
        if dir.is_empty() {
            continue;
        }
        let Ok(path) = CString::new(join_path(dir, bin)) else {
            continue;
        };
        if unsafe { libc::access(path.as_ptr(), libc::X_OK) } == 0 {
            return true;
        }
    }
    false
}

/// The real processes (live-host.ts:150): `which` reads this process's PATH, `run` spawns under this process's
/// environment with `opts.env` laid over it.
#[derive(Debug, Clone, Copy, Default)]
pub struct NodeExec;

impl HostExec for NodeExec {
    fn which<'a>(&'a self, bin: &'a str) -> LocalBoxFuture<'a, bool> {
        async move { on_path(bin, std::env::var_os("PATH").map(|p| lossy(&p)).as_deref()).await }.boxed_local()
    }

    fn run<'a>(&'a self, cmd: &'a str, args: &'a [String], opts: RunOptions) -> LocalBoxFuture<'a, Option<String>> {
        // The seam's `run` has no error: a command node refuses reads as a command that failed. `try_run` carries it.
        async move { self.try_run(cmd, args, opts).await.unwrap_or(None) }.boxed_local()
    }

    fn try_run<'a>(
        &'a self,
        cmd: &'a str,
        args: &'a [String],
        opts: RunOptions,
    ) -> LocalBoxFuture<'a, Result<Option<String>, SpawnRefusal>> {
        async move {
            let mut env: IndexMap<String, String> =
                std::env::vars_os().map(|(k, v)| (lossy(&k), lossy(&v))).filter(|(k, _)| !is_array_index(k)).collect();
            if let Some(given) = &opts.env {
                for (k, v) in given {
                    env.insert(k.clone(), v.clone());
                }
            }
            let env: Vec<(String, String)> = env.into_iter().collect();
            spawn_run(cmd, args, &env, opts).await
        }
        .boxed_local()
    }
}

/// A name JavaScript keeps in an object's indexed slots: a whole number below 2^32 - 1 with no leading zero. Node's
/// `process.env` cannot read such a name back, so spreading it leaves the variable out.
fn is_array_index(name: &str) -> bool {
    !name.is_empty()
        && name.bytes().all(|b| b.is_ascii_digit())
        && (name == "0" || !name.starts_with('0'))
        && name.parse::<u64>().is_ok_and(|n| n <= 4_294_967_294)
}

/// The pid and every process under it, breadth first, off one `ps -A -o pid=,ppid=` listing; `[pid]` alone when ps
/// fails (live-host.ts:157).
pub fn tree_of(pid: u32) -> Vec<u32> {
    let listing = match std::process::Command::new("ps").args(["-A", "-o", "pid=,ppid="]).output() {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).into_owned(),
        _ => return vec![pid],
    };
    let children = children_of(&listing);
    let mut tree = vec![pid];
    let mut i = 0;
    while i < tree.len() {
        if let Some(kids) = children.get(&tree[i]) {
            tree.extend_from_slice(kids);
        }
        i += 1;
    }
    tree
}

/// A pid token read as JavaScript's `Number()` reads it; anything that is not a whole number in u32 range is None.
fn pid_of(token: &str) -> Option<u32> {
    let n = to_number(token);
    (n.fract() == 0.0 && (0.0..=f64::from(u32::MAX)).contains(&n)).then_some(n as u32)
}

/// The children map `tree_of` walks, read off a `ps -A -o pid=,ppid=` listing; a line that is not two numbers is
/// skipped, and children keep the listing's order.
pub fn children_of(listing: &str) -> HashMap<u32, Vec<u32>> {
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    for line in listing.split('\n') {
        let mut tokens = js_trim(line).split(is_js_whitespace).filter(|t| !t.is_empty());
        let (Some(pid), Some(parent)) = (tokens.next(), tokens.next()) else {
            continue;
        };
        let (Some(pid), Some(parent)) = (pid_of(pid), pid_of(parent)) else {
            continue;
        };
        children.entry(parent).or_default().push(pid);
    }
    children
}

/// One read off a pipe; a pipe already closed never answers, so its select branch stays quiet.
async fn read_pipe<R: AsyncRead + Unpin>(pipe: &mut Option<R>, buf: &mut [u8]) -> io::Result<usize> {
    match pipe {
        Some(pipe) => pipe.read(buf).await,
        None => std::future::pending().await,
    }
}

/// One line as `util.inspect` quotes a string: `'` unless the text holds one and no `"`, then `"`, then a backtick, and
/// control characters and the quote in use escaped.
fn quote_line(line: &str) -> String {
    let quote = if !line.contains('\'') {
        '\''
    } else if !line.contains('"') {
        '"'
    } else if !line.contains('`') && !line.contains("${") {
        '`'
    } else {
        '\''
    };
    let mut out = String::new();
    out.push(quote);
    for c in line.chars() {
        match c {
            '\u{8}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            '\\' => out.push_str("\\\\"),
            c if c == quote && quote == '\'' => out.push_str("\\'"),
            c if (c as u32) < 0x20 || (0x7f..=0x9f).contains(&(c as u32)) => out.push_str(&format!("\\x{:02X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

/// What node's argument errors print for a value: `util.inspect` of the string, which cuts a text over 124 UTF-16 units
/// into its lines joined by `' +\n  '`, then the first 128 units and `...` when more.
fn inspect_string(s: &str) -> String {
    let units = s.encode_utf16().count();
    let inspected = if units > 124 { s.split_inclusive('\n').map(quote_line).collect::<Vec<_>>().join(" +\n  ") } else { quote_line(s) };
    if inspected.encode_utf16().count() > 128 {
        let cut: Vec<u16> = inspected.encode_utf16().take(128).collect();
        return format!("{}...", String::from_utf16_lossy(&cut));
    }
    inspected
}

fn must_be_free_of_nul(value: &str, name: &str, property: bool) -> Result<(), SpawnRefusal> {
    if !value.contains('\0') {
        return Ok(());
    }
    let message = if property {
        format!("The property '{name}' must be a string without null bytes. Received {}", inspect_string(value))
    } else {
        format!("The argument '{name}' must be a string without null bytes. Received {}", inspect_string(value))
    };
    Err(SpawnRefusal { message })
}

/// The checks node's `spawn` makes before it starts anything, in its order (child_process.js `normalizeSpawnArguments`).
fn check_spawn(cmd: &str, args: &[String], env: &[(String, String)]) -> Result<(), SpawnRefusal> {
    if cmd.is_empty() {
        return Err(SpawnRefusal { message: "The argument 'file' cannot be empty. Received ''".to_string() });
    }
    must_be_free_of_nul(cmd, "file", false)?;
    for (i, arg) in args.iter().enumerate() {
        must_be_free_of_nul(arg, &format!("args[{i}]"), false)?;
    }
    let mut ordered: Vec<&(String, String)> = env.iter().collect();
    ordered.sort_by_key(|(key, _)| if is_array_index(key) { (0, key.parse::<u64>().unwrap_or(0)) } else { (1, 0) });
    for (key, value) in ordered {
        let name = format!("options.env['{key}']");
        must_be_free_of_nul(key, &name, true)?;
        must_be_free_of_nul(value, &name, true)?;
    }
    Ok(())
}

/// One child as the leader of its own process group under exactly `env`, answered with its stdout on exit 0 and
/// `None` otherwise; the group dies with the run, past the budget or once the child exits (spec rules 5 to 12). A
/// signal already cancelled answers `None` first; then an empty command or a NUL byte is a refusal, as node throws it.
pub async fn spawn_run(cmd: &str, args: &[String], env: &[(String, String)], opts: RunOptions) -> Result<Option<String>, SpawnRefusal> {
    if opts.signal.as_ref().is_some_and(CancellationToken::is_cancelled) {
        return Ok(None);
    }
    check_spawn(cmd, args, env)?;
    Ok(spawn_checked(cmd, args, env, opts).await)
}

async fn spawn_checked(cmd: &str, args: &[String], env: &[(String, String)], opts: RunOptions) -> Option<String> {
    let mut command = tokio::process::Command::new(cmd);
    command.args(args).env_clear().envs(env.iter().map(|(k, v)| (k, v))).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    // SAFETY: between fork and exec the only calls are setsid and signal, which are async-signal-safe.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(io::Error::last_os_error());
            }
            // libuv hands a child the default for every signal, whatever the host ignores (nohup leaves HUP ignored).
            for sig in 1..32 {
                if sig != libc::SIGKILL && sig != libc::SIGSTOP {
                    libc::signal(sig, libc::SIG_DFL);
                }
            }
            Ok(())
        });
    }
    let mut child = command.spawn().ok()?;
    let pid = child.id()? as i32;
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let mut out: Vec<u8> = Vec::new();
    let mut bytes = 0usize;
    let mut buf = vec![0u8; 64 * 1024];
    let mut err_buf = vec![0u8; 64 * 1024];

    let grace = Duration::from_millis(CLOSE_GRACE_MS);
    let budget_ms = match opts.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS) {
        ms if ms > MAX_TIMER_MS => 1,
        ms => ms,
    };
    let budget_at = Instant::now() + Duration::from_millis(budget_ms);
    let mut budget_live = true;
    let mut grace_at = Instant::now();
    let mut grace_live = false;
    let cancellable = opts.signal.is_some();
    let token = opts.signal.clone().unwrap_or_default();
    let mut aborted = false;
    let mut failed = false;
    let mut status: Option<ExitStatus> = None;

    loop {
        if status.is_some() && stdout.is_none() && stderr.is_none() {
            break;
        }
        tokio::select! {
            read = read_pipe(&mut stdout, &mut buf), if stdout.is_some() => match read {
                Ok(n) if n > 0 => {
                    bytes += n;
                    if bytes > MAX_OUTPUT {
                        failed = true;
                        signal(-pid, libc::SIGKILL);
                    } else {
                        out.extend_from_slice(&buf[..n]);
                    }
                }
                _ => stdout = None,
            },
            read = read_pipe(&mut stderr, &mut err_buf), if stderr.is_some() => {
                if !matches!(read, Ok(n) if n > 0) {
                    stderr = None;
                }
            }
            waited = child.wait(), if status.is_none() => match waited {
                Ok(s) => {
                    status = Some(s);
                    grace_at = Instant::now() + grace;
                    grace_live = true;
                }
                Err(_) => {
                    failed = true;
                    break;
                }
            },
            _ = tokio::time::sleep_until(budget_at), if budget_live => {
                budget_live = false;
                failed = true;
                signal(-pid, libc::SIGTERM);
                if !grace_live {
                    grace_at = Instant::now() + grace;
                    grace_live = true;
                }
            }
            _ = tokio::time::sleep_until(grace_at), if grace_live => break,
            _ = token.cancelled(), if cancellable && !aborted => {
                aborted = true;
                failed = true;
                if status.is_some() {
                    break;
                }
                let tree = tree_of(pid as u32);
                signal(-pid, libc::SIGKILL);
                for p in tree {
                    signal(p as i32, libc::SIGKILL);
                }
            }
        }
    }

    signal(-pid, libc::SIGKILL);
    drop(stdout);
    drop(stderr);
    if status.is_none() {
        status = child.wait().await.ok();
    }
    match status {
        Some(s) if !failed && s.code() == Some(0) => Some(String::from_utf8_lossy(&out).into_owned()),
        _ => None,
    }
}

/// `darwin` and `linux` only (live-host.ts:254); the argument is Rust's `std::env::consts::OS` word.
pub fn platform_of(os: &str) -> Option<Platform> {
    match os {
        "macos" => Some(Platform::Darwin),
        "linux" => Some(Platform::Linux),
        _ => None,
    }
}

/// The name node gives this OS in `process.platform`, for the sentence `node_host` answers an unknown one with.
fn node_platform_word(os: &str) -> &str {
    match os {
        "windows" => "win32",
        "solaris" | "illumos" => "sunos",
        other => other,
    }
}

/// The home folder from the password entry of the effective user, as libuv reads it when `HOME` is unset.
fn passwd_home() -> Option<String> {
    let mut buf = vec![0u8; 1024];
    loop {
        // SAFETY: every pointer is to memory owned here and alive for the call; `pw_dir` is read before `buf` changes.
        unsafe {
            let mut entry: libc::passwd = std::mem::zeroed();
            let mut found: *mut libc::passwd = std::ptr::null_mut();
            let rc = libc::getpwuid_r(libc::geteuid(), &mut entry, buf.as_mut_ptr().cast(), buf.len(), &mut found);
            if rc == libc::ERANGE {
                buf.resize(buf.len() * 2, 0);
                continue;
            }
            if rc != 0 || found.is_null() || entry.pw_dir.is_null() {
                return None;
            }
            return Some(CStr::from_ptr(entry.pw_dir).to_string_lossy().into_owned());
        }
    }
}

/// This computer as a Host: platform, home, and SHELL, TERM_PROGRAM, XDG_CONFIG_HOME, CLAUDE_CONFIG_DIR and
/// CODEX_HOME where set and not empty; `stores` and `seeds` absent. An unknown platform is the TS throw's sentence. The
/// home is `HOME`, else the password entry's, as `os.homedir()` has it.
pub fn node_host() -> Result<Host, wsp_words::exit::HostError> {
    let platform = platform_of(std::env::consts::OS).ok_or_else(|| {
        HostError::new(format!("wsp reads a computer's setup on macOS or Linux only, not {}", node_platform_word(std::env::consts::OS)))
    })?;
    let var = |name: &str| std::env::var_os(name).map(|v| lossy(&v)).filter(|v| !v.is_empty());
    let home = match std::env::var_os("HOME") {
        Some(home) => lossy(&home),
        None => passwd_home()
            .ok_or_else(|| HostError::new("A system error occurred: uv_os_homedir returned ENOENT (no such file or directory)"))?,
    };
    Ok(Host {
        platform,
        home,
        shell: var("SHELL"),
        terminal: var("TERM_PROGRAM"),
        xdg_config_home: var("XDG_CONFIG_HOME"),
        claude_config_dir: var("CLAUDE_CONFIG_DIR"),
        codex_home: var("CODEX_HOME"),
        stores: None,
        seeds: None,
        fs: Rc::new(NodeFs),
        exec: Rc::new(NodeExec),
    })
}
