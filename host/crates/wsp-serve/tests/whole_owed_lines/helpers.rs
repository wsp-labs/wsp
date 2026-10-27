const REPORT: &str = "Parsed 40 files; the suite passes.";

const MIN: i64 = 60_000;

const HOUR: i64 = 60 * MIN;

async fn tick() {
    sleep_ms(50).await;
}

/// What a provider's refusal of a launch comes to once the stream wraps it: the machine is gone.
const LAUNCH_REFUSED: &str = "remote launch failed on m1: gone";

fn launch_lost() -> ExecFailure {
    ExecFailure { kind: ExecFailureKind::Unreached, message: "machine.exec on lab-box was not answered in 35s".into() }
}

fn launch_refused() -> ExecFailure {
    ExecFailure { kind: ExecFailureKind::Other, message: LAUNCH_REFUSED.into() }
}

/// A Claude Code that announces no msg_lifecycle_v1, so its turns tell no unread message.
fn old_init() -> String {
    let mut doc: Value = serde_json::from_str(&init()).expect("an init");
    doc["claude_code_version"] = json!("2.1.200");
    doc["capabilities"] = json!(["interrupt_receipt_v1"]);
    doc.to_string()
}

// A Codex app server's lines, as 0.155.1 prints them, trimmed to the fields the adapter reads.
const THREAD_ID: &str = "01a0e2c1-5d10-7b42-9a6e-3f1c2d4b5a60";

const TURN_ID: &str = "01a0e2c1-5e02-7c11-8d3f-9b2a1c0d4e71";

fn opened() -> Vec<String> {
    vec![
        r#"{"id":"wsp-initialize","result":{"userAgent":"wsp/0.155.1","codexHome":"/root/.codex","platformFamily":"unix","platformOs":"linux"}}"#.into(),
        format!(
            r#"{{"id":"wsp-thread","result":{{"thread":{{"id":"{THREAD_ID}","model":"gpt-5.6-sol","cwd":"/root/app","turns":[]}},"model":"gpt-5.6-sol","cwd":"/root/app","approvalPolicy":"on-request","sandbox":{{"type":"workspaceWrite"}}}}}}"#
        ),
    ]
}

fn turn_started() -> String {
    format!(
        r#"{{"method":"turn/started","params":{{"threadId":"{THREAD_ID}","turn":{{"id":"{TURN_ID}","items":[],"status":"inProgress"}}}}}}"#
    )
}

fn agent_message() -> String {
    format!(
        r#"{{"method":"item/completed","params":{{"item":{{"type":"agentMessage","id":"m1","text":"waiting"}},"threadId":"{THREAD_ID}","turnId":"{TURN_ID}"}}}}"#
    )
}

fn completed() -> String {
    format!(
        r#"{{"method":"turn/completed","params":{{"threadId":"{THREAD_ID}","turn":{{"id":"{TURN_ID}","items":[],"status":"completed"}}}}}}"#
    )
}

/// The line a recorded app-server run printed at `line` (1-based), its thread named as this case's.
fn recorded(fixture: &str, line: usize) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../packages/adapter-codex/test/fixtures").join(fixture);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let raw = text.split('\n').nth(line - 1).expect("the line").to_string();
    let named: Value = serde_json::from_str(&raw).expect("a json line");
    match named["params"]["threadId"].as_str() {
        None => raw,
        Some(id) => raw.split(id).collect::<Vec<_>>().join(THREAD_ID),
    }
}

// The warnings two recorded runs printed between the thread's answer and turn/started, and the one a server without bubblewrap prints
// right after initialize.
struct Warning {
    line: String,
    early: bool,
}

fn warnings() -> Vec<Warning> {
    vec![
        Warning { line: recorded("app-server-turn.jsonl", 5), early: false },
        Warning { line: recorded("app-server-subagent.jsonl", 5), early: false },
        Warning { line: recorded("catalog-probe-route.jsonl", 2), early: true },
    ]
}

/// Each run carries the workspace it was launched for, so a case with two leads tells their launches apart.
struct Tagged {
    inner: Arc<dyn ExecStreamFactory>,
    workspace: String,
}

