// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/runtime-history.test.ts at 65f6b89c7, case for case, the second part: the folder and message ids
//! a transcript keeps, how it is written out and capped, what the agent held, and the move of an older build's
//! transcripts. The helpers of that file's `describe` are in whole_runtime_history/mod.rs.
//!
//! Left out of the port, with the TS name and why:
//! - "a pending debounce holds the process open until it flushes; the idle window does not": it reads
//!   `fc.holding()`, the timers that are not `unref`ed. The Rust `Clock` has no `unref` (wsp-core clock.rs: "`unref` is
//!   dropped: it has no meaning in Rust"), so `FakeClock` cannot say which pending timers hold a process open.
//! - "persists flushes in order even when an earlier put finishes last": it holds the first `putBlob` of a transcript
//!   open on a promise. The Rust store keeps a transcript as rows, and `TranscriptRows::append` is a plain
//!   synchronous call, so a test on this single-threaded loop has nothing to await in it and cannot hold one open.
mod common;
#[path = "whole_runtime_history/mod.rs"]
mod fixture;

use common::comeback::{history, rows, within};
use common::*;
use fixture::*;
use serde_json::{json, Value};
use wsp_core::clock::FakeClock;
use wsp_core::types::api::StartOptions;
use wsp_core::types::internal::{TOOL_RESULT_KEPT, TRANSCRIPT_BYTES, TRANSCRIPT_FLUSH_MS};

async fn go(rt: &wsp_serve::AssembledRuntime, ws: &str, o: StartOptions) -> std::rc::Rc<dyn wsp_core::types::wiring::SessionHandle> {
    rt.sessions().start(ws, o, None).await.expect("a start")
}

async fn workspace(rt: &wsp_serve::AssembledRuntime, name: &str) -> String {
    create_on(rt, CreateOn::golden(name), None).await.expect("a workspace").view.id
}

fn thread_start(prompt: &str, thread: &str) -> StartOptions {
    StartOptions { prompt: prompt.into(), thread: Some(thread.into()), ..Default::default() }
}

fn starts_with_prompt(events: &[Value], prompt: &str) -> bool {
    events.iter().any(|e| e["type"] == "session.start" && e["prompt"] == prompt)
}

fn deltas_of(events: &Events, key: &str) -> Vec<Option<String>> {
    events.of("session.delta").iter().map(|e| text_at(e, key)).collect()
}

// "runtime session history" > "SessionView carries the folder: the start request's until the harness announces its own"
#[test]
fn session_view_carries_the_folder_the_start_requests_until_the_harness_announces_its_own() {
    drive(|| async {
        let m = Manual::new();
        let fx = runtime(&StubBackend::new(None)).adapter("claude", m.factory()).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        let handle = go(&rt, &ws, StartOptions { prompt: "go".into(), cwd: Some("/root/app".into()), ..Default::default() }).await;
        assert_eq!(j(&handle.view())["cwd"], "/root/app");
        m.start(Some("/root/app/packages/web"));
        assert_eq!(rows(&rt, &ws).await[0]["cwd"], "/root/app/packages/web");
        m.done("done");
        m.end();
        handle.finished().await;
        rt.close().await.expect("a close");
    });
}

// "runtime session history" > "SessionView keeps the harness folder when a tool call moves the shell; the delta event carries the move"
#[test]
fn session_view_keeps_the_harness_folder_when_a_tool_call_moves_the_shell_the_delta_event_carries_the_move() {
    drive(|| async {
        let m = Manual::new();
        let fx = runtime(&StubBackend::new(None)).adapter("claude", m.factory()).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        let events = fx.events();
        let handle = go(&rt, &ws, StartOptions { prompt: "go".into(), cwd: Some("/root".into()), ..Default::default() }).await;
        m.start(Some("/root"));
        m.tool("ls", None);
        m.tool("cd /root/2048 && ls", Some("/root/2048"));
        assert_eq!(rows(&rt, &ws).await[0]["cwd"], "/root");
        assert_eq!(deltas_of(&events, "cwd"), [None, Some("/root/2048".to_string())]);
        m.done("done");
        m.end();
        handle.finished().await;
        let events = history(&rt, &ws).await;
        let kept: Vec<Option<String>> = events.iter().filter(|e| e["type"] == "session.delta").map(|e| text_at(e, "cwd")).collect();
        assert_eq!(kept, [None, Some("/root/2048".to_string())]);
        rt.close().await.expect("a close");
    });
}

