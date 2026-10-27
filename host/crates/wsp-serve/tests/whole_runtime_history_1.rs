// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/runtime-history.test.ts at 65f6b89c7, case for case, the first part: a workspace's session events
//! replayed with their stamps, the start options a harness is handed, the catalog a machine's binary gives and the caches
//! behind it. The helpers of that file's `describe` are in whole_runtime_history/mod.rs. Left out, and why: "persists
//! flushes in order even when an earlier put finishes last" and "a pending debounce holds the process open until it
//! flushes; the idle window does not", in whole_runtime_history_2.rs.
mod common;
#[path = "whole_runtime_history/mod.rs"]
mod fixture;

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use common::comeback::{history, rows};
use common::gone::immediate;
use common::scripted::{adapter_of, after_a_turn, Plain};
use common::*;
use fixture::*;
use futures::channel::oneshot;
use serde_json::{json, Value};
use wsp_core::clock::FakeClock;
use wsp_core::types::api::StartOptions;
use wsp_core::types::events::CATALOG_TTL_MS;
use wsp_engine::errors::EngineError;
use wsp_engine::machine::ExecResult;
use wsp_serve::serve::{serve_runtime, ServeOptions};
use wsp_wire::views::session::SessionOrigin;
use wsp_words::views::session::fold_threads;

const SESSION_4: &str = "44444444-4444-4444-8444-444444444444";

async fn go(rt: &wsp_serve::AssembledRuntime, ws: &str, o: StartOptions) -> Rc<dyn wsp_core::types::wiring::SessionHandle> {
    rt.sessions().start(ws, o, None).await.expect("a start")
}

async fn workspace(rt: &wsp_serve::AssembledRuntime, name: &str) -> String {
    create_on(rt, CreateOn::golden(name), None).await.expect("a workspace").view.id
}

fn thread_start(prompt: &str, thread: &str) -> StartOptions {
    StartOptions { prompt: prompt.into(), thread: Some(thread.into()), ..Default::default() }
}

async fn catalogs(rt: &wsp_serve::AssembledRuntime, ws: &str) -> Vec<Value> {
    rt.harnesses().list(Some(ws.to_string()), None, None).await.expect("a catalog").iter().map(j).collect()
}

fn find<'a>(list: &'a [Value], harness: &str) -> &'a Value {
    list.iter().find(|c| c["harness"] == harness).expect("a catalog of that harness")
}

fn values(options: &Value) -> Vec<String> {
    options.as_array().expect("a list").iter().map(|o| o["value"].as_str().unwrap_or_default().to_string()).collect()
}

fn refusal_of<T>(answer: Result<T, wsp_words::exit::HostError>) -> String {
    match answer {
        Ok(_) => panic!("the start was taken"),
        Err(e) => e.message,
    }
}

fn answers_probe(backend: &std::sync::Arc<StubBackend>) {
    let out = probe_output();
    backend.set_exec_impl(exec_fn(move |_, cmd| Ok(exec_ok(if cmd.contains("claude --help") { &out } else { "" }))));
}

fn two_binaries(backend: &std::sync::Arc<StubBackend>) {
    let out = probe_output();
    backend.set_exec_impl(exec_fn(move |_, cmd| {
        Ok(exec_ok(if cmd.contains("claude --help") {
            &out
        } else if cmd == "codex --describe" {
            "0.9.0\n"
        } else {
            ""
        }))
    }));
}

// "runtime session history" > "replays a workspace's session events with the prompt on session.start, surviving a restart"
#[test]
fn replays_a_workspaces_session_events_with_the_prompt_on_session_start_surviving_a_restart() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let fx = runtime(&backend).store(&state.store).adapter("claude", scripted("hello")).build();
        let rt = fx.rt.clone();
        let a = workspace(&rt, "a").await;
        let b = workspace(&rt, "b").await;
        let first = go(&rt, &a, StartOptions { prompt: "say hello".into(), request_id: Some("req_a1".into()), ..Default::default() }).await;
        first.finished().await;

        let events = history(&rt, &a).await;
        assert_eq!(types_of(&events), ["session.start", "session.delta", "session.done", "session.end"]);
        assert_matches(&events[0], json!({ "type": "session.start", "workspaceId": a, "prompt": "say hello", "requestId": "req_a1" }));
        assert!(!events[1..].iter().any(|e| e.get("requestId").is_some()));
        assert!(history(&rt, &b).await.is_empty());
        // A client that sent no id leaves the start without one; the id is the client's, never minted here.
        go(&rt, &b, opts("say hello")).await.finished().await;
        assert!(history(&rt, &b).await[0].get("requestId").is_none());
        rt.workspaces().delete(&b, None).await.expect("a delete");

        // a fresh runtime over the same store still has it; deleting the workspace drops it
        let rt2 = runtime(&backend).store(&state.store).build().rt;
        assert_eq!(history(&rt2, &a).await, events);
        rt2.workspaces().delete(&a, None).await.expect("a delete");
        assert!(refusal_of(rt2.sessions().history(&a, None).await).contains("no such workspace"));
        assert!(state.store.get_blob("transcripts", &a).await.expect("a read").is_none());
    });
}

