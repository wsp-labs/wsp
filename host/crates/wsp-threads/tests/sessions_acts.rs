// SPDX-License-Identifier: AGPL-3.0-only
//! spec/sessions_acts.md. TS cases: runtime-steer.test.ts, steer-images.test.ts, permission-relay.test.ts,
//! thread-access.test.ts, session-title.test.ts, thread-marks.test.ts, aside.test.ts, session-run.test.ts
//! (the whole-runtime cases, ported at the seam).
#[path = "support_a/mod.rs"]
mod support;

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
    let by: ThreadScope = serde_json::from_value(json!({ "kind": "thread", "threadId": thread, "workspaceId": "w1", "rootThreadId": thread })).expect("a thread scope");
    Caller::Scoped { origin: WorkspaceOrigin::Here, by }
}

type Reached = Rc<RefCell<Vec<(Option<String>, String, Option<TreeTalk>, bool)>>>;
type EntryOfRow = dyn Fn(&RowAt, Option<&Caller>, Option<TreeTalk>, bool) -> LocalBoxFuture<'static, Result<Option<Entry>, HostError>>;
type AdapterFor = dyn Fn(&Entry, AdapterAsk) -> Result<BuiltAdapter, HostError>;
type LaunchAdapterFor = dyn Fn(&Entry, &str, Option<wsp_adapters::port::ServerValues>) -> LocalBoxFuture<'static, Result<BuiltAdapter, HostError>>;
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
    fn rename_session(&self, _id: String, _title: String, _exec: Arc<dyn HarnessExec>) -> Option<LocalBoxFuture<'static, Result<SessionRenameWrite, ExecFailure>>> {
        self.renames.then(|| futures::future::ready(Ok(SessionRenameWrite::Written)).boxed_local())
    }
    fn aside(&self, _q: AsideQuestion) -> Option<LocalBoxFuture<'static, Result<AsideAnswer, String>>> {
        self.aside.then(|| futures::future::ready(Err("this test answers through the context".to_string())).boxed_local())
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
    let state = workspace_state(&WorkspaceStateInput { phase: rec.phase.clone(), machine_state: None, reach: None, wake_refused: None });
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

// C1, C3: runtime-steer.test.ts:232, :286
#[test]
fn a_steer_is_kept_only_when_accepted() {
    run_local(async {
        let (host, area) = host();
        host.ids.uuids.borrow_mut().extend(["steer-1".to_string(), "steer-2".to_string()]);
        let steers = watch_steers(&host);
        let (taken, missed) = (Rc::new(RefCell::new(Vec::new())), Rc::new(RefCell::new(Vec::new())));
        put(&host, "sess-1", "w1", json!({ "threadId": "t1" }), Some(StubHandle { steer: Some(SteerTaken::Accepted), ..StubHandle::new(&taken) }));
        put(&host, "sess-2", "w1", json!({ "threadId": "t2" }), Some(StubHandle { steer: Some(SteerTaken::NotRunning), ..StubHandle::new(&missed) }));

        let got = area.steer_row("sess-1", steer_ask("and say pineapple", Some("req_2"), None), None).await.expect("a steer");
        assert_eq!(json_of(&got), json!({ "outcome": "accepted" }));
        assert_eq!(*taken.borrow(), vec!["steer(and say pineapple, Some(\"steer-1\"), [])".to_string()], "a fresh steer id, no images");
        assert_eq!(*steers.borrow(), vec![("sess-1".to_string(), "and say pineapple".to_string(), Some("req_2".to_string()), "steer-1".to_string())]);

        // The adapter says the line missed the turn: answered as it came, nothing kept.
        let late = area.steer_row("sess-2", steer_ask("racing", None, None), None).await.expect("a steer");
        assert_eq!(json_of(&late), json!({ "outcome": "not-running" }));
        assert_eq!(*missed.borrow(), vec!["steer(racing, Some(\"steer-2\"), [])".to_string()]);
        assert_eq!(steers.borrow().len(), 1, "nothing is recorded for a steer that missed");
    });
}

// C1: runtime-steer.test.ts:281, :286, :443
#[test]
fn steer_on_a_row_not_running_is_not_running() {
    run_local(async {
        let (host, area) = host();
        let steers = watch_steers(&host);
        let log = Rc::new(RefCell::new(Vec::new()));
        let road = || StubHandle { steer: Some(SteerTaken::Accepted), ..StubHandle::new(&log) };
        put(&host, "ended", "w1", json!({ "threadId": "t1", "status": "completed" }), Some(road()));
        put(&host, "launching", "w1", json!({ "threadId": "t2" }), None);
        put(&host, "plain", "w1", json!({ "threadId": "t3" }), Some(StubHandle::new(&log)));
        let outcome = |name: &'static str| {
            let area = area.clone();
            async move { json_of(&area.steer_row(name, steer_ask("too late", None, None), None).await.expect("an answer")) }
        };
        assert_eq!(outcome("ended").await, json!({ "outcome": "not-running" }), "a turn that ended");
        assert_eq!(outcome("launching").await, json!({ "outcome": "not-running" }), "a turn whose process this host does not hold");
        assert_eq!(outcome("plain").await, json!({ "outcome": "unsupported" }), "a harness that takes no message mid-turn");
        assert_eq!(outcome("nope").await, json!({ "outcome": "not-found" }));
        assert_eq!(*log.borrow(), Vec::<String>::new(), "no handle was asked");
        assert!(steers.borrow().is_empty());
    });
}

// C0, C1: permission-relay.test.ts:423
#[test]
fn a_steer_of_a_row_the_caller_does_not_reach_is_not_found() {
    run_local(async {
        let (host, area) = host();
        let log = Rc::new(RefCell::new(Vec::new()));
        put(&host, "sess-1", "w1", json!({ "threadId": "t1" }), Some(StubHandle { steer: Some(SteerTaken::Accepted), ..StubHandle::new(&log) }));
        let seen = Reached::default();
        unreached(&host, seen.clone());
        let got = area.steer_row("sess-1", steer_ask("x", None, None), Some(scoped("t9"))).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "outcome": "not-found" }));
        assert_eq!(*seen.borrow(), vec![(Some("t1".to_string()), "w1".to_string(), None, false)]);
        assert_eq!(*log.borrow(), Vec::<String>::new());
    });
}

// C0: runtime-steer.test.ts:485
#[test]
fn a_steer_on_a_workspace_that_will_not_send_is_refused() {
    run_local(async {
        let (host, area) = host();
        let log = Rc::new(RefCell::new(Vec::new()));
        put(&host, "sess-1", "w1", json!({ "threadId": "t1" }), Some(StubHandle { steer: Some(SteerTaken::Accepted), ..StubHandle::new(&log) }));
        for phase in ["pausing", "napping"] {
            host.add_entry(entry("w1", json!({ "phase": phase })));
            let err = area.steer_row("sess-1", steer_ask("x", None, None), None).await.expect_err("a refusal");
            assert_eq!(said(&err), (send_line(&host, true), None, None), "{phase}: the composer's sentence, no kind");
        }
        assert_eq!(*log.borrow(), Vec::<String>::new());
    });
}

