// SPDX-License-Identifier: AGPL-3.0-only
//! Scripted links for the pty and place tests: a daemon link whose ptys are scripted (packages/host/test/
//! fake-pty-link.ts at 65f6b89c7), a local terminal that records, and a host client whose replies a test plays.
#![allow(dead_code)]
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use base64::Engine;
use futures::future::BoxFuture;
use futures::FutureExt;
use serde_json::{json, Value};
use tokio::sync::{mpsc, watch};
use wsp_cli::signin_relay::{Detach, EventSink, Frame, PtyLink, RawMode, RelayOutput, RelayTerminal, TOOL_STARTS};
use wsp_cli::verbs::client::{FrameOff, FrameSink, HostClient};
use wsp_words::exit::HostError;

pub fn frame(v: Value) -> Frame {
    match v {
        Value::Object(m) => m,
        other => panic!("not an object: {other}"),
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FakePty {
    pub id: String,
    pub created: Frame,
    pub writes: Vec<String>,
    pub attached: bool,
    pub killed: bool,
    pub resizes: Vec<(i64, i64)>,
    pub exited: bool,
    /// The staged command the typed line ran, once one did.
    pub ran: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StagedCmd {
    pub command: String,
    pub cleared: bool,
}

type Script = Arc<dyn Fn(&FakeLink, &str, &str) + Send + Sync>;
type AnswerExec = Arc<dyn Fn(&str) -> Option<Frame> + Send + Sync>;
type OpOverride = Arc<dyn Fn(&str, &Frame) -> Option<Frame> + Send + Sync>;

#[derive(Default)]
struct State {
    ops: Vec<(String, Frame)>,
    staged: BTreeMap<String, StagedCmd>,
    ptys: Vec<FakePty>,
    partial: BTreeMap<String, String>,
    seq: u32,
    folders: u32,
    sinks: Vec<(u64, EventSink)>,
    next_sink: u64,
}

/// fake-pty-link.ts:53-157. Every op is recorded; a complete typed line (ending in \r) is echoed back as a tty
/// would, followed by the tool-start byte when the line prints it, then handed to the script with a staged
/// command standing where the line reads it.
pub struct FakeLink {
    state: Mutex<State>,
    pub banner: Mutex<Option<String>>,
    pub script: Mutex<Option<Script>>,
    pub answer_exec: Mutex<Option<AnswerExec>>,
    /// Answers an op in place of the fake (a refusal, say); the op is still recorded.
    pub override_op: Mutex<Option<OpOverride>>,
    pub bare: Option<bool>,
    closed: Mutex<Option<watch::Sender<bool>>>,
    views: Mutex<Vec<Arc<View>>>,
}

impl FakeLink {
    pub fn new() -> Arc<FakeLink> {
        FakeLink::with_bare(None)
    }

    pub fn with_bare(bare: Option<bool>) -> Arc<FakeLink> {
        Arc::new(FakeLink {
            state: Mutex::default(),
            banner: Mutex::new(None),
            script: Mutex::new(None),
            answer_exec: Mutex::new(None),
            override_op: Mutex::new(None),
            bare,
            closed: Mutex::new(None),
            views: Mutex::new(Vec::new()),
        })
    }

    pub fn set_script(&self, f: impl Fn(&FakeLink, &str, &str) + Send + Sync + 'static) {
        *self.script.lock().unwrap() = Some(Arc::new(f));
    }

    /// Gives this link a `closed` the test settles with the returned sender (`link.closed = new Promise(..)`).
    pub fn closable(&self) -> watch::Sender<bool> {
        let (tx, _rx) = watch::channel(false);
        *self.closed.lock().unwrap() = Some(tx.clone());
        tx
    }

    pub fn ops(&self) -> Vec<(String, Frame)> {
        self.state.lock().unwrap().ops.clone()
    }

    pub fn op_names(&self) -> Vec<String> {
        self.ops().into_iter().map(|(op, _)| op).collect()
    }

    pub fn ptys(&self) -> Vec<FakePty> {
        self.state.lock().unwrap().ptys.clone()
    }

    pub fn staged(&self) -> Vec<StagedCmd> {
        self.state.lock().unwrap().staged.values().cloned().collect()
    }

    /// The execs a caller ran, less the ones that staged a command or removed it (fake-pty-link.ts:160-162).
    pub fn execs_beside(&self) -> Vec<Frame> {
        self.ops()
            .into_iter()
            .filter(|(op, extra)| {
                op == "exec"
                    && !extra.contains_key("stdin")
                    && !extra.get("cmd").and_then(Value::as_str).unwrap_or("").starts_with("rm -rf -- '/staged/")
            })
            .map(|(_, extra)| extra)
            .collect()
    }

    pub fn emit(&self, e: Frame) {
        let sinks: Vec<EventSink> = self.state.lock().unwrap().sinks.iter().map(|(_, s)| s.clone()).collect();
        for s in sinks {
            s(&e);
        }
    }

    pub fn data(&self, pty: &str, text: &str) {
        self.emit(frame(json!({"type": "pty.data", "ptyId": pty, "data": text})));
    }

    pub fn exit(&self, pty: &str, code: i64) {
        if let Some(p) = self.state.lock().unwrap().ptys.iter_mut().find(|p| p.id == pty) {
            p.exited = true;
        }
        self.emit(frame(json!({"type": "pty.exit", "ptyId": pty, "exitCode": code})));
    }

    /// A fresh view over the same ptys, the way a redial gives one; its closed settles on `drop_view`.
    pub fn dial(self: &Arc<FakeLink>) -> Arc<View> {
        let (tx, _rx) = watch::channel(false);
        let view = Arc::new(View { link: self.clone(), dropped: Mutex::new(false), closed: tx });
        self.views.lock().unwrap().push(view.clone());
        view
    }

    /// The latest view goes away: closed settles and its ops fail as a dead socket's do.
    pub fn drop_view(&self) {
        let view = self.views.lock().unwrap().last().cloned().expect("nothing dialled");
        *view.dropped.lock().unwrap() = true;
        let _ = view.closed.send(true);
    }

    fn run_op(&self, op: &str, extra: Frame) -> Result<Frame, HostError> {
        self.state.lock().unwrap().ops.push((op.to_string(), extra.clone()));
        if let Some(f) = self.override_op.lock().unwrap().clone() {
            if let Some(answer) = f(op, &extra) {
                return Ok(answer);
            }
        }
        let pty_id = extra.get("ptyId").and_then(Value::as_str).unwrap_or("").to_string();
        match op {
            "pty.create" => {
                let mut st = self.state.lock().unwrap();
                st.seq += 1;
                let id = format!("pty_{}", st.seq);
                st.ptys.push(FakePty { id: id.clone(), created: extra, ..FakePty::default() });
                Ok(frame(json!({"ok": true, "ptyId": id, "pid": 100 + st.seq})))
            }
            "pty.attach" => {
                self.with_pty(&pty_id, |p| p.attached = true)?;
                if let Some(b) = self.banner.lock().unwrap().clone() {
                    self.data(&pty_id, &b);
                }
                Ok(frame(json!({"ok": true, "ptyId": pty_id})))
            }
            "pty.write" => {
                let s = extra.get("data").and_then(Value::as_str).unwrap_or("").to_string();
                self.with_pty(&pty_id, |p| p.writes.push(s.clone()))?;
                let lines: Vec<String> = {
                    let mut st = self.state.lock().unwrap();
                    let buf = format!("{}{s}", st.partial.get(&pty_id).cloned().unwrap_or_default());
                    let mut parts: Vec<String> = buf.split('\r').map(String::from).collect();
                    let rest = parts.pop().unwrap_or_default();
                    st.partial.insert(pty_id.clone(), rest);
                    parts
                };
                let staged_read = regex::Regex::new(r"^c=\$\((?:< |cat )'([^']+)/line'\); rm -rf -- '[^']+'; ").unwrap();
                for line in lines {
                    self.data(&pty_id, &format!("{line}\r\n"));
                    let read = staged_read.captures(&line).map(|c| (c.get(0).unwrap().end(), c[1].to_string()));
                    let held = read.as_ref().and_then(|(_, dir)| self.state.lock().unwrap().staged.get(dir).cloned());
                    if let Some(h) = &held {
                        self.with_pty(&pty_id, |p| p.ran = Some(h.command.clone()))?;
                    }
                    if line.contains("printf '\\036'") {
                        self.data(&pty_id, TOOL_STARTS);
                    }
                    let handed = match (&held, &read) {
                        (Some(h), Some((end, _))) => line[*end..].replace("eval \"$c\"", &h.command).replace("\"$c\"", &h.command),
                        _ => line.clone(),
                    };
                    if let Some(script) = self.script.lock().unwrap().clone() {
                        script(self, &pty_id, &handed);
                    }
                }
                Ok(frame(json!({"ok": true})))
            }
            "pty.resize" => {
                let (c, r) =
                    (extra.get("cols").and_then(Value::as_i64).unwrap_or(-1), extra.get("rows").and_then(Value::as_i64).unwrap_or(-1));
                self.with_pty(&pty_id, |p| p.resizes.push((c, r)))?;
                Ok(frame(json!({"ok": true})))
            }
            "exec" => {
                if let Some(stdin) = extra.get("stdin").and_then(Value::as_str) {
                    let command = String::from_utf8(base64::engine::general_purpose::STANDARD.decode(stdin).unwrap()).unwrap();
                    let mut st = self.state.lock().unwrap();
                    st.folders += 1;
                    let dir = format!("/staged/{}", st.folders);
                    st.staged.insert(dir.clone(), StagedCmd { command, cleared: false });
                    return Ok(frame(json!({"exitCode": 0, "stdout": dir, "stderr": "", "truncated": false})));
                }
                let cmd = extra.get("cmd").and_then(Value::as_str).unwrap_or("").to_string();
                let cleared = regex::Regex::new(r"^rm -rf -- '(/staged/\d+)'$").unwrap().captures(&cmd).map(|c| c[1].to_string());
                if let Some(dir) = &cleared {
                    if let Some(s) = self.state.lock().unwrap().staged.get_mut(dir) {
                        s.cleared = true;
                    }
                }
                if cleared.is_none() {
                    if let Some(f) = self.answer_exec.lock().unwrap().clone() {
                        if let Some(answer) = f(&cmd) {
                            return Ok(answer);
                        }
                    }
                }
                Ok(frame(json!({"ok": true, "exitCode": 0, "stdout": "", "stderr": "", "truncated": false})))
            }
            "pty.kill" => {
                self.with_pty(&pty_id, |p| p.killed = true)?;
                Ok(frame(json!({"ok": true})))
            }
            _ => Ok(frame(json!({"ok": true}))),
        }
    }

    fn with_pty(&self, id: &str, f: impl FnOnce(&mut FakePty)) -> Result<(), HostError> {
        let mut st = self.state.lock().unwrap();
        match st.ptys.iter_mut().find(|p| p.id == id) {
            Some(p) => {
                f(p);
                Ok(())
            }
            None => Err(HostError::new(format!("no pty {id}"))),
        }
    }

    fn add_sink(&self, sink: EventSink) -> u64 {
        let mut st = self.state.lock().unwrap();
        st.next_sink += 1;
        let id = st.next_sink;
        st.sinks.push((id, sink));
        id
    }

    fn remove_sink(&self, id: u64) {
        self.state.lock().unwrap().sinks.retain(|(i, _)| *i != id);
    }

    fn closed_future(&self) -> Option<BoxFuture<'static, ()>> {
        let tx = self.closed.lock().unwrap().clone()?;
        let mut rx = tx.subscribe();
        Some(
            async move {
                let _ = rx.wait_for(|v| *v).await;
            }
            .boxed(),
        )
    }
}

/// A shared handle to the fake, so a test keeps the link it hands to the code under test.
#[derive(Clone)]
pub struct Link(pub Arc<FakeLink>);

#[async_trait]
impl PtyLink for Link {
    async fn op(&self, op: &str, extra: Frame) -> Result<Frame, HostError> {
        self.0.run_op(op, extra)
    }
    fn on_event(&self, sink: EventSink) -> Detach {
        let id = self.0.add_sink(sink);
        let link = self.0.clone();
        Detach(Box::new(move || link.remove_sink(id)))
    }
    fn closed(&self) -> Option<BoxFuture<'static, ()>> {
        self.0.closed_future()
    }
    fn bare(&self) -> Option<bool> {
        self.0.bare
    }
}

pub struct View {
    link: Arc<FakeLink>,
    dropped: Mutex<bool>,
    closed: watch::Sender<bool>,
}

#[async_trait]
impl PtyLink for View {
    async fn op(&self, op: &str, extra: Frame) -> Result<Frame, HostError> {
        if *self.dropped.lock().unwrap() {
            return Err(HostError::new("daemon connection closed 1006"));
        }
        self.link.run_op(op, extra)
    }
    fn on_event(&self, sink: EventSink) -> Detach {
        let id = self.link.add_sink(sink);
        let link = self.link.clone();
        Detach(Box::new(move || link.remove_sink(id)))
    }
    fn closed(&self) -> Option<BoxFuture<'static, ()>> {
        let mut rx = self.closed.subscribe();
        Some(
            async move {
                let _ = rx.wait_for(|v| *v).await;
            }
            .boxed(),
        )
    }
    fn bare(&self) -> Option<bool> {
        None
    }
}

