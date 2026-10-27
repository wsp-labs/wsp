// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/permission-relay.test.ts at 65f6b89c7, case for case. A permission prompt a harness raises mid-turn,
//! all the way through the runtime: into the transcript as its own row with the options a person picks, answered from the chat,
//! standing on the thread's row for as long as nobody answers, cancelled with the turn on a stop, the access a thread starts at
//! on a machine the person keeps, and an access picked while a turn runs. The harness here is a fake that raises the prompt on
//! command, so nothing on a machine is needed and the runtime's own bookkeeping is what is under test. Each test names the TS
//! test above it. The TS `memoryStore()` is the sqlite store here.
mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use common::adapter::{event, result};
use common::local::local_wiring;
use common::*;
use futures::channel::oneshot;
use futures::future::LocalBoxFuture;
use futures::FutureExt as _;
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_adapters::port::{
    AccessOutcome, AdapterError, AnswerOutcome, ExecFailure, HarnessAdapter, HarnessSession, HarnessStartOptions, OnEvent,
    PermissionAnswer, SharedFuture, TurnResult, PERMISSION_ALLOW,
};
use wsp_core::clock::FakeClock;
use wsp_core::types::api::StartOptions;
use wsp_core::types::harness::HarnessAdapterFactory;
use wsp_core::types::wiring::{LocalWiring, SessionHandle};
use wsp_engine::copier::{fake_copier, Copier};
use wsp_wire::views::workspace::Caller;
use wsp_wire::views::workspace_more::WorkspaceOrigin;
use wsp_words::exit::Kind;
use wsp_words::thread_state::thread_word_of;
use wsp_words::views::session::fold_threads;
use wsp_words::words::computer::thread_word;
use wsp_words::words::tools::{
    denied_line, other_option_id, permission_ask_line, picked_option_id, question_options, PERMISSION_DENIED_LINE, QUESTION_TOOL,
};

const SESSION: &str = "22222222-2222-4222-8222-222222222222";
const PERMISSION_DENY: &str = "deny";

fn uuid() -> String {
    let n: u128 = rand::random();
    format!(
        "{:08x}-{:04x}-4{:03x}-8{:03x}-{:012x}",
        (n >> 96) as u32,
        (n >> 80) as u16,
        (n >> 68) as u16 & 0xfff,
        (n >> 56) as u16 & 0xfff,
        n as u64 & 0xffff_ffff_ffff
    )
}

fn ask_doc() -> Value {
    json!({
        "askId": "ask_1", "toolName": "Write", "toolUseId": "toolu_1",
        "input": "{\"file_path\":\"/root/out.txt\",\"content\":\"hi\"}", "detail": "out.txt",
        "options": [
            { "id": PERMISSION_ALLOW, "label": "Allow", "effect": "allow" },
            { "id": PERMISSION_DENY, "label": "Deny", "effect": "deny" },
            { "id": "mode:acceptEdits", "label": "acceptEdits", "effect": "mode", "mode": "acceptEdits" },
        ],
    })
}

/// What a running fake turn lets the test do to it.
struct Turn {
    on_event: OnEvent,
    session: String,
    open: RefCell<IndexMap<String, Value>>,
    /// What the adapter was asked to answer, in order: the option and the words a deny carries.
    answers: RefCell<Vec<Value>>,
    interrupted: Cell<bool>,
    tx: RefCell<Option<oneshot::Sender<TurnResult>>>,
}

impl Turn {
    /// Raises a prompt on the turn, as a CLI's control channel would.
    fn raise(&self, overrides: Value) {
        let mut ask = ask_doc();
        if let Some(over) = overrides.as_object() {
            ask.as_object_mut().expect("an object").extend(over.clone());
        }
        self.open.borrow_mut().insert(ask["askId"].as_str().expect("an id").to_string(), ask.clone());
        (self.on_event)(event(json!({ "type": "permission.ask", "sessionId": self.session, "ask": ask })));
    }

    /// Ends the turn with a reply.
    fn reply(&self) {
        let Some(tx) = self.tx.borrow_mut().take() else { return };
        let ids: Vec<String> = self.open.borrow().keys().cloned().collect();
        for ask_id in ids {
            self.open.borrow_mut().shift_remove(&ask_id);
            (self.on_event)(event(
                json!({ "type": "permission.close", "sessionId": self.session, "askId": ask_id, "outcome": "cancelled" }),
            ));
        }
        let done = result(if self.interrupted.get() {
            json!({ "status": "interrupted" })
        } else {
            json!({ "status": "completed", "text": "done" })
        });
        (self.on_event)(event(
            json!({ "type": "turn.done", "sessionId": self.session, "result": serde_json::to_value(&done).expect("a result") }),
        ));
        (self.on_event)(event(json!({ "type": "session.end", "sessionId": self.session, "exitCode": 0, "sawResult": true })));
        let _ = tx.send(done);
    }
}

type Turns = Rc<RefCell<Vec<Rc<Turn>>>>;

/// The id a launch hands the runtime and the id its events carry, which are one id unless the harness names its own session.
#[derive(Clone, Copy)]
enum Ids {
    One,
    /// Codex's: the launch is keyed by an id of its own, and the app-server names the thread it opened, which a resume then names.
    Codex,
    /// Claude's, where the CLI re-keys a resumed session: the launch resumes the thread's id and system/init names a new one.
    ClaudeRekey,
}

fn ids_of(ids: Ids, resume: Option<&str>) -> (String, String) {
    match ids {
        Ids::One => {
            let id = resume.map(str::to_string).unwrap_or_else(uuid);
            (id.clone(), id)
        }
        Ids::Codex => match resume {
            None => (uuid(), uuid()),
            Some(id) => (id.to_string(), id.to_string()),
        },
        Ids::ClaudeRekey => {
            let local = resume.map(str::to_string).unwrap_or_else(uuid);
            let session = if resume.is_none() { local.clone() } else { uuid() };
            (local, session)
        }
    }
}

/// An adapter whose turn raises whatever prompt the test tells it to and answers by handing the answer back, the way the claude
/// adapter's control channel does; the close event is the adapter's, as it is there.
fn asking_adapter(turns: &Turns, ids: Ids) -> HarnessAdapterFactory {
    let turns = turns.clone();
    Arc::new(move |_ctx| Ok(Arc::new(Asking { turns: turns.clone(), ids }) as Arc<dyn HarnessAdapter>))
}

struct Asking {
    turns: Turns,
    ids: Ids,
}

impl HarnessAdapter for Asking {
    fn steers(&self) -> bool {
        false
    }
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        let (local_id, session) = ids_of(self.ids, o.resume.as_deref());
        let on_event = o.on_event.clone().expect("an event sink");
        let (tx, rx) = oneshot::channel::<TurnResult>();
        let turn = Rc::new(Turn {
            on_event: on_event.clone(),
            session: session.clone(),
            open: RefCell::default(),
            answers: RefCell::default(),
            interrupted: Cell::new(false),
            tx: RefCell::new(Some(tx)),
        });
        on_event(event(json!({ "type": "session.start", "sessionId": session, "cwd": "/root" })));
        self.turns.borrow_mut().push(turn.clone());
        let finished: LocalBoxFuture<'static, TurnResult> = async move { rx.await.expect("the turn's end") }.boxed_local();
        Ok(Arc::new(AskingSession { local_id, finished: finished.shared(), turn }))
    }
}

struct AskingSession {
    local_id: String,
    finished: SharedFuture<TurnResult>,
    turn: Rc<Turn>,
}

