// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/child-exec.ts. Spec: spec/child_exec.md.
//!
//! One child process, read to its end: this computer's own shell and the ssh client both need it. The exit code stands
//! for the command's, a negative one for a signal, and a run cut at its deadline answers 124.
use std::path::PathBuf;

use indexmap::IndexMap;
use wsp_words::js::utf16;

use crate::machine::{ExecResult, OnLine};

/// child-exec.ts:11.
#[derive(Clone, Default)]
pub struct ChildOptions {
    pub cwd: Option<PathBuf>,
    /// The child's whole set of variables where given (nothing inherited); the host's own where `None`.
    pub env: Option<IndexMap<String, String>>,
    /// Past it the child's process group is killed and the result is 124 with the output so far.
    pub timeout_ms: Option<u64>,
    /// Each complete stdout line as it is read; the last partial line follows once, at the end.
    pub on_line: Option<OnLine>,
    /// Bytes written to the child's stdin, which is then closed; `None` leaves stdin inherited.
    pub stdin: Option<Vec<u8>>,
}

/// child-exec.ts:25. A child that could not be started at all: the code a shell answers a missing command with.
pub const CHILD_UNSTARTABLE: i64 = 127;
/// child-exec.ts:26.
pub const CHILD_TIMED_OUT: i64 = 124;
/// child-exec.ts:28. How long the pipes may stay quiet after the command exits before its answer goes without their close.
pub const DRAIN_MS: u64 = 200;
/// child-exec.ts:30. How long after the command exits its answer waits on pipes something it left running keeps writing to.
pub const DRAIN_CAP_MS: u64 = 1_000;
/// How much of a pipe one read takes: node's stream reads up to 64 KiB at a time.
const READ_BYTES: usize = 64 * 1024;

/// child-exec.ts:44. A stream's chunks as lines, the last partial one held until its newline or the flush; past `keep`
/// UTF-16 units only its tail is held, a half character at the cut written as U+FFFD. `None` keeps everything (the TS `Infinity`). Lines return to the caller rather
/// than going to a callback.
pub struct LineFeed {
    pending: String,
    keep: Option<usize>,
}

impl LineFeed {
    pub fn new(keep: Option<usize>) -> Self {
        LineFeed { pending: String::new(), keep }
    }

    /// Every line the chunk completes, in order, without its `\n` (a `\r` stays). Then the held tail is cut to its last
    /// `keep` UTF-16 units. Only the new text is searched for a newline: what was held has none.
    pub fn feed(&mut self, chunk: &str) -> Vec<String> {
        let mut lines = Vec::new();
        let mut from = 0;
        if let Some(first) = chunk.find('\n') {
            let mut line = std::mem::take(&mut self.pending);
            line.push_str(&chunk[..first]);
            lines.push(line);
            from = first + 1;
            while let Some(nl) = chunk[from..].find('\n') {
                lines.push(chunk[from..from + nl].to_string());
                from += nl + 1;
            }
        }
        self.pending.push_str(&chunk[from..]);
        if let Some(keep) = self.keep {
            if utf16::len(&self.pending) > keep {
                let tail = utf16::tail(&self.pending, keep);
                self.pending = if utf16::len(&tail) < keep { format!("\u{fffd}{tail}") } else { tail };
            }
        }
        lines
    }

    /// The held partial line where it is not empty; the feed holds nothing after.
    pub fn flush(&mut self) -> Option<String> {
        let rest = std::mem::take(&mut self.pending);
        (!rest.is_empty()).then_some(rest)
    }
}

/// One pipe's bytes read as UTF-8 across its reads: a character cut at the end of one read waits for the next, so a
/// read boundary never writes U+FFFD. Bytes that are not UTF-8 become U+FFFD as `String::from_utf8_lossy` has it.
#[derive(Default)]
pub struct Utf8Stream {
    held: Vec<u8>,
}

impl Utf8Stream {
    pub fn push(&mut self, bytes: &[u8]) -> String {
        let mut joined = std::mem::take(&mut self.held);
        joined.extend_from_slice(bytes);
        let mut out = String::with_capacity(joined.len());
        let mut rest = joined.as_slice();
        loop {
            match std::str::from_utf8(rest) {
                Ok(text) => {
                    out.push_str(text);
                    return out;
                }
                Err(e) => {
                    let (good, bad) = rest.split_at(e.valid_up_to());
                    out.push_str(std::str::from_utf8(good).unwrap_or_default());
                    match e.error_len() {
                        Some(n) => {
                            out.push('\u{fffd}');
                            rest = &bad[n..];
                        }
                        None => {
                            self.held = bad.to_vec();
                            return out;
                        }
                    }
                }
            }
        }
    }

