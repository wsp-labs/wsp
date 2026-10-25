// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/test/exec-detached.test.ts. Specs: spec/exec_put.md, spec/exec_run.md.
//!
//! The launch and poll contract over a scripted guest: a log that grows between polls, an exit file, a leader that can
//! die or be killed, and a machine that naps mid-run. The last block runs the real guest commands under this
//! machine's bash.
mod roads_common;

use std::collections::HashMap;
use std::fmt::Debug;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use futures::future::BoxFuture;
use futures::FutureExt;
use indexmap::IndexMap;
use roads_common::*;
use serde_json::json;
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;

use wsp_engine::errors::{Carried as CarriedFields, EngineError, ErrorKind, Result, WspError};
use wsp_engine::exec_detached::{
    exec_detached, exec_detached_in, put_files, GuestWrite, PutFilesOptions, CANCELLED_EXIT, DEADLINE_EXIT, INLINE_EXEC_MS,
};
use wsp_engine::golden_seam::EXEC_ENV;
use wsp_engine::machine::{ExecOptions, ExecResult, Machine, OnLine, RunOptions};
use wsp_wire::wire::limits::EXEC_BODY_MAX;

const BODY_MAX: usize = EXEC_BODY_MAX as usize;

#[derive(Clone, Default)]
struct Step {
    out: Option<String>,
    err: Option<String>,
    exit: Option<i64>,
    dead: bool,
    nap: bool,
    unusable: bool,
    gone: Option<EngineError>,
}

impl Step {
    fn new() -> Self {
        Self::default()
    }
    fn out(mut self, s: &str) -> Self {
        self.out = Some(s.to_string());
        self
    }
    fn err(mut self, s: &str) -> Self {
        self.err = Some(s.to_string());
        self
    }
    fn exit(mut self, code: i64) -> Self {
        self.exit = Some(code);
        self
    }
    /// The leader is gone.
    fn dead(mut self) -> Self {
        self.dead = true;
        self
    }
    /// The poll fails as it does while the machine is paused.
    fn nap(mut self) -> Self {
        self.nap = true;
        self
    }
    /// The backend reads the guest as dead: its shell will not start again for this poll or any other.
    fn unusable(mut self) -> Self {
        self.unusable = true;
        self
    }
    /// The provider no longer has the machine: this poll and every exec after it fail with this answer.
    fn gone(mut self, e: EngineError) -> Self {
        self.gone = Some(e);
        self
    }
}

fn take_quoted(s: &str) -> Option<(&str, &str)> {
    let at = s.find('\'')?;
    Some((&s[..at], &s[at + 1..]))
}

fn is_b64(s: &str) -> bool {
    s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'=')
}

fn all_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

