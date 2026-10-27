// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/runtime-comeback.test.ts at 65f6b89c7, case for case, the second half (from a press on a box slate
//! behind a sweep to the held reply's line). The first half is whole_runtime_comeback_1.rs. The last case of the file, "lets
//! go of the poll that reads it when it closes", counts node's active timers (`process.getActiveResourcesInfo`) and is not
//! ported: a Rust timer is a task, with no process-wide count a test can read.
#[macro_use]
mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use common::comeback::*;
use common::*;
use serde_json::{json, Value};
use tokio::io::AsyncWriteExt as _;
use wsp_core::clock::FakeClock;
use wsp_core::console::capture;
use wsp_core::status::StatusWatchOptions;
use wsp_core::types::api::StartOptions;
use wsp_core::types::internal::TRANSCRIPT_CAP;
use wsp_core::types::wiring::GoldenRecipe;
use wsp_engine::exec_detached::exec_detached_in;
use wsp_engine::machine::{ExecResult, Machine, RunOptions};
use wsp_wire::types::env::TURN_TOKEN_ENV;
use wsp_wire::wire::limits::LINK_RETRY_WINDOW_MS;
use wsp_words::words::thread::{still_running_line, RUN_GONE_LINE, TURN_STOPPED_LINE};

const TOKEN: &str = "deadbeefdeadbeefdeadbeef";

async fn immediate() {
    tokio::task::yield_now().await;
}

fn has_setsid() -> bool {
    std::process::Command::new("sh").args(["-c", "command -v setsid"]).output().is_ok_and(|o| o.status.success())
}

fn is_failed_line(text: &str) -> bool {
    // /^finished \(failed[^)]*\): /
    text.strip_prefix("finished (failed").and_then(|rest| rest.split_once("): ")).is_some_and(|(inside, _)| !inside.contains(')'))
}

// "a turn the host comes back to" > "a press on a box slate whose machine's runs are being swept runs nothing there until the sweep is done"
#[test]
fn a_press_on_a_box_slate_whose_machines_runs_are_being_swept_runs_nothing_there_until_the_sweep_is_done() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::new();
        let s = slow_listing(&backend, &state.store, &h, true).await;
        let (thread_id, caller) = s.thread.clone().expect("a thread");
        let machine = s.machine();
        let before = machine.run_log().len();

        let rt2 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        let slates = rt2.rt.slates();
        use wsp_slates::slates::{ApprovalScope, ApproveParams, SlateTarget, StateParams, WriteParams};
        let text = "<slate title=\"Held\"><value name=\"go\" start={0} />\n  <run name=\"box\" cmd=\"echo ran\" timeout={20} />\n  <when change={$go} do={start($box)} />\n  <column><output run={$box} /></column>\n</slate>";
        slates
            .write(
                WriteParams {
                    target: SlateTarget { thread_id: Some(thread_id.clone()), turn_token: None },
                    text: Some(text.into()),
                    ..Default::default()
                },
                Some(&caller),
            )
            .await
            .expect("a slate");
        slates
            .state(
                StateParams {
                    target: SlateTarget { thread_id: Some(thread_id.clone()), turn_token: None },
                    values: Some([("$go".to_string(), json!(1))].into_iter().collect()),
                    ..Default::default()
                },
                Some(&caller),
            )
            .await
            .expect("a state");
        let view = slates.get(&thread_id).await.expect("a read").expect("a slate");
        for ask in &view.asks {
            slates
                .approve(ApproveParams { thread_id: thread_id.clone(), key: ask.key().to_string(), scope: ApprovalScope::Thread })
                .await
                .expect("an approval");
        }
        sleep_ms(200).await;
        assert!(machine.run_log()[before..].is_empty());

        s.answer.open();
        let m = machine.clone();
        wait!([m] m.run_log().len() > before);
        let log = s.log();
        let ran = machine.run_log()[before].clone();
        assert!(log.iter().rposition(|c| *c == ran).expect("the run in the log") as i64 > s.reap_at().map_or(-1, |i| i as i64));
        rt2.rt.close().await.expect("a close");
    });
}

// "a turn the host comes back to" > "an export on a machine whose runs are being swept starts no run there until the sweep is done"
#[test]
fn an_export_on_a_machine_whose_runs_are_being_swept_starts_no_run_there_until_the_sweep_is_done() {
    use async_trait::async_trait;
    use wsp_core::types::wiring::{LandRequest, LandedProject, ProjectExportOptions, ProjectLander};
    use wsp_engine::vault::CacheRule;
    use wsp_words::exit::HostError;
    struct Lander;
    #[async_trait(?Send)]
    impl ProjectLander for Lander {
        fn caches(&self) -> CacheRule {
            CacheRule { dirs: vec![], files: vec![], markers: vec![] }
        }
        async fn probe(&self, _dest: &str) -> Result<Option<u64>, HostError> {
            Ok(None)
        }
        async fn land(&self, _req: LandRequest) -> Result<LandedProject, HostError> {
            Ok(LandedProject { files: 0, bytes: 0, agents: vec![] })
        }
    }
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::new();
        let s = slow_listing(&backend, &state.store, &h, false).await;
        let machine = s.machine();
        let before = machine.run_log().len();
        let rt2 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        let (r, ws) = (rt2.rt.clone(), s.slow.clone());
        let exported = tokio::task::spawn_local(async move {
            let _ = r
                .projects()
                .export(
                    ProjectExportOptions {
                        workspace_id: ws,
                        source: "/root/work/proj".into(),
                        dest: "/Users/dev/code/proj".into(),
                        replace: None,
                        agents: None,
                        lander: Rc::new(Lander),
                    },
                    None,
                )
                .await;
        });
        rt2.rt.workspaces().list(None).await.expect("a list");
        immediate().await;
        assert!(machine.run_log()[before..].is_empty());

        s.answer.open();
        exported.await.expect("the export");
        assert!(machine.run_log().len() > before);
        let ran = machine.run_log()[before].clone();
        assert!(s.log().iter().rposition(|c| *c == ran).expect("the run in the log") as i64 > s.reap_at().map_or(-1, |i| i as i64));
        rt2.rt.close().await.expect("a close");
    });
}

/// `bash -c cmd` in `dir`, as `execFile("bash", ["-c", cmd], { cwd: dir })`.
async fn bash(cmd: &str, dir: Option<&std::path::Path>) -> ExecResult {
    let mut c = tokio::process::Command::new("bash");
    c.arg("-c").arg(cmd);
    if let Some(dir) = dir {
        c.current_dir(dir);
    }
    let out = c.output().await.expect("bash runs");
    exec_exit(i64::from(out.status.code().unwrap_or(1)), &String::from_utf8_lossy(&out.stdout), &String::from_utf8_lossy(&out.stderr))
}

