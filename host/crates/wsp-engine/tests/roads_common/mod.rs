// SPDX-License-Identifier: AGPL-3.0-only
//! Stand-ins the roads tests share: an ssh transport and a local run from a closure, a link that records its frames,
//! a machine whose exec is a closure, and the TS oracle's fixtures.
#![allow(dead_code)]
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::future::BoxFuture;
use futures::FutureExt;
use serde_json::{Map, Value};
use tokio_util::sync::CancellationToken;

use wsp_engine::errors::{EngineError, Result};
use wsp_engine::link_backend::{LinkAsk, MachineLink};
use wsp_engine::machine::{ExecOptions, ExecResult, Framing, Machine, MachineKind, MachineLife, MachineState, RunOptions, SnapshotOptions};
use wsp_engine::ssh_backend::{SshLocalRun, SshReach, SshTransport, SshTransportOptions};

pub fn res(exit_code: i64, stdout: &str, stderr: &str) -> ExecResult {
    ExecResult { exit_code, stdout: stdout.to_string(), stderr: stderr.to_string() }
}

/// An error the way a TS test throws a plain `new Error(message)`.
pub fn plain(message: &str) -> EngineError {
    EngineError::Other { message: message.to_string(), code: None }
}

/// The TS oracle's answer for one unit, `tests/fixtures/roads/<name>.json`.
pub fn fixture(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/roads").join(name);
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))).unwrap()
}

/// One script an ssh transport was asked to carry.
#[derive(Clone)]
pub struct Carried {
    pub reach: SshReach,
    pub script: String,
    pub opts: SshTransportOptions,
}

/// A transport that never leaves this process: each call is recorded, then answered by `answer`.
pub fn transport<F>(answer: F) -> (SshTransport, Arc<Mutex<Vec<Carried>>>)
where
    F: Fn(&SshReach, &str, &SshTransportOptions) -> ExecResult + Send + Sync + 'static,
{
    let carried = Arc::new(Mutex::new(Vec::new()));
    let log = carried.clone();
    let answer = Arc::new(answer);
    let t: SshTransport = Arc::new(move |reach: SshReach, script: String, opts: SshTransportOptions| {
        log.lock().unwrap().push(Carried { reach: reach.clone(), script: script.clone(), opts: opts.clone() });
        let out = answer(&reach, &script, &opts);
        async move { out }.boxed()
    });
    (t, carried)
}

/// The scripts a recorded transport carried, in order.
pub fn scripts(carried: &Arc<Mutex<Vec<Carried>>>) -> Vec<String> {
    carried.lock().unwrap().iter().map(|c| c.script.clone()).collect()
}

/// A local run that never starts a child: each call (file, args) is recorded, then answered by `answer`.
pub fn local_run<F>(answer: F) -> (SshLocalRun, Arc<Mutex<Vec<Vec<String>>>>)
where
    F: Fn(&str, &[String]) -> ExecResult + Send + Sync + 'static,
{
    let asked = Arc::new(Mutex::new(Vec::new()));
    let log = asked.clone();
    let run: SshLocalRun = Arc::new(move |file: String, args: Vec<String>, _timeout: u64| {
        let mut line = vec![file.clone()];
        line.extend(args.iter().cloned());
        log.lock().unwrap().push(line);
        let out = answer(&file, &args);
        async move { out }.boxed()
    });
    (run, asked)
}

/// One frame a recorded link was sent.
#[derive(Debug, Clone)]
pub struct Sent {
    pub op: String,
    pub params: Map<String, Value>,
    pub ask: LinkAsk,
}

type Answer = Arc<dyn Fn(&Sent) -> BoxFuture<'static, Result<Map<String, Value>>> + Send + Sync>;

/// A link that records every frame and answers through a closure; `dials_back` and `forward` as a test sets them.
pub struct FakeLink {
    pub sent: Mutex<Vec<Sent>>,
    pub forwarded: Mutex<Vec<u16>>,
    answer: Answer,
    dials_back: Option<Arc<dyn Fn() -> bool + Send + Sync>>,
}

impl FakeLink {
    /// Answers each frame with what `answer` gives at once.
    pub fn new<F>(answer: F) -> Arc<Self>
    where
        F: Fn(&Sent) -> Result<Value> + Send + Sync + 'static,
    {
        Self::build(
            Arc::new(move |s: &Sent| {
                let out = answer(s).map(|v| v.as_object().cloned().unwrap_or_default());
                async move { out }.boxed()
            }),
            None,
        )
    }

    /// Answers each frame with a future of the test's, for an answer that never comes.
    pub fn with_future(answer: Answer) -> Arc<Self> {
        Self::build(answer, None)
    }

    /// As `new`, on a link that redials and answers `dials_back` with the closure.
    pub fn dialling<F, D>(answer: F, dials_back: D) -> Arc<Self>
    where
        F: Fn(&Sent) -> Result<Value> + Send + Sync + 'static,
        D: Fn() -> bool + Send + Sync + 'static,
    {
        Self::build(
            Arc::new(move |s: &Sent| {
                let out = answer(s).map(|v| v.as_object().cloned().unwrap_or_default());
                async move { out }.boxed()
            }),
            Some(Arc::new(dials_back)),
        )
    }

