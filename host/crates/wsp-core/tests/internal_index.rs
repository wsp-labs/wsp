// SPDX-License-Identifier: AGPL-3.0-only
//! The transcript index: runtime/src/types/internal.ts:229-554 at 65f6b89c7, spec/internal-index.md. Expected values
//! are the TS's own answers (tests/fixtures/core_types/internal.json); the index is compared as the JSON `indexBytes`
//! writes, objects compared by key and value and every array by order.
mod common;

use std::collections::BTreeSet;

use common::{ev, fixture, wire, wires};
use serde_json::{json, Value};
use wsp_core::types::internal::*;
use wsp_core::types::pending::BlobMark;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::session_events::SessionEvent;

fn events_of(scene: &str) -> Vec<SessionEvent> {
    wires(&fixture("internal")[scene]["events"])
}

fn index_json(index: &TranscriptIndex, of: Option<BlobMark>, lines: bool) -> Value {
    serde_json::from_slice(&index_bytes(index, of, lines)).expect("index bytes are JSON")
}

fn words(v: Value) -> ThreadWords {
    wire(&v)
}

// internal.ts:376-433: a chat folds into words by thread (a reply's pieces joined, a subagent's and a tool's left
// out, an earlier person row kept and a tool row not), the newest start per thread, the cut of each thread's newest
// end that started, each session's facts, the requests taken, and the highest pos. A copied row adds only words.
#[test]
fn a_chat_folds_into_the_index_the_ts_writes() {
    let f = fixture("internal");
    let index = index_of(&events_of("chat"));
    assert_eq!(index_json(&index, None, true), f["chat"]["index"]);
}

// internal.ts:390-392: a turn whose agent never took its prompt frees the request its start took, never a steer's.
#[test]
fn a_promptless_end_frees_the_requests_its_turn_started() {
    let f = fixture("internal");
    let index = index_of(&events_of("promptless"));
    assert_eq!(index_json(&index, None, true), f["promptless"]["index"]);
    assert_eq!(index.taken.keys().cloned().collect::<Vec<_>>(), vec!["r2", "r3"]);
}

// internal.ts:305-362: subagents start, gather model and usage, end with their last line or failure; an end with no
// start is dropped; the turn's end stops its own running children and drops their progress duration; a child of
// another turn runs on; a row with no thread names none.
#[test]
fn subagents_fold_into_each_threads_children() {
    let f = fixture("internal");
    let index = index_of(&events_of("children"));
    assert_eq!(index_json(&index, None, true), f["children"]["index"]);
    let kids = &index.children["t1"];
    assert_eq!(kids.keys().cloned().collect::<Vec<_>>(), vec!["k1", "k2", "k3", "k4"]);
}

// internal.ts:365-371: a dropped start row takes its child only while it is that child's newest start row, and a
// thread left with none leaves the map; the others keep their order.
#[test]
fn forget_child_takes_a_child_only_by_its_newest_start_row() {
    let f = &fixture("internal")["forget"];
    let events = events_of("children");
    let mut index = index_of(&events);
    forget_child(&mut index, &events[1]);
    assert_eq!(index_json(&index, None, true), f["afterWrongRow"]);
    let mut own: Value = fixture("internal")["children"]["events"][1].clone();
    own["at"] = json!(3);
    forget_child(&mut index, &wire(&own));
    assert_eq!(index_json(&index, None, true), f["afterOwnRow"]);
    let mut lone = index_of(&events[8..9]);
    forget_child(&mut lone, &events[8]);
    assert_eq!(index_json(&lone, None, true), f["lone"]);
    assert!(!lone.children.contains_key("t1"));
}

// internal.ts:510-511: kept without lines, each thread's words are cold and `of` names the transcript.
#[test]
fn index_bytes_without_lines_writes_every_thread_cold() {
    let f = fixture("internal");
    let index = index_of(&events_of("chat"));
    let mark = BlobMark { bytes: JsNumber(99.0), at: JsNumber(7.0) };
    assert_eq!(index_json(&index, Some(mark), false), f["indexBytesCold"]);
    assert!(index.words["t1"].cold.is_none(), "writing cold leaves the index itself warm");
}

// internal.ts:510: `of` is left out where none is given.
#[test]
fn index_bytes_leaves_out_an_absent_mark() {
    let index = index_of(&events_of("promptless"));
    let v = index_json(&index, None, true);
    assert!(v.get("of").is_none());
    assert_eq!(
        v.as_object().unwrap().keys().cloned().collect::<Vec<_>>(),
        vec!["words", "starts", "cut", "facts", "taken", "children", "trimmed", "pos"]
    );
}

