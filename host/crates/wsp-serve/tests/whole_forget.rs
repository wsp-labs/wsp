// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/forget.test.ts at 65f6b89c7, case for case. Forgetting, on both objects: a workspace whose
//! machine the provider no longer has, and a thread no turn ever ran on. The TS reads the transcript as a blob of the
//! memory store; the Rust memory store keeps no transcript rows, so every case here runs on the sqlite store and reads
//! the transcript as its rows. Each test names the TS test above it.
mod common;

use std::cell::Cell;
use std::collections::BTreeSet;
use std::future::Future;
use std::rc::Rc;
use std::time::Duration;

use common::scripted::{adapter_of, after_a_turn, done_and_end, Plain, StartFn};
use common::*;
use futures::FutureExt as _;
use serde_json::{json, Value};
use wsp_adapters::port::AdapterError;
use wsp_core::types::api::StartOptions;
use wsp_core::types::harness::HarnessAdapterFactory;
use wsp_core::types::internal::TRANSCRIPTS_HELD;
use wsp_serve::AssembledRuntime;
use wsp_store::store::Store;
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_words::exit::{HostError, Kind};
use wsp_words::views::session::fold_threads;
use wsp_words::words::computer::{forget_undriven_refusal, thread_forget_refusal, THIS_COMPUTER};
use wsp_words::words::thread::codex_not_signed_in_line;
use wsp_words::workspace_state::sign_in_refusal_line;

fn uuid() -> String {
    format!("{:08x}-0000-4000-8000-{:012x}", rand::random::<u32>(), rand::random::<u64>() & 0xffff_ffff_ffff)
}

fn start(prompt: &str) -> StartOptions {
    StartOptions { prompt: prompt.into(), ..Default::default() }
}

fn on_harness(prompt: &str, harness: &str) -> StartOptions {
    StartOptions { harness: Some(harness.into()), ..start(prompt) }
}

/// A harness that dies before it announces a session: only a done and an end, under its own local id.
fn dying() -> HarnessAdapterFactory {
    let start: StartFn = Rc::new(|o| {
        let local_id = uuid();
        let done = json!({ "status": "failed", "error": "claude exited with code 1 before emitting a result" });
        let events = done_and_end(&local_id, &done, 1, false);
        Ok(Plain::of(&local_id, after_a_turn(o.on_event.clone().expect("a sink"), events, done)))
    });
    adapter_of(start)
}

/// A harness that announces its session and replies: a turn that ran.
fn working() -> HarnessAdapterFactory {
    let start: StartFn = Rc::new(|o| {
        let session = uuid();
        let done = json!({ "status": "completed", "text": "all green" });
        let mut events = vec![json!({ "type": "session.start", "sessionId": session, "cwd": "/root/app" })];
        events.extend(done_and_end(&session, &done, 0, true));
        Ok(Plain::of(&session, after_a_turn(o.on_event.clone().expect("a sink"), events, done)))
    });
    adapter_of(start)
}

/// Each CLI's own announce-then-refuse sequence: both announce their session before they can know the machine has no
/// sign-in, then fail the turn with the cause and no work behind it.
fn refusing_sign_in(exit_code: i32, saw_result: bool, error: &str) -> HarnessAdapterFactory {
    let error = error.to_string();
    let start: StartFn = Rc::new(move |o| {
        let session = uuid();
        let done = json!({ "status": "failed", "refusal": "sign-in", "error": error });
        let mut events = vec![json!({ "type": "session.start", "sessionId": session, "cwd": "/root/app" })];
        events.extend(done_and_end(&session, &done, exit_code, saw_result));
        Ok(Plain::of(&session, after_a_turn(o.on_event.clone().expect("a sink"), events, done)))
    });
    adapter_of(start)
}

fn is_conflict(e: &HostError) -> bool {
    e.kind == Some(Kind::Conflict)
}

async fn refused<T>(r: impl Future<Output = Result<T, HostError>>) -> HostError {
    match r.await {
        Ok(_) => panic!("the call was not refused"),
        Err(e) => e,
    }
}

