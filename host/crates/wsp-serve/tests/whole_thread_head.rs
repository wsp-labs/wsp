// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/thread-head.test.ts at 65f6b89c7, case for case. A thread's head and its history pages: a
//! click on a tile draws the facts and the tail of one thread, and older events page in by position. The TS runs each
//! `describe.each(STORES)` case on a memory store (transcripts as blobs) and on a sqlite store (rows); a case here is a
//! `mod` with a `blobs` and a `rows` test, the first on `memory_state()` and the second on `sqlite_state()`. The
//! Rust memory store keeps no transcripts, so a `blobs` test that reads history is red at `transcripts.rs:58` by that
//! design. Each test names the TS test above it.
mod common;

use std::rc::Rc;
use std::sync::Arc;

use common::adapter::event;
use common::scripted::{after_a_turn, done_and_end, Plain};
use common::*;
use futures::future::LocalBoxFuture;
use futures::FutureExt as _;
use serde_json::{json, Value};
use wsp_adapters::port::{AdapterError, ExecFailure, HarnessAdapter, HarnessExec, HarnessSession, HarnessStartOptions, SessionRenameWrite};
use wsp_core::types::api::{PageWindow, StartOptions};
use wsp_core::types::harness::HarnessAdapterFactory;
use wsp_core::types::internal::TOOL_RESULT_KEPT;
use wsp_store::store::Store;
use wsp_wire::views::preferences::{HEAD_BYTES, HEAD_RESULT_CHARS, HISTORY_PAGE_BYTES};
use wsp_words::exit::Kind;

/// One delta of a scripted turn: its kind and its text, and on a result what else its adapter read.
#[derive(Clone)]
struct Line {
    kind: &'static str,
    text: String,
    facts: Option<Value>,
}

type Lines = Rc<dyn Fn(&str) -> Vec<Line>>;

fn uuid() -> String {
    format!("{:08x}-0000-4000-8000-{:012x}", rand::random::<u32>(), rand::random::<u64>() & 0xffff_ffff_ffff)
}

/// A harness whose turn writes the lines the test gives it for the prompt, then replies and exits. `hold` keeps the
/// turn open until the test lets it end. It keeps a name of a person's, so a rename lands.
fn scripted(lines: Lines, hold: Option<Rc<Gate>>) -> HarnessAdapterFactory {
    Arc::new(move |_ctx| Ok(Arc::new(Scripted { lines: lines.clone(), hold: hold.clone() }) as Arc<dyn HarnessAdapter>))
}

struct Scripted {
    lines: Lines,
    hold: Option<Rc<Gate>>,
}

impl HarnessAdapter for Scripted {
    fn steers(&self) -> bool {
        false
    }
    fn renames(&self) -> bool {
        true
    }
    fn rename_session(
        &self,
        _id: String,
        _title: String,
        _exec: Arc<dyn HarnessExec>,
    ) -> Option<LocalBoxFuture<'static, Result<SessionRenameWrite, ExecFailure>>> {
        Some(async { Ok(SessionRenameWrite::Written) }.boxed_local())
    }
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        let session = o.resume.clone().unwrap_or_else(uuid);
        let done = json!({ "status": "completed", "text": "done" });
        let on_event = o.on_event.clone().expect("a sink");
        let (lines, hold, prompt, cwd) = (self.lines.clone(), self.hold.clone(), o.prompt.clone(), o.cwd.clone());
        let id = session.clone();
        let finished: LocalBoxFuture<'static, wsp_adapters::port::TurnResult> = async move {
            for _ in 0..3 {
                tokio::task::yield_now().await;
            }
            on_event(event(json!({
                "type": "session.start", "sessionId": id, "cwd": cwd.unwrap_or_else(|| "/root/app".into()), "model": "claude-sonnet-4-5"
            })));
            for (n, l) in lines(&prompt).iter().enumerate() {
                let mut delta = json!({ "type": "turn.delta", "sessionId": id, "kind": l.kind, "text": l.text });
                if l.kind == "text" {
                    delta["messageId"] = json!(format!("m{n}"));
                } else {
                    delta["toolUseId"] = json!(format!("t{n}"));
                    delta["toolName"] = json!("Bash");
                }
                if let (Some(delta), Some(Value::Object(facts))) = (delta.as_object_mut(), &l.facts) {
                    delta.extend(facts.clone());
                }
                on_event(event(delta));
            }
            if let Some(hold) = hold {
                hold.wait().await;
            }
            for e in done_and_end(&id, &done, 0, true) {
                on_event(event(e));
            }
            common::adapter::result(done)
        }
        .boxed_local();
        Ok(Plain::of(&session, finished))
    }
}

