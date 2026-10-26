// SPDX-License-Identifier: AGPL-3.0-only
//! Part 2 of `transcripts_rows.rs`: the same world, the tests after it.
#![allow(dead_code, unused_imports)]

#[path = "support_b/mod.rs"]
mod support;
include!("transcripts_rows/world.rs");

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
    let mut events =
        vec![event(start_v(WS, "t1", "a", "start a")), text_delta(Some("t1"), "a1", 2), event(start_v(WS, "t1", "b", "start b"))];
    events.extend((1..=5001).map(|i| text_delta(Some("t1"), &format!("b{i}"), 3 + i)));
    assert_eq!(events.len(), 5004);
    let before = events.clone();
    let (gone, left) = trim(&mut events);
    assert_eq!(
        gone.iter().map(text_of).collect::<Vec<_>>(),
        vec!["start a", "a1", "start b", "b1"],
        "the old turn first, then the oldest of the newest turn, in list order"
    );
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
    let mut events: Vec<SessionEvent> =
        (1..=(TRANSCRIPT_CAP as i64 + 1)).map(|i| text_delta(Some("busy"), &format!("busy {i}"), i)).collect();
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
    let size = |thread: Option<&str>, kind: &str, pos: i64| EventSize {
        pos,
        thread_id: thread.map(String::from),
        kind: kind.to_string(),
        size: 10,
    };
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
        ctx.rows.seed(
            WS,
            vec![stamped(start_v(WS, "t1", "u1", "stream something"), 1, 10), stamped(delta_v(WS, "t1", "u1", "written"), 2, 10)],
        );
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
        assert_eq!(
            opened.iter().map(text_of).collect::<Vec<_>>(),
            vec!["one", "one reply", "t1 pending"],
            "that thread's rows, then that thread's pending"
        );
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
        assert_eq!(
            page.events.iter().map(text_of).collect::<Vec<_>>(),
            vec!["row 5", "pending 7", "pending 8"],
            "the pending events newest first, then the rows, answered oldest first"
        );
        assert_eq!(page.total, 5 + 2, "the thread's rows and its pending");
        assert_eq!(page.pos, 9, "the position the held index has issued");
        assert!(!page.trimmed);

        let before = area.read(WS, "t1", &ThreadRead { before: Some(7), limit: 2, ..read_all() }).expect("the read");
        assert_eq!(before.events.iter().map(pos_of).collect::<Vec<_>>(), vec![4, 5], "a pending event at or past before is skipped");

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
        assert_eq!(
            warned.lines(),
            vec!["the transcript of ws1 does not read (disk I/O error), so search and a send leave it out until it reads"]
        );
        assert_eq!(
            ctx.index_of(WS),
            Some(TranscriptIndex { pos: 3, ..TranscriptIndex::default() }),
            "an empty index that goes on after the newest row"
        );
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
        let mut index_one =
            index_of_words(vec![("a", held_a.clone()), ("b", held_b.clone()), ("c", words(&[c1.as_str(), c2.as_str(), c3.as_str()], 100))]);
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
            Some(&ThreadWords {
                lines: vec![c3.clone()],
                open: None,
                last: JsNumber(100.0),
                size: 150_000,
                first: Some(line_of('1', 200)),
                opened: None,
                cold: Some(true),
                room: Some(share)
            }),
            "the thread that passes what is left keeps its newest lines that fit"
        );
        let d = ctx.index_of("ws2").expect("an index").words.get("d").cloned().expect("d");
        assert_eq!(
            d,
            ThreadWords {
                lines: vec![],
                open: None,
                last: JsNumber(50.0),
                size: 10,
                first: Some("small text".into()),
                opened: None,
                cold: Some(true),
                room: Some(0)
            },
            "an older thread with nothing left to hold is let go whole, across workspaces"
        );
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
        ctx.sessions
            .with(|s| *s = sessions_of(vec![row(view(json!({ "id": "r1", "threadId": "run", "status": "running" })), "u1")]).take());
        let area = ctx.area();
        area.balance_words();
        let held = ctx.index_of(WS).expect("an index");
        assert_eq!(held.words.get("run"), Some(&big), "a thread whose turn runs holds all of its own, past the budget");
        let quiet = held.words.get("quiet").expect("quiet");
        assert_eq!(
            (quiet.cold, quiet.room, quiet.lines.len()),
            (Some(true), Some(0), 0),
            "the budget was spent, so the next thread holds none"
        );

        ctx.sessions
            .with(|s| *s = sessions_of(vec![row(view(json!({ "id": "r1", "threadId": "run", "status": "completed" })), "u1")]).take());
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

// TS: sqlite-transcripts.test.ts:337
#[test]
fn warming_reads_back_one_thread_a_tick() {
    local(async {
        let (ctx, _, b) = warm_ctx();
        let area = ctx.area();
        assert!(area.warm_one(), "a thread was read back");
        assert_eq!(
            ctx.index_of(WS).expect("an index").words.get("a"),
            Some(&warmed("alpha words", "alpha words", 300)),
            "its words back, the later of the read time and the time it was last spoken in"
        );
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
        assert_eq!(
            ctx.rows_called_of(&["word_events"]),
            vec!["word_events ws1 a", "word_events ws1 b"],
            "each tick reads one and asks again"
        );
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
            ThreadWords {
                lines: vec!["x first".into(), "fresh-zebra words".into()],
                open: Some("u1:".into()),
                last: JsNumber(1_000_000.0),
                size: 24,
                first: Some("x first".into()),
                opened: Some(JsNumber(777.0)),
                cold: None,
                room: None
            },
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
        assert!(
            !ctx.index_of(WS).expect("an index").words.contains_key("nobody"),
            "nothing is made for a thread the index holds no words for"
        );
        assert!(ctx.index_of("ws9").is_none(), "nor an index for a workspace that holds none");
    });
}
