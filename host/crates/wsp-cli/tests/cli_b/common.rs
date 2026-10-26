// SPDX-License-Identifier: AGPL-3.0-only
//! The harness cli-b's verb tests share: the recorded cases of `tests/fixtures/cli_b/<unit>.json`, a host that
//! answers each op with the case's frame and writes down every frame the line sends, and a terminal that keeps
//! what the line prints. The cases were recorded from the TS command line at 65f6b89c7 by
//! `tests/fixtures/cli_b/record/record.probe.ts`, over the replies of `daemon/crates/wsp-mcp/tests/answers`.
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::{Map, Value};
use wsp_cli::verbs::client::{
    has_tool, tool_failure, Again, BoxFuture, CliIo, DialFn, DialOpts, Dialer, Frame, FrameOff, FrameSink, HostClient, Paired, VerbDeps,
};
use wsp_cli::verbs::{find_verb, run_verb, verbs_for, RunDeps};
use wsp_words::exit::{HostError, Kind};

/// What the TS answered an op no reply names, after the op's name (mcp-record.test.ts:343).
pub const NOT_IN_RECORD: &str = "is not in this record";

/// One line's run as the TS printed it.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Run {
    pub frames: Vec<String>,
    pub stdout: Vec<String>,
    pub stderr: Vec<String>,
    #[serde(default)]
    pub streamed: Option<String>,
    pub exit: i32,
    /// The questions a terminal case was asked, in order; absent off a terminal.
    #[serde(default)]
    pub asked: Option<Vec<String>>,
}

/// A terminal a case types at: the secrets it pastes and the answers it gives, in order.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
pub struct Tty {
    pub secrets: Vec<String>,
    pub answers: Vec<String>,
}

/// One recorded case: the line, the host's frames, and what the TS line did as text and as `--json`. A case off a
/// tool's record also carries the tool's call and its TS answer, the `result` of the server's line.
#[derive(Clone, Debug, Deserialize)]
pub struct Case {
    #[serde(default)]
    pub tool: Option<String>,
    pub case: String,
    #[serde(default)]
    pub hand: bool,
    pub argv: Vec<String>,
    #[serde(default)]
    pub tty: Option<Tty>,
    #[serde(default)]
    pub cloud: bool,
    #[serde(default)]
    pub env: IndexMap<String, String>,
    #[serde(default)]
    pub files: IndexMap<String, String>,
    pub replies: IndexMap<String, String>,
    #[serde(default)]
    pub pushed: IndexMap<String, Vec<String>>,
    #[serde(default)]
    pub tool_arguments: Option<Value>,
    #[serde(default)]
    pub tool_asked: Option<Value>,
    #[serde(default)]
    pub tool_result: Option<Value>,
    pub text: Run,
    pub json: Run,
}

#[derive(Deserialize)]
struct Fixture {
    cases: Vec<Case>,
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cli_b")
}

/// Every case of a unit, in recorded order.
pub fn cases(unit: &str) -> Vec<Case> {
    let text = std::fs::read_to_string(fixtures().join(format!("{unit}.json"))).expect("fixture");
    serde_json::from_str::<Fixture>(&text).expect("fixture parses").cases
}

/// One case by its tool (or `hand`) and name.
pub fn case(unit: &str, tool: &str, name: &str) -> Case {
    cases(unit)
        .into_iter()
        .find(|c| c.case == name && (c.tool.as_deref() == Some(tool) || (tool == "hand" && c.hand)))
        .unwrap_or_else(|| panic!("no case {tool} / {name} in {unit}"))
}

/// The help pages of a unit's verbs as the TS printed them, keyed `local <verb>` and `cloud <verb>`.
pub fn pages(unit: &str) -> IndexMap<String, Value> {
    let text = std::fs::read_to_string(fixtures().join(format!("{unit}.pages.json"))).expect("pages fixture");
    serde_json::from_str(&text).expect("pages parse")
}

/// The pure functions' recorded inputs and outputs (`pure.json`).
pub fn pure() -> Value {
    let text = std::fs::read_to_string(fixtures().join("pure.json")).expect("pure fixture");
    serde_json::from_str(&text).expect("pure parses")
}

