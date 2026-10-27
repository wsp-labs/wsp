// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/runtime-create.test.ts at 65f6b89c7, case for case. Each test names the TS test above it.
mod common;

use std::rc::Rc;
use std::sync::Arc;

use common::*;
use serde_json::Value;
use wsp_core::clock::FakeClock;
use wsp_core::console::capture;
use wsp_core::status::StatusWatchOptions;
use wsp_core::types::api::PrepareAsk;
use wsp_core::types::internal::copy_key;
use wsp_core::types::wiring::{wired_place, WiredId};
use wsp_core::types::wiring::{GoldenRecipe, WakeOptions};
use wsp_engine::vault::CacheRule;
use wsp_wire::types::JsNumber;
use wsp_words::words::thread::image_served_wait_line;

const TOKEN: &str = "deadbeefdeadbeefdeadbeef";

/// A stub whose forks carry an edge route to the given port and whose guest hands out the daemon token.
fn edge_backend(port: u16) -> Arc<StubBackend> {
    let backend = StubBackend::new(None);
    backend.set_exec_impl(token_guest());
    let create = backend.create_fn();
    backend.set_create(Rc::new(move |spec, waiting, sent| {
        let made = create(spec, waiting, sent);
        Box::pin(async move {
            let m = made.await?;
            m.set_preview_url(&format!("ws://127.0.0.1:{port}"));
            Ok(m)
        })
    }));
    backend
}
fn stage_names(stages: &[Value]) -> Vec<String> {
    stages.iter().map(|e| e["stage"].as_str().unwrap_or_default().to_string()).collect()
}

fn messages(stages: &[Value]) -> Vec<String> {
    stages.iter().map(|e| e["message"].as_str().unwrap_or_default().to_string()).collect()
}

/// `^Cloning https:\/\/github\.com\/wsp\/stub-\d+\.git into \/root\/stub-\d+\.$`
fn is_clone_line(line: &str) -> bool {
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    line.strip_prefix("Cloning https://github.com/wsp/stub-")
        .and_then(|rest| rest.split_once(".git into /root/stub-"))
        .and_then(|(n, rest)| rest.strip_suffix('.').map(|m| (n, m)))
        .is_some_and(|(n, m)| digits(n) && digits(m))
}

// "runtime create stages" > "a plain create reports every awaited step in order, names the hostname before the daemon is asked, and announces created last"
#[test]
fn a_plain_create_reports_every_awaited_step_in_order_names_the_hostname_before_the_daemon_is_asked_and_announces_created_last() {
    drive(|| async {
        let daemon = daemon_of(TOKEN, "m1").await;
        let backend = edge_backend(daemon.port);
        let fx = runtime(&backend).with(|o| o.daemon_token = Some(TOKEN.into())).build();
        let events = fx.events();
        let ws = create_on(&fx.rt, CreateOn::golden("task-1"), None).await.expect("a workspace");
        let stages = events.creating();
        assert_eq!(
            stage_names(&stages),
            ["fork-requested", "hostname-set", "preview-route", "daemon-answering", "project-cloned", "ready"]
        );
        let said = messages(&stages);
        assert_eq!(
            said[..4],
            ["starting task-1 on default", "hostname set to task-1", "Preview route to the daemon minted.", "Daemon answered."]
        );
        assert!(is_clone_line(&said[4]), "the clone line: {}", said[4]);
        assert_eq!(said[5], "ready");
        for e in &stages {
            assert!(!e["message"].as_str().unwrap().contains(backend.machine(0).id()));
            assert_eq!(e["workspaceId"], ws.view.id);
            assert_eq!(e["name"], "task-1");
            assert!(e["elapsedMs"].is_number());
            assert!(e.get("notice").is_none());
        }
        let log = backend.machine(0).exec_log();
        let hostname = log.iter().position(|c| c == "hostname task-1 && echo task-1 > /etc/hostname").expect("the hostname step");
        let token = log.iter().position(|c| c.contains(wsp_wire::types::daemon_contract::DAEMON_TOKEN_PATH)).expect("the token step");
        assert!(hostname < token);
        let ready = events.position(|e| e["type"] == "workspace.creating" && e["stage"] == "ready").expect("ready");
        assert_eq!(events.position(|e| e["type"] == "workspace.created"), Some(ready + 1));
    });
}

// "a provider's wait for the image it forks from is a line of the create, under the step it waits in"
#[test]
fn a_providers_wait_for_the_image_it_forks_from_is_a_line_of_the_create_under_the_step_it_waits_in() {
    drive(|| async {
        let backend = StubBackend::new(None);
        backend.set_exec_impl(token_guest());
        let create = backend.create_fn();
        backend.set_create(Rc::new(move |spec, waiting, sent| {
            if let Some(waiting) = &waiting {
                waiting(&image_served_wait_line("Solari", 15_000.0), None);
            }
            create(spec, waiting, sent)
        }));
        let fx = runtime(&backend).build();
        let events = fx.events();
        create_on(&fx.rt, CreateOn::golden("waits"), None).await.expect("a workspace");
        let stages = events.creating();
        let first_two: Vec<(String, String)> =
            stages[..2].iter().map(|e| (e["stage"].as_str().unwrap().into(), e["message"].as_str().unwrap().into())).collect();
        assert_eq!(
            first_two,
            [
                ("fork-requested".to_string(), "starting waits on default".to_string()),
                ("fork-requested".to_string(), "waiting for Solari to serve the image; asking again in 15s".to_string()),
            ]
        );
        // Marked, so a surface showing the step folds each ask into it rather than drawing a step per ask.
        let waiting: Vec<Value> = stages.iter().map(|e| e.get("waiting").cloned().unwrap_or(Value::Null)).collect();
        let mut want = vec![Value::Null, json!(true)];
        want.extend(stages[2..].iter().map(|_| Value::Null));
        assert_eq!(waiting, want);
    });
}

// "a guest that refuses the hostname gets a verdict in the log, its own words on the line alone, and the create goes on"
#[test]
fn a_guest_that_refuses_the_hostname_gets_a_verdict_in_the_log_its_own_words_on_the_line_alone_and_the_create_goes_on() {
    drive(|| async {
        let backend = StubBackend::new(None);
        // The refusal a Linux guest's hostname step answers with where the kernel will not have it.
        backend.set_exec_impl(exec_fn(|_, cmd| {
            Ok(if cmd.starts_with("hostname ") {
                exec_exit(1, "", "hostname: sethostname: Operation not permitted\n")
            } else {
                token_guest_answer(cmd)
            })
        }));
        let place = wired_place(WiredId::Fixed("ascii".into()), backend.clone());
        let fx = runtime(&backend).with(move |o| o.places = Some(place)).build();
        let events = fx.events();
        let warned = capture();
        let ws = create_on(&fx.rt, CreateOn::golden("clone-test"), None).await.expect("a workspace");
        drop(warned);
        let stages = events.creating();
        assert_eq!(stages[0]["message"], "starting clone-test on ascii");
        let named = stages.iter().find(|e| e["stage"] == "hostname-set").expect("the hostname step");
        assert_eq!(named["message"], "hostname not set; the workspace keeps the machine's own name");
        // The guest's own words are evidence on the line's title, never a second sentence at a person, and never a
        // notice, which every surface draws as a line of its own.
        assert_eq!(named["detail"], "hostname clone-test on m1 failed: hostname: sethostname: Operation not permitted");
        assert!(named.get("notice").is_none());
        for e in &stages {
            let line = e["message"].as_str().unwrap();
            assert!(!line.contains("sethostname"));
            assert!(!line.contains("m1"));
        }
        assert_eq!(fx.rt.workspaces().get(&ws.view.id, None, None).await.expect("the workspace").name, "clone-test");
    });
}

