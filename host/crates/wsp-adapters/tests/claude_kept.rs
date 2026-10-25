// SPDX-License-Identifier: AGPL-3.0-only
//! The hard cases of the process a turn keeps for the thread's next message and the one warmed ahead of the first
//! (spec/claude_turn.md rules T1 to T6, spec/claude_adapter.md rules A1 and A2). No TS test reaches `keep`, `warm`
//! or `kept().next`; every expected value here was printed by the TS adapter at a3b870a6e over the same lines.

#[path = "common/claude_exec.rs"]
mod claude_exec;

use std::sync::Arc;

use claude_exec::{collect, types, until, FakeExec, Script, StreamOpts};
use serde_json::json;
use wsp_adapters::claude::{create_claude_adapter, AdapterDeps, AttachedSession};
use wsp_adapters::port::*;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::types::usage::UsageTokens;
use wsp_wire::views::session_events::TurnResultModelsItem;

const SID: &str = "e16ed170-8257-4668-879e-fe836341633c";

fn n(x: f64) -> JsNumber {
    JsNumber(x)
}

fn deps(exec: &Arc<FakeExec>) -> AdapterDeps {
    AdapterDeps { result_exit_ms: Some(5), interrupt_grace_ms: Some(5), ..AdapterDeps::new(exec.clone(), "/root/.claude") }
}

fn init() -> String {
    json!({"type": "system", "subtype": "init", "session_id": SID, "model": "claude-x", "cwd": "/w", "tools": []}).to_string()
}

fn said(text: &str, id: &str) -> String {
    json!({"type": "assistant", "uuid": format!("u-{id}"), "session_id": SID, "message": {"id": id, "model": "claude-x", "content": [{"type": "text", "text": text}], "usage": {"input_tokens": 10, "output_tokens": 5}}}).to_string()
}

/// A result whose running totals stand at `input` tokens in and half that out, for `cost` dollars.
fn result(cost: f64, input: u64) -> String {
    json!({"type": "result", "subtype": "success", "is_error": false, "result": "ok", "duration_ms": 40, "num_turns": 1, "session_id": SID, "total_cost_usd": cost, "usage": {"input_tokens": 10, "output_tokens": 5}, "modelUsage": {"claude-x": {"inputTokens": input, "outputTokens": input / 2, "cacheReadInputTokens": 0, "cacheCreationInputTokens": 0, "costUSD": cost, "contextWindow": 200000}}}).to_string()
}

fn completed(input: u64, cost: f64) -> TurnResult {
    TurnResult {
        status: TurnStatus::Completed,
        duration_ms: Some(n(40.0)),
        waited_ms: None,
        cost_usd: Some(n(cost)),
        tokens: Some(TurnTokens { input: n(input as f64), output: n((input / 2) as f64), cached: Some(n(0.0)), cache_write: Some(n(0.0)), reasoning: Some(n(0.0)), context: Some(n(15.0)), window: Some(n(200_000.0)) }),
        model: Some("claude-x".into()),
        models: Some(vec![TurnResultModelsItem { model: "claude-x".into(), tokens: UsageTokens { input, output: input / 2, cached: 0, cache_write: 0, reasoning: 0 }, cost_usd: Some(n(cost)) }]),
        text: Some("ok".into()),
        error: None,
        unreached: None,
        refusal: None,
        limit: None,
    }
}

