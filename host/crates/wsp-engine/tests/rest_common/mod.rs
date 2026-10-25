// SPDX-License-Identifier: AGPL-3.0-only
//! Fakes the rest slice's tests share: a machine whose every road is a closure, a fetch scripted per route, the clocks
//! the retries sleep on, and the failures node's fetch rejects with. Test code; no body here stands in for an
//! implementation.
#![allow(dead_code)]

use std::collections::VecDeque;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use serde_json::Value;
use wsp_engine::cancel::Cancel;

use wsp_engine::errors::{EngineError, Result, RetryClock, RoadCause, RoadError};
use wsp_engine::fetch::{Fetch, FetchRequest, FetchResponse};
use wsp_engine::machine::{
    BytesLanded, ExecOptions, ExecResult, Machine, MachineKind, MachineLife, MachineOffers, MachineState, PreviewReach, RunOptions,
    SnapshotOptions,
};

pub fn ok(stdout: &str) -> ExecResult {
    ExecResult { exit_code: 0.into(), stdout: stdout.to_string(), stderr: String::new() }
}

pub fn exit(code: i64, stdout: &str, stderr: &str) -> ExecResult {
    ExecResult { exit_code: code.into(), stdout: stdout.to_string(), stderr: stderr.to_string() }
}

pub fn other(message: &str) -> EngineError {
    EngineError::Other { message: message.to_string(), code: None }
}

pub type ExecFn = Arc<dyn Fn(&str, &ExecOptions) -> Result<ExecResult>>;
pub type PutFn = Arc<dyn Fn(&str, &[u8], Option<u64>) -> Result<Option<BytesLanded>>>;
pub type UrlFn = Arc<dyn Fn(&str) -> Result<String>>;
pub type PreviewFn = Arc<dyn Fn(u16) -> Result<PreviewReach>>;

/// One exec the fake machine was asked for.
#[derive(Debug, Clone, PartialEq)]
pub struct Ran {
    pub cmd: String,
    pub opts: ExecOptions,
}

/// A machine whose roads are closures; every exec is recorded in `ran`. The default exec answers exit 0 with nothing,
/// the default upload and download URLs fail, and no optional road is offered unless its closure is set.
pub struct FakeMachine {
    pub id: String,
    pub takes_stdin: bool,
    pub exec_fn: ExecFn,
    pub put_bytes_fn: Option<PutFn>,
    pub upload_url_fn: UrlFn,
    pub preview_fn: Option<PreviewFn>,
    pub ran: Mutex<Vec<Ran>>,
    pub puts: Mutex<Vec<(String, Vec<u8>, Option<u64>)>>,
    pub previews: Mutex<Vec<u16>>,
}

impl FakeMachine {
    pub fn new(id: &str) -> Self {
        FakeMachine {
            id: id.to_string(),
            takes_stdin: false,
            exec_fn: Arc::new(|_, _| Ok(ok(""))),
            put_bytes_fn: None,
            upload_url_fn: Arc::new(|_| Err(other("no signed upload URL"))),
            preview_fn: None,
            ran: Mutex::new(Vec::new()),
            puts: Mutex::new(Vec::new()),
            previews: Mutex::new(Vec::new()),
        }
    }

    /// A machine that answers every exec with `f`.
    pub fn exec(id: &str, f: impl Fn(&str, &ExecOptions) -> Result<ExecResult> + 'static) -> Self {
        let mut m = FakeMachine::new(id);
        m.exec_fn = Arc::new(f);
        m
    }

    /// A machine that answers every exec with these lines, newline-joined with a trailing newline, exit 0.
    pub fn answering(lines: &[&str]) -> Self {
        let out = format!("{}\n", lines.join("\n"));
        FakeMachine::exec("m1", move |_, _| Ok(ok(&out)))
    }

    pub fn ran(&self) -> Vec<Ran> {
        self.ran.lock().unwrap().clone()
    }

