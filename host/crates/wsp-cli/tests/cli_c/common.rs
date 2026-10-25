// SPDX-License-Identifier: AGPL-3.0-only
//! What cli-c's tests share: the recorded cases of `tests/fixtures/cli_c/<unit>.json`, a scripted host a helper is
//! called against, and the one way a result is held to the TS. The cases were recorded from the TS helpers of
//! `turns-help.ts`, `workspaces-help.ts` and `agents-help.ts` at 65f6b89c7 by `tests/fixtures/cli_c/record/`: each
//! helper called with the recorded arguments against a host answering from the case's script, writing down every
//! frame it sent (text as it crossed the socket, `JSON.stringify({...params, id, op})`), every line it said, and what
//! it returned or threw.
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Value};
use wsp_cli::verbs::client::{Again, BoxFuture, CliIo, Dialer, Frame, FrameOff, FrameSink, HostClient, Paired, VerbDeps};
use wsp_words::exit::{HostError, Kind};

/// What the TS answered an op no reply names, after the op's name.
pub const NOT_IN_RECORD: &str = "is not in this record";
pub const HOST_STOPPING_LINE: &str = "the host is restarting; the turn goes on and its reply lands in the thread";
pub const HOST_CLOSED_LINE: &str = "the host closed the connection";
pub const HOST_STOPPING_CLOSE: u16 = 4001;

/// The host a case scripts. Keys are an op, or `<op> #<n>` for its nth ask.
/// `replies`: the frame text answering an op. `pushed`: frames every listener hears while the op is under way,
/// before its reply. `after`: frames heard once the op's reply is back, on a later turn. `closes`: the socket goes
/// during the op, after its pushed frames and with no reply, under that close code. `closes_after`: the socket goes
/// under that code on a later turn, once the reply and its `after` frames are back. The text `<request id>` in a
/// pushed or after frame is the `requestId` the op was sent with. `redials`: what each later `deps.client()` answers.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Script {
    #[serde(default)]
    pub replies: IndexMap<String, String>,
    #[serde(default)]
    pub pushed: IndexMap<String, Vec<String>>,
    #[serde(default)]
    pub after: IndexMap<String, Vec<String>>,
    #[serde(default)]
    pub closes: IndexMap<String, u16>,
    #[serde(default, rename = "closesAfter")]
    pub closes_after: IndexMap<String, u16>,
    #[serde(default)]
    pub redials: Vec<Redial>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum Redial {
    Refuses { refuses: String },
    Script(Box<Script>),
}

/// What the TS threw: the message and the stamps `errorOf` of the recorder writes down.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct ErrOut {
    pub message: String,
    #[serde(default)]
    pub fix: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub errors: Option<Value>,
    #[serde(default)]
    pub warnings: Option<Value>,
    #[serde(default)]
    pub left: Option<Value>,
    #[serde(default, rename = "hostKey")]
    pub host_key: Option<String>,
    #[serde(default)]
    pub marks: Option<Vec<String>>,
}

fn present<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(d).map(Some)
}

/// One recorded case: a helper called with `args` against `script`, and what the TS did.
#[derive(Clone, Debug, Deserialize)]
pub struct Case {
    #[serde(rename = "fn")]
    pub func: String,
    pub case: String,
    #[serde(default)]
    pub args: Map<String, Value>,
    #[serde(default)]
    pub script: Script,
    #[serde(default)]
    pub env: IndexMap<String, String>,
    #[serde(default)]
    pub files: IndexMap<String, String>,
    #[serde(default)]
    pub cloud: bool,
    #[serde(default)]
    pub within: Option<u64>,
    pub frames: Vec<String>,
    #[serde(default)]
    pub said: Vec<String>,
    #[serde(default)]
    pub stdout: Vec<String>,
    #[serde(default)]
    pub stderr: Vec<String>,
    #[serde(default)]
    pub streamed: Option<String>,
    /// What it returned; present (even as null) only where it returned.
    #[serde(default, deserialize_with = "present")]
    pub value: Option<Value>,
    #[serde(default)]
    pub error: Option<ErrOut>,
    #[serde(default)]
    pub hung: bool,
}

#[derive(Deserialize)]
struct Fixture {
    cases: Vec<Case>,
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cli_c")
}