    /// The pipe ended: a character it left unfinished is U+FFFD.
    pub fn finish(&mut self) -> String {
        if std::mem::take(&mut self.held).is_empty() {
            String::new()
        } else {
            "\u{fffd}".to_string()
        }
    }
}

/// The timer a select arm waits on: `None` never fires.
async fn until(at: Option<tokio::time::Instant>) {
    match at {
        Some(at) => tokio::time::sleep_until(at).await,
        None => std::future::pending().await,
    }
}

/// One read of a pipe that is still open; a closed one never answers.
async fn read_some<R: tokio::io::AsyncRead + Unpin>(pipe: &mut Option<R>, buf: &mut [u8]) -> std::io::Result<usize> {
    use tokio::io::AsyncReadExt;
    match pipe {
        Some(pipe) => pipe.read(buf).await,
        None => std::future::pending().await,
    }
}

/// node's word for a system error that stopped a spawn.
pub(crate) fn errno_name(e: &std::io::Error) -> Option<&'static str> {
    Some(match e.raw_os_error()? {
        libc::EPERM => "EPERM",
        libc::ENOENT => "ENOENT",
        libc::EIO => "EIO",
        libc::E2BIG => "E2BIG",
        libc::ENOEXEC => "ENOEXEC",
        libc::EBADF => "EBADF",
        libc::EAGAIN => "EAGAIN",
        libc::ENOMEM => "ENOMEM",
        libc::EACCES => "EACCES",
        libc::EFAULT => "EFAULT",
        libc::ENOTDIR => "ENOTDIR",
        libc::EISDIR => "EISDIR",
        libc::EINVAL => "EINVAL",
        libc::ENFILE => "ENFILE",
        libc::EMFILE => "EMFILE",
        libc::ETXTBSY => "ETXTBSY",
        libc::ENOSPC => "ENOSPC",
        libc::ENAMETOOLONG => "ENAMETOOLONG",
        libc::ELOOP => "ELOOP",
        _ => return None,
    })
}

fn command(file: &str, args: &[String], opts: &ChildOptions) -> tokio::process::Command {
    use std::process::Stdio;
    let mut cmd = tokio::process::Command::new(file);
    cmd.args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).process_group(0);
    if let Some(cwd) = &opts.cwd {
        cmd.current_dir(cwd);
    }
    if let Some(vars) = &opts.env {
        cmd.env_clear().envs(vars);
    }
    cmd
}