/// A host as far as one line asks it (mcp-record.test.ts:351-390): each op answered with the case's frame, the one
/// under `<op> #<n>` for its nth ask where the case names one; `events.subscribe` answered ok where the case names
/// none; an op the case does not name refused `<op> is not in this record`. Every frame the line sends is kept as
/// the text that crossed the socket, `JSON.stringify({...params, id, op})`, ids from 2 since the dial's auth is 1,
/// a start's fresh `requestId` written as `<request id>`. The frames `pushed` names for an op reach every listener
/// while it is under way, before its reply.
pub struct ScriptedHost {
    replies: IndexMap<String, String>,
    pushed: IndexMap<String, Vec<String>>,
    pub frames: Mutex<Vec<String>>,
    seen: Mutex<HashMap<String, u32>>,
    next: AtomicU64,
    listeners: Mutex<Vec<(u64, FrameSink)>>,
    subscribed: Mutex<bool>,
}

impl ScriptedHost {
    pub fn new(replies: IndexMap<String, String>, pushed: IndexMap<String, Vec<String>>) -> Arc<ScriptedHost> {
        Arc::new(ScriptedHost {
            replies,
            pushed,
            frames: Mutex::new(Vec::new()),
            seen: Mutex::new(HashMap::new()),
            next: AtomicU64::new(2),
            listeners: Mutex::new(Vec::new()),
            subscribed: Mutex::new(false),
        })
    }

    pub fn sent(&self) -> Vec<String> {
        self.frames.lock().unwrap().clone()
    }

