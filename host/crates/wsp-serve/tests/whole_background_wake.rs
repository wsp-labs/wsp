// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/background-wake.test.ts at 65f6b89c7, case for case. A turn whose agent replied while work it
//! started in the background still ran, driven end to end: the runtime, the Claude adapter and a real process on this
//! computer, with a stand-in for the CLI that prints the lines Claude Code 2.1.280 printed in print mode. When the work
//! ends the CLI wakes its agent itself: a fresh init, then nothing until the woken agent's first message, which a
//! foreground command or a long think keeps off the stream for as long as it takes.
mod common;

use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::rc::Rc;
use std::sync::Arc;

use common::*;
use serde_json::Value;
use wsp_adapters::claude::adapter::{create_claude_adapter, AdapterDeps};
use wsp_adapters::port::{ExecStreamFactory, HarnessAdapter};
use wsp_core::local_exec::stream::{local_exec_stream, LocalExecOptions};
use wsp_core::machine_exec::factory::MachineExecOptions;
use wsp_core::types::api::StartOptions;
use wsp_core::types::harness::HarnessAdapterContext;
use wsp_core::types::wiring::{LocalWiring, Platform};
use wsp_engine::local_backend::{LocalBackend, LocalBackendOptions};
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_words::words::thread::background_tasks_line;
use wsp_words::words::turn::notify_tail;

/// How long the adapter waits on a silent CLI once a held reply's tasks are done; the stand-in's woken agent says
/// nothing for four times as long.
const WINDOW_MS: u64 = 250;

