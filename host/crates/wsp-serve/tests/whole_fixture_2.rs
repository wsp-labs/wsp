// SPDX-License-Identifier: AGPL-3.0-only
//! The whole-runtime fixture's own tests (#2476 join B): each proves the fixture answers as the TS `stubBackend` and
//! its helpers do (packages/runtime/test/stub-backend.ts, runtime-fixture.ts, held-port.ts, split-gateway.ts at
//! 65f6b89c7), with the TS line each one reads.
//! Part 2 of 2: the fixture cases part 2d added; the helpers both parts read are in whole_fixture/helpers.rs.
#![allow(dead_code)]
mod common;

use std::rc::Rc;
use std::sync::Arc;

use common::*;
use wsp_core::types::api::StartOptions;
use wsp_engine::errors::EngineError;
use wsp_engine::machine::{ExecOptions, MachineSpec};

include!("whole_fixture/helpers.rs");

// runtime-steer.test.ts:191 `steerable` and steer-images.test.ts:15 `harness`: a hand-driven harness keeps what each start and
// steer was handed, answers a steer as told, and says the events the test gives it.
#[test]
fn a_live_harness_keeps_its_starts_and_steers_answers_as_told_and_says_what_the_test_emits() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let live = Live::new(LiveOptions {
            steers: true,
            announces: false,
            session: Some("66666666-6666-4666-8666-666666666666".into()),
            ..Default::default()
        });
        let fx = runtime(&backend).adapter("claude", live.factory()).build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let run = fx
            .rt
            .sessions()
            .start(&ws.view.id, StartOptions { prompt: "go".into(), request_id: Some("req_1".into()), ..Default::default() }, None)
            .await
            .expect("a start");
        assert_eq!(live.turns(), 1);
        live.init(0);
        let steer =
            wsp_core::types::api::SteerAsk { prompt: "and say pineapple".into(), request_id: Some("req_2".into()), attachments: None };
        let taken = fx.rt.sessions().steer(&run.id(), steer.clone(), None).await.expect("a steer");
        assert_eq!(j(&taken), json!({ "outcome": "accepted" }));
        live.answer.set(false);
        let missed = fx.rt.sessions().steer(&run.id(), steer, None).await.expect("a steer");
        assert_eq!(j(&missed)["outcome"], "not-running");
        assert_eq!(live.steered.borrow().iter().map(|s| s.prompt.as_str()).collect::<Vec<_>>(), ["and say pineapple", "and say pineapple"]);
        assert_eq!(live.starts.borrow()[0].prompt, "go");
        live.end(0, "ok");
        assert_eq!(j(&run.finished().await)["text"], "ok");
        fx.rt.close().await.expect("a close");
    });
}

// bring-back.test.ts:18 `fakeDaemon`: every frame is kept with the token of its dial, an op is answered as the test says
// or as the default does, the dials and closes are counted, and a push reaches every open channel.
#[test]
fn a_recording_daemon_keeps_frames_with_their_dial_answers_per_op_counts_closes_and_pushes() {
    use wsp_core::daemon_channel::DaemonChannelOptions;
    drive(|| async {
        let daemon = RecDaemon::new(|_| json!({ "ok": true, "default": true }));
        daemon.answer("git.pr", |f| json!({ "ok": false, "error": format!("no pr for {}", f["cwd"].as_str().unwrap_or_default()) }));
        daemon.hello.set(Some(7));
        let heard: Rc<std::cell::RefCell<Vec<serde_json::Value>>> = Rc::default();
        let sink = heard.clone();
        let open = daemon.open();
        let channel = open(DaemonChannelOptions {
            url: "http://127.0.0.1:9".into(),
            token: "tok".into(),
            on_event: Rc::new(move |e| sink.borrow_mut().push(e)),
            connect_timeout_ms: None,
            clock: None,
        })
        .await
        .expect("a channel");
        let frame = |doc: serde_json::Value| doc.as_object().cloned().expect("an object");
        let pushed = channel.send(frame(json!({ "op": "git.push", "cwd": "/a" }))).await.expect("a reply");
        assert_eq!(serde_json::Value::Object(pushed), json!({ "id": 1, "ok": true, "default": true }));
        let pr = channel.send(frame(json!({ "op": "git.pr", "cwd": "/a" }))).await.expect("a reply");
        assert_eq!(serde_json::Value::Object(pr), json!({ "id": 1, "ok": false, "error": "no pr for /a" }));
        assert_eq!(daemon.ops(), ["git.push", "git.pr"]);
        assert_eq!(*daemon.tokens.borrow(), ["tok", "tok"]);
        assert_eq!(*daemon.dials.borrow(), [("http://127.0.0.1:9".to_string(), "tok".to_string())]);
        sleep_ms(10).await;
        daemon.push(json!({ "type": "pty.exit", "ptyId": "p1" }));
        assert_eq!(*heard.borrow(), [json!({ "type": "daemon.hello", "version": 7 }), json!({ "type": "pty.exit", "ptyId": "p1" })]);
        channel.close();
        daemon.push(json!({ "type": "after close" }));
        assert_eq!(heard.borrow().len(), 2);
        assert_eq!(daemon.closed.get(), 1);
    });
}

