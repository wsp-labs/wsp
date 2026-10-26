// SPDX-License-Identifier: AGPL-3.0-only
//! The Computer pane's plot drawn by GPUI's real offscreen renderer and read back as pixels: the dot on its newest
//! point. It runs without libtest's harness because AppKit panics unless the platform starts on the main thread.

use std::sync::Arc;

use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::{div, prelude::*, px, size, HeadlessAppContext, Window};

/// One plot, its newest point in the middle, its ring blue on a white page.
struct Plot {
    end: bool,
}

impl Render for Plot {
    fn render(&mut self, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        let line = ui::Sparkline::new(vec![(0., 0.5), (0.5, 0.5)], gpui_kit::red()).no_rules();
        let line = if self.end { line.end(gpui_kit::blue()) } else { line };
        div().size_full().bg(gpui_kit::white()).child(div().w(px(200.)).child(line))
    }
}

/// The plot drawn offscreen, read back as pixels.
fn plotted(cx: &mut HeadlessAppContext, end: bool) -> image::RgbaImage {
    let window: gpui_kit::AnyWindowHandle = cx
        .open_window(size(px(200.), px(96.)), move |window, cx| {
            let plot = cx.new(|_| Plot { end });
            cx.new(|cx| Root::new(plot, window, cx))
        })
        .unwrap()
        .into();
    cx.update_window(window, |_, window, cx| gpui_kit::test::TestWindowExt::render_frame(window, cx)).unwrap();
    cx.run_until_parked();
    cx.capture_screenshot(window).expect("offscreen rendering must be available")
}

fn main() {
    a_live_plot_draws_its_dot_on_the_newest_point();
    println!("test a_live_plot_draws_its_dot_on_the_newest_point ... ok");
}

fn a_live_plot_draws_its_dot_on_the_newest_point() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(ui::UiAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    let (dotted, bare) = (plotted(&mut cx, true), plotted(&mut cx, false));
    let scale = dotted.width() / 200;
    // The newest point: half across, half way up the 96 px plot; the ring stands 4 px above it, clear of the line.
    let ring = |img: &image::RgbaImage| img.get_pixel(100 * scale, 44 * scale).0;
    assert_eq!(ring(&dotted)[..3], [0, 0, 255], "the dot's ring on the newest point");
    assert_eq!(ring(&bare)[..3], [255, 255, 255], "and nothing there without it");
}