/// Waits for the first pty the code under test creates.
pub async fn first_pty(link: &FakeLink) -> FakePty {
    for _ in 0..200 {
        if let Some(p) = link.ptys().first() {
            return p.clone();
        }
        tick().await;
    }
    panic!("no pty was created");
}

pub async fn tick() {
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
}

/// A local terminal that records: what went out, every raw mode set, the size, and the ends a test drives.
pub struct Term {
    pub keys: mpsc::UnboundedSender<String>,
    pub resize: mpsc::UnboundedSender<()>,
    pub out: Arc<Out>,
    pub raw: Arc<Mutex<Vec<bool>>>,
}

pub struct Out {
    pub text: Mutex<String>,
    pub size: Mutex<(Option<u16>, Option<u16>)>,
}

impl RelayOutput for Out {
    fn write(&self, text: &str) {
        self.text.lock().unwrap().push_str(text);
    }
    fn columns(&self) -> Option<u16> {
        self.size.lock().unwrap().0
    }
    fn rows(&self) -> Option<u16> {
        self.size.lock().unwrap().1
    }
}

struct Raw {
    on: bool,
    seen: Arc<Mutex<Vec<bool>>>,
}

impl RawMode for Raw {
    fn is_raw(&self) -> bool {
        self.on
    }
    fn set_raw_mode(&mut self, on: bool) {
        self.on = on;
        self.seen.lock().unwrap().push(on);
    }
}

