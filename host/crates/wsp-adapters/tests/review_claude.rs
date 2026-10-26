// Review of #2447: the claude session's interrupt and its kept agent's close act at the call, as the TS's do.
#[path = "common/claude_exec.rs"]
mod claude_exec;

use std::sync::Arc;

use claude_exec::{collect, until, FakeExec, StreamOpts};
use serde_json::json;
use wsp_adapters::claude::{create_claude_adapter, AdapterDeps};
use wsp_adapters::port::*;

const SID: &str = "e16ed170-8257-4668-879e-fe836341633c";

fn deps(exec: &Arc<FakeExec>) -> AdapterDeps {
    AdapterDeps {
        result_exit_ms: Some(5),
        interrupt_grace_ms: Some(5),
        stop_task_wait_ms: Some(5),
        steer_join_ms: Some(5),
        ..AdapterDeps::new(exec.clone(), "/root/.claude-cfg")
    }
}

// adapter-claude/src/adapter.ts:1235-1252 runs to endRun's teardown inside the call; runtime/src/threads/turns.ts:645
// calls it as `void started.interrupt()`. Node printed {"before":[],"rightAfterCall":["teardown"]}.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_interrupt_nobody_awaits_still_stops_the_turn() {
    tokio::task::LocalSet::new().run_until(an_interrupt_nobody_awaits_still_stops_the_turn_body()).await
}

async fn an_interrupt_nobody_awaits_still_stops_the_turn_body() {
    let exec = FakeExec::manual(StreamOpts::default());
    let adapter: Arc<dyn HarnessAdapter> = Arc::new(create_claude_adapter(deps(&exec)).unwrap());
    let (events, on_event) = collect();
    let session = adapter.start(HarnessStartOptions { prompt: "x".into(), on_event: Some(on_event), ..Default::default() }).unwrap();
    exec.push(json!({"type": "system", "subtype": "init", "session_id": SID, "model": "claude-x", "cwd": "/w", "tools": []}).to_string());
    until(|| events.lock().unwrap().iter().any(|e| matches!(e, AdapterEvent::SessionStart { .. }))).await;
    assert_eq!(exec.order(), Vec::<String>::new());
    drop(session.interrupt());
    assert_eq!(exec.order(), vec!["teardown"]);
}

// protocol/src/kept-run.ts:169-173: close closes the input and starts its end wait inside the call; runtime/src/threads/
// threads.ts:113 calls it as `void kept.agent.close(o)`. adapter-claude/src/adapter.ts kept().close passes now through.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_kept_close_nobody_awaits_still_ends_the_process() {
    tokio::task::LocalSet::new().run_until(a_kept_close_nobody_awaits_still_ends_the_process_body()).await
}

async fn a_kept_close_nobody_awaits_still_ends_the_process_body() {
    let exec = FakeExec::manual(StreamOpts::default());
    let adapter: Arc<dyn HarnessAdapter> = Arc::new(create_claude_adapter(deps(&exec)).unwrap());
    let (_events, on_event) = collect();
    let first =
        adapter.start(HarnessStartOptions { prompt: "one".into(), keep: true, on_event: Some(on_event), ..Default::default() }).unwrap();
    exec.push(json!({"type": "system", "subtype": "init", "session_id": SID, "model": "claude-x", "cwd": "/w", "tools": []}).to_string());
    exec.push(json!({"type": "result", "subtype": "success", "is_error": false, "result": "ok", "duration_ms": 40, "num_turns": 1, "session_id": SID, "total_cost_usd": 0.25, "usage": {"input_tokens": 10, "output_tokens": 5}}).to_string());
    first.finished().await;
    assert_eq!(exec.order(), Vec::<String>::new());
    let kept = first.kept().expect("claude answers kept").expect("kept, up, and its result seen");
    drop(kept.close(true));
    until(|| exec.order().len() >= 2).await;
    assert_eq!(exec.order(), vec!["closeInput", "teardown"]);
}
