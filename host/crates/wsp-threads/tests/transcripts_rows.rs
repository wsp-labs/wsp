// SPDX-License-Identifier: AGPL-3.0-only
//! transcripts_rows (spec/transcripts_rows.md): record, flush, caps, reads and held words. Expected values read off
//! transcripts.ts at 65f6b89c7 and the TS tests named above each test.
mod support;

use std::cell::Cell;
use std::rc::Rc;

use serde_json::{json, Value};
use support::transcripts_ctx::FakeTranscriptsCtx;
use support::{err, event, json_of, local, row, sessions_of, settle, view};
use wsp_core::console;
use wsp_core::types::events::EventBus;
use wsp_core::types::internal::{index_bytes, index_of, thread_words_of, SessionFacts, ThreadWords, TranscriptIndex};
use wsp_store::sqlite_transcripts::EventSize;
use wsp_store::transcript_reader::{event_bytes, ThreadRead, TranscriptReader};
use wsp_threads::threads::transcripts::rows::{drop_oldest, drop_oldest_of, kept_event, mark_trimmed, Trimmed, PENDING_FLUSH_BYTES, TOOL_RESULT_KEPT, TRANSCRIPT_BYTES, TRANSCRIPT_CAP, TRANSCRIPT_FLUSH_MS, WORDS_HELD};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::session_events_more::SessionEvent;
use wsp_wire::wire::events_union::EventUnion;
use wsp_words::exit::HostError;

const WS: &str = "ws1";

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
    ThreadWords { lines: lines.iter().map(|l| l.to_string()).collect(), open: None, last: JsNumber(last as f64), size, first: lines.first().map(|l| l.chars().take(200).collect()), opened: None, cold: None, room: None }
}

fn cold(first: &str, size: usize, last: i64) -> ThreadWords {
    ThreadWords { lines: vec![], open: None, last: JsNumber(last as f64), size, first: Some(first.to_string()), opened: None, cold: Some(true), room: Some(0) }
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

// ---- recording ---------------------------------------------------------------------------------------------------

// TS: thread-head.test.ts:85
// TS: serve-replay.test.ts:234
#[test]
fn record_stamps_at_and_pos_after_the_events_keys_then_emits_and_pushes_the_head() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        let area = ctx.area();
        let calls = ctx.clone();
        let _heard = support::BusHandle(ctx.bus.clone()).on(
            "*",
            Rc::new(move |e: &EventUnion| {
                calls.call(format!("emit {}", json_of(e)["type"].as_str().unwrap_or("")));
                Ok::<(), HostError>(())
            }),
        );

        area.record(event(start_v(WS, "t1", "u1", "go")));
        let first = event(json!({ "type": "session.start", "workspaceId": WS, "sessionId": "s1", "turnId": "u1", "threadId": "t1", "prompt": "go", "at": 1_000_000, "pos": 1 }));
        assert_eq!(ctx.pending_of(WS), vec![first.clone()]);
        assert_eq!(ctx.emitted_json(), vec![json_of(&first)], "the bus is handed the stamped event");
        assert!(ctx.pending_json(WS)[0].get("seq").is_none(), "the transcript never carries a sequence");
        assert_eq!(ctx.pending_bytes_of(WS), Some(event_bytes(&first)));
        assert_eq!(ctx.index_of(WS).expect("an index").pos, 1);
        assert_eq!(ctx.called(), vec!["emit session.start", "pushHead t1"], "emitted first, then the head pushed");

        area.record(event(delta_v(WS, "t1", "u1", "reply")));
        ctx.clock.advance(5);
        area.record(event(end_v(WS, "t1", "u1")));
        let pending = ctx.pending_of(WS);
        assert_eq!(pending.len(), 3);
        assert_eq!((pos_of(&pending[1]), num(&field(&pending[1], "at"))), (2, 1_000_000.0));
        assert_eq!((pos_of(&pending[2]), num(&field(&pending[2], "at"))), (3, 1_000_005.0));
        assert_eq!(*ctx.pushed.borrow(), vec!["t1", "t1"], "a start and an end push the head, a delta does not");

        area.record(event(json!({ "type": "session.start", "workspaceId": WS, "sessionId": "s2", "prompt": "no thread" })));
        area.record(event(json!({ "type": "session.end", "workspaceId": WS, "sessionId": "s2", "exitCode": 0, "sawResult": true })));
        assert_eq!(*ctx.pushed.borrow(), vec!["t1", "t1"], "an event with no thread pushes no head");
        assert_eq!(ctx.index_of(WS).expect("an index").pos, 5);
    });
}

#[test]
fn record_keeps_an_earlier_rows_own_time() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        let area = ctx.area();
        let earlier = |who: &str, text: &str, at: Option<i64>| {
            let mut v = json!({ "type": "session.earlier", "workspaceId": WS, "sessionId": "s1", "threadId": "t1", "who": who, "text": text });
            if let Some(at) = at {
                v["at"] = json!(at);
            }
            event(v)
        };
        area.record(earlier("person", "hi", Some(555)));
        area.record(earlier("agent", "hello", None));
        let mut carried = delta_v(WS, "t1", "u1", "late");
        carried["at"] = json!(7);
        area.record(event(carried));
        let pending = ctx.pending_of(WS);
        assert_eq!(num(&field(&pending[0], "at")), 555.0, "an earlier row keeps the time its agent wrote it");
        assert_eq!(num(&field(&pending[1], "at")), 1_000_000.0, "an earlier row with none takes now");
        assert_eq!(num(&field(&pending[2], "at")), 1_000_000.0, "any other row takes now whatever it carried");
    });
}

#[test]
fn record_keeps_the_place_of_a_pos_the_event_carried() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        ctx.hold_index(WS, TranscriptIndex { pos: 7, ..TranscriptIndex::default() });
        let area = ctx.area();
        let mut carried = start_v(WS, "t1", "u1", "go");
        carried["pos"] = json!(99);
        carried["at"] = json!(3);
        area.record(event(carried));
        let stored = ctx.pending_json(WS).remove(0);
        assert_eq!(num(&stored["pos"]), 8.0, "the position is the index's next, not the one the event carried");
        assert_eq!(num(&stored["at"]), 1_000_000.0);
        let keys: Vec<&String> = stored.as_object().expect("an object").keys().collect();
        assert_eq!(keys.iter().filter(|k| k.as_str() == "pos").count(), 1);
        assert_eq!(keys.iter().filter(|k| k.as_str() == "at").count(), 1);
        assert_eq!(ctx.index_of(WS).expect("an index").pos, 8);
    });
}

// TS: thread-head.test.ts:99
#[test]
fn invariant_2_pos_is_never_issued_twice_across_record_and_copies() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        let area = ctx.area();
        let copy = |thread: &str, turn: &str, text: &str, pos: i64, at: Option<i64>| {
            let mut v = delta_v(WS, thread, turn, text);
            v["copiedFrom"] = json!("t0");
            v["pos"] = json!(pos);
            if let Some(at) = at {
                v["at"] = json!(at);
            }
            event(v)
        };
        area.record(event(start_v(WS, "t1", "u1", "go")));
        area.record(event(delta_v(WS, "t1", "u1", "reply")));
        area.record_copies(&[copy("t2", "u0", "old one", 40, Some(77)), copy("t2", "u0", "old two", 41, None)]);
        area.record(event(end_v(WS, "t1", "u1")));
        let positions: Vec<i64> = ctx.pending_of(WS).iter().map(pos_of).collect();
        assert_eq!(positions, vec![1, 2, 3, 4, 5], "each event takes the next position, a copy's own replaced");
        area.flush_transcript(WS).await;
        assert_eq!(ctx.rows.positions(WS), vec![1, 2, 3, 4, 5], "every row written once");
        assert!(ctx.pending_of(WS).is_empty());
        area.record_copies(&[copy("t2", "u0", "old three", 2, None)]);
        assert_eq!(ctx.pending_of(WS).iter().map(pos_of).collect::<Vec<_>>(), vec![6]);
        area.flush_transcript(WS).await;
        assert_eq!(ctx.rows.positions(WS), vec![1, 2, 3, 4, 5, 6]);
    });
}

#[test]
fn record_copies_emits_nothing() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        let area = ctx.area();
        let mut first = delta_v(WS, "t2", "u0", "old one");
        first["copiedFrom"] = json!("t0");
        first["pos"] = json!(40);
        first["at"] = json!(77);
        let mut second = start_v(WS, "t2", "u0", "old prompt");
        second["copiedFrom"] = json!("t0");
        area.record_copies(&[event(first), event(second)]);
        let pending = ctx.pending_json(WS);
        assert_eq!(pending.len(), 2);
        assert_eq!((num(&pending[0]["pos"]), num(&pending[0]["at"])), (1.0, 77.0), "the copy keeps the moment it was first written");
        assert_eq!(num(&pending[1]["pos"]), 2.0);
        assert!(pending[1].get("at").is_none(), "a copy with no time is given none");
        assert!(ctx.bus.emitted.borrow().is_empty(), "no client is told of a copy");
        assert!(ctx.pushed.borrow().is_empty(), "no head is pushed for a copy");
    });
}

