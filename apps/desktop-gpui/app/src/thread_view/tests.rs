// SPDX-License-Identifier: AGPL-3.0-only
use super::*;
use crate::screens::thread_view::find;
use gpui_kit::component::Root;
use gpui_kit::{size, TestAppContext, VisualTestContext};

/// `turns` settled turns from `from`, four events each, placed by pos as the host places them.
fn turns(from: usize, to: usize) -> Vec<SessionEvent> {
    said(from, to, |n| format!("Answer {n}.\n\nMore.\n\nAnd more."))
}

/// The turns from `from` to `to`, each replying `reply(n)`.
fn said(from: usize, to: usize, reply: impl Fn(usize) -> String) -> Vec<SessionEvent> {
    (from..to)
        .flat_map(|n| {
            let pos = 4 * n as u64 + 1;
            let event = |kind: &str, at: u64, extra: serde_json::Value| {
                let mut v = serde_json::json!({"workspaceId": "w", "sessionId": "s", "threadId": "t", "turnId": format!("u{n}"), "type": kind, "pos": at});
                v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
                serde_json::from_value::<SessionEvent>(v).unwrap()
            };
            [
                event("session.start", pos, serde_json::json!({"prompt": format!("Question {n}")})),
                event("session.delta", pos + 1, serde_json::json!({"kind": "text", "text": reply(n)})),
                event("session.done", pos + 2, serde_json::json!({"result": {"status": "completed"}})),
                event("session.end", pos + 3, serde_json::json!({"exitCode": 0, "sawResult": true})),
            ]
        })
        .collect()
}

fn window(cx: &mut TestAppContext) -> (Entity<ThreadView>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::ui::init(crate::ui::Side::Light, cx);
    });
    let view = cx.new(ThreadView::new);
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(view.clone(), window, cx));
    cx.simulate_resize(size(px(900.), px(600.)));
    (view, cx)
}

fn frames(cx: &mut VisualTestContext, n: usize) {
    for _ in 0..n {
        cx.executor().advance_clock(Duration::from_millis(16));
        cx.update(|window, cx| window.simulate_next_frame(cx));
        cx.run_until_parked();
    }
}

fn page(events: Vec<SessionEvent>) -> Page {
    Page { events, pos: 80, total: 80, trimmed: None }
}

/// `turns` with each reply a fence in `lang`.
fn fenced(from: usize, to: usize, lang: &str) -> Vec<SessionEvent> {
    said(from, to, |_| format!("Here:\n\n```{lang}\nx = 1\n```"))
}

#[gpui_kit::test]
fn the_grammars_a_thread_holds_compile_off_the_ui_thread_as_it_loads_and_as_its_pages_land(cx: &mut TestAppContext) {
    use crate::screens::thread_view::markdown::warming;
    let (view, cx) = window(cx);
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.load(fenced(10, 20, "python"), cx);
    });
    assert!(warming("python"), "the loaded thread's grammar waits for its first block to draw, on the UI thread");
    cx.run_until_parked();
    assert!(!warming("python"));
    view.update(cx, |v, cx| v.land(41, Ok(page(fenced(0, 10, "lua"))), cx));
    assert!(warming("lua"), "a landed page's grammar waits for its first block to draw, on the UI thread");
    cx.run_until_parked();
}

#[gpui_kit::test]
fn a_page_landing_under_a_reveal_keeps_it_on_its_row(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.load(turns(10, 20), cx);
        v.scroll_to_end(cx);
    });
    frames(cx, 3);
    let at = |v: &ThreadView| v.rows().iter().position(|r| r.text == "Question 15").unwrap();
    view.update(cx, |v, cx| {
        let ix = at(v);
        v.reveal(ix, px(100.), cx);
        v.land(41, Ok(page(turns(0, 10))), cx);
    });
    let (revealing, row) = view.read_with(cx, |v, _| (v.revealing.as_ref().map(|r| r.ix), at(v)));
    assert_eq!(revealing, Some(row), "the reveal still names the row it was asked for");
    frames(cx, 20);
    let top = view.read_with(cx, |v, _| v.list.bounds_for_item(at(v)).map(|b| b.top() - v.list.viewport_bounds().top()));
    assert!(top.is_some_and(|top| (top - px(100.)).abs() < px(1.)), "the revealed row stands at {top:?}, not 100 px");
}