// "a fork whose daemon never answers still becomes a workspace: the stage says so, with the backend's daemon budget in the fault"
#[test]
fn a_fork_whose_daemon_never_answers_still_becomes_a_workspace_the_stage_says_so_with_the_backends_daemon_budget_in_the_fault() {
    drive(|| async {
        let port = dropping_port().await;
        let backend = edge_backend(port.port);
        backend.lifecycle().set_daemon_answers_ms(300.0);
        let fx = runtime(&backend).with(|o| o.daemon_token = Some(TOKEN.into())).build();
        let events = fx.events();
        let ws = create_on(&fx.rt, CreateOn::golden("task-1"), None).await.expect("a workspace");
        let stages = events.creating();
        assert_eq!(
            stage_names(&stages),
            ["fork-requested", "hostname-set", "preview-route", "daemon-answering", "project-cloned", "ready"]
        );
        assert_eq!(stages[3]["message"], "Daemon did not answer.");
        assert!(stages[3]["notice"].as_str().unwrap().contains("daemon on m1 did not answer within 300 ms"), "{}", stages[3]["notice"]);
        assert!(!backend.machine(0).killed.get());
        let listed = fx.rt.workspaces().list(None).await.expect("a list");
        assert_eq!(listed.iter().map(|w| w.id.as_str()).collect::<Vec<_>>(), [ws.view.id.as_str()]);
    });
}

// "a fork whose machine answers for its own daemon is asked over that road, and the route this computer cannot dial decides nothing"
#[test]
fn a_fork_whose_machine_answers_for_its_own_daemon_is_asked_over_that_road_and_the_route_this_computer_cannot_dial_decides_nothing() {
    drive(|| async {
        let port = dropping_port().await;
        let backend = edge_backend(port.port);
        backend.lifecycle().set_daemon_answers_ms(300.0);
        let create = backend.create_fn();
        backend.set_create(Rc::new(move |spec, waiting, sent| {
            let made = create(spec, waiting, sent);
            Box::pin(async move {
                let m = made.await?;
                m.set_daemon_answer(true);
                Ok(m)
            })
        }));
        let fx = runtime(&backend).with(|o| o.daemon_token = Some(TOKEN.into())).build();
        let events = fx.events();
        create_on(&fx.rt, CreateOn::golden("task-1"), None).await.expect("a workspace");
        let stages = events.creating();
        assert_eq!(stages[3]["stage"], "daemon-answering");
        assert_eq!(stages[3]["message"], "Daemon answered.");
        assert!(stages[3].get("notice").is_none());
        assert_eq!(backend.machine(0).calls.daemon_answers.get(), 1);
    });
}

// "a fork with no route at all is still asked: the daemon-answering stage stands without a preview-route stage"
#[test]
fn a_fork_with_no_route_at_all_is_still_asked_the_daemon_answering_stage_stands_without_a_preview_route_stage() {
    drive(|| async {
        let backend = StubBackend::new(None);
        backend.set_exec_impl(token_guest());
        let create = backend.create_fn();
        backend.set_create(Rc::new(move |spec, waiting, sent| {
            let made = create(spec, waiting, sent);
            Box::pin(async move {
                let m = made.await?;
                m.set_daemon_answer(true);
                Ok(m)
            })
        }));
        let fx = runtime(&backend).with(|o| o.daemon_token = Some(TOKEN.into())).build();
        let events = fx.events();
        create_on(&fx.rt, CreateOn::golden("task-1"), None).await.expect("a workspace");
        let stages = events.creating();
        assert!(backend.machine(0).preview_fn().is_none());
        assert_eq!(stage_names(&stages), ["fork-requested", "hostname-set", "daemon-answering", "project-cloned", "ready"]);
        assert_eq!(stages[2]["message"], "Daemon answered.");
        assert!(stages[2].get("notice").is_none());
    });
}

// "a mint that fails is its own line, and the daemon is still asked over the road it has"
#[test]
fn a_mint_that_fails_is_its_own_line_and_the_daemon_is_still_asked_over_the_road_it_has() {
    drive(|| async {
        let backend = StubBackend::new(None);
        backend.set_exec_impl(token_guest());
        let create = backend.create_fn();
        backend.set_create(Rc::new(move |spec, waiting, sent| {
            let made = create(spec, waiting, sent);
            Box::pin(async move {
                let m = made.await?;
                m.set_preview(Some(Rc::new(|_| {
                    Box::pin(async {
                        Err(wsp_engine::errors::EngineError::Other {
                            message: "container publishes no port 7070; only the daemon's own port is published".into(),
                            code: None,
                        })
                    })
                })));
                m.set_daemon_answer(true);
                Ok(m)
            })
        }));
        let fx = runtime(&backend).with(|o| o.daemon_token = Some(TOKEN.into())).build();
        let events = fx.events();
        create_on(&fx.rt, CreateOn::golden("task-1"), None).await.expect("a workspace");
        let stages = events.creating();
        assert_eq!(stages[2]["stage"], "preview-route");
        assert_eq!(stages[2]["message"], "No preview route to the daemon.");
        assert!(stages[2]["notice"].as_str().unwrap().contains("publishes no port 7070"));
        assert_eq!(stages[3]["stage"], "daemon-answering");
        assert_eq!(stages[3]["message"], "Daemon answered.");
        assert!(stages[3].get("notice").is_none());
    });
}

// "a create that fails after the fork kills its machine, reports failed with the reason, and lists nothing"
#[test]
fn a_create_that_fails_after_the_fork_kills_its_machine_reports_failed_with_the_reason_and_lists_nothing() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let wrapped = WrappedStore::over(&state.store);
        wrapped.refuse_puts("workspaces", "disk full");
        let fx = runtime(&backend).store(&wrapped.store()).build();
        let events = fx.events();
        let refused = create_on(&fx.rt, CreateOn::golden("task-1"), None).await.expect_err("a refusal");
        assert_eq!(refused.message, "disk full");
        let stages = events.creating();
        // The project goes in before the record is written, so the clone stands and the store's own failure ends it.
        assert_eq!(stage_names(&stages), ["fork-requested", "hostname-set", "project-cloned", "failed"]);
        assert_eq!(stages.last().unwrap()["message"], "disk full");
        assert!(backend.machine(0).killed.get());
        assert!(fx.rt.workspaces().list(None).await.expect("a list").is_empty());
        assert!(!events.has("workspace.created"));
    });
}