// C2: steer-images.test.ts:105
#[test]
fn steer_refuses_files_the_harness_cannot_take() {
    run_local(async {
        let (host, area) = host();
        let log = Rc::new(RefCell::new(Vec::new()));
        put(&host, "sess-1", "w1", json!({ "threadId": "t1" }), Some(StubHandle { steer: Some(SteerTaken::Accepted), ..StubHandle::new(&log) }));
        let asked = adapters(&host, StubAdapter::default());
        let steers = watch_steers(&host);
        let refused = |files: Vec<Attachment>| {
            let area = area.clone();
            async move { said(&area.steer_row("sess-1", steer_ask("and this?", Some("req_2"), Some(files)), None).await.expect_err("a refusal")) }
        };
        assert_eq!(refused(vec![png()]).await, ("claude takes no image into a running turn".to_string(), None, None));
        assert_eq!(refused(vec![notes()]).await, ("a file that is not an image goes only with a message that starts a turn".to_string(), None, None));
        assert_eq!(refused(vec![png(); 6]).await, ("only 5 files fit one message; this one carries 6".to_string(), None, None), "the caps come first");
        assert!(asked.borrow().contains(&"adapter_for:claude".to_string()), "the row's own agent is asked whether its steer reads images");
        assert_eq!(*log.borrow(), Vec::<String>::new(), "nothing reaches the turn");
        assert!(steers.borrow().is_empty(), "nothing reaches the transcript");
    });
}

// C2, C3: steer-images.test.ts:67
#[test]
fn a_steer_hands_its_images_inline() {
    run_local(async {
        let (host, area) = host();
        host.ids.uuids.borrow_mut().push("steer-1".to_string());
        let log = Rc::new(RefCell::new(Vec::new()));
        put(&host, "sess-1", "w1", json!({ "threadId": "t1" }), Some(StubHandle { steer: Some(SteerTaken::Accepted), ..StubHandle::new(&log) }));
        adapters(&host, StubAdapter { steers_images: true, ..StubAdapter::default() });
        let steers = watch_steers(&host);
        let got = area.steer_row("sess-1", steer_ask("what is in this picture?", Some("req_2"), Some(vec![png()])), None).await.expect("a steer");
        assert_eq!(json_of(&got), json!({ "outcome": "accepted" }));
        let images = json!([{ "mediaType": "image/png", "bytes": png().bytes }]);
        assert_eq!(*log.borrow(), vec![format!("steer(what is in this picture?, Some(\"steer-1\"), {images})")]);
        assert_eq!(steers.borrow().len(), 1);
    });
}

// C4, C5, trap #1776: permission-relay.test.ts:229, :244
#[test]
fn answer_finds_the_running_row_by_the_agents_session_id() {
    run_local(async {
        let (host, area) = host();
        let (old, new, keyed, other) = (Rc::new(RefCell::new(Vec::new())), Rc::new(RefCell::new(Vec::new())), Rc::new(RefCell::new(Vec::new())), Rc::new(RefCell::new(Vec::new())));
        let road = |log: &Rc<RefCell<Vec<String>>>| StubHandle { answer: Some(SessionAnswerOutcome::Answered), ..StubHandle::new(log) };
        // The row keyed by the agent's id has ended; the turn that asks runs under a key of its own.
        put(&host, "agent-1", "w1", json!({ "threadId": "t1", "status": "completed", "claudeSessionId": "agent-1" }), Some(road(&old)));
        put(&host, "turn-2", "w1", json!({ "threadId": "t1", "claudeSessionId": "agent-1" }), Some(road(&new)));
        // A running row keyed by the id wins over another running row that carries the id as its session.
        put(&host, "k", "w1", json!({ "threadId": "t2", "claudeSessionId": "z" }), Some(road(&keyed)));
        put(&host, "turn-3", "w1", json!({ "threadId": "t2", "claudeSessionId": "k" }), Some(road(&other)));
        let ask = |id: &str| AnswerAsk { ask_id: "ask_1".into(), option_id: id.into(), reason: None };

        let got = area.answer_row("agent-1", ask("allow"), None).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "outcome": "answered" }));
        assert_eq!((old.borrow().len(), new.borrow().clone()), (0, vec!["answer(ask_1, allow, None)".to_string()]), "the running row that carries the id, not the ended row keyed by it");
        let got = area.answer_row("k", ask("allow"), None).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "outcome": "answered" }));
        assert_eq!((keyed.borrow().len(), other.borrow().len()), (1, 0));
    });
}

// C4: permission-relay.test.ts:411
#[test]
fn answer_falls_back_to_the_keyed_row_and_hands_back_the_outcome_whole() {
    run_local(async {
        let (host, area) = host();
        let log = Rc::new(RefCell::new(Vec::new()));
        // Nothing runs under the id, so the keyed row, ended as it is, takes the answer.
        put(&host, "turn-1", "w1", json!({ "threadId": "t1", "status": "completed" }), Some(StubHandle { answer: Some(SessionAnswerOutcome::NoOption), ..StubHandle::new(&log) }));
        let ask = AnswerAsk { ask_id: "ask_1".into(), option_id: "mode:bypassPermissions".into(), reason: Some("Run only the thread-rows file".into()) };
        let got = area.answer_row("turn-1", ask, None).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "outcome": "no-option" }));
        assert_eq!(*log.borrow(), vec!["answer(ask_1, mode:bypassPermissions, Some(\"Run only the thread-rows file\"))".to_string()]);
        let none = area.answer_row("s_nope", AnswerAsk { ask_id: "a".into(), option_id: "o".into(), reason: None }, None).await.expect("an answer");
        assert_eq!(json_of(&none), json!({ "outcome": "not-found" }));
    });
}

// C5: permission-relay.test.ts:216
#[test]
fn answer_without_a_handle_is_gone() {
    run_local(async {
        let (host, area) = host();
        let log = Rc::new(RefCell::new(Vec::new()));
        put(&host, "no-process", "w1", json!({ "threadId": "t1" }), None);
        put(&host, "no-road", "w1", json!({ "threadId": "t2" }), Some(StubHandle::new(&log)));
        let ask = || AnswerAsk { ask_id: "ask_1".into(), option_id: "allow".into(), reason: None };
        assert_eq!(json_of(&area.answer_row("no-process", ask(), None).await.expect("an answer")), json!({ "outcome": "gone" }));
        assert_eq!(json_of(&area.answer_row("no-road", ask(), None).await.expect("an answer")), json!({ "outcome": "unsupported" }));
        assert_eq!(*log.borrow(), Vec::<String>::new());
    });
}

