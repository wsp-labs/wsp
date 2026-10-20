// SPDX-License-Identifier: AGPL-3.0-only
use super::*;
use crate::screens::thread_view::find;
use gpui_kit::component::Root;
use gpui_kit::{size, TestAppContext, VisualTestContext};

/// `turns` settled turns from `from`, four events each, placed by pos as the host places them.
fn turns(from: usize, to: usize) -> Vec<SessionEvent> {
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
                event("session.delta", pos + 1, serde_json::json!({"kind": "text", "text": format!("Answer {n}.\n\nMore.\n\nAnd more.")})),
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
        v.set_listing(crate::transcript::Listing { sent: Some("look again".into()), ..listing }, cx);
    });
    sidebar.update(cx, |s, cx| s.apply(Change::Row { id: "s1".into(), row: Some(row("running")) }, cx));
    cx.run_until_parked();
    let sent = view.read_with(cx, |v, _| v.listing().and_then(|l| l.sent.clone()));
    assert_eq!(sent.as_deref(), Some("look again"), "the sidebar's next read dropped the send in flight");
}
