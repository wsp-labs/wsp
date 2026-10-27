// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/transcript-index.test.ts at 65f6b89c7. The host holds no transcript whole until one is opened: each
//! is an index (every thread's words, the facts a send reads) and the events written since its last flush; search and a
//! resume read the index alone. The Rust store keeps a transcript as rows of the state database, not as a file per
//! workspace, so the cases that count file reads, break a file or plant an index older than its transcript have no form here; see each case below.
#[macro_use]
mod common;

use std::rc::Rc;

use common::comeback::{history, opts};
use common::*;
use serde_json::{json, Value};
use wsp_core::types::api::StartOptions;

fn replies(map: &'static [(&'static str, &'static str)]) -> Rc<Scripted> {
    Scripted::new().cwd("/root/app").pieces(2).reply(move |p| map.iter().find(|(k, _)| *k == p).map_or("ok", |(_, v)| v).to_string())
}

async fn search(rt: &wsp_serve::AssembledRuntime, q: &str) -> Value {
    j(&rt.sessions().search(q, None).await.expect("a search"))
}

fn types_of(events: &[Value]) -> Vec<String> {
    events.iter().map(|e| e["type"].as_str().unwrap_or_default().to_string()).collect()
}

const REPLIES: &[(&str, &str)] = &[
    ("fix the redirect", "Done. The redirect now sends every old path to the canonical host and keeps the query string."),
    ("tidy the readme", "Rewrote the install section."),
    ("add a test", "Added one that proves the canonical host answers."),
];

// "a transcript read on demand" > "is read from its file when it is opened, and not again while it is one of the last few opened"
// Not ported: it counts `getBlob("transcripts")` reads. Here a transcript is read by many small statements on the state
// database, and no call of a test's own counts them as one file read.

// "a transcript read on demand" > "carries the events a running turn wrote since the last flush, in what an open reads and in what search finds"
#[test]
fn carries_the_events_a_running_turn_wrote_since_the_last_flush_in_what_an_open_reads_and_in_what_search_finds() {
    drive(|| async {
        let state = sqlite_state();
        let held = Driven::replay();
        let fx = runtime(&StubBackend::new(None)).store(&state.store).adapter("claude", held.factory()).build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        fx.rt.sessions().start(&ws, opts("stream something"), None).await.expect("a start");
        let turn = held.last();
        turn.emit(json!({ "type": "turn.delta", "kind": "text", "text": "a periwinkle answer", "messageId": "m1" }));
        assert_matches(
            history(&fx.rt, &ws).await.last().expect("an event"),
            json!({ "type": "session.delta", "text": "a periwinkle answer" }),
        );
        assert_eq!(search(&fx.rt, "periwinkle").await["hits"].as_array().map(Vec::len), Some(1));
        // Written after the open: the held transcript takes it too.
        turn.emit(json!({ "type": "turn.delta", "kind": "text", "text": " and more", "messageId": "m1" }));
        assert_matches(history(&fx.rt, &ws).await.last().expect("an event"), json!({ "text": " and more" }));
        assert_eq!(search(&fx.rt, "answer and more").await["hits"].as_array().map(Vec::len), Some(1));
        turn.finish(json!({ "status": "completed", "text": "done" }));
        fx.rt.close().await.expect("a close");
        let again = runtime(&StubBackend::new(None)).store(&state.store).build();
        assert_eq!(
            types_of(&history(&again.rt, &ws).await),
            ["session.start", "session.delta", "session.delta", "session.done", "session.end"]
        );
        again.rt.close().await.expect("a close");
    });
}

// "search off the index" > "finds after a restart what it found while the transcripts were held, reading no transcript"
// Ported without its last two lines: `reads()` is not counted (see the note at the top), so "reading no transcript" is not asserted.
#[test]
fn finds_after_a_restart_what_it_found_while_the_transcripts_were_held() {
    drive(|| async {
        let queries = ["canonical", "CANONICAL HOST", "rewrote", "readme", "old path to the", "st and kee", "nowhere at all", "the"];
        let state = sqlite_state();
        let adapter = replies(REPLIES);
        let fx = runtime(&StubBackend::new(None)).store(&state.store).adapter("claude", adapter.factory()).build();
        let a = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let b = create_on(&fx.rt, CreateOn::golden("b"), None).await.expect("a workspace").view.id;
        let redirect = fx.rt.sessions().start(&a, opts("fix the redirect"), None).await.expect("a start");
        redirect.finished().await;
        fx.rt.sessions().start(&a, opts("tidy the readme"), None).await.expect("a start").finished().await;
        fx.rt.sessions().start(&b, opts("add a test"), None).await.expect("a start").finished().await;
        let th = j(&redirect.view())["threadId"].as_str().expect("a thread").to_string();
        fx.rt
            .sessions()
            .start(&a, StartOptions { thread: Some(th), ..opts("fix the redirect") }, None)
            .await
            .expect("a start")
            .finished()
            .await;
        let mut live = vec![];
        for q in queries {
            live.push(search(&fx.rt, q).await);
        }
        assert!(live[0]["hits"].as_array().map_or(0, Vec::len) > 1);
        fx.rt.close().await.expect("a close");

        let after = runtime(&StubBackend::new(None)).store(&state.store).build();
        // On rows an index is kept without each thread's lines (`indexBytes(.., lines = false)`) and the host reads them back after
        // boot; the TS case runs on a memory store, which keeps them. A search before then says how many threads it left unread.
        let rt = after.rt.clone();
        wait!([rt] search(&rt, "canonical").await.get("unsearched").is_none());
        let mut found = vec![];
        for q in queries {
            found.push(search(&after.rt, q).await);
        }
        assert_eq!(found, live);
        after.rt.close().await.expect("a close");
    });
}