/// Every case of a unit, in recorded order.
pub fn cases(unit: &str) -> Vec<Case> {
    let text = std::fs::read_to_string(fixtures().join(format!("{unit}.json"))).expect("fixture");
    serde_json::from_str::<Fixture>(&text).expect("fixture parses").cases
}

/// One case by its helper and name, with the text `<cwd>` in its arguments, script, files and environment written as
/// `cwd`, the folder the case runs in (the recorder does the same).
pub fn case(unit: &str, func: &str, name: &str, cwd: &str) -> Case {
    let text = std::fs::read_to_string(fixtures().join(format!("{unit}.json"))).expect("fixture");
    let all: Value = serde_json::from_str(&text).expect("fixture parses");
    let hit = all["cases"].as_array().expect("cases").iter().find(|c| c["fn"] == func && c["case"] == name).unwrap_or_else(|| panic!("no case {func} / {name} in {unit}"));
    let mut c: Case = serde_json::from_str(&serde_json::to_string(hit).unwrap().replace("<cwd>", cwd)).expect("case reads");
    // What the TS printed is scrubbed back to `<cwd>`; only the inputs take the folder.
    let want = |v: Vec<String>| v;
    c.frames = hit["frames"].as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect();
    c.said = want(hit["said"].as_array().map(|a| a.iter().map(|s| s.as_str().unwrap().to_string()).collect()).unwrap_or_default());
    c.stdout = hit.get("stdout").and_then(|a| a.as_array()).map(|a| a.iter().map(|s| s.as_str().unwrap().to_string()).collect()).unwrap_or_default();
    c.stderr = hit.get("stderr").and_then(|a| a.as_array()).map(|a| a.iter().map(|s| s.as_str().unwrap().to_string()).collect()).unwrap_or_default();
    c.streamed = hit.get("streamed").and_then(|s| s.as_str()).map(String::from);
    if let Some(v) = hit.get("value") {
        c.value = Some(v.clone());
    }
    c
}

struct Gone {
    code: Option<u16>,
}

/// One socket as a script drives it (mcp-record.test.ts's host, grown by `after`, `closes` and the closed socket).
pub struct ScriptedHost {
    script: Script,
    log: Arc<Mutex<Vec<String>>>,
    ids: Arc<Mutex<Vec<String>>>,
    label: String,
    seen: Mutex<HashMap<String, u32>>,
    next: AtomicU64,
    listeners: Mutex<Vec<FrameSink>>,
    subscribed: Mutex<bool>,
    gone: Mutex<Option<Gone>>,
    closed_tx: tokio::sync::watch::Sender<Option<Option<u16>>>,
}

impl ScriptedHost {
    pub fn new(script: Script, log: Arc<Mutex<Vec<String>>>, ids: Arc<Mutex<Vec<String>>>) -> Arc<ScriptedHost> {
        let (closed_tx, _) = tokio::sync::watch::channel(None);
        Arc::new(ScriptedHost {
            script,
            log,
            ids,
            label: String::new(),
            seen: Mutex::new(HashMap::new()),
            next: AtomicU64::new(2),
            listeners: Mutex::new(Vec::new()),
            subscribed: Mutex::new(false),
            gone: Mutex::new(None),
            closed_tx,
        })
    }

    pub fn sent(&self) -> Vec<String> {
        self.log.lock().unwrap().clone()
    }

    fn hear(&self, frames: Option<&Vec<String>>, request_id: Option<&str>) {
        for text in frames.into_iter().flatten() {
            let text = match request_id {
                Some(id) => text.replace("<request id>", id),
                None => text.clone(),
            };
            let f: Frame = serde_json::from_str(&text).expect("pushed frame");
            let sinks: Vec<FrameSink> = self.listeners.lock().unwrap().clone();
            for sink in sinks {
                sink(&f);
            }
        }
    }

    fn key<T>(&self, op: &str, nth: u32, table: &IndexMap<String, T>) -> Option<String> {
        let numbered = format!("{op} #{nth}");
        if table.contains_key(&numbered) {
            Some(numbered)
        } else if table.contains_key(op) {
            Some(op.to_string())
        } else {
            None
        }
    }

