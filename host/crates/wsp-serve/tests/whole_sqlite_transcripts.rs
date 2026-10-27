// SPDX-License-Identifier: AGPL-3.0-only
//! The whole-runtime cases of packages/runtime/test/sqlite-transcripts.test.ts at 65f6b89c7: "a runtime whose store
//! keeps transcripts as rows" and "a runtime over rows, where a step does not go through". The module's own cases
//! ("the transcripts module", "the move of the transcript blobs into rows") run on the module in wsp-store, not here.
//! The state.db is read with rusqlite where the TS reads it with node:sqlite. Each test names the TS test above it.
mod common;

use std::rc::Rc;
use std::sync::Arc;

use base64::Engine as _;
use common::adapter::event;
use common::*;
use futures::future::LocalBoxFuture;
use futures::FutureExt as _;
use serde_json::{json, Value};
use wsp_adapters::port::{
    AdapterError, AttachmentRoad, ExecFailure, HarnessAdapter, HarnessSession, HarnessStartOptions, SharedFuture, TurnImage, TurnResult,
};
use wsp_core::types::api::{HistoryAsk, PageWindow, StartOptions};
use wsp_core::types::harness::HarnessAdapterFactory;
use wsp_store::store::Store;
use wsp_wire::types::attachments::Attachment;
use wsp_words::exit::Kind;

fn uuid() -> String {
    let n: u128 = rand::random();
    format!(
        "{:08x}-{:04x}-4{:03x}-8{:03x}-{:012x}",
        (n >> 96) as u32,
        (n >> 80) as u16,
        (n >> 68) as u16 & 0xfff,
        (n >> 56) as u16 & 0xfff,
        n as u64 & 0xffff_ffff_ffff
    )
}

#[derive(Clone)]
struct Line {
    kind: &'static str,
    text: String,
}

fn text(t: impl Into<String>) -> Line {
    Line { kind: "text", text: t.into() }
}

type Lines = Rc<dyn Fn(&str) -> Vec<Line>>;
type Hold = Rc<dyn Fn(&str) -> Option<Rc<Gate>>>;

/// A harness whose turn says what `lines` gives the prompt, then ends once `hold` lets it. It takes images inline.
fn scripted(lines: impl Fn(&str) -> Vec<Line> + 'static, hold: Option<Hold>) -> HarnessAdapterFactory {
    let lines: Lines = Rc::new(lines);
    Arc::new(move |_ctx| Ok(Arc::new(Scripted { lines: lines.clone(), hold: hold.clone() }) as Arc<dyn HarnessAdapter>))
}

struct Scripted {
    lines: Lines,
    hold: Option<Hold>,
}

impl HarnessAdapter for Scripted {
    fn steers(&self) -> bool {
        false
    }
    fn attachments(&self) -> Option<AttachmentRoad> {
        Some(AttachmentRoad::Inline)
    }
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        let session_id = o.resume.clone().unwrap_or_else(uuid);
        let on_event = o.on_event.clone().expect("an event sink");
        let gate = self.hold.as_ref().and_then(|h| h(&o.prompt));
        let lines = (self.lines)(&o.prompt);
        let (tx, rx) = futures::channel::oneshot::channel::<TurnResult>();
        let id = session_id.clone();
        tokio::task::spawn_local(async move {
            tokio::task::yield_now().await;
            on_event(event(json!({ "type": "session.start", "sessionId": id })));
            for (n, l) in lines.iter().enumerate() {
                let mut doc = json!({ "type": "turn.delta", "sessionId": id, "kind": l.kind, "text": l.text });
                if l.kind == "text" {
                    doc["messageId"] = json!(format!("m{n}"));
                } else {
                    doc["toolUseId"] = json!(format!("t{n}"));
                    doc["toolName"] = json!("Bash");
                }
                on_event(event(doc));
            }
            if let Some(gate) = gate {
                gate.wait().await;
            }
            let done = common::adapter::result(json!({ "status": "completed", "text": "done" }));
            on_event(event(json!({ "type": "turn.done", "sessionId": id, "result": serde_json::to_value(&done).expect("a result") })));
            on_event(event(json!({ "type": "session.end", "sessionId": id, "exitCode": 0, "sawResult": true })));
            let _ = tx.send(done);
        });
        let finished: LocalBoxFuture<'static, TurnResult> = async move { rx.await.expect("the turn's end") }.boxed_local();
        Ok(Arc::new(ScriptedSession { local_id: session_id, finished: finished.shared() }))
    }
}

struct ScriptedSession {
    local_id: String,
    finished: SharedFuture<TurnResult>,
}

impl HarnessSession for ScriptedSession {
    fn local_id(&self) -> String {
        self.local_id.clone()
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        self.finished.clone()
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        async {}.boxed_local()
    }
    fn steer(
        &self,
        _prompt: String,
        _id: Option<String>,
        _images: Vec<TurnImage>,
    ) -> Option<LocalBoxFuture<'static, Result<wsp_adapters::port::SteerOutcome, ExecFailure>>> {
        None
    }
}