async fn rows(rt: &AssembledRuntime, ws: &str) -> Vec<wsp_wire::views::session::SessionView> {
    rt.sessions().list(Some(ws.to_string()), None).await.expect("a list")
}

async fn thread_ids(rt: &AssembledRuntime, ws: &str) -> Vec<String> {
    fold_threads(&rows(rt, ws).await).into_iter().map(|t| t.id.clone()).collect()
}

async fn history(rt: &AssembledRuntime, ws: &str) -> Vec<Value> {
    rt.sessions().history(ws, None).await.expect("a history").iter().map(j).collect()
}

/// The workspace's transcript as the JSON of its events, read off the rows.
fn transcript(store: &Rc<dyn Store>, ws: &str) -> Vec<Value> {
    store.transcripts().expect("rows").all(ws).expect("a transcript").events.iter().map(j).collect()
}

fn listed(rows: &[wsp_wire::views::session::SessionView]) -> Vec<(String, Option<String>)> {
    rows.iter().map(|s| (s.id.clone(), s.thread_id.clone())).collect()
}

/// The state the cap leaves: the index holds no row of the workspace's threads, while the document keeps their records
/// and the transcript their events.
async fn capped(store: &Rc<dyn Store>, ws: &str) {
    let mut doc = store.get("sessions", ws).await.expect("a read").expect("a sessions doc");
    doc["sessions"] = json!([]);
    store.put("sessions", ws, doc).await.expect("a write");
}

async fn ws_of(fx: &Fx) -> String {
    create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id
}

// "workspaces.forget" > "drops a workspace whose machine is gone: record, transcripts and sessions leave the store and workspace.deleted follows"
#[test]
fn drops_a_workspace_whose_machine_is_gone_record_transcripts_and_sessions_leave_the_store_and_workspace_deleted_follows() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).build();
        let rt = &*fx.rt;
        let events = fx.events();
        let ws = create_on(rt, CreateOn::golden("first"), None).await.expect("a workspace").view.id;
        let blob = json!({ "workspaceId": ws, "events": [{ "type": "session.start", "sessionId": "s1" }] }).to_string();
        fx.store.put_blob("transcripts", &ws, blob.into_bytes()).await.expect("a blob");
        fx.store
            .put("sessions", &ws, json!({ "rows": [{ "id": "s1", "workspaceId": ws, "harness": "claude", "status": "completed" }] }))
            .await
            .expect("a put");
        backend.machine(0).killed.set(true);

        rt.workspaces().forget(&ws, None).await.expect("a forget");

        assert!(rt.workspaces().list(None).await.expect("a list").is_empty());
        let e = refused(rt.workspaces().get(&ws, None, None)).await;
        assert!(e.message.contains(&format!("no such workspace: {ws}")), "{}", e.message);
        assert!(fx.store.get("workspaces", &ws).await.expect("a read").is_none());
        assert!(fx.store.get_blob("transcripts", &ws).await.expect("a read").is_none());
        assert!(fx.store.get("sessions", &ws).await.expect("a read").is_none());
        let deleted = events.of("workspace.deleted");
        assert_eq!(deleted.len(), 1);
        assert_matches(&deleted[0], json!({ "type": "workspace.deleted", "workspaceId": ws }));
    });
}

// "sends a workspace on a computer wsp does not run to delete: its machine is never gone and there is no provider to pause it at"
#[test]
fn sends_a_workspace_on_a_computer_wsp_does_not_run_to_delete_its_machine_is_never_gone_and_there_is_no_provider_to_pause_it_at() {
    drive(|| async {
        let root = tempfile::tempdir().expect("a folder");
        let local = fake_local(root.path());
        let fx = runtime(&StubBackend::new(None)).with(move |o| o.local = Some(local)).build();
        let rt = &*fx.rt;
        let ws = create_on(rt, CreateOn::named("mac").on(HERE_PLACE_ID), None).await.expect("a workspace").view;
        let e = refused(rt.workspaces().forget(&ws.id, None)).await;
        assert_eq!(e.message, forget_undriven_refusal(&ws.name, THIS_COMPUTER));
        assert!(is_conflict(&e));
        let ids: Vec<String> = rt.workspaces().list(None).await.expect("a list").iter().map(|w| w.id.clone()).collect();
        assert_eq!(ids, [ws.id.clone()]);
        rt.close().await.expect("a close");
    });
}