// "a turn the host comes back to" > "a command run detached on a machine whose runs are being swept finishes, and the sweep does not end it"
#[test]
fn a_command_run_detached_on_a_machine_whose_runs_are_being_swept_finishes_and_the_sweep_does_not_end_it() {
    if !has_setsid() {
        return;
    }
    drive(|| async {
        // Real bash in a private folder: the sweep lists and reaps that folder alone, never the machine's own run folder.
        let dir = tempfile::tempdir().expect("a folder");
        let run_dir = dir.path().join("wsp-run").display().to_string();
        std::fs::create_dir(&run_dir).expect("the run folder");
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::new();
        let rt1 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        let slow = create_on(&rt1.rt, CreateOn::golden("slow"), None).await.expect("a workspace").view;
        rt1.rt.close().await.expect("a close");
        let machine_id = j(&slow)["machineId"].as_str().expect("a machine").to_string();
        let stubbed = backend.exec_impl_fn();
        let answered = Gate::new();
        let (waited, run_dir_of, cwd) = (answered.clone(), run_dir.clone(), dir.path().to_path_buf());
        backend.set_exec_impl(Rc::new(move |m, cmd, stdin| {
            let (stubbed, waited, run_dir, cwd, machine_id) =
                (stubbed.clone(), waited.clone(), run_dir_of.clone(), cwd.clone(), machine_id.clone());
            Box::pin(async move {
                if m.id() != machine_id {
                    return stubbed(m, cmd, stdin).await;
                }
                // As a box runs an exec given longer than its inline span.
                if cmd.contains("echo finished") {
                    return exec_detached_in(
                        &*m,
                        &cmd,
                        RunOptions { deadline_ms: 600_000, poll_ms: Some(50), ..Default::default() },
                        &run_dir,
                    )
                    .await;
                }
                if !cmd.contains(&run_dir) {
                    return stubbed(m, cmd, stdin).await;
                }
                if cmd.contains("printf '%s\\n' \"$d\"") {
                    waited.wait().await;
                }
                Ok(bash(&cmd, Some(&cwd)).await)
            })
        }));

        let rt2 = runtime(&backend)
            .store(&state.store)
            .adapter("claude", h.factory())
            .with({
                let run_dir = run_dir.clone();
                move |o| o.machine_exec = Some(wsp_core::machine_exec::MachineExecOptions { run_dir: Some(run_dir), ..Default::default() })
            })
            .build();
        let (r, id) = (rt2.rt.clone(), slow.id.clone());
        let ran = tokio::task::spawn_local(async move { r.workspaces().exec(&id, "sleep 1; echo finished", Some(600_000), None).await });
        // The listing goes out once the command is up where nothing holds it back, and after a wait where something does.
        let rd = run_dir.clone();
        within(1000, || {
            let rd = rd.clone();
            async move { std::fs::read_dir(&rd).is_ok_and(|d| d.flatten().any(|e| e.file_name().to_string_lossy().ends_with(".d"))) }
        })
        .await;
        answered.open();
        let result = ran.await.expect("the exec").expect("an exec");
        assert_matches(&j(&result), json!({ "exitCode": 0, "stdout": "finished\n" }));
        rt2.rt.close().await.expect("a close");
    });
}

/// A server that answers every request 502, which is a daemon port with nothing behind it.
async fn bad_gateway() -> (u16, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("a port");
    let port = listener.local_addr().expect("an address").port();
    let task = tokio::task::spawn_local(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::task::spawn_local(async move {
                let mut head = [0u8; 2048];
                let _ = tokio::io::AsyncReadExt::read(&mut stream, &mut head).await;
                let _ = stream.write_all(b"HTTP/1.1 502 Bad Gateway\r\ncontent-length: 0\r\nconnection: close\r\n\r\n").await;
            });
        }
    });
    (port, task)
}

struct Deploy {
    listing_answered: bool,
    result: Option<ExecResult>,
}

struct Down {
    deploys: Rc<RefCell<Vec<Deploy>>>,
    answer: Rc<Gate>,
    rt: Fx,
    stop: Rc<dyn Fn()>,
    server: tokio::task::JoinHandle<()>,
    _dir: Option<tempfile::TempDir>,
}

impl Down {
    async fn end(self) {
        (self.stop)();
        self.rt.rt.close().await.expect("a close");
        self.server.abort();
    }
}

/// `daemonDown`: a machine whose daemon port answers 502, so the reach poll puts the daemon back, and whose run listing the
/// test lets out by hand. With a run folder the machine's runs are real: `run` goes through execDetached into that folder on
/// this computer's bash, and the listing and the reap run there too.
async fn daemon_down(real: bool) -> Down {
    let (port, server) = bad_gateway().await;
    let backend = StubBackend::new(None);
    backend.set_exec_impl(token_guest());
    let deploys: Rc<RefCell<Vec<Deploy>>> = Rc::default();
    let listing_answered = Rc::new(Cell::new(false));
    let machine_ref: Rc<RefCell<Option<Arc<StubMachine>>>> = Rc::default();
    let recipe = {
        let (deploys, listing_answered) = (deploys.clone(), listing_answered.clone());
        GoldenRecipe {
            setup: "true".into(),
            smoke: "true".into(),
            deploy_daemon: Some(Rc::new(move |machine, _| {
                let (deploys, listing_answered) = (deploys.clone(), listing_answered.clone());
                Box::pin(async move {
                    let at = {
                        let mut d = deploys.borrow_mut();
                        d.push(Deploy { listing_answered: listing_answered.get(), result: None });
                        d.len() - 1
                    };
                    let r = machine.run("sleep 1; echo deployed", RunOptions { deadline_ms: 180_000, ..Default::default() }).await.ok();
                    deploys.borrow_mut()[at].result = r;
                    Ok(None)
                })
            })),
            ..Default::default()
        }
    };
    let state = sqlite_state();
    let rt1 = runtime(&backend)
        .store(&state.store)
        .with({
            let (recipe, _) = (recipe.clone(), 0);
            move |o| {
                o.daemon_token = Some(TOKEN.into());
                o.golden_recipe = Some(recipe);
            }
        })
        .build();
    let slow = create_on(&rt1.rt, CreateOn::golden("slow"), None).await.expect("a workspace").view;
    rt1.rt.close().await.expect("a close");
    deploys.borrow_mut().clear();
    let answer = Gate::new();
    let machine = backend
        .machines()
        .into_iter()
        .find(|m| Some(m.id().to_string()) == j(&slow)["machineId"].as_str().map(str::to_string))
        .expect("the machine");
    machine.set_preview_port_url(&format!("http://127.0.0.1:{port}"));
    *machine_ref.borrow_mut() = Some(machine);
    let dir = real.then(|| tempfile::tempdir().expect("a folder"));
    let mut run_dir = None;
    let listing = |cmd: &str| cmd.contains("printf '%s\\n' \"$d\"");
    if let Some(dir) = &dir {
        let rd = dir.path().join("wsp-run").display().to_string();
        std::fs::create_dir(&rd).expect("the run folder");
        let stubbed = backend.exec_impl_fn();
        let (waited, answered, rd2) = (answer.clone(), listing_answered.clone(), rd.clone());
        let machine_id = j(&slow)["machineId"].as_str().expect("a machine").to_string();
        backend.set_exec_impl(Rc::new(move |m, cmd, stdin| {
            let (stubbed, waited, answered, rd, machine_id) =
                (stubbed.clone(), waited.clone(), answered.clone(), rd2.clone(), machine_id.clone());
            Box::pin(async move {
                if m.id() != machine_id {
                    return stubbed(m, cmd, stdin).await;
                }
                // `machine.run = execDetached(...)`: the deploy's script goes through the detached road into the run folder.
                if cmd.contains("echo deployed") {
                    return exec_detached_in(&*m, &cmd, RunOptions { deadline_ms: 180_000, poll_ms: Some(50), ..Default::default() }, &rd)
                        .await;
                }
                if !cmd.contains(&rd) {
                    return stubbed(m, cmd, stdin).await;
                }
                if cmd.contains("printf '%s\\n' \"$d\"") {
                    waited.wait().await;
                    answered.set(true);
                }
                Ok(bash(&cmd, None).await)
            })
        }));
        run_dir = Some(rd);
    } else {
        let (waited, answered, machine_id) =
            (answer.clone(), listing_answered.clone(), j(&slow)["machineId"].as_str().expect("a machine").to_string());
        backend.set_exec_impl(exec_async(move |m, cmd| {
            let (waited, answered, machine_id) = (waited.clone(), answered.clone(), machine_id.clone());
            Box::pin(async move {
                if m.id() == machine_id && listing(&cmd) {
                    waited.wait().await;
                    answered.set(true);
                    return Ok(exec_ok(""));
                }
                Ok(token_guest_answer(&cmd))
            })
        }));
    }
    let watching = StatusWatchOptions {
        cost_interval_ms: Some(60_000),
        poll_interval_ms: Some(5),
        reconcile_min_ms: Some(60_000),
        ..Default::default()
    };
    let rt2 = runtime(&backend)
        .store(&state.store)
        .with(move |o| {
            o.daemon_token = Some(TOKEN.into());
            o.golden_recipe = Some(recipe);
            o.status = Some(watching);
            if let Some(rd) = run_dir {
                o.machine_exec = Some(wsp_core::machine_exec::MachineExecOptions { run_dir: Some(rd), ..Default::default() });
            }
        })
        .build();
    let stop = rt2.rt.status().watch(watching);
    Down { deploys, answer, rt: rt2, stop, server, _dir: dir }
}

