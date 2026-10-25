// SPDX-License-Identifier: AGPL-3.0-only
//! What every wsp-cli test file shares: the TS oracle's fixtures, a host scripted the way the tool server's record
//! scripts one (daemon/crates/wsp-mcp/tests/common), a dialer over it, and a terminal that records what a line said.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::watch;

use wsp_cli::verbs::client::{
    Again, BoxFuture, CliIo, Dialer, Frame, FrameOff, FrameSink, HostClient, Out, Paired, VerbContext, VerbDeps,
};
use wsp_cli::hosts::HostAim;
use wsp_words::exit::HostError;

/// The close code a host stopping sends (`HOST_STOPPING_CLOSE`).
pub const STOPPING_CLOSE: u16 = 4001;

/// A fixture the TS oracle wrote (`oracle/cli.oracle.ts`), with the cloud on where `cloud` says and a `_cloud` file
/// exists, else the one file both runs wrote alike.
pub fn fixture(name: &str, cloud: bool) -> Value {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures");
    let cloudy = dir.join(format!("{name}_cloud.json"));
    let path = if cloud && cloudy.exists() { cloudy } else { dir.join(format!("{name}.json")) };
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))).unwrap()
}

/// A JSON array of strings as a Vec.
pub fn strings(v: &Value) -> Vec<String> {
    v.as_array().unwrap_or_else(|| panic!("not an array: {v}")).iter().map(|s| s.as_str().unwrap().to_owned()).collect()
}

// ---------------------------------------------------------------------------------------------------------------
// The scripted host.
// ---------------------------------------------------------------------------------------------------------------

/// What the host answers, as the tool server's record writes it: each op's reply frame recorded under id 1 (or
/// `<op> #<n>` for its nth ask), the frames pushed ahead of an op's reply, and the op whose reply the host follows
/// by closing with 4001.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Script {
    #[serde(default)]
    pub replies: BTreeMap<String, String>,
    #[serde(default)]
    pub pushed: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub closes: Option<String>,
}

/// The state every socket of one scripted host shares: what it was asked, how often each op was asked, the dials.
#[derive(Default)]
pub struct HostState {
    pub asked: Mutex<Vec<Value>>,
    pub seen: Mutex<BTreeMap<String, usize>>,
    pub dials: AtomicUsize,
    pub closes_called: AtomicUsize,
    pub subscribes: AtomicUsize,
}

/// One socket to the scripted host, in process: no websocket, the same ordering. A request records its frame (the
/// params, then `op`; no id; a `requestId` as `<request id>`; never `events.subscribe`), sends every frame pushed
/// for that op to the sinks, then answers the recorded reply under its own id; where `closes` names the op the
/// socket is then gone with 4001, and every later request fails with the close words.
pub struct ScriptedClient {
    script: Script,
    state: Arc<HostState>,
    next: AtomicUsize,
    sinks: Arc<Mutex<Vec<(usize, FrameSink)>>>,
    sink_ids: AtomicUsize,
    gone: watch::Sender<Option<Option<u16>>>,
    subscribed: Mutex<bool>,
}

impl ScriptedClient {
    pub fn new(script: Script, state: Arc<HostState>) -> Arc<ScriptedClient> {
        Arc::new(ScriptedClient {
            script,
            state,
            next: AtomicUsize::new(1),
            sinks: Arc::new(Mutex::new(Vec::new())),
            sink_ids: AtomicUsize::new(0),
            gone: watch::channel(None).0,
            subscribed: Mutex::new(false),
        })
    }

    /// Ends the socket as a host does, with the code given (None: no code, as a dropped connection).
    pub fn end(&self, code: Option<u16>) {
        self.gone.send_replace(Some(code));
    }

    /// Sends a frame to every sink, as the host pushing an event.
    pub fn push(&self, frame: &str) {
        let frame: Frame = serde_json::from_str(frame).unwrap();
        let sinks: Vec<FrameSink> = self.sinks.lock().unwrap().iter().map(|(_, s)| s.clone()).collect();
        for sink in sinks {
            sink(&frame);
        }
    }