// TS: runtime-history.test.ts:1061
#[test]
fn record_hands_the_bus_the_whole_text_and_keeps_the_clipped_one() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        let area = ctx.area();
        let output = format!("first line\n{}", "r".repeat(TOOL_RESULT_KEPT * 4));
        let mut result = delta_v(WS, "t1", "u1", &output);
        result["kind"] = json!("tool_result");
        result["toolUseId"] = json!("tu_1");
        area.record(event(result));
        assert_eq!(ctx.emitted_json()[0]["text"], json!(output), "the live stream gets all of it");
        let kept = ctx.pending_of(WS).remove(0);
        assert_eq!(field(&kept, "text"), json!(output.chars().take(TOOL_RESULT_KEPT).collect::<String>()));
        assert_eq!(num(&field(&kept, "bytes")), output.len() as f64);
        assert_eq!(ctx.pending_bytes_of(WS), Some(event_bytes(&kept)), "pending counts the kept form");
        area.flush_transcript(WS).await;
        assert_eq!(field(&ctx.rows.kept(WS)[0], "text"), field(&kept, "text"), "the rows hold the clipped one");
    });
}

// TS: thread-head.test.ts:99
#[test]
fn positions_go_on_after_the_newest_row_across_a_restart_and_a_forget() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        ctx.rows.seed(WS, (1..=4).map(|p| stamped(delta_v(WS, "t1", "a", &format!("line {p}")), p, 10)).collect());
        let area = ctx.area();
        area.load_index(WS);
        area.record(event(start_v(WS, "t1", "b", "after the restart")));
        assert_eq!(pos_of(&ctx.pending_of(WS)[0]), 5, "a restart goes on after the newest row");
        area.record(event(start_v(WS, "junk", "j1", "refused")));
        area.record(event(end_v(WS, "junk", "j1")));
        area.flush_transcript(WS).await;
        assert_eq!(ctx.rows.positions(WS), vec![1, 2, 3, 4, 5, 6, 7]);
        area.drop_from_transcript(WS, "junk", None).await.expect("the forget lands");
        assert_eq!(ctx.rows.positions(WS), vec![1, 2, 3, 4, 5]);
        area.record(event(delta_v(WS, "t1", "b", "next")));
        assert_eq!(pos_of(&ctx.pending_of(WS)[0]), 8, "a forgotten thread's positions are not issued again");
    });
}

// ---- appending ---------------------------------------------------------------------------------------------------

// TS: runtime-history.test.ts:842
#[test]
fn append_flushes_at_an_end_and_past_a_mebibyte() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        let area = ctx.area();
        area.record(event(start_v(WS, "t1", "u1", "go")));
        assert_eq!(ctx.clock.pending(), 1, "a start arms the debounce");
        area.record(event(end_v(WS, "t1", "u1")));
        settle().await;
        assert_eq!(ctx.rows.positions(WS), vec![1, 2], "a session end lands at once, without the debounce");
        assert!(ctx.pending_of(WS).is_empty());
        assert_eq!(ctx.pending_bytes_of(WS), None);
        assert_eq!(ctx.clock.pending(), 0, "the flush cancelled the timer");
        assert!(area.transcript_timers.borrow().is_empty());

        let other = "ws2";
        area.append(sized_delta(other, "t9", 1, PENDING_FLUSH_BYTES));
        settle().await;
        assert!(ctx.rows.kept(other).is_empty(), "exactly a mebibyte waits: it must be strictly over");
        assert_eq!(ctx.pending_bytes_of(other), Some(PENDING_FLUSH_BYTES));
        area.append(sized_delta(other, "t9", 2, 200));
        settle().await;
        assert_eq!(ctx.rows.positions(other), vec![1, 2], "past a mebibyte flushes at once");
        assert!(ctx.pending_of(other).is_empty());
        assert_eq!(ctx.clock.pending(), 0, "a delta armed no timer");
    });
}

// TS: runtime-history.test.ts:852
#[test]
fn append_arms_one_timer_per_workspace_for_a_non_delta() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        let area = ctx.area();
        area.record(event(start_v(WS, "t1", "u1", "go")));
        area.record(event(plan_v(WS, "t1", "step one")));
        assert_eq!(ctx.clock.pending(), 1, "a later event does not arm a second timer");
        area.record(event(start_v("ws2", "t2", "u2", "go")));
        assert_eq!(ctx.clock.pending(), 2);
        assert_eq!(area.transcript_timers.borrow().keys().cloned().collect::<Vec<_>>(), vec![WS, "ws2"]);

        ctx.clock.advance(TRANSCRIPT_FLUSH_MS - 1);
        settle().await;
        assert!(ctx.rows.kept(WS).is_empty(), "the timer is 250 ms from the first event, not moved by the next");
        ctx.clock.advance(1);
        settle().await;
        assert_eq!(ctx.rows.positions(WS), vec![1, 2]);
        assert_eq!(ctx.rows.positions("ws2"), vec![1]);
        assert_eq!(ctx.clock.pending(), 0);
        assert!(area.transcript_timers.borrow().is_empty());

        area.record(event(plan_v(WS, "t1", "step two")));
        assert_eq!(ctx.clock.pending(), 1, "the next event arms a fresh timer");
    });
}

#[test]
fn a_delta_alone_arms_no_timer() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        let area = ctx.area();
        area.record(event(delta_v(WS, "t1", "u1", "partial")));
        assert_eq!(ctx.clock.pending(), 0);
        assert!(area.transcript_timers.borrow().is_empty());
        assert_eq!(ctx.pending_of(WS).len(), 1);
        ctx.clock.advance(TRANSCRIPT_FLUSH_MS * 4);
        settle().await;
        assert!(ctx.rows.kept(WS).is_empty(), "nothing flushes a delta that waits alone");

        area.record(event(plan_v(WS, "t1", "step")));
        assert_eq!(ctx.clock.pending(), 1);
        ctx.clock.advance(TRANSCRIPT_FLUSH_MS);
        settle().await;
        assert_eq!(ctx.rows.positions(WS), vec![1, 2], "the delta rides the next flush");
    });
}

#[test]
fn append_warms_cold_words_before_the_event_is_pending() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        ctx.rows.seed(WS, vec![stamped(start_v(WS, "t1", "u1", "hello earlier"), 1, 10)]);
        let mut index = index_of_words(vec![("t1", cold("hello earlier", 13, 5))]);
        index.pos = 1;
        ctx.hold_index(WS, index);
        let area = ctx.area();
        area.record(event(delta_v(WS, "t1", "u1", "more")));
        assert_eq!(ctx.rows_called_of(&["word_events"]), vec!["word_events ws1 t1"], "the words are read back first");
        let held = ctx.index_of(WS).expect("an index").words.get("t1").cloned().expect("t1's words");
        assert_eq!(held.lines, vec!["hello earlier", "more"], "read back once, then the event folded once");
        assert_eq!(held.cold, None, "the thread is warm again");
        assert_eq!(held.size, 17);
        assert_eq!(held.first.as_deref(), Some("hello earlier"));
        assert_eq!(held.last, JsNumber(1_000_000.0));
        assert_eq!(held.open.as_deref(), Some("u1:"));
    });
}

// ---- the kept form of an event -----------------------------------------------------------------------------------

fn delta_with(extra: Value) -> SessionEvent {
    let mut v = json!({ "type": "session.delta", "workspaceId": WS, "sessionId": "s1", "threadId": "t1", "kind": "text", "text": "" });
    for (k, x) in extra.as_object().expect("an object").iter() {
        v[k] = x.clone();
    }
    event(v)
}

// TS: runtime-history.test.ts:1061
// TS: thread-head.test.ts:258
#[test]
fn kept_event_clips_a_long_tool_result_with_its_bytes() {
    let output = format!("first line\n{}", "r".repeat(TOOL_RESULT_KEPT * 4));
    let long = delta_with(json!({ "kind": "tool_result", "text": output, "toolUseId": "tu_1", "exitCode": 2, "durationMs": 4100 }));
    let clipped = kept_event(&long);
    assert_eq!(
        clipped,
        delta_with(json!({ "kind": "tool_result", "text": output.chars().take(TOOL_RESULT_KEPT).collect::<String>(), "bytes": output.len(), "toolUseId": "tu_1", "exitCode": 2, "durationMs": 4100 })),
        "the text cut at the cap, the bytes of the whole, the exit code and the time kept"
    );
    let agents_own = delta_with(json!({ "kind": "tool_result", "text": "r".repeat(20_000), "bytes": 228_894 }));
    let kept = kept_event(&agents_own);
    assert_eq!(field(&kept, "text"), json!("r".repeat(TOOL_RESULT_KEPT)));
    assert_eq!(num(&field(&kept, "bytes")), 228_894.0, "an output the agent counted first keeps the agent's count");

    let wide = "é".repeat(20_000);
    let kept = kept_event(&delta_with(json!({ "kind": "tool_result", "text": wide })));
    assert_eq!(field(&kept, "text"), json!("é".repeat(TOOL_RESULT_KEPT)), "the cut is in UTF-16 units");
    assert_eq!(num(&field(&kept, "bytes")), 40_000.0, "the bytes are UTF-8");

    let straddling = format!("{}😀tail", "a".repeat(TOOL_RESULT_KEPT - 1));
    let kept = kept_event(&delta_with(json!({ "kind": "tool_result", "text": straddling })));
    assert_eq!(field(&kept, "text"), json!("a".repeat(TOOL_RESULT_KEPT - 1)), "a cut inside a surrogate pair drops the lone half");
    assert_eq!(num(&field(&kept, "bytes")), (TOOL_RESULT_KEPT - 1 + 4 + 4) as f64);

    let tool_use = delta_with(json!({ "kind": "tool_use", "text": "w".repeat(100_000), "toolName": "Write" }));
    assert_eq!(kept_event(&tool_use), tool_use, "a call's input is kept whole");
}

