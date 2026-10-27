// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/runtime-comeback.test.ts at 65f6b89c7, case for case, the first half ("a turn the host comes back
//! to", up to the export held behind a sweep). The second half is whole_runtime_comeback_2.rs. A run lives on the machine,
//! not in this process: a restart re-opens it by its handle and reads its log from byte zero.
#[macro_use]
mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use common::comeback::*;
use common::*;
use serde_json::{json, Value};
use wsp_core::clock::FakeClock;
use wsp_core::console::capture;
use wsp_core::types::api::StartOptions;
use wsp_core::types::internal::TRANSCRIPTS_HELD;
use wsp_core::types::wiring::GoldenRecipe;
use wsp_wire::wire::daemon_version::DAEMON_VERSION;

const TOKEN: &str = "deadbeefdeadbeefdeadbeef";

fn question() -> Value {
    json!({
        "askId": "ask_q",
        "toolName": "AskUserQuestion",
        "input": "{}",
        "options": [{ "id": "allow", "label": "Red", "effect": "allow" }],
    })
}

// "a turn the host comes back to" > "a turn re-opened after a restart still gives back the minutes it stood on a question before the host went"
#[test]
fn a_turn_re_opened_after_a_restart_still_gives_back_the_minutes_it_stood_on_a_question_before_the_host_went() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::new();
        let wait = FakeClock::at_now();
        let rt = runtime(&backend).store(&state.store).adapter("claude", h.factory()).clock(&wait).build();
        let ws = create_on(&rt.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        rt.rt.sessions().start(&ws, opts("merge both branches"), None).await.expect("a start");
        let (r, w) = (rt.rt.clone(), ws.clone());
        wait!([r, w] has_event(&r, &w, "session.start").await);
        let run = h.handles()[0].clone();
        h.emit(&run, json!({ "type": "permission.ask", "sessionId": "sess-1", "ask": question() }));
        let (r, w) = (rt.rt.clone(), ws.clone());
        wait!([r, w] !rows(&r, &w).await[0]["asking"].is_null());
        wait.advance(10 * 60_000);
        rt.rt.close().await.expect("a close");

        let rt2 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).clock(&wait).build();
        let (r, w) = (rt2.rt.clone(), ws.clone());
        wait!([r, w] !rows(&r, &w).await[0]["asking"].is_null());
        wait.advance(60_000);
        h.emit(
            &run,
            json!({ "type": "permission.close", "sessionId": "sess-1", "askId": "ask_q", "outcome": "allowed", "optionId": "allow" }),
        );
        wait.advance(60_000);
        h.emit(
            &run,
            json!({ "type": "turn.done", "sessionId": "sess-1", "result": { "status": "completed", "text": "done", "durationMs": 12 * 60_000 } }),
        );
        h.emit(&run, json!({ "type": "session.end", "sessionId": "sess-1", "exitCode": 0, "sawResult": true }));
        let (r, w) = (rt2.rt.clone(), ws.clone());
        wait!([r, w] rows(&r, &w).await[0]["status"] == "completed");
        let done = history(&rt2.rt, &ws).await.into_iter().find(|e| e["type"] == "session.done").expect("a done row");
        assert_matches(&done["result"], json!({ "durationMs": 12 * 60_000, "waitedMs": 11 * 60_000 }));
        rt2.rt.close().await.expect("a close");
    });
}