    fn ask(&self, op: &str, params: Frame) -> Result<Frame, HostError> {
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        let nth = {
            let mut seen = self.seen.lock().unwrap();
            let n = seen.entry(op.to_string()).or_insert(0);
            *n += 1;
            *n
        };
        let mut sent = Map::new();
        for (k, v) in params {
            sent.insert(k, v);
        }
        sent.insert("id".into(), Value::from(id));
        sent.insert("op".into(), Value::from(op));
        if sent.contains_key("requestId") {
            sent.insert("requestId".into(), Value::from("<request id>"));
        }
        self.frames.lock().unwrap().push(serde_json::to_string(&Value::Object(sent)).unwrap());
        let answer = self.replies.get(&format!("{op} #{nth}")).or_else(|| self.replies.get(op)).cloned();
        let answer = answer.unwrap_or_else(|| {
            if op == "events.subscribe" {
                r#"{"id":1,"ok":true}"#.to_string()
            } else {
                serde_json::json!({ "ok": false, "error": format!("{op} {NOT_IN_RECORD}") }).to_string()
            }
        });
        let frame: Frame = serde_json::from_str(&answer).expect("reply is a JSON object");
        for pushed in self.pushed.get(op).cloned().unwrap_or_default() {
            let f: Frame = serde_json::from_str(&pushed).expect("pushed frame");
            let sinks: Vec<FrameSink> = self.listeners.lock().unwrap().iter().map(|(_, s)| s.clone()).collect();
            for sink in sinks {
                sink(&f);
            }
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

#[async_trait]
impl HostClient for ScriptedHost {
    async fn request(&self, op: &str, params: Frame) -> Result<Frame, HostError> {
        self.ask(op, params)
    }

    async fn events(&self) -> Result<(), HostError> {
        if std::mem::replace(&mut *self.subscribed.lock().unwrap(), true) {
            return Ok(());
        }
        self.ask("events.subscribe", Frame::new()).map(|_| ())
    }

    fn on_frame(&self, sink: FrameSink) -> FrameOff {
        let id = self.next.load(Ordering::SeqCst) * 1_000_000 + self.listeners.lock().unwrap().len() as u64;
        self.listeners.lock().unwrap().push((id, sink));
        FrameOff(Box::new(|| {}))
    }

    async fn closed(&self) -> Option<u16> {
        std::future::pending().await
    }

    fn gone(&self) -> Option<Option<u16>> {
        None
    }

    fn close_words(&self) -> String {
        "the host closed the connection".into()
    }

    fn paired(&self) -> Option<Paired> {
        None
    }

    fn close(&self) {}

    fn terminate(&self) {}
}

/// The dial a line runs through: always this one scripted host.
pub struct Dial(pub Arc<ScriptedHost>);

#[async_trait]
impl DialFn for Dial {
    async fn dial(&self, _state_path: &str, _opts: DialOpts) -> Result<Arc<dyn HostClient>, HostError> {
        Ok(self.0.clone())
    }
}

/// A terminal that keeps what the line printed. Off a tty any question is a failure, as the TS harness's; at a
/// scripted tty each question takes the next answer or secret and is written down.
#[derive(Default)]
pub struct Captured {
    pub stdout: Mutex<Vec<String>>,
    pub stderr: Mutex<Vec<String>>,
    pub streamed: Mutex<String>,
    pub tty: Option<Tty>,
    pub asked: Mutex<Vec<String>>,
    answers: Mutex<Vec<String>>,
    secrets: Mutex<Vec<String>>,
}

impl Captured {
    pub fn at(tty: Option<Tty>) -> Captured {
        let (answers, secrets) = tty.as_ref().map(|t| (t.answers.clone(), t.secrets.clone())).unwrap_or_default();
        Captured { tty, answers: Mutex::new(answers), secrets: Mutex::new(secrets), ..Captured::default() }
    }

    fn take(&self, from: &Mutex<Vec<String>>, question: &str) -> Result<String, HostError> {
        let mut queue = from.lock().unwrap();
        if self.tty.is_none() || queue.is_empty() {
            return Err(HostError::new(format!("unexpected prompt: {question}")));
        }
        self.asked.lock().unwrap().push(question.to_string());
        Ok(queue.remove(0))
    }
}

#[async_trait]
impl CliIo for Captured {
    fn log(&self, line: &str) {
        self.stdout.lock().unwrap().push(line.to_string());
    }

    fn error(&self, line: &str) {
        self.stderr.lock().unwrap().push(line.to_string());
    }

    fn stream(&self, text: &str) {
        self.streamed.lock().unwrap().push_str(text);
    }

    fn is_tty(&self) -> bool {
        self.tty.is_some()
    }

    async fn ask(&self, question: &str) -> Result<String, HostError> {
        self.take(&self.answers, question)
    }

    async fn ask_secret(&self, question: &str, _variable: Option<&str>) -> Result<String, HostError> {
        self.take(&self.secrets, question)
    }
}

/// Runs one recorded line through the Rust command line as runVerb does (verbs.ts:694), against the case's host,
/// in a fresh folder holding the case's files, with the case's environment and the cloud on where it says so.
/// The folder's path is written `<cwd>` in what comes back, as the recorder wrote it.
pub fn run_line(c: &Case, json: bool) -> Run {
    std::env::set_var("TZ", "UTC");
    let dir = tempfile::tempdir().expect("temp folder");
    for (name, text) in &c.files {
        std::fs::write(dir.path().join(name), text).expect("case file");
    }
    let cwd = dir.path().to_string_lossy().into_owned();
    let mut argv = c.argv.clone();
    if json {
        argv.push("--json".into());
    }
    let host = ScriptedHost::new(c.replies.clone(), c.pushed.clone());
    let io = Arc::new(Captured::at(c.tty.clone()));
    let table = verbs_for(c.cloud);
    let verb = find_verb(&table, &argv).unwrap_or_else(|| panic!("no verb for {argv:?}"));
    let deps = RunDeps { cwd: Some(cwd.clone()), env: c.env.clone(), dial: Some(Arc::new(Dial(host.clone()))), ..RunDeps::default() };
    let state = |_: Option<&str>| "/state/state.json".to_string();
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime");
    let exit = rt.block_on(run_verb(&table, verb, &argv, io.clone(), &state, deps));
    let scrub = |s: &str| s.replace(&cwd, "<cwd>");
    let streamed = scrub(&io.streamed.lock().unwrap());
    let run = Run {
        frames: host.sent().iter().map(|s| scrub(s)).collect(),
        stdout: io.stdout.lock().unwrap().iter().map(|s| scrub(s)).collect(),
        stderr: io.stderr.lock().unwrap().iter().map(|s| scrub(s)).collect(),
        streamed: if streamed.is_empty() { None } else { Some(streamed) },
        exit,
        asked: c.tty.as_ref().map(|_| io.asked.lock().unwrap().iter().map(|s| scrub(s)).collect()),
    };
    run
}

/// The case run both ways and held to the TS byte for byte: the frames sent, every stdout and stderr line, what was
/// streamed, and the exit code.
pub fn holds(c: &Case) {
    for (json, want) in [(false, &c.text), (true, &c.json)] {
        let got = run_line(c, json);
        let how = if json { "--json" } else { "text" };
        assert_eq!(got.frames, want.frames, "{} ({how}): frames sent", c.case);
        assert_eq!(got.stdout, want.stdout, "{} ({how}): stdout", c.case);
        assert_eq!(got.stderr, want.stderr, "{} ({how}): stderr", c.case);
        assert_eq!(got.streamed, want.streamed, "{} ({how}): streamed", c.case);
        assert_eq!(got.exit, want.exit, "{} ({how}): exit code", c.case);
        assert_eq!(got.asked, want.asked, "{} ({how}): the questions asked", c.case);
    }
}

/// The case's tool call through the verb's `call`, held to the TS server's answer: the content text, the
/// structured content key for key and in order, and `isError`; and the ops it asked, with their fields.
pub fn tool_holds(c: &Case) {
    let tool = c.tool.as_deref().expect("a tool case");
    let table = verbs_for(c.cloud);
    let entry = table.iter().find(|v| v.name().replace(' ', "_") == tool).unwrap_or_else(|| panic!("no entry for {tool}"));
    let t = has_tool(entry).unwrap_or_else(|| panic!("{tool} has no tool"));
    let host = ScriptedHost::new(c.replies.clone(), c.pushed.clone());
    let deps = tool_deps(c, host.clone());
    let args = match c.tool_arguments.clone() {
        Some(Value::Object(m)) => m,
        _ => Frame::new(),
    };
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime");
    let call = t.call;
    let answer: BoxFuture<'_, _> = call(args, &deps);
    let got = match rt.block_on(answer) {
        Ok(r) => serde_json::to_value(&r).expect("result serializes"),
        Err(e) => serde_json::to_value(tool_failure(&e, None)).expect("failure serializes"),
    };
    let want = c.tool_result.clone().expect("a recorded answer");
    assert_eq!(serde_json::to_string(&got).unwrap(), serde_json::to_string(&want).unwrap(), "{}: the tool's answer", c.case);
    if let Some(Value::Array(asked)) = &c.tool_asked {
        let sent: Vec<Value> = host
            .sent()
            .iter()
            .map(|s| {
                let mut f: Map<String, Value> = serde_json::from_str(s).unwrap();
                f.remove("id");
                let op = f.remove("op").unwrap();
                let mut o = Map::new();
                o.insert("op".into(), op);
                o.extend(f);
                Value::Object(o)
            })
            .collect();
        assert_eq!(sent.len(), asked.len(), "{}: ops asked {sent:?}", c.case);
        for (s, a) in sent.iter().zip(asked) {
            assert_eq!(s, a, "{}: an op's fields", c.case);
        }
    }
}

/// What `deps.client()` hands a tool call: always the case's scripted host.
pub struct Held(pub Arc<ScriptedHost>);

#[async_trait]
impl Dialer for Held {
    async fn client(&self, _again: Option<Again>) -> Result<Arc<dyn HostClient>, HostError> {
        Ok(self.0.clone())
    }
}

/// The deps a tool call runs with: the case's environment and the scripted host behind `deps.client()`.
pub fn tool_deps(c: &Case, host: Arc<ScriptedHost>) -> VerbDeps {
    let mut env = c.env.clone();
    if c.cloud {
        env.insert(wsp_wire::types::env::CLOUD_ENV.to_owned(), "1".to_owned());
    }
    VerbDeps {
        state_path: "/state/state.json".into(),
        cwd: None,
        env,
        dialer: Arc::new(Held(host)),
        host_wait_ms: None,
        start: None,
        signals: None,
        dial: None,
        elsewhere: false,
        terminal: None,
        open: None,
    }
}

/// A verb's `--help` page held to the TS page, with the cloud on or off.
pub fn page_holds(unit: &str, verb: &str, cloud: bool) {
    let key = format!("{} {verb}", if cloud { "cloud" } else { "local" });
    let want = pages(unit).get(&key).cloned().unwrap_or_else(|| panic!("no page {key}"));
    let c = Case {
        tool: None,
        case: key.clone(),
        hand: true,
        argv: verb.split(' ').map(String::from).chain(["--help".to_string()]).collect(),
        cloud,
        env: IndexMap::new(),
        files: IndexMap::new(),
        replies: IndexMap::new(),
        pushed: IndexMap::new(),
        tool_arguments: None,
        tool_asked: None,
        tool_result: None,
        tty: None,
        text: Run::default(),
        json: Run::default(),
    };
    let got = run_line(&c, false);
    assert_eq!(serde_json::to_value(&got.stdout).unwrap(), want["stdout"], "{key}: the page");
    assert_eq!(got.exit, 0, "{key}: exit code");
    assert!(got.frames.is_empty(), "{key}: a page dials nothing");
}