// "a turn the host comes back to" > "the reach poll puts no daemon back on a machine whose run listing is still out"
#[test]
fn the_reach_poll_puts_no_daemon_back_on_a_machine_whose_run_listing_is_still_out() {
    drive(|| async {
        let d = daemon_down(false).await;
        // Long enough for the poll at 5 ms to see the port answer nothing twice and deploy, where nothing holds it back.
        let deploys = d.deploys.clone();
        within(1000, || {
            let deploys = deploys.clone();
            async move { !deploys.borrow().is_empty() }
        })
        .await;
        assert!(d.deploys.borrow().is_empty());
        d.answer.open();
        let deploys = d.deploys.clone();
        until_for(3000, &mut || {
            let deploys = deploys.clone();
            async move { !deploys.borrow().is_empty() }
        })
        .await;
        assert!(d.deploys.borrow()[0].listing_answered);
        d.end().await;
    });
}

// "a turn the host comes back to" > "a daemon the reach poll puts back on a machine whose runs are being swept is deployed, not ended by the sweep"
#[test]
fn a_daemon_the_reach_poll_puts_back_on_a_machine_whose_runs_are_being_swept_is_deployed_not_ended_by_the_sweep() {
    if !has_setsid() {
        return;
    }
    drive(|| async {
        let d = daemon_down(true).await;
        // The listing goes out once the deploy's claim is up where nothing holds it back, and after a wait where something does.
        let run_dir = d._dir.as_ref().expect("a folder").path().join("wsp-run");
        within(1000, || {
            let run_dir = run_dir.clone();
            async move { std::fs::read_dir(&run_dir).is_ok_and(|e| e.flatten().any(|f| f.file_name().to_string_lossy().ends_with(".d"))) }
        })
        .await;
        d.answer.open();
        let deploys = d.deploys.clone();
        until_for(5000, &mut || {
            let deploys = deploys.clone();
            async move { deploys.borrow().first().is_some_and(|x| x.result.is_some()) }
        })
        .await;
        assert_matches(&j(d.deploys.borrow()[0].result.as_ref().expect("a result")), json!({ "exitCode": 0, "stdout": "deployed\n" }));
        d.end().await;
    });
}

// "a turn the host comes back to" > "a run whose row this host could not re-open is ended on the machine, not left holding it"
#[test]
fn a_run_whose_row_this_host_could_not_re_open_is_ended_on_the_machine_not_left_holding_it() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::new();
        let down = host_went_down(&h, &state.store, &backend).await;
        let reaps = guest_runs(&backend, &[&down.run]);

        // No adapter for the row's harness: the turn reads as one the restart cut, and nothing here reads its run again.
        let rt2 = runtime(&backend).store(&state.store).build();
        let (r, w) = (rt2.rt.clone(), down.workspace_id.clone());
        wait!([r, w] rows(&r, &w).await[0]["status"] == "failed");

        let early = reaps.borrow().len();
        if early != 1 {
            sleep_ms(500).await;
            eprintln!("{early} reaps when the row read failed, {} half a second on", reaps.borrow().len());
        }
        assert_eq!(early, 1);
        assert!(reaps.borrow()[0].contains(&format!("rm -rf '{}'.*", down.run)));
        rt2.rt.close().await.expect("a close");
    });
}

// "a turn the host comes back to" > "a run the machine no longer holds ends the turn on those words, and a second restart does not end it again"
#[test]
fn a_run_the_machine_no_longer_holds_ends_the_turn_on_those_words_and_a_second_restart_does_not_end_it_again() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::new();
        let down = host_went_down(&h, &state.store, &backend).await;
        h.sweep(&down.run);

        let rt2 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        let (r, w) = (rt2.rt.clone(), down.workspace_id.clone());
        wait!([r, w] rows(&r, &w).await[0]["status"] == "failed");
        let history = history(&rt2.rt, &down.workspace_id).await;
        assert_matches(
            history.last().expect("an event"),
            json!({ "type": "session.end", "exitCode": null, "sawResult": false, "reason": RUN_GONE_LINE }),
        );
        rt2.rt.close().await.expect("a close");

        let rt3 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        assert_eq!(history_types(&rt3.rt, &down.workspace_id).await.iter().filter(|t| *t == "session.end").count(), 1);
        rt3.rt.close().await.expect("a close");
    });
}