// "runtime session history" > "stamps at and one turnId per start on every session event, and forwards the adapter's harness"
#[test]
fn stamps_at_and_one_turn_id_per_start_on_every_session_event_and_forwards_the_adapters_harness() {
    drive(|| async {
        let harness = json!({ "slashCommands": ["compact"], "permissionMode": "bypassPermissions", "agents": ["general-purpose"] });
        let said = harness.clone();
        let scripted = adapter_of(Rc::new(move |o| {
            let done = completed("ok");
            let events = vec![
                json!({ "type": "session.start", "sessionId": SESSION_4, "model": "claude-sonnet-4-5", "harness": said }),
                json!({ "type": "turn.delta", "sessionId": SESSION_4, "kind": "text", "text": "ok" }),
                done_of(SESSION_4, &done),
                end_of(SESSION_4),
            ];
            Ok(Plain::of(SESSION_4, after_a_turn(sink(&o), events, done)))
        }));
        let fx = runtime(&StubBackend::new(None)).adapter("claude", scripted).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        let before = now_ms();
        let first = go(&rt, &ws, opts("first")).await;
        first.finished().await;
        go(&rt, &ws, thread_start("second", &thread_of(&first))).await.finished().await;
        let after = now_ms();

        let events = history(&rt, &ws).await;
        assert_eq!(events.len(), 8);
        for e in &events {
            let at = e["at"].as_f64().expect("an at");
            assert!(at >= before as f64);
            assert!(at <= after as f64);
            assert!(is_uuid(e["turnId"].as_str().expect("a turnId")));
        }
        let turn_ids: HashSet<&Value> = events.iter().map(|e| &e["turnId"]).collect();
        assert_eq!(turn_ids.len(), 2);
        assert_eq!(events[..4].iter().map(|e| &e["turnId"]).collect::<HashSet<_>>().len(), 1);
        assert_eq!(events[4..].iter().map(|e| &e["turnId"]).collect::<HashSet<_>>().len(), 1);
        assert_matches(&events[0], json!({ "type": "session.start", "harness": harness }));
        rt.close().await.expect("a close");
    });
}

// "runtime session history" > "a harness that announces itself twice on one turn records one session.start, under the id it announced"
#[test]
fn a_harness_that_announces_itself_twice_on_one_turn_records_one_session_start_under_the_id_it_announced() {
    drive(|| async {
        let session_id = "66666666-6666-4666-8666-666666666666";
        let twice = adapter_of(Rc::new(move |o| {
            let done = completed("ok");
            let said = json!({ "type": "session.start", "sessionId": session_id, "model": "claude-sonnet-4-5", "cwd": "/root/work" });
            let events = vec![
                said.clone(),
                said,
                json!({ "type": "turn.delta", "sessionId": session_id, "kind": "text", "text": "ok" }),
                done_of(session_id, &done),
                end_of(session_id),
            ];
            Ok(Plain::of(session_id, after_a_turn(sink(&o), events, done)))
        }));
        let fx = runtime(&StubBackend::new(None)).adapter("claude", twice).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        go(&rt, &ws, opts("first")).await.finished().await;

        let events = history(&rt, &ws).await;
        assert_eq!(types_of(&events), ["session.start", "session.delta", "session.done", "session.end"]);
        assert_matches(&events[0], json!({ "type": "session.start", "sessionId": session_id, "prompt": "first", "cwd": "/root/work" }));
        assert_matches(&rows(&rt, &ws).await[0], json!({ "claudeSessionId": session_id, "cwd": "/root/work", "status": "completed" }));
        rt.close().await.expect("a close");
    });
}

// "runtime session history" > "stamps a threadId that a send into the thread keeps and a start without a thread replaces, so two threads replay as two"
#[test]
fn stamps_a_thread_id_that_a_send_into_the_thread_keeps_and_a_start_without_a_thread_replaces_so_two_threads_replay_as_two() {
    drive(|| async {
        let fx = runtime(&StubBackend::new(None)).adapter("claude", threaded()).build();
        let rt = fx.rt.clone();
        let live = fx.events();
        let ws = workspace(&rt, "a").await;
        let first = go(&rt, &ws, opts("first")).await;
        first.finished().await;
        go(&rt, &ws, thread_start("second", &thread_of(&first))).await.finished().await;
        go(&rt, &ws, opts("third")).await.finished().await;

        let events = history(&rt, &ws).await;
        assert_eq!(events.len(), 12);
        for e in &events {
            assert!(is_uuid(e["threadId"].as_str().expect("a threadId")));
        }
        assert_eq!(events[..8].iter().map(|e| &e["threadId"]).collect::<HashSet<_>>().len(), 1);
        assert_eq!(events[8..].iter().map(|e| &e["threadId"]).collect::<HashSet<_>>().len(), 1);
        assert_ne!(events[8]["threadId"], events[0]["threadId"]);
        assert_eq!(thread_runs(&events), 2);
        let heard: Vec<Value> = live
            .all()
            .iter()
            .filter(|e| {
                e.get("threadId").is_some()
                    && !["session.held", "thread.head", "session.row"].contains(&e["type"].as_str().unwrap_or_default())
            })
            .map(|e| e["threadId"].clone())
            .collect();
        assert_eq!(heard, events.iter().map(|e| e["threadId"].clone()).collect::<Vec<_>>());
        // The session rows carry the same ids, so a sidebar can fold rows into the threads the transcript folds into;
        // the resumed turn shares the first one's local id and so its row.
        assert_eq!(
            rows(&rt, &ws).await.iter().map(|s| s["threadId"].clone()).collect::<Vec<_>>(),
            [events[0]["threadId"].clone(), events[8]["threadId"].clone()]
        );
        rt.close().await.expect("a close");
    });
}

// "runtime session history" > "hands a resume the thread so far for a new session to open with, the turn being sent left out, and a fresh start nothing"
#[test]
fn hands_a_resume_the_thread_so_far_for_a_new_session_to_open_with_the_turn_being_sent_left_out_and_a_fresh_start_nothing() {
    drive(|| async {
        type Seed = std::sync::Arc<dyn Fn() -> futures::future::LocalBoxFuture<'static, Result<String, String>>>;
        let seeds: Rc<RefCell<Vec<Option<Seed>>>> = Rc::default();
        let (inner, kept) = (threaded_start(), seeds.clone());
        let claude = adapter_of(Rc::new(move |o| {
            kept.borrow_mut().push(o.seed.clone());
            inner(o)
        }));
        let fx = runtime(&StubBackend::new(None)).adapter("claude", claude).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        let first = go(&rt, &ws, opts("fix the login page")).await;
        first.finished().await;
        go(&rt, &ws, thread_start("now add a test", &thread_of(&first))).await.finished().await;
        assert_eq!(seeds.borrow().iter().map(|seed| seed.is_some()).collect::<Vec<_>>(), [false, true]);
        let seed = seeds.borrow()[1].clone().expect("a seed");
        let text = seed().await.expect("a seed text");
        assert!(text.contains("fix the login page"));
        assert!(!text.contains("now add a test"));
        rt.close().await.expect("a close");
    });
}