// "a turn the host comes back to" > "a run re-read after a restart files each call by its age against the run's newest stamp, so a call made long before stays out"
#[test]
fn a_run_re_read_after_a_restart_files_each_call_by_its_age_against_the_runs_newest_stamp_so_a_call_made_long_before_stays_out() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::new();
        let down = host_went_down(&h, &state.store, &backend).await;
        // The machine's clock means nothing to this host: only the twenty minutes between the run's two calls do.
        let stamp = 1_000_000;
        let rt = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        assert_eq!(statuses(&rt.rt, &down.workspace_id).await, ["running"]);
        h.emit(
            &down.run,
            json!({ "type": "limit", "sessionId": "sess-1", "limit": { "windows": [{ "kind": "session", "usedPercent": 10 }] } }),
        );
        h.emit(&down.run, json!({ "type": "turn.usage", "sessionId": "sess-1", "tokens": 30_000, "at": stamp }));
        h.emit(&down.run, json!({ "type": "turn.usage", "sessionId": "sess-1", "tokens": 1_500, "at": stamp + 20 * 60_000 }));
        rt.rt.close().await.expect("a close");

        let rt2 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        assert_eq!(statuses(&rt2.rt, &down.workspace_id).await, ["running"]);
        let r = rt2.rt.clone();
        wait!([r] j(&r.usage().accounts(None).await.expect("accounts"))["accounts"].as_array().is_some_and(|a| a.iter().any(|x| !x["burn"].is_null())));
        let accounts = j(&rt2.rt.usage().accounts(None).await.expect("accounts"));
        let burns: Vec<Value> =
            accounts["accounts"].as_array().expect("accounts").iter().filter(|a| !a["burn"].is_null()).map(|a| a["burn"].clone()).collect();
        assert_eq!(burns, [json!({ "tokensPerMinute": 100, "threads": 1 })]);
        rt2.rt.close().await.expect("a close");
    });
}

// "a turn the host comes back to" > "a thread's later turn re-opened after a restart is handed its own message, not the one that opened the thread"
#[test]
fn a_threads_later_turn_re_opened_after_a_restart_is_handed_its_own_message_not_the_one_that_opened_the_thread() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::named_at_launch();
        let rt = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        let ws = create_on(&rt.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let first = rt
            .rt
            .sessions()
            .start(&ws, StartOptions { prompt: "build it".into(), effort: Some("low".into()), ..Default::default() }, None)
            .await
            .expect("a start");
        let (r, w) = (rt.rt.clone(), ws.clone());
        wait!([r, w] has_event(&r, &w, "session.start").await);
        finish_run(&h, &h.handles()[0], "sess-1", "built");
        first.finished().await;
        let thread = j(&first.view())["threadId"].as_str().expect("a thread").to_string();
        rt.rt
            .sessions()
            .start(
                &ws,
                StartOptions { prompt: "now test it".into(), thread: Some(thread), effort: Some("high".into()), ..Default::default() },
                None,
            )
            .await
            .expect("a start");
        let hh = h.clone();
        wait!([hh] hh.handles().len() == 2);
        rt.rt.close().await.expect("a close");

        let rt2 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        let pairs: Vec<(String, String)> = rows(&rt2.rt, &ws)
            .await
            .iter()
            .map(|s| (s["status"].as_str().unwrap_or_default().into(), s["prompt"].as_str().unwrap_or_default().into()))
            .collect();
        assert_eq!(pairs, [("running".to_string(), "build it".to_string())]);
        assert_eq!(h.reopened(), [Reopened { prompt: Some("now test it".into()), effort: Some("high".into()) }]);
        rt2.rt.close().await.expect("a close");
    });
}

// "a turn the host comes back to" > "a run re-opened on a machine the restart found running off a provider read alone proves the machine takes commands: the next wake proves nothing"
#[test]
fn a_run_re_opened_on_a_machine_the_restart_found_running_off_a_provider_read_alone_proves_the_machine_takes_commands_the_next_wake_proves_nothing(
) {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::new();
        let down = host_went_down(&h, &state.store, &backend).await;
        // The host went down while a wake waited on this machine, which the provider runs.
        let mut record = state.store.get("workspaces", &down.workspace_id).await.expect("a read").expect("a record");
        record["phase"] = json!("waking");
        state.store.put("workspaces", &down.workspace_id, record).await.expect("a write");
        let proofs = Rc::new(Cell::new(0));
        let counted = proofs.clone();
        backend.machine(0).set_prove_road(Some(Rc::new(move || {
            counted.set(counted.get() + 1);
            Box::pin(async { Ok(()) })
        })));
        let rt2 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        assert_eq!(statuses(&rt2.rt, &down.workspace_id).await, ["running"]);
        assert_eq!(h.reopened().len(), 1);
        let woke = rt2.rt.workspaces().wake(&down.workspace_id, None).await.expect("a wake");
        assert_eq!(j(&woke)["phase"], "running");
        assert_eq!(proofs.get(), 0);
        rt2.rt.close().await.expect("a close");
    });
}

