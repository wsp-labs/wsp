// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/cloud-turn-end.test.ts at 65f6b89c7, case for case. A cloud thread's turn after its agent's
//! final reply, driven end to end: the real Claude adapter, the cloud road's machine exec stream and its launch script,
//! run by a real bash on this computer standing for the machine, with a stand-in `claude` on PATH that prints what the
//! 2.1.280 CLI prints. Only the commands that touch the run's folder reach bash; the rest of the runtime's guest
//! commands get the stub's answers. Each test names the TS test above it.
mod common;

use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;

use common::stubs::write_stub;
use common::*;
use serde_json::Value;
use wsp_core::machine_exec::MachineExecOptions;
use wsp_core::types::api::StartOptions;
use wsp_engine::machine::ExecResult;
use wsp_wire::types::js_number::JsInt;
use wsp_words::words::thread::{background_tasks_line, still_running_line};

struct Cloud {
    dir: tempfile::TempDir,
    bin: PathBuf,
    run_dir: PathBuf,
    fx: Fx,
}

impl Drop for Cloud {
    /// A run the closed runtime let go of is still on the stand-in machine: its group goes here, as a sweep would take it.
    fn drop(&mut self) {
        let script = format!(
            "for f in {}/*.pid; do kill -KILL -- -$(cat \"$f\") 2>/dev/null; done; kill -KILL $(cat {}/escaped.pid) 2>/dev/null; true",
            self.run_dir.display(),
            self.bin.display()
        );
        let _ = std::process::Command::new("bash").args(["-c", &script]).env("PATH", "/usr/bin:/bin").output();
        let _ = &self.dir;
    }
}

/// One command on the stand-in machine, as the daemon's exec runs it: bash -c, the bytes on stdin, answered once the
/// command exited and both its pipes closed.
fn bash(cmd: String, path: String, shim: String, stdin: Option<Vec<u8>>) -> stub_backend::Fut<ExecResult> {
    Box::pin(async move {
        use tokio::io::AsyncWriteExt as _;
        let mut child = tokio::process::Command::new("bash")
            .args(["-c", &cmd])
            .env("PATH", path)
            .env("BASH_ENV", shim)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("a bash");
        let mut input = child.stdin.take().expect("stdin");
        if let Some(bytes) = stdin {
            let _ = input.write_all(&bytes).await;
        }
        drop(input);
        let out = child.wait_with_output().await.expect("an exit");
        Ok(ExecResult {
            exit_code: JsInt::from(i64::from(out.status.code().unwrap_or(1))),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        })
    })
}

fn cloud() -> Cloud {
    let dir = tempfile::tempdir().expect("a folder");
    let bin = dir.path().join("bin");
    let run_dir = dir.path().join("run");
    std::fs::create_dir(&bin).expect("a folder");
    std::fs::create_dir(&run_dir).expect("a folder");
    write_stub(&bin.join("claude"), include_str!("common/stubs/fake_claude_cloud.sh"));
    let backend = StubBackend::new(None);
    let answer = backend.exec_impl_fn();
    let path = std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into());
    // Read by every bash the run starts. The machine's project folder is not on this computer, so the launch's cd lands
    // in the test's own folder; and the launch exports the turn's own PATH, so the stand-in is named by a function,
    // which wins over any claude on that PATH. agentWord's sh execs the agent off PATH, which no function reaches, so
    // that sh is handed the stand-in's folder first.
    let shim = dir.path().join("env.sh");
    std::fs::write(
        &shim,
        format!(
            "cd() {{ builtin cd {}; }}\nclaude() {{ {}/claude \"$@\"; }}\nsh() {{ PATH={}:$PATH command sh \"$@\"; }}\n",
            dir.path().display(),
            bin.display(),
            bin.display()
        ),
    )
    .expect("a shim");
    let (runs, shim_path) = (run_dir.display().to_string(), shim.display().to_string());
    backend.set_exec_impl(Rc::new(move |m, cmd, stdin| {
        if cmd.contains(&runs) {
            bash(cmd, path.clone(), shim_path.clone(), stdin)
        } else {
            answer(m, cmd, stdin)
        }
    }));
    // The idle limit short, so a turn nothing holds is cut in seconds, as it is at TURN_IDLE_MS on a real machine.
    let claude = wsp_core::adapters::harness_adapters()["claude"].clone();
    let runs = run_dir.display().to_string();
    let fx = runtime(&backend)
        .adapter("claude", claude)
        .with(move |o| {
            o.machine_exec =
                Some(MachineExecOptions { poll_ms: Some(100), idle_ms: Some(3_000.0), run_dir: Some(runs), ..Default::default() })
        })
        .build();
    Cloud { dir, bin, run_dir, fx }
}

