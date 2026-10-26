use std::cell::Cell;

use std::rc::Rc;

use serde_json::{json, Value};

use support::transcripts_ctx::{FakeTranscriptsCtx, WithState};

use support::{err, event, json_of, local, row, sessions_of, settle, view};

use wsp_core::console;

use wsp_core::types::events::EventBus;

use wsp_core::types::internal::{index_bytes, index_of, thread_words_of, SessionFacts, ThreadWords, TranscriptIndex};

use wsp_store::sqlite_transcripts::EventSize;

use wsp_store::transcript_reader::{event_bytes, ThreadRead, TranscriptReader};

use wsp_threads::threads::transcripts::rows::{
    drop_oldest, drop_oldest_of, kept_event, mark_trimmed, Trimmed, PENDING_FLUSH_BYTES as PENDING_FLUSH_USIZE, TOOL_RESULT_KEPT,
    TRANSCRIPT_BYTES as TRANSCRIPT_USIZE, TRANSCRIPT_CAP, TRANSCRIPT_FLUSH_MS, WORDS_HELD,
};

use wsp_wire::types::js_number::JsNumber;

use wsp_wire::views::session_events_more::SessionEvent;

use wsp_wire::wire::events_union::EventUnion;

use wsp_words::exit::HostError;

const WS: &str = "ws1";

const TRANSCRIPT_BYTES: u64 = TRANSCRIPT_USIZE as u64;

const PENDING_FLUSH_BYTES: u64 = PENDING_FLUSH_USIZE as u64;

// ---- events ------------------------------------------------------------------------------------------------------

fn start_v(ws: &str, thread: &str, turn: &str, prompt: &str) -> Value {
    json!({ "type": "session.start", "workspaceId": ws, "sessionId": "s1", "turnId": turn, "threadId": thread, "prompt": prompt })
}

fn delta_v(ws: &str, thread: &str, turn: &str, text: &str) -> Value {
    json!({ "type": "session.delta", "workspaceId": ws, "sessionId": "s1", "turnId": turn, "threadId": thread, "kind": "text", "text": text })
}

fn end_v(ws: &str, thread: &str, turn: &str) -> Value {
    json!({ "type": "session.end", "workspaceId": ws, "sessionId": "s1", "turnId": turn, "threadId": thread, "exitCode": 0, "sawResult": true })
}

fn plan_v(ws: &str, thread: &str, text: &str) -> Value {
    json!({ "type": "session.plan", "workspaceId": ws, "sessionId": "s1", "threadId": thread, "text": text })
}

fn stamped(mut v: Value, pos: i64, at: i64) -> SessionEvent {
    v["pos"] = json!(pos);
    v["at"] = json!(at);
    event(v)
}

fn pos_of(e: &SessionEvent) -> i64 {
    json_of(e)["pos"].as_f64().expect("a pos") as i64
}

fn field(e: &SessionEvent, key: &str) -> Value {
    json_of(e)[key].clone()
}

fn num(v: &Value) -> f64 {
    v.as_f64().expect("a number")
}

/// A text delta whose size under `event_bytes` is exactly `bytes`.
fn sized_delta(ws: &str, thread: &str, pos: i64, bytes: u64) -> SessionEvent {
    let make = |n: usize| stamped(delta_v(ws, thread, "a", &"x".repeat(n)), pos, 5);
    let base = event_bytes(&make(0));
    let e = make((bytes - base) as usize);
    assert_eq!(event_bytes(&e), bytes);
    e
}

/// Rows `1..=count` of one thread on `WS`: `first` at 1, text deltas of turn `a` between, an end at `count`; the held
/// index has issued up to `count`.
fn seed_thread(ctx: &FakeTranscriptsCtx, thread: &str, count: i64, first: Value) {
    seed_thread_second(ctx, thread, count, first, None);
}

fn seed_thread_second(ctx: &FakeTranscriptsCtx, thread: &str, count: i64, first: Value, second: Option<Value>) {
    let mut events = Vec::with_capacity(count as usize);
    events.push(stamped(first, 1, 10));
    for pos in 2..count {
        let v = match (&second, pos) {
            (Some(v), 2) => v.clone(),
            _ => delta_v(WS, thread, "a", &format!("d{pos}")),
        };
        events.push(stamped(v, pos, 10));
    }
    events.push(stamped(end_v(WS, thread, "a"), count, 10));
    ctx.rows.seed(WS, events);
    ctx.hold_index(WS, TranscriptIndex { pos: count as u64, ..TranscriptIndex::default() });
}

fn plain_start(thread: &str) -> Value {
    start_v(WS, thread, "a", "first")
}

fn words(lines: &[&str], last: i64) -> ThreadWords {
    let size = lines.iter().map(|l| l.chars().count()).sum();
    ThreadWords {
        lines: lines.iter().map(|l| l.to_string()).collect(),
        open: None,
        last: JsNumber(last as f64),
        size,
        first: lines.first().map(|l| l.chars().take(200).collect()),
        opened: None,
        cold: None,
        room: None,
    }
}

