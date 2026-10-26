// SPDX-License-Identifier: AGPL-3.0-only
//! What a caller does to a running Claude turn over its stdin channel, on a stream the test feeds by hand: the
//! permission prompt and its answers, a turn moved to another access mode, a subagent stopped by itself, and a
//! message steered while the agent writes its last words. Ports packages/adapter-claude/test/adapter-permissions.test.ts
//! and steer-hold.test.ts at a3b870a6e.

#[path = "common/claude_exec.rs"]
mod claude_exec;

use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::future::{FutureExt, LocalBoxFuture};
use futures::stream::{self, LocalBoxStream, StreamExt};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use wsp_adapters::claude::landmines::user_message_line;
use wsp_adapters::claude::turn::ClaudeSession;
use wsp_adapters::claude::{create_claude_adapter, AdapterDeps, AttachedSession};
use wsp_adapters::pending::QUESTION_TOOL;
use wsp_adapters::port::{
    AccessOutcome, AdapterAttachOptions, AdapterEvent, AnswerOutcome, DeltaKind, ExecFailure, ExecFailureKind, ExecOptions, ExecStream,
    ExecStreamFactory, HarnessStartOptions, PermissionAnswer, PermissionAsk, PermissionOutcome, SteerOutcome, TaskStop, TurnResult,
    TurnStatus, WriteOutcome, PERMISSION_ALLOW, PERMISSION_DENY,
};
use wsp_wire::wire::helpers::PermissionEffect;

use claude_exec::{collect, types, Call, Exit, FakeExec, Script, StreamOpts, RUN_HANDLE};

type Events = Arc<Mutex<Vec<AdapterEvent>>>;

fn snapshot(events: &Events) -> Vec<AdapterEvent> {
    events.lock().unwrap().clone()
}

/// A wait that panics rather than hang on a turn that never settles.
async fn bounded<F: Future>(f: F) -> F::Output {
    tokio::time::timeout(Duration::from_secs(5), f).await.expect("timed out after 5s")
}

/// vitest's `toMatchObject`: every key of `expected` is in `actual` and matches; arrays match element for element.
fn matches_object(actual: &Value, expected: &Value) -> bool {
    match (actual, expected) {
        (Value::Object(a), Value::Object(e)) => e.iter().all(|(k, v)| a.get(k).is_some_and(|x| matches_object(x, v))),
        (Value::Array(a), Value::Array(e)) => a.len() == e.len() && a.iter().zip(e).all(|(x, y)| matches_object(x, y)),
        _ => actual == expected,
    }
}

fn assert_matches_object(actual: &Value, expected: Value) {
    assert!(matches_object(actual, &expected), "expected {actual} to match object {expected}");
}

// ---------------------------------------------------------------------------------------------------------------
// adapter-permissions.test.ts
// ---------------------------------------------------------------------------------------------------------------

const SESSION: &str = "6bb3cdb1-97af-4ad0-96ff-35160ba2bd0b";
const ASK: &str = "d9aa99d3-be4e-4a2b-8766-1b9494cde4f6";
const NEXT_ASK: &str = "b2f1d8c0-11d2-4f23-9a55-2d6a4a3f0e77";

fn init_line() -> String {
    json!({ "type": "system", "subtype": "init", "session_id": SESSION, "model": "claude-sonnet-5", "cwd": "/root", "permissionMode": "default" }).to_string()
}

fn ask_line() -> String {
    json!({
        "type": "control_request",
        "request_id": ASK,
        "request": {
            "subtype": "can_use_tool",
            "tool_name": "Write",
            "input": { "file_path": "/root/out.txt", "content": "hi" },
            "description": "out.txt",
            "permission_suggestions": [{ "type": "setMode", "mode": "acceptEdits", "destination": "session" }],
            "tool_use_id": "toolu_1",
        },
    })
    .to_string()
}

fn next_ask_line() -> String {
    json!({
        "type": "control_request",
        "request_id": NEXT_ASK,
        "request": { "subtype": "can_use_tool", "tool_name": "Bash", "input": { "command": "cat out.txt" }, "description": "cat out.txt", "tool_use_id": "toolu_2" },
    })
    .to_string()
}

fn result_line() -> String {
    json!({ "type": "result", "subtype": "success", "session_id": SESSION, "result": "Done.", "usage": { "output_tokens": 3 }, "total_cost_usd": 0.01 }).to_string()
}

fn question_input() -> Value {
    json!({
        "questions": [
            {
                "question": "Tabs or spaces?",
                "header": "Indent",
                "options": [
                    { "label": "Tabs", "description": "Indent with tab characters." },
                    { "label": "Spaces", "description": "Indent with space characters." },
                ],
                "multiSelect": false,
            },
        ],
    })
}

/// The shape measured on the harness's own channel: a question carries requires_user_interaction and is answered by
/// the input it hands back, never by an allow.
fn question_ask_line() -> String {
    json!({
        "type": "control_request",
        "request_id": ASK,
        "request": { "subtype": "can_use_tool", "tool_name": QUESTION_TOOL, "input": question_input(), "tool_use_id": "toolu_q", "requires_user_interaction": true },
    })
    .to_string()
}

/// The question's ask line under another request id.
fn next_question_ask_line() -> String {
    let mut line = json::<Value>(&question_ask_line());
    line["request_id"] = json!(NEXT_ASK);
    line.to_string()
}

fn json<T: serde::de::DeserializeOwned>(line: &str) -> T {
    serde_json::from_str(line).unwrap_or_else(|e| panic!("not JSON ({e}): {line}"))
}

fn fixture_frames() -> Vec<Value> {
    claude_exec::fixture_lines("subagent-stream.jsonl").iter().map(|l| json::<Value>(l)).collect()
}

fn fixture_frame(holds: impl Fn(&Value) -> bool) -> String {
    let lines = claude_exec::fixture_lines("subagent-stream.jsonl");
    lines.into_iter().find(|l| holds(&json::<Value>(l))).expect("no such fixture frame")
}

fn agent_a() -> (String, String) {
    let frames = fixture_frames();
    let launch = frames.iter().find(|e| e["type"] == "system" && e["subtype"] == "task_started").expect("no task_started");
    (launch["tool_use_id"].as_str().unwrap().to_string(), launch["task_id"].as_str().unwrap().to_string())
}

/// A stream the test feeds: `push` sends the CLI's next line, `end` exits the process, and `stdin` is every line the
/// adapter wrote back, the launch's seed first. `fail_next` makes the next write throw; `gone` makes the process
/// refuse writes. Teardown and kill do nothing, as the TS `driven()`'s do.
struct Inner {
    stdin: Mutex<Vec<String>>,
    feed: Mutex<Option<mpsc::UnboundedSender<Result<String, ExecFailure>>>>,
    rx: Mutex<Option<mpsc::UnboundedReceiver<Result<String, ExecFailure>>>>,
    exit: Exit,
    writable: AtomicBool,
    failing: Mutex<Option<ExecFailure>>,
}

impl ExecStream for Inner {
    fn lines(&self) -> LocalBoxStream<'static, Result<String, ExecFailure>> {
        match self.rx.lock().unwrap().take() {
            Some(mut rx) => stream::poll_fn(move |cx| rx.poll_recv(cx)).boxed_local(),
            None => stream::empty().boxed_local(),
        }
    }
    fn teardown(&self) {}
    fn kill(&self) {}
    fn write(&self, line: String) -> LocalBoxFuture<'static, Result<WriteOutcome, ExecFailure>> {
        let outcome = if let Some(e) = self.failing.lock().unwrap().take() {
            Err(e)
        } else if !self.writable.load(Ordering::SeqCst) {
            Ok(WriteOutcome::Gone)
        } else {
            self.stdin.lock().unwrap().push(line);
            Ok(WriteOutcome::Written)
        };
        async move { outcome }.boxed_local()
    }
    fn close_input(&self) {}
    fn exited(&self) -> wsp_adapters::port::SharedFuture<Option<i32>> {
        self.exit.future()
    }
}

struct Factory(Arc<Inner>);

impl ExecStreamFactory for Factory {
    fn launch(&self, _command: &str, options: ExecOptions) -> Arc<dyn ExecStream> {
        // The launch seeds the channel with its user message, as the real factory does; the answer follows it.
        self.0.stdin.lock().unwrap().extend(options.input.unwrap_or_default());
        self.0.clone()
    }
}

#[derive(Clone)]
struct Io(Arc<Inner>);

impl Io {
    fn new() -> (Io, Arc<dyn ExecStreamFactory>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let inner = Arc::new(Inner {
            stdin: Default::default(),
            feed: Mutex::new(Some(tx)),
            rx: Mutex::new(Some(rx)),
            exit: Exit::new(),
            writable: AtomicBool::new(true),
            failing: Default::default(),
        });
        (Io(inner.clone()), Arc::new(Factory(inner)))
    }
    fn push(&self, line: impl Into<String>) {
        if let Some(tx) = self.0.feed.lock().unwrap().as_ref() {
            let _ = tx.send(Ok(line.into()));
        }
    }
    fn end(&self, code: Option<i32>) {
        self.0.feed.lock().unwrap().take();
        self.0.exit.resolve(code);
    }
    fn gone(&self) {
        self.0.writable.store(false, Ordering::SeqCst);
    }
    fn fail_next(&self, e: ExecFailure) {
        *self.0.failing.lock().unwrap() = Some(e);
    }
    fn stdin(&self) -> Vec<String> {
        self.0.stdin.lock().unwrap().clone()
    }
    /// The nth line of stdin as JSON.
    fn sent(&self, n: usize) -> Value {
        let all = self.stdin();
        json(all.get(n).unwrap_or_else(|| panic!("stdin has {} lines, wanted line {n}", all.len())))
    }
}