impl HarnessSession for AskingSession {
    fn local_id(&self) -> String {
        self.local_id.clone()
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        self.finished.clone()
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        let turn = self.turn.clone();
        async move {
            turn.interrupted.set(true);
            turn.reply();
        }
        .boxed_local()
    }
    fn answers(&self) -> bool {
        true
    }
    fn answer(&self, ask_id: String, a: PermissionAnswer) -> Option<LocalBoxFuture<'static, Result<AnswerOutcome, ExecFailure>>> {
        let turn = self.turn.clone();
        Some(
            async move {
                if turn.open.borrow_mut().shift_remove(&ask_id).is_none() {
                    return Ok(AnswerOutcome::Gone);
                }
                let outcome = j(&a.outcome);
                let mut rec = json!({ "askId": ask_id, "optionId": a.option_id, "outcome": outcome, "denyMessage": a.deny_message });
                if let Some(reason) = &a.reason {
                    rec["reason"] = json!(reason);
                }
                turn.answers.borrow_mut().push(rec);
                (turn.on_event)(event(json!({ "type": "permission.close", "sessionId": turn.session, "askId": ask_id, "outcome": outcome, "optionId": a.option_id })));
                Ok(AnswerOutcome::Answered)
            }
            .boxed_local(),
        )
    }
}

/// A harness that dies before it announces a session: the launch that leaves a thread no turn ever ran on, which is the one
/// shape sessions.forget takes.
fn dying() -> HarnessAdapterFactory {
    Arc::new(|_ctx| Ok(Arc::new(Dying) as Arc<dyn HarnessAdapter>))
}

struct Dying;

impl HarnessAdapter for Dying {
    fn steers(&self) -> bool {
        false
    }
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        let id = uuid();
        let on_event = o.on_event.clone().expect("an event sink");
        let done = result(json!({ "status": "failed", "error": "the harness exited with code 1 before emitting a result" }));
        let (tx, rx) = oneshot::channel::<TurnResult>();
        let (emit_id, finish) = (id.clone(), done.clone());
        tokio::task::spawn_local(async move {
            tokio::task::yield_now().await;
            on_event(event(
                json!({ "type": "turn.done", "sessionId": emit_id, "result": serde_json::to_value(&finish).expect("a result") }),
            ));
            on_event(event(json!({ "type": "session.end", "sessionId": emit_id, "exitCode": 1, "sawResult": false })));
            let _ = tx.send(finish);
        });
        let finished: LocalBoxFuture<'static, TurnResult> = async move { rx.await.expect("the turn's end") }.boxed_local();
        Ok(Arc::new(PlainSession { local_id: id, finished: finished.shared() }))
    }
}

struct PlainSession {
    local_id: String,
    finished: SharedFuture<TurnResult>,
}

impl HarnessSession for PlainSession {
    fn local_id(&self) -> String {
        self.local_id.clone()
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        self.finished.clone()
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        async {}.boxed_local()
    }
}

fn local_in(root: &std::path::Path) -> LocalWiring {
    let copier: Rc<dyn Copier> = Rc::new(fake_copier(None, None));
    let mut local = local_wiring(&std::fs::canonicalize(root).expect("a real path"), copier);
    local.daemon_road = None;
    local
}

fn start(prompt: &str) -> StartOptions {
    StartOptions { prompt: prompt.into(), ..Default::default() }
}

fn on_thread(prompt: &str, thread: &str) -> StartOptions {
    StartOptions { thread: Some(thread.into()), ..start(prompt) }
}

fn scope_of(thread: &str, workspace: &str) -> Caller {
    let by = serde_json::from_value(json!({ "kind": "thread", "threadId": thread, "workspaceId": workspace, "rootThreadId": thread }))
        .expect("a scope");
    Caller::Scoped { origin: WorkspaceOrigin::Here, by }
}

fn answer_ask(ask_id: &str, option_id: &str, reason: Option<&str>) -> wsp_core::types::api::AnswerAsk {
    wsp_core::types::api::AnswerAsk { ask_id: ask_id.into(), option_id: option_id.into(), reason: reason.map(str::to_string) }
}

// ===== a permission prompt relayed into the chat =====

struct Relay {
    fx: Fx,
    turns: Turns,
    wait: FakeClock,
    _root: tempfile::TempDir,
    _state: State,
}

fn relay_with(extra: Vec<(&str, HarnessAdapterFactory)>, claude_ids: Ids) -> Relay {
    let root = tempfile::tempdir().expect("a folder");
    let state = sqlite_state();
    let turns: Turns = Rc::default();
    let wait = FakeClock::at_now();
    let local = local_in(root.path());
    let backend = StubBackend::new(None);
    let mut wiring = runtime(&backend)
        .store(&state.store)
        .clock(&wait)
        .adapter("claude", asking_adapter(&turns, claude_ids))
        .with(move |o| o.local = Some(local));
    for (name, factory) in extra {
        wiring = wiring.adapter(name, factory);
    }
    Relay { fx: wiring.build(), turns, wait, _root: root, _state: state }
}

fn relay() -> Relay {
    relay_with(vec![], Ids::One)
}

impl Relay {
    async fn history(&self, ws: &str) -> Vec<Value> {
        self.fx.rt.sessions().history(ws, None).await.expect("a history").iter().map(j).collect()
    }
    async fn prompts(&self, ws: &str) -> Vec<Value> {
        self.history(ws).await.into_iter().filter(|e| e["type"] == "session.permission").collect()
    }
    async fn closes(&self, ws: &str) -> Vec<Value> {
        self.history(ws).await.into_iter().filter(|e| e["type"] == "session.permission.closed").collect()
    }
    fn turn(&self, n: usize) -> Rc<Turn> {
        self.turns.borrow()[n].clone()
    }
    async fn turns_are(&self, n: usize) {
        until(|| async { self.turns.borrow().len() == n }).await;
    }
    async fn prompts_are(&self, ws: &str, n: usize) {
        until(|| async { self.prompts(ws).await.len() == n }).await;
    }
    /// A started turn on the one local workspace, with the fake turn it opened.
    async fn started(&self) -> (Rc<dyn SessionHandle>, Rc<Turn>, String) {
        let ws = create_on(&self.fx.rt, CreateOn::named("mac").on("here"), None).await.expect("a workspace").view.id;
        let handle = self.fx.rt.sessions().start(&ws, start("write it"), None).await.expect("a start");
        self.turns_are(1).await;
        (handle, self.turn(0), ws)
    }
    async fn answer(&self, session: &str, ask: wsp_core::types::api::AnswerAsk) -> Value {
        j(&self.fx.rt.sessions().answer(session, ask, None).await.expect("an answer"))
    }
    async fn thread(&self, ws: &str) -> wsp_wire::views::session::ThreadView {
        let rows = self.fx.rt.sessions().list(Some(ws.to_string()), None).await.expect("a list");
        fold_threads(&rows).remove(0)
    }
}

fn answers_of(turn: &Turn) -> Vec<Value> {
    turn.answers.borrow().clone()
}

// "a permission prompt relayed into the chat" > "lands in the transcript as its own row, with the harness table's words for a mode option and the wait it gets"
#[test]
fn lands_in_the_transcript_as_its_own_row_with_the_harness_tables_words_for_a_mode_option_and_the_wait_it_gets() {
    drive(|| async {
        let r = relay();
        let (handle, turn, ws) = r.started().await;
        turn.raise(json!({}));
        r.prompts_are(&ws, 1).await;
        let row = r.prompts(&ws).await.remove(0);
        assert_matches(
            &row,
            json!({
                "type": "session.permission", "workspaceId": ws, "askId": "ask_1", "toolName": "Write", "toolUseId": "toolu_1",
                "detail": "out.txt", "input": "{\"file_path\":\"/root/out.txt\",\"content\":\"hi\"}", "turnId": handle.turn_id(),
            }),
        );
        // The adapter hands the CLI's own mode slug; the words for it live in the harness table, beside the picker's.
        assert_eq!(
            row["options"],
            json!([
                { "id": PERMISSION_ALLOW, "label": "Allow", "effect": "allow" },
                { "id": PERMISSION_DENY, "label": "Deny", "effect": "deny" },
                { "id": "mode:acceptEdits", "label": "Allow, then Accept edits", "effect": "mode", "mode": "acceptEdits" },
            ])
        );
        // Nothing closed it yet: the row is what the thread is waiting on.
        assert!(r.closes(&ws).await.is_empty());
        turn.reply();
        handle.finished().await;
        r.fx.rt.close().await.expect("a close");
    });
}