impl ExecStreamFactory for Tagged {
    fn launch(&self, command: &str, mut o: ExecOptions) -> Arc<dyn ExecStream> {
        o.env.insert("LAB_WORKSPACE".into(), self.workspace.clone());
        self.inner.launch(command, o)
    }
    fn attaches(&self) -> bool {
        self.inner.attaches()
    }
    fn attach(&self, run: &str, options: AttachRunOptions) -> Option<LocalBoxFuture<'static, Result<Attached, ExecFailure>>> {
        self.inner.attach(run, options)
    }
}

/// An attach the case can make go unanswered, as a computer that is dark when the next host re-opens its runs, or hold until it lets
/// go, as a cloud machine still restoring answers nothing for a while.
struct Reopening {
    inner: Arc<dyn ExecStreamFactory>,
    dark: Rc<Cell<bool>>,
    gate: Rc<RefCell<Option<Rc<Gate>>>>,
}

impl ExecStreamFactory for Reopening {
    fn launch(&self, command: &str, o: ExecOptions) -> Arc<dyn ExecStream> {
        self.inner.launch(command, o)
    }
    fn attaches(&self) -> bool {
        true
    }
    fn attach(&self, run: &str, options: AttachRunOptions) -> Option<LocalBoxFuture<'static, Result<Attached, ExecFailure>>> {
        let (inner, dark, gate, run) = (self.inner.clone(), self.dark.clone(), self.gate.clone(), run.to_string());
        Some(
            async move {
                let held = gate.borrow().clone();
                if let Some(held) = held {
                    held.wait().await;
                }
                if dark.get() {
                    return Err(launch_lost());
                }
                inner.attach(&run, options).expect("an attach").await
            }
            .boxed_local(),
        )
    }
}

fn claude_over(exec: Arc<dyn ExecStreamFactory>, result_exit_ms: u64) -> Result<Arc<dyn HarnessAdapter>, HostError> {
    let mut deps = AdapterDeps::new(exec, "/root/.claude-cfg");
    deps.result_exit_ms = Some(result_exit_ms);
    let adapter = create_claude_adapter(deps).map_err(|e| HostError::new(e.0))?;
    Ok(Arc::new(adapter) as Arc<dyn HarnessAdapter>)
}

fn codex_over(exec: Arc<dyn ExecStreamFactory>) -> Result<Arc<dyn HarnessAdapter>, HostError> {
    let adapter = create_codex_adapter(CodexAdapterDeps::new(exec, "/root/.codex", "codex login")).map_err(|e| HostError::new(e.0))?;
    Ok(Arc::new(adapter) as Arc<dyn HarnessAdapter>)
}

fn lead_of(name: &str) -> CreateOn {
    CreateOn { agents: Some(serde_json::Map::new()), ..CreateOn::golden(name) }
}

async fn history(rt: &Rc<AssembledRuntime>, ws: &str) -> Vec<Value> {
    rt.sessions().history(ws, None).await.expect("a history").iter().map(j).collect()
}

async fn done_count(rt: &Rc<AssembledRuntime>, ws: &str) -> usize {
    history(rt, ws).await.iter().filter(|e| e["type"] == "session.done").count()
}

async fn has_type(rt: &Rc<AssembledRuntime>, ws: &str, kind: &'static str) -> bool {
    history(rt, ws).await.iter().any(|e| e["type"] == kind)
}

fn writes_of(run: &Rc<Run>, words: &str) -> bool {
    run.writes().iter().any(|w| w.contains(words))
}

// ===== a child's finished line its lead cannot take yet =====

#[derive(Default)]
struct Opts {
    lead_notify: Option<Vec<String>>,
    result_exit_ms: Option<u64>,
    lead_init: Option<String>,
}

/// The handles of the runtime a case holds now.
struct Rt {
    rt: Rc<AssembledRuntime>,
    store: Rc<dyn wsp_store::store::Store>,
}

struct Refusing {
    on: RefCell<Option<String>>,
    refused: Cell<u32>,
}

