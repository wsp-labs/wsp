// SPDX-License-Identifier: AGPL-3.0-only
//! A fake `codex app-server` process for the codex adapter's tests: the Rust form of `wire`, `server`, `launcher`,
//! `collect` and `until` in packages/adapter-codex/test/adapter.test.ts:28-180 at a3b870a6e. A test file pulls it in
//! with `#[path = "common/codex_wire.rs"] mod codex_wire;`. Test code only.
#![allow(dead_code)]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::future::{LocalBoxFuture, FutureExt};
use futures::stream::{LocalBoxStream, StreamExt};
use serde_json::{Map, Value};
use tokio::sync::{watch, Notify};
use wsp_adapters::port::{AdapterEvent, AttachRunOptions, Attached, Env, ExecFailure, ExecFailureKind, ExecOptions, ExecStream, ExecStreamFactory, OnEvent, SharedFuture, WriteOutcome};

pub const RUN_HANDLE: &str = "/tmp/wsp-run/cd34";

/// A fixture file's non-blank lines, under tests/fixtures/<dir>/<name>.
pub fn fixture_lines(dir: &str, name: &str) -> Vec<String> {
    fixture_text(dir, name).lines().filter(|l| !l.trim().is_empty()).map(String::from).collect()
}

/// A fixture file whole.
pub fn fixture_text(dir: &str, name: &str) -> String {
    let path = format!("{}/tests/fixtures/{}/{}", env!("CARGO_MANIFEST_DIR"), dir, name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

pub fn parse(line: &str) -> Value {
    serde_json::from_str(line).unwrap_or_else(|e| panic!("not JSON ({e}): {line}"))
}

pub fn obj(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(m) => m,
        other => panic!("not an object: {other}"),
    }
}

/// What a write's fault does: fail before the line lands (`lost`) or after it landed (`landed`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    Lost,
    Landed,
}

pub type OnWrite = Arc<dyn Fn(&Value, &Wire)>;
pub type WriteFault = Arc<dyn Fn(&Value) -> Option<(Fault, ExecFailure)>>;

#[derive(Clone, Default)]
pub struct WireOpts {
    pub exit_code: Option<i32>,
    /// The process does not exit on EOF or teardown, only on kill or by hand.
    pub hang: bool,
    pub on_write: Option<OnWrite>,
    pub fault: Option<WriteFault>,
}

struct WireState {
    queue: VecDeque<Result<String, ExecFailure>>,
    done: bool,
    written: Vec<Value>,
    order: Vec<String>,
    closed: bool,
    taken: bool,
}

/// One app-server process under test: the lines it prints, the lines wsp wrote to it (the seed first), and how it ended.
#[derive(Clone)]
pub struct Wire {
    state: Arc<Mutex<WireState>>,
    wake: Arc<Notify>,
    exit: Arc<watch::Sender<Option<Option<i32>>>>,
    opts: WireOpts,
    run: Option<String>,
}

pub fn wire(opts: WireOpts) -> Wire {
    let (tx, _rx) = watch::channel(None);
    Wire {
        state: Arc::new(Mutex::new(WireState { queue: VecDeque::new(), done: false, written: Vec::new(), order: Vec::new(), closed: false, taken: false })),
        wake: Arc::new(Notify::new()),
        exit: Arc::new(tx),
        opts,
        run: Some(RUN_HANDLE.to_string()),
    }
}

