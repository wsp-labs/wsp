// SPDX-License-Identifier: AGPL-3.0-only
//! Where the Pull request pane's small pieces put their parts, as today's CSS puts them: a face's initial in the line
//! its row gives it (`Face` in `pull-request/parts.tsx`, a grid whose one row grows past a ring smaller than the line),
//! and a tab's count after its word (`TabLabel` in `PullRequestSurface.tsx`).

use desktop_gpui::screens::pr_pane::pull_request::parts;
use desktop_gpui::ui::{self, Segmented, Side};
use gpui_kit::{div, prelude::*, px, Bounds, Context, Pixels, TestAppContext, VisualTestContext, Window};

struct Face(Option<Pixels>);

impl Render for Face {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().p_4().child(parts::face("octo", px(16.), false, self.0, cx))
    }
}

struct Tabs;

impl Render for Tabs {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().w(px(400.)).child(
            Segmented::new("tabs", 0)
                .fill()
                .segment_size((px(13.), px(13.)), px(8.))
                .glyph_size(px(16.), px(6.))
                .count_segment("Conversation", "4")
                .segment("Commits"),
        )
    }
}

fn init(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
}

fn bounds(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} is drawn"))
}

/// The face's ring and its initial's line box, in a window drawing that one face.
fn face(line: Option<Pixels>, cx: &mut TestAppContext) -> (Bounds<Pixels>, Bounds<Pixels>) {
    init(cx);
    let (_, cx) = cx.add_window_view(|_, _| Face(line));
    cx.run_until_parked();
    (bounds(cx, "pr-face"), bounds(cx, "pr-face-initial"))
}

#[gpui_kit::test]
fn a_face_in_a_line_taller_than_its_ring_drops_its_initial_from_the_top(cx: &mut TestAppContext) {
    let (ring, initial) = face(Some(px(20.)), cx);
    assert_eq!(initial.top() - ring.top(), px(1.), "the initial's line starts inside the ring's 1 px edge");
    assert_eq!(initial.size.height, px(20.), "and is the row's whole 20 px line, so the initial sits 3 px low");
}

#[gpui_kit::test]
fn a_face_in_the_page_s_own_line_centres_its_initial(cx: &mut TestAppContext) {
    let (ring, initial) = face(None, cx);
    assert_eq!(initial.top() - ring.top(), px(1.));
    assert_eq!(initial.size.height, px(14.), "1.5 of 8 px fits the ring's 14 px, so the initial stands centred");
}

#[gpui_kit::test]
fn a_tab_s_count_stands_after_its_word_in_the_muted_mono(cx: &mut TestAppContext) {
    init(cx);
    let (_, cx) = cx.add_window_view(|_, _| Tabs);
    cx.run_until_parked();
    let count = bounds(cx, "segment-count");
    let tabs = bounds(cx, "segmented");
    assert!(count.left() > tabs.left() + px(100.) && count.right() < tabs.center().x, "the count ends the first tab's words: {count:?}");
}