struct Turn {
    ws: String,
    handle: Rc<dyn wsp_core::types::wiring::SessionHandle>,
    thread: String,
}

async fn turn(c: &Cloud, mode: &str) -> Turn {
    std::fs::write(c.bin.join("mode"), mode).expect("a mode");
    let ws = create_on(&c.fx.rt, CreateOn::golden("boat"), None).await.expect("a workspace").view.id;
    let handle =
        c.fx.rt
            .sessions()
            .start(&ws, StartOptions { prompt: "build the ticket".into(), notify: Some(vec!["me".into()]), ..Default::default() }, None)
            .await
            .expect("a start");
    let thread = handle.view().thread_id.expect("a thread");
    Turn { ws, handle, thread }
}

async fn notified(c: &Cloud, t: &Turn) -> Vec<Value> {
    let history = c.fx.rt.sessions().history(&t.ws, None).await.expect("a history");
    history.iter().map(j).filter(|e| e["type"] == "session.notify" && e["threadId"] == t.thread.as_str()).collect()
}

async fn lines(c: &Cloud, t: &Turn) -> Vec<String> {
    notified(c, t).await.iter().map(|e| e["text"].as_str().unwrap_or_default().to_string()).collect()
}

fn status(t: &Turn) -> String {
    j(&t.handle.view())["status"].as_str().unwrap_or_default().to_string()
}

/// `until(async () => (await notified(..)).length === n, ms)`.
async fn until_notified(c: &Cloud, t: &Turn, n: usize, ms: u64) {
    until_for(ms, &mut || async { notified(c, t).await.len() == n }).await;
}

/// Past the idle limit, which cuts a turn nothing holds.
async fn past_idle() {
    sleep_ms(4_000).await;
}

/// `/finished \(<status>[^)]*\)<tail>$/`: the line says it finished as `status`, then everything up to the first `)`, then
/// the `tail` (which opens with the `)`) to the end of the line.
fn finished_as(line: &str, status: &str, tail: &dyn Fn(&str) -> bool) -> bool {
    let key = format!("finished ({status}");
    let mut from = 0;
    while let Some(found) = line[from..].find(&key) {
        let rest = &line[from + found + key.len()..];
        if let Some(close) = rest.find(')') {
            if tail(&rest[close..]) {
                return true;
            }
        }
        from += found + 1;
    }
    false
}

/// The rest of the line from the first `)` on is `)` and then `tail`, to the end.
fn exact(tail: String) -> impl Fn(&str) -> bool {
    move |rest| rest.strip_prefix(')') == Some(tail.as_str())
}

// "a cloud turn after its agent's final reply" > "with a child it left holding the run's output open, the agent's exit ends the turn: completed, and the finished line goes out"
#[test]
fn with_a_child_it_left_holding_the_runs_output_open_the_agents_exit_ends_the_turn_completed_and_the_finished_line_goes_out() {
    drive(|| async {
        let c = cloud();
        let t = turn(&c, "plain").await;
        until_notified(&c, &t, 1, 15_000).await;
        t.handle.finished().await;
        assert_eq!(status(&t), "completed");
        c.fx.rt.close().await.expect("a close");
    });
}