// "refuses a workspace whose machine still exists, running or paused, with the reason, and keeps everything"
#[test]
fn refuses_a_workspace_whose_machine_still_exists_running_or_paused_with_the_reason_and_keeps_everything() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).build();
        let rt = &*fx.rt;
        let ws = create_on(rt, CreateOn::golden("live"), None).await.expect("a workspace").view.id;
        let e = refused(rt.workspaces().forget(&ws, None)).await;
        assert_eq!(e.message, "live's machine m1 is still running; pause it or delete it at the provider first");
        assert!(is_conflict(&e));
        rt.workspaces().nap(&ws, None).await.expect("a nap");
        let e = refused(rt.workspaces().forget(&ws, None)).await;
        assert!(e.message.contains("live's machine m1 is still paused; pause it or delete it at the provider first"), "{}", e.message);
        assert!(!backend.machine(0).killed.get());
        let ids: Vec<String> = rt.workspaces().list(None).await.expect("a list").iter().map(|w| w.id.clone()).collect();
        assert_eq!(ids, [ws.clone()]);
        assert_matches(&fx.store.get("workspaces", &ws).await.expect("a read").expect("a record"), json!({ "id": ws, "phase": "napping" }));
    });
}

// "refuses a workspace one gateway copy answers 404 for while the other still runs its machine, and keeps everything"
#[test]
fn refuses_a_workspace_one_gateway_copy_answers_404_for_while_the_other_still_runs_its_machine_and_keeps_everything() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).build();
        let rt = &*fx.rt;
        let ws = create_on(rt, CreateOn::golden("split"), None).await.expect("a workspace").view.id;
        backend.answers_gone_once(&backend.machine(0), Rc::new(|| true));
        let e = refused(rt.workspaces().forget(&ws, None)).await;
        assert_eq!(e.message, "split's machine m1 is still running; pause it or delete it at the provider first");
        assert!(is_conflict(&e));
        let ids: Vec<String> = rt.workspaces().list(None).await.expect("a list").iter().map(|w| w.id.clone()).collect();
        assert_eq!(ids, [ws.clone()]);
        assert_matches(&fx.store.get("workspaces", &ws).await.expect("a read").expect("a record"), json!({ "id": ws }));
    });
}

// "a launch refused before the agent started" > "records no thread at all: no row in the listing, none in the store, and nothing in the transcript"
#[test]
fn records_no_thread_at_all_no_row_in_the_listing_none_in_the_store_and_nothing_in_the_transcript() {
    drive(|| async {
        let refusing: StartFn = Rc::new(|_| Err(AdapterError("the harness would not launch".into())));
        let fx = runtime(&StubBackend::new(None)).adapter("claude", adapter_of(refusing)).build();
        let rt = &*fx.rt;
        let ws = ws_of(&fx).await;
        let e = refused(rt.sessions().start(&ws, start("build it"), None)).await;
        assert!(e.message.contains("the harness would not launch"), "{}", e.message);
        // Named again, as the person who tried twice did: a second refusal is a second nothing, not a second row.
        let e = refused(rt.sessions().start(&ws, start("build it"), None)).await;
        assert!(e.message.contains("the harness would not launch"), "{}", e.message);
        assert!(rows(rt, &ws).await.is_empty());
        assert!(fold_threads(&rows(rt, &ws).await).is_empty());
        assert!(history(rt, &ws).await.is_empty());
        rt.close().await.expect("a close");
        assert!(fx.store.get("sessions", &ws).await.expect("a read").is_none());
    });
}