/// A harness the agent refused before it announced a session: a thread no turn ran on, the kind forget takes.
fn refused() -> HarnessAdapterFactory {
    let start: common::scripted::StartFn = Rc::new(|o| {
        let local_id = uuid();
        let done = json!({ "status": "failed", "error": "exited with code 1 before emitting a result" });
        let events = done_and_end(&local_id, &done, 1, false);
        Ok(Plain::of(&local_id, after_a_turn(o.on_event.clone().expect("a sink"), events, done)))
    });
    common::scripted::adapter_of(start)
}

fn text(n: usize) -> Line {
    Line { kind: "text", text: format!("line {n}"), facts: None }
}

fn texts(n: usize, word: &str) -> Vec<Line> {
    (0..n).map(|i| Line { kind: "text", text: format!("{word} {i}"), facts: None }).collect()
}

fn lines_of(f: impl Fn(&str) -> Vec<Line> + 'static) -> Lines {
    Rc::new(f)
}

fn start(prompt: &str) -> StartOptions {
    StartOptions { prompt: prompt.into(), ..Default::default() }
}

struct Seeded {
    fx: Fx,
    ws: String,
    backend: Arc<StubBackend>,
}

async fn seeded(store: &Rc<dyn Store>, lines: Lines) -> Seeded {
    let backend = StubBackend::new(None);
    let fx = runtime(&backend).store(store).adapter("claude", scripted(lines, None)).build();
    let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
    Seeded { fx, ws, backend }
}

async fn history(rt: &wsp_serve::AssembledRuntime, ws: &str) -> Vec<Value> {
    rt.sessions().history(ws, None).await.expect("a history").iter().map(j).collect()
}

fn pos_of(e: &Value) -> i64 {
    e["pos"].as_i64().expect("a position")
}

fn byte_len(v: &impl serde::Serialize) -> usize {
    serde_json::to_string(v).expect("JSON").len()
}

/// The two stores a transcript is kept in: blobs by workspace in memory, and rows in the state database.
macro_rules! kept_as {
    ($name:ident, $case:ident) => {
        mod $name {
            use super::*;
            #[test]
            fn blobs() {
                drive(|| async { $case(memory_state()).await });
            }
            #[test]
            fn rows() {
                let state = sqlite_state();
                drive(|| async move { $case(state.store.clone()).await });
            }
        }
    };
}

// "positions on the transcript, kept as %s" > "stamps every recorded event with its place in the workspace's transcript, the same on the bus and in history"
async fn stamps_every_recorded_event(store: Rc<dyn Store>) {
    let s = seeded(&store, lines_of(|_| texts(3, "line"))).await;
    let rt = &*s.fx.rt;
    let heard = s.fx.events();
    let one = rt.sessions().start(&s.ws, start("one"), None).await.expect("a start");
    one.finished().await;
    rt.sessions().start(&s.ws, start("two"), None).await.expect("a start").finished().await;
    let history = history(rt, &s.ws).await;
    let positions: Vec<i64> = history.iter().map(pos_of).collect();
    assert_eq!(positions, (1..=history.len() as i64).collect::<Vec<_>>());
    let live: Vec<(Value, Value)> = heard
        .all()
        .iter()
        .filter(|e| e["type"].as_str().is_some_and(|t| t.starts_with("session.")) && e.get("pos").is_some())
        .map(|e| (e["type"].clone(), e["pos"].clone()))
        .collect();
    let kept: Vec<(Value, Value)> = history.iter().map(|e| (e["type"].clone(), e["pos"].clone())).collect();
    assert_eq!(live, kept);
    rt.close().await.expect("a close");
}
kept_as!(
    stamps_every_recorded_event_with_its_place_in_the_workspaces_transcript_the_same_on_the_bus_and_in_history,
    stamps_every_recorded_event
);

