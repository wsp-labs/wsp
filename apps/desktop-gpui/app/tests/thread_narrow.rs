// SPDX-License-Identifier: AGPL-3.0-only
//! The transcript under 640 px window width, as list.tsx pads it: `px-3` inside the scrollbar's 6 px gutter, `sm:px-5`
//! from 640 px up.

use desktop_gpui::thread_view::ThreadView;
use gpui_kit::component::Root;
use gpui_kit::{px, size, AppContext, TestAppContext, VisualTestContext};
use wsp_host_client::protocol::SessionEvent;

fn turn(n: usize) -> Vec<SessionEvent> {
    let pos = 4 * n as u64 + 1;
    let event = |kind: &str, at: u64, extra: serde_json::Value| {
        let mut v =
            serde_json::json!({"workspaceId": "w", "sessionId": "s", "threadId": "t", "turnId": format!("u{n}"), "type": kind, "pos": at});
        v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        serde_json::from_value::<SessionEvent>(v).unwrap()
    };
    vec![
        event("session.start", pos, serde_json::json!({"prompt": format!("Question {n}")})),
        event("session.delta", pos + 1, serde_json::json!({"kind": "text", "text": format!("Answer {n}.")})),
        event("session.done", pos + 2, serde_json::json!({"result": {"status": "completed"}})),
        event("session.end", pos + 3, serde_json::json!({"exitCode": 0, "sawResult": true})),
    ]
}

fn at_width(cx: &mut VisualTestContext, width: f32) {
    cx.simulate_resize(size(px(width), px(600.)));
    for _ in 0..3 {
        cx.run_until_parked();
        cx.update(|window, _| window.refresh());
    }
    cx.run_until_parked();
}

#[gpui_kit::test]
fn under_640_px_the_transcript_keeps_12_px_sides(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
    });
    let view = cx.new(ThreadView::new);
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(view.clone(), window, cx));
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.load((0..3).flat_map(turn).collect(), cx);
    });
    at_width(cx, 600.);
    let row = cx.debug_bounds("row-0").expect("the first row was not drawn");
    assert_eq!((row.left(), row.size.width), (px(18.), px(600. - 36.)), "6 px of gutter and `px-3` a side: {row:?}");
    at_width(cx, 700.);
    let row = cx.debug_bounds("row-0").expect("the first row was not drawn");
    assert_eq!((row.left(), row.size.width), (px(26.), px(700. - 52.)), "6 px of gutter and `sm:px-5` a side: {row:?}");
}
