// SPDX-License-Identifier: AGPL-3.0-only
// The transcript reader unit against the TS oracle (tests/fixtures/store/transcript_reader.json, made by the
// `transcript_reader` case of oracle/store.oracle.ts, which builds the same six events).
mod common;

use common::*;
use serde_json::{json, Value};
use wsp_store::transcript_reader::*;
use wsp_wire::views::session_events_more::SessionEvent;

fn oracle_events() -> Vec<SessionEvent> {
    vec![
        text(Some("a"), 1, &"x".repeat(10)),
        text(Some("b"), 2, "other"),
        text(Some("a"), 3, &"é".repeat(10)),
        text(None, 4, "none"),
        delta(Some("a"), None, "unnumbered", "text"),
        text(Some("a"), 6, &"y".repeat(30)),
    ]
}

fn read_of(before: Option<i64>, limit: usize, bytes: u64, strict: bool) -> ThreadRead {
    ThreadRead { before, limit, bytes, strict, shape: None }
}

fn picked(events: &[SessionEvent]) -> Vec<Value> {
    events
        .iter()
        .map(|e| match pos_of(e) {
            Some(p) => json!(p),
            None => value(e)["text"].clone(),
        })
        .collect()
}

#[test]
fn every_read_of_thread_a_picks_the_oracles_events_and_total() {
    let f = fixture("transcript_reader");
    let reads = &f["reads"];
    let sizes: Vec<u64> = reads["sizes"].as_array().unwrap().iter().map(|n| n.as_u64().unwrap()).collect();
    let two = sizes[5] + sizes[4];
    let big = 1_000_000_000;
    let all = usize::MAX;
    let cases: Vec<(&str, ThreadRead)> = vec![
        ("all", read_of(None, all, big, false)),
        ("limit2", read_of(None, 2, big, false)),
        ("limit0", read_of(None, 0, big, false)),
        ("before6", read_of(Some(6), all, big, false)),
        ("before1", read_of(Some(1), all, big, false)),
        ("before0", read_of(Some(0), all, big, false)),
        ("bytes0", read_of(None, all, 0, false)),
        ("bytes0strict", read_of(None, all, 0, true)),
        ("bytesExactTwo", read_of(None, all, two, false)),
        ("bytesOneShort", read_of(None, all, two - 1, false)),
        ("strictExactOne", read_of(None, all, sizes[5], true)),
        ("strictOneShort", read_of(None, all, sizes[5] - 1, true)),
    ];
    let events = oracle_events();
    for (name, o) in cases {
        let got = read_thread(&events, "a", &o);
        assert_eq!(picked(&got.events), reads[name]["pos"].as_array().unwrap().clone(), "case {name}: events");
        assert_eq!(got.total, reads[name]["total"].as_u64().unwrap(), "case {name}: total");
    }
}

#[test]
fn reading_thread_empty_finds_nothing_for_an_event_with_no_thread() {
    let f = fixture("transcript_reader");
    let got = read_thread(&oracle_events(), "", &read_of(None, usize::MAX, 1_000_000_000, false));
    assert_eq!(f["reads"]["threadEmpty"], json!({"events": [], "total": 0}));
    assert_eq!(got, ThreadPick { events: vec![], total: 0 });
}

fn newest_of_a() -> Vec<SessionEvent> {
    let mut e = oracle_events();
    e.retain(|e| value(e)["threadId"] == "a");
    e.reverse();
    e
}

#[test]
fn pick_newest_hands_back_copies_oldest_first() {
    let newest = newest_of_a();
    let mut oldest_first = newest.clone();
    oldest_first.reverse();
    let got = pick_newest(newest, &read_of(None, usize::MAX, 1_000_000_000, false));
    assert_eq!(got, oldest_first);
    assert_eq!(picked(&got), vec![json!(1), json!(3), json!("unnumbered"), json!(6)]);
}

fn cut_result(e: &SessionEvent) -> SessionEvent {
    let mut v = value(e);
    let whole = v["text"].as_str().unwrap().chars().count();
    v["text"] = json!("z".repeat(2048));
    v["cut"] = json!(whole);
    event(v)
}

#[test]
fn pick_newest_applies_the_shape_function_to_each_event() {
    let long1 = delta(Some("a"), Some(1), &"z".repeat(2049), "tool_result");
    let short2 = text(Some("a"), 2, "short");
    let long3 = delta(Some("a"), Some(3), &"z".repeat(3000), "tool_result");
    let o = ThreadRead { shape: Some(head_shape), ..read_of(None, usize::MAX, 1_000_000_000, false) };
    let got = pick_newest(vec![long3.clone(), short2.clone(), long1.clone()], &o);
    assert_eq!(got, vec![cut_result(&long1), short2, cut_result(&long3)]);
}

#[test]
fn pick_newest_skips_events_at_or_above_before_and_goes_on() {
    let e5 = text(Some("a"), 5, "five");
    let e2 = text(Some("a"), 2, "two");
    let nopos = delta(Some("a"), None, "no pos", "text");
    let e1 = text(Some("a"), 1, "one");
    let got = pick_newest(vec![e5, e2, nopos.clone(), e1.clone()], &read_of(Some(2), usize::MAX, 1_000_000_000, false));
    assert_eq!(got, vec![e1, nopos]);
}