// TS: subagents.test.ts:408
#[test]
fn kept_event_clips_a_subagents_text_without_bytes() {
    let long = "y".repeat(TOOL_RESULT_KEPT + 500);
    let thinking = kept_event(&delta_with(json!({ "kind": "thinking", "text": long, "parentToolUseId": "toolu_a1" })));
    assert_eq!(thinking, delta_with(json!({ "kind": "thinking", "text": "y".repeat(TOOL_RESULT_KEPT), "parentToolUseId": "toolu_a1" })));
    assert!(json_of(&thinking).get("bytes").is_none());
    let text = kept_event(&delta_with(json!({ "kind": "text", "text": long, "parentToolUseId": "toolu_a1" })));
    assert_eq!(text, delta_with(json!({ "kind": "text", "text": "y".repeat(TOOL_RESULT_KEPT), "parentToolUseId": "toolu_a1" })));
    let own = delta_with(json!({ "kind": "text", "text": long }));
    assert_eq!(kept_event(&own), own, "the lead's own words are clipped nowhere");
    let note = delta_with(json!({ "kind": "note", "text": long, "parentToolUseId": "toolu_a1" }));
    assert_eq!(kept_event(&note), note, "only text and thinking are clipped under a parent");
    let result = kept_event(&delta_with(json!({ "kind": "tool_result", "text": long, "parentToolUseId": "toolu_a1" })));
    assert_eq!(num(&field(&result, "bytes")), long.len() as f64, "a subagent's tool result is clipped as any is, with its bytes");
}

fn hunk(first: i64, lines: Vec<String>) -> Value {
    json!({ "oldStart": first, "oldLines": 50, "newStart": first, "newLines": 50, "lines": lines })
}

fn hundred_char_lines() -> Vec<String> {
    (0..100).map(|i| format!("{}{}", if i % 2 == 0 { "-" } else { "+" }, "x".repeat(99))).collect()
}

// TS: thread-head.test.ts:269
#[test]
fn kept_event_cuts_a_patch_and_marks_it_last() {
    let lines = hundred_char_lines();
    let big = json!([{ "path": "/root/app/big.ts", "hunks": [hunk(1, lines.clone()), hunk(101, lines.clone())] }]);
    let e = delta_with(json!({ "kind": "tool_result", "text": "updated", "patch": big, "toolUseId": "tu_2" }));
    let kept = kept_event(&e);
    let second = json!({ "oldStart": 101, "oldLines": 32, "newStart": 101, "newLines": 31, "lines": lines[..63].to_vec() });
    let want = json!([{ "path": "/root/app/big.ts", "hunks": [hunk(1, lines.clone()), second] }]);
    assert_eq!(kept, delta_with(json!({ "kind": "tool_result", "text": "updated", "patch": want, "patchCut": true, "toolUseId": "tu_2" })), "the first hunk whole, the second to the lines that fit in 16,384 characters");
    let json = json_of(&kept);
    assert_eq!(json.as_object().expect("an object").keys().last().map(String::as_str), Some("patchCut"), "patchCut goes last");
    assert!(json.get("bytes").is_none(), "a text that was not clipped gains no bytes");

    let long_text = "t".repeat(TOOL_RESULT_KEPT + 10);
    let both = kept_event(&delta_with(json!({ "kind": "tool_result", "text": long_text, "patch": big })));
    assert_eq!(field(&both, "text"), json!("t".repeat(TOOL_RESULT_KEPT)));
    assert_eq!(num(&field(&both, "bytes")), long_text.len() as f64);
    assert_eq!(field(&both, "patch"), json_of(&delta_with(json!({ "patch": want })))["patch"]);
    assert_eq!(field(&both, "patchCut"), json!(true));
}

#[test]
fn kept_event_leaves_a_short_delta_alone() {
    let exactly = delta_with(json!({ "kind": "tool_result", "text": "r".repeat(TOOL_RESULT_KEPT) }));
    assert_eq!(kept_event(&exactly), exactly, "a text of exactly the cap is not clipped");
    let small = json!([{ "path": "/root/app/f.txt", "hunks": [{ "oldStart": 1, "oldLines": 3, "newStart": 1, "newLines": 3, "lines": [" alpha", "-beta", "+BETA", " gamma"] }] }]);
    let edit = delta_with(json!({ "kind": "tool_result", "text": "updated", "patch": small }));
    assert_eq!(kept_event(&edit), edit, "a patch that fits is kept whole and unmarked");
    assert!(json_of(&kept_event(&edit)).get("patchCut").is_none());
    let lead = delta_with(json!({ "kind": "text", "text": "z".repeat(TOOL_RESULT_KEPT * 2) }));
    assert_eq!(kept_event(&lead), lead);
    let end = event(end_v(WS, "t1", "u1"));
    assert_eq!(kept_event(&end), end, "only a delta changes");
}

// ---- flushing ----------------------------------------------------------------------------------------------------

// TS: runtime-history.test.ts:822
#[test]
fn flush_coalesces_a_burst_into_one_write() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        let area = ctx.area();
        area.record(event(start_v(WS, "t1", "u1", "go")));
        for i in 0..20 {
            area.record(event(plan_v(WS, "t1", &format!("step {i}"))));
        }
        ctx.clock.advance(TRANSCRIPT_FLUSH_MS - 1);
        settle().await;
        assert_eq!(ctx.rows.transactions.get(), 0, "nothing is written before the debounce");
        ctx.clock.advance(1);
        settle().await;
        assert_eq!(ctx.rows.transactions.get(), 1);
        assert_eq!(ctx.rows.kept(WS).len(), 21);
        ctx.clock.advance(TRANSCRIPT_FLUSH_MS * 2);
        settle().await;
        assert_eq!(ctx.rows.transactions.get(), 1, "one write for the whole burst");
    });
}

#[test]
fn flush_asks_coalesce_into_the_one_queued_and_an_empty_flush_writes_nothing() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        let area = ctx.area();
        area.record(event(start_v(WS, "t1", "u1", "go")));
        area.record(event(plan_v(WS, "t1", "step")));
        assert_eq!(ctx.clock.pending(), 1);
        let first = area.flush_transcript(WS);
        let second = area.flush_transcript(WS);
        assert_eq!(ctx.clock.pending(), 0, "asking cancels the debounce");
        second.await;
        first.await;
        assert_eq!(ctx.rows.transactions.get(), 1, "two asks, one write");
        assert_eq!(ctx.rows.positions(WS), vec![1, 2]);
        area.flush_transcript(WS).await;
        assert_eq!(ctx.rows.transactions.get(), 1, "a workspace with nothing pending opens no transaction");
    });
}

// TS: runtime-history.test.ts:872
#[test]
fn flush_cancels_the_timer_and_writes_what_waits() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        let area = ctx.area();
        area.record(event(start_v(WS, "t1", "u1", "go")));
        area.record(event(plan_v(WS, "t1", "step")));
        area.flush_transcript(WS).await;
        assert_eq!(ctx.rows.kept(WS).iter().map(|e| field(e, "type")).collect::<Vec<_>>(), vec![json!("session.start"), json!("session.plan")]);
        assert_eq!(ctx.clock.pending(), 0, "no timer is left behind");
        ctx.clock.advance(TRANSCRIPT_FLUSH_MS * 2);
        settle().await;
        assert_eq!(ctx.rows.transactions.get(), 1);
    });
}

#[test]
fn cancel_flush_stops_the_timer_and_forgets_it() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        let area = ctx.area();
        area.record(event(plan_v(WS, "t1", "step")));
        assert_eq!(ctx.clock.pending(), 1);
        area.cancel_flush(WS);
        assert_eq!(ctx.clock.pending(), 0);
        assert!(area.transcript_timers.borrow().is_empty());
        ctx.clock.advance(TRANSCRIPT_FLUSH_MS * 2);
        settle().await;
        assert!(ctx.rows.kept(WS).is_empty());
        area.record(event(plan_v(WS, "t1", "again")));
        assert_eq!(ctx.clock.pending(), 1, "the workspace arms a timer again");
    });
}

#[test]
fn a_failed_flush_keeps_its_pending_events() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        let area = ctx.area();
        let warned = console::capture();
        ctx.rows.fail_append.set(true);
        area.record(event(start_v(WS, "t1", "u1", "go")));
        area.record(event(end_v(WS, "t1", "u1")));
        settle().await;
        assert!(ctx.rows.kept(WS).is_empty(), "the transaction rolled back");
        assert_eq!(ctx.pending_of(WS).len(), 2, "the pending events stay for the next trigger");
        assert!(ctx.pending_bytes_of(WS).is_some_and(|b| b > 0));
        assert!(warned.lines().is_empty(), "a failed flush is swallowed with no log");
        assert!(ctx.rows.indexes.borrow().is_empty());

        ctx.rows.fail_append.set(false);
        area.flush_transcript(WS).await;
        assert_eq!(ctx.rows.positions(WS), vec![1, 2]);
        assert!(ctx.pending_of(WS).is_empty());
        assert!(ctx.rows.indexes.borrow().contains_key(WS));
    });
}