// "runtime session history" > "the transcript keeps the harness's message id on each piece of text, so a reader folds the turn's replies apart"
#[test]
fn the_transcript_keeps_the_harnesss_message_id_on_each_piece_of_text_so_a_reader_folds_the_turns_replies_apart() {
    drive(|| async {
        let m = Manual::new();
        let fx = runtime(&StubBackend::new(None)).adapter("claude", m.factory()).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        let handle = go(&rt, &ws, opts("hold for 90 seconds")).await;
        m.start(None);
        m.say("Waiting for the hold to complete.", Some("msg_a"));
        m.say("Done.", Some("msg_b"));
        m.tool("ls", None);
        m.done("Done.");
        m.end();
        handle.finished().await;
        let events = history(&rt, &ws).await;
        let ids: Vec<Option<String>> = events.iter().filter(|e| e["type"] == "session.delta").map(|e| text_at(e, "messageId")).collect();
        assert_eq!(ids, [Some("msg_a".to_string()), Some("msg_b".to_string()), None]);
        rt.close().await.expect("a close");
    });
}

// "runtime session history" > "coalesces a burst of turn boundaries into one put after the debounce instead of one per event"
#[test]
fn coalesces_a_burst_of_turn_boundaries_into_one_put_after_the_debounce_instead_of_one_per_event() {
    drive(|| async {
        let state = sqlite_state();
        let counted = Counted::over(&state.store);
        let m = Manual::new();
        let fc = FakeClock::at_now();
        let fx = runtime(&StubBackend::new(None)).store(&counted.store()).clock(&fc).adapter("claude", m.factory()).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        go(&rt, &ws, opts("go")).await;
        m.start(None);
        for i in 0..20 {
            m.done(&format!("t{i}"));
        }
        fc.advance(TRANSCRIPT_FLUSH_MS as i64 - 1);
        assert_eq!(counted.puts(), 0);
        fc.advance(1);
        let seen = counted.clone();
        assert!(
            within(1_000, move || {
                let seen = seen.clone();
                async move { seen.puts() == 1 }
            })
            .await
        );
        assert_eq!(counted.stored(&ws).len(), 21);
        fc.advance(TRANSCRIPT_FLUSH_MS as i64 * 2);
        assert_eq!(counted.puts(), 1);
        m.end();
        rt.close().await.expect("a close");
    });
}

// "runtime session history" > "session.end lands in the store at once, without waiting out the debounce"
#[test]
fn session_end_lands_in_the_store_at_once_without_waiting_out_the_debounce() {
    drive(|| async {
        let state = sqlite_state();
        let counted = Counted::over(&state.store);
        let fx = runtime(&StubBackend::new(None)).store(&counted.store()).adapter("claude", scripted("x")).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        go(&rt, &ws, opts("go")).await.finished().await;
        let (seen, id) = (counted.clone(), ws.clone());
        assert!(
            within(100, move || {
                let (seen, id) = (seen.clone(), id.clone());
                async move { seen.stored(&id).len() == 4 }
            })
            .await
        );
        assert_eq!(types_of(&counted.stored(&ws)), ["session.start", "session.delta", "session.done", "session.end"]);
        assert_eq!(counted.puts(), 1);
    });
}

// "runtime session history" > "close() writes what is still waiting on the debounce and leaves no timer behind"
#[test]
fn close_writes_what_is_still_waiting_on_the_debounce_and_leaves_no_timer_behind() {
    drive(|| async {
        let state = sqlite_state();
        let counted = Counted::over(&state.store);
        let m = Manual::new();
        let fc = FakeClock::at_now();
        let fx = runtime(&StubBackend::new(None)).store(&counted.store()).clock(&fc).adapter("claude", m.factory()).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        go(&rt, &ws, opts("go")).await;
        m.start(None);
        m.done("t0");
        rt.close().await.expect("a close");
        assert_eq!(counted.puts(), 1);
        assert_eq!(types_of(&counted.stored(&ws)), ["session.start", "session.done"]);
        assert_eq!(fc.pending(), 0);
        fc.advance(TRANSCRIPT_FLUSH_MS as i64 * 2);
        assert_eq!(counted.puts(), 1);
    });
}

