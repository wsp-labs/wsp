// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the trait impls of commit 62879f6e9 (codex, opencode, cursor as `Arc<dyn HarnessAdapter>`), each case
//! against the TS object the factory returns at a3b870a6e. Every test fails on 47c39e1e5.

#[path = "common/codex_scripted.rs"]
mod codex_scripted;
#[path = "common/codex_wire.rs"]
mod codex_wire;

use std::sync::{Arc, Mutex};

use futures::future::{FutureExt, LocalBoxFuture};
use serde_json::{json, Value};
use wsp_adapters::codex::{create_codex_adapter, CodexAdapterDeps};
use wsp_adapters::cursor::adapter::{create_cursor_adapter, CursorAdapterDeps};
use wsp_adapters::opencode::adapter::{create_opencode_adapter, OpenCodeAdapterDeps};
use wsp_adapters::port::{DraftAsk, Env, ExecFailure, HarnessAdapter, HarnessExec, HarnessStartOptions, TitleTurn};

use codex_scripted::{no_events, scripted_exec, ScriptOpts, ScriptedExec};
use codex_wire::{launcher, scripted_server, OnWrite, WireOpts};

async fn settle_tasks(ms: u64) {
    for _ in 0..20 {
        tokio::task::yield_now().await;
    }
    tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
}

/// An exec that records each command and the env it was handed, and answers nothing.
#[derive(Clone, Default)]
struct Recorder(Arc<Mutex<Vec<(String, Option<Env>)>>>);

impl HarnessExec for Recorder {
    fn exec(&self, command: &str, env: Option<&Env>) -> LocalBoxFuture<'static, Result<String, ExecFailure>> {
        self.0.lock().unwrap().push((command.to_string(), env.cloned()));
        async { Ok(String::new()) }.boxed_local()
    }
}

fn dyn_exec(recorder: &Recorder) -> Arc<dyn HarnessExec> {
    Arc::new(recorder.clone())
}

fn codex_with_no_launch() -> Arc<dyn HarnessAdapter> {
    let exec = scripted_exec(Vec::new(), ScriptOpts::default(), "/tmp/wsp-run/cd34");
    Arc::new(create_codex_adapter(CodexAdapterDeps::new(exec.factory(), "/root/.codex", "codex login")).unwrap())
}

const ENV_READ: &str = "while IFS= read -r -d '' wsp_kv; do export \"$wsp_kv\"; done; unset wsp_kv";

fn hang() -> ScriptOpts {
    ScriptOpts { hang: true, ..Default::default() }
}

fn hi() -> HarnessStartOptions {
    HarnessStartOptions { prompt: "hi".into(), on_event: Some(no_events()), ..Default::default() }
}

/// A caller that does not await the interrupt, as the runtime's `void started.interrupt().catch(() => {})`.
async fn an_unawaited_interrupt_signals_the_process(adapter: Arc<dyn HarnessAdapter>, exec: &ScriptedExec) {
    let session = adapter.start(hi()).unwrap_or_else(|e| panic!("start refused: {e}"));
    drop(session.interrupt());
    settle_tasks(30).await;
    assert_eq!(exec.order(), vec!["teardown".to_string()]);
}

// adapter-opencode adapter.ts:238-241 (interrupt) with adapter-port.ts:362-363 (endRun tears down before its first await).
#[tokio::test]
async fn opencode_interrupt_nobody_awaits_still_tears_the_process_down() {
    tokio::task::LocalSet::new().run_until(opencode_interrupt_nobody_awaits_still_tears_the_process_down_body()).await
}

async fn opencode_interrupt_nobody_awaits_still_tears_the_process_down_body() {
    let exec = scripted_exec(Vec::new(), hang(), "/tmp/wsp-run/oc01");
    let adapter = Arc::new(create_opencode_adapter(OpenCodeAdapterDeps::new(exec.factory())));
    an_unawaited_interrupt_signals_the_process(adapter, &exec).await;
}

// adapter-cursor adapter.ts:210-213 (interrupt) with adapter-port.ts:362-363 (endRun tears down before its first await).
#[tokio::test]
async fn cursor_interrupt_nobody_awaits_still_tears_the_process_down() {
    tokio::task::LocalSet::new().run_until(cursor_interrupt_nobody_awaits_still_tears_the_process_down_body()).await
}

async fn cursor_interrupt_nobody_awaits_still_tears_the_process_down_body() {
    let exec = scripted_exec(Vec::new(), hang(), "/tmp/wsp-run/cu01");
    let adapter = Arc::new(create_cursor_adapter(CursorAdapterDeps::new(exec.factory())));
    an_unawaited_interrupt_signals_the_process(adapter, &exec).await;
}

const THREAD_ID: &str = "01a0e2c1-5d10-7b42-9a6e-3f1c2d4b5a60";
const TURN_ID: &str = "01a0e2c1-5e02-7c11-8d3f-9b2a1c0d4e71";

fn opened() -> Vec<String> {
    vec![
        json!({"id":"wsp-initialize","result":{"userAgent":"wsp/0.155.1","codexHome":"/root/.codex","platformFamily":"unix","platformOs":"linux"}}).to_string(),
        json!({"id":"wsp-thread","result":{"thread":{"id":THREAD_ID,"model":"gpt-5.6-sol","cwd":"/root/app","turns":[]},"model":"gpt-5.6-sol","cwd":"/root/app","approvalPolicy":"on-request","sandbox":{"type":"workspaceWrite"}}}).to_string(),
    ]
}