// C0, C1: permission-relay.test.ts:380, :423
#[test]
fn answer_of_another_trees_row_is_not_found_and_a_paused_workspace_refuses() {
    run_local(async {
        let (host, area) = host();
        let log = Rc::new(RefCell::new(Vec::new()));
        put(&host, "sess-1", "w1", json!({ "threadId": "t1" }), Some(StubHandle { answer: Some(SessionAnswerOutcome::Answered), ..StubHandle::new(&log) }));
        let ask = || AnswerAsk { ask_id: "ask_1".into(), option_id: "allow".into(), reason: None };
        host.add_entry(entry("w1", json!({ "phase": "napping" })));
        let err = area.answer_row("sess-1", ask(), None).await.expect_err("a refusal");
        assert_eq!(said(&err), (send_line(&host, true), None, None));
        host.add_entry(entry("w1", json!({})));
        let seen = Reached::default();
        unreached(&host, seen.clone());
        let got = area.answer_row("sess-1", ask(), Some(scoped("t9"))).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "outcome": "not-found" }));
        assert_eq!(*seen.borrow(), vec![(Some("t1".to_string()), "w1".to_string(), None, false)]);
        assert_eq!(*log.borrow(), Vec::<String>::new(), "the harness hears nothing");
    });
}

fn thread_with_record(host: &FakeHost, thread: &str, doc: Value) {
    host.add_record(thread, "w1", thread_record(doc));
}

// C6, C8: thread-keeps-agent.test.ts:133
#[test]
fn access_on_a_trimmed_thread_moves_its_record() {
    run_local(async {
        let (host, area) = host();
        thread_with_record(&host, "t9", json!({ "harness": "codex", "permissionMode": "read-only", "rootThreadId": "t9" }));
        let asked = adapters(&host, StubAdapter::default());
        let seen: Rc<RefCell<Vec<(Option<String>, String)>>> = Rc::default();
        host.hooks.set::<EntryOfRow>(
            "SessionsCtx::entry_of_row",
            Rc::new({
                let seen: Rc<RefCell<Vec<(Option<String>, String)>>> = Rc::clone(&seen);
                move |at: &RowAt, _o: Option<&Caller>, _k: Option<TreeTalk>, _b: bool| {
                    seen.borrow_mut().push((at.thread_id.clone(), at.workspace_id.clone()));
                    async move { Ok(Some(entry("w1", json!({})) as Entry)) }.boxed_local()
                }
            }),
        );

        let got = area.access_row("t9", "danger-full-access", None).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "outcome": "set" }));
        assert_eq!(*seen.borrow(), vec![(Some("t9".to_string()), "w1".to_string())], "reached as the thread of its record");
        assert_eq!(*asked.borrow(), vec!["codex".to_string()], "the record's agent");
        assert_eq!(
            record_of(&host, "t9"),
            Some(ThreadRecord { workspace_id: "w1".into(), ..thread_record(json!({ "harness": "codex", "permissionMode": "danger-full-access", "rootThreadId": "t9" })) })
        );
        assert_eq!(calls(&host, &["persist_sessions", "push_head"]), vec!["persist_sessions(w1)".to_string(), "push_head(t9)".to_string()]);

        let none = area.access_row("thr_nobody", "danger-full-access", None).await.expect("an answer");
        assert_eq!(json_of(&none), json!({ "outcome": "not-found" }));
    });
}

// C6, C9: permission-relay.test.ts:646, :664
#[test]
fn access_while_running_lands_on_the_record_and_the_row_follows() {
    run_local(async {
        let (host, area) = host();
        thread_with_record(&host, "t1", json!({ "harness": "claude", "permissionMode": "bypassPermissions" }));
        let log = Rc::new(RefCell::new(Vec::new()));
        put(&host, "turn-1", "w1", json!({ "threadId": "t1", "permissionMode": "bypassPermissions" }), Some(StubHandle { access: Some(AccessMove::Set), ..StubHandle::new(&log) }));
        adapters(&host, StubAdapter::default());

        let got = area.access_row("turn-1", "acceptEdits", None).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "outcome": "set" }));
        assert_eq!(*log.borrow(), vec!["set_access(acceptEdits)".to_string()]);
        assert_eq!(
            record_of(&host, "t1"),
            Some(ThreadRecord { workspace_id: "w1".into(), ..thread_record(json!({ "harness": "claude", "permissionMode": "acceptEdits" })) })
        );
        // The turn moved by the harness writes its own row; this road leaves a running row to the turn.
        assert_eq!(host.rows.borrow()["turn-1"].borrow().view.permission_mode.as_deref(), Some("bypassPermissions"));
        assert_eq!(calls(&host, &["persist_sessions", "push_head"]), vec!["persist_sessions(w1)".to_string(), "push_head(t1)".to_string()]);
    });
}

// C9: permission-relay.test.ts:664, :679, :691
#[test]
fn access_a_cli_refuses_is_unsupported() {
    run_local(async {
        for (road, name, want, handle_calls) in [
            (Some(AccessMove::Refused), "a CLI that refuses the mode", "unsupported", 1),
            (None, "a harness with no access road", "unsupported", 0),
            (Some(AccessMove::Gone), "a handle that says gone", "not-running", 1),
        ] {
            let (host, area) = host();
            thread_with_record(&host, "t1", json!({ "harness": "claude", "permissionMode": "bypassPermissions" }));
            let log = Rc::new(RefCell::new(Vec::new()));
            put(&host, "turn-1", "w1", json!({ "threadId": "t1", "permissionMode": "bypassPermissions" }), Some(StubHandle { access: road, ..StubHandle::new(&log) }));
            adapters(&host, StubAdapter::default());
            let got = area.access_row("turn-1", "dontAsk", None).await.expect("an answer");
            assert_eq!(json_of(&got), json!({ "outcome": want }), "{name}");
            assert_eq!(log.borrow().len(), handle_calls, "{name}");
            // The pick stands for the thread's next turn all the same.
            assert_eq!(record_of(&host, "t1").map(|h| h.permission_mode), Some(Some("dontAsk".to_string())), "{name}");
            assert_eq!(host.rows.borrow()["turn-1"].borrow().view.permission_mode.as_deref(), Some("bypassPermissions"), "{name}: the running turn keeps its mode");
        }
    });
}

// C9: permission-relay.test.ts:664
#[test]
fn access_on_a_turn_this_host_holds_no_process_for_is_not_running() {
    run_local(async {
        let (host, area) = host();
        let launching = row(view("turn-1", "w1", json!({ "threadId": "t1" })), "turn-1");
        launching.borrow_mut().launch = Some(done());
        host.add_row("turn-1", launching);
        adapters(&host, StubAdapter::default());
        let got = area.access_row("turn-1", "dontAsk", None).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "outcome": "not-running" }));
        assert_eq!(record_of(&host, "t1").map(|h| (h.workspace_id, h.harness, h.permission_mode)), Some(("w1".to_string(), "claude".to_string(), Some("dontAsk".to_string()))), "a thread with no record gets one off its row");
    });
}