// "until ready the workspace is neither listed nor reachable, so nothing opens a shell under the old hostname"
#[test]
fn until_ready_the_workspace_is_neither_listed_nor_reachable_so_nothing_opens_a_shell_under_the_old_hostname() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let gate = Rc::new(tokio::sync::Notify::new());
        let waiting = gate.clone();
        backend.set_exec_impl(exec_async(move |_m, cmd| {
            let gate = waiting.clone();
            Box::pin(async move {
                if cmd.starts_with("hostname ") {
                    gate.notified().await;
                }
                Ok(exec_ok(""))
            })
        }));
        let fx = runtime(&backend).build();
        let events = fx.events();
        let rt = fx.rt.clone();
        let made = tokio::task::spawn_local(async move { create_on(&rt, CreateOn::golden("task-1"), None).await });
        until(|| {
            let seen = !backend.machines().is_empty() && !backend.machine(0).exec_starting("hostname ").is_empty();
            async move { seen }
        })
        .await;
        assert!(fx.rt.workspaces().list(None).await.expect("a list").is_empty());
        assert!(status_rows(&fx.rt).await.is_empty());
        let id = events.creating()[0]["workspaceId"].as_str().unwrap().to_string();
        let refused = fx.rt.workspaces().get(&id, None, None).await.expect_err("a refusal");
        assert!(refused.message.contains("no such workspace"), "{}", refused.message);
        gate.notify_one();
        let ws = made.await.expect("the task").expect("a workspace");
        let listed = fx.rt.workspaces().list(None).await.expect("a list");
        assert_eq!(listed.iter().map(|w| w.id.as_str()).collect::<Vec<_>>(), [ws.view.id.as_str()]);
        assert_eq!(fx.rt.workspaces().get(&ws.view.id, None, None).await.expect("the workspace").id, ws.view.id);
    });
}

/// A stub whose guest answers ls/tar/untar/token like a golden fork; tar and untar commands are recorded per machine.
struct Guest {
    backend: Arc<StubBackend>,
    tars: Rc<std::cell::RefCell<Vec<String>>>,
    untars: Rc<std::cell::RefCell<Vec<String>>>,
    tgz_bytes: Rc<std::cell::Cell<u64>>,
}

fn guest_backend() -> Guest {
    let backend = StubBackend::new(None);
    let tars: Rc<std::cell::RefCell<Vec<String>>> = Rc::default();
    let untars: Rc<std::cell::RefCell<Vec<String>>> = Rc::default();
    let tgz_bytes = Rc::new(std::cell::Cell::new(1_000));
    let (t, u, b) = (tars.clone(), untars.clone(), tgz_bytes.clone());
    backend.set_exec_impl(exec_fn(move |m, cmd| {
        if cmd.contains(wsp_wire::types::daemon_contract::DAEMON_TOKEN_PATH) {
            return Ok(token_guest_answer(cmd));
        }
        if cmd.contains("ls -A /root") {
            return Ok(exec_ok("notes.md\n.local\n"));
        }
        if cmd.starts_with("wc -c <") {
            return Ok(exec_ok(&format!("{}\n", b.get())));
        }
        if cmd.contains("tar czf") {
            t.borrow_mut().push(m.id().to_string());
        }
        if cmd.contains("tar xzf") {
            u.borrow_mut().push(m.id().to_string());
        }
        Ok(exec_ok(""))
    }));
    // The fetch the TS stubs globally: a PUT answers 200 and a GET serves `tarbytes`.
    backend.set_downloads(Rc::new(|_| b"tarbytes".to_vec()));
    Guest { backend, tars, untars, tgz_bytes }
}

fn last_status(events: &Events) -> Value {
    events.statuses().last().cloned().expect("a status")
}

/// `expect(p).rejects.toThrow(/.../)` over a refusal.
fn said(e: &wsp_words::exit::HostError, parts: &[&str]) {
    assert!(in_order(&e.message, parts), "the refusal {:?} lacks {parts:?}", e.message);
}

// "runtime verified wake" > "resume returns but the daemon never answers: waking, one retry, then the wake fails on the same machine, nothing forked, imported or killed"
#[test]
fn resume_returns_but_the_daemon_never_answers_waking_one_retry_then_the_wake_fails_on_the_same_machine_nothing_forked_imported_or_killed()
{
    drive(|| async {
        let g = guest_backend();
        let port = dropping_port().await;
        let state = sqlite_state();
        g.backend.lifecycle().set_daemon_answers_ms(300.0);
        let fx = runtime(&g.backend).store(&state.store).build();
        let events = fx.events();
        let ws = create_on(&fx.rt, CreateOn::golden("x"), None).await.expect("a workspace");
        let m1 = g.backend.machine(0);
        m1.set_preview_url(&format!("ws://127.0.0.1:{}", port.port));

        fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        assert_eq!(*g.tars.borrow(), ["m1"]);
        assert_eq!(state.store.get_blob("vaults", &ws.view.id).await.expect("a read"), Some(b"tarbytes".to_vec()));

        let refused = fx.rt.workspaces().wake(&ws.view.id, None).await.expect_err("a refusal");
        assert!(
            refused.message.starts_with("the wake of m1 did not finish: attempt 1: daemon on m1 did not answer"),
            "{}",
            refused.message
        );
        let phases: Vec<Value> = events.statuses().iter().map(|s| s["phase"].clone()).collect();
        assert_eq!(phases[..3], [json!("pausing"), json!("napping"), json!("waking")]);
        assert_eq!(m1.resumes.get(), 2);
        assert!(!m1.killed.get());
        assert_eq!(g.backend.made(), 1);
        assert!(g.untars.borrow().is_empty());
        assert_eq!(fx.rt.workspaces().get(&ws.view.id, None, None).await.expect("the workspace").machine_id, "m1");
        assert!(!events.has("workspace.woken"));
        let last = last_status(&events);
        assert_eq!(last["phase"], "napping");
        assert_eq!(last["machineId"], "m1");
        let reason = last["reason"].as_str().expect("a reason");
        assert!(
            in_order(
                reason,
                &[
                    "attempt 1: daemon on m1 did not answer within 300 ms",
                    "created as {\"cpu\":2,\"memMb\":4096",
                    "attempt 2:",
                    "; the workspace keeps this machine and its disk"
                ]
            ) && reason.ends_with("; the workspace keeps this machine and its disk"),
            "{reason}"
        );
    });
}

