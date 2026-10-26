// SPDX-License-Identifier: AGPL-3.0-only
//! What a caller does to a running Claude turn over its stdin channel, on a stream the test feeds by hand: the
//! permission prompt and its answers, a turn moved to another access mode, a subagent stopped by itself, and a
//! message steered while the agent writes its last words. Ports packages/adapter-claude/test/adapter-permissions.test.ts
//! and steer-hold.test.ts at a3b870a6e.

#![allow(dead_code, unused_imports)]

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