// internal.ts:512-526: an index read back is the index written, `of` and the open reply included.
#[test]
fn index_read_gives_back_the_index_written() {
    let f = &fixture("internal")["reads"]["full"];
    let raw = fixture("internal")["indexBytesRaw"].as_str().unwrap().to_string();
    let read = index_read(raw.as_bytes()).expect("reads");
    assert_eq!(read.of, Some(BlobMark { bytes: JsNumber(99.0), at: JsNumber(7.0) }));
    assert_eq!(index_json(&read.index, read.of, true), f["index"]);
    assert_eq!(read.index.words["t1"].open.as_deref(), Some("u1:m2"));
}

// Runtime invariant 5 (packages/runtime/AGENTS.md), the index's half: an index that does not parse, or one written
// before it held taken, children and pos, reads as none, so boot makes it again from its transcript.
#[test]
fn invariant_5_an_index_that_does_not_read_is_none() {
    let reads = &fixture("internal")["reads"];
    for (name, bytes) in [
        ("notJson", "{nope"),
        ("noTaken", r#"{"words":[],"starts":[],"cut":[],"facts":[],"children":[],"pos":0}"#),
        ("noChildren", r#"{"words":[],"starts":[],"cut":[],"facts":[],"taken":[],"pos":0}"#),
        ("noPos", r#"{"words":[],"starts":[],"cut":[],"facts":[],"taken":[],"children":[]}"#),
        ("empty", ""),
    ] {
        assert_eq!(reads[name], Value::Null);
        assert!(index_read(bytes.as_bytes()).is_none(), "{name} reads as none");
    }
}

// internal.ts:517-521: words written before WORDS_HELD gain their size and first line as they are read.
#[test]
fn index_read_fills_the_size_and_first_line_of_old_words() {
    let f = &fixture("internal")["reads"];
    let raw = r#"{"words":[["t1",{"lines":["hello there","x"],"last":5}],["t2",{"lines":[],"last":0}]],"starts":[],"cut":[],"facts":[],"taken":[],"children":[],"pos":3}"#;
    let read = index_read(raw.as_bytes()).expect("reads");
    assert_eq!(read.of, None);
    assert_eq!(index_json(&read.index, None, true), f["oldWords"]["index"]);
    assert_eq!(read.index.words["t1"].size, 12);
    assert_eq!(read.index.words["t1"].first.as_deref(), Some("hello there"));
    assert_eq!(read.index.words["t2"].first, None);
}

// internal.ts:522: an index with no trimmed list reads with none trimmed; its children keep their keys.
#[test]
fn index_read_takes_a_missing_trimmed_list_as_empty() {
    let f = &fixture("internal")["reads"];
    let raw = r#"{"words":[],"starts":[["t1","s1"]],"cut":[["t1",true]],"facts":[["s1",{"model":"m"}]],"taken":[],"children":[["t1",[["k",{"id":"k","title":"k","state":"running","startedAt":1}]]]],"pos":9}"#;
    let read = index_read(raw.as_bytes()).expect("reads");
    assert_eq!(index_json(&read.index, None, true), f["noTrimmed"]["index"]);
    assert!(read.index.trimmed.is_empty());
    assert_eq!(read.index.pos, 9);
}

// internal.ts:436-440: one thread's words off its own events, and an empty set for a thread with none.
#[test]
fn thread_words_of_reads_one_thread() {
    let f = &fixture("internal")["words"];
    let events = events_of("chat");
    assert_eq!(serde_json::to_value(thread_words_of(&events, "t1")).unwrap(), f["t1"]);
    assert_eq!(serde_json::to_value(thread_words_of(&events, "nobody")).unwrap(), f["none"]);
}

fn long_words() -> ThreadWords {
    words(json!({ "lines": ["aaaa", "bbbbbb", "cc", "😀😀"], "last": 9, "size": 16, "first": "aaaa" }))
}

// internal.ts:443: cold words keep last, size, first and opened, and no lines.
#[test]
fn cold_words_let_every_line_go() {
    let f = &fixture("internal")["words"];
    let mut w = long_words();
    w.opened = Some(JsNumber(4.0));
    assert_eq!(serde_json::to_value(cold_words(&w)).unwrap(), f["cold"]);
}

// internal.ts:445-450: the newest lines that fit in `room` UTF-16 units are kept, the room noted; warm words that fit
// whole come back unchanged; cold words are cut again however much room there is.
#[test]
fn newest_of_keeps_the_newest_lines_that_fit() {
    let f = &fixture("internal")["words"];
    let w = long_words();
    assert_eq!(serde_json::to_value(newest_of(&w, 16)).unwrap(), f["newestFits"]);
    assert_eq!(serde_json::to_value(newest_of(&w, 6)).unwrap(), f["newest6"]);
    assert_eq!(serde_json::to_value(newest_of(&w, 5)).unwrap(), f["newest5"]);
    assert_eq!(serde_json::to_value(newest_of(&w, 0)).unwrap(), f["newest0"]);
    let mut cold = long_words();
    cold.cold = Some(true);
    assert_eq!(serde_json::to_value(newest_of(&cold, 100)).unwrap(), f["newestOfCold"]);
}

// internal.ts:452: warm words hold their size; cold ones the UTF-16 units of the lines they kept.
#[test]
fn held_of_counts_utf16_units() {
    let f = &fixture("internal")["words"];
    assert_eq!(held_of(&long_words()), f["heldWarm"].as_u64().unwrap() as usize);
    let cold = words(json!({ "lines": ["ab", "😀"], "last": 9, "size": 16, "first": "aaaa", "cold": true }));
    assert_eq!(held_of(&cold), f["heldCold"].as_u64().unwrap() as usize);
}

// internal.ts:253-256: a thread's first line is its first 200 UTF-16 units.
#[test]
fn the_first_line_is_cut_at_two_hundred_utf16_units() {
    let f = &fixture("internal")["words"];
    let prompt = format!("{}{}", "é".repeat(150), "😀".repeat(40));
    let w = thread_words_of(&[wire(&ev("session.steer", json!({ "threadId": "t", "prompt": prompt })))], "t");
    assert_eq!(w.first.as_deref(), f["firstOfLong"].as_str());
    assert_eq!(first_of(&prompt), f["firstOfLong"].as_str().unwrap());
}

// internal.ts:256, the one forced divergence (spec/internal-index.md): where the 200th unit is the first half of a
// pair, the TS keeps a lone surrogate (the oracle wrote "\ud83d"); a Rust string cannot, so the half is dropped.
#[test]
fn the_first_line_drops_a_surrogate_pair_the_cut_splits() {
    let f = &fixture("internal")["words"];
    assert!(f["firstOfSplit"].as_str().unwrap().ends_with("\\ud83d\""));
    let prompt = format!("{}😀tail", "é".repeat(199));
    assert_eq!(first_of(&prompt), "é".repeat(199));
}

// internal.ts:532-542: the first line of each thread holding the lowercased words, one hit a thread, the threads the
// caller cannot drive skipped, and every cold thread with no line kept counted as unsearched.
#[test]
fn search_finds_one_hit_a_thread_and_counts_cold_threads() {
    let f = &fixture("internal")["search"];
    let mut index = index_of(&events_of("chat"));
    index.words.insert("t9".into(), words(json!({ "lines": [], "last": 50, "size": 40, "cold": true })));
    index
        .words
        .insert("t8".into(), words(json!({ "lines": ["needle in a cold thread"], "last": 51, "size": 400, "cold": true, "room": 30 })));
    let hits = |found: &[SearchedHit]| -> Value {
        Value::Array(found.iter().map(|h| json!({ "hit": serde_json::to_value(&h.hit).unwrap(), "last": h.last })).collect())
    };
    let mut found = Vec::new();
    assert_eq!(search_index("w1", &index, "bug", &|_| true, &mut found), 1);
    assert_eq!(hits(&found), f["bug"]["found"]);
    let mut found = Vec::new();
    assert_eq!(search_index("w1", &index, "needle", &|t| t != "t9", &mut found), 0);
    assert_eq!(hits(&found), f["needle"]["found"]);
    let mut found = Vec::new();
    search_index("w1", &index, "ok", &|_| true, &mut found);
    assert_eq!(hits(&found), f["ok"]);
    let mut found = Vec::new();
    search_index("w1", &index, "emoji", &|_| true, &mut found);
    assert_eq!(hits(&found), f["emoji"]);
}

// internal.ts:544-548: 40 units before, 80 after, whitespace runs as one space, trimmed, an ellipsis where it goes on.
#[test]
fn snippet_around_matches_the_ts() {
    for row in fixture("internal")["search"]["snippets"].as_array().unwrap() {
        if row[0] == "split" {
            let text = format!("a{}hit", "😀".repeat(30));
            let want: String = serde_json::from_str(row[3].as_str().unwrap()).unwrap();
            assert_eq!(snippet_around(&text, 61, 3), want);
            continue;
        }
        let (text, at, len) = (row[0].as_str().unwrap(), row[1].as_u64().unwrap() as usize, row[2].as_u64().unwrap() as usize);
        assert_eq!(snippet_around(text, at, len), row[3].as_str().unwrap(), "{text:?}");
    }
}

// spec/internal-index.md: a window edge inside a surrogate pair drops that half, as the first line does.
#[test]
fn snippet_around_drops_a_pair_its_window_splits() {
    // "b", 30 emoji, "x", "hit": the hit is at unit 62, so the window starts at unit 22, the low half of the emoji
    // at units 21 and 22; the TS keeps that half, the port drops it and keeps the 19 whole emoji after it.
    let text = format!("b{}xhit", "😀".repeat(30));
    let got = snippet_around(&text, 62, 3);
    assert_eq!(got, format!("…{}xhit", "😀".repeat(19)));
    assert!(!got.contains('\u{FFFD}'));
}

// internal.ts:462-466: the threads named are folded again off their own events and move to the end of each map; the
// requests they took go and come back only as their events take them; facts stay.
#[test]
fn reindex_threads_refolds_only_the_threads_named() {
    let f = fixture("internal");
    let events = events_of("chat");
    let mut index = index_of(&events);
    let t1: Vec<SessionEvent> =
        fixture("internal")["chat"]["events"].as_array().unwrap().iter().filter(|e| e["threadId"] == "t1").skip(4).map(wire).collect();
    let threads: BTreeSet<String> = ["t1".to_string()].into();
    reindex_threads(&mut index, &threads, &|t| if t == "t1" { t1.clone() } else { vec![] });
    assert_eq!(index_json(&index, None, true), f["reindex"]);
    assert_eq!(index.words.keys().cloned().collect::<Vec<_>>(), vec!["t2", "t3", "t1"]);
}

// internal.ts:377: pos is the highest seen and never moves back.
#[test]
fn the_index_position_never_moves_back() {
    let mut index = empty_index();
    fold_event(&mut index, &wire(&ev("session.start", json!({ "threadId": "t1", "turnId": "u1", "pos": 5 }))));
    fold_event(
        &mut index,
        &wire(&ev("session.end", json!({ "threadId": "t1", "turnId": "u1", "pos": 3, "exitCode": 0, "sawResult": true }))),
    );
    assert_eq!(index.pos, 5);
    assert_eq!(index.cut.get("t1"), Some(&false));
}

fn turn_events() -> Vec<SessionEvent> {
    [
        ev("session.start", json!({ "threadId": "t1", "turnId": "u1", "at": 1 })),
        ev("session.delta", json!({ "threadId": "t1", "turnId": "u1", "kind": "text", "text": "a", "line": 1 })),
        ev("session.delta", json!({ "threadId": "t1", "turnId": "u1", "kind": "text", "text": "b", "line": 7 })),
        ev("session.delta", json!({ "threadId": "t1", "turnId": "u1", "kind": "text", "text": "c" })),
        ev("session.subagent", json!({ "threadId": "t1", "turnId": "u1", "task": "k", "state": "running", "line": 2 })),
        ev("session.compacted", json!({ "threadId": "t1", "turnId": "u1", "line": 3 })),
        ev("session.retry", json!({ "threadId": "t1", "turnId": "u1", "attempt": 1, "line": 4 })),
        ev("session.retry", json!({ "threadId": "t1", "turnId": "u1", "attempt": 2 })),
        ev("session.delta", json!({ "threadId": "t1", "turnId": "u2", "kind": "text", "text": "other", "line": 99 })),
    ]
    .iter()
    .map(wire)
    .collect()
}

fn written(w: &TurnWritten) -> Value {
    let mut v = json!({ "lines": w.lines, "subagents": w.subagents, "compactions": w.compactions, "retries": w.retries });
    if let Some(r) = &w.reply {
        v["reply"] = serde_json::to_value(r).unwrap();
    }
    v["started"] = json!(w.started);
    v["changes"] = json!(w.changes);
    v
}

// Runtime invariant 2 (packages/runtime/AGENTS.md): a re-opened turn writes no row twice, because turnWritten reads
// each count off the line stamp of the newest surviving row of that turn and kind, never off how many survive. The
// TS reads the newest delta even when it carries no line (u1's newest delta has none, so lines is 0); that is pinned
// as it is and filed as an after-port ticket.
#[test]
fn invariant_2_turn_written_reads_the_newest_stamp_of_each_kind() {
    let f = &fixture("internal")["turnWritten"];
    let events = turn_events();
    assert_eq!(written(&turn_written(&events, "u1")), f["u1"]);
    assert_eq!(written(&turn_written(&events[2..], "u1")), f["evicted"]);
    assert_eq!(written(&turn_written(&events, "u7")), f["none"]);
}

// internal.ts:494: with the unlined delta gone, the newest lined delta's stamp is the count, not the survivors'.
#[test]
fn turn_written_counts_by_stamp_after_the_head_was_evicted() {
    let events = turn_events();
    let kept: Vec<SessionEvent> = vec![events[2].clone(), events[4].clone()];
    let w = turn_written(&kept, "u1");
    assert_eq!((w.lines, w.subagents, w.started), (7, 2, true));
}

// internal.ts:497-502: a reply and a card are read off the newest done and changes; a retry with no line is the host's
// own and counts nothing; a start alone, or a reply alone, is started.
#[test]
fn turn_written_reads_reply_card_and_start() {
    let f = &fixture("internal")["turnWritten"];
    let mut events = turn_events();
    events.push(wire(&ev("session.done", json!({ "threadId": "t1", "turnId": "u1", "result": { "status": "completed" } }))));
    events
        .push(wire(&ev("session.changes", json!({ "threadId": "t1", "turnId": "u1", "from": "a", "to": "b", "files": [], "moved": [] }))));
    assert_eq!(written(&turn_written(&events, "u1")), f["u1Done"]);
    let base = turn_events();
    assert_eq!(written(&turn_written(&base[..1], "u1")), f["startOnly"]);
    assert_eq!(written(&turn_written(&base[7..8], "u1")), f["retryUnlined"]);
    let done = [wire(&ev("session.done", json!({ "turnId": "u5", "result": { "status": "failed", "error": "x" } })))];
    assert_eq!(written(&turn_written(&done, "u5")), f["doneOnly"]);
}

fn view() -> SessionView {
    wire(
        &json!({ "id": "s1", "workspaceId": "w1", "harness": "claude", "status": "completed", "lastLine": "said", "failure": "why", "subagents": [{ "id": "k", "title": "K", "state": "done", "asked": "q", "lastLine": "l", "failure": "f", "startedAt": 1, "depth": 1 }] }),
    )
}

// internal.ts:336-339: a listing that only names leaves out what turns said and were asked, subagents' too.
#[test]
fn named_only_leaves_out_what_turns_said() {
    let f = &fixture("internal")["views"];
    assert_eq!(serde_json::to_value(named_only(&view())).unwrap(), f["namedOnly"]);
    let bare: SessionView = wire(&json!({ "id": "s", "workspaceId": "w1", "harness": "claude", "status": "running" }));
    assert_eq!(serde_json::to_value(named_only(&bare)).unwrap(), f["namedOnlyBare"]);
}

// internal.ts:342-348: each line set where the end gave one and taken off where it gave none.
#[test]
fn write_lines_sets_and_clears_each_line() {
    let f = &fixture("internal")["views"];
    let mut a = view();
    write_lines(&mut a, Some("new".into()), None);
    assert_eq!(serde_json::to_value(&a).unwrap(), f["lines1"]);
    let mut b = view();
    write_lines(&mut b, None, Some("f2".into()));
    assert_eq!(serde_json::to_value(&b).unwrap(), f["lines2"]);
}

// internal.ts:351-362: end_children stops only the running children of that turn.
#[test]
fn end_children_stops_only_that_turns_running_children() {
    let events = events_of("children");
    let mut index = index_of(&events[..9]);
    end_children(&mut index, "t1", Some("u0"), Some(50.0));
    let k4 = &index.children["t1"]["k4"];
    assert_eq!(serde_json::to_value(&k4.view.state).unwrap(), json!("stopped"));
    assert_eq!(k4.view.ended_at.map(|n| n.0), Some(50.0));
    let k1 = &index.children["t1"]["k1"];
    assert_eq!(serde_json::to_value(&k1.view.state).unwrap(), json!("running"));
    end_children(&mut index, "t1", Some("u1"), None);
    let k1 = &index.children["t1"]["k1"];
    assert_eq!(k1.view.ended_at.map(|n| n.0), Some(2.0), "no end time reads the start time");
}
