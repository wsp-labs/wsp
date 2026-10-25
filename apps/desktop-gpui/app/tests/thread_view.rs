// SPDX-License-Identifier: AGPL-3.0-only
#[path = "../../host-client/tests/common/mod.rs"]
mod common;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use desktop_gpui::screens::Route;
use desktop_gpui::shell::{Launch, Shell};
use desktop_gpui::thread_view::ThreadView;
use desktop_gpui::transcript::RowKind;
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
    // A grammar landing after the first frame draws the rows on screen again to highlight them, with no new parse.
    cx.update(|_, cx| desktop_gpui::screens::thread_view::markdown::warm(["rust"], |_| {}, cx));
    cx.run_until_parked();
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
fn a_reader_a_few_pixels_above_the_end_still_follows_a_new_turn(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.load(transcript(TURNS), cx);
        v.scroll_to_end(cx);
    });
    cx.run_until_parked();
    view.update(cx, |v, cx| {
        v.list().scroll_by(px(-20.));
        cx.notify();
    });
    cx.run_until_parked();
    push_turn(&view, TURNS, cx);
    let end = view.read_with(cx, |v, _| v.list().is_scrolled_to_end());
    assert_eq!(end, Some(true), "a reader 20 px above the end was left above the new turn");
}

/// `turns` turns, then one whose call folds behind "Worked for".
fn ending_on_a_fold(turns: usize) -> Vec<SessionEvent> {
    let pos = 4 * turns as u64 + 1;
    let mut events = transcript(turns);
    events.extend([
        serde_json::json!({"type": "session.start", "pos": pos, "prompt": "Run it"}),
        serde_json::json!({"type": "session.delta", "pos": pos + 1, "kind": "tool_use", "toolName": "Bash", "toolUseId": "a", "text": "{\"command\":\"ls\"}"}),
        serde_json::json!({"type": "session.delta", "pos": pos + 2, "kind": "tool_result", "toolUseId": "a", "text": "out"}),
        serde_json::json!({"type": "session.delta", "pos": pos + 3, "kind": "text", "text": "Done."}),
        serde_json::json!({"type": "session.done", "pos": pos + 4, "result": {"status": "completed", "durationMs": 1200}}),
        serde_json::json!({"type": "session.end", "pos": pos + 5, "exitCode": 0, "sawResult": true}),
    ]
    .map(event));
    events
}

#[gpui_kit::test]
fn a_fold_pressed_at_the_end_opens_downward_from_where_it_was_pressed(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.load(ending_on_a_fold(10), cx);
        v.scroll_to_end(cx);
    });
    cx.run_until_parked();
    let fold = view.read_with(cx, |v, _| v.rows().iter().rposition(|r| matches!(r.kind, RowKind::Fold { .. }))).expect("no fold");
    let top = |cx: &mut VisualTestContext| view.read_with(cx, |v, _| v.list().bounds_for_item(fold).map(|b| b.top()));
    let before = top(cx).expect("the fold was not laid out");
    view.update(cx, |v, cx| v.toggle("long", cx));
    cx.run_until_parked();
    assert_eq!(top(cx), Some(before), "the fold moved from under the pointer as it opened");
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
    drive_to_end(&view, cx);
    assert!(view.read_with(cx, |v, _| v.driven_to_end()), "the scroll stopped short of the end at {:?}", offset(&view, cx));
    assert!(!frames.borrow().is_empty(), "no frame was stamped");
}

/// Frames until the driven scroll says it reached the end; it goes a screen at most each.
fn drive_to_end(view: &Entity<ThreadView>, cx: &mut VisualTestContext) {
    for _ in 0..1000 {
        if view.read_with(cx, |v, _| v.driven_to_end()) {
            return;
        }
        view.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
    }
}

#[gpui_kit::test]
fn a_list_narrowed_partway_down_keeps_its_length_and_a_driven_scroll_reaches_its_end(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.load(transcript(TURNS), cx);
    });
    cx.run_until_parked();
    while offset(&view, cx) < px(5000.) {
        view.update(cx, |v, cx| {
            v.list().scroll_by(px(400.));
            cx.notify();
        });
        cx.run_until_parked();
    }
    let length = |cx: &mut VisualTestContext| view.read_with(cx, |v, _| v.list().max_offset_for_scrollbar().y);
    let wide = length(cx);
    // The rows sit in a 768 px column, so narrowing the window changes the list's width and no row's height.
    cx.simulate_resize(size(px(880.), px(600.)));
    cx.run_until_parked();
    assert_eq!(length(cx), wide, "the list forgot its rows' heights when it narrowed");

    let frames = Rc::new(RefCell::new(Vec::new()));
    let long_ago = Instant::now() - Duration::from_secs(1000);
    view.update(cx, |v, cx| v.drive(2400., long_ago, frames.clone(), cx));
    drive_to_end(&view, cx);
    let end = view.read_with(cx, |v, _| v.list().is_scrolled_to_end());
    assert!(view.read_with(cx, |v, _| v.driven_to_end()), "the scroll stopped at {:?} with the end check {end:?}", offset(&view, cx));
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
    let reply = view.read_with(cx, |v, _| v.rows().iter().rev().find(|r| r.text.starts_with("Line 0")).map(|r| r.key.clone())).unwrap();
    let parsed_text = view.read_with(cx, |v, cx| v.body_source(&reply, cx));
    assert!(parsed_text.is_some_and(|t| t.ends_with(" word99")), "the reply's parsed document took every delta");
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
    let (built, kept) = view.read_with(cx, |v, _| (v.built(), v.kept()));
    assert!(kept > 0 && built > 0);
}