// "never issues a position twice: a restart goes on after the newest, and a forgotten thread's are not issued again"
async fn never_issues_a_position_twice(store: Rc<dyn Store>) {
    let s = seeded(&store, lines_of(|_| texts(2, "line"))).await;
    let rt = &*s.fx.rt;
    let first = rt.sessions().start(&s.ws, start("one"), None).await.expect("a start");
    first.finished().await;
    let thread_id = first.view().thread_id.expect("a thread");
    rt.sessions()
        .start(&s.ws, StartOptions { thread: Some(thread_id.clone()), ..start("two") }, None)
        .await
        .expect("a start")
        .finished()
        .await;
    let before = history(rt, &s.ws).await;
    let newest = pos_of(before.last().expect("an event"));
    rt.close().await.expect("a close");

    let again = runtime(&s.backend)
        .store(&store)
        .adapter("claude", scripted(lines_of(|_| texts(2, "line")), None))
        .adapter("codex", refused())
        .build();
    let rt = &*again.rt;
    rt.sessions().start(&s.ws, start("three"), None).await.expect("a start").finished().await;
    let after = history(rt, &s.ws).await;
    let added = &after[before.len()..];
    let positions: Vec<i64> = added.iter().map(pos_of).collect();
    assert_eq!(positions, (0..added.len() as i64).map(|i| newest + 1 + i).collect::<Vec<_>>());
    // The newest thread goes with its events, and the next event comes after the positions it held.
    let junk = rt.sessions().start(&s.ws, StartOptions { harness: Some("codex".into()), ..start("four") }, None).await.expect("a start");
    junk.finished().await;
    let dropped = history(rt, &s.ws).await.pop().expect("an event");
    assert_eq!(dropped["threadId"], json!(junk.view().thread_id));
    rt.sessions().forget(&junk.view().thread_id.expect("a thread"), None, None).await.expect("a forget");
    assert!(pos_of(&history(rt, &s.ws).await.pop().expect("an event")) < pos_of(&dropped));
    rt.sessions().start(&s.ws, StartOptions { thread: Some(thread_id), ..start("five") }, None).await.expect("a start").finished().await;
    let boundary = pos_of(added.last().expect("an added event"));
    let last: Vec<Value> = history(rt, &s.ws).await.into_iter().filter(|e| pos_of(e) > boundary).collect();
    let positions: Vec<i64> = last.iter().map(pos_of).collect();
    assert_eq!(positions, (0..last.len() as i64).map(|i| pos_of(&dropped) + 1 + i).collect::<Vec<_>>());
    rt.close().await.expect("a close");
}
kept_as!(
    never_issues_a_position_twice_a_restart_goes_on_after_the_newest_and_a_forgotten_threads_are_not_issued_again,
    never_issues_a_position_twice
);

// "positions on a transcript an older build wrote" > "numbers a transcript an older build wrote from one, and records the next event after them"
// The TS strips the `pos` of the blob `transcripts` and the `pos` of the blob `transcript-index` of the memory store.
// The Rust memory store keeps no such blobs and the sqlite store keeps rows, so the case cannot be written here.