// C6, C9: permission-relay.test.ts:747
#[test]
fn access_between_turns_sets_the_threads_latest_row() {
    run_local(async {
        let (host, area) = host();
        thread_with_record(&host, "t1", json!({ "harness": "claude", "permissionMode": "bypassPermissions" }));
        // Map order and start order differ: the row asked for by thread id is the last in the map, and the row
        // the thread's latest turn left is the one started last.
        put(&host, "r-a", "w1", json!({ "threadId": "t1", "status": "completed", "startedAt": T0 + 100, "permissionMode": "bypassPermissions" }), None);
        put(&host, "r-b", "w1", json!({ "threadId": "t1", "status": "completed", "startedAt": T0, "harness": "codex", "permissionMode": "bypassPermissions" }), None);
        let asked = adapters(&host, StubAdapter::default());

        let got = area.access_row("t1", "dontAsk", None).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "outcome": "set" }));
        assert_eq!(*asked.borrow(), vec!["codex".to_string()], "the last row of the thread in map order names the agent");
        assert_eq!(host.rows.borrow()["r-a"].borrow().view.permission_mode.as_deref(), Some("dontAsk"), "the latest row says the new mode");
        assert_eq!(host.rows.borrow()["r-b"].borrow().view.permission_mode.as_deref(), Some("bypassPermissions"));
        assert_eq!(record_of(&host, "t1").map(|h| h.permission_mode), Some(Some("dontAsk".to_string())));
        assert_eq!(calls(&host, &["persist_sessions", "push_head"]), vec!["persist_sessions(w1)".to_string(), "push_head(t1)".to_string()]);
    });
}

// C9
#[test]
fn a_row_with_no_thread_takes_the_mode_on_itself() {
    run_local(async {
        let (host, area) = host();
        put(&host, "solo", "w1", json!({ "status": "completed", "permissionMode": "default" }), None);
        adapters(&host, StubAdapter::default());
        let got = area.access_row("solo", "dontAsk", None).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "outcome": "set" }));
        assert_eq!(host.rows.borrow()["solo"].borrow().view.permission_mode.as_deref(), Some("dontAsk"));
        assert_eq!(host.records.borrow().len(), 0, "no thread, no record");
        assert_eq!(calls(&host, &["persist_sessions", "push_head"]), vec!["persist_sessions(w1)".to_string(), "push_head(solo)".to_string()]);
    });
}

fn claude_catalog() -> HarnessCatalog {
    serde_json::from_value(json!({
        "harness": "claude", "label": "Claude", "source": "table", "version": null, "models": [], "efforts": [], "contextWindows": [],
        "permissionModes": [{ "value": "default", "label": "Default" }, { "value": "acceptEdits", "label": "Accept edits" }],
        "steers": true, "renames": false, "images": false,
    }))
    .expect("a catalog the fixture builds")
}

// C7: permission-relay.test.ts:738, thread-access.test.ts:96
#[test]
fn access_refuses_a_mode_the_catalog_does_not_carry() {
    run_local(async {
        let (host, area) = host();
        thread_with_record(&host, "t1", json!({ "harness": "claude", "permissionMode": "default" }));
        let log = Rc::new(RefCell::new(Vec::new()));
        put(&host, "turn-1", "w1", json!({ "threadId": "t1" }), Some(StubHandle { access: Some(AccessMove::Set), ..StubHandle::new(&log) }));
        adapters(&host, StubAdapter::default());
        host.hooks.set::<CatalogOn>("LaunchCtx::catalog_on", Rc::new(|_h: &str, _e: &Entry, _a: &dyn HarnessAdapter| async move { Ok(Some(claude_catalog())) }.boxed_local()));

        let err = area.access_row("turn-1", "sideways", None).await.expect_err("a refusal");
        let want = start_picks(Some(&claude_catalog()), &StartPicks { permission_mode: Some("sideways".into()), ..StartPicks::default() }, false, None).expect_err("the catalog refuses");
        assert_eq!(want.kind, "usage");
        assert_eq!(said(&err), (want.message, None, Some(Kind::Usage)), "in the words a start refuses it with");
        assert_eq!(*log.borrow(), Vec::<String>::new());
        assert_eq!(record_of(&host, "t1").map(|h| h.permission_mode), Some(Some("default".to_string())), "nothing moved");
        assert_eq!(calls(&host, &["persist_sessions", "push_head"]), Vec::<String>::new());
        assert!(calls(&host, &["catalog_on"]).contains(&"catalog_on(claude)".to_string()));

        let ok = area.access_row("turn-1", "acceptEdits", None).await.expect("a mode the catalog carries");
        assert_eq!(json_of(&ok), json!({ "outcome": "set" }));
    });
}

// C0, C6: permission-relay.test.ts:711, :747
#[test]
fn access_of_a_row_not_reached_or_a_workspace_that_will_not_send() {
    run_local(async {
        let (host, area) = host();
        thread_with_record(&host, "t1", json!({ "harness": "claude", "permissionMode": "default" }));
        put(&host, "turn-1", "w1", json!({ "threadId": "t1", "status": "completed", "permissionMode": "default" }), None);
        adapters(&host, StubAdapter::default());
        let none = area.access_row("s_nope", "dontAsk", None).await.expect("an answer");
        assert_eq!(json_of(&none), json!({ "outcome": "not-found" }));

        host.add_entry(entry("w1", json!({ "phase": "pausing" })));
        let err = area.access_row("turn-1", "dontAsk", None).await.expect_err("a refusal");
        assert_eq!(said(&err), (send_line(&host, true), None, None));
        host.add_entry(entry("w1", json!({})));

        let seen = Reached::default();
        unreached(&host, seen.clone());
        let got = area.access_row("turn-1", "dontAsk", Some(scoped("t9"))).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "outcome": "not-found" }));
        assert_eq!(host.rows.borrow()["turn-1"].borrow().view.permission_mode.as_deref(), Some("default"), "moves nothing");
        assert_eq!(record_of(&host, "t1").map(|h| h.permission_mode), Some(Some("default".to_string())));
        assert_eq!(calls(&host, &["persist_sessions", "push_head"]), Vec::<String>::new());
    });
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
    put(&host, "turn-3", "w1", json!({ "threadId": "t2", "status": "completed", "claudeSessionId": "OTHER", "harnessTitle": "keep" }), None);
    put(&host, "turn-4", "w2", json!({ "threadId": "t3", "status": "completed", "claudeSessionId": "SESS", "harnessTitle": "elsewhere" }), None);
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

