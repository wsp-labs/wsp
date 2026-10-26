use std::cell::RefCell;

use std::rc::Rc;

use std::sync::Arc;

use support::*;

use wsp_account::account::rules::TreeOf;

use wsp_core::console;

use wsp_core::types::wiring::{AccessMove, SteerTaken};

use wsp_machines::machines::boot::entries::RowAt;

use wsp_machines::machines::sub_spawn::machine::Resumed;

use wsp_threads::threads::forks::{ForkPicks, ForkPlanSource, ForkPlanTurn};

use wsp_threads::threads::sessions::start::{Checked, Prepared};

use wsp_wire::types::attachments::Attachment;

use wsp_wire::types::conversations::{RESUME_A_THREAD_FIX, RESUME_A_THREAD_LINE};

use wsp_wire::types::env::{THREAD_ENV, TURN_TOKEN_ENV};

use wsp_wire::views::harness::mcp_servers_blocked;

use wsp_wire::views::preferences::default_preferences;

use wsp_wire::views::session::{ForkedFrom, SessionOrigin};

use wsp_wire::views::workspace::WorkspacePhase;

use wsp_wire::wire::runtime::{SessionAnswerResult, SessionStartOutcome};

use wsp_words::attachments::{attachment_record, files_blocked};

use wsp_words::exit::refusal;

use wsp_words::sub_thread::{sub_machine_taken_refusal, sub_opening};

use wsp_words::words::computer::{
    replaces_working_fix, replaces_working_line, send_files_across_line, thread_word, wait_across_line, RESTART_OPENS_FIX,
    RESTART_OPENS_LINE, SEND_FILES_ACROSS_FIX, WAIT_ACROSS_FIX,
};

use wsp_words::words::thread::{
    fork_refused_line, forks_not_line, thread_runs_on_line, ASIDE_NO_SESSION_LINE, EMPTY_TITLE_LINE, FORKS_NOT_FIX, FORK_REFUSED_FIX,
};

use wsp_words::words::turn::title_line;

use wsp_words::workspace_state::{send_refusal, workspace_state, SendRefusalKind, WorkspaceStateInput};

macro_rules! hook {
    ($host:expr, $key:expr, $ty:ty, $f:expr) => {
        $host.hooks.set::<$ty>($key, Rc::new($f))
    };
}

// ---------------------------------------------------------------------------------------------------------------
// fixtures

fn err(message: &str) -> HostError {
    HostError::new(message)
}

fn scope(thread: &str, workspace: &str, root: &str) -> ThreadScope {
    serde_json::from_value(json!({ "kind": "thread", "threadId": thread, "workspaceId": workspace, "rootThreadId": root }))
        .expect("a thread scope")
}

fn by(origin: WorkspaceOrigin, s: ThreadScope) -> Caller {
    Caller::Scoped { origin, by: s }
}

fn go(prompt: &str) -> StartOptions {
    StartOptions { prompt: prompt.to_string(), ..Default::default() }
}

fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn outside(harness: &str, id: &str, copy: bool) -> OutsideOpening {
    OutsideOpening {
        harness: harness.into(),
        id: id.into(),
        copy,
        project: "p1".into(),
        cwd: None,
        title: "lab codewords".into(),
        earlier: Vec::new(),
    }
}

fn taken(thread: &str) -> Taken {
    serde_json::from_value(json!({ "sessionId": "sess-1", "threadId": thread, "turnId": "turn-1", "outcome": "started" }))
        .expect("a taken start")
}

fn facts(thread: &str, status: &str) -> ThreadFacts {
    serde_json::from_value(json!({
        "id": format!("sess-{thread}"), "threadId": thread, "workspaceId": "w1", "harness": "claude", "startedBy": "agent", "status": status,
        "title": "A", "sessionId": format!("sess-{thread}"), "turns": 1, "ran": true,
    }))
    .expect("thread facts")
}

fn sub_json(lead: bool) -> Value {
    let mut doc = json!({
        "name": "web",
        "sub": { "on": "boat", "repo": "https://github.com/acme/web", "base": { "branch": "main", "sha": "abc" }, "branch": "wsp/x", "folder": "/root/web" },
    });
    if lead {
        doc["lead"] = json!({ "threadId": "lead-t", "workspaceId": "w0" });
    }
    doc
}

/// A turn as a caller holds it, for the answers the context scripts.
struct Handle {
    id: String,
    turn: String,
}

impl Handle {
    #[allow(clippy::new_ret_no_self)]
    fn new(id: &str, turn: &str) -> Rc<dyn SessionHandle> {
        Rc::new(Handle { id: id.into(), turn: turn.into() })
    }
}