// "a permission prompt relayed into the chat" > "an option picked in the chat reaches the harness and closes the row once"
#[test]
fn an_option_picked_in_the_chat_reaches_the_harness_and_closes_the_row_once() {
    drive(|| async {
        let r = relay();
        let (handle, turn, ws) = r.started().await;
        turn.raise(json!({}));
        r.prompts_are(&ws, 1).await;
        assert_eq!(r.answer(&handle.id(), answer_ask("ask_1", PERMISSION_ALLOW, None)).await, json!({ "outcome": "answered" }));
        assert_eq!(
            answers_of(&turn),
            [json!({ "askId": "ask_1", "optionId": PERMISSION_ALLOW, "outcome": "allowed", "denyMessage": PERMISSION_DENIED_LINE })]
        );
        let closes = r.closes(&ws).await;
        assert_eq!(closes.len(), 1);
        assert_matches(&closes[0], json!({ "askId": "ask_1", "outcome": "allowed", "optionId": PERMISSION_ALLOW }));
        // The prompt is closed: a second client picking on the same row answers nothing rather than a second time.
        assert_eq!(r.answer(&handle.id(), answer_ask("ask_1", PERMISSION_DENY, None)).await, json!({ "outcome": "gone" }));
        assert_eq!(answers_of(&turn).len(), 1);
        assert_eq!(r.closes(&ws).await.len(), 1);
        turn.reply();
        handle.finished().await;
        r.fx.rt.close().await.expect("a close");
    });
}

/// Answers the open prompt the way the app and `wsp thread allow` do, by the session id its row carries.
async fn allow_by_row(r: &Relay, ws: &str, turn: &Turn, handle_id: &str) {
    turn.raise(json!({}));
    r.prompts_are(ws, 1).await;
    let row = r.prompts(ws).await.remove(0);
    assert_ne!(row["sessionId"], handle_id);
    let session = row["sessionId"].as_str().expect("a session").to_string();
    assert_eq!(r.answer(&session, answer_ask("ask_1", PERMISSION_ALLOW, None)).await, json!({ "outcome": "answered" }));
    let answers = answers_of(turn);
    assert_eq!(answers.len(), 1);
    assert_matches(&answers[0], json!({ "askId": "ask_1", "optionId": PERMISSION_ALLOW, "outcome": "allowed" }));
    let closes = r.closes(ws).await;
    assert_eq!(closes.len(), 1);
    assert_matches(&closes[0], json!({ "askId": "ask_1", "outcome": "allowed" }));
}

// "a permission prompt relayed into the chat" > "is answered by its row's session id on a Codex thread, whose app-server names a thread id of its own"
#[test]
fn is_answered_by_its_rows_session_id_on_a_codex_thread_whose_app_server_names_a_thread_id_of_its_own() {
    drive(|| async {
        let turns: Turns = Rc::default();
        let r = relay_with(vec![("codex", asking_adapter(&turns, Ids::Codex))], Ids::One);
        let ws = create_on(&r.fx.rt, CreateOn::named("mac").on("here"), None).await.expect("a workspace").view.id;
        let opts = StartOptions { harness: Some("codex".into()), permission_mode: Some("read-only".into()), ..start("write it") };
        let handle = r.fx.rt.sessions().start(&ws, opts, None).await.expect("a start");
        until(|| async { turns.borrow().len() == 1 }).await;
        let turn = turns.borrow()[0].clone();
        allow_by_row(&r, &ws, &turn, &handle.id()).await;
        turn.reply();
        handle.finished().await;
        r.fx.rt.close().await.expect("a close");
    });
}

// "a permission prompt relayed into the chat" > "is answered by its row's session id on a Claude thread asking in a mode that asks, after the CLI re-keyed its resume"
#[test]
fn is_answered_by_its_rows_session_id_on_a_claude_thread_asking_in_a_mode_that_asks_after_the_cli_rekeyed_its_resume() {
    drive(|| async {
        let r = relay_with(vec![], Ids::ClaudeRekey);
        let ws = create_on(&r.fx.rt, CreateOn::named("mac").on("here"), None).await.expect("a workspace").view.id;
        let first =
            r.fx.rt
                .sessions()
                .start(&ws, StartOptions { permission_mode: Some("default".into()), ..start("one") }, None)
                .await
                .expect("a start");
        r.turns_are(1).await;
        r.turn(0).reply();
        first.finished().await;
        let thread = first.view().thread_id.expect("a thread");
        let next =
            r.fx.rt
                .sessions()
                .start(&ws, StartOptions { permission_mode: Some("default".into()), ..on_thread("write it", &thread) }, None)
                .await
                .expect("a start");
        r.turns_are(2).await;
        let turn = r.turn(1);
        allow_by_row(&r, &ws, &turn, &next.id()).await;
        turn.reply();
        next.finished().await;
        r.fx.rt.close().await.expect("a close");
    });
}

// "a permission prompt relayed into the chat" > "carries a subagent's prompt and a subagent's lines to the chat with the call that launched them"
#[test]
fn carries_a_subagents_prompt_and_a_subagents_lines_to_the_chat_with_the_call_that_launched_them() {
    drive(|| async {
        let r = relay();
        let (handle, turn, ws) = r.started().await;
        turn.raise(json!({ "askId": "ask_child", "parentToolUseId": "toolu_launch" }));
        r.prompts_are(&ws, 1).await;
        assert_matches(&r.prompts(&ws).await[0], json!({ "askId": "ask_child", "parentToolUseId": "toolu_launch" }));
        turn.reply();
        handle.finished().await;
        r.fx.rt.close().await.expect("a close");
    });
}

// "a permission prompt relayed into the chat" > "a question's own choices reach the chat as the prompt's options, and a pick naming several is one answer"
#[test]
fn a_questions_own_choices_reach_the_chat_as_the_prompts_options_and_a_pick_naming_several_is_one_answer() {
    drive(|| async {
        let r = relay();
        let (handle, turn, ws) = r.started().await;
        let input = json!({ "questions": [{ "question": "Which checks?", "header": "Checks", "options": [{ "label": "Types" }, { "label": "Lint" }], "multiSelect": true }] }).to_string();
        let options = question_options(QUESTION_TOOL, &input);
        turn.raise(json!({ "askId": "ask_q", "toolName": QUESTION_TOOL, "input": input, "options": j(&options) }));
        r.prompts_are(&ws, 1).await;
        let row = r.prompts(&ws).await.remove(0);
        // The words for a mode option are lent by the harness table; a question's own labels are lent nothing.
        assert_eq!(row["options"], j(&options));
        let both = picked_option_id(&options.iter().map(|o| o.id.clone()).collect::<Vec<_>>());
        assert_eq!(r.answer(&handle.id(), answer_ask("ask_q", &both, None)).await, json!({ "outcome": "answered" }));
        // Several ticks are one pick, and it reaches the harness whole rather than being refused as no option.
        assert_matches(&answers_of(&turn)[0], json!({ "optionId": both, "outcome": "allowed" }));
        turn.reply();
        handle.finished().await;
        r.fx.rt.close().await.expect("a close");
    });
}

// "a permission prompt relayed into the chat" > "a deny tells the agent the person refused, and a mode pick reads as an allow"
#[test]
fn a_deny_tells_the_agent_the_person_refused_and_a_mode_pick_reads_as_an_allow() {
    drive(|| async {
        let r = relay();
        let (handle, turn, ws) = r.started().await;
        turn.raise(json!({}));
        r.prompts_are(&ws, 1).await;
        assert_eq!(r.answer(&handle.id(), answer_ask("ask_1", PERMISSION_DENY, None)).await, json!({ "outcome": "answered" }));
        assert_matches(&answers_of(&turn)[0], json!({ "outcome": "denied", "denyMessage": PERMISSION_DENIED_LINE }));
        turn.raise(json!({ "askId": "ask_2" }));
        r.prompts_are(&ws, 2).await;
        assert_eq!(r.answer(&handle.id(), answer_ask("ask_2", "mode:acceptEdits", None)).await, json!({ "outcome": "answered" }));
        assert_matches(&answers_of(&turn)[1], json!({ "optionId": "mode:acceptEdits", "outcome": "allowed" }));
        turn.reply();
        handle.finished().await;
        r.fx.rt.close().await.expect("a close");
    });
}

