// SPDX-License-Identifier: AGPL-3.0-only
//! `cursor-agent -p --output-format stream-json` read into the runtime's events. TS adapter-cursor/src/adapter.ts. Spec:
//! spec/cursor.md rules 4-13.

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use futures::channel::oneshot;
use futures::future::{FutureExt, LocalBoxFuture};
use futures::stream::StreamExt;
use indexmap::IndexMap;
use serde_json::Value;

use crate::claude::fields::{num, parse_line, rec, string, Obj};
use crate::claude::turn::{fire, INTERRUPT_GRACE_MS, RUN_EXIT_MS};
use crate::cursor::command::{build_command, build_env, BuildCommandOptions, CursorEnvOptions};
use crate::pending::{refused_turn, Refusal};
use crate::port::{
    end_after_result, end_run, started, AdapterAttachOptions, AdapterError, AdapterEvent, AgentLaunch, AttachRunOptions, Attached,
    DeltaKind, Env, ExecFailure, ExecOptions, ExecStream, ExecStreamFactory, HarnessAdapter, HarnessSession, HarnessStartOptions, OnEvent,
    SharedFuture, TurnDelta, TurnResult, TurnStatus, TurnTokens,
};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::session::TurnRefusal;
use wsp_words::js::{js_key_order, js_trim};

/// TS adapter.ts:36.
#[derive(Clone)]
pub struct CursorAdapterDeps {
    pub exec: Arc<dyn ExecStreamFactory>,
    pub base_env: Option<IndexMap<String, Option<String>>>,
    /// wsp's half of a turn the CLI refuses for want of a credential.
    pub sign_in_refusal: Option<String>,
    pub api_key: Option<String>,
    pub key_env: Option<String>,
    /// INTERRUPT_GRACE_MS (5000) where `None`.
    pub interrupt_grace_ms: Option<u64>,
    /// RUN_EXIT_MS (10000) where `None`.
    pub result_exit_ms: Option<u64>,
    pub launch: Option<AgentLaunch>,
}

impl CursorAdapterDeps {
    /// Deps with every optional field `None`.
    pub fn new(exec: Arc<dyn ExecStreamFactory>) -> Self {
        CursorAdapterDeps {
            exec,
            base_env: None,
            sign_in_refusal: None,
            api_key: None,
            key_env: None,
            interrupt_grace_ms: None,
            result_exit_ms: None,
            launch: None,
        }
    }
}

/// What a session does. Not part of the port.
pub trait CursorCore {
    fn local_id(&self) -> String;
    fn session_id(&self) -> String;
    fn command(&self) -> Option<String>;
    fn run(&self) -> Option<String>;
    fn pid(&self) -> Option<u32>;
    fn finished(&self) -> SharedFuture<TurnResult>;
    fn interrupt(&self) -> LocalBoxFuture<'static, ()>;
}

/// TS adapter.ts:25.
#[derive(Clone)]
pub struct CursorSession {
    pub core: Arc<dyn CursorCore>,
}

impl CursorSession {
    /// The resume id, or one minted at start.
    pub fn local_id(&self) -> String {
        self.core.local_id()
    }
    /// The chat id init named, the local id until then.
    pub fn session_id(&self) -> String {
        self.core.session_id()
    }
    pub fn command(&self) -> Option<String> {
        self.core.command()
    }
    pub fn run(&self) -> Option<String> {
        self.core.run()
    }
    pub fn pid(&self) -> Option<u32> {
        self.core.pid()
    }
    pub fn finished(&self) -> SharedFuture<TurnResult> {
        self.core.finished()
    }
    /// TS adapter.ts:210.
    pub fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        self.core.interrupt()
    }
}

/// What attach found.
pub enum AttachedSession {
    Session(CursorSession),
    Gone,
}

/// TS adapter.ts:51. Fields are the implementer's to extend.
#[derive(Clone)]
pub struct CursorAdapter {
    pub deps: CursorAdapterDeps,
    pub env: Env,
    pub sessions: Arc<Mutex<IndexMap<String, CursorSession>>>,
}

/// TS adapter.ts:111.
pub fn create_cursor_adapter(deps: CursorAdapterDeps) -> CursorAdapter {
    let env = build_env(&CursorEnvOptions { base: deps.base_env.clone(), api_key: deps.api_key.clone(), key_env: deps.key_env.clone() });
    CursorAdapter { deps, env, sessions: Arc::new(Mutex::new(IndexMap::new())) }
}

