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
    /// `keep` UTF-16 units.
    pub fn feed(&mut self, chunk: &str) -> Vec<String> {
        self.pending.push_str(chunk);
        let mut lines = Vec::new();
        while let Some(nl) = self.pending.find('\n') {
            lines.push(self.pending[..nl].to_string());
            self.pending.drain(..=nl);
        }
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

/// Which stream a chunk came from, or that one closed.
enum Piece {
    Out(Vec<u8>),
    Err(Vec<u8>),
    OutClosed,
    ErrClosed,
}

/// The timer a select arm waits on: `None` never fires.
async fn until(at: Option<tokio::time::Instant>) {
    match at {
        Some(at) => tokio::time::sleep_until(at).await,
        None => std::future::pending().await,
    }
}

/// child-exec.ts:63. Spawns `file` with `args` as the leader of its own process group, reads stdout and stderr as UTF-8
/// to the end and answers; never fails. An unstartable child answers 127 with the spawn error's message and a newline
/// appended to stderr. The deadline SIGKILLs the whole group and the answer is 124. Once the child exits, its output is
/// read until the pipes close, go quiet for `DRAIN_MS`, or `DRAIN_CAP_MS` has passed. A signal death with no deadline is
/// -1; no code and no signal is 0.
pub async fn run_child(file: &str, args: &[String], opts: ChildOptions) -> ExecResult {
    use std::os::unix::process::ExitStatusExt;
    use std::process::Stdio;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::time::{Duration, Instant};

    let mut cmd = tokio::process::Command::new(file);
    cmd.args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).process_group(0);
    if let Some(cwd) = &opts.cwd {
        cmd.current_dir(cwd);
    }
    if let Some(vars) = &opts.env {
        cmd.env_clear().envs(vars);
    }
    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => {
            let why = match e.kind() {
                std::io::ErrorKind::NotFound => "ENOENT".to_string(),
                std::io::ErrorKind::PermissionDenied => "EACCES".to_string(),
                _ => e.to_string(),
            };
            return ExecResult { exit_code: CHILD_UNSTARTABLE.into(), stdout: String::new(), stderr: format!("spawn {file} {why}\n") };
        }
    };
    let pid = child.id();
    // A stdin the caller gave is written and closed; one it did not give stays open until the child is done, as a pipe does.
    let held_stdin = match child.stdin.take() {
        Some(mut stdin) => match opts.stdin.clone() {
            Some(bytes) => {
                tokio::spawn(async move {
                    let _ = stdin.write_all(&bytes).await;
                });
                None
            }
            None => Some(stdin),
        },
        None => None,
    };

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Piece>();
    if let Some(mut out) = child.stdout.take() {
        let tx = tx.clone();
        tokio::spawn(async move {
            let mut buf = vec![0u8; 8192];
            while let Ok(n) = out.read(&mut buf).await {
                if n == 0 {
                    break;
                }
                let _ = tx.send(Piece::Out(buf[..n].to_vec()));
            }
            let _ = tx.send(Piece::OutClosed);
        });
    } else {
        let _ = tx.send(Piece::OutClosed);
    }
    if let Some(mut err) = child.stderr.take() {
        let tx = tx.clone();
        tokio::spawn(async move {
            let mut buf = vec![0u8; 8192];
            while let Ok(n) = err.read(&mut buf).await {
                if n == 0 {
                    break;
                }
                let _ = tx.send(Piece::Err(buf[..n].to_vec()));
            }
            let _ = tx.send(Piece::ErrClosed);
        });
    } else {
        let _ = tx.send(Piece::ErrClosed);
    }
    drop(tx);

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
    let (mut out_closed, mut err_closed) = (false, false);
    let deadline = opts.timeout_ms.map(|ms| Instant::now() + Duration::from_millis(ms));
    let mut timed_out = false;
    let mut exited: Option<(i64, Instant, Instant)> = None;
    let answer = |code: Option<i32>, signal: Option<i32>, timed_out: bool| -> i64 {
        if timed_out {
            CHILD_TIMED_OUT
        } else {
            code.map_or(if signal.is_some() { -1 } else { 0 }, i64::from)
        }
    };
    let mut quiet_until: Option<Instant> = None;
    let mut cap_until: Option<Instant> = None;
    let exit_status = loop {
        tokio::select! {
            piece = rx.recv() => match piece {
                Some(Piece::Out(bytes)) => {
                    let text = String::from_utf8_lossy(&bytes).into_owned();
                    emit(&mut lines, &text);
                    stdout.push_str(&text);
                    if exited.is_some() {
                        quiet_until = Some(Instant::now() + Duration::from_millis(DRAIN_MS));
                    }
                }
                Some(Piece::Err(bytes)) => {
                    stderr.push_str(&String::from_utf8_lossy(&bytes));
                    if exited.is_some() {
                        quiet_until = Some(Instant::now() + Duration::from_millis(DRAIN_MS));
                    }
                }
                Some(Piece::OutClosed) => out_closed = true,
                Some(Piece::ErrClosed) => err_closed = true,
                None => {
                    out_closed = true;
                    err_closed = true;
                }
            },
            status = child.wait(), if exited.is_none() => {
                let (code, signal) = status.map(|s| (s.code(), s.signal())).unwrap_or((None, None));
                let code = answer(code, signal, timed_out);
                let now = Instant::now();
                quiet_until = Some(now + Duration::from_millis(DRAIN_MS));
                cap_until = Some(now + Duration::from_millis(DRAIN_CAP_MS));
                exited = Some((code, now, now));
            },
            _ = until(deadline), if !timed_out && exited.is_none() => {
                timed_out = true;
                if let Some(pid) = pid {
                    // SAFETY: the group leader is our child and not yet reaped, so its group id is still its pid.
                    unsafe { libc::kill(-(pid as i32), libc::SIGKILL) };
                }
            },
            _ = until(quiet_until), if exited.is_some() => break exited.map(|e| e.0),
            _ = until(cap_until), if exited.is_some() => break exited.map(|e| e.0),
        }
        if exited.is_some() && out_closed && err_closed {
            break exited.map(|e| e.0);
        }
    };
    let code = exit_status.unwrap_or(0);
    drop(held_stdin);
    if let Some(feed) = lines.as_mut() {
        if let (Some(rest), Some(on_line)) = (feed.flush(), opts.on_line.as_ref()) {
            on_line(&rest);
        }
    }
    ExecResult { exit_code: code.into(), stdout, stderr }
}
