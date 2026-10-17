// SPDX-License-Identifier: AGPL-3.0-only
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use desktop_gpui::thread_view::ThreadView;
use gpui_kit::component::Root;
use gpui_kit::{px, size, AppContext, Entity, ListOffset, Pixels, TestAppContext, VisualTestContext};
use wsp_host_client::protocol::SessionEvent;

const TURNS: usize = 300;

/// Two rows, the person's message and a reply with a fenced code block; the turn ends with no call to hide, so no
/// fold stands in for any of it.
fn turn(turn: usize) -> [SessionEvent; 4] {
    let pos = 4 * turn as u64 + 1;
    let scope = serde_json::json!({"workspaceId": "w", "sessionId": "s", "threadId": "t", "turnId": format!("u{turn}")});
    let mut start = scope.clone();
    start.as_object_mut().unwrap().extend([
        ("type".to_owned(), "session.start".into()),
        ("pos".to_owned(), pos.into()),
        ("prompt".to_owned(), format!("Question {turn}").into()),
    ]);
    let mut reply = scope;
    reply.as_object_mut().unwrap().extend([
        ("type".to_owned(), "session.delta".into()),
        ("pos".to_owned(), (pos + 1).into()),
        ("kind".to_owned(), "text".into()),
        ("text".to_owned(), format!("Answer {turn}\n\n- one\n- `two`\n\n```rust\nlet turn = {turn};\n```").into()),
    ]);
    let ending = |kind: &str, at: u64, extra: serde_json::Value| {
        let mut event = serde_json::json!({"workspaceId": "w", "sessionId": "s", "threadId": "t", "turnId": format!("u{turn}")});
        event.as_object_mut().unwrap().extend([("type".to_owned(), kind.into()), ("pos".to_owned(), at.into())]);
        event.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        serde_json::from_value::<SessionEvent>(event).unwrap()
    };
    [
        serde_json::from_value(start).unwrap(),
        serde_json::from_value(reply).unwrap(),
        ending("session.done", pos + 2, serde_json::json!({"result": {"status": "completed"}})),
        ending("session.end", pos + 3, serde_json::json!({"exitCode": 0, "sawResult": true})),
    ]
}

fn transcript(turns: usize) -> Vec<SessionEvent> {
    (0..turns).flat_map(turn).collect()
}

fn window(cx: &mut TestAppContext) -> (Entity<ThreadView>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
    });
    let view = cx.new(ThreadView::new);
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(view.clone(), window, cx));
    cx.simulate_resize(size(px(900.), px(600.)));
    (view, cx)
}

fn offset(view: &Entity<ThreadView>, cx: &mut VisualTestContext) -> Pixels {
    view.read_with(cx, |v, _| -v.list().scroll_px_offset_for_scrollbar().y)
}

fn last_row_shown(view: &Entity<ThreadView>, cx: &mut VisualTestContext) -> bool {
    view.read_with(cx, |v, _| v.list().item_is_below_viewport(v.rows().len() - 1) == Some(false))
}

fn push_turn(view: &Entity<ThreadView>, n: usize, cx: &mut VisualTestContext) {
    view.update(cx, |v, cx| turn(n).into_iter().for_each(|e| v.push(e, cx)));
    cx.run_until_parked();
}

#[gpui_kit::test]
fn the_list_builds_only_the_rows_on_screen(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.load(transcript(TURNS), cx);
    });
    cx.run_until_parked();

    let (rows, built, parsed) = view.read_with(cx, |v, _| (v.rows().len(), v.built(), v.parsed()));
    assert_eq!(rows, 2 * TURNS);
    // The rows on screen and those in the overdraw below it, each measured once: a few dozen of 600.
    assert!(built > 0 && built < 80, "built {built} rows of {rows}");
    assert_eq!(parsed, built, "each built row parsed its markdown once");

    // A frame with nothing new builds the rows on screen and no others; the overdraw keeps its measured heights.
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let again = view.read_with(cx, |v, _| v.built()) - built;
    assert!(again > 0 && again < built, "a redraw built {again} rows, the first layout {built}");
    assert_eq!(view.read_with(cx, |v, _| v.parsed()), parsed, "a redraw parsed nothing");

    // Scrolling down and back parses only rows never built before.
    view.update(cx, |v, cx| {
        v.list().scroll_by(px(1500.));
        cx.notify();
    });
    cx.run_until_parked();
    let below = view.read_with(cx, |v, _| v.parsed());
    assert!(below > parsed && below < 2 * TURNS / 4, "parsed {below} rows after a scroll");
    view.update(cx, |v, cx| {
        v.list().scroll_to(ListOffset::default());
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |v, _| v.parsed()), below, "scrolling back parsed a row again");
    println!("{rows} rows: first layout built {built}, a redraw {again}, parsed {below} after scrolling down and back");
}

