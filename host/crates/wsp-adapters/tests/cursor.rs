// SPDX-License-Identifier: AGPL-3.0-only
//! packages/adapter-cursor/test/adapter.test.ts and command.test.ts at a3b870a6e.
//!
//! Fixtures: no-login.jsonl is what cursor-agent 2026.09.26-dd393fe printed under an empty home with no credential, exit 1.
//! turn.jsonl and every inline line below are hand-written: no real model turn ran.

use wsp_adapters::port::AttachmentRoad;

#[path = "common/codex_scripted.rs"]
mod codex_scripted;
#[path = "common/codex_shell.rs"]
mod codex_shell;

use indexmap::IndexMap;
use serde_json::json;
use wsp_adapters::cursor::adapter::{create_cursor_adapter, AttachedSession, CursorAdapter, CursorAdapterDeps, CursorSession};
use wsp_adapters::cursor::command::{build_command, build_env, BuildCommandOptions, CursorEnvOptions, FORCE_MODE};
use wsp_adapters::port::{agent_word, AdapterAttachOptions, AdapterEvent, AgentLaunch, DeltaKind, Env, ExecFailure, ExecFailureKind, HarnessStartOptions, TurnDelta, TurnResult, TurnStatus, TurnTokens};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::session::TurnRefusal;

use codex_scripted::{fixture, kind_of, no_events, scripted_exec, wait_for, Events, ScriptOpts, ScriptedExec};
use codex_shell::{bash, pairs, write_stub, TempDir};

const CHAT: &str = "c6b62c6f-7ead-4fd6-9922-e952131177ff";
const RUN_HANDLE: &str = "/tmp/wsp-run/cu01";
const SIGN_IN_ROAD: &str = "sign in from a terminal on this Mac, then send again";

#[derive(Default)]
struct Extra {
    result_exit_ms: Option<u64>,
    interrupt_grace_ms: Option<u64>,
    api_key: Option<String>,
    key_env: Option<String>,
}

fn adapter_over(exec: &ScriptedExec, extra: Extra) -> CursorAdapter {
    let mut base = IndexMap::new();
    base.insert("PATH".to_string(), Some("/root/.local/bin:/usr/bin".to_string()));
    create_cursor_adapter(CursorAdapterDeps {
        base_env: Some(base),
        sign_in_refusal: Some(SIGN_IN_ROAD.to_string()),
        result_exit_ms: extra.result_exit_ms,
        interrupt_grace_ms: extra.interrupt_grace_ms,
        api_key: extra.api_key,
        key_env: extra.key_env,
        ..CursorAdapterDeps::new(exec.factory())
    })
}

fn start(adapter: &CursorAdapter, options: HarnessStartOptions) -> CursorSession {
    adapter.start(options).unwrap_or_else(|e| panic!("start refused: {e}"))
}

fn go(events: &Events) -> HarnessStartOptions {
    HarnessStartOptions { prompt: "go".into(), on_event: Some(events.on_event()), ..Default::default() }
}

fn silent(prompt: &str) -> HarnessStartOptions {
    HarnessStartOptions { prompt: prompt.into(), on_event: Some(no_events()), ..Default::default() }
}

fn bare(status: TurnStatus) -> TurnResult {
    TurnResult { status, duration_ms: None, waited_ms: None, cost_usd: None, tokens: None, model: None, models: None, text: None, error: None, unreached: None, refusal: None, limit: None }
}

fn failed(error: &str) -> TurnResult {
    TurnResult { error: Some(error.to_string()), ..bare(TurnStatus::Failed) }
}

fn delta(kind: DeltaKind, text: &str) -> TurnDelta {
    TurnDelta::new(CHAT, kind, text)
}

fn init() -> String {
    json!({ "type": "system", "subtype": "init", "session_id": CHAT }).to_string()
}

fn tool_use(text: &str, name: &str, id: &str) -> AdapterEvent {
    AdapterEvent::TurnDelta(TurnDelta { tool_name: Some(name.into()), tool_use_id: Some(id.into()), ..delta(DeltaKind::ToolUse, text) })
}

fn tool_result(text: &str, id: &str, is_error: bool) -> AdapterEvent {
    AdapterEvent::TurnDelta(TurnDelta { tool_use_id: Some(id.into()), is_error: Some(is_error), ..delta(DeltaKind::ToolResult, text) })
}

