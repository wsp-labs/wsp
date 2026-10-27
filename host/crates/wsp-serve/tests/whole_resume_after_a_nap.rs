// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/resume-after-a-nap.test.ts at 65f6b89c7, case for case. The adapters are the real ones and the
//! exec is scripted, so the line each harness would run on the machine is read word for word.
#[path = "../../wsp-adapters/tests/common/claude_exec.rs"]
mod claude_exec;
mod common;

use std::sync::Arc;

use claude_exec::{Call, FakeExec, StreamOpts};
use common::*;
use serde_json::{json, Value};
use wsp_adapters::port::agent_word;
use wsp_core::adapters::harness_adapters;
use wsp_core::types::api::StartOptions;
use wsp_core::types::harness::{HarnessAdapterContext, HarnessAdapterFactory};

const CLAUDE_SESSION: &str = "e16ed170-8257-4668-879e-fe836341633c";
const CODEX_THREAD: &str = "01a0e365-72f3-77e3-ba3a-3d18e12e9b95";

fn fixture(agent: &str, name: &str) -> Vec<String> {
    let path = format!("{}/../wsp-adapters/tests/fixtures/{agent}/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{path}: {e}"))
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(String::from)
        .collect()
}

/// `through(id, factory)`: the real adapter for that harness, launching through the scripted exec instead of the machine.
fn through(id: &str, exec: &Arc<FakeExec>) -> HarnessAdapterFactory {
    let real = harness_adapters().swap_remove(id).expect("a harness adapter");
    let exec = exec.clone();
    Arc::new(move |ctx: HarnessAdapterContext| real(HarnessAdapterContext { exec_stream: exec.clone(), ..ctx }))
}

struct Stopped {
    backend: Arc<StubBackend>,
    exec: Arc<FakeExec>,
    fx: Fx,
    workspace_id: String,
    thread_id: String,
}

/// A workspace with a thread that ran once and whose machine is then stopped.
async fn stopped_after_one_turn(harness: &str, lines: Vec<String>) -> Stopped {
    let exec = FakeExec::scripted(lines, StreamOpts::default());
    let backend = StubBackend::new(None);
    let fx = runtime(&backend).adapter(harness, through(harness, &exec)).build();
    let ws = create_on(&fx.rt, CreateOn::golden("pricing page"), None).await.expect("a workspace");
    let opened = fx
        .rt
        .sessions()
        .start(
            &ws.view.id,
            StartOptions { prompt: "start the dev server".into(), harness: Some(harness.into()), ..Default::default() },
            None,
        )
        .await
        .expect("a start");
    opened.finished().await;
    let thread_id = opened.view().thread_id.expect("a thread id");
    fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
    assert!(backend.machine(0).paused.get());
    Stopped { backend, exec, fx, workspace_id: ws.view.id, thread_id }
}

fn commands(exec: &FakeExec) -> Vec<String> {
    exec.calls().into_iter().map(|c: Call| c.command).collect()
}

// "a send after a stop" > "launches claude on the woken machine with the thread's own session id and the access its last turn ran at"
#[test]
fn launches_claude_on_the_woken_machine_with_the_threads_own_session_id_and_the_access_its_last_turn_ran_at() {
    drive(|| async {
        let s = stopped_after_one_turn("claude", fixture("claude", "stream-session.jsonl")).await;
        let first = commands(&s.exec);
        assert_eq!(first.len(), 1);
        // /--session-id [0-9a-f-]{36}$/
        let tail = first[0].rsplit_once("--session-id ").map(|(_, t)| t).unwrap_or_default();
        assert!(tail.len() == 36 && tail.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase() || c == '-'), "{}", first[0]);
        assert!(!first[0].contains("--resume"));
        s.fx.rt.workspaces().wake(&s.workspace_id, None).await.expect("a wake");
        let again =
            s.fx.rt
                .sessions()
                .start(
                    &s.workspace_id,
                    StartOptions { prompt: "what did you leave running".into(), thread: Some(s.thread_id.clone()), ..Default::default() },
                    None,
                )
                .await
                .expect("a start");
        again.finished().await;
        assert!(!s.backend.machine(0).paused.get());
        let all = commands(&s.exec);
        assert_eq!(all.len(), 2);
        assert!(all[1].contains(&format!("--resume {CLAUDE_SESSION}")), "{}", all[1]);
        assert!(!all[1].contains("--session-id"));
        // The access rides the resume: a turn that came back asking about each action would stop on the first one.
        assert!(all[1].contains("--dangerously-skip-permissions"));
        s.fx.rt.close().await.expect("a close");
    });
}