    fn build(answer: Answer, dials_back: Option<Arc<dyn Fn() -> bool + Send + Sync>>) -> Arc<Self> {
        Arc::new(Self { sent: Mutex::new(Vec::new()), forwarded: Mutex::new(Vec::new()), answer, dials_back })
    }

    pub fn sent(&self) -> Vec<Sent> {
        self.sent.lock().unwrap().clone()
    }

    pub fn ops(&self) -> Vec<String> {
        self.sent().into_iter().map(|s| s.op).collect()
    }
}

#[async_trait]
impl MachineLink for FakeLink {
    async fn request(&self, op: &str, params: Map<String, Value>, ask: LinkAsk) -> Result<Map<String, Value>> {
        let sent = Sent { op: op.to_string(), params, ask };
        self.sent.lock().unwrap().push(sent.clone());
        (self.answer)(&sent).await
    }
    fn has_dials_back(&self) -> bool {
        self.dials_back.is_some()
    }
    async fn dials_back(&self) -> bool {
        self.dials_back.as_ref().map(|d| d()).unwrap_or(false)
    }
    async fn forward(&self, place_port: u16) -> Result<u16> {
        let mut f = self.forwarded.lock().unwrap();
        f.push(place_port);
        Ok(51000 + f.len() as u16)
    }
}

/// One exec a scripted machine was sent.
#[derive(Debug, Clone)]
pub struct ExecCall {
    pub cmd: String,
    pub opts: ExecOptions,
}

type ExecFn = Arc<dyn Fn(&str, &ExecOptions) -> BoxFuture<'static, Result<ExecResult>> + Send + Sync>;

/// A machine whose exec is a closure and whose every other call fails: what the TS tests build with
/// `{ id, exec } as unknown as Machine`. `framing` stands in for `framed()` where a test gives one.
pub struct ScriptedMachine {
    pub id: String,
    pub calls: Mutex<Vec<ExecCall>>,
    exec: ExecFn,
    pub framing: Option<Arc<dyn Framing>>,
}

impl ScriptedMachine {
    pub fn new<F>(id: &str, exec: F) -> Arc<Self>
    where
        F: Fn(&str, &ExecOptions) -> Result<ExecResult> + Send + Sync + 'static,
    {
        Self::with_future(
            id,
            Arc::new(move |cmd: &str, o: &ExecOptions| {
                let out = exec(cmd, o);
                async move { out }.boxed()
            }),
        )
    }

    pub fn with_future(id: &str, exec: ExecFn) -> Arc<Self> {
        Arc::new(Self { id: id.to_string(), calls: Mutex::new(Vec::new()), exec, framing: None })
    }

    pub fn framed_by(
        id: &str,
        framing: Arc<dyn Framing>,
        exec: impl Fn(&str, &ExecOptions) -> Result<ExecResult> + Send + Sync + 'static,
    ) -> Arc<Self> {
        let exec = Arc::new(exec);
        Arc::new(Self {
            id: id.to_string(),
            calls: Mutex::new(Vec::new()),
            exec: Arc::new(move |cmd: &str, o: &ExecOptions| {
                let out = exec(cmd, o);
                async move { out }.boxed()
            }),
            framing: Some(framing),
        })
    }

    pub fn cmds(&self) -> Vec<String> {
        self.calls.lock().unwrap().iter().map(|c| c.cmd.clone()).collect()
    }
}

fn not_here<T>() -> Result<T> {
    Err(plain("not in this test"))
}

#[async_trait]
impl Machine for ScriptedMachine {
    fn id(&self) -> &str {
        &self.id
    }
    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }
    fn offers(&self) -> wsp_engine::machine::MachineOffers {
        wsp_engine::machine::MachineOffers { framed: self.framing.is_some(), ..Default::default() }
    }
    async fn exec(&self, cmd: &str, opts: ExecOptions) -> Result<ExecResult> {
        self.calls.lock().unwrap().push(ExecCall { cmd: cmd.to_string(), opts: opts.clone() });
        (self.exec)(cmd, &opts).await
    }
    async fn run(&self, _script: &str, _opts: RunOptions) -> Result<ExecResult> {
        not_here()
    }
    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> Result<String> {
        not_here()
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> Result<()> {
        not_here()
    }
    async fn resume(&self, _signal: Option<CancellationToken>) -> Result<()> {
        not_here()
    }
    async fn kill(&self) -> Result<()> {
        not_here()
    }
    async fn state(&self) -> Result<MachineState> {
        not_here()
    }
    async fn download_url(&self, _path: &str) -> Result<String> {
        not_here()
    }
    async fn upload_url(&self, _path: &str) -> Result<String> {
        not_here()
    }
    async fn framed(&self) -> Result<Arc<dyn Framing>> {
        self.framing.clone().ok_or_else(|| plain("no framing"))
    }
}

/// A framing from two closures' worth of words, for the upload refusal cases.
pub struct TestFraming {
    pub pad: String,
    pub no_room: Option<String>,
}

impl Framing for TestFraming {
    fn wrap(&self, cmd: &str) -> String {
        format!("{}\n{cmd}", self.pad)
    }
    fn no_room(&self) -> Option<String> {
        self.no_room.clone()
    }
}