    fn reply_for(&self, op: &str, id: usize) -> Frame {
        let nth = {
            let mut seen = self.state.seen.lock().unwrap();
            let count = seen.entry(op.to_owned()).or_insert(0);
            *count += 1;
            *count
        };
        let recorded = self.script.replies.get(&format!("{op} #{nth}")).or_else(|| self.script.replies.get(op));
        let text = match recorded {
            Some(recorded) => {
                let rest = recorded.strip_prefix(r#"{"id":1,"#).expect("a frame recorded with its id first");
                format!(r#"{{"id":{id},{rest}"#)
            }
            None if op == "events.subscribe" || op == "auth" => json!({ "id": id, "ok": true }).to_string(),
            None => json!({ "id": id, "ok": false, "error": format!("{op} is not in this record") }).to_string(),
        };
        serde_json::from_str(&text).unwrap()
    }
}

/// The failure a reply with `ok !== true` is read as (client.ts:266), for the scripted socket.
fn refused(op: &str, frame: &Frame) -> HostError {
    let mut e = HostError::new(frame.get("error").and_then(Value::as_str).map(str::to_owned).unwrap_or_else(|| format!("{op} failed")));
    if let Some(kind) = frame.get("kind").and_then(Value::as_str) {
        e.kind = Some(serde_json::from_value(json!(kind)).unwrap());
    }
    e.errors = frame.get("errors").and_then(Value::as_array).cloned();
    e.warnings = frame.get("warnings").and_then(Value::as_array).cloned();
    e
}

#[async_trait]
impl HostClient for ScriptedClient {
    async fn request(&self, op: &str, params: Frame) -> Result<Frame, HostError> {
        if self.gone.borrow().is_some() {
            return Err(HostError::new(self.close_words()));
        }
        let id = self.next.fetch_add(1, Ordering::SeqCst);
        if op != "events.subscribe" && op != "auth" {
            let mut frame = params.clone();
            frame.shift_remove("id");
            frame.shift_remove("op");
            frame.insert("op".to_owned(), json!(op));
            if frame.contains_key("requestId") {
                frame.insert("requestId".to_owned(), json!("<request id>"));
            }
            self.state.asked.lock().unwrap().push(Value::Object(frame));
        }
        let reply = self.reply_for(op, id);
        for frame in self.script.pushed.get(op).into_iter().flatten() {
            self.push(frame);
        }
        if self.script.closes.as_deref() == Some(op) {
            // The reply went out ahead of the close, as the record's host sends it.
            let answer = if reply.get("ok") == Some(&json!(true)) { Ok(reply) } else { Err(refused(op, &reply)) };
            self.end(Some(STOPPING_CLOSE));
            return answer;
        }
        tokio::task::yield_now().await;
        if reply.get("ok") == Some(&json!(true)) {
            Ok(reply)
        } else {
            Err(refused(op, &reply))
        }
    }

    async fn events(&self) -> Result<(), HostError> {
        let first = {
            let mut subscribed = self.subscribed.lock().unwrap();
            !std::mem::replace(&mut *subscribed, true)
        };
        if first {
            self.state.subscribes.fetch_add(1, Ordering::SeqCst);
            self.request("events.subscribe", Frame::new()).await?;
        }
        Ok(())
    }

    fn on_frame(&self, sink: FrameSink) -> FrameOff {
        let id = self.sink_ids.fetch_add(1, Ordering::SeqCst);
        self.sinks.lock().unwrap().push((id, sink));
        let sinks = self.sinks.clone();
        FrameOff(Box::new(move || sinks.lock().unwrap().retain(|(n, _)| *n != id)))
    }

    async fn closed(&self) -> Option<u16> {
        let mut rx = self.gone.subscribe();
        loop {
            if let Some(code) = *rx.borrow() {
                return code;
            }
            if rx.changed().await.is_err() {
                return None;
            }
        }
    }

    fn gone(&self) -> Option<Option<u16>> {
        *self.gone.borrow()
    }

    fn close_words(&self) -> String {
        match *self.gone.borrow() {
            Some(Some(STOPPING_CLOSE)) => wsp_words::words::thread::lines::HOST_STOPPING_LINE.to_owned(),
            _ => wsp_wire::types::daemon_contract::HOST_CLOSED_LINE.to_owned(),
        }
    }

    fn paired(&self) -> Option<Paired> {
        None
    }

    fn close(&self) {
        self.state.closes_called.fetch_add(1, Ordering::SeqCst);
        if self.gone.borrow().is_none() {
            self.end(None);
        }
    }

    fn terminate(&self) {
        self.close();
    }
}

/// The scripted host behind a dialer: every dial a new socket on the same script and the same record of asks.
pub struct ScriptedHost {
    pub script: Script,
    pub state: Arc<HostState>,
    pub sockets: Mutex<Vec<Arc<ScriptedClient>>>,
    /// Dials that fail with this error instead of opening a socket, counted down.
    pub refuse_dials: Mutex<Vec<HostError>>,
}

impl ScriptedHost {
    pub fn new(script: Script) -> Arc<ScriptedHost> {
        Arc::new(ScriptedHost { script, state: Arc::default(), sockets: Mutex::new(Vec::new()), refuse_dials: Mutex::new(Vec::new()) })
    }

    pub fn replying(replies: &[(&str, Value)]) -> Arc<ScriptedHost> {
        let mut script = Script::default();
        for (op, frame) in replies {
            let mut frame = frame.as_object().unwrap().clone();
            frame.shift_remove("id");
            let mut text = String::from(r#"{"id":1,"#);
            let rest = Value::Object(frame).to_string();
            text.push_str(&rest[1..]);
            script.replies.insert((*op).to_owned(), text);
        }
        ScriptedHost::new(script)
    }

    /// What the host was asked across every socket, in order, each frame its params then `op`.
    pub fn asked(&self) -> Vec<Value> {
        self.state.asked.lock().unwrap().clone()
    }

    pub fn ops(&self) -> Vec<String> {
        self.asked().iter().map(|f| f["op"].as_str().unwrap().to_owned()).collect()
    }

    pub fn dials(&self) -> usize {
        self.state.dials.load(Ordering::SeqCst)
    }

    pub fn socket(&self, n: usize) -> Arc<ScriptedClient> {
        self.sockets.lock().unwrap()[n].clone()
    }
}

#[async_trait]
impl Dialer for ScriptedHost {
    async fn client(&self, _again: Option<Again>) -> Result<Arc<dyn HostClient>, HostError> {
        self.state.dials.fetch_add(1, Ordering::SeqCst);
        if let Some(e) = self.refuse_dials.lock().unwrap().pop() {
            return Err(e);
        }
        let socket = ScriptedClient::new(self.script.clone(), self.state.clone());
        self.sockets.lock().unwrap().push(socket.clone());
        Ok(socket)
    }
}

/// A dialer that hands back the same socket on every plain dial, as a verb's own `client()` does, and a new one on
/// a dial with `again`.
pub struct OneSocket(pub Arc<ScriptedHost>, pub Mutex<Option<Arc<dyn HostClient>>>);

#[async_trait]
impl Dialer for OneSocket {
    async fn client(&self, again: Option<Again>) -> Result<Arc<dyn HostClient>, HostError> {
        if again.is_none() {
            if let Some(c) = self.1.lock().unwrap().clone() {
                return Ok(c);
            }
        }
        let c = self.0.client(again).await?;
        *self.1.lock().unwrap() = Some(c.clone());
        Ok(c)
    }
}

/// The deps a tool call is handed: this state, the environment given, the scripted host as its dialer.
pub fn deps_on(host: Arc<ScriptedHost>, env: &[(&str, &str)]) -> VerbDeps {
    VerbDeps {
        state_path: "/nowhere/state.json".to_owned(),
        cwd: Some("/root".to_owned()),
        env: env.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect::<IndexMap<_, _>>(),
        dialer: Arc::new(OneSocket(host, Mutex::new(None))),
        host_wait_ms: Some(1_000),
        start: None,
        signals: None,
        dial: None,
        elsewhere: false,
        terminal: None,
        open: None,
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The terminal.
// ---------------------------------------------------------------------------------------------------------------

/// A terminal that records every line: stdout's, stderr's, the raw stream, and each question asked.
#[derive(Default)]
pub struct RecordingIo {
    pub lines: Mutex<Vec<String>>,
    pub errors: Mutex<Vec<String>>,
    pub streamed: Mutex<Vec<String>>,
    pub asked: Mutex<Vec<String>>,
    pub tty: bool,
    pub answer: String,
}

impl RecordingIo {
    pub fn new() -> Arc<RecordingIo> {
        Arc::new(RecordingIo { answer: "no".to_owned(), ..RecordingIo::default() })
    }

    pub fn at_terminal(answer: &str) -> Arc<RecordingIo> {
        Arc::new(RecordingIo { tty: true, answer: answer.to_owned(), ..RecordingIo::default() })
    }

    pub fn lines(&self) -> Vec<String> {
        self.lines.lock().unwrap().clone()
    }

    pub fn errors(&self) -> Vec<String> {
        self.errors.lock().unwrap().clone()
    }

    pub fn streamed(&self) -> Vec<String> {
        self.streamed.lock().unwrap().clone()
    }

    pub fn asked(&self) -> Vec<String> {
        self.asked.lock().unwrap().clone()
    }
}

#[async_trait]
impl CliIo for RecordingIo {
    fn log(&self, line: &str) {
        self.lines.lock().unwrap().push(line.to_owned());
    }
    fn error(&self, line: &str) {
        self.errors.lock().unwrap().push(line.to_owned());
    }
    fn stream(&self, text: &str) {
        self.streamed.lock().unwrap().push(text.to_owned());
    }
    async fn ask(&self, question: &str) -> Result<String, HostError> {
        self.asked.lock().unwrap().push(question.to_owned());
        Ok(self.answer.clone())
    }
    fn is_tty(&self) -> bool {
        self.tty
    }
    async fn ask_secret(&self, question: &str, _variable: Option<&str>) -> Result<String, HostError> {
        self.asked.lock().unwrap().push(question.to_owned());
        Ok(String::new())
    }
}

/// A verb's context over the scripted host: the parsed line, the terminal, `--json` where the flags say.
pub fn context(host: Arc<ScriptedHost>, io: Arc<RecordingIo>, args: &[&str], flags: wsp_cli::verbs::client::Flags, usage: &str) -> VerbContext {
    let json = matches!(flags.get("json"), Some(wsp_cli::verbs::client::FlagValue::Bool(true)));
    VerbContext {
        deps: deps_on(host, &[]),
        args: args.iter().map(|s| (*s).to_owned()).collect(),
        flags,
        io: io.clone(),
        out: Out::formatter(io, json),
        aim: HostAim::Here,
        usage: usage.to_owned(),
    }
}

/// A future boxed for a `DialWithin`.
pub fn boxed<'a, T>(f: impl std::future::Future<Output = T> + Send + 'a) -> BoxFuture<'a, T> {
    Box::pin(f)
}

// ---------------------------------------------------------------------------------------------------------------
// The tool server's record (daemon/crates/wsp-mcp/tests/answers), replayed against a verb's tool call.
// ---------------------------------------------------------------------------------------------------------------

#[derive(Deserialize, Clone)]
pub struct Answers {
    pub tool: String,
    pub cases: Vec<Case>,
}

#[derive(Deserialize, Clone)]
pub struct Case {
    pub case: String,
    pub arguments: Value,
    #[serde(default)]
    pub replies: BTreeMap<String, String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub pushed: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub closes: Option<String>,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub cloud: bool,
    pub line: String,
    pub asked: Vec<Value>,
    #[serde(default)]
    pub guest: Option<Answered>,
}

#[derive(Deserialize, Clone)]
pub struct Answered {
    pub line: String,
    pub asked: Vec<Value>,
}

/// The recorded answers of one tool.
pub fn answers(tool: &str) -> Answers {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../daemon/crates/wsp-mcp/tests/answers").join(format!("{tool}.json"));
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))).unwrap()
}

/// The line the TS server printed for an answer: `{"result":<answer>,"jsonrpc":"2.0","id":1}` through `jsonLine`.
pub fn server_line(answer: &wsp_cli::verbs::client::CallToolResult) -> String {
    let result = serde_json::to_value(answer).unwrap();
    let line = json!({ "result": result, "jsonrpc": "2.0", "id": 1 });
    wsp_words::exit::json_line(&line, None)
}

/// The asks sorted by their text, as the record's own replay compares them (the order two requests sent together
/// reach the host is not the code's to fix).
pub fn in_order(mut frames: Vec<Value>) -> Vec<Value> {
    frames.sort_by_key(Value::to_string);
    frames
}