// "is refused before the machine is asked when no adapter here runs that agent, and leaves nothing either"
#[test]
fn is_refused_before_the_machine_is_asked_when_no_adapter_here_runs_that_agent_and_leaves_nothing_either() {
    drive(|| async {
        let fx = runtime(&StubBackend::new(None)).adapter("claude", working()).build();
        let rt = &*fx.rt;
        let ws = ws_of(&fx).await;
        let e = refused(rt.sessions().start(&ws, on_harness("build it", "gemini"), None)).await;
        assert!(e.message.contains("no adapter registered for harness \"gemini\""), "{}", e.message);
        assert!(rows(rt, &ws).await.is_empty());
        assert!(history(rt, &ws).await.is_empty());
        rt.close().await.expect("a close");
    });
}

// "sessions.forget" > "drops a thread no turn ever ran on: its row and its transcript rows go, the thread beside it is untouched, and nothing is asked of the machine"
#[test]
fn drops_a_thread_no_turn_ever_ran_on_its_row_and_its_transcript_rows_go_the_thread_beside_it_is_untouched_and_nothing_is_asked_of_the_machine(
) {
    drive(|| async {
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).adapter("claude", working()).adapter("codex", dying()).build();
        let rt = &*fx.rt;
        let ws = ws_of(&fx).await;
        let ran = rt.sessions().start(&ws, start("build it"), None).await.expect("a start");
        ran.finished().await;
        let junk = rt.sessions().start(&ws, on_harness("build it", "codex"), None).await.expect("a start");
        junk.finished().await;
        let kept = ran.view().thread_id.expect("a thread");
        let going = junk.view().thread_id.expect("a thread");
        let folded: Vec<Value> = fold_threads(&rows(rt, &ws).await).iter().map(j).collect();
        let seen: Vec<Value> = folded.iter().map(|t| json!({ "id": t["id"], "status": t["status"], "ran": t["ran"] })).collect();
        assert_eq!(
            seen,
            [json!({ "id": kept, "status": "completed", "ran": true }), json!({ "id": going, "status": "failed", "ran": false })]
        );

        rt.sessions().forget(&going, None, None).await.expect("a forget");

        assert_eq!(thread_ids(rt, &ws).await, [kept.clone()]);
        let threads: BTreeSet<String> =
            history(rt, &ws).await.iter().map(|e| e["threadId"].as_str().unwrap_or_default().to_string()).collect();
        assert_eq!(threads, BTreeSet::from([kept.clone()]));
        assert!(!backend.machine(0).exec_log().join("\n").contains(&going));
        rt.close().await.expect("a close");
        let stored = fx.store.get("sessions", &ws).await.expect("a read").expect("a sessions doc");
        let stored_threads: Vec<Value> = stored["sessions"].as_array().expect("rows").iter().map(|s| s["threadId"].clone()).collect();
        assert_eq!(stored_threads, [json!(kept)]);
        assert!(transcript(&fx.store, &ws).iter().all(|e| e["threadId"] == kept.as_str()));
    });
}

// "on a transcript that does not read is refused before anything changes, and goes once it reads"
#[test]
fn on_a_transcript_that_does_not_read_is_refused_before_anything_changes_and_goes_once_it_reads() {
    drive(|| async {
        let state = sqlite_state();
        let wrapped = WrappedStore::over(&state.store);
        let store = wrapped.store();
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).store(&store).adapter("claude", working()).adapter("codex", dying()).build();
        let rt = &*fx.rt;
        let ws = ws_of(&fx).await;
        rt.sessions().start(&ws, start("build it"), None).await.expect("a start").finished().await;
        let junk = rt.sessions().start(&ws, on_harness("build it", "codex"), None).await.expect("a start");
        junk.finished().await;
        let going = junk.view().thread_id.expect("a thread");
        rt.close().await.expect("a close");

        wrapped.set_unreadable(true);
        let after = runtime(&backend).store(&store).adapter("claude", working()).adapter("codex", dying()).build();
        let e = refused(after.rt.sessions().forget(&going, None, None)).await;
        assert!(e.message.contains(&format!("the transcript of {ws} does not read")), "{}", e.message);
        assert!(thread_ids(&after.rt, &ws).await.contains(&going));
        wrapped.set_unreadable(false);
        after.rt.sessions().forget(&going, None, None).await.expect("a forget");
        assert!(!thread_ids(&after.rt, &ws).await.contains(&going));
        assert!(!history(&after.rt, &ws).await.iter().any(|e| e["threadId"] == going.as_str()));
        after.rt.close().await.expect("a close");
    });
}