// "runtime session history" > "reads the disk at a turn's end where a stop snapshots it, and a row says so only once use passes 90%"
#[test]
fn reads_the_disk_at_a_turns_end_where_a_stop_snapshots_it_and_a_row_says_so_only_once_use_passes_90() {
    drive(|| async {
        let backend = StubBackend::new(None);
        backend.set_capabilities(|c| c.pause_mode = Some(wsp_wire::wire::capabilities::PauseMode::Disk));
        // Used and free in kB, as df prints them: 89.6%, 90.0% and 90.1% used.
        let readings = ["896000 104000", "900000 100000", "901000 99000"];
        let answer = backend.exec_impl_fn();
        let reads = Rc::new(std::cell::Cell::new(0usize));
        let counted = reads.clone();
        backend.set_exec_impl(Rc::new(move |m, cmd, stdin| {
            if cmd == wsp_engine::disk_sync::DISK_USE_CMD {
                let said = format!("{}\n", readings[counted.get()]);
                counted.set(counted.get() + 1);
                return Box::pin(async move { Ok(exec_ok(&said)) });
            }
            answer(m, cmd, stdin)
        }));
        let fx = runtime(&backend).adapter("claude", threaded()).build();
        let rt = fx.rt.clone();
        let events = fx.events();
        let ws = workspace(&rt, "a").await;
        for n in [1usize, 2] {
            go(&rt, &ws, opts(&format!("build {n}"))).await.finished().await;
            let counted = reads.clone();
            common::comeback::within(2_000, move || {
                let counted = counted.clone();
                async move { counted.get() == n }
            })
            .await;
            sleep_ms(20).await;
            // Neither just under the line nor on it says anything.
            assert!(status_rows(&rt).await[0]["reason"].is_null());
        }
        go(&rt, &ws, opts("build 3")).await.finished().await;
        let line = wsp_words::words::thread::disk_full_line(90.1);
        let seen = events.clone();
        common::comeback::within(2_000, move || {
            let (seen, line) = (seen.clone(), line.clone());
            async move { seen.statuses().iter().any(|s| s["reason"] == line.as_str()) }
        })
        .await;
        let reason = status_rows(&rt).await[0]["reason"].as_str().unwrap_or_default().to_string();
        assert!(reason.starts_with("its disk is 90.1% full: "), "{reason}");
        rt.close().await.expect("a close");
    });
}

// "runtime session history" > "a row says who opened its thread: a resumed turn keeps the answer of the turn it resumes, a fresh start gives its own"
#[test]
fn a_row_says_who_opened_its_thread_a_resumed_turn_keeps_the_answer_of_the_turn_it_resumes_a_fresh_start_gives_its_own() {
    drive(|| async {
        let fx = runtime(&StubBackend::new(None)).adapter("claude", threaded()).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        let first = go(&rt, &ws, StartOptions { prompt: "first".into(), started_by: Some(SessionOrigin::Cli), ..Default::default() }).await;
        first.finished().await;
        go(&rt, &ws, thread_start("second", &thread_of(&first))).await.finished().await;
        go(&rt, &ws, opts("third")).await.finished().await;
        assert_eq!(
            rows(&rt, &ws).await.iter().map(|s| json!([s["prompt"], s["startedBy"]])).collect::<Vec<_>>(),
            [json!(["first", "cli"]), json!(["third", "person"])]
        );
        rt.close().await.expect("a close");
    });
}

async fn titles(rt: &wsp_serve::AssembledRuntime, ws: &str) -> Vec<Value> {
    let sessions = rt.sessions().list(Some(ws.to_string()), None).await.expect("a list");
    fold_threads(&sessions)
        .iter()
        .map(|t| {
            let v = j(t);
            json!([v["title"], v["status"]])
        })
        .collect()
}

// "runtime session history" > "a thread is titled by its first prompt: a second send leaves the row's prompt, and so the folded title, unchanged, also across a restart"
#[test]
fn a_thread_is_titled_by_its_first_prompt_a_second_send_leaves_the_rows_prompt_and_so_the_folded_title_unchanged_also_across_a_restart() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let fx = runtime(&backend).store(&state.store).adapter("claude", threaded()).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        let opening = "You are a builder for the wsp repo";
        let first = go(&rt, &ws, StartOptions { prompt: opening.into(), started_by: Some(SessionOrigin::Cli), ..Default::default() }).await;
        first.finished().await;
        let thread = thread_of(&first);
        assert_eq!(titles(&rt, &ws).await, [json!([opening, "completed"])]);
        go(&rt, &ws, thread_start("GitHub is signed in on this machine now", &thread)).await.finished().await;
        assert_eq!(titles(&rt, &ws).await, [json!([opening, "completed"])]);
        let events = history(&rt, &ws).await;
        assert_eq!(
            events.iter().filter(|e| e["type"] == "session.start").map(|e| e["prompt"].clone()).collect::<Vec<_>>(),
            [json!(opening), json!("GitHub is signed in on this machine now")]
        );
        rt.close().await.expect("a close");

        let again = runtime(&backend).store(&state.store).adapter("claude", threaded()).build().rt;
        go(&again, &ws, thread_start("Push the branch", &thread)).await.finished().await;
        assert_eq!(titles(&again, &ws).await, [json!([opening, "completed"])]);
        again.close().await.expect("a close");
    });
}

// "runtime session history" > "sessions.history over the socket carries the threadId"
#[test]
fn sessions_history_over_the_socket_carries_the_thread_id() {
    drive(|| async {
        let fx = runtime(&StubBackend::new(None)).adapter("claude", threaded()).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        go(&rt, &ws, opts("first")).await.finished().await;
        let served = serve_runtime(rt.clone(), ServeOptions { port: Some(0), auth_token: "t".into(), ..Default::default() })
            .await
            .expect("the runtime serves");
        let res = common::local::ws_request(served.port(), "t", "sessions.history", json!({ "workspaceId": ws })).await;
        let events = res["events"].as_array().cloned().expect("events");
        assert_eq!(types_of(&events), ["session.start", "session.delta", "session.done", "session.end"]);
        for e in &events {
            assert!(is_uuid(e["threadId"].as_str().expect("a threadId")));
        }
        assert_eq!(
            events.iter().map(|e| e["threadId"].clone()).collect::<Vec<_>>(),
            history(&rt, &ws).await.iter().map(|e| e["threadId"].clone()).collect::<Vec<_>>()
        );
        served.close().await.expect("a close");
        rt.close().await.expect("a close");
    });
}