// "a turn the host comes back to" > "the run outlives the host: the row keeps its run, stays running across the restart and completes with its reply"
#[test]
fn the_run_outlives_the_host_the_row_keeps_its_run_stays_running_across_the_restart_and_completes_with_its_reply() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::new();
        let down = host_went_down(&h, &state.store, &backend).await;
        let stored = state.store.get("sessions", &down.workspace_id).await.expect("a read").expect("a record");
        let pairs: Vec<Value> = stored["sessions"].as_array().expect("sessions").iter().map(|s| json!([s["status"], s["run"]])).collect();
        assert_eq!(pairs, [json!(["running", down.run])]);

        let rt2 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        assert_eq!(statuses(&rt2.rt, &down.workspace_id).await, ["running"]);
        // A harness that hands the message over after its process starts may not have handed it yet when the host went.
        assert_eq!(h.reopened().iter().map(|r| r.prompt.clone()).collect::<Vec<_>>(), [Some("build it".to_string())]);
        h.emit(&down.run, json!({ "type": "turn.delta", "sessionId": "sess-1", "kind": "text", "text": "wrote the fix" }));
        h.emit(&down.run, json!({ "type": "turn.done", "sessionId": "sess-1", "result": { "status": "completed", "text": "done" } }));
        h.emit(&down.run, json!({ "type": "session.end", "sessionId": "sess-1", "exitCode": 0, "sawResult": true }));
        let (r, w) = (rt2.rt.clone(), down.workspace_id.clone());
        wait!([r, w] rows(&r, &w).await[0]["status"] == "completed");
        let history = history(&rt2.rt, &down.workspace_id).await;
        // the line the old host already wrote is read past, so the replay costs the transcript nothing
        assert_eq!(
            history.iter().map(|e| e["type"].as_str().unwrap_or_default()).collect::<Vec<_>>(),
            ["session.start", "session.delta", "session.delta", "session.done", "session.end"]
        );
        assert_eq!(
            history.iter().filter(|e| e["type"] == "session.delta").map(|e| e["text"].as_str().unwrap_or_default()).collect::<Vec<_>>(),
            ["reading the ticket", "wrote the fix"]
        );
        assert_matches(history.last().expect("an event"), json!({ "type": "session.end", "exitCode": 0, "sawResult": true }));
        // The name goes out under the one start row the turn writes, so the host that re-opened the run asks nothing:
        // its own asked-set is empty and the row is still on its seed, and the start row is what stands in for both.
        assert_eq!(h.asked(), ["build it"]);
        assert!(rows(&rt2.rt, &down.workspace_id).await[0].get("harnessTitle").is_none());
        rt2.rt.close().await.expect("a close");
    });
}

// "a turn the host comes back to" > "more running turns than the host holds transcripts, re-opened together, each read past what it already wrote"
#[test]
fn more_running_turns_than_the_host_holds_transcripts_re_opened_together_each_read_past_what_it_already_wrote() {
    drive(|| async {
        // Every turn is re-opened at once, and each open lets the oldest held transcript go: a turn that looked its
        // transcript up again found nothing and wrote its start and its lines a second time.
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::new();
        let rt = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        let mut spaces = vec![];
        for n in 0..TRANSCRIPTS_HELD + 2 {
            let ws = create_on(&rt.rt, CreateOn::golden(&format!("w{n}")), None).await.expect("a workspace").view.id;
            rt.rt.sessions().start(&ws, opts("build it"), None).await.expect("a start");
            let (r, w) = (rt.rt.clone(), ws.clone());
            wait!([r, w] has_event(&r, &w, "session.start").await);
            spaces.push(ws);
        }
        let runs = h.handles();
        for (n, run) in runs.iter().enumerate() {
            h.emit(run, json!({ "type": "turn.delta", "sessionId": format!("sess-{}", n + 1), "kind": "text", "text": format!("reading ticket {n}") }));
        }
        rt.rt.close().await.expect("a close");

        let rt2 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        rt2.rt.workspaces().list(None).await.expect("a list");
        for (n, run) in runs.iter().enumerate() {
            finish_run(&h, run, &format!("sess-{}", n + 1), "done");
        }
        for ws in &spaces {
            let (r, w) = (rt2.rt.clone(), ws.clone());
            wait!([r, w] rows(&r, &w).await[0]["status"] == "completed");
        }
        for (n, ws) in spaces.iter().enumerate() {
            let history = history(&rt2.rt, ws).await;
            assert_eq!(
                history.iter().map(|e| e["type"].as_str().unwrap_or_default()).collect::<Vec<_>>(),
                ["session.start", "session.delta", "session.done", "session.end"]
            );
            assert_eq!(
                history
                    .iter()
                    .filter(|e| e["type"] == "session.delta")
                    .map(|e| e["text"].as_str().unwrap_or_default().to_string())
                    .collect::<Vec<_>>(),
                [format!("reading ticket {n}")]
            );
        }
        rt2.rt.close().await.expect("a close");
    });
}