// "a backend declaring one wake attempt: resume, the check fails, no re-pause, and the wake fails on the same machine"
#[test]
fn a_backend_declaring_one_wake_attempt_resume_the_check_fails_no_re_pause_and_the_wake_fails_on_the_same_machine() {
    drive(|| async {
        let g = guest_backend();
        let port = dropping_port().await;
        g.backend.lifecycle().set_daemon_answers_ms(300.0);
        g.backend.lifecycle().set_wake_attempts(1);
        let fx = runtime(&g.backend).build();
        let events = fx.events();
        let ws = create_on(&fx.rt, CreateOn::golden("x"), None).await.expect("a workspace");
        let m1 = g.backend.machine(0);
        m1.set_preview_url(&format!("ws://127.0.0.1:{}", port.port));
        fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        assert_eq!(m1.calls.pause.get(), 1);
        let refused = fx.rt.workspaces().wake(&ws.view.id, None).await.expect_err("a refusal");
        said(&refused, &["daemon on m1 did not answer within 300 ms"]);
        // One resume and no re-pause after the nap's own: a provider that bills every start is not asked for a second.
        assert_eq!(m1.resumes.get(), 1);
        assert_eq!(m1.calls.pause.get(), 1);
        assert!(!m1.killed.get());
        assert_eq!(g.backend.made(), 1);
        assert!(g.untars.borrow().is_empty());
        let reason = last_status(&events)["reason"].as_str().expect("a reason").to_string();
        // The fault names the backend's own daemon budget, on a wake as on a fork.
        assert!(reason.starts_with("the wake of m1 did not finish: attempt 1: daemon on m1 did not answer within 300 ms"), "{reason}");
        assert!(!reason.contains("attempt 2"));
    });
}

// "a machine that answers for its own daemon is asked over that road on a wake, so a route this computer cannot dial never throws the container away"
#[test]
fn a_machine_that_answers_for_its_own_daemon_is_asked_over_that_road_on_a_wake_so_a_route_this_computer_cannot_dial_never_throws_the_container_away(
) {
    drive(|| async {
        let g = guest_backend();
        let port = dropping_port().await;
        g.backend.lifecycle().set_daemon_answers_ms(300.0);
        let fx = runtime(&g.backend).with(|o| o.daemon_token = Some(TOKEN.into())).build();
        let ws = create_on(&fx.rt, CreateOn::golden("x"), None).await.expect("a workspace");
        let m1 = g.backend.machine(0);
        // The route mints and nothing on this computer answers it: a container's published port on a box belongs to
        // that box's own loopback, and the wake check used to read that silence as a dead guest.
        m1.set_preview_url(&format!("ws://127.0.0.1:{}", port.port));
        m1.set_daemon_answer(true);
        fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        let woken = fx.rt.workspaces().wake(&ws.view.id, None).await.expect("a wake");
        assert_matches(&j(&woken), json!({ "machineId": "m1", "phase": "running" }));
        assert_eq!(m1.calls.daemon_answers.get(), 1);
        assert_eq!(g.backend.made(), 1);
        assert!(!m1.killed.get());
        assert!(g.untars.borrow().is_empty());
    });
}

// "a machine whose own answer is no still fails the wake, and the fault names the port nothing listens on"
#[test]
fn a_machine_whose_own_answer_is_no_still_fails_the_wake_and_the_fault_names_the_port_nothing_listens_on() {
    drive(|| async {
        let g = guest_backend();
        g.backend.lifecycle().set_daemon_answers_ms(300.0);
        let fx = runtime(&g.backend).with(|o| o.daemon_token = Some(TOKEN.into())).build();
        let events = fx.events();
        let ws = create_on(&fx.rt, CreateOn::golden("x"), None).await.expect("a workspace");
        let m1 = g.backend.machine(0);
        m1.set_daemon_answer(false);
        fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        let refused = fx.rt.workspaces().wake(&ws.view.id, None).await.expect_err("a refusal");
        said(&refused, &["nothing listens"]);
        assert_eq!(fx.rt.workspaces().get(&ws.view.id, None, None).await.expect("the workspace").machine_id, "m1");
        let reason = last_status(&events)["reason"].as_str().expect("a reason").to_string();
        assert!(reason.contains("nothing listens on the daemon's port inside m1"), "{reason}");
    });
}

// "a machine that cannot be asked at all says what happened, not that a budget ran out"
#[test]
fn a_machine_that_cannot_be_asked_at_all_says_what_happened_not_that_a_budget_ran_out() {
    drive(|| async {
        let g = guest_backend();
        g.backend.lifecycle().set_daemon_answers_ms(300.0);
        let fx = runtime(&g.backend).with(|o| o.daemon_token = Some(TOKEN.into())).build();
        let events = fx.events();
        let ws = create_on(&fx.rt, CreateOn::golden("x"), None).await.expect("a workspace");
        let m1 = g.backend.machine(0);
        m1.set_daemon_unaskable("container m1 is not running");
        fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        let refused = fx.rt.workspaces().wake(&ws.view.id, None).await.expect_err("a refusal");
        said(&refused, &["could not be asked"]);
        let reason = last_status(&events)["reason"].as_str().expect("a reason").to_string();
        assert!(reason.contains("could not be asked (container m1 is not running)"), "{reason}");
        assert!(!reason.contains("did not answer within"));
    });
}

// "a size mismatch fails the wake before any ping, naming both values"
#[test]
fn a_size_mismatch_fails_the_wake_before_any_ping_naming_both_values() {
    drive(|| async {
        let g = guest_backend();
        let warned = capture();
        let fx = runtime(&g.backend).build();
        let events = fx.events();
        let ws = create_on(&fx.rt, CreateOn::golden("x").mem(4096.0), None).await.expect("a workspace");
        let m1 = g.backend.machine(0);
        let minted = Rc::new(std::cell::Cell::new(0));
        let counted = minted.clone();
        m1.set_preview(Some(Rc::new(move |_| {
            counted.set(counted.get() + 1);
            Box::pin(async {
                Ok(wsp_engine::machine::PreviewReach {
                    url: "ws://127.0.0.1:1".into(),
                    token: "e".into(),
                    expires_at: wsp_wire::types::JsNumber((now_ms() + 3_600_000) as f64),
                })
            })
        })));
        fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        m1.set_shape(serde_json::from_value(json!({ "cpu": 2, "memMb": 2048, "createdAt": "2026-09-02T19:03:35.000Z" })).expect("a shape"));

        let started = std::time::Instant::now();
        let refused = fx.rt.workspaces().wake(&ws.view.id, None).await.expect_err("a refusal");
        said(&refused, &["memMb 2048 != 4096"]);
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
        assert_eq!(minted.get(), 0);
        assert_eq!(fx.rt.workspaces().get(&ws.view.id, None, None).await.expect("the workspace").machine_id, "m1");
        let reason = last_status(&events)["reason"].as_str().expect("a reason").to_string();
        assert!(reason.contains("memMb 2048 != 4096"), "{reason}");
        assert!(reason.contains("provider view {\"cpu\":2,\"memMb\":2048,\"createdAt\":\"2026-09-02T19:03:35.000Z\"}"), "{reason}");
        assert!(warned.lines().iter().any(|l| l.contains("2048") && l.contains("4096")));
    });
}

// "syncs the machine's disk before the vault is read off it and the provider pauses it"
#[test]
fn syncs_the_machines_disk_before_the_vault_is_read_off_it_and_the_provider_pauses_it() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).build();
        let ws = create_on(&fx.rt, CreateOn::golden("x"), None).await.expect("a workspace");
        let m = backend.machine(0);
        let before = m.exec_log().len();
        fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        let log: Vec<String> = m.exec_log()[before..].to_vec();
        let synced = log.iter().position(|c| c == wsp_engine::disk_sync::DISK_SYNC_CMD).expect("the disk sync");
        let tar = log.iter().position(|c| c.contains("tar czf")).expect("the vault tar");
        assert!(synced < tar);
        assert!(m.paused.get());
    });
}