struct Setup {
    fed: Rc<FedRuns>,
    factory: Arc<dyn ExecStreamFactory>,
    backend: Arc<StubBackend>,
    state: Rc<State>,
    refusing: Rc<Refusing>,
    result_exit_ms: u64,
    rt: RefCell<Fx>,
    lead_run: Rc<Run>,
    lead_ws: String,
    lead: Rc<dyn SessionHandle>,
    lead_thread: String,
    kid_ws: String,
    kid_thread: RefCell<Option<String>>,
}

fn host(s: &SetupParts, on: &FakeClock) -> Fx {
    let (factory, refusing, exit_ms) = (s.factory.clone(), s.refusing.clone(), s.result_exit_ms);
    // Once a case says so, the agent refuses outright to start on the lead's folder, a plain refusal rather than a computer that did not
    // answer; each one it refused is counted.
    let claude: HarnessAdapterFactory = Arc::new(move |ctx| {
        if refusing.on.borrow().as_deref() == Some(ctx.workspace_id.as_str()) {
            refusing.refused.set(refusing.refused.get() + 1);
            return Err(HostError::new("claude will not start here"));
        }
        claude_over(Arc::new(Tagged { inner: factory.clone(), workspace: ctx.workspace_id.clone() }), exit_ms)
    });
    runtime(&s.backend).store(&s.state.store).clock(on).adapter("claude", claude).build()
}

struct SetupParts {
    factory: Arc<dyn ExecStreamFactory>,
    backend: Arc<StubBackend>,
    state: Rc<State>,
    refusing: Rc<Refusing>,
    result_exit_ms: u64,
}

/// `leadNotify` is who the lead's own ends tell, and `resultExitMs` how long a reply held for an open message waits for the CLI before
/// it stands.
async fn setup(clock: &FakeClock, o: Opts) -> Setup {
    let (factory, fed) = fed_runs();
    let fed = Rc::new(fed);
    let parts = SetupParts {
        factory,
        backend: StubBackend::new(None),
        state: Rc::new(sqlite_state()),
        refusing: Rc::new(Refusing { on: RefCell::new(None), refused: Cell::new(0) }),
        result_exit_ms: o.result_exit_ms.unwrap_or(60_000),
    };
    let rt = host(&parts, clock);
    let lead_ws = create_on(&rt.rt, lead_of("lead"), None).await.expect("a workspace").view.id;
    let start = StartOptions { prompt: "orchestrate".into(), notify: o.lead_notify.clone(), ..Default::default() };
    let lead = rt.rt.sessions().start(&lead_ws, start, None).await.expect("a start");
    until(|| async { fed.run_of("orchestrate").is_some() }).await;
    let lead_run = fed.run_of("orchestrate").expect("the run");
    lead_run.push(&[o.lead_init.unwrap_or_else(init)]);
    until(|| async { has_type(&rt.rt, &lead_ws, "session.start").await }).await;
    let lead_thread = lead.view().thread_id.expect("a thread");
    let kid_ws = create_on(&rt.rt, CreateOn::golden("kid"), None).await.expect("a workspace").view.id;
    Setup {
        fed,
        factory: parts.factory,
        backend: parts.backend,
        state: parts.state,
        refusing: parts.refusing,
        result_exit_ms: parts.result_exit_ms,
        rt: RefCell::new(rt),
        lead_run,
        lead_ws,
        lead,
        lead_thread,
        kid_ws,
        kid_thread: RefCell::new(None),
    }
}

impl Setup {
    fn rt(&self) -> Rt {
        let held = self.rt.borrow();
        Rt { rt: held.rt.clone(), store: held.store.clone() }
    }

    fn parts(&self) -> SetupParts {
        SetupParts {
            factory: self.factory.clone(),
            backend: self.backend.clone(),
            state: self.state.clone(),
            refusing: self.refusing.clone(),
            result_exit_ms: self.result_exit_ms,
        }
    }

    async fn close(&self) {
        let rt = self.rt();
        rt.rt.close().await.expect("a close");
    }