// "runtime session history" > "SessionView carries the prompt, when it started and when it ended"
#[test]
fn session_view_carries_the_prompt_when_it_started_and_when_it_ended() {
    drive(|| async {
        let m = Manual::new();
        let fx = runtime(&StubBackend::new(None)).adapter("claude", m.factory()).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        let before = now_ms();
        let handle = go(&rt, &ws, opts("go")).await;
        m.start(None);
        let running = j(&handle.view());
        assert_eq!(running["prompt"], "go");
        assert!(running["startedAt"].as_f64().expect("a startedAt") >= before as f64);
        assert!(running["startedAt"].as_f64().expect("a startedAt") <= now_ms() as f64);
        assert!(running["endedAt"].is_null());

        m.done("done");
        m.end();
        handle.finished().await;
        let ended = rows(&rt, &ws).await.remove(0);
        assert!(ended["endedAt"].as_f64().expect("an endedAt") >= ended["startedAt"].as_f64().expect("a startedAt"));
        assert!(ended["endedAt"].as_f64().expect("an endedAt") <= now_ms() as f64);
        assert_eq!(ended["prompt"], "go");
        rt.close().await.expect("a close");
    });
}

// "runtime session history" > "keeps the row running while the process lives past its reply, refuses a send until it exits, then completes"
#[test]
fn keeps_the_row_running_while_the_process_lives_past_its_reply_refuses_a_send_until_it_exits_then_completes() {
    drive(|| async {
        let m = Manual::new();
        let fx = runtime(&StubBackend::new(None)).adapter("claude", m.factory()).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        let handle = go(&rt, &ws, opts("go")).await;
        m.start(None);
        m.done("here is the reply");
        // The reply is recorded, but the process has not exited: the row is still running, not completed.
        assert_eq!(j(&handle.view())["status"], "running");
        assert_eq!(rows(&rt, &ws).await[0]["status"], "running");
        assert_eq!(types_of(&history(&rt, &ws).await), ["session.start", "session.done"]);
        // A send while the process lives is never refused and never a second agent in the same folder: it waits for
        // that process to exit and runs as the thread's next turn.
        let again = {
            let (rt, ws, ask) = (rt.clone(), ws.clone(), thread_start("again", &thread_of(&handle)));
            tokio::task::spawn_local(async move { rt.sessions().start(&ws, ask, None).await })
        };
        settle().await;
        assert_eq!(*m.starts.borrow(), ["go"]);

        m.end();
        let queued = again.await.expect("the task").expect("a start");
        assert_eq!(j(&queued.outcome()), "queued");
        assert_eq!(*m.starts.borrow(), ["go", "again"]);
        handle.finished().await;
        rt.close().await.expect("a close");
    });
}

// "runtime session history" > "stamps the attempt a start names on the thread's row, keeps it through a restart, and folds it onto the thread"
#[test]
fn stamps_the_attempt_a_start_names_on_the_threads_row_keeps_it_through_a_restart_and_folds_it_onto_the_thread() {
    drive(|| async {
        let m = Manual::new();
        let state = sqlite_state();
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).store(&state.store).adapter("claude", m.factory()).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        let handle = go(&rt, &ws, StartOptions { prompt: "go".into(), attempt: Some("att_1".into()), ..Default::default() }).await;
        assert_matches(&j(&handle.view()), json!({ "attempt": "att_1" }));
        m.start(None);
        m.done("done");
        m.end();
        handle.finished().await;
        let folded: Vec<Value> = fold_threads(&rt.sessions().list(Some(ws.clone()), None).await.expect("a list")).iter().map(j).collect();
        assert_matches(&Value::Array(folded), json!([{ "id": thread_of(&handle), "attempt": "att_1" }]));
        assert!(!m.last().expect("a start").keys.contains(&"attempt"));
        rt.close().await.expect("a close");
        // The machine outlives the host, so the restarted host reads the same provider.
        let rt2 = runtime(&backend).store(&state.store).adapter("claude", m.factory()).build().rt;
        assert_eq!(rows(&rt2, &ws).await.iter().map(|row| row["attempt"].clone()).collect::<Vec<_>>(), [json!("att_1")]);
        let other = go(&rt2, &ws, opts("alone")).await;
        assert!(j(&other.view()).get("attempt").is_none());
        m.start(None);
        m.done("done");
        m.end();
        other.finished().await;
        rt2.close().await.expect("a close");
    });
}

// "runtime session history" > "passes the picked model, effort and permission mode to the harness and records them on the SessionView"
#[test]
fn passes_the_picked_model_effort_and_permission_mode_to_the_harness_and_records_them_on_the_session_view() {
    drive(|| async {
        let m = Manual::new();
        let fx = runtime(&StubBackend::new(None)).adapter("claude", m.factory()).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        let ask = StartOptions {
            prompt: "go".into(),
            model: Some("claude-opus-5-5".into()),
            effort: Some("high".into()),
            permission_mode: Some("acceptEdits".into()),
            ..Default::default()
        };
        let handle = go(&rt, &ws, ask).await;
        let start = m.last().expect("a start").started;
        assert_eq!(
            (start.model.as_deref(), start.effort.as_deref(), start.permission_mode.as_deref()),
            (Some("claude-opus-5-5"), Some("high"), Some("acceptEdits"))
        );
        assert_matches(&j(&handle.view()), json!({ "model": "claude-opus-5-5", "effort": "high", "permissionMode": "acceptEdits" }));
        // The CLI announces the model it resolved; that name replaces the request's on the view.
        m.start(None);
        assert_eq!(rows(&rt, &ws).await[0]["model"], "claude-sonnet-4-5");
        m.done("done");
        m.end();
        handle.finished().await;
        rt.close().await.expect("a close");
    });
}