/// A harness whose turn says "forgotten words" and fails before its result, so the thread did no work and may be forgotten.
fn refused_once() -> HarnessAdapterFactory {
    Arc::new(|_ctx| Ok(Arc::new(RefusedOnce) as Arc<dyn HarnessAdapter>))
}

struct RefusedOnce;

impl HarnessAdapter for RefusedOnce {
    fn steers(&self) -> bool {
        false
    }
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        let id = uuid();
        let on_event = o.on_event.clone().expect("an event sink");
        let result = common::adapter::result(json!({ "status": "failed", "error": "exited with code 1 before emitting a result" }));
        let (tx, rx) = futures::channel::oneshot::channel::<TurnResult>();
        let (emit_id, done) = (id.clone(), result.clone());
        tokio::task::spawn_local(async move {
            tokio::task::yield_now().await;
            on_event(event(
                json!({ "type": "turn.delta", "sessionId": emit_id, "kind": "text", "text": "forgotten words", "messageId": "m0" }),
            ));
            on_event(event(json!({ "type": "turn.done", "sessionId": emit_id, "result": serde_json::to_value(&done).expect("a result") })));
            on_event(event(json!({ "type": "session.end", "sessionId": emit_id, "exitCode": 1, "sawResult": false })));
            let _ = tx.send(done);
        });
        let finished: LocalBoxFuture<'static, TurnResult> = async move { rx.await.expect("the turn's end") }.boxed_local();
        Ok(Arc::new(ScriptedSession { local_id: id, finished: finished.shared() }))
    }
}

/// A harness whose turn fails before it starts: it says `turn.done` and `session.end` and no `session.start`.
fn never_starts() -> HarnessAdapterFactory {
    Arc::new(|_ctx| Ok(Arc::new(NeverStarts) as Arc<dyn HarnessAdapter>))
}

struct NeverStarts;

impl HarnessAdapter for NeverStarts {
    fn steers(&self) -> bool {
        false
    }
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        let id = uuid();
        let on_event = o.on_event.clone().expect("an event sink");
        let result = common::adapter::result(json!({ "status": "failed", "error": "not signed in" }));
        let (tx, rx) = futures::channel::oneshot::channel::<TurnResult>();
        let (emit_id, done) = (id.clone(), result.clone());
        tokio::task::spawn_local(async move {
            tokio::task::yield_now().await;
            on_event(event(json!({ "type": "turn.done", "sessionId": emit_id, "result": serde_json::to_value(&done).expect("a result") })));
            on_event(event(json!({ "type": "session.end", "sessionId": emit_id, "exitCode": 1, "sawResult": false })));
            let _ = tx.send(done);
        });
        let finished: LocalBoxFuture<'static, TurnResult> = async move { rx.await.expect("the turn's end") }.boxed_local();
        Ok(Arc::new(ScriptedSession { local_id: id, finished: finished.shared() }))
    }
}

fn start(prompt: &str) -> StartOptions {
    StartOptions { prompt: prompt.into(), ..Default::default() }
}

fn on_thread(prompt: &str, thread: &str) -> StartOptions {
    StartOptions { thread: Some(thread.into()), ..start(prompt) }
}

/// Starts a turn, waits for it, and answers the thread it ran on.
async fn turn(rt: &Fx, ws: &str, opts: StartOptions) -> String {
    let run = rt.rt.sessions().start(ws, opts, None).await.expect("a start");
    run.finished().await;
    run.view().thread_id.expect("a thread")
}

fn page_of(thread: &str) -> PageWindow {
    PageWindow { thread_id: thread.into(), ..Default::default() }
}

async fn page(rt: &Fx, ws: &str, window: PageWindow) -> Value {
    j(&rt.rt.sessions().page(ws, window, None).await.expect("a page"))
}

async fn history(rt: &Fx, ws: &str) -> Vec<Value> {
    rt.rt.sessions().history(ws, None).await.expect("a history").iter().map(j).collect()
}

async fn hits(rt: &Fx, query: &str) -> Vec<String> {
    let found = j(&rt.rt.sessions().search(query, None).await.expect("a search"));
    found["hits"].as_array().expect("hits").iter().map(|h| h["threadId"].as_str().expect("a thread").to_string()).collect()
}

async fn search(rt: &Fx, query: &str) -> Value {
    j(&rt.rt.sessions().search(query, None).await.expect("a search"))
}

fn n3(i: usize) -> String {
    format!("{i:03}")
}

/// What a history holds, less the moment each event was stamped, which is each runtime's own clock.
fn shape_of(events: &[Value]) -> Vec<Value> {
    events
        .iter()
        .map(|e| {
            let mut e = e.clone();
            let o = e.as_object_mut().expect("an event");
            o.remove("at");
            o.insert("workspaceId".into(), json!(""));
            for key in ["sessionId", "turnId", "threadId"] {
                if o.contains_key(key) {
                    o.insert(key.into(), json!(""));
                }
            }
            e
        })
        .collect()
}

fn db_path(state: &State) -> std::path::PathBuf {
    state.dir.path().join("state.db")
}