impl SessionHandle for Handle {
    fn id(&self) -> String {
        self.id.clone()
    }
    fn workspace_id(&self) -> String {
        "w1".into()
    }
    fn finished(&self) -> futures::future::Shared<LocalBoxFuture<'static, TurnResult>> {
        std::future::pending::<TurnResult>().boxed_local().shared()
    }
    fn turn_id(&self) -> String {
        self.turn.clone()
    }
    fn outcome(&self) -> SessionStartOutcome {
        SessionStartOutcome::Started
    }
    fn notice(&self) -> Option<String> {
        None
    }
    fn view(&self) -> SessionView {
        view(&self.id, "w1", json!({}))
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        async {}.boxed_local()
    }
    fn steer(&self, _p: String, _id: Option<String>, _i: Vec<TurnImage>) -> Option<Local<'static, SteerTaken>> {
        None
    }
    fn tells_unread(&self) -> Option<bool> {
        None
    }
    fn answer(&self, _a: String, _o: String, _r: Option<String>) -> Option<Local<'static, SessionAnswerResult>> {
        None
    }
    fn set_access(&self, _m: String) -> Option<Local<'static, AccessMove>> {
        None
    }
    fn stop_task(&self, _t: String) -> Option<Local<'static, TaskStop>> {
        None
    }
}

/// The adapter the checks meet: which roads it has is the only thing they read.
#[derive(Clone, Default)]
struct Agent {
    forks: bool,
    by_count: bool,
    takes_servers: bool,
    road: Option<AttachmentRoad>,
}

impl HarnessAdapter for Agent {
    fn start(&self, _o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        panic!("the checks never start a session")
    }
    fn steers(&self) -> bool {
        false
    }
    fn fork_session(&self, _s: String, _t: ForkTurn) -> Option<LocalBoxFuture<'static, Result<ForkedSession, String>>> {
        if self.forks {
            Some(futures::future::ready(Err("not forked here".to_string())).boxed_local())
        } else {
            None
        }
    }
    fn forks_by_count(&self) -> bool {
        self.by_count
    }
    fn mcp_servers(&self) -> bool {
        self.takes_servers
    }
    fn attachments(&self) -> Option<AttachmentRoad> {
        self.road
    }
}

struct Fx {
    host: Rc<FakeHost>,
    area: SessionsArea,
    asked: Rc<RefCell<Vec<(String, AdapterAsk)>>>,
}

fn fx() -> Fx {
    fx_on(Agent::default())
}

fn fx_on(agent: Agent) -> Fx {
    let host = FakeHost::new();
    host.add_entry(entry("w1", json!({ "name": "lab" })));
    host.ids.uuids.borrow_mut().push("thr-new".to_string());
    hook!(host, "LaunchCtx::preferences", dyn Fn() -> LocalBoxFuture<'static, Result<Preferences, HostError>>, || async {
        Ok(default_preferences())
    }
    .boxed_local());
    hook!(
        host,
        "SessionsCtx::fork_opened",
        dyn Fn(&str, StartOptions, Option<&Caller>) -> LocalBoxFuture<'static, Result<(StartOptions, Option<ForkPlan>), HostError>>,
        |_w: &str, o: StartOptions, _c: Option<&Caller>| async move { Ok((o, None)) }.boxed_local()
    );
    let asked = Rc::new(RefCell::new(Vec::new()));
    let seen = asked.clone();
    let forks = agent.forks;
    hook!(host, "SessionsCtx::adapter_forks", dyn Fn(&dyn HarnessAdapter) -> bool, move |_a: &dyn HarnessAdapter| forks);
    let agent: Arc<dyn HarnessAdapter> = Arc::new(agent);
    hook!(
        host,
        "LaunchCtx::adapter_for",
        dyn Fn(&Entry, AdapterAsk) -> Result<BuiltAdapter, HostError>,
        move |_e: &Entry, ask: AdapterAsk| {
            let h = ask.named.clone().expect("the harness is named");
            seen.borrow_mut().push((h.clone(), ask));
            Ok(BuiltAdapter { harness: h, adapter: agent.clone() })
        }
    );
    let area = SessionsArea::new(host.clone());
    Fx { host, area, asked }
}

impl Fx {
    async fn prepare_with(
        &self,
        claimed: &RefCell<Option<String>>,
        opts: StartOptions,
        origin: Option<Caller>,
    ) -> Result<Checked, HostError> {
        self.area.prepare("w1", opts, origin, claimed).await
    }

    async fn prepare(&self, opts: StartOptions, origin: Option<Caller>) -> Result<Checked, HostError> {
        self.prepare_with(&RefCell::new(None), opts, origin).await
    }

    async fn launches(&self, opts: StartOptions, origin: Option<Caller>) -> Box<Prepared> {
        match self.prepare(opts, origin).await {
            Ok(Checked::Launch(p)) => p,
            Ok(Checked::Answered(_)) => panic!("the start was answered, not prepared"),
            Err(e) => panic!("the start was refused: {}", e.message),
        }
    }

    async fn refused(&self, opts: StartOptions, origin: Option<Caller>) -> HostError {
        match self.prepare(opts, origin).await {
            Ok(Checked::Launch(_)) => panic!("the start was prepared, not refused"),
            Ok(Checked::Answered(_)) => panic!("the start was answered, not refused"),
            Err(e) => e,
        }
    }