// "runtime session history" > "a harness with an adapter but no table row gets the picks as named and nothing else of the request"
#[test]
fn a_harness_with_an_adapter_but_no_table_row_gets_the_picks_as_named_and_nothing_else_of_the_request() {
    drive(|| async {
        let m = Manual::new();
        let fx = runtime(&StubBackend::new(None)).adapter("aider", m.factory()).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        let ask = StartOptions {
            prompt: "go".into(),
            harness: Some("aider".into()),
            model: Some("gpt-9".into()),
            cwd: Some("/w".into()),
            started_by: Some(SessionOrigin::Cli),
            request_id: Some("r1".into()),
            ..Default::default()
        };
        let handle = go(&rt, &ws, ask).await;
        let last = m.last().expect("a start");
        let mut keys = last.keys.clone();
        keys.sort();
        assert_eq!(keys, ["cwd", "model", "onEvent", "prompt"]);
        assert_eq!((last.started.model.as_deref(), last.started.cwd.as_deref()), (Some("gpt-9"), Some("/w")));
        let view = j(&handle.view());
        assert!(view.get("requestId").is_none());
        assert_matches(&view, json!({ "harness": "aider", "model": "gpt-9", "startedBy": "cli" }));
        m.done("done");
        m.end();
        handle.finished().await;
        rt.close().await.expect("a close");
    });
}

// "runtime session history" > "a start without picks hands the harness the model, effort and access the catalog marks, the ones the composer shows, and nothing else"
#[test]
fn a_start_without_picks_hands_the_harness_the_model_effort_and_access_the_catalog_marks_the_ones_the_composer_shows_and_nothing_else() {
    drive(|| async {
        let m = Manual::new();
        let fx = runtime(&StubBackend::new(None)).adapter("claude", m.factory()).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        let handle = go(&rt, &ws, opts("go")).await;
        // cwd rides every start now: a workspace is one project's copy and the thread opens in that project's folder.
        assert_eq!(m.last().expect("a start").keys, ["prompt", "cwd", "model", "effort", "permissionMode", "onEvent"]);
        let view = j(&handle.view());
        assert_eq!(view["model"], "claude-opus-5-5");
        assert_eq!(view["effort"], "high");
        // The access is named too: unnamed, it reached the adapter as nothing, which every adapter here reads as its
        // own skip-everything flag, so what the picker showed and what the CLI ran could differ.
        assert_eq!(view["permissionMode"], "bypassPermissions");
        m.done("done");
        m.end();
        handle.finished().await;
        rt.close().await.expect("a close");
    });
}

// "runtime session history" > "lists a catalog per harness with an adapter, from the table when no workspace is named"
#[test]
fn lists_a_catalog_per_harness_with_an_adapter_from_the_table_when_no_workspace_is_named() {
    drive(|| async {
        let fx = runtime(&StubBackend::new(None)).adapter("claude", Manual::new().factory()).build();
        let listed: Vec<Value> = fx.rt.harnesses().list(None, None, None).await.expect("a catalog").iter().map(j).collect();
        // Codex and the rest sit in the table for the day an adapter lands; without one they cannot run a turn and are not listed.
        assert_eq!(listed.iter().map(|c| c["harness"].clone()).collect::<Vec<_>>(), [json!("claude")]);
        let claude = &listed[0];
        assert!(!claude["efforts"].as_array().expect("efforts").is_empty());
        // Marked as the one an unnamed start runs, so a client without the catalog package can pick its list.
        assert_matches(claude, json!({ "source": "table", "version": claude_pin(), "isDefault": true }));
        let bare = runtime(&StubBackend::new(None)).build();
        assert_eq!(bare.rt.harnesses().list(None, None, None).await.expect("a catalog").len(), 0);
    });
}

// "runtime session history" > "harnesses.list on a workspace" > "asks the machine's binary and serves its models, efforts and modes as the catalog"
#[test]
fn asks_the_machines_binary_and_serves_its_models_efforts_and_modes_as_the_catalog() {
    drive(|| async {
        let backend = StubBackend::new(None);
        answers_probe(&backend);
        let fx = runtime(&backend).adapter("claude", Manual::new().factory()).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        let listed = catalogs(&rt, &ws).await;
        let claude = find(&listed, "claude");
        // The wire's isDefault is the harness an unnamed start runs, whichever source answered.
        assert_matches(claude, json!({ "source": "harness", "version": "2.1.257", "isDefault": true }));
        assert_eq!(values(&claude["models"]), ["claude-opus-5", "claude-fable-5-1", "claude-sonnet-5", "claude-haiku-4-5-20251001"]);
        // A binary that still offers Opus 5 keeps it among its current models, under the table's name for it.
        assert_matches(&claude["models"][0], json!({ "label": "Opus 5", "isDefault": true, "contextWindows": ["200k", "1m"] }));
        assert!(!values(&claude["legacyModels"]).contains(&"claude-opus-5".to_string()));
        assert_eq!(values(&claude["permissionModes"]), ["default", "acceptEdits", "auto", "bypassPermissions", "manual", "dontAsk"]);
        assert_eq!(probes(&backend).len(), 1);
        // The probe is the adapter's line under the guest's login, so it runs under the guest's config dir, never HOME.
        // A provider's exec drops stdin, so on its single-login machine the login's variables are piped in from the text.
        assert!(probes(&backend)[0].contains("'CLAUDE_CONFIG_DIR=/root/.claude-cfg'"), "{}", probes(&backend)[0]);
        rt.close().await.expect("a close");
    });
}

// "runtime session history" > "harnesses.list on a workspace" > "a start before any list fills the cache, and the list still marks the default harness"
#[test]
fn a_start_before_any_list_fills_the_cache_and_the_list_still_marks_the_default_harness() {
    drive(|| async {
        let backend = StubBackend::new(None);
        answers_probe(&backend);
        let fx = runtime(&backend).adapter("claude", Manual::new().factory()).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        go(&rt, &ws, opts("first")).await;
        let listed = catalogs(&rt, &ws).await;
        assert_matches(find(&listed, "claude"), json!({ "source": "harness", "version": "2.1.257", "isDefault": true }));
        assert_eq!(probes(&backend).len(), 1);
        rt.close().await.expect("a close");
    });
}