enum Source<'a> {
    Inline(&'a str),
    Pieces { path: &'a str, last: usize },
}

/// `printf %s '<b64>'` or `cat '<path>'.{0..<n>}`, and what follows it.
fn parse_source(s: &str) -> Option<(Source<'_>, &str)> {
    if let Some(rest) = s.strip_prefix("printf %s '") {
        let (b64, rest) = take_quoted(rest)?;
        return is_b64(b64).then_some((Source::Inline(b64), rest));
    }
    let rest = s.strip_prefix("cat '")?;
    let (path, rest) = take_quoted(rest)?;
    let rest = rest.strip_prefix(".{0..")?;
    let end = rest.find('}')?;
    all_digits(&rest[..end]).then_some(())?;
    Some((Source::Pieces { path, last: rest[..end].parse().ok()? }, &rest[end + 1..]))
}

/// `printf %s '<b64>' > '<path>'.<i> || exit 1`.
fn parse_piece(line: &str) -> Option<(&str, &str, &str)> {
    let rest = line.strip_prefix("printf %s '")?;
    let (b64, rest) = take_quoted(rest)?;
    is_b64(b64).then_some(())?;
    let rest = rest.strip_prefix(" > '")?;
    let (path, rest) = take_quoted(rest)?;
    let rest = rest.strip_prefix('.')?;
    let digits = rest.find(' ').unwrap_or(rest.len());
    (all_digits(&rest[..digits]) && &rest[digits..] == " || exit 1").then_some((b64, path, &rest[..digits]))
}

/// `<source> | base64 -d > '<path>' || exit 1`.
fn parse_land(line: &str) -> Option<(Source<'_>, &str)> {
    let (src, rest) = parse_source(line)?;
    let rest = rest.strip_prefix(" | base64 -d > '")?;
    let (path, rest) = take_quoted(rest)?;
    (rest == " || exit 1").then_some((src, path))
}

/// `[ "$(cat '<mark>' 2>/dev/null)" = '<key>' ] || { <source> | base64 -d >> '<path>' && printf %s '<key>' > '<mark>'; } || exit 1`.
fn parse_append(line: &str) -> Option<(&str, &str, Source<'_>, &str)> {
    let rest = line.strip_prefix("[ \"$(cat '")?;
    let (mark, rest) = take_quoted(rest)?;
    let rest = rest.strip_prefix(" 2>/dev/null)\" = '")?;
    let (key, rest) = take_quoted(rest)?;
    let rest = rest.strip_prefix(" ] || { ")?;
    let (src, rest) = parse_source(rest)?;
    let rest = rest.strip_prefix(" | base64 -d >> '")?;
    let (path, rest) = take_quoted(rest)?;
    (rest == format!(" && printf %s '{key}' > '{mark}'; }} || exit 1")).then_some((mark, key, src, path))
}

/// The guest's disk as the upload lines write it: pieces by name, files by path, an append onto what is there once
/// per key the marker beside it holds, as the real shell would.
#[derive(Default)]
struct GuestDisk {
    files: IndexMap<String, String>,
    pieces: HashMap<String, String>,
    marks: IndexMap<String, String>,
}

impl GuestDisk {
    fn decode(&self, src: &Source) -> String {
        let joined = match src {
            Source::Inline(b64) => b64.to_string(),
            Source::Pieces { path, last } => {
                (0..=*last).map(|i| self.pieces.get(&format!("{path}.{i}")).cloned().unwrap_or_default()).collect()
            }
        };
        String::from_utf8_lossy(&STANDARD.decode(joined).expect("base64 the guest could not decode")).into_owned()
    }

    /// Applies the lines of one exec and answers how many numbered pieces it wrote.
    fn apply(&mut self, cmd: &str) -> usize {
        let mut pieces = 0;
        for line in cmd.split('\n') {
            if let Some((b64, path, i)) = parse_piece(line) {
                self.pieces.insert(format!("{path}.{i}"), b64.to_string());
                pieces += 1;
                continue;
            }
            if let Some((mark, key, src, path)) = parse_append(line) {
                if self.marks.get(mark).map(String::as_str) == Some(key) {
                    continue;
                }
                let text = self.decode(&src);
                self.files.entry(path.to_string()).or_default().push_str(&text);
                self.marks.insert(mark.to_string(), key.to_string());
                continue;
            }
            if let Some((src, path)) = parse_land(line) {
                let text = self.decode(&src);
                self.files.insert(path.to_string(), text);
            }
        }
        pieces
    }
}

fn transient_502() -> EngineError {
    EngineError::Provider(WspError {
        kind: ErrorKind::Transient,
        status: 502,
        code: None,
        message: "Bad Gateway".to_string(),
        request_id: None,
        window: None,
        said: None,
    })
}

fn unusable_guest() -> EngineError {
    EngineError::GuestUnusable {
        machine_id: "m1".to_string(),
        provider: "Boat".to_string(),
        detail: "Error 24".to_string(),
        status: 200,
        line: "m1 cannot start a shell: Boat said Error 24".to_string(),
    }
}

fn not_found(message: &str, request_id: Option<&str>) -> EngineError {
    EngineError::Provider(WspError {
        kind: ErrorKind::Missing,
        status: 404,
        code: None,
        message: message.to_string(),
        request_id: request_id.map(str::to_string),
        window: None,
        said: None,
    })
}

struct GuestState {
    steps: Vec<Step>,
    out: Vec<u8>,
    err: Vec<u8>,
    exit: String,
    alive: bool,
    step: usize,
    gone: Option<EngineError>,
    kills: Vec<String>,
    polls: Vec<(usize, usize)>,
    cleaned: usize,
    disk: GuestDisk,
}

/// The byte offset a poll reads `stream` from: the number after `tail -c +` ahead of that stream's file, less one.
fn offset_of(cmd: &str, stream: &str) -> usize {
    let tail = format!(" \"$b.{stream}\"");
    let at = cmd.find(&tail).unwrap_or_else(|| panic!("poll without a read of {stream}: {cmd}"));
    let head = &cmd[..at];
    let digits = head.bytes().rev().take_while(u8::is_ascii_digit).count();
    assert!(head[..head.len() - digits].ends_with("tail -c +"), "poll read of {stream} without tail -c +N: {cmd}");
    head[head.len() - digits..].parse::<usize>().unwrap() - 1
}

fn chunk(buf: &[u8], at: usize) -> String {
    let start = at.min(buf.len());
    STANDARD.encode(&buf[start..(at + 262_144).min(buf.len())])
}

/// A guest that answers the transport: steps apply one per poll, before the poll reads.
struct Guest {
    machine: Arc<ScriptedMachine>,
    state: Arc<Mutex<GuestState>>,
}

fn guest(steps: Vec<Step>) -> Guest {
    let state = Arc::new(Mutex::new(GuestState {
        steps,
        out: Vec::new(),
        err: Vec::new(),
        exit: String::new(),
        alive: true,
        step: 0,
        gone: None,
        kills: Vec::new(),
        polls: Vec::new(),
        cleaned: 0,
        disk: GuestDisk::default(),
    }));
    let st = state.clone();
    let machine = ScriptedMachine::new("m1", move |cmd: &str, _o: &ExecOptions| -> Result<ExecResult> {
        let mut s = st.lock().unwrap();
        if let Some(e) = &s.gone {
            return Err(e.clone());
        }
        if cmd.contains("echo WSP_PIECE") {
            if s.disk.apply(cmd) != 1 {
                return Err(plain(&format!("piece exec without a numbered file: {cmd}")));
            }
            return Ok(res(0, "WSP_PIECE\n", ""));
        }
        if cmd.contains("echo WSP_LAUNCHED") {
            s.disk.apply(cmd);
            return Ok(res(0, "WSP_LAUNCHED\n", ""));
        }
        if cmd.contains("kill -TERM") {
            s.kills.push(cmd.to_string());
            s.alive = false;
            return Ok(res(0, "", ""));
        }
        if cmd.starts_with("rm -") {
            s.cleaned += 1;
            return Ok(res(0, "", ""));
        }
        if cmd.contains("echo WSP_POLL") {
            if let Some(step) = s.steps.get(s.step).cloned() {
                s.step += 1;
                if step.nap {
                    return Err(transient_502());
                }
                if step.unusable {
                    return Err(unusable_guest());
                }
                if let Some(gone) = step.gone {
                    s.gone = Some(gone.clone());
                    return Err(gone);
                }
                if let Some(out) = step.out {
                    s.out.extend_from_slice(out.as_bytes());
                }
                if let Some(err) = step.err {
                    s.err.extend_from_slice(err.as_bytes());
                }
                if let Some(exit) = step.exit {
                    s.exit = exit.to_string();
                }
                if step.dead {
                    s.alive = false;
                }
            }
            let from = (offset_of(cmd, "out"), offset_of(cmd, "err"));
            s.polls.push(from);
            let body = format!(
                "WSP_POLL\n{}\n{}\n{}\n{}\nWSP_POLL_END\n",
                s.exit,
                chunk(&s.out, from.0),
                chunk(&s.err, from.1),
                if s.alive { "up" } else { "down" }
            );
            return Ok(res(0, &body, ""));
        }
        Err(plain(&format!("guest got an unexpected command: {cmd}")))
    });
    Guest { machine, state }
}

impl Guest {
    fn calls(&self) -> Vec<ExecCall> {
        self.machine.calls.lock().unwrap().clone()
    }
    fn kills(&self) -> Vec<String> {
        self.state.lock().unwrap().kills.clone()
    }
    fn polls(&self) -> Vec<(usize, usize)> {
        self.state.lock().unwrap().polls.clone()
    }
    fn cleaned(&self) -> usize {
        self.state.lock().unwrap().cleaned
    }
    fn script(&self) -> Option<String> {
        self.state.lock().unwrap().disk.files.iter().find(|(path, _)| path.ends_with(".sh")).map(|(_, text)| text.clone())
    }
    fn dynamic(&self) -> Arc<dyn Machine> {
        self.machine.clone()
    }
}

/// The bytes the provider counts: its request with the backend's wrapper around the command.
fn solari_body(cmd: &str) -> usize {
    serde_json::to_string(&json!({ "cmd": "bash", "args": ["-c", format!("{EXEC_ENV}\n{cmd}")], "timeoutMs": INLINE_EXEC_MS }))
        .unwrap()
        .len()
}

fn is_poll(c: &ExecCall) -> bool {
    c.cmd.contains("echo WSP_POLL")
}

/// Every exec before the first poll: the launch, with the pieces ahead of it when the script needs them.
fn launch_calls(calls: &[ExecCall]) -> Vec<ExecCall> {
    let first_poll = calls.iter().position(is_poll).expect("no poll was sent");
    calls[..first_poll].to_vec()
}

/// Forty kilobytes of lines with quotes and bytes outside ASCII, so a piece boundary that broke the encoding would show.
fn big_script() -> String {
    (0..480).map(|i| format!("echo 'line {i} it'\\''s ünïcödé {}'", "y".repeat(50))).collect::<Vec<_>>().join("\n")
}

fn big_input() -> String {
    (0..200).map(|i| format!("prompt line {i} with a 'quote' and ünïcödé {}", "z".repeat(60))).collect::<Vec<_>>().join("\n")
}

fn run_opts(deadline_ms: u64, poll_ms: u64) -> RunOptions {
    RunOptions { deadline_ms, poll_ms: Some(poll_ms), ..Default::default() }
}

fn collecting() -> (OnLine, Arc<Mutex<Vec<String>>>) {
    let lines = Arc::new(Mutex::new(Vec::new()));
    let log = lines.clone();
    (Arc::new(move |l: &str| log.lock().unwrap().push(l.to_string())), lines)
}

fn texts(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

async fn failure<T: Debug>(fut: impl std::future::Future<Output = Result<T>>) -> EngineError {
    fut.await.expect_err("expected the call to fail")
}

fn hex12(s: &str) -> bool {
    s.len() == 12 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The twelve hex characters after the first `/tmp/wsp-run/` in a command: the run's own random name.
fn run_id_in(cmd: &str) -> String {
    let at = cmd.find("/tmp/wsp-run/").expect("no run path in the command") + "/tmp/wsp-run/".len();
    let id = &cmd[at..at + 12];
    assert!(hex12(id), "not a run id: {id}");
    id.to_string()
}

/// The key an append in `cmd` holds its marker to: twelve hex characters, a dot and the file's place in the upload.
fn append_key(cmd: &str, mark: &str) -> String {
    let head = format!("[ \"$(cat '{mark}' 2>/dev/null)\" = '");
    let at = cmd.find(&head).expect("no append of this marker in the command") + head.len();
    let key = cmd[at..].split('\'').next().unwrap();
    assert!(key.len() == 14 && hex12(&key[..12]) && &key[12..] == ".0", "not an upload key: {key}");
    key.to_string()
}

/// A wrapper machine named `id` whose exec is `f`, which is handed the machine it wraps.
fn delegating<F>(id: &str, inner: Arc<dyn Machine>, f: F) -> Arc<ScriptedMachine>
where
    F: Fn(Arc<dyn Machine>, String, ExecOptions) -> BoxFuture<'static, Result<ExecResult>> + Send + Sync + 'static,
{
    let f = Arc::new(f);
    ScriptedMachine::with_future(id, Arc::new(move |cmd: &str, o: &ExecOptions| f(inner.clone(), cmd.to_string(), o.clone())))
}

/// The machine with every exec that carries `mark` run twice, as the backend does when the first answer is lost.
fn twice(machine: Arc<dyn Machine>, mark: &'static str) -> Arc<ScriptedMachine> {
    let id = machine.id().to_string();
    delegating(&id, machine, move |inner, cmd, o| {
        async move {
            let first = inner.exec(&cmd, o.clone()).await?;
            if cmd.contains(mark) {
                inner.exec(&cmd, o).await
            } else {
                Ok(first)
            }
        }
        .boxed()
    })
}

fn write(path: &str, text: &str) -> GuestWrite {
    GuestWrite { path: path.to_string(), text: text.to_string(), append: false }
}

fn append(path: &str, text: &str) -> GuestWrite {
    GuestWrite { path: path.to_string(), text: text.to_string(), append: true }
}

const KILL_TAIL: &str = "; p=$(cat \"$b.pid\" 2>/dev/null); if [ -n \"$p\" ]; then kill -TERM -- \"-$p\" \"$p\" 2>/dev/null; sleep 2; kill -KILL -- \"-$p\" \"$p\" 2>/dev/null; fi; true";

#[tokio::test]
async fn launches_the_script_detached_reads_both_streams_from_where_the_last_poll_stopped() {
    let g = guest(vec![Step::new().out("one\ntw"), Step::new().out("o\nthree\n").err("warn\n"), Step::new().exit(3)]);
    let (on_line, lines) = collecting();
    let res = exec_detached(&*g.machine, "brew install gh", RunOptions { on_line: Some(on_line), ..run_opts(10_000, 1) }).await.unwrap();
    assert_eq!(res, ExecResult { exit_code: 3, stdout: "one\ntwo\nthree\n".to_string(), stderr: "warn\n".to_string() });
    assert_eq!(*lines.lock().unwrap(), texts(&["one", "two", "three", "warn"]));
    assert_eq!(g.script().as_deref(), Some("brew install gh"));
    assert_eq!(g.polls(), vec![(0, 0), (6, 0), (14, 5)]);
    assert_eq!(g.cleaned(), 1);
    assert_eq!(g.kills(), Vec::<String>::new());
}

#[tokio::test]
async fn every_exec_it_sends_is_bounded_at_the_inline_cap() {
    let g = guest(vec![Step::new().out("x\n").exit(0)]);
    exec_detached(&*g.machine, "true", run_opts(10_000, 1)).await.unwrap();
    let calls = g.calls();
    assert!(calls.len() >= 3, "{} execs", calls.len());
    for c in &calls {
        let timeout = c.opts.timeout_ms.unwrap_or_else(|| panic!("no timeout on {}", c.cmd));
        assert!(timeout <= INLINE_EXEC_MS, "{timeout} on {}", c.cmd);
    }
}

#[tokio::test]
async fn a_full_chunk_is_followed_at_once_by_the_next_read() {
    let big = format!("{}tail\n", "x".repeat(262_144));
    let g = guest(vec![Step::new().out(&big).exit(0)]);
    let res = exec_detached(&*g.machine, "true", run_opts(10_000, 1)).await.unwrap();
    assert_eq!(res.stdout, big);
    assert_eq!(g.polls().iter().map(|p| p.0).collect::<Vec<_>>(), vec![0, 262_144]);
}

#[tokio::test]
async fn the_exit_file_ends_the_run_only_once_nothing_is_left_to_read() {
    let a = "a".repeat(262_144);
    let g = guest(vec![Step::new().exit(0).out(&a), Step::new().out("b\n")]);
    let res = exec_detached(&*g.machine, "true", run_opts(10_000, 1)).await.unwrap();
    assert_eq!(res.stdout, format!("{a}b\n"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn past_the_deadline_it_kills_the_recorded_pid_with_its_group() {
    let g = guest(vec![Step::new().out("started\n")]);
    let t0 = Instant::now();
    let res = exec_detached(&*g.machine, "sleep 999", run_opts(40, 1)).await.unwrap();
    assert!(t0.elapsed() < Duration::from_millis(2_000), "{:?}", t0.elapsed());
    assert_eq!(res.exit_code, DEADLINE_EXIT);
    assert_eq!(res.stdout, "started\n");
    let kills = g.kills();
    assert_eq!(kills.len(), 1);
    let rest = kills[0].strip_prefix("b=/tmp/wsp-run/").unwrap_or_else(|| panic!("kill without the run's base: {}", kills[0]));
    assert!(hex12(&rest[..12]), "{}", kills[0]);
    assert_eq!(&rest[12..], KILL_TAIL);
    assert_eq!(g.cleaned(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_cancel_kills_the_recorded_group_cleans_up_and_answers_at_once() {
    let g = guest(vec![Step::new().out("started\n"), Step::new(), Step::new(), Step::new(), Step::new(), Step::new()]);
    let cancel = CancellationToken::new();
    let on_line_cancel = cancel.clone();
    let t0 = Instant::now();
    let res = exec_detached(
        &*g.machine,
        "sleep 999",
        RunOptions {
            signal: Some(cancel.clone()),
            on_line: Some(Arc::new(move |_l: &str| on_line_cancel.cancel())),
            ..run_opts(60_000, 200)
        },
    )
    .await
    .unwrap();
    assert!(t0.elapsed() < Duration::from_millis(2_000), "{:?}", t0.elapsed());
    assert_eq!(res.exit_code, CANCELLED_EXIT);
    assert_eq!(res.stdout, "started\n");
    assert_eq!(g.kills().len(), 1);
    assert_eq!(g.cleaned(), 1);

    let before = guest(vec![Step::new().exit(0)]);
    let gone = CancellationToken::new();
    gone.cancel();
    let res = exec_detached(&*before.machine, "true", RunOptions { signal: Some(gone), ..run_opts(10_000, 1) }).await.unwrap();
    assert_eq!(res.exit_code, CANCELLED_EXIT);
    assert!(before.calls().is_empty(), "{:?}", before.machine.cmds());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_cancel_during_the_pause_between_polls_cuts_the_pause_short() {
    // A poll that fails while the machine naps is retried a whole poll_ms later, the longest pause the loop takes.
    let g = guest(vec![Step::new().nap(), Step::new().nap()]);
    let cancel = CancellationToken::new();
    let later = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        later.cancel();
    });
    let t0 = Instant::now();
    let res = exec_detached(&*g.machine, "sleep 999", RunOptions { signal: Some(cancel), ..run_opts(600_000, 60_000) }).await.unwrap();
    assert!(t0.elapsed() < Duration::from_millis(1_000), "{:?}", t0.elapsed());
    assert_eq!(res.exit_code, CANCELLED_EXIT);
    assert_eq!(g.kills().len(), 1);
}

#[tokio::test]
async fn a_poll_that_fails_while_the_machine_naps_is_retried() {
    let g = guest(vec![Step::new().out("before\n"), Step::new().nap(), Step::new().nap(), Step::new().out("after\n").exit(0)]);
    let res = exec_detached(&*g.machine, "true", run_opts(10_000, 1)).await.unwrap();
    assert_eq!(res, ExecResult { exit_code: 0, stdout: "before\nafter\n".to_string(), stderr: String::new() });
    assert_eq!(g.calls().iter().filter(|c| is_poll(c)).count(), 4);
}

#[tokio::test]
async fn a_poll_that_meets_a_guest_the_backend_calls_unusable() {
    let g = guest(vec![Step::new().out("started\n"), Step::new().unusable()]);
    let e = failure(exec_detached(&*g.machine, "sleep 999", run_opts(1_000, 1))).await;
    assert!(matches!(e, EngineError::GuestUnusable { .. }), "{e:?}");
    // Nothing is sent to a guest that cannot run anything: no kill, no cleanup, and no waiting out the deadline.
    assert_eq!(g.kills(), Vec::<String>::new());
    assert_eq!(g.cleaned(), 0);
}

#[tokio::test]
async fn a_poll_that_meets_the_providers_404() {
    let missing = not_found("Sandbox not found", Some("req-1"));
    let g = guest(vec![Step::new().out("started\n"), Step::new().gone(missing.clone())]);
    let t0 = Instant::now();
    let e = failure(exec_detached(&*g.machine, "sleep 999", run_opts(1_000, 1))).await;
    assert_eq!(e, missing);
    assert!(t0.elapsed() < Duration::from_millis(1_000), "{:?}", t0.elapsed());
    // The poll that met the 404 is the last exec sent: no kill, no cleanup and no further poll.
    let calls = g.calls();
    let polls: Vec<usize> = calls.iter().enumerate().filter(|(_, c)| is_poll(c)).map(|(at, _)| at).collect();
    assert_eq!(polls.len(), 2);
    assert_eq!(calls.len(), polls[1] + 1);
}

#[tokio::test]
async fn a_poll_that_meets_a_machine_the_provider_stopped() {
    let stopped = EngineError::MachineStopped {
        machine_id: "m1".to_string(),
        message: "Sandbox machine is not running.".to_string(),
        carried: CarriedFields {
            kind: Some(ErrorKind::Conflict),
            status: Some(409),
            code: Some("machine_not_running".to_string()),
            request_id: None,
        },
    };
    let g = guest(vec![Step::new().out("started\n"), Step::new().gone(stopped.clone())]);
    let e = failure(exec_detached(&*g.machine, "sleep 999", run_opts(1_000, 1))).await;
    assert_eq!(e, stopped);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_nap_that_outlasts_the_deadline_still_ends_in_124() {
    let g = guest((0..200).map(|_| Step::new().nap()).collect());
    let res = exec_detached(&*g.machine, "true", run_opts(30, 1)).await.unwrap();
    assert_eq!(res.exit_code, DEADLINE_EXIT);
}

#[tokio::test]
async fn a_leader_gone_without_an_exit_file_answers_minus_one() {
    let g = guest(vec![Step::new().out("partial"), Step::new().dead()]);
    let (on_line, lines) = collecting();
    let res = exec_detached(&*g.machine, "true", RunOptions { on_line: Some(on_line), ..run_opts(10_000, 1) }).await.unwrap();
    assert_eq!(res.exit_code, -1);
    assert_eq!(res.stdout, "partial");
    assert!(res.stderr.contains("ended without reporting an exit code"), "{:?}", res.stderr);
    assert_eq!(*lines.lock().unwrap(), texts(&["partial"]));
}

#[tokio::test]
async fn the_launch_sets_the_umask_before_the_script_is_written() {
    let g = guest(vec![Step::new().exit(0)]);
    exec_detached(&*g.machine, "true", run_opts(10_000, 1)).await.unwrap();
    let calls = launch_calls(&g.calls());
    let launch: Vec<&str> = calls.last().unwrap().cmd.split('\n').collect();
    let umask = launch.iter().position(|l| *l == "umask 077").expect("the launch never sets umask 077");
    let landing = launch.iter().position(|l| l.contains("base64 -d >")).expect("the launch never lands the script");
    assert!(umask < landing, "umask at line {umask}, the script lands at line {landing}");
}

#[tokio::test]
async fn a_script_that_fits_one_exec_body_goes_up_in_one_launch_exec() {
    let g = guest(vec![Step::new().out("x\n").exit(0)]);
    exec_detached(&*g.machine, "brew install gh", run_opts(10_000, 1)).await.unwrap();
    let launch = launch_calls(&g.calls());
    assert_eq!(launch.len(), 1);
    assert!(launch[0].cmd.contains("printf %s 'YnJldyBpbnN0YWxsIGdo' | base64 -d"), "{}", launch[0].cmd);
    assert!(solari_body(&launch[0].cmd) <= BODY_MAX);
    assert_eq!(g.script().as_deref(), Some("brew install gh"));
}

#[tokio::test]
async fn a_40_kb_script_goes_up_in_pieces() {
    let big = big_script();
    assert!(big.len() > 40_000);
    let g = guest(vec![Step::new().out("x\n").exit(0)]);
    let res = exec_detached(&*g.machine, &big, run_opts(10_000, 1)).await.unwrap();
    assert_eq!(res.exit_code, 0);
    assert_eq!(g.script(), Some(big));
    let calls = g.calls();
    let launch = launch_calls(&calls);
    assert_eq!(launch.iter().filter(|c| c.cmd.contains("echo WSP_PIECE")).count(), 4);
    let last = &launch.last().unwrap().cmd;
    let id = run_id_in(last);
    let joined = format!(
        "cat '/tmp/wsp-run/{id}.sh'.{{0..3}} | base64 -d > '/tmp/wsp-run/{id}.sh' || exit 1\nrm -f '/tmp/wsp-run/{id}.sh'.{{0..3}}\n"
    );
    assert!(last.contains(&joined), "{last}");
    for c in &calls {
        assert!(solari_body(&c.cmd) <= BODY_MAX, "{}", &c.cmd[..80]);
    }
    // The pieces are cut as large as the cap allows, so the count is the smallest that fits.
    for c in &launch[..3] {
        assert!(solari_body(&c.cmd) > BODY_MAX - 1024);
    }
}

#[tokio::test]
async fn a_piece_posted_twice_lands_once() {
    let big = big_script();
    let g = guest(vec![Step::new().out("x\n").exit(0)]);
    let retrying = twice(g.dynamic(), "echo WSP_PIECE");
    exec_detached(&*retrying, &big, run_opts(10_000, 1)).await.unwrap();
    assert_eq!(g.calls().iter().filter(|c| c.cmd.contains("echo WSP_PIECE")).count(), 8);
    assert_eq!(g.script(), Some(big));
}

#[tokio::test]
async fn a_piece_that_does_not_confirm_fails_the_run_before_the_launch() {
    let machine = ScriptedMachine::new("m9", |_cmd, _o| Ok(res(1, "", "bash: printf: write error: No space left on device")));
    let e = failure(exec_detached(&*machine, &big_script(), run_opts(1_000, 1))).await;
    assert_eq!(
        e.to_string(),
        "a piece did not land on m9: nothing came back saying WSP_PIECE, the word the guest prints once a piece is written; it exited 1 and said: bash: printf: write error: No space left on device"
    );
}

#[tokio::test]
async fn a_launch_the_machine_answered_nothing_to_names_the_handshake() {
    // The exec landed and the guest printed neither the handshake nor a reason. Exit 0 reads as success on every
    // other road, so the code on its own would say the run started.
    let machine = ScriptedMachine::new("m9", |_cmd, _o| Ok(res(0, "", "")));
    let e = failure(exec_detached(&*machine, "true", run_opts(1_000, 1))).await;
    assert_eq!(
        e.to_string(),
        "launch failed on m9: nothing came back saying WSP_LAUNCHED, the word the guest prints once the run is up; it printed nothing and exited 0"
    );
}

#[tokio::test]
async fn a_launch_that_meets_the_providers_404_fails_with_that_answer() {
    let missing = not_found("Sandbox not found", None);
    let thrown = missing.clone();
    let machine = ScriptedMachine::new("m9", move |_cmd, _o| Err(thrown.clone()));
    let e = failure(exec_detached(&*machine, "true", run_opts(1_000, 1))).await;
    assert_eq!(e, missing);
    let calls = machine.cmds();
    assert_eq!(calls.len(), 2);
    assert!(calls[0].contains("echo WSP_LAUNCHED"), "{}", calls[0]);
    assert!(calls[1].starts_with("rm -rf "), "{}", calls[1]);
}

#[tokio::test]
async fn a_launch_that_does_not_confirm_fails_the_run() {
    let machine = ScriptedMachine::new("m9", |_cmd, _o| Ok(res(1, "", "bash: base64: not found")));
    let e = failure(exec_detached(&*machine, "true", run_opts(1_000, 1))).await;
    assert_eq!(
        e.to_string(),
        "launch failed on m9: nothing came back saying WSP_LAUNCHED, the word the guest prints once the run is up; it exited 1 and said: bash: base64: not found"
    );
}

/// A guest that only has a disk: every exec lands what it carries and answers the marker the caller asked for.
struct DiskGuest {
    machine: Arc<ScriptedMachine>,
    disk: Arc<Mutex<GuestDisk>>,
}

fn disk_guest(seed: &[(&str, &str)]) -> DiskGuest {
    let disk = Arc::new(Mutex::new(GuestDisk::default()));
    for (path, text) in seed {
        disk.lock().unwrap().files.insert(path.to_string(), text.to_string());
    }
    let d = disk.clone();
    let machine = ScriptedMachine::new("m2", move |cmd: &str, _o: &ExecOptions| -> Result<ExecResult> {
        d.lock().unwrap().apply(cmd);
        let last = cmd.split('\n').next_back().unwrap();
        Ok(res(0, &format!("{}\n", last.strip_prefix("echo ").unwrap_or(last)), ""))
    });
    DiskGuest { machine, disk }
}

impl DiskGuest {
    fn calls(&self) -> Vec<String> {
        self.machine.cmds()
    }
    fn keys(&self) -> Vec<Option<String>> {
        self.machine.calls.lock().unwrap().iter().map(|c| c.opts.idempotency_key.clone()).collect()
    }
    fn file(&self, path: &str) -> Option<String> {
        self.disk.lock().unwrap().files.get(path).cloned()
    }
    fn marks(&self) -> Vec<String> {
        self.disk.lock().unwrap().marks.keys().cloned().collect()
    }
    fn dynamic(&self) -> Arc<dyn Machine> {
        self.machine.clone()
    }
}

#[tokio::test]
async fn an_input_file_over_the_cap_goes_up_in_pieces_beside_a_script_that_fits() {
    let input = big_input();
    assert!(input.len() > 20_000);
    let g = disk_guest(&[]);
    let res = put_files(
        &*g.machine,
        &[write("/tmp/wsp-run/t1.sh", "claude -p"), write("/tmp/wsp-run/t1.in", &input)],
        PutFilesOptions {
            before: texts(&["mkdir /tmp/wsp-run/t1.d || exit 0"]),
            after: texts(&["mkfifo /tmp/wsp-run/t1.fifo", "echo WSP_LAUNCHED"]),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(res.stdout, "WSP_LAUNCHED\n");
    let calls = g.calls();
    assert_eq!(calls.iter().filter(|c| c.ends_with("echo WSP_PIECE")).count(), 2);
    for c in &calls {
        assert!(solari_body(c) <= BODY_MAX, "{}", &c[..80]);
    }
    assert_eq!(g.file("/tmp/wsp-run/t1.sh").as_deref(), Some("claude -p"));
    assert_eq!(g.file("/tmp/wsp-run/t1.in"), Some(input));
    let last: Vec<&str> = calls.last().unwrap().split('\n').collect();
    assert_eq!(last[..3], ["mkdir -p '/tmp/wsp-run'", "mkdir /tmp/wsp-run/t1.d || exit 0", "set -o pipefail"]);
    assert_eq!(last[3], "printf %s 'Y2xhdWRlIC1w' | base64 -d > '/tmp/wsp-run/t1.sh' || exit 1");
    assert_eq!(
        last[4..],
        [
            "cat '/tmp/wsp-run/t1.in'.{0..1} | base64 -d > '/tmp/wsp-run/t1.in' || exit 1",
            "rm -f '/tmp/wsp-run/t1.in'.{0..1}",
            "mkfifo /tmp/wsp-run/t1.fifo",
            "echo WSP_LAUNCHED"
        ]
    );
}

#[tokio::test]
async fn names_every_exec_of_one_upload() {
    let g = disk_guest(&[]);
    put_files(
        &*g.machine,
        &[write("/tmp/wsp-run/k1.sh", "echo hi"), write("/tmp/wsp-run/k1.in", &big_input())],
        PutFilesOptions { after: texts(&["echo WSP_LAUNCHED"]), ..Default::default() },
    )
    .await
    .unwrap();
    let keys = g.keys();
    assert!(keys.iter().all(Option::is_some), "{keys:?}");
    let distinct: std::collections::HashSet<_> = keys.iter().collect();
    assert_eq!(distinct.len(), keys.len());
    // One upload, one name, so a road that asks the same exec again knows it is the same ask.
    let uploads: std::collections::HashSet<_> = keys.iter().map(|k| k.as_ref().unwrap().split('/').next().unwrap().to_string()).collect();
    assert_eq!(uploads.len(), 1);
}

#[tokio::test]
async fn an_append_over_the_cap_goes_up_in_pieces() {
    let input = big_input();
    let g = disk_guest(&[("/tmp/wsp-run/t2.in", "first\n")]);
    put_files(&*g.machine, &[append("/tmp/wsp-run/t2.in", &format!("{input}\n"))], PutFilesOptions::default()).await.unwrap();
    let calls = g.calls();
    assert_eq!(calls.iter().filter(|c| c.ends_with("echo WSP_PIECE")).count(), 2);
    for c in &calls {
        assert!(solari_body(c) <= BODY_MAX);
    }
    let last = calls.last().unwrap();
    let key = append_key(last, "/tmp/wsp-run/t2.in.appended");
    let tail = format!(
        "\n[ \"$(cat '/tmp/wsp-run/t2.in.appended' 2>/dev/null)\" = '{key}' ] || {{ cat '/tmp/wsp-run/t2.in'.{{0..1}} | base64 -d >> '/tmp/wsp-run/t2.in' && printf %s '{key}' > '/tmp/wsp-run/t2.in.appended'; }} || exit 1\nrm -f '/tmp/wsp-run/t2.in'.{{0..1}}"
    );
    assert!(last.ends_with(&tail), "{last}");
    assert_eq!(g.file("/tmp/wsp-run/t2.in"), Some(format!("first\n{input}\n")));
}

#[tokio::test]
async fn an_append_that_fits_goes_up_in_one_exec() {
    let g = disk_guest(&[("/tmp/wsp-run/t3.in", "first\n")]);
    put_files(&*g.machine, &[append("/tmp/wsp-run/t3.in", "second\n")], PutFilesOptions::default()).await.unwrap();
    let calls = g.calls();
    assert_eq!(calls.len(), 1);
    let key = append_key(&calls[0], "/tmp/wsp-run/t3.in.appended");
    assert_eq!(
        calls[0],
        format!(
            "mkdir -p '/tmp/wsp-run'\nset -o pipefail\n[ \"$(cat '/tmp/wsp-run/t3.in.appended' 2>/dev/null)\" = '{key}' ] || {{ printf %s 'c2Vjb25kCg==' | base64 -d >> '/tmp/wsp-run/t3.in' && printf %s '{key}' > '/tmp/wsp-run/t3.in.appended'; }} || exit 1"
        )
    );
    assert_eq!(g.file("/tmp/wsp-run/t3.in").as_deref(), Some("first\nsecond\n"));
}

#[tokio::test]
async fn an_append_exec_run_twice_lands_the_line_once() {
    let g = disk_guest(&[("/tmp/wsp-run/t5.in", "first\n")]);
    let res = put_files(&*twice(g.dynamic(), "base64 -d >>"), &[append("/tmp/wsp-run/t5.in", "steer\n")], PutFilesOptions::default())
        .await
        .unwrap();
    assert_eq!(res.exit_code, 0);
    let calls = g.calls();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0], calls[1]);
    assert_eq!(g.file("/tmp/wsp-run/t5.in").as_deref(), Some("first\nsteer\n"));
}

#[tokio::test]
async fn a_piece_form_append_whose_join_runs_twice_lands_one_copy() {
    let input = big_input();
    let g = disk_guest(&[("/tmp/wsp-run/t6.in", "first\n")]);
    let res =
        put_files(&*twice(g.dynamic(), "base64 -d >>"), &[append("/tmp/wsp-run/t6.in", &format!("{input}\n"))], PutFilesOptions::default())
            .await
            .unwrap();
    assert_eq!(res.exit_code, 0);
    assert_eq!(g.calls().iter().filter(|c| c.contains("base64 -d >>")).count(), 2);
    assert_eq!(g.file("/tmp/wsp-run/t6.in"), Some(format!("first\n{input}\n")));
    assert_eq!(g.marks(), vec!["/tmp/wsp-run/t6.in.appended".to_string()]);
}

#[tokio::test]
async fn two_appends_carry_two_keys_in_one_marker() {
    let g = disk_guest(&[("/tmp/wsp-run/t7.in", "first\n")]);
    put_files(&*g.machine, &[append("/tmp/wsp-run/t7.in", "second\n")], PutFilesOptions::default()).await.unwrap();
    put_files(&*g.machine, &[append("/tmp/wsp-run/t7.in", "third\n")], PutFilesOptions::default()).await.unwrap();
    assert_eq!(g.marks(), vec!["/tmp/wsp-run/t7.in.appended".to_string()]);
    assert_eq!(g.file("/tmp/wsp-run/t7.in").as_deref(), Some("first\nsecond\nthird\n"));
}

#[tokio::test]
async fn two_appends_to_one_path_in_one_call_each_land() {
    let g = disk_guest(&[("/tmp/wsp-run/t8.in", "first\n")]);
    put_files(
        &*g.machine,
        &[append("/tmp/wsp-run/t8.in", "second\n"), append("/tmp/wsp-run/t8.in", "third\n")],
        PutFilesOptions::default(),
    )
    .await
    .unwrap();
    assert_eq!(g.file("/tmp/wsp-run/t8.in").as_deref(), Some("first\nsecond\nthird\n"));
}

#[tokio::test]
async fn answers_the_pieces_the_upload_took() {
    let g = disk_guest(&[]);
    let big = put_files(&*g.machine, &[write("/tmp/wsp-run/t9.in", &big_input())], PutFilesOptions::default()).await.unwrap();
    assert_eq!(big.pieces as usize, g.calls().iter().filter(|c| c.ends_with("echo WSP_PIECE")).count());
    assert!(big.pieces > 1, "{}", big.pieces);
    // Nothing was cut up: the one exec that lands the file is not a piece.
    let small = put_files(&*g.machine, &[write("/tmp/wsp-run/t10.in", "one line\n")], PutFilesOptions::default()).await.unwrap();
    assert_eq!(small.pieces, 0);
}

#[tokio::test]
async fn a_piece_that_does_not_confirm_fails_before_the_last_exec_goes() {
    let machine = ScriptedMachine::new("m9", |_cmd, _o| Ok(res(0, "", "")));
    let e = failure(put_files(&*machine, &[write("/tmp/wsp-run/t4.in", &big_input())], PutFilesOptions::default())).await;
    assert_eq!(
        e.to_string(),
        "a piece did not land on m9: nothing came back saying WSP_PIECE, the word the guest prints once a piece is written; it printed nothing and exited 0"
    );
    assert_eq!(machine.cmds().len(), 1);
}

/// A file's permission bits as three octal digits, read by this process rather than by a stat(1) whose flags differ
/// between the BSD and GNU builds.
fn mode_of(path: impl AsRef<Path>) -> String {
    format!("{:o}", fs::metadata(path).unwrap().permissions().mode() & 0o777)
}

/// The modes of a run's own files, taken the first time the script writes a line. A run removes its files when it
/// ends, so after exec_detached answers there is nothing left on disk to read.
#[derive(Clone)]
struct FileModes {
    dir: String,
    seen: Arc<Mutex<HashMap<String, String>>>,
}

impl FileModes {
    fn new(run_dir: &str) -> Self {
        Self { dir: run_dir.to_string(), seen: Arc::new(Mutex::new(HashMap::new())) }
    }
    fn read(&self) {
        for entry in fs::read_dir(&self.dir).expect("the run folder") {
            let entry = entry.unwrap();
            let ext = Path::new(&entry.file_name()).extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
            let mut seen = self.seen.lock().unwrap();
            if !seen.contains_key(&ext) {
                if let Ok(meta) = entry.metadata() {
                    seen.insert(ext, format!("{:o}", meta.permissions().mode() & 0o777));
                }
            }
        }
    }
    fn of(&self, suffix: &str) -> Option<String> {
        self.seen.lock().unwrap().get(suffix).cloned()
    }
    fn on_line(&self) -> OnLine {
        let this = self.clone();
        Arc::new(move |_l: &str| this.read())
    }
}

/// This machine's bash as the guest; setsid is perl's setpgrp where the OS has none and base64 loses -w0. The shims
/// are written only where /proc/1/stat is missing, so on Linux the guest is plain bash. `default_mask` is the umask
/// every exec starts under: this box's own is 077, and the two cases that compare against a shell's default mask need
/// the usual 022 for the comparison to say anything.
struct Local {
    machine: Arc<ScriptedMachine>,
    run_dir: String,
    _dir: TempDir,
}

fn local_guest(default_mask: Option<u32>) -> Local {
    let dir = tempfile::Builder::new().prefix("wsp-detached-").tempdir().unwrap();
    let shim_dir = dir.path().join("shims");
    fs::create_dir(&shim_dir).unwrap();
    if !Path::new("/proc/1/stat").exists() {
        let shims = [
            ("setsid", "#!/bin/sh\nexec perl -e 'setpgrp(0, 0); exec @ARGV or die $!' -- \"$@\"\n"),
            (
                "base64",
                "#!/bin/sh\nargs=\"\"\nfor a in \"$@\"; do [ \"$a\" = \"-w0\" ] || args=\"$args $a\"; done\nexec /usr/bin/base64 $args\n",
            ),
        ];
        for (name, body) in shims {
            let path = shim_dir.join(name);
            fs::write(&path, body).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
    let path = format!("{}:{}", shim_dir.display(), std::env::var("PATH").unwrap_or_default());
    let machine = ScriptedMachine::with_future(
        "local",
        Arc::new(move |cmd: &str, _o: &ExecOptions| {
            let cmd = match default_mask {
                Some(mask) => format!("umask {mask:03o}\n{cmd}"),
                None => cmd.to_string(),
            };
            let path = path.clone();
            async move {
                let out = tokio::process::Command::new("bash")
                    .arg("-c")
                    .arg(&cmd)
                    .env("PATH", path)
                    .output()
                    .await
                    .map_err(|e| plain(&e.to_string()))?;
                Ok(res(out.status.code().unwrap_or(1) as i64, &String::from_utf8_lossy(&out.stdout), &String::from_utf8_lossy(&out.stderr)))
            }
            .boxed()
        }),
    );
    let run_dir = dir.path().join("run").to_string_lossy().into_owned();
    Local { machine, run_dir, _dir: dir }
}

impl Local {
    async fn ls(&self) -> String {
        self.machine.exec(&format!("ls {}", self.run_dir), ExecOptions::default()).await.unwrap().stdout
    }
    fn names(&self) -> Vec<String> {
        let mut names: Vec<String> =
            fs::read_dir(&self.run_dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn runs_the_real_launch_poll_and_cleanup_commands() {
    let local = local_guest(None);
    let (on_line, lines) = collecting();
    let res = exec_detached_in(
        &*local.machine,
        "echo one; sleep 0.3; echo two; echo warn >&2; exit 3",
        RunOptions { on_line: Some(on_line), ..run_opts(20_000, 50) },
        &local.run_dir,
    )
    .await
    .unwrap();
    assert_eq!(res, ExecResult { exit_code: 3, stdout: "one\ntwo\n".to_string(), stderr: "warn\n".to_string() });
    // A poll reads stdout then stderr, so where warn lands among the lines is timing; each stream keeps its order.
    let lines = lines.lock().unwrap().clone();
    assert_eq!(lines.iter().filter(|l| *l != "warn").cloned().collect::<Vec<_>>(), texts(&["one", "two"]));
    assert!(lines.contains(&"warn".to_string()), "{lines:?}");
    assert_eq!(local.ls().await, "");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_script_and_the_streams_of_a_run_are_readable_by_nobody_but_their_owner() {
    let local = local_guest(Some(0o022));
    let seen = FileModes::new(&local.run_dir);
    let res = exec_detached_in(
        &*local.machine,
        "echo running",
        RunOptions { on_line: Some(seen.on_line()), ..run_opts(20_000, 50) },
        &local.run_dir,
    )
    .await
    .unwrap();
    assert_eq!(res, ExecResult { exit_code: 0, stdout: "running\n".to_string(), stderr: String::new() });
    assert_eq!(
        [seen.of(".sh"), seen.of(".out"), seen.of(".err")],
        [Some("600".to_string()), Some("600".to_string()), Some("600".to_string())]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_detached_script_creates_files_as_its_own_shell_would() {
    let local = local_guest(Some(0o022));
    let seen = FileModes::new(&local.run_dir);
    // The script only makes things; every mode is read off the disk by this process, which says the same on any OS.
    let script = ["d=$(dirname \"$0\")", "touch \"$d/made\"", "mkdir \"$d/dir\"", "echo running"].join("\n");
    let res =
        exec_detached_in(&*local.machine, &script, RunOptions { on_line: Some(seen.on_line()), ..run_opts(20_000, 50) }, &local.run_dir)
            .await
            .unwrap();
    assert_eq!(res.exit_code, 0);
    let outside = local.machine.exec(&format!("cd {} && touch ref && mkdir refd", local.run_dir), ExecOptions::default()).await.unwrap();
    assert_eq!(outside.exit_code, 0);
    // What the script makes is what a command outside the run makes: the road lends it no mask of its own.
    let at = |name: &str| mode_of(Path::new(&local.run_dir).join(name));
    assert_eq!([at("made"), at("dir")], [at("ref"), at("refd")]);
    // The run's own script and streams are still nobody else's.
    assert_eq!(
        [seen.of(".sh"), seen.of(".out"), seen.of(".err")],
        [Some("600".to_string()), Some("600".to_string()), Some("600".to_string())]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_launch_that_lands_its_script_and_then_fails_leaves_nothing() {
    let local = local_guest(None);
    let sent = Arc::new(Mutex::new(Vec::<String>::new()));
    let log = sent.clone();
    let run_dir = local.run_dir.clone();
    let watched = delegating("local", local.machine.clone(), move |inner, cmd, o| {
        let log = log.clone();
        let run_dir = run_dir.clone();
        async move {
            log.lock().unwrap().push(cmd.clone());
            if !cmd.contains("echo WSP_LAUNCHED") {
                return inner.exec(&cmd, o).await;
            }
            // The script lands and the launch then fails before it confirms, the way a lost answer does: nothing starts,
            // and the file holding the caller's text is on the guest with nobody coming back for it.
            let lines: Vec<&str> = cmd.split('\n').collect();
            let cut = lines.iter().position(|l| l.starts_with("setsid")).expect("no setsid line in the launch");
            let landed = inner.exec(&lines[..cut].join("\n"), o).await?;
            let scripts =
                fs::read_dir(&run_dir).unwrap().filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().ends_with(".sh")).count();
            assert_eq!(scripts, 1);
            Ok(ExecResult { stdout: String::new(), ..landed })
        }
        .boxed()
    });
    let e = failure(exec_detached_in(&*watched, "true", run_opts(1_000, 1), &local.run_dir)).await;
    assert!(e.to_string().contains("launch failed"), "{e}");
    assert!(sent.lock().unwrap().iter().any(|c| c.starts_with("rm -rf")));
    assert_eq!(local.names(), Vec::<String>::new());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_launch_posted_twice_under_one_base_starts_the_script_once() {
    let local = local_guest(None);
    let marks = format!("{}/../marks", local.run_dir);
    let retrying = twice(local.machine.clone(), "echo WSP_LAUNCHED");
    let res = exec_detached_in(&*retrying, &format!("echo ran >> {marks}; echo hi"), run_opts(20_000, 50), &local.run_dir).await.unwrap();
    assert_eq!(res, ExecResult { exit_code: 0, stdout: "hi\n".to_string(), stderr: String::new() });
    assert_eq!(fs::read_to_string(&marks).unwrap(), "ran\n");
    assert_eq!(local.ls().await, "");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_script_over_the_cap_runs_for_real_from_its_pieces() {
    let local = local_guest(None);
    let retrying = twice(local.machine.clone(), "echo WSP_PIECE");
    let res = exec_detached_in(&*retrying, &format!("{}\necho tail", big_script()), run_opts(20_000, 50), &local.run_dir).await.unwrap();
    assert_eq!(res.exit_code, 0);
    assert_eq!(res.stderr, "");
    let lines: Vec<&str> = res.stdout.split('\n').collect();
    assert_eq!(lines[lines.len() - 3..], [format!("line 479 it's ünïcödé {}", "y".repeat(50)).as_str(), "tail", ""]);
    assert_eq!(lines.len(), 482);
    assert_eq!(local.ls().await, "");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_files_one_over_the_cap_and_appended_land_byte_for_byte() {
    let local = local_guest(None);
    let run = &local.run_dir;
    let input = big_input();
    let first = put_files(
        &*local.machine,
        &[write(&format!("{run}/f.sh"), "echo hi\n"), write(&format!("{run}/f.in"), "first\n")],
        PutFilesOptions { after: texts(&["echo WSP_LAUNCHED"]), ..Default::default() },
    )
    .await
    .unwrap();
    assert_eq!(first.stdout, "WSP_LAUNCHED\n");
    let second =
        put_files(&*local.machine, &[append(&format!("{run}/f.in"), &format!("{input}\n"))], PutFilesOptions::default()).await.unwrap();
    assert_eq!(second.exit_code, 0);
    assert_eq!(fs::read_to_string(format!("{run}/f.sh")).unwrap(), "echo hi\n");
    assert_eq!(fs::read_to_string(format!("{run}/f.in")).unwrap(), format!("first\n{input}\n"));
    assert_eq!(local.names(), texts(&["f.in", "f.in.appended", "f.sh"]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_appends_to_one_path_leave_one_marker() {
    let local = local_guest(None);
    let run = &local.run_dir;
    put_files(&*local.machine, &[write(&format!("{run}/h.in"), "first\n")], PutFilesOptions::default()).await.unwrap();
    // The job's log on a computer is appended to in batches all through a run, one upload each: a marker per
    // append fills that folder with a file per batch and nothing ever sweeps them.
    let second = put_files(
        &*twice(local.machine.clone(), "base64 -d >>"),
        &[append(&format!("{run}/h.in"), "second\n")],
        PutFilesOptions::default(),
    )
    .await
    .unwrap();
    assert_eq!(second.exit_code, 0);
    put_files(&*local.machine, &[append(&format!("{run}/h.in"), "third\n")], PutFilesOptions::default()).await.unwrap();
    assert_eq!(fs::read_to_string(format!("{run}/h.in")).unwrap(), "first\nsecond\nthird\n");
    assert_eq!(local.names(), texts(&["h.in", "h.in.appended"]));
    let marker = fs::read_to_string(format!("{run}/h.in.appended")).unwrap();
    assert!(marker.len() == 14 && hex12(&marker[..12]) && &marker[12..] == ".0", "{marker:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_append_exec_and_a_piece_form_join_each_run_twice() {
    let local = local_guest(None);
    let run = &local.run_dir;
    let input = big_input();
    put_files(&*local.machine, &[write(&format!("{run}/g.in"), "first\n")], PutFilesOptions::default()).await.unwrap();
    let retrying = twice(local.machine.clone(), "base64 -d >>");
    let small = put_files(&*retrying, &[append(&format!("{run}/g.in"), "steer\n")], PutFilesOptions::default()).await.unwrap();
    assert_eq!(small.exit_code, 0);
    assert_eq!(fs::read_to_string(format!("{run}/g.in")).unwrap(), "first\nsteer\n");
    let big = put_files(&*retrying, &[append(&format!("{run}/g.in"), &format!("{input}\n"))], PutFilesOptions::default()).await.unwrap();
    assert_eq!(big.exit_code, 0);
    assert_eq!(fs::read_to_string(format!("{run}/g.in")).unwrap(), format!("first\nsteer\n{input}\n"));
    assert_eq!(local.names(), texts(&["g.in", "g.in.appended"]));
}

/// The launch has put a byte on the run's stdout file.
fn started(run_dir: &str) -> bool {
    Path::new(run_dir).exists()
        && fs::read_dir(run_dir)
            .map(|dir| {
                dir.flatten()
                    .any(|e| e.file_name().to_string_lossy().ends_with(".out") && e.metadata().map(|m| m.len() > 0).unwrap_or(false))
            })
            .unwrap_or(false)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_deadline_kill_ends_the_launched_session_for_real() {
    tokio::time::timeout(Duration::from_secs(15), async {
        let local = local_guest(None);
        // The deadline counts from before the launch, so under load the kill can land before the script's first line: the launch answers once it is on disk.
        let run_dir = local.run_dir.clone();
        let settled = delegating("local", local.machine.clone(), move |inner, cmd, o| {
            let run_dir = run_dir.clone();
            async move {
                let res = inner.exec(&cmd, o).await?;
                if cmd.contains("echo WSP_LAUNCHED") {
                    while !started(&run_dir) {
                        tokio::time::sleep(Duration::from_millis(20)).await;
                    }
                }
                Ok(res)
            }
            .boxed()
        });
        let res = exec_detached_in(&*settled, "echo pid $$; sleep 30", run_opts(800, 50), &local.run_dir).await.unwrap();
        assert_eq!(res.exit_code, DEADLINE_EXIT);
        let digits: String = res.stdout.split("pid ").nth(1).unwrap_or("").chars().take_while(char::is_ascii_digit).collect();
        let pid: u32 = digits.parse().unwrap_or(0);
        assert!(pid > 0, "no pid in {:?}", res.stdout);
        tokio::time::sleep(Duration::from_millis(200)).await;
        let probe =
            local.machine.exec(&format!("kill -0 {pid} 2>/dev/null && echo alive || echo gone"), ExecOptions::default()).await.unwrap();
        assert_eq!(probe.stdout, "gone\n");
        assert_eq!(local.ls().await, "");
    })
    .await
    .expect("the test took longer than 15 s");
}
