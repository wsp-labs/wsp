// SPDX-License-Identifier: AGPL-3.0-only
//! transcripts_rows (spec/transcripts_rows.md): record, flush, caps, reads and held words. Expected values read off
//! transcripts.ts at 65f6b89c7 and the TS tests named above each test.
#![allow(dead_code, unused_imports)]

#[path = "support_b/mod.rs"]
mod support;
include!("transcripts_rows/world.rs");

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
        let first = event(
            json!({ "type": "session.start", "workspaceId": WS, "sessionId": "s1", "turnId": "u1", "threadId": "t1", "prompt": "go", "at": 1_000_000, "pos": 1 }),
        );
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
            let mut v =
                json!({ "type": "session.earlier", "workspaceId": WS, "sessionId": "s1", "threadId": "t1", "who": who, "text": text });
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
        ctx.clock.advance(0);
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
        ctx.clock.advance(0);
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

        ctx.clock.advance(TRANSCRIPT_FLUSH_MS as i64 - 1);
        settle().await;
        assert!(ctx.rows.kept(WS).is_empty(), "the timer is 250 ms from the first event, not moved by the next");
        ctx.clock.advance(1);
        settle().await;
        assert_eq!(ctx.rows.positions(WS), vec![1, 2]);
        assert_eq!(ctx.rows.positions("ws2"), vec![1]);
        ctx.clock.advance(0);
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
        ctx.clock.advance(TRANSCRIPT_FLUSH_MS as i64 * 4);
        settle().await;
        assert!(ctx.rows.kept(WS).is_empty(), "nothing flushes a delta that waits alone");

        area.record(event(plan_v(WS, "t1", "step")));
        assert_eq!(ctx.clock.pending(), 1);
        ctx.clock.advance(TRANSCRIPT_FLUSH_MS as i64);
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

// TS: runtime-history.test.ts:1061
// TS: thread-head.test.ts:258
#[test]
fn kept_event_clips_a_long_tool_result_with_its_bytes() {
    let output = format!("first line\n{}", "r".repeat(TOOL_RESULT_KEPT * 4));
    let long = delta_with(json!({ "kind": "tool_result", "text": output, "toolUseId": "tu_1", "exitCode": 2, "durationMs": 4100 }));
    let clipped = kept_event(&long);
    assert_eq!(
        clipped,
        delta_with(
            json!({ "kind": "tool_result", "text": output.chars().take(TOOL_RESULT_KEPT).collect::<String>(), "bytes": output.len(), "toolUseId": "tu_1", "exitCode": 2, "durationMs": 4100 })
        ),
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

// TS: thread-head.test.ts:269
#[test]
fn kept_event_cuts_a_patch_and_marks_it_last() {
    let lines = hundred_char_lines();
    let big = json!([{ "path": "/root/app/big.ts", "hunks": [hunk(1, lines.clone()), hunk(101, lines.clone())] }]);
    let e = delta_with(json!({ "kind": "tool_result", "text": "updated", "patch": big, "toolUseId": "tu_2" }));
    let kept = kept_event(&e);
    let second = json!({ "oldStart": 101, "oldLines": 32, "newStart": 101, "newLines": 31, "lines": lines[..63].to_vec() });
    let want = json!([{ "path": "/root/app/big.ts", "hunks": [hunk(1, lines.clone()), second] }]);
    assert_eq!(
        kept,
        delta_with(json!({ "kind": "tool_result", "text": "updated", "patch": want, "patchCut": true, "toolUseId": "tu_2" })),
        "the first hunk whole, the second to the lines that fit in 16,384 characters"
    );
    let json = json_of(&kept);
    assert_eq!(json.as_object().expect("an object").keys().next_back().map(String::as_str), Some("patchCut"), "patchCut goes last");
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
        ctx.clock.advance(TRANSCRIPT_FLUSH_MS as i64 - 1);
        settle().await;
        assert_eq!(ctx.rows.transactions.get(), 0, "nothing is written before the debounce");
        ctx.clock.advance(1);
        settle().await;
        assert_eq!(ctx.rows.transactions.get(), 1);
        assert_eq!(ctx.rows.kept(WS).len(), 21);
        ctx.clock.advance(TRANSCRIPT_FLUSH_MS as i64 * 2);
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
        assert_eq!(
            ctx.rows.kept(WS).iter().map(|e| field(e, "type")).collect::<Vec<_>>(),
            vec![json!("session.start"), json!("session.plan")]
        );
        ctx.clock.advance(0);
        assert_eq!(ctx.clock.pending(), 0, "no timer is left behind");
        ctx.clock.advance(TRANSCRIPT_FLUSH_MS as i64 * 2);
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
        ctx.clock.advance(TRANSCRIPT_FLUSH_MS as i64 * 2);
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
            vec![
                "atomically",
                "append ws1 4",
                "total ws1 t1",
                "sizes ws1 t1",
                "at ws1 1",
                "remove ws1 1,2,3",
                "thread ws1 t1",
                "put_index ws1"
            ],
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
        assert_eq!(
            ctx.store.logged(),
            vec!["deleteBlob attachments/t1.req_a.0", "put attachment-keys/ws1"],
            "its blob goes and its thread leaves the document"
        );
        assert_eq!(
            ctx.store.get_now("attachment-keys", WS),
            Some(json!({ "threads": { "t2": [{ "key": "t2.req_z.0", "mediaType": "image/png" }] } }))
        );
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
        assert_eq!(ctx.rows.positions(WS)[0], 4, "the steer row was among those dropped");
        assert_eq!(ctx.rows_called_of(&["at"]), vec!["at ws1 1"], "only starts are read back before they go");
        assert!(ctx.store.logged().is_empty(), "a flush's trim lets go of a start's images alone");
    });
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
        assert_eq!(
            ids,
            vec![(json!("t1"), Value::Null), (json!("t1"), json!("alpha reply")), (json!("t1"), json!("t1 pending"))],
            "the other thread keeps its events and its pending ones are written; the dropped thread's pending are not"
        );
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
        assert_eq!(
            kept,
            vec![(json!("t1"), json!("u1")), (json!("t3"), json!("u2")), (json!("t1"), json!("u1"))],
            "turn two of the thread went, with its pending event; the other thread's turn of the same id stays"
        );
        assert_eq!(ctx.rows.positions(WS), vec![1, 4, 5]);
        assert!(ctx.pending_of(WS).is_empty());
        let words = ctx.index_of(WS).expect("an index").words.get("t1").cloned().expect("t1 words");
        assert_eq!(words.lines, vec!["one", "pending of turn one"], "the thread's words are read again off what is left");
    });
}
