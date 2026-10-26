use std::cell::RefCell;

use std::rc::Rc;

use std::sync::Arc;

use support::*;

use wsp_core::types::wiring::{AccessMove, SteerTaken};

use wsp_wire::types::attachments::Attachment;

use wsp_wire::types::run_block::RUN_OUTPUT_MAX_CHARS;

use wsp_wire::views::harness::start_picks;

use wsp_wire::views::session_events_more::RunStep;

use wsp_wire::wire::runtime::{SessionAnswerOutcome, SessionAnswerResult, SessionStartOutcome};

use wsp_words::exit::not_found_refusal;

use wsp_words::run_block::run_output_tail;

use wsp_words::words::computer::thread_word;

use wsp_words::words::thread::{aside_unsupported_line, ASIDE_EMPTY_LINE, ASIDE_NO_SESSION_LINE, BLANK_ASIDE_LINE, EMPTY_TITLE_LINE};

use wsp_words::workspace_state::{action_refusal, send_refusal, workspace_state, SendRefusalKind, WorkspaceStateInput};

fn refusal(message: &str, kind: Option<Kind>) -> HostError {
    match kind {
        Some(k) => HostError::with_kind(message, k),
        None => HostError::new(message),
    }
}

fn json_of<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).expect("a value that serializes")
}

fn calls(host: &FakeHost, names: &[&str]) -> Vec<String> {
    host.hooks.calls().into_iter().filter(|c| names.iter().any(|n| c == n || c.starts_with(&format!("{n}(")))).collect()
}

fn scoped(thread: &str) -> Caller {
    let by: ThreadScope =
        serde_json::from_value(json!({ "kind": "thread", "threadId": thread, "workspaceId": "w1", "rootThreadId": thread }))
            .expect("a thread scope");
    Caller::Scoped { origin: WorkspaceOrigin::Here, by }
}

type Reached = Rc<RefCell<Vec<(Option<String>, String, Option<TreeTalk>, bool)>>>;

type EntryOfRow = dyn Fn(&RowAt, Option<&Caller>, Option<TreeTalk>, bool) -> LocalBoxFuture<'static, Result<Option<Entry>, HostError>>;

type AdapterFor = dyn Fn(&Entry, AdapterAsk) -> Result<BuiltAdapter, HostError>;

type LaunchAdapterFor =
    dyn Fn(&Entry, &str, Option<wsp_adapters::port::ServerValues>) -> LocalBoxFuture<'static, Result<BuiltAdapter, HostError>>;

type CatalogOn = dyn Fn(&str, &Entry, &dyn HarnessAdapter) -> LocalBoxFuture<'static, Result<Option<HarnessCatalog>, HostError>>;

type WriteSession = dyn Fn(&str, &Entry, Arc<dyn HarnessAdapter>, &str) -> LocalBoxFuture<'static, Result<SessionRenameWrite, HostError>>;

type AsideHook = dyn Fn(&dyn HarnessAdapter, AsideQuestion) -> LocalBoxFuture<'static, Result<String, HostError>>;

/// A session handle a test drives: each road is present or not, and each call is logged.
struct StubHandle {
    view: SessionView,
    log: Rc<RefCell<Vec<String>>>,
    steer: Option<SteerTaken>,
    answer: Option<SessionAnswerOutcome>,
    access: Option<AccessMove>,
}

impl StubHandle {
    fn new(log: &Rc<RefCell<Vec<String>>>) -> StubHandle {
        StubHandle { view: view("h", "w1", json!({})), log: log.clone(), steer: None, answer: None, access: None }
    }
}

