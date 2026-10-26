// SPDX-License-Identifier: AGPL-3.0-only
//! The hard cases of units claude-turn and claude-adapter (spec/claude_turn.md, spec/claude_adapter.md): what survives
//! a host restart, re-entry from on_event, the transport's failure, the declared capabilities. The attach case was
//! run against the TS adapter at a3b870a6e and these are its events.

#[path = "common/claude_exec.rs"]
mod claude_exec;

use std::sync::{Arc, Mutex, OnceLock};

use claude_exec::{collect, types, until, FakeExec, Script, StreamOpts};
use serde_json::json;
use wsp_adapters::claude::landmines::user_message_line;
use wsp_adapters::claude::{create_claude_adapter, AdapterDeps, AttachedSession, ClaudeSession};
use wsp_adapters::port::*;
use wsp_wire::types::js_number::JsNumber;

const SID: &str = "e16ed170-8257-4668-879e-fe836341633c";

fn manual_attaching(taken: Vec<String>) -> Arc<FakeExec> {
    Arc::new(FakeExec {
        script: Script::Manual,
        opts: StreamOpts { run: Some("/tmp/run".into()), taken: Some(taken), ..Default::default() },
        calls: Default::default(),
        order: Default::default(),
        writes: Default::default(),
        streams: Default::default(),
        attach: Some(false),
        attached: Default::default(),
        held: Default::default(),
    })
}

fn deps(exec: &Arc<FakeExec>) -> AdapterDeps {
    AdapterDeps { result_exit_ms: Some(5), interrupt_grace_ms: Some(5), ..AdapterDeps::new(exec.clone(), "/root/.claude") }
}