    fn ask(self: &Arc<Self>, op: &str, params: Frame) -> Result<Frame, HostError> {
        if self.gone.lock().unwrap().is_some() {
            return Err(HostError::new(self.close_words()));
        }
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        let nth = {
            let mut seen = self.seen.lock().unwrap();
            let n = seen.entry(op.to_string()).or_insert(0);
            *n += 1;
            *n
        };
        let request_id = params.get("requestId").and_then(|v| v.as_str()).map(String::from);
        if let Some(id) = &request_id {
            self.ids.lock().unwrap().push(id.clone());
        }
        let mut sent = Map::new();
        for (k, v) in params {
            sent.insert(k, v);
        }
        sent.insert("id".into(), Value::from(id));
        sent.insert("op".into(), Value::from(op));
        if sent.contains_key("requestId") {
            sent.insert("requestId".into(), Value::from("<request id>"));
        }
        self.log.lock().unwrap().push(format!("{}{}", self.label, serde_json::to_string(&Value::Object(sent)).unwrap()));
        if let Some(k) = self.key(op, nth, &self.script.pushed) {
            self.hear(self.script.pushed.get(&k), request_id.as_deref());
        }
        if let Some(k) = self.key(op, nth, &self.script.closes) {
            let code = self.script.closes[&k];
            *self.gone.lock().unwrap() = Some(Gone { code: Some(code) });
            let _ = self.closed_tx.send(Some(Some(code)));
            return Err(HostError::new(self.close_words()));
        }
        let answer = match self.key(op, nth, &self.script.replies) {
            Some(k) => self.script.replies[&k].clone(),
            None if op == "events.subscribe" => r#"{"id":1,"ok":true}"#.to_string(),
            None => serde_json::json!({ "ok": false, "error": format!("{op} {NOT_IN_RECORD}") }).to_string(),
        };
        let frame: Frame = serde_json::from_str(&answer).expect("reply is a JSON object");
        let later = self.key(op, nth, &self.script.after).map(|k| self.script.after[&k].clone());
        let close_later = self.key(op, nth, &self.script.closes_after).map(|k| self.script.closes_after[&k]);
        if later.is_some() || close_later.is_some() {
            let me = self.clone();
            tokio::spawn(async move {
                tokio::task::yield_now().await;
                me.hear(later.as_ref(), request_id.as_deref());
                if let Some(code) = close_later {
                    *me.gone.lock().unwrap() = Some(Gone { code: Some(code) });
                    let _ = me.closed_tx.send(Some(Some(code)));
                }
            });
        }
        if frame.get("ok") != Some(&Value::Bool(true)) {
            let message = match frame.get("error") {
                Some(Value::String(s)) => s.clone(),
                _ => format!("{op} failed"),
            };
            let mut e = match frame.get("kind") {
                Some(Value::String(k)) => HostError::with_kind(message, Kind::parse(k)),
                _ => HostError::new(message),
            };
            if let Some(Value::Array(a)) = frame.get("errors") {
                e.errors = Some(a.clone());
            }
            if let Some(Value::Array(a)) = frame.get("warnings") {
                e.warnings = Some(a.clone());
            }
            return Err(e);
        }
        Ok(frame)
    }
}

/// The socket a case hands out: the scripted host behind the `Arc`, which `ask` needs to schedule `after`.
pub struct Socket(pub Arc<ScriptedHost>);

#[async_trait]
impl HostClient for Socket {
    async fn request(&self, op: &str, params: Frame) -> Result<Frame, HostError> {
        self.0.ask(op, params)
    }

    async fn events(&self) -> Result<(), HostError> {
        if std::mem::replace(&mut *self.0.subscribed.lock().unwrap(), true) {
            return Ok(());
        }
        self.0.ask("events.subscribe", Frame::new()).map(|_| ())
    }

    fn on_frame(&self, sink: FrameSink) -> FrameOff {
        self.0.listeners.lock().unwrap().push(sink.clone());
        let host = self.0.clone();
        FrameOff(Box::new(move || host.listeners.lock().unwrap().retain(|s| !Arc::ptr_eq(s, &sink))))
    }

    async fn closed(&self) -> Option<u16> {
        let mut rx = self.0.closed_tx.subscribe();
        loop {
            if let Some(code) = *rx.borrow() {
                return code;
            }
            if rx.changed().await.is_err() {
                return std::future::pending().await;
            }
        }
    }

    fn gone(&self) -> Option<Option<u16>> {
        self.0.gone.lock().unwrap().as_ref().map(|g| g.code)
    }