impl Term {
    pub fn text(&self) -> String {
        self.out.text.lock().unwrap().clone()
    }
    pub fn type_keys(&self, s: &str) {
        self.keys.send(s.to_string()).unwrap();
    }
    pub fn raw_seen(&self) -> Vec<bool> {
        self.raw.lock().unwrap().clone()
    }
    pub fn set_size(&self, cols: u16, rows: u16) {
        *self.out.size.lock().unwrap() = (Some(cols), Some(rows));
    }
}

/// A terminal at 120 by 40 (signin-relay.test.ts:22-56), a tty unless told otherwise.
pub fn terminal(tty: bool, cols: u16, rows: u16) -> (Term, RelayTerminal) {
    let (keys, input) = mpsc::unbounded_channel();
    let (resize, resized) = mpsc::unbounded_channel();
    let out = Arc::new(Out { text: Mutex::new(String::new()), size: Mutex::new((Some(cols), Some(rows))) });
    let raw = Arc::new(Mutex::new(Vec::new()));
    let relay = RelayTerminal {
        input,
        raw: if tty { Some(Box::new(Raw { on: false, seen: raw.clone() })) } else { None },
        output: out.clone(),
        resized,
    };
    (Term { keys, resize, out, raw }, relay)
}

pub fn strip_vt(s: &str) -> String {
    let re = regex::Regex::new(r"\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1b\[[0-9;?]*[ -/]*[@-~]|\x1b[@-Z\\-_]").unwrap();
    re.replace_all(s, "").into_owned()
}