fn start() -> (Events, ClaudeSession, Io) {
    let (io, factory) = Io::new();
    let adapter = create_claude_adapter(AdapterDeps::new(factory, "/root/.claude-cfg")).unwrap();
    let (events, on_event) = collect();
    let session = adapter
        .start(HarnessStartOptions {
            prompt: "write it".into(),
            permission_mode: Some("default".into()),
            on_event: Some(on_event),
            ..Default::default()
        })
        .unwrap();
    (events, session, io)
}

/// Waits for the adapter's reader to drain what it was fed.
async fn settle() {
    tokio::time::sleep(Duration::from_millis(20)).await;
}

fn asks(events: &Events) -> Vec<AdapterEvent> {
    snapshot(events).into_iter().filter(|e| matches!(e, AdapterEvent::PermissionAsk { .. })).collect()
}

fn ask_bodies(events: &Events) -> Vec<PermissionAsk> {
    snapshot(events).into_iter().filter_map(|e| if let AdapterEvent::PermissionAsk { ask, .. } = e { Some(ask) } else { None }).collect()
}

fn closes(events: &Events) -> Vec<AdapterEvent> {
    snapshot(events).into_iter().filter(|e| matches!(e, AdapterEvent::PermissionClose { .. })).collect()
}

fn closed(ask_id: &str, outcome: PermissionOutcome, option_id: Option<&str>) -> AdapterEvent {
    AdapterEvent::PermissionClose { session_id: SESSION.into(), ask_id: ask_id.into(), outcome, option_id: option_id.map(String::from) }
}

fn answer_with(option_id: &str, outcome: PermissionOutcome, deny_message: &str) -> PermissionAnswer {
    PermissionAnswer { option_id: option_id.into(), outcome, deny_message: deny_message.into(), reason: None }
}

fn allow_unused() -> PermissionAnswer {
    answer_with(PERMISSION_ALLOW, PermissionOutcome::Allowed, "unused")
}

async fn finish(session: &ClaudeSession, io: &Io) {
    io.push(result_line());
    io.end(Some(0));
    bounded(session.finished()).await;
}

fn set_access(session: &ClaudeSession, mode: &'static str) -> JoinHandle<Result<AccessOutcome, ExecFailure>> {
    let session = session.clone();
    tokio::task::spawn_local(async move { session.set_access(mode).await })
}

fn stop_task(session: &ClaudeSession, task: &'static str) -> JoinHandle<Result<TaskStop, ExecFailure>> {
    let session = session.clone();
    tokio::task::spawn_local(async move { session.stop_task(task).await })
}

/// The CLI's control_response to the request on `line`, an error where `error` is given.
fn answered(line: &Value, error: Option<&str>) -> String {
    let request_id = line["request_id"].clone();
    match error {
        None => json!({ "type": "control_response", "response": { "subtype": "success", "request_id": request_id } }),
        Some(e) => json!({ "type": "control_response", "response": { "subtype": "error", "request_id": request_id, "error": e } }),
    }
    .to_string()
}

// TS: adapter-permissions.test.ts:152
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn relays_the_prompt_writes_the_answer_down_the_cli_channel_and_closes_the_row_once() {
    tokio::task::LocalSet::new().run_until(relays_the_prompt_writes_the_answer_down_the_cli_channel_and_closes_the_row_once_body()).await
}

async fn relays_the_prompt_writes_the_answer_down_the_cli_channel_and_closes_the_row_once_body() {
    let (events, session, io) = start();
    io.push(init_line());
    io.push(ask_line());
    settle().await;

    let relayed = asks(&events);
    assert_eq!(relayed.len(), 1);
    let AdapterEvent::PermissionAsk { session_id, ask } = &relayed[0] else { unreachable!() };
    assert_eq!(session_id, SESSION);
    assert_eq!(ask.ask_id, ASK);
    assert_eq!(ask.tool_name, "Write");
    assert_eq!(ask.detail.as_deref(), Some("out.txt"));
    assert_eq!(ask.tool_use_id.as_deref(), Some("toolu_1"));
    assert_eq!(closes(&events).len(), 0);
    // The launch's own user message is the first line on the channel; the answer is what follows it.
    assert_eq!(io.stdin().len(), 1);

    assert_eq!(session.answer(ASK, allow_unused()).await.unwrap(), AnswerOutcome::Answered);
    assert_matches_object(
        &io.sent(1),
        json!({ "type": "control_response", "response": { "subtype": "success", "request_id": ASK, "response": { "behavior": "allow" } } }),
    );
    assert_eq!(closes(&events), vec![closed(ASK, PermissionOutcome::Allowed, Some(PERMISSION_ALLOW))]);

    // A second answer has nothing to answer: the row is closed and the CLI would ignore the line.
    assert_eq!(session.answer(ASK, answer_with(PERMISSION_DENY, PermissionOutcome::Denied, "no")).await.unwrap(), AnswerOutcome::Gone);
    assert_eq!(io.stdin().len(), 2);
    assert_eq!(closes(&events).len(), 1);

    finish(&session, &io).await;
}

// TS: adapter-permissions.test.ts:179
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_answer_whose_write_failed_leaves_the_prompt_open_so_answering_again_lands_it() {
    tokio::task::LocalSet::new().run_until(an_answer_whose_write_failed_leaves_the_prompt_open_so_answering_again_lands_it_body()).await
}

async fn an_answer_whose_write_failed_leaves_the_prompt_open_so_answering_again_lands_it_body() {
    let (events, session, io) = start();
    io.push(init_line());
    io.push(ask_line());
    settle().await;
    let refused = ExecFailure { kind: ExecFailureKind::Other, message: "Boat is refusing requests from this computer's address".into() };
    io.fail_next(refused.clone());
    assert_eq!(session.answer(ASK, allow_unused()).await.unwrap_err(), refused);
    assert_eq!(io.stdin().len(), 1);
    assert_eq!(closes(&events).len(), 0);

    assert_eq!(session.answer(ASK, allow_unused()).await.unwrap(), AnswerOutcome::Answered);
    assert_eq!(io.stdin().len(), 2);
    assert_matches_object(
        &io.sent(1),
        json!({ "type": "control_response", "response": { "subtype": "success", "request_id": ASK, "response": { "behavior": "allow" } } }),
    );
    assert_eq!(closes(&events), vec![closed(ASK, PermissionOutcome::Allowed, Some(PERMISSION_ALLOW))]);

    finish(&session, &io).await;
}

// TS: adapter-permissions.test.ts:200
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn puts_a_questions_own_choices_on_the_prompt_and_sends_the_pick_as_the_calls_answer_with_no_allow_in_front() {
    tokio::task::LocalSet::new()
        .run_until(puts_a_questions_own_choices_on_the_prompt_and_sends_the_pick_as_the_calls_answer_with_no_allow_in_front_body())
        .await
}

async fn puts_a_questions_own_choices_on_the_prompt_and_sends_the_pick_as_the_calls_answer_with_no_allow_in_front_body() {
    let (events, session, io) = start();
    io.push(init_line());
    io.push(question_ask_line());
    settle().await;

    let relayed = ask_bodies(&events);
    assert_eq!(relayed.len(), 1);
    let options = relayed[0].options.clone();
    // Two choices, two buttons, and neither an allow nor a deny: there is no consent in a question to give.
    assert_eq!(options.iter().map(|o| o.label.as_str()).collect::<Vec<_>>(), vec!["Tabs", "Spaces"]);
    assert_eq!(options.iter().map(|o| o.effect).collect::<Vec<_>>(), vec![PermissionEffect::Answer, PermissionEffect::Answer]);
    assert!(!options.iter().any(|o| o.id == PERMISSION_ALLOW || o.id == PERMISSION_DENY));

    assert_eq!(
        session.answer(ASK, answer_with(&options[1].id, PermissionOutcome::Allowed, "unused")).await.unwrap(),
        AnswerOutcome::Answered
    );
    let answer = io.sent(1);
    // The harness reads the pick off the input it gets back, keyed by the question's own text.
    assert_matches_object(
        &answer["response"]["response"],
        json!({ "behavior": "allow", "updatedInput": { "answers": { "Tabs or spaces?": "Spaces" } } }),
    );
    assert_eq!(answer["response"]["response"]["updatedInput"]["questions"], question_input()["questions"]);
    assert_ne!(answer["response"]["response"]["behavior"], json!("deny"));

    finish(&session, &io).await;
}

// TS: adapter-permissions.test.ts:226
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn names_the_subagent_a_prompt_came_from_by_the_call_that_launched_it() {
    tokio::task::LocalSet::new().run_until(names_the_subagent_a_prompt_came_from_by_the_call_that_launched_it_body()).await
}