const STDERR_TAIL_LINES: usize = 5;
const NO_CREDENTIAL_SENTENCE: &str = "Error: Authentication required";

/// The turn's text since its last tool call, and what the reader has seen. TS adapter.ts:58-70.
#[derive(Default)]
struct State {
    session_id: String,
    interrupt_requested: bool,
    turn_result: Option<TurnResult>,
    /// The text since the last tool call: the reply, where the result's own field runs every segment together.
    segment: String,
    no_credential: Option<String>,
    stderr: Vec<String>,
}

/// One Cursor turn: its stream's reader and what a caller does to it. TS adapter.ts:58-210 (`follow`).
struct Turn {
    local_id: String,
    command: Option<String>,
    model: Option<String>,
    cwd: Option<String>,
    stream: Arc<dyn ExecStream>,
    grace_ms: u64,
    result_exit_ms: u64,
    sign_in_refusal: Option<String>,
    on_event: OnEvent,
    finished: SharedFuture<TurnResult>,
    st: RefCell<State>,
}

struct FollowOptions {
    stream: Arc<dyn ExecStream>,
    local_id: String,
    command: Option<String>,
    model: Option<String>,
    cwd: Option<String>,
    on_event: OnEvent,
}

fn bare(status: TurnStatus) -> TurnResult {
    TurnResult {
        status,
        duration_ms: None,
        waited_ms: None,
        cost_usd: None,
        tokens: None,
        model: None,
        models: None,
        text: None,
        error: None,
        unreached: None,
        refusal: None,
        limit: None,
    }
}

/// TS adapter.ts `NO_CREDENTIAL`: `^Error: Authentication required\b`.
fn is_no_credential(text: &str) -> bool {
    text.strip_prefix(NO_CREDENTIAL_SENTENCE)
        .is_some_and(|rest| rest.chars().next().is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_')))
}

/// Every `message.content[].text` string joined with no separator; a part with no string text adds nothing. TS
/// adapter.ts:72.
fn text_of(event: &Obj) -> String {
    match rec(event.get("message")).and_then(|message| message.get("content")) {
        Some(Value::Array(parts)) => parts.iter().map(|part| string(rec(Some(part)).and_then(|p| p.get("text"))).unwrap_or("")).collect(),
        _ => String::new(),
    }
}

/// With --stream-partial-output only a line stamped with a time and naming no model call is new text. TS adapter.ts:144.
fn is_streamed_text(event: &Obj) -> bool {
    event.get("timestamp_ms").is_some() && event.get("model_call_id").is_none()
}

/// The call under the first key of `tool_call` in JS order. TS adapter.ts:94.
struct Call<'a> {
    name: String,
    input: String,
    body: &'a Obj,
}

fn call_of(event: &Obj) -> Option<Call<'_>> {
    let call = rec(event.get("tool_call"))?;
    let (key, value) = js_key_order(call.iter()).into_iter().next()?;
    let body = rec(Some(value))?;
    if key == "function" {
        return Some(Call {
            name: string(body.get("name")).unwrap_or(key).to_string(),
            input: string(body.get("arguments")).unwrap_or("{}").to_string(),
            body,
        });
    }
    let input = match body.get("args") {
        None | Some(Value::Null) => "{}".to_string(),
        Some(args) => wsp_wire::text::json_stringify(args),
    };
    Some(Call { name: key.strip_suffix("ToolCall").unwrap_or(key).to_string(), input, body })
}

/// The text a tool's result reads as and whether it is an error. TS adapter.ts:102.
fn result_of(body: &Obj) -> (String, bool) {
    let entry = rec(body.get("result")).and_then(|result| js_key_order(result.iter()).into_iter().next());
    let (kind, value) = match entry {
        Some((kind, value)) => (kind.as_str(), Some(value)),
        None => ("success", None),
    };
    let fields = rec(value);
    if kind == "success" {
        let text = string(fields.and_then(|f| f.get("content")))
            .or_else(|| string(fields.and_then(|f| f.get("stdout"))))
            .map(String::from)
            .unwrap_or_else(|| match value {
                None | Some(Value::Null) => "{}".to_string(),
                Some(value) => wsp_wire::text::json_stringify(value),
            });
        return (text, false);
    }
    let why = string(fields.and_then(|f| f.get("reason")))
        .or_else(|| string(fields.and_then(|f| f.get("error"))))
        .or_else(|| string(fields.and_then(|f| f.get("stderr"))))
        .map(String::from)
        .or_else(|| value.map(wsp_wire::text::json_stringify));
    (why.map_or_else(|| kind.to_string(), |why| format!("{kind}: {why}")), true)
}