#[test]
fn on_transcript_queue_runs_steps_in_order_and_each_caller_gets_its_own_result() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        let area = ctx.area();
        let order = Rc::new(std::cell::RefCell::new(Vec::<&str>::new()));
        let (o1, o2) = (order.clone(), order.clone());
        let first = area.on_transcript_queue::<u32>(
            WS,
            Box::pin(async move {
                support::settle().await;
                o1.borrow_mut().push("one");
                Err::<u32, _>(err("the first step failed"))
            }),
        );
        let second = area.on_transcript_queue::<u32>(
            WS,
            Box::pin(async move {
                o2.borrow_mut().push("two");
                Ok::<u32, HostError>(7)
            }),
        );
        assert_eq!(second.await.expect("the second step still runs"), 7);
        assert_eq!(first.await.expect_err("the first caller sees its own failure").message, "the first step failed");
        assert_eq!(*order.borrow(), vec!["one", "two"], "a step waits for the one before it");

        let (gate, held) = futures::channel::oneshot::channel::<()>();
        let stuck = area.on_transcript_queue::<()>(WS, Box::pin(async move { held.await.map_err(|_| err("dropped")) }));
        let elsewhere = area.on_transcript_queue::<u32>("ws2", Box::pin(async { Ok::<u32, HostError>(1) }));
        assert_eq!(elsewhere.await.expect("another workspace's queue is its own"), 1);
        drop(gate);
        let _ = stuck.await;
    });
}

// ---- settling rows -----------------------------------------------------------------------------------------------

// TS: sqlite-transcripts.test.ts:231
// TS: sqlite-transcripts.test.ts:254
// TS: sqlite-transcripts.test.ts:284
// TS: runtime-history.test.ts:910
#[test]
fn settle_rows_appends_trims_reindexes_and_writes_the_index_in_one_transaction() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        seed_thread(&ctx, "t1", 4999, plain_start("t1"));
        let area = ctx.area();
        area.record(event(start_v(WS, "t1", "b", "second")));
        area.record(event(delta_v(WS, "t1", "b", "five thousand and one")));
        area.record(event(delta_v(WS, "t1", "b", "five thousand and two")));
        area.record(event(end_v(WS, "t1", "b")));
        settle().await;

        let positions = ctx.rows.positions(WS);
        assert_eq!(positions.len(), TRANSCRIPT_CAP, "the thread is held at its cap");
        assert_eq!((positions[0], positions[positions.len() - 1]), (4, 5003), "the three oldest events outside the newest turn went");
        assert_eq!(ctx.rows.transactions.get(), 1, "one transaction");
        assert_eq!(
            ctx.rows_called_of(&["atomically", "append", "total", "sizes", "at", "remove", "thread", "put_index"]),
            vec!["atomically", "append ws1 4", "total ws1 t1", "sizes ws1 t1", "at ws1 1", "remove ws1 1,2,3", "thread ws1 t1", "put_index ws1"],
            "the pending appended, the cap read, the start read back, the rows removed, the thread read again, the index put"
        );
        assert!(ctx.rows_called_of(&["all"]).is_empty(), "the folder is never read whole");
        assert!(ctx.pending_of(WS).is_empty());
        assert_eq!(ctx.pending_bytes_of(WS), None);
        assert!(ctx.store.logged().is_empty(), "no start of these held an image");

        let index = ctx.index_of(WS).expect("the index is held");
        assert!(index.trimmed.contains("t1"), "the thread the caps took events from is marked");
        assert_eq!(index.pos, 5003);
        assert_eq!(index.words.get("t1"), Some(&thread_words_of(&ctx.rows.kept(WS), "t1")), "the thread's words read again off its rows");
        let written: Value = serde_json::from_str(&ctx.rows.indexes.borrow()[WS]).expect("the index row is JSON");
        assert_eq!(written["trimmed"], json!(["t1"]));
        assert_eq!(num(&written["pos"]), 5003.0);
        assert_eq!(written["words"][0][1]["lines"], json!([]), "the kept index holds no lines");
        assert_eq!(written["words"][0][1]["cold"], json!(true));
    });
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

// TS: sqlite-transcripts.test.ts:625
// TS: session-images.test.ts:211
#[test]
fn settle_rows_releases_a_dropped_starts_images_after_the_commit() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        seed_thread(&ctx, "t1", 4999, start_with_image());
        images_seeded(&ctx);
        let area = ctx.area();
        push_past_the_cap(&area);
        settle().await;
        assert_eq!(ctx.rows.positions(WS)[0], 4, "the start that carried the image is gone");
        assert_eq!(ctx.store.logged(), vec!["deleteBlob attachments/t1.req_a.0", "put attachment-keys/ws1"], "its blob goes and its thread leaves the document");
        assert_eq!(ctx.store.get_now("attachment-keys", WS), Some(json!({ "threads": { "t2": [{ "key": "t2.req_z.0", "mediaType": "image/png" }] } })));
        assert!(ctx.store.blob_now("attachments", "t1.req_a.0").is_none());
        assert!(ctx.store.blob_now("attachments", "t2.req_z.0").is_some(), "another thread's image stays");
    });
}

#[test]
fn a_flush_that_rolls_back_releases_no_images_and_keeps_pending() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        seed_thread(&ctx, "t1", 4999, start_with_image());
        images_seeded(&ctx);
        let area = ctx.area();
        ctx.watched.fail_put_index.set(true);
        push_past_the_cap(&area);
        settle().await;
        assert_eq!(ctx.rows.positions(WS).len(), 4999, "the removal rolled back with the append");
        assert_eq!(ctx.rows.positions(WS)[0], 1);
        assert_eq!(ctx.pending_of(WS).len(), 4, "pending is as it was");
        assert!(ctx.store.logged().is_empty(), "no image is released before the commit");
        assert!(ctx.store.blob_now("attachments", "t1.req_a.0").is_some());
    });
}

#[test]
fn settle_rows_reads_back_only_starts_so_a_dropped_steers_image_stays() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        let steer = json!({ "type": "session.steer", "workspaceId": WS, "sessionId": "s1", "turnId": "a", "threadId": "t1", "prompt": "and this", "requestId": "req_s", "attachments": [{ "mediaType": "image/png", "bytes": 2 }] });
        seed_thread_second(&ctx, "t1", 4999, plain_start("t1"), Some(steer));
        ctx.store.seed("attachment-keys", WS, json!({ "threads": { "t1": [{ "key": "t1.req_s.0", "mediaType": "image/png" }] } }));
        ctx.store.seed_blob("attachments", "t1.req_s.0", &[7]);
        let area = ctx.area();
        push_past_the_cap(&area);
        settle().await;
        assert_eq!(ctx.rows.positions(WS)[0], 3, "the steer row was among those dropped");
        assert_eq!(ctx.rows_called_of(&["at"]), vec!["at ws1 1"], "only starts are read back before they go");
        assert!(ctx.store.logged().is_empty(), "a flush's trim lets go of a start's images alone");
    });
}

// ---- dropping a thread -------------------------------------------------------------------------------------------

fn session_start(session: &str, thread: &str, model: &str, copied_from: Option<&str>, pos: i64) -> SessionEvent {
    let mut v = json!({ "type": "session.start", "workspaceId": WS, "sessionId": session, "turnId": format!("u-{session}"), "threadId": thread, "prompt": format!("prompt of {thread}"), "model": model });
    if let Some(from) = copied_from {
        v["copiedFrom"] = json!(from);
    }
    stamped(v, pos, 10)
}

// TS: folder-threads.test.ts:691
// TS: folder-threads.test.ts:736
// TS: forget.test.ts:152
// TS: sqlite-transcripts.test.ts:458
// TS: sqlite-transcripts.test.ts:525
#[test]
fn drop_from_transcript_takes_one_threads_events_and_its_sessions_facts() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        let events = vec![
            session_start("s1", "t1", "claude-opus-5", None, 1),
            stamped(delta_v(WS, "t1", "u-s1", "alpha reply"), 2, 10),
            session_start("s1", "t2", "claude-opus-5", Some("t1"), 3),
            session_start("s2", "t2", "claude-sonnet-5", None, 4),
            stamped(delta_v(WS, "t2", "u-s2", "beta reply"), 5, 10),
        ];
        ctx.rows.seed(WS, events.clone());
        let mut index = index_of(&events);
        index.pos = 5;
        let before = index.facts.get("s1").cloned();
        assert_eq!(before, Some(SessionFacts { model: Some("claude-opus-5".into()), ..SessionFacts::default() }));
        ctx.hold_index(WS, index);
        let area = ctx.area();
        area.record(event(delta_v(WS, "t1", "u-s1", "t1 pending")));
        area.record(event(delta_v(WS, "t2", "u-s2", "t2 pending")));
        ctx.forget_rows_calls();

        area.drop_from_transcript(WS, "t2", None).await.expect("the drop lands");

        let kept = ctx.rows.kept(WS);
        let ids: Vec<(Value, Value)> = kept.iter().map(|e| (field(e, "threadId"), field(e, "text"))).collect();
        assert_eq!(ids, vec![(json!("t1"), Value::Null), (json!("t1"), json!("alpha reply")), (json!("t1"), json!("t1 pending"))], "the other thread keeps its events and its pending ones are written; the dropped thread's pending are not");
        assert!(ctx.pending_of(WS).is_empty());
        assert!(ctx.rows_called_of(&["all"]).is_empty(), "the folder is never read whole");
        assert_eq!(ctx.rows_called_of(&["remove"]), vec!["remove ws1 3,4,5"]);
        let index = ctx.index_of(WS).expect("an index");
        assert_eq!(index.facts.get("s1"), before.as_ref(), "a fork's copied start names its source's session, whose facts stay");
        assert!(!index.facts.contains_key("s2"), "the fork's own session loses its facts");
        assert!(!index.words.contains_key("t2"));
        assert!(!index.starts.contains_key("t2"));
        assert!(index.words.contains_key("t1"));
        assert!(ctx.rows.indexes.borrow().contains_key(WS), "the index is written with the removal");
    });
}