    pub fn cmds(&self) -> Vec<String> {
        self.ran().into_iter().map(|r| r.cmd).collect()
    }
}

#[async_trait(?Send)]
impl Machine for FakeMachine {
    fn id(&self) -> &str {
        &self.id
    }
    fn kind(&self) -> MachineKind {
        serde_json::from_value(Value::String("sandbox".into())).unwrap()
    }
    fn takes_stdin(&self) -> bool {
        self.takes_stdin
    }
    fn offers(&self) -> MachineOffers {
        MachineOffers { put_bytes: self.put_bytes_fn.is_some(), preview_url: self.preview_fn.is_some(), ..MachineOffers::default() }
    }
    async fn exec(&self, cmd: &str, opts: ExecOptions) -> Result<ExecResult> {
        self.ran.lock().unwrap().push(Ran { cmd: cmd.to_string(), opts: opts.clone() });
        (self.exec_fn)(cmd, &opts)
    }
    async fn run(&self, _script: &str, _opts: RunOptions) -> Result<ExecResult> {
        Ok(ok(""))
    }
    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> Result<String> {
        Ok("s1".into())
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> Result<()> {
        Ok(())
    }
    async fn resume(&self, _signal: Option<&Cancel>) -> Result<()> {
        Ok(())
    }
    async fn kill(&self) -> Result<()> {
        Ok(())
    }
    async fn state(&self) -> Result<MachineState> {
        Ok(serde_json::from_value(Value::String("running".into())).unwrap())
    }
    async fn download_url(&self, _path: &str) -> Result<String> {
        Err(other("no signed download URL"))
    }
    async fn upload_url(&self, path: &str) -> Result<String> {
        (self.upload_url_fn)(path)
    }
    async fn put_bytes(&self, path: &str, bytes: &[u8], timeout_ms: Option<u64>) -> Result<Option<BytesLanded>> {
        self.puts.lock().unwrap().push((path.to_string(), bytes.to_vec(), timeout_ms));
        match &self.put_bytes_fn {
            Some(f) => f(path, bytes, timeout_ms),
            None => panic!("put_bytes called on a machine that does not offer it"),
        }
    }
    async fn preview_url(&self, port: u16) -> Result<PreviewReach> {
        self.previews.lock().unwrap().push(port);
        match &self.preview_fn {
            Some(f) => f(port),
            None => panic!("preview_url called on a machine that does not offer it"),
        }
    }
}

// ---- fetch

/// What a scripted route plays for one call.
#[derive(Clone)]
pub enum Play {
    /// A reply: status, body text, headers.
    Reply(u16, String, Vec<(String, String)>),
    /// The fetch rejects with this.
    Fail(RoadError),
    /// Holds until the request's abort fires, then rejects as fetch does: TimeoutError for the cap, AbortError for the
    /// caller's signal. A request with no abort hangs for ever.
    Hangs,
}

/// A JSON reply.
pub fn reply(status: u16, body: Value) -> Play {
    Play::Reply(status, body.to_string(), Vec::new())
}

/// A JSON reply with headers.
pub fn reply_with(status: u16, body: Value, headers: &[(&str, &str)]) -> Play {
    Play::Reply(status, body.to_string(), headers.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect())
}

/// A reply whose body is this text as it is.
pub fn reply_text(status: u16, text: &str) -> Play {
    Play::Reply(status, text.to_string(), Vec::new())
}

/// node's "fetch failed" whose cause is a system error carrying `code`, as a lookup that never left this computer.
pub fn no_road(code: &str) -> RoadError {
    RoadError {
        name: "TypeError".into(),
        message: "fetch failed".into(),
        code: None,
        cause: Some(RoadCause { message: format!("getaddrinfo {code} api.getsolari.com"), code: Some(code.into()), errors: None }),
    }
}

/// node's "fetch failed" with no cause at all.
pub fn fetch_failed() -> RoadError {
    RoadError { name: "TypeError".into(), message: "fetch failed".into(), code: None, cause: None }
}

/// node's "fetch failed" whose cause is an AggregateError holding one error per address tried.
pub fn aggregate(codes: &[&str]) -> RoadError {
    let errors = codes.iter().map(|c| RoadCause { message: format!("connect {c}"), code: Some(c.to_string()), errors: None }).collect();
    RoadError {
        name: "TypeError".into(),
        message: "fetch failed".into(),
        code: None,
        cause: Some(RoadCause { message: "".into(), code: None, errors: Some(errors) }),
    }
}

/// One call the fake fetch saw.
#[derive(Clone, Debug)]
pub struct Seen {
    pub method: String,
    pub url: String,
    pub path: String,
    pub query: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Value>,
    pub had_abort: bool,
}

impl Seen {
    pub fn key(&self) -> String {
        format!("{} {}", self.method, self.path)
    }
    pub fn header(&self, name: &str) -> Option<String> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.clone())
    }
}

