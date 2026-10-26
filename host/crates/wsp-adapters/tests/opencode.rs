// SPDX-License-Identifier: AGPL-3.0-only
//! packages/adapter-opencode/test/adapter.test.ts and command.test.ts at a3b870a6e.
//!
//! Fixtures: model-not-found.jsonl and catalog-probe.txt are what opencode 1.18.18 printed under an empty home with no
//! credential. turn.jsonl and every inline line below are hand-written: no real model turn ran.

use wsp_adapters::port::AttachmentRoad;

#[path = "common/codex_scripted.rs"]
mod codex_scripted;
#[path = "common/codex_shell.rs"]
mod codex_shell;

use std::sync::{Arc, Mutex};

use futures::future::{FutureExt, LocalBoxFuture};
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_adapters::opencode::adapter::create_opencode_adapter;
use wsp_adapters::opencode::adapter::{AttachedSession, OpenCodeAdapter, OpenCodeAdapterDeps, OpenCodeSession};
use wsp_adapters::opencode::command::{build_command, build_env, BuildCommandOptions, OpenCodeEnvOptions, AUTO_MODE};
use wsp_adapters::port::{
    agent_word, AdapterAttachOptions, AdapterEvent, AgentLaunch, DeltaKind, Env, ExecFailure, HarnessCatalogAnswer,
    HarnessCatalogModelProbe, HarnessCatalogProbe, HarnessExec, HarnessStartOptions, TurnDelta, TurnImage, TurnResult, TurnStatus,
    TurnTokens,
};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::session::TurnRefusal;

use codex_scripted::{fixture, kind_of, no_events, scripted_exec, wait_for, Events, ScriptOpts, ScriptedExec};
use codex_shell::{bash, pairs, write_stub, TempDir};

const SESSION: &str = "ses_f1d4a1b2cffeQwErTyUiOpAsDf";
const RUN_HANDLE: &str = "/tmp/wsp-run/oc01";
const SIGN_IN_ROAD: &str = "sign this workspace in from the Workspace panel, then send again";

#[derive(Default)]
struct Extra {
    result_exit_ms: Option<u64>,
    interrupt_grace_ms: Option<u64>,
}

fn path_base() -> IndexMap<String, Option<String>> {
    let mut base = IndexMap::new();
    base.insert("PATH".to_string(), Some("/root/.local/bin:/usr/bin".to_string()));
    base
}

fn adapter_over(exec: &ScriptedExec, extra: Extra) -> OpenCodeAdapter {
    create_opencode_adapter(OpenCodeAdapterDeps {
        base_env: Some(path_base()),
        sign_in_refusal: Some(SIGN_IN_ROAD.to_string()),
        result_exit_ms: extra.result_exit_ms,
        interrupt_grace_ms: extra.interrupt_grace_ms,
        ..OpenCodeAdapterDeps::new(exec.factory())
    })
}

fn start(adapter: &OpenCodeAdapter, options: HarnessStartOptions) -> OpenCodeSession {
    adapter.start(options).unwrap_or_else(|e| panic!("start refused: {e}"))
}

fn line(kind: &str, fields: Value) -> String {
    let mut object = json!({ "type": kind, "timestamp": 1790510000000_i64, "sessionID": SESSION });
    for (k, v) in fields.as_object().unwrap() {
        object[k] = v.clone();
    }
    serde_json::to_string(&object).unwrap()
}

fn n(v: f64) -> Option<JsNumber> {
    Some(JsNumber(v))
}

fn bare(status: TurnStatus) -> TurnResult {
    TurnResult {
        status,
        duration_ms: None,
        waited_ms: None,
        cost_usd: None,
        tokens: None,
        model: None,
        models: None,
        text: None,
        error: None,
        unreached: None,
        refusal: None,
        limit: None,
    }
}

fn failed(error: &str) -> TurnResult {
    TurnResult { error: Some(error.to_string()), ..bare(TurnStatus::Failed) }
}

fn delta(kind: DeltaKind, text: &str) -> TurnDelta {
    TurnDelta::new(SESSION, kind, text)
}

fn session_end_fields(event: &AdapterEvent) -> (&str, Option<i32>, bool) {
    match event {
        AdapterEvent::SessionEnd { session_id, exit_code, saw_result, .. } => (session_id.as_str(), *exit_code, *saw_result),
        other => panic!("not a session.end: {other:?}"),
    }
}

struct Probe {
    answer: String,
    asked: Arc<Mutex<Vec<String>>>,
}

impl HarnessExec for Probe {
    fn exec(&self, command: &str, _env: Option<&Env>) -> LocalBoxFuture<'static, Result<String, ExecFailure>> {
        self.asked.lock().unwrap().push(command.to_string());
        let answer = self.answer.clone();
        async move { Ok(answer) }.boxed_local()
    }
}

fn probe_over(answer: &str) -> (Arc<dyn HarnessExec>, Arc<Mutex<Vec<String>>>) {
    let asked = Arc::new(Mutex::new(Vec::new()));
    (Arc::new(Probe { answer: answer.to_string(), asked: asked.clone() }), asked)
}

fn turn_start(prompt: &str, events: &Events) -> HarnessStartOptions {
    HarnessStartOptions { prompt: prompt.into(), on_event: Some(events.on_event()), ..Default::default() }
}

mod opencode_adapter_over_an_opencode_run_format_json_turn {
    use super::*;

    #[tokio::test]
    async fn launches_the_run_line_with_the_picks_under_the_login_environment_the_prompt_on_t() {
        tokio::task::LocalSet::new()
            .run_until(launches_the_run_line_with_the_picks_under_the_login_environment_the_prompt_on_t_body())
            .await
    }