// "drops a thread's rows for good when other transcripts open while it writes the thread's row away"
#[test]
fn drops_a_threads_rows_for_good_when_other_transcripts_open_while_it_writes_the_threads_row_away() {
    drive(|| async {
        // The forget took the thread out of the held copy, waited on its session index, and flushed: four opens in that
        // wait let the copy go, and the flush wrote the file back as it was, the thread's rows in it.
        let state = sqlite_state();
        let wrapped = WrappedStore::over(&state.store);
        let store = wrapped.store();
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).store(&store).adapter("claude", working()).adapter("codex", dying()).build();
        let rt = &*fx.rt;
        let ws = ws_of(&fx).await;
        rt.sessions().start(&ws, start("build it"), None).await.expect("a start").finished().await;
        let junk = rt.sessions().start(&ws, on_harness("build it", "codex"), None).await.expect("a start");
        junk.finished().await;
        let going = junk.view().thread_id.expect("a thread");
        let mut others = vec![];
        for n in 0..TRANSCRIPTS_HELD {
            let other = create_on(rt, CreateOn::golden(&format!("other {n}")), None).await.expect("a workspace").view.id;
            rt.sessions().start(&other, start("elsewhere"), None).await.expect("a start").finished().await;
            others.push(other);
        }
        rt.close().await.expect("a close");

        let after = runtime(&backend).store(&store).adapter("claude", working()).adapter("codex", dying()).build();
        let _ = history(&after.rt, &ws).await;
        let (reached, open) = wrapped.gate_puts("sessions");
        let forgetting = {
            let rt = after.rt.clone();
            let going = going.clone();
            tokio::task::spawn_local(async move { rt.sessions().forget(&going, None, None).await })
        };
        tokio::time::timeout(Duration::from_secs(5), reached.wait()).await.expect("the forget never wrote the sessions index");
        for other in &others {
            let _ = history(&after.rt, other).await;
        }
        open.open();
        forgetting.await.expect("the forget task").expect("a forget");
        assert!(!history(&after.rt, &ws).await.iter().any(|e| e["threadId"] == going.as_str()));
        after.rt.close().await.expect("a close");
        assert!(!transcript(&store, &ws).iter().any(|e| e["threadId"] == going.as_str()));
    });
}

// "refuses a thread whose turn did work, in one sentence, and keeps its row and its transcript whole"
#[test]
fn refuses_a_thread_whose_turn_did_work_in_one_sentence_and_keeps_its_row_and_its_transcript_whole() {
    drive(|| async {
        let fx = runtime(&StubBackend::new(None)).adapter("claude", working()).build();
        let rt = &*fx.rt;
        let ws = ws_of(&fx).await;
        let handle = rt.sessions().start(&ws, start("build it"), None).await.expect("a start");
        handle.finished().await;
        let thread = handle.view().thread_id.expect("a thread");

        let e = refused(rt.sessions().forget(&thread, None, None)).await;
        assert_eq!(e.message, thread_forget_refusal(&thread));
        assert!(is_conflict(&e));

        assert_eq!(thread_ids(rt, &ws).await, [thread.clone()]);
        let types: Vec<String> = history(rt, &ws).await.iter().map(|e| e["type"].as_str().unwrap_or_default().to_string()).collect();
        assert_eq!(types, ["session.start", "session.done", "session.end"]);
        // A name this runtime holds nothing by, the same shape a workspace name nothing holds has: a value nothing takes.
        let e = refused(rt.sessions().forget("thr_nobody", None, None)).await;
        assert_eq!(e.message, "no thread thr_nobo");
        assert_eq!(e.kind, Some(Kind::NotFound));
        rt.close().await.expect("a close");
    });
}