// "a permission prompt relayed into the chat" > "a deny with the person's reason tells the agent the reason, and an answer typed in Other reaches the harness"
#[test]
fn a_deny_with_the_persons_reason_tells_the_agent_the_reason_and_an_answer_typed_in_other_reaches_the_harness() {
    drive(|| async {
        let r = relay();
        let (handle, turn, ws) = r.started().await;
        turn.raise(json!({}));
        r.prompts_are(&ws, 1).await;
        let said = r.answer(&handle.id(), answer_ask("ask_1", PERMISSION_DENY, Some("Run only the thread-rows file"))).await;
        assert_eq!(said, json!({ "outcome": "answered" }));
        assert_matches(
            &answers_of(&turn)[0],
            json!({ "outcome": "denied", "denyMessage": denied_line(Some("Run only the thread-rows file")), "reason": "Run only the thread-rows file" }),
        );
        let input = json!({ "questions": [{ "question": "Which one?", "header": "Pick", "options": [{ "label": "A" }, { "label": "B" }], "multiSelect": false }] }).to_string();
        turn.raise(
            json!({ "askId": "ask_q", "toolName": QUESTION_TOOL, "input": input, "options": j(&question_options(QUESTION_TOOL, &input)) }),
        );
        r.prompts_are(&ws, 2).await;
        let typed = other_option_id(0, "neither, C");
        assert_eq!(r.answer(&handle.id(), answer_ask("ask_q", &typed, None)).await, json!({ "outcome": "answered" }));
        assert_matches(&answers_of(&turn)[1], json!({ "optionId": typed, "outcome": "allowed" }));
        turn.reply();
        handle.finished().await;
        r.fx.rt.close().await.expect("a close");
    });
}

// "a permission prompt relayed into the chat" > "stands open for as long as the turn lives, well past the five minutes a person may be away for"
#[test]
fn stands_open_for_as_long_as_the_turn_lives_well_past_the_five_minutes_a_person_may_be_away_for() {
    drive(|| async {
        let r = relay();
        let (handle, turn, ws) = r.started().await;
        turn.raise(json!({}));
        r.prompts_are(&ws, 1).await;
        r.wait.advance(20 * 60_000);
        tokio::task::yield_now().await;
        assert!(r.closes(&ws).await.is_empty());
        assert!(answers_of(&turn).is_empty());
        assert_eq!(j(&handle.view())["status"], "running");
        // The person comes back to the question they were asked and answers it.
        assert_eq!(r.answer(&handle.id(), answer_ask("ask_1", PERMISSION_ALLOW, None)).await, json!({ "outcome": "answered" }));
        assert_eq!(
            answers_of(&turn),
            [json!({ "askId": "ask_1", "optionId": PERMISSION_ALLOW, "outcome": "allowed", "denyMessage": PERMISSION_DENIED_LINE })]
        );
        turn.reply();
        handle.finished().await;
        r.fx.rt.close().await.expect("a close");
    });
}

// "a permission prompt relayed into the chat" > "puts what it is asking on the thread's row while it stands and takes it off when it is answered"
#[test]
fn puts_what_it_is_asking_on_the_threads_row_while_it_stands_and_takes_it_off_when_it_is_answered() {
    drive(|| async {
        let r = relay();
        let (handle, turn, ws) = r.started().await;
        assert_eq!(thread_word_of(&r.thread(&ws).await), "Working");
        turn.raise(json!({}));
        let asking = permission_ask_line("Write", ask_doc()["input"].as_str().expect("an input"), Some("out.txt"));
        until(|| async { j(&r.thread(&ws).await)["asking"] == asking.as_str() }).await;
        assert_eq!(thread_word_of(&r.thread(&ws).await), "Needs you");
        r.answer(&handle.id(), answer_ask("ask_1", PERMISSION_ALLOW, None)).await;
        until(|| async { j(&r.thread(&ws).await).get("asking").is_none() }).await;
        assert_eq!(thread_word_of(&r.thread(&ws).await), "Working");
        assert_eq!(r.closes(&ws).await.len(), 1);
        turn.reply();
        handle.finished().await;
        r.fx.rt.close().await.expect("a close");
    });
}

// "a permission prompt relayed into the chat" > "a stop takes the open prompt with the turn: the row closes as cancelled and answering it afterwards answers nothing"
#[test]
fn a_stop_takes_the_open_prompt_with_the_turn_the_row_closes_as_cancelled_and_answering_it_afterwards_answers_nothing() {
    drive(|| async {
        let r = relay();
        let (handle, turn, ws) = r.started().await;
        turn.raise(json!({}));
        r.prompts_are(&ws, 1).await;
        let stopped = r.fx.rt.sessions().interrupt(&handle.id(), None, None).await.expect("an interrupt");
        assert_eq!(j(&stopped), json!({ "outcome": "accepted" }));
        let closes = r.closes(&ws).await;
        assert_eq!(closes.len(), 1);
        assert_matches(&closes[0], json!({ "askId": "ask_1", "outcome": "cancelled" }));
        assert!(closes[0].get("optionId").is_none());
        assert_eq!(r.answer(&handle.id(), answer_ask("ask_1", PERMISSION_ALLOW, None)).await, json!({ "outcome": "gone" }));
        assert!(answers_of(&turn).is_empty());
        r.fx.rt.close().await.expect("a close");
    });
}

// "a permission prompt relayed into the chat" > "a turn the runtime cuts under an open prompt closes its row and stops its wait, so no row waits on an answer forever"
#[test]
fn a_turn_the_runtime_cuts_under_an_open_prompt_closes_its_row_and_stops_its_wait_so_no_row_waits_on_an_answer_forever() {
    drive(|| async {
        let r = relay();
        // The cut roads (a nap, a machine gone, a zombie, a delete) all end the turn from this side rather than through the
        // harness, so this is the road the adapter's own close never travels.
        let cloud = create_on(&r.fx.rt, CreateOn::golden("b1"), None).await.expect("a workspace").view.id;
        let handle = r.fx.rt.sessions().start(&cloud, start("write it"), None).await.expect("a start");
        r.turns_are(1).await;
        let turn = r.turn(0);
        turn.raise(json!({}));
        r.prompts_are(&cloud, 1).await;

        r.fx.rt.workspaces().nap(&cloud, None).await.expect("a nap");
        until(|| async { r.closes(&cloud).await.len() == 1 }).await;
        let cut = r.history(&cloud).await;
        let closes = r.closes(&cloud).await;
        assert_matches(&closes[0], json!({ "askId": "ask_1", "outcome": "cancelled" }));
        assert!(closes[0].get("optionId").is_none());
        // The row closes before the turn ends, so a transcript reads the prompt going with the turn, not after it.
        let at = |kind: &str| cut.iter().position(|e| e["type"] == kind).expect("an event");
        assert!(at("session.permission.closed") < at("session.end"));
        // A paused workspace refuses the verb in its own words, as a send gets, and the closed row offers nothing anyway.
        let paused =
            r.fx.rt.sessions().answer(&handle.id(), answer_ask("ask_1", PERMISSION_ALLOW, None), None).await.err().expect("a refusal");
        assert!(paused.message.contains("Workspace is paused; wake it to send"), "{}", paused.message);

        // Nothing is left waiting on a turn that is over, and no second row lands however long the clock runs.
        r.wait.advance(20 * 60_000);
        assert!(answers_of(&turn).is_empty());
        assert_eq!(r.closes(&cloud).await.len(), 1);
        r.fx.rt.close().await.expect("a close");
    });
}