#[gpui_kit::test]
fn an_empty_thread_says_to_send_a_message_and_a_loading_one_says_so_centred(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update(cx, |v, cx| v.show("t", cx));
    cx.executor().advance_clock(desktop_gpui::thread_view::HOLD);
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |v, _| v.placeholder()), Some("Loading transcript"));
    assert!(cx.debug_bounds("transcript-loading").is_some(), "Loading transcript is not drawn");
    view.update(cx, |v, cx| v.load(Vec::new(), cx));
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |v, _| v.placeholder()), Some("Send a message to start the conversation."));
    let words = cx.debug_bounds("transcript-words").expect("the empty thread's line is not drawn");
    assert_eq!((words.origin.y, words.size.height), (px(0.), px(600.)), "the line is centred in the whole view: {words:?}");
    assert!(cx.debug_bounds("transcript-loading").is_none());
}

fn keys(view: &Entity<ThreadView>, cx: &mut VisualTestContext) -> Vec<String> {
    view.read_with(cx, |v, _| v.rows().iter().map(|r| r.key.clone()).collect())
}

fn reply_key(view: &Entity<ThreadView>, cx: &mut VisualTestContext) -> String {
    view.read_with(cx, |v, _| v.rows().iter().find(|r| matches!(r.kind, RowKind::Agent { .. })).unwrap().key.clone())
}

#[gpui_kit::test]
fn a_switch_keeps_the_last_threads_rows_drawn_until_the_next_ones_land_then_swaps_them_in_one_frame(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.load(transcript(3), cx);
    });
    cx.run_until_parked();
    let (old, old_reply) = (keys(&view, cx), reply_key(&view, cx));

    view.update(cx, |v, cx| v.show("u", cx));
    for _ in 0..3 {
        cx.executor().advance_clock(Duration::from_millis(40));
        cx.run_until_parked();
        assert_eq!(keys(&view, cx), old, "the last thread's rows went before the next thread's landed");
        assert!(cx.debug_bounds("row-0").is_some(), "the last thread's rows are not drawn through the switch");
        assert!(view.read_with(cx, |v, _| v.body_of(&old_reply).is_some()), "the held rows were parsed again or dropped");
        assert!(cx.debug_bounds("transcript-loading").is_none(), "the loading line flashed");
        assert!(cx.debug_bounds("transcript-held").is_some(), "the held rows take input for the thread they were not drawn for");
        assert_eq!(view.read_with(cx, |v, _| (v.thread().map(str::to_owned), v.placeholder())), (Some("u".to_owned()), None));
    }

    view.update(cx, |v, cx| v.load((100..103).flat_map(turn).collect(), cx));
    cx.run_until_parked();
    let (new, new_reply) = (keys(&view, cx), reply_key(&view, cx));
    assert!(new.iter().all(|k| !old.contains(k)), "the new thread's rows share keys with the last thread's: {new:?}");
    assert!(view.read_with(cx, |v, _| v.body_of(&new_reply).is_some()), "the first frame after the page did not draw its rows");
    assert!(view.read_with(cx, |v, _| v.body_of(&old_reply).is_none()), "the last thread's rows outlived the swap");
    assert!(cx.debug_bounds("row-0").is_some());
    assert!(cx.debug_bounds("transcript-held").is_none() && cx.debug_bounds("transcript-loading").is_none());
}