impl SessionHandle for StubHandle {
    fn id(&self) -> String {
        self.view.id.clone()
    }
    fn workspace_id(&self) -> String {
        "w1".into()
    }
    fn finished(&self) -> futures::future::Shared<LocalBoxFuture<'static, TurnResult>> {
        futures::future::pending::<TurnResult>().boxed_local().shared()
    }
    fn turn_id(&self) -> String {
        "turn".into()
    }
    fn outcome(&self) -> SessionStartOutcome {
        SessionStartOutcome::Started
    }
    fn notice(&self) -> Option<String> {
        None
    }
    fn view(&self) -> SessionView {
        self.view.clone()
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        async {}.boxed_local()
    }
    fn steer(&self, prompt: String, id: Option<String>, images: Vec<TurnImage>) -> Option<Local<'static, SteerTaken>> {
        let taken = self.steer?;
        self.log.borrow_mut().push(format!("steer({prompt}, {id:?}, {})", serde_json::to_string(&images).expect("images serialize")));
        Some(async move { Ok(taken) }.boxed_local())
    }
    fn tells_unread(&self) -> Option<bool> {
        None
    }
    fn answer(&self, ask_id: String, option_id: String, reason: Option<String>) -> Option<Local<'static, SessionAnswerResult>> {
        let outcome = self.answer?;
        self.log.borrow_mut().push(format!("answer({ask_id}, {option_id}, {reason:?})"));
        Some(async move { Ok(SessionAnswerResult { outcome }) }.boxed_local())
    }
    fn set_access(&self, mode: String) -> Option<Local<'static, AccessMove>> {
        let moved = self.access?;
        self.log.borrow_mut().push(format!("set_access({mode})"));
        Some(async move { Ok(moved) }.boxed_local())
    }
    fn stop_task(&self, _task: String) -> Option<Local<'static, TaskStop>> {
        None
    }
}

/// An adapter with the optional roads a test names; nothing here ever starts a turn.
#[derive(Clone, Default)]
struct StubAdapter {
    steers_images: bool,
    renames: bool,
    aside: bool,
    aside_servers: bool,
    mcp_servers: bool,
}

impl HarnessAdapter for StubAdapter {
    fn start(&self, _options: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        Err(AdapterError("no turn starts in this test".into()))
    }
    fn steers(&self) -> bool {
        true
    }
    fn steers_images(&self) -> bool {
        self.steers_images
    }
    fn rename_session(
        &self,
        _id: String,
        _title: String,
        _exec: Arc<dyn HarnessExec>,
    ) -> Option<LocalBoxFuture<'static, Result<SessionRenameWrite, ExecFailure>>> {
        self.renames.then(|| futures::future::ready(Ok(SessionRenameWrite::Written)).boxed_local())
    }
    fn aside(&self, _q: AsideQuestion) -> Option<LocalBoxFuture<'static, Result<AsideAnswer, String>>> {
        self.aside.then(|| futures::future::ready(Err("this test answers through the context".to_string())).boxed_local())
    }
    fn renames(&self) -> bool {
        self.renames
    }
    fn asides(&self) -> bool {
        self.aside
    }
    fn aside_servers(&self) -> bool {
        self.aside_servers
    }
    fn mcp_servers(&self) -> bool {
        self.mcp_servers
    }
}

/// The workspace record an entry holds.
fn record_in(entry: &Entry) -> WorkspaceRecord {
    entry.borrow().record.borrow().clone()
}

fn host() -> (Rc<FakeHost>, SessionsArea) {
    let host = FakeHost::new();
    host.add_entry(entry("w1", json!({})));
    let area = SessionsArea::new(host.clone());
    (host, area)
}

fn put(host: &FakeHost, key: &str, workspace_id: &str, extra: Value, handle: Option<StubHandle>) {
    let r = row(view(key, workspace_id, extra), key);
    r.borrow_mut().handle = handle.map(|h| Rc::new(h) as Rc<dyn SessionHandle>);
    host.add_row(key, r);
}

fn unreached(host: &FakeHost, seen: Reached) {
    host.hooks.set::<EntryOfRow>(
        "SessionsCtx::entry_of_row",
        Rc::new(move |at: &RowAt, _o: Option<&Caller>, talk: Option<TreeTalk>, being_made: bool| {
            seen.borrow_mut().push((at.thread_id.clone(), at.workspace_id.clone(), talk, being_made));
            async move { Ok(None) }.boxed_local()
        }),
    );
}