async fn names_the_subagent_a_prompt_came_from_by_the_call_that_launched_it_body() {
    let (call, agent) = agent_a();
    // Both frames come off the recorded fixture: the CLI binds its own handle for a subagent to the launching call
    // once, in task_started, and a prompt its run raises names only that handle.
    let launch_frame = fixture_frame(|e| e["subtype"] == "task_started" && e["tool_use_id"] == json!(call));
    let child_ask_frame = fixture_frame(|e| e["request"]["agent_id"] == json!(agent));
    let (events, session, io) = start();
    io.push(init_line());
    io.push(launch_frame);
    io.push(child_ask_frame);
    io.push(ask_line());
    settle().await;

    let relayed = ask_bodies(&events);
    assert_eq!(relayed.iter().map(|a| a.parent_tool_use_id.clone()).collect::<Vec<_>>(), vec![Some(call), None]);
    // The launch itself is the CLI's own bookkeeping and is no line of the turn's.
    assert_eq!(types(&snapshot(&events)).iter().filter(|t| **t == "turn.delta").count(), 0);

    finish(&session, &io).await;
}

// TS: adapter-permissions.test.ts:246
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_deny_carries_the_words_the_agent_reads_as_the_calls_result() {
    tokio::task::LocalSet::new().run_until(a_deny_carries_the_words_the_agent_reads_as_the_calls_result_body()).await
}

async fn a_deny_carries_the_words_the_agent_reads_as_the_calls_result_body() {
    let (events, session, io) = start();
    io.push(init_line());
    io.push(ask_line());
    settle().await;
    let denied = answer_with(PERMISSION_DENY, PermissionOutcome::Denied, "the person denied this in the chat");
    assert_eq!(session.answer(ASK, denied).await.unwrap(), AnswerOutcome::Answered);
    assert_eq!(io.sent(1)["response"]["response"], json!({ "behavior": "deny", "message": "the person denied this in the chat" }));
    assert_eq!(closes(&events), vec![closed(ASK, PermissionOutcome::Denied, Some(PERMISSION_DENY))]);
    finish(&session, &io).await;
}

// TS: adapter-permissions.test.ts:259
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_runtimes_own_answer_for_a_prompt_nobody_came_to_closes_the_row_as_unanswered() {
    tokio::task::LocalSet::new().run_until(the_runtimes_own_answer_for_a_prompt_nobody_came_to_closes_the_row_as_unanswered_body()).await
}

async fn the_runtimes_own_answer_for_a_prompt_nobody_came_to_closes_the_row_as_unanswered_body() {
    let (events, session, io) = start();
    io.push(init_line());
    io.push(ask_line());
    settle().await;
    let unanswered = answer_with(PERMISSION_DENY, PermissionOutcome::Unanswered, "nobody answered");
    assert_eq!(session.answer(ASK, unanswered).await.unwrap(), AnswerOutcome::Answered);
    assert_eq!(closes(&events), vec![closed(ASK, PermissionOutcome::Unanswered, Some(PERMISSION_DENY))]);
    finish(&session, &io).await;
}

// TS: adapter-permissions.test.ts:271
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_prompt_the_cli_withdraws_closes_as_cancelled_and_answering_it_afterwards_answers_nothing() {
    tokio::task::LocalSet::new()
        .run_until(a_prompt_the_cli_withdraws_closes_as_cancelled_and_answering_it_afterwards_answers_nothing_body())
        .await
}

async fn a_prompt_the_cli_withdraws_closes_as_cancelled_and_answering_it_afterwards_answers_nothing_body() {
    let (events, session, io) = start();
    io.push(init_line());
    io.push(ask_line());
    io.push(json!({ "type": "control_cancel_request", "request_id": ASK }).to_string());
    settle().await;
    assert_eq!(closes(&events), vec![closed(ASK, PermissionOutcome::Cancelled, None)]);
    assert_eq!(session.answer(ASK, allow_unused()).await.unwrap(), AnswerOutcome::Gone);
    finish(&session, &io).await;
}

// TS: adapter-permissions.test.ts:284
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_prompt_whose_process_ends_under_it_closes_as_cancelled_rather_than_waiting_for_an_answer() {
    tokio::task::LocalSet::new()
        .run_until(a_prompt_whose_process_ends_under_it_closes_as_cancelled_rather_than_waiting_for_an_answer_body())
        .await
}

async fn a_prompt_whose_process_ends_under_it_closes_as_cancelled_rather_than_waiting_for_an_answer_body() {
    let (events, session, io) = start();
    io.push(init_line());
    io.push(ask_line());
    settle().await;
    assert_eq!(closes(&events).len(), 0);
    io.gone();
    io.end(None);
    bounded(session.finished()).await;
    assert_eq!(closes(&events), vec![closed(ASK, PermissionOutcome::Cancelled, None)]);
    assert_eq!(session.answer(ASK, allow_unused()).await.unwrap(), AnswerOutcome::Gone);
}

// TS: adapter-permissions.test.ts:297
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn refuses_an_option_the_prompt_never_offered_without_writing_anything_to_the_cli() {
    tokio::task::LocalSet::new().run_until(refuses_an_option_the_prompt_never_offered_without_writing_anything_to_the_cli_body()).await
}

async fn refuses_an_option_the_prompt_never_offered_without_writing_anything_to_the_cli_body() {
    let (_events, session, io) = start();
    io.push(init_line());
    io.push(ask_line());
    settle().await;
    let refused = session.answer(ASK, answer_with("mode:bypassPermissions", PermissionOutcome::Allowed, "unused")).await.unwrap_err();
    assert!(refused.message.contains("not an option"), "{}", refused.message);
    assert_eq!(io.stdin().len(), 1);
    finish(&session, &io).await;
}

// TS: adapter-permissions.test.ts:309
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_control_request_of_another_subtype_is_refused_down_the_channel_so_the_cli_stops_waiting_on_it() {
    tokio::task::LocalSet::new()
        .run_until(a_control_request_of_another_subtype_is_refused_down_the_channel_so_the_cli_stops_waiting_on_it_body())
        .await
}

async fn a_control_request_of_another_subtype_is_refused_down_the_channel_so_the_cli_stops_waiting_on_it_body() {
    let (events, session, io) = start();
    io.push(init_line());
    io.push(json!({ "type": "control_request", "request_id": "req_h", "request": { "subtype": "hook_callback" } }).to_string());
    settle().await;
    assert_eq!(asks(&events).len(), 0);
    assert_eq!(
        io.sent(1),
        json!({ "type": "control_response", "response": { "subtype": "error", "request_id": "req_h", "error": "wsp answers no hook_callback control request" } })
    );
    finish(&session, &io).await;
}

// TS: adapter-permissions.test.ts:329
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn writes_the_clis_own_set_permission_mode_down_the_channel_and_settles_once_the_cli_answers_it() {
    tokio::task::LocalSet::new()
        .run_until(writes_the_clis_own_set_permission_mode_down_the_channel_and_settles_once_the_cli_answers_it_body())
        .await
}

async fn writes_the_clis_own_set_permission_mode_down_the_channel_and_settles_once_the_cli_answers_it_body() {
    let (_events, session, io) = start();
    io.push(init_line());
    settle().await;
    let moved = set_access(&session, "bypassPermissions");
    settle().await;
    assert_matches_object(
        &io.sent(1),
        json!({ "type": "control_request", "request": { "subtype": "set_permission_mode", "mode": "bypassPermissions" } }),
    );
    io.push(answered(&io.sent(1), None));
    assert_eq!(bounded(moved).await.unwrap().unwrap(), AccessOutcome::Set);
    finish(&session, &io).await;
}

// TS: adapter-permissions.test.ts:343
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_mode_the_cli_will_not_take_comes_back_refused_rather_than_reading_as_applied() {
    tokio::task::LocalSet::new().run_until(a_mode_the_cli_will_not_take_comes_back_refused_rather_than_reading_as_applied_body()).await
}

async fn a_mode_the_cli_will_not_take_comes_back_refused_rather_than_reading_as_applied_body() {
    let (_events, session, io) = start();
    io.push(init_line());
    settle().await;
    let moved = set_access(&session, "sideways");
    settle().await;
    io.push(answered(&io.sent(1), Some("invalid permission mode")));
    assert_eq!(bounded(moved).await.unwrap().unwrap(), AccessOutcome::Refused);
    finish(&session, &io).await;
}

// TS: adapter-permissions.test.ts:356
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_mode_that_asks_nobody_answers_the_prompt_the_turn_is_stopped_on_and_every_one_it_raises_after() {
    tokio::task::LocalSet::new()
        .run_until(the_mode_that_asks_nobody_answers_the_prompt_the_turn_is_stopped_on_and_every_one_it_raises_after_body())
        .await
}

async fn the_mode_that_asks_nobody_answers_the_prompt_the_turn_is_stopped_on_and_every_one_it_raises_after_body() {
    let (events, session, io) = start();
    io.push(init_line());
    io.push(ask_line());
    settle().await;
    assert_eq!(asks(&events).len(), 1);

    let moved = set_access(&session, "bypassPermissions");
    settle().await;
    // The request goes down the channel all the same, and this CLI refuses it: the mode is a launch flag on it.
    assert_matches_object(
        &io.sent(1),
        json!({ "type": "control_request", "request": { "subtype": "set_permission_mode", "mode": "bypassPermissions" } }),
    );
    io.push(answered(&io.sent(1), Some("the session was not launched with --dangerously-skip-permissions")));
    // The prompt in front of the person is allowed by this host, so the turn goes on without a click.
    assert_eq!(bounded(moved).await.unwrap().unwrap(), AccessOutcome::Set);
    assert_matches_object(
        &io.sent(2),
        json!({ "type": "control_response", "response": { "request_id": ASK, "response": { "behavior": "allow" } } }),
    );
    assert_eq!(closes(&events), vec![closed(ASK, PermissionOutcome::Allowed, Some(PERMISSION_ALLOW))]);

    io.push(next_ask_line());
    settle().await;
    // The next call is allowed the same way and no prompt is put to anyone.
    assert_eq!(asks(&events).len(), 1);
    assert_matches_object(
        &io.sent(3),
        json!({ "type": "control_response", "response": { "request_id": NEXT_ASK, "response": { "behavior": "allow" } } }),
    );

    finish(&session, &io).await;
}