// "carries the access the thread's last turn ran at, not the kind's default"
#[test]
fn carries_the_access_the_threads_last_turn_ran_at_not_the_kinds_default() {
    drive(|| async {
        let exec = FakeExec::scripted(fixture("claude", "stream-session.jsonl"), StreamOpts::default());
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).adapter("claude", through("claude", &exec)).build();
        let ws = create_on(&fx.rt, CreateOn::golden("careful"), None).await.expect("a workspace");
        let opened = fx
            .rt
            .sessions()
            .start(
                &ws.view.id,
                StartOptions {
                    prompt: "go".into(),
                    harness: Some("claude".into()),
                    permission_mode: Some("acceptEdits".into()),
                    ..Default::default()
                },
                None,
            )
            .await
            .expect("a start");
        opened.finished().await;
        fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        fx.rt.workspaces().wake(&ws.view.id, None).await.expect("a wake");
        let again = fx
            .rt
            .sessions()
            .start(&ws.view.id, StartOptions { prompt: "more".into(), thread: opened.view().thread_id, ..Default::default() }, None)
            .await
            .expect("a start");
        again.finished().await;
        let all = commands(&exec);
        assert!(all[1].contains(&format!("--resume {CLAUDE_SESSION}")), "{}", all[1]);
        assert!(all[1].contains("--permission-mode 'acceptEdits'"), "{}", all[1]);
        assert!(!all[1].contains("--dangerously-skip-permissions"));
        fx.rt.close().await.expect("a close");
    });
}

// "resumes codex on the thread it started, at the access that asks nobody"
#[test]
fn resumes_codex_on_the_thread_it_started_at_the_access_that_asks_nobody() {
    drive(|| async {
        let s = stopped_after_one_turn("codex", fixture("codex", "app-server-turn.jsonl")).await;
        let first = commands(&s.exec);
        assert_eq!(first.len(), 1);
        // The server line carries the thread's own wsp server, which takes the place of any the person's config names.
        let want = format!("&& {} app-server -c tools.update_plan.enabled='true' -c mcp_servers.wsp.command=", agent_word("codex", None));
        assert!(first[0].contains(&want), "{}", first[0]);
        let thread_line = |at: usize| -> Value {
            let calls = s.exec.calls();
            let last = calls[at].input.as_ref().expect("a seed").last().expect("a seed line").clone();
            serde_json::from_str(&last).expect("a json line")
        };
        assert_eq!(thread_line(0)["method"], "thread/start");
        s.fx.rt.workspaces().wake(&s.workspace_id, None).await.expect("a wake");
        let again =
            s.fx.rt
                .sessions()
                .start(
                    &s.workspace_id,
                    StartOptions { prompt: "what did you leave running".into(), thread: Some(s.thread_id.clone()), ..Default::default() },
                    None,
                )
                .await
                .expect("a start");
        again.finished().await;
        assert!(!s.backend.machine(0).paused.get());
        let all = commands(&s.exec);
        assert_eq!(all.len(), 2);
        assert_matches(
            &thread_line(1),
            json!({ "method": "thread/resume", "params": { "threadId": CODEX_THREAD, "sandbox": "danger-full-access", "approvalPolicy": "never" } }),
        );
        s.fx.rt.close().await.expect("a close");
    });
}