#[gpui_kit::test]
fn a_page_landing_over_a_reader_at_the_top_stands_its_rows_above_them(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.load(turns(100, 110), cx);
        v.scroll_to_top(cx);
    });
    frames(cx, 3);
    view.update(cx, |v, cx| v.land(401, Ok(page(turns(50, 100))), cx));
    frames(cx, 3);
    let (above, screen) = view.read_with(cx, |v, _| (-v.list.scroll_px_offset_for_scrollbar().y, v.list.viewport_bounds().size.height));
    assert!(above > screen * 2., "the page's rows count {above:?} above the reader, so the next page is asked for at once");
}

#[gpui_kit::test]
fn a_turn_starting_leaves_a_reader_scrolled_up_on_their_row(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update(cx, |v, cx| {
        v.show("t", cx);
        _ = v.transcript.can(Can { rewinds_conversation: true, rewinds_by_count: true, ..Can::default() });
        v.load(turns(0, 30), cx);
        v.scroll_to_end(cx);
    });
    frames(cx, 3);
    let at = |v: &ThreadView, text: &str| v.rows().iter().position(|r| r.text == text).unwrap();
    view.update(cx, |v, cx| {
        let ix = at(v, "Question 15");
        v.reveal(ix, px(100.), cx);
    });
    frames(cx, 20);
    let top = |v: &ThreadView| v.rows()[v.list.logical_scroll_top().item_ix].key.clone();
    let reply = view.read_with(cx, |v, _| v.rows()[at(v, "Answer 15.\n\nMore.\n\nAnd more.")].key.clone());
    let rewind: &'static str = Box::leak(format!("rewind-to-here-{reply}").into_boxed_str());
    let read = view.read_with(cx, |v, _| top(v));
    assert!(cx.debug_bounds(rewind).is_some(), "a settled thread offers Rewind to here on an earlier reply");
    let start = turns(30, 31).swap_remove(0);
    view.update(cx, |v, cx| v.push(start, cx));
    frames(cx, 5);
    let (now, following) = view.read_with(cx, |v, _| (top(v), v.list.is_following_tail()));
    assert_eq!((now, following), (read, false), "a turn starting moved the reader off the row they read");
    assert!(cx.debug_bounds(rewind).is_none(), "Rewind to here stands while the turn runs");
}

#[gpui_kit::test]
fn a_driven_scroll_lays_out_every_row_a_page_landed_before_it_reaches_the_end(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.load(turns(150, 200), cx);
        v.scroll_to_end(cx);
    });
    frames(cx, 3);
    view.update(cx, |v, cx| v.land(601, Ok(Page { events: turns(0, 150), pos: 800, total: 800, trimmed: None }), cx));
    frames(cx, 3);
    let long_ago = Instant::now() - Duration::from_secs(1000);
    view.update(cx, |v, cx| v.drive(2400., long_ago, Rc::new(RefCell::new(Vec::new())), cx));
    for _ in 0..1000 {
        if view.read_with(cx, |v, _| v.driven_to_end()) {
            break;
        }
        frames(cx, 1);
    }
    // The list answers the end check only once every row is laid out.
    let end = view.read_with(cx, |v, _| (v.driven_to_end(), v.list.is_scrolled_to_end()));
    assert_eq!(end, (true, Some(true)), "the scroll passed over rows of the page it never laid out");
}

#[gpui_kit::test]
fn a_page_landing_folds_the_page_and_the_turn_it_meets_not_the_thread(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.load(turns(25, 125), cx);
        v.scroll_to_end(cx);
    });
    frames(cx, 3);
    let folded = view.update(cx, |v, cx| {
        v.transcript.take_cost();
        v.land(101, Ok(Page { events: turns(0, 25), pos: 500, total: 500, trimmed: None }), cx);
        v.transcript.take_cost()
    });
    assert!(folded < 300, "a page of 100 events under 400 held cost {folded} to land");
    let whole = view.read_with(cx, |v, _| {
        let mut t = Transcript::new();
        t.load(turns(0, 125));
        (t.rows() == v.rows(), v.oldest, v.complete)
    });
    assert_eq!(whole, (true, Some(1), true), "the landed page reads as the whole thread");
}