// "runtime session history" > "harnesses.list on a workspace" > "an adapter without a probe line gets the table, marked so, and the machine is not asked"
#[test]
fn an_adapter_without_a_probe_line_gets_the_table_marked_so_and_the_machine_is_not_asked() {
    drive(|| async {
        let backend = StubBackend::new(None);
        answers_probe(&backend);
        let fx = runtime(&backend).adapter("claude", threaded()).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        let listed = catalogs(&rt, &ws).await;
        assert_matches(find(&listed, "claude"), json!({ "source": "table", "version": claude_pin() }));
        assert_eq!(probes(&backend).len(), 0);
        rt.close().await.expect("a close");
    });
}

// "runtime session history" > "harnesses.list on a workspace" > "each adapter decides whether its binary is asked, whatever its harness id, and each harness keeps its own answer"
#[test]
fn each_adapter_decides_whether_its_binary_is_asked_whatever_its_harness_id_and_each_harness_keeps_its_own_answer() {
    drive(|| async {
        let backend = StubBackend::new(None);
        two_binaries(&backend);
        let fx = runtime(&backend).adapter("claude", Manual::new().factory()).adapter("codex", codex_probing()).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        let listed = catalogs(&rt, &ws).await;
        assert_matches(find(&listed, "claude"), json!({ "source": "harness", "version": "2.1.257" }));
        let codex = find(&listed, "codex");
        assert_matches(codex, json!({ "source": "harness", "version": "0.9.0", "label": "Codex" }));
        assert!(codex.get("isDefault").is_none());
        assert_eq!(
            codex["models"],
            json!([{ "value": "gpt-5-codex", "label": "Codex", "isDefault": true, "efforts": ["high"], "contextWindows": [] }])
        );
        // The table lends the words for values the binary only names.
        assert_eq!(
            codex["permissionModes"],
            json!([{ "value": "read-only", "label": "Read only", "description": "Reads only; edits no files and runs no command that writes" }])
        );
        assert_eq!(probes(&backend).len(), 1);
        assert_eq!(codex_probes(&backend).len(), 1);
        rt.close().await.expect("a close");

        let quiet = StubBackend::new(None);
        two_binaries(&quiet);
        let fx2 = runtime(&quiet).adapter("claude", threaded()).adapter("codex", codex_probing()).build();
        let rt2 = fx2.rt.clone();
        let ws2 = workspace(&rt2, "b").await;
        let later = catalogs(&rt2, &ws2).await;
        assert_matches(find(&later, "claude"), json!({ "source": "table", "version": claude_pin() }));
        assert_matches(find(&later, "codex"), json!({ "source": "harness", "version": "0.9.0" }));
        assert_eq!(probes(&quiet).len(), 0);
        assert_eq!(codex_probes(&quiet).len(), 1);
        rt2.close().await.expect("a close");
    });
}

// "runtime session history" > "harnesses.list on a workspace" > "a binary that named why it described nothing keeps that harness's table and lends the footer its words"
#[test]
fn a_binary_that_named_why_it_described_nothing_keeps_that_harnesss_table_and_lends_the_footer_its_words() {
    drive(|| async {
        let backend = StubBackend::new(None);
        backend.set_exec_impl(exec_fn(|_, _| Ok(exec_ok(""))));
        let why = "Codex is not signed in where this thread runs; run codex login there";
        let fx = runtime(&backend).adapter("codex", refusing(why)).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        let listed = catalogs(&rt, &ws).await;
        let codex = find(&listed, "codex");
        let table = j(&wsp_core::harness_catalog::harness_catalog("codex").expect("a codex row"));
        assert_matches(codex, json!({ "source": "table", "version": table["version"], "refusal": why }));
        assert_eq!(values(&codex["models"]), values(&table["models"]));
        assert!(!codex["models"].as_array().expect("models").is_empty());
        rt.close().await.expect("a close");
    });
}

// "runtime session history" > "harnesses.list on a workspace" > "a probe the machine refused is said once, and the table answers until the next probe"
#[test]
fn a_probe_the_machine_refused_is_said_once_and_the_table_answers_until_the_next_probe() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let why = "timeoutMs must be at most 26000 for a dedicated sandbox";
        backend.set_exec_impl(exec_fn(move |_, cmd| {
            if cmd.contains("claude --help") {
                return Err(EngineError::Other { message: why.into(), code: None });
            }
            Ok(exec_ok(""))
        }));
        let warn = wsp_core::console::capture();
        let fx = runtime(&backend).adapter("claude", Manual::new().factory()).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        let listed = catalogs(&rt, &ws).await;
        let claude = find(&listed, "claude");
        assert_matches(claude, json!({ "source": "table", "version": claude_pin() }));
        let table = j(&wsp_core::harness_catalog::harness_catalog("claude").expect("a claude row"));
        assert_eq!(values(&claude["models"]), values(&table["models"]));
        let said = |warn: &wsp_core::console::Captured| -> Vec<String> {
            warn.lines().into_iter().filter(|line| line.contains("the probe of the agent failed")).collect()
        };
        let lines = said(&warn);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains(backend.machines()[0].id()));
        assert!(lines[0].contains("claude"));
        assert!(lines[0].contains(why));
        // The cache holds the failure too, so a person opening the composer twice reads one line and costs one exec.
        catalogs(&rt, &ws).await;
        assert_eq!(said(&warn).len(), 1);
        assert_eq!(probes(&backend).len(), 1);
        rt.close().await.expect("a close");
    });
}

// "runtime session history" > "harnesses.list on a workspace" > "a session start asks the binary of the harness that starts, when its adapter probes, and no other"
#[test]
fn a_session_start_asks_the_binary_of_the_harness_that_starts_when_its_adapter_probes_and_no_other() {
    drive(|| async {
        let backend = StubBackend::new(None);
        two_binaries(&backend);
        let fx = runtime(&backend).adapter("claude", threaded()).adapter("codex", codex_probing()).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        go(&rt, &ws, StartOptions { prompt: "go".into(), harness: Some("codex".into()), ..Default::default() }).await.finished().await;
        immediate().await;
        assert_eq!(codex_probes(&backend).len(), 1);
        assert_eq!(probes(&backend).len(), 0);
        go(&rt, &ws, opts("go")).await.finished().await;
        immediate().await;
        assert_eq!(probes(&backend).len(), 0);
        rt.close().await.expect("a close");
    });
}