// "refuses a thread whose rows fell off the index cap while its record stands, and leaves its transcript whole"
#[test]
fn refuses_a_thread_whose_rows_fell_off_the_index_cap_while_its_record_stands_and_leaves_its_transcript_whole() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let fx = runtime(&backend).store(&state.store).adapter("claude", working()).build();
        let ws = ws_of(&fx).await;
        let handle = fx.rt.sessions().start(&ws, start("build it"), None).await.expect("a start");
        handle.finished().await;
        let thread = handle.view().thread_id.expect("a thread");
        fx.rt.close().await.expect("a close");
        // The state the cap leaves: the index holds no row of the thread, while the workspace's document keeps the
        // thread's record and the transcript keeps its events.
        capped(&state.store, &ws).await;
        let after = runtime(&backend).store(&state.store).adapter("claude", working()).build();
        // Listed off its record, its row gone.
        assert_eq!(listed(&rows(&after.rt, &ws).await), [(thread.clone(), Some(thread.clone()))]);

        // The record is written once a turn was handed over, so a thread with a record and no row is one that ran: the
        // forget is refused in the one sentence, as it is for a thread whose row still says so.
        let e = refused(after.rt.sessions().forget(&thread, None, None)).await;
        assert_eq!(e.message, thread_forget_refusal(&thread));
        assert!(is_conflict(&e));

        let seen: Vec<(Value, Value)> = history(&after.rt, &ws).await.iter().map(|e| (e["threadId"].clone(), e["type"].clone())).collect();
        assert_eq!(
            seen,
            [(json!(thread), json!("session.start")), (json!(thread), json!("session.done")), (json!(thread), json!("session.end"))]
        );
        after.rt.close().await.expect("a close");
        assert_eq!(transcript(&state.store, &ws).len(), 3);
    });
}

const CLAUDE_EXIT: (i32, bool) = (1, true);
const CODEX_EXIT: (i32, bool) = (0, true);

fn claude_error() -> String {
    format!("Not logged in · Please run /login; {}", sign_in_refusal_line(Some(&WorkspaceKind::Local)))
}

// "a turn the agent refused for want of a sign-in" > "is a thread that ran nothing on either agent, whatever the row's session id says, and the forget takes it"
#[test]
fn is_a_thread_that_ran_nothing_on_either_agent_whatever_the_rows_session_id_says_and_the_forget_takes_it() {
    drive(|| async {
        let codex_error = codex_not_signed_in_line("codex login --device-auth");
        let fx = runtime(&StubBackend::new(None))
            .adapter("claude", refusing_sign_in(CLAUDE_EXIT.0, CLAUDE_EXIT.1, &claude_error()))
            .adapter("codex", refusing_sign_in(CODEX_EXIT.0, CODEX_EXIT.1, &codex_error))
            .build();
        let rt = &*fx.rt;
        let ws = ws_of(&fx).await;
        rt.sessions().start(&ws, start("ship it"), None).await.expect("a start").finished().await;
        rt.sessions().start(&ws, on_harness("ship it", "codex"), None).await.expect("a start").finished().await;

        let listed_rows = rows(rt, &ws).await;
        // The session id is on both rows: the announce landed, which is exactly why it cannot be the rule.
        let seen: Vec<Value> = listed_rows
            .iter()
            .map(|r| {
                let r = j(r);
                json!([r["harness"], r["status"], r.get("claudeSessionId").is_some(), r["refusal"]])
            })
            .collect();
        assert_eq!(seen, [json!(["claude", "failed", true, "sign-in"]), json!(["codex", "failed", true, "sign-in"])]);
        assert_eq!(fold_threads(&listed_rows).iter().map(|t| j(t)["ran"].clone()).collect::<Vec<_>>(), [json!(false), json!(false)]);

        for thread in thread_ids(rt, &ws).await {
            rt.sessions().forget(&thread, None, None).await.expect("a forget");
        }

        assert!(rows(rt, &ws).await.is_empty());
        assert!(history(rt, &ws).await.is_empty());
        rt.close().await.expect("a close");
        let stored = fx.store.get("sessions", &ws).await.expect("a read").expect("a sessions doc");
        assert_eq!(stored["sessions"], json!([]));
    });
}