// "runtime session history" > "caps the persisted transcript so a chatty thread cannot grow the store without bound"
#[test]
fn caps_the_persisted_transcript_so_a_chatty_thread_cannot_grow_the_store_without_bound() {
    drive(|| async {
        let state = sqlite_state();
        let counted = Counted::over(&state.store);
        let fx = runtime(&StubBackend::new(None)).store(&counted.store()).adapter("claude", scripted("x")).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        let first = go(&rt, &ws, opts("t0")).await;
        first.finished().await;
        for i in 1..1300 {
            go(&rt, &ws, thread_start(&format!("t{i}"), &thread_of(&first))).await.finished().await;
        }
        let events = history(&rt, &ws).await;
        assert!(events.len() <= 5000);
        assert_matches(events.last().expect("an event"), json!({ "type": "session.end" }));
        assert!(starts_with_prompt(&events, "t1299"));
        assert!(!starts_with_prompt(&events, "t0"));
        // close() waits the put chain out, so nothing drains into the next test's clock.
        rt.close().await.expect("a close");
        let persisted = counted.stored(&ws);
        assert!(persisted.len() <= 5000);
        assert!(starts_with_prompt(&persisted, "t1299"));
    });
}

// "runtime session history" > "caps a thread in bytes as well as in events, so its tool calls cannot fill the host's memory"
#[test]
fn caps_a_thread_in_bytes_as_well_as_in_events_so_its_tool_calls_cannot_fill_the_hosts_memory() {
    drive(|| async {
        let state = sqlite_state();
        let counted = Counted::over(&state.store);
        let fx = runtime(&StubBackend::new(None)).store(&counted.store()).adapter("claude", tooled(&"w".repeat(256 * 1024), "ok")).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        let first = go(&rt, &ws, opts("t0")).await;
        first.finished().await;
        for i in 1..40 {
            go(&rt, &ws, thread_start(&format!("t{i}"), &thread_of(&first))).await.finished().await;
        }
        let events = history(&rt, &ws).await;
        assert!(json_bytes(&events) <= TRANSCRIPT_BYTES);
        assert!(starts_with_prompt(&events, "t39"));
        assert!(!starts_with_prompt(&events, "t0"));
        rt.close().await.expect("a close");
        let persisted = counted.stored(&ws);
        assert!(json_bytes(&persisted) <= TRANSCRIPT_BYTES);
        // A runtime reading the file back holds the same events, not more.
        let again = runtime(&StubBackend::new(None)).store(&counted.store()).build().rt;
        assert_eq!(history(&again, &ws).await.len(), events.len());
        again.close().await.expect("a close");
    });
}

// "runtime session history" > "one thread's traffic never trims another thread's history off the workspace they share"
#[test]
fn one_threads_traffic_never_trims_another_threads_history_off_the_workspace_they_share() {
    drive(|| async {
        let state = sqlite_state();
        let counted = Counted::over(&state.store);
        let fx = runtime(&StubBackend::new(None)).store(&counted.store()).adapter("claude", tooled(&"w".repeat(256 * 1024), "ok")).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        // Every thread of a project's folder is one workspace: a quiet thread, then busy ones beside it.
        let quiet = go(&rt, &ws, opts("the quiet one")).await;
        quiet.finished().await;
        let quiet_held = history(&rt, &ws).await.len();
        // Three busy threads, each past the byte cap on its own.
        let mut busy: Vec<String> = vec![];
        for b in 0..3 {
            let first = go(&rt, &ws, opts(&format!("busy {b}.0"))).await;
            first.finished().await;
            busy.push(thread_of(&first));
            for i in 1..20 {
                go(&rt, &ws, thread_start(&format!("busy {b}.{i}"), &thread_of(&first))).await.finished().await;
            }
        }
        let events = history(&rt, &ws).await;
        let of_quiet: Vec<Value> = events.iter().filter(|e| e["threadId"] == thread_of(&quiet)).cloned().collect();
        assert!(starts_with_prompt(&of_quiet, "the quiet one"));
        assert_matches(of_quiet.last().expect("an event"), json!({ "type": "session.end" }));
        assert_eq!(of_quiet.len(), quiet_held);
        // Every busy thread keeps its newest turn whole and stays inside its own cap.
        for (b, thread_id) in busy.iter().enumerate() {
            let own: Vec<Value> = events.iter().filter(|e| e["threadId"] == thread_id.as_str()).cloned().collect();
            assert!(starts_with_prompt(&own, &format!("busy {b}.19")));
            assert!(!starts_with_prompt(&own, &format!("busy {b}.0")));
            assert!(json_bytes(&own) <= TRANSCRIPT_BYTES);
        }
        rt.close().await.expect("a close");
    });
}