    async fn launches_the_run_line_with_the_picks_under_the_login_environment_the_prompt_on_t_body() {
        let exec = scripted_exec(fixture("opencode", "turn.jsonl"), ScriptOpts::default(), RUN_HANDLE);
        let adapter = adapter_over(&exec, Extra::default());
        let session = start(
            &adapter,
            HarnessStartOptions {
                prompt: "list the three largest files".into(),
                cwd: Some("/root/app".into()),
                model: Some("anthropic/claude-sonnet-4-5".into()),
                effort: Some("high".into()),
                permission_mode: Some("auto".into()),
                on_event: Some(no_events()),
                ..Default::default()
            },
        );
        session.finished().await;
        let call = exec.calls().remove(0);
        assert!(
            call.command.starts_with(&format!("cd '/root/app' && {} run --format json", agent_word("opencode", None))),
            "{}",
            call.command
        );
        assert!(
            call.command.contains(
                "-m anthropic/claude-sonnet-4-5 --variant high --auto <<'WSP_PROMPT_END'\nlist the three largest files\nWSP_PROMPT_END"
            ),
            "{}",
            call.command
        );
        assert_eq!(call.input, None);
        let mut env = Env::new();
        env.insert("PATH".into(), "/root/.local/bin:/usr/bin".into());
        assert_eq!(call.env, env);
        assert_eq!(session.command(), Some(call.command.clone()));
        assert!(!adapter.steers());
        assert_eq!(adapter.attachments(), Some(AttachmentRoad::File));
        assert!(!adapter.mcp_servers());
        assert_eq!(session.run(), Some(RUN_HANDLE.to_string()));
    }

    #[tokio::test]
    async fn normalizes_a_turn_into_session_start_text_and_tool_deltas_turn_done_at_the_last() {
        tokio::task::LocalSet::new().run_until(normalizes_a_turn_into_session_start_text_and_tool_deltas_turn_done_at_the_last_body()).await
    }

    async fn normalizes_a_turn_into_session_start_text_and_tool_deltas_turn_done_at_the_last_body() {
        let exec = scripted_exec(fixture("opencode", "turn.jsonl"), ScriptOpts::default(), RUN_HANDLE);
        let events = Events::default();
        let session = start(
            &adapter_over(&exec, Extra::default()),
            HarnessStartOptions {
                prompt: "go".into(),
                cwd: Some("/root/app".into()),
                model: Some("anthropic/claude-sonnet-4-5".into()),
                on_event: Some(events.on_event()),
                ..Default::default()
            },
        );
        let result = session.finished().await;
        assert_eq!(session.session_id(), SESSION);
        let all = events.all();
        assert_eq!(
            all.iter().map(kind_of).collect::<Vec<_>>(),
            ["session.start", "delta:text", "delta:tool_use", "delta:tool_result", "delta:text", "turn.done", "session.end"]
        );
        assert_eq!(
            all[0],
            AdapterEvent::SessionStart {
                session_id: SESSION.into(),
                model: Some("anthropic/claude-sonnet-4-5".into()),
                cwd: Some("/root/app".into()),
                tools: None,
                harness: None,
                prompted: None
            }
        );
        assert_eq!(
            all[1],
            AdapterEvent::TurnDelta(TurnDelta {
                message_id: Some("msg_f1d4a1b2c001".into()),
                ..delta(DeltaKind::Text, "I will list the files by size.")
            })
        );
        assert_eq!(
            all[2],
            AdapterEvent::TurnDelta(TurnDelta {
                tool_name: Some("bash".into()),
                tool_use_id: Some("call_f1d4ls01".into()),
                ..delta(
                    DeltaKind::ToolUse,
                    &serde_json::to_string(&json!({ "command": "ls -S | head -3", "description": "Lists the three largest files" }))
                        .unwrap()
                )
            })
        );
        assert_eq!(
            all[3],
            AdapterEvent::TurnDelta(TurnDelta {
                tool_use_id: Some("call_f1d4ls01".into()),
                is_error: Some(false),
                ..delta(DeltaKind::ToolResult, "pnpm-lock.yaml\nREADME.md\npackage.json\n")
            })
        );
        assert_eq!(result.status, TurnStatus::Completed);
        assert_eq!(result.text, Some("The three largest are pnpm-lock.yaml, README.md and package.json.".to_string()));
        assert_eq!(result.cost_usd, n(0.002));
        // Every step's input side summed, cache reads and writes in; what the model held is the last step's whole count.
        assert_eq!(
            result.tokens,
            Some(TurnTokens {
                input: JsNumber(3300.0),
                output: JsNumber(190.0),
                cached: n(1000.0),
                cache_write: n(0.0),
                reasoning: n(50.0),
                context: n(2340.0),
                window: None
            })
        );
        assert_eq!(
            all.last().unwrap(),
            &AdapterEvent::SessionEnd { session_id: SESSION.into(), exit_code: Some(0), saw_result: true, failure: None }
        );
    }

    #[tokio::test]
    async fn a_tool_part_that_failed_is_an_error_result_carrying_the_tool_s_own_error() {
        tokio::task::LocalSet::new().run_until(a_tool_part_that_failed_is_an_error_result_carrying_the_tool_s_own_error_body()).await
    }

    async fn a_tool_part_that_failed_is_an_error_result_carrying_the_tool_s_own_error_body() {
        let failed_part = line(
            "tool_use",
            json!({ "part": { "id": "prt_1", "sessionID": SESSION, "messageID": "msg_1", "type": "tool", "callID": "call_rd", "tool": "read", "state": { "status": "error", "input": { "filePath": "/nope" }, "error": "File not found: /nope", "time": { "start": 1, "end": 2 } } } }),
        );
        let events = Events::default();
        start(
            &adapter_over(&scripted_exec(vec![failed_part], ScriptOpts::default(), RUN_HANDLE), Extra::default()),
            turn_start("go", &events),
        )
        .finished()
        .await;
        assert!(events.all().contains(&AdapterEvent::TurnDelta(TurnDelta {
            tool_use_id: Some("call_rd".into()),
            is_error: Some(true),
            ..delta(DeltaKind::ToolResult, "File not found: /nope")
        })));
    }

    #[tokio::test]
    async fn the_turn_ends_at_the_step_whose_reason_ends_the_loop_and_a_process_that_lingers() {
        tokio::task::LocalSet::new().run_until(the_turn_ends_at_the_step_whose_reason_ends_the_loop_and_a_process_that_lingers_body()).await
    }