fn read_db(state: &State) -> rusqlite::Connection {
    rusqlite::Connection::open_with_flags(db_path(state), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).expect("state.db opens")
}

fn event_rows(d: &rusqlite::Connection, ws: &str) -> Vec<(i64, i64, String)> {
    let mut stmt = d.prepare("select rowid, pos, json from events where workspace = ? order by pos").expect("a statement");
    let rows = stmt.query_map([ws], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).expect("rows");
    rows.map(|r| r.expect("a row")).collect()
}

fn index_json(d: &rusqlite::Connection, ws: &str) -> Option<String> {
    d.query_row("select json from transcript_index where workspace = ?", [ws], |r| r.get(0)).ok()
}

fn delta(workspace: &str, thread: &str, pos: i64) -> Value {
    json!({ "type": "session.delta", "workspaceId": workspace, "sessionId": "s", "threadId": thread, "kind": "text", "text": format!("line {pos}"), "pos": pos })
}

fn count_lines(n: usize, prompt: &str) -> Vec<Line> {
    (0..n).map(|i| text(format!("{prompt} {i}"))).collect()
}

fn forgotten_words() -> &'static str {
    "forgotten words"
}

// "a runtime whose store keeps transcripts as rows" > "appends a turn's events as rows, writes no transcript blob, and never rewrites a row an earlier turn wrote"
#[test]
fn appends_a_turns_events_as_rows_writes_no_transcript_blob_and_never_rewrites_a_row_an_earlier_turn_wrote() {
    drive(|| async {
        let state = sqlite_state();
        let watched = Watched::over(&state.store);
        let backend = StubBackend::new(None);
        let rt = runtime(&backend).store(&watched.store()).adapter("claude", scripted(|_| vec![text("hello")], None)).build();
        let ws = create_on(&rt.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let first = turn(&rt, &ws, start("one")).await;
        rt.rt.close().await.expect("a close");
        let d = read_db(&state);
        let before = event_rows(&d, &ws);
        assert!(before.len() > 2, "{} rows", before.len());

        let backend2 = StubBackend::new(None);
        let again = runtime(&backend2).store(&watched.store()).adapter("claude", scripted(|_| vec![text("again")], None)).build();
        turn(&again, &ws, on_thread("two", &first)).await;
        again.rt.close().await.expect("a close");
        let after = event_rows(&d, &ws);
        assert_eq!(after[..before.len()], before[..]);
        assert!(after.len() > before.len());
        assert_eq!(watched.blobs.borrow().iter().filter(|c| c.starts_with("transcript")).count(), 0);
    });
}

// "a runtime whose store keeps transcripts as rows" > "drops past the caps exactly the events the blobs drop"
#[test]
fn drops_past_the_caps_exactly_the_events_the_blobs_drop() {
    drive(|| async {
        let big = "z".repeat(12 * 1024);
        let lines = move |prompt: &str| -> Vec<Line> {
            if prompt.starts_with("count") {
                count_lines(1800, prompt)
            } else {
                (0..120).map(|_| Line { kind: "tool_result", text: big.clone() }).collect()
            }
        };
        let run = |store: Rc<dyn Store>| {
            let lines = lines.clone();
            async move {
                let backend = StubBackend::new(None);
                let rt = runtime(&backend).store(&store).adapter("claude", scripted(lines, None)).build();
                let ws = create_on(&rt.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
                let a = turn(&rt, &ws, start("count a")).await;
                let b = turn(&rt, &ws, start("bytes b")).await;
                for n in [2, 3, 4] {
                    turn(&rt, &ws, on_thread(&format!("count a{n}"), &a)).await;
                }
                for n in [2, 3] {
                    turn(&rt, &ws, on_thread(&format!("bytes b{n}"), &b)).await;
                }
                rt.rt.close().await.expect("a close");
                let backend2 = StubBackend::new(None);
                let again = runtime(&backend2).store(&store).build();
                let events = history(&again, &ws).await;
                let found = hits(&again, "count a 5").await.len();
                again.rt.close().await.expect("a close");
                (events, found)
            }
        };
        let (blobs_history, blobs_search) = run(memory_state()).await;
        let (rows_history, rows_search) = run(sqlite_state().store).await;
        let threads: std::collections::HashSet<String> =
            blobs_history.iter().map(|e| e["threadId"].as_str().unwrap_or_default().to_string()).collect();
        assert_eq!(threads.len(), 2);
        for thread in &threads {
            assert!(blobs_history.iter().filter(|e| e["threadId"] == thread.as_str()).count() <= 5000);
        }
        assert!(blobs_history.len() < 1800 * 4 + 120 * 3);
        assert_eq!(shape_of(&rows_history), shape_of(&blobs_history));
        assert_eq!(rows_search, blobs_search);
    });
}

// "a runtime whose store keeps transcripts as rows" > "marks a thread the caps took events from on its head and its pages, in state.db, across a restart"
#[test]
fn marks_a_thread_the_caps_took_events_from_on_its_head_and_its_pages_in_state_db_across_a_restart() {
    drive(|| async {
        let lines = |prompt: &str| count_lines(if prompt.starts_with("busy") { 1800 } else { 1 }, prompt);
        let state = sqlite_state();
        let backend = StubBackend::new(None);
        let rt = runtime(&backend).store(&state.store).adapter("claude", scripted(lines, None)).build();
        let ws = create_on(&rt.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let quiet_id = turn(&rt, &ws, start("quiet")).await;
        let busy_id = turn(&rt, &ws, start("busy 1")).await;
        assert!(page(&rt, &ws, page_of(&busy_id)).await.get("trimmed").is_none());
        for n in [2, 3] {
            turn(&rt, &ws, on_thread(&format!("busy {n}"), &busy_id)).await;
        }
        assert_eq!(page(&rt, &ws, page_of(&busy_id)).await["trimmed"], true);
        assert_eq!(j(&rt.rt.sessions().head(&busy_id, None, None).await.expect("a head"))["trimmed"], true);
        assert!(page(&rt, &ws, page_of(&quiet_id)).await.get("trimmed").is_none());
        assert!(j(&rt.rt.sessions().head(&quiet_id, None, None).await.expect("a head")).get("trimmed").is_none());
        rt.rt.close().await.expect("a close");
        let d = read_db(&state);
        let index = index_json(&d, &ws).expect("an index");
        let parsed: Value = serde_json::from_str(&index).expect("json");
        assert_eq!(parsed["trimmed"], json!([busy_id]));
        let again = runtime(&backend).store(&state.store).build();
        assert_eq!(page(&again, &ws, page_of(&busy_id)).await["trimmed"], true);
        assert_eq!(j(&again.rt.sessions().head(&busy_id, None, None).await.expect("a head"))["trimmed"], true);
        assert!(page(&again, &ws, page_of(&quiet_id)).await.get("trimmed").is_none());
        again.rt.close().await.expect("a close");
    });
}

// "a runtime whose store keeps transcripts as rows" > "keeps a thread's every event while another thread in the same folder runs past what the two hold together"
#[test]
fn keeps_a_threads_every_event_while_another_thread_in_the_same_folder_runs_past_what_the_two_hold_together() {
    drive(|| async {
        let lines = |prompt: &str| count_lines(1020, prompt);
        let state = sqlite_state();
        let backend = StubBackend::new(None);
        let rt = runtime(&backend).store(&state.store).adapter("claude", scripted(lines, None)).build();
        let ws = create_on(&rt.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let x_id = turn(&rt, &ws, start("x 1")).await;
        for n in [2, 3] {
            turn(&rt, &ws, on_thread(&format!("x {n}"), &x_id)).await;
        }
        let wide = || PageWindow { thread_id: x_id.clone(), before: None, limit: Some(10_000) };
        let before = page(&rt, &ws, wide()).await;
        assert!(before["total"].as_i64().expect("a total") > 3000);
        let y_id = turn(&rt, &ws, start("y 1")).await;
        for n in [2, 3] {
            turn(&rt, &ws, on_thread(&format!("y {n}"), &y_id)).await;
        }
        let after = page(&rt, &ws, wide()).await;
        assert_eq!(after["total"], before["total"]);
        assert!(after.get("trimmed").is_none());
        let first =
            history(&rt, &ws).await.into_iter().find(|e| e["threadId"] == x_id.as_str() && e["type"] == "session.start").expect("a start");
        assert_matches(&first, json!({ "prompt": "x 1" }));
        rt.rt.close().await.expect("a close");
        let again = runtime(&backend).store(&state.store).build();
        assert_eq!(page(&again, &ws, wide()).await["total"], before["total"]);
        again.rt.close().await.expect("a close");
    });
}

// "a runtime whose store keeps transcripts as rows" > "searches the threads whose words it holds at 300 threads, reading none back, says how many older ones it did not, and searches one again once it is opened"
#[test]
fn searches_the_threads_whose_words_it_holds_at_300_threads_reading_none_back_says_how_many_older_ones_it_did_not_and_searches_one_again_once_it_is_opened(
) {
    drive(|| async {
        // 300 threads of about 20 KB of reply each: 6 MB of words, past what the host holds.
        let lines =
            |prompt: &str| vec![text(format!("{prompt} says marker-{} {}", &prompt[prompt.len() - 3..], "filler words ".repeat(1600)))];
        let state = sqlite_state();
        let watched = Watched::over(&state.store);
        let backend = StubBackend::new(None);
        let rt = runtime(&backend).store(&watched.store()).adapter("claude", scripted(lines, None)).build();
        let ws = create_on(&rt.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let mut threads = Vec::new();
        for i in 0..300 {
            threads.push(turn(&rt, &ws, start(&format!("thread {}", n3(i)))).await);
        }
        let found = |rt: &Fx, n: usize| {
            let rt = rt.rt.clone();
            async move {
                let found = j(&rt.sessions().search(&format!("marker-{}", n3(n)), None).await.expect("a search"));
                found["hits"]
                    .as_array()
                    .expect("hits")
                    .iter()
                    .map(|h| h["threadId"].as_str().expect("a thread").to_string())
                    .collect::<Vec<_>>()
            }
        };
        // "th" is in every event's keys ("threadId"), so a search over the stored JSON would read every let-go thread back.
        let short_search = |rt: &Fx| {
            let rt = rt.rt.clone();
            let watched = watched.clone();
            async move {
                watched.rows.word_reads.set(0);
                let r = j(&rt.sessions().search("th", None).await.expect("a search"));
                (watched.rows.word_reads.get(), r.get("unsearched").and_then(Value::as_i64))
            }
        };
        let opened = |rt: &Fx, n: usize, ws: String, thread: String| {
            let handle = rt.rt.clone();
            let rt_for_found = rt.rt.clone();
            async move {
                handle.sessions().page(&ws, page_of(&thread), None).await.expect("a page");
                let again = |rt: Rc<wsp_serve::AssembledRuntime>| async move {
                    let found = j(&rt.sessions().search(&format!("marker-{}", n3(n)), None).await.expect("a search"));
                    found["hits"]
                        .as_array()
                        .expect("hits")
                        .iter()
                        .map(|h| h["threadId"].as_str().expect("a thread").to_string())
                        .collect::<Vec<_>>()
                };
                for _ in 0..50 {
                    if !again(rt_for_found.clone()).await.is_empty() {
                        break;
                    }
                    sleep_ms(10).await;
                }
                again(rt_for_found).await
            }
        };
        let (reads, unsearched) = short_search(&rt).await;
        assert_eq!(reads, 0);
        assert!(unsearched.expect("an unsearched count") > 150);
        assert_eq!(found(&rt, 299).await, vec![threads[299].clone()]);
        assert_eq!(found(&rt, 0).await, Vec::<String>::new());
        assert_eq!(opened(&rt, 0, ws.clone(), threads[0].clone()).await, vec![threads[0].clone()]);
        rt.rt.close().await.expect("a close");
        // The index kept beside the rows names each thread's first words and holds none of its lines.
        let d = read_db(&state);
        let index: Value = serde_json::from_str(&index_json(&d, &ws).expect("an index")).expect("json");
        let words = index["words"].as_array().expect("words");
        assert_eq!(words.len(), 300);
        assert!(words.iter().all(|w| w[1]["lines"].as_array().is_some_and(Vec::is_empty)));
        let first_of = words.iter().find(|w| w[0] == threads[0].as_str()).expect("thread 0");
        assert_eq!(first_of[1]["first"], "thread 000");
        let again = runtime(&backend).store(&watched.store()).build();
        // A restart reads the newest threads' words back one thread at a time, off the loop's other work.
        for _ in 0..100 {
            if !found(&again, 299).await.is_empty() {
                break;
            }
            sleep_ms(10).await;
        }
        assert_eq!(found(&again, 299).await, vec![threads[299].clone()]);
        let (reads, unsearched) = short_search(&again).await;
        assert_eq!(reads, 0);
        assert!(unsearched.expect("an unsearched count") > 150);
        assert_eq!(found(&again, 150).await, Vec::<String>::new());
        assert_eq!(opened(&again, 150, ws.clone(), threads[150].clone()).await, vec![threads[150].clone()]);
        again.rt.close().await.expect("a close");
    });
}

// "a runtime whose store keeps transcripts as rows" > "finds its newest words once the person opens it"
#[test]
fn finds_its_newest_words_once_the_person_opens_it() {
    drive(|| async {
        let lines = |prompt: &str| vec![text(format!("{prompt} said {}", "long words ".repeat(110_000)))];
        let state = sqlite_state();
        let backend = StubBackend::new(None);
        let rt = runtime(&backend).store(&state.store).adapter("claude", scripted(lines, None)).build();
        let ws = create_on(&rt.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let x_id = turn(&rt, &ws, start("turn one")).await;
        turn(&rt, &ws, on_thread("turn two zebra", &x_id)).await;
        sleep_ms(400).await;
        page(&rt, &ws, page_of(&x_id)).await;
        sleep_ms(200).await;
        assert_eq!(hits(&rt, "zebra").await, vec![x_id]);
        rt.rt.close().await.expect("a close");
    });
}

// "a runtime whose store keeps transcripts as rows" > "holds the newest words of a thread that alone passes what the host holds and lets its oldest go, across a restart"
#[test]
fn holds_the_newest_words_of_a_thread_that_alone_passes_what_the_host_holds_and_lets_its_oldest_go_across_a_restart() {
    drive(|| async {
        // Each turn's reply is 1.2 M characters, so two turns pass WORDS_HELD and the first turn's words are the ones let go.
        let lines = |prompt: &str| {
            vec![text(if prompt.starts_with("older") {
                "short".to_string()
            } else {
                format!("{prompt} said {}", "long words ".repeat(110_000))
            })]
        };
        let state = sqlite_state();
        let backend = StubBackend::new(None);
        let rt = runtime(&backend).store(&state.store).adapter("claude", scripted(lines, None)).build();
        let ws = create_on(&rt.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        turn(&rt, &ws, start("older thread walrus")).await;
        let x_id = turn(&rt, &ws, start("turn one aardvark")).await;
        turn(&rt, &ws, on_thread("turn two zebra", &x_id)).await;
        sleep_ms(400).await;
        let zebra = search(&rt, "zebra").await;
        assert_eq!(zebra["unsearched"], 1);
        assert_eq!(zebra["hits"].as_array().map(Vec::len), Some(1));
        assert_eq!(zebra["hits"][0]["threadId"], x_id.as_str());
        assert_eq!(search(&rt, "aardvark").await, json!({ "hits": [], "unsearched": 1 }));
        // The older thread holds nothing, so it alone is counted.
        assert_eq!(search(&rt, "walrus").await, json!({ "hits": [], "unsearched": 1 }));
        rt.rt.close().await.expect("a close");
        let again = runtime(&backend).store(&state.store).build();
        for _ in 0..100 {
            if !hits(&again, "zebra").await.is_empty() {
                break;
            }
            sleep_ms(10).await;
        }
        let zebra = search(&again, "zebra").await;
        assert_eq!(zebra["unsearched"], 1);
        assert_eq!(zebra["hits"].as_array().map(Vec::len), Some(1));
        assert_eq!(zebra["hits"][0]["threadId"], x_id.as_str());
        assert_eq!(search(&again, "aardvark").await, json!({ "hits": [], "unsearched": 1 }));
        again.rt.close().await.expect("a close");
    });
}

// "a runtime whose store keeps transcripts as rows" > "names no thread whose agent failed before it started as unsearched after a restart, since it has no words"
#[test]
fn names_no_thread_whose_agent_failed_before_it_started_as_unsearched_after_a_restart_since_it_has_no_words() {
    drive(|| async {
        let state = sqlite_state();
        let backend = StubBackend::new(None);
        let rt = runtime(&backend).store(&state.store).adapter("claude", never_starts()).build();
        let ws = create_on(&rt.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        turn(&rt, &ws, start("fix the login page")).await;
        rt.rt.close().await.expect("a close");
        let again = runtime(&backend).store(&state.store).build();
        assert_eq!(search(&again, "login").await, json!({ "hits": [] }));
        again.rt.close().await.expect("a close");
    });
}

// "a runtime whose store keeps transcripts as rows" > "forgets one thread of 300 off that thread's rows alone, and leaves the other threads' words"
#[test]
fn forgets_one_thread_of_300_off_that_threads_rows_alone_and_leaves_the_other_threads_words() {
    drive(|| async {
        let lines = |prompt: &str| {
            vec![text(format!("{prompt} says marker-{} {}", &prompt[prompt.len().saturating_sub(3)..], "filler words ".repeat(1600)))]
        };
        let state = sqlite_state();
        let watched = Watched::over(&state.store);
        let backend = StubBackend::new(None);
        let rt =
            runtime(&backend).store(&watched.store()).adapter("claude", scripted(lines, None)).adapter("codex", refused_once()).build();
        let ws = create_on(&rt.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        for i in 0..300 {
            turn(&rt, &ws, start(&format!("thread {}", n3(i)))).await;
        }
        let junk =
            rt.rt.sessions().start(&ws, StartOptions { harness: Some("codex".into()), ..start("refused") }, None).await.expect("a start");
        junk.finished().await;
        sleep_ms(300).await;
        assert_eq!(hits(&rt, forgotten_words()).await.len(), 1);
        watched.rows.folder_reads.set(0);
        rt.rt.sessions().forget(&junk.view().thread_id.expect("a thread"), None, None).await.expect("a forget");
        assert_eq!(watched.rows.folder_reads.get(), 0);
        assert!(hits(&rt, forgotten_words()).await.is_empty());
        assert_eq!(hits(&rt, "marker-299").await.len(), 1);
        let thread = junk.view().thread_id.expect("a thread");
        let read = rt
            .rt
            .sessions()
            .thread_history(&ws, HistoryAsk { thread_id: Some(thread), ..Default::default() }, None)
            .await
            .expect("a history");
        assert!(read.is_empty());
        rt.rt.close().await.expect("a close");
    });
}

// "a runtime whose store keeps transcripts as rows" > "keeps a let-go thread's newest words when another folder's flush reads it back before its own folder flushes"
#[test]
fn keeps_a_let_go_threads_newest_words_when_another_folders_flush_reads_it_back_before_its_own_folder_flushes() {
    drive(|| async {
        let held = Gate::new();
        // "big" holds 2,097,135 of the 2,097,152 characters held, so x's 19 are let go.
        let lines = |prompt: &str| match prompt {
            "big" => vec![text(format!("{}zz", "filler ".repeat(299_590)))],
            "x again" => vec![text("fresh-zebra words")],
            other => vec![text(format!("{other} said"))],
        };
        let gate = held.clone();
        let hold: Hold = Rc::new(move |p| (p == "x again").then(|| gate.clone()));
        let state = sqlite_state();
        let backend = StubBackend::new(None);
        let rt = runtime(&backend).store(&state.store).adapter("claude", scripted(lines, Some(hold))).build();
        let a = create_on(&rt.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let b = create_on(&rt.rt, CreateOn::golden("b"), None).await.expect("a workspace").view.id;
        let x_id = turn(&rt, &a, start("x first")).await;
        turn(&rt, &b, start("big")).await;
        let again = rt.rt.sessions().start(&a, on_thread("x again", &x_id), None).await.expect("a start");
        sleep_ms(20).await;
        turn(&rt, &b, start("small")).await;
        held.open();
        again.finished().await;
        sleep_ms(400).await;
        assert_eq!(hits(&rt, "fresh-zebra").await, vec![x_id.clone()]);
        assert_eq!(hits(&rt, "x again").await, vec![x_id]);
        rt.rt.close().await.expect("a close");
    });
}

// "a runtime whose store keeps transcripts as rows" > "reads one thread's every event off that thread alone, never its folder's rows"
#[test]
fn reads_one_threads_every_event_off_that_thread_alone_never_its_folders_rows() {
    drive(|| async {
        let state = sqlite_state();
        let watched = Watched::over(&state.store);
        let backend = StubBackend::new(None);
        let rt = runtime(&backend).store(&watched.store()).adapter("claude", scripted(|p| vec![text(format!("{p} said"))], None)).build();
        let ws = create_on(&rt.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let one = turn(&rt, &ws, start("one")).await;
        turn(&rt, &ws, start("two")).await;
        let folder = history(&rt, &ws).await;
        watched.rows.folder_reads.set(0);
        let read: Vec<Value> = rt
            .rt
            .sessions()
            .thread_history(&ws, HistoryAsk { thread_id: Some(one.clone()), ..Default::default() }, None)
            .await
            .expect("a history")
            .iter()
            .map(j)
            .collect();
        assert_eq!(watched.rows.folder_reads.get(), 0);
        let want: Vec<Value> = folder.into_iter().filter(|e| e["threadId"] == one.as_str()).collect();
        assert_eq!(read, want);
        rt.rt.close().await.expect("a close");
    });
}

// "a runtime whose store keeps transcripts as rows" > "forgets a thread's rows, and search no longer finds its words"
#[test]
fn forgets_a_threads_rows_and_search_no_longer_finds_its_words() {
    drive(|| async {
        let state = sqlite_state();
        let backend = StubBackend::new(None);
        let rt = runtime(&backend)
            .store(&state.store)
            .adapter("claude", scripted(|_| vec![text("stays")], None))
            .adapter("codex", refused_once())
            .build();
        let ws = create_on(&rt.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        turn(&rt, &ws, start("kept words")).await;
        let junk =
            rt.rt.sessions().start(&ws, StartOptions { harness: Some("codex".into()), ..start("refused") }, None).await.expect("a start");
        junk.finished().await;
        let junk_id = junk.view().thread_id.expect("a thread");
        assert_eq!(hits(&rt, forgotten_words()).await.len(), 1);
        rt.rt.sessions().forget(&junk_id, None, None).await.expect("a forget");
        assert!(hits(&rt, forgotten_words()).await.is_empty());
        assert_eq!(hits(&rt, "kept words").await.len(), 1);
        assert!(history(&rt, &ws).await.iter().all(|e| e["threadId"] != junk_id.as_str()));
        rt.rt.close().await.expect("a close");
        let again = runtime(&backend).store(&state.store).build();
        assert!(hits(&again, forgotten_words()).await.is_empty());
        assert!(history(&again, &ws).await.iter().all(|e| e["threadId"] != junk_id.as_str()));
        again.rt.close().await.expect("a close");
    });
}

// "a runtime over rows, where a step does not go through" > "boots past a blob whose rows the table refuses, keeping its bytes aside, and moves the blobs after it"
#[test]
fn boots_past_a_blob_whose_rows_the_table_refuses_keeping_its_bytes_aside_and_moves_the_blobs_after_it() {
    drive(|| async {
        let state = sqlite_state();
        let store = &state.store;
        let twice = serde_json::to_vec(&json!({ "workspaceId": "a", "events": [delta("a", "t", 1), delta("a", "t", 1)] })).expect("json");
        store.put_blob("transcripts", "a", twice.clone()).await.expect("a blob");
        let b_blob = serde_json::to_vec(&json!({ "workspaceId": "b", "events": [delta("b", "t", 1)] })).expect("json");
        store.put_blob("transcripts", "b", b_blob).await.expect("a blob");
        let backend = StubBackend::new(None);
        let rt = runtime(&backend).store(store).build();
        rt.rt.sessions().list(None, None).await.expect("a list");
        rt.rt.close().await.expect("a close");
        assert!(store.get_blob("transcripts", "a").await.expect("a read").is_none());
        assert!(store.get_blob("transcripts", "b").await.expect("a read").is_none());
        let rows = store.transcripts().expect("rows");
        assert!(rows.all("a").expect("rows").events.is_empty());
        let b_events: Vec<Value> = rows.all("b").expect("rows").events.iter().map(j).collect();
        assert_eq!(b_events, vec![delta("b", "t", 1)]);
        let d = read_db(&state);
        let mut stmt = d.prepare("select id, bytes from blobs where collection = 'transcripts-unparsed'").expect("a statement");
        let aside: Vec<(String, Vec<u8>)> =
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).expect("rows").map(|r| r.expect("a row")).collect();
        let named: Vec<(String, String)> = aside
            .iter()
            .map(|(id, bytes)| (id.split('.').next().unwrap_or_default().to_string(), String::from_utf8_lossy(bytes).into_owned()))
            .collect();
        assert_eq!(named, vec![("a".to_string(), String::from_utf8_lossy(&twice).into_owned())]);
        let again = runtime(&backend).store(store).build();
        again.rt.sessions().list(None, None).await.expect("a list");
        again.rt.close().await.expect("a close");
    });
}

// "a runtime over rows, where a step does not go through" > "goes on after the newest row when a workspace's index does not read at boot, and leaves the kept index for the next boot"
#[test]
fn goes_on_after_the_newest_row_when_a_workspaces_index_does_not_read_at_boot_and_leaves_the_kept_index_for_the_next_boot() {
    drive(|| async {
        let state = sqlite_state();
        let backend = StubBackend::new(None);
        let said = |p: &str| vec![text(format!("said {p}"))];
        let rt = runtime(&backend).store(&state.store).adapter("claude", scripted(said, None)).build();
        let ws = create_on(&rt.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let first = turn(&rt, &ws, start("one")).await;
        rt.rt.close().await.expect("a close");
        {
            let d = rusqlite::Connection::open(db_path(&state)).expect("state.db opens");
            d.execute("delete from transcript_index where workspace = ?", [&ws]).expect("a delete");
        }
        let flaky = Watched::over(&state.store);
        flaky.rows.fail_all.set(true);
        let again = runtime(&backend).store(&flaky.store()).adapter("claude", scripted(said, None)).build();
        again.rt.sessions().list(None, None).await.expect("a list");
        flaky.rows.fail_all.set(false);
        turn(&again, &ws, on_thread("two", &first)).await;
        again.rt.close().await.expect("a close");

        let after = runtime(&backend).store(&state.store).build();
        let events = history(&after, &ws).await;
        let words: Vec<String> =
            events.iter().filter(|e| e["type"] == "session.delta").map(|e| e["text"].as_str().unwrap_or_default().to_string()).collect();
        assert_eq!(words, ["said one", "said two"]);
        let positions: Vec<i64> = events.iter().map(|e| e["pos"].as_i64().expect("a pos")).collect();
        assert_eq!(positions, (1..=events.len() as i64).collect::<Vec<_>>());
        assert_eq!(hits(&after, "said one").await.len(), 1);
        after.rt.close().await.expect("a close");
    });
}

// "a runtime over rows, where a step does not go through" > "lets a start's images go when a flush drops the start, not when a read trims a copy of it"
#[test]
fn lets_a_starts_images_go_when_a_flush_drops_the_start_not_when_a_read_trims_a_copy_of_it() {
    drive(|| async {
        let held = Gate::new();
        let lines = |p: &str| {
            count_lines(
                if p == "fill" {
                    4960
                } else if p == "more" {
                    40
                } else {
                    10
                },
                p,
            )
        };
        let gate = held.clone();
        let hold: Hold = Rc::new(move |p| (p == "more").then(|| gate.clone()));
        let state = sqlite_state();
        let backend = StubBackend::new(None);
        let rt = runtime(&backend).store(&state.store).adapter("claude", scripted(lines, Some(hold))).build();
        let ws = create_on(&rt.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
        let bytes = base64::engine::general_purpose::STANDARD.encode(vec![7u8; 64]);
        let image = Attachment { media_type: "image/png".into(), bytes: bytes.clone(), name: None };
        let opts = StartOptions { attachments: Some(vec![image]), request_id: Some("req_a".into()), ..start("first") };
        let thread_id = turn(&rt, &ws, opts).await;
        turn(&rt, &ws, on_thread("fill", &thread_id)).await;
        let more = rt.rt.sessions().start(&ws, on_thread("more", &thread_id), None).await.expect("a start");
        sleep_ms(20).await;
        // The events not yet flushed take the transcript past its count, so the read drops the oldest start from its copy.
        assert!(!history(&rt, &ws).await.iter().any(|e| e["type"] == "session.start" && e["requestId"] == "req_a"));
        sleep_ms(20).await;
        let kept = j(&rt.rt.sessions().attachment(&ws, &thread_id, "req_a", 0, None).await.expect("the attachment"));
        assert_eq!(kept, json!({ "mediaType": "image/png", "bytes": bytes }));
        held.open();
        more.finished().await;
        rt.rt.close().await.expect("a close");
        let again = runtime(&backend).store(&state.store).build();
        let gone = again.rt.sessions().attachment(&ws, &thread_id, "req_a", 0, None).await.expect_err("a refusal");
        assert_eq!(gone.kind, Some(Kind::NotFound));
        again.rt.close().await.expect("a close");
    });
}
