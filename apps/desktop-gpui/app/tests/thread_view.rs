// SPDX-License-Identifier: AGPL-3.0-only
use desktop_gpui::thread_view::ThreadView;
use gpui::{px, size, AppContext, TestAppContext};
use gpui_component::Root;
use wsp_host_client::protocol::SessionEvent;

const TURNS: usize = 300;

/// Two rows per turn, the person's message and the reply; no turn settles, so no fold hides a row.
fn transcript() -> Vec<SessionEvent> {
    let mut events = Vec::new();
    for turn in 0..TURNS {
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
            ("text".to_owned(), format!("Answer {turn}\n\n- one\n- `two`").into()),
        ]);
        events.push(serde_json::from_value(start).unwrap());
        events.push(serde_json::from_value(reply).unwrap());
    }
    events
}

#[gpui::test]
fn the_list_builds_only_the_rows_on_screen(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let view = cx.new(ThreadView::new);
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(view.clone(), window, cx));
    cx.simulate_resize(size(px(900.), px(600.)));
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.load(transcript(), cx);
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
        v.list().scroll_to(gpui::ListOffset::default());
        cx.notify();
    });
    cx.run_until_parked();
    assert_eq!(view.read_with(cx, |v, _| v.parsed()), below, "scrolling back parsed a row again");
    println!("{rows} rows: first layout built {built}, a redraw {again}, parsed {below} after scrolling down and back");
}
