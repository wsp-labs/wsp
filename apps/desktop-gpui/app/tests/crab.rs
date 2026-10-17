// SPDX-License-Identifier: AGPL-3.0-only
//! A walking crab asks for its next frame only while it is drawn inside the window's visible area: one clipped away
//! below the fold asks for none. A list row out of view is not drawn at all, so its crab asks for none either.

use desktop_gpui::ui::status::CrabView;
use desktop_gpui::ui::{self, ActivePalette, Side};
use gpui_kit::component::Root;
use gpui_kit::{div, prelude::*, px, size, AppContext, Context, Entity, TestAppContext, Window};

struct Holder {
    crab: Entity<CrabView>,
    top: f32,
}

impl Render for Holder {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .overflow_hidden()
            .child(div().absolute().top(px(self.top)).left(px(10.)).w(px(16.)).h(px(14.)).child(self.crab.clone()))
    }
}

fn crab_at(cx: &mut TestAppContext, top: f32) -> bool {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    let crab = cx.new(|cx| CrabView::new(cx.palette().status_working));
    let holder = cx.new(|_| Holder { crab: crab.clone(), top });
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(holder, window, cx));
    cx.simulate_resize(size(px(400.), px(300.)));
    // Past the frame the first draw asked for, at whatever size the window opened, to the draws at 400 by 300.
    for _ in 0..3 {
        cx.executor().advance_clock(std::time::Duration::from_secs(1));
        cx.run_until_parked();
    }
    crab.read_with(cx, |c, _| c.walking())
}

#[gpui_kit::test]
fn a_crab_in_view_asks_for_its_next_frame(cx: &mut TestAppContext) {
    assert!(crab_at(cx, 100.));
}

#[gpui_kit::test]
fn a_crab_below_the_fold_asks_for_none(cx: &mut TestAppContext) {
    assert!(!crab_at(cx, 1000.));
}