// C10, C11, C12: session-title.test.ts:689, :801
#[test]
fn rename_names_every_row_of_the_session() {
    run_local(async {
        let (host, area) = renaming_host();
        let ms: Rc<RefCell<Vec<u64>>> = Rc::default();
        let seen = ms.clone();
        host.hooks.set::<dyn Fn(&Entry, u64) -> Arc<dyn HarnessExec>>(
            "TurnsCtx::harness_exec",
            Rc::new(move |_e: &Entry, ms: u64| {
                seen.borrow_mut().push(ms);
                Arc::new(NoExec) as Arc<dyn HarnessExec>
            }),
        );
        let wrote = writes(&host, SessionRenameWrite::Written);

        let got = area.rename_row("turn-2", "  the name he typed in wsp  ", None).await.expect("a rename");
        assert_eq!(json_of(&got), json!({ "outcome": "renamed" }));
        assert_eq!(*wrote.borrow(), vec![("SESS".to_string(), "the name he typed in wsp".to_string())], "the harness's own session id, the trimmed name");
        let named = Some("the name he typed in wsp".to_string());
        assert_eq!(
            titles(&host),
            vec![
                (named.clone(), Some("person".to_string())),
                (named, Some("person".to_string())),
                (Some("keep".to_string()), None),
                (Some("elsewhere".to_string()), None),
            ],
            "every row of that workspace and session; no other session, no other workspace"
        );
        assert_eq!(
            calls(&host, &["write_session", "persist_sessions", "push_head"]),
            vec!["write_session(SESS, the name he typed in wsp)".to_string(), "persist_sessions(w1)".to_string(), "push_head(t1)".to_string()]
        );
        assert!(ms.borrow().iter().all(|m| *m == wsp_core::types::events::SESSION_TITLE_TIMEOUT_MS), "the exec the write runs on is bounded by the title timeout");
    });
}

// C10, C11: session-title.test.ts:726, :737, :748, :760
#[test]
fn rename_with_no_session_is_no_session() {
    run_local(async {
        // The store answers no-session, or fails with its own line: the rows keep the title they had.
        for (outcome, want) in [
            (SessionRenameWrite::NoSession, json!({ "outcome": "no-session" })),
            (SessionRenameWrite::Failed { error: "database is locked".into() }, json!({ "outcome": "failed", "error": "database is locked" })),
        ] {
            let (host, area) = renaming_host();
            let wrote = writes(&host, outcome);
            let got = area.rename_row("turn-2", "the name", None).await.expect("an answer");
            assert_eq!(json_of(&got), want);
            assert_eq!(wrote.borrow().len(), 1);
            assert_eq!(titles(&host)[1], (Some("old".to_string()), None));
            assert_eq!(calls(&host, &["persist_sessions", "push_head"]), Vec::<String>::new());
        }

        // A thread whose harness never named a session has nothing to name, and the machine is not asked.
        let (host, area) = renaming_host();
        put(&host, "bare", "w1", json!({ "threadId": "t5", "status": "completed" }), None);
        let wrote = writes(&host, SessionRenameWrite::Written);
        let got = area.rename_row("bare", "the name", None).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "outcome": "no-session" }));
        assert_eq!(*wrote.borrow(), Vec::<(String, String)>::new());

        // No road to write a name is unsupported, and it is said before the missing session is.
        adapters(&host, StubAdapter::default());
        let got = area.rename_row("bare", "the name", None).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "outcome": "unsupported" }));
        assert_eq!(*wrote.borrow(), Vec::<(String, String)>::new());
    });
}

// C10: session-title.test.ts:773, :780, :790
#[test]
fn a_blank_rename_is_refused() {
    run_local(async {
        let (host, area) = renaming_host();
        let wrote = writes(&host, SessionRenameWrite::Written);
        // The blank name is refused before the row is looked up.
        let err = area.rename_row("no-such-session", "   ", None).await.expect_err("a refusal");
        assert_eq!(said(&err), (EMPTY_TITLE_LINE.to_string(), None, None));
        assert_eq!(calls(&host, &["entry_of_row", "launch_adapter_for"]), Vec::<String>::new());

        let none = area.rename_row("no-such-session", "the name", None).await.expect("an answer");
        assert_eq!(json_of(&none), json!({ "outcome": "not-found" }));

        let seen = Reached::default();
        unreached(&host, seen.clone());
        let none = area.rename_row("turn-1", "the name", Some(scoped("t9"))).await.expect("an answer");
        assert_eq!(json_of(&none), json!({ "outcome": "not-found" }));
        host.hooks.set::<EntryOfRow>(
            "SessionsCtx::entry_of_row",
            Rc::new(|_at: &RowAt, _o: Option<&Caller>, _k: Option<TreeTalk>, _b: bool| async move { Ok(Some(entry("w1", json!({ "phase": "napping" })) as Entry)) }.boxed_local()),
        );
        let err = area.rename_row("turn-1", "the name", None).await.expect_err("a refusal");
        let rec = record_in(&entry("w1", json!({ "phase": "napping" })));
        let state = workspace_state(&WorkspaceStateInput { phase: rec.phase.clone(), machine_state: None, reach: None, wake_refused: None });
        assert_eq!(said(&err), (action_refusal(&state, "rename", rec.gone.as_deref(), Some(&rec.name)).expect("a refusal"), None, None));
        assert_eq!(calls(&host, &["copy_blocked"]), Vec::<String>::new(), "the workspace's state is asked before the copy");
        assert_eq!(*wrote.borrow(), Vec::<(String, String)>::new(), "the machine is asked nothing");
    });
}

// C10: the copy check comes first and its refusal is the caller's
#[test]
fn rename_stops_at_a_copy_refusal() {
    run_local(async {
        let (host, area) = renaming_host();
        let wrote = writes(&host, SessionRenameWrite::Written);
        host.hooks.set::<dyn Fn(&Entry) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "SessionsCtx::copy_blocked",
            Rc::new(|_e: &Entry| async move { Err(refusal("the copy is still landing", Some(Kind::Conflict))) }.boxed_local()),
        );
        let err = area.rename_row("turn-2", "the name", None).await.expect_err("a refusal");
        assert_eq!(said(&err), ("the copy is still landing".to_string(), None, Some(Kind::Conflict)));
        assert_eq!(calls(&host, &["launch_adapter_for"]), Vec::<String>::new());
        assert_eq!(*wrote.borrow(), Vec::<(String, String)>::new());
    });
}

// C13: thread-marks.test.ts:42
#[test]
fn read_marks_the_thread_read_now() {
    run_local(async {
        let (host, area) = host();
        let marked: Rc<RefCell<Vec<(Vec<String>, Value, Option<Caller>)>>> = Rc::default();
        let seen = marked.clone();
        host.hooks.set::<dyn Fn(&[String], StampsOf, Option<&Caller>) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "SessionsCtx::mark",
            Rc::new(move |ids: &[String], stamps: StampsOf, o: Option<&Caller>| {
                let stamped = ids.iter().map(|id| Value::Object(stamps(id))).collect::<Vec<_>>();
                seen.borrow_mut().push((ids.to_vec(), Value::Array(stamped), o.cloned()));
                async move { Ok(()) }.boxed_local()
            }),
        );
        host.clock.advance(1_234);
        area.read_thread("t1", Some(scoped("t1"))).await.expect("a read");
        let marked = marked.borrow();
        assert_eq!(marked.len(), 1);
        assert_eq!(marked[0].0, vec!["t1".to_string()]);
        assert_eq!(marked[0].2, Some(scoped("t1")), "the caller is handed on");
        let stamps = marked[0].1.as_array().expect("one stamp set per thread");
        assert_eq!(stamps.len(), 1);
        let keys: Vec<&String> = stamps[0].as_object().expect("stamps").keys().collect();
        assert_eq!(keys, vec!["readAt"], "the read stamp alone");
        assert_eq!(stamps[0]["readAt"].as_f64(), Some((T0 + 1_234) as f64), "now on the host's clock");
    });
}