impl Turn {
    fn emit(&self, event: AdapterEvent) {
        (self.on_event)(event)
    }

    fn current_session_id(&self) -> String {
        self.st.borrow().session_id.clone()
    }

    /// TS adapter.ts:190-205.
    fn exit_result(&self, st: &State, exit_code: Option<i32>, stream_error: Option<&str>) -> TurnResult {
        if st.interrupt_requested {
            return bare(TurnStatus::Interrupted);
        }
        if let Some(sentence) = &st.no_credential {
            let result = TurnResult { error: Some(sentence.clone()), ..bare(TurnStatus::Failed) };
            let refusal = Refusal { road: self.sign_in_refusal.clone(), cause: Some(TurnRefusal::SignIn) };
            return refused_turn(result, Some(&refusal));
        }
        let error = match stream_error {
            Some(error) => error.to_string(),
            None => {
                let said = if st.stderr.is_empty() { String::new() } else { format!(": {}", st.stderr.join("\n")) };
                let code = exit_code.map_or_else(|| "null".to_string(), |code| code.to_string());
                format!("cursor-agent exited with code {code} before its turn ended{said}")
            }
        };
        TurnResult { error: Some(error), ..bare(TurnStatus::Failed) }
    }

    /// TS adapter.ts:118-130 and the `finished` IIFE after it.
    async fn read_all(self: Arc<Self>) -> TurnResult {
        let mut lines = self.stream.lines();
        let mut stream_failure: Option<ExecFailure> = None;
        while let Some(item) = lines.next().await {
            match item {
                Ok(raw) => self.read_line(&raw),
                Err(failure) => {
                    stream_failure = Some(failure);
                    break;
                }
            }
        }
        let exit_code = self.stream.exited().await;
        let saw_result = self.st.borrow().turn_result.is_some();
        if !saw_result {
            let stream_error = stream_failure.as_ref().map(|failure| failure.message.clone());
            let result = self.exit_result(&self.st.borrow(), exit_code, stream_error.as_deref());
            self.st.borrow_mut().turn_result = Some(result.clone());
            let session_id = self.current_session_id();
            self.emit(AdapterEvent::TurnDone { session_id, result, held: None });
        }
        let (session_id, result) = {
            let st = self.st.borrow();
            (st.session_id.clone(), st.turn_result.clone())
        };
        self.emit(AdapterEvent::SessionEnd { session_id, exit_code, saw_result, failure: stream_failure });
        result.unwrap_or_else(|| bare(TurnStatus::Failed))
    }

    /// One stdout line, in the order of adapter.ts:131-190.
    fn read_line(&self, raw: &str) {
        let Some(event) = parse_line(raw) else {
            let text = js_trim(raw);
            let mut st = self.st.borrow_mut();
            if is_no_credential(text) {
                st.no_credential = Some(text.to_string());
            }
            if !text.is_empty() {
                st.stderr.push(text.to_string());
                if st.stderr.len() > STDERR_TAIL_LINES {
                    st.stderr.remove(0);
                }
            }
            return;
        };
        match string(event.get("type")) {
            Some("system") => {
                if string(event.get("subtype")) != Some("init") {
                    return;
                }
                let session_id = {
                    let mut st = self.st.borrow_mut();
                    if let Some(id) = string(event.get("session_id")) {
                        st.session_id = id.to_string();
                    }
                    st.session_id.clone()
                };
                self.emit(AdapterEvent::SessionStart {
                    session_id,
                    model: self.model.clone(),
                    cwd: self.cwd.clone(),
                    tools: None,
                    harness: None,
                    prompted: None,
                });
            }
            Some("assistant") => {
                let text = text_of(&event);
                if !is_streamed_text(&event) || text.is_empty() {
                    return;
                }
                let session_id = {
                    let mut st = self.st.borrow_mut();
                    st.segment.push_str(&text);
                    st.session_id.clone()
                };
                self.emit(AdapterEvent::TurnDelta(TurnDelta::new(session_id, DeltaKind::Text, text)));
            }
            Some("tool_call") => {
                let Some(call) = call_of(&event) else {
                    return;
                };
                let tool_use_id = string(event.get("call_id")).unwrap_or("").to_string();
                match string(event.get("subtype")) {
                    Some("started") => {
                        let session_id = {
                            let mut st = self.st.borrow_mut();
                            st.segment.clear();
                            st.session_id.clone()
                        };
                        let mut delta = TurnDelta::new(session_id, DeltaKind::ToolUse, call.input);
                        delta.tool_name = Some(call.name);
                        delta.tool_use_id = Some(tool_use_id);
                        self.emit(AdapterEvent::TurnDelta(delta));
                    }
                    Some("completed") => {
                        let (text, is_error) = result_of(call.body);
                        let mut delta = TurnDelta::new(self.current_session_id(), DeltaKind::ToolResult, text);
                        delta.tool_use_id = Some(tool_use_id);
                        delta.is_error = Some(is_error);
                        self.emit(AdapterEvent::TurnDelta(delta));
                    }
                    _ => {}
                }
            }
            Some("result") => self.result(&event),
            _ => {}
        }
    }