// "a thread's head, kept as %s" > "answers the thread's facts and its newest events under the head's bytes, tool results cut and marked"
async fn answers_the_threads_facts_and_its_newest_events(store: Rc<dyn Store>) {
    let long = "x".repeat(HEAD_RESULT_CHARS as usize * 3);
    let busy_lines: Vec<Line> = (0..300)
        .map(|i| match i % 3 {
            2 => Line { kind: "tool_result", text: long.clone(), facts: None },
            1 => Line { kind: "tool_use", text: format!("{{\"command\":\"ls {i}\"}}"), facts: None },
            _ => Line { kind: "text", text: format!("step {i}"), facts: None },
        })
        .collect();
    let s = seeded(&store, lines_of(move |p| if p == "busy" { busy_lines.clone() } else { texts(2, "other") })).await;
    let rt = &*s.fx.rt;
    let busy = rt
        .sessions()
        .start(&s.ws, StartOptions { permission_mode: Some("acceptEdits".into()), ..start("busy") }, None)
        .await
        .expect("a start");
    busy.finished().await;
    rt.sessions().start(&s.ws, start("quiet"), None).await.expect("a start").finished().await;
    let thread_id = busy.view().thread_id.expect("a thread");

    let head = rt.sessions().head(&thread_id, None, None).await.expect("a head");
    assert!(byte_len(&head) < HEAD_BYTES as usize);
    let everything = history(rt, &s.ws).await;
    let all: Vec<&Value> = everything.iter().filter(|e| e["threadId"] == thread_id.as_str()).collect();
    let head = j(&head);
    let events = head["events"].as_array().expect("events").clone();
    assert_eq!(head["total"], json!(all.len()));
    assert_eq!(head["pos"], everything.last().expect("an event")["pos"]);
    // The newest of the thread's events, in order and with no gap, and none of the other thread's.
    assert!(events.len() > 10);
    assert!(events.len() < all.len());
    let want: Vec<&Value> = all[all.len() - events.len()..].iter().map(|e| &e["pos"]).collect();
    assert_eq!(events.iter().map(|e| &e["pos"]).collect::<Vec<_>>(), want);
    assert_eq!(events.last().expect("an event")["type"], "session.end");
    let results: Vec<&Value> = events.iter().filter(|e| e["type"] == "session.delta" && e["kind"] == "tool_result").collect();
    assert!(!results.is_empty());
    for r in results {
        assert_matches(r, json!({ "text": &long[..HEAD_RESULT_CHARS as usize], "cut": long.len() }));
    }
    // What the host holds is not cut.
    let held = all.iter().find(|e| e["type"] == "session.delta" && e["kind"] == "tool_result").expect("a result");
    assert_matches(held, json!({ "text": long }));
    let cwd = busy.view().cwd;
    assert_matches(
        &head["facts"],
        json!({
            "id": thread_id, "workspaceId": s.ws, "harness": "claude", "status": "completed", "title": "busy",
            "model": "claude-sonnet-4-5", "permissionMode": "acceptEdits", "cwd": cwd
        }),
    );
    assert!(head["facts"].get("turnId").is_none());
    rt.close().await.expect("a close");
}
kept_as!(
    answers_the_threads_facts_and_its_newest_events_under_the_heads_bytes_tool_results_cut_and_marked,
    answers_the_threads_facts_and_its_newest_events
);