// "a turn the host comes back to" > "a machine that answers nothing about the run leaves the turn running, kills nothing, and the poll that finds the machine gone is what settles it"
#[test]
fn a_machine_that_answers_nothing_about_the_run_leaves_the_turn_running_kills_nothing_and_the_poll_that_finds_the_machine_gone_is_what_settles_it(
) {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::new();
        let down = host_went_down(&h, &state.store, &backend).await;
        h.unreach(Some("gateway said 502"));

        let rt2 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        assert_eq!(statuses(&rt2.rt, &down.workspace_id).await, ["running"]);
        // Nothing was told about the turn either way: no end row, and the run is still there to be read next time.
        assert!(!has_event(&rt2.rt, &down.workspace_id, "session.end").await);
        assert!(h.handles().contains(&down.run));
        // The machine really is away, and the road that watches machines can still settle a row this host never opened.
        let listed = rt2.rt.workspaces().list(None).await.expect("a list");
        rt2.rt.workspaces().delete(j(&listed[0])["id"].as_str().expect("an id"), None).await.expect("a delete");
        let (r, w) = (rt2.rt.clone(), down.workspace_id.clone());
        wait!([r, w] rows(&r, &w).await.iter().all(|s| s["status"] != "running"));
        rt2.rt.close().await.expect("a close");
    });
}

// "a turn the host comes back to" > "a transcript that does not read leaves the turn running with no run opened, so no reader is left holding its lines"
#[test]
fn a_transcript_that_does_not_read_leaves_the_turn_running_with_no_run_opened_so_no_reader_is_left_holding_its_lines() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::new();
        let down = host_went_down(&h, &state.store, &backend).await;
        let failing = WrappedStore::over(&state.store);
        failing.fail_transcripts("disk said EIO");
        let warn = Rc::new(capture());
        let rt2 = runtime(&backend).store(&failing.store()).adapter("claude", h.factory()).build();
        assert_eq!(statuses(&rt2.rt, &down.workspace_id).await, ["running"]);
        let w = warn.clone();
        wait!([w] w.lines().iter().any(|l| l.contains("was left running")));
        assert!(h.reopened().is_empty());
        rt2.rt.close().await.expect("a close");
    });
}

// "a turn the host comes back to" > "a stop on a turn the restart could not reach ends it as stopped and unreached, and the next idle window naps the machine"
#[test]
fn a_stop_on_a_turn_the_restart_could_not_reach_ends_it_as_stopped_and_unreached_and_the_next_idle_window_naps_the_machine() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::new();
        let down = host_went_down(&h, &state.store, &backend).await;
        h.unreach(Some("daemon connect timed out after 15000ms"));
        let warn = Rc::new(capture());
        let fc = FakeClock::at_now();
        let window = 20 * 60_000;
        let rt2 = runtime(&backend)
            .store(&state.store)
            .adapter("claude", h.factory())
            .clock(&fc)
            .with(move |o| o.idle_default_window_ms = Some(window as u64))
            .build();
        let row = rows(&rt2.rt, &down.workspace_id).await.remove(0);
        let w = warn.clone();
        wait!([w] w.lines().iter().any(|l| l.contains("was left running")));
        fc.advance(window * 2);
        immediate().await;
        assert!(!backend.machine(0).paused.get());

        let stopped = rt2.rt.sessions().interrupt(row["id"].as_str().expect("an id"), None, None).await.expect("an interrupt");
        assert_eq!(j(&stopped)["outcome"], "accepted");
        assert_eq!(statuses(&rt2.rt, &down.workspace_id).await, ["interrupted"]);
        let end = history(&rt2.rt, &down.workspace_id).await.into_iter().rfind(|e| e["type"] == "session.done").expect("a done row");
        assert_matches(&end, json!({ "result": { "status": "interrupted", "error": TURN_STOPPED_LINE, "unreached": true } }));
        assert_eq!(status_rows(&rt2.rt).await[0]["idleAt"], json!(fc.now() + window));
        fc.advance(window);
        let (r, w) = (rt2.rt.clone(), down.workspace_id.clone());
        wait!([r, w] j(&r.workspaces().get(&w, None, None).await.expect("a workspace"))["phase"] == "napping");
        assert!(backend.machine(0).paused.get());
        rt2.rt.close().await.expect("a close");
    });
}

// "a turn the host comes back to" > "a stop while a later ask after an unreached turn is still out leaves the turn to that ask, and the turn ends once"
#[test]
fn a_stop_while_a_later_ask_after_an_unreached_turn_is_still_out_leaves_the_turn_to_that_ask_and_the_turn_ends_once() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::new();
        let down = host_went_down(&h, &state.store, &backend).await;
        h.unreach(Some("daemon connect timed out after 15000ms"));
        // The later ask is held at the attach, as one waiting out a slow link is.
        let holder = Holder::at(None);
        let adapter = h.gated(&holder);
        let warn = Rc::new(capture());
        let fc = FakeClock::at_now();
        let rt2 = runtime(&backend)
            .store(&state.store)
            .adapter("claude", adapter)
            .clock(&fc)
            .with(|o| o.idle_default_window_ms = Some(20 * 60_000))
            .build();
        let row = rows(&rt2.rt, &down.workspace_id).await.remove(0);
        let w = warn.clone();
        wait!([w] w.lines().iter().any(|l| l.contains("was left running")));
        let open = Gate::new();
        holder.hold(&open);
        h.unreach(None);
        let before = holder.asked.get();
        let (hd, f) = (holder.clone(), fc.clone());
        wait!([hd, f] {
            if hd.asked.get() == before {
                f.advance(LINK_RETRY_WINDOW_MS);
            }
            hd.asked.get() > before
        });
        let stopped = rt2.rt.sessions().interrupt(row["id"].as_str().expect("an id"), None, None).await.expect("an interrupt");
        assert_eq!(j(&stopped)["outcome"], "not-running");
        open.open();
        let hh = h.clone();
        wait!([hh] hh.reopened().len() == 1);
        finish_run(&h, &down.run, "sess-1", "done");
        let (r, w) = (rt2.rt.clone(), down.workspace_id.clone());
        wait!([r, w] rows(&r, &w).await[0]["status"] == "completed");
        assert_eq!(history_types(&rt2.rt, &down.workspace_id).await.iter().filter(|t| *t == "session.end").count(), 1);
        rt2.rt.close().await.expect("a close");
    });
}