    fn close_words(&self) -> String {
        self.0.close_words()
    }

    fn paired(&self) -> Option<Paired> {
        None
    }

    fn close(&self) {}

    fn terminate(&self) {}
}

impl ScriptedHost {
    fn close_words(&self) -> String {
        match self.gone.lock().unwrap().as_ref().and_then(|g| g.code) {
            Some(HOST_STOPPING_CLOSE) => HOST_STOPPING_LINE.to_string(),
            _ => HOST_CLOSED_LINE.to_string(),
        }
    }
}

/// `deps.client()` of a case: with no `again` it answers the case's own socket (`first`); `client(Some(again))`, the
/// dial after a host stopped, and `redial()`, the callback a start or a follow is handed, both answer the next of
/// `redials` in order, writing `<dialled again>` into the log where the frames are; one beyond them refuses `no host
/// serving the state file`.
pub struct Deps {
    script: Script,
    log: Arc<Mutex<Vec<String>>>,
    ids: Arc<Mutex<Vec<String>>>,
    first: Arc<dyn HostClient>,
    redialled: AtomicU64,
}

impl Deps {
    pub fn new(script: Script, log: Arc<Mutex<Vec<String>>>, ids: Arc<Mutex<Vec<String>>>, first: Arc<dyn HostClient>) -> Arc<Deps> {
        Arc::new(Deps { script, log, ids, first, redialled: AtomicU64::new(0) })
    }

    fn dial(&self, at: usize) -> Result<Arc<dyn HostClient>, HostError> {
        match self.script.redials.get(at).cloned() {
            None => Err(HostError::new("no host serving the state file")),
            Some(Redial::Refuses { refuses }) => Err(HostError::new(refuses)),
            Some(Redial::Script(s)) => {
                self.log.lock().unwrap().push("<dialled again>".to_string());
                Ok(Arc::new(Socket(ScriptedHost::new(*s, self.log.clone(), self.ids.clone()))))
            }
        }
    }

    /// The callback `startDetached` and `follow` dial again through.
    pub async fn redial(&self) -> Result<Arc<dyn HostClient>, HostError> {
        self.dial(self.redialled.fetch_add(1, Ordering::SeqCst) as usize)
    }
}

#[async_trait]
impl Dialer for Deps {
    async fn client(&self, again: Option<Again>) -> Result<Arc<dyn HostClient>, HostError> {
        match again {
            None => Ok(self.first.clone()),
            Some(_) => self.redial().await,
        }
    }
}

/// How a case's terminal is set up (`IoOpts` of the recorder): `muted` wraps every muted line in `<m>..</m>`,
/// `same_screen` says the stream is the person's own screen, `key` is the key typed at a prompt (a key is offered
/// only where one is given).
#[derive(Clone, Debug, Default)]
pub struct IoOpts {
    pub muted: bool,
    pub same_screen: bool,
    pub key: Option<String>,
}

impl IoOpts {
    /// Read off the case's `io` argument, `{muted?, sameScreen?, key?}`; none given is a plain terminal.
    pub fn of(case: &Case) -> IoOpts {
        let io = case.args.get("io");
        IoOpts {
            muted: io.and_then(|o| o["muted"].as_bool()).unwrap_or(false),
            same_screen: io.and_then(|o| o["sameScreen"].as_bool()).unwrap_or(false),
            key: io.and_then(|o| o["key"].as_str()).map(String::from),
        }
    }
}

/// A terminal that keeps what the line printed, as the recorder's `io` does.
#[derive(Default)]
pub struct Sinks {
    pub opts: IoOpts,
    pub stdout: Mutex<Vec<String>>,
    pub stderr: Mutex<Vec<String>>,
    pub streamed: Mutex<String>,
    pub said: Mutex<Vec<String>>,
    pub answers: Mutex<Vec<String>>,
}

impl Sinks {
    /// The `say` callback of helpers that take one: each line goes to `said`.
    pub fn say(self: &Arc<Self>) -> impl Fn(&str) + Send + Sync + 'static {
        let me = self.clone();
        move |line: &str| me.said.lock().unwrap().push(line.to_string())
    }
}

#[async_trait]
impl CliIo for Sinks {
    fn log(&self, line: &str) {
        self.stdout.lock().unwrap().push(line.to_string());
    }

    fn error(&self, line: &str) {
        self.stderr.lock().unwrap().push(line.to_string());
    }