// stub-backend.ts:399 `projectOn(rt, computer, source, { base })`: a project that names the base its pull requests open against.
#[test]
fn project_with_names_the_base_a_project_opens_its_pull_requests_against() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).build();
        let named = project_with(&fx.rt, None, None, None, Some("main")).await.expect("a project");
        let plain = project_on(&fx.rt, None, None, None).await.expect("a project");
        assert_eq!(j(&named)["base"], "main");
        assert!(j(&plain).get("base").is_none());
    });
}

// stub-backend.ts:437 `createOn(rt, { agents, ... })`: the keys of `agents` a test names reach the create.
#[test]
fn create_on_hands_the_agents_to_the_create() {
    use common::projects::AgentsCaps;
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).build();
        let made = create_on(&fx.rt, CreateOn::golden("a").agents(2, 3), None).await.expect("a workspace");
        let view = j(&made.view);
        assert_eq!(view["agents"]["maxMachines"], 2);
        assert_eq!(view["agents"]["maxDepth"], 3);
    });
}

// rewind.test.ts:53 `fakeDaemon({ unreached })` and the `open` that rejects with a door's refusal: a dial refused, a send that
// never reaches the daemon and so keeps no frame.
#[test]
fn a_recording_daemon_refuses_dials_and_fails_sends_where_a_test_says() {
    use wsp_core::daemon_channel::{ChannelError, DaemonChannelOptions, DaemonDoorError};
    drive(|| async {
        let daemon = RecDaemon::new(|_| json!({ "ok": true }));
        let door = Rc::new(std::cell::Cell::new(true));
        let shut = door.clone();
        daemon.refusing_dials(move || {
            shut.get().then(|| {
                let mut door = wsp_words::exit::HostError::with_kind(DaemonDoorError::new(302, "").message, wsp_words::exit::Kind::Refused);
                door.status = Some(302.0);
                door
            })
        });
        let dial = || DaemonChannelOptions {
            url: "http://127.0.0.1:9".into(),
            token: "tok".into(),
            on_event: Rc::new(|_| {}),
            connect_timeout_ms: None,
            clock: None,
        };
        let open = daemon.open();
        let refused = open(dial()).await.err().expect("a refusal");
        assert_eq!(refused.message, "the connection was refused with 302");
        assert_eq!(refused.status, Some(302.0));
        assert!(daemon.dials.borrow().is_empty());
        door.set(false);
        let channel = open(dial()).await.expect("a channel");
        let down = Rc::new(std::cell::Cell::new(true));
        let stopped = down.clone();
        daemon.failing_sends(move || stopped.get().then(|| ChannelError::Refused { message: "Sandbox machine is not running.".into() }));
        let failed = channel.send(json!({ "op": "git.push" }).as_object().cloned().expect("an object")).await.expect_err("a failure");
        assert_eq!(failed.to_string(), "Sandbox machine is not running.");
        assert!(daemon.frames.borrow().is_empty());
        down.set(false);
        channel.send(json!({ "op": "git.push" }).as_object().cloned().expect("an object")).await.expect("a reply");
        assert_eq!(daemon.ops(), ["git.push"]);
    });
}