fn session_end_fields(event: &AdapterEvent) -> (Option<i32>, bool) {
    match event {
        AdapterEvent::SessionEnd { exit_code, saw_result, .. } => (*exit_code, *saw_result),
        other => panic!("not a session.end: {other:?}"),
    }
}

mod cursor_adapter_over_an_agent_p_stream_json_turn {
    use super::*;

    #[tokio::test]
    async fn launches_the_print_line_with_the_key_in_the_environment_and_none_on_the_line_and() {
        let exec = scripted_exec(fixture("cursor", "turn.jsonl"), ScriptOpts::default(), RUN_HANDLE);
        let adapter = adapter_over(&exec, Extra { api_key: Some("key_x".into()), key_env: Some("CURSOR_API_KEY".into()), ..Default::default() });
        let session = start(
            &adapter,
            HarnessStartOptions {
                prompt: "summarize the readme".into(),
                cwd: Some("/root/app".into()),
                model: Some("sonnet-4.5".into()),
                permission_mode: Some("force".into()),
                on_event: Some(no_events()),
                ..Default::default()
            },
        );
        session.finished().await;
        let call = exec.calls().remove(0);
        assert!(
            call.command.starts_with(&format!(
                "cd '/root/app' && {} -p --output-format stream-json --stream-partial-output --trust --workspace '/root/app' --model 'sonnet-4.5' --force <<'WSP_PROMPT_END'",
                agent_word("cursor-agent", None)
            )),
            "{}",
            call.command
        );
        assert!(!call.command.contains("key_x"), "{}", call.command);
        let mut env = Env::new();
        env.insert("PATH".into(), "/root/.local/bin:/usr/bin".into());
        env.insert("CURSOR_API_KEY".into(), "key_x".into());
        assert_eq!(call.env, env);
        assert_eq!(call.input, None);
        assert_eq!(session.command(), Some(call.command.clone()));
        assert!(!adapter.steers());
        assert_eq!(adapter.attachments(), None);
        assert!(!adapter.mcp_servers());
    }

