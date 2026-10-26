// SPDX-License-Identifier: AGPL-3.0-only
//! `opencode run --format json` read into the runtime's events. TS adapter-opencode/src/adapter.ts. Spec:
//! spec/opencode.md rules 5-14.

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use futures::channel::oneshot;
use futures::future::{FutureExt, LocalBoxFuture};
use futures::stream::StreamExt;
use indexmap::IndexMap;
use serde_json::Value;

use crate::claude::fields::{num, parse_line, rec, string, Obj};
use crate::claude::turn::{fire, INTERRUPT_GRACE_MS, RUN_EXIT_MS};
use crate::opencode::catalog::{catalog_probe_command, parse_catalog_probe, parse_version, version_probe_command};
use crate::opencode::command::{build_command, build_env, BuildCommandOptions, OpenCodeEnvOptions};
use crate::pending::{refused_turn, Refusal};
use crate::port::{
    end_after_result, end_run, system_now, AdapterAttachOptions, AdapterError, AdapterEvent, AgentLaunch, AttachRunOptions, Attached,
    AttachmentRoad, DeltaKind, Env, ExecFailure, ExecOptions, ExecStream, ExecStreamFactory, HarnessAdapter, HarnessCatalogAnswer,
    HarnessExec, HarnessSession, HarnessStartOptions, Now, OnEvent, SharedFuture, TurnDelta, TurnResult, TurnStatus, TurnTokens,
};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::session::TurnRefusal;
use wsp_words::js::js_trim;

/// TS adapter.ts:41.
#[derive(Clone)]
pub struct OpenCodeAdapterDeps {
    pub exec: Arc<dyn ExecStreamFactory>,
    pub base_env: Option<IndexMap<String, Option<String>>>,
    /// wsp's half of a turn OpenCode refuses for want of a sign-in.
    pub sign_in_refusal: Option<String>,
    pub api_key: Option<String>,
    pub key_env: Option<String>,
    /// INTERRUPT_GRACE_MS (5000) where `None`.
    pub interrupt_grace_ms: Option<u64>,
    /// RUN_EXIT_MS (10000) where `None`.
    pub result_exit_ms: Option<u64>,
    pub launch: Option<AgentLaunch>,
    /// The system clock where `None`.
    pub now: Option<Now>,
}

impl OpenCodeAdapterDeps {
    /// Deps with every optional field `None`.
    pub fn new(exec: Arc<dyn ExecStreamFactory>) -> Self {
        OpenCodeAdapterDeps {
            exec,
            base_env: None,
            sign_in_refusal: None,
            api_key: None,
            key_env: None,
            interrupt_grace_ms: None,
            result_exit_ms: None,
            launch: None,
            now: None,
        }
    }
}

/// What a session does. Not part of the port.
pub trait OpenCodeCore {
    fn local_id(&self) -> String;
    fn session_id(&self) -> String;
    fn command(&self) -> Option<String>;
    fn run(&self) -> Option<String>;
    fn pid(&self) -> Option<u32>;
    fn finished(&self) -> SharedFuture<TurnResult>;
    fn interrupt(&self) -> LocalBoxFuture<'static, ()>;
}

/// TS adapter.ts:30.
#[derive(Clone)]
pub struct OpenCodeSession {
    pub core: Arc<dyn OpenCodeCore>,
}

impl OpenCodeSession {
    /// The resume id, or one minted at start.
    pub fn local_id(&self) -> String {
        self.core.local_id()
    }
    /// The session id the first event named, the local id until then.
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
    /// TS adapter.ts:238.
    pub fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        self.core.interrupt()
    }
}

/// What attach found.
pub enum AttachedSession {
    Session(OpenCodeSession),
    Gone,
}

/// TS adapter.ts:54. Fields are the implementer's to extend.
#[derive(Clone)]
pub struct OpenCodeAdapter {
    pub deps: OpenCodeAdapterDeps,
    pub env: Env,
    pub sessions: Arc<Mutex<IndexMap<String, OpenCodeSession>>>,
}

/// TS adapter.ts:124.
pub fn create_opencode_adapter(deps: OpenCodeAdapterDeps) -> OpenCodeAdapter {
    let env = build_env(&OpenCodeEnvOptions { base: deps.base_env.clone(), api_key: deps.api_key.clone(), key_env: deps.key_env.clone() });
    OpenCodeAdapter { deps, env, sessions: Arc::new(Mutex::new(IndexMap::new())) }
}

