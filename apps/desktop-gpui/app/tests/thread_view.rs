// SPDX-License-Identifier: AGPL-3.0-only
use desktop_gpui::thread_view::ThreadView;
use gpui_kit::component::Root;
use gpui_kit::{px, size, AppContext, Entity, ListOffset, Pixels, TestAppContext, VisualTestContext};
use wsp_host_client::protocol::SessionEvent;

const TURNS: usize = 300;

/// Two rows, the person's message and a reply with a fenced code block; the turn never settles, so no fold hides a row.
fn turn(turn: usize) -> [SessionEvent; 2] {
    let pos = 2 * turn as u64 + 1;
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
    [serde_json::from_value(start).unwrap(), serde_json::from_value(reply).unwrap()]
}

fn transcript(turns: usize) -> Vec<SessionEvent> {
    (0..turns).flat_map(turn).collect()
}

fn window(cx: &mut TestAppContext) -> (Entity<ThreadView>, &mut VisualTestContext) {
    cx.update(gpui_kit::init);
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