// "runtime session history" > "passes what the agent held after each of its own calls by on the bus, and writes it only for a turn whose result names none"
#[test]
fn passes_what_the_agent_held_after_each_of_its_own_calls_by_on_the_bus_and_writes_it_only_for_a_turn_whose_result_names_none() {
    drive(|| async {
        let fx = runtime(&StubBackend::new(None)).adapter("claude", holding(vec![false, true], false)).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        let events = fx.events();
        let first = go(&rt, &ws, opts("/compact")).await;
        first.finished().await;
        go(&rt, &ws, thread_start("again", &thread_of(&first))).await.finished().await;
        // A subagent's call names no figure and passes nothing; the first turn's row goes out as it is written.
        let passed: Vec<Value> = events.of("session.context").iter().map(|e| json!([e["context"]])).collect();
        assert_eq!(nums(&Value::Array(passed)), nums(&json!([[26_000], [27_500], [27_500], [26_000], [27_500]])));
        let events = history(&rt, &ws).await;
        // The first turn's result named nothing, so its last reading is its row; the second's result is the reading.
        assert_eq!(
            nums(&Value::Array(held_rows(&events))),
            nums(&json!([["compacted", 1, 27_500, 3_100], [first.turn_id(), 27_500, 200_000], ["compacted", 1, 27_500, 3_100]]))
        );
        rt.close().await.expect("a close");
    });
}

// "runtime session history" > "writes what a running turn's agent held as its host closes, for the next window to read off the transcript"
#[test]
fn writes_what_a_running_turns_agent_held_as_its_host_closes_for_the_next_window_to_read_off_the_transcript() {
    drive(|| async {
        let state = sqlite_state();
        let fx = runtime(&StubBackend::new(None)).store(&state.store).adapter("claude", holding(vec![], true)).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        let handle = go(&rt, &ws, opts("work")).await;
        let (seen, id) = (rt.clone(), ws.clone());
        within(5_000, move || {
            let (seen, id) = (seen.clone(), id.clone());
            async move { held_rows(&history(&seen, &id).await).len() == 1 }
        })
        .await;
        assert_eq!(nums(&Value::Array(held_rows(&history(&rt, &ws).await))), nums(&json!([["compacted", 1, 27_500, 3_100]])));
        rt.close().await.expect("a close");
        let again = runtime(&StubBackend::new(None)).store(&state.store).build().rt;
        assert_eq!(
            nums(&Value::Array(held_rows(&history(&again, &ws).await))),
            nums(&json!([["compacted", 1, 27_500, 3_100], [handle.turn_id(), 27_500, 200_000]]))
        );
        again.close().await.expect("a close");
    });
}

// "runtime session history" > "keeps a tool result's opening characters in the transcript and hands the live stream all of it"
#[test]
fn keeps_a_tool_results_opening_characters_in_the_transcript_and_hands_the_live_stream_all_of_it() {
    drive(|| async {
        let output = format!("first line\n{}", "r".repeat(TOOL_RESULT_KEPT * 4));
        let fx = runtime(&StubBackend::new(None)).adapter("claude", tooled("{}", &output)).build();
        let rt = fx.rt.clone();
        let ws = workspace(&rt, "a").await;
        let live = fx.events();
        go(&rt, &ws, opts("read it")).await.finished().await;
        let heard: Vec<String> = live
            .of("session.delta")
            .iter()
            .filter(|e| e["kind"] == "tool_result")
            .map(|e| e["text"].as_str().unwrap_or_default().to_string())
            .collect();
        assert_eq!(heard, std::slice::from_ref(&output));
        let kept: Vec<String> = history(&rt, &ws)
            .await
            .iter()
            .filter(|e| e["type"] == "session.delta" && e["kind"] == "tool_result")
            .map(|e| e["text"].as_str().unwrap_or_default().to_string())
            .collect();
        assert_eq!(kept, [output[..TOOL_RESULT_KEPT].to_string()]);
        rt.close().await.expect("a close");
    });
}

