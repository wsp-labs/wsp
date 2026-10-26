// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the readers: where the Rust crate reads what the agent tools leave on disk and print differently from
//! the TypeScript at a3b870a6e it was ported from. Every expected value below is what node printed for the same input.

#![allow(dead_code, unused_imports)]

#[path = "common/claude_exec.rs"]
mod claude_exec;

use std::sync::Arc;

use futures::future::{FutureExt, LocalBoxFuture};
use serde_json::{json, Value};
use wsp_adapters::claude::adapter::{create_claude_adapter, AdapterDeps};
use wsp_adapters::claude::conversations::{claude_conversations, ConversationDeps};
use wsp_adapters::port::{
    AdapterEvent, ConversationRoad, DeltaKind, EarlierAsk, EarlierRead, Env, ExecFailure, HarnessExec, HarnessStartOptions,
};

use claude_exec::{collect, FakeExec, StreamOpts};

const A: &str = "11111111-1111-4111-8111-111111111111";
const B: &str = "22222222-2222-4222-8222-222222222222";

fn init_line(session: &str) -> String {
    json!({"type": "system", "subtype": "init", "cwd": "/w", "session_id": session, "model": "m", "tools": []}).to_string()
}

fn result_line(session: &str) -> String {
    json!({"type": "result", "subtype": "success", "is_error": false, "result": "x", "duration_ms": 5, "session_id": session}).to_string()
}

/// One run of `lines` to its end, as a turn that resumes `resume` where one is named; every event it emitted.
async fn run_lines(lines: Vec<String>, resume: Option<&str>) -> Vec<AdapterEvent> {
    let exec = FakeExec::scripted(lines, StreamOpts::default());
    let adapter = create_claude_adapter(AdapterDeps::new(exec.clone(), "/root/.claude-cfg")).unwrap();
    let (events, on_event) = collect();
    let opts =
        HarnessStartOptions { prompt: "go".to_string(), resume: resume.map(String::from), on_event: Some(on_event), ..Default::default() };
    adapter.start(opts).unwrap().finished().await;
    let all = events.lock().unwrap().clone();
    all
}

/// A transcripts.read reply that found the session and carries `earlier`, read through the store.
async fn earlier_of(earlier: Value) -> f64 {
    let store = claude_conversations(ConversationDeps { config_dir: "/root/.claude-cfg".into(), ..Default::default() });
    let exec: Arc<dyn HarnessExec> =
        Arc::new(|_command: &str, _env: Option<&Env>| -> LocalBoxFuture<'static, Result<String, ExecFailure>> {
            async { Ok(String::new()) }.boxed_local()
        });
    let reply = json!({"found": true, "messages": [], "earlier": earlier});
    let road = ConversationRoad {
        exec,
        ask: Arc::new(move |_frame| {
            let reply = reply.clone();
            async move { Ok(reply.as_object().cloned().unwrap_or_default()) }.boxed_local()
        }),
    };
    let ask = EarlierAsk { cwds: vec!["/w".into()], last: 5, probe: false, tail: None, count: true };
    match store.earlier("sid".into(), ask, road).await.unwrap() {
        EarlierRead::Read(read) => read.earlier,
        other => panic!("expected a read, got {other:?}"),
    }
}

// adapter-claude/src/adapter.ts:1082: the held reply's turn.tasks names claudeSessionId (the resume's id until an init
// says otherwise); Rust turn.rs:1087 names the result line's session_id.
#[tokio::test]
async fn a_held_reply_names_the_session_the_turn_follows_not_the_result_lines_own() {
    tokio::task::LocalSet::new().run_until(a_held_reply_names_the_session_the_turn_follows_not_the_result_lines_own_body()).await
}