fn aside_host() -> (Rc<FakeHost>, SessionsArea) {
    let (host, area) = host();
    put(&host, "turn-a", "w1", json!({ "threadId": "t1", "status": "completed", "claudeSessionId": "OLD", "cwd": "/root/a", "model": "claude-opus-5", "startedAt": T0 }), None);
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

// C14, C15: aside.test.ts:117, :272, :286
#[test]
fn aside_asks_the_latest_session_with_what_it_ran_at() {
    run_local(async {
        let (host, area) = aside_host();
        let asked_for = adapters(&host, with_aside());
        let vault = {
            let mut server = IndexMap::new();
            server.insert("url".to_string(), json!("http://x"));
            let mut entries = IndexMap::new();
            entries.insert("srv".to_string(), server);
            wsp_adapters::port::ServerValues { entries: Some(entries), config: None }
        };
        let given = vault.clone();
        host.hooks.set::<dyn Fn() -> wsp_adapters::port::ServerValues>("LaunchCtx::vault_server_values", Rc::new(move || given.clone()));
        let vault_seen: Rc<RefCell<Vec<Option<wsp_adapters::port::ServerValues>>>> = Rc::default();
        let log = vault_seen.clone();
        host.hooks.set::<LaunchAdapterFor>(
            "SessionsCtx::launch_adapter_for",
            Rc::new(move |_e: &Entry, h: &str, v: Option<wsp_adapters::port::ServerValues>| {
                log.borrow_mut().push(v);
                let built = BuiltAdapter { harness: h.to_string(), adapter: Arc::new(with_aside()) };
                async move { Ok(built) }.boxed_local()
            }),
        );
        let asked = aside_answers(&host, ANSWER);
        let history = host.recorded_json();

        let got = area.aside_row("turn-a", "what did I last ask?", None, None).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "text": ANSWER }));
        assert_eq!(*vault_seen.borrow(), vec![Some(vault)], "the vault's servers go with the adapter");
        assert!(asked_for.borrow().is_empty(), "the bare adapter is the only one built");
        {
            let asked = asked.borrow();
            assert_eq!(asked.len(), 1);
            let q = &asked[0];
            // The row named is the thread's first; the question goes at the latest row's session, folder, model and effort.
            assert_eq!(
                (q.session.as_str(), q.question.as_str(), q.cut.as_deref(), q.cwd.as_deref(), q.model.as_deref(), q.effort.as_deref()),
                ("SESS", "what did I last ask?", None, Some("/root/b"), Some("claude-sonnet-5"), Some("high"))
            );
            assert_eq!((q.context_window.clone(), q.fast), (None, None));
            assert!(q.on_text.is_none() && q.mcp_servers.is_none() && q.server_values.is_none());
        }
        assert_eq!(host.recorded_json(), history, "nothing of it is kept");
        assert_eq!((host.passed.borrow().len(), host.emitted.borrow().len()), (0, 0));

        // The window and fast ride along, fast only where true.
        {
            let rows = host.rows.borrow();
            let mut latest = rows["turn-b"].borrow_mut();
            latest.view.context_window = Some("1m".into());
            latest.view.fast = Some(true);
        }
        area.aside_row("turn-a", "again?", None, None).await.expect("an answer");
        host.rows.borrow()["turn-b"].borrow_mut().view.fast = Some(false);
        area.aside_row("turn-a", "and again?", None, None).await.expect("an answer");
        let asked = asked.borrow();
        assert_eq!((asked[1].context_window.as_deref(), asked[1].fast), (Some("1m"), Some(true)));
        assert_eq!((asked[2].context_window.as_deref(), asked[2].fast), (Some("1m"), None));
    });
}

// C15
#[test]
fn aside_carries_the_cut_a_rewind_left_owing() {
    run_local(async {
        let (host, area) = aside_host();
        adapters(&host, with_aside());
        thread_with_record(&host, "t1", json!({ "harness": "claude", "cutOwed": "turn-x" }));
        let asked = aside_answers(&host, ANSWER);
        area.aside_row("turn-a", "what is this?", None, None).await.expect("an answer");
        assert_eq!(asked.borrow()[0].cut.as_deref(), Some("turn-x"));
    });
}

// C14: aside.test.ts:236
#[test]
fn a_blank_aside_and_an_unreached_row_are_refused() {
    run_local(async {
        let (host, area) = aside_host();
        adapters(&host, with_aside());
        let asked = aside_answers(&host, ANSWER);
        let err = area.aside_row("turn-a", "   ", None, None).await.expect_err("a refusal");
        assert_eq!(said(&err), (BLANK_ASIDE_LINE.to_string(), None, None));
        assert_eq!(calls(&host, &["entry_of_row"]), Vec::<String>::new());

        let err = area.aside_row("sess_nothing", "hi", None, None).await.expect_err("a refusal");
        assert_eq!(said(&err), said(&not_found_refusal("no session sess_nothing")));

        let seen = Reached::default();
        unreached(&host, seen.clone());
        let err = area.aside_row("turn-a", "hi", Some(scoped("t9")), None).await.expect_err("a refusal");
        assert_eq!(said(&err), said(&not_found_refusal("no session turn-a")));

        host.hooks.set::<EntryOfRow>(
            "SessionsCtx::entry_of_row",
            Rc::new(|_at: &RowAt, _o: Option<&Caller>, _k: Option<TreeTalk>, _b: bool| async move { Ok(Some(entry("w1", json!({ "phase": "pausing" })) as Entry)) }.boxed_local()),
        );
        let err = area.aside_row("turn-a", "hi", None, None).await.expect_err("a refusal");
        assert_eq!(said(&err), (send_line_for("pausing"), None, None), "the send refusal, with no name in it");
        assert!(asked.borrow().is_empty());
    });
}

fn send_line_for(phase: &str) -> String {
    let rec = record_in(&entry("w1", json!({ "phase": phase })));
    let state = workspace_state(&WorkspaceStateInput { phase: rec.phase.clone(), machine_state: None, reach: None, wake_refused: None });
    send_refusal(&SendRefusalKind::State(state), rec.gone.as_deref(), None).expect("a refusal")
}

// C15: aside.test.ts:236
#[test]
fn an_aside_with_no_road_or_no_session_is_refused() {
    run_local(async {
        let (host, area) = aside_host();
        adapters(&host, StubAdapter::default());
        let asked = aside_answers(&host, ANSWER);
        let err = area.aside_row("turn-a", "anyone there?", None, None).await.expect_err("a refusal");
        assert_eq!(said(&err), (aside_unsupported_line("claude"), None, None));

        adapters(&host, with_aside());
        host.rows.borrow()["turn-b"].borrow_mut().view.claude_session_id = None;
        let err = area.aside_row("turn-a", "anyone there?", None, None).await.expect_err("a refusal");
        assert_eq!(said(&err), (ASIDE_NO_SESSION_LINE.to_string(), None, None), "the latest row names no session, whatever the row asked for holds");

        // No road and no session: the road is said first.
        adapters(&host, StubAdapter::default());
        let err = area.aside_row("turn-a", "anyone there?", None, None).await.expect_err("a refusal");
        assert_eq!(said(&err), (aside_unsupported_line("claude"), None, None));
        assert!(asked.borrow().is_empty(), "nothing is asked");
    });
}