// "%s is one warning naming the nap with the machine's own answer, and the vault is still stored and the machine still paused"
// (the two rows: "an exec that throws" and "a sync that exits 1")
#[test]
fn an_exec_that_throws_is_one_warning_naming_the_nap_with_the_machines_own_answer_and_the_vault_is_still_stored_and_the_machine_still_paused(
) {
    sync_refused(|| Err(wsp_engine::errors::EngineError::Other { message: "socket hang up".into(), code: None }), "socket hang up");
}

#[test]
fn a_sync_that_exits_1_is_one_warning_naming_the_nap_with_the_machines_own_answer_and_the_vault_is_still_stored_and_the_machine_still_paused(
) {
    sync_refused(|| Ok(exec_exit(1, "", "sync: Input/output error")), "it exited 1 and said: sync: Input/output error");
}

fn sync_refused(
    answer: impl Fn() -> Result<wsp_engine::machine::ExecResult, wsp_engine::errors::EngineError> + 'static,
    said_by_the_nap: &'static str,
) {
    drive(move || async move {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let plain = backend.exec_impl_fn();
        backend.set_exec_impl(Rc::new(move |m, cmd, stdin| {
            if cmd == wsp_engine::disk_sync::DISK_SYNC_CMD {
                let answered = answer();
                return Box::pin(async move { answered });
            }
            plain(m, cmd, stdin)
        }));
        let warned = capture();
        let fx = runtime(&backend).store(&state.store).build();
        let ws = create_on(&fx.rt, CreateOn::golden("x"), None).await.expect("a workspace");
        fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        assert_eq!(warned.lines(), [format!("disk sync before the nap of {} failed: {said_by_the_nap}; napping anyway", ws.view.id)]);
        assert!(state.store.get_blob("vaults", &ws.view.id).await.expect("a read").is_some());
        assert!(backend.machine(0).paused.get());
        assert_eq!(j(&fx.rt.workspaces().get(&ws.view.id, None, None).await.expect("the workspace"))["phase"], "napping");
    });
}

// "a healthy wake stays on the same machine even though createdAt moved, imports nothing, and reports the daemon reachable"
#[test]
fn a_healthy_wake_stays_on_the_same_machine_even_though_created_at_moved_imports_nothing_and_reports_the_daemon_reachable() {
    drive(|| async {
        let g = guest_backend();
        let daemon = daemon_of(TOKEN, "m1").await;
        let fx = runtime(&g.backend).with(|o| o.daemon_token = Some(TOKEN.into())).build();
        let events = fx.events();
        let ws = create_on(&fx.rt, CreateOn::golden("x"), None).await.expect("a workspace");
        let m1 = g.backend.machine(0);
        m1.set_preview_url(&format!("ws://127.0.0.1:{}", daemon.port));
        fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        let mut shape = m1.shape_now();
        shape.created_at = Some("2026-09-02T20:11:04.444Z".into()); // every Solari resume does this
        m1.set_shape(shape);
        let woken = fx.rt.workspaces().wake(&ws.view.id, None).await.expect("a wake");
        assert_matches(&j(&woken), json!({ "machineId": "m1", "phase": "running" }));
        assert_eq!(m1.resumes.get(), 1);
        assert!(!m1.killed.get());
        assert_eq!(g.backend.made(), 1);
        assert!(g.untars.borrow().is_empty());
        assert_matches(&events.of("workspace.woken")[0], json!({ "machineId": "m1" }));
        let last = last_status(&events);
        assert_eq!(last["reach"]["state"], "reachable");
        assert!(last.get("reason").is_none());
    });
}

// "every nap replaces the stashed vault; one over the cap is refused with a warning, the previous stays, the record keeps saying it and no status line carries the reason"
#[test]
fn every_nap_replaces_the_stashed_vault_one_over_the_cap_is_refused_with_a_warning_the_previous_stays_the_record_keeps_saying_it_and_no_status_line_carries_the_reason(
) {
    drive(|| async {
        let g = guest_backend();
        let warned = capture();
        let state = sqlite_state();
        // The stamp on the record is the runtime's clock, so this test holds one: two naps a millisecond apart under a
        // busy box would otherwise write the same ISO string and the refresh could not be told from a stale write.
        let fc = FakeClock::new(1_788_851_404_444);
        let fx = runtime(&g.backend)
            .store(&state.store)
            .clock(&fc)
            .with(|o| {
                o.wake = Some(WakeOptions { vault_cap_bytes: Some(5_000), ..Default::default() });
                o.vault_caches = Some(CacheRule {
                    dirs: vec!["node_modules".into(), "dist".into()],
                    files: vec![".DS_Store".into()],
                    markers: vec![".git".into()],
                });
            })
            .build();
        let events = fx.events();
        let ws = create_on(&fx.rt, CreateOn::golden("x"), None).await.expect("a workspace");
        let id = ws.view.id.clone();
        // This workspace's own last status and its own warnings.
        let nap_reason = |events: &Events| -> Option<String> {
            events.statuses().iter().rev().find(|s| s["id"] == id.as_str()).and_then(|s| s["reason"].as_str().map(str::to_string))
        };
        let vault_warnings = |warned: &wsp_core::console::Captured| -> Vec<String> {
            warned.lines().into_iter().filter(|l| l.starts_with(&format!("nap vault for {id}"))).collect()
        };
        fx.rt.workspaces().nap(&id, None).await.expect("a nap");
        assert_eq!(state.store.get_blob("vaults", &id).await.expect("a read"), Some(b"tarbytes".to_vec()));
        assert_eq!(nap_reason(&events), None);
        let stored = "2026-09-08T07:10:04.444Z";
        let held = j(&fx.rt.workspaces().get(&id, None, None).await.expect("the workspace"));
        assert_eq!(held["vaultedAt"], stored);
        assert!(held.get("vaultRefused").is_none());
        let script = g.backend.machine(0).run_log().into_iter().find(|s| s.contains("tar czf")).expect("the vault script");
        assert!(script.contains(
            "find 'root/notes.md' -mindepth 1 -path '*/.git' -prune -o \\( \\( -type d \\( -name 'node_modules' -o -name 'dist' \\) \\) -o \\( -type f \\( -name '.DS_Store' \\) \\) -o \\( -type d -exec test -f '{}/.git' \\; \\) \\) -prune -print > "
        ), "{script}");
        assert!(in_order(&script, &["tar czf '", "' --exclude='", "' --no-recursion --null -T '", ".keep'"]), "{script}");
        fx.rt.workspaces().wake(&id, None).await.expect("a wake");
        g.tgz_bytes.set(6_000);
        fx.rt.workspaces().nap(&id, None).await.expect("a nap");
        assert_eq!(*g.tars.borrow(), ["m1", "m1"]);
        assert_eq!(state.store.get_blob("vaults", &id).await.expect("a read"), Some(b"tarbytes".to_vec()));
        assert_eq!(
            vault_warnings(&warned),
            [format!("nap vault for {id} not stored, previous kept: the export was 6 KB, over the 5 KB cap")]
        );
        assert_eq!(j(&fx.rt.workspaces().get(&id, None, None).await.expect("the workspace"))["phase"], "napping");
        // No status line for it at all: the record says it until a nap stores one, and the pane's own backup line is
        // where a person reads the verdict. A status reason would have taken the row's third line from the spend.
        assert_eq!(nap_reason(&events), None);
        // The record says it until a nap stores one, which is what the row and the tab read.
        let refused = j(&fx.rt.workspaces().get(&id, None, None).await.expect("the workspace"));
        assert_eq!(refused["vaultRefused"], "the export was 6 KB, over the 5 KB cap");
        assert_eq!(refused["vaultedAt"], stored);
        assert_matches(
            &state.store.get("workspaces", &id).await.expect("a read").expect("a record"),
            json!({ "vaultRefused": "the export was 6 KB, over the 5 KB cap", "vaultedAt": stored }),
        );
        fx.rt.workspaces().wake(&id, None).await.expect("a wake");
        g.tgz_bytes.set(1_000);
        // Well inside the status poll's interval, so the only thing that moves is the stamp the next stash writes.
        fc.advance(1_000);
        fx.rt.workspaces().nap(&id, None).await.expect("a nap");
        assert_eq!(nap_reason(&events), None);
        let fresh = j(&fx.rt.workspaces().get(&id, None, None).await.expect("the workspace"));
        assert!(fresh.get("vaultRefused").is_none());
        assert_eq!(fresh["vaultedAt"], "2026-09-08T07:10:05.444Z");
        fx.rt.workspaces().delete(&id, None).await.expect("a delete");
        assert_eq!(state.store.get_blob("vaults", &id).await.expect("a read"), None);
    });
}