    fn stream(&self, text: &str) {
        self.streamed.lock().unwrap().push_str(text);
    }

    fn muted(&self, text: &str) -> Option<String> {
        self.opts.muted.then(|| format!("<m>{text}</m>"))
    }

    fn same_screen(&self) -> bool {
        self.opts.same_screen
    }

    fn answers_keys(&self) -> bool {
        self.opts.key.is_some()
    }

    async fn answer_key(&self, _accept: &[String], _until: BoxFuture<'_, ()>) -> Option<String> {
        self.opts.key.clone()
    }

    async fn ask(&self, question: &str) -> Result<String, HostError> {
        Err(HostError::new(format!("unexpected prompt: {question}")))
    }

    async fn ask_secret(&self, question: &str, _variable: Option<&str>) -> Result<String, HostError> {
        Err(HostError::new(format!("unexpected prompt: {question}")))
    }
}

/// Everything a driver is handed: the case, the one client of its first dial, the deps whose `client()` dials the
/// rest, the sinks and the folder the case's files are in.
pub struct Env {
    pub case: Case,
    pub client: Arc<dyn HostClient>,
    pub deps: VerbDeps,
    pub dialer: Arc<Deps>,
    pub sinks: Arc<Sinks>,
    pub cwd: String,
}

impl Env {
    /// Whether the case runs with the cloud on (the recorder loads the TS with `WSP_CLOUD=1`). A helper that reads
    /// `CLOUD_ON` in the TS takes it as a parameter here (`cloud_on: bool`, as `verbs_for(cloud_on)` does), so a test
    /// runs both settings in one process.
    pub fn cloud(&self) -> bool {
        self.case.cloud
    }

    pub fn arg(&self, name: &str) -> Option<&Value> {
        self.case.args.get(name)
    }

    pub fn str_arg(&self, name: &str) -> Option<String> {
        self.arg(name).and_then(|v| v.as_str()).map(String::from)
    }

    pub fn strs(&self, name: &str) -> Vec<String> {
        self.arg(name).and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default()
    }

    pub fn flag(&self, name: &str) -> bool {
        self.arg(name).and_then(|v| v.as_bool()).unwrap_or(false)
    }
}

/// What a driver does with a case: calls the Rust helper with the case's arguments and answers what it returned as
/// the JSON the TS value was written as (`norm` of the recorder), or the error it failed with.
pub type Driver = for<'a> fn(&'a Env) -> BoxFuture<'a, Result<Value, HostError>>;

/// What one case's run left: the case as recorded, what the Rust helper returned (None where it hung), the socket's
/// log (frames sent and `<dialled again>` marks), the request ids minted, and the terminal's lines.
pub struct Ran {
    pub case: Case,
    pub got: Option<Result<Value, HostError>>,
    pub log: Vec<String>,
    pub ids: Vec<String>,
    pub sinks: Arc<Sinks>,
    pub cwd: String,
}

impl Ran {
    /// The ops sent, in order, off the frames (marks of a dial again are left out).
    pub fn ops(&self) -> Vec<String> {
        self.log.iter().filter_map(|f| serde_json::from_str::<Map<String, Value>>(f).ok()).filter_map(|m| m.get("op").and_then(|o| o.as_str()).map(String::from)).collect()
    }

    /// The frame of an op's nth ask (from 0), as the JSON object that crossed the socket.
    pub fn frame(&self, op: &str, nth: usize) -> Map<String, Value> {
        self.log
            .iter()
            .filter_map(|f| serde_json::from_str::<Map<String, Value>>(f).ok())
            .filter(|m| m.get("op").and_then(|o| o.as_str()) == Some(op))
            .nth(nth)
            .unwrap_or_else(|| panic!("no {op} frame number {nth} in {:?}", self.log))
    }
}

