// SPDX-License-Identifier: AGPL-3.0-only
//! A scripted exec road for the OpenCode and Cursor adapters' tests: the Rust form of `scriptedExec` in
//! packages/adapter-opencode/test/adapter.test.ts:29-60 and packages/adapter-cursor/test/adapter.test.ts:29-63 at
//! a3b870a6e. Pull it in with `#[path = "common/codex_scripted.rs"] mod codex_scripted;`. Test code only.
#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use futures::future::{BoxFuture, FutureExt};
use futures::stream::{BoxStream, StreamExt};
use tokio::sync::watch;
use wsp_adapters::port::{AdapterEvent, AttachRunOptions, Attached, Env, ExecFailure, ExecOptions, ExecStream, ExecStreamFactory, OnEvent, SharedFuture, WriteOutcome};

#[derive(Clone, Default)]
pub struct ScriptOpts {
    pub exit_code: Option<i32>,
    /// The stream ends only once the process is signalled.
    pub hang: bool,
    /// The stream throws this after its lines, the process exiting with no code first.
    pub fails: Option<ExecFailure>,
}

#[derive(Clone, Debug)]
pub struct Call {
    pub command: String,
    pub env: Env,
    pub input: Option<Vec<String>>,
}

#[derive(Clone)]
struct Exit(Arc<watch::Sender<Option<Option<i32>>>>);

impl Exit {
    fn new() -> Self {
        Exit(Arc::new(watch::channel(None).0))
    }
    fn resolve(&self, code: Option<i32>) {
        self.0.send_if_modified(|v| {
            if v.is_none() {
                *v = Some(code);
                true
            } else {
                false
            }
        });
    }
    fn future(&self) -> SharedFuture<Option<i32>> {
        let mut rx = self.0.subscribe();
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

struct Scripted {
    lines: Mutex<Option<Vec<String>>>,
    opts: ScriptOpts,
    exit: Exit,
    order: Arc<Mutex<Vec<String>>>,
    run: String,
}

impl ExecStream for Scripted {
    fn lines(&self) -> BoxStream<'static, Result<String, ExecFailure>> {
        let lines = self.lines.lock().unwrap().take().unwrap_or_default();
        let opts = self.opts.clone();
        let exit = self.exit.clone();
        let head = futures::stream::iter(lines.into_iter().map(Ok));
        let tail = futures::stream::once(async move {
            if let Some(f) = opts.fails {
                exit.resolve(None);
                return Some(Err(f));
            }
            if opts.hang {
                exit.future().await;
            } else {
                exit.resolve(Some(opts.exit_code.unwrap_or(0)));
            }
            None
        })
        .filter_map(|x| async move { x });
        head.chain(tail).boxed()
    }
    fn run(&self) -> Option<String> {
        Some(self.run.clone())
    }
    fn teardown(&self) {
        self.order.lock().unwrap().push("teardown".into());
        self.exit.resolve(Some(143));
    }
    fn kill(&self) {
        self.order.lock().unwrap().push("kill".into());
        self.exit.resolve(None);
    }
    fn write(&self, _line: String) -> BoxFuture<'static, Result<WriteOutcome, ExecFailure>> {
        async { Ok(WriteOutcome::Written) }.boxed()
    }
    fn close_input(&self) {
        self.order.lock().unwrap().push("closeInput".into());
    }
    fn exited(&self) -> SharedFuture<Option<i32>> {
        self.exit.future()
    }
}

type AttachFn = Arc<dyn Fn(&str, AttachRunOptions) -> BoxFuture<'static, Result<Attached, ExecFailure>> + Send + Sync>;

/// Replays `lines` on every launch. TS `scriptedExec`.
#[derive(Clone)]
pub struct ScriptedExec {
    lines: Vec<String>,
    opts: ScriptOpts,
    run: String,
    pub calls: Arc<Mutex<Vec<Call>>>,
    pub order: Arc<Mutex<Vec<String>>>,
    pub attached: Arc<Mutex<Vec<(String, bool)>>>,
    attach_ok: Arc<Mutex<bool>>,
}

pub fn scripted_exec(lines: Vec<String>, opts: ScriptOpts, run: &str) -> ScriptedExec {
    ScriptedExec { lines, opts, run: run.to_string(), calls: Arc::default(), order: Arc::default(), attached: Arc::default(), attach_ok: Arc::default() }
}

impl ScriptedExec {
    pub fn factory(&self) -> Arc<dyn ExecStreamFactory> {
        Arc::new(self.clone())
    }
    pub fn calls(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }
    pub fn order(&self) -> Vec<String> {
        self.order.lock().unwrap().clone()
    }
    pub fn attached(&self) -> Vec<(String, bool)> {
        self.attached.lock().unwrap().clone()
    }
    /// Turns on attach: it records the run and the input flag and answers a fresh replay of the lines.
    pub fn with_attach(self) -> Self {
        *self.attach_ok.lock().unwrap() = true;
        self
    }
    fn stream(&self) -> Arc<dyn ExecStream> {
        Arc::new(Scripted { lines: Mutex::new(Some(self.lines.clone())), opts: self.opts.clone(), exit: Exit::new(), order: self.order.clone(), run: self.run.clone() })
    }
}

impl ExecStreamFactory for ScriptedExec {
    fn launch(&self, command: &str, options: ExecOptions) -> Arc<dyn ExecStream> {
        self.calls.lock().unwrap().push(Call { command: command.to_string(), env: options.env.clone(), input: options.input.clone() });
        self.stream()
    }
    fn attach(&self, run: &str, options: AttachRunOptions) -> Option<BoxFuture<'static, Result<Attached, ExecFailure>>> {
        if !*self.attach_ok.lock().unwrap() {
            return None;
        }
        self.attached.lock().unwrap().push((run.to_string(), options.input));
        let stream = self.stream();
        Some(async move { Ok(Attached::Stream(stream)) }.boxed())
    }
}

/// Events in order.
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
}

pub fn no_events() -> OnEvent {
    Arc::new(|_| {})
}

/// `delta:<kind>` for a delta, else the TS event type.
pub fn kind_of(e: &AdapterEvent) -> String {
    match e {
        AdapterEvent::TurnDelta(d) => format!("delta:{}", serde_json::to_value(d.kind).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default()),
        AdapterEvent::SessionStart { .. } => "session.start".into(),
        AdapterEvent::TurnDone { .. } => "turn.done".into(),
        AdapterEvent::SessionEnd { .. } => "session.end".into(),
        other => format!("{other:?}").split_whitespace().next().unwrap_or("").to_string(),
    }
}

/// Waits up to a second for `check`. TS `vi.waitFor`.
pub async fn wait_for(check: impl Fn() -> bool) {
    for _ in 0..1000 {
        if check() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }
    assert!(check(), "the condition never held");
}

/// A fixture's non-blank lines, under tests/fixtures/<dir>/<name>.
pub fn fixture(dir: &str, name: &str) -> Vec<String> {
    let path = format!("{}/tests/fixtures/{}/{}", env!("CARGO_MANIFEST_DIR"), dir, name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}")).lines().filter(|l| !l.trim().is_empty()).map(String::from).collect()
}