// sqlite-transcripts.test.ts:233, :341 and :462: a store that keeps the collection of each blob written, counts the reads of a
// folder's rows and of a thread's words, and fails the folder read where a test says.
#[test]
fn a_watched_store_counts_the_reads_of_its_rows_and_keeps_each_blob_collection() {
    drive(|| async {
        let state = sqlite_state();
        let watched = Watched::over(&state.store);
        let store = watched.store();
        store.put_blob("vaults", "a", vec![1]).await.expect("a blob");
        assert_eq!(*watched.blobs.borrow(), ["vaults"]);
        let rows = store.transcripts().expect("rows");
        rows.all("w").expect("rows");
        rows.word_events("w", "t").expect("rows");
        assert_eq!((watched.rows.folder_reads.get(), watched.rows.word_reads.get()), (1, 1));
        watched.rows.fail_all.set(true);
        assert_eq!(rows.all("w").expect_err("a failure").message, "disk I/O error");
        assert_eq!(watched.rows.folder_reads.get(), 2);
    });
}

// worktree-sweep.test.ts:136 `host()` and folder-threads.test.ts:128 `here()`: a host stood on this computer reads a real folder,
// and `Quick` answers each turn at once with the folder it was started in.
#[test]
fn a_host_on_this_computer_runs_a_quick_turn_in_the_project_folder_and_makes_no_copy() {
    use common::local::{git_copier, local_wiring};
    drive(|| async {
        let dir = tempfile::tempdir().expect("a folder");
        let root = std::fs::canonicalize(dir.path()).expect("a real path");
        let repo = temp_repo();
        let (copier, log) = git_copier();
        let quick = Quick::new(QuickOptions::default());
        let backend = StubBackend::new(None);
        let local = local_wiring(&root, copier);
        let state = sqlite_state();
        let fx = runtime(&backend).store(&state.store).adapter("claude", quick.factory()).with(move |o| o.local = Some(local)).build();
        let project = fx
            .rt
            .projects()
            .add(wsp_core::types::api::ProjectAddAsk { source: repo.display().to_string(), ..Default::default() }, None)
            .await
            .expect("a project");
        let at = fx
            .rt
            .workspaces()
            .folder_for(wsp_core::types::api::FolderForAsk { project: Some(project.project.id.clone()), ..Default::default() }, None)
            .await
            .expect("a folder");
        let run = fx
            .rt
            .sessions()
            .start(&at.workspace.id, StartOptions { prompt: "hello".into(), ..Default::default() }, None)
            .await
            .expect("a start");
        run.finished().await;
        assert_eq!(quick.cwds(), [Some(repo.display().to_string())]);
        assert!(log.lock().unwrap().asks.is_empty());
        assert!(log.lock().unwrap().worktrees.is_empty());
        fx.rt.close().await.expect("a close");
    });
}

// stub-script.ts `writeStub`: a script the shell can run, made executable.
#[test]
fn write_stub_makes_a_script_the_shell_can_run() {
    let dir = tempfile::tempdir().expect("a folder");
    let path = dir.path().join("codex");
    common::local::write_stub(&path, "#!/bin/sh\necho stubbed\n");
    let out = std::process::Command::new(&path).output().expect("the stub runs");
    assert_eq!(String::from_utf8_lossy(&out.stdout), "stubbed\n");
}

// fed-runs.ts: runs of the real adapter fed by hand: lines pushed reach the stream, a write that loses its answer lands or
// not as told, and an attach after a restart reads the run again from its first line with what its input took.
#[test]
fn a_fed_run_streams_the_lines_pushed_loses_a_write_as_told_and_reads_again_on_attach() {
    use futures::StreamExt as _;
    use wsp_adapters::port::{AttachRunOptions, Attached, ExecOptions};
    drive(|| async {
        let (factory, fed) = common::fed::fed_runs();
        let stream = factory.launch("claude", ExecOptions { input: Some(vec!["open the run".into()]), ..Default::default() });
        let run = fed.run_of("open the run").expect("the run");
        run.push(&["one".into(), "two".into()]);
        let mut lines = stream.lines();
        assert_eq!(lines.next().await.expect("a line").expect("ok"), "one");
        assert_eq!(lines.next().await.expect("a line").expect("ok"), "two");
        run.loses.set(Some(common::fed::Loses::Landed));
        assert!(stream.write("{\"uuid\":\"u1\"}".into()).await.is_err());
        run.loses.set(Some(common::fed::Loses::Lost));
        assert!(stream.write("{\"uuid\":\"u2\"}".into()).await.is_err());
        stream.write("{\"uuid\":\"u3\"}".into()).await.expect("a write");
        assert_eq!(run.writes(), ["{\"uuid\":\"u1\"}", "{\"uuid\":\"u3\"}"]);
        assert_eq!(fed.steered("open the run", 1), "u3");
        run.end(Some(0));
        assert!(lines.next().await.is_none());
        let id = run.id.clone();
        let attached =
            factory.attach(&id, AttachRunOptions { input: true, started_at: 0.0, from: None }).expect("an attach").await.expect("found");
        let Attached::Stream(again) = attached else { panic!("the run is gone") };
        let mut replay = again.lines();
        assert_eq!(replay.next().await.expect("a line").expect("ok"), "one");
        assert_eq!(again.taken().expect("what its input took"), ["open the run", "{\"uuid\":\"u1\"}", "{\"uuid\":\"u3\"}"]);
        let gone = factory
            .attach("/tmp/wsp-run/none", AttachRunOptions { input: true, started_at: 0.0, from: None })
            .expect("an attach")
            .await
            .expect("an answer");
        assert!(matches!(gone, Attached::Gone));
    });
}