    #[tokio::test]
    async fn normalizes_a_turn_session_start_off_init_only_the_streamed_text_and_never_its_tw() {
        let events = Events::default();
        let session = start(
            &adapter_over(&scripted_exec(fixture("cursor", "turn.jsonl"), ScriptOpts::default(), RUN_HANDLE), Extra::default()),
            HarnessStartOptions { prompt: "go".into(), cwd: Some("/root/app".into()), model: Some("sonnet-4.5".into()), on_event: Some(events.on_event()), ..Default::default() },
        );
        let result = session.finished().await;
        assert_eq!(session.session_id(), CHAT);
        let all = events.all();
        assert_eq!(
            all.iter().map(kind_of).collect::<Vec<_>>(),
            ["session.start", "delta:text", "delta:text", "delta:tool_use", "delta:tool_result", "delta:text", "turn.done", "session.end"]
        );
        assert_eq!(
            all[0],
            AdapterEvent::SessionStart { session_id: CHAT.into(), model: Some("sonnet-4.5".into()), cwd: Some("/root/app".into()), tools: None, harness: None, prompted: None }
        );
        let streamed: String = all
            .iter()
            .filter_map(|e| match e {
                AdapterEvent::TurnDelta(d) if d.kind == DeltaKind::Text => Some(d.text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(streamed, "I will read the README.It is a sample project.");
        assert_eq!(all[3], tool_use(&serde_json::to_string(&json!({ "path": "README.md" })).unwrap(), "read", "toolu_vrtx_01Nn"));
        assert_eq!(all[4], tool_result("# Project\n\nThis is a sample project.", "toolu_vrtx_01Nn", false));
        // The reply is the text after the last tool call; the result's own field runs every segment together unspaced.
        assert_eq!(
            result,
            TurnResult {
                duration_ms: Some(JsNumber(5234.0)),
                text: Some("It is a sample project.".into()),
                tokens: Some(TurnTokens { input: JsNumber(1200.0), output: JsNumber(40.0), cached: None, cache_write: None, reasoning: None, context: None, window: None }),
                ..bare(TurnStatus::Completed)
            }
        );
        assert_eq!(all.last().unwrap(), &AdapterEvent::SessionEnd { session_id: CHAT.into(), exit_code: Some(0), saw_result: true, failure: None });
    }

    #[tokio::test]
    async fn a_tool_the_cli_refused_is_an_error_result_in_its_own_words_and_a_function_call_k() {
        let lines = vec![
            init(),
            json!({ "type": "tool_call", "subtype": "started", "call_id": "c1", "tool_call": { "shellToolCall": { "args": { "command": "rm -rf build" } } }, "session_id": CHAT }).to_string(),
            json!({ "type": "tool_call", "subtype": "completed", "call_id": "c1", "tool_call": { "shellToolCall": { "args": { "command": "rm -rf build" }, "result": { "rejected": { "reason": "not allowed without --force" } } } }, "session_id": CHAT }).to_string(),
            json!({ "type": "tool_call", "subtype": "started", "call_id": "c2", "tool_call": { "function": { "name": "web_search", "arguments": "{\"q\":\"x\"}" } }, "session_id": CHAT }).to_string(),
        ];
        let events = Events::default();
        start(&adapter_over(&scripted_exec(lines, ScriptOpts::default(), RUN_HANDLE), Extra::default()), go(&events)).finished().await;
        let all = events.all();
        assert!(all.contains(&tool_use(&serde_json::to_string(&json!({ "command": "rm -rf build" })).unwrap(), "shell", "c1")), "{all:?}");
        assert!(all.contains(&tool_result("rejected: not allowed without --force", "c1", true)), "{all:?}");
        assert!(all.contains(&tool_use("{\"q\":\"x\"}", "web_search", "c2")), "{all:?}");
    }

    #[tokio::test]
    async fn no_credential_is_a_sign_in_refusal_the_cli_s_sentence_first_and_wsp_s_road_besid() {
        let events = Events::default();
        let exec = scripted_exec(fixture("cursor", "no-login.jsonl"), ScriptOpts { exit_code: Some(1), ..Default::default() }, RUN_HANDLE);
        let result = start(&adapter_over(&exec, Extra::default()), HarnessStartOptions { prompt: "say hi".into(), on_event: Some(events.on_event()), ..Default::default() }).finished().await;
        assert_eq!(
            result,
            TurnResult {
                refusal: Some(TurnRefusal::SignIn),
                ..failed(&format!("Error: Authentication required. Please run 'agent login' first, or set CURSOR_API_KEY environment variable.; {SIGN_IN_ROAD}"))
            }
        );
        let all = events.all();
        assert_eq!(session_end_fields(all.last().unwrap()), (Some(1), false));
    }

    #[tokio::test]
    async fn a_stream_its_transport_failed_hands_that_very_error_to_the_runtime_on_session_en() {
        let lost = ExecFailure { kind: ExecFailureKind::NoAnswer, message: "remote launch failed on m1: daemon on m1 did not answer within 30000 ms".into() };
        let events = Events::default();
        let exec = scripted_exec(vec![], ScriptOpts { fails: Some(lost.clone()), ..Default::default() }, RUN_HANDLE);
        let result = start(&adapter_over(&exec, Extra::default()), go(&events)).finished().await;
        assert_eq!(result.status, TurnStatus::Failed);
        assert!(result.error.as_deref().unwrap_or_default().contains(&lost.message), "{:?}", result.error);
        match events.all().last().unwrap() {
            AdapterEvent::SessionEnd { failure, .. } => assert_eq!(failure, &Some(lost)),
            other => panic!("not a session.end: {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_process_that_dies_before_its_result_fails_the_turn_with_what_it_printed_last() {
        let lines = vec![init(), "Error: Cannot use this model: nope. Available models: gpt-5, sonnet-4.5".to_string()];
        let exec = scripted_exec(lines, ScriptOpts { exit_code: Some(1), ..Default::default() }, RUN_HANDLE);
        let result = start(&adapter_over(&exec, Extra::default()), silent("go")).finished().await;
        assert_eq!(result, failed("cursor-agent exited with code 1 before its turn ended: Error: Cannot use this model: nope. Available models: gpt-5, sonnet-4.5"));
    }

    #[tokio::test]
    async fn a_result_marked_as_an_error_fails_the_turn_with_its_own_text() {
        let lines = vec![json!({ "type": "result", "subtype": "error", "is_error": true, "duration_ms": 9, "result": "the model refused", "session_id": CHAT }).to_string()];
        let exec = scripted_exec(lines, ScriptOpts::default(), RUN_HANDLE);
        let result = start(&adapter_over(&exec, Extra::default()), silent("go")).finished().await;
        assert_eq!(result, TurnResult { duration_ms: Some(JsNumber(9.0)), ..failed("the model refused") });
    }

    #[tokio::test]
    async fn a_process_that_lingers_past_its_result_is_ended() {
        let exec = scripted_exec(fixture("cursor", "turn.jsonl"), ScriptOpts { hang: true, ..Default::default() }, RUN_HANDLE);
        let result = start(&adapter_over(&exec, Extra { result_exit_ms: Some(5), interrupt_grace_ms: Some(5), ..Default::default() }), silent("go")).finished().await;
        assert_eq!(result.status, TurnStatus::Completed);
        assert!(exec.order().contains(&"teardown".to_string()), "{:?}", exec.order());
    }

    #[tokio::test]
    async fn interrupt_ends_the_process_and_the_turn_reads_interrupted() {
        let exec = scripted_exec(vec![init()], ScriptOpts { hang: true, ..Default::default() }, RUN_HANDLE);
        let events = Events::default();
        let session = start(&adapter_over(&exec, Extra { interrupt_grace_ms: Some(5), ..Default::default() }), go(&events));
        wait_for(|| matches!(events.all().first(), Some(AdapterEvent::SessionStart { .. }))).await;
        session.interrupt().await;
        assert_eq!(session.finished().await, bare(TurnStatus::Interrupted));
        assert_eq!(exec.order()[0], "teardown");
    }

    #[tokio::test]
    async fn an_attach_re_opens_the_run_with_no_channel() {
        let exec = scripted_exec(fixture("cursor", "turn.jsonl"), ScriptOpts::default(), RUN_HANDLE).with_attach();
        let events = Events::default();
        let attached = adapter_over(&exec, Extra::default())
            .attach(AdapterAttachOptions {
                run: RUN_HANDLE.into(),
                session_id: CHAT.into(),
                started_at: 1.0,
                model: Some("sonnet-4.5".into()),
                cwd: Some("/root/app".into()),
                prompt: None,
                effort: None,
                from: None,
                steered: None,
                on_event: events.on_event(),
            })
            .expect("the exec factory has an attach")
            .await
            .unwrap_or_else(|e| panic!("attach failed: {e}"));
        let session = match attached {
            AttachedSession::Session(s) => s,
            AttachedSession::Gone => panic!("the run was there"),
        };
        assert_eq!(session.finished().await.status, TurnStatus::Completed);
        assert_eq!(exec.attached(), vec![(RUN_HANDLE.to_string(), false)]);
        match &events.all()[0] {
            AdapterEvent::SessionStart { session_id, model, cwd, .. } => {
                assert_eq!(session_id, CHAT);
                assert_eq!(model, &Some("sonnet-4.5".to_string()));
                assert_eq!(cwd, &Some("/root/app".to_string()));
            }
            other => panic!("not a session.start: {other:?}"),
        }
    }
}

mod a_person_s_setup_for_cursor_on_a_computer {
    use super::*;

    #[tokio::test]
    async fn runs_the_program_in_place_of_cursor_agent_the_launch_words_after_p() {
        let exec = scripted_exec(fixture("cursor", "turn.jsonl"), ScriptOpts::default(), RUN_HANDLE);
        let adapter = create_cursor_adapter(CursorAdapterDeps {
            launch: Some(AgentLaunch { program: Some("/opt/ca".into()), args: Some(vec!["--sandbox".into(), "disabled".into()]) }),
            ..CursorAdapterDeps::new(exec.factory())
        });
        start(&adapter, silent("hi")).finished().await;
        assert!(exec.calls()[0].command.contains("'/opt/ca' -p '--sandbox' 'disabled' --output-format stream-json"), "{}", exec.calls()[0].command);
    }
}

fn opts(prompt: &str) -> BuildCommandOptions {
    BuildCommandOptions { prompt: prompt.into(), ..Default::default() }
}

mod the_agent_print_line {
    use super::*;

    #[test]
    fn resumes_by_chat_and_takes_the_model() {
        let line = build_command(&BuildCommandOptions {
            cwd: Some("/root/app".into()),
            resume: Some(CHAT.into()),
            model: Some("claude-opus-4-8[context=1m,effort=high]".into()),
            ..opts("list the files")
        })
        .unwrap();
        assert!(line.contains(" --model 'claude-opus-4-8[context=1m,effort=high]'"), "{line}");
        assert!(line.contains(&format!(" --resume {CHAT}")), "{line}");
    }

    #[test]
    fn sends_force_on_the_force_mode_alone_and_refuses_a_mode_the_row_does_not_carry() {
        assert!(!build_command(&opts("hi")).unwrap().contains("--force"));
        assert!(!build_command(&BuildCommandOptions { permission_mode: Some("default".into()), ..opts("hi") }).unwrap().contains("--force"));
        assert!(build_command(&BuildCommandOptions { permission_mode: Some(FORCE_MODE.into()), ..opts("hi") }).unwrap().contains(" --force"));
        let err = build_command(&BuildCommandOptions { permission_mode: Some("yolo".into()), ..opts("hi") }).unwrap_err();
        assert!(err.contains("permissionMode must be one of default, force"), "{err}");
    }

    #[test]
    fn never_carries_a_key_it_rides_the_environment_where_no_process_list_shows_it() {
        let line = build_command(&BuildCommandOptions { permission_mode: Some(FORCE_MODE.into()), model: Some("gpt-5".into()), ..opts("hi") }).unwrap();
        assert!(!line.contains("api-key") && !line.contains("CURSOR_API_KEY"), "{line}");
        let mut base = IndexMap::new();
        base.insert("PATH".to_string(), Some("/usr/bin".to_string()));
        base.insert("GONE".to_string(), None);
        let mut keyed = Env::new();
        keyed.insert("PATH".into(), "/usr/bin".into());
        keyed.insert("CURSOR_API_KEY".into(), "key_x".into());
        assert_eq!(build_env(&CursorEnvOptions { base: Some(base), api_key: Some("key_x".into()), key_env: Some("CURSOR_API_KEY".into()) }), keyed);
        let mut plain_base = IndexMap::new();
        plain_base.insert("PATH".to_string(), Some("/usr/bin".to_string()));
        let mut plain = Env::new();
        plain.insert("PATH".into(), "/usr/bin".into());
        assert_eq!(build_env(&CursorEnvOptions { base: Some(plain_base), ..Default::default() }), plain);
    }

    #[test]
    fn the_prompt_reaches_the_binary_s_stdin_whole_through_the_heredoc_with_no_prompt_w() {
        let prompt = format!("{}\nit's \"quoted\" $HOME", "y".repeat(200_000));
        assert!(build_command(&opts(&prompt)).unwrap().contains(&format!("<<'WSP_PROMPT_END'\n{prompt}\nWSP_PROMPT_END")));
        let err = build_command(&opts("a\nWSP_PROMPT_END")).unwrap_err();
        assert!(err.contains("ends the prompt"), "{err}");
        let dir = TempDir::new("wsp-cursor-line-");
        write_stub(&dir.join("cursor-agent"), "#!/bin/sh\nprintf \"args:%s\\n\" \"$*\"\ncat\n");
        let line = build_command(&BuildCommandOptions { cwd: Some(dir.s()), ..opts("it's $HOME\nline two") }).unwrap();
        let ran = bash(&line, &pairs(&[("PATH", &format!("{}:/usr/bin:/bin", dir.s()))]));
        assert_eq!(ran.status, 0, "{}", ran.stderr);
        assert_eq!(ran.stdout, format!("args:-p --output-format stream-json --stream-partial-output --trust --workspace {}\nit's $HOME\nline two\n", dir.s()));
    }

    #[test]
    fn refuses_a_model_or_a_chat_id_that_would_read_as_a_flag() {
        let err = build_command(&BuildCommandOptions { model: Some("--force".into()), ..opts("x") }).unwrap_err();
        assert!(err.contains("model"), "{err}");
        let err = build_command(&BuildCommandOptions { resume: Some("--force".into()), ..opts("x") }).unwrap_err();
        assert!(err.contains("resume"), "{err}");
    }
}

/// Rules of spec/cursor.md no TS case pins, written from adapter.ts at the lines cited.
mod rules_this_port_pins {
    use super::*;

    #[tokio::test]
    async fn every_init_line_names_the_chat_and_says_the_start_again() {
        // Rule 6; adapter.ts:139-143.
        let second = serde_json::json!({"type": "system", "subtype": "init", "session_id": "chat-two"}).to_string();
        let events = Events::default();
        let exec = scripted_exec(vec![init(), second], ScriptOpts::default(), RUN_HANDLE);
        let session = start(&adapter_over(&exec, Extra::default()), go(&events));
        let _ = session.finished().await;
        let starts: Vec<String> = events.all().iter().filter_map(|e| match e { AdapterEvent::SessionStart { session_id, .. } => Some(session_id.clone()), _ => None }).collect();
        assert_eq!(starts, vec![CHAT.to_string(), "chat-two".to_string()]);
        assert_eq!(session.session_id(), "chat-two");
    }
}