// "runtime session history" > "harnesses.list on a workspace" > "a session start re-asks the binary, within the same TTL, so an upgrade on the machine shows within minutes"
#[test]
fn a_session_start_re_asks_the_binary_within_the_same_ttl_so_an_upgrade_on_the_machine_shows_within_minutes() {
    drive(|| async {
        let backend = StubBackend::new(None);
        answers_probe(&backend);
        let fc = FakeClock::at_now();
        let m = Manual::new();
        let fx = runtime(&backend).clock(&fc).adapter("claude", m.factory()).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        let handle = go(&rt, &ws, opts("go")).await;
        immediate().await;
        assert_eq!(probes(&backend).len(), 1);
        catalogs(&rt, &ws).await;
        assert_eq!(probes(&backend).len(), 1);
        m.done("ok");
        m.end();
        handle.finished().await;
        fc.advance(CATALOG_TTL_MS as i64);
        go(&rt, &ws, opts("again")).await.finished().await;
        immediate().await;
        assert_eq!(probes(&backend).len(), 2);
        rt.close().await.expect("a close");
    });
}

// "runtime session history" > "harnesses.list on a workspace" > "past the TTL a session start answers off the lists it holds, while the binary answers the version they were read off, as it is asked again, and the next start reads the new answer"
#[test]
fn past_the_ttl_a_session_start_answers_off_the_lists_it_holds_while_the_binary_answers_the_version_they_were_read_off_as_it_is_asked_again_and_the_next_start_reads_the_new_answer(
) {
    drive(|| async {
        let backend = StubBackend::new(None);
        let asked: Rc<RefCell<Option<oneshot::Sender<ExecResult>>>> = Rc::default();
        let (seen, waiting, out) = (backend.clone(), asked.clone(), probe_output());
        backend.set_exec_impl(exec_async(move |_m, cmd| {
            if cmd.contains("claude --version") && !cmd.contains("claude --help") {
                let head = out.split("__WSP_CATALOG_SEP__").next().unwrap_or_default().to_string();
                return Box::pin(async move { Ok(exec_ok(&head)) });
            }
            if !cmd.contains("claude --help") {
                return Box::pin(async { Ok(exec_ok("")) });
            }
            if probes(&seen).len() == 1 {
                let whole = out.clone();
                return Box::pin(async move { Ok(exec_ok(&whole)) });
            }
            let (tx, rx) = oneshot::channel();
            *waiting.borrow_mut() = Some(tx);
            Box::pin(async move { Ok(rx.await.expect("the test answers the probe")) })
        }));
        let fc = FakeClock::at_now();
        let m = Manual::new();
        let fx = runtime(&backend).clock(&fc).adapter("claude", m.factory()).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        assert_matches(find(&catalogs(&rt, &ws).await, "claude"), json!({ "source": "harness" }));
        fc.advance(CATALOG_TTL_MS as i64);
        let ask = StartOptions { prompt: "go".into(), model: Some("claude-opus-5".into()), ..Default::default() };
        let handle = go(&rt, &ws, ask).await;
        assert_eq!(probes(&backend).len(), 2);
        assert_eq!(m.last().expect("a start").started.model.as_deref(), Some("claude-opus-5"));
        m.done("ok");
        m.end();
        handle.finished().await;
        let answer = asked.borrow_mut().take().expect("the probe was asked");
        let _ = answer.send(exec_ok("garbage\n"));
        immediate().await;
        assert_matches(find(&catalogs(&rt, &ws).await, "claude"), json!({ "source": "table" }));
        assert_eq!(probes(&backend).len(), 2);
        rt.close().await.expect("a close");
    });
}

// "runtime session history" > "harnesses.list on a workspace" > "past the TTL does not run a binary to read its version once its command reads as installing on its first run again"
#[test]
fn past_the_ttl_does_not_run_a_binary_to_read_its_version_once_its_command_reads_as_installing_on_its_first_run_again() {
    drive(|| async {
        let shim = "#!/bin/sh\n# ascii-harness-shim\nexec /usr/local/lib/ascii-harnesses/claude \"$@\"\n";
        let backend = StubBackend::new(None);
        let lazy = Rc::new(std::cell::Cell::new(false));
        let version_reads: Rc<RefCell<Vec<String>>> = Rc::default();
        let (is_lazy, reads, out) = (lazy.clone(), version_reads.clone(), probe_output());
        backend.set_exec_impl(exec_fn(move |_, cmd| {
            if cmd.contains("head -c 4096") {
                return Ok(exec_ok(&if is_lazy.get() { format!("\x1e{shim}\x1e") } else { "\x1e\x1e".to_string() }));
            }
            if cmd.contains("ascii-lazy-harness") {
                return Ok(exec_ok(if is_lazy.get() { "1\n" } else { "0\n" }));
            }
            if cmd.contains("claude --help") {
                return Ok(exec_ok(&out));
            }
            if cmd.contains("claude --version") {
                if is_lazy.get() {
                    reads.borrow_mut().push(cmd.to_string());
                }
                return Ok(exec_ok(out.split("__WSP_CATALOG_SEP__").next().unwrap_or_default()));
            }
            Ok(exec_ok(""))
        }));
        let fc = FakeClock::at_now();
        let fx = runtime(&backend).clock(&fc).adapter("claude", Manual::new().factory()).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        assert_matches(find(&catalogs(&rt, &ws).await, "claude"), json!({ "source": "harness" }));
        lazy.set(true);
        fc.advance(CATALOG_TTL_MS as i64);
        catalogs(&rt, &ws).await;
        rt.close().await.expect("a close");
        assert!(version_reads.borrow().is_empty(), "{:?}", version_reads.borrow());
    });
}

