// SPDX-License-Identifier: AGPL-3.0-only
use gpui_kit::component::Root;
use gpui_kit::{prelude::*, px, size, Entity, Modifiers, TestAppContext, VisualTestContext};
use serde_json::json;
use wsp_host_client::protocol::SessionEvent;

use super::fragment_source;
use crate::thread_view::ThreadView;
use crate::transcript::RowKind;

struct Reply(Entity<gpui_kit::base::text::TextViewState>, bool);

impl Render for Reply {
    fn render(&mut self, _: &mut gpui_kit::Window, cx: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        use crate::ui::ActivePalette as _;
        let ground = self.1.then(|| cx.palette().background);
        gpui_kit::div().size_full().child(super::reply(&self.0, false, None, None, ground, cx))
    }
}

/// An agent's reply on the page.
fn reply<'a>(cx: &'a mut TestAppContext, markdown: &str, width: f32) -> &'a mut VisualTestContext {
    reply_on(cx, markdown, width, true)
}

/// A reply on the page where `page`, else on a surface it is not told of.
fn reply_on<'a>(cx: &'a mut TestAppContext, markdown: &str, width: f32, page: bool) -> &'a mut VisualTestContext {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::ui::init(crate::ui::Side::Light, cx);
    });
    let markdown = markdown.to_owned();
    let body = cx.new(|cx| gpui_kit::base::text::TextViewState::markdown(&markdown, cx));
    let reply = cx.new(|_| Reply(body, page));
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(reply.clone(), window, cx));
    cx.simulate_resize(size(px(width), px(800.)));
    cx.run_until_parked();
    cx
}

#[gpui_kit::test]
fn a_web_link_shows_its_address_on_hover_and_another_link_none(cx: &mut TestAppContext) {
    let cx = reply(cx, "Read [the docs](https://wsp.dev/docs/start) first.", 600.);
    let tip = cx.debug_bounds("link-tip").expect("no hover target over the web link");
    cx.simulate_mouse_move(tip.center(), None, Modifiers::none());
    cx.executor().advance_clock(std::time::Duration::from_secs(1));
    cx.run_until_parked();
    assert!(cx.debug_bounds("link-address").is_some(), "the link's address did not show on hover");
    cx.simulate_mouse_move(gpui_kit::point(px(590.), px(790.)), None, Modifiers::none());
    cx.executor().advance_clock(std::time::Duration::from_secs(1));
    cx.run_until_parked();
    assert!(cx.debug_bounds("link-address").is_none(), "the address stayed after the pointer left");
}

/// A reply drawn while it holds its body, and nothing once it lets go.
struct Holder(Option<Entity<gpui_kit::base::text::TextViewState>>);

impl Render for Holder {
    fn render(&mut self, _: &mut gpui_kit::Window, cx: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        gpui_kit::div().size_full().children(self.0.as_ref().map(|body| super::reply(body, false, None, None, None, cx)))
    }
}

#[gpui_kit::test]
fn a_replys_body_is_freed_once_no_view_draws_it(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::ui::init(crate::ui::Side::Light, cx);
    });
    let body = cx.new(|cx| gpui_kit::base::text::TextViewState::markdown("See [the docs](https://wsp.dev) and [^1].\n\n[^1]: A note.", cx));
    let weak = body.downgrade();
    let holder = cx.new(|_| Holder(Some(body)));
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(holder.clone(), window, cx));
    cx.run_until_parked();
    holder.update(cx, |h, cx| {
        h.0 = None;
        cx.notify();
    });
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(weak.upgrade().is_none(), "the reply's TextViewState outlived the view that drew it");
}

#[gpui_kit::test]
fn a_details_block_folds_under_its_summary_and_opens_on_a_press(cx: &mut TestAppContext) {
    let cx = reply(cx, "<details>\n<summary>More <b>here</b></summary>\n\nHidden body.\n\n</details>\n\nAfter.", 600.);
    let summary = cx.debug_bounds("details-summary").expect("the details block drew no summary row");
    assert!(cx.debug_bounds("details-body").is_none(), "a details block starts shut");
    cx.simulate_click(summary.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("details-body").is_some(), "a press on the summary did not open it");
    cx.simulate_click(summary.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("details-body").is_none(), "a second press did not shut it");
}

#[gpui_kit::test]
fn a_details_block_written_open_starts_open(cx: &mut TestAppContext) {
    let cx = reply(cx, "<details open>\n<summary>More</summary>\n\nShown body.\n\n</details>", 600.);
    assert!(cx.debug_bounds("details-body").is_some(), "an open details block started shut");
}

#[gpui_kit::test]
fn a_link_to_a_note_shows_no_address(cx: &mut TestAppContext) {
    let cx = reply(cx, "See [^1].\n\n[^1]: The note.", 600.);
    assert!(cx.debug_bounds("link-tip").is_none(), "a footnote's number took a web link's hover");
}

#[gpui_kit::test]
fn a_wide_table_on_a_surface_the_reply_does_not_know_draws_no_fade(cx: &mut TestAppContext) {
    let head: Vec<String> = (1..=12).map(|n| format!("Column number {n}")).collect();
    let table = format!("| {} |\n|{}\n| {} |", head.join(" | "), "---|".repeat(12), head.join(" | "));
    let cx = reply_on(cx, &table, 500., false);
    assert!(cx.debug_bounds("table-fade-right").is_none(), "a reply with no ground painted the page's colour over its table");
}