async fn a_held_reply_names_the_session_the_turn_follows_not_the_result_lines_own_body() {
    let lines = vec![
        json!({"type": "system", "subtype": "background_tasks_changed", "tasks": [{"task_id": "t1", "description": "d"}], "session_id": B})
            .to_string(),
        json!({"type": "assistant", "session_id": B, "message": {"id": "m1", "content": [{"type": "text", "text": "hi"}]}}).to_string(),
        json!({"type": "result", "subtype": "success", "is_error": false, "result": "hi", "duration_ms": 5, "session_id": B}).to_string(),
    ];
    let events = run_lines(lines, Some(A)).await;
    let held: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            AdapterEvent::TurnTasks { session_id, replied: Some(_), .. } => Some(session_id.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(held, vec![A]);
}

// adapter-claude/src/adapter.ts:468 passes `num(event.spawn_depth)` on as the number it was; Rust turn.rs:755 casts to u32.
#[tokio::test]
async fn a_subagent_depth_that_is_no_whole_number_goes_on_as_it_was() {
    tokio::task::LocalSet::new().run_until(a_subagent_depth_that_is_no_whole_number_goes_on_as_it_was_body()).await
}

async fn a_subagent_depth_that_is_no_whole_number_goes_on_as_it_was_body() {
    let mut seen = Vec::new();
    for depth in [json!(1.5), json!(-1)] {
        let started = json!({"type": "system", "subtype": "task_started", "task_id": "a1", "tool_use_id": "toolu_1", "task_type": "local_agent", "description": "d", "spawn_depth": depth, "session_id": A});
        let events = run_lines(vec![init_line(A), started.to_string(), result_line(A)], None).await;
        let subagent = events.iter().find(|e| matches!(e, AdapterEvent::Subagent { .. })).expect("a subagent event");
        seen.push(serde_json::to_value(subagent).unwrap()["depth"].clone());
    }
    assert_eq!(seen, vec![json!(1.5), json!(-1)]);
}

// adapter-claude/src/conversations.ts:104 hands `reply.earlier` on whole; Rust conversations.rs:226 casts it to u32.
#[tokio::test]
async fn the_earlier_count_a_transcripts_read_reports_is_handed_on_whole() {
    tokio::task::LocalSet::new().run_until(the_earlier_count_a_transcripts_read_reports_is_handed_on_whole_body()).await
}

async fn the_earlier_count_a_transcripts_read_reports_is_handed_on_whole_body() {
    assert_eq!(earlier_of(json!(-1)).await, -1.0);
    assert_eq!(earlier_of(json!(1e20)).await, 1e20);
}

// adapter-claude/src/adapter.ts:565 prints the 200-deep input back whole (node: 1201 chars). The port keeps wsp-words'
// KEPT_DEPTH cap, which guards the stack (ruled on #2447): every container nested past 128 is emptied, 746 chars.
#[tokio::test]
async fn a_tool_call_input_nested_past_128_levels_is_shown_with_its_inner_containers_emptied() {
    tokio::task::LocalSet::new().run_until(a_tool_call_input_nested_past_128_levels_is_shown_with_its_inner_containers_emptied_body()).await
}

async fn a_tool_call_input_nested_past_128_levels_is_shown_with_its_inner_containers_emptied_body() {
    let deep = format!("{}1{}", "{\"a\":".repeat(200), "}".repeat(200));
    let call = format!(
        "{{\"type\":\"assistant\",\"session_id\":\"{A}\",\"message\":{{\"id\":\"m2\",\"content\":[{{\"type\":\"tool_use\",\"id\":\"t1\",\"name\":\"Foo\",\"input\":{deep}}}]}}}}"
    );
    let events = run_lines(vec![init_line(A), call, result_line(A)], None).await;
    let call = events
        .iter()
        .find_map(|e| match e {
            AdapterEvent::TurnDelta(d) if d.kind == DeltaKind::ToolUse => Some(d.clone()),
            _ => None,
        })
        .expect("a tool_use delta");
    assert_eq!(call.text.len(), 746);
    assert_ne!(call.text, deep);
}