// "a turn the host comes back to" > "a turn the restart could not reach keeps its machine awake past every idle window, and the window starts from its end"
#[test]
fn a_turn_the_restart_could_not_reach_keeps_its_machine_awake_past_every_idle_window_and_the_window_starts_from_its_end() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::new();
        let down = host_went_down(&h, &state.store, &backend).await;
        h.unreach(Some("daemon connect timed out after 15000ms"));
        let warn = Rc::new(capture());
        let fc = FakeClock::at_now();
        let window = 20 * 60_000;
        let counted = WrappedStore::over(&state.store);
        let rt2 = runtime(&backend)
            .store(&counted.store())
            .adapter("claude", h.factory())
            .clock(&fc)
            .with(move |o| o.idle_default_window_ms = Some(window as u64))
            .build();
        assert_eq!(statuses(&rt2.rt, &down.workspace_id).await, ["running"]);
        let w = warn.clone();
        wait!("wait 1", [w] w.lines().iter().any(|l| l.contains("was left running")));
        fc.advance(window * 3);
        immediate().await;
        assert!(!backend.machine(0).paused.get());
        assert_eq!(j(&rt2.rt.workspaces().get(&down.workspace_id, None, None).await.expect("a workspace"))["phase"], "running");
        assert_eq!(statuses(&rt2.rt, &down.workspace_id).await, ["running"]);
        assert!(status_rows(&rt2.rt).await[0].get("idleAt").is_none());

        // Each round that the machine leaves unanswered asks once more, and says nothing and writes nothing.
        let puts = counted.puts_into("sessions");
        let asked = h.attempts();
        for round in 0..5 {
            fc.advance(LINK_RETRY_WINDOW_MS);
            let hh = h.clone();
            let reached = within(2000, || {
                let hh = hh.clone();
                async move { hh.attempts() == asked + round + 1 }
            })
            .await;
            assert!(reached, "round {round}: {} attempts, want {}", h.attempts() - asked, round + 1);
            immediate().await;
        }
        assert_eq!(warn.lines().iter().filter(|l| l.contains("was left running")).count(), 1);
        assert_eq!(counted.puts_into("sessions"), puts);

        // The machine answers again: the run is asked after a link window on, read, and its end is what the window counts from.
        h.unreach(None);
        let (hh, f) = (h.clone(), fc.clone());
        wait!("wait 3", [hh, f] {
            f.advance(LINK_RETRY_WINDOW_MS);
            hh.reopened().len() == 1
        });
        let (r, w) = (rt2.rt.clone(), down.workspace_id.clone());
        wait!("wait 4", [r, w] history(&r, &w).await.iter().filter(|e| e["type"] == "session.delta").count() == 1);
        fc.advance(window * 2);
        immediate().await;
        assert!(!backend.machine(0).paused.get());
        finish_run(&h, &down.run, "sess-1", "done");
        let (r, w) = (rt2.rt.clone(), down.workspace_id.clone());
        wait!("wait 5", [r, w] rows(&r, &w).await[0]["status"] == "completed");
        let ended = fc.now();
        assert_eq!(status_rows(&rt2.rt).await[0]["idleAt"], json!(ended + window));
        fc.advance(window - 1);
        immediate().await;
        assert!(!backend.machine(0).paused.get());
        fc.advance(1);
        let (r, w) = (rt2.rt.clone(), down.workspace_id.clone());
        wait!("wait 6", [r, w] j(&r.workspaces().get(&w, None, None).await.expect("a workspace"))["phase"] == "napping");
        assert!(backend.machine(0).paused.get());
        rt2.rt.close().await.expect("a close");
    });
}

fn notifying(prompt: &str) -> StartOptions {
    StartOptions { prompt: prompt.into(), notify: Some(vec!["me".into()]), ..Default::default() }
}

// "a turn the host comes back to" > "a turn whose reply is already written settles completed when the run is gone at boot, and tells its parent nothing more"
#[test]
fn a_turn_whose_reply_is_already_written_settles_completed_when_the_run_is_gone_at_boot_and_tells_its_parent_nothing_more() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::new();
        let rt1 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        let ws = create_on(&rt1.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        rt1.rt.sessions().start(&ws, notifying("build it"), None).await.expect("a start");
        let (r, w) = (rt1.rt.clone(), ws.clone());
        wait!([r, w] has_event(&r, &w, "session.start").await);
        let run = h.handles()[0].clone();
        h.emit(&run, json!({ "type": "turn.done", "sessionId": "sess-1", "result": { "status": "completed", "text": "done" } }));
        let (r, w) = (rt1.rt.clone(), ws.clone());
        wait!([r, w] has_event(&r, &w, "session.notify").await);
        rt1.rt.close().await.expect("a close");
        h.sweep(&run);

        let rt2 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        assert_eq!(statuses(&rt2.rt, &ws).await, ["completed"]);
        assert_eq!(history_types(&rt2.rt, &ws).await, ["session.start", "session.notify", "session.done", "session.end"]);
        let history = history(&rt2.rt, &ws).await;
        assert_matches(history.last().expect("an event"), json!({ "type": "session.end", "sawResult": true, "reason": RUN_GONE_LINE }));
        rt2.rt.close().await.expect("a close");
    });
}

// "a turn the host comes back to" > "a turn longer than the transcript cap replays without writing a line twice: the count comes off the last row's stamp, not off how many rows survive"
#[test]
fn a_turn_longer_than_the_transcript_cap_replays_without_writing_a_line_twice_the_count_comes_off_the_last_rows_stamp_not_off_how_many_rows_survive(
) {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::new();
        let rt1 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        let ws = create_on(&rt1.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        rt1.rt.sessions().start(&ws, opts("build it"), None).await.expect("a start");
        let (r, w) = (rt1.rt.clone(), ws.clone());
        wait!([r, w] has_event(&r, &w, "session.start").await);
        let run = h.handles()[0].clone();
        let printed = 5200;
        for i in 0..printed {
            h.emit(&run, json!({ "type": "turn.delta", "sessionId": "sess-1", "kind": "text", "text": format!("line {i}") }));
        }
        // A non-delta row is what flushes the transcript, so the capped store is what the next host reads.
        h.emit(&run, json!({ "type": "turn.done", "sessionId": "sess-1", "result": { "status": "completed", "text": "done" } }));
        let before = history(&rt1.rt, &ws).await.iter().filter(|e| e["type"] == "session.delta").count();
        assert!(before < printed, "{before}");
        assert!(TRANSCRIPT_CAP < printed);
        rt1.rt.close().await.expect("a close");

        let rt2 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        rows(&rt2.rt, &ws).await;
        h.emit(&run, json!({ "type": "session.end", "sessionId": "sess-1", "exitCode": 0, "sawResult": true }));
        let (r, w) = (rt2.rt.clone(), ws.clone());
        wait!([r, w] rows(&r, &w).await[0]["status"] == "completed");
        let deltas: Vec<Value> = history(&rt2.rt, &ws).await.into_iter().filter(|e| e["type"] == "session.delta").collect();
        let texts: std::collections::HashSet<String> = deltas.iter().map(|e| e["text"].as_str().unwrap_or_default().to_string()).collect();
        assert_eq!(texts.len(), deltas.len());
        assert_matches(deltas.last().expect("a delta"), json!({ "text": format!("line {}", printed - 1), "line": printed }));
        rt2.rt.close().await.expect("a close");
    });
}