fn oracle_delta() -> SessionEvent {
    delta(Some("a"), Some(1), "😀 é \u{1}", "text")
}

fn oracle_end() -> (SessionEvent, Value) {
    let v = json!({"type": "session.end", "workspaceId": "w", "sessionId": "s", "pos": 2, "exitCode": null, "sawResult": false, "at": 1792923718163.5});
    (event(v.clone()), v)
}

#[test]
fn event_json_of_a_delta_is_the_oracles_text_and_size() {
    let f = fixture("transcript_reader");
    let got = event_json(&oracle_delta());
    assert_eq!(got, EventJson { json: f["eventJson"][0]["json"].as_str().unwrap().to_string(), bytes: f["eventJson"][0]["bytes"].as_u64().unwrap() });
}

#[test]
fn event_json_of_a_session_end_is_the_oracles_size_and_value() {
    let f = fixture("transcript_reader");
    let (e, v) = oracle_end();
    let got = event_json(&e);
    assert_eq!(got.bytes, f["eventJson"][1]["bytes"].as_u64().unwrap());
    assert_eq!(serde_json::from_str::<Value>(&got.json).unwrap(), v);
    assert_eq!(got.bytes, utf16(&got.json));
}

#[test]
fn event_bytes_is_the_size_event_json_counts() {
    let f = fixture("transcript_reader");
    let (end, _) = oracle_end();
    assert_eq!(event_bytes(&oracle_delta()), f["eventJson"][0]["bytes"].as_u64().unwrap());
    assert_eq!(event_bytes(&oracle_delta()), event_json(&oracle_delta()).bytes);
    assert_eq!(event_bytes(&end), f["eventJson"][1]["bytes"].as_u64().unwrap());
    assert_eq!(event_bytes(&end), event_json(&end).bytes);
}

#[test]
fn event_bytes_count_utf16_units_not_utf8_bytes() {
    let got = event_json(&text(Some("a"), 1, "😀"));
    assert_eq!(
        got.json,
        r#"{"type":"session.delta","workspaceId":"w","sessionId":"s","threadId":"a","pos":1,"kind":"text","text":"😀"}"#
    );
    assert_eq!(got.bytes, utf16(&got.json));
    assert_ne!(got.bytes, got.json.len() as u64);
}

#[test]
fn head_shape_cuts_a_long_tool_result_to_2048_and_marks_its_whole_length() {
    let f = fixture("transcript_reader");
    let long = &f["head"]["long"];
    let input = delta(Some("a"), Some(1), &"z".repeat(2049), "tool_result");
    let mut want = value(&input);
    want["text"] = json!("z".repeat(long["textLength"].as_u64().unwrap() as usize));
    want["cut"] = long["cut"].clone();
    assert_eq!(long["cut"], json!(2049));
    assert_eq!(value(&head_shape(&input)), want);
}

#[test]
fn head_shape_leaves_a_tool_result_of_exactly_2048_alone() {
    let input = delta(Some("a"), Some(1), &"z".repeat(2048), "tool_result");
    assert_eq!(fixture("transcript_reader")["head"]["exact"]["textLength"], json!(2048));
    assert_eq!(value(&head_shape(&input)), value(&input));
}

#[test]
fn head_shape_leaves_a_long_text_delta_alone() {
    let input = text(Some("a"), 1, &"z".repeat(5000));
    assert_eq!(fixture("transcript_reader")["head"]["textLong"]["textLength"], json!(5000));
    assert_eq!(value(&head_shape(&input)), value(&input));
}

#[test]
fn head_shape_cuts_a_patch_and_marks_it_and_keeps_the_short_text() {
    let f = fixture("transcript_reader");
    let patched = &f["head"]["patched"];
    let lines: Vec<String> = (0..40).map(|i| format!("+line {i} {}", "p".repeat(90))).collect();
    let mut v = value(&delta(Some("a"), Some(1), "short", "tool_result"));
    v["patch"] = json!([
        {"path": "a.ts", "hunks": [{"oldStart": 1, "oldLines": 0, "newStart": 1, "newLines": 40, "lines": lines}]},
        {"path": "b.ts", "hunks": []}
    ]);
    let input = event(v.clone());
    let mut want = v;
    want["patch"] = patched["patch"].clone();
    want["patchCut"] = json!(true);
    assert_eq!(patched["patchCut"], json!(true));
    assert_eq!(value(&head_shape(&input)), want);
}

#[test]
fn head_shape_drops_the_half_of_an_emoji_the_cut_would_split() {
    let f = fixture("transcript_reader");
    assert_eq!(f["head"]["emoji"]["cut"], json!(2053));
    let input = delta(Some("a"), Some(1), &format!("{}😀tail", "z".repeat(2047)), "tool_result");
    let mut want = value(&input);
    want["text"] = json!("z".repeat(2047));
    want["cut"] = json!(2053);
    assert_eq!(value(&head_shape(&input)), want);
}