fn cold(first: &str, size: usize, last: i64) -> ThreadWords {
    ThreadWords {
        lines: vec![],
        open: None,
        last: JsNumber(last as f64),
        size,
        first: Some(first.to_string()),
        opened: None,
        cold: Some(true),
        room: Some(0),
    }
}

fn index_of_words(rows: Vec<(&str, ThreadWords)>) -> TranscriptIndex {
    let mut index = TranscriptIndex::default();
    for (thread, w) in rows {
        index.words.insert(thread.to_string(), w);
    }
    index
}

fn read_all() -> ThreadRead {
    ThreadRead { before: None, limit: usize::MAX, bytes: 1 << 40, strict: false, shape: None }
}

// ---- the kept form of an event -----------------------------------------------------------------------------------

fn delta_with(extra: Value) -> SessionEvent {
    let mut v = json!({ "type": "session.delta", "workspaceId": WS, "sessionId": "s1", "threadId": "t1", "kind": "text", "text": "" });
    for (k, x) in extra.as_object().expect("an object").iter() {
        v[k] = x.clone();
    }
    event(v)
}

fn hunk(first: i64, lines: Vec<String>) -> Value {
    json!({ "oldStart": first, "oldLines": 50, "newStart": first, "newLines": 50, "lines": lines })
}

fn hundred_char_lines() -> Vec<String> {
    (0..100).map(|i| format!("{}{}", if i % 2 == 0 { "-" } else { "+" }, "x".repeat(99))).collect()
}

fn images_seeded(ctx: &FakeTranscriptsCtx) {
    ctx.store.seed(
        "attachment-keys",
        WS,
        json!({ "threads": { "t1": [{ "key": "t1.req_a.0", "mediaType": "image/png" }], "t2": [{ "key": "t2.req_z.0", "mediaType": "image/png" }] } }),
    );
    ctx.store.seed_blob("attachments", "t1.req_a.0", &[5, 5]);
    ctx.store.seed_blob("attachments", "t2.req_z.0", &[6, 6]);
}

fn start_with_image() -> Value {
    let mut v = start_v(WS, "t1", "a", "look at this");
    v["attachments"] = json!([{ "mediaType": "image/png", "bytes": 2 }]);
    v["requestId"] = json!("req_a");
    v
}

fn push_past_the_cap(area: &wsp_threads::threads::transcripts::TranscriptsArea) {
    area.record(event(start_v(WS, "t1", "b", "second")));
    area.record(event(delta_v(WS, "t1", "b", "five thousand and one")));
    area.record(event(delta_v(WS, "t1", "b", "five thousand and two")));
    area.record(event(end_v(WS, "t1", "b")));
}

// ---- dropping a thread -------------------------------------------------------------------------------------------

fn session_start(session: &str, thread: &str, model: &str, copied_from: Option<&str>, pos: i64) -> SessionEvent {
    let mut v = json!({ "type": "session.start", "workspaceId": WS, "sessionId": session, "turnId": format!("u-{session}"), "threadId": thread, "prompt": format!("prompt of {thread}"), "model": model });
    if let Some(from) = copied_from {
        v["copiedFrom"] = json!(from);
    }
    stamped(v, pos, 10)
}

// ---- the caps ----------------------------------------------------------------------------------------------------

fn text_delta(thread: Option<&str>, text: &str, pos: i64) -> SessionEvent {
    let mut v = json!({ "type": "session.delta", "workspaceId": WS, "sessionId": "s1", "kind": "text", "text": text, "pos": pos });
    if let Some(t) = thread {
        v["threadId"] = json!(t);
    }
    event(v)
}

fn text_of(e: &SessionEvent) -> String {
    let v = json_of(e);
    v["text"].as_str().or_else(|| v["prompt"].as_str()).unwrap_or("").to_string()
}

fn trim(events: &mut Vec<SessionEvent>) -> (Vec<SessionEvent>, u64) {
    let mut gone = Vec::new();
    let left = drop_oldest(events, None, &mut |e| gone.push(e.clone()));
    (gone, left)
}

fn total(events: &[SessionEvent]) -> u64 {
    events.iter().map(event_bytes).sum()
}

// ---- held words --------------------------------------------------------------------------------------------------

fn line_of(c: char, n: usize) -> String {
    c.to_string().repeat(n)
}

fn warm_ctx() -> (Rc<FakeTranscriptsCtx>, ThreadWords, ThreadWords) {
    let ctx = FakeTranscriptsCtx::new();
    ctx.rows.seed(WS, vec![stamped(start_v(WS, "a", "u1", "alpha words"), 1, 20), stamped(start_v(WS, "b", "u2", "beta words"), 2, 20)]);
    let (a, b) = (cold("alpha", 100, 300), cold("beta", 100, 200));
    let mut index = index_of_words(vec![("a", a.clone()), ("b", b.clone())]);
    index.pos = 2;
    ctx.hold_index(WS, index);
    (ctx, a, b)
}

fn warmed(first: &str, line: &str, last: i64) -> ThreadWords {
    ThreadWords {
        lines: vec![line.to_string()],
        open: None,
        last: JsNumber(last as f64),
        size: line.chars().count(),
        first: Some(first.to_string()),
        opened: None,
        cold: None,
        room: None,
    }
}