    /// The thread's record and one finished row, on `workspace`.
    fn thread_on(&self, thread: &str, workspace: &str, harness: &str) {
        thread_in(&self.host, thread, workspace, harness, "completed");
    }
}

fn thread_in(host: &FakeHost, thread: &str, workspace: &str, harness: &str, status: &str) {
    host.add_record(thread, workspace, thread_record(json!({ "harness": harness })));
    let id = format!("sess-{thread}");
    let r = row(
        view(
            &id,
            workspace,
            json!({ "threadId": thread, "harness": harness, "status": status, "startedAt": T0, "claudeSessionId": format!("cs-{thread}") }),
        ),
        &format!("turn-{thread}"),
    );
    host.add_row(&id, r);
}

/// The calls of these methods, in the order they were made.
fn order(host: &FakeHost, names: &[&str]) -> Vec<String> {
    host.hooks.calls().into_iter().filter(|c| names.iter().any(|n| c == n || c.starts_with(&format!("{n}(")))).collect()
}

fn nothing_held(host: &FakeHost) {
    assert!(
        host.rows.borrow().keys().all(|k| k.starts_with("sess-")),
        "a refusal holds no row of its own (only the fixtures' threads have rows)"
    );
    assert!(host.rows.borrow().values().all(|r| r.borrow().launch.is_none()), "a refusal leaves no launching row behind");
    assert_eq!(host.hooks.called("cap_hold"), Vec::<String>::new(), "a refusal holds no place under a cap");
}

fn before_the_mint(host: &FakeHost) {
    nothing_held(host);
    assert_eq!(host.hooks.called("thread_launch"), Vec::<String>::new(), "nothing is minted before the checks pass");
}

fn usage(line: &str, fix: &str) -> HostError {
    refusal(line, fix, Some(Kind::Usage))
}

// ---------------------------------------------------------------------------------------------------------------
// the mint and S25

fn minted(fx: &Fx) {
    hook!(
        fx.host,
        "LaunchCtx::thread_launch",
        dyn Fn(&Entry, &str, &str, bool) -> LocalBoxFuture<'static, Result<ThreadLaunch, HostError>>,
        |_e: &Entry, _t: &str, _r: &str, _a: bool| {
            let mut env = IndexMap::new();
            env.insert("WSP_HOST".to_string(), "h-tok".to_string());
            async move {
                Ok(ThreadLaunch {
                    scoped_device_id: Some("dev-1".into()),
                    env,
                    wsp: Some(McpServerSpec { command: "wsp".into(), args: strings(&["mcp"]), no_slate: None }),
                })
            }
            .boxed_local()
        }
    );
}

fn plan_with(has_turn: bool, session: Option<&str>) -> ForkPlan {
    ForkPlan {
        source: ForkPlanSource {
            thread_id: "src-t".into(),
            workspace_id: "w1".into(),
            entry: entry("w1", json!({})),
            harness: "claude".into(),
            title: "Source".into(),
            session: session.map(String::from),
            cwd: None,
        },
        turn: if has_turn {
            Some(ForkPlanTurn { turn_id: "turn-src".into(), anchor: None, at_ref: None, after: Vec::new() })
        } else {
            None
        },
        events: Vec::new(),
        picks: ForkPicks::default(),
        forked_from: ForkedFrom { thread_id: "src-t".into(), turn_id: None, title: "Source".into() },
    }
}

/// `ForkPlan` has no equality: the fields a plan is told apart by.
fn same_plan(got: &Option<ForkPlan>, want: &Option<ForkPlan>) {
    match (got, want) {
        (None, None) => {}
        (Some(g), Some(w)) => {
            assert_eq!(g.source.thread_id, w.source.thread_id);
            assert_eq!(g.source.session, w.source.session);
            assert_eq!(g.turn, w.turn);
            assert_eq!(g.events.len(), w.events.len());
            assert_eq!(g.picks, w.picks);
            assert_eq!(g.forked_from, w.forked_from);
        }
        _ => panic!(
            "the plan was {}, and was to be {}",
            if got.is_some() { "held" } else { "none" },
            if want.is_some() { "held" } else { "none" }
        ),
    }
}

fn id_of(entry: &Entry) -> String {
    entry.borrow().record.borrow().id.clone()
}

fn forking(fx: &Fx, plan: ForkPlan) {
    hook!(
        fx.host,
        "SessionsCtx::fork_opened",
        dyn Fn(&str, StartOptions, Option<&Caller>) -> LocalBoxFuture<'static, Result<(StartOptions, Option<ForkPlan>), HostError>>,
        move |_w: &str, o: StartOptions, _c: Option<&Caller>| {
            let plan = plan.clone();
            async move { Ok((o, Some(plan))) }.boxed_local()
        }
    );
}