/// The stand-in `claude`: the agent puts the gate in the background and replies, the gate ends, the CLI wakes the agent,
/// and the woken agent runs a foreground command for `silentS` seconds before it replies. `mark` is touched when the
/// wake's init is out, so a case knows the silence has begun. Any other call (the catalog probe, a title) answers nothing.
fn stand_in(mark: &str, silent_s: u32) -> String {
    r##"#!/bin/sh
for a; do [ "$prev" = --session-id ] && sid=$a; prev=$a; done
[ -n "$sid" ] || exit 0
read -r _
cat <<EOF
{"type":"system","subtype":"init","cwd":"$PWD","session_id":"$sid","tools":["Bash"],"model":"claude-opus-5-5"}
{"type":"assistant","message":{"model":"claude-opus-5-5","id":"msg_1","type":"message","role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"Bash","input":{"command":"pnpm test","run_in_background":true}}]},"parent_tool_use_id":null,"session_id":"$sid"}
{"type":"system","subtype":"background_tasks_changed","tasks":[{"task_id":"b1","task_type":"local_bash","description":"pnpm test"}],"session_id":"$sid"}
{"type":"system","subtype":"task_started","task_id":"b1","tool_use_id":"toolu_1","description":"pnpm test","is_backgrounded":true,"task_type":"local_bash","session_id":"$sid"}
{"type":"user","message":{"role":"user","content":[{"tool_use_id":"toolu_1","type":"tool_result","content":"Command running in background with ID: b1. You will be notified when it completes.","is_error":false}]},"parent_tool_use_id":null,"session_id":"$sid"}
{"type":"assistant","message":{"model":"claude-opus-5-5","id":"msg_2","type":"message","role":"assistant","content":[{"type":"text","text":"Waiting on the gate."}]},"parent_tool_use_id":null,"session_id":"$sid"}
{"type":"result","subtype":"success","is_error":false,"duration_ms":4000,"total_cost_usd":0.1,"result":"Waiting on the gate.","num_turns":2,"result_index":0,"session_id":"$sid","usage":{"input_tokens":4,"output_tokens":90}}
EOF
sleep 0.3
cat <<EOF
{"type":"system","subtype":"background_tasks_changed","tasks":[],"session_id":"$sid"}
{"type":"system","subtype":"task_updated","task_id":"b1","patch":{"status":"completed"},"session_id":"$sid"}
{"type":"system","subtype":"task_notification","task_id":"b1","tool_use_id":"toolu_1","status":"completed","summary":"Background command completed (exit code 0)","session_id":"$sid"}
{"type":"system","subtype":"init","cwd":"$PWD","session_id":"$sid","tools":["Bash"],"model":"claude-opus-5-5"}
{"type":"assistant","message":{"model":"claude-opus-5-5","id":"msg_3","type":"message","role":"assistant","content":[{"type":"tool_use","id":"toolu_2","name":"Bash","input":{"command":"git push"}}]},"parent_tool_use_id":null,"session_id":"$sid"}
EOF
touch 'MARK_PATH'
sleep SILENT_S
cat <<EOF
{"type":"user","message":{"role":"user","content":[{"tool_use_id":"toolu_2","type":"tool_result","content":"pushed","is_error":false}]},"parent_tool_use_id":null,"session_id":"$sid"}
{"type":"assistant","message":{"model":"claude-opus-5-5","id":"msg_4","type":"message","role":"assistant","content":[{"type":"text","text":"The gate passed and the branch is pushed."}]},"parent_tool_use_id":null,"session_id":"$sid"}
{"type":"result","subtype":"success","is_error":false,"duration_ms":900,"total_cost_usd":0.3,"result":"The gate passed and the branch is pushed.","num_turns":2,"result_index":1,"origin":{"kind":"task-notification"},"session_id":"$sid","usage":{"input_tokens":4,"output_tokens":30}}
EOF
cat >/dev/null
"##
    .replace("MARK_PATH", mark)
    .replace("SILENT_S", &silent_s.to_string())
}

/// The stand-in `claude` for a turn that leaves a server running: the agent starts it in the background and replies, and
/// the process ends with the server still in the set, as a nohup'd node server does that never exits.
fn server_left_running() -> String {
    r##"#!/bin/sh
for a; do [ "$prev" = --session-id ] && sid=$a; prev=$a; done
[ -n "$sid" ] || exit 0
read -r _
cat <<EOF
{"type":"system","subtype":"init","cwd":"$PWD","session_id":"$sid","tools":["Bash"],"model":"claude-opus-5-5"}
{"type":"assistant","message":{"model":"claude-opus-5-5","id":"msg_1","type":"message","role":"assistant","content":[{"type":"tool_use","id":"toolu_1","name":"Bash","input":{"command":"nohup node server.js","run_in_background":true}}]},"parent_tool_use_id":null,"session_id":"$sid"}
{"type":"system","subtype":"background_tasks_changed","tasks":[{"task_id":"b1","task_type":"local_bash","description":"nohup node server.js"}],"session_id":"$sid"}
{"type":"system","subtype":"task_started","task_id":"b1","tool_use_id":"toolu_1","description":"nohup node server.js","is_backgrounded":true,"task_type":"local_bash","session_id":"$sid"}
{"type":"result","subtype":"success","is_error":false,"duration_ms":4000,"total_cost_usd":0.1,"result":"The screenshots are in shots/; the preview server is still up on :4173.","num_turns":2,"result_index":0,"session_id":"$sid","usage":{"input_tokens":4,"output_tokens":90}}
EOF
"##
    .to_string()
}

/// A script the code under test runs by name on a PATH, written whole before it is first run.
fn write_stub(path: &str, script: &str) {
    let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o755).open(path).expect("a stub");
    f.write_all(script.as_bytes()).expect("a stub's text");
}

struct Local {
    fx: Fx,
    root: tempfile::TempDir,
    mark: String,
}

/// A runtime whose claude is the stand-in, its woken agent silent for `silent_s` seconds; `idle_ms` is the quiet a turn is
/// cut after, the real limit where absent.
fn local_runtime(script: impl FnOnce(&str) -> String, idle_ms: Option<f64>) -> Local {
    let root = tempfile::tempdir().expect("a folder");
    let folder = std::fs::canonicalize(root.path()).expect("a real path").display().to_string();
    let mark = format!("{folder}/woken");
    let bin = format!("{folder}/bin");
    std::fs::create_dir(&bin).expect("a bin folder");
    write_stub(&format!("{bin}/claude"), &script(&mark));
    let (exec_root, run_dir) = (folder.clone(), format!("{folder}/runs"));
    let home = format!("{folder}/.claude");
    let path = format!("{bin}:{}", std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into()));
    let wiring = LocalWiring {
        backend: Arc::new(LocalBackend::new(LocalBackendOptions { root: folder.clone().into(), env: None })),
        folder: folder.clone(),
        exec_stream: Rc::new(move |opts: Option<MachineExecOptions>, waiting| {
            let mut local = LocalExecOptions::new(exec_root.clone(), run_dir.clone());
            if let Some(opts) = opts {
                local.idle_ms = opts.idle_ms;
                local.deadline_ms = opts.deadline_ms;
                local.now = opts.now;
                local.poll_ms = opts.poll_ms;
                local.reading = opts.reading;
            }
            if let Some(ms) = idle_ms {
                local.idle_ms = Some(ms);
            }
            local_exec_stream(local, waiting) as Arc<dyn ExecStreamFactory>
        }),
        home: Rc::new(move |_| home.clone()),
        home_dir: folder.clone(),
        roots_path: format!("{folder}/roots"),
        env: Rc::new(move || indexmap::IndexMap::from([("PATH".to_string(), path.clone())])),
        daemon_road: None,
        here_daemon: None,
        restart_daemon: None,
        sys_samples: None,
        copier: Some(common::copying::copying_fake() as Rc<dyn wsp_engine::copier::Copier>),
        platform: Platform::Linux,
        close: None,
    };
    let backend = StubBackend::new(None);
    let claude: wsp_core::types::harness::HarnessAdapterFactory = Arc::new(|ctx: HarnessAdapterContext| {
        let config_dir = (ctx.home)("claude");
        let mut deps = AdapterDeps::new(ctx.exec_stream.clone(), config_dir);
        deps.base_env = Some(ctx.env.iter().map(|(k, v)| (k.clone(), Some(v.clone()))).collect());
        deps.result_exit_ms = Some(WINDOW_MS);
        let adapter = create_claude_adapter(deps).map_err(|e| wsp_words::exit::HostError::new(e.0))?;
        Ok(Arc::new(adapter) as Arc<dyn HarnessAdapter>)
    });
    let fx = runtime(&backend)
        .adapter("claude", claude)
        .with(move |o| {
            o.local = Some(wiring);
        })
        .build();
    Local { fx, root, mark }
}