// TS: adapter-permissions.test.ts:384
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_mode_that_asks_nobody_never_answers_a_question_one_open_stays_with_the_person_and_one_raised_after_reaches_them() {
    tokio::task::LocalSet::new()
        .run_until(
            the_mode_that_asks_nobody_never_answers_a_question_one_open_stays_with_the_person_and_one_raised_after_reaches_them_body(),
        )
        .await
}

async fn the_mode_that_asks_nobody_never_answers_a_question_one_open_stays_with_the_person_and_one_raised_after_reaches_them_body() {
    let (events, session, io) = start();
    io.push(init_line());
    io.push(question_ask_line());
    settle().await;
    assert_eq!(asks(&events).len(), 1);
    let moved = set_access(&session, "bypassPermissions");
    settle().await;
    io.push(answered(&io.sent(1), Some("the session was not launched with --dangerously-skip-permissions")));
    assert_eq!(bounded(moved).await.unwrap().unwrap(), AccessOutcome::Set);
    // Nothing answered the open question: only the set_permission_mode request went down the channel.
    assert_eq!(io.stdin().len(), 2);
    assert_eq!(closes(&events), Vec::<AdapterEvent>::new());
    io.push(next_question_ask_line());
    settle().await;
    let relayed = ask_bodies(&events);
    assert_eq!(relayed.len(), 2);
    assert_eq!(relayed.iter().map(|a| a.ask_id.as_str()).collect::<Vec<_>>(), vec![ASK, NEXT_ASK]);
    assert_eq!(io.stdin().len(), 2);
    finish(&session, &io).await;
}

// TS: adapter-permissions.test.ts:406
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_turn_launched_at_bypass_allows_a_tools_prompt_itself_and_leaves_a_question_with_the_person() {
    tokio::task::LocalSet::new()
        .run_until(a_turn_launched_at_bypass_allows_a_tools_prompt_itself_and_leaves_a_question_with_the_person_body())
        .await
}

async fn a_turn_launched_at_bypass_allows_a_tools_prompt_itself_and_leaves_a_question_with_the_person_body() {
    let (events, session, io) = start();
    let mut init = json::<Value>(&init_line());
    init["permissionMode"] = json!("bypassPermissions");
    io.push(init.to_string());
    io.push(ask_line());
    settle().await;
    assert_eq!(asks(&events), Vec::<AdapterEvent>::new());
    assert_eq!(io.stdin().len(), 2);
    assert_matches_object(
        &io.sent(1),
        json!({ "type": "control_response", "response": { "subtype": "success", "request_id": ASK, "response": { "behavior": "allow" } } }),
    );
    io.push(next_question_ask_line());
    settle().await;
    let relayed = ask_bodies(&events);
    assert_eq!(relayed.len(), 1);
    assert_eq!(relayed[0].ask_id, NEXT_ASK);
    assert_eq!(relayed[0].tool_name, QUESTION_TOOL);
    assert_eq!(io.stdin().len(), 2);
    finish(&session, &io).await;
}

// TS: adapter-permissions.test.ts:423
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_mode_the_cli_offered_on_the_open_prompt_answers_it_as_that_option_and_one_it_did_not_leaves_it_for_the_person() {
    tokio::task::LocalSet::new()
        .run_until(a_mode_the_cli_offered_on_the_open_prompt_answers_it_as_that_option_and_one_it_did_not_leaves_it_for_the_person_body())
        .await
}

async fn a_mode_the_cli_offered_on_the_open_prompt_answers_it_as_that_option_and_one_it_did_not_leaves_it_for_the_person_body() {
    let (events, session, io) = start();
    io.push(init_line());
    io.push(ask_line());
    settle().await;

    // The prompt is a write and the CLI suggested accept edits for it, so the pick allows this call and stops the
    // asking for the rest of the session in one answer.
    let edits = set_access(&session, "acceptEdits");
    settle().await;
    io.push(answered(&io.sent(1), None));
    assert_eq!(bounded(edits).await.unwrap().unwrap(), AccessOutcome::Set);
    assert_matches_object(
        &io.sent(2)["response"]["response"],
        json!({ "behavior": "allow", "updatedPermissions": [{ "type": "setMode", "mode": "acceptEdits", "destination": "session" }] }),
    );
    assert_eq!(closes(&events), vec![closed(ASK, PermissionOutcome::Allowed, Some("mode:acceptEdits"))]);

    finish(&session, &io).await;
}

// TS: adapter-permissions.test.ts:443
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_mode_the_open_prompt_carries_no_option_for_moves_the_turn_and_leaves_that_prompt_standing() {
    tokio::task::LocalSet::new()
        .run_until(a_mode_the_open_prompt_carries_no_option_for_moves_the_turn_and_leaves_that_prompt_standing_body())
        .await
}

async fn a_mode_the_open_prompt_carries_no_option_for_moves_the_turn_and_leaves_that_prompt_standing_body() {
    let (events, session, io) = start();
    io.push(init_line());
    io.push(ask_line());
    settle().await;
    let moved = set_access(&session, "manual");
    settle().await;
    io.push(answered(&io.sent(1), None));
    assert_eq!(bounded(moved).await.unwrap().unwrap(), AccessOutcome::Set);
    // Nothing was answered for the person: manual says nothing about the write in front of them.
    assert_eq!(io.stdin().len(), 2);
    assert_eq!(closes(&events).len(), 0);
    assert_eq!(session.answer(ASK, allow_unused()).await.unwrap(), AnswerOutcome::Answered);
    assert_eq!(closes(&events), vec![closed(ASK, PermissionOutcome::Allowed, Some(PERMISSION_ALLOW))]);
    finish(&session, &io).await;
}

// TS: adapter-permissions.test.ts:462
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_turn_that_has_not_announced_itself_and_one_whose_channel_is_shut_take_nothing_and_write_nothing() {
    tokio::task::LocalSet::new()
        .run_until(a_turn_that_has_not_announced_itself_and_one_whose_channel_is_shut_take_nothing_and_write_nothing_body())
        .await
}

async fn a_turn_that_has_not_announced_itself_and_one_whose_channel_is_shut_take_nothing_and_write_nothing_body() {
    let (_events, session, io) = start();
    assert_eq!(session.set_access("manual").await.unwrap(), AccessOutcome::Gone);
    assert_eq!(io.stdin().len(), 1);
    io.push(init_line());
    settle().await;
    io.gone();
    assert_eq!(session.set_access("manual").await.unwrap(), AccessOutcome::Gone);
    assert_eq!(io.stdin().len(), 1);
    io.end(None);
    bounded(session.finished()).await;
}

// TS: adapter-permissions.test.ts:475
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_turn_that_replies_with_the_request_open_settles_it_at_the_result_not_minutes_later_when_the_process_goes() {
    tokio::task::LocalSet::new()
        .run_until(a_turn_that_replies_with_the_request_open_settles_it_at_the_result_not_minutes_later_when_the_process_goes_body())
        .await
}

async fn a_turn_that_replies_with_the_request_open_settles_it_at_the_result_not_minutes_later_when_the_process_goes_body() {
    let (_events, session, io) = start();
    io.push(init_line());
    settle().await;
    let moved = set_access(&session, "manual");
    settle().await;
    assert_eq!(io.stdin().len(), 2);
    // The result closes the channel, so nothing can answer this any more; the caller hears at once rather than
    // waiting on a CLI that lingers past its reply.
    io.push(result_line());
    assert_eq!(bounded(moved).await.unwrap().unwrap(), AccessOutcome::Gone);
    io.end(Some(0));
    bounded(session.finished()).await;
}

// TS: adapter-permissions.test.ts:490
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_process_that_dies_with_the_request_open_settles_it_as_gone_rather_than_leaving_the_caller_waiting() {
    tokio::task::LocalSet::new()
        .run_until(a_process_that_dies_with_the_request_open_settles_it_as_gone_rather_than_leaving_the_caller_waiting_body())
        .await
}

async fn a_process_that_dies_with_the_request_open_settles_it_as_gone_rather_than_leaving_the_caller_waiting_body() {
    let (_events, session, io) = start();
    io.push(init_line());
    settle().await;
    let moved = set_access(&session, "manual");
    settle().await;
    assert_eq!(io.stdin().len(), 2);
    io.end(None);
    assert_eq!(bounded(moved).await.unwrap().unwrap(), AccessOutcome::Gone);
    bounded(session.finished()).await;
}

fn launched(task: &str, call: &str) -> String {
    json!({ "type": "system", "subtype": "task_started", "task_id": task, "tool_use_id": call, "description": "count", "task_type": "local_agent", "spawn_depth": 1, "session_id": SESSION }).to_string()
}