// TS: rewind.test.ts:250
// TS: rewind.test.ts:222
#[test]
fn drop_from_transcript_with_a_predicate_cuts_only_the_turns_it_names() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        let events = vec![
            stamped(start_v(WS, "t1", "u1", "one"), 1, 10),
            stamped(start_v(WS, "t1", "u2", "two"), 2, 10),
            stamped(delta_v(WS, "t1", "u2", "two reply"), 3, 10),
            stamped(start_v(WS, "t3", "u2", "other thread, same turn id"), 4, 10),
        ];
        ctx.rows.seed(WS, events.clone());
        let mut index = index_of(&events);
        index.pos = 4;
        ctx.hold_index(WS, index);
        let area = ctx.area();
        area.record(event(delta_v(WS, "t1", "u1", "pending of turn one")));
        area.record(event(delta_v(WS, "t1", "u2", "pending of turn two")));

        let drops: Rc<dyn Fn(&SessionEvent) -> bool> = Rc::new(|e| json_of(e)["turnId"] == "u2");
        area.drop_from_transcript(WS, "t1", Some(drops)).await.expect("the cut lands");

        let kept: Vec<(Value, Value)> = ctx.rows.kept(WS).iter().map(|e| (field(e, "threadId"), field(e, "turnId"))).collect();
        assert_eq!(kept, vec![(json!("t1"), json!("u1")), (json!("t3"), json!("u2")), (json!("t1"), json!("u1"))], "turn two of the thread went, with its pending event; the other thread's turn of the same id stays");
        assert_eq!(ctx.rows.positions(WS), vec![1, 4, 5]);
        assert!(ctx.pending_of(WS).is_empty());
        let words = ctx.index_of(WS).expect("an index").words.get("t1").cloned().expect("t1 words");
        assert_eq!(words.lines, vec!["one", "pending of turn one"], "the thread's words are read again off what is left");
    });
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

// TS: runtime-history.test.ts:910
#[test]
fn drop_oldest_keeps_a_transcript_under_both_caps_whole() {
    let mut events = vec![event(start_v(WS, "t1", "a", "go"))];
    events.extend((1..TRANSCRIPT_CAP as i64).map(|i| text_delta(Some("t1"), &format!("line {i}"), i + 1)));
    assert_eq!(events.len(), TRANSCRIPT_CAP);
    let before = events.clone();
    let (gone, left) = trim(&mut events);
    assert_eq!(events, before, "exactly the count cap changes nothing");
    assert!(gone.is_empty());
    assert_eq!(left, total(&before));

    let each = TRANSCRIPT_BYTES / 10;
    let mut weighty: Vec<SessionEvent> = (0..9).map(|i| sized_delta(WS, "t1", i + 1, each)).collect();
    weighty.push(sized_delta(WS, "t1", 10, TRANSCRIPT_BYTES - 9 * each));
    assert_eq!(total(&weighty), TRANSCRIPT_BYTES);
    let before = weighty.clone();
    let (gone, left) = trim(&mut weighty);
    assert_eq!(weighty, before, "exactly the byte cap changes nothing");
    assert!(gone.is_empty());
    assert_eq!(left, TRANSCRIPT_BYTES);

    weighty.push(sized_delta(WS, "t1", 11, 200));
    let (gone, left) = trim(&mut weighty);
    assert_eq!(gone, vec![before[0].clone()], "one byte over drops the oldest event");
    assert_eq!(weighty.len(), 10);
    assert_eq!(left, TRANSCRIPT_BYTES - event_bytes(&before[0]) + 200);
}

// TS: sqlite-transcripts.test.ts:254
#[test]
fn drop_oldest_drops_outside_the_newest_turn_first() {
    let mut events = vec![event(start_v(WS, "t1", "a", "start a")), text_delta(Some("t1"), "a1", 2), event(start_v(WS, "t1", "b", "start b"))];
    events.extend((1..=5001).map(|i| text_delta(Some("t1"), &format!("b{i}"), 3 + i)));
    assert_eq!(events.len(), 5004);
    let before = events.clone();
    let (gone, left) = trim(&mut events);
    assert_eq!(gone.iter().map(text_of).collect::<Vec<_>>(), vec!["start a", "a1", "start b", "b1"], "the old turn first, then the oldest of the newest turn, in list order");
    assert_eq!(events.len(), TRANSCRIPT_CAP);
    assert_eq!(text_of(&events[0]), "b2");
    assert_eq!(text_of(&events[events.len() - 1]), "b5001");
    assert_eq!(left, total(&before) - total(&gone));
}

#[test]
fn drop_oldest_never_drops_a_threads_newest_event() {
    let mut alone = vec![sized_delta(WS, "t1", 1, TRANSCRIPT_BYTES + 1000)];
    let (gone, left) = trim(&mut alone);
    assert!(gone.is_empty(), "a lone event over the byte cap stays");
    assert_eq!(alone.len(), 1);
    assert_eq!(left, TRANSCRIPT_BYTES + 1000);

    let tiny = sized_delta(WS, "t1", 1, 200);
    let big = sized_delta(WS, "t1", 2, TRANSCRIPT_BYTES + 1000);
    let mut pair = vec![tiny.clone(), big.clone()];
    let (gone, left) = trim(&mut pair);
    assert_eq!(gone, vec![tiny], "the older goes and the newest stays though it is still over");
    assert_eq!(pair, vec![big]);
    assert_eq!(left, TRANSCRIPT_BYTES + 1000);

    let mut counted: Vec<SessionEvent> = (1..=(TRANSCRIPT_CAP as i64 + 1)).map(|i| text_delta(Some("t1"), &format!("n{i}"), i)).collect();
    let (gone, _) = trim(&mut counted);
    assert_eq!(gone.iter().map(text_of).collect::<Vec<_>>(), vec!["n1"], "a thread with no start drops its oldest first");
    assert_eq!(text_of(&counted[counted.len() - 1]), format!("n{}", TRANSCRIPT_CAP + 1));
}

// TS: runtime-history.test.ts:971
#[test]
fn drop_oldest_caps_each_thread_alone() {
    let mut events: Vec<SessionEvent> = (1..=(TRANSCRIPT_CAP as i64 + 1)).map(|i| text_delta(Some("busy"), &format!("busy {i}"), i)).collect();
    events.extend((1..=100).map(|i| text_delta(Some("quiet"), &format!("quiet {i}"), 6000 + i)));
    let before_quiet: Vec<SessionEvent> = events.iter().filter(|e| field(e, "threadId") == json!("quiet")).cloned().collect();
    let (gone, _) = trim(&mut events);
    assert_eq!(gone.iter().map(text_of).collect::<Vec<_>>(), vec!["busy 1"], "only the busy thread loses an event");
    let quiet_now: Vec<SessionEvent> = events.iter().filter(|e| field(e, "threadId") == json!("quiet")).cloned().collect();
    assert_eq!(quiet_now, before_quiet);
}

// TS: runtime-history.test.ts:951
// TS: runtime-history.test.ts:971
#[test]
fn drop_oldest_caps_each_threads_bytes_alone() {
    let big = 256 * 1024;
    let mut events = Vec::new();
    let mut pos = 0;
    let mut next = || {
        pos += 1;
        pos
    };
    events.push(stamped(start_v(WS, "quiet", "q", "the quiet one"), next(), 5));
    events.push(sized_delta(WS, "quiet", next(), 300));
    let quiet = events.clone();
    let mut busy_first = Vec::new();
    for t in ["busy0", "busy1", "busy2"] {
        busy_first.push(events.len());
        events.push(stamped(start_v(WS, t, "x0", &format!("{t}.0")), next(), 5));
        for _ in 0..20 {
            events.push(sized_delta(WS, t, next(), big));
        }
        events.push(stamped(start_v(WS, t, "x1", &format!("{t}.1")), next(), 5));
        events.push(sized_delta(WS, t, next(), 300));
    }
    let before = events.clone();
    let (gone, _) = trim(&mut events);
    assert_eq!(gone.len(), 3 * 6, "each busy thread loses its old start and five of its big events");
    for (n, t) in ["busy0", "busy1", "busy2"].iter().enumerate() {
        let from = busy_first[n];
        assert_eq!(&gone[n * 6..(n + 1) * 6], &before[from..from + 6], "{t}: the oldest events outside its newest turn, in list order");
        let held: Vec<SessionEvent> = events.iter().filter(|e| field(e, "threadId") == json!(t)).cloned().collect();
        assert!(total(&held) <= TRANSCRIPT_BYTES, "{t}: inside its own cap");
        assert!(held.iter().any(|e| text_of(e) == format!("{t}.1")), "{t}: its newest turn is whole");
    }
    let quiet_now: Vec<SessionEvent> = events.iter().filter(|e| field(e, "threadId") == json!("quiet")).cloned().collect();
    assert_eq!(quiet_now, quiet, "the quiet thread keeps its prompt and its reply");
}