async fn replies(l: &Local, ws: &str) -> Vec<Value> {
    l.fx.rt.sessions().history(ws, None).await.expect("a history").iter().map(|e| j(e)).filter(|e| e["type"] == "session.done").collect()
}

async fn first_status(l: &Local, ws: &str) -> String {
    j(&l.fx.rt.sessions().list(Some(ws.to_string()), None).await.expect("a list")[0])["status"].as_str().unwrap_or_default().to_string()
}

fn start(prompt: &str) -> StartOptions {
    StartOptions { prompt: prompt.into(), ..Default::default() }
}

// "a turn whose background work ends and wakes its agent" > "stays working through the woken agent's silence, and the woken agent's reply is the turn's, timed from the launch"
#[test]
fn stays_working_through_the_woken_agents_silence_and_the_woken_agents_reply_is_the_turns_timed_from_the_launch() {
    drive(|| async {
        let l = local_runtime(|mark| stand_in(mark, 1), None);
        let ws = create_on(&l.fx.rt, CreateOn::named("mac").on(HERE_PLACE_ID), None).await.expect("a workspace");
        let handle =
            l.fx.rt
                .sessions()
                .start(&ws.view.id, start("run the gate in the background and push when it passes"), None)
                .await
                .expect("a start");

        let mark = l.mark.clone();
        until_for(5_000, &mut || {
            let mark = mark.clone();
            async move { std::path::Path::new(&mark).exists() }
        })
        .await;
        sleep_ms(WINDOW_MS * 2).await;
        // The woken agent is at work and has said nothing yet: the thread is working, and no reply is out.
        assert_eq!(replies(&l, &ws.view.id).await, Vec::<Value>::new());
        assert_eq!(first_status(&l, &ws.view.id).await, "running");

        let result = j(&handle.finished().await);
        assert_matches(
            &result,
            serde_json::json!({ "status": "completed", "text": "The gate passed and the branch is pushed.", "costUsd": 0.3 }),
        );
        // The CLI times the woken reply from the wake alone (900ms here); the turn ran from the launch to it: the held
        // reply's 4s, the gate's 0.3s and the woken agent's second.
        let took = result["durationMs"].as_f64().expect("a duration");
        assert!(took >= 4_000.0 + 1_000.0, "{took}");
        assert!(took < 4_000.0 + 5_000.0, "{took}");
        assert_eq!(replies(&l, &ws.view.id).await.len(), 1);
        assert_eq!(first_status(&l, &ws.view.id).await, "completed");
        l.fx.rt.close().await.expect("a close");
        drop(l.root);
    });
}