// "runtime session history" > "moves the transcripts an older build kept inside the state file into files of their own"
#[test]
fn moves_the_transcripts_an_older_build_kept_inside_the_state_file_into_files_of_their_own() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let rt = runtime(&backend).store(&state.store).build().rt;
        let ws = workspace(&rt, "a").await;
        rt.close().await.expect("a close");
        let events = vec![
            json!({ "type": "session.start", "workspaceId": ws, "sessionId": "s1", "threadId": "thr_1", "prompt": "go", "at": 1 }),
            json!({ "type": "session.end", "workspaceId": ws, "sessionId": "s1", "threadId": "thr_1", "exitCode": 0, "sawResult": true, "at": 2 }),
        ];
        state.store.put("transcripts", &ws, json!({ "workspaceId": ws, "events": events })).await.expect("a put");

        let after = runtime(&backend).store(&state.store).build().rt;
        // The move gives each event its place in the transcript, in order from one.
        assert_eq!(history(&after, &ws).await, placed(&events));
        assert!(state.store.list("transcripts").await.expect("a list").is_empty());
        let file = state.store.get_blob("transcripts", &ws).await.expect("a read").expect("a transcript file");
        let doc: Value = serde_json::from_slice(&file).expect("a transcript file is JSON");
        assert_eq!(doc, json!({ "workspaceId": ws, "events": placed(&events) }));
        after.close().await.expect("a close");
    });
}

// "runtime session history" > "moves every event of a transcript past the byte cap to its files, and holds only the tail"
#[test]
fn moves_every_event_of_a_transcript_past_the_byte_cap_to_its_files_and_holds_only_the_tail() {
    drive(|| async {
        // The owner's state held 36194 events, and trimming them at the move would have dropped 2395 for good.
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let rt = runtime(&backend).store(&state.store).build().rt;
        let ws = workspace(&rt, "a").await;
        rt.close().await.expect("a close");
        let events: Vec<Value> = (0..1000).map(|i| delta(&ws, i + 1, &"k".repeat(8 * 1024))).collect();
        state.store.put("transcripts", &ws, json!({ "workspaceId": ws, "events": events })).await.expect("a put");

        let after = runtime(&backend).store(&state.store).build().rt;
        let held = history(&after, &ws).await;
        assert!(json_bytes(&held) <= TRANSCRIPT_BYTES);
        assert_eq!(held.last(), placed(&events).last());
        assert_eq!(on_disk(&state.store, &ws).await, placed(&events));
        // A turn after the move rewrites the tail and leaves the head as the move wrote it.
        after.close().await.expect("a close");
        let again = runtime(&backend).store(&state.store).adapter("claude", scripted("more")).build().rt;
        go(&again, &ws, opts("more")).await.finished().await;
        again.close().await.expect("a close");
        let kept = on_disk(&state.store, &ws).await;
        let head = events.len().saturating_sub(held.len());
        assert_eq!(kept.iter().take(head).cloned().collect::<Vec<_>>(), placed(&events).into_iter().take(head).collect::<Vec<_>>());
        assert!(kept.iter().any(|e| e["prompt"] == "more"));
    });
}

// "runtime session history" > "merges a transcript an older build wrote back into the state file with what its files hold, and never duplicates"
#[test]
fn merges_a_transcript_an_older_build_wrote_back_into_the_state_file_with_what_its_files_hold_and_never_duplicates() {
    drive(|| async {
        let backend = StubBackend::new(None);
        let state = sqlite_state();
        let rt = runtime(&backend).store(&state.store).build().rt;
        let ws = workspace(&rt, "a").await;
        rt.close().await.expect("a close");
        let first: Vec<Value> = (0..3000).map(|i| delta(&ws, i + 1, "a")).collect();
        state.store.put("transcripts", &ws, json!({ "workspaceId": ws, "events": first })).await.expect("a put");
        let moved = runtime(&backend).store(&state.store).build().rt;
        assert_eq!(history(&moved, &ws).await.len(), first.len());
        moved.close().await.expect("a close");
        // An older build on the same state finds no transcript in it and writes the two events it recorded there.
        let older = vec![delta(&ws, 5001, "b"), delta(&ws, 5002, "c")];
        state.store.put("transcripts", &ws, json!({ "workspaceId": ws, "events": older })).await.expect("a put");

        let all: Vec<Value> = first.iter().chain(older.iter()).cloned().collect();
        let after = runtime(&backend).store(&state.store).build().rt;
        assert_eq!(history(&after, &ws).await, placed(&all));
        after.close().await.expect("a close");
        // A move cut short after its files were written leaves the same events in both places: they are kept once.
        state.store.put("transcripts", &ws, json!({ "workspaceId": ws, "events": all })).await.expect("a put");
        let redone = runtime(&backend).store(&state.store).build().rt;
        assert_eq!(history(&redone, &ws).await, placed(&all));
        assert_eq!(on_disk(&state.store, &ws).await, placed(&all));
        redone.close().await.expect("a close");
    });
}