#[gpui_kit::test]
fn a_page_landing_keeps_a_reader_on_a_row_folded_again_with_rows_that_moved_below_it(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    // The failed start's line stands after the thread's first message, so a page above moves it from under the
    // reader's row, which the head that changed holds.
    let failed = crate::transcript::StartFailed {
        on: "Boat".into(),
        step: "Asking Boat for a machine".into(),
        said: "no machine free".into(),
        fix: None,
        prompt: None,
        log: Vec::new(),
        failed: true,
        ..Default::default()
    };
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.set_listing(crate::transcript::Listing { thread: "t".into(), start_failed: Some(failed), ..Default::default() }, cx);
        v.load(turns(10, 20), cx);
        v.scroll_to_top(cx);
    });
    frames(cx, 3);
    let top = |v: &ThreadView| v.rows()[v.list.logical_scroll_top().item_ix].text.clone();
    assert_eq!(view.read_with(cx, |v, _| top(v)), "Question 10");
    view.update(cx, |v, cx| v.land(41, Ok(page(turns(0, 10))), cx));
    frames(cx, 3);
    assert_eq!(view.read_with(cx, |v, _| top(v)), "Question 10", "the page moved the reader off the row they read");
}

#[gpui_kit::test]
fn escape_out_of_find_on_a_subagents_page_keeps_the_keys_in_its_transcript(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    let heard = Rc::new(RefCell::new(Vec::new()));
    let into = heard.clone();
    cx.update(|_, cx| cx.subscribe(&view, move |_, event: &ThreadViewEvent, _| into.borrow_mut().push(event.clone())).detach());
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.load(turns(0, 3), cx);
        v.set_page(Some("call-1".to_owned()), cx);
    });
    frames(cx, 2);
    cx.update(|window, cx| view.update(cx, |v, cx| find::open(v, window, cx)));
    frames(cx, 2);
    cx.update(|window, cx| view.update(cx, |v, cx| find::close(v, window, cx)));
    frames(cx, 2);
    let asked = heard.borrow().contains(&ThreadViewEvent::FocusComposer);
    let held = cx.update(|window, cx| view.read(cx).holds_focus(window, cx));
    assert_eq!((asked, held), (false, true), "closing find on the page sent the keys to a composer it does not draw");
}

#[gpui_kit::test]
fn a_failed_page_read_is_asked_again_after_a_wait(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.load(turns(10, 20), cx);
        v.complete = false;
        v.land(41, Err(Failure::new("the host is busy".to_owned())), cx);
    });
    assert_eq!(view.read_with(cx, |v, _| (v.complete, v.paging.is_some())), (false, true), "the thread still has older pages");
    cx.executor().advance_clock(RETRY[0] + Duration::from_millis(10));
    cx.run_until_parked();
    assert!(view.read_with(cx, |v, _| v.paging.is_none()), "after the wait the next page may be asked for");
}

#[gpui_kit::test]
fn a_newest_page_past_a_gap_is_not_laid_under_the_pages_held(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.load(turns(0, 10), cx);
        let (drawn, _) = oneshot::channel();
        v.arrive(Arrival::History { events: turns(80, 90), total: 360, trimmed: false, drawn }, cx);
    });
    let (held, oldest) = view.read_with(cx, |v, _| (v.rows().iter().any(|r| r.text == "Question 5"), v.oldest));
    assert_eq!((held, oldest), (false, Some(321)), "the turns between the pages held and the newest page are missing");
}

/// A send in flight stands while the sidebar's rows move under it: the listing read again keeps its words.
#[gpui_kit::test]
fn a_relist_keeps_the_send_in_flight(cx: &mut TestAppContext) {
    use crate::screens::sidebar::host::Change;
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::ui::init(crate::ui::Side::Light, cx);
    });
    let sidebar = cx.new(crate::sidebar::Sidebar::new);
    cx.update(|cx| crate::screens::terminal::read_workspaces_from(&sidebar, cx));
    let row = |status: &str| serde_json::json!({"id": "s1", "workspaceId": "w", "harness": "claude", "status": status, "threadId": "t", "startedAt": 1.0});
    sidebar.update(cx, |s, cx| s.apply(Change::Row { id: "s1".into(), row: Some(row("completed")) }, cx));
    let view = cx.new(ThreadView::new);
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(view.clone(), window, cx));
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.relist(cx);
        let listing = v.listing().cloned().expect("the sidebar's listing");
        let sent = crate::transcript::Sent { text: "look again".into(), files: Vec::new(), request: None };
        v.set_listing(crate::transcript::Listing { sent: Some(sent), ..listing }, cx);
    });
    sidebar.update(cx, |s, cx| s.apply(Change::Row { id: "s1".into(), row: Some(row("running")) }, cx));
    cx.run_until_parked();
    let sent = view.read_with(cx, |v, _| v.listing().and_then(|l| l.sent.as_ref().map(|s| s.text.clone())));
    assert_eq!(sent.as_deref(), Some("look again"), "the sidebar's next read dropped the send in flight");
}