// "still refuses the forget once the person signed in and the thread's next turn did work"
#[test]
fn still_refuses_the_forget_once_the_person_signed_in_and_the_threads_next_turn_did_work() {
    drive(|| async {
        let signed_in = Rc::new(Cell::new(false));
        let flag = signed_in.clone();
        let start_fn: StartFn = Rc::new(move |o| {
            let session = o.resume.clone().unwrap_or_else(uuid);
            let on = flag.get();
            let done = if on {
                json!({ "status": "completed", "text": "all green" })
            } else {
                json!({ "status": "failed", "refusal": "sign-in", "error": claude_error() })
            };
            let mut events = vec![json!({ "type": "session.start", "sessionId": session, "cwd": "/root/app" })];
            if on {
                events.push(json!({ "type": "turn.delta", "sessionId": session, "kind": "text", "text": "all green" }));
            }
            events.extend(done_and_end(&session, &done, if on { 0 } else { CLAUDE_EXIT.0 }, true));
            Ok(Plain::of(&session, after_a_turn(o.on_event.clone().expect("a sink"), events, done)))
        });
        let fx = runtime(&StubBackend::new(None)).adapter("claude", adapter_of(start_fn)).build();
        let rt = &*fx.rt;
        let ws = ws_of(&fx).await;
        let refused_start = rt.sessions().start(&ws, start("ship it"), None).await.expect("a start");
        refused_start.finished().await;
        let thread = refused_start.view().thread_id.expect("a thread");
        let ran =
            |rows: &[wsp_wire::views::session::SessionView]| fold_threads(rows).iter().map(|t| j(t)["ran"].clone()).collect::<Vec<_>>();
        assert_eq!(ran(&rows(rt, &ws).await), [json!(false)]);

        signed_in.set(true);
        rt.sessions()
            .start(&ws, StartOptions { thread: Some(thread.clone()), ..start("ship it") }, None)
            .await
            .expect("a start")
            .finished()
            .await;

        assert_eq!(ran(&rows(rt, &ws).await), [json!(true)]);
        let e = refused(rt.sessions().forget(&thread, None, None)).await;
        assert_eq!(e.message, thread_forget_refusal(&thread));
        assert!(is_conflict(&e));
        rt.close().await.expect("a close");
    });
}

// "a thread whose rows fell off the index cap" > "is forgotten where its only turn died before the agent announced itself, since its record says no turn worked"
#[test]
fn is_forgotten_where_its_only_turn_died_before_the_agent_announced_itself_since_its_record_says_no_turn_worked() {
    drive(|| async {
        // The TS adapter's `finished` rejects; the Rust port's is a result, so the process's death is a failed result
        // with no events, not a rejection.
        let dies = || {
            let start_fn: StartFn = Rc::new(|o| {
                let done = json!({ "status": "failed", "error": "the agent's process died at launch" });
                Ok(Plain::of(&uuid(), after_a_turn(o.on_event.clone().expect("a sink"), vec![], done)))
            });
            adapter_of(start_fn)
        };
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let fx = runtime(&backend).store(&state.store).adapter("claude", dies()).build();
        let ws = ws_of(&fx).await;
        let handle = fx.rt.sessions().start(&ws, start("build it"), None).await.expect("a start");
        handle.finished().await;
        let thread = handle.view().thread_id.expect("a thread");
        fx.rt.close().await.expect("a close");
        let doc = state.store.get("sessions", &ws).await.expect("a read").expect("a sessions doc");
        assert_matches(&doc["threads"][&thread], json!({ "harness": "claude", "worked": false }));
        capped(&state.store, &ws).await;
        let after = runtime(&backend).store(&state.store).adapter("claude", dies()).build();
        // Listed off its record, its row gone.
        assert_eq!(listed(&rows(&after.rt, &ws).await), [(thread.clone(), Some(thread.clone()))]);

        after.rt.sessions().forget(&thread, None, None).await.expect("a forget");

        after.rt.close().await.expect("a close");
        let doc = state.store.get("sessions", &ws).await.expect("a read").expect("a sessions doc");
        assert!(doc.get("threads").and_then(|t| t.get(&thread)).is_none());
    });
}