// TS: runtime-history.test.ts:951
#[test]
fn drop_oldest_applies_bytes_only_when_the_whole_is_over() {
    let weighed = Cell::new(0);
    let weigh = |_: &SessionEvent| {
        weighed.set(weighed.get() + 1);
        7u64
    };
    let mut counted: Vec<SessionEvent> = (1..=(TRANSCRIPT_CAP as i64 + 1)).map(|i| text_delta(Some("t1"), &format!("n{i}"), i)).collect();
    let mut gone = Vec::new();
    let left = drop_oldest_of(&mut counted, 100, &weigh, &mut |e| gone.push(e.clone()));
    assert_eq!(left, 93, "the bytes left are the bytes given less what went");
    assert_eq!(gone.len(), 1);
    assert_eq!(weighed.get(), 1, "a transcript over its count alone is trimmed by count: only what goes is weighed");

    weighed.set(0);
    let big: u64 = 3 * 1024 * 1024;
    let weigh_big = |_: &SessionEvent| {
        weighed.set(weighed.get() + 1);
        big
    };
    let mut three = vec![text_delta(Some("t1"), "e1", 1), text_delta(Some("t1"), "e2", 2), text_delta(Some("t1"), "e3", 3)];
    let mut gone = Vec::new();
    let left = drop_oldest_of(&mut three, 3 * big, &weigh_big, &mut |e| gone.push(e.clone()));
    assert_eq!(gone.iter().map(text_of).collect::<Vec<_>>(), vec!["e1", "e2"]);
    assert_eq!(left, big);
    assert_eq!(weighed.get(), 3 + 2, "over its bytes, every event is weighed to count them and each one dropped again");

    let mut small: Vec<SessionEvent> = (1..=(TRANSCRIPT_CAP as i64 + 1)).map(|i| text_delta(Some("t1"), &format!("n{i}"), i)).collect();
    let mut gone = Vec::new();
    let all = total(&small);
    let left = drop_oldest(&mut small, Some(all), &mut |e| gone.push(e.clone()));
    assert_eq!(gone.len(), 1);
    assert_eq!(left, all - event_bytes(&gone[0]), "drop_oldest weighs by event_bytes");
}

#[test]
fn an_event_with_no_thread_is_thread_empty_and_never_marked() {
    let threadless = text_delta(None, "n", 1);
    let threaded = text_delta(Some("t1"), "n", 2);
    assert_eq!(Trimmed::thread(&threadless), None);
    assert_eq!(Trimmed::thread(&threaded), Some("t1"));
    assert!(!Trimmed::is_start(&threaded));
    assert!(Trimmed::is_start(&event(start_v(WS, "t1", "a", "go"))));
    let size = |thread: Option<&str>, kind: &str, pos: i64| EventSize { pos, thread_id: thread.map(String::from), kind: kind.to_string(), size: 10 };
    assert_eq!(Trimmed::thread(&size(None, "session.delta", 1)), None);
    assert_eq!(Trimmed::thread(&size(Some("t1"), "session.delta", 1)), Some("t1"));
    assert!(Trimmed::is_start(&size(Some("t1"), "session.start", 1)));
    assert!(!Trimmed::is_start(&size(Some("t1"), "session.end", 1)));

    let mut index = TranscriptIndex::default();
    mark_trimmed(&mut index, None);
    assert!(index.trimmed.is_empty(), "an event with no thread is never marked");
    mark_trimmed(&mut index, Some("t1"));
    mark_trimmed(&mut index, Some("t1"));
    assert_eq!(index.trimmed.iter().cloned().collect::<Vec<_>>(), vec!["t1"]);

    let mut events: Vec<SessionEvent> = (1..=(TRANSCRIPT_CAP as i64 + 1)).map(|i| text_delta(None, &format!("n{i}"), i)).collect();
    let (gone, _) = trim(&mut events);
    assert_eq!(gone.iter().map(text_of).collect::<Vec<_>>(), vec!["n1"], "threadless events are one thread, capped as any");

    let mut sizes: Vec<EventSize> = (1..=(TRANSCRIPT_CAP as i64 + 1)).map(|p| size(None, "session.delta", p)).collect();
    let mut dropped = Vec::new();
    let left = drop_oldest_of(&mut sizes, 10 * (TRANSCRIPT_CAP as u64 + 1), &|e: &EventSize| e.size, &mut |e| dropped.push(e.pos));
    assert_eq!(dropped, vec![1]);
    assert_eq!(left, 10 * TRANSCRIPT_CAP as u64);
    assert_eq!(sizes.len(), TRANSCRIPT_CAP);
}

// ---- reads -------------------------------------------------------------------------------------------------------

// TS: transcript-index.test.ts:86
// TS: sqlite-transcripts.test.ts:625
#[test]
fn open_transcript_trims_in_memory_and_deletes_no_row() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        seed_thread(&ctx, "t1", 4999, start_with_image());
        images_seeded(&ctx);
        let area = ctx.area();
        for i in 0..3 {
            area.record(event(delta_v(WS, "t1", "b", &format!("pending {i}"))));
        }
        ctx.forget_rows_calls();
        let opened = area.open_transcript(WS).expect("the open reads");
        assert_eq!(opened.len(), TRANSCRIPT_CAP, "the copy is trimmed to the cap");
        assert_eq!(pos_of(&opened[0]), 3, "the oldest two went from the copy");
        assert_eq!(text_of(&opened[opened.len() - 1]), "pending 2", "the pending events come after the rows");
        assert_eq!(ctx.rows.positions(WS).len(), 4999, "no row is deleted");
        assert!(ctx.rows_called_of(&["remove", "append", "put_index"]).is_empty());
        assert_eq!(ctx.pending_of(WS).len(), 3, "the pending events are untouched");
        assert!(ctx.index_of(WS).expect("an index").trimmed.is_empty(), "a read marks nothing trimmed");
        settle().await;
        assert!(ctx.store.logged().is_empty(), "a read releases no image");
        assert!(ctx.store.blob_now("attachments", "t1.req_a.0").is_some());
    });
}

// TS: transcript-index.test.ts:86
#[test]
fn open_transcript_carries_the_events_written_since_the_last_flush() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        ctx.rows.seed(WS, vec![stamped(start_v(WS, "t1", "u1", "stream something"), 1, 10), stamped(delta_v(WS, "t1", "u1", "written"), 2, 10)]);
        ctx.hold_index(WS, TranscriptIndex { pos: 2, ..TranscriptIndex::default() });
        let area = ctx.area();
        area.record(event(delta_v(WS, "t1", "u1", "a periwinkle answer")));
        let opened = area.open_transcript(WS).expect("the open reads");
        assert_eq!(opened.iter().map(text_of).collect::<Vec<_>>(), vec!["stream something", "written", "a periwinkle answer"]);
        assert_eq!(ctx.rows_called_of(&["all"]), vec!["all ws1"]);
    });
}

// TS: sqlite-transcripts.test.ts:506
#[test]
fn open_thread_reads_one_threads_rows_and_pending() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        ctx.rows.seed(
            WS,
            vec![
                stamped(start_v(WS, "t1", "u1", "one"), 1, 10),
                stamped(delta_v(WS, "t1", "u1", "one reply"), 2, 10),
                stamped(start_v(WS, "t2", "u2", "two"), 3, 10),
            ],
        );
        ctx.hold_index(WS, TranscriptIndex { pos: 3, ..TranscriptIndex::default() });
        let area = ctx.area();
        area.record(event(delta_v(WS, "t1", "u1", "t1 pending")));
        area.record(event(delta_v(WS, "t2", "u2", "t2 pending")));
        ctx.forget_rows_calls();
        let opened = area.open_thread(WS, "t1").expect("the open reads");
        assert_eq!(opened.iter().map(text_of).collect::<Vec<_>>(), vec!["one", "one reply", "t1 pending"], "that thread's rows, then that thread's pending");
        assert_eq!(ctx.rows_called(), vec!["thread ws1 t1"], "never the folder's rows");
    });
}

#[test]
fn open_thread_trims_its_copy_against_its_own_size() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        seed_thread(&ctx, "t1", 4999, plain_start("t1"));
        let area = ctx.area();
        for i in 0..3 {
            area.record(event(delta_v(WS, "t1", "b", &format!("pending {i}"))));
        }
        let opened = area.open_thread(WS, "t1").expect("the open reads");
        assert_eq!(opened.len(), TRANSCRIPT_CAP);
        assert_eq!(pos_of(&opened[0]), 3);
        assert_eq!(ctx.rows.positions(WS).len(), 4999);
        assert!(ctx.index_of(WS).expect("an index").trimmed.is_empty());
    });
}

// TS: thread-head.test.ts:306
#[test]
fn read_pages_pending_then_rows_newest_first_with_total_and_pos() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        let mut rows = vec![stamped(start_v(WS, "t1", "u1", "go"), 1, 10)];
        rows.extend((2..=5).map(|p| stamped(delta_v(WS, "t1", "u1", &format!("row {p}")), p, 10)));
        rows.push(stamped(delta_v(WS, "other", "u9", "someone else's"), 6, 10));
        ctx.rows.seed(WS, rows);
        ctx.hold_index(WS, TranscriptIndex { pos: 6, ..TranscriptIndex::default() });
        let area = ctx.area();
        area.record(event(delta_v(WS, "t1", "u1", "pending 7")));
        area.record(event(delta_v(WS, "t1", "u1", "pending 8")));
        area.record(event(delta_v(WS, "other", "u9", "pending of another thread")));

        let page = area.read(WS, "t1", &ThreadRead { limit: 3, ..read_all() }).expect("the read");
        assert_eq!(page.events.iter().map(text_of).collect::<Vec<_>>(), vec!["row 5", "pending 7", "pending 8"], "the pending events newest first, then the rows, answered oldest first");
        assert_eq!(page.total, 5 + 2, "the thread's rows and its pending");
        assert_eq!(page.pos, 9, "the position the held index has issued");
        assert!(!page.trimmed);

        let before = area.read(WS, "t1", &ThreadRead { before: Some(7), limit: 2, ..read_all() }).expect("the read");
        assert_eq!(before.events.iter().map(|e| pos_of(e)).collect::<Vec<_>>(), vec![4, 5], "a pending event at or past before is skipped");

        ctx.transcript_state.with(|s| {
            s.index.get_mut(WS).expect("an index").trimmed.insert("t1".into());
        });
        assert!(area.read(WS, "t1", &read_all()).expect("the read").trimmed, "trimmed only where the index marks the thread");
        assert!(!area.read(WS, "other", &read_all()).expect("the read").trimmed);

        let none = area.read("ws9", "t1", &read_all()).expect("the read");
        assert_eq!((none.pos, none.total, none.events.len(), none.trimmed), (0, 0, 0, false), "no held index reads pos 0");
        assert!(ctx.index_of("ws9").is_none(), "a read puts no index in");

        let through_the_trait = TranscriptReader::read(&*area, WS, "t1", &ThreadRead { limit: 3, ..read_all() }).await.expect("the read");
        assert_eq!(through_the_trait.events.iter().map(text_of).collect::<Vec<_>>(), vec!["row 5", "pending 7", "pending 8"]);
    });
}