    /// The lead replies and its process exits, as a coordinator's turn ends to wait on its children.
    async fn lead_waits(&self) {
        self.lead_run.push(&[said("msg_l", "waiting"), result("waiting", 1, None)]);
        let rt = self.rt();
        until(|| async { has_type(&rt.rt, &self.lead_ws, "session.done").await }).await;
        self.lead_run.end(Some(0));
        self.lead.finished().await;
    }

    /// The lead replies with a message still open in its turn, so its reply is held until its process exits.
    async fn lead_exits(&self) {
        self.lead_run.push(&[said("msg_l", "waiting"), result("waiting", 1, None)]);
        tick().await;
        self.lead_run.end(Some(0));
        self.lead.finished().await;
    }

    /// A child opened with the lead as its target, or the targets named, ends with REPORT.
    async fn kid_ends(&self, notify: Option<Vec<String>>, meanwhile: Option<&dyn Fn()>) {
        let rt = self.rt();
        let opts = StartOptions {
            prompt: "parse the files".into(),
            notify: Some(notify.unwrap_or_else(|| vec![self.lead_thread.clone()])),
            ..Default::default()
        };
        let kid = rt.rt.sessions().start(&self.kid_ws, opts, None).await.expect("a start");
        let thread = kid.view().thread_id.expect("a thread");
        *self.kid_thread.borrow_mut() = Some(thread.clone());
        until(|| async { self.fed.run_of("parse the files").is_some() }).await;
        let run = self.fed.run_of("parse the files").expect("the run");
        if let Some(meanwhile) = meanwhile {
            meanwhile();
        }
        run.push(&[init(), said("msg_k", REPORT), result(REPORT, 1, None)]);
        let ws = kid.view().workspace_id.clone();
        until(|| async { history(&rt.rt, &ws).await.iter().any(|e| e["type"] == "session.done" && e["threadId"] == thread.as_str()) })
            .await;
        run.end(Some(0));
        kid.finished().await;
        tick().await;
    }

    fn kid_thread(&self) -> String {
        self.kid_thread.borrow().clone().expect("the child's thread")
    }

    /// The rows the person was given for a thread's lines: the child's by default.
    async fn to_person(&self, thread: Option<&str>) -> Vec<String> {
        let rt = self.rt();
        let thread = thread.map_or_else(|| self.kid_thread(), str::to_string);
        let mut all = history(&rt.rt, &self.kid_ws).await;
        all.extend(history(&rt.rt, &self.lead_ws).await);
        all.into_iter()
            .filter(|e| e["type"] == "session.notify" && e["notify"] == NOTIFY_ME && e["threadId"] == thread.as_str())
            .map(|e| e["text"].as_str().expect("a line").to_string())
            .collect()
    }

    /// The lead's runs opened with the child's line, in launch order.
    fn launches(&self) -> Vec<Rc<Run>> {
        self.fed.runs_of(REPORT)
    }

    /// Every write of the child's line into a running turn of the lead.
    fn steers(&self) -> Vec<String> {
        self.lead_run.writes().into_iter().filter(|w| w.contains(REPORT)).collect()
    }

    /// The clock moves in steps until the lead's nth launch of the line shows.
    async fn next_try(&self, on: &FakeClock, n: usize) {
        for _ in 0..40 {
            if self.launches().len() >= n {
                break;
            }
            on.advance(5_000);
            tick().await;
        }
        assert_eq!(self.launches().len(), n);
    }

    /// A launch the lead's agent took up: it announces itself, answers and exits.
    async fn taken(&self, n: usize) {
        let run = self.launches()[n].clone();
        run.push(&[init(), said(&format!("msg_t{n}"), "read it"), result("read it", 1, None)]);
        let rt = self.rt();
        until(|| async { done_count(&rt.rt, &self.lead_ws).await >= 2 }).await;
        run.end(Some(0));
        tick().await;
    }

    async fn owed(&self) -> usize {
        self.rt().store.list(NOTIFY_OWED).await.expect("a list").len()
    }