// T1, T2, T3: a kept turn's result leaves the process up and nothing signalled; its next turn runs on the same process
// under the same local id, writes its message under the session the CLI named, and costs only what it added.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_kept_process_takes_the_next_turn_under_the_same_local_id_and_costs_only_what_it_added() {
    let exec = FakeExec::manual(StreamOpts::default());
    let adapter = create_claude_adapter(deps(&exec)).unwrap();
    let (first_events, on_event) = collect();
    let first = adapter.start(HarnessStartOptions { prompt: "one".into(), keep: true, on_event: Some(on_event), ..Default::default() }).unwrap();
    assert!(first.kept().is_none(), "no result seen yet");
    exec.push(init());
    exec.push(said("hi", "m1"));
    exec.push(result(0.25, 100));
    assert_eq!(first.finished().await, completed(100, 0.25));
    assert_eq!(exec.order(), Vec::<String>::new(), "a kept turn's end closes its view, never the process's input, and signals nothing");
    assert_eq!(types(&first_events.lock().unwrap()), vec!["session.start", "turn.usage", "turn.delta", "turn.anchor", "turn.done", "session.end"]);

    let kept = first.kept().expect("kept, up, and its result seen");
    assert_eq!(kept.session_file(), Some(SessionFile { folder: "/root/.claude/projects".into(), name: format!("{SID}.jsonl"), depth: 1 }));
    let (second_events, on_event) = collect();
    let second = kept.next(KeptTurn { prompt: "two".into(), images: vec![], after: None, on_event });
    assert_eq!(second.local_id(), first.local_id(), "the thread's turns on one process share the registry key");
    assert_eq!(second.claude_session_id(), first.local_id(), "until its own init names the session");
    assert_eq!(second.command(), None, "only a launch or a warm process's first turn has a command");
    let sessions = adapter.sessions();
    assert_eq!(sessions.len(), 1);
    assert!(sessions[&first.local_id()].same(&second), "the next turn replaces the first in the registry");
    until(|| exec.writes().len() == 1).await;
    assert_eq!(exec.writes(), vec![format!(r#"{{"type":"user","message":{{"role":"user","content":[{{"type":"text","text":"two"}}]}},"parent_tool_use_id":null,"session_id":"{SID}"}}"#)], "written at once, under the session the CLI named, with no uuid");

    exec.push(init());
    exec.push(said("again", "m2"));
    exec.push(result(0.4, 160));
    assert_eq!(second.finished().await, completed(60, 0.15000000000000002), "the second turn starts from the first's totals");
    assert_eq!(types(&second_events.lock().unwrap()), vec!["session.start", "turn.usage", "turn.delta", "turn.anchor", "turn.done", "session.end"]);
    assert_eq!(exec.order(), vec!["write"]);

    let kept = second.kept().expect("still up");
    kept.close(true).await;
    assert_eq!(exec.order(), vec!["write", "closeInput", "teardown"], "close(now) closes the process's input and ends its tree at once");
}

// T3: on a road that holds no line (no write_after), the next turn's line waits for `after`: written once it
// resolves, never where it rejects.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_kept_turns_line_waits_for_after_and_goes_nowhere_where_it_rejects() {
    let exec = FakeExec::manual(StreamOpts::default());
    let adapter = create_claude_adapter(deps(&exec)).unwrap();
    let first = adapter.start(HarnessStartOptions { prompt: "one".into(), keep: true, on_event: Some(Arc::new(|_| {})), ..Default::default() }).unwrap();
    let done = json!({"type": "result", "subtype": "success", "is_error": false, "result": "ok", "duration_ms": 40, "num_turns": 1, "session_id": SID, "usage": {"input_tokens": 10, "output_tokens": 5}}).to_string();
    exec.push(init());
    exec.push(done.clone());
    first.finished().await;
    let (go, gate) = tokio::sync::oneshot::channel::<Result<(), String>>();
    let after: Settle = Box::pin(async move { gate.await.unwrap_or(Err("dropped".into())) });
    let second = first.kept().expect("kept").next(KeptTurn { prompt: "two".into(), images: vec![], after: Some(after), on_event: Arc::new(|_| {}) });
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    assert!(exec.writes().is_empty(), "held until after settles");
    go.send(Ok(())).unwrap();
    until(|| exec.writes().len() == 1).await;
    assert_eq!(claude_exec::json(&exec.writes()[0])["message"]["content"][0]["text"], json!("two"));
    exec.push(init());
    exec.push(done);
    second.finished().await;
    let refused: Settle = Box::pin(async { Err("the seed went".to_string()) });
    let _third = second.kept().expect("kept").next(KeptTurn { prompt: "three".into(), images: vec![], after: Some(refused), on_event: Arc::new(|_| {}) });
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    assert_eq!(exec.writes().len(), 1, "a rejected after drops the line");
    assert_eq!(exec.order(), vec!["write"]);
}

// T4: a turn that failed on a kept process gives the process up: it is launched again for the next message.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_failed_turn_releases_its_process_and_keeps_nothing() {
    let exec = FakeExec::manual(StreamOpts::default());
    let adapter = create_claude_adapter(deps(&exec)).unwrap();
    let (_events, on_event) = collect();
    let session = adapter.start(HarnessStartOptions { prompt: "one".into(), keep: true, on_event: Some(on_event), ..Default::default() }).unwrap();
    exec.push(init());
    exec.push(json!({"type": "result", "subtype": "error_during_execution", "is_error": true, "errors": ["MCP server exploded"], "duration_ms": 9, "session_id": SID}).to_string());
    let result = session.finished().await;
    assert_eq!(result, TurnResult { status: TurnStatus::Failed, duration_ms: Some(n(9.0)), waited_ms: None, cost_usd: None, tokens: None, model: Some("claude-x".into()), models: None, text: None, error: Some("MCP server exploded".into()), unreached: None, refusal: None, limit: None });
    assert_eq!(exec.order(), vec!["closeInput", "teardown"], "released: the view's close reaches the process, which is ended once the wait passes");
    assert!(session.kept().is_none());
}