// TS: thread-head.test.ts:306
#[test]
fn read_pages_back_by_position_and_the_pages_put_together_are_the_thread() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        ctx.rows.seed(WS, (1..=450).map(|p| stamped(delta_v(WS, "t1", "u1", &format!("line {p}")), p, 10)).collect());
        ctx.hold_index(WS, TranscriptIndex { pos: 450, ..TranscriptIndex::default() });
        let area = ctx.area();
        let whole = ctx.rows.kept(WS);
        let first = area.read(WS, "t1", &ThreadRead { limit: 200, ..read_all() }).expect("the read");
        assert_eq!(first.events, whole[250..].to_vec());
        assert_eq!((first.total, first.pos), (450, 450));
        let mut pages = vec![first.events.clone()];
        let mut before = pos_of(&first.events[0]);
        loop {
            let page = area.read(WS, "t1", &ThreadRead { before: Some(before), limit: 200, ..read_all() }).expect("the read");
            if page.events.is_empty() {
                break;
            }
            before = pos_of(&page.events[0]);
            pages.insert(0, page.events);
        }
        assert_eq!(pages.len(), 3);
        assert_eq!(pages.concat(), whole);
    });
}

// TS: thread-head.test.ts:333
#[test]
fn read_stops_at_the_pages_bytes_past_its_first_event() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        ctx.rows.seed(WS, (1..=5).map(|p| stamped(delta_v(WS, "t1", "u1", &"y".repeat(12 * 1024)), p, 10)).collect());
        ctx.hold_index(WS, TranscriptIndex { pos: 5, ..TranscriptIndex::default() });
        let area = ctx.area();
        let whole = ctx.rows.kept(WS);
        let one = event_bytes(&whole[4]) + 1;
        let tight = area.read(WS, "t1", &ThreadRead { limit: 200, bytes: 1, ..read_all() }).expect("the read");
        assert_eq!(tight.events, vec![whole[4].clone()], "the newest always comes, so a page back always moves");
        let strict = area.read(WS, "t1", &ThreadRead { limit: 200, bytes: 1, strict: true, ..read_all() }).expect("the read");
        assert!(strict.events.is_empty(), "strict holds even the newest to the bytes");
        let two = area.read(WS, "t1", &ThreadRead { limit: 200, bytes: 2 * one, ..read_all() }).expect("the read");
        assert_eq!(two.events, whole[3..].to_vec(), "each event counts its JSON and a comma");
        let short = area.read(WS, "t1", &ThreadRead { limit: 200, bytes: 2 * one - 1, ..read_all() }).expect("the read");
        assert_eq!(short.events, vec![whole[4].clone()]);
        assert_eq!(two.total, 5);
    });
}

// ---- loading the index -------------------------------------------------------------------------------------------

// TS: transcript-index.test.ts:129
#[test]
fn load_index_holds_a_kept_index_and_rebuilds_one_that_does_not_read() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        let warned = console::capture();
        let mut kept = TranscriptIndex { pos: 7, ..TranscriptIndex::default() };
        kept.trimmed.insert("t9".into());
        ctx.rows.indexes.borrow_mut().insert(WS.into(), String::from_utf8(index_bytes(&kept, None, false)).expect("utf-8"));
        let area = ctx.area();
        area.load_index(WS);
        let held = ctx.index_of(WS).expect("the kept index is held");
        assert_eq!((held.pos, held.trimmed.iter().cloned().collect::<Vec<_>>()), (7, vec!["t9".to_string()]));
        assert_eq!(ctx.rows_called(), vec!["newest_pos ws1", "index ws1"], "no rows are read for an index that reads");
        assert_eq!(ctx.clock.pending(), 1, "words are balanced, which asks for warming");
        assert!(ctx.unread().is_empty());

        let events: Vec<SessionEvent> = vec![
            stamped(start_v("ws2", "t1", "u1", "fix the redirect"), 1, 10),
            stamped(delta_v("ws2", "t1", "u1", "Done. The redirect now sends every path."), 2, 10),
            stamped(end_v("ws2", "t1", "u1"), 3, 10),
        ];
        ctx.rows.seed("ws2", events.clone());
        ctx.rows.indexes.borrow_mut().insert("ws2".into(), "not json at all".into());
        area.load_index("ws2");
        let rebuilt = index_of(&events);
        assert_eq!(ctx.index_of("ws2"), Some(rebuilt.clone()), "an index that does not read is rebuilt from the rows");
        assert_eq!(ctx.rows.indexes.borrow()["ws2"], String::from_utf8(index_bytes(&rebuilt, None, false)).expect("utf-8"), "and written");

        ctx.rows.seed("ws3", events[..2].to_vec());
        area.load_index("ws3");
        assert_eq!(ctx.index_of("ws3"), Some(index_of(&events[..2])), "a missing index is read off the rows");
        assert!(ctx.rows.indexes.borrow().contains_key("ws3"));

        ctx.forget_rows_calls();
        area.load_index("ws9");
        assert!(ctx.index_of("ws9").is_none(), "a workspace with no rows holds no index at all");
        assert!(ctx.rows_called_of(&["put_index"]).is_empty());
        assert!(!ctx.rows.indexes.borrow().contains_key("ws9"));
        assert!(ctx.unread().is_empty());
        assert!(warned.lines().is_empty());
    });
}

// TS: sqlite-transcripts.test.ts:592
#[test]
fn invariant_5_an_unread_index_keeps_writing_rows_and_skips_the_index_until_reindexed() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        let warned = console::capture();
        let seeded = vec![
            stamped(start_v(WS, "t1", "u1", "one"), 1, 10),
            stamped(delta_v(WS, "t1", "u1", "said one"), 2, 10),
            stamped(end_v(WS, "t1", "u1"), 3, 10),
        ];
        ctx.rows.seed(WS, seeded.clone());
        ctx.watched.fail_all.set(true);
        let area = ctx.area();
        area.load_index(WS);
        assert_eq!(warned.lines(), vec!["the transcript of ws1 does not read (disk I/O error), so search and a send leave it out until it reads"]);
        assert_eq!(ctx.index_of(WS), Some(TranscriptIndex { pos: 3, ..TranscriptIndex::default() }), "an empty index that goes on after the newest row");
        assert_eq!(ctx.unread(), vec![WS]);
        ctx.watched.fail_all.set(false);

        area.record(event(start_v(WS, "t1", "u2", "two")));
        area.record(event(delta_v(WS, "t1", "u2", "said two")));
        area.flush_transcript(WS).await;
        assert_eq!(ctx.rows.positions(WS), vec![1, 2, 3, 4, 5], "the flush still appends its rows");
        assert!(!ctx.rows.indexes.borrow().contains_key(WS), "and skips the index while it is unread");
        assert_eq!(ctx.unread(), vec![WS]);

        let drops: Rc<dyn Fn(&SessionEvent) -> bool> = Rc::new(|e| json_of(e)["turnId"] == "u2");
        area.drop_from_transcript(WS, "t1", Some(drops)).await.expect("the cut lands");
        assert_eq!(ctx.rows.positions(WS), vec![1, 2, 3]);
        assert!(ctx.unread().is_empty(), "a reindex clears the mark");
        let mut whole = index_of(&seeded);
        whole.pos = 5;
        assert_eq!(ctx.index_of(WS), Some(whole.clone()), "the whole index is rebuilt off the rows, keeping the positions issued");
        assert_eq!(ctx.rows.indexes.borrow()[WS], String::from_utf8(index_bytes(&whole, None, false)).expect("utf-8"));
    });
}

// ---- held words --------------------------------------------------------------------------------------------------

fn line_of(c: char, n: usize) -> String {
    c.to_string().repeat(n)
}

// TS: sqlite-transcripts.test.ts:337
// TS: sqlite-transcripts.test.ts:408
#[test]
fn balance_words_holds_the_newest_threads_inside_the_budget() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        let (a, b) = (line_of('a', 1_000_000), line_of('b', 1_000_000));
        let (c1, c2, c3) = (line_of('1', 50_000), line_of('2', 50_000), line_of('3', 50_000));
        let held_a = words(&[a.as_str()], 300);
        let held_b = words(&[b.as_str()], 200);
        let mut index_one = index_of_words(vec![("a", held_a.clone()), ("b", held_b.clone()), ("c", words(&[c1.as_str(), c2.as_str(), c3.as_str()], 100))]);
        index_one.pos = 1;
        ctx.hold_index(WS, index_one);
        ctx.hold_index("ws2", index_of_words(vec![("d", words(&["small text"], 50))]));
        let area = ctx.area();
        area.balance_words();
        let one = ctx.index_of(WS).expect("an index");
        assert_eq!(one.words.get("a"), Some(&held_a), "a thread inside the budget is as it was");
        assert_eq!(one.words.get("b"), Some(&held_b));
        let share = WORDS_HELD - 2_000_000;
        assert_eq!(
            one.words.get("c"),
            Some(&ThreadWords { lines: vec![c3.clone()], open: None, last: JsNumber(100.0), size: 150_000, first: Some(line_of('1', 200)), opened: None, cold: Some(true), room: Some(share) }),
            "the thread that passes what is left keeps its newest lines that fit"
        );
        let d = ctx.index_of("ws2").expect("an index").words.get("d").cloned().expect("d");
        assert_eq!(d, ThreadWords { lines: vec![], open: None, last: JsNumber(50.0), size: 10, first: Some("small text".into()), opened: None, cold: Some(true), room: Some(0) }, "an older thread with nothing left to hold is let go whole, across workspaces");
        assert_eq!(ctx.clock.pending(), 1, "warming is asked for");
        assert!(area.warming.get());
    });
}