#[gpui_kit::test]
fn a_new_event_follows_the_end_and_leaves_a_view_scrolled_away_where_it_is(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.load(transcript(TURNS), cx);
        v.scroll_to_end(cx);
    });
    cx.run_until_parked();

    push_turn(&view, TURNS, cx);
    assert!(last_row_shown(&view, cx), "a view at the end did not follow a new turn");

    view.update(cx, |v, cx| {
        v.list().scroll_by(px(-1500.));
        cx.notify();
    });
    cx.run_until_parked();
    let read_up = offset(&view, cx);
    push_turn(&view, TURNS + 1, cx);
    assert_eq!(offset(&view, cx), read_up, "a new turn moved a view the person had scrolled up");

    // Back at the top, with every row between it and the end never measured.
    view.update(cx, |v, cx| {
        v.list().scroll_to(ListOffset::default());
        cx.notify();
    });
    cx.run_until_parked();
    push_turn(&view, TURNS + 2, cx);
    assert_eq!(offset(&view, cx), px(0.), "a new turn moved a view at the top of the thread");
}

#[gpui_kit::test]
fn a_thread_that_grows_past_the_window_keeps_following(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.load(transcript(1), cx);
        v.scroll_to_end(cx);
    });
    cx.run_until_parked();
    for n in 1..20 {
        push_turn(&view, n, cx);
        assert!(last_row_shown(&view, cx), "turn {n} went below the window");
    }
    assert!(offset(&view, cx) > px(0.), "the thread never grew past the window");
}

#[gpui_kit::test]
fn rows_sit_in_a_centered_column_as_wide_as_the_web_apps(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.load(transcript(3), cx);
    });
    cx.run_until_parked();
    let row = cx.debug_bounds("row-0").expect("the first row was not drawn");
    assert_eq!(row.size.width, px(768.), "{row:?}");
    assert_eq!(row.left(), px((900. - 768.) / 2.), "{row:?}");
}

#[gpui_kit::test]
fn a_scroll_driven_from_the_top_reaches_the_end_and_stamps_its_frames(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.load(transcript(TURNS), cx);
        v.scroll_to_end(cx);
    });
    cx.run_until_parked();
    let frames = Rc::new(RefCell::new(Vec::new()));
    let long_ago = Instant::now() - Duration::from_secs(1000);
    view.update(cx, |v, cx| v.drive(2400., long_ago, frames.clone(), cx));
    for _ in 0..10 {
        view.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
    }
    assert!(view.read_with(cx, |v, _| v.driven_to_end()), "the scroll stopped short of the end at {:?}", offset(&view, cx));
    assert!(!frames.borrow().is_empty(), "no frame was stamped");
}

fn event(value: serde_json::Value) -> SessionEvent {
    let mut v = serde_json::json!({"workspaceId": "w", "sessionId": "s", "threadId": "t", "turnId": "long"});
    v.as_object_mut().unwrap().extend(value.as_object().unwrap().clone());
    serde_json::from_value(v).unwrap()
}

/// Three short turns, then one whose reply is past the 4 KB gpui-kit parses on the spot, so its markdown is parsed in
/// the background and its row grows after the first frame.
fn opened_on_a_long_reply(cx: &mut TestAppContext) -> (Entity<ThreadView>, &mut VisualTestContext) {
    let (view, cx) = window(cx);
    let long: String = (0..200).map(|i| format!("Line {i} of a reply that runs for several screens.\n\n")).collect();
    let mut events = transcript(3);
    events.push(event(serde_json::json!({"type": "session.start", "pos": 13, "prompt": "Explain it all"})));
    events.push(event(serde_json::json!({"type": "session.delta", "pos": 14, "kind": "text", "text": long})));
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.load(events, cx);
        v.scroll_to_end(cx);
    });
    cx.run_until_parked();
    (view, cx)
}

