// SPDX-License-Identifier: AGPL-3.0-only
//! Fake exec roads for the Claude adapter's tests: the Rust form of `scriptedExec`, `manualExec`, `collect` and
//! `until` in packages/adapter-claude/test/adapter.test.ts:28-150 at a3b870a6e. A test file pulls it in with
//! `#[path = "common/claude_exec.rs"] mod claude_exec;`. Test code only: nothing here is the crate's.
#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use futures::future::{BoxFuture, FutureExt};
use futures::stream::{self, BoxStream, StreamExt};
use tokio::sync::{mpsc, watch};
use wsp_adapters::port::{AdapterEvent, Env, ExecFailure, ExecOptions, ExecSecret, ExecStream, ExecStreamFactory, OnEvent, Settle, SharedFuture, WriteOutcome};

/// What the guest calls the run every scripted stream stands for.
pub const RUN_HANDLE: &str = "/tmp/wsp-run/ab12";

/// The fixture session id of stream-session.jsonl.
pub const FIXTURE_SESSION_ID: &str = "e16ed170-8257-4668-879e-fe836341633c";

/// A fixture file's non-blank lines.
pub fn fixture_lines(name: &str) -> Vec<String> {
    let path = format!("{}/tests/fixtures/claude/{}", env!("CARGO_MANIFEST_DIR"), name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}")).lines().filter(|l| !l.trim().is_empty()).map(String::from).collect()
}

/// A guest's login environment as the runtime hands it.
pub fn guest_env() -> wsp_adapters::claude::landmines::BaseEnv {
    [("HOME", "/root"), ("CLAUDE_CONFIG_DIR", "/root/.claude-cfg"), ("IS_SANDBOX", "1")].into_iter().map(|(k, v)| (k.to_string(), Some(v.to_string()))).collect()
}

/// A process's exit, settled once.
#[derive(Clone)]
pub struct Exit {
    tx: Arc<watch::Sender<Option<Option<i32>>>>,
}

impl Exit {
    pub fn new() -> Self {
        let (tx, _rx) = watch::channel(None);
        Exit { tx: Arc::new(tx) }
    }
    /// The first call wins, as a promise's resolve does.
    pub fn resolve(&self, code: Option<i32>) {
        self.tx.send_if_modified(|v| {
            if v.is_none() {
                *v = Some(code);
                true
            } else {
                false
            }
        });
    }
    pub fn settled(&self) -> Option<Option<i32>> {
        *self.tx.borrow()
    }
    pub fn future(&self) -> SharedFuture<Option<i32>> {
        let mut rx = self.tx.subscribe();
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
        .boxed()
        .shared()
    }
}

/// One launch the factory was asked for.
#[derive(Clone, Debug)]
pub struct Call {
    pub command: String,
    pub env: Env,
    pub input: Option<Vec<String>>,
    /// The launch named `input_after`.
    pub input_after: bool,
    pub secret: Option<ExecSecret>,
}

/// How a fake stream answers its calls.
#[derive(Clone, Default)]
pub struct StreamOpts {
    /// The code a scripted stream ends with once its lines are read: `Some(None)` is a null exit. 0 where `None`.
    pub exit_code: Option<Option<i32>>,
    /// A scripted stream ends only on kill (or teardown, unless `slow_teardown`).
    pub hang: bool,
    pub signalled: Option<String>,
    /// Runs inside write() before the line lands.
    pub before_write: Option<Arc<dyn Fn() -> BoxFuture<'static, ()> + Send + Sync>>,
    /// The guest reports the process over: write answers Gone and no line lands.
    pub gone: bool,
    /// Teardown leaves the process up, as a CLI held by background tasks is.
    pub slow_teardown: bool,
    /// What `taken()` answers, as an attached run's channel read back.
    pub taken: Option<Vec<String>>,
    pub run: Option<String>,
}

/// A fake ExecStream. `order` notes "teardown", "kill", "write" and "closeInput" in the order they came; `writes`
/// holds every line that landed.
pub struct FakeStream {
    lines: Mutex<Option<BoxStream<'static, Result<String, ExecFailure>>>>,
    pub exit: Exit,
    exited: SharedFuture<Option<i32>>,
    pub opts: StreamOpts,
    pub order: Arc<Mutex<Vec<String>>>,
    pub writes: Arc<Mutex<Vec<String>>>,
    feed: Mutex<Option<mpsc::UnboundedSender<Result<String, ExecFailure>>>>,
    manual: bool,
}