// "search off the index" > "makes a missing index again from its transcript, once, and finds what the transcript holds"
// Not ported: it deletes the index blob and counts transcript reads. A transcript's index is a row `TranscriptRows` can write
// (`put_index`) but not delete without the transcript (`clear` takes both), and no read count exists (see the note at the top).

// "a send read off the index" > "resumes a thread whose rows fell off the index cap in its session and folder, without opening its transcript"
// Ported without `expect(atStart).toBe(before)`, the count of transcript reads at the harness's start (see the note at the top).
#[test]
fn resumes_a_thread_whose_rows_fell_off_the_index_cap_in_its_session_and_folder() {
    drive(|| async {
        let state = sqlite_state();
        let first = replies(&[("start", "ok")]);
        let backend = StubBackend::new(None);
        let fx = runtime(&backend).store(&state.store).adapter("claude", first.factory()).build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let opened =
            fx.rt.sessions().start(&ws, StartOptions { cwd: Some("/root/app/sub".into()), ..opts("start") }, None).await.expect("a start");
        opened.finished().await;
        let (th, session) = (
            j(&opened.view())["threadId"].as_str().expect("a thread").to_string(),
            j(&opened.view())["claudeSessionId"].as_str().expect("a session").to_string(),
        );
        fx.rt.close().await.expect("a close");
        // The state the cap leaves: no row of the thread, its record and its transcript standing.
        let mut doc = state.store.get("sessions", &ws).await.expect("a read").expect("a document");
        doc["sessions"] = json!([]);
        state.store.put("sessions", &ws, doc).await.expect("a put");

        let next = replies(&[("more", "again")]);
        let after = runtime(&backend).store(&state.store).adapter("claude", next.factory()).build();
        after.rt.sessions().start(&ws, StartOptions { thread: Some(th), ..opts("more") }, None).await.expect("a start").finished().await;
        let began = {
            let starts = next.starts.borrow();
            (starts[0].resume.clone(), starts[0].cwd.clone())
        };
        assert_eq!(began, (Some(session), Some("/root/app/sub".to_string())));
        after.rt.close().await.expect("a close");
    });
}

// "a transcript file that did not read" and "a transcript file that reads and does not parse": not ported. Both break the file
// `blobs/transcripts/<workspace>` of a `jsonFileStore` (a link to itself, a torn write) and read the `transcripts-unparsed`
// folder; the Rust store keeps rows in one database and has no such file to break.

// "an index file that does not parse" > "is made again from its transcript at boot, as a missing one is"
#[test]
fn an_index_that_does_not_parse_is_made_again_from_its_transcript_at_boot_as_a_missing_one_is() {
    drive(|| async {
        let state = sqlite_state();
        let adapter = Scripted::new().cwd("/root/app").pieces(2).reply(|p| {
            if p == "one" {
                "a periwinkle answer".into()
            } else {
                "an ordinary answer".into()
            }
        });
        let fx = runtime(&StubBackend::new(None)).store(&state.store).adapter("claude", adapter.factory()).build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        fx.rt.sessions().start(&ws, opts("one"), None).await.expect("a start").finished().await;
        fx.rt.close().await.expect("a close");
        state.store.transcripts().expect("rows").put_index(&ws, "{\"of\":{\"by").expect("an index write");

        let warned = wsp_core::console::capture();
        let after = runtime(&StubBackend::new(None)).store(&state.store).build();
        assert_eq!(search(&after.rt, "periwinkle").await["hits"].as_array().map(Vec::len), Some(1));
        drop(warned);
        after.rt.close().await.expect("a close");
    });
}

// "an index write" > "that fails leaves nothing to be written twice, and boot reads that transcript again"
#[test]
fn an_index_write_that_fails_leaves_nothing_to_be_written_twice_and_boot_reads_that_transcript_again() {
    drive(|| async {
        let state = sqlite_state();
        let failing = WrappedStore::over(&state.store);
        let adapter = Scripted::new().cwd("/root/app").pieces(2).reply(|p| format!("reply to {p}"));
        let fx = runtime(&StubBackend::new(None)).store(&failing.store()).adapter("claude", adapter.factory()).build();
        let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        fx.rt.sessions().start(&ws, opts("one"), None).await.expect("a start").finished().await;
        let warned = wsp_core::console::capture();
        failing.fail_next_index_write("disk full");
        fx.rt.sessions().start(&ws, opts("two"), None).await.expect("a start").finished().await;
        sleep_ms(20).await;
        drop(warned);
        fx.rt.close().await.expect("a close");
        let texts = |h: Vec<Value>| {
            h.iter()
                .filter(|e| e["type"] == "session.delta" && e["kind"] == "text")
                .map(|e| e["text"].as_str().unwrap_or_default().to_string())
                .collect::<Vec<_>>()
                .join("")
        };

        let after = runtime(&StubBackend::new(None)).store(&state.store).adapter("claude", adapter.factory()).build();
        after.rt.sessions().start(&ws, opts("three"), None).await.expect("a start").finished().await;
        assert_eq!(texts(history(&after.rt, &ws).await), "reply to onereply to tworeply to three");
        after.rt.close().await.expect("a close");
    });
}

// "an index write" > "left older than its transcript, as a crash between the two writes leaves it, is made again at boot"
// Not ported: it holds the index of a transcript file back and plays it over a newer file. On rows the index is written in the
// transaction that changes the rows (`loadRowsIndex`, transcripts.ts), so a crash cannot leave the two apart and boot keeps the
// index it reads; a stale one planted by hand is used as it stands.