    async fn the_turn_ends_at_the_step_whose_reason_ends_the_loop_and_a_process_that_lingers_body() {
        let exec = scripted_exec(fixture("opencode", "turn.jsonl"), ScriptOpts { hang: true, ..Default::default() }, RUN_HANDLE);
        let events = Events::default();
        let result = start(&adapter_over(&exec, Extra { result_exit_ms: Some(5), interrupt_grace_ms: Some(5) }), turn_start("go", &events))
            .finished()
            .await;
        assert_eq!(result.status, TurnStatus::Completed);
        assert!(exec.order().contains(&"teardown".to_string()), "{:?}", exec.order());
        assert_eq!(events.all().iter().filter(|e| matches!(e, AdapterEvent::TurnDone { .. })).count(), 1);
    }

    #[tokio::test]
    async fn a_step_that_asked_for_tools_does_not_end_the_turn_the_process_exiting_after_it_w() {
        tokio::task::LocalSet::new()
            .run_until(a_step_that_asked_for_tools_does_not_end_the_turn_the_process_exiting_after_it_w_body())
            .await
    }

    async fn a_step_that_asked_for_tools_does_not_end_the_turn_the_process_exiting_after_it_w_body() {
        let lines: Vec<String> = fixture("opencode", "turn.jsonl").into_iter().take(4).collect();
        let exec = scripted_exec(lines, ScriptOpts::default(), RUN_HANDLE);
        let result = start(
            &adapter_over(&exec, Extra::default()),
            HarnessStartOptions { prompt: "go".into(), on_event: Some(no_events()), ..Default::default() },
        )
        .finished()
        .await;
        assert_eq!(result.status, TurnStatus::Completed);
        assert_eq!(result.text, Some("I will list the files by size.".to_string()));
    }

    #[tokio::test]
    async fn an_unknown_model_fails_the_turn_in_the_log_s_own_words_not_the_stream_s_server_e() {
        tokio::task::LocalSet::new()
            .run_until(an_unknown_model_fails_the_turn_in_the_log_s_own_words_not_the_stream_s_server_e_body())
            .await
    }

    async fn an_unknown_model_fails_the_turn_in_the_log_s_own_words_not_the_stream_s_server_e_body() {
        let exec = scripted_exec(
            fixture("opencode", "model-not-found.jsonl"),
            ScriptOpts { exit_code: Some(1), ..Default::default() },
            RUN_HANDLE,
        );
        let events = Events::default();
        let result = start(&adapter_over(&exec, Extra::default()), turn_start("say hi", &events)).finished().await;
        assert_eq!(result.status, TurnStatus::Failed);
        assert_eq!(
            result.error,
            Some("ProviderModelNotFoundError: Model not found: anthropic/claude-sonnet-4-5. Did you mean: claude-sonnet-4-5, claude-sonnet-4-5-20250929?".to_string())
        );
        let all = events.all();
        let (_, exit_code, saw_result) = session_end_fields(all.last().unwrap());
        assert_eq!((exit_code, saw_result), (Some(1), false));
    }

    #[tokio::test]
    async fn a_provider_that_refuses_the_credential_is_a_sign_in_refusal_with_wsp_s_road_besi() {
        tokio::task::LocalSet::new()
            .run_until(a_provider_that_refuses_the_credential_is_a_sign_in_refusal_with_wsp_s_road_besi_body())
            .await
    }

    async fn a_provider_that_refuses_the_credential_is_a_sign_in_refusal_with_wsp_s_road_besi_body() {
        let refused = line(
            "error",
            json!({ "error": { "name": "ProviderAuthError", "data": { "providerID": "anthropic", "message": "Invalid API key" } } }),
        );
        let exec = scripted_exec(vec![refused], ScriptOpts { exit_code: Some(1), ..Default::default() }, RUN_HANDLE);
        let result = start(
            &adapter_over(&exec, Extra::default()),
            HarnessStartOptions { prompt: "go".into(), on_event: Some(no_events()), ..Default::default() },
        )
        .finished()
        .await;
        assert_eq!(result, TurnResult { refusal: Some(TurnRefusal::SignIn), ..failed(&format!("Invalid API key; {SIGN_IN_ROAD}")) });
        let unauthorized = line(
            "error",
            json!({ "error": { "name": "APIError", "data": { "message": "Unauthorized", "statusCode": 401, "isRetryable": false } } }),
        );
        let exec = scripted_exec(vec![unauthorized], ScriptOpts { exit_code: Some(1), ..Default::default() }, RUN_HANDLE);
        let second = start(
            &adapter_over(&exec, Extra::default()),
            HarnessStartOptions { prompt: "go".into(), on_event: Some(no_events()), ..Default::default() },
        )
        .finished()
        .await;
        assert_eq!(second.refusal, Some(TurnRefusal::SignIn));
    }

    #[tokio::test]
    async fn a_process_that_dies_before_any_event_fails_the_turn_with_what_it_printed() {
        tokio::task::LocalSet::new().run_until(a_process_that_dies_before_any_event_fails_the_turn_with_what_it_printed_body()).await
    }

    async fn a_process_that_dies_before_any_event_fails_the_turn_with_what_it_printed_body() {
        let exec = scripted_exec(
            vec!["bash: opencode: command not found".into()],
            ScriptOpts { exit_code: Some(127), ..Default::default() },
            RUN_HANDLE,
        );
        let result = start(
            &adapter_over(&exec, Extra::default()),
            HarnessStartOptions { prompt: "go".into(), on_event: Some(no_events()), ..Default::default() },
        )
        .finished()
        .await;
        assert_eq!(result, failed("opencode exited with code 127 before its turn ended: bash: opencode: command not found"));
    }

    #[tokio::test]
    async fn interrupt_ends_the_process_and_the_turn_reads_interrupted() {
        tokio::task::LocalSet::new().run_until(interrupt_ends_the_process_and_the_turn_reads_interrupted_body()).await
    }