/// The wheel at `at`, as a trackpad's fingers send it mid-gesture, scrolling toward the transcript's end.
fn wheel(at: gpui_kit::Point<Pixels>) -> gpui_kit::ScrollWheelEvent {
    gpui_kit::ScrollWheelEvent {
        position: at,
        delta: gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(1.5), px(-30.))),
        touch_phase: gpui_kit::TouchPhase::Moved,
        ..Default::default()
    }
}

#[gpui_kit::test]
fn a_trackpad_scroll_runs_straight_through_a_tool_output_a_code_block_and_a_wide_table(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    cx.simulate_resize(size(px(900.), px(1400.)));
    let at = |kind: &str, pos: u64, extra: serde_json::Value| {
        let mut v = serde_json::json!({"workspaceId": "w", "sessionId": "s", "threadId": "t", "turnId": "u0", "type": kind, "pos": pos});
        v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        serde_json::from_value::<SessionEvent>(v).unwrap()
    };
    let output: String = (1..=40).map(|n| format!("line {n}\n")).collect();
    let code: String = (1..=30).map(|n| format!("    let row_{n} = rows.get({n}).map(|row| row.height).unwrap_or_default();\n")).collect();
    let heads: Vec<String> = (1..=10).map(|n| format!("Column {n} heading")).collect();
    let table = format!("| {} |\n|{}\n| {} |", heads.join(" | "), " --- |".repeat(10), heads.join(" | "));
    let mut events = vec![
        at("session.start", 1, serde_json::json!({"prompt": "go"})),
        at(
            "session.delta",
            2,
            serde_json::json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": "a", "text": "{\"command\":\"seq 40\"}"}),
        ),
        at("session.delta", 3, serde_json::json!({"kind": "tool_result", "toolUseId": "a", "text": output, "exitCode": 0})),
        at("session.delta", 4, serde_json::json!({"kind": "text", "text": format!("Here:\n\n```rust\n{code}```\n\n{table}\n")})),
        at("session.done", 5, serde_json::json!({"result": {"status": "completed"}})),
        at("session.end", 6, serde_json::json!({"exitCode": 0, "sawResult": true})),
    ];
    events.extend(turns(2, 40));
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.load(events, cx);
    });
    for key in ["u0", "group:tool:u0:a", "u0:call:a"] {
        view.update(cx, |v, cx| v.toggle(key, cx));
        cx.run_until_parked();
    }
    view.update(cx, |v, cx| v.scroll_to_top(cx));
    frames(cx, 3);
    let top = |cx: &mut VisualTestContext| view.read_with(cx, |v, _| v.list.scroll_px_offset_for_scrollbar().y);
    let over = |cx: &mut VisualTestContext, selector: &'static str, down: f32| {
        let b = cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} was not drawn"));
        gpui_kit::point(b.center().x, b.bottom() + px(down))
    };
    let output = cx.debug_bounds("output").expect("the open call's output").center();
    let code = over(cx, "code-copy", 40.);
    let table = over(cx, "table-copy", 30.);
    for (spot, place) in [("tool output", output), ("code block", code), ("wide table", table)] {
        let before = top(cx);
        cx.simulate_event(wheel(place));
        cx.run_until_parked();
        assert!(top(cx) < before, "the wheel over the {spot} never reached the transcript");
        view.update(cx, |v, cx| v.scroll_to_top(cx));
        frames(cx, 2);
    }
}