type Handler = Arc<dyn Fn(&Seen) -> Option<Play>>;

/// A fetch scripted per `METHOD /path` (the path still percent-encoded, no query): a route plays its list in order and
/// repeats the last; a call no route names answers 404 `{"error":"no route <key>"}`. `handler`, where set, answers
/// first and may read the call (its body, its query).
pub struct FakeFetch {
    routes: Mutex<IndexMap<String, VecDeque<Play>>>,
    handler: Option<Handler>,
    pub calls: Mutex<Vec<Seen>>,
}

impl FakeFetch {
    pub fn new(routes: Vec<(&str, Vec<Play>)>) -> Arc<Self> {
        Arc::new(FakeFetch {
            routes: Mutex::new(routes.into_iter().map(|(k, v)| (k.to_string(), v.into_iter().collect())).collect()),
            handler: None,
            calls: Mutex::new(Vec::new()),
        })
    }

    /// One fixed play per route.
    pub fn fixed(routes: Vec<(&str, Play)>) -> Arc<Self> {
        FakeFetch::new(routes.into_iter().map(|(k, p)| (k, vec![p])).collect())
    }

    /// Every call answered by `f`; `None` falls through to the 404.
    pub fn handled(f: impl Fn(&Seen) -> Option<Play> + 'static) -> Arc<Self> {
        Arc::new(FakeFetch { routes: Mutex::new(IndexMap::new()), handler: Some(Arc::new(f)), calls: Mutex::new(Vec::new()) })
    }

    pub fn calls(&self) -> Vec<Seen> {
        self.calls.lock().unwrap().clone()
    }

    /// `METHOD /path` of every call, in order.
    pub fn keys(&self) -> Vec<String> {
        self.calls().iter().map(Seen::key).collect()
    }

    pub fn sent(&self, key: &str) -> usize {
        self.keys().iter().filter(|k| *k == key).count()
    }
}

fn split_url(url: &str) -> (String, String) {
    let after_scheme = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    let path_q = after_scheme.find('/').map(|i| &after_scheme[i..]).unwrap_or("/");
    match path_q.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (path_q.to_string(), String::new()),
    }
}