#[gpui_kit::test]
fn a_wide_tables_edges_fade_while_more_lies_past_them(cx: &mut TestAppContext) {
    let head: Vec<String> = (1..=12).map(|n| format!("Column number {n}")).collect();
    let table = format!("| {} |\n|{}\n| {} |", head.join(" | "), "---|".repeat(12), head.join(" | "));
    let cx = reply(cx, &table, 500.);
    assert!(cx.debug_bounds("table-fade-left").is_none(), "a table at its start fades its left edge");
    let right = cx.debug_bounds("table-fade-right").expect("no fade on the right edge with more past it");
    assert_eq!(right.size.width, px(24.), "the fade is 1.5 rem wide");
    let over = right.center() - gpui_kit::point(px(100.), px(0.));
    let wheel = |dx: f32| gpui_kit::ScrollWheelEvent {
        position: over,
        delta: gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(dx), px(0.))),
        ..Default::default()
    };
    cx.simulate_event(wheel(-10.));
    cx.run_until_parked();
    let left = cx.debug_bounds("table-fade-left").expect("no fade on the left edge after a scroll");
    assert_eq!(left.size.width, px(10.), "the left fade is as wide as what was scrolled, under 1.5 rem");
    cx.simulate_event(wheel(-100_000.));
    cx.run_until_parked();
    assert!(cx.debug_bounds("table-fade-right").is_none(), "a table at its end fades its right edge");
    assert!(cx.debug_bounds("table-fade-left").is_some());
}

const NOTED: &str = "See [^a] and [^1].\n\n[^1]: The first.\n   [^a]: The other.";

#[test]
fn a_chip_opens_its_file_with_dots_walked() {
    let meta = |code: &str| super::inline_file(code, Some("/r/sub")).expect("a file chip");
    assert_eq!(super::chip_path(&meta("../x.rs")).as_deref(), Some("/r/x.rs"));
    assert_eq!(super::chip_path(&meta("./src/../a.rs:12")).as_deref(), Some("/r/sub/a.rs"));
    assert_eq!(super::chip_path(&meta("/r/sub/a.rs")).as_deref(), Some("/r/sub/a.rs"));
}

#[test]
fn a_fragment_names_a_footnotes_note_or_its_first_reference() {
    let at = |fragment: &str| fragment_source(NOTED, fragment).map(|r| &NOTED[r]);
    assert_eq!(at("user-content-fn-1"), Some("[^1]: The first."));
    assert_eq!(at("user-content-user-content-fn-A"), Some("   [^a]: The other."));
    assert_eq!(at("fnref-1"), Some("[^1]"));
    assert_eq!(fragment_source(NOTED, "fnref-1").map(|r| r.start), NOTED.find("[^1]"));
    assert_eq!(at("setup"), None, "a heading has no id in a reply, as today's has none");
    assert_eq!(at("fn-2"), None);
}

fn event(kind: &str, pos: u64, extra: serde_json::Value) -> SessionEvent {
    let mut v = json!({"type": kind, "workspaceId": "w", "sessionId": "s", "threadId": "t", "turnId": "u1", "pos": pos});
    v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    serde_json::from_value(v).unwrap()
}

/// A reply opening on a footnote's number whose note stands 80 lines further down, in a window too short for both.
fn noted(cx: &mut TestAppContext) -> (Entity<ThreadView>, &mut VisualTestContext) {
    let filler: String = (0..80).map(|n| format!("Line {n}.\n\n")).collect();
    let text = format!("[^1] opens it.\n\n{filler}[^1]: The note.");
    let events = vec![
        event("session.start", 1, json!({"prompt": "go"})),
        event("session.delta", 2, json!({"kind": "text", "text": text, "messageId": "m1"})),
        event("session.done", 3, json!({"result": {"status": "completed", "durationMs": 2_000}})),
        event("session.end", 4, json!({"exitCode": 0, "sawResult": true})),
    ];
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::ui::init(crate::ui::Side::Light, cx);
    });
    let view = cx.new(ThreadView::new);
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(view.clone(), window, cx));
    cx.simulate_resize(size(px(900.), px(500.)));
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.load(events, cx);
        v.scroll_to_top(cx);
    });
    cx.run_until_parked();
    (view, cx)
}

fn top(view: &Entity<ThreadView>, cx: &mut VisualTestContext) -> (usize, gpui_kit::Pixels) {
    view.read_with(cx, |v, _| {
        let at = v.list().logical_scroll_top();
        (at.item_ix, at.offset_in_item)
    })
}

#[gpui_kit::test]
fn a_footnotes_number_scrolls_to_its_note_and_the_notes_mark_back(cx: &mut TestAppContext) {
    let (view, cx) = noted(cx);
    let reply = view.read_with(cx, |v, _| v.rows().iter().position(|r| matches!(r.kind, RowKind::Agent { .. })).expect("the reply"));
    let row: &'static str = Box::leak(format!("row-{reply}").into_boxed_str());
    let start = top(&view, cx);
    // The number is the reply's first glyph, raised to the line's top, inside the reply's `px-1`.
    let at = cx.debug_bounds(row).expect("the reply's row").origin + gpui_kit::point(px(10.), px(2.));
    cx.simulate_click(at, Modifiers::none());
    for _ in 0..3 {
        cx.update(|window, cx| window.simulate_next_frame(cx));
        cx.run_until_parked();
    }
    let moved = top(&view, cx);
    assert!(moved > start, "the footnote's number left the list where it stood: {moved:?}");
    let body = view.read_with(cx, |v, _| v.body_of(&v.rows()[reply].key)).expect("the reply's body");
    cx.update(|_, cx| assert!(super::jump(&body, "user-content-fnref-1", cx), "the note's mark found no number"));
    for _ in 0..3 {
        cx.update(|window, cx| window.simulate_next_frame(cx));
        cx.run_until_parked();
    }
    assert!(top(&view, cx) < moved, "the note's mark did not lead back up");
}