    /// TS adapter.ts:160-180: the first result line ends the turn, and the process has a while to exit.
    fn result(&self, event: &Obj) {
        if self.st.borrow().turn_result.is_some() {
            return;
        }
        let duration_ms = num(event.get("duration_ms")).map(JsNumber);
        let said = string(event.get("result")).unwrap_or("").to_string();
        let tokens = rec(event.get("usage")).map(|usage| TurnTokens {
            input: JsNumber(num(usage.get("inputTokens")).unwrap_or(0.0)),
            output: JsNumber(num(usage.get("outputTokens")).unwrap_or(0.0)),
            cached: None,
            cache_write: None,
            reasoning: None,
            context: None,
            window: None,
        });
        let result = if event.get("is_error") == Some(&Value::Bool(true)) {
            let error = if said.is_empty() { "cursor reported a failed turn".to_string() } else { said };
            TurnResult { duration_ms, error: Some(error), ..bare(TurnStatus::Failed) }
        } else {
            let segment = self.st.borrow().segment.clone();
            let text = if segment.is_empty() { said } else { segment };
            TurnResult { duration_ms, text: Some(text), tokens, ..bare(TurnStatus::Completed) }
        };
        self.st.borrow_mut().turn_result = Some(result.clone());
        let session_id = self.current_session_id();
        self.emit(AdapterEvent::TurnDone { session_id, result, held: None });
        let stream = self.stream.clone();
        let (exit_ms, grace) = (self.result_exit_ms, self.grace_ms);
        fire(async move { end_after_result(&*stream, exit_ms, grace).await }.boxed_local());
    }
}

impl CursorCore for Turn {
    fn local_id(&self) -> String {
        self.local_id.clone()
    }
    fn session_id(&self) -> String {
        self.current_session_id()
    }
    fn command(&self) -> Option<String> {
        self.command.clone()
    }
    fn run(&self) -> Option<String> {
        self.stream.run()
    }
    fn pid(&self) -> Option<u32> {
        self.stream.pid()
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        self.finished.clone()
    }
    /// TS adapter.ts:210: the mark is set at the call, the end waits on the returned future.
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        self.st.borrow_mut().interrupt_requested = true;
        let stream = self.stream.clone();
        let grace = self.grace_ms;
        started(async move { end_run(&*stream, grace).await }.boxed_local())
    }
}

/// Reads `o.stream` on a task of its own from the moment it is called, and registers the session in `sessions` under
/// its local id. TS adapter.ts:58 (`follow`).
fn follow(deps: &CursorAdapterDeps, sessions: &Arc<Mutex<IndexMap<String, CursorSession>>>, o: FollowOptions) -> CursorSession {
    let (finish_tx, finish_rx) = oneshot::channel::<TurnResult>();
    let finished: SharedFuture<TurnResult> =
        async move { finish_rx.await.unwrap_or_else(|_| bare(TurnStatus::Failed)) }.boxed_local().shared();
    let turn = Arc::new(Turn {
        local_id: o.local_id.clone(),
        command: o.command,
        model: o.model,
        cwd: o.cwd,
        stream: o.stream,
        grace_ms: deps.interrupt_grace_ms.unwrap_or(INTERRUPT_GRACE_MS),
        result_exit_ms: deps.result_exit_ms.unwrap_or(RUN_EXIT_MS),
        sign_in_refusal: deps.sign_in_refusal.clone(),
        on_event: o.on_event,
        finished,
        st: RefCell::new(State { session_id: o.local_id.clone(), ..State::default() }),
    });
    let session = CursorSession { core: turn.clone() };
    sessions.lock().unwrap().insert(o.local_id, session.clone());
    tokio::task::spawn_local(async move {
        let result = turn.read_all().await;
        let _ = finish_tx.send(result);
    });
    session
}