/// Runs a case through its driver and keeps everything it left, asserting nothing.
pub fn run(unit: &str, func: &str, name: &str, driver: Driver) -> Ran {
    std::env::set_var("TZ", "UTC");
    let dir = tempfile::tempdir().expect("temp folder");
    let c = case(unit, func, name, &dir.path().to_string_lossy());
    for (file, text) in &c.files {
        let path = dir.path().join(file);
        std::fs::create_dir_all(path.parent().unwrap()).expect("case folder");
        std::fs::write(path, text).expect("case file");
    }
    let cwd = dir.path().to_string_lossy().into_owned();
    let log = Arc::new(Mutex::new(Vec::new()));
    let ids = Arc::new(Mutex::new(Vec::new()));
    let sinks = Arc::new(Sinks { opts: IoOpts::of(&c), ..Sinks::default() });
    let first: Arc<dyn HostClient> = Arc::new(Socket(ScriptedHost::new(c.script.clone(), log.clone(), ids.clone())));
    let dialer = Deps::new(c.script.clone(), log.clone(), ids.clone(), first.clone());
    let deps = VerbDeps {
        state_path: "/state/state.json".into(),
        cwd: Some(cwd.clone()),
        env: c.env.clone(),
        dialer: dialer.clone(),
        host_wait_ms: Some(200),
        start: None,
        signals: None,
        dial: None,
        elsewhere: false,
        terminal: None,
        open: None,
    };
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime");
    let got = rt.block_on(async {
        let env = Env { case: c.clone(), client: first, deps, dialer: dialer.clone(), sinks: sinks.clone(), cwd: cwd.clone() };
        tokio::time::timeout(Duration::from_millis(c.within.unwrap_or(2000)), driver(&env)).await
    });
    let (log, ids) = (log.lock().unwrap().clone(), ids.lock().unwrap().clone());
    Ran { case: c, got: got.ok(), log, ids, sinks, cwd }
}

/// Runs a case through its driver and holds everything to the TS: the frames sent, the lines said and printed, the
/// value or the error, and a hang for a hang.
pub fn holds(unit: &str, func: &str, name: &str, driver: Driver) {
    let ran = run(unit, func, name, driver);
    let (c, cwd) = (&ran.case, &ran.cwd);
    // The request id a start carries is minted fresh each run: written `<request id>` wherever it shows.
    let scrub = |s: &str| ran.ids.iter().fold(s.replace(cwd.as_str(), "<cwd>"), |t, id| t.replace(id.as_str(), "<request id>"));
    let said = |what: &Mutex<Vec<String>>| what.lock().unwrap().iter().map(|s| scrub(s)).collect::<Vec<_>>();
    let label = format!("{func} / {name}");
    assert_eq!(c.hung, ran.got.is_none(), "{label}: whether the call hung");
    assert_eq!(ran.log.iter().map(|s| scrub(s)).collect::<Vec<_>>(), c.frames, "{label}: frames sent");
    assert_eq!(said(&ran.sinks.said), c.said, "{label}: lines said");
    assert_eq!(said(&ran.sinks.stdout), c.stdout, "{label}: stdout");
    assert_eq!(said(&ran.sinks.stderr), c.stderr, "{label}: stderr");
    let streamed = scrub(&ran.sinks.streamed.lock().unwrap());
    assert_eq!(if streamed.is_empty() { None } else { Some(streamed) }, c.streamed, "{label}: streamed");
    let Some(done) = &ran.got else { return };
    match done {
        Ok(value) => {
            assert!(c.error.is_none(), "{label}: it returned where the TS threw {:?}", c.error);
            assert_eq!(scrub(&serde_json::to_string(value).unwrap()), serde_json::to_string(c.value.as_ref().expect("a recorded value")).unwrap(), "{label}: the value returned");
        }
        Err(e) => {
            let want = c.error.as_ref().unwrap_or_else(|| panic!("{label}: it threw {:?} where the TS returned {:?}", e.message, c.value));
            assert_eq!(e.message, want.message, "{label}: the message thrown");
            assert_eq!(e.fix, want.fix, "{label}: the fix");
            assert_eq!(e.kind.as_ref().map(|k| k.as_str().to_string()), want.kind, "{label}: the kind stamped");
            assert_eq!(e.errors.clone().map(Value::Array), want.errors, "{label}: errors");
            assert_eq!(e.warnings.clone().map(Value::Array), want.warnings, "{label}: warnings");
            assert_eq!(e.left.clone().map(Value::Array), want.left, "{label}: left");
            assert_eq!(e.host_key, want.host_key, "{label}: host key");
            let mut marks = e.marks.clone();
            marks.sort();
            let mut want_marks = want.marks.clone().unwrap_or_default();
            want_marks.sort();
            assert_eq!(marks, want_marks, "{label}: marks");
        }
    }
}