// "a woken agent the idle rule cuts reads failed with the cut's line, and keeps the words it gave before the wake"
#[test]
fn a_woken_agent_the_idle_rule_cuts_reads_failed_with_the_cuts_line_and_keeps_the_words_it_gave_before_the_wake() {
    drive(|| async {
        let l = local_runtime(|mark| stand_in(mark, 10), Some(1_500.0));
        let ws = create_on(&l.fx.rt, CreateOn::named("mac").on(HERE_PLACE_ID), None).await.expect("a workspace");
        let handle =
            l.fx.rt
                .sessions()
                .start(&ws.view.id, start("run the gate in the background and push when it passes"), None)
                .await
                .expect("a start");

        let result = j(&handle.finished().await);
        assert_eq!(result["status"], "failed");
        // /^stopped after .* with no output for /
        let error = result["error"].as_str().expect("an error");
        assert!(error.starts_with("stopped after ") && error.contains(" with no output for "), "{error}");
        assert_eq!(result["text"], "Waiting on the gate.");
        assert_eq!(first_status(&l, &ws.view.id).await, "failed");
        l.fx.rt.close().await.expect("a close");
        drop(l.root);
    });
}

// "a turn whose process ends with a server it started still running reads done, its last line naming that work"
#[test]
fn a_turn_whose_process_ends_with_a_server_it_started_still_running_reads_done_its_last_line_naming_that_work() {
    drive(|| async {
        let l = local_runtime(|_| server_left_running(), None);
        let ws = create_on(&l.fx.rt, CreateOn::named("mac").on(HERE_PLACE_ID), None).await.expect("a workspace");
        let handle =
            l.fx.rt.sessions().start(&ws.view.id, start("start the preview server and take the screenshots"), None).await.expect("a start");

        let done = handle.finished().await;
        let result = j(&done);
        assert_eq!(result["status"], "completed");
        assert!(result.get("error").is_none(), "{result}");
        let lines: Vec<&str> = result["text"].as_str().expect("a text").split('\n').collect();
        assert_eq!(
            lines,
            ["The screenshots are in shots/; the preview server is still up on :4173.", "", background_tasks_line(1.0).as_str()]
        );
        assert_eq!(notify_tail(&done).as_deref(), Some(background_tasks_line(1.0).as_str()));
        assert_eq!(first_status(&l, &ws.view.id).await, "completed");
        l.fx.rt.close().await.expect("a close");
        drop(l.root);
    });
}