// steer-unread.test.ts `host()` and turn-costs.test.ts `runtime()`: a wiring on this computer carries the launch environment, the run
// folder and the poll a test names.
#[test]
fn a_local_wiring_carries_the_launch_variables_and_the_run_folder_a_test_names() {
    use common::local::{git_copier, local_wiring_with, LocalOpts};
    let dir = tempfile::tempdir().expect("a folder");
    let (copier, _log) = git_copier();
    let vars = indexmap::IndexMap::from([("STUB_MODE".to_string(), "dies".to_string())]);
    let local = local_wiring_with(dir.path(), copier, LocalOpts { vars: Some(vars.clone()), poll_ms: Some(20), ..Default::default() });
    assert_eq!((local.env)(), vars);
    assert_eq!((local.home)("claude"), format!("{}/.claude", dir.path().display()));
    assert_eq!(local.folder, dir.path().display().to_string());
    assert!(local.copier.is_some());
}

// channels.test.ts:139 `brokeredFork`: where the broker is ready the record keeps the branch of record a create names, and where it is
// not the record keeps none.
#[test]
fn a_ready_broker_makes_the_record_keep_the_branch_of_record_a_create_names() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).with(|o| o.broker = Some(TestBroker::ready().rc())).build();
        let made =
            create_on(&fx.rt, CreateOn::golden("a").branch_of_record(4242, "acme", "lab", "wsp/a"), None).await.expect("a workspace");
        let record = fx.store.get("workspaces", &made.view.id).await.expect("a read").expect("a record");
        assert_matches(&record["branchOfRecord"], json!({ "repoId": 4242, "owner": "acme", "name": "lab", "branch": "wsp/a" }));
        let backend = StubBackend::new(None);
        let off = runtime(&backend).with(|o| o.broker = Some(TestBroker::gated(|| false).rc())).build();
        let made =
            create_on(&off.rt, CreateOn::golden("b").branch_of_record(4242, "acme", "lab", "wsp/b"), None).await.expect("a workspace");
        let record = off.store.get("workspaces", &made.view.id).await.expect("a read").expect("a record");
        assert!(record.get("branchOfRecord").is_none());
    });
}

// conversations.test.ts `fakeLocal` over `new LocalBackend({ env })`: every exec of this computer's backend runs under the variables a
// test names.
#[test]
fn a_local_wiring_runs_the_backends_execs_under_the_variables_a_test_names() {
    use common::local::{git_copier, local_wiring_with, LocalOpts};
    drive(|| async {
        let dir = tempfile::tempdir().expect("a folder");
        let (copier, _log) = git_copier();
        let vars =
            indexmap::IndexMap::from([("FIXTURE_MARK".to_string(), "kept".to_string()), ("PATH".to_string(), "/usr/bin:/bin".to_string())]);
        let local = local_wiring_with(dir.path(), copier, LocalOpts { backend_vars: Some(vars), ..Default::default() });
        let machine = local.backend.get("local").await.expect("a machine");
        let said = machine.exec("printf %s \"$FIXTURE_MARK\"", ExecOptions::default()).await.expect("an exec");
        assert_eq!(said.stdout, "kept");
    });
}