// "a permission prompt relayed into the chat" > "a thread of another tree forgetting a thread on the workspace is told not-found, and the thread stands"
#[test]
fn a_thread_of_another_tree_forgetting_a_thread_on_the_workspace_is_told_not_found_and_the_thread_stands() {
    drive(|| async {
        let r = relay_with(vec![("codex", dying())], Ids::One);
        let ws = create_on(&r.fx.rt, CreateOn::named("mac").on("here"), None).await.expect("a workspace").view.id;
        // A launch that never got going: the one thread shape forget takes, so what holds it back here is the tree.
        let junk =
            r.fx.rt
                .sessions()
                .start(&ws, StartOptions { harness: Some("codex".into()), ..start("never got going") }, None)
                .await
                .expect("a start");
        junk.finished().await;
        let target = junk.view().thread_id.expect("a thread");
        let beside =
            r.fx.rt
                .sessions()
                .start(&ws, StartOptions { harness: Some("claude".into()), ..start("beside it") }, None)
                .await
                .expect("a start");
        r.turns_are(1).await;
        let beside_thread = beside.view().thread_id.expect("a thread");
        let outside = scope_of(&beside_thread, &ws);
        // The absence a thread id nothing holds answers, so the caller learns nothing about a tree that is not its own.
        let refused = r.fx.rt.sessions().forget(&target, Some(outside), None).await.err().expect("a refusal");
        assert_eq!(refused.message, format!("no thread {}", thread_word(&target)));
        assert_eq!(refused.kind, Some(Kind::NotFound));
        let ids = |rows: &[wsp_wire::views::session::SessionView]| fold_threads(rows).iter().map(|t| t.id.clone()).collect::<Vec<_>>();
        let rows = r.fx.rt.sessions().list(Some(ws.clone()), None).await.expect("a list");
        assert!(ids(&rows).contains(&target));
        // The person, who is no thread, forgets it as before.
        r.fx.rt.sessions().forget(&target, None, None).await.expect("a forget");
        let rows = r.fx.rt.sessions().list(Some(ws.clone()), None).await.expect("a list");
        assert_eq!(ids(&rows), [beside_thread]);
        r.turn(0).reply();
        beside.finished().await;
        r.fx.rt.close().await.expect("a close");
    });
}

// "a permission prompt relayed into the chat" > "answers what it can rather than throwing: an unknown session, an unknown prompt, an option the prompt never carried"
#[test]
fn answers_what_it_can_rather_than_throwing_an_unknown_session_an_unknown_prompt_an_option_the_prompt_never_carried() {
    drive(|| async {
        let r = relay();
        let (handle, turn, ws) = r.started().await;
        turn.raise(json!({}));
        r.prompts_are(&ws, 1).await;
        assert_eq!(r.answer("s_nope", answer_ask("ask_1", PERMISSION_ALLOW, None)).await, json!({ "outcome": "not-found" }));
        assert_eq!(r.answer(&handle.id(), answer_ask("ask_9", PERMISSION_ALLOW, None)).await, json!({ "outcome": "gone" }));
        assert_eq!(r.answer(&handle.id(), answer_ask("ask_1", "mode:bypassPermissions", None)).await, json!({ "outcome": "no-option" }));
        assert!(answers_of(&turn).is_empty());
        turn.reply();
        handle.finished().await;
        r.fx.rt.close().await.expect("a close");
    });
}

// "a permission prompt relayed into the chat" > "a thread of another tree on the workspace is told not-found, as the interrupt tells it, and answers nothing; the asking thread answers"
#[test]
fn a_thread_of_another_tree_on_the_workspace_is_told_not_found_as_the_interrupt_tells_it_and_answers_nothing_the_asking_thread_answers() {
    drive(|| async {
        let r = relay();
        let (handle, turn, ws) = r.started().await;
        turn.raise(json!({}));
        r.prompts_are(&ws, 1).await;
        // A second thread the person opened on the same workspace is its own root: the first thread is outside its tree.
        let beside = r.fx.rt.sessions().start(&ws, start("beside it"), None).await.expect("a start");
        r.turns_are(2).await;
        let beside_thread = beside.view().thread_id.expect("a thread");
        let outside = scope_of(&beside_thread, &ws);
        // The same absence the sibling verbs answer, before the workspace is read: the prompt stays open and the harness hears
        // nothing, so a thread of one tree cannot grant what another tree's agent asked to do.
        let stopped = r.fx.rt.sessions().interrupt(&handle.id(), Some(outside.clone()), None).await.expect("an answer");
        assert_eq!(j(&stopped), json!({ "outcome": "not-found" }));
        let answered =
            r.fx.rt.sessions().answer(&handle.id(), answer_ask("ask_1", PERMISSION_ALLOW, None), Some(outside)).await.expect("an answer");
        assert_eq!(j(&answered), json!({ "outcome": "not-found" }));
        assert!(answers_of(&turn).is_empty());
        assert!(r.closes(&ws).await.is_empty());
        // A caller inside the tree answers as the person does.
        let asking_thread = handle.view().thread_id.expect("a thread");
        let inside = scope_of(&asking_thread, &ws);
        let answered =
            r.fx.rt.sessions().answer(&handle.id(), answer_ask("ask_1", PERMISSION_ALLOW, None), Some(inside)).await.expect("an answer");
        assert_eq!(j(&answered), json!({ "outcome": "answered" }));
        let answers = answers_of(&turn);
        assert_eq!(answers.len(), 1);
        assert_matches(&answers[0], json!({ "askId": "ask_1", "optionId": PERMISSION_ALLOW, "outcome": "allowed" }));
        assert_eq!(r.closes(&ws).await.len(), 1);
        r.turn(1).reply();
        beside.finished().await;
        turn.reply();
        handle.finished().await;
        r.fx.rt.close().await.expect("a close");
    });
}

// ===== the access a thread starts at =====

/// A runtime whose turns record their picks and end at once, so a start's access is readable off the row.
struct Recording {
    fx: Fx,
    picks: Rc<RefCell<Vec<Option<String>>>>,
    _root: tempfile::TempDir,
    _state: State,
}

struct Recorder {
    picks: Rc<RefCell<Vec<Option<String>>>>,
}

impl HarnessAdapter for Recorder {
    fn steers(&self) -> bool {
        false
    }
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        self.picks.borrow_mut().push(o.permission_mode.clone());
        let on_event = o.on_event.clone().expect("an event sink");
        let done = result(json!({ "status": "completed", "text": "ok" }));
        let (tx, rx) = oneshot::channel::<TurnResult>();
        let finish = done.clone();
        tokio::task::spawn_local(async move {
            tokio::task::yield_now().await;
            on_event(event(json!({ "type": "session.start", "sessionId": SESSION })));
            on_event(event(
                json!({ "type": "turn.done", "sessionId": SESSION, "result": serde_json::to_value(&finish).expect("a result") }),
            ));
            on_event(event(json!({ "type": "session.end", "sessionId": SESSION, "exitCode": 0, "sawResult": true })));
            let _ = tx.send(finish);
        });
        let finished: LocalBoxFuture<'static, TurnResult> = async move { rx.await.expect("the turn's end") }.boxed_local();
        Ok(Arc::new(PlainSession { local_id: SESSION.into(), finished: finished.shared() }))
    }
}

fn recording() -> Recording {
    let root = tempfile::tempdir().expect("a folder");
    let state = sqlite_state();
    let picks: Rc<RefCell<Vec<Option<String>>>> = Rc::default();
    let kept = picks.clone();
    let factory: HarnessAdapterFactory = Arc::new(move |_ctx| Ok(Arc::new(Recorder { picks: kept.clone() }) as Arc<dyn HarnessAdapter>));
    let local = local_in(root.path());
    let backend = StubBackend::new(None);
    let fx = runtime(&backend).store(&state.store).adapter("claude", factory).with(move |o| o.local = Some(local)).build();
    Recording { fx, picks, _root: root, _state: state }
}