// "a turn the host comes back to" > "a turn the host restart did not end keeps its token: the coordinator thread still names itself with me after the restart"
#[test]
fn a_turn_the_host_restart_did_not_end_keeps_its_token_the_coordinator_thread_still_names_itself_with_me_after_the_restart() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::new();
        let rt1 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        let ws = create_on(&rt1.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let turn = rt1.rt.sessions().start(&ws, opts("coordinate the builders"), None).await.expect("a start");
        let (r, w) = (rt1.rt.clone(), ws.clone());
        wait!([r, w] has_event(&r, &w, "session.start").await);
        let thread_id = j(&turn.view())["threadId"].as_str().expect("a thread").to_string();
        let token = h.envs.borrow()[0][TURN_TOKEN_ENV].clone();
        // The row the next host reads carries it beside the run, since the process it is in outlives this host.
        let stored = state.store.get("sessions", &ws).await.expect("a read").expect("a record");
        let pairs: Vec<Value> =
            stored["sessions"].as_array().expect("sessions").iter().map(|s| json!([s["status"], s["turnToken"]])).collect();
        assert_eq!(pairs, [json!(["running", token])]);
        rt1.rt.close().await.expect("a close");

        // The host came back and re-opened the run, so that turn is still working with the same variable in its process.
        let rt2 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        assert_eq!(statuses(&rt2.rt, &ws).await, ["running"]);
        let kid = rt2
            .rt
            .sessions()
            .start(
                &ws,
                StartOptions {
                    prompt: "build it".into(),
                    notify: Some(vec!["me".into()]),
                    turn_token: Some(token),
                    started_by: Some(wsp_wire::views::session::SessionOrigin::Agent),
                    ..Default::default()
                },
                None,
            )
            .await
            .expect("a start");
        let kid_thread = j(&kid.view())["threadId"].clone();
        assert_ne!(kid_thread, json!(thread_id));
        let kid_run = h.handles()[1].clone();
        finish_run(&h, &kid_run, "sess-2", "done");
        let (r, w) = (rt2.rt.clone(), ws.clone());
        wait!([r, w] has_event(&r, &w, "session.notify").await);
        let notify = history(&rt2.rt, &ws).await.into_iter().find(|e| e["type"] == "session.notify").expect("a notify");
        assert_matches(&notify, json!({ "threadId": kid_thread, "notify": thread_id }));
        rt2.rt.close().await.expect("a close");
    });
}

// "a turn the host comes back to" > "a turn the host restart did not end keeps the device its launch carried, and hands it back at its own exit"
#[test]
fn a_turn_the_host_restart_did_not_end_keeps_the_device_its_launch_carried_and_hands_it_back_at_its_own_exit() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::new();
        let rt1 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        let ws = create_on(&rt1.rt, CreateOn::golden("a").agents(json!({ "maxMachines": 3, "maxDepth": 1 })), None)
            .await
            .expect("a workspace")
            .view
            .id;
        rt1.rt.sessions().start(&ws, opts("coordinate the builders"), None).await.expect("a start");
        let (r, w) = (rt1.rt.clone(), ws.clone());
        wait!([r, w] has_event(&r, &w, "session.start").await);
        let device = j(&rt1.rt.devices().list().await.expect("devices")[0].clone())["id"].clone();
        // The row the next host reads carries the device beside the run, for the same reason it carries the token: the
        // process out there still holds both, and only the row knows which device to take away when that turn ends.
        let stored = state.store.get("sessions", &ws).await.expect("a read").expect("a record");
        let pairs: Vec<Value> =
            stored["sessions"].as_array().expect("sessions").iter().map(|s| json!([s["status"], s["scopeDeviceId"]])).collect();
        assert_eq!(pairs, [json!(["running", device])]);
        rt1.rt.close().await.expect("a close");

        let rt2 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        assert_eq!(statuses(&rt2.rt, &ws).await, ["running"]);
        // The load's sweep spares a device whose thread is running, so the re-opened turn is what hands it back.
        let listed: Vec<Value> = rt2.rt.devices().list().await.expect("devices").iter().map(|d| j(d)["id"].clone()).collect();
        assert_eq!(listed, [device]);
        let run = h.handles()[0].clone();
        finish_run(&h, &run, "sess-1", "done");
        let r = rt2.rt.clone();
        wait!([r] r.devices().list().await.expect("devices").is_empty());
        assert!(rt2.rt.devices().list().await.expect("devices").is_empty());
        rt2.rt.close().await.expect("a close");
    });
}

// "a turn the host comes back to" > "a host with no adapter for the harness cannot re-open the run, so the turn reads as one the restart cut"
#[test]
fn a_host_with_no_adapter_for_the_harness_cannot_re_open_the_run_so_the_turn_reads_as_one_the_restart_cut() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::new();
        let down = host_went_down(&h, &state.store, &backend).await;

        let rt2 = runtime(&backend).store(&state.store).build();
        assert_eq!(statuses(&rt2.rt, &down.workspace_id).await, ["failed"]);
        let history = history(&rt2.rt, &down.workspace_id).await;
        assert_matches(
            history.last().expect("an event"),
            json!({ "type": "session.end", "reason": "host restarted while the agent was working" }),
        );
        rt2.rt.close().await.expect("a close");
    });
}

// "a turn the host comes back to" > "a turn whose reply landed before the restart has its line recorded once, not again on the replay"
#[test]
fn a_turn_whose_reply_landed_before_the_restart_has_its_line_recorded_once_not_again_on_the_replay() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::new();
        let rt1 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        let ws = create_on(&rt1.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        rt1.rt.sessions().start(&ws, notifying("build it"), None).await.expect("a start");
        let (r, w) = (rt1.rt.clone(), ws.clone());
        wait!([r, w] has_event(&r, &w, "session.start").await);
        let run = h.handles()[0].clone();
        // the reply landed and the harness process had not exited when the host went down
        h.emit(&run, json!({ "type": "turn.done", "sessionId": "sess-1", "result": { "status": "completed", "text": "done" } }));
        let (r, w) = (rt1.rt.clone(), ws.clone());
        wait!([r, w] has_event(&r, &w, "session.notify").await);
        assert_matches(&rows(&rt1.rt, &ws).await[0], json!({ "status": "running" }));
        rt1.rt.close().await.expect("a close");

        let rt2 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        assert_eq!(statuses(&rt2.rt, &ws).await, ["running"]);
        h.emit(&run, json!({ "type": "session.end", "sessionId": "sess-1", "exitCode": 0, "sawResult": true }));
        let (r, w) = (rt2.rt.clone(), ws.clone());
        wait!([r, w] rows(&r, &w).await[0]["status"] == "completed");
        assert_eq!(history_types(&rt2.rt, &ws).await, ["session.start", "session.notify", "session.done", "session.end"]);
        rt2.rt.close().await.expect("a close");
    });
}