/// The step reasons after which the session loop runs another step. TS adapter.ts:70.
const LOOP_GOES_ON: [&str; 3] = ["tool-calls", "unknown", "content-filter"];
const STDERR_TAIL_LINES: usize = 10;

/// The first failure of a turn: its line and the cause it is classed by. TS adapter.ts:84.
#[derive(Clone)]
struct Failed {
    line: String,
    cause: Option<TurnRefusal>,
}

/// The turn's running totals and what the reader has seen. TS adapter.ts:124-150 (the closure's `let`s).
#[derive(Default)]
struct State {
    session_id: String,
    announced: bool,
    interrupt_requested: bool,
    turn_result: Option<TurnResult>,
    last_text: Option<String>,
    failure: Option<Failed>,
    cost_usd: f64,
    input: f64,
    output: f64,
    reasoning: f64,
    cache_read: f64,
    cache_write: f64,
    /// The last step's whole count, which is what the model held at the turn's end.
    held: Option<f64>,
    stderr: Vec<String>,
}

/// One OpenCode turn: its stream's reader and what a caller does to it. TS adapter.ts:124-300 (`follow`).
struct Turn {
    local_id: String,
    command: Option<String>,
    model: Option<String>,
    cwd: Option<String>,
    started_at: f64,
    now: Now,
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
    started_at: f64,
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

/// A number field, zero where absent or not a number. TS adapter.ts:61.
fn number(obj: Option<&Obj>, key: &str) -> f64 {
    num(obj.and_then(|o| o.get(key))).unwrap_or(0.0)
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn is_line_terminator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

/// The first `\berror="..."` field of `line`, its escapes read as the characters they name. TS adapter.ts:96-110.
fn error_field(line: &str) -> Option<String> {
    const KEY: &str = "error=\"";
    let mut from = 0;
    while let Some(at) = line[from..].find(KEY).map(|i| i + from) {
        from = at + 1;
        if line[..at].chars().next_back().is_some_and(is_word) {
            continue;
        }
        let mut out = String::new();
        let mut chars = line[at + KEY.len()..].chars();
        let mut closed = false;
        while let Some(c) = chars.next() {
            match c {
                '"' => {
                    closed = true;
                    break;
                }
                '\\' => match chars.next() {
                    Some(next) if !is_line_terminator(next) => out.push(next),
                    _ => break,
                },
                c => out.push(c),
            }
        }
        if closed {
            return Some(out);
        }
    }
    None
}

/// What the log line carrying this ref says failed. TS adapter.ts:96.
fn logged_error(stderr: &[String], reference: &str) -> Option<String> {
    let marker = format!("ref={reference} ");
    stderr.iter().filter(|line| line.contains(&marker)).find_map(|line| error_field(line))
}

/// TS adapter.ts:111.
fn failure_of(error: Option<&Obj>, stderr: &[String]) -> Failed {
    let name = string(error.and_then(|e| e.get("name")));
    let data = rec(error.and_then(|e| e.get("data")));
    let data_str = |key: &str| string(data.and_then(|d| d.get(key)));
    let logged = data_str("ref").and_then(|reference| logged_error(stderr, reference));
    let line = logged
        .or_else(|| data_str("message").map(String::from))
        .or_else(|| name.map(String::from))
        .unwrap_or_else(|| "opencode reported an error".to_string());
    let status = data.and_then(|d| d.get("statusCode")).and_then(Value::as_f64);
    let refused = name == Some("ProviderAuthError") || (name == Some("APIError") && status == Some(401.0));
    Failed { line, cause: refused.then_some(TurnRefusal::SignIn) }
}

/// The two events one tool part draws. TS adapter.ts:186.
fn tool_deltas(part: &Obj, session_id: &str) -> Vec<AdapterEvent> {
    let state = rec(part.get("state"));
    let tool_use_id = string(part.get("callID")).or_else(|| string(part.get("id"))).unwrap_or("").to_string();
    let failed = string(state.and_then(|s| s.get("status"))) == Some("error");
    let input = match state.and_then(|s| s.get("input")) {
        None | Some(Value::Null) => "{}".to_string(),
        Some(input) => wsp_wire::text::json_stringify(input),
    };
    let mut call = TurnDelta::new(session_id, DeltaKind::ToolUse, input);
    call.tool_name = Some(string(part.get("tool")).unwrap_or("tool").to_string());
    call.tool_use_id = Some(tool_use_id.clone());
    let output = string(state.and_then(|s| s.get(if failed { "error" } else { "output" }))).unwrap_or("");
    let mut result = TurnDelta::new(session_id, DeltaKind::ToolResult, output);
    result.tool_use_id = Some(tool_use_id);
    result.is_error = Some(failed);
    vec![AdapterEvent::TurnDelta(call), AdapterEvent::TurnDelta(result)]
}

/// Adds one finished step to the totals. TS adapter.ts:153.
fn add_step(st: &mut State, part: &Obj) {
    let tokens = rec(part.get("tokens"));
    let cache = rec(tokens.and_then(|t| t.get("cache")));
    let (input, output, reasoning) = (number(tokens, "input"), number(tokens, "output"), number(tokens, "reasoning"));
    let (read, write) = (number(cache, "read"), number(cache, "write"));
    st.cost_usd += number(Some(part), "cost");
    st.input += input;
    st.output += output;
    st.reasoning += reasoning;
    st.cache_read += read;
    st.cache_write += write;
    if tokens.is_some() {
        st.held = Some(input + read + write + output);
    }
}

/// OpenCode counts cache reads and writes apart from the input, so the input side is the three together. TS
/// adapter.ts:163.
fn tokens_of(st: &State) -> TurnTokens {
    TurnTokens {
        input: JsNumber(st.input + st.cache_read + st.cache_write),
        output: JsNumber(st.output),
        cached: Some(JsNumber(st.cache_read)),
        cache_write: Some(JsNumber(st.cache_write)),
        reasoning: Some(JsNumber(st.reasoning)),
        context: st.held.map(JsNumber),
        window: None,
    }
}

impl Turn {
    fn emit(&self, event: AdapterEvent) {
        (self.on_event)(event)
    }

    fn current_session_id(&self) -> String {
        self.st.borrow().session_id.clone()
    }

    fn elapsed(&self) -> f64 {
        (self.now)() as f64 - self.started_at
    }

    /// TS adapter.ts:163.
    fn completed(&self, st: &State) -> TurnResult {
        TurnResult {
            duration_ms: Some(JsNumber(self.elapsed())),
            cost_usd: Some(JsNumber(st.cost_usd)),
            tokens: Some(tokens_of(st)),
            text: st.last_text.clone(),
            ..bare(TurnStatus::Completed)
        }
    }

    /// TS adapter.ts:133.
    fn failed(&self, failure: &Failed) -> TurnResult {
        let result = TurnResult { error: Some(failure.line.clone()), ..bare(TurnStatus::Failed) };
        match failure.cause {
            Some(cause) => refused_turn(result, Some(&Refusal { road: self.sign_in_refusal.clone(), cause: Some(cause) })),
            None => result,
        }
    }

    /// The turn's result where the process ended with none. TS adapter.ts:226-240.
    fn exit_result(&self, st: &State, exit_code: Option<i32>, stream_error: Option<&str>) -> TurnResult {
        if st.interrupt_requested {
            return bare(TurnStatus::Interrupted);
        }
        if let Some(failure) = &st.failure {
            return self.failed(failure);
        }
        if exit_code == Some(0) && st.announced {
            return self.completed(st);
        }
        let error = match stream_error {
            Some(error) => error.to_string(),
            None => {
                let said = if st.stderr.is_empty() { String::new() } else { format!(": {}", st.stderr.join("\n")) };
                let code = exit_code.map_or_else(|| "null".to_string(), |code| code.to_string());
                format!("opencode exited with code {code} before its turn ended{said}")
            }
        };
        TurnResult { error: Some(error), ..bare(TurnStatus::Failed) }
    }

    /// TS adapter.ts:166-273 and the `finished` IIFE after it.
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

    /// One stdout line, in the order of adapter.ts:166-215.
    fn read_line(&self, raw: &str) {
        let Some(event) = parse_line(raw) else {
            let text = js_trim(raw);
            if !text.is_empty() {
                let mut st = self.st.borrow_mut();
                st.stderr.push(text.to_string());
                if st.stderr.len() > STDERR_TAIL_LINES {
                    st.stderr.remove(0);
                }
            }
            return;
        };
        let Some(kind) = string(event.get("type")) else {
            return;
        };
        if let Some(id) = string(event.get("sessionID")) {
            let announce = {
                let mut st = self.st.borrow_mut();
                if st.announced {
                    None
                } else {
                    st.announced = true;
                    st.session_id = id.to_string();
                    Some(id.to_string())
                }
            };
            if let Some(session_id) = announce {
                self.emit(AdapterEvent::SessionStart {
                    session_id,
                    model: self.model.clone(),
                    cwd: self.cwd.clone(),
                    tools: None,
                    harness: None,
                    prompted: None,
                });
            }
        }
        let part = rec(event.get("part"));
        match kind {
            "text" => {
                let text = string(part.and_then(|p| p.get("text"))).unwrap_or("");
                if js_trim(text).is_empty() {
                    return;
                }
                let session_id = {
                    let mut st = self.st.borrow_mut();
                    st.last_text = Some(text.to_string());
                    st.session_id.clone()
                };
                let mut delta = TurnDelta::new(session_id, DeltaKind::Text, text);
                delta.message_id = string(part.and_then(|p| p.get("messageID"))).map(String::from);
                self.emit(AdapterEvent::TurnDelta(delta));
            }
            "tool_use" => {
                if let Some(part) = part {
                    let session_id = self.current_session_id();
                    for delta in tool_deltas(part, &session_id) {
                        self.emit(delta);
                    }
                }
            }
            "step_finish" => {
                if let Some(part) = part {
                    self.step_finish(part);
                }
            }
            "error" => {
                let failure = {
                    let st = self.st.borrow();
                    st.failure.is_none().then(|| failure_of(rec(event.get("error")), &st.stderr))
                };
                if let Some(failure) = failure {
                    self.st.borrow_mut().failure = Some(failure);
                }
            }
            _ => {}
        }
    }

    /// TS adapter.ts:200-215: a step whose reason ends the loop completes the turn, and the process has a while to exit.
    fn step_finish(&self, part: &Obj) {
        let (result, session_id) = {
            let mut st = self.st.borrow_mut();
            add_step(&mut st, part);
            let reason = string(part.get("reason")).unwrap_or("unknown");
            if st.turn_result.is_some() || st.failure.is_some() || LOOP_GOES_ON.contains(&reason) {
                return;
            }
            let result = self.completed(&st);
            st.turn_result = Some(result.clone());
            (result, st.session_id.clone())
        };
        self.emit(AdapterEvent::TurnDone { session_id, result, held: None });
        let stream = self.stream.clone();
        let (exit_ms, grace) = (self.result_exit_ms, self.grace_ms);
        fire(async move { end_after_result(&*stream, exit_ms, grace).await }.boxed_local());
    }
}

impl OpenCodeCore for Turn {
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
    /// TS adapter.ts:238: the mark is set at the call, the end waits on the returned future.
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        self.st.borrow_mut().interrupt_requested = true;
        let stream = self.stream.clone();
        let grace = self.grace_ms;
        async move { end_run(&*stream, grace).await }.boxed_local()
    }
}

/// Reads `o.stream` on a task of its own from the moment it is called, and registers the session in `sessions` under
/// its local id. TS adapter.ts:124 (`follow`).
fn follow(deps: &OpenCodeAdapterDeps, sessions: &Arc<Mutex<IndexMap<String, OpenCodeSession>>>, o: FollowOptions) -> OpenCodeSession {
    let (finish_tx, finish_rx) = oneshot::channel::<TurnResult>();
    let finished: SharedFuture<TurnResult> =
        async move { finish_rx.await.unwrap_or_else(|_| bare(TurnStatus::Failed)) }.boxed_local().shared();
    let turn = Arc::new(Turn {
        local_id: o.local_id.clone(),
        command: o.command,
        model: o.model,
        cwd: o.cwd,
        started_at: o.started_at,
        now: deps.now.clone().unwrap_or_else(system_now),
        stream: o.stream,
        grace_ms: deps.interrupt_grace_ms.unwrap_or(INTERRUPT_GRACE_MS),
        result_exit_ms: deps.result_exit_ms.unwrap_or(RUN_EXIT_MS),
        sign_in_refusal: deps.sign_in_refusal.clone(),
        on_event: o.on_event,
        finished,
        st: RefCell::new(State { session_id: o.local_id.clone(), ..State::default() }),
    });
    let session = OpenCodeSession { core: turn.clone() };
    sessions.lock().unwrap().insert(o.local_id, session.clone());
    tokio::task::spawn_local(async move {
        let result = turn.read_all().await;
        let _ = finish_tx.send(result);
    });
    session
}

impl OpenCodeAdapter {
    /// `run` reads its message and closes stdin: no steer. TS adapter.ts:298.
    pub fn steers(&self) -> bool {
        false
    }
    /// `-f` reads each image off the machine. TS adapter.ts:299.
    pub fn attachments(&self) -> Option<crate::port::AttachmentRoad> {
        Some(crate::port::AttachmentRoad::File)
    }
    /// Takes no MCP servers with a launch.
    pub fn mcp_servers(&self) -> bool {
        false
    }
    /// TS adapter.ts:252. `Err` where the TS throws before anything runs.
    pub fn start(&self, options: HarnessStartOptions) -> Result<OpenCodeSession, AdapterError> {
        if options.context_window.is_some() {
            return Err(AdapterError("opencode takes no context window".into()));
        }
        let images = options
            .images
            .iter()
            .map(|image| {
                image
                    .path
                    .clone()
                    .ok_or_else(|| AdapterError("opencode reads images off the machine's disk; this one has no path on it".into()))
            })
            .collect::<Result<Vec<String>, AdapterError>>()?;
        let command = build_command(&BuildCommandOptions {
            prompt: options.prompt.clone(),
            resume: options.resume.clone(),
            cwd: options.cwd.clone(),
            model: options.model.clone(),
            effort: options.effort.clone(),
            permission_mode: options.permission_mode.clone(),
            title: options.title.clone(),
            images,
            launch: self.deps.launch.clone(),
        })
        .map_err(AdapterError)?;
        let stream = self.deps.exec.launch(&command, ExecOptions { env: self.env.clone(), ..Default::default() });
        let now = self.deps.now.clone().unwrap_or_else(system_now);
        Ok(follow(
            &self.deps,
            &self.sessions,
            FollowOptions {
                stream,
                local_id: options.resume.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                started_at: now() as f64,
                command: Some(command),
                model: options.model,
                cwd: options.cwd,
                on_event: options.on_event.unwrap_or_else(|| Arc::new(|_| {})),
            },
        ))
    }
    /// TS adapter.ts:282. `None` where the exec factory has no attach.
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
                            started_at: options.started_at,
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
    pub fn sessions(&self) -> IndexMap<String, OpenCodeSession> {
        self.sessions.lock().unwrap().clone()
    }
    pub fn env(&self) -> Env {
        self.env.clone()
    }
    /// TS adapter.ts:300.
    pub async fn probe_catalog(&self, exec: Arc<dyn HarnessExec>) -> Result<Option<HarnessCatalogAnswer>, ExecFailure> {
        let stdout = exec.exec(&catalog_probe_command(self.deps.base_env.as_ref(), self.deps.launch.as_ref()), None).await?;
        Ok(parse_catalog_probe(&stdout))
    }
    /// TS adapter.ts:301.
    pub async fn probe_version(&self, exec: Arc<dyn HarnessExec>) -> Result<Option<String>, ExecFailure> {
        let stdout = exec.exec(&version_probe_command(self.deps.base_env.as_ref(), self.deps.launch.as_ref()), None).await?;
        Ok(parse_version(&stdout))
    }
}

/// The runtime's view of the OpenCode adapter: the members of adapter.ts:278-304 its object declares.
impl HarnessAdapter for OpenCodeAdapter {
    fn start(&self, options: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        OpenCodeAdapter::start(self, options).map(|session| Arc::new(session) as Arc<dyn HarnessSession>)
    }
    fn attach(
        &self,
        options: AdapterAttachOptions,
    ) -> Option<LocalBoxFuture<'static, Result<Option<Arc<dyn HarnessSession>>, ExecFailure>>> {
        OpenCodeAdapter::attach(self, options).map(|pending| {
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
        OpenCodeAdapter::steers(self)
    }
    fn attachments(&self) -> Option<AttachmentRoad> {
        OpenCodeAdapter::attachments(self)
    }
    fn probe_catalog(
        &self,
        exec: Arc<dyn HarnessExec>,
    ) -> Option<LocalBoxFuture<'static, Result<Option<HarnessCatalogAnswer>, ExecFailure>>> {
        let me = self.clone();
        Some(async move { me.probe_catalog(exec).await }.boxed_local())
    }
    fn probe_version(&self, exec: Arc<dyn HarnessExec>) -> Option<LocalBoxFuture<'static, Result<Option<String>, ExecFailure>>> {
        let me = self.clone();
        Some(async move { me.probe_version(exec).await }.boxed_local())
    }
    fn env(&self) -> Option<Env> {
        Some(OpenCodeAdapter::env(self))
    }
}

impl HarnessSession for OpenCodeSession {
    fn local_id(&self) -> String {
        OpenCodeSession::local_id(self)
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        OpenCodeSession::finished(self)
    }
    fn run(&self) -> Option<String> {
        OpenCodeSession::run(self)
    }
    fn pid(&self) -> Option<u32> {
        OpenCodeSession::pid(self)
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        OpenCodeSession::interrupt(self)
    }
}