// "rebuild forks the golden, imports the nap-time vault, kills the old machine, keeps the id and name, and pushes status"
#[test]
fn rebuild_forks_the_golden_imports_the_nap_time_vault_kills_the_old_machine_keeps_the_id_and_name_and_pushes_status() {
    drive(|| async {
        let g = guest_backend();
        let state = sqlite_state();
        let fx = runtime(&g.backend).store(&state.store).build();
        let events = fx.events();
        let ws = create_on(&fx.rt, CreateOn::golden("hello").mem(2048.0), None).await.expect("a workspace");
        let id = ws.view.id.clone();
        let m1 = g.backend.machine(0);
        fx.rt.workspaces().nap(&id, None).await.expect("a nap");
        assert_eq!(state.store.get_blob("vaults", &id).await.expect("a read"), Some(b"tarbytes".to_vec()));
        // The provider resumed it behind our back and handed back a running machine whose guest is dead.
        m1.paused.set(false);
        assert_eq!(state.store.get("workspaces", &id).await.expect("a read").expect("a record")["phase"], "napping");

        let rebuilt = fx.rt.workspaces().rebuild(&id, None).await.expect("a rebuild");
        assert_matches(&j(&rebuilt), json!({ "id": id, "name": "hello", "machineId": "m2", "phase": "running", "golden": "snap_g" }));
        assert!(m1.killed.get());
        assert_eq!(m1.resumes.get(), 0);
        assert_matches(&j(&g.backend.machine(1).spec_now()), json!({ "fromSnapshot": "snap_g", "memMb": 2048 }));
        assert_eq!(*g.untars.borrow(), ["m2"]);
        assert_matches(&events.of("workspace.upgraded")[0], json!({ "workspaceId": id, "machineId": "m2" }));
        let last = last_status(&events);
        assert_matches(&last, json!({ "phase": "running", "machineId": "m2" }));
        assert!(in_order(last["reason"].as_str().expect("a reason"), &["rebuilt", "m1", "m2", "vault"]), "{last}");
        assert_matches(
            &state.store.get("workspaces", &id).await.expect("a read").expect("a record"),
            json!({ "machineId": "m2", "phase": "running", "firstLife": true }),
        );
        assert_ne!(status_rows(&fx.rt).await[0]["reach"]["state"], "zombie");
    });
}

// "rebuild of a workspace that never napped imports nothing and says so"
#[test]
fn rebuild_of_a_workspace_that_never_napped_imports_nothing_and_says_so() {
    drive(|| async {
        let g = guest_backend();
        let fx = runtime(&g.backend).build();
        let events = fx.events();
        let ws = create_on(&fx.rt, CreateOn::golden("fresh"), None).await.expect("a workspace");
        let rebuilt = fx.rt.workspaces().rebuild(&ws.view.id, None).await.expect("a rebuild");
        assert_eq!(rebuilt.machine_id, "m2");
        assert!(g.backend.machine(0).killed.get());
        assert!(g.untars.borrow().is_empty());
        assert!(in_order(last_status(&events)["reason"].as_str().expect("a reason"), &["rebuilt", "m1", "m2", "no vault"]));
    });
}

/// A provider that clamps memory: whatever is asked, the machine it builds has 2048 MB.
fn clamping_backend() -> Arc<StubBackend> {
    let backend = StubBackend::new(None);
    let create = backend.create_fn();
    backend.set_create(Rc::new(move |spec, waiting, sent| {
        let made = create(spec, waiting, sent);
        Box::pin(async move {
            let m = made.await?;
            m.set_memory(2048.0);
            Ok(m)
        })
    }));
    backend
}

// "runtime workspace size" > "create asks for an explicit size: the pricing default when the caller names none"
#[test]
fn create_asks_for_an_explicit_size_the_pricing_default_when_the_caller_names_none() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).build();
        create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        create_on(&fx.rt, CreateOn::golden("b").mem(2048.0), None).await.expect("a workspace");
        assert_matches(&j(&backend.machine(0).spec_now()), json!({ "cpu": 2, "memMb": 4096 }));
        assert_matches(&j(&backend.machine(1).spec_now()), json!({ "cpu": 2, "memMb": 2048 }));
    });
}

// "the size shown and billed is what the provider built, not what was asked, and it survives a restart"
#[test]
fn the_size_shown_and_billed_is_what_the_provider_built_not_what_was_asked_and_it_survives_a_restart() {
    drive(|| async {
        let backend = clamping_backend();
        let state = sqlite_state();
        let fx = runtime(&backend)
            .store(&state.store)
            .with(|o| {
                o.status = Some(StatusWatchOptions { cost_interval_ms: Some(15), poll_interval_ms: Some(60_000), ..Default::default() })
            })
            .build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        assert_eq!(j(&backend.machine(0).spec_now())["memMb"], 4096);
        let built = json!({ "cpu": 2, "memMb": 2048 });
        let rate = 2.0 * 0.035 + 2048.0 / 1024.0 * 0.01;
        let status = status_rows(&fx.rt).await.remove(0);
        assert_matches(&status, json!({ "size": built, "rateUsdPerHour": rate }));

        let costs: Rc<std::cell::RefCell<Vec<f64>>> = Rc::default();
        let seen = costs.clone();
        let _listen = {
            fx.rt.events().on(
                "workspace.cost",
                Rc::new(move |e| {
                    if let Some(rate) = serde_json::to_value(e).ok().and_then(|v| v["rateUsdPerHour"].as_f64()) {
                        seen.borrow_mut().push(rate);
                    }
                    Ok(())
                }),
            )
        };
        let stop = fx.rt.status().watch(StatusWatchOptions::default());
        while costs.borrow().is_empty() {
            sleep_ms(5).await;
        }
        stop();
        assert!((costs.borrow()[0] - rate).abs() < 1e-10);

        let again = runtime(&backend).store(&state.store).build();
        assert_matches(&status_rows(&again.rt).await[0], json!({ "id": ws.view.id, "size": built }));
    });
}