// "a turn the host comes back to" > "a reply held over background work has its line sent once, not again on the replay, and the held end sends its outcome alone"
#[test]
fn a_reply_held_over_background_work_has_its_line_sent_once_not_again_on_the_replay_and_the_held_end_sends_its_outcome_alone() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::new();
        let rt1 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        let ws = create_on(&rt1.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        rt1.rt.sessions().start(&ws, notifying("build it"), None).await.expect("a start");
        let (r, w) = (rt1.rt.clone(), ws.clone());
        wait!([r, w] has_event(&r, &w, "session.start").await);
        let run = h.handles()[0].clone();
        let reply = json!({ "status": "completed", "text": "Pushed; the tests run in the background." });
        h.emit(&run, json!({ "type": "turn.tasks", "sessionId": "sess-1", "running": 1, "replied": reply }));
        let (r, w) = (rt1.rt.clone(), ws.clone());
        wait!([r, w] has_event(&r, &w, "session.notify").await);
        rt1.rt.close().await.expect("a close");

        let rt2 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        assert_eq!(statuses(&rt2.rt, &ws).await, ["running"]);
        h.emit(&run, json!({ "type": "turn.done", "sessionId": "sess-1", "result": reply, "held": true }));
        h.emit(&run, json!({ "type": "session.end", "sessionId": "sess-1", "exitCode": 0, "sawResult": true }));
        let (r, w) = (rt2.rt.clone(), ws.clone());
        wait!([r, w] rows(&r, &w).await[0]["status"] == "completed");
        assert_eq!(history_types(&rt2.rt, &ws).await, ["session.start", "session.notify", "session.notify", "session.done", "session.end"]);
        let told: Vec<String> = notifies(&rt2.rt, &ws).await.iter().map(|t| without_thread(t, "")).collect();
        assert_eq!(told, [format!("finished (completed): {}", still_running_line(1.0)), "finished (completed)".to_string()]);
        rt2.rt.close().await.expect("a close");
    });
}

/// `heldLineSent`: a turn told to `notify`, its agent's reply held over a background task and its line sent: the runtime and the run.
async fn held_line_sent(h: &Rc<MachineRuns>, store: &Rc<dyn wsp_store::store::Store>, backend: &Arc<StubBackend>) -> (Fx, String, String) {
    let rt = runtime(backend).store(store).adapter("claude", h.factory()).build();
    let ws = create_on(&rt.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
    rt.rt.sessions().start(&ws, notifying("build it"), None).await.expect("a start");
    let (r, w) = (rt.rt.clone(), ws.clone());
    wait!([r, w] has_event(&r, &w, "session.start").await);
    let run = h.handles().last().cloned().expect("a run");
    h.emit(&run, json!({ "type": "turn.tasks", "sessionId": "sess-1", "running": 1, "replied": { "status": "completed", "text": "Pushed; CI runs in the background." } }));
    let (r, w) = (rt.rt.clone(), ws.clone());
    wait!([r, w] has_event(&r, &w, "session.notify").await);
    (rt, ws, run)
}

// "a turn the host comes back to" > "a held reply's line and the woken agent's line into a napping lead both outlive a host restart"
#[test]
fn a_held_replys_line_and_the_woken_agents_line_into_a_napping_lead_both_outlive_a_host_restart() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::new();
        let rt1 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        let lead = create_on(&rt1.rt, CreateOn::golden("lead"), None).await.expect("a workspace").view.id;
        let kid_ws = create_on(&rt1.rt, CreateOn::golden("kid"), None).await.expect("a workspace").view.id;
        let lead_turn = rt1.rt.sessions().start(&lead, opts("orchestrate"), None).await.expect("a start");
        let (r, w) = (rt1.rt.clone(), lead.clone());
        wait!([r, w] has_event(&r, &w, "session.start").await);
        let lead_run = h.handles()[0].clone();
        h.emit(&lead_run, json!({ "type": "turn.done", "sessionId": "sess-1", "result": { "status": "completed", "text": "waiting" } }));
        h.emit(&lead_run, json!({ "type": "session.end", "sessionId": "sess-1", "exitCode": 0, "sawResult": true }));
        lead_turn.finished().await;
        rt1.rt.workspaces().nap(&lead, None).await.expect("a nap");
        let lead_thread = j(&lead_turn.view())["threadId"].as_str().expect("a thread").to_string();
        rt1.rt
            .sessions()
            .start(&kid_ws, StartOptions { prompt: "build it".into(), notify: Some(vec![lead_thread]), ..Default::default() }, None)
            .await
            .expect("a start");
        let (r, w) = (rt1.rt.clone(), kid_ws.clone());
        wait!([r, w] has_event(&r, &w, "session.start").await);
        let kid_run = h.handles()[1].clone();
        h.emit(&kid_run, json!({ "type": "turn.tasks", "sessionId": "sess-2", "running": 1, "replied": { "status": "completed", "text": "first reply: pushed, CI in the background" } }));
        h.emit(&kid_run, json!({ "type": "session.start", "sessionId": "sess-2" }));
        h.emit(
            &kid_run,
            json!({ "type": "turn.done", "sessionId": "sess-2", "result": { "status": "completed", "text": "second reply: CI passed" } }),
        );
        h.emit(&kid_run, json!({ "type": "session.end", "sessionId": "sess-2", "exitCode": 0, "sawResult": true }));
        let (r, w) = (rt1.rt.clone(), kid_ws.clone());
        wait!([r, w] rows(&r, &w).await[0]["status"] == "completed");
        rt1.rt.close().await.expect("a close");

        let rt2 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        rows(&rt2.rt, &lead).await;
        rt2.rt.workspaces().wake(&lead, None).await.expect("a wake");
        let hh = h.clone();
        wait!([hh] hh.prompts.borrow().len() == 3);
        let woken = h.handles()[2].clone();
        h.emit(&woken, json!({ "type": "turn.done", "sessionId": "sess-1", "result": { "status": "completed", "text": "read it" } }));
        h.emit(&woken, json!({ "type": "session.end", "sessionId": "sess-1", "exitCode": 0, "sawResult": true }));
        let hh = h.clone();
        within(2000, || {
            let hh = hh.clone();
            async move { hh.prompts.borrow().len() == 4 }
        })
        .await;
        let mut told: Vec<String> = h.prompts.borrow()[2..].iter().map(|p| without_thread(p, "finished (completed): ")).collect();
        told.sort();
        assert_eq!(
            told,
            [format!("first reply: pushed, CI in the background\n\n{}", still_running_line(1.0)), "second reply: CI passed".to_string()]
        );
        rt2.rt.close().await.expect("a close");
    });
}

