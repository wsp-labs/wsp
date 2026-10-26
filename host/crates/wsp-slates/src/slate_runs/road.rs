// SPDX-License-Identifier: AGPL-3.0-only
//! Unit runs_road: slate-runs.ts:165-195, 244-361, 491-497 at 65f6b89c7. Spec: spec/runs_road.md.
//!
//! `on.line` and `on.end` are called with no borrow of the road's own state held (rule 3.3).
use std::cell::RefCell;
use std::io;
use std::os::unix::process::ExitStatusExt;
use std::process::Stdio;
use std::rc::Rc;

use indexmap::IndexMap;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::Child;
use tokio::sync::oneshot;
use tokio::task::{spawn_local, AbortHandle};
use wsp_core::clock::{Cancel, Clock};
use wsp_wire::types::run_block::RUN_OUTPUT_MAX_CHARS;
use wsp_wire::wire::daemon::{EXEC_DEADLINE_EXIT, EXEC_OUTPUT_MAX};
use wsp_words::js::{js_trim, number_string, utf16};
use wsp_words::run_block::run_output_tail;

use super::parts::{Reshape, ReshapeAnswer, CLOSE_GRACE_MS};

/// Which stream a line came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stream {
    Out,
    Err,
}

/// What a road starts: the agent's text as bash -c's one argument, the values beside it, never in it.
#[derive(Clone, Debug, PartialEq)]
pub struct RoadStart {
    pub cmd: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub env: IndexMap<String, String>,
    pub stdin: Option<String>,
    pub timeout_s: f64,
    pub stream: bool,
}

/// How a command ended, its streams whole and not yet scrubbed. `error` is the spawn error's message.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RoadEnd {
    pub code: Option<i64>,
    pub signal: Option<String>,
    pub error: Option<String>,
    pub out: String,
    pub err: String,
    pub cut: bool,
    pub timed_out: bool,
}

/// What a road tells as the command runs: each line where it streams, then the end, once.
pub trait RoadEvents {
    fn line(&self, stream: Stream, text: &str);
    fn end(&self, end: RoadEnd);
}

/// A start's answer: the kill takes the whole group; the pid is the command's on this computer, where it runs here.
#[derive(Clone)]
pub struct RoadStarted {
    pub kill: Rc<dyn Fn()>,
    pub pid: Option<u32>,
}

/// A `then`: `cmd` under bash -c with `input` on stdin.
#[derive(Clone)]
pub struct ReshapeStart {
    pub cmd: String,
    pub input: String,
    pub cwd: String,
    pub env: IndexMap<String, String>,
    pub timeout_s: f64,
    pub scrub: Rc<dyn Fn(&str) -> String>,
}

/// The computer a run's command runs on: this one's child, or the thread's own machine.
pub trait RunRoad {
    /// SLATE_DIR on that computer; empty for this one.
    fn slate_dir(&self) -> String;
    fn start(&self, o: RoadStart, on: Rc<dyn RoadEvents>) -> RoadStarted;
    fn reshape(&self, o: ReshapeStart) -> Reshape;
}