// "with the CLI still reporting a background task that never ends, the reply's finished line goes out within seconds and the task holds the turn"
#[test]
fn with_the_cli_still_reporting_a_background_task_that_never_ends_the_replys_finished_line_goes_out_within_seconds_and_the_task_holds_the_turn(
) {
    drive(|| async {
        let c = cloud();
        let t = turn(&c, "bg").await;
        let at = Instant::now();
        until_notified(&c, &t, 1, 5_000).await;
        let took = at.elapsed().as_millis();
        past_idle().await;
        let told = lines(&c, &t).await;
        assert!(took < 3_000, "{took} ms");
        assert_eq!(status(&t), "running");
        assert_eq!(told.len(), 1, "{told:?}");
        assert!(
            finished_as(&told[0], "completed", &exact(": 1 background task still running; another line comes when this turn ends".into())),
            "{}",
            told[0]
        );
        c.fx.rt.close().await.expect("a close");
    });
}

// "woken by its task after the held reply, the agent's new reply sends a second line, and the end sends no third"
#[test]
fn woken_by_its_task_after_the_held_reply_the_agents_new_reply_sends_a_second_line_and_the_end_sends_no_third() {
    drive(|| async {
        let c = cloud();
        let t = turn(&c, "wake").await;
        until_notified(&c, &t, 1, 5_000).await;
        assert_eq!(status(&t), "running");
        t.handle.finished().await;
        let told = lines(&c, &t).await;
        assert_eq!(status(&t), "completed");
        assert_eq!(told.len(), 2, "{told:?}");
        assert!(told[0].contains(&still_running_line(1.0)), "{}", told[0]);
        assert!(finished_as(&told[1], "completed", &exact(": The tests passed.".into())), "{}", told[1]);
        c.fx.rt.close().await.expect("a close");
    });
}

/// `` `pnpm exec vitest run` completed, [\d.]+m?s after the reply ``, to the end of the line.
fn after_the_reply(rest: &str) -> bool {
    let Some(rest) = rest.strip_prefix("): `pnpm exec vitest run` completed, ") else { return false };
    let Some(rest) = rest.strip_suffix("s after the reply") else { return false };
    let rest = rest.strip_suffix('m').unwrap_or(rest);
    !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit() || c == '.')
}

// "with its task ended and the agent not woken, the end sends a second line saying how the task ended, and not the reply's words again"
#[test]
fn with_its_task_ended_and_the_agent_not_woken_the_end_sends_a_second_line_saying_how_the_task_ended_and_not_the_replys_words_again() {
    drive(|| async {
        let c = cloud();
        let t = turn(&c, "quiet").await;
        until_notified(&c, &t, 1, 5_000).await;
        t.handle.finished().await;
        let told = lines(&c, &t).await;
        assert_eq!(status(&t), "completed");
        assert_eq!(told.len(), 2, "{told:?}");
        assert!(told[0].contains(&still_running_line(1.0)), "{}", told[0]);
        assert!(finished_as(&told[1], "completed", &after_the_reply), "{}", told[1]);
        c.fx.rt.close().await.expect("a close");
    });
}

// "ended with its task still running and nobody woken, the end sends a second line saying the task was left running"
#[test]
fn ended_with_its_task_still_running_and_nobody_woken_the_end_sends_a_second_line_saying_the_task_was_left_running() {
    drive(|| async {
        let c = cloud();
        let t = turn(&c, "left").await;
        t.handle.finished().await;
        let told = lines(&c, &t).await;
        assert_eq!(status(&t), "completed");
        assert_eq!(told.len(), 2, "{told:?}");
        assert!(told[0].contains(&still_running_line(1.0)), "{}", told[0]);
        assert!(finished_as(&told[1], "completed", &exact(format!(": {}", background_tasks_line(1.0)))), "{}", told[1]);
        c.fx.rt.close().await.expect("a close");
    });
}

// "a message steered into the held turn wakes the agent, and when it is cut before replying, its failed line goes out"
#[test]
fn a_message_steered_into_the_held_turn_wakes_the_agent_and_when_it_is_cut_before_replying_its_failed_line_goes_out() {
    drive(|| async {
        let c = cloud();
        let t = turn(&c, "steer").await;
        until_notified(&c, &t, 1, 5_000).await;
        let steered =
            c.fx.rt
                .sessions()
                .start(
                    &t.ws,
                    StartOptions { prompt: "also fix the flaky test".into(), thread: Some(t.thread.clone()), ..Default::default() },
                    None,
                )
                .await
                .expect("a start");
        assert_eq!(j(&steered.outcome()), "steered");
        t.handle.finished().await;
        let told = lines(&c, &t).await;
        assert_eq!(status(&t), "failed");
        assert_eq!(told.len(), 2, "{told:?}");
        assert!(told[0].contains(&still_running_line(1.0)), "{}", told[0]);
        assert!(told[1].contains("finished (failed"), "{}", told[1]);
        c.fx.rt.close().await.expect("a close");
    });
}