#[async_trait(?Send)]
impl Fetch for FakeFetch {
    async fn fetch(&self, req: FetchRequest) -> std::result::Result<FetchResponse, RoadError> {
        let (path, query) = split_url(&req.url);
        let body = req.body.as_ref().map(|b| serde_json::from_slice(b).unwrap_or(Value::String(String::from_utf8_lossy(b).into_owned())));
        let seen = Seen {
            method: req.method.clone(),
            url: req.url.clone(),
            path,
            query,
            headers: req.headers.clone(),
            body,
            had_abort: req.abort.is_some(),
        };
        self.calls.lock().unwrap().push(seen.clone());
        let play = self.handler.as_ref().and_then(|h| h(&seen)).or_else(|| {
            let mut routes = self.routes.lock().unwrap();
            routes.get_mut(&seen.key()).map(|q| if q.len() > 1 { q.pop_front().unwrap() } else { q[0].clone() })
        });
        let play = play.unwrap_or_else(|| reply(404, serde_json::json!({ "error": format!("no route {}", seen.key()) })));
        match play {
            Play::Reply(status, text, headers) => Ok(FetchResponse { status, headers, body: text.into_bytes() }),
            Play::Fail(e) => Err(e),
            Play::Hangs => match req.abort {
                Some(abort) => {
                    abort.token.cancelled().await;
                    let timed_out = abort.timed_out.load(Ordering::SeqCst);
                    Err(RoadError {
                        name: if timed_out { "TimeoutError" } else { "AbortError" }.into(),
                        message: if timed_out { "The operation was aborted due to timeout" } else { "This operation was aborted" }.into(),
                        code: Some(if timed_out { "23" } else { "20" }.into()),
                        cause: None,
                    })
                }
                None => futures::future::pending().await,
            },
        }
    }
}

// ---- clocks

/// A clock the sleeps move and record, so a bound is spent in no time; `random` answers `draw`.
pub struct MovingClock {
    pub t: AtomicI64,
    pub waits: Mutex<Vec<u64>>,
    pub draw: f64,
}

impl MovingClock {
    pub fn new() -> Arc<Self> {
        Arc::new(MovingClock { t: AtomicI64::new(0), waits: Mutex::new(Vec::new()), draw: 0.5 })
    }
    pub fn waits(&self) -> Vec<u64> {
        self.waits.lock().unwrap().clone()
    }
}

#[async_trait(?Send)]
impl RetryClock for MovingClock {
    fn now(&self) -> i64 {
        self.t.load(Ordering::SeqCst)
    }
    async fn sleep(&self, ms: u64) {
        self.waits.lock().unwrap().push(ms);
        self.t.fetch_add(ms as i64, Ordering::SeqCst);
    }
    fn random(&self) -> f64 {
        self.draw
    }
}

/// The wall clock, with sleeps recorded and skipped.
pub struct NoWaitClock {
    pub waits: Mutex<Vec<u64>>,
}

impl NoWaitClock {
    pub fn new() -> Arc<Self> {
        Arc::new(NoWaitClock { waits: Mutex::new(Vec::new()) })
    }
    pub fn waits(&self) -> Vec<u64> {
        self.waits.lock().unwrap().clone()
    }
}

#[async_trait(?Send)]
impl RetryClock for NoWaitClock {
    fn now(&self) -> i64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64
    }
    async fn sleep(&self, ms: u64) {
        self.waits.lock().unwrap().push(ms);
    }
    fn random(&self) -> f64 {
        0.5
    }
}

/// A clock frozen at 0 that sleeps for nothing.
pub struct FrozenClock;

#[async_trait(?Send)]
impl RetryClock for FrozenClock {
    fn now(&self) -> i64 {
        0
    }
    async fn sleep(&self, _ms: u64) {}
    fn random(&self) -> f64 {
        0.5
    }
}

// ---- warn lines

/// Collects the lines a backend warns with.
#[derive(Clone, Default)]
pub struct Warned(pub Arc<Mutex<Vec<String>>>);

impl Warned {
    pub fn sink(&self) -> wsp_engine::machine::OnLine {
        let lines = self.0.clone();
        Arc::new(move |l: &str| lines.lock().unwrap().push(l.to_string()))
    }
    pub fn lines(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}

/// A future that is already done, for closures typed to hand one back.
pub fn done<T: Send + 'static>(v: T) -> LocalBoxFuture<'static, T> {
    Box::pin(async move { v })
}

pub fn labels(pairs: &[(&str, &str)]) -> IndexMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

pub fn strings(xs: &[&str]) -> Vec<String> {
    xs.iter().map(|s| s.to_string()).collect()
}