/// The fan-out turn the stop_task cases share: init and two launched subagents, a1 and b2, fed and not yet settled.
fn begin(stop_task_wait_ms: Option<u64>) -> (Events, ClaudeSession, Io) {
    let (io, factory) = Io::new();
    let mut deps = AdapterDeps::new(factory, "/root/.claude-cfg");
    deps.stop_task_wait_ms = stop_task_wait_ms;
    let adapter = create_claude_adapter(deps).unwrap();
    let (events, on_event) = collect();
    let session = adapter.start(HarnessStartOptions { prompt: "fan out".into(), on_event: Some(on_event), ..Default::default() }).unwrap();
    io.push(init_line());
    io.push(launched("a1", "toolu_a"));
    io.push(launched("b2", "toolu_b"));
    (events, session, io)
}

// TS: adapter-permissions.test.ts:527
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn writes_the_clis_own_stop_task_for_that_subagent_alone_and_settles_accepted_on_the_clis_answer() {
    tokio::task::LocalSet::new()
        .run_until(writes_the_clis_own_stop_task_for_that_subagent_alone_and_settles_accepted_on_the_clis_answer_body())
        .await
}

async fn writes_the_clis_own_stop_task_for_that_subagent_alone_and_settles_accepted_on_the_clis_answer_body() {
    let (_events, session, io) = begin(None);
    settle().await;
    let stopped = stop_task(&session, "a1");
    settle().await;
    assert_eq!(
        io.stdin()[1],
        json!({ "type": "control_request", "request_id": "wsp-stop-task-1", "request": { "subtype": "stop_task", "task_id": "a1" } })
            .to_string()
    );
    assert_eq!(io.stdin().len(), 2);
    io.push(answered(&io.sent(1), None));
    assert_eq!(bounded(stopped).await.unwrap().unwrap(), TaskStop::Accepted);
    finish(&session, &io).await;
}

// TS: adapter-permissions.test.ts:539
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_task_the_cli_does_not_hold_comes_back_refused_in_the_clis_own_words() {
    tokio::task::LocalSet::new().run_until(a_task_the_cli_does_not_hold_comes_back_refused_in_the_clis_own_words_body()).await
}

async fn a_task_the_cli_does_not_hold_comes_back_refused_in_the_clis_own_words_body() {
    let (_events, session, io) = begin(None);
    settle().await;
    let stopped = stop_task(&session, "zz9");
    settle().await;
    io.push(answered(&io.sent(1), Some("No task found with ID: zz9")));
    assert_eq!(bounded(stopped).await.unwrap().unwrap(), TaskStop::Refused { error: "No task found with ID: zz9".into() });
    finish(&session, &io).await;
}

// TS: adapter-permissions.test.ts:549
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_cli_with_no_stop_for_this_sessions_tasks_reads_as_unsupported_the_answer_an_agent_with_no_such_request_gives() {
    tokio::task::LocalSet::new()
        .run_until(a_cli_with_no_stop_for_this_sessions_tasks_reads_as_unsupported_the_answer_an_agent_with_no_such_request_gives_body())
        .await
}

async fn a_cli_with_no_stop_for_this_sessions_tasks_reads_as_unsupported_the_answer_an_agent_with_no_such_request_gives_body() {
    let (_events, session, io) = begin(None);
    settle().await;
    let stopped = stop_task(&session, "a1");
    settle().await;
    io.push(answered(&io.sent(1), Some("stop_task is not supported in this context (callback not registered)")));
    assert_eq!(bounded(stopped).await.unwrap().unwrap(), TaskStop::Unsupported);
    finish(&session, &io).await;
}

// TS: adapter-permissions.test.ts:559
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_cli_that_answers_nothing_inside_the_wait_comes_back_refused_rather_than_holding_the_caller() {
    tokio::task::LocalSet::new()
        .run_until(a_cli_that_answers_nothing_inside_the_wait_comes_back_refused_rather_than_holding_the_caller_body())
        .await
}

async fn a_cli_that_answers_nothing_inside_the_wait_comes_back_refused_rather_than_holding_the_caller_body() {
    let (_events, session, io) = begin(Some(20));
    settle().await;
    let stopped = bounded(session.stop_task("a1")).await.unwrap();
    assert_eq!(stopped, TaskStop::Refused { error: "it did not answer within 20ms".into() });
    finish(&session, &io).await;
}

// TS: adapter-permissions.test.ts:567
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_subagent_already_over_and_a_turn_not_running_write_nothing_and_read_not_running() {
    tokio::task::LocalSet::new().run_until(a_subagent_already_over_and_a_turn_not_running_write_nothing_and_read_not_running_body()).await
}

async fn a_subagent_already_over_and_a_turn_not_running_write_nothing_and_read_not_running_body() {
    let (_events, session, io) = begin(None);
    io.push(json!({ "type": "system", "subtype": "task_notification", "task_id": "b2", "tool_use_id": "toolu_b", "status": "completed", "summary": "30", "session_id": SESSION }).to_string());
    settle().await;
    assert_eq!(session.stop_task("b2").await.unwrap(), TaskStop::NotRunning);
    assert_eq!(io.stdin().len(), 1);
    finish(&session, &io).await;
    assert_eq!(session.stop_task("a1").await.unwrap(), TaskStop::NotRunning);
    assert_eq!(io.stdin().len(), 1);
}

// TS: adapter-permissions.test.ts:578
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stop_still_waiting_when_the_turn_replies_settles_not_running_since_nothing_can_answer_it_now() {
    tokio::task::LocalSet::new()
        .run_until(a_stop_still_waiting_when_the_turn_replies_settles_not_running_since_nothing_can_answer_it_now_body())
        .await
}

async fn a_stop_still_waiting_when_the_turn_replies_settles_not_running_since_nothing_can_answer_it_now_body() {
    let (_events, session, io) = begin(None);
    settle().await;
    let stopped = stop_task(&session, "a1");
    settle().await;
    io.push(result_line());
    assert_eq!(bounded(stopped).await.unwrap().unwrap(), TaskStop::NotRunning);
    io.end(Some(0));
    bounded(session.finished()).await;
}

// ---------------------------------------------------------------------------------------------------------------
// steer-hold.test.ts
// ---------------------------------------------------------------------------------------------------------------

const SID: &str = "e16ed170-8257-4668-879e-fe836341633c";
const LINE: &str = "thread 1234abcd finished (completed): reply with the single word BANANA";
const PARSER: &str = "thread aaaa1111 finished (completed): built the parser.";
const PRINTER: &str = "thread bbbb2222 finished (completed): built the printer.";
const STORY: &str = "# The Lighthouse Keeper's Last Watch\n\nOld Thomas had tended the lighthouse for forty-three years.";

fn init_declaring(capabilities: &[&str]) -> String {
    json!({ "type": "system", "subtype": "init", "cwd": "/work", "session_id": SID, "tools": ["Bash"], "model": "claude-haiku-4-5-20251001", "permissionMode": "bypassPermissions", "claude_code_version": "2.1.280", "capabilities": capabilities }).to_string()
}

fn init() -> String {
    init_declaring(&[
        "interrupt_receipt_v1",
        "interrupt_cancel_queued_v1",
        "msg_lifecycle_v1",
        "mcp_read_resource_v1",
        "mcp_tool_ui_meta_v1",
    ])
}

/// A CLI that reports its messages and cannot be asked to cancel the ones it queues.
fn init_no_cancel() -> String {
    init_declaring(&["interrupt_receipt_v1", "msg_lifecycle_v1"])
}

fn said(id: &str, text: &str) -> String {
    json!({ "type": "assistant", "message": { "model": "claude-haiku-4-5-20251001", "id": id, "type": "message", "role": "assistant", "content": [{ "type": "text", "text": text }], "usage": { "input_tokens": 10, "output_tokens": 300 } }, "parent_tool_use_id": null, "session_id": SID }).to_string()
}

fn result(text: &str, ms: u64, cost: f64) -> String {
    json!({ "type": "result", "subtype": "success", "is_error": false, "duration_ms": ms, "result": text, "stop_reason": "end_turn", "session_id": SID, "total_cost_usd": cost, "usage": { "input_tokens": 10, "output_tokens": 300 } }).to_string()
}

/// The result the CLI prints for a turn the interrupt stopped.
fn stopped() -> String {
    json!({ "type": "result", "subtype": "error_during_execution", "is_error": true, "terminal_reason": "aborted_streaming", "duration_ms": 812, "stop_reason": null, "session_id": SID, "total_cost_usd": 0.0127 }).to_string()
}

fn lifecycle(uuid: &str, state: &str) -> String {
    json!({ "type": "command_lifecycle", "command_uuid": uuid, "state": state, "uuid": "0b7f3a52-6c1d-4e8a-9f2b-3d4c5e6f7a8b", "session_id": SID }).to_string()
}

fn interrupt_receipt(response: Value) -> String {
    json!({ "type": "control_response", "response": { "subtype": "success", "request_id": "wsp-interrupt-1", "response": response } })
        .to_string()
}