impl Wire {
    /// The server prints these lines.
    pub fn push<S: AsRef<str>>(&self, lines: &[S]) {
        let mut s = self.state.lock().unwrap();
        for line in lines {
            s.queue.push_back(Ok(line.as_ref().to_string()));
        }
        drop(s);
        self.wake.notify_one();
    }
    pub fn push1(&self, line: impl AsRef<str>) {
        self.push(&[line.as_ref()]);
    }
    /// The stream throws this after what it printed.
    pub fn fail(&self, failure: ExecFailure) {
        self.state.lock().unwrap().queue.push_back(Err(failure));
        self.wake.notify_one();
    }
    /// The process ends with `code`; the first exit wins.
    pub fn exit(&self, code: Option<i32>) {
        {
            let mut s = self.state.lock().unwrap();
            if s.done {
                return;
            }
            s.done = true;
        }
        self.wake.notify_one();
        self.exit.send_if_modified(|v| {
            if v.is_none() {
                *v = Some(code);
                true
            } else {
                false
            }
        });
    }
    pub fn written(&self) -> Vec<Value> {
        self.state.lock().unwrap().written.clone()
    }
    /// The written messages with this method.
    pub fn written_method(&self, method: &str) -> Vec<Value> {
        self.written().into_iter().filter(|m| m.get("method").and_then(Value::as_str) == Some(method)).collect()
    }
    pub fn wrote(&self, method: &str) -> bool {
        !self.written_method(method).is_empty()
    }
    pub fn order(&self) -> Vec<String> {
        self.state.lock().unwrap().order.clone()
    }
    pub fn closed(&self) -> bool {
        self.state.lock().unwrap().closed
    }
    pub fn exited_code(&self) -> Option<Option<i32>> {
        *self.exit.borrow()
    }
    /// This wire as the stream an adapter reads.
    pub fn stream(&self) -> Arc<dyn ExecStream> {
        Arc::new(self.clone())
    }
    /// The seed lines, written as the TS fake writes them: each through `write`, so each reaches `on_write`.
    pub fn seed(&self, lines: &[String]) {
        for line in lines {
            drop(self.write(line.clone()));
        }
    }
}

impl ExecStream for Wire {
    fn lines(&self) -> LocalBoxStream<'static, Result<String, ExecFailure>> {
        {
            let mut s = self.state.lock().unwrap();
            if s.taken {
                return futures::stream::empty().boxed_local();
            }
            s.taken = true;
        }
        let state = self.state.clone();
        let wake = self.wake.clone();
        futures::stream::unfold((state, wake, false), |(state, wake, ended)| async move {
            if ended {
                return None;
            }
            loop {
                let waker = wake.clone();
                let notified = waker.notified();
                {
                    let mut s = state.lock().unwrap();
                    if let Some(next) = s.queue.pop_front() {
                        let failed = next.is_err();
                        drop(s);
                        return Some((next, (state, wake, failed)));
                    }
                    if s.done {
                        return None;
                    }
                }
                notified.await;
            }
        })
        .boxed_local()
    }
    fn run(&self) -> Option<String> {
        self.run.clone()
    }
    fn teardown(&self) {
        self.state.lock().unwrap().order.push("teardown".into());
        if !self.opts.hang {
            self.exit(Some(self.opts.exit_code.unwrap_or(0)));
        }
    }
    fn kill(&self) {
        self.state.lock().unwrap().order.push("kill".into());
        self.exit(None);
    }
    fn write(&self, line: String) -> LocalBoxFuture<'static, Result<WriteOutcome, ExecFailure>> {
        if self.state.lock().unwrap().done {
            return async { Ok(WriteOutcome::Gone) }.boxed_local();
        }
        let message = parse(&line);
        let fault = self.opts.fault.as_ref().and_then(|f| f(&message));
        if let Some((Fault::Lost, failure)) = &fault {
            let failure = failure.clone();
            return async move { Err(failure) }.boxed_local();
        }
        self.state.lock().unwrap().written.push(message.clone());
        if let Some(on_write) = &self.opts.on_write {
            on_write(&message, self);
        }
        match fault {
            Some((_, failure)) => async move { Err(failure) }.boxed_local(),
            None => async { Ok(WriteOutcome::Written) }.boxed_local(),
        }
    }
    fn close_input(&self) {
        self.state.lock().unwrap().closed = true;
        if !self.opts.hang {
            let me = self.clone();
            let code = self.opts.exit_code.unwrap_or(0);
            tokio::task::spawn_local(async move {
                tokio::task::yield_now().await;
                me.exit(Some(code));
            });
        }
    }
    fn exited(&self) -> SharedFuture<Option<i32>> {
        let mut rx = self.exit.subscribe();
        async move {
            loop {
                if let Some(code) = *rx.borrow_and_update() {
                    return code;
                }
                if rx.changed().await.is_err() {
                    return None;
                }
            }
        }
        .boxed_local()
        .shared()
    }
}