fn turn_started() -> String {
    json!({"method":"turn/started","params":{"threadId":THREAD_ID,"turn":{"id":TURN_ID,"items":[],"status":"inProgress"}}}).to_string()
}

fn completed() -> String {
    json!({"method":"turn/completed","params":{"threadId":THREAD_ID,"turn":{"id":TURN_ID,"items":[],"status":"completed"}}}).to_string()
}

// adapter-codex adapter.ts:1217 (kept close) with kept-run.ts:169-173 (close is an async function: its end wait is already running).
#[tokio::test]
async fn codex_kept_close_nobody_awaits_still_ends_a_process_that_ignores_eof() {
    tokio::task::LocalSet::new().run_until(codex_kept_close_nobody_awaits_still_ends_a_process_that_ignores_eof_body()).await
}

async fn codex_kept_close_nobody_awaits_still_ends_a_process_that_ignores_eof_body() {
    let on_write: OnWrite = Arc::new(|m, w| {
        let method = m.get("method").and_then(Value::as_str).unwrap_or("");
        if method == "thread/start" {
            w.push(&opened());
        }
        if method == "turn/start" {
            w.push(&[turn_started(), completed()]);
        }
        if method == "thread/unsubscribe" {
            w.push1(json!({"id":"wsp-unsubscribe","result":{"status":"unsubscribed"}}).to_string());
        }
    });
    let launch = launcher(scripted_server(WireOpts { hang: true, on_write: Some(on_write), ..Default::default() }));
    let adapter: Arc<dyn HarnessAdapter> = Arc::new(
        create_codex_adapter(CodexAdapterDeps {
            result_exit_ms: Some(60),
            interrupt_grace_ms: Some(40),
            ..CodexAdapterDeps::new(launch.factory(), "/root/.codex", "codex login")
        })
        .unwrap(),
    );
    let session =
        adapter.start(HarnessStartOptions { prompt: "one".into(), keep: true, on_event: Some(no_events()), ..Default::default() }).unwrap();
    session.finished().await;
    settle_tasks(20).await;
    let kept = session.kept().expect("codex sessions answer kept").expect("the turn left no kept agent");
    let wire = launch.wire(0);
    let before = wire.order().len();
    drop(kept.close(true));
    settle_tasks(200).await;
    assert_eq!(wire.order()[before..].to_vec(), vec!["teardown".to_string(), "kill".to_string()]);
}

// adapter-codex session-title.ts:74-77 (questionLine joins the word, "exec" and the flags, then a single space before --json).
#[tokio::test]
async fn codex_title_for_through_the_trait_runs_the_ts_command() {
    tokio::task::LocalSet::new().run_until(codex_title_for_through_the_trait_runs_the_ts_command_body()).await
}

async fn codex_title_for_through_the_trait_runs_the_ts_command_body() {
    let recorder = Recorder::default();
    let turn = TitleTurn { opening: "fix the bug".into(), reply: Some("done".into()), model: Some("gpt-5".into()) };
    codex_with_no_launch().title_for(turn, dyn_exec(&recorder)).unwrap().await.unwrap();
    let want = format!(
        "{ENV_READ}; cd ~ && codex exec --json --skip-git-repo-check -c sandbox_mode='\"read-only\"' -c approval_policy='\"never\"' -m gpt-5 - <<'WSP_PROMPT_END'\nName this coding agent thread in 3 to 6 words, no more than 34 characters, sentence case, no quotes and no full stop.\nDo not repeat the opening words, and do not say anything is done, fixed or working.\nAnswer with the title alone and nothing else; a longer answer is cut short.\n\nThe opening turn:\nfix the bug\n\nThe reply:\ndone\nWSP_PROMPT_END"
    );
    let commands: Vec<String> = recorder.0.lock().unwrap().iter().map(|(command, _)| command.clone()).collect();
    assert_eq!(commands, vec![want]);
}

// adapter-codex session-title.ts:84-85 (draftForCommand over questionLine, session-title.ts:74-77).
#[tokio::test]
async fn codex_draft_for_through_the_trait_runs_the_ts_command() {
    tokio::task::LocalSet::new().run_until(codex_draft_for_through_the_trait_runs_the_ts_command_body()).await
}

async fn codex_draft_for_through_the_trait_runs_the_ts_command_body() {
    let recorder = Recorder::default();
    codex_with_no_launch()
        .draft_for(DraftAsk { prompt_file: "/tmp/p.txt".into(), model: None }, dyn_exec(&recorder))
        .unwrap()
        .await
        .unwrap();
    let want = format!(
        "{ENV_READ}; cd ~ && codex exec --json --skip-git-repo-check -c sandbox_mode='\"read-only\"' -c approval_policy='\"never\"' - < '/tmp/p.txt'"
    );
    let commands: Vec<String> = recorder.0.lock().unwrap().iter().map(|(command, _)| command.clone()).collect();
    assert_eq!(commands, vec![want]);
}

// adapter-codex adapter.ts:1433 (renameSession calls exec as it is called, before the caller awaits anything).
#[tokio::test]
async fn codex_rename_session_nobody_awaits_still_runs_its_command() {
    tokio::task::LocalSet::new().run_until(codex_rename_session_nobody_awaits_still_runs_its_command_body()).await
}

async fn codex_rename_session_nobody_awaits_still_runs_its_command_body() {
    let recorder = Recorder::default();
    drop(codex_with_no_launch().rename_session("thr-1".into(), "New name".into(), dyn_exec(&recorder)).unwrap());
    settle_tasks(10).await;
    assert_eq!(recorder.0.lock().unwrap().len(), 1);
}