impl Recording {
    fn picks(&self) -> Vec<Option<String>> {
        self.picks.borrow().clone()
    }
    async fn here(&self) -> wsp_core::types::wiring::CreatedWorkspace {
        create_on(&self.fx.rt, CreateOn::named("mac").on("here"), None).await.expect("a workspace")
    }
    async fn claude(&self, ws: &str) -> Value {
        let list = self.fx.rt.harnesses().list(Some(ws.to_string()), None, None).await.expect("a list");
        j(&list[0])
    }
}

fn some(modes: &[&str]) -> Vec<Option<String>> {
    modes.iter().map(|m| Some((*m).to_string())).collect()
}

fn prefs(doc: Value) -> wsp_wire::views::preferences::PreferencesPatch {
    serde_json::from_value(doc).expect("a patch")
}

// "the access a thread starts at" > "on this computer the composer's access list marks the mode that asks nothing until the person sets another"
#[test]
fn on_this_computer_the_composers_access_list_marks_the_mode_that_asks_nothing_until_the_person_sets_another() {
    drive(|| async {
        let r = recording();
        let local = r.here().await;
        let claude = r.claude(&local.view.id).await;
        assert_eq!(claude["access"]["ask"], "default");
        let modes = claude["permissionModes"].as_array().expect("modes");
        assert_eq!(modes.iter().find(|o| o["isDefault"] == true).map(|o| o["value"].clone()), Some(json!("bypassPermissions")));
        // Every mode that asks is still one pick away, in the same list, in the same order.
        assert!(modes.iter().any(|o| o["value"] == "default"));
        r.fx.rt.close().await.expect("a close");
    });
}

// "the access a thread starts at" > "a thread on this computer runs every action without asking, explicitly, and a send that names none keeps it"
#[test]
fn a_thread_on_this_computer_runs_every_action_without_asking_explicitly_and_a_send_that_names_none_keeps_it() {
    drive(|| async {
        let r = recording();
        let local = r.here().await.view.id;
        let first = r.fx.rt.sessions().start(&local, start("one"), None).await.expect("a start");
        first.finished().await;
        assert_eq!(r.picks(), some(&["bypassPermissions"]));
        // The second turn resumes the thread and names no access; the mode is named rather than left out, so a thread whose
        // person picked another one keeps it instead of falling to the adapter's own flag.
        let thread = first.view().thread_id.expect("a thread");
        r.fx.rt.sessions().start(&local, on_thread("two", &thread), None).await.expect("a start").finished().await;
        assert_eq!(r.picks(), some(&["bypassPermissions", "bypassPermissions"]));
        // A mode named on a send is dropped: a thread's access is the thread's own, and sessions.access is the one road that
        // changes what it may touch.
        let named = StartOptions { permission_mode: Some("dontAsk".into()), ..on_thread("three", &thread) };
        r.fx.rt.sessions().start(&local, named, None).await.expect("a start").finished().await;
        assert_eq!(r.picks(), some(&["bypassPermissions", "bypassPermissions", "bypassPermissions"]));
        r.fx.rt.close().await.expect("a close");
    });
}

// "the access a thread starts at" > "a thread nobody named an access for starts at the agent's default, the project's over it, and never at a workspace's last pick"
#[test]
fn a_thread_nobody_named_an_access_for_starts_at_the_agents_default_the_projects_over_it_and_never_at_a_workspaces_last_pick() {
    drive(|| async {
        let r = recording();
        let local = r.here().await;
        let id = local.view.id.clone();
        r.fx.rt.sessions().start(&id, start("one"), None).await.expect("a start").finished().await;
        assert_eq!(r.picks(), some(&["bypassPermissions"]));
        // The composer's pick is for one thread: one kept per workspace decides no later thread.
        r.fx.rt.preferences().set(prefs(json!({ "access": { id.clone(): "default" } }))).await.expect("a set");
        r.fx.rt.sessions().start(&id, start("two"), None).await.expect("a start").finished().await;
        assert_eq!(r.picks(), some(&["bypassPermissions", "bypassPermissions"]));
        r.fx.rt.preferences().set(prefs(json!({ "agentDefaults": { "claude": { "access": "ask" } } }))).await.expect("a set");
        r.fx.rt.sessions().start(&id, start("three"), None).await.expect("a start").finished().await;
        assert_eq!(r.picks().last().cloned().flatten().as_deref(), Some("default"));
        let project = j(&local.view)["project"]["id"].as_str().expect("a project").to_string();
        r.fx.rt.preferences().set(prefs(json!({ "projectDefaults": { project: { "access": "auto-edit" } } }))).await.expect("a set");
        r.fx.rt.sessions().start(&id, start("four"), None).await.expect("a start").finished().await;
        assert_eq!(r.picks().last().cloned().flatten().as_deref(), Some("acceptEdits"));
        // The composer's list carries the same mark, so what the picker shows is what a start runs.
        let claude = r.claude(&id).await;
        let modes = claude["permissionModes"].as_array().expect("modes");
        assert_eq!(modes.iter().find(|o| o["isDefault"] == true).map(|o| o["value"].clone()), Some(json!("acceptEdits")));
        r.fx.rt.close().await.expect("a close");
    });
}

// "the access a thread starts at" > "a word the agent maps to no mode is refused at the set and on a start, never run looser"
#[test]
fn a_word_the_agent_maps_to_no_mode_is_refused_at_the_set_and_on_a_start_never_run_looser() {
    drive(|| async {
        let r = recording();
        let local = r.here().await;
        let id = local.view.id.clone();
        let want = "Claude Code takes no plan access; it takes ask, auto-edit, full";
        let set =
            r.fx.rt
                .preferences()
                .set(prefs(json!({ "agentDefaults": { "claude": { "access": "plan" } } })))
                .await
                .err()
                .expect("a refusal");
        assert_eq!((set.message.as_str(), set.kind), (want, Some(Kind::Usage)));
        let project = j(&local.view)["project"]["id"].as_str().expect("a project").to_string();
        let set =
            r.fx.rt
                .preferences()
                .set(prefs(json!({ "projectDefaults": { project: { "access": "plan" } } })))
                .await
                .err()
                .expect("a refusal");
        assert_eq!(set.kind, Some(Kind::Usage));
        let access = StartOptions { access: Some(serde_json::from_value(json!("plan")).expect("an access")), ..start("one") };
        let started = r.fx.rt.sessions().start(&id, access, None).await.err().expect("a refusal");
        assert_eq!((started.message.as_str(), started.kind), (want, Some(Kind::Usage)));
        assert!(r.picks().is_empty());
        let two = StartOptions { access: Some(serde_json::from_value(json!("auto-edit")).expect("an access")), ..start("two") };
        r.fx.rt.sessions().start(&id, two, None).await.expect("a start").finished().await;
        assert_eq!(r.picks(), some(&["acceptEdits"]));
        r.fx.rt.close().await.expect("a close");
    });
}

// "the access a thread starts at" > "a start that names an access wins over every default, and a resumed thread keeps its own"
#[test]
fn a_start_that_names_an_access_wins_over_every_default_and_a_resumed_thread_keeps_its_own() {
    drive(|| async {
        let r = recording();
        let id = r.here().await.view.id;
        let first = r.fx.rt.sessions().start(&id, start("one"), None).await.expect("a start");
        first.finished().await;
        r.fx.rt.preferences().set(prefs(json!({ "agentDefaults": { "claude": { "access": "auto-edit" } } }))).await.expect("a set");
        // The thread opened before the default keeps the access its own turns ran at; the default is what a new thread reads.
        let thread = first.view().thread_id.expect("a thread");
        r.fx.rt.sessions().start(&id, on_thread("two", &thread), None).await.expect("a start").finished().await;
        // A start that names one wins over both.
        let named = StartOptions { permission_mode: Some("dontAsk".into()), ..start("three") };
        r.fx.rt.sessions().start(&id, named, None).await.expect("a start").finished().await;
        assert_eq!(r.picks(), some(&["bypassPermissions", "bypassPermissions", "dontAsk"]));
        r.fx.rt.close().await.expect("a close");
    });
}