/// A recorded server: everything up to its answer to the thread request once that request arrives, the rest once a
/// turn starts. TS adapter.test.ts:107.
pub fn server(lines: Vec<String>, opts: WireOpts) -> impl Fn(&[String]) -> Wire + 'static {
    let split = lines.iter().position(|l| l.contains(r#""id":"wsp-thread""#)).map(|i| i + 1).unwrap_or(0);
    let head: Vec<String> = lines[..split].to_vec();
    let tail: Vec<String> = lines[split..].to_vec();
    move |seed: &[String]| {
        let (head, tail) = (head.clone(), tail.clone());
        let on_write: OnWrite = Arc::new(move |m: &Value, w: &Wire| {
            let method = m.get("method").and_then(Value::as_str).unwrap_or("");
            if method == "thread/start" || method == "thread/resume" || method == "thread/fork" {
                w.push(&head);
            }
            if method == "turn/start" {
                w.push(&tail);
            }
        });
        let w = wire(WireOpts { on_write: Some(on_write), ..opts.clone() });
        w.seed(seed);
        w
    }
}

/// A server scripted by hand: `on_write` answers what wsp writes; the seed goes through it.
pub fn scripted_server(opts: WireOpts) -> impl Fn(&[String]) -> Wire + 'static {
    move |seed: &[String]| {
        let w = wire(opts.clone());
        w.seed(seed);
        w
    }
}

#[derive(Clone, Debug)]
pub struct Call {
    pub command: String,
    pub env: Env,
    pub input: Option<Vec<String>>,
    pub secret_input: bool,
}

type Make = Arc<dyn Fn(&[String]) -> Wire>;
type AttachFn = Arc<dyn Fn(&str, AttachRunOptions) -> LocalBoxFuture<'static, Result<Attached, ExecFailure>>>;

/// The exec factory under test: every launch recorded, each process a wire.
#[derive(Clone)]
pub struct Launch {
    make: Make,
    pub calls: Arc<Mutex<Vec<Call>>>,
    pub wires: Arc<Mutex<Vec<Wire>>>,
    pub attach_with: Arc<Mutex<Option<AttachFn>>>,
}

pub fn launcher(make: impl Fn(&[String]) -> Wire + 'static) -> Launch {
    Launch { make: Arc::new(make), calls: Arc::new(Mutex::new(Vec::new())), wires: Arc::new(Mutex::new(Vec::new())), attach_with: Arc::new(Mutex::new(None)) }
}

impl Launch {
    pub fn factory(&self) -> Arc<dyn ExecStreamFactory> {
        Arc::new(self.clone())
    }
    pub fn calls(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }
    pub fn call(&self, i: usize) -> Call {
        self.calls()[i].clone()
    }
    pub fn wire(&self, i: usize) -> Wire {
        self.wires.lock().unwrap()[i].clone()
    }
    pub fn wire_count(&self) -> usize {
        self.wires.lock().unwrap().len()
    }
    pub fn set_attach(&self, f: impl Fn(&str, AttachRunOptions) -> LocalBoxFuture<'static, Result<Attached, ExecFailure>> + 'static) {
        *self.attach_with.lock().unwrap() = Some(Arc::new(f));
    }
}