/// A host client whose replies the test plays (place-signin.test.ts:107-131, places-add.test.ts:39-95).
pub struct FakeClient {
    pub sent: Mutex<Vec<(String, Frame)>>,
    pub answer: Mutex<Box<dyn FnMut(&str, &Frame) -> Result<Frame, HostError> + Send>>,
    sinks: Mutex<Vec<(u64, FrameSink)>>,
    next: Mutex<u64>,
    pub events_called: Mutex<u32>,
}

impl FakeClient {
    pub fn new(answer: impl FnMut(&str, &Frame) -> Result<Frame, HostError> + Send + 'static) -> Arc<FakeClient> {
        Arc::new(FakeClient {
            sent: Mutex::default(),
            answer: Mutex::new(Box::new(answer)),
            sinks: Mutex::default(),
            next: Mutex::new(0),
            events_called: Mutex::new(0),
        })
    }
    pub fn push(&self, f: Value) {
        let f = frame(f);
        let sinks: Vec<FrameSink> = self.sinks.lock().unwrap().iter().map(|(_, s)| s.clone()).collect();
        for s in sinks {
            s(&f);
        }
    }
    pub fn sent(&self) -> Vec<(String, Frame)> {
        self.sent.lock().unwrap().clone()
    }
    pub fn listeners(&self) -> usize {
        self.sinks.lock().unwrap().len()
    }
}

/// The client as the code under test holds it.
#[derive(Clone)]
pub struct Client(pub Arc<FakeClient>);

#[async_trait]
impl HostClient for Client {
    async fn request(&self, op: &str, params: Frame) -> Result<Frame, HostError> {
        self.0.sent.lock().unwrap().push((op.to_string(), params.clone()));
        let answer = (self.0.answer.lock().unwrap())(op, &params);
        answer
    }
    async fn events(&self) -> Result<(), HostError> {
        *self.0.events_called.lock().unwrap() += 1;
        Ok(())
    }
    fn on_frame(&self, sink: FrameSink) -> FrameOff {
        let id = {
            let mut n = self.0.next.lock().unwrap();
            *n += 1;
            *n
        };
        self.0.sinks.lock().unwrap().push((id, sink));
        let me = self.0.clone();
        FrameOff(Box::new(move || me.sinks.lock().unwrap().retain(|(i, _)| *i != id)))
    }
    async fn closed(&self) -> Option<u16> {
        futures::future::pending::<()>().await;
        None
    }
    fn gone(&self) -> Option<Option<u16>> {
        None
    }
    fn close_words(&self) -> String {
        String::new()
    }
    fn paired(&self) -> Option<wsp_cli::verbs::client::Paired> {
        None
    }
    fn close(&self) {}
    fn terminate(&self) {}
}