// "the access a thread starts at" > "a throwaway machine's list and its threads are unchanged: bypass is the default and carries no machine's name"
#[test]
fn a_throwaway_machines_list_and_its_threads_are_unchanged_bypass_is_the_default_and_carries_no_machines_name() {
    drive(|| async {
        let r = recording();
        let cloud = create_on(&r.fx.rt, CreateOn::golden("b1"), None).await.expect("a workspace").view.id;
        let claude = r.claude(&cloud).await;
        let modes = claude["permissionModes"].as_array().expect("modes");
        assert_eq!(modes.iter().find(|o| o["isDefault"] == true).map(|o| o["value"].clone()), Some(json!("bypassPermissions")));
        assert_eq!(modes.iter().find(|o| o["value"] == "bypassPermissions").map(|o| o["label"].clone()), Some(json!("Bypass")));
        r.fx.rt.sessions().start(&cloud, start("one"), None).await.expect("a start").finished().await;
        assert_eq!(r.picks(), some(&["bypassPermissions"]));
        r.fx.rt.close().await.expect("a close");
    });
}

// ===== an access picked while a turn runs =====

/// What a held fake turn lets the test do: the modes it was moved to, and its reply.
struct HeldTurn {
    modes: RefCell<Vec<String>>,
    tx: RefCell<Option<oneshot::Sender<TurnResult>>>,
    on_event: OnEvent,
    session: String,
}

impl HeldTurn {
    fn reply(&self) {
        let Some(tx) = self.tx.borrow_mut().take() else { return };
        let done = result(json!({ "status": "completed", "text": "done" }));
        (self.on_event)(event(
            json!({ "type": "turn.done", "sessionId": self.session, "result": serde_json::to_value(&done).expect("a result") }),
        ));
        (self.on_event)(event(json!({ "type": "session.end", "sessionId": self.session, "exitCode": 0, "sawResult": true })));
        let _ = tx.send(done);
    }
}

#[derive(Clone, Copy)]
enum Moves {
    Set,
    Refused,
    None,
}

struct HeldAdapter {
    moves: Moves,
    turns: Rc<RefCell<Vec<Rc<HeldTurn>>>>,
    picks: Rc<RefCell<Vec<Option<String>>>>,
}

impl HarnessAdapter for HeldAdapter {
    fn steers(&self) -> bool {
        false
    }
    fn moves_access(&self) -> bool {
        !matches!(self.moves, Moves::None)
    }
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        self.picks.borrow_mut().push(o.permission_mode.clone());
        let session = o.resume.clone().unwrap_or_else(uuid);
        let on_event = o.on_event.clone().expect("an event sink");
        let (tx, rx) = oneshot::channel::<TurnResult>();
        on_event(event(json!({ "type": "session.start", "sessionId": session, "cwd": "/root" })));
        let turn = Rc::new(HeldTurn { modes: RefCell::default(), tx: RefCell::new(Some(tx)), on_event, session: session.clone() });
        self.turns.borrow_mut().push(turn.clone());
        let finished: LocalBoxFuture<'static, TurnResult> = async move { rx.await.expect("the turn's end") }.boxed_local();
        Ok(Arc::new(HeldSession { local_id: session, finished: finished.shared(), turn, moves: self.moves }))
    }
}

struct HeldSession {
    local_id: String,
    finished: SharedFuture<TurnResult>,
    turn: Rc<HeldTurn>,
    moves: Moves,
}

impl HarnessSession for HeldSession {
    fn local_id(&self) -> String {
        self.local_id.clone()
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        self.finished.clone()
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        let turn = self.turn.clone();
        async move { turn.reply() }.boxed_local()
    }
    fn set_access(&self, mode: String) -> Option<LocalBoxFuture<'static, Result<AccessOutcome, ExecFailure>>> {
        let (turn, moves) = (self.turn.clone(), self.moves);
        match moves {
            Moves::None => None,
            _ => Some(
                async move {
                    turn.modes.borrow_mut().push(mode);
                    Ok(if matches!(moves, Moves::Set) { AccessOutcome::Set } else { AccessOutcome::Refused })
                }
                .boxed_local(),
            ),
        }
    }
}

/// A turn that stays open until the test replies. `moves` is what its harness answers a mode change with, and `None` is a
/// harness that takes none mid-turn. A send resumes its thread's session and a thread the send opens gets one of its own, as a
/// CLI's would, so two threads on one workspace hold two rows.
struct Held {
    fx: Fx,
    turns: Rc<RefCell<Vec<Rc<HeldTurn>>>>,
    picks: Rc<RefCell<Vec<Option<String>>>>,
    _root: tempfile::TempDir,
    _state: State,
}

fn held(moves: Moves) -> Held {
    let root = tempfile::tempdir().expect("a folder");
    let state = sqlite_state();
    let turns: Rc<RefCell<Vec<Rc<HeldTurn>>>> = Rc::default();
    let picks: Rc<RefCell<Vec<Option<String>>>> = Rc::default();
    let (kept_turns, kept_picks) = (turns.clone(), picks.clone());
    let factory: HarnessAdapterFactory = Arc::new(move |_ctx| {
        Ok(Arc::new(HeldAdapter { moves, turns: kept_turns.clone(), picks: kept_picks.clone() }) as Arc<dyn HarnessAdapter>)
    });
    let local = local_in(root.path());
    let backend = StubBackend::new(None);
    let fx = runtime(&backend).store(&state.store).adapter("claude", factory).with(move |o| o.local = Some(local)).build();
    Held { fx, turns, picks, _root: root, _state: state }
}

impl Held {
    fn picks(&self) -> Vec<Option<String>> {
        self.picks.borrow().clone()
    }
    fn turn(&self, n: usize) -> Rc<HeldTurn> {
        self.turns.borrow()[n].clone()
    }
    async fn turns_are(&self, n: usize) {
        until(|| async { self.turns.borrow().len() == n }).await;
    }
    /// A running turn on the one local workspace.
    async fn running(&self) -> (Rc<dyn SessionHandle>, String) {
        let ws = create_on(&self.fx.rt, CreateOn::named("mac").on("here"), None).await.expect("a workspace").view.id;
        let handle = self.fx.rt.sessions().start(&ws, start("run it"), None).await.expect("a start");
        self.turns_are(1).await;
        (handle, ws)
    }
    async fn access(&self, session: &str, mode: &str, origin: Option<Caller>) -> Value {
        j(&self.fx.rt.sessions().access(session, mode, origin).await.expect("an answer"))
    }
    async fn modes(&self, ws: &str) -> Vec<Value> {
        self.fx
            .rt
            .sessions()
            .list(Some(ws.to_string()), None)
            .await
            .expect("a list")
            .iter()
            .map(|s| j(s)["permissionMode"].clone())
            .collect()
    }
}

// "an access picked while a turn runs" > "reaches the turn in front of the person, and the row carries the mode its later turns resume at"
#[test]
fn reaches_the_turn_in_front_of_the_person_and_the_row_carries_the_mode_its_later_turns_resume_at() {
    drive(|| async {
        let h = held(Moves::Set);
        let (handle, ws) = h.running().await;
        assert_eq!(j(&handle.view())["permissionMode"], "bypassPermissions");
        assert_eq!(h.access(&handle.id(), "acceptEdits", None).await, json!({ "outcome": "set" }));
        assert_eq!(*h.turn(0).modes.borrow(), ["acceptEdits"]);
        // The row is what a resume reads its access off, so a turn moved mid-flight must not leave it saying the old one.
        assert_eq!(j(&handle.view())["permissionMode"], "acceptEdits");
        h.turn(0).reply();
        handle.finished().await;
        let thread = handle.view().thread_id.expect("a thread");
        let next = h.fx.rt.sessions().start(&ws, on_thread("again", &thread), None).await.expect("a start");
        h.turns_are(2).await;
        h.turn(1).reply();
        next.finished().await;
        assert_eq!(h.picks(), some(&["bypassPermissions", "acceptEdits"]));
        h.fx.rt.close().await.expect("a close");
    });
}