impl FakeStream {
    /// Replays `lines`, then ends with `opts.exit_code` (or, with `hang`, waits for an exit).
    pub fn scripted(lines: Vec<String>, opts: StreamOpts, order: Arc<Mutex<Vec<String>>>, writes: Arc<Mutex<Vec<String>>>) -> Arc<Self> {
        let exit = Exit::new();
        let (hang, code) = (opts.hang, opts.exit_code.unwrap_or(Some(0)));
        let tail_exit = exit.clone();
        let tail = stream::once(async move {
            if hang {
                tail_exit.future().await;
            } else {
                tail_exit.resolve(code);
            }
            None
        })
        .filter_map(|x: Option<Result<String, ExecFailure>>| async move { x });
        let body = stream::iter(lines.into_iter().map(Ok)).chain(tail).boxed();
        let exited = exit.future();
        Arc::new(FakeStream { lines: Mutex::new(Some(body)), exit, exited, opts, order, writes, feed: Mutex::new(None), manual: false })
    }

    /// Lines the test pushes by hand; `end` closes the stream and settles the exit.
    pub fn manual(opts: StreamOpts, order: Arc<Mutex<Vec<String>>>, writes: Arc<Mutex<Vec<String>>>) -> Arc<Self> {
        let exit = Exit::new();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let body = stream::poll_fn(move |cx| rx.poll_recv(cx)).boxed();
        let exited = exit.future();
        Arc::new(FakeStream { lines: Mutex::new(Some(body)), exit, exited, opts, order, writes, feed: Mutex::new(Some(tx)), manual: true })
    }

    /// One stdout line of a manual stream.
    pub fn push(&self, line: impl Into<String>) {
        if let Some(tx) = self.feed.lock().unwrap().as_ref() {
            let _ = tx.send(Ok(line.into()));
        }
    }

    /// The transport throws `failure` mid-stream, ending it.
    pub fn fail(&self, failure: ExecFailure) {
        if let Some(tx) = self.feed.lock().unwrap().take() {
            let _ = tx.send(Err(failure));
        }
    }

    /// Ends a manual stream's lines and settles its exit with `code`.
    pub fn end(&self, code: Option<i32>) {
        self.feed.lock().unwrap().take();
        self.exit.resolve(code);
    }

    fn note(&self, what: &str) {
        self.order.lock().unwrap().push(what.to_string());
    }
}

impl ExecStream for FakeStream {
    fn lines(&self) -> BoxStream<'static, Result<String, ExecFailure>> {
        self.lines.lock().unwrap().take().unwrap_or_else(|| stream::empty().boxed())
    }
    fn run(&self) -> Option<String> {
        self.opts.run.clone()
    }
    /// Scripted: ends with its code unless it hangs. Manual: ends with 143 unless `slow_teardown`.
    fn teardown(&self) {
        self.note("teardown");
        if self.manual {
            if !self.opts.slow_teardown {
                self.end(Some(143));
            }
        } else if !self.opts.hang {
            self.exit.resolve(self.opts.exit_code.unwrap_or(Some(0)));
        }
    }
    fn kill(&self) {
        self.note("kill");
        self.feed.lock().unwrap().take();
        self.exit.resolve(None);
    }
    fn write(&self, line: String) -> BoxFuture<'static, Result<WriteOutcome, ExecFailure>> {
        self.note("write");
        let before = self.opts.before_write.clone();
        let gone = self.opts.gone;
        let writes = self.writes.clone();
        async move {
            if let Some(before) = before {
                before().await;
            }
            if gone {
                return Ok(WriteOutcome::Gone);
            }
            writes.lock().unwrap().push(line);
            Ok(WriteOutcome::Written)
        }
        .boxed()
    }
    fn close_input(&self) {
        self.note("closeInput");
    }
    fn taken(&self) -> Option<Vec<String>> {
        self.opts.taken.clone()
    }
    fn exited(&self) -> SharedFuture<Option<i32>> {
        self.exited.clone()
    }
    fn signalled(&self) -> Option<String> {
        if self.exit.settled().is_some() {
            self.opts.signalled.clone()
        } else {
            None
        }
    }
}

/// What a fake factory makes its streams from.
#[derive(Clone)]
pub enum Script {
    /// Every launch replays these lines.
    Lines(Vec<String>),
    /// Every launch is a manual stream; the newest is `FakeExec::current()`.
    Manual,
}

/// A fake ExecStreamFactory recording every launch. `attach` is offered only where `attach` is set: `Some(true)`
/// answers Gone, `Some(false)` a stream of the script.
pub struct FakeExec {
    pub script: Script,
    pub opts: StreamOpts,
    pub calls: Arc<Mutex<Vec<Call>>>,
    pub order: Arc<Mutex<Vec<String>>>,
    pub writes: Arc<Mutex<Vec<String>>>,
    pub streams: Arc<Mutex<Vec<Arc<FakeStream>>>>,
    pub attach: Option<bool>,
    pub attached: Arc<Mutex<Vec<(String, wsp_adapters::port::AttachRunOptions)>>>,
    /// Held input_after futures, in launch order, so a test can see the seed wait.
    pub held: Arc<Mutex<Vec<Settle>>>,
}