    /// The host goes down and a second one starts on the same store, on a clock that goes on from where the first stood.
    async fn restart(&self, next: &FakeClock) {
        self.close().await;
        let again = host(&self.parts(), next);
        *self.rt.borrow_mut() = again;
        self.rt().rt.sessions().list(Some(self.lead_ws.clone()), None).await.expect("a list");
    }

    /// Steps the clock a minute at a time for `minutes`, failing every launch of the line with `fails` as it shows.
    async fn keep_failing(&self, on: &FakeClock, minutes: usize, fails: fn() -> ExecFailure) {
        let mut failed: Vec<Rc<Run>> = vec![];
        for _ in 0..minutes {
            for run in self.launches() {
                if failed.iter().any(|f| Rc::ptr_eq(f, &run)) {
                    continue;
                }
                failed.push(run.clone());
                run.fail_with(fails());
            }
            tick().await;
            on.advance(MIN);
            tick().await;
        }
    }

    fn refuse_starts(&self) {
        *self.refusing.on.borrow_mut() = Some(self.lead_ws.clone());
    }

    fn refused_starts(&self) -> u32 {
        self.refusing.refused.get()
    }
}

// ===== a Codex lead =====

/// A host over a store, the real Claude Code and Codex adapters over runs fed by hand.
fn codex_host(backend: &Arc<StubBackend>, state: &State, exec: &Arc<dyn ExecStreamFactory>, on: &FakeClock) -> Fx {
    let (for_claude, for_codex) = (exec.clone(), exec.clone());
    let claude: HarnessAdapterFactory = Arc::new(move |_ctx| claude_over(for_claude.clone(), 60_000));
    let codex: HarnessAdapterFactory = Arc::new(move |_ctx| codex_over(for_codex.clone()));
    runtime(backend).store(&state.store).clock(on).adapter("claude", claude).adapter("codex", codex).build()
}

async fn person_rows(rt: &Rc<AssembledRuntime>, kid_ws: &str, lead_ws: &str, kid_thread: &str) -> usize {
    let mut all = history(rt, kid_ws).await;
    all.extend(history(rt, lead_ws).await);
    all.iter().filter(|e| e["type"] == "session.notify" && e["notify"] == NOTIFY_ME && e["threadId"] == kid_thread).count()
}

// ===== after a restart =====

// ===== a Codex lead's line try =====

struct CodexLead {
    fed: FedRuns,
    rt: Fx,
    state: Rc<State>,
    kid_ws: String,
    lead_ws: String,
    kid_thread: String,
    at: FakeClock,
    try_run: Rc<Run>,
}

/// A Codex lead whose first turn has ended, and a child of it that ended with REPORT: the line's first try is the lead's third run.
async fn codex_lead() -> CodexLead {
    let (factory, fed) = fed_runs();
    let backend = StubBackend::new(None);
    let state = Rc::new(sqlite_state());
    let at = FakeClock::at_now();
    let rt = codex_host(&backend, &state, &factory, &at);
    let lead_ws = create_on(&rt.rt, lead_of("lead"), None).await.expect("a workspace").view.id;
    let lead = rt
        .rt
        .sessions()
        .start(&lead_ws, StartOptions { prompt: "orchestrate".into(), harness: Some("codex".into()), ..Default::default() }, None)
        .await
        .expect("a start");
    until(|| async { fed.runs.borrow().len() == 1 }).await;
    let lead_run = fed.runs.borrow()[0].clone();
    lead_run.push(&opened());
    until(|| async { writes_of(&lead_run, "turn/start") }).await;
    lead_run.push(&[turn_started(), agent_message(), completed()]);
    until(|| async { has_type(&rt.rt, &lead_ws, "session.done").await }).await;
    lead_run.end(Some(0));
    lead.finished().await;
    let kid_ws = create_on(&rt.rt, CreateOn::golden("kid"), None).await.expect("a workspace").view.id;
    let opts = StartOptions {
        prompt: "parse the files".into(),
        notify: Some(vec![lead.view().thread_id.expect("a thread")]),
        ..Default::default()
    };
    let kid = rt.rt.sessions().start(&kid_ws, opts, None).await.expect("a start");
    let kid_thread = kid.view().thread_id.expect("a thread");
    until(|| async { fed.run_of("parse the files").is_some() }).await;
    let kid_run = fed.run_of("parse the files").expect("the run");
    kid_run.push(&[init(), said("msg_k", REPORT), result(REPORT, 1, None)]);
    until(|| async { history(&rt.rt, &kid_ws).await.iter().any(|e| e["type"] == "session.done" && e["threadId"] == kid_thread.as_str()) })
        .await;
    kid_run.end(Some(0));
    kid.finished().await;
    tick().await;
    until(|| async { fed.runs.borrow().len() == 3 }).await;
    let try_run = fed.runs.borrow()[2].clone();
    CodexLead { fed, rt, state, kid_ws, lead_ws, kid_thread, at, try_run }
}