// ws-client.ts `WsClient`: a socket authenticated with a token answers a request by its id and keeps the frames the host pushes it.
#[test]
fn a_ws_client_answers_a_request_and_keeps_the_events_the_host_pushes() {
    use common::local::WsClient;
    use wsp_serve::serve::{serve_runtime, ServeOptions};
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).build();
        let served = serve_runtime(fx.rt.clone(), ServeOptions { port: Some(0), auth_token: "secret".into(), ..Default::default() })
            .await
            .expect("the runtime serves");
        let mut client = WsClient::connect(served.port(), "secret").await;
        assert_eq!(client.request("events.subscribe", json!({})).await["ok"], true);
        let made = create_on(&fx.rt, CreateOn::golden("pushed"), None).await.expect("a workspace");
        let events = client.events.clone();
        until(|| async {
            events.borrow().iter().any(|e| e["type"] == "workspace.created" && e["workspace"]["id"] == made.view.id.as_str())
        })
        .await;
        client.close();
        served.close().await.expect("a close");
        fx.rt.close().await.expect("a close");
    });
}

// folder-threads.test.ts `tools()` and `machine.exec = ...`: a backend whose one machine runs every command through a hook of the test's own
// first, which may change the command or answer in its place.
#[test]
fn an_around_backend_runs_each_command_through_the_hook_that_may_change_or_answer_it() {
    use common::local::AroundBackend;
    use wsp_engine::machine::{ExecResult, MachineBackend as _};
    drive(|| async {
        let dir = tempfile::tempdir().expect("a folder");
        let seen: Rc<std::cell::RefCell<Vec<String>>> = Rc::default();
        let kept = seen.clone();
        let around: common::local::Around = Rc::new(move |cmd: String| {
            kept.borrow_mut().push(cmd.clone());
            Box::pin(async move {
                if cmd.contains("refuse") {
                    return Err(ExecResult { exit_code: 1.into(), stdout: String::new(), stderr: "refused".into() });
                }
                Ok(cmd.replace("one", "two"))
            })
        });
        let backend = AroundBackend::new(dir.path(), None, around);
        let machine = backend.get("local").await.expect("a machine");
        assert_eq!(machine.exec("printf one", ExecOptions::default()).await.expect("an exec").stdout, "two");
        let refused = machine.exec("refuse", ExecOptions::default()).await.expect("an answer");
        assert_eq!((refused.exit_code.0 .0 as i64, refused.stderr.as_str()), (1, "refused"));
        assert_eq!(*seen.borrow(), ["printf one", "refuse"]);
    });
}

// folder-threads.test.ts `here()`: a host over a folder of its own where a thread runs in the project's folder, with no copy made.
#[test]
fn a_folder_host_runs_a_thread_in_the_projects_folder_and_keeps_every_daemon_frame() {
    use common::folder_host::{here, HereOpts, Scratch};
    use wsp_core::types::api::{FolderForAsk, ProjectAddAsk};
    drive(|| async {
        let scratch = Scratch::new();
        let h = here(&scratch, HereOpts::default());
        let folder = scratch.repo();
        let project =
            h.fx.rt
                .projects()
                .add(ProjectAddAsk { source: folder.display().to_string(), ..Default::default() }, None)
                .await
                .expect("a project");
        let at =
            h.fx.rt
                .workspaces()
                .folder_for(FolderForAsk { project: Some(project.project.id), ..Default::default() }, None)
                .await
                .expect("a folder");
        h.fx.rt
            .sessions()
            .start(&at.workspace.id, StartOptions { prompt: "hello".into(), ..Default::default() }, None)
            .await
            .expect("a start")
            .finished()
            .await;
        assert_eq!(h.agents.cwds(), [Some(folder.display().to_string())]);
        until(|| async { h.ops().iter().any(|o| o == "git.checkpoint") }).await;
        assert!(h.copier.lock().unwrap().asks.is_empty());
        h.fx.rt.close().await.expect("a close");
    });
}

// folder-threads.test.ts `here(..., again)` and `sqliteStore(...)`: a folder host writes to the store a test hands it, and not to its own.
#[test]
fn a_folder_host_writes_to_the_store_a_test_hands_it() {
    use common::folder_host::{add, ask, folder_for, here, HereOpts, Scratch};
    drive(|| async {
        let scratch = Scratch::new();
        let mine = sqlite_state();
        let h = here(&scratch, HereOpts { store: Some(mine.store.clone()), ..Default::default() });
        let project = add(&h, &scratch.repo()).await;
        folder_for(&h, ask(project["id"].as_str().expect("an id"), None, None)).await;
        assert_eq!(mine.store.list("workspaces").await.expect("a list").len(), 1);
        assert!(h.state.store.list("workspaces").await.expect("a list").is_empty());
        h.fx.rt.close().await.expect("a close");
    });
}