// T5: an attached turn has no keeper: its result leaves nothing for a next message, which launches again.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_attached_turn_keeps_no_process() {
    let exec = Arc::new(FakeExec {
        script: Script::Manual,
        opts: StreamOpts { run: Some("/tmp/run".into()), ..Default::default() },
        calls: Default::default(),
        order: Default::default(),
        writes: Default::default(),
        streams: Default::default(),
        attach: Some(false),
        attached: Default::default(),
        held: Default::default(),
    });
    let adapter = create_claude_adapter(deps(&exec)).unwrap();
    let (_events, on_event) = collect();
    let attach = AdapterAttachOptions { run: "/tmp/run".into(), session_id: SID.into(), started_at: 1.0, model: None, cwd: None, prompt: None, effort: None, from: None, steered: None, on_event };
    let AttachedSession::Session(session) = adapter.attach(attach).expect("attaches").await.expect("answered") else { panic!("the run is there") };
    assert_eq!(adapter.sessions().keys().cloned().collect::<Vec<_>>(), vec![SID.to_string()], "registered under the row's session id");
    exec.push(said("hi", "m1"));
    exec.push(result(0.25, 100));
    let result = session.finished().await;
    assert_eq!(
        result,
        TurnResult { status: TurnStatus::Completed, duration_ms: Some(n(40.0)), waited_ms: None, cost_usd: None, tokens: Some(TurnTokens { input: n(10.0), output: n(5.0), cached: None, cache_write: None, reasoning: None, context: Some(n(15.0)), window: Some(n(200_000.0)) }), model: None, models: None, text: Some("ok".into()), error: None, unreached: None, refusal: None, limit: None },
        "no init was read and no totals were saved: no model, no cost, the result's own usage"
    );
    assert!(session.kept().is_none());
    assert_eq!(exec.order(), vec!["closeInput", "teardown"], "nothing keeps the process: its input closes and it is ended once the wait passes");
}

// A1, A2: a warm process starts with no input, rests at once, and its first turn is a fresh session under the warm id
// with the warm command; its close ends it after the result wait.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_warm_process_rests_until_its_first_turn_which_is_a_fresh_session_under_the_warm_id() {
    let exec = FakeExec::manual(StreamOpts::default());
    let adapter = create_claude_adapter(deps(&exec)).unwrap();
    let warm = adapter.warm(HarnessWarmOptions { model: Some("opus".into()), ..Default::default() }).unwrap();
    let calls = exec.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].input, Some(vec![]), "a line channel with nothing in it");
    assert_eq!(exec.order(), Vec::<String>::new(), "the first view closes at once without touching the process");
    assert_eq!(warm.session_file(), None, "no session file before a turn");
    let (events, on_event) = collect();
    let session = warm.next(KeptTurn { prompt: "first".into(), images: vec![], after: None, on_event });
    let id = session.local_id();
    assert!(calls[0].command.ends_with(&format!(" --model 'opus' --session-id {id}")), "{}", calls[0].command);
    assert_eq!(session.claude_session_id(), id);
    assert_eq!(session.command(), Some(calls[0].command.clone()), "the warm command, the guard being empty");
    until(|| exec.writes().len() == 1).await;
    assert_eq!(exec.writes(), vec![format!(r#"{{"type":"user","message":{{"role":"user","content":[{{"type":"text","text":"first"}}]}},"parent_tool_use_id":null,"session_id":"{id}"}}"#)]);
    assert_eq!(adapter.sessions().keys().cloned().collect::<Vec<_>>(), vec![id.clone()]);
    exec.push(init());
    exec.push(said("hi", "m1"));
    exec.push(result(0.25, 100));
    assert_eq!(session.finished().await, completed(100, 0.25), "fresh: the whole total is this turn's");
    assert_eq!(types(&events.lock().unwrap()), vec!["session.start", "turn.usage", "turn.delta", "turn.anchor", "turn.done", "session.end"]);
    warm.close(false).await;
    assert_eq!(exec.order(), vec!["write", "closeInput", "teardown"], "close waits result_exit_ms, then ends the tree");
}