fn init(capabilities: &[&str]) -> String {
    json!({"type": "system", "subtype": "init", "session_id": SID, "model": "m", "cwd": "/w", "tools": [], "capabilities": capabilities})
        .to_string()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_attached_turn_tells_a_kept_steer_unread() {
    tokio::task::LocalSet::new().run_until(an_attached_turn_tells_a_kept_steer_unread_body()).await
}

async fn an_attached_turn_tells_a_kept_steer_unread_body() {
    let taken = vec![user_message_line("first", SID, &[], None), user_message_line("steer one", SID, &[], Some("s1"))];
    let exec = manual_attaching(taken);
    let adapter = create_claude_adapter(deps(&exec)).unwrap();
    let (events, on_event) = collect();
    let attach = AdapterAttachOptions {
        run: "/tmp/run".into(),
        session_id: SID.into(),
        started_at: 1.0,
        model: None,
        cwd: None,
        prompt: None,
        effort: None,
        from: Some(40),
        steered: Some(vec!["s2".into()]),
        on_event,
    };
    let session = match adapter.attach(attach).expect("this exec attaches").await.expect("the machine answered") {
        AttachedSession::Session(s) => s,
        AttachedSession::Gone => panic!("the run is there"),
    };
    let (run, options) = exec.attached.lock().unwrap()[0].clone();
    assert_eq!((run.as_str(), options), ("/tmp/run", AttachRunOptions { input: true, started_at: 1.0, from: Some(40) }));
    assert_eq!(exec.calls().len(), 0, "nothing is launched");
    assert_eq!(
        session.steer("more", Some("s3".into()), vec![]).await,
        Ok(SteerOutcome::Accepted),
        "an attached CLI announced itself long ago: no wait for an init"
    );
    assert_eq!(exec.writes().iter().map(|w| claude_exec::json(w)["uuid"].clone()).collect::<Vec<_>>(), vec![json!("s3")]);
    exec.push(init(&["msg_lifecycle_v1"]));
    exec.push(json!({"type": "command_lifecycle", "command_uuid": "s1", "state": "started"}).to_string());
    exec.push(json!({"type": "command_lifecycle", "command_uuid": "s1", "state": "completed"}).to_string());
    exec.push(json!({"type": "assistant", "uuid": "a-1", "message": {"id": "m1", "model": "m", "content": [{"type": "text", "text": "ok"}]}, "session_id": SID}).to_string());
    exec.push(json!({"type": "result", "subtype": "success", "is_error": false, "result": "ok", "duration_ms": 3, "session_id": SID, "usage": {"input_tokens": 1, "output_tokens": 1}}).to_string());
    exec.end(Some(0));
    let result = session.finished().await;
    assert_eq!(
        result,
        TurnResult {
            status: TurnStatus::Completed,
            duration_ms: Some(JsNumber(3.0)),
            waited_ms: None,
            cost_usd: None,
            tokens: Some(TurnTokens {
                input: JsNumber(1.0),
                output: JsNumber(1.0),
                cached: None,
                cache_write: None,
                reasoning: None,
                context: None,
                window: None
            }),
            model: Some("m".into()),
            models: None,
            text: Some("ok".into()),
            error: None,
            unreached: None,
            refusal: None,
            limit: None
        },
        "an attached resume knows no saved totals, so its cost is left out"
    );
    let events = events.lock().unwrap().clone();
    assert_eq!(types(&events), vec!["session.start", "turn.delta", "turn.anchor", "turn.unread", "turn.done", "session.end"]);
    assert_eq!(
        events[3],
        AdapterEvent::TurnUnread { session_id: SID.into(), ids: vec!["s2".into(), "s3".into()] },
        "s1 was read; the kept s2 and the s3 written after the attach never were"
    );
    assert_eq!(events[2], AdapterEvent::TurnAnchor { session_id: SID.into(), anchor: "a-1".into(), kept: None });
    assert_eq!(
        (session.from(), session.run(), session.command(), session.tells_unread()),
        (Some(40), Some("/tmp/run".to_string()), None, true)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_prompt_answered_from_inside_on_event_closes_once() {
    tokio::task::LocalSet::new().run_until(a_prompt_answered_from_inside_on_event_closes_once_body()).await
}

async fn a_prompt_answered_from_inside_on_event_closes_once_body() {
    let exec = FakeExec::manual(StreamOpts::default());
    let adapter = create_claude_adapter(deps(&exec)).unwrap();
    let held: Arc<OnceLock<ClaudeSession>> = Default::default();
    let seen: Arc<Mutex<Vec<AdapterEvent>>> = Default::default();
    let (cell, sink) = (held.clone(), seen.clone());
    let on_event: OnEvent = Arc::new(move |event| {
        if let Some(session) = cell.get() {
            // Synchronous reads from inside the callback must not wait on the turn's own lock.
            let _ = (session.claude_session_id(), session.tells_unread(), session.kept().is_some());
            if let AdapterEvent::PermissionAsk { ask, .. } = &event {
                let (session, ask_id) = (session.clone(), ask.ask_id.clone());
                tokio::task::spawn_local(async move {
                    let answer = PermissionAnswer {
                        option_id: PERMISSION_ALLOW.into(),
                        outcome: PermissionOutcome::Allowed,
                        deny_message: String::new(),
                        reason: None,
                    };
                    let first = session.answer(&ask_id, answer.clone()).await;
                    let second = session.answer(&ask_id, answer).await;
                    assert_eq!((first, second), (Ok(AnswerOutcome::Answered), Ok(AnswerOutcome::Gone)));
                });
            }
        }
        sink.lock().unwrap().push(event);
    });
    let session = adapter
        .start(HarnessStartOptions {
            prompt: "go".into(),
            permission_mode: Some("default".into()),
            on_event: Some(on_event),
            ..Default::default()
        })
        .unwrap();
    held.set(session.clone()).ok();
    exec.push(init(&[]));
    exec.push(json!({"type": "control_request", "request_id": "req-1", "request": {"subtype": "can_use_tool", "tool_name": "Bash", "input": {"command": "ls"}, "tool_use_id": "tu-1"}}).to_string());
    until(|| seen.lock().unwrap().iter().any(|e| matches!(e, AdapterEvent::PermissionClose { .. }))).await;
    let closes: Vec<_> = seen.lock().unwrap().iter().filter(|e| matches!(e, AdapterEvent::PermissionClose { .. })).cloned().collect();
    assert_eq!(
        closes,
        vec![AdapterEvent::PermissionClose {
            session_id: SID.into(),
            ask_id: "req-1".into(),
            outcome: PermissionOutcome::Allowed,
            option_id: Some("allow".into())
        }]
    );
    let answer = exec.writes().last().cloned().expect("the answer was written");
    assert_eq!(
        claude_exec::json(&answer),
        json!({"type": "control_response", "response": {"subtype": "success", "request_id": "req-1", "response": {"behavior": "allow", "updatedInput": {"command": "ls"}}}})
    );
    exec.end(Some(0));
    session.finished().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_transport_failure_rides_session_end() {
    tokio::task::LocalSet::new().run_until(a_transport_failure_rides_session_end_body()).await
}

async fn a_transport_failure_rides_session_end_body() {
    let exec = FakeExec::manual(StreamOpts::default());
    let adapter = create_claude_adapter(deps(&exec)).unwrap();
    let (events, on_event) = collect();
    let session = adapter.start(HarnessStartOptions { prompt: "go".into(), on_event: Some(on_event), ..Default::default() }).unwrap();
    let failure = ExecFailure { kind: ExecFailureKind::Unreached, message: "the launch never reached the machine".into() };
    until(|| !exec.streams.lock().unwrap().is_empty()).await;
    exec.current().fail(failure.clone());
    exec.current().exit.resolve(None);
    let result = session.finished().await;
    assert_eq!(
        (result.status, result.error.as_deref()),
        (TurnStatus::Failed, Some("the launch never reached the machine")),
        "the transport's words are the turn's error, word for word"
    );
    let events = events.lock().unwrap().clone();
    assert_eq!(
        events.last(),
        Some(&AdapterEvent::SessionEnd { session_id: session.local_id(), exit_code: None, saw_result: false, failure: Some(failure) }),
        "the failure's kind reaches the runtime"
    );
}

#[test]
fn the_adapter_declares_what_claude_code_takes() {
    let exec = FakeExec::manual(StreamOpts::default());
    let base: wsp_adapters::claude::landmines::BaseEnv =
        [("CLAUDE_CONFIG_DIR".to_string(), Some("/root/.claude-cfg".to_string()))].into_iter().collect();
    let adapter = create_claude_adapter(AdapterDeps { base_env: Some(base), ..deps(&exec) }).unwrap();
    let h: &dyn HarnessAdapter = &adapter;
    assert_eq!(
        (
            h.steers(),
            h.reports_edits(),
            h.moves_access(),
            h.steers_images(),
            h.mcp_servers(),
            h.denies_servers(),
            h.waits_for_prompt(),
            h.aside_servers(),
            h.forks_by_count()
        ),
        (true, true, true, true, true, true, true, true, false)
    );
    assert_eq!(h.attachments(), Some(AttachmentRoad::Inline));
    assert_eq!(h.compacts().as_deref(), Some("/compact"));
    assert_eq!(h.terminal_resume().as_deref(), Some("CLAUDE_CONFIG_DIR=/root/.claude-cfg claude --resume"));
    assert_eq!(h.session_file(SID), Some(SessionFile { folder: "/root/.claude/projects".into(), name: format!("{SID}.jsonl"), depth: 1 }));
    assert!(
        h.revert(RevertAsk { session: SID.into(), cwd: None, at: RevertAt::BeforeTurn("a".into()) }).is_none(),
        "claude rewinds by a fork"
    );
    assert_eq!(
        h.screen_commands().iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
        vec!["login", "logout", "model", "permissions", "config", "help"]
    );
    assert_eq!(h.env(), Some(adapter.env()));
    assert!(h.conversations().is_some());
    assert!(create_claude_adapter(AdapterDeps::new(exec.clone(), " relative/dir"))
        .is_err_and(|e| e.0 == "configDir must be an absolute path, got \" relative/dir\""));
}