    async fn interrupt_ends_the_process_and_the_turn_reads_interrupted_body() {
        let exec = scripted_exec(
            vec![line("step_start", json!({ "part": { "id": "p", "sessionID": SESSION, "messageID": "m", "type": "step-start" } }))],
            ScriptOpts { hang: true, ..Default::default() },
            RUN_HANDLE,
        );
        let events = Events::default();
        let session = start(&adapter_over(&exec, Extra { interrupt_grace_ms: Some(5), ..Default::default() }), turn_start("go", &events));
        wait_for(|| matches!(events.all().first(), Some(AdapterEvent::SessionStart { .. }))).await;
        session.interrupt().await;
        assert_eq!(session.finished().await, bare(TurnStatus::Interrupted));
        assert_eq!(exec.order()[0], "teardown");
        let all = events.all();
        let (session_id, _, saw_result) = session_end_fields(all.last().unwrap());
        assert_eq!((session_id, saw_result), (SESSION, false));
    }

    #[tokio::test]
    async fn an_attach_re_opens_the_run_with_no_channel_and_takes_the_model_and_folder_off_th() {
        tokio::task::LocalSet::new()
            .run_until(an_attach_re_opens_the_run_with_no_channel_and_takes_the_model_and_folder_off_th_body())
            .await
    }