/// The old-daemon fixtures of the three update cases: a daemon one version behind, the recipe whose deploy is counted.
struct Update {
    daemon: HelloingDaemon,
    deployed: Rc<RefCell<Vec<String>>>,
}

fn deploying(deployed: &Rc<RefCell<Vec<String>>>, also: impl Fn() + 'static) -> GoldenRecipe {
    let seen = deployed.clone();
    GoldenRecipe {
        setup: "true".into(),
        smoke: "true".into(),
        deploy_daemon: Some(Rc::new(move |m, _| {
            seen.borrow_mut().push(m.id().to_string());
            also();
            Box::pin(async { Ok(None) })
        })),
        ..Default::default()
    }
}

fn waits_line(ws: &str) -> String {
    format!("daemon on m1 (workspace {ws}): update waits for the running turn")
}

// "a turn the host comes back to" > "a host that comes back to a turn still running on an old daemon says in its log that the update waits for the turn, and deploys once it ends"
#[test]
fn a_host_that_comes_back_to_a_turn_still_running_on_an_old_daemon_says_in_its_log_that_the_update_waits_for_the_turn_and_deploys_once_it_ends(
) {
    drive(|| async {
        let backend = StubBackend::new(None);
        backend.set_exec_impl(token_guest());
        let state = sqlite_state();
        let h = MachineRuns::new();
        let u = Update { daemon: helloing_daemon(DAEMON_VERSION - 1, false, None).await, deployed: Rc::default() };
        let warned = Rc::new(capture());
        let down = host_went_down(&h, &state.store, &backend).await;
        backend.machine(0).set_preview_url(&format!("ws://127.0.0.1:{}", u.daemon.port));
        let recipe = deploying(&u.deployed, || {});
        let rt2 = runtime(&backend)
            .store(&state.store)
            .adapter("claude", h.factory())
            .with(move |o| {
                o.daemon_token = Some(TOKEN.into());
                o.golden_recipe = Some(recipe);
                o.daemon_hello_timeout_ms = Some(2_000);
            })
            .build();
        assert_eq!(statuses(&rt2.rt, &down.workspace_id).await, ["running"]);
        let waits = waits_line(&down.workspace_id);
        let (w, l) = (warned.clone(), waits.clone());
        wait!([w, l] w.lines().contains(&l));
        sleep_ms(100).await;
        assert!(u.deployed.borrow().is_empty());

        finish_run(&h, &down.run, "sess-1", "done");
        let d = u.deployed.clone();
        wait!([d] d.borrow().len() == 1);
        assert_eq!(*u.deployed.borrow(), ["m1"]);
        assert_eq!(warned.lines().into_iter().filter(|l| l.starts_with("daemon on")).collect::<Vec<_>>(), [waits]);
        rt2.rt.close().await.expect("a close");
    });
}