/// child-exec.ts:63. Spawns `file` with `args` as the leader of its own process group, reads stdout and stderr as UTF-8
/// to the end and answers; never fails. An unstartable child answers 127 with the spawn error's message and a newline
/// appended to stderr. The deadline SIGKILLs the whole group and the answer is 124. Once the child exits, its output is
/// read until the pipes close, go quiet for `DRAIN_MS`, or `DRAIN_CAP_MS` has passed, and then both pipes are closed
/// whatever still holds their other end. A signal death with no deadline is -1; no code and no signal is 0. An
/// executable with no `#!` line is run by `/bin/sh`, as `execvp` does.
pub async fn run_child(file: &str, args: &[String], opts: ChildOptions) -> ExecResult {
    use std::os::unix::process::ExitStatusExt;
    use tokio::io::AsyncWriteExt;
    use tokio::time::{Duration, Instant};

    let spawned = match command(file, args, &opts).spawn() {
        Err(e) if e.raw_os_error() == Some(libc::ENOEXEC) => {
            let by_shell: Vec<String> = std::iter::once(file.to_string()).chain(args.iter().cloned()).collect();
            command("/bin/sh", &by_shell, &opts).spawn()
        }
        other => other,
    };
    let mut child = match spawned {
        Ok(child) => child,
        Err(e) => {
            let why = errno_name(&e).map_or_else(|| e.to_string(), str::to_string);
            return ExecResult { exit_code: CHILD_UNSTARTABLE.into(), stdout: String::new(), stderr: format!("spawn {file} {why}\n") };
        }
    };
    let pid = child.id();
    // A stdin the caller gave is written and closed; one it did not give stays open until the child is done, as a pipe does.
    let mut writer = None;
    let held_stdin = match child.stdin.take() {
        Some(mut stdin) => match opts.stdin.clone() {
            Some(bytes) => {
                writer = Some(tokio::spawn(async move {
                    let _ = stdin.write_all(&bytes).await;
                }));
                None
            }
            None => Some(stdin),
        },
        None => None,
    };

    let mut out = child.stdout.take();
    let mut err = child.stderr.take();
    let (mut out_buf, mut err_buf) = (vec![0u8; READ_BYTES], vec![0u8; READ_BYTES]);
    let (mut out_text, mut err_text) = (Utf8Stream::default(), Utf8Stream::default());
    let mut lines = opts.on_line.as_ref().map(|_| LineFeed::new(None));
    let emit = |lines: &mut Option<LineFeed>, chunk: &str| {
        if let (Some(feed), Some(on_line)) = (lines.as_mut(), opts.on_line.as_ref()) {
            for line in feed.feed(chunk) {
                on_line(&line);
            }
        }
    };
    let mut stdout = String::new();
    let mut stderr = String::new();
    let deadline = opts.timeout_ms.map(|ms| Instant::now() + Duration::from_millis(ms));
    let mut timed_out = false;
    let mut exited: Option<i64> = None;
    let mut quiet_until: Option<Instant> = None;
    let mut cap_until: Option<Instant> = None;
    // Everything runs in this one task: a chunk is worked through before the next is read, so what the pipes hold
    // when the command exits is read under the drain timers, not queued behind them.
    loop {
        tokio::select! {
            n = read_some(&mut out, &mut out_buf), if out.is_some() => {
                let text = match n {
                    Ok(n) if n > 0 => out_text.push(&out_buf[..n]),
                    _ => {
                        out = None;
                        out_text.finish()
                    }
                };
                emit(&mut lines, &text);
                stdout.push_str(&text);
                if exited.is_some() && out.is_some() {
                    quiet_until = Some(Instant::now() + Duration::from_millis(DRAIN_MS));
                }
            }
            n = read_some(&mut err, &mut err_buf), if err.is_some() => {
                let text = match n {
                    Ok(n) if n > 0 => err_text.push(&err_buf[..n]),
                    _ => {
                        err = None;
                        err_text.finish()
                    }
                };
                stderr.push_str(&text);
                if exited.is_some() && err.is_some() {
                    quiet_until = Some(Instant::now() + Duration::from_millis(DRAIN_MS));
                }
            }
            status = child.wait(), if exited.is_none() => {
                let (code, signal) = status.map(|s| (s.code(), s.signal())).unwrap_or((None, None));
                let answer = if timed_out { CHILD_TIMED_OUT } else { code.map_or(if signal.is_some() { -1 } else { 0 }, i64::from) };
                let now = Instant::now();
                quiet_until = Some(now + Duration::from_millis(DRAIN_MS));
                cap_until = Some(now + Duration::from_millis(DRAIN_CAP_MS));
                exited = Some(answer);
            }
            _ = until(deadline), if !timed_out && exited.is_none() => {
                timed_out = true;
                if let Some(pid) = pid {
                    // SAFETY: the group leader is our child and not yet reaped, so its group id is still its pid.
                    unsafe { libc::kill(-(pid as i32), libc::SIGKILL) };
                }
            }
            _ = until(quiet_until), if exited.is_some() => break,
            _ = until(cap_until), if exited.is_some() => break,
        }
        if exited.is_some() && out.is_none() && err.is_none() {
            break;
        }
    }
    // The answer is out: closing the read ends gives a child still writing its SIGPIPE, and holds no descriptor.
    drop(out);
    drop(err);
    drop(held_stdin);
    if let Some(writer) = writer {
        writer.abort();
    }
    let (rest_out, rest_err) = (out_text.finish(), err_text.finish());
    emit(&mut lines, &rest_out);
    stdout.push_str(&rest_out);
    stderr.push_str(&rest_err);
    if let Some(feed) = lines.as_mut() {
        if let (Some(rest), Some(on_line)) = (feed.flush(), opts.on_line.as_ref()) {
            on_line(&rest);
        }
    }
    ExecResult { exit_code: exited.unwrap_or(0).into(), stdout, stderr }
}