    async fn an_attach_re_opens_the_run_with_no_channel_and_takes_the_model_and_folder_off_th_body() {
        let exec = scripted_exec(fixture("opencode", "turn.jsonl"), ScriptOpts::default(), RUN_HANDLE).with_attach();
        let events = Events::default();
        let attached = adapter_over(&exec, Extra::default())
            .attach(AdapterAttachOptions {
                run: RUN_HANDLE.into(),
                session_id: SESSION.into(),
                started_at: 1.0,
                model: Some("anthropic/claude-sonnet-4-5".into()),
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
        assert_eq!(session.command(), None);
        assert_eq!(session.finished().await.status, TurnStatus::Completed);
        assert_eq!(exec.attached(), vec![(RUN_HANDLE.to_string(), false)]);
        match &events.all()[0] {
            AdapterEvent::SessionStart { session_id, model, cwd, .. } => {
                assert_eq!(session_id, SESSION);
                assert_eq!(model, &Some("anthropic/claude-sonnet-4-5".to_string()));
                assert_eq!(cwd, &Some("/root/app".to_string()));
            }
            other => panic!("not a session.start: {other:?}"),
        }
    }

    #[tokio::test]
    async fn an_image_reaches_the_line_as_a_file_on_the_machine_and_one_with_no_path_is_refus() {
        tokio::task::LocalSet::new()
            .run_until(an_image_reaches_the_line_as_a_file_on_the_machine_and_one_with_no_path_is_refus_body())
            .await
    }

    async fn an_image_reaches_the_line_as_a_file_on_the_machine_and_one_with_no_path_is_refus_body() {
        let exec = scripted_exec(fixture("opencode", "turn.jsonl"), ScriptOpts::default(), RUN_HANDLE);
        let adapter = adapter_over(&exec, Extra::default());
        start(
            &adapter,
            HarnessStartOptions {
                prompt: "what is this".into(),
                images: vec![TurnImage {
                    media_type: "image/png".into(),
                    bytes: "".into(),
                    path: Some("/root/.wsp/images/t/a.png".into()),
                }],
                on_event: Some(no_events()),
                ..Default::default()
            },
        );
        assert!(exec.calls()[0].command.contains("-f '/root/.wsp/images/t/a.png'"), "{}", exec.calls()[0].command);
        let refused = adapter
            .start(HarnessStartOptions {
                prompt: "x".into(),
                images: vec![TurnImage { media_type: "image/png".into(), bytes: "AAAA".into(), path: None }],
                on_event: Some(no_events()),
                ..Default::default()
            })
            .err()
            .expect("an image with no path is refused");
        assert!(refused.0.contains("no path"), "{refused}");
    }
}

mod what_opencode_reports_about_itself {
    use super::*;

    #[tokio::test]
    async fn reads_its_version_and_each_model_with_the_variants_it_offers_off_opencode_models() {
        tokio::task::LocalSet::new()
            .run_until(reads_its_version_and_each_model_with_the_variants_it_offers_off_opencode_models_body())
            .await
    }

    async fn reads_its_version_and_each_model_with_the_variants_it_offers_off_opencode_models_body() {
        let (exec, asked) = probe_over(&fixture("opencode", "catalog-probe.txt").join("\n"));
        let probe =
            adapter_over(&scripted_exec(vec![], ScriptOpts::default(), RUN_HANDLE), Extra::default()).probe_catalog(exec).await.unwrap();
        assert_eq!(
            asked.lock().unwrap()[0],
            "cd ~ && export PATH='/root/.local/bin:/usr/bin'; opencode --version; echo __WSP_CATALOG_SEP__; opencode models --verbose"
        );
        let efforts = |names: &[&str]| Some(names.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        let model = |slug: &str, label: &str, efforts: Option<Vec<String>>| HarnessCatalogModelProbe {
            slug: slug.into(),
            label: label.into(),
            description: None,
            efforts,
            default_effort: None,
            context_windows: vec![],
            is_default: false,
            fast: None,
        };
        assert_eq!(
            probe,
            Some(HarnessCatalogAnswer::Probe(HarnessCatalogProbe {
                version: Some("1.18.18".into()),
                models: vec![
                    model("opencode/big-pickle", "Big Pickle", efforts(&[])),
                    model("opencode/ling-3.0-flash-fin-free", "Ling 3.0 Flash Fin Free", efforts(&["low", "medium", "high"])),
                ],
                efforts: vec!["low".into(), "medium".into(), "high".into()],
                permission_modes: vec!["auto".into()],
            }))
        );
    }

    #[tokio::test]
    async fn answers_nothing_when_the_binary_printed_no_model() {
        tokio::task::LocalSet::new().run_until(answers_nothing_when_the_binary_printed_no_model_body()).await
    }

    async fn answers_nothing_when_the_binary_printed_no_model_body() {
        let adapter = adapter_over(&scripted_exec(vec![], ScriptOpts::default(), RUN_HANDLE), Extra::default());
        let (exec, _) = probe_over("bash: opencode: command not found\n");
        assert_eq!(adapter.probe_catalog(exec).await.unwrap(), None);
        let (exec, _) = probe_over("1.18.18\n__WSP_CATALOG_SEP__\n");
        assert_eq!(adapter.probe_catalog(exec).await.unwrap(), None);
    }
}

mod a_person_s_setup_for_opencode_on_a_computer {
    use super::*;

    #[tokio::test]
    async fn runs_the_program_in_place_of_opencode_on_a_turn_and_a_probe_the_launch_words_on() {
        tokio::task::LocalSet::new().run_until(runs_the_program_in_place_of_opencode_on_a_turn_and_a_probe_the_launch_words_on_body()).await
    }

    async fn runs_the_program_in_place_of_opencode_on_a_turn_and_a_probe_the_launch_words_on_body() {
        let exec = scripted_exec(fixture("opencode", "turn.jsonl"), ScriptOpts::default(), RUN_HANDLE);
        let adapter = create_opencode_adapter(OpenCodeAdapterDeps {
            launch: Some(AgentLaunch { program: Some("/opt/oc".into()), args: Some(vec!["--port".into(), "0".into()]) }),
            ..OpenCodeAdapterDeps::new(exec.factory())
        });
        start(&adapter, HarnessStartOptions { prompt: "hi".into(), on_event: Some(no_events()), ..Default::default() }).finished().await;
        assert!(exec.calls()[0].command.contains("'/opt/oc' run '--port' '0' --format json"), "{}", exec.calls()[0].command);
        let (probe, asked) = probe_over("");
        adapter.probe_catalog(probe).await.unwrap();
        let first = asked.lock().unwrap()[0].clone();
        assert!(first.contains("'/opt/oc' --version"), "{first}");
        assert!(first.contains("'/opt/oc' models --verbose"), "{first}");
        assert!(!first.contains("--port"), "{first}");
    }
}

fn opts(prompt: &str) -> BuildCommandOptions {
    BuildCommandOptions { prompt: prompt.into(), ..Default::default() }
}

mod the_opencode_run_line {
    use super::*;

    #[test]
    fn runs_one_json_turn_in_the_folder_resumes_by_session_and_takes_the_model_the_vari() {
        let line = build_command(&BuildCommandOptions {
            cwd: Some("/root/app".into()),
            resume: Some("ses_f1d55cde0ffe2JZqKKpAnaa1c3".into()),
            model: Some("openrouter/anthropic/claude-sonnet-4.5".into()),
            effort: Some("high".into()),
            title: Some("Largest files".into()),
            permission_mode: Some(AUTO_MODE.into()),
            ..opts("list the files")
        })
        .unwrap();
        assert!(
            line.starts_with(&format!(
                "cd '/root/app' && {} run --format json --print-logs --log-level ERROR --dir '/root/app'",
                agent_word("opencode", None)
            )),
            "{line}"
        );
        assert!(line.contains(" -s ses_f1d55cde0ffe2JZqKKpAnaa1c3"), "{line}");
        assert!(line.contains(" -m openrouter/anthropic/claude-sonnet-4.5"), "{line}");
        assert!(line.contains(" --variant high"), "{line}");
        assert!(line.contains(" --title='Largest files'"), "{line}");
        assert!(line.contains(" --auto"), "{line}");
    }

    #[test]
    fn sends_auto_on_the_auto_mode_alone_and_refuses_any_other_mode() {
        assert!(!build_command(&opts("hi")).unwrap().contains("--auto"));
        let err = build_command(&BuildCommandOptions { permission_mode: Some("default".into()), ..opts("hi") }).unwrap_err();
        assert!(err.contains("permissionMode must be auto"), "{err}");
    }

    #[test]
    fn carries_the_prompt_on_stdin_never_as_an_argument_so_no_length_meets_the_per_argu() {
        let prompt = format!("{}\nit's \"quoted\" $HOME `not run`", "x".repeat(200_000));
        let line = build_command(&opts(&prompt)).unwrap();
        assert!(line.contains(&format!("<<'WSP_PROMPT_END'\n{prompt}\nWSP_PROMPT_END")));
        let err = build_command(&opts("one\nWSP_PROMPT_END\ntwo")).unwrap_err();
        assert!(err.contains("ends the prompt"), "{err}");
    }

    #[test]
    fn the_heredoc_reaches_the_binary_s_stdin_whole_with_nothing_on_its_argument_list() {
        let dir = TempDir::new("wsp-opencode-line-");
        // A stand-in that prints its arguments and then its stdin, so the shell line runs as the machine would run it.
        write_stub(&dir.join("opencode"), "#!/bin/sh\nprintf \"args:%s\\n\" \"$*\"\ncat\n");
        let line = build_command(&BuildCommandOptions { cwd: Some(dir.s()), ..opts("it's $HOME\nline two") }).unwrap();
        let ran = bash(&line, &pairs(&[("PATH", &format!("{}:/usr/bin:/bin", dir.s()))]));
        assert_eq!(ran.status, 0, "{}", ran.stderr);
        assert_eq!(ran.stdout, format!("args:run --format json --print-logs --log-level ERROR --dir {}\nit's $HOME\nline two\n", dir.s()));
    }

    #[test]
    fn takes_one_f_per_image_each_an_absolute_path_after_every_other_flag() {
        let line = build_command(&BuildCommandOptions {
            images: vec!["/root/.wsp/images/t1/a.png".into(), "/root/.wsp/images/t1/b c.png".into()],
            ..opts("what is this")
        })
        .unwrap();
        assert!(line.contains(" -f '/root/.wsp/images/t1/a.png' -f '/root/.wsp/images/t1/b c.png' <<'WSP_PROMPT_END'"), "{line}");
        let err = build_command(&BuildCommandOptions { images: vec!["-rf".into()], ..opts("x") }).unwrap_err();
        assert!(err.contains("absolute path"), "{err}");
    }

    #[test]
    fn refuses_a_value_that_would_read_as_a_flag_or_break_out_of_its_word() {
        let err = build_command(&BuildCommandOptions { resume: Some("--auto".into()), ..opts("x") }).unwrap_err();
        assert!(err.contains("resume"), "{err}");
        let err = build_command(&BuildCommandOptions { model: Some("-m".into()), ..opts("x") }).unwrap_err();
        assert!(err.contains("provider/model"), "{err}");
        let err = build_command(&BuildCommandOptions { model: Some("claude-sonnet".into()), ..opts("x") }).unwrap_err();
        assert!(err.contains("provider/model"), "{err}");
        let err = build_command(&BuildCommandOptions { effort: Some("high; rm -rf /".into()), ..opts("x") }).unwrap_err();
        assert!(err.contains("effort"), "{err}");
    }
}

mod the_opencode_turn_s_environment {
    use super::*;

    #[test]
    fn is_the_machine_s_login_environment_with_the_vault_s_key_under_the_variable_its_r() {
        let mut base = IndexMap::new();
        base.insert("PATH".to_string(), Some("/usr/bin".to_string()));
        base.insert("GONE".to_string(), None);
        let mut plain = Env::new();
        plain.insert("PATH".into(), "/usr/bin".into());
        assert_eq!(build_env(&OpenCodeEnvOptions { base: Some(base), ..Default::default() }), plain);
        let mut keyed_base = IndexMap::new();
        keyed_base.insert("PATH".to_string(), Some("/usr/bin".to_string()));
        let mut keyed = plain.clone();
        keyed.insert("OPENCODE_API_KEY".into(), "sk-x".into());
        assert_eq!(
            build_env(&OpenCodeEnvOptions {
                base: Some(keyed_base),
                api_key: Some("sk-x".into()),
                key_env: Some("OPENCODE_API_KEY".into())
            }),
            keyed
        );
    }
}

/// Rules of spec/opencode.md no TS case pins, written from adapter.ts at the lines cited.
mod rules_this_port_pins {
    use super::*;

    #[tokio::test]
    async fn the_first_error_is_the_turns_failure_and_a_later_one_changes_nothing() {
        tokio::task::LocalSet::new().run_until(the_first_error_is_the_turns_failure_and_a_later_one_changes_nothing_body()).await
    }

    async fn the_first_error_is_the_turns_failure_and_a_later_one_changes_nothing_body() {
        // Rule 11; adapter.ts:202 (`failure ??=`).
        let first = line("error", json!({"error": {"name": "APIError", "data": {"message": "first words"}}}));
        let second = line("error", json!({"error": {"name": "ProviderAuthError", "data": {"message": "second words"}}}));
        let exec = scripted_exec(vec![first, second], ScriptOpts { exit_code: Some(1), ..Default::default() }, RUN_HANDLE);
        let result = start(&adapter_over(&exec, Extra::default()), turn_start("go", &Events::default())).finished().await;
        assert_eq!(result, failed("first words"));
    }

    #[tokio::test]
    async fn a_text_part_that_trims_empty_is_neither_said_nor_the_reply() {
        tokio::task::LocalSet::new().run_until(a_text_part_that_trims_empty_is_neither_said_nor_the_reply_body()).await
    }

    async fn a_text_part_that_trims_empty_is_neither_said_nor_the_reply_body() {
        // Rule 8; adapter.ts:183.
        let said = line("text", json!({"part": {"type": "text", "text": "the reply", "messageID": "m1"}}));
        let blank = line("text", json!({"part": {"type": "text", "text": "  \n"}}));
        let events = Events::default();
        let exec = scripted_exec(vec![said, blank], ScriptOpts::default(), RUN_HANDLE);
        let result = start(&adapter_over(&exec, Extra::default()), turn_start("go", &events)).finished().await;
        assert_eq!(result.text.as_deref(), Some("the reply"));
        let texts: Vec<String> = events
            .all()
            .iter()
            .filter_map(|e| match e {
                AdapterEvent::TurnDelta(d) => Some(d.text.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(texts, vec!["the reply".to_string()]);
    }
}

/// Rules O1 to O10 of spec/opencode.md, "Rules added for the port", each value printed by the TS at 65f6b89c7.
mod rules_added {
    use super::*;
    use wsp_adapters::opencode::catalog::parse_catalog_probe;

    const ROAD_WORDS: &str = SIGN_IN_ROAD;

    fn stop(tokens: Option<Value>, reason: &str) -> String {
        let mut part = json!({"type": "step-finish", "reason": reason, "cost": 0});
        if let Some(t) = tokens {
            part["tokens"] = t;
        }
        line("step_finish", json!({ "part": part }))
    }

    async fn run(lines: Vec<String>, opts: ScriptOpts, resume: Option<&str>) -> (TurnResult, Events, OpenCodeSession, OpenCodeAdapter) {
        let events = Events::default();
        let exec = scripted_exec(lines, opts, RUN_HANDLE);
        let adapter = adapter_over(&exec, Extra { result_exit_ms: Some(5), interrupt_grace_ms: Some(5) });
        let session = start(&adapter, HarnessStartOptions { resume: resume.map(String::from), ..turn_start("go", &events) });
        (session.finished().await, events, session, adapter)
    }

    fn deltas(events: &Events) -> Vec<TurnDelta> {
        events
            .all()
            .into_iter()
            .filter_map(|e| match e {
                AdapterEvent::TurnDelta(d) => Some(d),
                _ => None,
            })
            .collect()
    }

    fn zero_tokens(context: Option<f64>) -> TurnTokens {
        TurnTokens {
            input: JsNumber(0.0),
            output: JsNumber(0.0),
            cached: n(0.0),
            cache_write: n(0.0),
            reasoning: n(0.0),
            context: context.map(JsNumber),
            window: None,
        }
    }

    #[tokio::test]
    async fn a_tool_part_reads_its_id_name_input_and_error_as_the_ts_does_on_odd_fields() {
        tokio::task::LocalSet::new().run_until(a_tool_part_reads_its_id_name_input_and_error_as_the_ts_does_on_odd_fields_body()).await
    }

    async fn a_tool_part_reads_its_id_name_input_and_error_as_the_ts_does_on_odd_fields_body() {
        // O1; adapter.ts:186-198. callID "" shadows id; no state draws `{}` and an empty result; a string input is JSON-
        // quoted; only the exact status `error` is an error, and a non-string error reads "".
        let tool = |part: Value| {
            let mut p = json!({"type": "tool"});
            for (k, v) in part.as_object().unwrap() {
                p[k] = v.clone();
            }
            line("tool_use", json!({ "part": p }))
        };
        let lines = vec![
            tool(json!({"callID": "", "id": "pid", "tool": "", "state": {"status": "completed", "input": "raw", "output": "out"}})),
            tool(json!({"id": "pid2", "tool": 5})),
            tool(json!({"callID": "c3", "tool": "bash", "state": {"status": "Error", "error": "e", "output": "o"}})),
            tool(json!({"callID": "c4", "tool": "bash", "state": {"status": "error", "error": 5, "output": "o"}})),
            stop(None, "stop"),
        ];
        let (_, events, _, _) = run(lines, ScriptOpts::default(), None).await;
        let used = |text: &str, name: &str, id: &str| TurnDelta {
            tool_name: Some(name.into()),
            tool_use_id: Some(id.into()),
            ..delta(DeltaKind::ToolUse, text)
        };
        let came = |text: &str, id: &str, err: bool| TurnDelta {
            tool_use_id: Some(id.into()),
            is_error: Some(err),
            ..delta(DeltaKind::ToolResult, text)
        };
        assert_eq!(
            deltas(&events),
            vec![
                used("\"raw\"", "", ""),
                came("out", "", false),
                used("{}", "tool", "pid2"),
                came("", "pid2", false),
                used("{}", "bash", "c3"),
                came("o", "c3", false),
                used("{}", "bash", "c4"),
                came("", "c4", true)
            ]
        );
    }

    #[tokio::test]
    async fn an_empty_reason_ends_the_turn_and_context_rides_only_where_tokens_is_an_object() {
        tokio::task::LocalSet::new().run_until(an_empty_reason_ends_the_turn_and_context_rides_only_where_tokens_is_an_object_body()).await
    }

    async fn an_empty_reason_ends_the_turn_and_context_rides_only_where_tokens_is_an_object_body() {
        // O2; adapter.ts:200-215.
        let text = line("text", json!({"part": {"type": "text", "text": "a"}}));
        let (r, _, _, _) = run(vec![text, stop(Some(json!({})), "")], ScriptOpts { hang: true, ..Default::default() }, None).await;
        assert!(r.duration_ms.is_some());
        assert_eq!(
            TurnResult { duration_ms: None, ..r },
            TurnResult { cost_usd: n(0.0), tokens: Some(zero_tokens(Some(0.0))), text: Some("a".into()), ..bare(TurnStatus::Completed) }
        );
        let (r, _, _, _) = run(vec![stop(None, "stop")], ScriptOpts { hang: true, ..Default::default() }, None).await;
        assert_eq!(
            TurnResult { duration_ms: None, ..r },
            TurnResult { cost_usd: n(0.0), tokens: Some(zero_tokens(None)), ..bare(TurnStatus::Completed) }
        );
    }

    #[tokio::test]
    async fn lines_after_the_result_are_still_said_and_a_later_error_changes_nothing() {
        tokio::task::LocalSet::new().run_until(lines_after_the_result_are_still_said_and_a_later_error_changes_nothing_body()).await
    }

    async fn lines_after_the_result_are_still_said_and_a_later_error_changes_nothing_body() {
        // O3; adapter.ts:166-215.
        let lines = vec![
            stop(None, "stop"),
            line("text", json!({"part": {"type": "text", "text": "late"}})),
            line("error", json!({"error": {"name": "APIError", "data": {"message": "late failure"}}})),
        ];
        let (r, events, _, _) = run(lines, ScriptOpts::default(), None).await;
        assert_eq!(r.status, TurnStatus::Completed);
        assert_eq!(events.all().iter().map(kind_of).collect::<Vec<_>>(), ["session.start", "turn.done", "delta:text", "session.end"]);
    }

    #[tokio::test]
    async fn an_empty_sign_in_message_reads_as_the_road_alone_and_a_string_401_is_no_sign_in() {
        tokio::task::LocalSet::new().run_until(an_empty_sign_in_message_reads_as_the_road_alone_and_a_string_401_is_no_sign_in_body()).await
    }

    async fn an_empty_sign_in_message_reads_as_the_road_alone_and_a_string_401_is_no_sign_in_body() {
        // O4; adapter.ts:84-95, 230-240.
        let refused = TurnResult { error: Some(ROAD_WORDS.into()), refusal: Some(TurnRefusal::SignIn), ..bare(TurnStatus::Failed) };
        let fail = |e: Value| vec![line("error", json!({ "error": e }))];
        let one = ScriptOpts { exit_code: Some(1), ..Default::default() };
        assert_eq!(run(fail(json!({"name": "APIError", "data": {"message": "", "statusCode": 401}})), one.clone(), None).await.0, refused);
        assert_eq!(
            run(fail(json!({"name": "APIError", "data": {"message": "Unauthorized", "statusCode": "401"}})), one.clone(), None).await.0,
            failed("Unauthorized")
        );
        assert_eq!(run(fail(json!({"name": "ProviderAuthError", "data": {"message": ""}})), one, None).await.0, refused);
    }

    #[tokio::test]
    async fn the_logged_error_is_the_first_line_naming_the_ref_with_an_error_field_each_escape_dropped() {
        tokio::task::LocalSet::new()
            .run_until(the_logged_error_is_the_first_line_naming_the_ref_with_an_error_field_each_escape_dropped_body())
            .await
    }

    async fn the_logged_error_is_the_first_line_naming_the_ref_with_an_error_field_each_escape_dropped_body() {
        // O5; adapter.ts:96-110. `ref=r10` is not `ref=r1 `; `xerror=` is not the field; `\n` reads `n`.
        let lines = vec![
            "INFO ref=r1 hello".to_string(),
            r#"ERROR ref=r10 error="ten""#.to_string(),
            r#"ERROR ref=r1 xerror="no" error="first \"q\" \n""#.to_string(),
            r#"ERROR ref=r1 error="second""#.to_string(),
            line("error", json!({"error": {"name": "UnknownError", "data": {"message": "placeholder", "ref": "r1"}}})),
        ];
        assert_eq!(run(lines, ScriptOpts { exit_code: Some(1), ..Default::default() }, None).await.0, failed("first \"q\" n"));
    }

    #[tokio::test]
    async fn the_stderr_tail_is_the_last_ten_non_blank_trimmed_lines_unreadable_json_among_them() {
        tokio::task::LocalSet::new()
            .run_until(the_stderr_tail_is_the_last_ten_non_blank_trimmed_lines_unreadable_json_among_them_body())
            .await
    }

    async fn the_stderr_tail_is_the_last_ten_non_blank_trimmed_lines_unreadable_json_among_them_body() {
        // O6; adapter.ts:166-172, 70.
        let mut lines: Vec<String> = [r#"{"a":1}"#, "[1]", "", "  "].map(String::from).to_vec();
        lines.extend((1..=9).map(|i| format!("  l{i}  ")));
        let (r, _, _, _) = run(lines, ScriptOpts { exit_code: Some(2), ..Default::default() }, None).await;
        assert_eq!(r, failed("opencode exited with code 2 before its turn ended: [1]\nl1\nl2\nl3\nl4\nl5\nl6\nl7\nl8\nl9"));
    }

    #[tokio::test]
    async fn the_first_string_session_id_announces_once_an_empty_one_included_and_the_registry_keeps_the_local_id() {
        tokio::task::LocalSet::new()
            .run_until(the_first_string_session_id_announces_once_an_empty_one_included_and_the_registry_keeps_the_local_id_body())
            .await
    }

    async fn the_first_string_session_id_announces_once_an_empty_one_included_and_the_registry_keeps_the_local_id_body() {
        // O7; adapter.ts:174-180, 252-270.
        let raw = |sid: Option<&str>, text: &str| {
            let mut v = json!({"type": "text", "timestamp": 1, "part": {"type": "text", "text": text}});
            if let Some(s) = sid {
                v["sessionID"] = json!(s);
            }
            v.to_string()
        };
        let (r, events, session, adapter) =
            run(vec![raw(None, "one"), raw(Some("newer"), "two"), raw(Some("third"), "three")], ScriptOpts::default(), Some("older")).await;
        assert_eq!(r.status, TurnStatus::Completed);
        let said: Vec<(String, String)> = events.all().iter().map(|e| (kind_of(e), e.session_id().to_string())).collect();
        let s = |k: &str, id: &str| (k.to_string(), id.to_string());
        assert_eq!(
            said,
            [
                s("delta:text", "older"),
                s("session.start", "newer"),
                s("delta:text", "newer"),
                s("delta:text", "newer"),
                s("turn.done", "newer"),
                s("session.end", "newer")
            ]
        );
        assert_eq!((session.local_id(), session.session_id()), ("older".to_string(), "newer".to_string()));
        assert_eq!(adapter.sessions().keys().cloned().collect::<Vec<_>>(), ["older".to_string()]);
        let (r, events, _, _) = run(vec![raw(Some(""), "x")], ScriptOpts::default(), None).await;
        assert_eq!(r.status, TurnStatus::Completed);
        assert!(events.all().iter().all(|e| e.session_id().is_empty()));
    }

    #[test]
    fn a_model_s_efforts_are_its_variants_in_js_key_order_integer_names_first() {
        // O8; catalog.ts:46-69 (`Object.keys(variants)`).
        let probe = format!(
            "1.18.18\n__WSP_CATALOG_SEP__\nopencode/m1\n{}\n",
            json!({"name": "M", "variants": {"b": {}, "2": {}, "a": {}, "1": {}}})
        );
        let Some(HarnessCatalogAnswer::Probe(p)) = parse_catalog_probe(&probe) else { panic!("no probe") };
        let order = ["1", "2", "b", "a"].map(String::from).to_vec();
        assert_eq!(p.models[0].efforts, Some(order.clone()));
        assert_eq!(p.efforts, order);
    }

    #[test]
    fn no_folder_is_home_with_no_dir_and_an_empty_folder_is_a_folder() {
        // O9; command.ts:63-83.
        let agent = r#"sh -c '[ -z "${WSP_AGENT_PID_FILE:-}" ] || echo $$ > "$WSP_AGENT_PID_FILE"; unset WSP_AGENT_PID_FILE; exec "$0" "$@"' opencode"#;
        let o = BuildCommandOptions { prompt: "p".into(), permission_mode: Some(AUTO_MODE.into()), ..Default::default() };
        assert_eq!(
            build_command(&o).unwrap(),
            format!("cd ~ && {agent} run --format json --print-logs --log-level ERROR --auto <<'WSP_PROMPT_END'\np\nWSP_PROMPT_END")
        );
        assert_eq!(
            build_command(&BuildCommandOptions { cwd: Some(String::new()), ..o }).unwrap(),
            format!(
                "cd '' && {agent} run --format json --print-logs --log-level ERROR --dir '' --auto <<'WSP_PROMPT_END'\np\nWSP_PROMPT_END"
            )
        );
    }

    #[tokio::test]
    async fn a_stop_after_the_result_keeps_it_and_the_end_reads_the_process_s_own_code() {
        tokio::task::LocalSet::new().run_until(a_stop_after_the_result_keeps_it_and_the_end_reads_the_process_s_own_code_body()).await
    }

    async fn a_stop_after_the_result_keeps_it_and_the_end_reads_the_process_s_own_code_body() {
        // O10; adapter.ts:271-280.
        let events = Events::default();
        let exec = scripted_exec(vec![stop(None, "stop")], ScriptOpts { hang: true, ..Default::default() }, RUN_HANDLE);
        let session =
            start(&adapter_over(&exec, Extra { result_exit_ms: Some(1000), interrupt_grace_ms: Some(5) }), turn_start("go", &events));
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        session.interrupt().await;
        assert_eq!(session.finished().await.status, TurnStatus::Completed);
        assert_eq!(session_end_fields(events.all().last().unwrap()), (SESSION, Some(143), true));
        assert_eq!(exec.order(), ["teardown"]);
    }
}