/// A stream the test feeds by hand, its writes recorded; `taken` stands for the lines an attach's run channel holds,
/// and the factory offers attach.
fn manual_exec(taken: Option<Vec<String>>) -> Arc<FakeExec> {
    Arc::new(FakeExec {
        script: Script::Manual,
        opts: StreamOpts { run: Some(RUN_HANDLE.to_string()), taken, ..Default::default() },
        calls: Default::default(),
        order: Default::default(),
        writes: Default::default(),
        streams: Default::default(),
        attach: Some(false),
        attached: Default::default(),
        held: Default::default(),
    })
}

fn push_all(exec: &FakeExec, lines: Vec<String>) {
    for line in lines {
        exec.push(line);
    }
}

/// The uuid the nth steer line went out under.
fn steered(exec: &FakeExec, n: usize) -> String {
    let users: Vec<String> = exec.writes().into_iter().filter(|line| json::<Value>(line)["type"] == "user").collect();
    json::<Value>(users.get(n).unwrap_or_else(|| panic!("{} steer lines, wanted {n}", users.len())))["uuid"]
        .as_str()
        .expect("steer line has no uuid")
        .to_string()
}

fn done_events(events: &Events) -> Vec<AdapterEvent> {
    snapshot(events).into_iter().filter(|e| matches!(e, AdapterEvent::TurnDone { .. })).collect()
}

fn has_event(events: &Events, which: impl Fn(&AdapterEvent) -> bool) -> bool {
    snapshot(events).iter().any(which)
}

fn session_started(events: &Events) -> bool {
    has_event(events, |e| matches!(e, AdapterEvent::SessionStart { .. }))
}