impl ExecStreamFactory for Launch {
    fn launch(&self, command: &str, options: ExecOptions) -> Arc<dyn ExecStream> {
        let input = options.input.clone();
        self.calls.lock().unwrap().push(Call { command: command.to_string(), env: options.env.clone(), input: input.clone(), secret_input: options.secret.as_ref().map(|s| s.input).unwrap_or(false) });
        let w = (self.make)(&input.unwrap_or_default());
        self.wires.lock().unwrap().push(w.clone());
        w.stream()
    }
    fn attach(&self, run: &str, options: AttachRunOptions) -> Option<LocalBoxFuture<'static, Result<Attached, ExecFailure>>> {
        let f = self.attach_with.lock().unwrap().clone()?;
        Some(f(run, options))
    }
}

/// Events as an adapter emits them, in order.
#[derive(Clone, Default)]
pub struct Events(pub Arc<Mutex<Vec<AdapterEvent>>>);

impl Events {
    pub fn on_event(&self) -> OnEvent {
        let me = self.0.clone();
        Arc::new(move |e| me.lock().unwrap().push(e))
    }
    pub fn all(&self) -> Vec<AdapterEvent> {
        self.0.lock().unwrap().clone()
    }
    /// Each event's type as the TS names it, a delta as `delta:<kind>`.
    pub fn kinds(&self) -> Vec<String> {
        self.all().iter().map(kind_of).collect()
    }
    pub fn any(&self, kind: &str) -> bool {
        self.kinds().iter().any(|k| k == kind)
    }
    pub fn of(&self, kind: &str) -> Vec<AdapterEvent> {
        self.all().into_iter().filter(|e| kind_of(e) == kind || type_of(e) == kind).collect()
    }
    pub fn deltas(&self) -> Vec<wsp_adapters::port::TurnDelta> {
        self.all()
            .into_iter()
            .filter_map(|e| match e {
                AdapterEvent::TurnDelta(d) => Some(d),
                _ => None,
            })
            .collect()
    }
}

pub fn no_events() -> OnEvent {
    Arc::new(|_| {})
}

/// The TS event type: `session.start`, `turn.delta`, ...
pub fn type_of(e: &AdapterEvent) -> String {
    let s = match e {
        AdapterEvent::SessionStart { .. } => "session.start",
        AdapterEvent::TurnDelta(_) => "turn.delta",
        AdapterEvent::TurnDone { .. } => "turn.done",
        AdapterEvent::TurnCompacted { .. } => "turn.compacted",
        AdapterEvent::TurnRetry { .. } => "turn.retry",
        AdapterEvent::TurnPlan { .. } => "turn.plan",
        AdapterEvent::TurnTasks { .. } => "turn.tasks",
        AdapterEvent::TurnUnread { .. } => "turn.unread",
        AdapterEvent::TurnAnchor { .. } => "turn.anchor",
        AdapterEvent::Limit { .. } => "limit",
        AdapterEvent::TurnUsage { .. } => "turn.usage",
        AdapterEvent::Subagent { .. } => "subagent",
        AdapterEvent::PermissionAsk { .. } => "permission.ask",
        AdapterEvent::PermissionClose { .. } => "permission.close",
        AdapterEvent::SessionEnd { .. } => "session.end",
    };
    s.to_string()
}

/// `delta:<kind>` for a delta, else the type.
pub fn kind_of(e: &AdapterEvent) -> String {
    match e {
        AdapterEvent::TurnDelta(d) => format!("delta:{}", serde_json::to_value(d.kind).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default()),
        other => type_of(other),
    }
}

/// Waits up to 200 ms, a millisecond at a time, for `check`; fails the test where it never holds. TS adapter.test.ts:172.
pub async fn until(check: impl Fn() -> bool) {
    for _ in 0..200 {
        if check() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    assert!(check(), "the condition never held");
}

pub async fn sleep_ms(ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

/// True when `fut` is still pending after `ms`. TS adapter.test.ts:773 (`pending`).
pub async fn pending_after<T>(fut: impl std::future::Future<Output = T> + Unpin, ms: u64) -> bool {
    tokio::time::timeout(Duration::from_millis(ms), fut).await.is_err()
}

pub fn failure(message: &str) -> ExecFailure {
    ExecFailure { kind: ExecFailureKind::Other, message: message.to_string() }
}
