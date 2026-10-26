// SPDX-License-Identifier: AGPL-3.0-only
//! spec/sessions_start.md. TS cases: verbs-send-notify.test.ts:91 (S2), conversations.test.ts:683 (S5, S16),
//! thread-warm-agent.test.ts:82, :186, :197, :213, :241 (S7, S8, S26), sub-spawn.test.ts:237 (S10),
//! replaces.test.ts:73, :96, :136 (S13, S20), box-lead.test.ts:183, :232 (S14), spawn-guard.test.ts:753, :767, :1233,
//! :1309, :1351 and runtime-notify.test.ts:283, :296, :474, :580 (S20, S21, invariant 3), sub-queue.test.ts:175 (S24),
//! runtime-index.test.ts:255, thread-keeps-agent.test.ts:89, thread-defaults.test.ts:185, session-title.test.ts:684,
//! box-nap.test.ts:867. Each case is ported at this unit's seam: `prepare` and `start_send` over a scripted host.
#[path = "support_a/mod.rs"]
mod support;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use support::*;
use wsp_account::account::rules::TreeOf;
use wsp_core::console;
use wsp_machines::machines::boot::entries::RowAt;
use wsp_machines::machines::sub_spawn::machine::Resumed;
use wsp_threads::threads::forks::{ForkPicks, ForkPlanSource, ForkPlanTurn};
use wsp_threads::threads::sessions::start::{Checked, Prepared};
use wsp_core::types::wiring::{AccessMove, SteerTaken};
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
    replaces_working_fix, replaces_working_line, send_files_across_line, thread_word, wait_across_line, RESTART_OPENS_FIX, RESTART_OPENS_LINE,
    SEND_FILES_ACROSS_FIX, WAIT_ACROSS_FIX,
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
    serde_json::from_value(json!({ "kind": "thread", "threadId": thread, "workspaceId": workspace, "rootThreadId": root })).expect("a thread scope")
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
    OutsideOpening { harness: harness.into(), id: id.into(), copy, project: "p1".into(), cwd: None, title: "lab codewords".into(), earlier: Vec::new() }
}