impl FakeExec {
    fn with(script: Script, opts: StreamOpts) -> Arc<Self> {
        Arc::new(FakeExec { script, opts, calls: Default::default(), order: Default::default(), writes: Default::default(), streams: Default::default(), attach: None, attached: Default::default(), held: Default::default() })
    }
    /// `scriptedExec(lines, opts)`, every stream reporting RUN_HANDLE.
    pub fn scripted(lines: Vec<String>, mut opts: StreamOpts) -> Arc<Self> {
        opts.run.get_or_insert_with(|| RUN_HANDLE.to_string());
        Self::with(Script::Lines(lines), opts)
    }
    /// `manualExec(opts)`.
    pub fn manual(opts: StreamOpts) -> Arc<Self> {
        Self::with(Script::Manual, opts)
    }
    /// The same factory with attach offered.
    pub fn attaching(lines: Vec<String>, gone: bool) -> Arc<Self> {
        let mut f = FakeExec { script: Script::Lines(lines), opts: StreamOpts { run: Some(RUN_HANDLE.to_string()), ..Default::default() }, calls: Default::default(), order: Default::default(), writes: Default::default(), streams: Default::default(), attach: Some(gone), attached: Default::default(), held: Default::default() };
        f.attach = Some(gone);
        Arc::new(f)
    }
    /// The newest stream launched.
    pub fn current(&self) -> Arc<FakeStream> {
        self.streams.lock().unwrap().last().cloned().expect("no stream launched yet")
    }
    pub fn push(&self, line: impl Into<String>) {
        self.current().push(line);
    }
    pub fn end(&self, code: Option<i32>) {
        self.current().end(code);
    }
    pub fn calls(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }
    pub fn order(&self) -> Vec<String> {
        self.order.lock().unwrap().clone()
    }
    pub fn writes(&self) -> Vec<String> {
        self.writes.lock().unwrap().clone()
    }
    fn make(&self) -> Arc<FakeStream> {
        let stream = match &self.script {
            Script::Lines(lines) => FakeStream::scripted(lines.clone(), self.opts.clone(), self.order.clone(), self.writes.clone()),
            Script::Manual => FakeStream::manual(self.opts.clone(), self.order.clone(), self.writes.clone()),
        };
        self.streams.lock().unwrap().push(stream.clone());
        stream
    }
}

impl ExecStreamFactory for FakeExec {
    fn launch(&self, command: &str, options: ExecOptions) -> Arc<dyn ExecStream> {
        let ExecOptions { env, input, input_after, secret } = options;
        self.calls.lock().unwrap().push(Call { command: command.to_string(), env, input, input_after: input_after.is_some(), secret });
        if let Some(after) = input_after {
            self.held.lock().unwrap().push(after);
        }
        self.make()
    }
    fn attach(&self, run: &str, options: wsp_adapters::port::AttachRunOptions) -> Option<BoxFuture<'static, Result<wsp_adapters::port::Attached, ExecFailure>>> {
        let gone = self.attach?;
        self.attached.lock().unwrap().push((run.to_string(), options));
        let stream = if gone { None } else { Some(self.make()) };
        Some(
            async move {
                Ok(match stream {
                    Some(s) => wsp_adapters::port::Attached::Stream(s),
                    None => wsp_adapters::port::Attached::Gone,
                })
            }
            .boxed(),
        )
    }
}

/// `collect()`: every event in order, and the callback that records them.
pub fn collect() -> (Arc<Mutex<Vec<AdapterEvent>>>, OnEvent) {
    let events: Arc<Mutex<Vec<AdapterEvent>>> = Default::default();
    let sink = events.clone();
    (events, Arc::new(move |e| sink.lock().unwrap().push(e)))
}

/// `until(check)`: up to 200 polls 1 ms apart, then panics "condition never held".
pub async fn until(check: impl Fn() -> bool) {
    for _ in 0..200 {
        if check() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }
    assert!(check(), "condition never held");
}

/// The event type names in order, the TS tests' `events.map(e => e.type)`.
pub fn types(events: &[AdapterEvent]) -> Vec<&'static str> {
    events
        .iter()
        .map(|e| match e {
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
        })
        .collect()
}

/// The deltas among `events`, in order.
pub fn deltas(events: &[AdapterEvent]) -> Vec<wsp_adapters::port::TurnDelta> {
    events.iter().filter_map(|e| if let AdapterEvent::TurnDelta(d) = e { Some(d.clone()) } else { None }).collect()
}

/// The one turn.done's result; panics unless there is exactly one.
pub fn done(events: &[AdapterEvent]) -> wsp_adapters::port::TurnResult {
    let all: Vec<_> = events.iter().filter_map(|e| if let AdapterEvent::TurnDone { result, .. } = e { Some(result.clone()) } else { None }).collect();
    assert_eq!(all.len(), 1, "expected one turn.done, got {}", all.len());
    all.into_iter().next().unwrap()
}

/// A JSON line as an object, for asserting on a written line's fields.
pub fn json(line: &str) -> serde_json::Value {
    serde_json::from_str(line).unwrap_or_else(|e| panic!("not JSON ({e}): {line}"))
}