// "is refused where its one turn announced, worked and was cut by a host restart before its rows fell off"
#[test]
fn is_refused_where_its_one_turn_announced_worked_and_was_cut_by_a_host_restart_before_its_rows_fell_off() {
    drive(|| async {
        let hangs = || {
            let start_fn: StartFn = Rc::new(|o| {
                let session = uuid();
                let on_event = o.on_event.clone().expect("a sink");
                let announce = session.clone();
                tokio::task::spawn_local(async move {
                    on_event(common::adapter::event(json!({ "type": "session.start", "sessionId": announce, "cwd": "/root/app" })));
                });
                Ok(Plain::of(&session, futures::future::pending().boxed_local()))
            });
            adapter_of(start_fn)
        };
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let fx = runtime(&backend).store(&state.store).adapter("claude", hangs()).build();
        let ws = ws_of(&fx).await;
        let handle = fx.rt.sessions().start(&ws, start("build it"), None).await.expect("a start");
        until(|| async { rows(&fx.rt, &ws).await.first().is_some_and(|r| r.claude_session_id.is_some()) }).await;
        let thread = handle.view().thread_id.expect("a thread");
        fx.rt.close().await.expect("a close");
        // The host that comes back cannot re-open the run, so the restart load cuts the turn.
        let restarted = runtime(&backend).store(&state.store).adapter("claude", hangs()).build();
        until(|| async { j(&rows(&restarted.rt, &ws).await[0])["status"] == "failed" }).await;
        restarted.rt.close().await.expect("a close");
        capped(&state.store, &ws).await;
        let after = runtime(&backend).store(&state.store).adapter("claude", hangs()).build();
        // Listed off its record, its row gone.
        assert_eq!(listed(&rows(&after.rt, &ws).await), [(thread.clone(), Some(thread.clone()))]);

        let e = refused(after.rt.sessions().forget(&thread, None, None)).await;
        assert_eq!(e.message, thread_forget_refusal(&thread));
        assert!(is_conflict(&e));

        after.rt.close().await.expect("a close");
    });
}

// "is refused where its worked turns fell off and its one row left is a later launch refused for want of a sign-in"
#[test]
fn is_refused_where_its_worked_turns_fell_off_and_its_one_row_left_is_a_later_launch_refused_for_want_of_a_sign_in() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let fx = runtime(&backend).store(&state.store).adapter("claude", working()).build();
        let ws = ws_of(&fx).await;
        let handle = fx.rt.sessions().start(&ws, start("build it"), None).await.expect("a start");
        handle.finished().await;
        let thread = handle.view().thread_id.expect("a thread");
        fx.rt.close().await.expect("a close");
        capped(&state.store, &ws).await;
        let refusing = refusing_sign_in(1, true, &sign_in_refusal_line(Some(&WorkspaceKind::Local)));
        let after = runtime(&backend).store(&state.store).adapter("claude", refusing).build();
        after
            .rt
            .sessions()
            .start(&ws, StartOptions { thread: Some(thread.clone()), ..start("and again") }, None)
            .await
            .expect("a start")
            .finished()
            .await;
        let seen: Vec<(Option<String>, Value)> =
            rows(&after.rt, &ws).await.iter().map(|r| (r.thread_id.clone(), j(r)["refusal"].clone())).collect();
        assert_eq!(seen, [(Some(thread.clone()), json!("sign-in"))]);

        let e = refused(after.rt.sessions().forget(&thread, None, None)).await;
        assert_eq!(e.message, thread_forget_refusal(&thread));
        assert!(is_conflict(&e));

        let starts: Vec<Value> =
            history(&after.rt, &ws).await.iter().filter(|e| e["type"] == "session.start").map(|e| e["threadId"].clone()).collect();
        assert_eq!(starts, [json!(thread), json!(thread)]);
        after.rt.close().await.expect("a close");
    });
}