// "stopped well after the held reply's line, with nothing woken, the lead is told the stop once, timed to the stop, and not the reply's words again"
#[test]
fn stopped_well_after_the_held_replys_line_with_nothing_woken_the_lead_is_told_the_stop_once_timed_to_the_stop_and_not_the_replys_words_again(
) {
    drive(|| async {
        let c = cloud();
        let t = turn(&c, "bg").await;
        until_notified(&c, &t, 1, 5_000).await;
        sleep_ms(3_000).await;
        let stopped = c.fx.rt.sessions().interrupt(&t.handle.view().id, None, None).await.expect("an interrupt");
        assert_eq!(j(&stopped)["outcome"], "accepted");
        let told = lines(&c, &t).await;
        assert_eq!(status(&t), "interrupted");
        assert_eq!(told.len(), 2, "{told:?}");
        assert!(told[0].contains(&format!("finished (completed, 1.2s, $0.01): {}", still_running_line(1.0))), "{}", told[0]);
        let stopped_line = |rest: &str| {
            // `, [\d.]+s, \$0\.01\)$`
            let Some(rest) = rest.strip_prefix(", ") else { return false };
            let Some((secs, rest)) = rest.split_once("s, ") else { return false };
            !secs.is_empty() && secs.chars().all(|c| c.is_ascii_digit() || c == '.') && rest == "$0.01)"
        };
        let key = "finished (interrupted";
        let at = told[1].find(key).expect("the stop line");
        let tail = &told[1][at + key.len()..];
        assert!(stopped_line(tail), "{}", told[1]);
        // The CLI timed the reply at 1.2 s; the stop came at least 3 s after the reply's line went.
        let after = tail.strip_prefix(", ").and_then(|t| t.split_once('s')).map(|(secs, _)| secs.parse::<f64>().expect("seconds"));
        assert!(after.expect("seconds") >= 4.2, "{}", told[1]);
        c.fx.rt.close().await.expect("a close");
    });
}

// "stopped after a steer woke the agent under the held reply, before it replied, the lead is told the stop once and not the reply's words again"
#[test]
fn stopped_after_a_steer_woke_the_agent_under_the_held_reply_before_it_replied_the_lead_is_told_the_stop_once_and_not_the_replys_words_again(
) {
    drive(|| async {
        let c = cloud();
        let t = turn(&c, "stop").await;
        until_notified(&c, &t, 1, 5_000).await;
        let steered =
            c.fx.rt
                .sessions()
                .start(
                    &t.ws,
                    StartOptions { prompt: "also fix the flaky test".into(), thread: Some(t.thread.clone()), ..Default::default() },
                    None,
                )
                .await
                .expect("a start");
        assert_eq!(j(&steered.outcome()), "steered");
        until_for(5_000, &mut || async {
            let history = c.fx.rt.sessions().history(&t.ws, None).await.expect("a history");
            history.iter().map(j).any(|e| e["type"] == "session.delta" && e["text"] == "Looking at the flaky test.")
        })
        .await;
        let stopped = c.fx.rt.sessions().interrupt(&t.handle.view().id, None, None).await.expect("an interrupt");
        assert_eq!(j(&stopped)["outcome"], "accepted");
        let told = lines(&c, &t).await;
        assert_eq!(status(&t), "interrupted");
        assert_eq!(told.len(), 2, "{told:?}");
        assert!(told[0].contains(&still_running_line(1.0)), "{}", told[0]);
        assert!(finished_as(&told[1], "interrupted", &|rest| rest == ")"), "{}", told[1]);
        c.fx.rt.close().await.expect("a close");
    });
}