/// What `launch_adapter_for` and `adapter_for` answer: this adapter under the harness asked for.
fn adapters(host: &FakeHost, adapter: StubAdapter) -> Rc<RefCell<Vec<String>>> {
    let asked: Rc<RefCell<Vec<String>>> = Rc::default();
    let (seen, built) = (asked.clone(), adapter.clone());
    host.hooks.set::<LaunchAdapterFor>(
        "SessionsCtx::launch_adapter_for",
        Rc::new(move |_e: &Entry, h: &str, _v: Option<wsp_adapters::port::ServerValues>| {
            seen.borrow_mut().push(h.to_string());
            let built = BuiltAdapter { harness: h.to_string(), adapter: Arc::new(built.clone()) };
            async move { Ok(built) }.boxed_local()
        }),
    );
    let seen = asked.clone();
    host.hooks.set::<AdapterFor>(
        "LaunchCtx::adapter_for",
        Rc::new(move |_e: &Entry, a: AdapterAsk| {
            let h = a.named.unwrap_or_default();
            seen.borrow_mut().push(format!("adapter_for:{h}"));
            Ok(BuiltAdapter { harness: h, adapter: Arc::new(adapter.clone()) })
        }),
    );
    asked
}

fn record_of(host: &FakeHost, thread: &str) -> Option<ThreadRecord> {
    host.records.borrow().get(thread).cloned()
}

fn send_line(host: &FakeHost, with_name: bool) -> String {
    let rec = record_in(&host.live("w1").expect("a live workspace"));
    let state = workspace_state(&WorkspaceStateInput { phase: rec.phase, machine_state: None, reach: None, wake_refused: None });
    let name = if with_name { Some(rec.name.as_str()) } else { None };
    send_refusal(&SendRefusalKind::State(state), rec.gone.as_deref(), name).expect("the state refuses a send")
}

fn png() -> Attachment {
    Attachment {
        media_type: "image/png".into(),
        bytes: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==".into(),
        name: Some("dot.png".into()),
    }
}

fn notes() -> Attachment {
    Attachment { media_type: "text/plain".into(), bytes: "bm90ZXM=".into(), name: Some("notes.txt".into()) }
}

fn steer_ask(prompt: &str, request_id: Option<&str>, attachments: Option<Vec<Attachment>>) -> SteerAsk {
    SteerAsk { prompt: prompt.into(), request_id: request_id.map(String::from), attachments }
}

type Steers = Rc<RefCell<Vec<(String, String, Option<String>, String)>>>;

fn watch_steers(host: &FakeHost) -> Steers {
    let seen: Steers = Rc::default();
    let log = seen.clone();
    host.hooks.set::<dyn Fn(&Row, &str, &SteerAsk, Option<&Caller>, &str)>(
        "SessionsCtx::record_steer",
        Rc::new(move |_r: &Row, id: &str, ask: &SteerAsk, _o: Option<&Caller>, steer_id: &str| {
            log.borrow_mut().push((id.to_string(), ask.prompt.clone(), ask.request_id.clone(), steer_id.to_string()));
        }),
    );
    seen
}

fn thread_with_record(host: &FakeHost, thread: &str, doc: Value) {
    host.add_record(thread, "w1", thread_record(doc));
}

fn claude_catalog() -> HarnessCatalog {
    serde_json::from_value(json!({
        "harness": "claude", "label": "Claude", "source": "table", "version": null, "models": [], "efforts": [], "contextWindows": [],
        "permissionModes": [{ "value": "default", "label": "Default" }, { "value": "acceptEdits", "label": "Accept edits" }],
        "steers": true, "renames": false, "images": false,
    }))
    .expect("a catalog the fixture builds")
}

type Writes = Rc<RefCell<Vec<(String, String)>>>;

/// The context's session write answers `outcome` and notes what it was asked to write.
fn writes(host: &FakeHost, outcome: SessionRenameWrite) -> Writes {
    let seen: Writes = Rc::default();
    let log = seen.clone();
    host.hooks.set::<WriteSession>(
        "SessionsCtx::write_session",
        Rc::new(move |s: &str, _e: &Entry, _a: Arc<dyn HarnessAdapter>, t: &str| {
            log.borrow_mut().push((s.to_string(), t.to_string()));
            let outcome = outcome.clone();
            async move { Ok(outcome) }.boxed_local()
        }),
    );
    seen
}