// "a rebuilt fork asks for the recorded size and records what came back; an upgrade records its new size"
#[test]
fn a_rebuilt_fork_asks_for_the_recorded_size_and_records_what_came_back_an_upgrade_records_its_new_size() {
    drive(|| async {
        let backend = clamping_backend();
        backend.set_exec_impl(exec_fn(|_, cmd| Ok(exec_ok(if cmd.contains("ls -A /root") { "notes.md\n" } else { "" }))));
        backend.set_downloads(Rc::new(|_| b"tarbytes".to_vec()));
        let fx = runtime(&backend).build();
        let statuses = {
            let seen: Rc<std::cell::RefCell<Vec<Value>>> = Rc::default();
            let kept = seen.clone();
            let _listen = fx.rt.events().on(
                "workspace.status",
                Rc::new(move |e| {
                    kept.borrow_mut().push(serde_json::to_value(e).expect("an event"));
                    Ok(())
                }),
            );
            seen
        };
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        fx.rt.workspaces().rebuild(&ws.view.id, None).await.expect("a rebuild");
        assert_matches(&j(&backend.machine(1).spec_now()), json!({ "cpu": 2, "memMb": 2048 }));
        let pushed = statuses.borrow().last().cloned().expect("a pushed status");
        assert_eq!(pushed["status"]["size"], json!({ "cpu": 2, "memMb": 2048 }));

        backend.machine(1).set_preview(None);
        fx.rt.workspaces().upgrade(&ws.view.id, None).await.expect("an upgrade");
        assert_matches(&j(&backend.machine(2).spec_now()), json!({ "cpu": 2, "memMb": 2048 }));
        assert_eq!(status_rows(&fx.rt).await[0]["size"], json!({ "cpu": 2, "memMb": 2048 }));
    });
}

// "a fork inherits the size its golden was sealed at unless the caller overrides it"
#[test]
fn a_fork_inherits_the_size_its_golden_was_sealed_at_unless_the_caller_overrides_it() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let version = json!({ "version": 1, "snapshotId": "snap_golden-v1", "baseTemplate": "base", "setupSha": "s", "createdAt": "2026-09-01T00:00:00.000Z", "smoke": { "cmd": "true", "exitCode": 0 }, "size": { "cpu": 2, "memMb": 8192 } });
        state.store.put("goldens", &copy_key("default", "big"), json!({ "head": 1, "versions": [version] })).await.expect("a manifest");
        let fx = runtime(&backend).store(&state.store).build();
        create_on(&fx.rt, CreateOn::named("a").golden_is("snap_golden-v1"), None).await.expect("a workspace");
        create_on(&fx.rt, CreateOn::named("b").golden_is("snap_golden-v1").mem(2048.0), None).await.expect("a workspace");
        create_on(&fx.rt, CreateOn::named("c").golden_is("snap_elsewhere"), None).await.expect("a workspace");
        assert_matches(&j(&backend.machine(0).spec_now()), json!({ "cpu": 2, "memMb": 8192 }));
        assert_matches(&j(&backend.machine(1).spec_now()), json!({ "cpu": 2, "memMb": 2048 }));
        assert_matches(&j(&backend.machine(2).spec_now()), json!({ "cpu": 2, "memMb": 4096 }));
        let mut sizes: Vec<i64> = status_rows(&fx.rt).await.iter().map(|s| s["size"]["memMb"].as_i64().expect("a size")).collect();
        sizes.sort();
        assert_eq!(sizes, [2048, 4096, 8192]);
    });
}

// "seal records the builder's size on the version, as the provider reported it"
#[test]
fn seal_records_the_builders_size_on_the_version_as_the_provider_reported_it() {
    drive(|| async {
        let backend = clamping_backend();
        let fx = runtime(&backend)
            .with(|o| {
                o.golden_recipe = Some(GoldenRecipe {
                    setup: "install".into(),
                    smoke: "true".into(),
                    cpu: Some(JsNumber(2.0)),
                    mem_mb: Some(JsNumber(4096.0)),
                    ..Default::default()
                })
            })
            .build();
        let b = fx.rt.golden().prepare(PrepareAsk::default()).await.expect("a builder");
        assert_matches(&j(&backend.machine(0).spec_now()), json!({ "cpu": 2, "memMb": 4096 }));
        let sealed = fx.rt.golden().seal(&b.id, None, None).await.expect("a seal");
        assert_eq!(j(&sealed.version)["size"], json!({ "cpu": 2, "memMb": 2048 }));
        assert_matches(&j(&backend.machine(1).spec_now()), json!({ "cpu": 2, "memMb": 4096 }));
    });
}

/// A provider whose forks boot as desktop machines: every machine it builds streams a display.
fn desktop_backend() -> Arc<StubBackend> {
    let backend = StubBackend::new(None);
    let create = backend.create_fn();
    backend.set_create(Rc::new(move |mut spec, waiting, sent| {
        spec.kind = wsp_engine::machine::MachineKind::Desktop;
        create(spec, waiting, sent)
    }));
    backend
}

// "runtime workspace screen" > "a desktop machine's stream rides the view and the status; a sandbox carries no screen"
#[test]
fn a_desktop_machines_stream_rides_the_view_and_the_status_a_sandbox_carries_no_screen() {
    drive(|| async {
        let desktop = desktop_backend();
        let fx = runtime(&desktop).build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        let screen = json!({ "streamUrl": "wss://stub/stream/m1" });
        assert_eq!(j(&ws.view)["screen"], screen);
        assert_eq!(j(&fx.rt.workspaces().get(&ws.view.id, None, None).await.expect("the workspace"))["screen"], screen);
        assert_eq!(status_rows(&fx.rt).await[0]["screen"], screen);

        let headless = runtime(&StubBackend::new(None)).build();
        let sandbox = create_on(&headless.rt, CreateOn::golden("b"), None).await.expect("a workspace");
        assert!(j(&sandbox.view).get("screen").is_none());
        assert!(status_rows(&headless.rt).await[0].get("screen").is_none());
    });
}