/// steer-hold's own `until`: up to 500 polls 2 ms apart.
async fn until_slow(check: impl Fn() -> bool) {
    for _ in 0..500 {
        if check() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    assert!(check(), "condition never held");
}

async fn settle_long() {
    tokio::time::sleep(Duration::from_millis(60)).await;
}

fn adapter_on(exec: &Arc<FakeExec>, result_exit_ms: Option<u64>, interrupt_grace_ms: Option<u64>) -> wsp_adapters::claude::ClaudeAdapter {
    let mut deps = AdapterDeps::new(exec.clone(), "/root/.claude-cfg");
    deps.result_exit_ms = result_exit_ms;
    deps.interrupt_grace_ms = interrupt_grace_ms;
    create_claude_adapter(deps).unwrap()
}

fn start_turn(adapter: &wsp_adapters::claude::ClaudeAdapter, prompt: &str, keep: bool) -> (Events, ClaudeSession) {
    let (events, on_event) = collect();
    let session =
        adapter.start(HarnessStartOptions { prompt: prompt.into(), keep, on_event: Some(on_event), ..Default::default() }).unwrap();
    (events, session)
}

async fn steer(session: &ClaudeSession, text: &str) -> SteerOutcome {
    session.steer(text, None, vec![]).await.unwrap()
}

fn interrupt(session: &ClaudeSession) -> JoinHandle<()> {
    let session = session.clone();
    tokio::task::spawn_local(async move { session.interrupt().await })
}

fn interrupt_written(exec: &FakeExec) -> bool {
    exec.writes().iter().any(|line| line.contains("\"interrupt\""))
}

fn unread_ids(events: &Events) -> Option<Vec<String>> {
    snapshot(events).into_iter().find_map(|e| if let AdapterEvent::TurnUnread { ids, .. } = e { Some(ids) } else { None })
}

fn status_and_text(result: &TurnResult) -> (TurnStatus, Option<&str>) {
    (result.status, result.text.as_deref())
}

// TS: steer-hold.test.ts:112
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_steered_message_is_answered_in_the_same_turn_which_ends_the_moment_the_cli_completes_it() {
    tokio::task::LocalSet::new()
        .run_until(a_steered_message_is_answered_in_the_same_turn_which_ends_the_moment_the_cli_completes_it_body())
        .await
}

async fn a_steered_message_is_answered_in_the_same_turn_which_ends_the_moment_the_cli_completes_it_body() {
    let exec = manual_exec(None);
    // The silence window is far past the test, so only the CLI's own word on the message can end the turn.
    let adapter = adapter_on(&exec, Some(60_000), None);
    let (events, session) = start_turn(&adapter, "Write a 250-word story about a lighthouse.", false);
    exec.push(init());
    until_slow(|| session_started(&events)).await;
    assert_eq!(steer(&session, LINE).await, SteerOutcome::Accepted);
    let uuid = steered(&exec, 0);
    push_all(
        &exec,
        vec![
            lifecycle(&uuid, "queued"),
            said("msg_1", STORY),
            result(STORY, 5164, 0.01280775),
            lifecycle(&uuid, "started"),
            init(),
            said("msg_2", "BANANA"),
            result("BANANA", 3700, 0.0170532),
        ],
    );
    settle_long().await;
    assert_eq!(done_events(&events), Vec::<AdapterEvent>::new());
    assert_eq!(exec.order(), vec!["write"]);

    exec.push(lifecycle(&uuid, "completed"));
    until_slow(|| done_events(&events).len() == 1).await;
    assert_eq!(exec.order(), vec!["write", "closeInput"]);
    exec.end(Some(0));
    let answer = bounded(session.finished()).await;
    assert_eq!(status_and_text(&answer), (TurnStatus::Completed, Some("BANANA")));
    assert!((answer.cost_usd.expect("no cost").0 - 0.0170532).abs() < 0.005);
    assert_eq!(done_events(&events).len(), 1);
    assert!(!has_event(&events, |e| matches!(e, AdapterEvent::TurnUnread { .. })));
    // A reply held for a steered message alone is told nowhere early, so the turn's end is its one line.
    assert!(!has_event(&events, |e| matches!(e, AdapterEvent::TurnTasks { replied: Some(_), .. })));
    assert!(matches!(&done_events(&events)[0], AdapterEvent::TurnDone { held: None, .. }));
}

// TS: steer-hold.test.ts:141
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_messages_at_once_which_the_cli_takes_as_one_turn_are_both_answered_before_the_turn_ends() {
    tokio::task::LocalSet::new()
        .run_until(two_messages_at_once_which_the_cli_takes_as_one_turn_are_both_answered_before_the_turn_ends_body())
        .await
}

async fn two_messages_at_once_which_the_cli_takes_as_one_turn_are_both_answered_before_the_turn_ends_body() {
    let exec = manual_exec(None);
    let adapter = adapter_on(&exec, Some(60_000), None);
    let (events, session) = start_turn(&adapter, "Write a 250-word story about a lighthouse.", false);
    exec.push(init());
    until_slow(|| session_started(&events)).await;
    let (first, second) = tokio::join!(session.steer(PARSER, None, vec![]), session.steer(PRINTER, None, vec![]));
    assert_eq!((first.unwrap(), second.unwrap()), (SteerOutcome::Accepted, SteerOutcome::Accepted));
    let (parser, printer) = (steered(&exec, 0), steered(&exec, 1));
    let both = "Thanks for the update! Both the parser and printer are built.";
    push_all(
        &exec,
        vec![
            lifecycle(&parser, "queued"),
            lifecycle(&printer, "queued"),
            said("msg_1", STORY),
            result(STORY, 5503, 0.012894),
            lifecycle(&parser, "started"),
            lifecycle(&printer, "started"),
            init(),
            said("msg_2", both),
            result(both, 2986, 0.01694495),
            lifecycle(&parser, "completed"),
        ],
    );
    settle_long().await;
    assert_eq!(done_events(&events), Vec::<AdapterEvent>::new());

    exec.push(lifecycle(&printer, "completed"));
    until_slow(|| done_events(&events).len() == 1).await;
    exec.end(Some(0));
    let answer = bounded(session.finished()).await;
    assert_eq!(status_and_text(&answer), (TurnStatus::Completed, Some(both)));
    assert_eq!(done_events(&events).len(), 1);
}

// TS: steer-hold.test.ts:172
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_slash_command_steered_then_ends_the_turn_on_the_agents_reply_and_not_on_its_own_words() {
    tokio::task::LocalSet::new()
        .run_until(a_slash_command_steered_then_ends_the_turn_on_the_agents_reply_and_not_on_its_own_words_body())
        .await
}

async fn a_slash_command_steered_then_ends_the_turn_on_the_agents_reply_and_not_on_its_own_words_body() {
    let exec = manual_exec(None);
    let adapter = adapter_on(&exec, Some(60_000), None);
    let (events, session) = start_turn(&adapter, "Write a 250-word story about a lighthouse.", false);
    exec.push(init());
    until_slow(|| session_started(&events)).await;
    steer(&session, "/cost").await;
    let uuid = steered(&exec, 0);
    let cost = "Total cost:            $0.0128\nTotal duration (API):  5s";
    push_all(
        &exec,
        vec![
            lifecycle(&uuid, "queued"),
            said("msg_1", STORY),
            result(STORY, 5465, 0.01283775),
            lifecycle(&uuid, "started"),
            init(),
            json!({ "type": "assistant", "message": { "id": "ac793b79-283a-458a-80cc-a046634e400f", "model": "<synthetic>", "role": "assistant", "type": "message", "content": [{ "type": "text", "text": cost }] }, "parent_tool_use_id": null, "session_id": SID }).to_string(),
            json!({ "type": "result", "subtype": "success", "is_error": false, "duration_api_ms": 0, "num_turns": 0, "result": cost, "stop_reason": null, "session_id": SID, "total_cost_usd": 0.01283775 }).to_string(),
            lifecycle(&uuid, "completed"),
        ],
    );
    until_slow(|| done_events(&events).len() == 1).await;
    exec.end(Some(0));
    // The command's words are a row of their own in the pane; the reply a lead reads is the agent's.
    let answer = bounded(session.finished()).await;
    assert_eq!(status_and_text(&answer), (TurnStatus::Completed, Some(STORY)));
    assert!(has_event(&events, |e| matches!(e, AdapterEvent::TurnDelta(d) if d.kind == DeltaKind::Text && d.text == cost)));
    assert!(matches!(&done_events(&events)[0], AdapterEvent::TurnDone { held: None, .. }));
}

// TS: steer-hold.test.ts:200
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_message_the_agent_took_at_a_tools_end_is_answered_by_the_turns_own_result_which_ends_it() {
    tokio::task::LocalSet::new()
        .run_until(a_message_the_agent_took_at_a_tools_end_is_answered_by_the_turns_own_result_which_ends_it_body())
        .await
}

async fn a_message_the_agent_took_at_a_tools_end_is_answered_by_the_turns_own_result_which_ends_it_body() {
    let exec = manual_exec(None);
    let adapter = adapter_on(&exec, Some(60_000), None);
    let (events, session) = start_turn(&adapter, "Run sleep 4 && echo done, then say what it printed.", false);
    exec.push(init());
    until_slow(|| session_started(&events)).await;
    steer(&session, "Also tell me the number seven.").await;
    let uuid = steered(&exec, 0);
    push_all(
        &exec,
        vec![
            lifecycle(&uuid, "queued"),
            json!({ "type": "user", "message": { "role": "user", "content": [{ "tool_use_id": "toolu_1", "type": "tool_result", "content": "done", "is_error": false }] }, "parent_tool_use_id": null, "session_id": SID }).to_string(),
            lifecycle(&uuid, "started"),
            said("msg_2", "The command printed done. Seven."),
            result("The command printed done. Seven.", 6100, 0.02),
            lifecycle(&uuid, "completed"),
        ],
    );
    until_slow(|| done_events(&events).len() == 1).await;
    exec.end(Some(0));
    let answer = bounded(session.finished()).await;
    assert_eq!(status_and_text(&answer), (TurnStatus::Completed, Some("The command printed done. Seven.")));
    assert!(!has_event(&events, |e| matches!(e, AdapterEvent::TurnUnread { .. })));
}

// TS: steer-hold.test.ts:223
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn on_a_cli_that_declares_no_reports_on_its_messages_the_turn_ends_at_its_reply_as_it_always_has() {
    tokio::task::LocalSet::new()
        .run_until(on_a_cli_that_declares_no_reports_on_its_messages_the_turn_ends_at_its_reply_as_it_always_has_body())
        .await
}

async fn on_a_cli_that_declares_no_reports_on_its_messages_the_turn_ends_at_its_reply_as_it_always_has_body() {
    let exec = manual_exec(None);
    let adapter = adapter_on(&exec, Some(60_000), None);
    let (events, session) = start_turn(&adapter, "go", false);
    exec.push(init_declaring(&[]));
    until_slow(|| session_started(&events)).await;
    steer(&session, LINE).await;
    exec.push(result(STORY, 5164, 0.0128));
    until_slow(|| done_events(&events).len() == 1).await;
    assert_eq!(exec.order(), vec!["write", "closeInput"]);
    exec.end(Some(0));
    let answer = bounded(session.finished()).await;
    assert_eq!(status_and_text(&answer), (TurnStatus::Completed, Some(STORY)));
    // Nothing says it went unread: a CLI that reports nothing may have read it at any call's end.
    assert!(!has_event(&events, |e| matches!(e, AdapterEvent::TurnUnread { .. })));
}

// TS: steer-hold.test.ts:244
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_steered_message_is_told_unread_before_the_turns_end_when_the_process_dies_with_it_queued() {
    tokio::task::LocalSet::new()
        .run_until(a_steered_message_is_told_unread_before_the_turns_end_when_the_process_dies_with_it_queued_body())
        .await
}

async fn a_steered_message_is_told_unread_before_the_turns_end_when_the_process_dies_with_it_queued_body() {
    let exec = manual_exec(None);
    let adapter = adapter_on(&exec, Some(40), None);
    let (events, session) = start_turn(&adapter, "go", false);
    exec.push(init());
    until_slow(|| session_started(&events)).await;
    steer(&session, LINE).await;
    exec.push(lifecycle(&steered(&exec, 0), "queued"));
    exec.end(Some(137));
    assert_eq!(bounded(session.finished()).await.status, TurnStatus::Failed);
    let all = snapshot(&events);
    let told: Vec<&str> = types(&all).into_iter().filter(|t| *t == "turn.unread" || *t == "turn.done").collect();
    assert_eq!(told, vec!["turn.unread", "turn.done"]);
    assert_eq!(unread_ids(&events), Some(vec![steered(&exec, 0)]));
}

// TS: steer-hold.test.ts:260
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stop_with_one_still_queued_asks_the_cli_to_cancel_it_answers_at_once_tells_it_unread_and_keeps_the_process() {
    tokio::task::LocalSet::new()
        .run_until(a_stop_with_one_still_queued_asks_the_cli_to_cancel_it_answers_at_once_tells_it_unread_and_keeps_the_process_body())
        .await
}

async fn a_stop_with_one_still_queued_asks_the_cli_to_cancel_it_answers_at_once_tells_it_unread_and_keeps_the_process_body() {
    let exec = manual_exec(None);
    let adapter = adapter_on(&exec, Some(60_000), Some(60_000));
    let (events, session) = start_turn(&adapter, "go", true);
    exec.push(init());
    until_slow(|| session_started(&events)).await;
    steer(&session, LINE).await;
    let uuid = steered(&exec, 0);
    exec.push(lifecycle(&uuid, "queued"));
    let stopping = interrupt(&session);
    until_slow(|| interrupt_written(&exec)).await;
    let line = exec.writes().into_iter().find(|line| line.contains("\"interrupt\"")).unwrap();
    assert_matches_object(&json::<Value>(&line), json!({ "request": { "subtype": "interrupt", "cancel_queued": true } }));
    push_all(&exec, vec![lifecycle(&uuid, "cancelled"), interrupt_receipt(json!({ "still_queued": [], "cancelled": [uuid] })), stopped()]);
    bounded(stopping).await.unwrap();
    assert_eq!(bounded(session.finished()).await.status, TurnStatus::Interrupted);
    assert_eq!(unread_ids(&events), Some(vec![steered(&exec, 0)]));
    assert!(!exec.order().contains(&"teardown".to_string()));
    assert!(session.kept().is_some());
}

// TS: steer-hold.test.ts:281
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stop_with_one_still_queued_on_a_cli_that_cannot_cancel_it_answers_at_once_tells_it_unread_and_the_process_goes() {
    tokio::task::LocalSet::new()
        .run_until(a_stop_with_one_still_queued_on_a_cli_that_cannot_cancel_it_answers_at_once_tells_it_unread_and_the_process_goes_body())
        .await
}

async fn a_stop_with_one_still_queued_on_a_cli_that_cannot_cancel_it_answers_at_once_tells_it_unread_and_the_process_goes_body() {
    let exec = manual_exec(None);
    let adapter = adapter_on(&exec, Some(60_000), Some(60_000));
    let (events, session) = start_turn(&adapter, "go", true);
    exec.push(init_no_cancel());
    until_slow(|| session_started(&events)).await;
    steer(&session, LINE).await;
    let uuid = steered(&exec, 0);
    exec.push(lifecycle(&uuid, "queued"));
    let stopping = interrupt(&session);
    until_slow(|| interrupt_written(&exec)).await;
    push_all(&exec, vec![interrupt_receipt(json!({ "still_queued": [uuid] })), stopped()]);
    bounded(stopping).await.unwrap();
    assert_eq!(bounded(session.finished()).await.status, TurnStatus::Interrupted);
    assert_eq!(unread_ids(&events), Some(vec![steered(&exec, 0)]));
    assert!(exec.order().contains(&"teardown".to_string()));
    assert!(session.kept().is_none());
}

// TS: steer-hold.test.ts:301
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stop_of_a_reply_held_for_a_message_the_cli_has_not_taken_up_answers_at_once_with_the_held_words() {
    tokio::task::LocalSet::new()
        .run_until(a_stop_of_a_reply_held_for_a_message_the_cli_has_not_taken_up_answers_at_once_with_the_held_words_body())
        .await
}

async fn a_stop_of_a_reply_held_for_a_message_the_cli_has_not_taken_up_answers_at_once_with_the_held_words_body() {
    let exec = manual_exec(None);
    let adapter = adapter_on(&exec, Some(60_000), Some(60_000));
    let (events, session) = start_turn(&adapter, "go", true);
    exec.push(init());
    until_slow(|| session_started(&events)).await;
    steer(&session, LINE).await;
    let uuid = steered(&exec, 0);
    push_all(&exec, vec![lifecycle(&uuid, "queued"), said("msg_1", STORY), result(STORY, 5164, 0.0128)]);
    settle_long().await;
    assert_eq!(done_events(&events), Vec::<AdapterEvent>::new());
    let stopping = interrupt(&session);
    until_slow(|| interrupt_written(&exec)).await;
    push_all(&exec, vec![lifecycle(&uuid, "cancelled"), interrupt_receipt(json!({ "still_queued": [], "cancelled": [uuid] }))]);
    bounded(stopping).await.unwrap();
    let answer = bounded(session.finished()).await;
    assert_eq!(status_and_text(&answer), (TurnStatus::Interrupted, Some(STORY)));
    assert_eq!(unread_ids(&events), Some(vec![steered(&exec, 0)]));
    assert!(session.kept().is_some());
}

// TS: steer-hold.test.ts:322
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_same_stop_on_a_cli_that_cannot_cancel_answers_at_once_with_the_held_words_and_the_process_goes() {
    tokio::task::LocalSet::new()
        .run_until(the_same_stop_on_a_cli_that_cannot_cancel_answers_at_once_with_the_held_words_and_the_process_goes_body())
        .await
}

async fn the_same_stop_on_a_cli_that_cannot_cancel_answers_at_once_with_the_held_words_and_the_process_goes_body() {
    let exec = manual_exec(None);
    let adapter = adapter_on(&exec, Some(60_000), Some(60_000));
    let (events, session) = start_turn(&adapter, "go", true);
    exec.push(init_no_cancel());
    until_slow(|| session_started(&events)).await;
    steer(&session, LINE).await;
    push_all(&exec, vec![lifecycle(&steered(&exec, 0), "queued"), said("msg_1", STORY), result(STORY, 5164, 0.0128)]);
    settle_long().await;
    assert_eq!(done_events(&events), Vec::<AdapterEvent>::new());
    bounded(session.interrupt()).await;
    let answer = bounded(session.finished()).await;
    assert_eq!(status_and_text(&answer), (TurnStatus::Interrupted, Some(STORY)));
    assert_eq!(unread_ids(&events), Some(vec![steered(&exec, 0)]));
    assert!(exec.order().contains(&"teardown".to_string()));
}

// TS: steer-hold.test.ts:339
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stop_while_the_cli_answers_a_steered_message_answers_at_once_and_keeps_the_process_since_nothing_is_left_queued() {
    tokio::task::LocalSet::new()
        .run_until(a_stop_while_the_cli_answers_a_steered_message_answers_at_once_and_keeps_the_process_since_nothing_is_left_queued_body())
        .await
}

async fn a_stop_while_the_cli_answers_a_steered_message_answers_at_once_and_keeps_the_process_since_nothing_is_left_queued_body() {
    let exec = manual_exec(None);
    let adapter = adapter_on(&exec, Some(60_000), Some(60_000));
    let (events, session) = start_turn(&adapter, "go", true);
    exec.push(init());
    until_slow(|| session_started(&events)).await;
    steer(&session, "Now write a 300-word poem about the sea.").await;
    let uuid = steered(&exec, 0);
    push_all(
        &exec,
        vec![lifecycle(&uuid, "queued"), said("msg_1", STORY), result(STORY, 5374, 0.0127), lifecycle(&uuid, "started"), init()],
    );
    settle_long().await;
    let stopping = interrupt(&session);
    until_slow(|| interrupt_written(&exec)).await;
    push_all(&exec, vec![said("msg_2", "# The Sea's Language"), stopped(), lifecycle(&uuid, "cancelled")]);
    bounded(stopping).await.unwrap();
    // The stopped answer's result says nothing, so the words the agent gave before it are the turn's.
    let answer = bounded(session.finished()).await;
    assert_eq!(status_and_text(&answer), (TurnStatus::Interrupted, Some(STORY)));
    assert!(!has_event(&events, |e| matches!(e, AdapterEvent::TurnUnread { .. })));
    assert!(!exec.order().contains(&"teardown".to_string()));
    assert!(session.kept().is_some());
}

fn attach_options(events_sink: wsp_adapters::port::OnEvent) -> AdapterAttachOptions {
    AdapterAttachOptions {
        run: RUN_HANDLE.into(),
        session_id: SID.into(),
        started_at: 1.0,
        model: None,
        cwd: None,
        prompt: None,
        effort: None,
        from: None,
        steered: None,
        on_event: events_sink,
    }
}

async fn attach(adapter: &wsp_adapters::claude::ClaudeAdapter, on_event: wsp_adapters::port::OnEvent) -> ClaudeSession {
    match adapter.attach(attach_options(on_event)).expect("the factory offers attach").await.unwrap() {
        AttachedSession::Session(s) => s,
        AttachedSession::Gone => panic!("gone"),
    }
}

// TS: steer-hold.test.ts:363
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_reopened_turn_waits_on_the_messages_its_runs_channel_took_after_the_turns_opening_and_is_answered_in_the_same_turn() {
    tokio::task::LocalSet::new()
        .run_until(
            a_reopened_turn_waits_on_the_messages_its_runs_channel_took_after_the_turns_opening_and_is_answered_in_the_same_turn_body(),
        )
        .await
}

async fn a_reopened_turn_waits_on_the_messages_its_runs_channel_took_after_the_turns_opening_and_is_answered_in_the_same_turn_body() {
    let earlier = user_message_line("an earlier turn's steer", SID, &[], Some("11111111-1111-4111-8111-111111111111"));
    let uuid = "33333333-3333-4333-8333-333333333333";
    let exec = manual_exec(Some(vec![
        user_message_line("first", SID, &[], None),
        earlier,
        user_message_line("Write a story.", SID, &[], None),
        user_message_line(LINE, SID, &[], Some(uuid)),
    ]));
    let adapter = adapter_on(&exec, Some(60_000), None);
    let (events, on_event) = collect();
    let session = attach(&adapter, on_event).await;
    // The log replayed from its first byte: the reply the first host was holding, and the CLI starting on the line.
    push_all(
        &exec,
        vec![init(), lifecycle(uuid, "queued"), said("msg_1", STORY), result(STORY, 5164, 0.0128), lifecycle(uuid, "started"), init()],
    );
    settle_long().await;
    assert_eq!(done_events(&events), Vec::<AdapterEvent>::new());
    assert_eq!(exec.order(), Vec::<String>::new());

    push_all(&exec, vec![said("msg_2", "BANANA"), result("BANANA", 3700, 0.017), lifecycle(uuid, "completed")]);
    until_slow(|| done_events(&events).len() == 1).await;
    exec.end(Some(0));
    let answer = bounded(session.finished()).await;
    assert_eq!(status_and_text(&answer), (TurnStatus::Completed, Some("BANANA")));
}

// TS: steer-hold.test.ts:383
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_reopened_turn_tells_unread_a_message_its_runs_channel_took_that_the_cli_never_queued_when_the_process_is_gone() {
    tokio::task::LocalSet::new()
        .run_until(a_reopened_turn_tells_unread_a_message_its_runs_channel_took_that_the_cli_never_queued_when_the_process_is_gone_body())
        .await
}

async fn a_reopened_turn_tells_unread_a_message_its_runs_channel_took_that_the_cli_never_queued_when_the_process_is_gone_body() {
    let lost = "33333333-3333-4333-8333-333333333333";
    let exec = manual_exec(Some(vec![user_message_line("Write a story.", SID, &[], None), user_message_line(LINE, SID, &[], Some(lost))]));
    let adapter = adapter_on(&exec, Some(40), None);
    let (events, on_event) = collect();
    let session = attach(&adapter, on_event).await;
    // The CLI printed nothing about the message, so only what its init declares says it never read it.
    push_all(&exec, vec![init(), said("msg_1", STORY), result(STORY, 5164, 0.0128)]);
    exec.end(Some(143));
    bounded(session.finished()).await;
    assert_eq!(unread_ids(&events), Some(vec![lost.to_string()]));
}

// TS: steer-hold.test.ts:398
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_steer_goes_out_under_a_uuid_of_its_own_which_the_turns_opening_line_does_not_carry() {
    tokio::task::LocalSet::new()
        .run_until(every_steer_goes_out_under_a_uuid_of_its_own_which_the_turns_opening_line_does_not_carry_body())
        .await
}

async fn every_steer_goes_out_under_a_uuid_of_its_own_which_the_turns_opening_line_does_not_carry_body() {
    let exec = manual_exec(None);
    let adapter = adapter_on(&exec, None, None);
    let (events, session) = start_turn(&adapter, "go", false);
    exec.push(init());
    until_slow(|| session_started(&events)).await;
    steer(&session, "one").await;
    steer(&session, "two").await;
    let (one, two) = (steered(&exec, 0), steered(&exec, 1));
    let v4 = regex::Regex::new(r"^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$").unwrap();
    assert!(v4.is_match(&one), "{one}");
    assert_ne!(two, one);
    assert_eq!(exec.writes(), vec![user_message_line("one", SID, &[], Some(&one)), user_message_line("two", SID, &[], Some(&two))]);
    // The runtime names the id, which the turn's unread messages are told by.
    session.steer("three", Some("a7d1c2e3-0000-4000-8000-000000000003".into()), vec![]).await.unwrap();
    assert_eq!(steered(&exec, 2), "a7d1c2e3-0000-4000-8000-000000000003");
    let calls: Vec<Call> = exec.calls();
    let seed = calls[0].input.as_ref().expect("the launch seeds the channel");
    assert!(json::<Value>(&seed[0]).get("uuid").is_none());
    exec.end(Some(0));
    bounded(session.finished()).await;
}
