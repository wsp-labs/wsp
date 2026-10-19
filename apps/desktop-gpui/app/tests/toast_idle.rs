// SPDX-License-Identifier: AGPL-3.0-only
//! A toast raised in a window nothing else redraws: it must size itself and show, with only the frames the window
//! asks for delivered, 16 ms apart (no pointer, no key, no refresh from outside).

use gpui_kit::component::Root;
use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{div, prelude::*, px, size, Context, ElementId, TestAppContext, Window};

use desktop_gpui::ui::toast::Notice;

struct Page;

impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}

#[gpui_kit::test]
fn a_toast_raised_in_an_idle_window_sizes_itself_and_shows(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        desktop_gpui::ui::init(desktop_gpui::ui::Side::Light, cx);
    });
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(cx.new(|_| Page), window, cx));
    cx.simulate_resize(size(px(1280.), px(800.)));
    cx.run_until_parked();
    cx.update(|window, cx| desktop_gpui::ui::notify_in(window, cx, Notice::error("No editor wsp knows is installed on this computer.")));
    cx.run_until_parked();
    let toast = ElementId::NamedInteger("notice".into(), 1);
    for _ in 0..30 {
        cx.executor().advance_clock(std::time::Duration::from_millis(16));
        cx.update(|window, cx| window.simulate_next_frame(cx));
        cx.run_until_parked();
    }
    let shown = cx.update(|window, _| window.try_find(toast.clone()).map(|t| (t.visible(), t.bounds())));
    let Some((visible, bounds)) = shown else { panic!("after 30 frames the toast is still unsized: no notice drawn") };
    assert!(visible && bounds.size.height > px(40.), "the toast stands at {bounds:?}, visible {visible}");
}