// "names the running turn while one runs"
async fn names_the_running_turn(store: Rc<dyn Store>) {
    let gate = Gate::new();
    let fx = runtime(&StubBackend::new(None))
        .store(&store)
        .adapter("claude", scripted(lines_of(|_| texts(2, "line")), Some(gate.clone())))
        .build();
    let rt = &*fx.rt;
    let ws = create_on(rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
    let handle = rt.sessions().start(&ws, start("go"), None).await.expect("a start");
    sleep_ms(1).await;
    let head = j(&rt.sessions().head(&handle.view().thread_id.expect("a thread"), None, None).await.expect("a head"));
    assert_matches(&head["facts"], json!({ "status": "running", "turnId": handle.turn_id() }));
    let types: Vec<&Value> = head["events"].as_array().expect("events").iter().map(|e| &e["type"]).collect();
    assert_eq!(types, ["session.start", "session.delta", "session.delta"]);
    gate.open();
    handle.finished().await;
    rt.close().await.expect("a close");
}
kept_as!(names_the_running_turn_while_one_runs, names_the_running_turn);

// "stays under the head's bytes on the wire, multi-byte text and the commas between events counted"
async fn stays_under_the_heads_bytes_on_the_wire(store: Rc<dyn Store>) {
    let wide = lines_of(|_| {
        (0..2000).map(|i| Line { kind: "text", text: format!("{i} ✓ réponse: {}", "日本語".repeat(20)), facts: None }).collect()
    });
    let s = seeded(&store, wide).await;
    let rt = &*s.fx.rt;
    let handle = rt.sessions().start(&s.ws, start("wide"), None).await.expect("a start");
    handle.finished().await;
    let head = rt.sessions().head(&handle.view().thread_id.expect("a thread"), None, None).await.expect("a head");
    // The reply as the socket sends it, with its id and ok beside the head.
    let wire = "{\"id\":123456,\"ok\":true,".len() + byte_len(&head) - 1;
    assert!(wire < HEAD_BYTES as usize, "{wire}");
    assert!(wire as f64 > HEAD_BYTES as f64 * 0.9, "{wire}");
    rt.close().await.expect("a close");
}
kept_as!(
    stays_under_the_heads_bytes_on_the_wire_multi_byte_text_and_the_commas_between_events_counted,
    stays_under_the_heads_bytes_on_the_wire
);

// "is not found for a thread the host holds no row of"
async fn is_not_found_for_a_thread_the_host_holds_no_row_of(store: Rc<dyn Store>) {
    let s = seeded(&store, lines_of(|_| vec![text(0)])).await;
    let rt = &*s.fx.rt;
    let e = match rt.sessions().head("no-such-thread", None, None).await {
        Ok(_) => panic!("a head of no thread"),
        Err(e) => e,
    };
    assert_eq!(e.kind, Some(Kind::NotFound));
    rt.close().await.expect("a close");
}
kept_as!(is_not_found_for_a_thread_the_host_holds_no_row_of, is_not_found_for_a_thread_the_host_holds_no_row_of);

// "goes out on the bus when a turn starts and ends, on a rename and on an access change"
async fn goes_out_on_the_bus(store: Rc<dyn Store>) {
    let s = seeded(&store, lines_of(|_| texts(2, "line"))).await;
    let rt = &*s.fx.rt;
    let heard = s.fx.events();
    let handle = rt.sessions().start(&s.ws, start("go"), None).await.expect("a start");
    handle.finished().await;
    let thread_id = handle.view().thread_id.expect("a thread");
    let heads = || heard.of("thread.head");
    let seen: Vec<(Value, Value)> = heads().iter().map(|h| (h["threadId"].clone(), h["facts"]["status"].clone())).collect();
    assert_eq!(seen, [(json!(thread_id), json!("running")), (json!(thread_id), json!("completed"))]);
    assert_eq!(heads()[0]["facts"]["turnId"], handle.turn_id().as_str());
    assert_eq!(heads()[1]["pos"], history(rt, &s.ws).await.last().expect("an event")["pos"]);

    let renamed = rt.sessions().rename(&handle.view().id, "a better name", None).await.expect("a rename");
    assert_eq!(j(&renamed), json!({ "outcome": "renamed" }));
    assert_matches(
        heads().last().expect("a head"),
        json!({ "type": "thread.head", "workspaceId": s.ws, "threadId": thread_id, "facts": { "title": "a better name" } }),
    );
    let access = rt.sessions().access(&handle.view().id, "acceptEdits", None).await.expect("an access");
    assert_eq!(j(&access), json!({ "outcome": "set" }));
    assert_matches(heads().last().expect("a head"), json!({ "threadId": thread_id, "facts": { "permissionMode": "acceptEdits" } }));
    assert_eq!(heads().len(), 4);
    rt.close().await.expect("a close");
}
kept_as!(goes_out_on_the_bus_when_a_turn_starts_and_ends_on_a_rename_and_on_an_access_change, goes_out_on_the_bus);

fn build_output() -> String {
    (0..2000).map(|i| format!("compiling module {i} of the workspace, step {}", i * 7)).collect::<Vec<_>>().join("\n")
}

/// A rewrite of a 2,000-line file, as twenty hunks of a hundred lines.
fn hunks() -> Vec<Value> {
    (0..20)
        .map(|h| {
            let lines: Vec<String> = (0..100)
                .map(|i| {
                    format!("{}const value{h}_{i} = compute({h}, {i}); // a line long enough to count", if i % 2 == 0 { "-" } else { "+" })
                })
                .collect();
            json!({ "oldStart": h * 100 + 1, "oldLines": 50, "newStart": h * 100 + 1, "newLines": 50, "lines": lines })
        })
        .collect()
}

fn small_patch() -> Value {
    json!([{ "path": "/root/app/f.txt", "hunks": [{ "oldStart": 1, "oldLines": 3, "newStart": 1, "newLines": 3, "lines": [" alpha", "-beta", "+BETA", " gamma"] }] }])
}

fn tool_lines() -> Lines {
    let edit = json!([{ "path": "/root/app/big.ts", "hunks": hunks() }]);
    let build = build_output();
    lines_of(move |_| {
        let mut lines = vec![
            Line { kind: "tool_use", text: "{\"command\":\"npm run build\"}".into(), facts: None },
            Line { kind: "tool_result", text: build.clone(), facts: Some(json!({ "exitCode": 2, "durationMs": 4100 })) },
            Line { kind: "tool_use", text: "{\"command\":\"seq 1 40000\"}".into(), facts: None },
            Line { kind: "tool_result", text: "1\n2\n3".into(), facts: Some(json!({ "bytes": 228894 })) },
            Line { kind: "tool_use", text: "{\"file_path\":\"/root/app/big.ts\"}".into(), facts: None },
            Line { kind: "tool_result", text: "updated".into(), facts: Some(json!({ "patch": edit })) },
            Line { kind: "tool_use", text: "{\"file_path\":\"/root/app/f.txt\"}".into(), facts: None },
            Line { kind: "tool_result", text: "updated".into(), facts: Some(json!({ "patch": small_patch() })) },
        ];
        lines.extend(texts(3, "after"));
        lines
    })
}

fn results(events: &[Value]) -> Vec<Value> {
    events.iter().filter(|e| e["type"] == "session.delta" && e["kind"] == "tool_result").cloned().collect()
}

fn patch_lines(event: &Value) -> Vec<String> {
    event["patch"]
        .as_array()
        .expect("a patch")
        .iter()
        .flat_map(|f| f["hunks"].as_array().expect("hunks").iter())
        .flat_map(|h| h["lines"].as_array().expect("lines").iter().map(|l| l.as_str().unwrap_or_default().to_string()))
        .collect()
}

// "a tool result, kept as %s" > "keeps a command's output to the cap with the bytes of the whole, its exit code and how long it ran"
async fn keeps_a_commands_output_to_the_cap(store: Rc<dyn Store>) {
    let build = build_output();
    assert!(build.len() > TOOL_RESULT_KEPT);
    let s = seeded(&store, tool_lines()).await;
    let rt = &*s.fx.rt;
    rt.sessions().start(&s.ws, start("build"), None).await.expect("a start").finished().await;
    let kept = results(&history(rt, &s.ws).await);
    assert_matches(&kept[0], json!({ "text": &build[..TOOL_RESULT_KEPT], "bytes": build.len(), "exitCode": 2, "durationMs": 4100 }));
    // An output the agent cut first keeps the agent's count of the whole.
    assert_matches(&kept[1], json!({ "text": "1\n2\n3", "bytes": 228894 }));
    rt.close().await.expect("a close");
}
kept_as!(
    keeps_a_commands_output_to_the_cap_with_the_bytes_of_the_whole_its_exit_code_and_how_long_it_ran,
    keeps_a_commands_output_to_the_cap
);

// "keeps an edit's hunks under the same cap, its first lines in order, and says when some were left out"
async fn keeps_an_edits_hunks_under_the_same_cap(store: Rc<dyn Store>) {
    let s = seeded(&store, tool_lines()).await;
    let rt = &*s.fx.rt;
    rt.sessions().start(&s.ws, start("edit"), None).await.expect("a start").finished().await;
    let kept = results(&history(rt, &s.ws).await);
    let (edit, small) = (&kept[2], &kept[3]);
    let hunks_kept = edit["patch"][0]["hunks"].as_array().expect("hunks").len();
    assert!(hunks_kept > 0);
    assert!(hunks_kept < hunks().len());
    let shown = patch_lines(edit);
    let all: Vec<String> = hunks()
        .iter()
        .flat_map(|h| h["lines"].as_array().expect("lines").iter().map(|l| l.as_str().unwrap_or_default().to_string()))
        .collect();
    assert_eq!(shown, all[..shown.len()]);
    assert!(shown.join("").chars().count() <= TOOL_RESULT_KEPT);
    assert_eq!(edit["patchCut"], true);
    assert_matches(small, json!({ "patch": small_patch() }));
    assert!(small.get("patchCut").is_none());
    rt.close().await.expect("a close");
}
kept_as!(
    keeps_an_edits_hunks_under_the_same_cap_its_first_lines_in_order_and_says_when_some_were_left_out,
    keeps_an_edits_hunks_under_the_same_cap
);

// "a head with a 2,000-line output and a 2,000-line edit stays under its bytes, the edit cut to what fits and the small one whole"
async fn a_head_with_a_2000_line_output_and_edit(store: Rc<dyn Store>) {
    let s = seeded(&store, tool_lines()).await;
    let rt = &*s.fx.rt;
    let turn = rt.sessions().start(&s.ws, start("both"), None).await.expect("a start");
    turn.finished().await;
    let head = rt.sessions().head(&turn.view().thread_id.expect("a thread"), None, None).await.expect("a head");
    assert!(byte_len(&head) < HEAD_BYTES as usize);
    let head = j(&head);
    let events = head["events"].as_array().expect("events").clone();
    let kept = results(&events);
    let (build, edit, small) = (&kept[0], &kept[2], &kept[3]);
    assert_matches(build, json!({ "exitCode": 2, "durationMs": 4100, "bytes": build_output().len(), "cut": TOOL_RESULT_KEPT }));
    assert_eq!(build["text"].as_str().expect("text").chars().count(), HEAD_RESULT_CHARS as usize);
    assert_eq!(edit["patchCut"], true);
    let shown = patch_lines(edit).join("").chars().count();
    assert!(shown > 0);
    assert!(shown <= HEAD_RESULT_CHARS as usize);
    assert_matches(small, json!({ "patch": small_patch() }));
    assert!(small.get("patchCut").is_none());
    assert_eq!(events.last().expect("an event")["type"], "session.end");
    rt.close().await.expect("a close");
}
kept_as!(
    a_head_with_a_2000_line_output_and_a_2000_line_edit_stays_under_its_bytes_the_edit_cut_to_what_fits_and_the_small_one_whole,
    a_head_with_a_2000_line_output_and_edit
);

// "a history page, kept as %s" > "pages one thread back by position, newest page first, and the pages put together are the thread"
async fn pages_one_thread_back_by_position(store: Rc<dyn Store>) {
    let s = seeded(&store, lines_of(|p| texts(if p == "long" { 4900 } else { 40 }, p))).await;
    let rt = &*s.fx.rt;
    rt.sessions().start(&s.ws, start("short"), None).await.expect("a start").finished().await;
    let long = rt.sessions().start(&s.ws, start("long"), None).await.expect("a start");
    long.finished().await;
    let thread_id = long.view().thread_id.expect("a thread");
    let whole: Vec<Value> = history(rt, &s.ws).await.into_iter().filter(|e| e["threadId"] == thread_id.as_str()).collect();

    let window = |before: Option<u64>, limit: Option<u64>| PageWindow { thread_id: thread_id.clone(), before, limit };
    let first = rt.sessions().page(&s.ws, window(None, Some(200)), None).await.expect("a page");
    let first_events: Vec<Value> = first.events.iter().map(j).collect();
    assert_eq!(first_events, whole[whole.len() - 200..]);
    assert_eq!(first.total.0 .0 as usize, whole.len());
    assert_eq!(j(&first.pos), whole.last().expect("an event")["pos"]);
    assert!(byte_len(&first) < 500 * 1024);

    let mut pages: Vec<Vec<Value>> = vec![first_events.clone()];
    let mut before = pos_of(&first_events[0]);
    loop {
        let page = rt.sessions().page(&s.ws, window(Some(before as u64), Some(1000)), None).await.expect("a page");
        if page.events.is_empty() {
            break;
        }
        let events: Vec<Value> = page.events.iter().map(j).collect();
        before = pos_of(&events[0]);
        pages.insert(0, events);
    }
    assert_eq!(pages.concat(), whole);
    // The default page.
    assert_eq!(rt.sessions().page(&s.ws, window(None, None), None).await.expect("a page").events.len(), 200);
    rt.close().await.expect("a close");
}
kept_as!(pages_one_thread_back_by_position_newest_page_first_and_the_pages_put_together_are_the_thread, pages_one_thread_back_by_position);

// "stops at the page's bytes past its first event"
async fn stops_at_the_pages_bytes(store: Rc<dyn Store>) {
    let big = "y".repeat(12 * 1024);
    let s =
        seeded(&store, lines_of(move |_| (0..100).map(|_| Line { kind: "tool_result", text: big.clone(), facts: None }).collect())).await;
    let rt = &*s.fx.rt;
    let handle = rt.sessions().start(&s.ws, start("big"), None).await.expect("a start");
    handle.finished().await;
    let window = PageWindow { thread_id: handle.view().thread_id.expect("a thread"), before: None, limit: Some(200) };
    let page = rt.sessions().page(&s.ws, window, None).await.expect("a page");
    assert!(page.events.len() < 100);
    assert!(byte_len(&page.events) <= HISTORY_PAGE_BYTES as usize + 2);
    assert_eq!(j(page.events.last().expect("an event"))["type"], "session.end");
    rt.close().await.expect("a close");
}
kept_as!(stops_at_the_pages_bytes_past_its_first_event, stops_at_the_pages_bytes);

// "answers a thread the caller reaches no row of with nothing"
async fn answers_a_thread_the_caller_reaches_no_row_of_with_nothing(store: Rc<dyn Store>) {
    let s = seeded(&store, lines_of(|_| vec![text(0)])).await;
    let rt = &*s.fx.rt;
    rt.sessions().start(&s.ws, start("one"), None).await.expect("a start").finished().await;
    let window = PageWindow { thread_id: "elsewhere".into(), before: None, limit: None };
    let page = rt.sessions().page(&s.ws, window, None).await.expect("a page");
    assert_matches(&j(&page), json!({ "events": [], "total": 0 }));
    rt.close().await.expect("a close");
}
kept_as!(answers_a_thread_the_caller_reaches_no_row_of_with_nothing, answers_a_thread_the_caller_reaches_no_row_of_with_nothing);