impl CodexLead {
    async fn to_person(&self) -> usize {
        person_rows(&self.rt.rt, &self.kid_ws, &self.lead_ws, &self.kid_thread).await
    }
    async fn steps(&self, n: usize, ms: i64) {
        for _ in 0..n {
            self.at.advance(ms);
            tick().await;
        }
    }
    fn runs(&self) -> usize {
        self.fed.runs.borrow().len()
    }
    async fn owed(&self) -> Vec<Value> {
        self.rt.store.list(NOTIFY_OWED).await.expect("a list").iter().map(|v| json!(format!("{v:?}"))).collect()
    }
}

// ===== two hosts on one store =====

/// A host or two on one store, with the real Claude Code and Codex adapters over runs fed by hand, and an attach the case can make go
/// unanswered or hold until it lets go.
struct TwoHosts {
    fed: Rc<FedRuns>,
    factory: Arc<dyn ExecStreamFactory>,
    dark: Rc<Cell<bool>>,
    gate: Rc<RefCell<Option<Rc<Gate>>>>,
    backend: Arc<StubBackend>,
    state: Rc<State>,
}

fn two_hosts() -> TwoHosts {
    let (fed_factory, fed) = fed_runs();
    let (dark, gate): (Rc<Cell<bool>>, Rc<RefCell<Option<Rc<Gate>>>>) = (Rc::default(), Rc::default());
    let factory: Arc<dyn ExecStreamFactory> = Arc::new(Reopening { inner: fed_factory, dark: dark.clone(), gate: gate.clone() });
    TwoHosts { fed: Rc::new(fed), factory, dark, gate, backend: StubBackend::new(None), state: Rc::new(sqlite_state()) }
}

impl TwoHosts {
    fn host(&self, on: &FakeClock) -> Fx {
        codex_host(&self.backend, &self.state, &self.factory, on)
    }
    fn go_dark(&self) {
        self.dark.set(true);
    }
    fn hold(&self) {
        *self.gate.borrow_mut() = Some(Gate::new());
    }
    fn release(&self) {
        let held = self.gate.borrow_mut().take();
        if let Some(held) = held {
            held.open();
        }
    }
    async fn owed(&self) -> usize {
        self.state.store.list(NOTIFY_OWED).await.expect("a list").len()
    }
}

struct Dark {
    h: TwoHosts,
    b: FakeClock,
    lead_ws: String,
    kid_ws: String,
    kid_thread: String,
    rt: Fx,
}