fn renaming_host() -> (Rc<FakeHost>, SessionsArea) {
    let (host, area) = host();
    put(&host, "turn-1", "w1", json!({ "threadId": "t1", "status": "completed", "claudeSessionId": "SESS" }), None);
    put(&host, "turn-2", "w1", json!({ "threadId": "t1", "status": "completed", "claudeSessionId": "SESS", "harnessTitle": "old" }), None);
    put(
        &host,
        "turn-3",
        "w1",
        json!({ "threadId": "t2", "status": "completed", "claudeSessionId": "OTHER", "harnessTitle": "keep" }),
        None,
    );
    put(
        &host,
        "turn-4",
        "w2",
        json!({ "threadId": "t3", "status": "completed", "claudeSessionId": "SESS", "harnessTitle": "elsewhere" }),
        None,
    );
    adapters(&host, StubAdapter { renames: true, ..StubAdapter::default() });
    (host, area)
}

fn titles(host: &FakeHost) -> Vec<(Option<String>, Option<String>)> {
    ["turn-1", "turn-2", "turn-3", "turn-4"]
        .iter()
        .map(|k| {
            let v = json_of(&host.rows.borrow()[*k].borrow().view);
            (v["harnessTitle"].as_str().map(String::from), v["titleSource"].as_str().map(String::from))
        })
        .collect()
}

fn aside_host() -> (Rc<FakeHost>, SessionsArea) {
    let (host, area) = host();
    put(
        &host,
        "turn-a",
        "w1",
        json!({ "threadId": "t1", "status": "completed", "claudeSessionId": "OLD", "cwd": "/root/a", "model": "claude-opus-5", "startedAt": T0 }),
        None,
    );
    put(
        &host,
        "turn-b",
        "w1",
        json!({ "threadId": "t1", "status": "completed", "claudeSessionId": "SESS", "cwd": "/root/b", "model": "claude-sonnet-5", "effort": "high", "startedAt": T0 + 1_000 }),
        None,
    );
    (host, area)
}

const ANSWER: &str = "You are in /root/b and last asked for the second thing.";

type Asked = Rc<RefCell<Vec<AsideQuestion>>>;

/// The context's aside answers `text`, streaming two pieces first where the question asks for them.
fn aside_answers(host: &FakeHost, text: &'static str) -> Asked {
    let asked: Asked = Rc::default();
    let seen = asked.clone();
    host.hooks.set::<AsideHook>(
        "SessionsCtx::aside",
        Rc::new(move |_a: &dyn HarnessAdapter, q: AsideQuestion| {
            if let Some(on_text) = &q.on_text {
                for piece in ["You are in ", "/root/b"] {
                    on_text(piece.to_string());
                }
            }
            seen.borrow_mut().push(q);
            async move { Ok(text.to_string()) }.boxed_local()
        }),
    );
    asked
}

fn with_aside() -> StubAdapter {
    StubAdapter { aside: true, ..StubAdapter::default() }
}

fn send_line_for(phase: &str) -> String {
    let rec = record_in(&entry("w1", json!({ "phase": phase })));
    let state = workspace_state(&WorkspaceStateInput { phase: rec.phase, machine_state: None, reach: None, wake_refused: None });
    send_refusal(&SendRefusalKind::State(state), rec.gone.as_deref(), None).expect("a refusal")
}

fn wsp_spec(args: &[&str]) -> McpServerSpec {
    serde_json::from_value(json!({ "command": "node", "args": args })).expect("a server spec")
}

struct Servers {
    asks: Rc<RefCell<Vec<AdapterAsk>>>,
    asked: Asked,
    revoked: Rc<RefCell<Vec<String>>>,
    values_for: Rc<RefCell<Vec<(String, String)>>>,
}