// "a turn the host comes back to" > "a woken agent's own end goes out after a held reply's line, and the woken reply only once on a replay"
#[test]
fn a_woken_agents_own_end_goes_out_after_a_held_replys_line_and_the_woken_reply_only_once_on_a_replay() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::new();
        let (rt1, ws, run) = held_line_sent(&h, &state.store, &backend).await;
        h.emit(&run, json!({ "type": "session.start", "sessionId": "sess-1" }));
        h.emit(&run, json!({ "type": "turn.done", "sessionId": "sess-1", "result": { "status": "completed", "text": "CI passed." } }));
        let (r, w) = (rt1.rt.clone(), ws.clone());
        wait!([r, w] notifies(&r, &w).await.len() == 2);
        rt1.rt.close().await.expect("a close");

        let rt2 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        assert_eq!(statuses(&rt2.rt, &ws).await, ["running"]);
        h.emit(&run, json!({ "type": "session.end", "sessionId": "sess-1", "exitCode": 0, "sawResult": true }));
        let (r, w) = (rt2.rt.clone(), ws.clone());
        wait!([r, w] rows(&r, &w).await[0]["status"] == "completed");
        let told: Vec<String> = notifies(&rt2.rt, &ws).await.iter().map(|t| without_thread(t, "finished (completed): ")).collect();
        assert_eq!(told, [still_running_line(1.0), "CI passed.".to_string()]);
        rt2.rt.close().await.expect("a close");
    });
}

// "a turn the host comes back to" > "a stop after a restart, the agent woken under the held reply and not replied, tells the stop alone: the replay names the words already told"
#[test]
fn a_stop_after_a_restart_the_agent_woken_under_the_held_reply_and_not_replied_tells_the_stop_alone_the_replay_names_the_words_already_told(
) {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::new();
        let (rt1, ws, run) = held_line_sent(&h, &state.store, &backend).await;
        rt1.rt.close().await.expect("a close");

        let rt2 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        assert_eq!(statuses(&rt2.rt, &ws).await, ["running"]);
        h.emit(&run, json!({ "type": "session.start", "sessionId": "sess-1" }));
        h.emit(&run, json!({ "type": "turn.done", "sessionId": "sess-1", "result": { "status": "interrupted", "text": "Pushed; CI runs in the background." } }));
        h.emit(&run, json!({ "type": "session.end", "sessionId": "sess-1", "exitCode": null, "sawResult": true }));
        let (r, w) = (rt2.rt.clone(), ws.clone());
        wait!([r, w] rows(&r, &w).await[0]["status"] == "interrupted");
        let told: Vec<String> = notifies(&rt2.rt, &ws).await.iter().map(|t| without_thread(t, "")).collect();
        assert_eq!(told, [format!("finished (completed): {}", still_running_line(1.0)), "finished (interrupted)".to_string()]);
        rt2.rt.close().await.expect("a close");
    });
}

// "a turn the host comes back to" > "a held reply's line is not sent again by a restarted host whose transcript the cap trimmed past that line, and its end sends the outcome alone"
#[test]
fn a_held_replys_line_is_not_sent_again_by_a_restarted_host_whose_transcript_the_cap_trimmed_past_that_line_and_its_end_sends_the_outcome_alone(
) {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::new();
        let (rt1, ws, run) = held_line_sent(&h, &state.store, &backend).await;
        for i in 0..TRANSCRIPT_CAP + 100 {
            h.emit(&run, json!({ "type": "turn.delta", "sessionId": "sess-1", "kind": "text", "text": format!("working {i}") }));
        }
        let (r, w) = (rt1.rt.clone(), ws.clone());
        wait!([r, w] notifies(&r, &w).await.is_empty());
        rt1.rt.close().await.expect("a close");

        let rt2 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        let told = rt2.events();
        assert_eq!(statuses(&rt2.rt, &ws).await, ["running"]);
        h.emit(
            &run,
            json!({ "type": "turn.done", "sessionId": "sess-1", "result": { "status": "completed", "text": "Pushed; CI runs in the background." }, "held": true }),
        );
        h.emit(&run, json!({ "type": "session.end", "sessionId": "sess-1", "exitCode": 0, "sawResult": true }));
        let (r, w) = (rt2.rt.clone(), ws.clone());
        wait!([r, w] rows(&r, &w).await[0]["status"] == "completed");
        let said: Vec<String> = told.of("session.notify").iter().map(|e| e["text"].as_str().unwrap_or_default().to_string()).collect();
        // [expect.stringMatching(/^thread \w+ finished \(completed\)$/)]
        assert_eq!(said.len(), 1, "{said:?} in the history: {:?}", notifies(&rt2.rt, &ws).await);
        assert_eq!(without_thread(&said[0], "finished (completed)"), "", "{said:?}");
        assert!(said[0].starts_with("thread ") && said[0].ends_with(" finished (completed)"), "{said:?}");
        rt2.rt.close().await.expect("a close");
    });
}

// "a turn the host comes back to" > "a host cut after a held reply's line sends the cut's line, which the held line promised: a nap, and a restart that finds the run gone"
#[test]
fn a_host_cut_after_a_held_replys_line_sends_the_cuts_line_which_the_held_line_promised_a_nap_and_a_restart_that_finds_the_run_gone() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::new();
        let (napped, ws, _) = held_line_sent(&h, &state.store, &backend).await;
        napped.rt.workspaces().nap(&ws, None).await.expect("a nap");
        let (r, w) = (napped.rt.clone(), ws.clone());
        wait!([r, w] rows(&r, &w).await[0]["status"] != "running");
        let told: Vec<String> = notifies(&napped.rt, &ws).await.iter().map(|t| without_thread(t, "")).collect();
        assert_eq!(told.len(), 2, "{told:?}");
        assert_eq!(told[0], format!("finished (completed): {}", still_running_line(1.0)));
        assert!(is_failed_line(&told[1]), "{told:?}");
        napped.rt.close().await.expect("a close");

        let state2 = sqlite_state();
        let (gone, ws2, run2) = held_line_sent(&h, &state2.store, &backend).await;
        gone.rt.close().await.expect("a close");
        h.sweep(&run2);
        let rt2 = runtime(&backend).store(&state2.store).adapter("claude", h.factory()).build();
        assert!(!statuses(&rt2.rt, &ws2).await.contains(&"running".to_string()));
        assert_eq!(history_types(&rt2.rt, &ws2).await, ["session.start", "session.notify", "session.notify", "session.end"]);
        let second = notifies(&rt2.rt, &ws2).await.remove(1);
        assert!(second.contains("finished (failed"), "{second}");
        assert!(
            second
                .split("finished (failed")
                .nth(1)
                .is_some_and(|rest| rest.split_once("): ").is_some_and(|(inside, _)| !inside.contains(')'))),
            "{second}"
        );
        rt2.rt.close().await.expect("a close");
    });
}

// "a turn the host comes back to" > "a host cut after the agent was woken under a held reply sends the cut's line"
#[test]
fn a_host_cut_after_the_agent_was_woken_under_a_held_reply_sends_the_cuts_line() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::new();
        let (rt, ws, run) = held_line_sent(&h, &state.store, &backend).await;
        h.emit(&run, json!({ "type": "session.start", "sessionId": "sess-1" }));
        rt.rt.workspaces().nap(&ws, None).await.expect("a nap");
        let (r, w) = (rt.rt.clone(), ws.clone());
        wait!([r, w] notifies(&r, &w).await.len() == 2);
        assert!(notifies(&rt.rt, &ws).await[1].contains("finished (failed)"));
        rt.rt.close().await.expect("a close");
    });
}
