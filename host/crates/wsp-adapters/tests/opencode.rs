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

use futures::future::{BoxFuture, FutureExt};
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_adapters::opencode::adapter::{AttachedSession, OpenCodeAdapter, OpenCodeAdapterDeps, OpenCodeSession};
use wsp_adapters::opencode::adapter::create_opencode_adapter;
use wsp_adapters::opencode::command::{build_command, build_env, BuildCommandOptions, OpenCodeEnvOptions, AUTO_MODE};
use wsp_adapters::port::{
    agent_word, AdapterAttachOptions, AdapterEvent, AgentLaunch, DeltaKind, Env, ExecFailure, HarnessCatalogAnswer, HarnessCatalogModelProbe,
    HarnessCatalogProbe, HarnessExec, HarnessStartOptions, TurnDelta, TurnImage, TurnResult, TurnStatus, TurnTokens,
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
    TurnResult { status, duration_ms: None, waited_ms: None, cost_usd: None, tokens: None, model: None, models: None, text: None, error: None, unreached: None, refusal: None, limit: None }
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
    fn exec(&self, command: &str, _env: Option<&Env>) -> BoxFuture<'static, Result<String, ExecFailure>> {
        self.asked.lock().unwrap().push(command.to_string());
        let answer = self.answer.clone();
        async move { Ok(answer) }.boxed()
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
        assert!(call.command.starts_with(&format!("cd '/root/app' && {} run --format json", agent_word("opencode", None))), "{}", call.command);
        assert!(
            call.command.contains("-m anthropic/claude-sonnet-4-5 --variant high --auto <<'WSP_PROMPT_END'\nlist the three largest files\nWSP_PROMPT_END"),
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
        let exec = scripted_exec(fixture("opencode", "turn.jsonl"), ScriptOpts::default(), RUN_HANDLE);
        let events = Events::default();
        let session = start(
            &adapter_over(&exec, Extra::default()),
            HarnessStartOptions { prompt: "go".into(), cwd: Some("/root/app".into()), model: Some("anthropic/claude-sonnet-4-5".into()), on_event: Some(events.on_event()), ..Default::default() },
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
            AdapterEvent::SessionStart { session_id: SESSION.into(), model: Some("anthropic/claude-sonnet-4-5".into()), cwd: Some("/root/app".into()), tools: None, harness: None, prompted: None }
        );
        assert_eq!(all[1], AdapterEvent::TurnDelta(TurnDelta { message_id: Some("msg_f1d4a1b2c001".into()), ..delta(DeltaKind::Text, "I will list the files by size.") }));
        assert_eq!(
            all[2],
            AdapterEvent::TurnDelta(TurnDelta {
                tool_name: Some("bash".into()),
                tool_use_id: Some("call_f1d4ls01".into()),
                ..delta(DeltaKind::ToolUse, &serde_json::to_string(&json!({ "command": "ls -S | head -3", "description": "Lists the three largest files" })).unwrap())
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
            Some(TurnTokens { input: JsNumber(3300.0), output: JsNumber(190.0), cached: n(1000.0), cache_write: n(0.0), reasoning: n(50.0), context: n(2340.0), window: None })
        );
        assert_eq!(all.last().unwrap(), &AdapterEvent::SessionEnd { session_id: SESSION.into(), exit_code: Some(0), saw_result: true, failure: None });
    }

    #[tokio::test]
    async fn a_tool_part_that_failed_is_an_error_result_carrying_the_tool_s_own_error() {
        let failed_part = line(
            "tool_use",
            json!({ "part": { "id": "prt_1", "sessionID": SESSION, "messageID": "msg_1", "type": "tool", "callID": "call_rd", "tool": "read", "state": { "status": "error", "input": { "filePath": "/nope" }, "error": "File not found: /nope", "time": { "start": 1, "end": 2 } } } }),
        );
        let events = Events::default();
        start(&adapter_over(&scripted_exec(vec![failed_part], ScriptOpts::default(), RUN_HANDLE), Extra::default()), turn_start("go", &events)).finished().await;
        assert!(events.all().contains(&AdapterEvent::TurnDelta(TurnDelta {
            tool_use_id: Some("call_rd".into()),
            is_error: Some(true),
            ..delta(DeltaKind::ToolResult, "File not found: /nope")
        })));
    }

    #[tokio::test]
    async fn the_turn_ends_at_the_step_whose_reason_ends_the_loop_and_a_process_that_lingers() {
        let exec = scripted_exec(fixture("opencode", "turn.jsonl"), ScriptOpts { hang: true, ..Default::default() }, RUN_HANDLE);
        let events = Events::default();
        let result = start(&adapter_over(&exec, Extra { result_exit_ms: Some(5), interrupt_grace_ms: Some(5) }), turn_start("go", &events)).finished().await;
        assert_eq!(result.status, TurnStatus::Completed);
        assert!(exec.order().contains(&"teardown".to_string()), "{:?}", exec.order());
        assert_eq!(events.all().iter().filter(|e| matches!(e, AdapterEvent::TurnDone { .. })).count(), 1);
    }

    #[tokio::test]
    async fn a_step_that_asked_for_tools_does_not_end_the_turn_the_process_exiting_after_it_w() {
        let lines: Vec<String> = fixture("opencode", "turn.jsonl").into_iter().take(4).collect();
        let exec = scripted_exec(lines, ScriptOpts::default(), RUN_HANDLE);
        let result = start(&adapter_over(&exec, Extra::default()), HarnessStartOptions { prompt: "go".into(), on_event: Some(no_events()), ..Default::default() }).finished().await;
        assert_eq!(result.status, TurnStatus::Completed);
        assert_eq!(result.text, Some("I will list the files by size.".to_string()));
    }

    #[tokio::test]
    async fn an_unknown_model_fails_the_turn_in_the_log_s_own_words_not_the_stream_s_server_e() {
        let exec = scripted_exec(fixture("opencode", "model-not-found.jsonl"), ScriptOpts { exit_code: Some(1), ..Default::default() }, RUN_HANDLE);
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
        let refused = line("error", json!({ "error": { "name": "ProviderAuthError", "data": { "providerID": "anthropic", "message": "Invalid API key" } } }));
        let exec = scripted_exec(vec![refused], ScriptOpts { exit_code: Some(1), ..Default::default() }, RUN_HANDLE);
        let result = start(&adapter_over(&exec, Extra::default()), HarnessStartOptions { prompt: "go".into(), on_event: Some(no_events()), ..Default::default() }).finished().await;
        assert_eq!(result, TurnResult { refusal: Some(TurnRefusal::SignIn), ..failed(&format!("Invalid API key; {SIGN_IN_ROAD}")) });
        let unauthorized = line("error", json!({ "error": { "name": "APIError", "data": { "message": "Unauthorized", "statusCode": 401, "isRetryable": false } } }));
        let exec = scripted_exec(vec![unauthorized], ScriptOpts { exit_code: Some(1), ..Default::default() }, RUN_HANDLE);
        let second = start(&adapter_over(&exec, Extra::default()), HarnessStartOptions { prompt: "go".into(), on_event: Some(no_events()), ..Default::default() }).finished().await;
        assert_eq!(second.refusal, Some(TurnRefusal::SignIn));
    }

    #[tokio::test]
    async fn a_process_that_dies_before_any_event_fails_the_turn_with_what_it_printed() {
        let exec = scripted_exec(vec!["bash: opencode: command not found".into()], ScriptOpts { exit_code: Some(127), ..Default::default() }, RUN_HANDLE);
        let result = start(&adapter_over(&exec, Extra::default()), HarnessStartOptions { prompt: "go".into(), on_event: Some(no_events()), ..Default::default() }).finished().await;
        assert_eq!(result, failed("opencode exited with code 127 before its turn ended: bash: opencode: command not found"));
    }

    #[tokio::test]
    async fn interrupt_ends_the_process_and_the_turn_reads_interrupted() {
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
        let exec = scripted_exec(fixture("opencode", "turn.jsonl"), ScriptOpts::default(), RUN_HANDLE);
        let adapter = adapter_over(&exec, Extra::default());
        start(
            &adapter,
            HarnessStartOptions {
                prompt: "what is this".into(),
                images: vec![TurnImage { media_type: "image/png".into(), bytes: "".into(), path: Some("/root/.wsp/images/t/a.png".into()) }],
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
        let (exec, asked) = probe_over(&fixture("opencode", "catalog-probe.txt").join("\n"));
        let probe = adapter_over(&scripted_exec(vec![], ScriptOpts::default(), RUN_HANDLE), Extra::default()).probe_catalog(exec).await.unwrap();
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
        assert!(line.starts_with(&format!("cd '/root/app' && {} run --format json --print-logs --log-level ERROR --dir '/root/app'", agent_word("opencode", None))), "{line}");
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
        let line = build_command(&BuildCommandOptions { images: vec!["/root/.wsp/images/t1/a.png".into(), "/root/.wsp/images/t1/b c.png".into()], ..opts("what is this") }).unwrap();
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
            build_env(&OpenCodeEnvOptions { base: Some(keyed_base), api_key: Some("sk-x".into()), key_env: Some("OPENCODE_API_KEY".into()) }),
            keyed
        );
    }
}

/// Rules of spec/opencode.md no TS case pins, written from adapter.ts at the lines cited.
mod rules_this_port_pins {
    use super::*;

    #[tokio::test]
    async fn the_first_error_is_the_turns_failure_and_a_later_one_changes_nothing() {
        // Rule 11; adapter.ts:202 (`failure ??=`).
        let first = line("error", json!({"error": {"name": "APIError", "data": {"message": "first words"}}}));
        let second = line("error", json!({"error": {"name": "ProviderAuthError", "data": {"message": "second words"}}}));
        let exec = scripted_exec(vec![first, second], ScriptOpts { exit_code: Some(1), ..Default::default() }, RUN_HANDLE);
        let result = start(&adapter_over(&exec, Extra::default()), turn_start("go", &Events::default())).finished().await;
        assert_eq!(result, failed("first words"));
    }

    #[tokio::test]
    async fn a_text_part_that_trims_empty_is_neither_said_nor_the_reply() {
        // Rule 8; adapter.ts:183.
        let said = line("text", json!({"part": {"type": "text", "text": "the reply", "messageID": "m1"}}));
        let blank = line("text", json!({"part": {"type": "text", "text": "  \n"}}));
        let events = Events::default();
        let exec = scripted_exec(vec![said, blank], ScriptOpts::default(), RUN_HANDLE);
        let result = start(&adapter_over(&exec, Extra::default()), turn_start("go", &events)).finished().await;
        assert_eq!(result.text.as_deref(), Some("the reply"));
        let texts: Vec<String> = events.all().iter().filter_map(|e| match e { AdapterEvent::TurnDelta(d) => Some(d.text.clone()), _ => None }).collect();
        assert_eq!(texts, vec!["the reply".to_string()]);
    }
}
