// SPDX-License-Identifier: AGPL-3.0-only
//! Rules of spec/codex_turn.md that no case of packages/adapter-codex/test/adapter.test.ts pins, each written from the
//! TS code at the line the rule cites (a3b870a6e): the kept server's background hold, the keeper's release, the
//! registry, notes held until the thread is announced, and the one counter steers and interrupts share.

#[path = "common/codex_wire.rs"]
mod codex_wire;

use std::sync::Arc;

use codex_wire::*;
use serde_json::Value;
use wsp_adapters::codex::adapter::{create_codex_adapter, CodexAdapter, CodexAdapterDeps};
use wsp_adapters::port::{AdapterEvent, AsideQuestion, DeltaKind, HarnessStartOptions, KeptTurn, TaskStop, TurnStatus};

const THREAD: &str = "01a0e2c1-5d10-7b42-9a6e-3f1c2d4b5a60";
const TURN: &str = "01a0e2c1-5e02-7c11-8d3f-9b2a1c0d4e71";
const CHILD: &str = "019fcfd6-2883-77e0-9013-4410ede70371";
const CHILD_TURN: &str = "019fcfd6-28bf-7e00-a873-4554526dc845";

fn thread_answer() -> String {
    format!(
        r#"{{"id":"wsp-thread","result":{{"thread":{{"id":"{THREAD}","model":"gpt-5.6-sol","cwd":"/root/app","turns":[]}},"model":"gpt-5.6-sol","cwd":"/root/app"}}}}"#
    )
}
fn turn_started() -> String {
    format!(r#"{{"method":"turn/started","params":{{"threadId":"{THREAD}","turn":{{"id":"{TURN}","items":[],"status":"inProgress"}}}}}}"#)
}
fn completed(status: &str) -> String {
    format!(r#"{{"method":"turn/completed","params":{{"threadId":"{THREAD}","turn":{{"id":"{TURN}","items":[],"status":"{status}"}}}}}}"#)
}
fn command(phase: &str, id: &str) -> String {
    let status = if phase == "started" { "inProgress" } else { "completed" };
    format!(
        r#"{{"method":"item/{phase}","params":{{"item":{{"type":"commandExecution","id":"{id}","command":"/bin/zsh -lc 'sleep 60'","status":"{status}"}},"threadId":"{THREAD}","turnId":"{TURN}"}}}}"#
    )
}

fn adapter(launch: &Launch) -> CodexAdapter {
    create_codex_adapter(CodexAdapterDeps {
        interrupt_grace_ms: Some(15),
        result_exit_ms: Some(15),
        ..CodexAdapterDeps::new(launch.factory(), "/root/.codex", "codex login --device-auth")
    })
    .unwrap()
}

fn start(adapter: &CodexAdapter, prompt: &str, keep: bool, events: &Events) -> wsp_adapters::codex::CodexSession {
    adapter.start(HarnessStartOptions { prompt: prompt.into(), keep, on_event: Some(events.on_event()), ..Default::default() }).unwrap()
}

fn method_of(m: &Value) -> String {
    m.get("method").and_then(Value::as_str).unwrap_or("").to_string()
}

/// A server whose turn starts a background command and completes while it runs, the way codex-cli 0.155.1 leaves a
/// background command's item open past its turn (background.ts:1-4).
fn backgrounding() -> Launch {
    launcher(scripted_server(WireOpts {
        on_write: Some(Arc::new(|m: &Value, w: &Wire| match method_of(m).as_str() {
            "thread/start" | "thread/resume" => w.push(&[thread_answer()]),
            "turn/start" => w.push(&[turn_started(), command("started", "cmd_bg"), completed("completed")]),
            _ => {}
        })),
        ..Default::default()
    }))
}

#[tokio::test]
async fn a_kept_turn_that_leaves_a_command_running_holds_the_thread_and_lets_it_go_once_t() {
    tokio::task::LocalSet::new().run_until(a_kept_turn_that_leaves_a_command_running_holds_the_thread_and_lets_it_go_once_t_body()).await
}

async fn a_kept_turn_that_leaves_a_command_running_holds_the_thread_and_lets_it_go_once_t_body() {
    // spec rule 11 and 22; adapter.ts:758-762, background.ts:15-24.
    let launch = backgrounding();
    let events = Events::default();
    let first = start(&adapter(&launch), "one", true, &events);
    assert_eq!(first.finished().await.status, TurnStatus::Completed);
    let w = launch.wire(0);
    sleep_ms(10).await;
    assert!(!w.wrote("thread/unsubscribe"), "a running command keeps the thread subscribed");
    assert!(!w.closed(), "a kept server's stdin stays open");
    let kept = first.kept().expect("the server is kept");
    assert!((kept.working)(), "the command the turn left going is work the server still runs");
    w.push1(command("completed", "cmd_bg"));
    until(|| w.wrote("thread/unsubscribe")).await;
    let unsubscribe = w.written_method("thread/unsubscribe");
    assert_eq!(
        unsubscribe,
        vec![serde_json::json!({"id": "wsp-unsubscribe", "method": "thread/unsubscribe", "params": {"threadId": THREAD}})]
    );
    assert!(!(kept.working)());
}

#[tokio::test]
async fn a_failed_turn_releases_its_keeper_so_the_next_turn_launches_again() {
    tokio::task::LocalSet::new().run_until(a_failed_turn_releases_its_keeper_so_the_next_turn_launches_again_body()).await
}

async fn a_failed_turn_releases_its_keeper_so_the_next_turn_launches_again_body() {
    // spec rule 11; adapter.ts:753-755.
    let launch = launcher(scripted_server(WireOpts {
        on_write: Some(Arc::new(|m: &Value, w: &Wire| match method_of(m).as_str() {
            "thread/start" => w.push(&[thread_answer()]),
            "turn/start" => w.push(&[turn_started(), completed("failed")]),
            _ => {}
        })),
        ..Default::default()
    }));
    let session = start(&adapter(&launch), "one", true, &Events::default());
    assert_eq!(session.finished().await.status, TurnStatus::Failed);
    assert!(session.kept().is_none(), "a server is kept only past a turn that went as it should");
    assert!(!launch.wire(0).wrote("thread/unsubscribe"));
}

#[tokio::test]
async fn an_interrupt_with_a_command_running_takes_the_kept_server_down_with_the_turn() {
    tokio::task::LocalSet::new().run_until(an_interrupt_with_a_command_running_takes_the_kept_server_down_with_the_turn_body()).await
}

async fn an_interrupt_with_a_command_running_takes_the_kept_server_down_with_the_turn_body() {
    // spec rule 20; adapter.ts:1200-1202.
    let launch = launcher(scripted_server(WireOpts {
        on_write: Some(Arc::new(|m: &Value, w: &Wire| match method_of(m).as_str() {
            "thread/start" => w.push(&[thread_answer()]),
            "turn/start" => w.push(&[turn_started(), command("started", "cmd_fg")]),
            "turn/interrupt" => w.push(&[format!(r#"{{"id":{},"result":{{}}}}"#, m["id"]), completed("interrupted")]),
            _ => {}
        })),
        ..Default::default()
    }));
    let events = Events::default();
    let session = start(&adapter(&launch), "loop", true, &events);
    until(|| events.any("turn.anchor")).await;
    sleep_ms(5).await;
    session.interrupt().await;
    assert_eq!(session.finished().await.status, TurnStatus::Interrupted);
    assert!(session.kept().is_none(), "turn/interrupt leaves the command going, so the server goes with the turn");
    assert!(launch.wire(0).closed(), "a released keeper's view closes the process's own stdin");
}

#[tokio::test]
async fn a_turn_joins_the_registry_under_its_local_id_and_a_side_question_never_does() {
    tokio::task::LocalSet::new().run_until(a_turn_joins_the_registry_under_its_local_id_and_a_side_question_never_does_body()).await
}

async fn a_turn_joins_the_registry_under_its_local_id_and_a_side_question_never_does_body() {
    // spec rule 1; adapter.ts:1225.
    let launch = launcher(scripted_server(WireOpts {
        on_write: Some(Arc::new(|m: &Value, w: &Wire| match method_of(m).as_str() {
            "thread/resume" => w.push(&[thread_answer()]),
            "thread/fork" => w.push(&[r#"{"id":"wsp-thread","result":{"thread":{"id":"01a0e2d0-0000-7000-8000-000000000001"}}}"#]),
            "turn/start" => {
                let thread = m["params"]["threadId"].as_str().unwrap_or("").to_string();
                w.push(&[
                    format!(r#"{{"method":"item/completed","params":{{"item":{{"type":"agentMessage","id":"m1","text":"ok"}},"threadId":"{thread}","turnId":"t1"}}}}"#),
                    format!(r#"{{"method":"turn/completed","params":{{"threadId":"{thread}","turn":{{"id":"t1","items":[],"status":"completed"}}}}}}"#),
                ])
            }
            _ => {}
        })),
        ..Default::default()
    }));
    let a = adapter(&launch);
    let answer = a.aside(AsideQuestion { session: THREAD.into(), question: "where?".into(), ..Default::default() }).await.unwrap();
    assert_eq!(answer.text, "ok");
    assert!(a.sessions().is_empty(), "a side question joins no registry");
    let session = a
        .start(HarnessStartOptions { prompt: "go".into(), resume: Some(THREAD.into()), on_event: Some(no_events()), ..Default::default() })
        .unwrap();
    let _ = session.finished().await;
    assert_eq!(a.sessions().keys().cloned().collect::<Vec<_>>(), vec![THREAD.to_string()]);
}

#[tokio::test]
async fn notes_said_before_the_thread_is_announced_follow_its_session_start_in_order() {
    tokio::task::LocalSet::new().run_until(notes_said_before_the_thread_is_announced_follow_its_session_start_in_order_body()).await
}

async fn notes_said_before_the_thread_is_announced_follow_its_session_start_in_order_body() {
    // spec rule 2; adapter.ts:637-652.
    let launch = launcher(scripted_server(WireOpts {
        on_write: Some(Arc::new(|m: &Value, w: &Wire| match method_of(m).as_str() {
            "thread/start" => w.push(&[
                r#"{"method":"configWarning","params":{"summary":"first warning"}}"#.to_string(),
                r#"{"method":"warning","params":{"message":"second warning"}}"#.to_string(),
                r#"{"method":"warning","params":{"summary":7}}"#.to_string(),
                thread_answer(),
            ]),
            "turn/start" => w.push(&[turn_started(), completed("completed")]),
            _ => {}
        })),
        ..Default::default()
    }));
    let events = Events::default();
    let session = start(&adapter(&launch), "go", false, &events);
    let _ = session.finished().await;
    let all = events.all();
    assert!(matches!(&all[0], AdapterEvent::SessionStart { session_id, .. } if session_id == THREAD));
    let notes: Vec<(String, String)> =
        events.deltas().into_iter().filter(|d| d.kind == DeltaKind::Note).map(|d| (d.session_id, d.text)).collect();
    assert_eq!(notes, vec![(THREAD.to_string(), "first warning".to_string()), (THREAD.to_string(), "second warning".to_string())]);
    assert_eq!(events.kinds()[1..3], ["delta:note".to_string(), "delta:note".to_string()]);
}

#[tokio::test]
async fn steers_and_interrupts_are_numbered_off_one_counter() {
    tokio::task::LocalSet::new().run_until(steers_and_interrupts_are_numbered_off_one_counter_body()).await
}

async fn steers_and_interrupts_are_numbered_off_one_counter_body() {
    // spec rule 17; adapter.ts:604, 1137, 1157.
    let launch = launcher(scripted_server(WireOpts {
        on_write: Some(Arc::new(|m: &Value, w: &Wire| {
            match method_of(m).as_str() {
            "thread/start" => w.push(&[thread_answer()]),
            "turn/start" => w.push(&[
                turn_started(),
                format!(r#"{{"method":"turn/started","params":{{"threadId":"{CHILD}","turn":{{"id":"{CHILD_TURN}","items":[],"status":"inProgress"}}}}}}"#),
            ]),
            "turn/steer" | "turn/interrupt" => w.push(&[format!(r#"{{"id":{},"result":{{}}}}"#, m["id"])]),
            _ => {}
        }
        })),
        ..Default::default()
    }));
    let events = Events::default();
    let session = start(&adapter(&launch), "go", false, &events);
    until(|| events.any("subagent")).await;
    assert_eq!(session.steer("one".into()).await, wsp_adapters::port::SteerOutcome::Accepted);
    assert_eq!(session.stop_task(CHILD.into()).await, TaskStop::Accepted);
    assert_eq!(session.steer("two".into()).await, wsp_adapters::port::SteerOutcome::Accepted);
    let w = launch.wire(0);
    let ids: Vec<String> = w
        .written()
        .iter()
        .filter(|m| matches!(method_of(m).as_str(), "turn/steer" | "turn/interrupt"))
        .map(|m| m["id"].as_str().unwrap_or("").to_string())
        .collect();
    assert_eq!(ids, vec!["wsp-steer-1", "wsp-interrupt-2", "wsp-steer-3"]);
    w.push(&[completed("completed")]);
    w.exit(Some(0));
    let _ = session.finished().await;
}

#[tokio::test]
async fn a_kept_servers_next_turn_resumes_the_thread_with_the_first_turns_options_then_st() {
    tokio::task::LocalSet::new().run_until(a_kept_servers_next_turn_resumes_the_thread_with_the_first_turns_options_then_st_body()).await
}

async fn a_kept_servers_next_turn_resumes_the_thread_with_the_first_turns_options_then_st_body() {
    // spec rule 21; adapter.ts:1238-1260: the resume and the turn go together, the resume first.
    let launch = launcher(scripted_server(WireOpts {
        on_write: Some(Arc::new(|m: &Value, w: &Wire| match method_of(m).as_str() {
            "thread/start" | "thread/resume" => w.push(&[thread_answer()]),
            "turn/start" => w.push(&[turn_started(), completed("completed")]),
            "thread/unsubscribe" => w.push(&[r#"{"id":"wsp-unsubscribe","result":{}}"#]),
            _ => {}
        })),
        ..Default::default()
    }));
    let a = adapter(&launch);
    let first = a
        .start(HarnessStartOptions {
            prompt: "one".into(),
            keep: true,
            cwd: Some("/root/app".into()),
            model: Some("gpt-5.5".into()),
            permission_mode: Some("workspace-write".into()),
            on_event: Some(no_events()),
            ..Default::default()
        })
        .unwrap();
    let _ = first.finished().await;
    let w = launch.wire(0);
    until(|| w.wrote("thread/unsubscribe")).await;
    let before = w.written().len();
    let kept = first.kept().expect("kept");
    let second = (kept.next)(KeptTurn { prompt: "two".into(), images: vec![], after: None, on_event: no_events() }).unwrap();
    assert_eq!(second.finished().await.status, TurnStatus::Completed);
    let after: Vec<Value> = w.written()[before..].to_vec();
    assert_eq!(
        after[0],
        serde_json::json!({"id": "wsp-thread", "method": "thread/resume", "params": {"threadId": THREAD, "excludeTurns": true, "cwd": "/root/app", "model": "gpt-5.5", "sandbox": "workspace-write", "approvalPolicy": "on-request"}})
    );
    assert_eq!(method_of(&after[1]), "turn/start");
    assert_eq!(after[1]["params"]["input"][0]["text"], "two");
    assert_eq!(second.local_id(), THREAD, "keyed as a cold resume of the thread is, by the thread id");
    assert_eq!(launch.calls().len(), 1);
}