// "runtime session history" > "harnesses.list on a workspace" > "answers from the table, marked so, when the binary gives nothing or the exec fails, and does not ask again within the TTL"
#[test]
fn answers_from_the_table_marked_so_when_the_binary_gives_nothing_or_the_exec_fails_and_does_not_ask_again_within_the_ttl() {
    drive(|| async {
        let backend = StubBackend::new(None);
        backend.set_exec_impl(exec_fn(|_, cmd| {
            if cmd.contains("claude --help") {
                return Err(EngineError::Other { message: "exec timed out".into(), code: None });
            }
            Ok(exec_ok(""))
        }));
        let fc = FakeClock::at_now();
        let fx = runtime(&backend).clock(&fc).adapter("claude", Manual::new().factory()).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        let listed = catalogs(&rt, &ws).await;
        let first = find(&listed, "claude");
        assert_matches(first, json!({ "source": "table", "version": claude_pin() }));
        assert_eq!(values(&first["models"]), ["claude-opus-5-5", "claude-fable-5-1", "claude-sonnet-5-5", "claude-haiku-5-5"]);
        backend.set_exec_impl(exec_fn(|_, cmd| Ok(exec_ok(if cmd.contains("claude --help") { "garbage\n" } else { "" }))));
        catalogs(&rt, &ws).await;
        assert_eq!(probes(&backend).len(), 1);
        fc.advance(CATALOG_TTL_MS as i64);
        let again = catalogs(&rt, &ws).await;
        assert_eq!(probes(&backend).len(), 2);
        assert_eq!(find(&again, "claude")["source"], "table");
        rt.close().await.expect("a close");
    });
}

// "runtime session history" > "harnesses.list on a workspace" > "a start's model, effort and access mode are checked against the binary's catalog and refused with its list; a new thread without a model runs the default it marks"
#[test]
fn a_starts_model_effort_and_access_mode_are_checked_against_the_binarys_catalog_and_refused_with_its_list_a_new_thread_without_a_model_runs_the_default_it_marks(
) {
    drive(|| async {
        let backend = StubBackend::new(None);
        answers_probe(&backend);
        let m = Manual::new();
        let fx = runtime(&backend).adapter("claude", m.factory()).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        let refused = rt
            .sessions()
            .start(&ws, StartOptions { prompt: "go".into(), model: Some("claude-opus-4-1".into()), ..Default::default() }, None)
            .await;
        assert!(refusal_of(refused).contains(
            "model \"claude-opus-4-1\" is not one claude takes; one of: Opus 5 (claude-opus-5), Fable 5.1 (claude-fable-5-1), Sonnet 5 (claude-sonnet-5), Haiku 4.5 (claude-haiku-4-5-20251001); legacy: Opus 4.8 (claude-opus-4-8)"
        ));
        let refused = rt
            .sessions()
            .start(&ws, StartOptions { prompt: "go".into(), permission_mode: Some("yolo".into()), ..Default::default() }, None)
            .await;
        assert!(refusal_of(refused).starts_with("access mode \"yolo\" is not one claude takes; one of: Default (default), "));
        assert!(m.last().is_none());
        assert!(rows(&rt, &ws).await.is_empty());
        let ask = StartOptions {
            prompt: "go".into(),
            effort: Some("high".into()),
            permission_mode: Some("acceptEdits".into()),
            ..Default::default()
        };
        let handle = go(&rt, &ws, ask).await;
        let start = m.last().expect("a start").started;
        assert_eq!(
            (start.model.as_deref(), start.effort.as_deref(), start.permission_mode.as_deref()),
            (Some("claude-opus-5"), Some("high"), Some("acceptEdits"))
        );
        m.start(None);
        m.done("ok");
        m.end();
        handle.finished().await;
        let resumed = go(&rt, &ws, thread_start("more", &thread_of(&handle))).await;
        // A send that names no model runs on the one the thread ran on, as its agent announced it.
        assert_eq!(m.last().expect("a start").started.model.as_deref(), Some("claude-sonnet-4-5"));
        m.done("ok");
        m.end();
        resumed.finished().await;
        // The binary's handshake lists no older model, and the table's legacy ones still start as named.
        let listed = catalogs(&rt, &ws).await;
        let heard = find(&listed, "claude");
        assert_eq!(heard["source"], "harness");
        assert!(values(&heard["legacyModels"]).contains(&"claude-opus-4-8".to_string()));
        let ask = StartOptions {
            prompt: "old".into(),
            model: Some("claude-opus-4-8".into()),
            effort: Some("xhigh".into()),
            ..Default::default()
        };
        let legacy = go(&rt, &ws, ask).await;
        let start = m.last().expect("a start").started;
        assert_eq!((start.model.as_deref(), start.effort.as_deref()), (Some("claude-opus-4-8"), Some("xhigh")));
        m.done("ok");
        m.end();
        legacy.finished().await;
        assert_eq!(probes(&backend).len(), 1);
        rt.close().await.expect("a close");
    });
}

// "runtime session history" > "harnesses.list on a workspace" > "keeps one answer per machine and leaves a napping workspace's binary alone"
#[test]
fn keeps_one_answer_per_machine_and_leaves_a_napping_workspaces_binary_alone() {
    drive(|| async {
        let backend = StubBackend::new(None);
        answers_probe(&backend);
        let fx = runtime(&backend).adapter("claude", Manual::new().factory()).build();
        let rt = fx.rt.clone();
        let a = workspace(&rt, "a").await;
        let b = workspace(&rt, "b").await;
        catalogs(&rt, &a).await;
        catalogs(&rt, &a).await;
        catalogs(&rt, &b).await;
        assert_eq!(probes(&backend).len(), 2);
        rt.workspaces().nap(&b, None).await.expect("a nap");
        let napping = catalogs(&rt, &b).await;
        assert_eq!(find(&napping, "claude")["source"], "table");
        assert_eq!(probes(&backend).len(), 2);
        let nope = rt.harnesses().list(Some("ws_nope".into()), None, None).await;
        assert!(refusal_of(nope).contains("no such workspace"));
        rt.close().await.expect("a close");
    });
}