// "an access picked while a turn runs" > "a harness that takes no mode change mid-turn answers unsupported and the turn keeps the access it started at; the thread's next turn runs at the pick"
#[test]
fn a_harness_that_takes_no_mode_change_mid_turn_answers_unsupported_and_the_turn_keeps_the_access_it_started_at_the_threads_next_turn_runs_at_the_pick(
) {
    drive(|| async {
        let h = held(Moves::None);
        let (handle, ws) = h.running().await;
        assert_eq!(h.access(&handle.id(), "dontAsk", None).await, json!({ "outcome": "unsupported" }));
        assert_eq!(j(&handle.view())["permissionMode"], "bypassPermissions");
        h.turn(0).reply();
        handle.finished().await;
        // The pick landed on the thread's record all the same, which is where its next turn reads its access.
        let thread = handle.view().thread_id.expect("a thread");
        let next = h.fx.rt.sessions().start(&ws, on_thread("again", &thread), None).await.expect("a start");
        h.turns_are(2).await;
        h.turn(1).reply();
        next.finished().await;
        assert_eq!(h.picks(), some(&["bypassPermissions", "dontAsk"]));
        h.fx.rt.close().await.expect("a close");
    });
}

// "an access picked while a turn runs" > "a CLI that refuses the mode reads as unsupported too, rather than as a change that landed"
#[test]
fn a_cli_that_refuses_the_mode_reads_as_unsupported_too_rather_than_as_a_change_that_landed() {
    drive(|| async {
        let h = held(Moves::Refused);
        let (handle, _ws) = h.running().await;
        assert_eq!(h.access(&handle.id(), "dontAsk", None).await, json!({ "outcome": "unsupported" }));
        assert_eq!(*h.turn(0).modes.borrow(), ["dontAsk"]);
        assert_eq!(j(&handle.view())["permissionMode"], "bypassPermissions");
        h.turn(0).reply();
        handle.finished().await;
        // The turn kept its mode to its end; over, its row says the pick its thread's next turn runs at.
        assert_eq!(j(&handle.view())["permissionMode"], "dontAsk");
        h.fx.rt.close().await.expect("a close");
    });
}

// "an access picked while a turn runs" > "a pick the running turn did not take reaches its row as the turn ends, so no row says a mode the next turn will not run at"
#[test]
fn a_pick_the_running_turn_did_not_take_reaches_its_row_as_the_turn_ends_so_no_row_says_a_mode_the_next_turn_will_not_run_at() {
    drive(|| async {
        let h = held(Moves::None);
        let (handle, ws) = h.running().await;
        assert_eq!(h.access(&handle.id(), "dontAsk", None).await, json!({ "outcome": "unsupported" }));
        // While the turn runs, its row says what it is running at: the harness took no change.
        assert_eq!(j(&handle.view())["permissionMode"], "bypassPermissions");
        assert_eq!(h.modes(&ws).await, [json!("bypassPermissions")]);
        h.turn(0).reply();
        handle.finished().await;
        // Between turns every client folds the thread's access off this row, and the composer reads the row over the
        // transcript's start stamp, so the row says the mode the record holds and the next turn runs at.
        assert_eq!(j(&handle.view())["permissionMode"], "dontAsk");
        assert_eq!(h.modes(&ws).await, [json!("dontAsk")]);
        let thread = handle.view().thread_id.expect("a thread");
        let next = h.fx.rt.sessions().start(&ws, on_thread("again", &thread), None).await.expect("a start");
        h.turns_are(2).await;
        h.turn(1).reply();
        next.finished().await;
        assert_eq!(h.picks(), some(&["bypassPermissions", "dontAsk"]));
        h.fx.rt.close().await.expect("a close");
    });
}

// "an access picked while a turn runs" > "a thread of another tree on the workspace is told not-found, as the interrupt tells it, and moves nothing"
#[test]
fn a_thread_of_another_tree_on_the_workspace_is_told_not_found_as_the_interrupt_tells_it_and_moves_nothing() {
    drive(|| async {
        let h = held(Moves::Set);
        let (handle, ws) = h.running().await;
        h.turn(0).reply();
        handle.finished().await;
        // A second thread the person opened on the same workspace is its own root: the first thread is outside its tree.
        let beside = h.fx.rt.sessions().start(&ws, start("beside it"), None).await.expect("a start");
        h.turns_are(2).await;
        h.turn(1).reply();
        beside.finished().await;
        let beside_thread = beside.view().thread_id.expect("a thread");
        let scoped = scope_of(&beside_thread, &ws);
        // The same absence the sibling verbs answer, before the workspace is read, so a thread learns nothing of a row it may
        // not drive; and the one road that changes a thread's access is shut to it.
        let stopped = h.fx.rt.sessions().interrupt(&handle.id(), Some(scoped.clone()), None).await.expect("an answer");
        assert_eq!(j(&stopped), json!({ "outcome": "not-found" }));
        assert_eq!(h.access(&handle.id(), "dontAsk", Some(scoped.clone())).await, json!({ "outcome": "not-found" }));
        assert_eq!(h.modes(&ws).await, [json!("bypassPermissions"), json!("bypassPermissions")]);
        // A thread's own row is still its own to move.
        assert_eq!(h.access(&beside.id(), "dontAsk", Some(scoped)).await, json!({ "outcome": "set" }));
        // The first thread's next turn runs at what it ran at.
        let thread = handle.view().thread_id.expect("a thread");
        let next = h.fx.rt.sessions().start(&ws, on_thread("again", &thread), None).await.expect("a start");
        h.turns_are(3).await;
        h.turn(2).reply();
        next.finished().await;
        assert_eq!(h.picks(), some(&["bypassPermissions", "bypassPermissions", "bypassPermissions"]));
        h.fx.rt.close().await.expect("a close");
    });
}

// "an access picked while a turn runs" > "a mode the harness's own list does not carry is refused in the words a start refuses it with"
#[test]
fn a_mode_the_harnesss_own_list_does_not_carry_is_refused_in_the_words_a_start_refuses_it_with() {
    drive(|| async {
        let h = held(Moves::Set);
        let (handle, _ws) = h.running().await;
        let refused = h.fx.rt.sessions().access(&handle.id(), "sideways", None).await.err().expect("a refusal");
        assert!(refused.message.contains("not one claude takes"), "{}", refused.message);
        assert!(h.turn(0).modes.borrow().is_empty());
        h.turn(0).reply();
        handle.finished().await;
        h.fx.rt.close().await.expect("a close");
    });
}

// "an access picked while a turn runs" > "answers rather than throwing: an unknown session is not found, and a pick on a thread between turns is set on its record"
#[test]
fn answers_rather_than_throwing_an_unknown_session_is_not_found_and_a_pick_on_a_thread_between_turns_is_set_on_its_record() {
    drive(|| async {
        let h = held(Moves::Set);
        let (handle, ws) = h.running().await;
        assert_eq!(h.access("s_nope", "dontAsk", None).await, json!({ "outcome": "not-found" }));
        h.turn(0).reply();
        handle.finished().await;
        // No process is touched: the thread's record takes the mode, the row every client folds the access off says it too,
        // and the thread's next turn runs at it. This is the one road that changes a thread's access, since a send into a
        // thread names none.
        assert_eq!(h.access(&handle.id(), "dontAsk", None).await, json!({ "outcome": "set" }));
        assert!(h.turn(0).modes.borrow().is_empty());
        assert_eq!(h.modes(&ws).await, [json!("dontAsk")]);
        let thread = handle.view().thread_id.expect("a thread");
        let named = StartOptions { permission_mode: Some("acceptEdits".into()), ..on_thread("again", &thread) };
        let next = h.fx.rt.sessions().start(&ws, named, None).await.expect("a start");
        h.turns_are(2).await;
        h.turn(1).reply();
        next.finished().await;
        assert_eq!(h.picks(), some(&["bypassPermissions", "dontAsk"]));
        h.fx.rt.close().await.expect("a close");
    });
}