// "a turn the host comes back to" > "waits out the running turn on a store whose sessions listing answers last, since the sync starts after the rows are in"
#[test]
fn waits_out_the_running_turn_on_a_store_whose_sessions_listing_answers_last_since_the_sync_starts_after_the_rows_are_in() {
    drive(|| async {
        let backend = StubBackend::new(None);
        backend.set_exec_impl(token_guest());
        let state = sqlite_state();
        let h = MachineRuns::new();
        let u = Update { daemon: helloing_daemon(DAEMON_VERSION - 1, false, None).await, deployed: Rc::default() };
        let warned = Rc::new(capture());
        let down = host_went_down(&h, &state.store, &backend).await;
        backend.machine(0).set_preview_url(&format!("ws://127.0.0.1:{}", u.daemon.port));
        // A deploy is what releases the held listing, so a host that starts the sync before the rows are in reaches
        // this read with its deploy already done rather than with a wait nobody can time.
        let rows_in = Gate::new();
        let release = rows_in.clone();
        let recipe = deploying(&u.deployed, move || release.open());
        let late = WrappedStore::over(&state.store);
        late.hold_list("sessions", &rows_in);
        let rt2 = runtime(&backend)
            .store(&late.store())
            .adapter("claude", h.factory())
            .with(move |o| {
                o.daemon_token = Some(TOKEN.into());
                o.golden_recipe = Some(recipe);
                o.daemon_hello_timeout_ms = Some(2_000);
            })
            .build();
        let (r, w) = (rt2.rt.clone(), down.workspace_id.clone());
        let listing = tokio::task::spawn_local(async move { statuses(&r, &w).await });
        let opened = rows_in.clone();
        let timer = tokio::task::spawn_local(async move {
            sleep_ms(300).await;
            opened.open();
        });
        assert_eq!(listing.await.expect("a listing"), ["running"]);
        timer.abort();
        // Nothing was deployed while the rows were still coming in, because no sync had started.
        assert!(u.deployed.borrow().is_empty());
        let waits = waits_line(&down.workspace_id);
        let (w, l) = (warned.clone(), waits.clone());
        wait!([w, l] w.lines().contains(&l));
        assert!(u.deployed.borrow().is_empty());

        finish_run(&h, &down.run, "sess-1", "done");
        let d = u.deployed.clone();
        wait!([d] d.borrow().len() == 1);
        assert_eq!(*u.deployed.borrow(), ["m1"]);
        assert_eq!(warned.lines().into_iter().filter(|l| l.starts_with("daemon on")).collect::<Vec<_>>(), [waits]);
        rt2.rt.close().await.expect("a close");
    });
}

// "a turn the host comes back to" > "hands no deploy to a machine that napped while the update waited for its turn, and leaves no note on its row"
#[test]
fn hands_no_deploy_to_a_machine_that_napped_while_the_update_waited_for_its_turn_and_leaves_no_note_on_its_row() {
    drive(|| async {
        let backend = StubBackend::new(None);
        backend.set_exec_impl(token_guest());
        let state = sqlite_state();
        let h = MachineRuns::new();
        let u = Update { daemon: helloing_daemon(DAEMON_VERSION - 1, false, None).await, deployed: Rc::default() };
        let warned = Rc::new(capture());
        let down = host_went_down(&h, &state.store, &backend).await;
        backend.machine(0).set_preview_url(&format!("ws://127.0.0.1:{}", u.daemon.port));
        let recipe = deploying(&u.deployed, || {});
        let rt2 = runtime(&backend)
            .store(&state.store)
            .adapter("claude", h.factory())
            .with(move |o| {
                o.daemon_token = Some(TOKEN.into());
                o.golden_recipe = Some(recipe);
                o.daemon_hello_timeout_ms = Some(2_000);
            })
            .build();
        assert_eq!(statuses(&rt2.rt, &down.workspace_id).await, ["running"]);
        let waits = waits_line(&down.workspace_id);
        let (w, l) = (warned.clone(), waits.clone());
        wait!([w, l] w.lines().contains(&l));
        // The nap moves the phase before it ends the turn the update is waiting on, so the wait comes back to a
        // machine the provider has paused.
        rt2.rt.workspaces().nap(&down.workspace_id, None).await.expect("a nap");
        sleep_ms(100).await;
        assert!(u.deployed.borrow().is_empty());
        assert!(j(&rt2.rt.workspaces().get(&down.workspace_id, None, None).await.expect("a workspace")).get("daemonNote").is_none());
        assert_eq!(warned.lines().into_iter().filter(|l| l.starts_with("daemon on")).collect::<Vec<_>>(), [waits]);
        rt2.rt.close().await.expect("a close");
    });
}

