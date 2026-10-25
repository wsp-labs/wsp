// SPDX-License-Identifier: AGPL-3.0-only
//! A file chip's tooltip sets its path in the machine's mono face, read on the real text system. It runs without
//! libtest's harness because AppKit panics unless the platform starts on the main thread.

use std::sync::Arc;

use desktop_gpui::screens::thread_view::markdown::path_text;
use desktop_gpui::ui::{self, theme, Side};
use gpui_kit::base::TestSupportExt as _;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{prelude::*, Context, HeadlessAppContext, Window};

/// The path a chip's tooltip shows, in a window of its own, as wide as the machine's fonts set it.
struct Tip(&'static str);

impl Render for Tip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The machine's mono face, read here: gpui-kit picks the theme's once per process, and a test with no fonts may
        // have picked it first.
        let installed = cx.text_system().all_font_names();
        let mono = theme::first_installed(&theme::MONO, &installed, cfg!(target_os = "macos")).expect("a mono face");
        gpui_kit::component::h_flex().child(path_text(self.0.into(), mono).test_support())
    }
}

fn main() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(ui::UiAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    let mut width = |path: &'static str| {
        let options = gpui_kit::WindowOptions { focus: false, show: false, ..Default::default() };
        let (handle, _) = cx.update(|cx| gpui_kit::open_window(options, cx, move |_, cx| cx.new(|_| Tip(path)))).unwrap();
        cx.update_window(handle, |_, window, cx| TestWindowExt::render_frame(window, cx)).unwrap();
        cx.update_window(handle, |_, window, _| window.find("file-chip-tip").bounds().size.width).unwrap()
    };
    assert_eq!(width("/w/iiiiiiii.rs"), width("/w/mmmmmmmm.rs"), "the path's tooltip is not in mono");
    println!("test a_chips_tooltip_shows_its_path_in_mono ... ok");
}