/// A Claude Code lead, kept awake, whose child ended with REPORT; then the host goes down with the line's try out and unannounced, and
/// the next host comes up while the lead's computer answers nothing.
async fn dark_restart() -> Dark {
    let h = two_hosts();
    let a = FakeClock::at_now();
    let rt = h.host(&a);
    let lead_ws = create_on(&rt.rt, CreateOn { idle_window_ms: wsp_core::types::wiring::IdleWindow::Off, ..lead_of("lead") }, None)
        .await
        .expect("a workspace")
        .view
        .id;
    let lead =
        rt.rt.sessions().start(&lead_ws, StartOptions { prompt: "orchestrate".into(), ..Default::default() }, None).await.expect("a start");
    until(|| async { h.fed.run_of("orchestrate").is_some() }).await;
    h.fed.run_of("orchestrate").expect("the run").push(&[init(), said("msg_l", "waiting"), result("waiting", 1, None)]);
    until(|| async { has_type(&rt.rt, &lead_ws, "session.done").await }).await;
    h.fed.run_of("orchestrate").expect("the run").end(Some(0));
    lead.finished().await;
    let kid_ws = create_on(&rt.rt, CreateOn::golden("kid"), None).await.expect("a workspace").view.id;
    let opts = StartOptions {
        prompt: "parse the files".into(),
        notify: Some(vec![lead.view().thread_id.expect("a thread")]),
        ..Default::default()
    };
    let kid = rt.rt.sessions().start(&kid_ws, opts, None).await.expect("a start");
    let kid_thread = kid.view().thread_id.expect("a thread");
    until(|| async { h.fed.run_of("parse the files").is_some() }).await;
    h.fed.run_of("parse the files").expect("the run").push(&[init(), said("msg_k", REPORT), result(REPORT, 1, None)]);
    until(|| async { has_type(&rt.rt, &kid_ws, "session.done").await }).await;
    h.fed.run_of("parse the files").expect("the run").end(Some(0));
    kid.finished().await;
    tick().await;
    assert_eq!(h.fed.runs_of(REPORT).len(), 1);
    h.go_dark();
    rt.rt.close().await.expect("a close");
    let b = FakeClock::new(a.now());
    let rt = h.host(&b);
    rt.rt.sessions().list(Some(lead_ws.clone()), None).await.expect("a list");
    tick().await;
    Dark { h, b, lead_ws, kid_ws, kid_thread, rt }
}

impl Dark {
    async fn to_person(&self) -> usize {
        person_rows(&self.rt.rt, &self.kid_ws, &self.lead_ws, &self.kid_thread).await
    }
    async fn ends_of_lead(&self) -> usize {
        history(&self.rt.rt, &self.lead_ws).await.iter().filter(|e| e["type"] == "session.end").count()
    }
}

/// The rows a turn wrote, in order.
fn rows_of(history: &[Value], turn: &str) -> Vec<String> {
    history
        .iter()
        .filter(|e| e["turnId"] == turn)
        .map(|e| {
            if e["type"] == "session.delta" {
                format!("delta:{}", e["kind"].as_str().unwrap_or_default())
            } else {
                e["type"].as_str().unwrap_or_default().to_string()
            }
        })
        .collect()
}