// "a turn the host comes back to" > "a turn cut on the way back takes its open prompt with it, so no settled thread is left reading as waiting on a person"
#[test]
fn a_turn_cut_on_the_way_back_takes_its_open_prompt_with_it_so_no_settled_thread_is_left_reading_as_waiting_on_a_person() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::new();
        let down = host_went_down(&h, &state.store, &backend).await;
        h.emit(
            &down.run,
            json!({
                "type": "permission.ask",
                "sessionId": "sess-1",
                "ask": { "askId": "ask_1", "toolName": "Write", "detail": "out.txt", "input": "{\"file_path\":\"/root/out.txt\"}", "options": [{ "id": "allow", "label": "Allow", "effect": "allow" }] },
            }),
        );
        let (store, w) = (state.store.clone(), down.workspace_id.clone());
        wait!([store, w] !store.get("sessions", &w).await.expect("a read").expect("a record")["sessions"][0]["asking"].is_null());

        // The host comes back with no adapter for the harness, so the run cannot be re-opened and the turn is cut.
        let rt2 = runtime(&backend).store(&state.store).build();
        let (r, w) = (rt2.rt.clone(), down.workspace_id.clone());
        wait!([r, w] rows(&r, &w).await[0]["status"] == "failed");
        let row = rows(&rt2.rt, &down.workspace_id).await.remove(0);
        assert!(row.get("asking").is_none());
        let views: Vec<wsp_wire::views::session::SessionView> = vec![serde_json::from_value(row).expect("a row")];
        let threads = wsp_words::views::session::fold_threads(&views);
        assert_eq!(wsp_words::thread_state::thread_word_of(&threads[0]), "Failed");
        rt2.rt.close().await.expect("a close");
    });
}

// "a turn the host comes back to" > "a host that connects ends the runs on the machine that no thread of its own holds, and leaves the one it re-opened"
#[test]
fn a_host_that_connects_ends_the_runs_on_the_machine_that_no_thread_of_its_own_holds_and_leaves_the_one_it_re_opened() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::new();
        let down = host_went_down(&h, &state.store, &backend).await;
        let orphan = "/tmp/wsp-run/aabbccddeeff";
        let reaps = guest_runs(&backend, &[&down.run, orphan]);

        let rt2 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        assert_eq!(statuses(&rt2.rt, &down.workspace_id).await, ["running"]);

        let kept = reaps.clone();
        wait!([kept] !kept.borrow().is_empty());
        assert_eq!(reaps.borrow().len(), 1);
        assert!(reaps.borrow()[0].contains(&format!("rm -rf '{orphan}'.*")));
        assert!(!reaps.borrow()[0].contains(&down.run));
        rt2.rt.close().await.expect("a close");
    });
}

// "a turn the host comes back to" > "a machine that has not answered which runs it holds holds its own verbs, and no verb on another workspace waits on it"
#[test]
fn a_machine_that_has_not_answered_which_runs_it_holds_holds_its_own_verbs_and_no_verb_on_another_workspace_waits_on_it() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::new();
        let s = slow_listing(&backend, &state.store, &h, false).await;

        let rt2 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        let slow_answered = Rc::new(Cell::new(false));
        let (r, id, flag) = (rt2.rt.clone(), s.slow.clone(), slow_answered.clone());
        tokio::task::spawn_local(async move {
            let _ = r.workspaces().get(&id, None, None).await;
            flag.set(true);
        });
        assert_eq!(j(&rt2.rt.workspaces().get(&s.other, None, None).await.expect("a workspace"))["name"], "other");
        assert!(rows(&rt2.rt, &s.other).await.is_empty());
        immediate().await;
        assert!(!slow_answered.get());

        s.answer.open();
        let flag = slow_answered.clone();
        wait!([flag] flag.get());
        assert!(s.reap_at().is_some());
        rt2.rt.close().await.expect("a close");
    });
}