/// A host where the bare adapter is `bare`, the one a launch builds is `scoped`, a launch mints device `dev-1` and
/// the workspace is `brokered` or not.
fn servers_host(bare: StubAdapter, scoped: StubAdapter, brokered: bool, root: &'static str) -> (Rc<FakeHost>, SessionsArea, Servers) {
    let (host, area) = aside_host();
    adapters(&host, bare);
    let asks: Rc<RefCell<Vec<AdapterAsk>>> = Rc::default();
    let seen = asks.clone();
    host.hooks.set::<AdapterFor>(
        "LaunchCtx::adapter_for",
        Rc::new(move |_e: &Entry, a: AdapterAsk| {
            let h = a.named.clone().unwrap_or_default();
            seen.borrow_mut().push(a);
            Ok(BuiltAdapter { harness: h, adapter: Arc::new(scoped.clone()) })
        }),
    );
    host.hooks.set::<dyn Fn(&Entry, &str, &str, bool) -> LocalBoxFuture<'static, Result<ThreadLaunch, HostError>>>(
        "LaunchCtx::thread_launch",
        Rc::new(|_e: &Entry, _t: &str, _r: &str, _aside: bool| {
            let mut env = IndexMap::new();
            env.insert("WSP_TOKEN".to_string(), "tok".to_string());
            async move {
                Ok(ThreadLaunch {
                    scoped_device_id: Some("dev-1".into()),
                    env,
                    wsp: Some(wsp_spec(&["/opt/wsp/dist/bin.js", "mcp", "--scoped"])),
                })
            }
            .boxed_local()
        }),
    );
    host.hooks.set::<dyn Fn(&str) -> String>("SessionsCtx::root_of", Rc::new(move |_t: &str| root.to_string()));
    host.hooks.set::<dyn Fn(&WorkspaceRecord) -> bool>("SessionsCtx::brokered", Rc::new(move |_r: &WorkspaceRecord| brokered));
    let values_for: Rc<RefCell<Vec<(String, String)>>> = Rc::default();
    let seen = values_for.clone();
    host.hooks.set::<dyn Fn(&Entry, &str, &str) -> LocalBoxFuture<'static, Result<Option<wsp_adapters::port::ServerValues>, HostError>>>(
        "SessionsCtx::server_values_for",
        Rc::new(move |_e: &Entry, h: &str, cwd: &str| {
            seen.borrow_mut().push((h.to_string(), cwd.to_string()));
            let values = wsp_adapters::port::ServerValues { entries: None, config: None };
            async move { Ok(Some(values)) }.boxed_local()
        }),
    );
    let revoked: Rc<RefCell<Vec<String>>> = Rc::default();
    let seen = revoked.clone();
    host.hooks.set::<dyn Fn(&str) -> LocalBoxFuture<'static, Result<(), HostError>>>(
        "LaunchCtx::revoke_device",
        Rc::new(move |d: &str| {
            seen.borrow_mut().push(d.to_string());
            async move { Ok(()) }.boxed_local()
        }),
    );
    let asked = aside_answers(&host, ANSWER);
    (host, area, Servers { asks, asked, revoked, values_for })
}

fn full_servers() -> StubAdapter {
    StubAdapter { aside: true, aside_servers: true, mcp_servers: true, ..StubAdapter::default() }
}

const ORDER: [&str; 6] = ["launch_adapter_for", "thread_launch", "adapter_for", "server_values_for", "aside", "revoke_device"];

fn run_host() -> (Rc<FakeHost>, SessionsArea) {
    let (host, area) = host();
    put(&host, "turn-1", "w1", json!({ "threadId": "t1", "status": "completed", "claudeSessionId": "agent-1" }), None);
    (host, area)
}

fn step(extra: Value) -> RunStep {
    let mut doc = json!({ "threadId": "t1", "turnId": "turn-1", "runId": "run-1", "block": "turn-1:m0@12", "command": "kill 60082 60083", "state": "running" });
    for (k, v) in extra.as_object().expect("extra is an object") {
        doc[k] = v.clone();
    }
    serde_json::from_value(doc).expect("a run step")
}

fn run_event(extra: Value) -> Value {
    let mut doc = json!({
        "type": "session.run", "workspaceId": "w1", "sessionId": "agent-1", "turnId": "turn-1", "threadId": "t1", "runId": "run-1",
        "block": "turn-1:m0@12", "command": "kill 60082 60083", "state": "running",
    });
    for (k, v) in extra.as_object().expect("extra is an object") {
        doc[k] = v.clone();
    }
    doc
}