#[gpui_kit::test]
fn a_thread_whose_last_reply_is_long_opens_at_its_end(cx: &mut TestAppContext) {
    let (view, cx) = opened_on_a_long_reply(cx);
    let (last, viewport) = view.read_with(cx, |v, _| (v.list().bounds_for_item(v.rows().len() - 1), v.list().viewport_bounds()));
    let last = last.expect("the last row was laid out");
    assert!(
        (last.bottom() - viewport.bottom()).abs() <= px(1.),
        "the last row ends at {:?}, the window at {:?}",
        last.bottom(),
        viewport.bottom()
    );
}

#[gpui_kit::test]
fn a_reply_growing_while_the_person_reads_it_does_not_move_the_view(cx: &mut TestAppContext) {
    let (view, cx) = opened_on_a_long_reply(cx);
    view.update(cx, |v, cx| {
        v.list().scroll_by(px(-300.));
        cx.notify();
    });
    cx.run_until_parked();
    let read = offset(&view, cx);
    view.update(cx, |v, cx| v.push(event(serde_json::json!({"type": "session.delta", "pos": 15, "kind": "text", "text": "More."})), cx));
    cx.run_until_parked();
    assert_eq!(offset(&view, cx), read, "a delta to the reply being read moved the view");
}

#[gpui_kit::test]
fn a_streaming_reply_takes_each_delta_into_the_document_it_already_parsed(cx: &mut TestAppContext) {
    let (view, cx) = opened_on_a_long_reply(cx);
    let (parsed, rows, kept) = view.read_with(cx, |v, _| (v.parsed(), v.rows().len(), v.kept()));
    for i in 0..100 {
        let delta = serde_json::json!({"type": "session.delta", "pos": 15 + i, "kind": "text", "text": format!(" word{i}")});
        view.update(cx, |v, cx| v.push(event(delta), cx));
        cx.run_until_parked();
    }
    let reply = view.read_with(cx, |v, _| v.rows().iter().rev().find(|r| r.text.starts_with("Line 0")).map(|r| r.text.clone()));
    assert!(reply.is_some_and(|r| r.ends_with(" word99")), "the reply took every delta");
    assert_eq!(view.read_with(cx, |v, _| v.rows().len()), rows, "deltas to the open reply add no row");
    assert_eq!(view.read_with(cx, |v, _| v.parsed()), parsed, "a delta was parsed as a new document rather than appended");
    assert_eq!(view.read_with(cx, |v, _| v.kept()), kept);
}

#[gpui_kit::test]
fn a_scroll_through_a_long_thread_keeps_only_the_parsed_rows_near_the_view(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.load(transcript(TURNS), cx);
    });
    cx.run_until_parked();
    let mut most = 0;
    while view.read_with(cx, |v, _| v.list().is_scrolled_to_end()) != Some(true) {
        view.update(cx, |v, cx| {
            v.list().scroll_by(px(400.));
            cx.notify();
        });
        cx.run_until_parked();
        most = most.max(view.read_with(cx, |v, _| v.kept()));
    }
    // Two screens behind and a screen past the 800 px overdraw ahead, at the 28 px shortest row: 116 rows at 600 px.
    assert!(most <= 116, "held {most} parsed rows of {}", 2 * TURNS);
    let parsed = view.read_with(cx, |v, _| v.parsed());
    view.update(cx, |v, cx| {
        v.list().scroll_to(ListOffset::default());
        cx.notify();
    });
    cx.run_until_parked();
    assert!(view.read_with(cx, |v, _| v.parsed()) > parsed, "the top's rows were never dropped, so nothing parsed again");
    println!("{} rows: held at most {most} parsed, {parsed} parses down the thread", 2 * TURNS);
}