// "a turn the host comes back to" > "a machine whose running turn has not answered the re-open holds no verb on another workspace"
#[test]
fn a_machine_whose_running_turn_has_not_answered_the_re_open_holds_no_verb_on_another_workspace() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::new();
        let down = host_went_down(&h, &state.store, &backend).await;
        let rt1 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        let other = create_on(&rt1.rt, CreateOn::golden("other"), None).await.expect("a workspace").view.id;
        rt1.rt.close().await.expect("a close");
        let answered = Gate::new();
        let held = held_attach(&h, &answered);

        let before = h.reopened().len();
        let rt2 = runtime(&backend).store(&state.store).adapter("claude", held).build();
        assert_eq!(j(&rt2.rt.workspaces().get(&other, None, None).await.expect("a workspace"))["name"], "other");
        assert!(rows(&rt2.rt, &other).await.is_empty());
        assert_eq!(h.reopened().len(), before);

        answered.open();
        let hh = h.clone();
        wait!([hh] hh.reopened().len() == before + 1);
        assert_eq!(statuses(&rt2.rt, &down.workspace_id).await, ["running"]);
        rt2.rt.close().await.expect("a close");
    });
}

// "a turn the host comes back to" > "a listing of every thread does not wait to title a row on a machine whose turns the host is still re-opening"
#[test]
fn a_listing_of_every_thread_does_not_wait_to_title_a_row_on_a_machine_whose_turns_the_host_is_still_re_opening() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::new();
        let down = host_went_down(&h, &state.store, &backend).await;
        let answered = Gate::new();
        // A machine still restoring answers neither the re-open nor a read of the agent's own store until it is back.
        let held = held_attach_and_title(&h, &answered);

        let rt2 = runtime(&backend).store(&state.store).adapter("claude", held).build();
        let listed = rt2.rt.sessions().list(None, None).await.expect("a list");
        assert_eq!(
            listed.iter().map(|s| (j(s)["workspaceId"].clone(), j(s)["status"].clone())).collect::<Vec<_>>(),
            [(json!(down.workspace_id), json!("running"))]
        );
        answered.open();
        rt2.rt.close().await.expect("a close");
    });
}

// "a turn the host comes back to" > "a thread's send into a thread on a machine whose runs are being swept starts nothing there until the sweep is done"
#[test]
fn a_threads_send_into_a_thread_on_a_machine_whose_runs_are_being_swept_starts_nothing_there_until_the_sweep_is_done() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let h = MachineRuns::new();
        let s = slow_listing(&backend, &state.store, &h, true).await;
        let (thread_id, caller) = s.thread.clone().expect("a thread");
        let before = h.envs.borrow().len();

        let rt2 = runtime(&backend).store(&state.store).adapter("claude", h.factory()).build();
        let (r, slow) = (rt2.rt.clone(), s.slow.clone());
        let sent = tokio::task::spawn_local(async move {
            r.sessions()
                .start(&slow, StartOptions { prompt: "and the tests".into(), thread: Some(thread_id), ..Default::default() }, Some(caller))
                .await
                .map(|_| ())
        });
        rt2.rt.workspaces().list(None).await.expect("a list");
        immediate().await;
        assert_eq!(h.envs.borrow().len(), before);

        s.answer.open();
        sent.await.expect("the send").expect("a start");
        assert_eq!(h.envs.borrow().len(), before + 1);
        rt2.rt.close().await.expect("a close");
    });
}

/// `held`: the harness whose attach waits for `gate`.
fn held_attach(h: &Rc<MachineRuns>, gate: &Rc<Gate>) -> wsp_core::types::harness::HarnessAdapterFactory {
    h.holding(gate, false)
}

/// `held` of the listing case: the attach and the title read both wait for `gate`.
fn held_attach_and_title(h: &Rc<MachineRuns>, gate: &Rc<Gate>) -> wsp_core::types::harness::HarnessAdapterFactory {
    h.holding(gate, true)
}

/// `await new Promise(resolve => setImmediate(resolve))`.
async fn immediate() {
    tokio::task::yield_now().await;
}