/// A lead whose child ended with REPORT; the host goes down with the line's try out, and its agent takes the line and answers it
/// meanwhile. The next host comes up `after` minutes later, and its re-open of the try answers only once a timer due at once has fired,
/// as a cloud machine still restoring does.
async fn up_later(after: i64) -> (Vec<String>, Vec<Value>, usize, usize) {
    let h = two_hosts();
    let a = FakeClock::at_now();
    let mut rt = h.host(&a);
    let lead_ws = create_on(&rt.rt, CreateOn { idle_window_ms: wsp_core::types::wiring::IdleWindow::Off, ..lead_of("lead") }, None)
        .await
        .expect("a workspace")
        .view
        .id;
    let lead =
        rt.rt.sessions().start(&lead_ws, StartOptions { prompt: "orchestrate".into(), ..Default::default() }, None).await.expect("a start");
    until(|| async { h.fed.run_of("orchestrate").is_some() }).await;
    h.fed.run_of("orchestrate").expect("the run").push(&[init(), said("msg_l", "waiting"), result("waiting", 1, None)]);
    until(|| async { has_type(&rt.rt, &lead_ws, "session.done").await }).await;
    h.fed.run_of("orchestrate").expect("the run").end(Some(0));
    lead.finished().await;
    let kid_ws = create_on(&rt.rt, CreateOn::golden("kid"), None).await.expect("a workspace").view.id;
    let opts = StartOptions {
        prompt: "parse the files".into(),
        notify: Some(vec![lead.view().thread_id.expect("a thread")]),
        ..Default::default()
    };
    let kid = rt.rt.sessions().start(&kid_ws, opts, None).await.expect("a start");
    let kid_thread = kid.view().thread_id.expect("a thread");
    until(|| async { h.fed.run_of("parse the files").is_some() }).await;
    h.fed.run_of("parse the files").expect("the run").push(&[init(), said("msg_k", REPORT), result(REPORT, 1, None)]);
    until(|| async { has_type(&rt.rt, &kid_ws, "session.done").await }).await;
    h.fed.run_of("parse the files").expect("the run").end(Some(0));
    kid.finished().await;
    tick().await;
    let try_run = h.fed.runs_of(REPORT)[0].clone();
    rt.rt.close().await.expect("a close");
    h.hold();
    let b = FakeClock::new(a.now() + after * MIN);
    rt = h.host(&b);
    let listing = {
        let (rt, ws) = (rt.rt.clone(), lead_ws.clone());
        tokio::task::spawn_local(async move { rt.sessions().list(Some(ws), None).await })
    };
    tick().await;
    b.advance(1);
    tick().await;
    h.release();
    tick().await;
    try_run.push(&[
        init(),
        said("msg_t", "Read the parser report; merging it next."),
        result("Read the parser report; merging it next.", 1, None),
    ]);
    tick().await;
    try_run.end(Some(0));
    for _ in 0..10 {
        tick().await;
        b.advance(10_000);
    }
    let _ = listing.await;
    let lead_history = history(&rt.rt, &lead_ws).await;
    let try_turn =
        lead_history.iter().rfind(|e| e["type"] == "session.end").expect("an end")["turnId"].as_str().expect("a turn").to_string();
    let to_person: Vec<Value> = history(&rt.rt, &kid_ws)
        .await
        .into_iter()
        .filter(|e| e["type"] == "session.notify" && e["notify"] == NOTIFY_ME && e["threadId"] == kid_thread.as_str())
        .collect();
    let rows = rows_of(&lead_history, &try_turn);
    let owed = h.owed().await;
    let launches = h.fed.runs_of(REPORT).len();
    rt.rt.close().await.expect("a close");
    (rows, to_person, launches, owed)
}

fn took_the_line(r: (Vec<String>, Vec<Value>, usize, usize)) {
    let (rows, to_person, launches, owed) = r;
    assert!(to_person.is_empty());
    assert_eq!(rows, ["session.start", "delta:text", "session.done", "session.end"]);
    assert_eq!(launches, 1);
    assert_eq!(owed, 0);
}

async fn warning_case(w: Warning) {
    let h = two_hosts();
    let rt = h.host(&FakeClock::at_now());
    let ws = create_on(&rt.rt, CreateOn::golden("lead"), None).await.expect("a workspace").view.id;
    let lead = rt
        .rt
        .sessions()
        .start(&ws, StartOptions { prompt: "orchestrate".into(), harness: Some("codex".into()), ..Default::default() }, None)
        .await
        .expect("a start");
    until(|| async { h.fed.runs.borrow().len() == 1 }).await;
    let run = h.fed.runs.borrow()[0].clone();
    let lines =
        if w.early { vec![opened()[0].clone(), w.line.clone(), opened()[1].clone()] } else { [opened(), vec![w.line.clone()]].concat() };
    run.push(&lines);
    until(|| async { writes_of(&run, "turn/start") }).await;
    run.push(&[turn_started(), agent_message(), completed()]);
    until(|| async { has_type(&rt.rt, &ws, "session.done").await }).await;
    run.end(Some(0));
    lead.finished().await;
    let rows: Vec<String> = history(&rt.rt, &ws)
        .await
        .iter()
        .filter(|e| e["type"] == "session.start" || e["type"] == "session.delta" || e["type"] == "session.done")
        .map(|e| {
            if e["type"] == "session.delta" {
                format!("delta:{}", e["kind"].as_str().unwrap_or_default())
            } else {
                e["type"].as_str().unwrap_or_default().to_string()
            }
        })
        .collect();
    assert_eq!(rows, ["session.start", "delta:note", "delta:text", "session.done"]);
    rt.rt.close().await.expect("a close");
}