/// Catches: a command's bash or an edited file's grammar compiling on the UI thread the first time its call scrolls in.
#[gpui_kit::test]
fn the_grammars_of_a_threads_commands_and_edits_compile_off_the_ui_thread_as_their_calls_open(cx: &mut TestAppContext) {
    use crate::screens::thread_view::markdown::warming;
    let (view, cx) = window(cx);
    let at = |kind: &str, pos: u64, extra: serde_json::Value| {
        let mut v = serde_json::json!({"workspaceId": "w", "sessionId": "s", "threadId": "t", "turnId": "u0", "type": kind, "pos": pos});
        v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        serde_json::from_value::<SessionEvent>(v).unwrap()
    };
    let events = vec![
        at("session.start", 1, serde_json::json!({"prompt": "go"})),
        at(
            "session.delta",
            2,
            serde_json::json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": "a", "text": "{\"command\":\"ls\"}"}),
        ),
        at("session.delta", 3, serde_json::json!({"kind": "tool_result", "toolUseId": "a", "text": "a", "exitCode": 0})),
        at(
            "session.delta",
            4,
            serde_json::json!({"kind": "tool_use", "toolName": "Edit", "toolUseId": "b", "text": "{\"file_path\":\"src/a.py\"}"}),
        ),
        at(
            "session.delta",
            5,
            serde_json::json!({"kind": "tool_result", "toolUseId": "b", "text": "ok", "patch": [{"path": "src/a.py", "hunks": [
                {"oldStart": 1, "oldLines": 1, "newStart": 1, "newLines": 1, "lines": ["-a = 1", "+a = 2"]}
            ]}]}),
        ),
        at("session.delta", 6, serde_json::json!({"kind": "text", "text": "Done."})),
        at("session.done", 7, serde_json::json!({"result": {"status": "completed"}})),
        at("session.end", 8, serde_json::json!({"exitCode": 0, "sawResult": true})),
    ];
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.load(events, cx);
    });
    cx.run_until_parked();
    view.update(cx, |v, cx| {
        v.toggle("u0", cx);
        v.toggle("group:tool:u0:a", cx);
    });
    let calls = view.read_with(cx, |v, _| v.transcript().rows().iter().any(|r| matches!(r.kind, RowKind::Calls { .. })));
    assert!(calls, "opening the turn shows no calls");
    assert!(warming("bash"), "a command's grammar waits for its row to draw, on the UI thread");
    assert!(warming("python"), "an edited file's grammar waits for its diff to draw, on the UI thread");
    cx.run_until_parked();
}

#[gpui_kit::test]
fn a_rows_kind_name_is_its_variant_as_debug_writes_it(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.load(fenced(0, 2, "rust"), cx);
    });
    let kinds: Vec<(&str, String)> =
        view.read_with(cx, |v, _| v.transcript().rows().iter().map(|r| (r.kind.name(), format!("{:?}", r.kind))).collect());
    assert!(kinds.len() >= 4, "{kinds:?}");
    for (name, debug) in kinds {
        assert_eq!(debug.split([' ', '{', '(']).next(), Some(name));
    }
}

/// Catches: the composer's compact rule read off the events held alone. A newest page from the middle of a long thread
/// holds no turn's start though its turn runs, and its messages stand on older pages; a thread whose older events the
/// host dropped holds its message only in the page.
#[gpui_kit::test]
fn the_composer_reads_a_running_turn_and_older_messages_off_a_page_holding_neither(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    let event = |kind: &str, pos: u64, extra: serde_json::Value| {
        let mut v = serde_json::json!({"workspaceId": "w", "sessionId": "s9", "threadId": "t", "turnId": "u9", "type": kind, "pos": pos});
        v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        serde_json::from_value::<SessionEvent>(v).unwrap()
    };
    let calls = |from: u64| -> Vec<SessionEvent> {
        (from..from + 200)
            .map(|pos| {
                event(
                    "session.delta",
                    pos,
                    serde_json::json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": format!("c{pos}"), "text": "{}"}),
                )
            })
            .collect()
    };
    let ended = |mut events: Vec<SessionEvent>, pos: u64| {
        events.push(event("session.done", pos, serde_json::json!({"result": {"status": "completed"}})));
        events.push(event("session.end", pos + 1, serde_json::json!({"exitCode": 0, "sawResult": true})));
        events
    };
    let seen = |events: Vec<SessionEvent>, total: u64, cx: &mut VisualTestContext| {
        view.update(cx, |v, cx| {
            v.show("t", cx);
            let (drawn, _) = oneshot::channel();
            v.arrive(Arrival::History { events, total, trimmed: false, drawn }, cx);
        });
        view.read_with(cx, |v, _| {
            let seen = crate::screens::thread_view::seen(v);
            (seen.compact(), seen.running().map(str::to_owned))
        })
    };
    assert_eq!(seen(calls(7_001), 7_200, cx), (true, Some("s9".to_owned())), "a page of a running turn's calls");
    assert_eq!(seen(ended(calls(7_001), 7_201), 7_202, cx), (true, None), "a page of calls under older pages");
    let said = vec![event("session.delta", 7_001, serde_json::json!({"kind": "text", "text": "Done."}))];
    assert_eq!(seen(ended(said, 7_002), 3, cx), (true, None), "a page holding the agent's words, its turn's start trimmed");
}