// TS: sqlite-transcripts.test.ts:408
#[test]
fn a_running_thread_holds_all_its_words() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        let lines: Vec<String> = ['x', 'y', 'z'].iter().map(|c| line_of(*c, 1_000_000)).collect();
        let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        let big = words(&refs, 300);
        let q = line_of('q', 1000);
        ctx.hold_index(WS, index_of_words(vec![("run", big.clone()), ("quiet", words(&[q.as_str()], 200))]));
        ctx.sessions.with(|s| *s = sessions_of(vec![row(view(json!({ "id": "r1", "threadId": "run", "status": "running" })), "u1")]));
        let area = ctx.area();
        area.balance_words();
        let held = ctx.index_of(WS).expect("an index");
        assert_eq!(held.words.get("run"), Some(&big), "a thread whose turn runs holds all of its own, past the budget");
        let quiet = held.words.get("quiet").expect("quiet");
        assert_eq!((quiet.cold, quiet.room, quiet.lines.len()), (Some(true), Some(0), 0), "the budget was spent, so the next thread holds none");

        ctx.sessions.with(|s| *s = sessions_of(vec![row(view(json!({ "id": "r1", "threadId": "run", "status": "completed" })), "u1")]));
        area.balance_words();
        let held = ctx.index_of(WS).expect("an index");
        let run = held.words.get("run").expect("run");
        assert_eq!(run.lines, lines[1..].to_vec(), "once its turn ends it is cut to the newest lines that fit");
        assert_eq!((run.cold, run.room, run.size), (Some(true), Some(WORDS_HELD), 3_000_000));
    });
}

#[test]
fn balance_words_ranks_by_the_later_of_last_and_opened_ties_in_held_order() {
    local(async {
        let line = line_of('p', 1_500_000);
        let rank = |order: &[&str]| {
            let ctx = FakeTranscriptsCtx::new();
            ctx.hold_index(WS, index_of_words(order.iter().map(|t| (*t, words(&[line.as_str()], 100))).collect()));
            ctx.area().balance_words();
            let index = ctx.index_of(WS).expect("an index");
            order.iter().map(|t| index.words[*t].cold == Some(true)).collect::<Vec<bool>>()
        };
        assert_eq!(rank(&["p", "q"]), vec![false, true], "equal ranks keep their held order: the first takes the budget");
        assert_eq!(rank(&["q", "p"]), vec![false, true]);

        let ctx = FakeTranscriptsCtx::new();
        let mut opened = words(&[line.as_str()], 10);
        opened.opened = Some(JsNumber(500.0));
        ctx.hold_index(WS, index_of_words(vec![("spoke", words(&[line.as_str()], 400)), ("opened", opened)]));
        ctx.area().balance_words();
        let index = ctx.index_of(WS).expect("an index");
        assert_eq!(index.words["opened"].cold, None, "opened at 500 ranks above spoken at 400");
        assert_eq!(index.words["spoke"].cold, Some(true));
    });
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
    ThreadWords { lines: vec![line.to_string()], open: None, last: JsNumber(last as f64), size: line.chars().count(), first: Some(first.to_string()), opened: None, cold: None, room: None }
}

// TS: sqlite-transcripts.test.ts:337
#[test]
fn warming_reads_back_one_thread_a_tick() {
    local(async {
        let (ctx, _, b) = warm_ctx();
        let area = ctx.area();
        assert!(area.warm_one(), "a thread was read back");
        assert_eq!(ctx.index_of(WS).expect("an index").words.get("a"), Some(&warmed("alpha words", "alpha words", 300)), "its words back, the later of the read time and the time it was last spoken in");
        assert_eq!(ctx.index_of(WS).expect("an index").words.get("b"), Some(&b), "the next thread waits for the next tick");
        assert_eq!(ctx.rows_called_of(&["word_events"]), vec!["word_events ws1 a"]);
        assert!(area.warm_one());
        assert_eq!(ctx.index_of(WS).expect("an index").words.get("b"), Some(&warmed("beta words", "beta words", 200)));
        assert!(!area.warm_one(), "with none left, warming stops");
        assert_eq!(ctx.rows_called_of(&["word_events"]), vec!["word_events ws1 a", "word_events ws1 b"]);

        let (ctx, _, _) = warm_ctx();
        let area = ctx.area();
        area.balance_words();
        area.balance_words();
        assert_eq!(ctx.clock.pending(), 1, "at most one warming is pending");
        assert!(area.warming.get());
        assert!(ctx.rows_called_of(&["word_events"]).is_empty(), "nothing is read before the tick");
        ctx.clock.advance(0);
        assert_eq!(ctx.rows_called_of(&["word_events"]), vec!["word_events ws1 a", "word_events ws1 b"], "each tick reads one and asks again");
        assert_eq!(ctx.clock.pending(), 0);
        assert!(!area.warming.get());
        assert_eq!(ctx.index_of(WS).expect("an index").words.get("a"), Some(&warmed("alpha words", "alpha words", 300)));
    });
}

#[test]
fn warming_skips_a_thread_with_room_and_stops_when_the_host_closed() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        let mut roomy = cold("roomy", 100, 300);
        roomy.room = Some(100);
        ctx.hold_index(WS, index_of_words(vec![("roomy", roomy)]));
        let area = ctx.area();
        assert!(!area.warm_one(), "a thread that holds what it may is not read back");
        assert!(ctx.rows_called_of(&["word_events"]).is_empty());

        let (ctx, a, b) = warm_ctx();
        ctx.closed.set(true);
        let area = ctx.area();
        area.balance_words();
        ctx.clock.advance(0);
        assert!(ctx.rows_called_of(&["word_events"]).is_empty(), "a closed host warms nothing");
        assert_eq!(ctx.clock.pending(), 0);
        assert!(!area.warming.get());
        assert_eq!(ctx.index_of(WS).expect("an index").words.get("a"), Some(&a));
        assert_eq!(ctx.index_of(WS).expect("an index").words.get("b"), Some(&b));
    });
}

// TS: sqlite-transcripts.test.ts:481
#[test]
fn words_of_reads_rows_then_the_workspaces_pending_and_keeps_what_the_thread_had() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        ctx.rows.seed(WS, vec![stamped(start_v(WS, "x", "u1", "x first"), 1, 20), stamped(start_v(WS, "y", "u2", "y first"), 2, 20)]);
        ctx.hold_index(WS, TranscriptIndex { pos: 2, ..TranscriptIndex::default() });
        let area = ctx.area();
        area.record(event(delta_v(WS, "x", "u1", "fresh-zebra words")));
        area.record(event(delta_v(WS, "y", "u2", "another thread's")));
        let had = ThreadWords { opened: Some(JsNumber(777.0)), ..cold("x first", 7, 5) };
        let back = area.words_of(WS, "x", Some(&had)).expect("the read");
        assert_eq!(
            back,
            ThreadWords { lines: vec!["x first".into(), "fresh-zebra words".into()], open: Some("u1:".into()), last: JsNumber(1_000_000.0), size: 24, first: Some("x first".into()), opened: Some(JsNumber(777.0)), cold: None, room: None },
            "the rows' words and then the events not yet flushed, the later last, the opened kept"
        );
        let fresh = area.words_of(WS, "x", None).expect("the read");
        assert_eq!(fresh.opened, None);
        let had_later = cold("x first", 7, 9_999_999);
        assert_eq!(area.words_of(WS, "x", Some(&had_later)).expect("the read").last, JsNumber(9_999_999.0), "last is the later of the two");
    });
}

#[test]
fn opened_words_ranks_a_thread_as_just_spoken() {
    local(async {
        let ctx = FakeTranscriptsCtx::new();
        ctx.hold_index(WS, index_of_words(vec![("cold", cold("c", 10, 50)), ("warm", words(&["warm line"], 70))]));
        let area = ctx.area();
        area.opened_words(WS, "warm");
        assert_eq!(ctx.index_of(WS).expect("an index").words["warm"].opened, Some(JsNumber(1_000_000.0)), "opened is now");
        assert_eq!(ctx.clock.pending(), 0, "a warm thread asks for no balance");
        ctx.clock.advance(250);
        area.opened_words(WS, "cold");
        let held = ctx.index_of(WS).expect("an index");
        assert_eq!(held.words["cold"].opened, Some(JsNumber(1_000_250.0)));
        assert_eq!(ctx.clock.pending(), 1, "a thread that was let go balances, which asks for warming");

        area.opened_words(WS, "nobody");
        area.opened_words("ws9", "cold");
        assert!(!ctx.index_of(WS).expect("an index").words.contains_key("nobody"), "nothing is made for a thread the index holds no words for");
        assert!(ctx.index_of("ws9").is_none(), "nor an index for a workspace that holds none");
    });
}