impl CursorAdapter {
    /// No control channel: no steer. TS adapter.ts:261.
    pub fn steers(&self) -> bool {
        false
    }
    /// No image flag is documented, so none. TS adapter.ts:49.
    pub fn attachments(&self) -> Option<crate::port::AttachmentRoad> {
        None
    }
    /// Takes no MCP servers with a launch.
    pub fn mcp_servers(&self) -> bool {
        false
    }
    /// TS adapter.ts:219. `Err` where the TS throws before anything runs.
    pub fn start(&self, options: HarnessStartOptions) -> Result<CursorSession, AdapterError> {
        if options.context_window.is_some() {
            return Err(AdapterError("cursor takes no context window".into()));
        }
        if options.effort.is_some() {
            return Err(AdapterError("cursor takes its effort inside the model name, not as its own pick".into()));
        }
        let command = build_command(&BuildCommandOptions {
            prompt: options.prompt.clone(),
            resume: options.resume.clone(),
            cwd: options.cwd.clone(),
            model: options.model.clone(),
            permission_mode: options.permission_mode.clone(),
            launch: self.deps.launch.clone(),
        })
        .map_err(AdapterError)?;
        let stream = self.deps.exec.launch(&command, ExecOptions { env: self.env.clone(), ..Default::default() });
        Ok(follow(
            &self.deps,
            &self.sessions,
            FollowOptions {
                stream,
                local_id: options.resume.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                command: Some(command),
                model: options.model,
                cwd: options.cwd,
                on_event: options.on_event.unwrap_or_else(|| Arc::new(|_| {})),
            },
        ))
    }
    /// TS adapter.ts:246. `None` where the exec factory has no attach.
    pub fn attach(&self, options: AdapterAttachOptions) -> Option<LocalBoxFuture<'static, Result<AttachedSession, ExecFailure>>> {
        let pending = self.deps.exec.attach(&options.run, AttachRunOptions { input: false, started_at: options.started_at, from: None })?;
        let deps = self.deps.clone();
        let sessions = self.sessions.clone();
        Some(
            async move {
                match pending.await? {
                    Attached::Gone => Ok(AttachedSession::Gone),
                    Attached::Stream(stream) => Ok(AttachedSession::Session(follow(
                        &deps,
                        &sessions,
                        FollowOptions {
                            stream,
                            local_id: options.session_id,
                            command: None,
                            model: options.model,
                            cwd: options.cwd,
                            on_event: options.on_event,
                        },
                    ))),
                }
            }
            .boxed_local(),
        )
    }
    pub fn sessions(&self) -> IndexMap<String, CursorSession> {
        self.sessions.lock().unwrap().clone()
    }
    pub fn env(&self) -> Env {
        self.env.clone()
    }
}

/// The runtime's view of the Cursor adapter: the members of adapter.ts:242-264 its object declares.
impl HarnessAdapter for CursorAdapter {
    fn start(&self, options: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        CursorAdapter::start(self, options).map(|session| Arc::new(session) as Arc<dyn HarnessSession>)
    }
    fn attach(
        &self,
        options: AdapterAttachOptions,
    ) -> Option<LocalBoxFuture<'static, Result<Option<Arc<dyn HarnessSession>>, ExecFailure>>> {
        CursorAdapter::attach(self, options).map(|pending| {
            async move {
                Ok(match pending.await? {
                    AttachedSession::Session(session) => Some(Arc::new(session) as Arc<dyn HarnessSession>),
                    AttachedSession::Gone => None,
                })
            }
            .boxed_local()
        })
    }
    fn steers(&self) -> bool {
        CursorAdapter::steers(self)
    }
    fn env(&self) -> Option<Env> {
        Some(CursorAdapter::env(self))
    }
}

impl HarnessSession for CursorSession {
    fn local_id(&self) -> String {
        CursorSession::local_id(self)
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        CursorSession::finished(self)
    }
    fn run(&self) -> Option<String> {
        CursorSession::run(self)
    }
    fn pid(&self) -> Option<u32> {
        CursorSession::pid(self)
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        CursorSession::interrupt(self)
    }
}