#[gpui_kit::test]
fn the_loading_line_waits_150ms_with_nothing_to_draw_and_a_second_switch_keeps_the_first_ones_wait(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    let loading = |view: &Entity<ThreadView>, cx: &mut VisualTestContext| {
        let said = view.read_with(cx, |v, _| v.placeholder()) == Some("Loading transcript");
        assert_eq!(said, cx.debug_bounds("transcript-loading").is_some(), "the placeholder and the drawn line disagree");
        said
    };
    // A first open, with no rows before it, says nothing for 150 ms.
    view.update(cx, |v, cx| v.show("t", cx));
    cx.run_until_parked();
    assert!(!loading(&view, cx), "the loading line showed at once");
    cx.executor().advance_clock(Duration::from_millis(149));
    cx.run_until_parked();
    assert!(!loading(&view, cx), "the loading line showed under 150 ms");
    cx.executor().advance_clock(Duration::from_millis(1));
    cx.run_until_parked();
    assert!(loading(&view, cx), "nothing to draw at 150 ms and no loading line");
    view.update(cx, |v, cx| v.load(transcript(3), cx));
    cx.run_until_parked();
    assert!(!loading(&view, cx));

    // A switch holds the rows; a second one 100 ms in keeps the 150 ms the first one started.
    view.update(cx, |v, cx| v.show("u", cx));
    cx.executor().advance_clock(Duration::from_millis(100));
    cx.run_until_parked();
    view.update(cx, |v, cx| v.show("v", cx));
    cx.executor().advance_clock(Duration::from_millis(49));
    cx.run_until_parked();
    assert!(!loading(&view, cx) && cx.debug_bounds("row-0").is_some(), "the rows went under 150 ms from the first switch");
    cx.executor().advance_clock(Duration::from_millis(1));
    cx.run_until_parked();
    assert!(loading(&view, cx), "the second switch started the wait again");
    assert!(view.read_with(cx, |v, _| v.rows().is_empty()) && cx.debug_bounds("row-0").is_none(), "the held rows outlived 150 ms");
    assert!(cx.debug_bounds("transcript-held").is_none());

    // Past the wait, a further switch still has nothing to draw and says so at once.
    view.update(cx, |v, cx| v.show("w", cx));
    cx.run_until_parked();
    assert!(loading(&view, cx), "a switch past the wait hid the loading line");
    view.update(cx, |v, cx| v.load((100..103).flat_map(turn).collect(), cx));
    cx.run_until_parked();
    assert!(!loading(&view, cx) && cx.debug_bounds("row-0").is_some());
}

#[gpui_kit::test]
fn a_new_reply_parses_once(cx: &mut TestAppContext) {
    let (view, cx) = window(cx);
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.load(transcript(3), cx);
        v.scroll_to_end(cx);
    });
    cx.run_until_parked();
    push_turn(&view, 3, cx);
    for _ in 0..3 {
        cx.update(|window, cx| window.simulate_next_frame(cx));
        cx.run_until_parked();
    }
    let key = view.read_with(cx, |v, _| v.rows().iter().rev().find(|r| matches!(r.kind, RowKind::Agent { .. })).unwrap().key.clone());
    let body = view.read_with(cx, |v, _| v.body_of(&key)).expect("the new reply was drawn");
    let text = cx.update(|_, cx| format!("{:?}", body.read(cx).rendered_text()));
    assert!(text.contains("revision: 1,"), "the new reply was parsed again once drawn: {text}");
}

/// `--glass` draws its composer-sized stand-in only where no thread screen docks the real composer, whose own glass
/// it would blur over a second time.
#[gpui_kit::test]
fn the_glass_stand_in_stays_off_a_mounted_thread_screen(cx: &mut TestAppContext) {
    let host = common::Throwaway::start();
    let folder = host.repo("repo");
    host.wsp(&["add", &folder.to_string_lossy()]);
    host.wsp(&["run", "repo", "first", "--agent", "claude"]);
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client =
        runtime.block_on(wsp_host_client::HostClient::connect(&host.state(), &host.env())).unwrap_or_else(|f| panic!("{}", f.message));
    let threads = runtime.block_on(client.threads()).unwrap_or_else(|f| panic!("{}", f.message));
    let first = threads.iter().find(|t| t.prompt.as_deref() == Some("first")).expect("the thread");
    let (workspace_id, thread_id) = (first.workspace_id.clone(), first.thread_id.clone().expect("a thread id"));

    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
        desktop_gpui::screens::app_shell::menu::init(cx);
    });
    let (state, vars) = (host.state(), host.env());
    let mut shell = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let launch = Launch { glass: true, ..Launch::default() };
        let view = cx.new(|cx| Shell::new(state, vars, runtime.clone(), launch, window, cx));
        shell = Some(view.clone());
        Root::new(view, window, cx)
    });
    let shell = shell.unwrap();
    cx.simulate_resize(size(px(1280.), px(800.)));
    let until = |cx: &mut VisualTestContext, what: &str, done: &dyn Fn(&mut VisualTestContext) -> bool| {
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            cx.run_until_parked();
            if done(cx) {
                return;
            }
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(50));
        }
    };
    until(cx, "the host", &|cx| shell.read_with(cx, |s, _| s.connected()));
    assert!(cx.debug_bounds("glass-stand-in").is_some(), "with no thread screen the stand-in shows");

    shell.update_in(cx, |s, window, cx| {
        s.go(Route::Thread { workspace_id: workspace_id.clone(), thread_id: thread_id.clone() }, window, cx)
    });
    until(cx, "the thread screen", &|cx| shell.read_with(cx, |s, _| s.thread_screen().is_some()));
    cx.run_until_parked();
    assert!(cx.debug_bounds("glass-stand-in").is_none(), "the stand-in blurs over the docked composer's own glass");
}