fn taken(thread: &str) -> Taken {
    serde_json::from_value(json!({ "sessionId": "sess-1", "threadId": thread, "turnId": "turn-1", "outcome": "started" })).expect("a taken start")
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
    hook!(host, "LaunchCtx::preferences", dyn Fn() -> LocalBoxFuture<'static, Result<Preferences, HostError>>, || async { Ok(default_preferences()) }.boxed_local());
    hook!(
        host,
        "SessionsCtx::fork_opened",
        dyn Fn(&str, StartOptions, Option<&Caller>) -> LocalBoxFuture<'static, Result<(StartOptions, Option<ForkPlan>), HostError>>,
        |_w: &str, o: StartOptions, _c: Option<&Caller>| async move { Ok((o, None)) }.boxed_local()
    );
    let asked = Rc::new(RefCell::new(Vec::new()));
    let seen = asked.clone();
    let agent: Arc<dyn HarnessAdapter> = Arc::new(agent);
    hook!(host, "LaunchCtx::adapter_for", dyn Fn(&Entry, AdapterAsk) -> Result<BuiltAdapter, HostError>, move |_e: &Entry, ask: AdapterAsk| {
        let h = ask.named.clone().expect("the harness is named");
        seen.borrow_mut().push((h.clone(), ask));
        Ok(BuiltAdapter { harness: h, adapter: agent.clone() })
    });
    let area = SessionsArea::new(host.clone());
    Fx { host, area, asked }
}

impl Fx {
    async fn prepare_with(&self, claimed: &RefCell<Option<String>>, opts: StartOptions, origin: Option<Caller>) -> Result<Checked, HostError> {
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
        view(&id, workspace, json!({ "threadId": thread, "harness": harness, "status": status, "startedAt": T0, "claudeSessionId": format!("cs-{thread}") })),
        &format!("turn-{thread}"),
    );
    host.add_row(&id, r);
}

/// The calls of these methods, in the order they were made.
fn order(host: &FakeHost, names: &[&str]) -> Vec<String> {
    host.hooks.calls().into_iter().filter(|c| names.iter().any(|n| c == n || c.starts_with(&format!("{n}(")))).collect()
}

fn nothing_held(host: &FakeHost) {
    assert!(host.rows.borrow().keys().all(|k| k.starts_with("sess-")), "a refusal holds no row of its own (only the fixtures' threads have rows)");
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
// S1, S2, S3

// S1: sessions.ts:259
#[test]
fn preferences_are_the_held_copy_else_the_stores() {
    run_local(async {
        let fx = fx();
        let p = fx.launches(go("hi"), None).await;
        assert_eq!(p.prefs, default_preferences());
        assert_eq!(order(&fx.host, &["ready", "preferences_held", "preferences"]), strings(&["ready", "preferences_held", "preferences"]));

        let fx = fx_on(Agent::default());
        let mut held = default_preferences();
        held.labs = true;
        let held_copy = held.clone();
        hook!(fx.host, "LaunchCtx::preferences_held", dyn Fn() -> Option<Preferences>, move || Some(held_copy.clone()));
        let p = fx.launches(go("hi"), None).await;
        assert_eq!(p.prefs, held);
        assert_eq!(order(&fx.host, &["ready", "preferences_held", "preferences"]), strings(&["ready", "preferences_held"]));
    })
}

// S2: verbs-send-notify.test.ts:91
#[test]
fn a_start_sent_again_answers_the_turn_it_opened_and_starts_nothing() {
    run_local(async {
        let fx = fx();
        fx.thread_on("t1", "w1", "claude");
        let held = taken("t1");
        hook!(fx.host, "SessionsCtx::taken", dyn Fn(&str, &str) -> Option<Taken>, move |_w: &str, r: &str| if r == "req-1" { Some(held.clone()) } else { None });
        let opened = Handle::new("sess-1", "turn-1");
        hook!(
            fx.host,
            "SessionsCtx::taken_turn",
            dyn Fn(&str, Taken) -> LocalBoxFuture<'static, Result<Option<Rc<dyn SessionHandle>>, HostError>>,
            move |_w: &str, _t: Taken| {
                let a = opened.clone();
                async move { Ok(Some(a)) }.boxed_local()
            }
        );
        let opts = StartOptions { thread: Some("t1".into()), request_id: Some("req-1".into()), ..go("second") };
        match fx.prepare(opts, None).await {
            Ok(Checked::Answered(h)) => assert_eq!((h.id(), h.turn_id(), h.outcome()), ("sess-1".to_string(), "turn-1".to_string(), SessionStartOutcome::Started)),
            Ok(Checked::Launch(_)) => panic!("a start already taken is not launched again"),
            Err(e) => panic!("refused: {}", e.message),
        }
        assert_eq!(
            fx.host.hooks.calls(),
            strings(&["ready", "preferences_held", "preferences", "taken(w1, req-1)", "entry_of_row(Some(\"t1\"), w1, false)", "taken_turn(w1)"])
        );
        before_the_mint(&fx.host);
    })
}

// S2: verbs-send-notify.test.ts:91
#[test]
fn a_start_with_no_request_id_looks_nothing_up() {
    run_local(async {
        let fx = fx();
        fx.launches(go("hi"), None).await;
        assert_eq!(fx.host.hooks.called("taken"), Vec::<String>::new());
        assert_eq!(fx.host.hooks.called("taken_turn"), Vec::<String>::new());
    })
}

// S2, S12
#[test]
fn a_taken_start_the_caller_may_not_send_to_is_not_answered() {
    run_local(async {
        let fx = fx();
        fx.thread_on("t1", "w1", "claude");
        let held = taken("t1");
        hook!(fx.host, "SessionsCtx::taken", dyn Fn(&str, &str) -> Option<Taken>, move |_w: &str, _r: &str| Some(held.clone()));
        hook!(
            fx.host,
            "SessionsCtx::entry_of_row",
            dyn Fn(&RowAt, Option<&Caller>, Option<TreeTalk>, bool) -> LocalBoxFuture<'static, Result<Option<Entry>, HostError>>,
            |_a: &RowAt, _o: Option<&Caller>, _k: Option<TreeTalk>, _b: bool| async { Ok(None) }.boxed_local()
        );
        let opts = StartOptions { thread: Some("t1".into()), request_id: Some("req-1".into()), ..go("second") };
        let e = fx.refused(opts, None).await;
        assert_eq!(said(&e), said(&err("no thread t1 in this folder")));
        assert_eq!(fx.host.hooks.called("taken_turn"), Vec::<String>::new());
        assert_eq!(fx.host.hooks.called("entry_of_row").len(), 2, "once for the taken start, once for the send");
        before_the_mint(&fx.host);
    })
}

// S2
#[test]
fn a_taken_start_the_turn_cannot_answer_goes_on_as_a_new_start() {
    run_local(async {
        let fx = fx();
        fx.thread_on("t1", "w1", "claude");
        let held = taken("t1");
        hook!(fx.host, "SessionsCtx::taken", dyn Fn(&str, &str) -> Option<Taken>, move |_w: &str, _r: &str| Some(held.clone()));
        let opts = StartOptions { thread: Some("t1".into()), request_id: Some("req-1".into()), ..go("second") };
        let p = fx.launches(opts, None).await;
        assert_eq!((p.thread_id.as_str(), p.opens), ("t1", false));
        assert_eq!(order(&fx.host, &["taken_turn", "fork_opened"]), strings(&["taken_turn(w1)", "fork_opened(w1)"]));
    })
}

// S3
#[test]
fn what_fork_opened_hands_back_is_what_the_checks_read_and_carry() {
    run_local(async {
        let fx = fx();
        let plan = plan_with(false, Some("src-sess"));
        let handed = plan.clone();
        hook!(
            fx.host,
            "SessionsCtx::fork_opened",
            dyn Fn(&str, StartOptions, Option<&Caller>) -> LocalBoxFuture<'static, Result<(StartOptions, Option<ForkPlan>), HostError>>,
            move |_w: &str, o: StartOptions, _c: Option<&Caller>| {
                let plan = handed.clone();
                async move { Ok((StartOptions { prompt: format!("forked: {}", o.prompt), harness: Some("claude".into()), ..o }, Some(plan))) }.boxed_local()
            }
        );
        let p = fx.launches(go("on it"), None).await;
        assert_eq!(p.opts.prompt, "forked: on it");
        same_plan(&p.plan, &Some(plan));
        assert_eq!(order(&fx.host, &["fork_opened", "fork_turn_of"]), strings(&["fork_opened(w1)", "fork_turn_of(false)"]));
    })
}

// ---------------------------------------------------------------------------------------------------------------
// S4, S5, S6, S12

// S4, S12: runtime-index.test.ts:255, spawn-guard.test.ts:1289
#[test]
fn a_thread_of_another_workspace_is_no_thread_in_this_folder() {
    run_local(async {
        let line = said(&err("no thread t1 in this folder"));
        let named = StartOptions { thread: Some("t1".into()), ..go("x") };

        let fx = fx();
        assert_eq!(said(&fx.refused(named.clone(), None).await), line, "a thread nothing holds");
        assert_eq!(fx.host.hooks.called("entry_of"), Vec::<String>::new());
        before_the_mint(&fx.host);

        let fx = fx_on(Agent::default());
        thread_in(&fx.host, "t1", "w2", "claude", "completed");
        assert_eq!(said(&fx.refused(named.clone(), None).await), line, "a thread whose rows are on another workspace");
        before_the_mint(&fx.host);

        let fx = fx_on(Agent::default());
        fx.host.add_record("t1", "w2", thread_record(json!({ "harness": "claude" })));
        assert_eq!(said(&fx.refused(named.clone(), None).await), line, "a thread whose record is on another workspace");
        before_the_mint(&fx.host);

        let fx = fx_on(Agent::default());
        fx.thread_on("t1", "w1", "claude");
        hook!(
            fx.host,
            "SessionsCtx::entry_of_row",
            dyn Fn(&RowAt, Option<&Caller>, Option<TreeTalk>, bool) -> LocalBoxFuture<'static, Result<Option<Entry>, HostError>>,
            |_a: &RowAt, _o: Option<&Caller>, _k: Option<TreeTalk>, _b: bool| async { Ok(None) }.boxed_local()
        );
        let child = by(WorkspaceOrigin::Relayed, scope("kid", "w1", "lead-t"));
        assert_eq!(said(&fx.refused(named, Some(child)).await), line, "a thread the caller's tree does not reach");
        before_the_mint(&fx.host);
    })
}

// S12
#[test]
fn an_opening_reads_its_entry_with_the_callers_origin_unless_it_may_open_there() {
    run_local(async {
        let child = by(WorkspaceOrigin::Relayed, scope("kid", "w1", "lead-t"));
        for (opens_there, origin, read_with) in [(true, Some(child.clone()), None), (false, Some(child.clone()), Some(child.clone())), (true, None, None)] {
            let fx = fx();
            let live = fx.host.live("w1").expect("the workspace");
            let seen: Rc<RefCell<Vec<(String, Option<Caller>)>>> = Rc::new(RefCell::new(Vec::new()));
            let log = seen.clone();
            hook!(fx.host, "LaunchCtx::entry_of", dyn Fn(&str, Option<Caller>) -> LocalBoxFuture<'static, Result<Entry, HostError>>, move |w: &str, o: Option<Caller>| {
                log.borrow_mut().push((w.to_string(), o));
                let e = live.clone();
                async move { Ok(e) }.boxed_local()
            });
            hook!(fx.host, "SessionsCtx::opens_in", dyn Fn(&WorkspaceRecord, Option<&Caller>) -> bool, move |_r: &WorkspaceRecord, _o: Option<&Caller>| opens_there);
            fx.launches(go("hi"), origin).await;
            assert_eq!(*seen.borrow(), vec![("w1".to_string(), read_with)]);
        }
    })
}

// S12
#[test]
fn a_send_reads_its_entry_by_the_thread_and_the_workspace() {
    run_local(async {
        let fx = fx();
        fx.thread_on("t1", "w1", "claude");
        let seen: Rc<RefCell<Vec<(Option<String>, String, Option<Caller>, Option<TreeTalk>, bool)>>> = Rc::new(RefCell::new(Vec::new()));
        let log = seen.clone();
        let live = fx.host.live("w1").expect("the workspace");
        hook!(
            fx.host,
            "SessionsCtx::entry_of_row",
            dyn Fn(&RowAt, Option<&Caller>, Option<TreeTalk>, bool) -> LocalBoxFuture<'static, Result<Option<Entry>, HostError>>,
            move |at: &RowAt, o: Option<&Caller>, k: Option<TreeTalk>, b: bool| {
                log.borrow_mut().push((at.thread_id.clone(), at.workspace_id.clone(), o.cloned(), k, b));
                let e = live.clone();
                async move { Ok(Some(e)) }.boxed_local()
            }
        );
        let child = by(WorkspaceOrigin::Relayed, scope("kid", "w1", "lead-t"));
        fx.launches(StartOptions { thread: Some("t1".into()), ..go("and more") }, Some(child.clone())).await;
        assert_eq!(*seen.borrow(), vec![(Some("t1".to_string()), "w1".to_string(), Some(child), Some(TreeTalk::Send), false)]);
        assert_eq!(fx.host.hooks.called("entry_of"), Vec::<String>::new(), "a send reads no opening's entry");
    })
}

// S5, S16: conversations.test.ts:683
#[test]
fn an_outside_conversation_opens_a_thread_only() {
    run_local(async {
        let want = said(&usage(RESUME_A_THREAD_LINE, RESUME_A_THREAD_FIX));

        let fx = fx();
        fx.thread_on("t1", "w1", "claude");
        let e = fx.refused(StartOptions { thread: Some("t1".into()), outside: Some(outside("claude", "other", false)), ..go("x") }, None).await;
        assert_eq!(said(&e), want, "named beside a thread");
        before_the_mint(&fx.host);
        assert_eq!(fx.host.hooks.called("agent_off"), Vec::<String>::new());

        let fx = fx_on(Agent::default());
        fx.thread_on("t1", "w1", "claude");
        let sent_into = StartOptions { r#as: Some(StartAs { thread_id: "t1".into(), turn_id: "turn-x".into() }), outside: Some(outside("claude", "other", false)), ..go("x") };
        let e = fx.refused(sent_into, None).await;
        assert_eq!(said(&e), want, "sent into a thread that has run");
        before_the_mint(&fx.host);
        assert_eq!(fx.host.hooks.called("agent_off"), Vec::<String>::new());
    })
}

// S6
#[test]
fn resume_is_the_latest_rows_session_else_the_transcripts_else_an_uncopied_outsides() {
    run_local(async {
        let named = StartOptions { thread: Some("t1".into()), ..go("more") };

        let fx = fx();
        fx.thread_on("t1", "w1", "claude");
        assert_eq!(fx.launches(named.clone(), None).await.resume, Some("cs-t1".to_string()), "the row's session");

        let fx = fx_on(Agent::default());
        fx.host.add_record("t1", "w1", thread_record(json!({ "harness": "claude" })));
        hook!(fx.host, "TrimmedCtx::started_as", dyn Fn(&str, &str) -> Option<String>, |_w: &str, t: &str| if t == "t1" { Some("sess-from-transcript".to_string()) } else { None });
        assert_eq!(fx.launches(named, None).await.resume, Some("sess-from-transcript".to_string()), "the transcript's session");

        let fx = fx_on(Agent::default());
        let p = fx.launches(StartOptions { outside: Some(outside("claude", "outside-1", false)), ..go("go on") }, None).await;
        assert_eq!(p.resume, Some("outside-1".to_string()), "an outside conversation the start does not copy");

        let fx = fx_on(Agent::default());
        let p = fx.launches(StartOptions { outside: Some(outside("claude", "outside-1", true)), ..go("go on") }, None).await;
        assert_eq!(p.resume, None, "a copy resumes nothing");

        let fx = fx_on(Agent::default());
        assert_eq!(fx.launches(go("fresh"), None).await.resume, None);
    })
}

// ---------------------------------------------------------------------------------------------------------------
// S7, S8, S26

// S7, S8: thread-warm-agent.test.ts:82
#[test]
fn an_opening_send_claims_the_warm_agent() {
    run_local(async {
        let fx = fx();
        hook!(fx.host, "SessionsCtx::claim_warm", dyn Fn(&str, &str) -> Option<String>, |_w: &str, _h: &str| Some("thr-warm".to_string()));
        let claimed = RefCell::new(None);
        let p = match fx.prepare_with(&claimed, StartOptions { harness: Some("claude".into()), ..go("one") }, None).await {
            Ok(Checked::Launch(p)) => p,
            _ => panic!("the start was not prepared"),
        };
        assert_eq!((p.thread_id.as_str(), p.opens), ("thr-warm", true));
        assert_eq!(*claimed.borrow(), Some("thr-warm".to_string()));
        assert_eq!(fx.host.hooks.called("claim_warm"), strings(&["claim_warm(w1, claude)"]));
        assert_eq!(fx.host.ids.uuids.borrow().clone(), strings(&["thr-new"]), "no fresh id is drawn for it");
    })
}

// S7: thread-warm-agent.test.ts:197, :213, :241
#[test]
fn a_start_that_changes_the_launch_takes_no_warm_agent() {
    run_local(async {
        let child = by(WorkspaceOrigin::Relayed, scope("kid", "w1", "lead-t"));
        let plan = plan_with(false, Some("src-sess"));
        let mut servers = IndexMap::new();
        servers.insert("mine".to_string(), McpServerSpec { command: "x".into(), args: Vec::new(), no_slate: None });
        let base = || StartOptions { harness: Some("claude".into()), ..go("one") };
        let variants: Vec<(&str, StartOptions, Option<Caller>, Option<ForkPlan>)> = vec![
            ("no agent named", go("one"), None, None),
            ("a thread's own start", base(), Some(child), None),
            ("a fork", base(), None, Some(plan)),
            ("a copy of an outside conversation", StartOptions { outside: Some(outside("claude", "o-1", true)), ..base() }, None, None),
            ("a title", StartOptions { title: Some("Named".into()), ..base() }, None, None),
            ("servers", StartOptions { mcp_servers: Some(servers), ..base() }, None, None),
            ("asks until stopped", StartOptions { asks_until_stopped: true, ..base() }, None, None),
        ];
        for (why, opts, origin, plan) in variants {
            let fx = fx_on(Agent { takes_servers: true, ..Default::default() });
            hook!(fx.host, "SessionsCtx::claim_warm", dyn Fn(&str, &str) -> Option<String>, |_w: &str, _h: &str| Some("thr-warm".to_string()));
            hook!(
                fx.host,
                "SessionsCtx::fork_opened",
                dyn Fn(&str, StartOptions, Option<&Caller>) -> LocalBoxFuture<'static, Result<(StartOptions, Option<ForkPlan>), HostError>>,
                move |_w: &str, o: StartOptions, _c: Option<&Caller>| {
                    let plan = plan.clone();
                    async move { Ok((o, plan)) }.boxed_local()
                }
            );
            let claimed = RefCell::new(None);
            let p = match fx.prepare_with(&claimed, opts, origin).await {
                Ok(Checked::Launch(p)) => p,
                _ => panic!("{why}: the start was not prepared"),
            };
            assert_eq!(fx.host.hooks.called("claim_warm"), Vec::<String>::new(), "{why}");
            assert_eq!(*claimed.borrow(), None, "{why}");
            assert_eq!(p.thread_id, "thr-new", "{why}");
        }
    })
}

// S7, S26: thread-warm-agent.test.ts:186
#[test]
fn a_refused_start_reaps_the_agent_it_claimed() {
    run_local(async {
        let fx = fx();
        hook!(fx.host, "SessionsCtx::claim_warm", dyn Fn(&str, &str) -> Option<String>, |_w: &str, _h: &str| Some("thr-warm".to_string()));
        let opts = StartOptions { harness: Some("claude".into()), notify: Some(strings(&["no-such-thread"])), ..go("one") };
        let e = fx.area.start_send("w1", opts, None).await.err().expect("the start is refused");
        assert_eq!(said(&e), said(&err("no thread no-such-thread to notify")));
        assert_eq!(order(&fx.host, &["claim_warm", "reap_kept", "sub_done"]), strings(&["claim_warm(w1, claude)", "reap_kept(thr-warm)"]));
        before_the_mint(&fx.host);
    })
}

// S26
#[test]
fn a_refused_start_that_claimed_nothing_reaps_nothing() {
    run_local(async {
        let fx = fx();
        let opts = StartOptions { notify: Some(strings(&["no-such-thread"])), ..go("one") };
        let e = fx.area.start_send("w1", opts, None).await.err().expect("the start is refused");
        assert_eq!(said(&e), said(&err("no thread no-such-thread to notify")));
        assert_eq!(order(&fx.host, &["reap_kept", "sub_done"]), Vec::<String>::new());
    })
}

// S26: sessions.ts:1494
#[test]
fn a_refused_start_on_a_sub_machine_arms_its_done() {
    run_local(async {
        let fx = fx();
        fx.host.add_entry(entry("w1", sub_json(true)));
        fx.host.add_record("sub-t", "w1", thread_record(json!({ "harness": "claude" })));
        let e = fx.area.start_send("w1", go("beside it"), None).await.err().expect("the start is refused");
        assert_eq!(said(&e), said(&sub_machine_taken_refusal("web", "sub-t")));
        assert_eq!(order(&fx.host, &["sub_done"]), strings(&["sub_done(w1)"]));

        let fx = fx_on(Agent::default());
        let e = fx.area.start_send("w1", StartOptions { notify: Some(strings(&["nobody"])), ..go("x") }, None).await.err().expect("refused");
        assert_eq!(said(&e), said(&err("no thread nobody to notify")));
        assert_eq!(order(&fx.host, &["sub_done"]), Vec::<String>::new(), "a workspace that is no sub-machine arms nothing");
    })
}

// S8
#[test]
fn the_thread_id_is_the_named_one_else_as_else_a_fresh_uuid() {
    run_local(async {
        let fx = fx();
        let p = fx.launches(StartOptions { r#as: Some(StartAs { thread_id: "as-t".into(), turn_id: "as-turn".into() }), ..go("hi") }, None).await;
        assert_eq!((p.thread_id.as_str(), p.opens), ("as-t", true));
        assert_eq!(fx.host.ids.uuids.borrow().clone(), strings(&["thr-new"]));

        let fx = fx_on(Agent::default());
        fx.thread_on("t1", "w1", "claude");
        let p = fx.launches(StartOptions { thread: Some("t1".into()), ..go("hi") }, None).await;
        assert_eq!((p.thread_id.as_str(), p.opens), ("t1", false));

        let fx = fx_on(Agent::default());
        let p = fx.launches(go("hi"), None).await;
        assert_eq!((p.thread_id.as_str(), p.opens), ("thr-new", true));
        assert!(fx.host.ids.uuids.borrow().is_empty());
    })
}

// S9
#[test]
fn a_thread_a_record_or_a_row_names_is_a_send_not_an_opening() {
    run_local(async {
        let as_t1 = || StartOptions { r#as: Some(StartAs { thread_id: "t1".into(), turn_id: "as-turn".into() }), ..go("hi") };

        let fx = fx();
        fx.host.add_record("t1", "w1", thread_record(json!({ "harness": "claude" })));
        let p = fx.launches(as_t1(), None).await;
        assert_eq!((p.opens, p.cleared), (false, true), "a record and no rows");
        assert_eq!(fx.host.hooks.called("spawn_guard"), strings(&["spawn_guard(send)"]));

        let fx = fx_on(Agent::default());
        fx.host.add_row("sess-t1", row(view("sess-t1", "w1", json!({ "threadId": "t1", "status": "completed", "startedAt": T0 })), "turn-t1"));
        let p = fx.launches(as_t1(), None).await;
        assert_eq!(p.opens, false, "a row and no record");
        assert_eq!(fx.host.hooks.called("spawn_guard"), strings(&["spawn_guard(send)"]));

        let fx = fx_on(Agent::default());
        let p = fx.launches(as_t1(), None).await;
        assert_eq!(p.opens, true);
        assert_eq!(fx.host.hooks.called("spawn_guard"), strings(&["spawn_guard(thread_new)"]));
    })
}

// ---------------------------------------------------------------------------------------------------------------
// S10, S11

// S10: sub-spawn.test.ts:237
#[test]
fn a_sub_machine_runs_one_thread() {
    run_local(async {
        let fx = fx();
        fx.host.add_entry(entry("w1", sub_json(true)));
        fx.host.add_record("sub-t", "w1", thread_record(json!({ "harness": "claude" })));
        let e = fx.refused(go("beside it"), None).await;
        assert_eq!(said(&e), said(&sub_machine_taken_refusal("web", "sub-t")));
        assert_eq!(said(&e).2, Some(Kind::Usage));
        before_the_mint(&fx.host);
        assert_eq!(fx.host.hooks.called("entry_of"), Vec::<String>::new());

        let fx = fx_on(Agent::default());
        fx.host.add_entry(entry("w1", sub_json(true)));
        fx.host.add_record("sub-t", "w1", thread_record(json!({ "harness": "claude" })));
        fx.host.add_row("sess-sub", row(view("sess-sub", "w1", json!({ "threadId": "sub-t", "status": "completed", "startedAt": T0 })), "turn-sub"));
        let live = fx.host.live("w1").expect("the sub-machine");
        hook!(
            fx.host,
            "SessionsCtx::resume_sub",
            dyn Fn(&Entry, &str, &str, &str) -> LocalBoxFuture<'static, Result<Resumed, HostError>>,
            move |_e: &Entry, _t: &str, _h: &str, _p: &str| {
                let entry = live.clone();
                async move { Ok(Resumed { entry, preamble: None, notices: Vec::new() }) }.boxed_local()
            }
        );
        let p = fx.launches(StartOptions { thread: Some("sub-t".into()), ..go("pushed it") }, None).await;
        assert_eq!((p.thread_id.as_str(), p.opens), ("sub-t", false), "a send into its thread is not a second thread");
    })
}

// S11
#[test]
fn an_opening_the_computer_rule_keeps_out_is_refused_in_its_own_words() {
    run_local(async {
        let child = by(WorkspaceOrigin::Relayed, scope("kid", "w9", "lead-t"));
        let want = usage("a folder refusal", "its fix");
        let machine = usage("a machine refusal", "its fix");

        let fx = fx();
        let w = want.clone();
        hook!(fx.host, "SessionsCtx::opens_in", dyn Fn(&WorkspaceRecord, Option<&Caller>) -> bool, |_r: &WorkspaceRecord, _o: Option<&Caller>| false);
        hook!(fx.host, "SessionsCtx::of_threads_repository", dyn Fn(Option<&Caller>, &str) -> bool, |_o: Option<&Caller>, _p: &str| true);
        hook!(fx.host, "SessionsCtx::elsewhere_refusal", dyn Fn(Option<&Caller>, &str, &str) -> Option<HostError>, move |_o: Option<&Caller>, _p: &str, _n: &str| Some(w.clone()));
        let e = fx.refused(go("hi"), Some(child.clone())).await;
        assert_eq!(said(&e), said(&want), "a folder");
        assert_eq!(fx.host.hooks.called("elsewhere_refusal"), strings(&["elsewhere_refusal(p1, lab)"]));
        assert_eq!(fx.host.hooks.called("away_for"), Vec::<String>::new());
        before_the_mint(&fx.host);

        let fx = fx_on(Agent::default());
        fx.host.add_entry(entry("w1", json!({ "name": "lab", "kind": "cloud" })));
        let m = machine.clone();
        hook!(fx.host, "SessionsCtx::opens_in", dyn Fn(&WorkspaceRecord, Option<&Caller>) -> bool, |_r: &WorkspaceRecord, _o: Option<&Caller>| false);
        hook!(fx.host, "SessionsCtx::of_threads_repository", dyn Fn(Option<&Caller>, &str) -> bool, |_o: Option<&Caller>, _p: &str| true);
        hook!(fx.host, "SessionsCtx::away_for", dyn Fn(&WorkspaceRecord, Option<&Caller>, &str, &str) -> Option<HostError>, move |_r: &WorkspaceRecord, _o: Option<&Caller>, _a: &str, _w: &str| Some(m.clone()));
        let e = fx.refused(go("hi"), Some(child.clone())).await;
        assert_eq!(said(&e), said(&machine), "a machine");
        assert_eq!(fx.host.hooks.called("away_for"), strings(&["away_for(start, w1)"]));
        assert_eq!(fx.host.hooks.called("elsewhere_refusal"), Vec::<String>::new());
        before_the_mint(&fx.host);

        let fx = fx_on(Agent::default());
        hook!(fx.host, "SessionsCtx::opens_in", dyn Fn(&WorkspaceRecord, Option<&Caller>) -> bool, |_r: &WorkspaceRecord, _o: Option<&Caller>| false);
        let w = want.clone();
        hook!(fx.host, "SessionsCtx::elsewhere_refusal", dyn Fn(Option<&Caller>, &str, &str) -> Option<HostError>, move |_o: Option<&Caller>, _p: &str, _n: &str| Some(w.clone()));
        fx.launches(go("hi"), Some(child.clone())).await;
        assert_eq!(fx.host.hooks.called("elsewhere_refusal"), Vec::<String>::new(), "a project of another repository reads as absent here");

        let fx = fx_on(Agent::default());
        hook!(fx.host, "SessionsCtx::of_threads_repository", dyn Fn(Option<&Caller>, &str) -> bool, |_o: Option<&Caller>, _p: &str| true);
        let w = want;
        hook!(fx.host, "SessionsCtx::elsewhere_refusal", dyn Fn(Option<&Caller>, &str, &str) -> Option<HostError>, move |_o: Option<&Caller>, _p: &str, _n: &str| Some(w.clone()));
        fx.launches(go("hi"), Some(child)).await;
        assert_eq!(fx.host.hooks.called("elsewhere_refusal"), Vec::<String>::new(), "a workspace the caller may open in is not asked");
    })
}

// ---------------------------------------------------------------------------------------------------------------
// S13

// S13: replaces.test.ts:96
#[test]
fn a_restart_only_opens() {
    run_local(async {
        let fx = fx();
        fx.thread_on("t1", "w1", "claude");
        let e = fx.refused(StartOptions { thread: Some("t1".into()), replaces: Some("a1".into()), ..go("and the parser") }, None).await;
        assert_eq!(said(&e), said(&usage(RESTART_OPENS_LINE, RESTART_OPENS_FIX)));
        assert_eq!(fx.host.hooks.called("thread_facts"), Vec::<String>::new(), "the rule is read before the replaced thread is");
        before_the_mint(&fx.host);
    })
}

// S13: replaces.test.ts:73
#[test]
fn a_restart_of_a_busy_thread_is_refused() {
    run_local(async {
        let fx = fx();
        hook!(fx.host, "SessionsCtx::thread_facts", dyn Fn(&str) -> Option<ThreadFacts>, |t: &str| if t == "a1" { Some(facts("a1", "running")) } else { None });
        let e = fx.refused(StartOptions { replaces: Some("a1".into()), ..go("build the parser") }, None).await;
        assert_eq!(said(&e), said(&usage(&replaces_working_line("a1"), &replaces_working_fix("a1"))));
        assert_eq!(order(&fx.host, &["entry_of", "thread_facts", "agent_off"]).first().map(String::as_str), Some("entry_of(w1)"));
        assert_eq!(fx.host.hooks.called("agent_off"), Vec::<String>::new());
        before_the_mint(&fx.host);
    })
}

// ---------------------------------------------------------------------------------------------------------------
// S14, S15

// S14: box-lead.test.ts:183, :232. The caller acts on its own box and not on w1; each project names its computer.
#[test]
fn a_send_across_computers_carries_no_files_and_no_follow() {
    run_local(async {
        let road = Some(AttachmentRoad::Inline);
        let child = by(WorkspaceOrigin::Relayed, scope("kid", "wbox", "lead-t"));
        let across = |fx: &Fx| {
            fx.host.add_entry(entry("wbox", json!({ "name": "box", "kind": "place", "project": "pbox" })));
            fx.thread_on("t1", "w1", "claude");
            hook!(fx.host, "SessionsCtx::acts_on", dyn Fn(&WorkspaceRecord, Option<&Caller>) -> bool, |r: &WorkspaceRecord, _o: Option<&Caller>| r.id != "w1");
            hook!(fx.host, "SessionsCtx::project_computer_name", dyn Fn(&str) -> String, |p: &str| if p == "pbox" { "hetzner".to_string() } else { "default".to_string() });
        };
        let file = Attachment { media_type: "text/plain".into(), bytes: "aGk=".into(), name: Some("notes.sh".into()) };
        let sent = |o: StartOptions| StartOptions { thread: Some("t1".into()), ..o };

        let fx = fx_on(Agent { road, ..Default::default() });
        across(&fx);
        let e = fx.refused(sent(StartOptions { attachments: Some(vec![file.clone()]), ..go("see the file") }), Some(child.clone())).await;
        assert_eq!(said(&e), said(&usage(&send_files_across_line("hetzner", "default", "t1"), SEND_FILES_ACROSS_FIX)));
        before_the_mint(&fx.host);

        let e = fx.refused(sent(StartOptions { followed: Some(true), ..go("which branch?") }), Some(child.clone())).await;
        assert_eq!(said(&e), said(&usage(&wait_across_line("hetzner", "default", "t1"), WAIT_ACROSS_FIX)));
        before_the_mint(&fx.host);

        let p = fx.launches(sent(go("the box is done")), Some(child.clone())).await;
        assert_eq!((p.thread_id.as_str(), p.opens), ("t1", false), "its words alone go through");

        let fx = fx_on(Agent { road, ..Default::default() });
        across(&fx);
        let p = fx.launches(sent(StartOptions { attachments: Some(vec![file.clone()]), followed: Some(true), ..go("person") }), None).await;
        assert_eq!(p.thread_id, "t1", "the person's own send carries both");

        let fx = fx_on(Agent { road, ..Default::default() });
        fx.host.add_entry(entry("wbox", json!({ "name": "box", "kind": "place", "project": "pbox" })));
        fx.thread_on("t1", "w1", "claude");
        let p = fx.launches(sent(StartOptions { attachments: Some(vec![file]), followed: Some(true), ..go("full reach") }), Some(child)).await;
        assert_eq!(p.thread_id, "t1", "a caller that acts on the workspace carries both");
    })
}

// S15
#[test]
fn a_worktree_removed_by_hand_reads_gone() {
    run_local(async {
        let tree = |extra: Value| {
            let mut w = json!({ "path": "/wt/a", "made": true });
            for (k, v) in extra.as_object().expect("object") {
                w[k] = v.clone();
            }
            json!({ "name": "lab", "worktree": w })
        };

        let fx = fx();
        fx.host.add_entry(entry("w1", tree(json!({}))));
        hook!(fx.host, "LaunchCtx::folder_exists", dyn Fn(&str) -> bool, |_p: &str| false);
        let p = fx.launches(go("hi"), None).await;
        assert_eq!(p.gone_now, Some("/wt/a".to_string()));
        assert_eq!(fx.host.hooks.called("worktree_gone"), strings(&["worktree_gone(removed, true)"]));
        assert_eq!(order(&fx.host, &["worktree_gone", "agent_off"]), strings(&["worktree_gone(removed, true)", "agent_off(claude)"]));

        let fx = fx_on(Agent::default());
        fx.host.add_entry(entry("w1", tree(json!({}))));
        let p = fx.launches(go("hi"), None).await;
        assert_eq!(p.gone_now, None, "a folder that is there");
        assert_eq!(fx.host.hooks.called("worktree_gone"), Vec::<String>::new());

        let fx = fx_on(Agent::default());
        fx.host.add_entry(entry("w1", tree(json!({ "gone": true }))));
        hook!(fx.host, "LaunchCtx::folder_exists", dyn Fn(&str) -> bool, |_p: &str| false);
        let p = fx.launches(go("hi"), None).await;
        assert_eq!(p.gone_now, None, "a worktree already read as gone");
        assert_eq!(fx.host.hooks.called("worktree_gone"), Vec::<String>::new());
    })
}

// ---------------------------------------------------------------------------------------------------------------
// S16, S17

// S16: thread-keeps-agent.test.ts:89, :119
#[test]
fn a_thread_keeps_its_agent() {
    run_local(async {
        let fx = fx();
        fx.thread_on("t1", "w1", "codex");
        let e = fx.refused(StartOptions { thread: Some("t1".into()), harness: Some("claude".into()), ..go("two") }, None).await;
        assert_eq!(said(&e), said(&err(&thread_runs_on_line("codex", "claude"))));
        before_the_mint(&fx.host);
        assert_eq!(fx.host.hooks.called("agent_off"), Vec::<String>::new());

        let fx = fx_on(Agent::default());
        fx.thread_on("t1", "w1", "codex");
        hook!(fx.host, "LaunchCtx::default_agent_of", dyn Fn(&Preferences, &Entry) -> String, |_p: &Preferences, _e: &Entry| "claude".to_string());
        let p = fx.launches(StartOptions { thread: Some("t1".into()), ..go("two") }, None).await;
        assert_eq!(p.opts.harness, Some("codex".to_string()), "a send naming none runs on the thread's agent");
        assert_eq!(fx.host.hooks.called("default_agent_of"), Vec::<String>::new());
        let p = fx.launches(StartOptions { thread: Some("t1".into()), harness: Some("codex".into()), ..go("two") }, None).await;
        assert_eq!(p.opts.harness, Some("codex".to_string()), "naming its own agent is no refusal");
    })
}

// S16: permission-relay.test.ts:251
#[test]
fn a_send_drops_its_access() {
    run_local(async {
        let picks = || StartOptions { permission_mode: Some("plan".into()), access: Some(AccessChoice::Full), ..go("two") };

        let fx = fx();
        fx.thread_on("t1", "w1", "claude");
        let p = fx.launches(StartOptions { thread: Some("t1".into()), ..picks() }, None).await;
        assert_eq!(p.opts.permission_mode, None);
        assert!(p.opts.access.is_none());

        let fx = fx_on(Agent::default());
        let p = fx.launches(picks(), None).await;
        assert_eq!(p.opts.permission_mode, Some("plan".to_string()), "an opening keeps what it named");
        assert!(matches!(p.opts.access, Some(AccessChoice::Full)));
    })
}

// S17: thread-defaults.test.ts:185
#[test]
fn an_agent_turned_off_is_refused() {
    run_local(async {
        let fx = fx();
        hook!(fx.host, "LaunchCtx::agent_off", dyn Fn(&Entry, &str) -> bool, |_e: &Entry, h: &str| h == "claude");
        hook!(fx.host, "SessionsCtx::agent_label", dyn Fn(&str) -> String, |_h: &str| "Claude Code".to_string());
        hook!(fx.host, "SessionsCtx::computer_of", dyn Fn(&Entry) -> String, |_e: &Entry| "mac".to_string());
        let e = fx.refused(StartOptions { harness: Some("claude".into()), ..go("one") }, None).await;
        // agentOffLine (thread-defaults.ts:374) has no wsp-words function yet, so the sentence is spelled out.
        assert_eq!(said(&e), ("Claude Code is off on mac, so no thread there runs it".to_string(), None, Some(Kind::Usage)));
        assert_eq!(fx.host.hooks.called("confine_setup"), Vec::<String>::new());
        assert_eq!(fx.host.hooks.called("refuse_sub_send"), Vec::<String>::new());
        before_the_mint(&fx.host);
    })
}

// S17
#[test]
fn the_agent_is_the_threads_else_the_outsides_else_the_asked_else_the_default() {
    run_local(async {
        let codex_default = |fx: &Fx| hook!(fx.host, "LaunchCtx::default_agent_of", dyn Fn(&Preferences, &Entry) -> String, |_p: &Preferences, _e: &Entry| "codex".to_string());

        let fx = fx();
        codex_default(&fx);
        let p = fx.launches(go("one"), None).await;
        assert_eq!(p.opts.harness, Some("codex".to_string()), "none named: the default");
        assert_eq!(order(&fx.host, &["default_agent_of", "agent_off", "confine_setup"]), strings(&["default_agent_of", "agent_off(codex)", "confine_setup(codex)"]));
        assert_eq!(fx.asked.borrow()[0].0, "codex");

        let fx = fx_on(Agent::default());
        codex_default(&fx);
        let p = fx.launches(StartOptions { harness: Some("claude".into()), ..go("one") }, None).await;
        assert_eq!(p.opts.harness, Some("claude".to_string()), "named");
        assert_eq!(fx.host.hooks.called("default_agent_of"), Vec::<String>::new());

        let fx = fx_on(Agent::default());
        let p = fx.launches(StartOptions { harness: Some("claude".into()), outside: Some(outside("codex", "o-1", false)), ..go("one") }, None).await;
        assert_eq!(p.opts.harness, Some("codex".to_string()), "the outside conversation's agent beats the asked one");
    })
}

// ---------------------------------------------------------------------------------------------------------------
// S18, S19

// S18: sub-queue.test.ts:308
#[test]
fn a_deleted_workspace_refuses_the_start() {
    run_local(async {
        let fx = fx();
        let ghost: Entry = entry("w1", json!({ "name": "Second" }));
        hook!(fx.host, "LaunchCtx::entry_of", dyn Fn(&str, Option<Caller>) -> LocalBoxFuture<'static, Result<Entry, HostError>>, move |_w: &str, _o: Option<Caller>| {
            let e = ghost.clone();
            async move { Ok(e) }.boxed_local()
        });
        // The workspace is gone from `live` by the time the check runs (sessions.ts:351), though its entry was read.
        hook!(fx.host, "SessionsCtx::live_entry", dyn Fn(&str) -> Option<Entry>, |_w: &str| None);
        let e = fx.refused(go("hi"), None).await;
        assert_eq!(said(&e), said(&err(&wsp_words::place_state::deleted_before_start_line("Second"))));
        assert_eq!(fx.host.hooks.called("refuse_sub_send"), Vec::<String>::new());
        before_the_mint(&fx.host);
    })
}

// S18: box-nap.test.ts:867
#[test]
fn a_napped_workspace_refuses_in_its_words() {
    run_local(async {
        let fx = fx();
        fx.host.add_entry(entry("w1", json!({ "name": "x", "phase": "napping" })));
        let e = fx.refused(go("go"), None).await;
        let state = workspace_state(&WorkspaceStateInput { phase: WorkspacePhase::Napping, machine_state: None, reach: None, wake_refused: None });
        let want = send_refusal(&SendRefusalKind::State(state), None, Some("x")).expect("a napping workspace refuses a send");
        assert_eq!(said(&e), said(&err(&want)));
        assert_eq!(fx.host.hooks.called("refuse_sub_send"), strings(&["refuse_sub_send"]));
        before_the_mint(&fx.host);

        let fx = fx_on(Agent::default());
        fx.host.add_entry(entry("w1", json!({ "name": "x", "phase": "gone", "gone": "the provider lost it" })));
        let e = fx.refused(go("go"), None).await;
        let state = workspace_state(&WorkspaceStateInput { phase: WorkspacePhase::Gone, machine_state: None, reach: None, wake_refused: None });
        let want = send_refusal(&SendRefusalKind::State(state), Some("the provider lost it"), Some("x")).expect("a gone workspace refuses a send");
        assert_eq!(said(&e), said(&err(&want)));
        before_the_mint(&fx.host);
    })
}

// S18: sessions.ts:358
#[test]
fn an_unchecked_running_machine_is_proven_before_its_state_is_read() {
    run_local(async {
        let fx = fx();
        let unchecked = entry("w1", json!({ "name": "lab" }));
        unchecked.borrow_mut().unchecked = true;
        fx.host.add_entry(unchecked);
        fx.launches(go("hi"), None).await;
        assert_eq!(order(&fx.host, &["confine_setup", "proven", "refuse_sub_send", "copy_blocked"]), strings(&["confine_setup(claude)", "proven", "refuse_sub_send", "copy_blocked"]));

        let fx = fx_on(Agent::default());
        let napping = entry("w1", json!({ "name": "lab", "phase": "napping" }));
        napping.borrow_mut().unchecked = true;
        fx.host.add_entry(napping);
        fx.refused(go("hi"), None).await;
        assert_eq!(fx.host.hooks.called("proven"), Vec::<String>::new(), "only a running machine is proven");

        let fx = fx_on(Agent::default());
        fx.launches(go("hi"), None).await;
        assert_eq!(fx.host.hooks.called("proven"), Vec::<String>::new(), "a checked machine is not");
    })
}

// S18
#[test]
fn a_sub_machines_own_refusal_stands_and_ends_the_checks() {
    run_local(async {
        let fx = fx();
        let own = usage("wait for it", "a fix");
        let o = own.clone();
        hook!(fx.host, "SessionsCtx::refuse_sub_send", dyn Fn(&Entry) -> Result<(), HostError>, move |_e: &Entry| Err(o.clone()));
        let e = fx.refused(go("hi"), None).await;
        assert_eq!(said(&e), said(&own));
        assert_eq!(fx.host.hooks.called("copy_blocked"), Vec::<String>::new());
        before_the_mint(&fx.host);
    })
}

// S19
#[test]
fn a_thread_not_running_is_checked_for_a_blocked_copy_its_mounts_and_a_kept_agent() {
    run_local(async {
        let fx = fx();
        let p = fx.launches(go("hi"), None).await;
        assert!(p.cleared);
        assert_eq!(
            order(&fx.host, &["copy_blocked", "worktree_mounted", "kept_check", "spawn_guard"]),
            strings(&["copy_blocked", "worktree_mounted", "kept_check(thr-new)", "spawn_guard(thread_new)"])
        );

        let fx = fx_on(Agent::default());
        hook!(fx.host, "SessionsCtx::copy_blocked", dyn Fn(&Entry) -> LocalBoxFuture<'static, Result<(), HostError>>, |_e: &Entry| async { Err(HostError::new("this computer is blocked")) }.boxed_local());
        let e = fx.refused(go("hi"), None).await;
        assert_eq!(said(&e), said(&err("this computer is blocked")));
        assert_eq!(order(&fx.host, &["worktree_mounted", "kept_check", "spawn_guard"]), Vec::<String>::new());
        before_the_mint(&fx.host);
    })
}

// S19
#[test]
fn a_running_thread_is_not_checked_that_way_and_is_not_cleared() {
    run_local(async {
        let fx = fx();
        thread_in(&fx.host, "t1", "w1", "claude", "running");
        let p = fx.launches(StartOptions { thread: Some("t1".into()), ..go("more") }, None).await;
        assert!(!p.cleared);
        assert_eq!(order(&fx.host, &["copy_blocked", "worktree_mounted", "kept_check"]), Vec::<String>::new());
        assert_eq!(fx.host.hooks.called("spawn_guard"), strings(&["spawn_guard(send)"]));
    })
}

// S19: session-title.test.ts:684
#[test]
fn a_blank_title_is_refused() {
    run_local(async {
        let fx = fx();
        let e = fx.refused(StartOptions { title: Some("  \n ".into()), ..go("go") }, None).await;
        assert_eq!(said(&e), said(&err(EMPTY_TITLE_LINE)));
        assert_eq!(fx.host.hooks.called("spawn_guard"), Vec::<String>::new());
        before_the_mint(&fx.host);
    })
}

// S19
#[test]
fn a_title_is_the_first_line_trimmed_and_the_guard_names_the_act() {
    run_local(async {
        let fx = fx();
        let raw = "  Review  1671\nsecond line";
        let p = fx.launches(StartOptions { title: Some(raw.into()), ..go("go") }, None).await;
        assert_eq!(p.title, Some(title_line(raw)));

        let fx = fx_on(Agent::default());
        let child = by(WorkspaceOrigin::Relayed, scope("kid", "w1", "lead-t"));
        let denial = usage("a thread may not start here", "ask the lead");
        let d = denial.clone();
        hook!(fx.host, "SessionsCtx::spawn_guard", dyn Fn(&str, Option<&Caller>) -> Result<(), HostError>, move |_a: &str, _o: Option<&Caller>| Err(d.clone()));
        let e = fx.refused(go("hi"), Some(child)).await;
        assert_eq!(said(&e), said(&denial));
        assert_eq!(fx.host.hooks.called("spawn_guard"), strings(&["spawn_guard(thread_new)"]));
        before_the_mint(&fx.host);
    })
}

// ---------------------------------------------------------------------------------------------------------------
// S20, S21, invariant 3

// S20: replaces.test.ts:136
#[test]
fn a_persons_restart_of_a_child_stands_where_the_replaced_thread_stood() {
    run_local(async {
        let restart = || StartOptions { replaces: Some("a1".into()), ..go("build the parser") };
        let finished = |fx: &Fx| hook!(fx.host, "SessionsCtx::thread_facts", dyn Fn(&str) -> Option<ThreadFacts>, |t: &str| if t == "a1" { Some(facts("a1", "completed")) } else { None });

        let fx = fx();
        finished(&fx);
        hook!(fx.host, "SessionsCtx::parent_of", dyn Fn(&str) -> Option<String>, |t: &str| if t == "a1" { Some("lead-t".to_string()) } else { None });
        hook!(fx.host, "SessionsCtx::root_of", dyn Fn(&str) -> String, |t: &str| if t == "a1" { "lead-t".to_string() } else { t.to_string() });
        let p = fx.launches(restart(), None).await;
        assert_eq!(p.tree, TreeOf { parent_thread_id: Some("lead-t".into()), root_thread_id: Some("lead-t".into()) });
        assert_eq!(p.replaces, Some("a1".to_string()));
        assert_eq!(fx.host.hooks.called("tree_of"), Vec::<String>::new());

        let fx = fx_on(Agent::default());
        finished(&fx);
        let stands = TreeOf { parent_thread_id: Some("top".into()), root_thread_id: Some("top".into()) };
        let s = stands.clone();
        hook!(fx.host, "SessionsCtx::tree_of", dyn Fn(Option<&ThreadScope>) -> TreeOf, move |_s: Option<&ThreadScope>| s.clone());
        let p = fx.launches(restart(), None).await;
        assert_eq!(p.tree, stands, "a replaced thread with no parent is the person's own: the caller's tree");
        assert_eq!(fx.host.hooks.called("tree_of"), strings(&["tree_of"]));
    })
}

// S20
#[test]
fn an_opening_hangs_under_its_callers_root_and_a_send_reads_its_threads_own_tree() {
    run_local(async {
        let fx = fx();
        let kid = scope("kid", "w1", "lead-t");
        let seen: Rc<RefCell<Vec<Option<ThreadScope>>>> = Rc::new(RefCell::new(Vec::new()));
        let log = seen.clone();
        hook!(fx.host, "SessionsCtx::tree_of", dyn Fn(Option<&ThreadScope>) -> TreeOf, move |s: Option<&ThreadScope>| {
            log.borrow_mut().push(s.cloned());
            TreeOf { parent_thread_id: Some("kid".into()), root_thread_id: Some("lead-t".into()) }
        });
        let p = fx.launches(go("child"), Some(by(WorkspaceOrigin::Relayed, kid.clone()))).await;
        assert_eq!(p.tree, TreeOf { parent_thread_id: Some("kid".into()), root_thread_id: Some("lead-t".into()) });
        assert_eq!(*seen.borrow(), vec![Some(kid)]);

        let fx = fx_on(Agent::default());
        fx.thread_on("t1", "w1", "claude");
        hook!(fx.host, "SessionsCtx::parent_of", dyn Fn(&str) -> Option<String>, |t: &str| if t == "t1" { Some("kid".to_string()) } else { None });
        hook!(fx.host, "SessionsCtx::root_of", dyn Fn(&str) -> String, |t: &str| if t == "t1" { "lead-t".to_string() } else { t.to_string() });
        let p = fx.launches(StartOptions { thread: Some("t1".into()), ..go("more") }, None).await;
        assert_eq!(p.tree, TreeOf { parent_thread_id: Some("kid".into()), root_thread_id: Some("lead-t".into()) });
        assert_eq!(fx.host.hooks.called("tree_of"), Vec::<String>::new());

        let fx = fx_on(Agent::default());
        fx.thread_on("t1", "w1", "claude");
        let p = fx.launches(StartOptions { thread: Some("t1".into()), ..go("more") }, None).await;
        assert_eq!(p.tree, TreeOf::default(), "a thread that is its own root with no parent carries no tree keys");
    })
}

// S20, S21, invariant 3: spawn-guard.test.ts:1309, runtime-notify.test.ts:474
#[test]
fn invariant_3_the_tree_and_notify_by_come_from_the_callers_token() {
    run_local(async {
        let lead = scope("T", "w1", "R");
        let caller = by(WorkspaceOrigin::Relayed, lead.clone());
        let tokened = || StartOptions { notify: Some(strings(&["me"])), turn_token: Some("tok-T".into()), started_by: Some(SessionOrigin::Agent), ..go("build it") };
        let wire = |fx: &Fx, seen: &Rc<RefCell<Vec<Option<ThreadScope>>>>| {
            fx.thread_on("T", "w1", "claude");
            hook!(fx.host, "SessionsCtx::thread_of_token", dyn Fn(&str) -> Result<String, HostError>, |t: &str| Ok(if t == "tok-T" { "T".to_string() } else { "someone-else".to_string() }));
            let log = seen.clone();
            hook!(fx.host, "SessionsCtx::tree_of", dyn Fn(Option<&ThreadScope>) -> TreeOf, move |s: Option<&ThreadScope>| {
                log.borrow_mut().push(s.cloned());
                match s {
                    Some(s) => TreeOf { parent_thread_id: Some(s.thread_id.clone()), root_thread_id: Some(s.root_thread_id.clone()) },
                    None => TreeOf::default(),
                }
            });
        };

        // An opening out of a thread: the tree and the notify scope are the caller's, and me is that thread.
        let seen = Rc::new(RefCell::new(Vec::new()));
        let fx = fx();
        wire(&fx, &seen);
        let p = fx.launches(tokened(), Some(caller.clone())).await;
        assert_eq!(*seen.borrow(), vec![Some(lead.clone())]);
        assert_eq!(p.tree, TreeOf { parent_thread_id: Some("T".into()), root_thread_id: Some("R".into()) });
        assert_eq!(p.asked, Some(strings(&["T"])));
        assert_eq!(p.notify, Some(strings(&["T"])));
        assert_eq!(p.notify_by, Some(lead.clone()));
        assert_eq!(p.notify_road, Some(WorkspaceOrigin::Relayed));
        assert_eq!(fx.host.hooks.called("thread_of_token"), strings(&["thread_of_token(tok-T)"]));

        // The same message from the person: no scope, no road, and me stays me, since nothing names a thread.
        let seen = Rc::new(RefCell::new(Vec::new()));
        let fx = fx_on(Agent::default());
        wire(&fx, &seen);
        let p = fx.launches(StartOptions { turn_token: None, ..tokened() }, None).await;
        assert_eq!(*seen.borrow(), vec![None]);
        assert_eq!(p.tree, TreeOf::default());
        assert_eq!(p.asked, Some(strings(&["me"])));
        assert_eq!((p.notify_by.clone(), p.notify_road), (None, None));
        assert_eq!(fx.host.hooks.called("thread_of_token"), Vec::<String>::new());

        // The same message by a road with no thread behind it: a road and no scope.
        let seen = Rc::new(RefCell::new(Vec::new()));
        let fx = fx_on(Agent::default());
        wire(&fx, &seen);
        let p = fx.launches(StartOptions { turn_token: None, ..tokened() }, Some(Caller::Road(WorkspaceOrigin::Paired))).await;
        assert_eq!(*seen.borrow(), vec![None]);
        assert_eq!((p.notify_by, p.notify_road), (None, Some(WorkspaceOrigin::Paired)));

        // A send into a thread that names no targets takes what the thread's opener registered, whoever sends it.
        let fx = fx_on(Agent::default());
        fx.thread_on("t1", "w1", "claude");
        let registered = NotifyOn { notify: strings(&["boss"]), by: Some(scope("boss", "w1", "boss")), road: Some(WorkspaceOrigin::Here) };
        let r = registered.clone();
        hook!(fx.host, "SessionsCtx::notify_on", dyn Fn(&str) -> Option<NotifyOn>, move |t: &str| if t == "t1" { Some(r.clone()) } else { None });
        let p = fx.launches(StartOptions { thread: Some("t1".into()), ..go("more") }, Some(caller.clone())).await;
        assert_eq!((p.asked, p.notify, p.notify_by, p.notify_road), (None, Some(strings(&["boss"])), registered.by.clone(), Some(WorkspaceOrigin::Here)));

        // A send that names targets stamps the caller's scope and road over the registered ones.
        let fx = fx_on(Agent::default());
        fx.thread_on("t1", "w1", "claude");
        fx.thread_on("T", "w1", "claude");
        let r = registered.clone();
        hook!(fx.host, "SessionsCtx::notify_on", dyn Fn(&str) -> Option<NotifyOn>, move |t: &str| if t == "t1" { Some(r.clone()) } else { None });
        let p = fx.launches(StartOptions { thread: Some("t1".into()), notify: Some(strings(&["T"])), ..go("more") }, Some(caller)).await;
        assert_eq!((p.notify, p.notify_by, p.notify_road), (Some(strings(&["T"])), Some(lead), Some(WorkspaceOrigin::Relayed)));
    })
}

// S21: runtime-notify.test.ts:474, :580
#[test]
fn notify_me_is_the_callers_thread() {
    run_local(async {
        let fx = fx();
        for t in ["T1", "T2"] {
            fx.thread_on(t, "w1", "claude");
        }
        hook!(fx.host, "SessionsCtx::thread_of_token", dyn Fn(&str) -> Result<String, HostError>, |t: &str| match t {
            "tok-1" => Ok("T1".to_string()),
            "tok-2" => Ok("T2".to_string()),
            other => Ok(other.to_string()),
        });
        let run = |notify: &[&str], token: Option<&str>| StartOptions { notify: Some(strings(notify)), turn_token: token.map(String::from), ..go("build") };

        assert_eq!(fx.launches(run(&["me"], Some("tok-1")), None).await.notify, Some(strings(&["T1"])));
        assert_eq!(fx.launches(run(&["me"], Some("tok-2")), None).await.notify, Some(strings(&["T2"])), "each token resolves to its own thread");
        assert_eq!(fx.launches(run(&["T1", "T1"], None), None).await.notify, Some(strings(&["T1"])), "named twice is one target");
        assert_eq!(fx.launches(run(&["me", "T1"], Some("tok-1")), None).await.notify, Some(strings(&["T1"])), "me and its own thread are one target");
        assert_eq!(fx.launches(run(&["T2", "me", "T2"], Some("tok-1")), None).await.notify, Some(strings(&["T2", "T1"])), "the first of a target is kept, in order");
        let before = fx.host.hooks.called("thread_of_token").len();
        assert_eq!(fx.launches(run(&["me"], None), None).await.notify, Some(strings(&["me"])), "no token: me is the person");
        assert_eq!(fx.host.hooks.called("thread_of_token").len(), before);
    })
}

// S21: runtime-notify.test.ts:283, :296, spawn-guard.test.ts:1309, :1351
#[test]
fn notify_refuses_itself_a_stranger_and_a_cycle() {
    run_local(async {
        // a stranger: not held, then held but not reached
        let fx = fx();
        let e = fx.refused(StartOptions { notify: Some(strings(&["other-thread"])), ..go("out") }, None).await;
        assert_eq!(said(&e), said(&err("no thread other-thread to notify")));
        before_the_mint(&fx.host);

        let fx = fx_on(Agent::default());
        fx.thread_on("mine", "w1", "claude");
        hook!(fx.host, "SessionsCtx::reaches_row", dyn Fn(&SessionView, Option<&Caller>, Option<TreeTalk>) -> bool, |_v: &SessionView, _o: Option<&Caller>, _k: Option<TreeTalk>| false);
        let child = by(WorkspaceOrigin::Relayed, scope("kid", "w1", "lead-t"));
        let e = fx.refused(StartOptions { notify: Some(strings(&["mine"])), ..go("out") }, Some(child)).await;
        assert_eq!(said(&e), said(&err("no thread mine to notify")));
        before_the_mint(&fx.host);

        // itself, by id and through me
        let fx = fx_on(Agent::default());
        fx.thread_on("t1", "w1", "claude");
        hook!(fx.host, "SessionsCtx::thread_of_token", dyn Fn(&str) -> Result<String, HostError>, |_t: &str| Ok("t1".to_string()));
        for opts in [
            StartOptions { thread: Some("t1".into()), notify: Some(strings(&["t1"])), ..go("again") },
            StartOptions { thread: Some("t1".into()), notify: Some(strings(&["me"])), turn_token: Some("tok".into()), ..go("again") },
        ] {
            let e = fx.refused(opts, None).await;
            assert_eq!(said(&e), said(&err("a thread cannot notify itself")));
        }
        before_the_mint(&fx.host);

        // a cycle: the target's chain reaches this thread
        let target = "bbbbbbbb-1234-5678";
        let fx = fx_on(Agent::default());
        fx.thread_on(target, "w1", "claude");
        hook!(fx.host, "SessionsCtx::notify_on", dyn Fn(&str) -> Option<NotifyOn>, move |t: &str| if t == target { Some(NotifyOn { notify: strings(&["c-thread"]), by: None, road: None }) } else { None });
        let reached: Rc<RefCell<Vec<Vec<String>>>> = Rc::new(RefCell::new(Vec::new()));
        let log = reached.clone();
        hook!(fx.host, "SessionsCtx::notify_reach", dyn Fn(&[String]) -> IndexSet<String>, move |n: &[String]| {
            log.borrow_mut().push(n.to_vec());
            ["c-thread", "thr-new"].iter().map(|s| s.to_string()).collect()
        });
        let e = fx.refused(StartOptions { notify: Some(strings(&[target])), ..go("loop") }, None).await;
        assert_eq!(said(&e), said(&err("thread bbbbbbbb already notifies this thread; a cycle would run forever")));
        assert_eq!(*reached.borrow(), vec![strings(&["c-thread"])]);
        before_the_mint(&fx.host);

        // a chain that does not come back passes
        hook!(fx.host, "SessionsCtx::notify_reach", dyn Fn(&[String]) -> IndexSet<String>, |_n: &[String]| ["c-thread"].iter().map(|s| s.to_string()).collect());
        fx.host.ids.uuids.borrow_mut().push("thr-new".into());
        let p = fx.launches(StartOptions { notify: Some(strings(&[target])), ..go("no loop") }, None).await;
        assert_eq!(p.notify, Some(strings(&[target])));
    })
}

// ---------------------------------------------------------------------------------------------------------------
// S22, S23, S24

// S22, S18
#[test]
fn a_released_sub_thread_is_resumed_on_its_new_machine() {
    run_local(async {
        let fx = fx();
        let mut doc = sub_json(false);
        doc["phase"] = json!("released");
        fx.host.add_entry(entry("w1", doc));
        let moved = fx.host.add_entry(entry("w2", json!({ "name": "web", "kind": "cloud" })));
        let to = moved.clone();
        hook!(
            fx.host,
            "SessionsCtx::resume_sub",
            dyn Fn(&Entry, &str, &str, &str) -> LocalBoxFuture<'static, Result<Resumed, HostError>>,
            move |_e: &Entry, _t: &str, _h: &str, _p: &str| {
                let entry = to.clone();
                async move { Ok(Resumed { entry, preamble: Some("You run on a machine.".into()), notices: strings(&["moved", "again"]) }) }.boxed_local()
            }
        );
        let minted_for: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        let log = minted_for.clone();
        hook!(
            fx.host,
            "LaunchCtx::thread_launch",
            dyn Fn(&Entry, &str, &str, bool) -> LocalBoxFuture<'static, Result<ThreadLaunch, HostError>>,
            move |e: &Entry, _t: &str, _r: &str, _a: bool| {
                log.borrow_mut().push(id_of(e));
                async { Ok(ThreadLaunch::default()) }.boxed_local()
            }
        );
        let p = fx.launches(go("pushed it"), None).await;
        assert_eq!(id_of(&p.entry), "w2");
        assert_eq!(p.opts.prompt, sub_opening("You run on a machine.", "pushed it"));
        assert_eq!(p.notice, Some("moved\nagain".to_string()));
        assert_eq!(p.clouds, None, "a sub-machine is told no clouds");
        assert_eq!(*minted_for.borrow(), strings(&["w2"]), "the launch is minted for the machine the turn runs on");
        assert_eq!(order(&fx.host, &["refuse_sub_send", "resume_sub", "thread_launch"]), strings(&["refuse_sub_send", "resume_sub(thr-new, claude, pushed it)", "thread_launch(thr-new, thr-new, false)"]));
    })
}

// S23
#[test]
fn clouds_are_told_for_a_folder_workspace_that_is_no_sub_machine() {
    run_local(async {
        let fx = fx();
        assert_eq!(fx.launches(go("hi"), None).await.clouds, Some(Vec::new()), "a folder, with no clouds held");
        assert_eq!(fx.host.hooks.called("place_rows"), strings(&["place_rows"]));

        let fx = fx_on(Agent::default());
        fx.host.add_entry(entry("w1", json!({ "name": "lab", "kind": "cloud" })));
        assert_eq!(fx.launches(go("hi"), None).await.clouds, None, "a machine");
        assert_eq!(fx.host.hooks.called("place_rows"), Vec::<String>::new());
    })
}

// S24: sub-queue.test.ts:175
#[test]
fn a_queued_workspace_holds_the_start() {
    run_local(async {
        let fx = fx();
        fx.host.add_entry(entry("w1", json!({ "name": "Second", "phase": "queued", "queue": { "since": "2026-10-01T00:00:00.000Z", "why": "slot" } })));
        let kid = scope("kid", "w1", "lead-t");
        let caller = by(WorkspaceOrigin::Relayed, kid);
        let tree = TreeOf { parent_thread_id: Some("lead-t".into()), root_thread_id: Some("lead-t".into()) };
        let t = tree.clone();
        hook!(fx.host, "SessionsCtx::tree_of", dyn Fn(Option<&ThreadScope>) -> TreeOf, move |_s: Option<&ThreadScope>| t.clone());
        hook!(fx.host, "SessionsCtx::notify_on", dyn Fn(&str) -> Option<NotifyOn>, |_t: &str| Some(NotifyOn { notify: strings(&["boss"]), by: None, road: None }));
        let held: Rc<RefCell<Vec<(String, String, TreeOf, String, Option<String>, Option<String>, Option<Vec<String>>, Option<Caller>)>>> = Rc::new(RefCell::new(Vec::new()));
        let log = held.clone();
        hook!(
            fx.host,
            "SessionsCtx::hold_queued",
            dyn Fn(&str, &str, TreeOf, StartOptions, Option<Vec<String>>, Option<&Caller>) -> LocalBoxFuture<'static, Result<Rc<dyn SessionHandle>, HostError>>,
            move |w: &str, t: &str, tree: TreeOf, o: StartOptions, n: Option<Vec<String>>, c: Option<&Caller>| {
                log.borrow_mut().push((w.to_string(), t.to_string(), tree, o.prompt.clone(), o.title.clone(), o.harness.clone(), n, c.cloned()));
                async { Ok(Handle::new("held-row", "held-turn")) }.boxed_local()
            }
        );
        let opts = StartOptions { title: Some("  Second  \nmore".into()), ..go("build it") };
        match fx.prepare(opts, Some(caller.clone())).await {
            Ok(Checked::Answered(h)) => assert_eq!((h.id(), h.turn_id()), ("held-row".to_string(), "held-turn".to_string())),
            Ok(Checked::Launch(_)) => panic!("a queued workspace launches nothing"),
            Err(e) => panic!("refused: {}", e.message),
        }
        let want_title = title_line("  Second  \nmore");
        assert_eq!(
            *held.borrow(),
            vec![("w1".to_string(), "thr-new".to_string(), tree, "build it".to_string(), Some(want_title), Some("claude".to_string()), None, Some(caller))],
            "the targets passed are the ones the start named, none here, never the registered ones"
        );
        assert_eq!(fx.host.hooks.called("adapter_for"), Vec::<String>::new());
        before_the_mint(&fx.host);
    })
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
            async move { Ok(ThreadLaunch { scoped_device_id: Some("dev-1".into()), env, wsp: Some(McpServerSpec { command: "wsp".into(), args: strings(&["mcp"]), no_slate: None }) }) }.boxed_local()
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
        turn: if has_turn { Some(ForkPlanTurn { turn_id: "turn-src".into(), anchor: None, at_ref: None, after: Vec::new() }) } else { None },
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
        _ => panic!("the plan was {}, and was to be {}", if got.is_some() { "held" } else { "none" }, if want.is_some() { "held" } else { "none" }),
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

// every check passed: what the launch is handed
#[test]
fn a_plain_opening_prepares_everything_the_launch_needs() {
    run_local(async {
        let fx = fx_on(Agent { takes_servers: true, ..Default::default() });
        minted(&fx);
        let p = fx.launches(go("hello"), None).await;
        assert_eq!(p.workspace_id, "w1");
        assert_eq!(p.origin, None);
        assert_eq!(p.opts.prompt, "hello");
        assert_eq!(p.opts.harness, Some("claude".to_string()));
        assert_eq!(p.prefs, default_preferences());
        same_plan(&p.plan, &None);
        assert_eq!((p.thread_id.as_str(), p.opens, p.cleared), ("thr-new", true, true));
        assert_eq!(id_of(&p.entry), "w1");
        assert_eq!((p.replaces.clone(), p.resume.clone(), p.gone_now.clone(), p.title.clone()), (None, None, None, None));
        assert_eq!(p.tree, TreeOf::default());
        assert_eq!((p.asked.clone(), p.notify.clone(), p.notify_by.clone(), p.notify_road, p.notice.clone()), (None, None, None, None, None));
        assert_eq!(p.clouds, Some(Vec::new()));
        assert_eq!(p.turn_token, "01".repeat(16), "randomBytes(16) as hex, drawn after the thread id");
        assert_eq!(p.launch.scoped_device_id, Some("dev-1".to_string()));
        assert_eq!(p.built.harness, "claude");
        assert_eq!(p.fork_turn, None);
        assert!(p.records.is_empty());
        assert_eq!(p.mcp_servers.as_ref().map(|m| m.keys().cloned().collect::<Vec<_>>()), Some(strings(&["wsp"])));
        assert!(!p.waiting.get());

        let asked = fx.asked.borrow();
        assert_eq!(asked.len(), 1);
        let (harness, ask) = &asked[0];
        assert_eq!(harness, "claude");
        let env: Vec<(&str, &str)> = ask.turn_env.iter().flatten().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        assert_eq!(env, vec![(TURN_TOKEN_ENV, p.turn_token.as_str()), (THREAD_ENV, "thr-new"), ("WSP_HOST", "h-tok")]);
        assert_eq!(ask.thread, Some("thr-new".to_string()));
        assert!(!ask.asks_until_stopped);
        let reads = ask.waiting.as_ref().expect("the waiting flag");
        assert!(!reads(), "the adapter reads the flag the row writes");
        p.waiting.set(true);
        assert!(reads(), "the adapter reads the flag the row writes");
        p.waiting.set(false);
        assert_eq!(
            order(
                &fx.host,
                &["ready", "preferences_held", "preferences", "fork_opened", "entry_of", "default_agent_of", "agent_off", "confine_setup", "refuse_sub_send", "copy_blocked", "worktree_mounted", "kept_check", "spawn_guard", "tree_of", "notify_on", "place_rows", "thread_launch", "adapter_for"]
            ),
            strings(&[
                "ready", "preferences_held", "preferences", "fork_opened(w1)", "entry_of(w1)", "default_agent_of", "agent_off(claude)", "confine_setup(claude)", "refuse_sub_send", "copy_blocked",
                "worktree_mounted", "kept_check(thr-new)", "spawn_guard(thread_new)", "tree_of", "notify_on(thr-new)", "place_rows", "thread_launch(thr-new, thr-new, false)", "adapter_for(claude)"
            ])
        );

        let fx = fx_on(Agent::default());
        fx.launches(StartOptions { asks_until_stopped: true, ..go("hello") }, None).await;
        assert!(fx.asked.borrow()[0].1.asks_until_stopped);
    })
}

// S25: spawn-guard.test.ts:1233
#[test]
fn a_refusal_after_the_mint_revokes_the_device() {
    run_local(async {
        let revoked = |fx: &Fx| order(&fx.host, &["revoke_device"]);

        // a harness no adapter is built for
        let fx = fx();
        minted(&fx);
        hook!(fx.host, "LaunchCtx::adapter_for", dyn Fn(&Entry, AdapterAsk) -> Result<BuiltAdapter, HostError>, |_e: &Entry, a: AdapterAsk| Err(HostError::new(format!("no adapter for {}", a.named.unwrap_or_default()))));
        let e = fx.refused(StartOptions { harness: Some("nope".into()), ..go("hi") }, None).await;
        settle().await;
        assert_eq!(said(&e), said(&err("no adapter for nope")));
        assert_eq!(revoked(&fx), strings(&["revoke_device(dev-1)"]));
        nothing_held(&fx.host);

        // a fork the agent cannot make
        let fx = fx_on(Agent::default());
        minted(&fx);
        forking(&fx, plan_with(true, Some("src-sess")));
        let e = fx.refused(go("hi"), None).await;
        settle().await;
        assert_eq!(said(&e), said(&usage(&forks_not_line("claude"), FORKS_NOT_FIX)));
        assert_eq!(revoked(&fx), strings(&["revoke_device(dev-1)"]));
        nothing_held(&fx.host);

        // a fork of a thread whose agent has said nothing yet
        let fx = fx_on(Agent { forks: true, ..Default::default() });
        minted(&fx);
        forking(&fx, plan_with(true, None));
        let e = fx.refused(go("hi"), None).await;
        settle().await;
        assert_eq!(said(&e), said(&refusal(&fork_refused_line("claude", ASIDE_NO_SESSION_LINE), FORK_REFUSED_FIX, Some(Kind::Conflict))));
        assert_eq!(revoked(&fx), strings(&["revoke_device(dev-1)"]));
        nothing_held(&fx.host);

        // files the harness refuses
        let fx = fx_on(Agent::default());
        minted(&fx);
        let file = Attachment { media_type: "image/png".into(), bytes: "aGk=".into(), name: Some("shot.png".into()) };
        let records = vec![attachment_record(&file)];
        let e = fx.refused(StartOptions { attachments: Some(vec![file]), ..go("hi") }, None).await;
        settle().await;
        let line = files_blocked(&records, None, "claude").expect("a harness with no road refuses an image (attachments.ts:149)");
        assert_eq!(said(&e), said(&err(&line)));
        assert_eq!(revoked(&fx), strings(&["revoke_device(dev-1)"]));
        nothing_held(&fx.host);

        // servers a harness takes none of, spawn-guard.test.ts:767
        let fx = fx_on(Agent::default());
        minted(&fx);
        let mut servers = IndexMap::new();
        servers.insert("mine".to_string(), McpServerSpec { command: "x".into(), args: Vec::new(), no_slate: None });
        let line = mcp_servers_blocked(Some(&servers), false, "claude").expect("a harness that takes none refuses servers");
        let e = fx.refused(StartOptions { mcp_servers: Some(servers), ..go("hi") }, None).await;
        settle().await;
        assert_eq!(said(&e), said(&err(&line)));
        assert_eq!(revoked(&fx), strings(&["revoke_device(dev-1)"]));
        nothing_held(&fx.host);
    })
}

// S25
#[test]
fn a_launch_with_no_scoped_device_revokes_nothing_and_a_failed_revoke_is_only_warned() {
    run_local(async {
        let fx = fx();
        hook!(fx.host, "LaunchCtx::adapter_for", dyn Fn(&Entry, AdapterAsk) -> Result<BuiltAdapter, HostError>, |_e: &Entry, _a: AdapterAsk| Err(HostError::new("no adapter")));
        let e = fx.refused(go("hi"), None).await;
        settle().await;
        assert_eq!(said(&e), said(&err("no adapter")));
        assert_eq!(fx.host.hooks.called("revoke_device"), Vec::<String>::new());

        let fx = fx_on(Agent::default());
        minted(&fx);
        hook!(fx.host, "LaunchCtx::adapter_for", dyn Fn(&Entry, AdapterAsk) -> Result<BuiltAdapter, HostError>, |_e: &Entry, _a: AdapterAsk| Err(HostError::new("no adapter")));
        hook!(fx.host, "LaunchCtx::revoke_device", dyn Fn(&str) -> LocalBoxFuture<'static, Result<(), HostError>>, |_d: &str| async { Err(HostError::new("the door is down")) }.boxed_local());
        let lines = console::capture();
        let e = fx.refused(go("hi"), None).await;
        settle().await;
        assert_eq!(said(&e), said(&err("no adapter")), "the refusal stands, not the revoke's failure");
        assert_eq!(lines.lines(), vec![format!("the token of thread {} was not taken away: the door is down", thread_word("thr-new"))]);
    })
}

// S25: the fork the agent can make carries its turn, by count where the agent forks by count
#[test]
fn a_fork_the_agent_can_make_carries_its_turn() {
    run_local(async {
        let fx = fx_on(Agent { forks: true, by_count: true, ..Default::default() });
        forking(&fx, plan_with(true, Some("src-sess")));
        hook!(fx.host, "SessionsCtx::fork_turn_of", dyn Fn(&ForkPlan, bool) -> Result<Option<ForkTurn>, HostError>, |_p: &ForkPlan, by_count: bool| Ok(Some(if by_count { ForkTurn::After(Vec::new()) } else { ForkTurn::Anchor("turn-src".into()) })));
        let p = fx.launches(go("hi"), None).await;
        assert_eq!(p.fork_turn, Some(ForkTurn::After(Vec::new())));
        same_plan(&p.plan, &Some(plan_with(true, Some("src-sess"))));
    })
}

// spawn-guard.test.ts:753, :767: the wsp tools ride a launch for a harness that takes servers
#[test]
fn the_wsp_tools_ride_the_launch_for_a_harness_that_takes_servers() {
    run_local(async {
        let wsp = McpServerSpec { command: "wsp".into(), args: strings(&["mcp"]), no_slate: None };
        let mine = McpServerSpec { command: "x".into(), args: Vec::new(), no_slate: None };

        let fx = fx_on(Agent { takes_servers: true, ..Default::default() });
        minted(&fx);
        let p = fx.launches(go("hi"), None).await;
        assert_eq!(p.mcp_servers.map(|m| m.into_iter().collect::<Vec<_>>()), Some(vec![("wsp".to_string(), wsp.clone())]));

        let fx = fx_on(Agent { takes_servers: true, ..Default::default() });
        minted(&fx);
        let mut named = IndexMap::new();
        named.insert("mine".to_string(), mine.clone());
        let p = fx.launches(StartOptions { mcp_servers: Some(named), ..go("hi") }, None).await;
        assert_eq!(p.mcp_servers.map(|m| m.into_iter().collect::<Vec<_>>()), Some(vec![("wsp".to_string(), wsp.clone()), ("mine".to_string(), mine.clone())]), "the wsp server first, the caller's after");

        let fx = fx_on(Agent::default());
        minted(&fx);
        let p = fx.launches(go("hi"), None).await;
        assert!(p.mcp_servers.is_none(), "a harness that takes none gets none and is not refused");

        let fx = fx_on(Agent { takes_servers: true, ..Default::default() });
        minted(&fx);
        hook!(fx.host, "SessionsCtx::tree_of", dyn Fn(Option<&ThreadScope>) -> TreeOf, |_s: Option<&ThreadScope>| TreeOf { parent_thread_id: Some("lead-t".into()), root_thread_id: Some("lead-t".into()) });
        let child = by(WorkspaceOrigin::Relayed, scope("kid", "w1", "lead-t"));
        let p = fx.launches(go("hi"), Some(child)).await;
        let served = wsp_threads::threads::launch::served_to(Some(&wsp), "lead-t", "thr-new").expect("a server");
        assert_eq!(p.mcp_servers.map(|m| m.into_iter().collect::<Vec<_>>()), Some(vec![("wsp".to_string(), served)]), "a thread under another serves its tools with no slate");
        assert_eq!(fx.host.hooks.called("thread_launch"), strings(&["thread_launch(thr-new, lead-t, false)"]), "the launch is minted for the tree's root");
    })
}