// C15: aside.test.ts:131
#[test]
fn aside_streams_text_only_with_an_ask_id() {
    run_local(async {
        let (host, area) = aside_host();
        adapters(&host, with_aside());
        let asked = aside_answers(&host, ANSWER);
        let got = area.aside_row("turn-a", "what did I last ask?", None, Some("ask-1".into())).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "text": ANSWER }));
        let piece = |text: &str| json!({ "type": "aside.text", "workspaceId": "w1", "threadId": "t1", "askId": "ask-1", "text": text });
        assert_eq!(*host.passed.borrow(), vec![piece("You are in "), piece("/root/b")], "each piece passed on the bus under the question's id");
        assert_eq!(host.recorded_json(), Vec::<Value>::new(), "none of them kept for a replay");
        assert!(asked.borrow()[0].on_text.is_some());

        // Asked with no id, the pieces go nowhere.
        host.passed.borrow_mut().clear();
        area.aside_row("turn-a", "again?", None, None).await.expect("an answer");
        assert!(asked.borrow()[1].on_text.is_none());
        assert_eq!(*host.passed.borrow(), Vec::<Value>::new());
    });
}

// C16: aside.test.ts:313
#[test]
fn an_empty_aside_answer_is_refused() {
    run_local(async {
        let (host, area) = aside_host();
        adapters(&host, with_aside());
        aside_answers(&host, " \n");
        let err = area.aside_row("turn-a", "what did I last ask?", None, None).await.expect_err("a refusal");
        assert_eq!(said(&err), (ASIDE_EMPTY_LINE.to_string(), None, None));
    });
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
            async move { Ok(ThreadLaunch { scoped_device_id: Some("dev-1".into()), env, wsp: Some(wsp_spec(&["/opt/wsp/dist/bin.js", "mcp", "--scoped"])) }) }.boxed_local()
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

// C17: aside.test.ts:152
#[test]
fn an_aside_with_servers_revokes_its_token_after() {
    run_local(async {
        let (host, area, s) = servers_host(full_servers(), full_servers(), false, "t1");
        let got = area.aside_row("turn-a", "what did I last ask?", None, None).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "text": ANSWER }));
        assert_eq!(
            calls(&host, &ORDER),
            vec![
                "launch_adapter_for(claude)".to_string(),
                "thread_launch(t1, t1, true)".to_string(),
                "adapter_for(claude)".to_string(),
                "server_values_for(claude, /root/b)".to_string(),
                "aside".to_string(),
                "revoke_device(dev-1)".to_string(),
            ],
            "a launch is minted for the thread, its device revoked once the answer is in"
        );
        assert_eq!(*s.revoked.borrow(), vec!["dev-1".to_string()]);
        let asks = s.asks.borrow();
        assert_eq!(asks.len(), 1);
        assert_eq!(asks[0].turn_env.as_ref().and_then(|e| e.get("WSP_TOKEN")).map(String::as_str), Some("tok"), "the copy is built with the launch's pair");
        let asked = s.asked.borrow();
        assert_eq!(json_of(&asked[0].mcp_servers), json!({ "wsp": { "command": "node", "args": ["/opt/wsp/dist/bin.js", "mcp", "--scoped"] } }), "the wsp server a turn of the thread gets");
        assert_eq!(asked[0].server_values, Some(wsp_adapters::port::ServerValues { entries: None, config: None }), "server values read in the folder the thread ran in");
        assert_eq!(*s.values_for.borrow(), vec![("claude".to_string(), "/root/b".to_string())]);
    });
}

// C17: aside.test.ts:295
#[test]
fn a_thread_another_thread_started_is_served_with_no_slate() {
    run_local(async {
        let (_host, area, s) = servers_host(full_servers(), full_servers(), false, "t0");
        area.aside_row("turn-a", "how is it going?", None, None).await.expect("an answer");
        let asked = s.asked.borrow();
        assert_eq!(
            json_of(&asked[0].mcp_servers),
            json!({ "wsp": { "command": "node", "args": ["/opt/wsp/dist/bin.js", "mcp", "--scoped", "--no-slate"], "noSlate": true } })
        );
    });
}

// C17: aside.test.ts:152
#[test]
fn a_token_that_will_not_go_is_warned_and_the_answer_stands() {
    run_local(async {
        let (host, area, _s) = servers_host(full_servers(), full_servers(), false, "t1");
        host.hooks.set::<dyn Fn(&str) -> LocalBoxFuture<'static, Result<(), HostError>>>(
            "LaunchCtx::revoke_device",
            Rc::new(|_d: &str| async move { Err(refusal("the door is shut", None)) }.boxed_local()),
        );
        let warned = wsp_core::console::capture();
        let got = area.aside_row("turn-a", "what did I last ask?", None, None).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "text": ANSWER }));
        assert_eq!(warned.lines(), vec![format!("the token of a side question on thread {} was not taken away: the door is shut", thread_word("t1"))]);
    });
}

// C17
#[test]
fn an_aside_that_fails_still_takes_the_token_back() {
    run_local(async {
        let (host, area, s) = servers_host(full_servers(), full_servers(), false, "t1");
        host.hooks.set::<AsideHook>("SessionsCtx::aside", Rc::new(|_a: &dyn HarnessAdapter, _q: AsideQuestion| async move { Err(refusal("the harness fell over", None)) }.boxed_local()));
        let err = area.aside_row("turn-a", "what did I last ask?", None, None).await.expect_err("the failure");
        assert_eq!(said(&err), ("the harness fell over".to_string(), None, None));
        assert_eq!(*s.revoked.borrow(), vec!["dev-1".to_string()]);
    });
}

// C17: aside.test.ts:180
#[test]
fn a_harness_that_loads_no_servers_is_asked_bare() {
    run_local(async {
        let (host, area, s) = servers_host(with_aside(), full_servers(), false, "t1");
        let got = area.aside_row("turn-a", "what did I last ask?", None, None).await.expect("an answer");
        assert_eq!(json_of(&got), json!({ "text": ANSWER }));
        assert_eq!(calls(&host, &ORDER), vec!["launch_adapter_for(claude)".to_string(), "aside".to_string()], "no launch, no token");
        let asked = s.asked.borrow();
        assert!(asked[0].mcp_servers.is_none() && asked[0].server_values.is_none());
        assert!(s.revoked.borrow().is_empty());
    });
}