/// Needs a tokio `LocalSet` on the current thread.
pub fn reshape_result(o: ReshapeStart, clock: Rc<dyn Clock>) -> Reshape {
    let (tx, rx) = oneshot::channel();
    let mut cmd = tokio::process::Command::new("bash");
    cmd.arg("-c")
        .arg(&o.cmd)
        .current_dir(&o.cwd)
        .env_clear()
        .envs(&o.env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    new_session(&mut cmd);
    let pid = match cmd.spawn() {
        Err(e) => {
            let end = RoadEnd { error: Some(spawn_failure(&e)), ..RoadEnd::default() };
            let _ = tx.send(reshape_answer(&end, o.timeout_s, &*o.scrub));
            None
        }
        Ok(child) => {
            let pid = child.id();
            let mode = Mode::Reshape { timeout_s: o.timeout_s, scrub: o.scrub.clone() };
            let run = Rc::new(Run::new(clock, pid, mode));
            run.life.borrow_mut().answer = Some(tx);
            wire(&run, child, Some(o.input), o.timeout_s);
            pid
        }
    };
    let done = Box::pin(async move {
        rx.await.unwrap_or(ReshapeAnswer::Failed { why: "then ended with no answer".into(), exit: None, err: String::new() })
    });
    Reshape { done, kill: group_killer(pid) }
}

/// What a `then` that ended says: the JSON it printed, or why not, on whichever computer it ran.
pub fn reshape_answer(end: &RoadEnd, timeout_s: f64, scrub: &dyn Fn(&str) -> String) -> ReshapeAnswer {
    let err = run_output_tail(&scrub(&end.err), RUN_OUTPUT_MAX_CHARS as usize);
    if let Some(error) = &end.error {
        return ReshapeAnswer::Failed { why: format!("then could not start: {}", scrub(error)), exit: None, err };
    }
    if end.timed_out {
        return ReshapeAnswer::Failed {
            why: format!("then timed out after {} s", number_string(timeout_s)),
            exit: Some(EXEC_DEADLINE_EXIT),
            err,
        };
    }
    if end.code != Some(0) {
        let said = end.code.map_or_else(|| "a signal".to_string(), |c| c.to_string());
        return ReshapeAnswer::Failed { why: format!("then exited with {said}"), exit: end.code, err };
    }
    let out = scrub(&end.out);
    match serde_json::from_str::<serde_json::Value>(&out) {
        Ok(json) => ReshapeAnswer::Json(json),
        Err(_) => {
            let trimmed = js_trim(&out);
            let why = if trimmed.is_empty() {
                "then printed no JSON".to_string()
            } else {
                let first = trimmed.split('\n').next().unwrap_or_default();
                format!("then printed no JSON: {}", utf16::cut(first, 120))
            };
            ReshapeAnswer::Failed { why, exit: Some(0), err }
        }
    }
}

/// This computer's road. Needs a tokio `LocalSet` on the current thread.
pub fn local_road(clock: Rc<dyn Clock>) -> Rc<dyn RunRoad> {
    Rc::new(LocalRoad { clock })
}

struct LocalRoad {
    clock: Rc<dyn Clock>,
}

impl RunRoad for LocalRoad {
    fn slate_dir(&self) -> String {
        String::new()
    }
    fn start(&self, o: RoadStart, on: Rc<dyn RoadEvents>) -> RoadStarted {
        let mut cmd = tokio::process::Command::new("bash");
        cmd.arg("-c")
            .arg(&o.cmd)
            .arg("bash")
            .args(&o.args)
            .current_dir(&o.cwd)
            .env_clear()
            .envs(&o.env)
            .stdin(if o.stdin.is_some() { Stdio::piped() } else { Stdio::null() })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        new_session(&mut cmd);
        match cmd.spawn() {
            Err(e) => {
                let end = RoadEnd { error: Some(spawn_failure(&e)), ..RoadEnd::default() };
                spawn_local(async move { on.end(end) });
                RoadStarted { kill: group_killer(None), pid: None }
            }
            Ok(child) => {
                let pid = child.id();
                let mode = Mode::Road { on, stream: o.stream };
                let run = Rc::new(Run::new(self.clock.clone(), pid, mode));
                wire(&run, child, o.stdin, o.timeout_s);
                RoadStarted { kill: group_killer(pid), pid }
            }
        }
    }
    fn reshape(&self, o: ReshapeStart) -> Reshape {
        reshape_result(o, self.clock.clone())
    }
}

/// SIGKILL to the group `pid` leads; every error ignored.
pub fn kill_group(pid: u32) {
    if pid == 0 {
        return;
    }
    // SAFETY: a positive pid names a child this host started, which leads its own group; the result is ignored as the TS does.
    unsafe {
        libc::kill(-(pid as i32), libc::SIGKILL);
    }
}

const OUTPUT_MAX: usize = EXEC_OUTPUT_MAX as usize;
const READ_BYTES: usize = 64 * 1024;
const STREAMS: [Stream; 2] = [Stream::Out, Stream::Err];

fn group_killer(pid: Option<u32>) -> Rc<dyn Fn()> {
    Rc::new(move || {
        if let Some(pid) = pid {
            kill_group(pid);
        }
    })
}

fn new_session(cmd: &mut tokio::process::Command) {
    // SAFETY: between fork and exec the only call is setsid, which is async-signal-safe; it is what node's detached spawn does.
    unsafe {
        cmd.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

fn spawn_failure(e: &io::Error) -> String {
    let word = match e.raw_os_error() {
        Some(libc::ENOENT) => "ENOENT".to_string(),
        Some(libc::EACCES) => "EACCES".to_string(),
        Some(libc::EPERM) => "EPERM".to_string(),
        Some(libc::ENOEXEC) => "ENOEXEC".to_string(),
        Some(libc::ENOTDIR) => "ENOTDIR".to_string(),
        Some(libc::ENOMEM) => "ENOMEM".to_string(),
        Some(libc::EMFILE) => "EMFILE".to_string(),
        Some(libc::ENFILE) => "ENFILE".to_string(),
        Some(libc::EAGAIN) => "EAGAIN".to_string(),
        Some(libc::ETXTBSY) => "ETXTBSY".to_string(),
        _ => e.to_string(),
    };
    format!("spawn bash {word}")
}

fn signal_name(n: i32) -> Option<String> {
    let name = match n {
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
        _ => return None,
    };
    Some(name.to_string())
}

fn decode(pending: &mut Vec<u8>, chunk: &[u8]) -> String {
    let mut bytes = std::mem::take(pending);
    bytes.extend_from_slice(chunk);
    let mut out = String::new();
    let mut at = 0;
    loop {
        match std::str::from_utf8(&bytes[at..]) {
            Ok(text) => {
                out.push_str(text);
                break;
            }
            Err(e) => {
                let valid = at + e.valid_up_to();
                out.push_str(&String::from_utf8_lossy(&bytes[at..valid]));
                match e.error_len() {
                    Some(n) => {
                        out.push('\u{FFFD}');
                        at = valid + n;
                    }
                    None => {
                        pending.extend_from_slice(&bytes[valid..]);
                        break;
                    }
                }
            }
        }
    }
    out
}

#[derive(Default)]
struct Capture {
    bytes: [Vec<u8>; 2],
    pending: [Vec<u8>; 2],
    partial: [String; 2],
    total: usize,
    cut: bool,
}

impl Capture {
    fn take(&mut self, idx: usize, chunk: &[u8], lines: bool) -> Vec<String> {
        let chunk = if self.total + chunk.len() > OUTPUT_MAX {
            self.cut = true;
            &chunk[..OUTPUT_MAX - self.total]
        } else {
            chunk
        };
        self.total += chunk.len();
        if chunk.is_empty() {
            return Vec::new();
        }
        self.bytes[idx].extend_from_slice(chunk);
        if !lines {
            return Vec::new();
        }
        let decoded = decode(&mut self.pending[idx], chunk);
        let text = std::mem::take(&mut self.partial[idx]) + &decoded;
        let mut parts: Vec<String> = text.split('\n').map(str::to_string).collect();
        self.partial[idx] = parts.pop().unwrap_or_default();
        parts
    }
    fn finish(&mut self, idx: usize) -> Option<String> {
        let mut rest = std::mem::take(&mut self.partial[idx]);
        rest.push_str(&String::from_utf8_lossy(&std::mem::take(&mut self.pending[idx])));
        (!rest.is_empty()).then_some(rest)
    }
    fn text(&self, idx: usize) -> String {
        String::from_utf8_lossy(&self.bytes[idx]).into_owned()
    }
}

enum Mode {
    Road { on: Rc<dyn RoadEvents>, stream: bool },
    Reshape { timeout_s: f64, scrub: Rc<dyn Fn(&str) -> String> },
}

#[derive(Default)]
struct Life {
    cap: Capture,
    closed: [bool; 2],
    exit: Option<(Option<i64>, Option<String>)>,
    timed_out: bool,
    ended: bool,
    readers: Vec<AbortHandle>,
    deadline: Option<Cancel>,
    grace: Option<Cancel>,
    answer: Option<oneshot::Sender<ReshapeAnswer>>,
}

struct Run {
    clock: Rc<dyn Clock>,
    pid: Option<u32>,
    mode: Mode,
    life: RefCell<Life>,
}

impl Run {
    fn new(clock: Rc<dyn Clock>, pid: Option<u32>, mode: Mode) -> Run {
        Run { clock, pid, mode, life: RefCell::new(Life::default()) }
    }

    fn kill(&self) {
        if let Some(pid) = self.pid {
            kill_group(pid);
        }
    }

    fn streams(&self) -> bool {
        matches!(self.mode, Mode::Road { stream: true, .. })
    }

    fn chunk(&self, idx: usize, bytes: &[u8]) {
        let streams = self.streams();
        let lines = self.life.borrow_mut().cap.take(idx, bytes, streams);
        if let Mode::Road { on, .. } = &self.mode {
            for line in lines {
                on.line(STREAMS[idx], &line);
            }
        }
    }

    fn pipe_closed(&self, idx: usize) {
        self.life.borrow_mut().closed[idx] = true;
        self.settle_if_closed();
    }

    fn exited(self: &Rc<Self>, code: Option<i64>, signal: Option<String>) {
        self.life.borrow_mut().exit = Some((code, signal));
        if matches!(self.mode, Mode::Road { .. }) {
            self.kill();
            let weak = Rc::downgrade(self);
            let grace = self.clock.schedule(
                Box::new(move || {
                    if let Some(run) = weak.upgrade() {
                        run.grace();
                    }
                }),
                CLOSE_GRACE_MS,
            );
            self.life.borrow_mut().grace = Some(grace);
        }
        self.settle_if_closed();
    }

    fn deadline(&self) {
        self.life.borrow_mut().timed_out = true;
        self.kill();
    }

    fn grace(&self) {
        let readers = std::mem::take(&mut self.life.borrow_mut().readers);
        for reader in readers {
            reader.abort();
        }
        self.end();
    }

    fn settle_if_closed(&self) {
        let settled = {
            let life = self.life.borrow();
            !life.ended && life.exit.is_some() && life.closed == [true, true]
        };
        if settled {
            self.end();
        }
    }

    fn end(&self) {
        let (deadline, grace, rests, out, err, cut, timed_out, exit, answer) = {
            let mut life = self.life.borrow_mut();
            if life.ended {
                return;
            }
            life.ended = true;
            let rests = if self.streams() { [life.cap.finish(0), life.cap.finish(1)] } else { [None, None] };
            (
                life.deadline.take(),
                life.grace.take(),
                rests,
                life.cap.text(0),
                life.cap.text(1),
                life.cap.cut,
                life.timed_out,
                life.exit.clone(),
                life.answer.take(),
            )
        };
        if let Some(cancel) = deadline {
            cancel();
        }
        if let Some(cancel) = grace {
            cancel();
        }
        self.kill();
        let (code, signal) = exit.unwrap_or((None, None));
        match &self.mode {
            Mode::Road { on, .. } => {
                for (idx, rest) in rests.into_iter().enumerate() {
                    if let Some(line) = rest {
                        on.line(STREAMS[idx], &line);
                    }
                }
                on.end(RoadEnd { code, signal, error: None, out, err, cut, timed_out });
            }
            Mode::Reshape { timeout_s, scrub } => {
                let end = RoadEnd { code, signal: None, error: None, out, err, cut: false, timed_out };
                let answer_for = reshape_answer(&end, *timeout_s, scrub.as_ref());
                if let Some(tx) = answer {
                    let _ = tx.send(answer_for);
                }
            }
        }
    }
}

fn wire(run: &Rc<Run>, mut child: Child, input: Option<String>, timeout_s: f64) {
    if let (Some(text), Some(mut stdin)) = (input, child.stdin.take()) {
        spawn_local(async move {
            let _ = stdin.write_all(text.as_bytes()).await;
            let _ = stdin.shutdown().await;
        });
    }
    let mut readers = Vec::new();
    if let Some(out) = child.stdout.take() {
        readers.push(read_pipe(run, 0, out));
    }
    if let Some(err) = child.stderr.take() {
        readers.push(read_pipe(run, 1, err));
    }
    run.life.borrow_mut().readers = readers;
    let waiter = run.clone();
    spawn_local(async move {
        let (code, signal) = match child.wait().await {
            Ok(status) => (status.code().map(i64::from), status.signal().and_then(signal_name)),
            Err(_) => (None, None),
        };
        waiter.exited(code, signal);
    });
    let weak = Rc::downgrade(run);
    let deadline = run.clock.schedule(
        Box::new(move || {
            if let Some(run) = weak.upgrade() {
                run.deadline();
            }
        }),
        (timeout_s * 1_000.0) as i64,
    );
    run.life.borrow_mut().deadline = Some(deadline);
}

fn read_pipe<R: AsyncRead + Unpin + 'static>(run: &Rc<Run>, idx: usize, mut pipe: R) -> AbortHandle {
    let run = run.clone();
    spawn_local(async move {
        let mut buf = vec![0u8; READ_BYTES];
        loop {
            match pipe.read(&mut buf).await {
                Ok(n) if n > 0 => run.chunk(idx, &buf[..n]),
                _ => {
                    run.pipe_closed(idx);
                    break;
                }
            }
        }
    })
    .abort_handle()
}