// "the stream survives a restart and a wake on the same machine; a rebuild refreshes it from the new machine"
#[test]
fn the_stream_survives_a_restart_and_a_wake_on_the_same_machine_a_rebuild_refreshes_it_from_the_new_machine() {
    drive(|| async {
        let backend = desktop_backend();
        let state = sqlite_state();
        let rt1 = runtime(&backend).store(&state.store).build();
        let ws = create_on(&rt1.rt, CreateOn::golden("a"), None).await.expect("a workspace");
        rt1.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");

        let rt2 = runtime(&backend).store(&state.store).build();
        let m1 = json!({ "streamUrl": "wss://stub/stream/m1" });
        assert_eq!(j(&rt2.rt.workspaces().get(&ws.view.id, None, None).await.expect("the workspace"))["screen"], m1);
        let woken = rt2.rt.workspaces().wake(&ws.view.id, None).await.expect("a wake");
        assert_eq!(woken.machine_id, "m1");
        assert_eq!(j(&woken)["screen"], m1);

        let pushed = {
            let seen: Rc<std::cell::RefCell<Vec<Value>>> = Rc::default();
            let kept = seen.clone();
            let _listen = rt2.rt.events().on(
                "workspace.status",
                Rc::new(move |e| {
                    kept.borrow_mut().push(serde_json::to_value(e).expect("an event"));
                    Ok(())
                }),
            );
            let rebuilt = rt2.rt.workspaces().rebuild(&ws.view.id, None).await.expect("a rebuild");
            assert_eq!(rebuilt.machine_id, "m2");
            assert_eq!(j(&rebuilt)["screen"], json!({ "streamUrl": "wss://stub/stream/m2" }));
            seen
        };
        let last = pushed.borrow().last().cloned().expect("a pushed status");
        assert_eq!(last["status"]["screen"], json!({ "streamUrl": "wss://stub/stream/m2" }));
        assert_eq!(
            state.store.get("workspaces", &ws.view.id).await.expect("a read").expect("a record")["screen"],
            json!({ "streamUrl": "wss://stub/stream/m2" })
        );
    });
}

fn golden_version(kind: Option<&str>) -> Value {
    let mut version = json!({ "version": 1, "snapshotId": "snap_golden-v1", "baseTemplate": "base", "setupSha": "sha1", "createdAt": "2026-08-11T00:00:00.000Z", "smoke": { "cmd": "true", "exitCode": 0 } });
    if let Some(kind) = kind {
        // `kind` sits after `baseTemplate` in the TS literal.
        let mut keyed = serde_json::Map::new();
        for (k, v) in version.as_object().expect("an object").clone() {
            keyed.insert(k.clone(), v);
            if k == "baseTemplate" {
                keyed.insert("kind".into(), json!(kind));
            }
        }
        version = Value::Object(keyed);
    }
    version
}

// "runtime fork kind" > "forks a desktop golden as kind desktop on create, rebuild and upgrade, carrying the stream on the view"
#[test]
fn forks_a_desktop_golden_as_kind_desktop_on_create_rebuild_and_upgrade_carrying_the_stream_on_the_view() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        state
            .store
            .put("goldens", &copy_key("default", "default"), json!({ "head": 1, "versions": [golden_version(Some("desktop"))] }))
            .await
            .expect("a manifest");
        let fx = runtime(&backend).store(&state.store).build();

        let ws = create_on(&fx.rt, CreateOn::named("a").golden_is("snap_golden-v1"), None).await.expect("a workspace");
        assert_eq!(j(&backend.machine(0).spec_now())["kind"], "desktop");
        assert_eq!(j(&ws.view)["screen"], json!({ "streamUrl": "wss://stub/stream/m1" }));

        let rebuilt = fx.rt.workspaces().rebuild(&ws.view.id, None).await.expect("a rebuild");
        assert_eq!(j(&backend.machine(1).spec_now())["kind"], "desktop");
        assert_eq!(j(&rebuilt)["screen"], json!({ "streamUrl": "wss://stub/stream/m2" }));

        let upgraded = fx.rt.workspaces().upgrade(&ws.view.id, None).await.expect("an upgrade");
        assert_eq!(j(&backend.machine(2).spec_now())["kind"], "desktop");
        assert_eq!(j(&upgraded)["screen"], json!({ "streamUrl": "wss://stub/stream/m3" }));
    });
}

// "a manifest sealed before versions recorded a kind still forks sandbox"
#[test]
fn a_manifest_sealed_before_versions_recorded_a_kind_still_forks_sandbox() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        state
            .store
            .put("goldens", &copy_key("default", "default"), json!({ "head": 1, "versions": [golden_version(None)] }))
            .await
            .expect("a manifest");
        let fx = runtime(&backend).store(&state.store).build();
        let ws = create_on(&fx.rt, CreateOn::named("a").golden_is("snap_golden-v1"), None).await.expect("a workspace");
        assert_eq!(j(&backend.machine(0).spec_now())["kind"], "sandbox");
        assert!(j(&ws.view).get("screen").is_none());
    });
}

// "nap vault against the stub backend" > "stores a real tar from the stub download URL without warning"
#[test]
fn stores_a_real_tar_from_the_stub_download_url_without_warning() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let warned = capture();
        let fx = runtime(&backend).store(&state.store).build();
        let ws = create_on(&fx.rt, CreateOn::golden("x"), None).await.expect("a workspace");
        fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        assert!(warned.lines().is_empty(), "{:?}", warned.lines());
        let vault = state.store.get_blob("vaults", &ws.view.id).await.expect("a read").expect("a vault");
        let mut unpacked = Vec::new();
        std::io::Read::read_to_end(&mut flate2::read::GzDecoder::new(&vault[..]), &mut unpacked).expect("a gzip archive");
        assert_eq!(unpacked, vec![0u8; 1024]);
    });
}

// "warns once, keeps the previous vault and leaves the napping status clean when the download URL cannot be fetched"
#[test]
fn warns_once_keeps_the_previous_vault_and_leaves_the_napping_status_clean_when_the_download_url_cannot_be_fetched() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let warned = capture();
        let fx = runtime(&backend).store(&state.store).build();
        let events = fx.events();
        let ws = create_on(&fx.rt, CreateOn::golden("x"), None).await.expect("a workspace");
        fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        let first = state.store.get_blob("vaults", &ws.view.id).await.expect("a read");
        fx.rt.workspaces().wake(&ws.view.id, None).await.expect("a wake");

        backend.machine(0).set_download_url(Rc::new(|_| Box::pin(async { Ok("http://127.0.0.1:1/nothing-listens-here".to_string()) })));
        fx.rt.workspaces().nap(&ws.view.id, None).await.expect("a nap");
        assert_eq!(warned.lines(), [format!("nap vault for {} not stored, previous kept: fetch failed", ws.view.id)]);
        assert_eq!(state.store.get_blob("vaults", &ws.view.id).await.expect("a read"), first);
        // What the fetch answered stays on the record and off every status: a sentence here would have taken the
        // row's third line from the spend for a note about a step already taken.
        let last = last_status(&events);
        assert_eq!(last["phase"], "napping");
        assert!(last.get("reason").is_none());
        assert_eq!(j(&fx.rt.workspaces().get(&ws.view.id, None, None).await.expect("the workspace"))["vaultRefused"], "fetch failed");
    });
}