// C17
#[test]
fn a_brokered_workspace_is_asked_through_a_launch_even_for_a_harness_with_no_servers() {
    run_local(async {
        let (host, area, s) = servers_host(with_aside(), with_aside(), true, "t1");
        area.aside_row("turn-a", "what did I last ask?", None, None).await.expect("an answer");
        assert_eq!(
            calls(&host, &ORDER),
            vec![
                "launch_adapter_for(claude)".to_string(),
                "thread_launch(t1, t1, true)".to_string(),
                "adapter_for(claude)".to_string(),
                "aside".to_string(),
                "revoke_device(dev-1)".to_string(),
            ],
            "a harness that loads no servers reads no server values"
        );
        let asked = s.asked.borrow();
        assert!(asked[0].mcp_servers.is_none() && asked[0].server_values.is_none());
    });
}

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

// C18: session-run.test.ts:75
#[test]
fn run_is_the_persons_alone() {
    run_local(async {
        let (host, area) = run_host();
        let err = area.run_step(step(json!({})), Some(scoped("t1"))).await.expect_err("a refusal");
        assert_eq!(said(&err), (wsp_wire::views::session_events::RUN_PERSONS_LINE.to_string(), None, None));
        assert_eq!(calls(&host, &["ready"]), vec!["ready".to_string()], "ready first, then the scope");
        assert_eq!(calls(&host, &["entry_of_row", "open_thread"]), Vec::<String>::new());
        assert_eq!(host.recorded_json(), Vec::<Value>::new());

        // A thread with no row and no record, and one the caller does not reach, are no thread.
        let err = area.run_step(step(json!({ "threadId": "nope" })), None).await.expect_err("a refusal");
        assert_eq!(said(&err), said(&not_found_refusal(&format!("no thread {}", thread_word("nope")))));
        let seen = Reached::default();
        unreached(&host, seen.clone());
        let err = area.run_step(step(json!({})), None).await.expect_err("a refusal");
        assert_eq!(said(&err), said(&not_found_refusal(&format!("no thread {}", thread_word("t1")))));
        assert_eq!(*seen.borrow(), vec![(Some("t1".to_string()), "w1".to_string(), None, false)]);
        assert_eq!(host.recorded_json(), Vec::<Value>::new());
    });
}

// C18: the workspace comes off the thread's record where no row holds it
#[test]
fn a_run_on_a_thread_known_by_its_record_alone_is_recorded() {
    run_local(async {
        let (host, area) = host();
        thread_with_record(&host, "t7", json!({ "harness": "claude" }));
        let seen: Rc<RefCell<Vec<(Option<String>, String)>>> = Rc::default();
        host.hooks.set::<EntryOfRow>(
            "SessionsCtx::entry_of_row",
            Rc::new({
                let seen: Rc<RefCell<Vec<(Option<String>, String)>>> = Rc::clone(&seen);
                move |at: &RowAt, _o: Option<&Caller>, _k: Option<TreeTalk>, _b: bool| {
                    seen.borrow_mut().push((at.thread_id.clone(), at.workspace_id.clone()));
                    async move { Ok(Some(entry("w1", json!({})) as Entry)) }.boxed_local()
                }
            }),
        );
        let got = area.run_step(step(json!({ "threadId": "t7", "turnId": "turn-7" })), None).await.expect("a run");
        assert_eq!(*seen.borrow(), vec![(Some("t7".to_string()), "w1".to_string())]);
        // No row and no session: the step's own turn names the session.
        assert_eq!(host.recorded_of("session.run")[0]["sessionId"], "turn-7");
        assert_eq!(got.session_id, "turn-7");
    });
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

// C19: session-run.test.ts:58
#[test]
fn an_ended_run_is_answered_from_the_transcript() {
    run_local(async {
        let (host, area) = run_host();
        host.recorded.borrow_mut().push(event(run_event(json!({ "runId": "run-2", "state": "exited", "exitCode": 130, "at": T0 - 5 }))));
        host.recorded.borrow_mut().push(event(run_event(json!({ "runId": "run-3", "state": "running" }))));
        let before = host.recorded_json();

        // run-2 ended; another window reports it lost, and it stays as it ended.
        let got = area.run_step(step(json!({ "runId": "run-2", "state": "lost" })), None).await.expect("a run");
        let mut want = run_event(json!({ "runId": "run-2", "state": "exited", "exitCode": 130, "at": T0 - 5 }));
        want.as_object_mut().expect("an object").remove("type");
        assert_eq!(json_of(&got), want);
        assert_eq!(host.recorded_json(), before, "nothing recorded");

        // A run the transcript holds only as running is not an ending: its end is recorded.
        area.run_step(step(json!({ "runId": "run-3", "state": "exited", "exitCode": 0 })), None).await.expect("a run");
        assert_eq!(host.recorded_json().len(), before.len() + 1);
        assert_eq!(host.recorded_json().last().expect("an event")["state"], "exited");
    });
}

// C20: session-run.test.ts:34, :58
#[test]
fn a_run_step_is_recorded_with_its_tail() {
    run_local(async {
        let (host, area) = run_host();
        let started = area.run_step(step(json!({ "ptyId": "pty_7" })), None).await.expect("a run");
        let mut want = run_event(json!({ "ptyId": "pty_7" }));
        assert_eq!(host.recorded_json(), vec![want.clone()], "recorded as the event, the agent's own session on it");
        want.as_object_mut().expect("an object").remove("type");
        want["at"] = json!(T0);
        assert_eq!(json_of(&started), want, "answered with the time it was recorded");

        host.clock.advance(10);
        let long = (0..40_000).map(|i| format!("y {i}")).collect::<Vec<_>>().join("\n");
        let ended = area.run_step(step(json!({ "runId": "run-2", "state": "exited", "exitCode": 130, "output": long })), None).await.expect("a run");
        let tail = run_output_tail(&long, RUN_OUTPUT_MAX_CHARS as usize);
        assert!(tail.ends_with("y 39999") && tail.len() <= RUN_OUTPUT_MAX_CHARS as usize);
        let recorded = host.recorded_of("session.run");
        assert_eq!(recorded[1], run_event(json!({ "runId": "run-2", "state": "exited", "exitCode": 130, "output": tail })), "no ptyId on a step that carried none");
        assert_eq!(json_of(&ended)["output"], json!(tail));
        assert_eq!(json_of(&ended)["at"], json!(T0 + 10));
    });
}

// C20: the session is the latest row's agent session, else its id, else the step's turn
#[test]
fn a_run_names_the_latest_rows_session_else_its_id() {
    run_local(async {
        let (host, area) = host();
        put(&host, "old", "w1", json!({ "threadId": "t1", "status": "completed", "claudeSessionId": "agent-old", "startedAt": T0 }), None);
        put(&host, "new", "w1", json!({ "threadId": "t1", "status": "completed", "startedAt": T0 + 5 }), None);
        area.run_step(step(json!({ "runId": "a" })), None).await.expect("a run");
        host.rows.borrow()["new"].borrow_mut().view.claude_session_id = Some("agent-new".into());
        area.run_step(step(json!({ "runId": "b" })), None).await.expect("a run");
        let sessions: Vec<Value> = host.recorded_of("session.run").iter().map(|e| e["sessionId"].clone()).collect();
        assert_eq!(sessions, vec![json!("new"), json!("agent-new")], "the latest row's own id where it has no agent session");
    });
}
